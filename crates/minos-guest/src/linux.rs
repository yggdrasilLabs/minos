//! The PID 1 path: print the boot marker, mount `/proc`, `/sys`, and a tmpfs on `/tmp`, read the job from
//! `/dev/vdb`, run its steps in order, send one status frame to the host over vsock, then power off or return
//! as the job says.
//!
//! This is the only Linux-only file in the guest; `main.rs` gates the module (02-CONTEXT.md D-03). Every step
//! failure becomes a field in that step's result. Nothing here panics on bad input, and nothing returns early
//! from [`run`] except the job's own `return` finish.
//!
//! A `run` step owns a process group and a deadline. Its pipes are drained without blocking, so a daemon that
//! inherits stdout cannot hold the step open after the command exits, and at the deadline the whole group is
//! killed, so a command that sleeps forever costs the job one step's timeout rather than the host deadline and
//! every earlier result. Orphans are reaped before every step as well as after a `run`.

use std::ffi::CStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpStream};
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use minos_guest::job::{Connect, Finish, Job, JobError, Run, read_job};
use minos_guest::job::{ConsoleFlood, ScratchRoundtrip, Step, VsockConnect, VsockStream};
use minos_guest::pattern::{fill_pattern, pattern_byte};
use minos_guest::report::{
    BootResult, ConnectAttempt, ConnectResult, ConsoleFloodResult, Report, RunResult,
    ScratchResult, StepResult, VsockConnectResult, VsockStreamResult, encode_frame,
};
use minos_guest::{BOOT_MARKER, FRAME_SENT_MARKER, JOB_ERROR_MARKER, POWEROFF_MARKER};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::io::Errno;
use rustix::mount::{MountFlags, mount};
use rustix::process::{Pid, Signal, WaitOptions, kill_process_group, wait};
use rustix::system::{RebootCommand, reboot};
use vsock::{VMADDR_CID_HOST, VsockStream as Vsock};

/// The job disk: the second virtio-blk device, attached read-only by the launcher.
const JOB_DISK: &str = "/dev/vdb";
/// The scratch disk: the third virtio-blk device, the only writable one.
const SCRATCH_DISK: &str = "/dev/vdc";
/// Chunk size for pattern writes and reads.
const CHUNK: usize = 64 * 1024;
/// Most bytes read from any one small kernel file (`/proc/cmdline` and the like).
const SMALL_FILE_CAP: u64 = 64 * 1024;
/// Console lines from `console_flood` are 79 printable bytes and a newline.
const FLOOD_LINE: u64 = 80;
/// Pause between polls of a running command's pipes and exit status.
const RUN_POLL: Duration = Duration::from_millis(5);
/// Flags on `/proc` and `/sys`: nothing on either is a device, setuid, or executable. `/sys` is also read-only,
/// so a command cannot write `/sys/power/state` and turn its own failure into an unexplained power-off that the
/// host would classify as `Crashed` rather than attribute to the command.
const PSEUDO_FLAGS: MountFlags = MountFlags::NOSUID
    .union(MountFlags::NODEV)
    .union(MountFlags::NOEXEC);
/// Flags on the `/tmp` tmpfs: no devices and no setuid. Execution stays allowed, because build-time checks
/// compile and run small programs in a temporary directory.
const TMP_FLAGS: MountFlags = MountFlags::NOSUID.union(MountFlags::NODEV);
/// Size cap of the `/tmp` tmpfs, as a share of guest memory. The root is read-only `SquashFS`, so this and the
/// scratch disk are the only writable paths; the cap keeps a command from filling guest memory through `/tmp`.
const TMP_SIZE: &CStr = c"size=25%";

/// Runs the guest as PID 1.
pub fn run() -> ExitCode {
    console_line(BOOT_MARKER);
    mount_at("proc", "/proc", "proc", PSEUDO_FLAGS, None);
    mount_at(
        "sysfs",
        "/sys",
        "sysfs",
        PSEUDO_FLAGS.union(MountFlags::RDONLY),
        None,
    );
    mount_at("tmpfs", "/tmp", "tmpfs", TMP_FLAGS, Some(TMP_SIZE));

    let job = match load_job() {
        Ok(job) => job,
        Err(error) => {
            console_line(&format!("{JOB_ERROR_MARKER} {error}"));
            power_off()
        }
    };

    let mut report = Report::new();
    for step in job.steps() {
        reap_orphans();
        let result = match step {
            Step::ReportBoot(_) => StepResult::ReportBoot(report_boot()),
            Step::Connect(step) => StepResult::Connect(connect(step)),
            Step::VsockConnect(step) => StepResult::VsockConnect(vsock_connect(*step)),
            Step::VsockStream(step) => {
                StepResult::VsockStream(vsock_stream(job.vsock_port(), *step))
            }
            Step::ScratchRoundtrip(step) => StepResult::ScratchRoundtrip(scratch_roundtrip(*step)),
            Step::Run(step) => StepResult::Run(run_command(step)),
            Step::ConsoleFlood(step) => StepResult::ConsoleFlood(console_flood(*step)),
            Step::Hang(_) => hang(),
        };
        report.push(result);
    }

    match encode_frame(&report) {
        Ok(frame) => match send_frame(job.vsock_port(), &frame) {
            Ok(()) => console_line(&format!("{FRAME_SENT_MARKER} {} bytes", frame.len())),
            Err(error) => console_line(&format!("minos-guest: sending the status frame: {error}")),
        },
        Err(error) => console_line(&format!("minos-guest: encoding the status frame: {error}")),
    }

    match job.finish() {
        Finish::Return => ExitCode::SUCCESS,
        Finish::Poweroff => power_off(),
    }
}

/// Writes one line to the console (stdout is `/dev/console`, which is hvc0). A console write error has
/// nowhere to be reported, so it is dropped rather than allowed to panic the way `println!` would.
fn console_line(text: &str) {
    let mut out = io::stdout().lock();
    let written = writeln!(out, "{text}").and_then(|()| out.flush());
    drop(written);
}

/// Mounts `fstype` at `target`. A failure is printed on the console and the boot goes on: the packer creates
/// every mount point, and a step that needs the mount reports its own error.
fn mount_at(source: &str, target: &str, fstype: &str, flags: MountFlags, data: Option<&CStr>) {
    if let Err(error) = mount(source, target, fstype, flags, data) {
        console_line(&format!("minos-guest: mount {target}: {error}"));
    }
}

fn load_job() -> Result<Job, JobError> {
    let mut disk = File::open(JOB_DISK).map_err(JobError::Io)?;
    read_job(&mut disk)
}

/// Asks the kernel to power off. PID 1 must not exit unless the job said `return`, so if the call fails the
/// guest prints the error and sleeps until the host deadline ends the VM.
fn power_off() -> ! {
    console_line(POWEROFF_MARKER);
    if let Err(error) = reboot(RebootCommand::PowerOff) {
        console_line(&format!("minos-guest: power off: {error}"));
    }
    hang()
}

fn hang() -> ! {
    loop {
        std::thread::sleep(Duration::from_hours(1));
    }
}

fn lossy(text: &std::ffi::OsStr) -> String {
    text.to_string_lossy().into_owned()
}

/// Reads a small kernel-provided file, trimmed. An unreadable file becomes a marked string in the report.
fn read_trimmed(path: &Path) -> String {
    let mut bytes = Vec::new();
    match File::open(path).and_then(|file| file.take(SMALL_FILE_CAP).read_to_end(&mut bytes)) {
        Ok(_) => String::from_utf8_lossy(&bytes).trim().to_owned(),
        Err(error) => format!("<unreadable: {error}>"),
    }
}

/// Sorted entry names of a directory. An unreadable directory becomes one marked entry.
fn dir_names(path: &str) -> Vec<String> {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .map(|entry| match entry {
                    Ok(entry) => lossy(&entry.file_name()),
                    Err(error) => format!("<unreadable entry: {error}>"),
                })
                .collect();
            names.sort();
            names
        }
        Err(error) => vec![format!("<unreadable: {error}>")],
    }
}

fn report_boot() -> BootResult {
    let cmdline = read_trimmed(Path::new("/proc/cmdline"));
    let osrelease = read_trimmed(Path::new("/proc/sys/kernel/osrelease"));
    let args: Vec<String> = std::env::args_os().map(|arg| lossy(&arg)).collect();
    let mut env: Vec<(String, String)> = std::env::vars_os()
        .map(|(key, value)| (lossy(&key), lossy(&value)))
        .collect();
    env.sort();
    let net_ifaces = dir_names("/sys/class/net");
    let virtio_devices: Vec<String> = dir_names("/sys/bus/virtio/devices")
        .into_iter()
        .map(|entry| {
            let device = read_trimmed(
                &Path::new("/sys/bus/virtio/devices")
                    .join(&entry)
                    .join("device"),
            );
            format!("{entry}:{device}")
        })
        .collect();

    console_line(&format!("minos-guest: cmdline {cmdline}"));
    console_line(&format!("minos-guest: osrelease {osrelease}"));
    console_line(&format!("minos-guest: args {}", args.join(" ")));
    let env_line: Vec<String> = env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    console_line(&format!("minos-guest: env {}", env_line.join(" ")));
    console_line(&format!("minos-guest: net_ifaces {}", net_ifaces.join(" ")));
    console_line(&format!(
        "minos-guest: virtio_devices {}",
        virtio_devices.join(" ")
    ));

    BootResult::new(cmdline, osrelease, args, env, net_ifaces, virtio_devices)
}

fn connect(step: &Connect) -> ConnectResult {
    let results = step
        .targets()
        .iter()
        .map(
            |target| match TcpStream::connect_timeout(target, step.timeout()) {
                Ok(stream) => {
                    drop(stream);
                    ConnectAttempt {
                        target: target.to_string(),
                        ok: true,
                        error: None,
                        raw_os_error: None,
                    }
                }
                Err(error) => ConnectAttempt {
                    target: target.to_string(),
                    ok: false,
                    error: Some(error.to_string()),
                    raw_os_error: error.raw_os_error(),
                },
            },
        )
        .collect();
    ConnectResult { results }
}

fn vsock_connect(step: VsockConnect) -> VsockConnectResult {
    match Vsock::connect_with_cid_port(VMADDR_CID_HOST, step.port()) {
        Ok(stream) => {
            drop(stream);
            VsockConnectResult {
                port: step.port(),
                ok: true,
                error: None,
                raw_os_error: None,
            }
        }
        Err(error) => VsockConnectResult {
            port: step.port(),
            ok: false,
            error: Some(error.to_string()),
            raw_os_error: error.raw_os_error(),
        },
    }
}

/// Bytes of the next chunk: `CHUNK`, or fewer at the end.
fn chunk_len(remaining: u64) -> usize {
    usize::try_from(remaining).map_or(CHUNK, |remaining| remaining.min(CHUNK))
}

fn to_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Writes `data`, counting every byte accepted, and stops at the first error. Unlike `write_all`, the count
/// is exact even when the error arrives partway through.
fn write_counted(out: &mut impl Write, data: &[u8]) -> (u64, Option<io::Error>) {
    let mut rest = data;
    let mut written = 0_u64;
    while !rest.is_empty() {
        match out.write(rest) {
            Ok(0) => return (written, Some(io::Error::from(io::ErrorKind::WriteZero))),
            Ok(n) => {
                written = written.saturating_add(to_u64(n));
                rest = rest.get(n..).unwrap_or_default();
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return (written, Some(error)),
        }
    }
    (written, None)
}

fn vsock_stream(port: u32, step: VsockStream) -> VsockStreamResult {
    let requested = step.bytes();
    let mut stream = match Vsock::connect_with_cid_port(VMADDR_CID_HOST, port) {
        Ok(stream) => stream,
        Err(error) => {
            return VsockStreamResult {
                requested,
                written: 0,
                error: Some(error.to_string()),
                raw_os_error: error.raw_os_error(),
            };
        }
    };
    let mut buf = vec![0_u8; CHUNK];
    let mut written = 0_u64;
    let mut failure = None;
    while written < requested {
        let Some(chunk) = buf.get_mut(..chunk_len(requested.saturating_sub(written))) else {
            break;
        };
        fill_pattern(written, chunk);
        let (count, error) = write_counted(&mut stream, chunk);
        written = written.saturating_add(count);
        if error.is_some() {
            failure = error;
            break;
        }
    }
    // The host may already have closed its end after its cap; a shutdown error adds nothing to the report.
    drop(stream.shutdown(Shutdown::Write));
    drop(stream);
    VsockStreamResult {
        requested,
        written,
        raw_os_error: failure.as_ref().and_then(io::Error::raw_os_error),
        error: failure.map(|error| error.to_string()),
    }
}

/// Writes the pattern to the scratch disk, syncs, reads it back, and compares. `Ok(Some(..))` describes a
/// mismatch.
fn scratch_check(bytes: u64) -> io::Result<Option<String>> {
    let mut disk = OpenOptions::new()
        .read(true)
        .write(true)
        .open(SCRATCH_DISK)?;
    let mut buf = vec![0_u8; CHUNK];
    let mut done = 0_u64;
    while done < bytes {
        let Some(chunk) = buf.get_mut(..chunk_len(bytes.saturating_sub(done))) else {
            break;
        };
        fill_pattern(done, chunk);
        disk.write_all(chunk)?;
        done = done.saturating_add(to_u64(chunk.len()));
    }
    disk.sync_all()?;
    disk.seek(SeekFrom::Start(0))?;
    let mut expected = vec![0_u8; CHUNK];
    done = 0;
    while done < bytes {
        let len = chunk_len(bytes.saturating_sub(done));
        let (Some(got), Some(want)) = (buf.get_mut(..len), expected.get_mut(..len)) else {
            break;
        };
        disk.read_exact(got)?;
        fill_pattern(done, want);
        if let Some(at) = got.iter().zip(want.iter()).position(|(a, b)| a != b) {
            return Ok(Some(format!(
                "read-back mismatch at byte {}",
                done.saturating_add(to_u64(at))
            )));
        }
        done = done.saturating_add(to_u64(len));
    }
    Ok(None)
}

fn scratch_roundtrip(step: ScratchRoundtrip) -> ScratchResult {
    let bytes = step.bytes();
    match scratch_check(bytes) {
        Ok(None) => ScratchResult {
            bytes,
            ok: true,
            error: None,
        },
        Ok(Some(mismatch)) => ScratchResult {
            bytes,
            ok: false,
            error: Some(mismatch),
        },
        Err(error) => ScratchResult {
            bytes,
            ok: false,
            error: Some(error.to_string()),
        },
    }
}

/// Output captured from one child pipe.
struct Captured {
    text: String,
    truncated: bool,
}

/// One child pipe drained without blocking: up to `cap` bytes are kept, the rest is read and discarded so the
/// child never stalls on a full pipe, and a pipe closed by its last writer or a read error ends the capture.
struct Capture<R> {
    pipe: Option<R>,
    bytes: Vec<u8>,
    cap: usize,
    truncated: bool,
    error: Option<io::Error>,
}

impl<R: Read + AsFd> Capture<R> {
    /// Takes the pipe and makes it non-blocking. If that fails, the pipe is dropped and the failure recorded,
    /// so the child meets a closed stream rather than one nobody drains.
    fn new(pipe: Option<R>, cap: u64) -> Self {
        let mut capture = Self {
            pipe: None,
            bytes: Vec::new(),
            cap: usize::try_from(cap).unwrap_or(usize::MAX),
            truncated: false,
            error: None,
        };
        if let Some(pipe) = pipe {
            match fcntl_setfl(&pipe, OFlags::NONBLOCK) {
                Ok(()) => capture.pipe = Some(pipe),
                Err(errno) => capture.error = Some(errno.into()),
            }
        }
        capture
    }

    /// Reads everything ready now, returning once the pipe would block, is closed, or fails.
    fn pump(&mut self, buf: &mut [u8]) {
        while let Some(pipe) = self.pipe.as_mut() {
            match pipe.read(buf) {
                Ok(0) => self.pipe = None,
                Ok(read) => {
                    let room = self.cap.saturating_sub(self.bytes.len());
                    if read > room {
                        self.truncated = true;
                    }
                    if let Some(kept) = buf.get(..read.min(room)) {
                        self.bytes.extend_from_slice(kept);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    self.error = Some(error);
                    self.pipe = None;
                }
            }
        }
    }

    fn finish(self) -> Captured {
        let mut text = String::from_utf8_lossy(&self.bytes).into_owned();
        if let Some(error) = self.error {
            text.push_str("\n<read error: ");
            text.push_str(&error.to_string());
            text.push('>');
        }
        Captured {
            text,
            truncated: self.truncated,
        }
    }
}

/// Kills every process in `group`. An empty group (`ESRCH`) is the normal case after a clean exit.
fn kill_group(group: Pid) {
    match kill_process_group(group, Signal::KILL) {
        Ok(()) | Err(Errno::SRCH) => {}
        Err(errno) => console_line(&format!("minos-guest: kill process group: {errno}")),
    }
}

fn run_command(step: &Run) -> RunResult {
    let argv = step.argv();
    let spawned = Command::new(step.program())
        .args(step.args())
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Its own group, so the deadline can kill everything the command started, not only the leader.
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return RunResult {
                argv,
                status: None,
                signal: None,
                timed_out: false,
                stdout: String::new(),
                stdout_truncated: false,
                stderr: String::new(),
                stderr_truncated: false,
                error: Some(error.to_string()),
            };
        }
    };
    let group = Pid::from_child(&child);
    let cap = step.stdout_cap();
    let mut stdout = Capture::new(child.stdout.take(), cap);
    let mut stderr = Capture::new(child.stderr.take(), cap);
    let deadline = Instant::now().checked_add(step.timeout());
    let mut buf = vec![0_u8; CHUNK];
    let mut timed_out = false;
    let waited = loop {
        stdout.pump(&mut buf);
        stderr.pump(&mut buf);
        match child.try_wait() {
            // The leader is gone: take what it left queued and stop. A daemon that kept a pipe open does not
            // keep the step open.
            Ok(Some(status)) => {
                stdout.pump(&mut buf);
                stderr.pump(&mut buf);
                break Ok(status);
            }
            Ok(None) => {}
            Err(error) => break Err(error),
        }
        if deadline.is_none_or(|deadline| Instant::now() >= deadline) {
            timed_out = true;
            // The leader is unreaped, so the group id is still its own.
            kill_group(group);
            let waited = child.wait();
            stdout.pump(&mut buf);
            stderr.pump(&mut buf);
            break waited;
        }
        std::thread::sleep(RUN_POLL);
    };
    // Whatever the command left behind in its group dies with the step. The guest forks nothing between the
    // reap and this kill, so the group id has not been reused.
    kill_group(group);
    reap_orphans();
    let stdout = stdout.finish();
    let stderr = stderr.finish();
    match waited {
        Ok(status) => RunResult {
            argv,
            status: status.code(),
            signal: status.signal(),
            timed_out,
            stdout: stdout.text,
            stdout_truncated: stdout.truncated,
            stderr: stderr.text,
            stderr_truncated: stderr.truncated,
            error: None,
        },
        Err(error) => RunResult {
            argv,
            status: None,
            signal: None,
            timed_out,
            stdout: stdout.text,
            stdout_truncated: stdout.truncated,
            stderr: stderr.text,
            stderr_truncated: stderr.truncated,
            error: Some(error.to_string()),
        },
    }
}

/// PID 1 inherits every orphan. Reap whatever has exited, without blocking. `wait` (any child) rather than
/// `waitpid(None, ..)`, which in rustix 1.x waits only for children in the caller's process group and would
/// miss an orphan that changed its group. Called before every step and after a `run`, so a daemon one step
/// left behind is reaped before the next step starts.
fn reap_orphans() {
    while let Ok(Some(_)) = wait(WaitOptions::NOHANG) {}
}

fn console_flood(step: ConsoleFlood) -> ConsoleFloodResult {
    let requested = step.bytes();
    let mut out = io::stdout().lock();
    let mut buf = vec![0_u8; CHUNK];
    let mut written = 0_u64;
    let mut failure = None;
    while written < requested {
        let Some(chunk) = buf.get_mut(..chunk_len(requested.saturating_sub(written))) else {
            break;
        };
        for (byte, index) in chunk.iter_mut().zip(written..=u64::MAX) {
            *byte = if index % FLOOD_LINE == FLOOD_LINE - 1 {
                b'\n'
            } else {
                b'a' + pattern_byte(index) % 26
            };
        }
        let (count, error) = write_counted(&mut out, chunk);
        written = written.saturating_add(count);
        if error.is_some() {
            failure = error;
            break;
        }
    }
    if failure.is_none() {
        failure = out.flush().err();
    }
    ConsoleFloodResult {
        requested,
        written,
        error: failure.map(|error| error.to_string()),
    }
}

fn send_frame(port: u32, frame: &[u8]) -> io::Result<()> {
    let mut stream = Vsock::connect_with_cid_port(VMADDR_CID_HOST, port)?;
    stream.write_all(frame)?;
    stream.shutdown(Shutdown::Write)?;
    drop(stream);
    Ok(())
}
