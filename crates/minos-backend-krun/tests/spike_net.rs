//! Egress and vsock spikes (02-08-PLAN.md): SPIKE-02 (the guest has no network path at all) and SPIKE-03 (a
//! guest-to-host stream over the vsock route, read through a byte cap), plus the K4 throughput record.
//!
//! The launch is 02-05's `LaunchConfig` and 02-03's `minos-vmm`, unchanged: `msb_krun`'s inet hijack stays at its
//! default (off), no `tcp_listen_remap` is configured, there is no network device, and the only vsock route is the
//! `unix_connect` on port 1024. libkrunfw 5.6.2 builds TSI into the guest kernel, so the egress result rests
//! entirely on the VMM never enabling the hijack; `egress_fails_closed` is the live test of that.
//!
//! Every test is ignored and needs a VM host. A missing variable or an unentitled helper is an error carrying the
//! setup commands below, never a skip (D-02). Phase 5 promotes `egress_fails_closed` to the permanent KRUN-03
//! egress test. Guest bytes reach the terminal only through `GuestBytes::excerpt`, the decoded status frame
//! (printed with control and bidi characters escaped), and pattern and length comparisons.
//!
//! Run: `cargo test -p minos-backend-krun --test spike_net -- --ignored --nocapture --test-threads=1`.

use std::error::Error;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::time::Duration;

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles, Launch, MAX_FRAME_BYTES, VmRun};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// 02-06-PLAN.md E-1 and E-4, plus `MINOS_SPIKE_HOST_IP`: what to run and set when a prerequisite is missing.
const SETUP: &str = "\
build and sign first:
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)
  MINOS_SPIKE_HOST_IP=$(ipconfig getifaddr en0)   (egress_fails_closed only; the active interface from route -n get default)";

/// The guest's vsock port for the stream and the status frame; the only port with a route.
const VSOCK_PORT: u32 = 1024;
/// A vsock port with no route: a guest connect to it must fail.
const UNMAPPED_VSOCK_PORT: u32 = 1025;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the supervisor reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// Most characters of any one excerpt (at least the console cap, so nothing is cut before the digest).
const EXCERPT_CHARS: usize = 1_048_576;
/// Console lines printed from the end of the log (the evidence protocol).
const TAIL_LINES: usize = 40;
/// Console lines printed wherever they appear.
const CONSOLE_MARKERS: [&str; 3] = ["MINOS-", "Kernel panic", "minos-vmm"];
/// Per-target timeout of the guest's `connect` step.
const CONNECT_TIMEOUT_MS: u64 = 3000;
/// Port the IPv6 targets use when the IPv6 canary cannot be bound (discard; nothing listens).
const DISCARD_PORT: u16 = 9;
/// The host cap on the stream connection in the at-cap and over-cap tests.
const STREAM_CAP: u64 = 1_048_576;
/// The host cap on the status-frame connection.
const FRAME_CAP: u64 = 1_048_576;
/// Bytes the over-cap test streams: four times the cap.
const OVER_CAP_BYTES: u64 = 4_194_304;
/// Bytes the throughput test streams (K4).
const THROUGHPUT_BYTES: u64 = 67_108_864;
/// The host cap on the stream connection in the throughput test.
const THROUGHPUT_CAP: u64 = 134_217_728;
/// The pattern period the guest uses (`minos-guest` `pattern.rs`).
const PATTERN_MODULUS: u64 = 251;

/// Byte `index` of the guest's stream pattern: `index % 251`.
fn pattern(index: u64) -> u8 {
    u8::try_from(index % PATTERN_MODULUS).unwrap_or(0)
}

/// An absolute path from a required environment variable. Missing or relative is an error with the setup text.
fn env_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {SETUP}"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path; {SETUP}").into());
    }
    Ok(path)
}

/// The host's LAN address from `MINOS_SPIKE_HOST_IP`.
fn host_ip() -> TestResult<IpAddr> {
    let value = std::env::var("MINOS_SPIKE_HOST_IP")
        .map_err(|_| format!("MINOS_SPIKE_HOST_IP is not set; {SETUP}"))?;
    value
        .parse()
        .map_err(|_| format!("MINOS_SPIKE_HOST_IP is not an IP address; {SETUP}").into())
}

/// The canonical libkrunfw path from `MINOS_KRUNFW` and its SHA-256.
fn krunfw() -> TestResult<(PathBuf, String)> {
    let path = std::fs::canonicalize(env_path("MINOS_KRUNFW")?)?;
    let sha256 = supervise::sha256_file(&path)?;
    Ok((path, sha256))
}

/// `MINOS_VMM`, which must carry the hypervisor entitlement.
fn signed_vmm() -> TestResult<PathBuf> {
    let vmm = env_path("MINOS_VMM")?;
    if !supervise::has_hypervisor_entitlement(&vmm)? {
        return Err(format!(
            "{} lacks com.apple.security.hypervisor (cargo build relinks it without one); {SETUP}",
            vmm.display()
        )
        .into());
    }
    Ok(vmm)
}

/// Host-side settings for one launch.
struct Limits {
    max_duration_ms: u64,
    host_deadline: Duration,
    conn_caps: Vec<u64>,
}

/// Launches `vmm` once in root-disk mode for `job_json` in a fresh job directory and returns what it left behind.
fn launch(vmm: &Path, job_json: &str, limits: Limits) -> TestResult<VmRun> {
    let guest = std::fs::read(env_path("MINOS_GUEST")?)?;
    let image = minimal_root_image(&guest)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(job_json.as_bytes())?;
    supervise::write_disks(&files, image.bytes(), &input, SCRATCH_CAP_BYTES)?;
    let listener = supervise::bind_output(&files)?;
    let (krunfw, krunfw_sha256) = krunfw()?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw, krunfw_sha256),
        Boot::RootDisk,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        limits.max_duration_ms,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let argv = [vmm, files.config()];
    let run = supervise::run(
        Launch::for_config(argv, &files, listener, &config)
            .host_deadline(limits.host_deadline)
            .conn_caps(limits.conn_caps),
    )?;
    Ok(run)
}

/// `text` with every control character except newline, and every bidi override, written as `\u{..}`.
fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
        if (c.is_control() && c != '\n') || bidi {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// The last [`TAIL_LINES`] lines plus every earlier line containing a [`CONSOLE_MARKERS`] entry, with the count
/// of lines left out.
fn console_digest(text: &str) -> String {
    let total = text.lines().count();
    let tail_start = total.saturating_sub(TAIL_LINES);
    let mut out = String::new();
    let mut omitted = 0_usize;
    for (index, line) in text.lines().enumerate() {
        if index >= tail_start || CONSOLE_MARKERS.iter().any(|marker| line.contains(marker)) {
            out.push_str(line);
            out.push('\n');
        } else {
            omitted = omitted.saturating_add(1);
        }
    }
    format!("[{total} lines; {omitted} omitted: not in the last {TAIL_LINES} and no marker]\n{out}")
}

/// Prints everything a run left behind, so the test output is the evidence. A connection that is not a status
/// frame is described by its length, overflow flag, pattern match, and digest, never by its bytes.
fn print_run(label: &str, run: &VmRun) -> TestResult {
    println!(
        "== {label}: exit_code {:?}, signal {:?}, killed_at_deadline {}, elapsed {:?}, connections {}",
        run.exit_code(),
        run.signal(),
        run.killed_at_deadline(),
        run.elapsed(),
        run.connections().len()
    );
    println!(
        "-- console ({} bytes, overflowed {}):\n{}",
        run.console().len(),
        run.console().overflowed(),
        console_digest(&run.console().excerpt(EXCERPT_CHARS))
    );
    println!(
        "-- minos-vmm stderr ({} bytes):\n{}",
        run.vmm_stderr().len(),
        run.vmm_stderr().excerpt(EXCERPT_CHARS)
    );
    println!(
        "-- minos-vmm stdout ({} bytes):\n{}",
        run.vmm_stdout().len(),
        run.vmm_stdout().excerpt(EXCERPT_CHARS)
    );
    for (index, connection) in run.connections().iter().enumerate() {
        match connection.status_frame(MAX_FRAME_BYTES) {
            Ok(frame) => println!(
                "-- connection {index} ({} bytes), frame:\n{}",
                connection.len(),
                printable(&serde_json::to_string_pretty(&frame)?)
            ),
            Err(error) => println!(
                "-- connection {index} ({} bytes, overflowed {}, matches pattern {}, sha256 {}): not a frame: {error}",
                connection.len(),
                connection.overflowed(),
                connection.matches_pattern(pattern),
                connection.sha256_hex()
            ),
        }
    }
    println!("== end {label}");
    Ok(())
}

/// The status frame carried by connection `index`.
fn frame_at(run: &VmRun, index: usize) -> TestResult<Value> {
    let connection = run
        .connections()
        .get(index)
        .ok_or_else(|| format!("no connection {index}"))?;
    Ok(connection.status_frame(MAX_FRAME_BYTES)?)
}

/// The result object of the first step named `key` in a frame.
fn step<'a>(frame: &'a Value, key: &str) -> TestResult<&'a Value> {
    frame
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.iter().find_map(|step| step.get(key)))
        .ok_or_else(|| format!("the frame has no {key} result").into())
}

/// The string array at `key` in `object`.
fn strings<'a>(object: &'a Value, key: &str) -> TestResult<Vec<&'a str>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} is not an array"))?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| format!("{key} holds a non-string").into())
        })
        .collect()
}

/// The unsigned integer at `key` in `object`.
fn number(object: &Value, key: &str) -> TestResult<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{key} is not an unsigned integer").into())
}

/// A non-blocking canary listener on `addr`.
fn canary(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Accepts every pending connection on a non-blocking canary and returns how many there were. A connection the
/// peer aborted before the accept still counts: something reached the listener.
fn drain(listener: &TcpListener) -> TestResult<usize> {
    let mut count = 0_usize;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                drop(stream);
                count = count.saturating_add(1);
            }
            Err(error) if error.kind() == ErrorKind::ConnectionAborted => {
                count = count.saturating_add(1);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(count),
            Err(error) => return Err(error.into()),
        }
    }
}

/// The job that streams `bytes` of the pattern on the job's vsock port, then sends the frame and powers off.
fn stream_job(bytes: u64) -> String {
    json!({
        "v": 1,
        "vsock_port": VSOCK_PORT,
        "steps": [{"vsock_stream": {"bytes": bytes}}],
        "finish": "poweroff",
    })
    .to_string()
}

/// Prints and returns the guest's `vsock_stream` result from the frame on connection `index`.
fn stream_result(run: &VmRun, index: usize) -> TestResult<Value> {
    let frame = frame_at(run, index)?;
    let result = step(&frame, "vsock_stream")?.clone();
    println!(
        "guest vsock_stream: requested {}, written {}, error {}, raw_os_error {}",
        result.get("requested").unwrap_or(&Value::Null),
        result.get("written").unwrap_or(&Value::Null),
        printable(&result.get("error").unwrap_or(&Value::Null).to_string()),
        result.get("raw_os_error").unwrap_or(&Value::Null)
    );
    Ok(result)
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, MINOS_SPIKE_HOST_IP; see 02-06-PLAN.md E-1 and 02-08-PLAN.md"]
fn egress_fails_closed() -> TestResult {
    let vmm = signed_vmm()?;
    let host = host_ip()?;

    let v4 = canary(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    let v4_port = v4.local_addr()?.port();
    let v6 = match canary(SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))) {
        Ok(listener) => Some(listener),
        Err(error) => {
            println!("IPv6 canary bind failed: {error}; IPv6 targets use port {DISCARD_PORT}");
            None
        }
    };
    let v6_port = match &v6 {
        Some(listener) => listener.local_addr()?.port(),
        None => DISCARD_PORT,
    };
    let host_port = if host.is_ipv4() { v4_port } else { v6_port };
    println!("canaries: IPv4 0.0.0.0:{v4_port}, IPv6 [::]:{v6_port}");

    let targets = [
        "1.1.1.1:443".to_owned(),
        "8.8.8.8:53".to_owned(),
        SocketAddr::from((Ipv4Addr::LOCALHOST, v4_port)).to_string(),
        SocketAddr::new(host, host_port).to_string(),
        SocketAddr::from((Ipv6Addr::LOCALHOST, v6_port)).to_string(),
        "[2606:4700:4700::1111]:443".to_owned(),
    ];
    let job = json!({
        "v": 1,
        "vsock_port": VSOCK_PORT,
        "steps": [
            {"report_boot": {}},
            {"connect": {"targets": targets, "timeout_ms": CONNECT_TIMEOUT_MS}},
            {"vsock_connect": {"port": UNMAPPED_VSOCK_PORT}},
        ],
        "finish": "poweroff",
    })
    .to_string();

    let run = launch(
        &vmm,
        &job,
        Limits {
            max_duration_ms: 60_000,
            host_deadline: Duration::from_secs(90),
            conn_caps: Vec::new(),
        },
    )?;
    let v4_count = drain(&v4)?;
    let v6_count = match &v6 {
        Some(listener) => drain(listener)?,
        None => 0,
    };
    print_run("egress_fails_closed", &run)?;

    let frame = match run.connections() {
        [connection] => connection.status_frame(MAX_FRAME_BYTES)?,
        connections => {
            return Err(
                format!("expected exactly one connection, got {}", connections.len()).into(),
            );
        }
    };
    let attempts = step(&frame, "connect")?
        .get("results")
        .and_then(Value::as_array)
        .ok_or("connect results is not an array")?;
    for attempt in attempts {
        println!(
            "connect {}: ok {}, error {}, raw_os_error {}",
            printable(&attempt.get("target").unwrap_or(&Value::Null).to_string()),
            attempt.get("ok").unwrap_or(&Value::Null),
            printable(&attempt.get("error").unwrap_or(&Value::Null).to_string()),
            attempt.get("raw_os_error").unwrap_or(&Value::Null)
        );
    }
    let vsock = step(&frame, "vsock_connect")?;
    println!(
        "vsock_connect port {}: ok {}, error {}, raw_os_error {}",
        vsock.get("port").unwrap_or(&Value::Null),
        vsock.get("ok").unwrap_or(&Value::Null),
        printable(&vsock.get("error").unwrap_or(&Value::Null).to_string()),
        vsock.get("raw_os_error").unwrap_or(&Value::Null)
    );
    println!("canary connections accepted: IPv4 {v4_count}, IPv6 {v6_count}");
    let boot = step(&frame, "report_boot")?;
    let ifaces = strings(boot, "net_ifaces")?;
    let devices = strings(boot, "virtio_devices")?;
    println!(
        "net_ifaces {}; virtio_devices {}",
        printable(&format!("{ifaces:?}")),
        printable(&format!("{devices:?}"))
    );

    assert_eq!(run.exit_code(), Some(0));
    assert!(!run.killed_at_deadline());
    let reported: Vec<&str> = attempts
        .iter()
        .filter_map(|attempt| attempt.get("target").and_then(Value::as_str))
        .collect();
    assert_eq!(
        reported, targets,
        "the guest did not report every target in order"
    );
    for attempt in attempts {
        assert_eq!(
            attempt.get("ok"),
            Some(&Value::Bool(false)),
            "a guest TCP connect succeeded: {}",
            printable(&attempt.to_string())
        );
    }
    assert_eq!(
        vsock.get("ok"),
        Some(&Value::Bool(false)),
        "the guest connected to unmapped vsock port {UNMAPPED_VSOCK_PORT}"
    );
    assert_eq!(v4_count, 0, "the IPv4 canary accepted a connection");
    assert_eq!(v6_count, 0, "the IPv6 canary accepted a connection");
    assert!(
        ifaces.is_empty() || ifaces == ["lo"],
        "network interfaces: {}",
        printable(&format!("{ifaces:?}"))
    );
    assert!(
        !devices.iter().any(|device| device.ends_with(":0x0001")),
        "a virtio-net device (id 0x0001) is present"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn vsock_stream_within_cap() -> TestResult {
    let vmm = signed_vmm()?;
    let conn_caps = vec![STREAM_CAP, FRAME_CAP];
    let run = launch(
        &vmm,
        &stream_job(STREAM_CAP),
        Limits {
            max_duration_ms: 60_000,
            host_deadline: Duration::from_secs(90),
            conn_caps,
        },
    )?;
    print_run("vsock_stream_within_cap", &run)?;
    let result = stream_result(&run, 1)?;

    assert_eq!(run.exit_code(), Some(0));
    assert_eq!(run.connections().len(), 2);
    let stream = run.connections().first().ok_or("no stream connection")?;
    assert_eq!(u64::try_from(stream.len())?, STREAM_CAP);
    assert!(!stream.overflowed());
    assert!(stream.matches_pattern(pattern));
    assert_eq!(number(&result, "written")?, STREAM_CAP);
    assert_eq!(result.get("error"), Some(&Value::Null));
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn vsock_stream_over_cap() -> TestResult {
    let vmm = signed_vmm()?;
    let conn_caps = vec![STREAM_CAP, FRAME_CAP];
    let run = launch(
        &vmm,
        &stream_job(OVER_CAP_BYTES),
        Limits {
            max_duration_ms: 60_000,
            host_deadline: Duration::from_secs(90),
            conn_caps,
        },
    )?;
    print_run("vsock_stream_over_cap", &run)?;
    let result = stream_result(&run, 1)?;

    assert_eq!(run.exit_code(), Some(0));
    let stream = run.connections().first().ok_or("no stream connection")?;
    assert_eq!(u64::try_from(stream.len())?, STREAM_CAP);
    assert!(stream.overflowed());
    assert!(stream.matches_pattern(pattern));
    assert!(number(&result, "written")? < OVER_CAP_BYTES);
    assert!(
        matches!(result.get("error"), Some(Value::String(_))),
        "the guest saw no write error"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn vsock_throughput_64mib() -> TestResult {
    let vmm = signed_vmm()?;
    let conn_caps = vec![THROUGHPUT_CAP, FRAME_CAP];
    let run = launch(
        &vmm,
        &stream_job(THROUGHPUT_BYTES),
        Limits {
            max_duration_ms: 120_000,
            host_deadline: Duration::from_secs(150),
            conn_caps,
        },
    )?;
    print_run("vsock_throughput_64mib", &run)?;
    let result = stream_result(&run, 1)?;
    let seconds = run.elapsed().as_secs_f64();
    let mebibytes = f64::from(u32::try_from(THROUGHPUT_BYTES / 1_048_576)?);
    println!(
        "throughput: {THROUGHPUT_BYTES} bytes, elapsed {:?} (spawn to exit, boot and power-off included), {:.1} MiB/s over the whole run",
        run.elapsed(),
        mebibytes / seconds
    );

    assert_eq!(run.exit_code(), Some(0));
    let stream = run.connections().first().ok_or("no stream connection")?;
    assert_eq!(u64::try_from(stream.len())?, THROUGHPUT_BYTES);
    assert!(!stream.overflowed());
    assert!(stream.matches_pattern(pattern));
    assert_eq!(number(&result, "written")?, THROUGHPUT_BYTES);
    assert_eq!(result.get("error"), Some(&Value::Null));
    Ok(())
}
