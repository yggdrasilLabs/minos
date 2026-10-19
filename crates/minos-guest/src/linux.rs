//! The Linux half of the guest: PID 1, the stage trampoline, and the Linux implementations of the four traits
//! [`run_job`] works through (GUEST-01, GUEST-03, GUEST-05; 05-CONTEXT D-05, D-07, D-08).
//!
//! This is the only Linux-only file in the guest; `main.rs` gates the module (02-CONTEXT.md D-03).
//!
//! **PID 1** ([`pid1`]) does boot-time work only until it reads the job: the boot marker, `/proc` with
//! `hidepid=invisible` (a stage cannot see PID 1's or any other uid's processes), and `/sys` read-only. Nothing
//! per-run exists before the job arrives (CLAUDE.md: a Lambda snapshot captures memory as well as disk). Then, in
//! order:
//!
//! 1. it reads the input disk `/dev/vdb` once, closes it, and removes the device node (mode 0 if the removal
//!    fails), so no later process can read the per-launch token (05-RESEARCH Pitfall 1, Assumption A1);
//! 2. it mounts a tmpfs on `/scratch` capped at the job's scratch size, with no devices and no setuid;
//! 3. it calls [`run_job`] with a [`ReadJobSource`] over what it read, a [`ChunkSink`] over [`VsockConnector`],
//!    a [`LinuxExecutor`], and a [`LinuxHandover`], which bind-mounts `/scratch/tmp` on `/tmp` and
//!    `/scratch/out` on `/out` as it hands them over, so a stage can write only scratch (the root image is
//!    read-only `SquashFS`);
//! 4. it writes one console line saying what was sent, syncs, and powers off.
//!
//! A PID 1 panic is a kernel panic; with `panic=-1` the VM ends without any authenticated output, which the
//! host classifies as `Crashed` (02-SPIKE-EVIDENCE.md P-PANIC).
//!
//! **The trampoline** ([`trampoline`]) is `/init --minos-stage <limits> -- <argv>`, spawned by PID 1 already as
//! the stage uid. It refuses to run as PID 1 or as root, sets every stage rlimit with the current and maximum
//! value equal, and `exec`s the command; a limit it cannot set ends it with 126 before any candidate code runs
//! (D-08; 05-RESEARCH Pattern 5). No pre-exec hook is used: std's is `unsafe`, which is forbidden here.
//!
//! **A stage** runs as uid and gid 64000 with no supplementary groups, the environment cleared to
//! [`STAGE_ENV`], stdin from `/dev/null`, stdout and stderr on one pipe drained into a capped buffer, and its own
//! process group. PID 1 waits for that one child by pid, never for any child while the stage runs, and at the
//! stage's deadline records `TimedOut`. Then it always kills every process but itself with `kill(-1, SIGKILL)`,
//! which also reaches a process that left the group with `setsid`, and reaps until no child remains, so nothing
//! the candidate started can race the packer (05-RESEARCH Patterns 5 and 6, Pitfalls 7 and 12).

use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io::{self, BufReader, PipeReader, PipeWriter, Read, Write};
use std::net::Shutdown;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use minos_core::guest::{OUTPUT_VSOCK_PORT, StageEnd};
use minos_guest::pack::LOG_CAP_BYTES;
use minos_guest::run::{Handover, JobEnd, StageCommand, StageExecutor, StageRun, run_job};
use minos_guest::sink::{ChunkSink, Connector};
use minos_guest::source::{DiskJobSource, GuestInput, JobSource, ReadJobSource, SourceFailure};
use minos_guest::stage::{
    GUEST_GID, GUEST_UID, OUT_MOUNT, SCRATCH_MOUNT, SCRATCH_OUT_DIR, SCRATCH_TMP_DIR, STAGE_ENV,
    TMP_MOUNT, parse_trampoline, scratch_mount_data, trampoline_argv,
};
use minos_guest::{BOOT_MARKER, OUTPUT_SENT_MARKER, POWEROFF_MARKER, SETUP_ERROR_MARKER};
use rustix::fs::{AtFlags, CWD, Gid, Mode, OFlags, Uid, chmod, chownat, fcntl_setfl, sync, unlink};
use rustix::io::Errno;
use rustix::mount::{MountFlags, mount, mount_bind};
use rustix::process::{
    Pid, Resource, Rlimit, Signal, WaitOptions, WaitStatus, geteuid, getpid, kill_process_group,
    setrlimit, wait, waitpid,
};
use rustix::system::{RebootCommand, reboot};
use vsock::{VMADDR_CID_HOST, VsockStream};

/// Where the trampoline lives: PID 1's own binary.
const INIT_PATH: &str = "/init";
/// The input disk: the second virtio-blk device, attached read-only by the launcher. Read once, then removed.
const INPUT_DISK: &str = "/dev/vdb";
/// Flags on `/proc` and `/sys`: nothing on either is a device, setuid, or executable. `/sys` is also read-only,
/// so a stage cannot write `/sys/power/state` and turn its own failure into an unexplained power-off.
const PSEUDO_FLAGS: MountFlags = MountFlags::NOSUID
    .union(MountFlags::NODEV)
    .union(MountFlags::NOEXEC);
/// `/proc` hides every process the reader cannot ptrace, PID 1 included, from a stage.
const PROC_DATA: &std::ffi::CStr = c"hidepid=invisible";
/// Flags on the scratch tmpfs: no devices and no setuid. Execution stays allowed, because builds run the programs
/// they compile there. The `/tmp` and `/out` bind mounts inherit these flags from it.
const SCRATCH_FLAGS: MountFlags = MountFlags::NOSUID.union(MountFlags::NODEV);
/// The trampoline's exit code when it refuses to run or cannot set a limit: no candidate code ran.
const TRAMPOLINE_REFUSED: u8 = 126;
/// The trampoline's exit code when `exec` of the stage command failed.
const TRAMPOLINE_EXEC_FAILED: u8 = 127;
/// How often PID 1 polls the stage child while it runs.
const STAGE_POLL: Duration = Duration::from_millis(10);
/// How often the drain thread looks at an empty pipe.
const DRAIN_POLL: Duration = Duration::from_millis(5);
/// Read size of the drain thread.
const DRAIN_BUF: usize = 64 * 1024;
/// Most time PID 1 spends killing and reaping after a stage.
const REAP_BOUND: Duration = Duration::from_secs(5);
/// How often PID 1 repeats the kill while children remain.
const REAP_POLL: Duration = Duration::from_millis(10);

// ---------------------------------------------------------------------------------------------------------------
// PID 1
// ---------------------------------------------------------------------------------------------------------------

/// Runs the guest as PID 1. Never returns: it powers off.
pub fn pid1() -> ! {
    console_line(BOOT_MARKER);
    mount_at("proc", "/proc", "proc", PSEUDO_FLAGS, Some(PROC_DATA));
    mount_at(
        "sysfs",
        "/sys",
        "sysfs",
        PSEUDO_FLAGS.union(MountFlags::RDONLY),
        None,
    );

    // Per-run work starts here. Nothing above depends on the job.
    let read = read_input();
    protect_input_disk();
    let Some(read) = read else { power_off() };
    let token = match &read {
        Ok(input) => input.token().clone(),
        Err(failure) => {
            if let Some(token) = failure.token() {
                token.clone()
            } else {
                console_line(&format!("{SETUP_ERROR_MARKER} {}", chain(failure.error())));
                power_off()
            }
        }
    };
    if let Ok(input) = &read {
        mount_scratch(input.job().scratch_mib());
    }

    let mut source = ReadJobSource::new(read);
    let mut sink = ChunkSink::new(VsockConnector, token);
    let mut executor = LinuxExecutor;
    let mut handover = LinuxHandover::default();
    let end = run_job(
        &mut source,
        &mut sink,
        &mut executor,
        &mut handover,
        Path::new(SCRATCH_MOUNT),
        vcpus(),
    );
    report_end(&end);
    sync();
    power_off()
}

/// The VM's vCPU count, as the kernel reports it to PID 1, which runs on every vCPU. Each stage's `RLIMIT_CPU`
/// is scaled by it (review CR-01). A count the kernel cannot report is taken as one, and a count over `u8` is
/// saturated; `StageLimits::for_stage` clamps it to the sandbox maximum.
fn vcpus() -> u8 {
    thread::available_parallelism().map_or(1, |count| u8::try_from(count.get()).unwrap_or(u8::MAX))
}

/// Opens the input disk, reads it once through a [`DiskJobSource`], and closes it. `None` when it cannot be
/// opened, after a setup-error console line: without the disk there is no token, so nothing can be sent.
fn read_input() -> Option<Result<GuestInput, SourceFailure>> {
    let disk = match File::open(INPUT_DISK) {
        Ok(disk) => disk,
        Err(error) => {
            console_line(&format!(
                "{SETUP_ERROR_MARKER} open {INPUT_DISK}: {}",
                error.kind()
            ));
            return None;
        }
    };
    // The source and the file it owns are dropped at the end of this statement, so the disk is closed before
    // its node is removed.
    Some(DiskJobSource::new(BufReader::new(disk)).read_input())
}

/// Removes the input disk's device node, or makes it mode 0 if the removal fails, so no later process can open
/// it and read the token. The stage uid could not open the node anyway (devtmpfs makes block devices root-owned
/// and not world-readable); this keeps the token unreadable even to a process that gained a group.
fn protect_input_disk() {
    if let Err(errno) = unlink(INPUT_DISK) {
        console_line(&format!("minos-guest: unlink {INPUT_DISK}: {errno}"));
        if let Err(errno) = chmod(INPUT_DISK, Mode::empty()) {
            console_line(&format!("minos-guest: chmod {INPUT_DISK}: {errno}"));
        }
    }
}

/// Mounts the scratch tmpfs. A failure is a console line; `run_job` then cannot create its directories on the
/// read-only root and sends an authenticated `setup: failed {mount}` status.
fn mount_scratch(scratch_mib: u16) {
    match CString::new(scratch_mount_data(scratch_mib)) {
        Ok(data) => mount_at(
            "tmpfs",
            SCRATCH_MOUNT,
            "tmpfs",
            SCRATCH_FLAGS,
            Some(data.as_c_str()),
        ),
        Err(_) => console_line("minos-guest: the scratch mount data holds a NUL"),
    }
}

/// Writes what [`run_job`] did as console lines: the output-sent marker with the connection count, or why
/// nothing was sent, then the first setup error, if any.
fn report_end(end: &JobEnd) {
    match end.sent() {
        Some(Ok(report)) => console_line(&format!("{OUTPUT_SENT_MARKER} {}", report.connections())),
        Some(Err(error)) => console_line(&format!("minos-guest: output not sent: {error}")),
        None => console_line(&format!("{SETUP_ERROR_MARKER} no output was packed")),
    }
    if let Some(error) = end.error() {
        console_line(&format!("minos-guest: {}", chain(error)));
    }
}

/// `error` and its sources, joined by `": "`. Every guest error message is fixed text and numbers.
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(next) = source {
        text.push_str(": ");
        text.push_str(&next.to_string());
        source = next.source();
    }
    text
}

/// Writes one line to the console (stdout is `/dev/console`). A console write error has nowhere to be reported,
/// so it is dropped rather than allowed to panic the way `println!` would.
fn console_line(text: &str) {
    let mut out = io::stdout().lock();
    let written = writeln!(out, "{text}").and_then(|()| out.flush());
    drop(written);
}

/// Mounts `fstype` at `target`. A failure is printed on the console; the step that needs the mount fails on its
/// own and reports it.
fn mount_at(
    source: &str,
    target: &str,
    fstype: &str,
    flags: MountFlags,
    data: Option<&std::ffi::CStr>,
) {
    if let Err(errno) = mount(source, target, fstype, flags, data) {
        console_line(&format!("minos-guest: mount {target}: {errno}"));
    }
}

/// Asks the kernel to power off. PID 1 must never exit, so if the call fails the guest prints the error and
/// sleeps until the host deadline ends the VM.
fn power_off() -> ! {
    console_line(POWEROFF_MARKER);
    if let Err(errno) = reboot(RebootCommand::PowerOff) {
        console_line(&format!("minos-guest: power off: {errno}"));
    }
    loop {
        thread::sleep(Duration::from_hours(1));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The trampoline
// ---------------------------------------------------------------------------------------------------------------

/// Runs as `/init --minos-stage <limits> -- <argv>`: sets the stage's limits, then `exec`s the command.
///
/// Exits 126 without `exec` when it is PID 1 or root, when its argv is not the one PID 1 writes, or when any
/// limit cannot be set, so no candidate code ever runs without its limits. Exits 127 when `exec` fails.
pub fn trampoline() -> ExitCode {
    if getpid().is_init() || geteuid().is_root() {
        stderr_line(
            "minos-guest: the stage trampoline runs only as the stage uid, never as PID 1 or root",
        );
        return ExitCode::from(TRAMPOLINE_REFUSED);
    }
    let args: Vec<OsString> = std::env::args_os().collect();
    let (limits, argv) = match parse_trampoline(&args) {
        Ok(parsed) => parsed,
        Err(error) => {
            stderr_line(&format!("minos-guest: {error}"));
            return ExitCode::from(TRAMPOLINE_REFUSED);
        }
    };
    let wanted = [
        (Resource::As, "RLIMIT_AS", limits.address_space_bytes()),
        (Resource::Fsize, "RLIMIT_FSIZE", limits.file_size_bytes()),
        (Resource::Nproc, "RLIMIT_NPROC", limits.processes()),
        (Resource::Cpu, "RLIMIT_CPU", limits.cpu_seconds()),
        (Resource::Nofile, "RLIMIT_NOFILE", limits.open_files()),
        (Resource::Core, "RLIMIT_CORE", 0),
    ];
    for (resource, name, value) in wanted {
        let limit = Rlimit {
            current: Some(value),
            maximum: Some(value),
        };
        if let Err(errno) = setrlimit(resource, limit) {
            stderr_line(&format!("minos-guest: setting {name}: {errno}"));
            return ExitCode::from(TRAMPOLINE_REFUSED);
        }
    }
    let mut argv = argv.into_iter();
    let Some(program) = argv.next() else {
        stderr_line("minos-guest: the stage has no command");
        return ExitCode::from(TRAMPOLINE_REFUSED);
    };
    let error = Command::new(program).args(argv).exec();
    stderr_line(&format!("minos-guest: exec: {}", error.kind()));
    ExitCode::from(TRAMPOLINE_EXEC_FAILED)
}

/// Writes one line to stderr, which for a stage is the captured log. A write error is dropped.
fn stderr_line(text: &str) {
    drop(writeln!(io::stderr(), "{text}"));
}

// ---------------------------------------------------------------------------------------------------------------
// The trait implementations
// ---------------------------------------------------------------------------------------------------------------

/// Opens one vsock connection per output chunk to the host CID on [`OUTPUT_VSOCK_PORT`].
#[derive(Debug, Clone, Copy)]
pub struct VsockConnector;

impl Connector for VsockConnector {
    type Stream = VsockStream;

    fn connect(&mut self) -> io::Result<VsockStream> {
        VsockStream::connect_with_cid_port(VMADDR_CID_HOST, OUTPUT_VSOCK_PORT)
    }

    fn close(&mut self, stream: VsockStream) -> io::Result<()> {
        stream.shutdown(Shutdown::Write)?;
        drop(stream);
        Ok(())
    }
}

/// Hands a directory tree to the stage uid, and bind-mounts the scratch `tmp` and `out` directories on `/tmp`
/// and `/out` the first time each is handed over. [`run_job`] calls it after creating the directories and before
/// the first stage.
#[derive(Debug, Default)]
pub struct LinuxHandover {
    bound_tmp: bool,
    bound_out: bool,
}

impl Handover for LinuxHandover {
    fn hand_over(&mut self, dir: &Path) -> io::Result<()> {
        chown_tree(dir)?;
        if dir == Path::new(SCRATCH_TMP_DIR) && !self.bound_tmp {
            mount_bind(dir, TMP_MOUNT)?;
            self.bound_tmp = true;
        }
        if dir == Path::new(SCRATCH_OUT_DIR) && !self.bound_out {
            mount_bind(dir, OUT_MOUNT)?;
            self.bound_out = true;
        }
        Ok(())
    }
}

/// Changes the owner of `root` and everything under it to the stage uid and gid, never following a link: each
/// entry is examined with `symlink_metadata` and changed with `fchownat(AT_SYMLINK_NOFOLLOW)`, and only a real
/// directory is descended into. No stage has run yet, so nothing else changes the tree meanwhile.
fn chown_tree(root: &Path) -> io::Result<()> {
    let uid = Some(Uid::from_raw(GUEST_UID));
    let gid = Some(Gid::from_raw(GUEST_GID));
    let mut pending: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path)?;
        chownat(CWD, &path, uid, gid, AtFlags::SYMLINK_NOFOLLOW)?;
        if meta.file_type().is_dir() {
            for entry in fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(())
}

/// Runs each stage through the trampoline as the stage uid, under its deadline, then kills and reaps every
/// process but PID 1.
#[derive(Debug, Clone, Copy)]
pub struct LinuxExecutor;

impl StageExecutor for LinuxExecutor {
    fn run_stage(&mut self, command: &StageCommand) -> StageRun {
        let (reader, writer) = match io::pipe() {
            Ok(pipe) => pipe,
            Err(error) => return spawn_failed("pipe", &error),
        };
        if let Err(errno) = fcntl_setfl(&reader, OFlags::NONBLOCK) {
            return spawn_failed("pipe", &errno.into());
        }
        let stop = AtomicBool::new(false);
        thread::scope(|scope| {
            let stop = &stop;
            let drain = match thread::Builder::new()
                .name("minos-drain".to_owned())
                .spawn_scoped(scope, move || drain(reader, stop))
            {
                Ok(drain) => drain,
                Err(error) => return spawn_failed("drain thread", &error),
            };
            let end = match spawn_stage(command, writer) {
                Ok(child) => wait_stage(child, command.deadline()),
                Err(error) => {
                    console_line(&format!("minos-guest: spawn stage: {}", error.kind()));
                    StageEnd::SpawnFailed
                }
            };
            kill_all_and_reap();
            stop.store(true, Ordering::Release);
            let log = drain.join().unwrap_or_default();
            StageRun::new(end, log)
        })
    }
}

/// A stage that could not be started because PID 1 could not set up `what`.
fn spawn_failed(what: &str, error: &io::Error) -> StageRun {
    console_line(&format!("minos-guest: stage {what}: {}", error.kind()));
    StageRun::new(StageEnd::SpawnFailed, Vec::new())
}

/// Spawns the trampoline for `command` as the stage uid, with `writer` as its stdout and stderr. The command and
/// with it PID 1's copies of the write end are dropped before this returns, so the drain sees the end of the
/// pipe once every stage process is gone.
fn spawn_stage(command: &StageCommand, writer: PipeWriter) -> io::Result<Pid> {
    let stderr = writer.try_clone()?;
    let argv = trampoline_argv(Path::new(INIT_PATH), &command.limits(), command.argv());
    let mut argv = argv.into_iter();
    let program = argv.next().unwrap_or_else(|| OsString::from(INIT_PATH));
    let mut spawn = Command::new(program);
    spawn
        .args(argv)
        // std drops supplementary groups (setgroups(0)) when a root parent sets a uid.
        .uid(GUEST_UID)
        .gid(GUEST_GID)
        .env_clear()
        .envs(STAGE_ENV)
        .current_dir(command.cwd())
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(stderr);
    let child = spawn.spawn();
    drop(spawn);
    child.map(|child| Pid::from_child(&child))
}

/// Waits for the stage child `child`, by its pid only, until it ends or `deadline` passes. Never waits for any
/// other child, so an orphan the stage left cannot be reaped in its place.
fn wait_stage(child: Pid, deadline: Duration) -> StageEnd {
    let started = Instant::now();
    let mut reported = false;
    loop {
        match waitpid(Some(child), WaitOptions::NOHANG) {
            Ok(Some((_, status))) => {
                if let Some(end) = end_of(status) {
                    return end;
                }
            }
            Ok(None) | Err(Errno::INTR) => {}
            Err(errno) => {
                // Not expected for PID 1's own unreaped child. The stage is then treated as running until its
                // deadline, and killed with everything else.
                if !reported {
                    console_line(&format!("minos-guest: waitpid: {errno}"));
                    reported = true;
                }
            }
        }
        if started.elapsed() >= deadline {
            return StageEnd::TimedOut;
        }
        thread::sleep(STAGE_POLL);
    }
}

/// How a stage that changed state ended, or `None` for a state that is not an end.
fn end_of(status: WaitStatus) -> Option<StageEnd> {
    if let Some(code) = status.exit_status() {
        return Some(StageEnd::Exited { code });
    }
    status
        .terminating_signal()
        .map(|signal| StageEnd::Signaled { signal })
}

/// Kills every process but PID 1 with `kill(-1, SIGKILL)` and reaps until no child remains, repeating the kill
/// while children remain, for at most [`REAP_BOUND`].
fn kill_all_and_reap() {
    let started = Instant::now();
    loop {
        match kill_process_group(Pid::INIT, Signal::KILL) {
            Ok(()) | Err(Errno::SRCH) => {}
            Err(errno) => console_line(&format!("minos-guest: kill every process: {errno}")),
        }
        loop {
            match wait(WaitOptions::NOHANG) {
                Ok(Some(_)) | Err(Errno::INTR) => {}
                Ok(None) => break,
                Err(Errno::CHILD) => return,
                Err(errno) => {
                    console_line(&format!("minos-guest: reap: {errno}"));
                    return;
                }
            }
        }
        if started.elapsed() >= REAP_BOUND {
            console_line("minos-guest: processes remain after the kill");
            return;
        }
        thread::sleep(REAP_POLL);
    }
}

/// Drains the stage pipe into a buffer of at most [`LOG_CAP_BYTES`] plus one byte, so the packer can tell that
/// the log was cut. Bytes beyond the cap are read and discarded, so a stage never stalls on a full pipe.
///
/// The pipe is non-blocking. The drain ends at the end of the pipe, on a read error, or once `stop` is set and
/// one more pass finds the pipe empty: PID 1 sets `stop` after every stage process is gone, and the extra pass
/// collects whatever they wrote before dying.
fn drain(mut reader: PipeReader, stop: &AtomicBool) -> Vec<u8> {
    let cap = LOG_CAP_BYTES.saturating_add(1);
    let mut kept = Vec::new();
    let mut buf = vec![0_u8; DRAIN_BUF];
    let mut last_pass = false;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => {
                let room = cap.saturating_sub(kept.len());
                if let Some(part) = buf.get(..read.min(room)) {
                    kept.extend_from_slice(part);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if last_pass {
                    break;
                }
                if stop.load(Ordering::Acquire) {
                    last_pass = true;
                } else {
                    thread::sleep(DRAIN_POLL);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    kept
}
