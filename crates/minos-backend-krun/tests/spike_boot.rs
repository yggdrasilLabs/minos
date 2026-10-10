//! Boot spikes (02-06-PLAN.md): SPIKE-04 (an unentitled helper is refused), SPIKE-01 as amended by 02-CONTEXT.md
//! D-18 (boot from a raw read-only `SquashFS` root on `/dev/vda` whose `/init` is the guest), and the literal
//! initramfs attempt, kept as negative evidence (D-18).
//!
//! Every test is ignored and needs a VM host. A missing variable or an unentitled helper is an error carrying the
//! setup commands below, never a skip (D-02). Guest bytes reach the terminal only through `GuestBytes::excerpt`
//! and the decoded status frame, printed with control and bidi characters escaped.
//!
//! Run: `cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1`.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use minos_backend_krun::initramfs::initramfs_image;
use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles, Launch, MAX_FRAME_BYTES, VmRun};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// 02-06-PLAN.md E-1 and E-4: what to run and set when a prerequisite is missing.
const SETUP: &str = "\
build and sign first:
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
  cargo build -p minos-vmm
  cp target/debug/minos-vmm <scratch>/minos-vmm-unsigned   (unsigned_helper_is_refused only)
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_VMM_UNSIGNED=<scratch>/minos-vmm-unsigned   (unsigned_helper_is_refused only)
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)";

/// The guest's vsock port for the status frame; the jobs below name the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the supervisor reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// Most characters of any one excerpt printed (at least the console cap, so the console prints whole).
const EXCERPT_CHARS: usize = 1_048_576;

/// `report_boot` only, then power off.
const REPORT_BOOT_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}}],"finish":"poweroff"}"#;
/// The canonical job (02-01 E-9): `report_boot`, a 64 KiB scratch round trip, then power off.
const CANONICAL_JOB: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"scratch_roundtrip":{"bytes":65536}}],"finish":"poweroff"}"#;

/// The guest's `/proc/cmdline` in root-disk mode, split on ASCII whitespace, with the boot-time value
/// normalized. Assembled by `msb_krun` 0.1.41 `build_kernel_cmdline` as the aarch64 `DEFAULT_KERNEL_CMDLINE`, then
/// minos-vmm's `cmdline`, then `root=/dev/root init=` plus minos-vmm's `init_path`, then the `KRUN_*` variables,
/// then ` -- ` and the (empty) arguments.
const EXPECTED_CMDLINE: &[&str] = &[
    "reboot=k",
    "panic=-1",
    "panic_print=0",
    "nomodule",
    "console=hvc0",
    "rootfstype=virtiofs",
    "rw",
    "quiet",
    "no-kvmapf",
    "rootfstype=squashfs",
    "ro",
    "ignore_loglevel",
    "root=/dev/root",
    "init=/init",
    "root=/dev/vda",
    "KRUN_BOOT_START_NS=<ns>",
    "--",
];

/// How the kernel is told to find `/init`.
enum BootMode {
    /// The decided path (D-18): the root disk on `/dev/vda`.
    RootDisk,
    /// The negative-evidence path: this newc archive as the initramfs.
    Initramfs(Vec<u8>),
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

/// The guest binary named by `MINOS_GUEST`.
fn guest() -> TestResult<Vec<u8>> {
    Ok(std::fs::read(env_path("MINOS_GUEST")?)?)
}

/// Launches `vmm` once for `job_json` in a fresh job directory and returns what it left behind.
fn launch(
    vmm: &Path,
    root_image: &[u8],
    job_json: &str,
    boot: BootMode,
    max_duration_ms: u64,
    host_deadline: Duration,
) -> TestResult<VmRun> {
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(job_json.as_bytes())?;
    supervise::write_disks(&files, root_image, &input, SCRATCH_CAP_BYTES)?;
    let boot = match boot {
        BootMode::RootDisk => Boot::RootDisk,
        BootMode::Initramfs(archive) => {
            File::create_new(files.initramfs())?.write_all(&archive)?;
            Boot::initramfs(
                files.initramfs(),
                supervise::sha256_file(files.initramfs())?,
            )
        }
    };
    let listener = supervise::bind_output(&files)?;
    let (krunfw, krunfw_sha256) = krunfw()?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw, krunfw_sha256),
        boot,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        max_duration_ms,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let argv = [vmm, files.config()];
    let run = supervise::run(Launch::new(argv, &files, listener).host_deadline(host_deadline))?;
    Ok(run)
}

/// Splits `/proc/cmdline` on ASCII whitespace and normalizes an all-digit `KRUN_BOOT_START_NS=` value.
fn cmdline_tokens(text: &str) -> Vec<String> {
    const BOOT_START: &str = "KRUN_BOOT_START_NS=";
    text.split_ascii_whitespace()
        .map(|token| match token.strip_prefix(BOOT_START) {
            Some(ns) if !ns.is_empty() && ns.bytes().all(|byte| byte.is_ascii_digit()) => {
                format!("{BOOT_START}<ns>")
            }
            _ => token.to_owned(),
        })
        .collect()
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

/// Prints everything a run left behind, so the test output is the evidence.
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
        run.console().excerpt(EXCERPT_CHARS)
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
                "-- connection {index} ({} bytes, overflowed {}): not a frame: {error}",
                connection.len(),
                connection.overflowed()
            ),
        }
    }
    println!("== end {label}");
    Ok(())
}

/// The status frame of a run that made exactly one connection.
fn only_frame(run: &VmRun) -> TestResult<Value> {
    match run.connections() {
        [connection] => Ok(connection.status_frame(MAX_FRAME_BYTES)?),
        connections => {
            Err(format!("expected exactly one connection, got {}", connections.len()).into())
        }
    }
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

/// The string at `key` in `object`.
fn string<'a>(object: &'a Value, key: &str) -> TestResult<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} is not a string").into())
}

#[test]
#[ignore = "needs MINOS_VMM_UNSIGNED (minos-vmm copied before signing), MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn unsigned_helper_is_refused() -> TestResult {
    let unsigned = env_path("MINOS_VMM_UNSIGNED")?;
    if supervise::has_hypervisor_entitlement(&unsigned)? {
        return Err(format!(
            "wrong binary: {} carries com.apple.security.hypervisor; copy target/debug/minos-vmm before signing; {SETUP}",
            unsigned.display()
        )
        .into());
    }
    let image = minimal_root_image(&guest()?)?;
    let run = launch(
        &unsigned,
        image.bytes(),
        REPORT_BOOT_JOB,
        BootMode::RootDisk,
        30_000,
        Duration::from_mins(1),
    )?;
    print_run("unsigned_helper_is_refused", &run)?;

    assert_eq!(
        run.exit_code(),
        Some(70),
        "the unentitled helper must exit 70"
    );
    assert!(
        !run.console().contains(b"MINOS-GUEST-BOOT-OK"),
        "the guest must not boot"
    );
    assert!(run.vmm_stderr().contains(b"minos-vmm: error:"));
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn boot_root_disk() -> TestResult {
    let vmm = signed_vmm()?;
    let image = minimal_root_image(&guest()?)?;
    let run = launch(
        &vmm,
        image.bytes(),
        CANONICAL_JOB,
        BootMode::RootDisk,
        60_000,
        Duration::from_mins(2),
    )?;
    print_run("boot_root_disk", &run)?;

    assert_eq!(run.exit_code(), Some(0));
    assert!(!run.killed_at_deadline());
    for marker in [
        "MINOS-GUEST-BOOT-OK",
        "MINOS-GUEST-FRAME-SENT",
        "MINOS-GUEST-POWEROFF",
    ] {
        assert!(
            run.console().contains(marker.as_bytes()),
            "console lacks {marker}"
        );
    }

    let frame = only_frame(&run)?;
    assert_eq!(frame.get("v"), Some(&Value::from(1)));
    let boot = step(&frame, "report_boot")?;

    let tokens = cmdline_tokens(string(boot, "cmdline")?);
    println!(
        "normalized cmdline tokens: {}",
        printable(&format!("{tokens:?}"))
    );
    println!("osrelease: {}", printable(string(boot, "osrelease")?));
    assert_eq!(tokens, EXPECTED_CMDLINE);
    let last = |prefix: &str| {
        tokens
            .iter()
            .rev()
            .find(|token| token.starts_with(prefix))
            .cloned()
    };
    assert_eq!(last("root=").as_deref(), Some("root=/dev/vda"));
    assert_eq!(last("rootfstype=").as_deref(), Some("rootfstype=squashfs"));
    assert!(tokens.iter().any(|token| token == "init=/init"));
    assert!(!string(boot, "osrelease")?.is_empty());

    let devices = strings(boot, "virtio_devices")?;
    assert!(
        !devices.iter().any(|device| device.ends_with(":0x001a")),
        "a virtio-fs device (id 0x001a) is present"
    );

    let env = boot
        .get("env")
        .and_then(Value::as_array)
        .ok_or("env is not an array")?;
    for pair in env {
        let Some([Value::String(key), Value::String(value)]) = pair.as_array().map(Vec::as_slice)
        else {
            return Err("env holds something other than a [key, value] pair".into());
        };
        assert!(
            !key.starts_with("MSB_") && !key.starts_with("DYLD_"),
            "guest env has {}",
            printable(key)
        );
        assert!(!value.contains("/Users/"), "guest env leaks a host path");
    }

    let ifaces = strings(boot, "net_ifaces")?;
    assert!(
        ifaces.is_empty() || ifaces == ["lo"],
        "network interfaces: {}",
        printable(&format!("{ifaces:?}"))
    );

    let scratch = step(&frame, "scratch_roundtrip")?;
    assert_eq!(scratch.get("ok"), Some(&Value::Bool(true)));
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn boot_initramfs_attempt() -> TestResult {
    let vmm = signed_vmm()?;
    let guest = guest()?;
    let image = minimal_root_image(&guest)?;
    let archive = initramfs_image(&guest)?;
    let run = launch(
        &vmm,
        image.bytes(),
        REPORT_BOOT_JOB,
        BootMode::Initramfs(archive),
        30_000,
        Duration::from_mins(1),
    )?;
    print_run("boot_initramfs_attempt", &run)?;

    assert!(
        !run.console().contains(b"MINOS-GUEST-BOOT-OK"),
        "the kernel ran /init from the initramfs, overturning 02-RESEARCH.md Pitfall 1"
    );
    assert!(
        run.connections()
            .iter()
            .all(|connection| connection.status_frame(MAX_FRAME_BYTES).is_err()),
        "a connection carried a valid status frame"
    );
    assert!(
        run.console().contains(b"Kernel panic") || run.console().contains(b"VFS"),
        "the console shows neither a kernel panic nor a VFS error"
    );
    Ok(())
}
