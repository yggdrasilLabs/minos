//! Wave 0 proof (05-03-PLAN.md, GUEST-04, S03-ANSWER): the authenticated 4 KiB chunk protocol of
//! `minos_core::chunk` carries the 16 MiB output cap from a real guest under the deny-default Seatbelt profile, and
//! its throughput is measured, which S03-ANSWER asked for.
//!
//! The guest's spike-only `chunk_stream` step sends a pattern payload with `minos_core::chunk::ChunkStream`, one
//! short vsock connection per chunk (at most 4144 bytes, under the 8192-byte single-connection limit of the pinned
//! `msb_krun_devices`, 02-SPIKE-EVIDENCE.md DIAG-01). The host accepts on the listener, waiting with
//! `rustix::event::poll` (100 ms) until the helper exits, reads each connection to EOF through
//! `MAX_CHUNK_BYTES + 1` with a 5 s per-connection deadline, and routes `MINOSCK1` connections to a
//! `minos_core::chunk::Reassembler` and the `MINOSST1` connection to the status-frame decoder. The token is a fixed
//! test value (32 bytes of 0x5a) written in the job; a production token never is.
//!
//! Every test is ignored; a missing variable or an unentitled helper is an error carrying the setup commands, never a
//! skip. Guest-derived text is printed only with control and format characters escaped.
//!
//! Run: `cargo test -p minos-backend-krun --test wave0_chunks -- --ignored --nocapture --test-threads=1`.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles};
use minos_core::chunk::{
    CHUNK_MAGIC, ChunkFault, ChunkToken, KIND_END, MAX_CHUNK_BYTES, Reassembler,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{Mode, OFlags};
use rustix::process::{Pid, Signal, kill_process_group};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to run and set when a prerequisite is missing (CLAUDE.md Commands).
const SETUP: &str = "\
build and sign first:
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib";

/// The Seatbelt launcher.
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
/// The Homebrew symlink to libkrunfw. The profile lists it beside its Cellar target (`MINOS_KRUNFW`).
const KRUNFW_LINK: &str = "/opt/homebrew/lib/libkrunfw.5.dylib";
/// The profile, relative to this crate's manifest directory.
const PROFILE: &str = "../minos-vmm/minos-vmm.sb";

/// The guest's vsock port for chunks and the status frame; the jobs below name the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the test reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 512 MiB, room for the 16 MiB payload in guest memory.
const MACHINE: Machine = Machine::new(1, 512);
/// The helper's own deadline.
const MAX_DURATION_MS: u64 = 120_000;
/// The host's deadline: the helper's plus the 30 s grace.
const HOST_DEADLINE: Duration = Duration::from_secs(150);
/// The reassembler's byte cap: 17 MiB, above the 16 MiB stream.
const MAX_TOTAL: u64 = 17 * 1024 * 1024;
/// Most bytes kept from the helper's stdout and stderr.
const VMM_STDIO_CAP: u64 = 65_536;
/// Longest time spent reading one vsock connection.
const CONNECTION_DEADLINE: Duration = Duration::from_secs(5);
/// How long one listener poll waits before the accept thread checks whether the helper exited.
const ACCEPT_POLL: Duration = Duration::from_millis(100);
/// The fixed test token: 32 bytes of 0x5a.
const TOKEN_BYTE: u8 = 0x5a;
/// Offset of the kind byte in a chunk header: magic (8), token (32), seq (4).
const KIND_OFFSET: usize = 44;
/// The S03-ANSWER selection bound for 16 MiB, from the first chunk accept to the END (05-03-PLAN.md Task 3).
const WALL_TIME_BOUND: Duration = Duration::from_secs(15);

/// The status frame magic, version, and payload cap (`minos-guest` `report.rs`).
const FRAME_MAGIC: &[u8; 8] = b"MINOSST1";
const FRAME_VERSION: u32 = 1;
const FRAME_MAX_BYTES: u64 = 1_048_576;

/// The guest's pattern: byte `i` is `i % 251` (`minos-guest` `pattern.rs`).
fn pattern(len: usize) -> Vec<u8> {
    (0_u64..)
        .take(len)
        .map(|index| u8::try_from(index % 251).unwrap_or(0))
        .collect()
}

/// The job: one `chunk_stream` of `bytes` with the test token, then power off.
fn chunk_job(bytes: u64) -> String {
    let token = base16ct::lower::encode_string(&[TOKEN_BYTE; 32]);
    format!(
        r#"{{"v":1,"vsock_port":{VSOCK_PORT},"steps":[{{"chunk_stream":{{"bytes":{bytes},"token":"{token}"}}}}],"finish":"poweroff"}}"#
    )
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

/// The canonical libkrunfw path from `MINOS_KRUNFW` and its SHA-256.
fn krunfw() -> TestResult<(PathBuf, String)> {
    let path = fs::canonicalize(env_path("MINOS_KRUNFW")?)?;
    let sha256 = supervise::sha256_file(&path)?;
    Ok((path, sha256))
}

/// The canonical `MINOS_VMM`, which must carry the hypervisor entitlement.
fn signed_vmm() -> TestResult<PathBuf> {
    let vmm = fs::canonicalize(env_path("MINOS_VMM")?)?;
    if !supervise::has_hypervisor_entitlement(&vmm)? {
        return Err(format!(
            "{} lacks com.apple.security.hypervisor (cargo build relinks it without one); {SETUP}",
            vmm.display()
        )
        .into());
    }
    Ok(vmm)
}

/// The guest binary named by `MINOS_GUEST`.
fn guest() -> TestResult<Vec<u8>> {
    Ok(fs::read(env_path("MINOS_GUEST")?)?)
}

/// The canonical profile path.
fn profile() -> TestResult<PathBuf> {
    Ok(fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(PROFILE),
    )?)
}

/// `path` with its parent directory canonicalized and its last component kept as written.
fn canonical_entry(path: &Path) -> TestResult<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    Ok(fs::canonicalize(parent)?.join(name))
}

/// `sandbox-exec -f <profile> -D NAME=value ... <program> <program_args>...` as an argv array.
fn seatbelt_argv(
    profile: &Path,
    params: &BTreeMap<&'static str, PathBuf>,
    program: &Path,
    program_args: &[&OsStr],
) -> Vec<OsString> {
    let mut argv = vec![
        OsString::from(SANDBOX_EXEC),
        OsString::from("-f"),
        profile.as_os_str().to_owned(),
    ];
    for (name, value) in params {
        let mut define = OsString::from(*name);
        define.push("=");
        define.push(value.as_os_str());
        argv.push(OsString::from("-D"));
        argv.push(define);
    }
    argv.push(program.as_os_str().to_owned());
    argv.extend(program_args.iter().map(|arg| (*arg).to_owned()));
    argv
}

/// Prints an argv, one word per line, so the evidence shows exactly what ran.
fn print_argv(label: &str, argv: &[OsString]) {
    println!("== {label} argv:");
    for word in argv {
        println!("  {}", word.to_string_lossy());
    }
}

/// Lossy text of `bytes` with every control character except newline and tab, and every invisible format
/// character that can hide or reorder text, written as `\u{..}`.
fn printable(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let format = matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        );
        if (c.is_control() && c != '\n' && c != '\t') || format {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// Waits up to `timeout` for `fd` to be readable. `true` when it is (or has hung up).
fn readable(fd: &impl std::os::fd::AsFd, timeout: Duration) -> TestResult<bool> {
    let timeout = Timespec::try_from(timeout)?;
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    loop {
        match poll(&mut fds, Some(&timeout)) {
            Ok(ready) => return Ok(ready > 0),
            Err(rustix::io::Errno::INTR) => {}
            Err(errno) => return Err(errno.into()),
        }
    }
}

/// Reads one connection to EOF through `MAX_CHUNK_BYTES + 1` bytes, for at most [`CONNECTION_DEADLINE`].
fn read_connection(stream: UnixStream) -> TestResult<Vec<u8>> {
    stream.set_nonblocking(true)?;
    let until = Instant::now()
        .checked_add(CONNECTION_DEADLINE)
        .ok_or("deadline overflow")?;
    let cap = u64::try_from(MAX_CHUNK_BYTES)?.saturating_add(1);
    let mut reader = stream.take(cap);
    let mut bytes = Vec::with_capacity(MAX_CHUNK_BYTES.saturating_add(1));
    let mut buf = vec![0_u8; 8192];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => bytes.extend_from_slice(buf.get(..read).ok_or("read past the buffer")?),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                readable(reader.get_ref(), left)?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    Ok(bytes)
}

/// What the host side of one chunked stream observed.
struct Received {
    reassembler: Reassembler,
    first_fault: Option<ChunkFault>,
    chunk_connections: u32,
    frames: Vec<Vec<u8>>,
    other_connections: u32,
    first_chunk_at: Option<Instant>,
    end_at: Option<Instant>,
}

/// Accepts connections until `exited` is set and no connection is pending, routing each by its magic.
fn receive(listener: &UnixListener, exited: &AtomicBool) -> TestResult<Received> {
    let mut received = Received {
        reassembler: Reassembler::new(ChunkToken::from_bytes([TOKEN_BYTE; 32]), MAX_TOTAL),
        first_fault: None,
        chunk_connections: 0,
        frames: Vec::new(),
        other_connections: 0,
        first_chunk_at: None,
        end_at: None,
    };
    loop {
        if !readable(listener, ACCEPT_POLL)? {
            if exited.load(Ordering::SeqCst) {
                return Ok(received);
            }
            continue;
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::ConnectionAborted
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let bytes = read_connection(stream)?;
        if bytes.starts_with(&CHUNK_MAGIC) {
            let now = Instant::now();
            received.first_chunk_at.get_or_insert(now);
            received.chunk_connections = received.chunk_connections.saturating_add(1);
            if bytes.get(KIND_OFFSET) == Some(&KIND_END) {
                received.end_at = Some(now);
            }
            if let Err(fault) = received.reassembler.accept(&bytes)
                && received.first_fault.is_none()
            {
                received.first_fault = Some(fault);
            }
        } else if bytes.starts_with(FRAME_MAGIC) {
            received.frames.push(bytes);
        } else {
            received.other_connections = received.other_connections.saturating_add(1);
        }
    }
}

/// Decodes a `MINOSST1` status frame into JSON whose `v` is 1, or explains why it is not one.
fn status_frame(bytes: &[u8]) -> TestResult<Value> {
    let (magic, rest) = bytes.split_first_chunk::<8>().ok_or("short frame")?;
    let (version, rest) = rest.split_first_chunk::<4>().ok_or("short frame")?;
    let (length, payload) = rest.split_first_chunk::<8>().ok_or("short frame")?;
    if magic != FRAME_MAGIC {
        return Err("not a MINOSST1 frame".into());
    }
    if u32::from_be_bytes(*version) != FRAME_VERSION {
        return Err("frame version is not 1".into());
    }
    let declared = u64::from_be_bytes(*length);
    if declared > FRAME_MAX_BYTES || u64::try_from(payload.len())? != declared {
        return Err("frame length does not match its payload".into());
    }
    let value: Value = serde_json::from_slice(payload)?;
    if value.get("v").and_then(Value::as_u64) != Some(1) {
        return Err("frame is not a v1 object".into());
    }
    Ok(value)
}

/// Reads a helper-written file through `cap + 1` bytes without following a symlink; a missing file is empty.
fn read_helper_file(path: &Path, cap: u64) -> TestResult<Vec<u8>> {
    let fd = match rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(Vec::new()),
        Err(errno) => return Err(errno.into()),
    };
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(format!("{} is not a regular file", path.display()).into());
    }
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Reads a file the test created through its own handle, from the start, through `cap + 1` bytes.
fn read_own_handle(mut file: File, cap: u64) -> TestResult<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Kills the child's process group if it is still running, and reaps it, so no early return leaves a VM behind.
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            kill_process_group(Pid::from_child(&self.0), Signal::KILL).ok();
        }
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

/// What one chunked launch left behind.
struct ChunkRun {
    status: ExitStatus,
    killed_at_deadline: bool,
    helper_elapsed: Duration,
    received: Received,
}

/// Launches a `chunk_stream` of `bytes` under Seatbelt and receives it.
fn chunk_run(label: &str, bytes: u64) -> TestResult<ChunkRun> {
    let vmm = signed_vmm()?;
    let image = minimal_root_image(&guest()?)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let job = chunk_job(bytes);
    println!("== {label}: job {}", printable(job.as_bytes()));
    supervise::write_disks(
        &files,
        image.bytes(),
        &input_disk_image(job.as_bytes())?,
        SCRATCH_CAP_BYTES,
    )?;
    let listener = supervise::bind_output(&files)?;
    let (krunfw, krunfw_sha256) = krunfw()?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw.clone(), krunfw_sha256),
        Boot::RootDisk,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        MAX_DURATION_MS,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let params = BTreeMap::from([
        ("VMM", vmm.clone()),
        ("CONFIG", canonical_entry(files.config())?),
        ("KRUNFW", fs::canonicalize(&krunfw)?),
        ("KRUNFW_LINK", canonical_entry(Path::new(KRUNFW_LINK))?),
        ("ROOT_IMG", canonical_entry(files.root())?),
        ("INPUT_IMG", canonical_entry(files.input())?),
        ("SCRATCH_IMG", canonical_entry(files.scratch())?),
        ("CONSOLE_LOG", canonical_entry(files.console())?),
        ("SOCK", canonical_entry(files.socket())?),
    ]);
    let argv = seatbelt_argv(&profile()?, &params, &vmm, &[files.config().as_os_str()]);
    print_argv(label, &argv);
    let (program, args) = argv.split_first().ok_or("empty argv")?;

    let stdout = File::create_new(files.vmm_stdout())?;
    let stderr = File::create_new(files.vmm_stderr())?;
    let stdout_handle = stdout.try_clone()?;
    let stderr_handle = stderr.try_clone()?;
    let exited = Arc::new(AtomicBool::new(false));
    let receive_exited = Arc::clone(&exited);
    let receiver = thread::spawn(move || {
        receive(&listener, &receive_exited).map_err(|error| error.to_string())
    });

    let start = Instant::now();
    let deadline = start
        .checked_add(HOST_DEADLINE)
        .ok_or("deadline overflow")?;
    let mut child = Reaper(
        Command::new(program)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .process_group(0)
            .spawn()?,
    );
    let group = Pid::from_child(&child.0);
    let mut killed_at_deadline = false;
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            kill_process_group(group, Signal::KILL)?;
            killed_at_deadline = true;
            break child.0.wait()?;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let helper_elapsed = start.elapsed();
    exited.store(true, Ordering::SeqCst);
    let received = receiver.join().map_err(|_| "receiver panicked")??;

    let console = read_helper_file(files.console(), CONSOLE_CAP_BYTES)?;
    let vmm_stdout = read_own_handle(stdout_handle, VMM_STDIO_CAP)?;
    let vmm_stderr = read_own_handle(stderr_handle, VMM_STDIO_CAP)?;
    println!(
        "== {label}: status {status:?}, exit code {:?}, signal {:?}, killed_at_deadline {killed_at_deadline}, helper elapsed {helper_elapsed:?}",
        status.code(),
        status.signal()
    );
    println!(
        "-- console ({} bytes):\n{}",
        console.len(),
        printable(&console)
    );
    println!(
        "-- minos-vmm stderr ({} bytes):\n{}",
        vmm_stderr.len(),
        printable(&vmm_stderr)
    );
    println!(
        "-- minos-vmm stdout ({} bytes):\n{}",
        vmm_stdout.len(),
        printable(&vmm_stdout)
    );
    println!(
        "-- host: chunk connections {}, frames {}, other connections {}, first fault {:?}, complete {}",
        received.chunk_connections,
        received.frames.len(),
        received.other_connections,
        received.first_fault,
        received.reassembler.is_complete()
    );
    for (index, frame) in received.frames.iter().enumerate() {
        match status_frame(frame) {
            Ok(value) => println!(
                "-- frame {index} ({} bytes):\n{}",
                frame.len(),
                printable(serde_json::to_string_pretty(&value)?.as_bytes())
            ),
            Err(error) => println!(
                "-- frame {index} ({} bytes): not a frame: {error}",
                frame.len()
            ),
        }
    }
    Ok(ChunkRun {
        status,
        killed_at_deadline,
        helper_elapsed,
        received,
    })
}

/// Checks a chunked run end to end and prints its measurement. Returns the wall time from the first chunk accept to
/// the END.
fn check_chunk_run(label: &str, bytes: u64) -> TestResult<Duration> {
    let run = chunk_run(label, bytes)?;
    assert_eq!(run.status.code(), Some(0), "the helper must exit 0");
    assert!(!run.killed_at_deadline, "the host deadline fired");
    assert_eq!(run.received.first_fault, None, "a chunk faulted");
    assert_eq!(
        run.received.other_connections, 0,
        "an unrouted connection arrived"
    );
    let [frame] = run.received.frames.as_slice() else {
        return Err(format!(
            "expected exactly one status frame, got {}",
            run.received.frames.len()
        )
        .into());
    };
    let frame = status_frame(frame)?;
    let result = frame
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.iter().find_map(|step| step.get("chunk_stream")))
        .ok_or("the frame has no chunk_stream result")?
        .clone();
    let wall = match (run.received.first_chunk_at, run.received.end_at) {
        (Some(first), Some(end)) => end.saturating_duration_since(first),
        _ => return Err("no first chunk or no END arrived".into()),
    };
    let chunk_connections = run.received.chunk_connections;
    let reassembled = run.received.reassembler.finish()?;
    let expected = pattern(usize::try_from(bytes)?);
    let exact = reassembled == expected;
    println!(
        "== {label} measurement: payload {bytes} bytes, wall time first chunk accept to END {wall:?} ({} ms), host chunk connections {chunk_connections}, guest connections {}, guest connect_retries {}, guest elapsed_ms {}, guest bytes_sent {}, guest error {}, helper total run time {:?} ({} ms), reassembled exactly {exact}",
        wall.as_millis(),
        result.get("connections").unwrap_or(&Value::Null),
        result.get("connect_retries").unwrap_or(&Value::Null),
        result.get("elapsed_ms").unwrap_or(&Value::Null),
        result.get("bytes_sent").unwrap_or(&Value::Null),
        printable(
            result
                .get("error")
                .unwrap_or(&Value::Null)
                .to_string()
                .as_bytes()
        ),
        run.helper_elapsed,
        run.helper_elapsed.as_millis()
    );
    assert!(exact, "the reassembled stream differs from the pattern");
    assert_eq!(
        result.get("error"),
        Some(&Value::Null),
        "the guest reported an error"
    );
    Ok(wall)
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn chunked_16_mib_reassembles_and_is_measured() -> TestResult {
    let wall = check_chunk_run("chunked_16_mib_reassembles_and_is_measured", 16_777_216)?;
    println!(
        "== chunks selection input: 16 MiB wall time {wall:?}, bound {WALL_TIME_BOUND:?}, within bound {}",
        wall <= WALL_TIME_BOUND
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn chunked_1_mib_reassembles() -> TestResult {
    check_chunk_run("chunked_1_mib_reassembles", 1_048_576)?;
    Ok(())
}
