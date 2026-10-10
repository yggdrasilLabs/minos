//! Seatbelt spike (02-09-PLAN.md): SPIKE-05, whether a deny-default Seatbelt profile around `minos-vmm`
//! (`crates/minos-vmm/minos-vmm.sb`) still lets it create and run a VM, and whether the same profile denies an
//! outbound TCP connection and a read of a file outside its allow list.
//!
//! The helper runs as `/usr/bin/sandbox-exec -f <profile> -D NAME=value ... <program> <args>`, an argv array with
//! every `-D` value canonical (`/private/tmp`, not `/tmp`). Every test is ignored. A missing variable or an
//! unentitled helper is an error carrying the setup commands below, never a skip (D-02). Guest bytes reach the
//! terminal only through `GuestBytes::excerpt` and the decoded status frame, printed with control and bidi
//! characters escaped.
//!
//! Run: `cargo test -p minos-backend-krun --test spike_seatbelt -- --ignored --nocapture --test-threads=1`.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

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
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)";

/// The Seatbelt launcher, deprecated by Apple but shipped with macOS.
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
/// The Homebrew symlink to libkrunfw. The profile lists it beside its Cellar target (`MINOS_KRUNFW`).
const KRUNFW_LINK: &str = "/opt/homebrew/lib/libkrunfw.5.dylib";
/// The profile, relative to this crate's manifest directory.
const PROFILE: &str = "../minos-vmm/minos-vmm.sb";

/// The guest's vsock port for the status frame; the job below names the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the supervisor reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// Most characters of any one excerpt printed (at least the console cap, so the console prints whole).
const EXCERPT_CHARS: usize = 1_048_576;

/// `report_boot`, one guest TCP connect to `1.1.1.1:443` with a 2 s timeout, then power off. The status frame is
/// about 1.5 KB, well under the 8 KiB-per-connection vsock limit (02-08 DIAG-01).
const BOOT_JOB: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"connect":{"targets":["1.1.1.1:443"],"timeout_ms":2000}}],"finish":"poweroff"}"#;

/// The bytes of the listed file in `profile_denies_unlisted_file`.
const LISTED_BYTES: &[u8] = b"minos-seatbelt-listed\n";
/// The bytes of the unlisted file in `profile_denies_unlisted_file`.
const UNLISTED_BYTES: &[u8] = b"minos-seatbelt-unlisted\n";

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

/// The canonical `MINOS_VMM`, which must carry the hypervisor entitlement.
fn signed_vmm() -> TestResult<PathBuf> {
    let vmm = std::fs::canonicalize(env_path("MINOS_VMM")?)?;
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

/// The canonical profile path.
fn profile() -> TestResult<PathBuf> {
    Ok(std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(PROFILE),
    )?)
}

/// `path` with its parent directory canonicalized and its last component kept as written, so a symlink or a file
/// that does not exist yet (the console log) gets a canonical spelling without being resolved.
fn canonical_entry(path: &Path) -> TestResult<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    Ok(std::fs::canonicalize(parent)?.join(name))
}

/// The profile's parameters for the job in `files`, with `vmm` as the one program the profile may exec.
fn job_params(
    files: &JobFiles,
    vmm: &Path,
    krunfw: &Path,
) -> TestResult<BTreeMap<&'static str, PathBuf>> {
    Ok(BTreeMap::from([
        ("VMM", std::fs::canonicalize(vmm)?),
        ("CONFIG", canonical_entry(files.config())?),
        ("KRUNFW", std::fs::canonicalize(krunfw)?),
        ("KRUNFW_LINK", canonical_entry(Path::new(KRUNFW_LINK))?),
        ("ROOT_IMG", canonical_entry(files.root())?),
        ("INPUT_IMG", canonical_entry(files.input())?),
        ("SCRATCH_IMG", canonical_entry(files.scratch())?),
        ("CONSOLE_LOG", canonical_entry(files.console())?),
        ("SOCK", canonical_entry(files.socket())?),
    ]))
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

/// Runs a host program (no VM) with an empty environment, null stdin, and captured output, and prints the result.
fn run_host(label: &str, argv: &[OsString]) -> TestResult<Output> {
    print_argv(label, argv);
    let (program, args) = argv.split_first().ok_or("empty argv")?;
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .output()?;
    println!(
        "-- {label}: status {:?}, exit code {:?}\n-- stdout:\n{}\n-- stderr:\n{}",
        output.status,
        output.status.code(),
        printable(&String::from_utf8_lossy(&output.stdout)),
        printable(&String::from_utf8_lossy(&output.stderr))
    );
    Ok(output)
}

/// A job directory whose job files hold small marker bytes, for the profile tests that run no VM.
fn marker_job(dir: &Path) -> TestResult<JobFiles> {
    let files = JobFiles::in_dir(dir)?;
    for path in [files.root(), files.input(), files.scratch(), files.config()] {
        File::create_new(path)?.write_all(LISTED_BYTES)?;
    }
    Ok(files)
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

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 02-06-PLAN.md E-1"]
fn boot_under_seatbelt() -> TestResult {
    let vmm = signed_vmm()?;
    let image = minimal_root_image(&guest()?)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(BOOT_JOB.as_bytes())?;
    supervise::write_disks(&files, image.bytes(), &input, SCRATCH_CAP_BYTES)?;
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
        60_000,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let params = job_params(&files, &vmm, &krunfw)?;
    let argv = seatbelt_argv(&profile()?, &params, &vmm, &[files.config().as_os_str()]);
    print_argv("boot_under_seatbelt", &argv);
    let run =
        supervise::run(Launch::new(argv, &files, listener).host_deadline(Duration::from_mins(2)))?;
    print_run("boot_under_seatbelt", &run)?;

    assert_eq!(run.exit_code(), Some(0));
    assert!(!run.killed_at_deadline());
    assert!(
        run.console().contains(b"MINOS-GUEST-BOOT-OK"),
        "console lacks MINOS-GUEST-BOOT-OK"
    );
    let frame = only_frame(&run)?;
    assert_eq!(frame.get("v"), Some(&Value::from(1)));
    step(&frame, "report_boot")?;
    let attempts = step(&frame, "connect")?
        .get("results")
        .and_then(Value::as_array)
        .ok_or("connect results is not an array")?;
    let [attempt] = attempts.as_slice() else {
        return Err(format!("expected one connect result, got {}", attempts.len()).into());
    };
    assert_eq!(
        attempt.get("ok"),
        Some(&Value::Bool(false)),
        "the guest reached 1.1.1.1:443"
    );
    Ok(())
}

#[test]
#[ignore = "needs MINOS_KRUNFW and /usr/bin/sandbox-exec (no VM); see 02-06-PLAN.md E-1"]
fn profile_denies_network() -> TestResult {
    let (krunfw, _) = krunfw()?;
    let dir = supervise::short_job_dir()?;
    let files = marker_job(dir.path())?;
    let nc = Path::new("/usr/bin/nc");
    let params = job_params(&files, nc, &krunfw)?;
    let args = ["-z", "-G", "2", "1.1.1.1", "443"].map(OsStr::new);
    let output = run_host(
        "profile_denies_network",
        &seatbelt_argv(&profile()?, &params, nc, &args),
    )?;
    assert!(
        !output.status.success(),
        "nc reached 1.1.1.1:443 under the profile"
    );
    Ok(())
}

#[test]
#[ignore = "needs MINOS_KRUNFW and /usr/bin/sandbox-exec (no VM); see 02-06-PLAN.md E-1"]
fn profile_denies_unlisted_file() -> TestResult {
    let (krunfw, _) = krunfw()?;
    let dir = supervise::short_job_dir()?;
    let files = marker_job(dir.path())?;
    let other = supervise::short_job_dir()?;
    let unlisted = std::fs::canonicalize(other.path())?.join("unlisted.txt");
    File::create_new(&unlisted)?.write_all(UNLISTED_BYTES)?;
    let cat = Path::new("/bin/cat");
    let params = job_params(&files, cat, &krunfw)?;
    let profile = profile()?;

    let denied = run_host(
        "profile_denies_unlisted_file (unlisted)",
        &seatbelt_argv(&profile, &params, cat, &[unlisted.as_os_str()]),
    )?;
    assert!(
        !denied.status.success(),
        "cat read the unlisted file under the profile"
    );
    assert!(denied.stdout.is_empty(), "cat printed unlisted bytes");

    let root = params.get("ROOT_IMG").ok_or("no ROOT_IMG parameter")?;
    let allowed = run_host(
        "profile_denies_unlisted_file (listed ROOT_IMG)",
        &seatbelt_argv(&profile, &params, cat, &[root.as_os_str()]),
    )?;
    assert_eq!(allowed.status.code(), Some(0));
    assert_eq!(allowed.stdout, LISTED_BYTES);
    Ok(())
}
