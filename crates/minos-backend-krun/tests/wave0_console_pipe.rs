//! Wave 0 proof (05-03-PLAN.md, KRUN-01, 05-RESEARCH Pattern 7, Assumptions A3 and A4): the guest console on the
//! helper's own stdout pipe, under the deny-default Seatbelt profile.
//!
//! The launch config names the console `/dev/fd/1` (`minos-vmm`'s `STDOUT_CONSOLE` literal) and the profile's
//! `CONSOLE_LOG` parameter is the same literal. The test spawns the helper argv directly with `std::process`, with
//! an empty environment, stdin from `/dev/null`, stdout and stderr piped, and its own process group. One thread
//! reads stdout through `take(cap + 1)`; another accepts the vsock listener, waiting with `rustix::event::poll`, for
//! the status frame. A3: the console bytes arrive on the pipe under Seatbelt. A4: the helper creates no regular file
//! for the console. The cap test shows the parent's cap is exact: the reader stops at cap + 1 bytes and the test
//! kills the helper's process group.
//!
//! Every test is ignored; a missing variable or an unentitled helper is an error carrying the setup commands, never a
//! skip. Guest-derived text is printed only with control and format characters escaped.
//!
//! Run: `cargo test -p minos-backend-krun --test wave0_console_pipe -- --ignored --nocapture --test-threads=1`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{ErrorKind, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
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
/// The helper's own stdout, `minos-vmm`'s `STDOUT_CONSOLE` literal.
const STDOUT_CONSOLE: &str = "/dev/fd/1";

/// The guest's vsock port for the status frame; the jobs below name the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// The console cap: the K7-04 figure, 256 KiB.
const CONSOLE_CAP: u64 = 262_144;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// The helper's own deadline.
const MAX_DURATION_MS: u64 = 60_000;
/// The host's deadline: the helper's plus the 30 s grace.
const HOST_DEADLINE: Duration = Duration::from_secs(90);
/// Most bytes kept from the helper's stderr.
const STDERR_CAP: u64 = 65_536;
/// Most bytes kept from one vsock connection.
const CONNECTION_CAP: u64 = 1_048_576;
/// Longest time spent reading one vsock connection.
const CONNECTION_DEADLINE: Duration = Duration::from_secs(5);
/// How long one listener poll waits before the accept thread checks whether the helper exited.
const ACCEPT_POLL: Duration = Duration::from_millis(100);
/// Most characters of console printed for the flood run.
const FLOOD_EXCERPT_CHARS: usize = 400;

/// `report_boot` only, then power off.
const REPORT_BOOT_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}}],"finish":"poweroff"}"#;
/// `report_boot`, then 4 MiB of console output, then power off.
const FLOOD_JOB: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"console_flood":{"bytes":4194304}}],"finish":"poweroff"}"#;

/// The status frame magic, version, and header length (`minos-guest` `report.rs`).
const FRAME_MAGIC: &[u8; 8] = b"MINOSST1";
const FRAME_VERSION: u32 = 1;
const FRAME_MAX_BYTES: u64 = 1_048_576;

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

/// The first `max` characters of [`printable`].
fn printable_prefix(bytes: &[u8], max: usize) -> String {
    printable(bytes).chars().take(max).collect()
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

/// Reads one connection to EOF through `cap + 1` bytes, for at most [`CONNECTION_DEADLINE`].
fn read_connection(stream: UnixStream, cap: u64) -> TestResult<Vec<u8>> {
    stream.set_nonblocking(true)?;
    let until = Instant::now()
        .checked_add(CONNECTION_DEADLINE)
        .ok_or("deadline overflow")?;
    let mut reader = stream.take(cap.saturating_add(1));
    let mut bytes = Vec::new();
    let mut buf = vec![0_u8; 65_536];
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

/// Accepts connections until `exited` is set and no connection is pending, reading each through its cap.
fn accept_connections(listener: &UnixListener, exited: &AtomicBool) -> TestResult<Vec<Vec<u8>>> {
    let mut connections = Vec::new();
    loop {
        if readable(listener, ACCEPT_POLL)? {
            match listener.accept() {
                Ok((stream, _)) => connections.push(read_connection(stream, CONNECTION_CAP)?),
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::WouldBlock
                            | ErrorKind::Interrupted
                            | ErrorKind::ConnectionAborted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        } else if exited.load(Ordering::SeqCst) {
            return Ok(connections);
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

/// What one console-pipe launch left behind.
struct PipeRun {
    status: ExitStatus,
    elapsed: Duration,
    console: Vec<u8>,
    stderr: Vec<u8>,
    connections: Vec<Vec<u8>>,
    killed_by_test: bool,
    job_dir_entries: BTreeSet<String>,
    expected_entries: BTreeSet<String>,
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

/// Launches `job` with the console on the helper's stdout. With `kill_at_cap`, the test kills the helper's process
/// group as soon as the stdout reader holds `CONSOLE_CAP + 1` bytes.
fn pipe_run(label: &str, job: &str, kill_at_cap: bool) -> TestResult<PipeRun> {
    let vmm = signed_vmm()?;
    let image = minimal_root_image(&guest()?)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
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
        Console::new(STDOUT_CONSOLE, CONSOLE_CAP),
        MAX_DURATION_MS,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let expected_entries: BTreeSet<String> = [
        files.root(),
        files.input(),
        files.scratch(),
        files.socket(),
        files.config(),
    ]
    .iter()
    .filter_map(|path| path.file_name())
    .map(|name| name.to_string_lossy().into_owned())
    .collect();
    let params = BTreeMap::from([
        ("VMM", vmm.clone()),
        ("CONFIG", canonical_entry(files.config())?),
        ("KRUNFW", fs::canonicalize(&krunfw)?),
        ("KRUNFW_LINK", canonical_entry(Path::new(KRUNFW_LINK))?),
        ("ROOT_IMG", canonical_entry(files.root())?),
        ("INPUT_IMG", canonical_entry(files.input())?),
        ("SCRATCH_IMG", canonical_entry(files.scratch())?),
        ("CONSOLE_LOG", PathBuf::from(STDOUT_CONSOLE)),
        ("SOCK", canonical_entry(files.socket())?),
    ]);
    let argv = seatbelt_argv(&profile()?, &params, &vmm, &[files.config().as_os_str()]);
    print_argv(label, &argv);
    let (program, args) = argv.split_first().ok_or("empty argv")?;

    let start = Instant::now();
    let deadline = start
        .checked_add(HOST_DEADLINE)
        .ok_or("deadline overflow")?;
    let mut child = Reaper(
        Command::new(program)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()?,
    );
    let group = Pid::from_child(&child.0);
    let mut stdout = child.0.stdout.take().ok_or("no stdout pipe")?;
    let stderr = child.0.stderr.take().ok_or("no stderr pipe")?;
    let exited = Arc::new(AtomicBool::new(false));
    let (full_tx, full_rx) = mpsc::channel::<()>();

    let stdout_reader = thread::spawn(move || {
        let mut console = Vec::new();
        let read = (&mut stdout)
            .take(CONSOLE_CAP.saturating_add(1))
            .read_to_end(&mut console);
        if u64::try_from(console.len()).is_ok_and(|len| len > CONSOLE_CAP) {
            full_tx.send(()).ok();
        }
        // The pipe is handed back, not dropped: a closed read end would end a writing helper with EPIPE before
        // the test's own signal.
        (stdout, console, read.err())
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stderr
            .take(STDERR_CAP.saturating_add(1))
            .read_to_end(&mut bytes);
        (bytes, read.err())
    });
    let accept_exited = Arc::clone(&exited);
    let acceptor = thread::spawn(move || {
        accept_connections(&listener, &accept_exited).map_err(|error| error.to_string())
    });

    let mut killed_by_test = false;
    let status = loop {
        if kill_at_cap && !killed_by_test && full_rx.try_recv().is_ok() {
            kill_process_group(group, Signal::KILL)?;
            killed_by_test = true;
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            kill_process_group(group, Signal::KILL)?;
            break child.0.wait()?;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let elapsed = start.elapsed();
    exited.store(true, Ordering::SeqCst);
    let (stdout_pipe, console, stdout_error) =
        stdout_reader.join().map_err(|_| "stdout reader panicked")?;
    drop(stdout_pipe);
    let (stderr_bytes, stderr_error) =
        stderr_reader.join().map_err(|_| "stderr reader panicked")?;
    let connections = acceptor.join().map_err(|_| "acceptor panicked")??;
    let job_dir_entries = fs::read_dir(files.dir())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()?;

    println!(
        "== {label}: status {status:?}, exit code {:?}, signal {:?}, killed_by_test {killed_by_test}, elapsed {elapsed:?}, connections {}",
        status.code(),
        status.signal(),
        connections.len()
    );
    println!(
        "-- stdout read error {:?}, stderr read error {:?}",
        stdout_error.map(|error| error.to_string()),
        stderr_error.map(|error| error.to_string())
    );
    println!(
        "-- job directory entries: {}",
        printable(format!("{job_dir_entries:?}").as_bytes())
    );
    Ok(PipeRun {
        status,
        elapsed,
        console,
        stderr: stderr_bytes,
        connections,
        killed_by_test,
        job_dir_entries,
        expected_entries,
    })
}

/// Prints the connections of a run, each decoded as a frame or explained.
fn print_connections(run: &PipeRun) -> TestResult {
    for (index, connection) in run.connections.iter().enumerate() {
        match status_frame(connection) {
            Ok(frame) => println!(
                "-- connection {index} ({} bytes), frame:\n{}",
                connection.len(),
                printable(serde_json::to_string_pretty(&frame)?.as_bytes())
            ),
            Err(error) => println!(
                "-- connection {index} ({} bytes): not a frame: {error}",
                connection.len()
            ),
        }
    }
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn console_pipe_carries_boot_output() -> TestResult {
    let label = "console_pipe_carries_boot_output";
    let run = pipe_run(label, REPORT_BOOT_JOB, false)?;
    println!(
        "-- console on the stdout pipe ({} bytes):\n{}",
        run.console.len(),
        printable(&run.console)
    );
    println!(
        "-- minos-vmm stderr ({} bytes):\n{}",
        run.stderr.len(),
        printable(&run.stderr)
    );
    print_connections(&run)?;
    println!("== end {label} ({:?})", run.elapsed);

    assert_eq!(run.status.code(), Some(0));
    assert!(
        run.console
            .windows(b"MINOS-GUEST-BOOT-OK".len())
            .any(|window| window == b"MINOS-GUEST-BOOT-OK"),
        "the stdout pipe lacks MINOS-GUEST-BOOT-OK"
    );
    let [connection] = run.connections.as_slice() else {
        return Err(format!(
            "expected exactly one connection, got {}",
            run.connections.len()
        )
        .into());
    };
    let frame = status_frame(connection)?;
    let booted = frame
        .get("steps")
        .and_then(Value::as_array)
        .is_some_and(|steps| steps.iter().any(|step| step.get("report_boot").is_some()));
    assert!(booted, "the frame has no report_boot result");
    assert_eq!(
        run.job_dir_entries, run.expected_entries,
        "the helper created a file in the job directory (A4)"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn console_pipe_cap_is_exact() -> TestResult {
    let label = "console_pipe_cap_is_exact";
    let run = pipe_run(label, FLOOD_JOB, true)?;
    println!(
        "-- console on the stdout pipe ({} bytes), first {FLOOD_EXCERPT_CHARS} characters:\n{}",
        run.console.len(),
        printable_prefix(&run.console, FLOOD_EXCERPT_CHARS)
    );
    println!(
        "-- minos-vmm stderr ({} bytes):\n{}",
        run.stderr.len(),
        printable(&run.stderr)
    );
    print_connections(&run)?;
    println!("== end {label} ({:?})", run.elapsed);

    assert!(run.killed_by_test, "the reader never reached cap + 1 bytes");
    assert_eq!(run.console.len(), 262_145, "the reader must hold cap + 1");
    assert_eq!(
        run.status.signal(),
        Some(Signal::KILL.as_raw()),
        "the helper must end by the test's SIGKILL"
    );
    assert_eq!(
        run.job_dir_entries, run.expected_entries,
        "the helper created a file in the job directory (A4)"
    );
    Ok(())
}
