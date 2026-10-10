//! K7 exit map (02-07-PLAN.md, D-05, folded into SPIKE-01): how a guest exit and a VMM or launcher failure each
//! surface to the process that started `minos-vmm`.
//!
//! Every test is ignored and needs a VM host. A missing variable or an unentitled helper is an error carrying the
//! setup commands below, never a skip (D-02). Scenarios K7-01 to K7-04 and K7-08 to K7-09 run through
//! `supervise::run`. K7-05, K7-06, and K7-10 need the helper's PID while it runs (or a chosen environment), so they
//! prepare the job directory with the same supervisor functions and spawn the helper directly. K7-07 orphans the
//! helper through `orphan_launcher`, which is this test binary run again with an argv array, so no shell is used.
//!
//! Guest bytes reach the terminal only through `GuestBytes::excerpt`, or, for a directly spawned helper, through a
//! capped `Read::take` and the same allowlist (printable characters, newline, tab).
//!
//! Run: `cargo test -p minos-backend-krun --test spike_exits -- --ignored --nocapture --test-threads=1`.

use std::error::Error;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixListener;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{
    self, JobFiles, Launch, MAX_FRAME_BYTES, VMM_STDIO_CAP, VmRun,
};
use tempfile::TempDir;

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

/// The guest's vsock port for the status frame; the jobs below name the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows in every scenario except K7-04.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// The K7-04 console cap (E-2).
const FLOOD_CONSOLE_CAP_BYTES: u64 = 262_144;
/// Console bytes the supervisor reads back in K7-04: large enough to hold the overshoot past the launcher's cap.
const FLOOD_READBACK_BYTES: u64 = 16 * 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// Most characters of any one excerpt (at least the largest read-back, so nothing is cut before the digest).
const EXCERPT_CHARS: usize = 16 * 1024 * 1024;
/// Console lines printed from the end of the log (the evidence protocol).
const TAIL_LINES: usize = 40;
/// Console lines printed wherever they appear.
const CONSOLE_MARKERS: [&str; 4] = [
    "MINOS-",
    "Kernel panic",
    "Attempted to kill init",
    "minos-vmm",
];
/// Boot marker the guest prints first.
const BOOT_MARKER: &[u8] = b"MINOS-GUEST-BOOT-OK";
/// How often a directly spawned helper is polled.
const POLL: Duration = Duration::from_millis(50);
/// How long to wait for the boot marker before signalling.
const BOOT_WAIT: Duration = Duration::from_secs(30);
/// A kernel path that does not exist (K7-09).
const MISSING_KRUNFW: &str = "/private/tmp/minos-no-such-libkrunfw.dylib";
/// A digest that matches no file (K7-08).
const ZERO_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// `report_boot`, then power off.
const POWEROFF_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}}],"finish":"poweroff"}"#;
/// `report_boot`, then return from PID 1.
const RETURN_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}}],"finish":"return"}"#;
/// `report_boot`, then sleep forever; the frame is never sent.
const HANG_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"hang":{}}],"finish":"poweroff"}"#;
/// `report_boot`, an 8 MiB console flood, then power off.
const FLOOD_JOB: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"console_flood":{"bytes":8388608}}],"finish":"poweroff"}"#;

/// Variables `orphan_launcher` reads (E-1).
const ORPHAN_CONFIG: &str = "MINOS_SPIKE_ORPHAN_CONFIG";
/// The job directory `orphan_launcher` writes the helper's files into.
const ORPHAN_DIR: &str = "MINOS_SPIKE_ORPHAN_DIR";
/// Set (to anything) to make `orphan_launcher` return only after the guest's boot marker (K7-07b).
const ORPHAN_WAIT_BOOT: &str = "MINOS_SPIKE_ORPHAN_WAIT_BOOT";
/// File in the job directory holding the orphaned helper's PID.
const PID_FILE: &str = "vmm.pid";

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

/// The real kernel: the `MINOS_KRUNFW` path and its digest.
fn real_kernel() -> TestResult<Kernel> {
    let (path, sha256) = krunfw()?;
    Ok(Kernel::new(path, sha256))
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

/// A prepared job directory: disks and `vmm.json`. The helper is not started.
struct Job {
    /// Keeps the directory until the test ends.
    _dir: TempDir,
    files: JobFiles,
}

/// Prepares one job directory for `job_json` with the given kernel, launcher deadline, and console cap, and
/// binds its output listener.
fn prepare(
    job_json: &str,
    kernel: Kernel,
    max_duration_ms: u64,
    console_cap: u64,
) -> TestResult<(Job, UnixListener)> {
    let guest = fs::read(env_path("MINOS_GUEST")?)?;
    let image = minimal_root_image(&guest)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(job_json.as_bytes())?;
    supervise::write_disks(&files, image.bytes(), &input, SCRATCH_CAP_BYTES)?;
    let listener = supervise::bind_output(&files)?;
    let config = LaunchConfig::new(
        kernel,
        Boot::RootDisk,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), console_cap),
        max_duration_ms,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    Ok((Job { _dir: dir, files }, listener))
}

/// Runs a prepared job through `supervise::run`.
fn run_job(
    vmm: &Path,
    job: &Job,
    listener: UnixListener,
    launch: LaunchLimits,
) -> TestResult<VmRun> {
    let argv = [vmm, job.files.config()];
    let run = supervise::run(
        Launch::new(argv, &job.files, listener)
            .host_deadline(launch.host_deadline)
            .console_cap(launch.console_readback),
    )?;
    Ok(run)
}

/// Host-side limits for one `supervise::run`.
#[derive(Clone, Copy)]
struct LaunchLimits {
    host_deadline: Duration,
    console_readback: u64,
}

impl LaunchLimits {
    const fn new(host_deadline: Duration) -> Self {
        Self {
            host_deadline,
            console_readback: CONSOLE_CAP_BYTES,
        }
    }
}

/// The allowlist `GuestBytes::excerpt` applies: printable characters, newline, and tab; invalid UTF-8 is U+FFFD.
fn sanitize(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|&character| {
            matches!(character, '\n' | '\t')
                || !(character.is_control()
                    || matches!(character, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        })
        .collect()
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

/// Code and signal of an exit status.
fn describe(status: ExitStatus) -> String {
    format!("code {:?}, signal {:?}", status.code(), status.signal())
}

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Number of connections that carry a valid v1 status frame.
fn frames(run: &VmRun) -> usize {
    run.connections()
        .iter()
        .filter(|connection| connection.status_frame(MAX_FRAME_BYTES).is_ok())
        .count()
}

/// Prints everything a supervised run left behind, so the test output is the evidence.
fn print_run(label: &str, run: &VmRun, console: &Path) -> TestResult {
    println!(
        "== {label}: status {}, killed_at_deadline {}, elapsed {:?}, connections {}, valid frames {}",
        describe(run.status()),
        run.killed_at_deadline(),
        run.elapsed(),
        run.connections().len(),
        frames(run)
    );
    match fs::metadata(console) {
        Ok(metadata) => println!("-- console file on disk: {} bytes", metadata.len()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            println!("-- console file on disk: absent");
        }
        Err(error) => return Err(error.into()),
    }
    println!(
        "-- console read back ({} bytes, overflowed {}):\n{}",
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
            Ok(frame) => {
                let steps: Vec<String> = frame
                    .get("steps")
                    .and_then(serde_json::Value::as_array)
                    .map(|steps| {
                        steps
                            .iter()
                            .filter_map(serde_json::Value::as_object)
                            .flat_map(|step| step.keys().map(|key| sanitize(key.as_bytes())))
                            .collect()
                    })
                    .unwrap_or_default();
                println!(
                    "-- connection {index} ({} bytes): v1 frame, steps {steps:?}",
                    connection.len()
                );
            }
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

/// Reads at most `cap` bytes of `path`; `None` if it does not exist.
fn read_capped(path: &Path, cap: u64) -> TestResult<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(cap).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Prints a directly spawned helper's console, stderr, and stdout through the capped reader and the allowlist.
fn print_files(label: &str, files: &JobFiles) -> TestResult {
    match read_capped(files.console(), CONSOLE_CAP_BYTES)? {
        Some(bytes) => println!(
            "-- console ({} bytes read, cap {CONSOLE_CAP_BYTES}):\n{}",
            bytes.len(),
            console_digest(&sanitize(&bytes))
        ),
        None => println!("-- console: absent"),
    }
    for (name, path) in [
        ("stderr", files.vmm_stderr()),
        ("stdout", files.vmm_stdout()),
    ] {
        match read_capped(path, VMM_STDIO_CAP)? {
            Some(bytes) => println!(
                "-- minos-vmm {name} ({} bytes):\n{}",
                bytes.len(),
                sanitize(&bytes)
            ),
            None => println!("-- minos-vmm {name}: absent"),
        }
    }
    println!("== end {label}");
    Ok(())
}

/// Whether the helper's stderr file contains `needle`.
fn stderr_has(files: &JobFiles, needle: &str) -> TestResult<bool> {
    Ok(read_capped(files.vmm_stderr(), VMM_STDIO_CAP)?
        .is_some_and(|bytes| contains(&bytes, needle.as_bytes())))
}

/// Counts connections pending on the listener without reading them (a directly spawned helper).
fn pending_connections(listener: &UnixListener) -> TestResult<usize> {
    listener.set_nonblocking(true)?;
    let mut count = 0_usize;
    loop {
        match listener.accept() {
            Ok(_) => count = count.saturating_add(1),
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(count),
            Err(error) => return Err(error.into()),
        }
    }
}

/// A directly spawned helper, killed and reaped on drop so no early return leaves one running.
struct Helper(Child);

impl Drop for Helper {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

/// Spawns `[vmm, config]` the way `supervise::run` does: an empty environment plus `env`, stdin from
/// `/dev/null`, stdout and stderr to the job directory's files, and its own process group.
fn spawn_helper(vmm: &Path, files: &JobFiles, env: &[(&str, &str)]) -> TestResult<Helper> {
    let stdout = File::create_new(files.vmm_stdout())?;
    let stderr = File::create_new(files.vmm_stderr())?;
    let child = Command::new(vmm)
        .arg(files.config())
        .env_clear()
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()?;
    Ok(Helper(child))
}

/// Polls the console every [`POLL`] until it shows the boot marker. The helper exiting first is an error.
fn wait_for_boot(helper: &mut Helper, files: &JobFiles) -> TestResult<Duration> {
    let start = Instant::now();
    loop {
        if read_capped(files.console(), CONSOLE_CAP_BYTES)?
            .is_some_and(|bytes| contains(&bytes, BOOT_MARKER))
        {
            return Ok(start.elapsed());
        }
        if let Some(status) = helper.0.try_wait()? {
            return Err(format!(
                "the helper exited before the boot marker: {}",
                describe(status)
            )
            .into());
        }
        if start.elapsed() >= BOOT_WAIT {
            return Err("no boot marker within 30 s".into());
        }
        thread::sleep(POLL);
    }
}

/// Waits up to `limit` for the helper to exit.
fn wait_exit(helper: &mut Helper, limit: Duration) -> TestResult<(ExitStatus, Duration)> {
    let start = Instant::now();
    loop {
        if let Some(status) = helper.0.try_wait()? {
            return Ok((status, start.elapsed()));
        }
        if start.elapsed() >= limit {
            return Err(format!("the helper did not exit within {limit:?}").into());
        }
        thread::sleep(POLL);
    }
}

/// Whether `/bin/ps -p <pid>` finds the process (it exits 0 only then).
fn process_exists(pid: u32) -> TestResult<bool> {
    let status = Command::new("/bin/ps")
        .args(["-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

/// `/bin/ps -p <pid> -o pid=,ppid=,stat=,comm=` output, for the evidence; empty when the process is gone.
fn ps_line(pid: u32) -> TestResult<String> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "pid=,ppid=,stat=,comm="])
        .env_clear()
        .stdin(Stdio::null())
        .output()?;
    Ok(format!(
        "exit {}; {}",
        describe(output.status),
        sanitize(&output.stdout).trim_end()
    ))
}

/// Sends SIGKILL to an orphaned helper on drop if it still exists, so a failed K7-07 leaves nothing running.
struct OrphanGuard(Option<u32>);

impl Drop for OrphanGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0
            && process_exists(pid).unwrap_or(true)
        {
            Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .env_clear()
                .status()
                .ok();
        }
    }
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_01_poweroff() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(POWEROFF_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits::new(Duration::from_secs(90)),
    )?;
    print_run("k7_01_poweroff", &run, job.files.console())?;

    assert_eq!(run.exit_code(), Some(0), "guest power-off must exit 0");
    assert!(!run.killed_at_deadline());
    assert_eq!(
        frames(&run),
        1,
        "the guest sent one frame before powering off"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_02_init_returns() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(RETURN_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits::new(Duration::from_mins(2)),
    )?;
    print_run("k7_02_init_returns", &run, job.files.console())?;

    assert_eq!(
        run.exit_code(),
        Some(0),
        "an init exit must surface as exit 0, the same as a power-off"
    );
    assert!(!run.killed_at_deadline());
    assert_eq!(frames(&run), 1, "the guest sent one frame before returning");
    // E-2 expected `Attempted to kill init`. libkrunfw v5.6.2 patch 0001 ("krunfw: Don't panic when init dies")
    // replaces that panic with `orderly_reboot()` unless VERBOSE_LIBKRUN is set, and patch 0002 makes the orderly
    // reboot skip `run_cmd` and print the line below before `kernel_restart` (evidence/02-07.md K7-02).
    assert!(
        run.console()
            .contains(b"reboot: Failed to start orderly reboot: forcing the issue"),
        "the console lacks the orderly-reboot line"
    );
    assert!(
        !run.console().contains(b"Attempted to kill init"),
        "the kernel panicked on the init exit"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_03_deadline() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(HANG_JOB, real_kernel()?, 5_000, CONSOLE_CAP_BYTES)?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits::new(Duration::from_secs(30)),
    )?;
    print_run("k7_03_deadline", &run, job.files.console())?;

    assert_eq!(
        run.exit_code(),
        Some(124),
        "the launcher deadline must exit 124"
    );
    assert!(!run.killed_at_deadline(), "the host deadline fired first");
    assert!(run.vmm_stderr().contains(b"minos-vmm: watchdog deadline"));
    assert!(run.vmm_stderr().contains(b"minos-vmm: exit 124"));
    assert_eq!(frames(&run), 0, "a hung guest sends no frame");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_04_console_cap() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(FLOOD_JOB, real_kernel()?, 60_000, FLOOD_CONSOLE_CAP_BYTES)?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits {
            host_deadline: Duration::from_mins(2),
            console_readback: FLOOD_READBACK_BYTES,
        },
    )?;
    print_run("k7_04_console_cap", &run, job.files.console())?;
    let size = fs::metadata(job.files.console())?.len();
    println!(
        "console file size {size} bytes against the {FLOOD_CONSOLE_CAP_BYTES}-byte cap: overshoot {} bytes",
        size.saturating_sub(FLOOD_CONSOLE_CAP_BYTES)
    );

    assert_eq!(run.exit_code(), Some(125), "the console cap must exit 125");
    assert!(!run.killed_at_deadline());
    assert!(
        run.vmm_stderr()
            .contains(b"minos-vmm: watchdog console-cap")
    );
    assert!(run.vmm_stderr().contains(b"minos-vmm: exit 125"));
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_05_sigterm() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(HANG_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let mut helper = spawn_helper(&vmm, &job.files, &[])?;
    let pid = helper.0.id();
    let booted = wait_for_boot(&mut helper, &job.files)?;
    println!("k7_05_sigterm: helper pid {pid} showed the boot marker after {booted:?}");
    let kill = Command::new("/bin/kill")
        .args(["-TERM", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .status()?;
    println!("/bin/kill -TERM {pid}: {}", describe(kill));
    assert!(kill.success(), "/bin/kill failed");
    let (status, after) = wait_exit(&mut helper, Duration::from_secs(30))?;
    println!(
        "== k7_05_sigterm: status {}, exited {after:?} after SIGTERM, pending connections {}",
        describe(status),
        pending_connections(&listener)?
    );
    print_files("k7_05_sigterm", &job.files)?;
    println!("ps after exit: {}", ps_line(pid)?);

    assert!(!process_exists(pid)?, "the helper is still running");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_06_sigkill() -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(HANG_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let mut helper = spawn_helper(&vmm, &job.files, &[])?;
    let pid = helper.0.id();
    let booted = wait_for_boot(&mut helper, &job.files)?;
    println!("k7_06_sigkill: helper pid {pid} showed the boot marker after {booted:?}");
    helper.0.kill()?;
    let (status, after) = wait_exit(&mut helper, Duration::from_secs(10))?;
    println!(
        "== k7_06_sigkill: status {}, reaped {after:?} after Child::kill, pending connections {}",
        describe(status),
        pending_connections(&listener)?
    );
    print_files("k7_06_sigkill", &job.files)?;
    println!("ps after reaping: {}", ps_line(pid)?);

    assert_eq!(status.signal(), Some(9), "SIGKILL must end the helper");
    assert!(!process_exists(pid)?, "the helper is still running");
    Ok(())
}

/// Orphans a helper through `orphan_launcher` and waits up to 15 s for it to go. With `wait_boot`, the
/// intermediate returns only after the guest's boot marker, so the helper's watchdog is already running when the
/// parent goes; without it, the intermediate returns as soon as the helper is spawned (E-1).
fn orphan_scenario(label: &str, wait_boot: bool) -> TestResult {
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(HANG_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let exe = std::env::current_exe()?;
    let mut intermediate = Command::new(&exe);
    intermediate
        .args([
            "orphan_launcher",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("MINOS_VMM", &vmm)
        .env(ORPHAN_CONFIG, job.files.config())
        .env(ORPHAN_DIR, job.files.dir())
        .stdin(Stdio::null());
    if wait_boot {
        intermediate.env(ORPHAN_WAIT_BOOT, "1");
    }
    let output = intermediate.output()?;
    let parent_exited = Instant::now();
    println!(
        "== intermediate (this test binary, orphan_launcher): status {}\n-- stdout:\n{}\n-- stderr:\n{}",
        describe(output.status),
        sanitize(&output.stdout),
        sanitize(&output.stderr)
    );
    assert!(output.status.success(), "orphan_launcher failed");

    let pid_bytes = read_capped(&job.files.dir().join(PID_FILE), 32)?
        .ok_or("orphan_launcher wrote no pid file")?;
    let pid: u32 = String::from_utf8(pid_bytes)?.trim().parse()?;
    let mut guard = OrphanGuard(Some(pid));
    println!("ps right after the intermediate exited: {}", ps_line(pid)?);

    let limit = Duration::from_secs(15);
    let gone_after = loop {
        if !process_exists(pid)? {
            break Some(parent_exited.elapsed());
        }
        if parent_exited.elapsed() >= limit {
            break None;
        }
        thread::sleep(POLL);
    };
    println!(
        "== {label}: helper pid {pid} gone after {gone_after:?} (limit {limit:?}), pending connections {}",
        pending_connections(&listener)?
    );
    if gone_after.is_none() {
        println!("ps at the limit: {}", ps_line(pid)?);
    }
    print_files(label, &job.files)?;

    assert!(
        gone_after.is_some(),
        "the orphaned helper outlived the 15 s limit"
    );
    guard.0 = None;
    assert!(stderr_has(&job.files, "minos-vmm: watchdog parent-gone")?);
    assert!(stderr_has(&job.files, "minos-vmm: exit 126")?);
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_07_parent_gone() -> TestResult {
    orphan_scenario("k7_07_parent_gone", false)
}

/// Diagnostic for K7-07 (evidence/02-07.md DEFECT-01): the same orphaning, but the parent goes only after the
/// guest booted, so the watchdog has already recorded the real parent.
#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_07b_parent_gone_after_boot() -> TestResult {
    orphan_scenario("k7_07b_parent_gone_after_boot", true)
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_08_kernel_hash_mismatch() -> TestResult {
    let vmm = signed_vmm()?;
    let (krunfw, _) = krunfw()?;
    let (job, listener) = prepare(
        POWEROFF_JOB,
        Kernel::new(krunfw, ZERO_SHA256),
        60_000,
        CONSOLE_CAP_BYTES,
    )?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits::new(Duration::from_mins(1)),
    )?;
    print_run("k7_08_kernel_hash_mismatch", &run, job.files.console())?;

    assert_eq!(
        run.exit_code(),
        Some(67),
        "a kernel digest mismatch must exit 67"
    );
    assert!(run.console().is_empty(), "the guest wrote to the console");
    assert_eq!(frames(&run), 0);
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_09_kernel_missing() -> TestResult {
    let vmm = signed_vmm()?;
    let (_, sha256) = krunfw()?;
    if Path::new(MISSING_KRUNFW).exists() {
        return Err(format!("{MISSING_KRUNFW} exists; remove it first").into());
    }
    let (job, listener) = prepare(
        POWEROFF_JOB,
        Kernel::new(MISSING_KRUNFW, sha256),
        60_000,
        CONSOLE_CAP_BYTES,
    )?;
    let run = run_job(
        &vmm,
        &job,
        listener,
        LaunchLimits::new(Duration::from_mins(1)),
    )?;
    print_run("k7_09_kernel_missing", &run, job.files.console())?;

    assert_eq!(run.exit_code(), Some(66), "a missing kernel must exit 66");
    assert!(run.console().is_empty(), "the guest wrote to the console");
    assert_eq!(frames(&run), 0);
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see 02-06-PLAN.md E-1"]
fn k7_10_injected_cmdline_env() -> TestResult {
    const NAME: &str = "MSB_KRUN_KERNEL_CMDLINE";
    let vmm = signed_vmm()?;
    let (job, listener) = prepare(POWEROFF_JOB, real_kernel()?, 60_000, CONSOLE_CAP_BYTES)?;
    let mut helper = spawn_helper(&vmm, &job.files, &[(NAME, "init=/bin/sh")])?;
    let (status, after) = wait_exit(&mut helper, Duration::from_secs(30))?;
    println!(
        "== k7_10_injected_cmdline_env: status {}, exited after {after:?}, pending connections {}",
        describe(status),
        pending_connections(&listener)?
    );
    print_files("k7_10_injected_cmdline_env", &job.files)?;

    assert_eq!(status.code(), Some(66), "an injected {NAME} must exit 66");
    assert!(stderr_has(&job.files, NAME)?, "stderr does not name {NAME}");
    assert!(
        read_capped(job.files.console(), CONSOLE_CAP_BYTES)?.is_none_or(|bytes| bytes.is_empty()),
        "the console has output"
    );
    Ok(())
}

/// The intermediate parent for K7-07 (E-1). Does nothing unless `MINOS_SPIKE_ORPHAN_CONFIG` is set. Otherwise it
/// spawns `[MINOS_VMM, config]` exactly as `supervise::run` would, writes the helper's PID to the job directory,
/// and returns without waiting, so this process exits and the helper is reparented.
#[test]
#[ignore = "entry point for k7_07_parent_gone; does nothing unless MINOS_SPIKE_ORPHAN_CONFIG is set (E-1)"]
fn orphan_launcher() -> TestResult {
    if std::env::var_os(ORPHAN_CONFIG).is_none() {
        println!("orphan_launcher: {ORPHAN_CONFIG} is not set; nothing to do");
        return Ok(());
    }
    let vmm = env_path("MINOS_VMM")?;
    let config = env_path(ORPHAN_CONFIG)?;
    let dir = env_path(ORPHAN_DIR)?;
    let stdout = File::create_new(dir.join(supervise::VMM_STDOUT_FILE))?;
    let stderr = File::create_new(dir.join(supervise::VMM_STDERR_FILE))?;
    let child = Command::new(&vmm)
        .arg(&config)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()?;
    let pid = child.id();
    File::create_new(dir.join(PID_FILE))?.write_all(pid.to_string().as_bytes())?;
    if std::env::var_os(ORPHAN_WAIT_BOOT).is_some() {
        let start = Instant::now();
        let console = dir.join(supervise::CONSOLE_FILE);
        while !read_capped(&console, CONSOLE_CAP_BYTES)?
            .is_some_and(|bytes| contains(&bytes, BOOT_MARKER))
        {
            if start.elapsed() >= BOOT_WAIT {
                return Err("no boot marker within 30 s".into());
            }
            thread::sleep(POLL);
        }
        println!("orphan_launcher: boot marker after {:?}", start.elapsed());
    }
    println!(
        "orphan_launcher: pid {} spawned minos-vmm pid {pid}; returning without waiting for it to exit",
        std::process::id()
    );
    Ok(())
}
