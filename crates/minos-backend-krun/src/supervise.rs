//! Runs one `minos-vmm` helper and classifies how it ended (KRUN-01, KRUN-05; refined K7 rule).
//!
//! Every byte that comes back from the helper is hostile. The console is the helper's stdout pipe (05-EVIDENCE.md
//! Mechanism selections, `console: pipe`): the guest kernel writes it through `/dev/fd/1`, and the supervisor
//! reads it on its own thread through `cap + 1` bytes, so a flood is detected rather than silently truncated, and
//! the whole process group is killed the moment it passes the cap. The helper's stderr is a second pipe, read
//! through [`VMM_STDIO_CAP`] + 1 bytes and then drained, so the helper never blocks on it. Both must be pipes:
//! the helper sets `RLIMIT_FSIZE` to 0, so a file-backed stderr would kill it with `SIGXFSZ` (05-07). Nothing is
//! re-opened by a path the helper could replace, because there is no path: the supervisor holds both read ends.
//!
//! [`Helper::spawn`] runs an absolute-path argv with an empty environment, stdin from `/dev/null`, and its own
//! process group. The backend always passes `seatbelt::argv`, so the program is `/usr/bin/sandbox-exec`, which
//! applies the profile and execs `minos-vmm` in the same process. [`watch`] polls the helper until it exits and
//! kills the group at the host deadline, at the console cap, or on a chunk fault. Dropping a [`Helper`] kills
//! the group too.
//!
//! [`classify`] is a pure function of what the host observed ([`Observed`]): the helper's exit, its stderr as
//! whole lines, why the host killed it (if it did), and how the chunk receiver ended. The helper's exit code is
//! never the guest's result; the guest's output stream is (02-SPIKE-EVIDENCE.md K7 table).

use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{self, ErrorKind, PipeReader, Read};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use minos_sandbox::{CrashCause, Ended};
use rustix::io::Errno;
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};
use sha2::{Digest, Sha256};

use crate::chunks::ReceiveOutcome;

/// Longest socket path the host binds: macOS `sun_path` is 104 bytes including the terminating NUL.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// What the host deadline adds to a plan's hard maximum duration: time for the helper to start, for its own
/// watchdog to trip, and for it to print its `minos-vmm: watchdog deadline` marker before the host gives up. The
/// helper's deadline must fire first, or that evidence of a timeout is lost.
/// `minos_sandbox::spec::BOOT_ALLOWANCE_MS` documents the same 30 s figure, and both sides pin it in tests
/// (`host_deadline_grace_is_pinned_for_minos_sandbox` here, `launcher_bounds_match_minos_vmm` there).
pub const HOST_DEADLINE_GRACE: Duration = Duration::from_secs(30);

/// Most bytes kept from the helper's stderr: twice the helper's own 64 KiB log budget (`minos-vmm`'s
/// `LOG_CAP_BYTES`), so the watchdog and exit lines it prints after a full log still fit. Anything past the cap
/// is drained and dropped.
pub const VMM_STDIO_CAP: u64 = 131_072;

/// The line `minos-vmm` prints when its watchdog stops the VM at `max_duration_ms`, before exiting 124.
pub const DEADLINE_MARKER: &str = "minos-vmm: watchdog deadline";

/// The line `minos-vmm` prints when its watchdog sees its parent gone, before exiting 126.
pub const PARENT_GONE_MARKER: &str = "minos-vmm: watchdog parent-gone";

/// How often [`watch`] and [`Helper::finish`] poll the helper.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long the helper may take to die after a SIGKILL before [`watch`] reports it as a host failure.
const KILL_WAIT: Duration = Duration::from_secs(5);

/// How long the pipe readers may take to see end-of-file once the helper and its group are gone.
const PIPE_DRAIN_WINDOW: Duration = Duration::from_secs(2);

/// Read size for [`sha256_file`].
const READ_CHUNK_BYTES: usize = 65_536;

/// Why the supervisor could not run or observe a helper. A guest that misbehaves is not an error: it is an
/// [`Observed`] value that [`classify`] turns into an [`Ended`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SuperviseError {
    /// A host filesystem operation on a named path failed.
    #[error("could not {action} {}", .path.display())]
    Io {
        /// What the supervisor was doing.
        action: &'static str,
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A host process, pipe, or thread operation failed.
    #[error("could not {action}")]
    Host {
        /// What the supervisor was doing.
        action: &'static str,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The launch argv has no program.
    #[error("the launch argv is empty")]
    EmptyArgv,
    /// The launch argv names its program by a relative path. `Command` resolves one through the supervisor's own
    /// `PATH`, even with the child's environment cleared, so it would run whatever a writable directory earlier
    /// in the user's `PATH` provides.
    #[error("the launch program {} is not an absolute path", .program.display())]
    RelativeProgram {
        /// The refused program.
        program: PathBuf,
    },
    /// The helper could not be started.
    #[error("could not spawn {}", .program.display())]
    Spawn {
        /// The program the argv named.
        program: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The helper was still running [`KILL_WAIT`] after the host killed it.
    #[error("the helper survived SIGKILL")]
    Survived,
    /// A pipe reader did not see end-of-file within [`PIPE_DRAIN_WINDOW`] of the helper's exit: something outside
    /// the helper's process group holds the pipe.
    #[error("a helper pipe stayed open after the helper exited")]
    PipeOpen,
}

/// The lowercase hex SHA-256 of the file at `path`, read in chunks.
///
/// # Errors
///
/// [`SuperviseError::Io`] if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, SuperviseError> {
    let mut file = File::open(path).map_err(io_at("open", path))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
    loop {
        let read = match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(source) => return Err(io_at("read", path)(source)),
        };
        if let Some(part) = chunk.get(..read) {
            hasher.update(part);
        }
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(base16ct::lower::encode_string(&digest))
}

/// How the helper process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperExit {
    /// It exited with this code.
    Code(i32),
    /// It was killed by `signal`. `sent_by_host` is true only when the signal is SIGKILL and the supervisor had
    /// sent one to the helper's group.
    Signal {
        /// The signal number.
        signal: i32,
        /// Whether the supervisor sent it.
        sent_by_host: bool,
    },
}

/// Why the supervisor killed the helper's process group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKill {
    /// The host deadline passed: the plan's hard maximum plus [`HOST_DEADLINE_GRACE`].
    Deadline,
    /// The console passed its cap, as the supervisor read it from the pipe.
    ConsoleCap,
    /// The chunk receiver saw the output stream break the protocol.
    ChunkFault,
}

/// The helper's process, shared by the supervisor and the console reader so either can kill it.
///
/// Kills the group and reaps the helper on drop, so no early return leaves a helper or its VM running. The group
/// is signalled only while the helper is unreaped (its pid then still names the helper, and the group cannot be
/// reused); every reap goes through [`Reaper::poll_exit`] under the same lock, which records the status.
struct Reaper {
    child: Child,
    exit: Option<ExitStatus>,
    host_killed: bool,
}

impl Reaper {
    /// Sends SIGKILL to the helper's group and to the helper, unless it has been reaped.
    ///
    /// The group id equals the helper's pid because it was spawned with `process_group(0)`. Signalling the group
    /// rather than the helper is what stops a VM when the helper is wrapped in something that forks: killing
    /// `sandbox-exec` alone would leave the VM running with the job disks open. An empty group (`ESRCH`) is not
    /// an error.
    fn kill(&mut self) -> io::Result<()> {
        if self.exit.is_some() {
            return Ok(());
        }
        self.host_killed = true;
        let pid = Pid::from_child(&self.child);
        let group = kill_process_group(pid, Signal::KILL);
        let leader = self.child.kill();
        match group {
            Ok(()) | Err(Errno::SRCH) => leader,
            // macOS answers EPERM for a group whose remaining members are all zombies (proven in 05-15). The
            // helper is unreaped, so if it has exited it is one of them: nothing is left to kill, and
            // `poll_exit` sweeps the group again before the reap.
            Err(Errno::PERM) if exited_unreaped(pid)? => leader,
            Err(errno) => leader.and(Err(errno.into())),
        }
    }

    /// The helper's exit status once it has exited, reaping it.
    ///
    /// The exit is first observed with `waitid(WNOWAIT)`, which leaves the helper a zombie: its pid, and so its
    /// process group id, cannot be reused yet. Whatever is left in the group is killed then, and only then is the
    /// helper reaped, so no group member outlives the helper and no signal can reach a reused pid.
    fn poll_exit(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.exit {
            return Ok(Some(status));
        }
        let pid = Pid::from_child(&self.child);
        if !exited_unreaped(pid)? {
            return Ok(None);
        }
        // The group outlives a leader only if a member left behind is still running; the leader's own exit is
        // already decided, so this kill is not the host's verdict and does not set `host_killed`. With no live
        // member left it fails with EPERM (only the zombie leader) or ESRCH, and either means nothing to kill.
        match kill_process_group(pid, Signal::KILL) {
            Ok(()) | Err(_) => {}
        }
        self.exit = self.child.try_wait()?;
        Ok(self.exit)
    }

    /// The recorded exit as a [`HelperExit`].
    fn helper_exit(&self, status: ExitStatus) -> HelperExit {
        match (status.code(), status.signal()) {
            (Some(code), _) => HelperExit::Code(code),
            (None, Some(signal)) => HelperExit::Signal {
                signal,
                sent_by_host: self.host_killed && signal == Signal::KILL.as_raw(),
            },
            (None, None) => HelperExit::Signal {
                signal: 0,
                sent_by_host: false,
            },
        }
    }
}

impl Drop for Reaper {
    fn drop(&mut self) {
        if self.exit.is_none() {
            self.kill().ok();
            self.child.wait().ok();
        }
    }
}

/// Whether the unreaped child `pid` has exited, observed with `waitid(WNOWAIT)` so it stays a zombie and keeps
/// its pid, and so its process group id, from being reused.
fn exited_unreaped(pid: Pid) -> io::Result<bool> {
    match waitid(
        WaitId::Pid(pid),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    ) {
        Ok(status) => Ok(status.is_some()),
        Err(Errno::INTR) => Ok(false),
        Err(errno) => Err(errno.into()),
    }
}

/// Locks the shared helper, recovering from a poisoned lock: the state is a child handle and two plain fields,
/// valid after any panic.
fn lock(reaper: &Mutex<Reaper>) -> MutexGuard<'_, Reaper> {
    reaper.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One running helper: its process, and the threads reading its console and stderr pipes.
///
/// Dropping it kills the helper's process group.
pub struct Helper {
    reaper: Arc<Mutex<Reaper>>,
    pid: u32,
    console_over: Arc<AtomicBool>,
    console: Option<JoinHandle<io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

impl Helper {
    /// Spawns `argv` with an empty environment, stdin from `/dev/null`, stdout (the console) and stderr on
    /// pipes, and its own process group, and starts the two pipe readers.
    ///
    /// The console reader reads through `console_cap + 1` bytes; on reaching them it raises
    /// [`Helper::console_over`] and kills the group. The stderr reader keeps [`VMM_STDIO_CAP`] + 1 bytes and
    /// drains the rest.
    ///
    /// # Errors
    ///
    /// [`SuperviseError::EmptyArgv`] or [`SuperviseError::RelativeProgram`] for a refused argv,
    /// [`SuperviseError::Spawn`] when the program cannot start, and [`SuperviseError::Host`] when a pipe or a
    /// reader thread cannot be made (the helper is then killed).
    pub fn spawn(argv: &[OsString], console_cap: u64) -> Result<Self, SuperviseError> {
        let (program, args) = argv.split_first().ok_or(SuperviseError::EmptyArgv)?;
        let program_path = Path::new(program);
        if !program_path.is_absolute() {
            return Err(SuperviseError::RelativeProgram {
                program: program_path.to_path_buf(),
            });
        }
        let (console_read, console_write) = io::pipe().map_err(host("make the console pipe"))?;
        let (stderr_read, stderr_write) = io::pipe().map_err(host("make the stderr pipe"))?;
        // The command holds the pipes' write ends; it is dropped at the end of this block, so the readers see
        // end-of-file once the helper's group is gone.
        let child = {
            let mut command = Command::new(program);
            command
                .args(args)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(console_write)
                .stderr(stderr_write)
                .process_group(0);
            command.spawn().map_err(|source| SuperviseError::Spawn {
                program: program_path.to_path_buf(),
                source,
            })?
        };
        let pid = child.id();
        let reaper = Arc::new(Mutex::new(Reaper {
            child,
            exit: None,
            host_killed: false,
        }));
        let console_over = Arc::new(AtomicBool::new(false));
        let mut helper = Self {
            reaper: Arc::clone(&reaper),
            pid,
            console_over: Arc::clone(&console_over),
            console: None,
            stderr: None,
        };
        helper.console = Some(
            thread::Builder::new()
                .name("minos-console".to_owned())
                .spawn(move || read_console(console_read, console_cap, &reaper, &console_over))
                .map_err(host("start the console reader"))?,
        );
        helper.stderr = Some(
            thread::Builder::new()
                .name("minos-vmm-stderr".to_owned())
                .spawn(move || read_stderr(stderr_read))
                .map_err(host("start the stderr reader"))?,
        );
        Ok(helper)
    }

    /// The helper's pid, which is also its process group id.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Whether the console passed its cap. Once true, the console reader has killed the group.
    #[must_use]
    pub fn console_over(&self) -> bool {
        self.console_over.load(Ordering::Acquire)
    }

    /// Sends SIGKILL to the helper's process group and to the helper, unless it has been reaped.
    ///
    /// # Errors
    ///
    /// [`SuperviseError::Host`] when a signal could not be sent.
    pub fn kill_group(&self) -> Result<(), SuperviseError> {
        lock(&self.reaper)
            .kill()
            .map_err(host("kill the helper's process group"))
    }

    /// The helper's exit, once it has exited; `None` while it runs. Observing the exit kills anything left in
    /// the helper's process group before the helper is reaped.
    ///
    /// # Errors
    ///
    /// [`SuperviseError::Host`] when the helper cannot be waited on.
    pub fn try_wait(&self) -> Result<Option<HelperExit>, SuperviseError> {
        let mut reaper = lock(&self.reaper);
        let status = reaper.poll_exit().map_err(host("wait for the helper"))?;
        Ok(status.map(|status| reaper.helper_exit(status)))
    }

    /// Waits for the helper to exit and its pipes to close, and returns what it left. Call it after [`watch`]; a
    /// helper still running is killed first.
    ///
    /// # Errors
    ///
    /// [`SuperviseError::Host`] when the helper cannot be waited on or a pipe could not be read,
    /// [`SuperviseError::Survived`] when it outlives a kill, and [`SuperviseError::PipeOpen`] when a pipe stays
    /// open after it exited.
    pub fn finish(mut self) -> Result<HelperEnd, SuperviseError> {
        let killed_at = Instant::now();
        let exit = loop {
            if let Some(exit) = self.try_wait()? {
                break exit;
            }
            if killed_at.elapsed() >= KILL_WAIT {
                return Err(SuperviseError::Survived);
            }
            self.kill_group().ok();
            thread::sleep(POLL_INTERVAL);
        };
        let console = join_reader(self.console.take())?;
        let stderr = join_reader(self.stderr.take())?;
        Ok(HelperEnd {
            exit,
            console,
            stderr_lines: whole_lines(&stderr),
        })
    }
}

impl Drop for Helper {
    /// Kills the helper's group and reaps the helper at once, so it does not linger as a zombie until the
    /// console reader lets go of the shared process.
    fn drop(&mut self) {
        let mut reaper = lock(&self.reaper);
        if reaper.exit.is_none() {
            reaper.kill().ok();
            if let Ok(status) = reaper.child.wait() {
                reaper.exit = Some(status);
            }
        }
    }
}

/// What a helper left when it ended: its exit, its console bytes (hostile, at most the cap plus one), and its
/// stderr as whole lines.
pub struct HelperEnd {
    exit: HelperExit,
    console: Vec<u8>,
    stderr_lines: Vec<String>,
}

impl HelperEnd {
    /// How the helper ended.
    #[must_use]
    pub const fn exit(&self) -> HelperExit {
        self.exit
    }

    /// The console bytes the supervisor read: hostile, at most the cap plus one byte.
    #[must_use]
    pub fn console(&self) -> &[u8] {
        &self.console
    }

    /// The helper's stderr, split into newline-terminated lines with every control character replaced by
    /// U+FFFD. An unterminated last line is dropped: `minos-vmm` ends every line it writes.
    #[must_use]
    pub fn stderr_lines(&self) -> &[String] {
        &self.stderr_lines
    }

    /// The exit, the console bytes, and the stderr lines, by value.
    #[must_use]
    pub fn into_parts(self) -> (HelperExit, Vec<u8>, Vec<String>) {
        (self.exit, self.console, self.stderr_lines)
    }
}

impl fmt::Debug for HelperEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperEnd")
            .field("exit", &self.exit)
            .field("console_len", &self.console.len())
            .field("stderr_lines", &self.stderr_lines.len())
            .finish()
    }
}

/// Polls `helper` every 10 ms until it exits, and returns why the host killed it, if it did.
///
/// The helper's group is killed, once, for the first of: the console passing its cap
/// ([`HostKill::ConsoleCap`]; the console reader has already killed it, so this is reported even when the exit is
/// seen first), `faulted()` returning true
/// ([`HostKill::ChunkFault`]), or `deadline` passing ([`HostKill::Deadline`]). An exit seen first wins: the host
/// kills only a helper still running.
///
/// # Errors
///
/// [`SuperviseError::Host`] when the helper cannot be waited on, and [`SuperviseError::Survived`] when it is still
/// running [`KILL_WAIT`] after the kill.
pub fn watch(
    helper: &Helper,
    deadline: Instant,
    faulted: impl Fn() -> bool,
) -> Result<Option<HostKill>, SuperviseError> {
    let mut killed: Option<(HostKill, Instant)> = None;
    loop {
        if helper.try_wait()?.is_some() {
            // The console reader kills the group itself, so its kill can end the helper before this loop sees
            // the flag; the flag is raised before that kill is sent.
            let why = killed
                .map(|(why, _)| why)
                .or_else(|| helper.console_over().then_some(HostKill::ConsoleCap));
            return Ok(why);
        }
        match killed {
            None => {
                let why = if helper.console_over() {
                    Some(HostKill::ConsoleCap)
                } else if faulted() {
                    Some(HostKill::ChunkFault)
                } else if Instant::now() >= deadline {
                    Some(HostKill::Deadline)
                } else {
                    None
                };
                if let Some(why) = why {
                    // A failed signal is re-sent on the next poll; the exit, not the signal, ends the loop.
                    helper.kill_group().ok();
                    killed = Some((why, Instant::now()));
                }
            }
            Some((_, at)) if at.elapsed() >= KILL_WAIT => return Err(SuperviseError::Survived),
            Some(_) => {
                helper.kill_group().ok();
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Reads the console pipe through `cap + 1` bytes. On reaching them it raises `over` and kills the group before
/// the pipe is dropped, so the flood stops at once.
fn read_console(
    pipe: PipeReader,
    cap: u64,
    reaper: &Mutex<Reaper>,
    over: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let limit = cap.saturating_add(1);
    let mut bytes = Vec::new();
    let mut reader = pipe.take(limit);
    reader.read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len >= limit) {
        over.store(true, Ordering::Release);
        // A failed kill is retried by `watch`, which sees `over`.
        lock(reaper).kill().ok();
    }
    Ok(bytes)
}

/// Reads the stderr pipe through [`VMM_STDIO_CAP`] + 1 bytes, then drains the rest so the helper never blocks.
fn read_stderr(mut pipe: PipeReader) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.by_ref()
        .take(VMM_STDIO_CAP.saturating_add(1))
        .read_to_end(&mut bytes)?;
    io::copy(&mut pipe, &mut io::sink())?;
    Ok(bytes)
}

/// Joins a pipe reader once its pipe has closed, waiting at most [`PIPE_DRAIN_WINDOW`].
fn join_reader(handle: Option<JoinHandle<io::Result<Vec<u8>>>>) -> Result<Vec<u8>, SuperviseError> {
    let Some(handle) = handle else {
        return Ok(Vec::new());
    };
    let start = Instant::now();
    while !handle.is_finished() {
        if start.elapsed() >= PIPE_DRAIN_WINDOW {
            return Err(SuperviseError::PipeOpen);
        }
        thread::sleep(POLL_INTERVAL);
    }
    handle
        .join()
        .map_err(|_| SuperviseError::Host {
            action: "join a pipe reader",
            source: io::Error::other("the pipe reader panicked"),
        })?
        .map_err(host("read a helper pipe"))
}

/// Splits `bytes` into newline-terminated lines, replacing every control character with U+FFFD, so a line that
/// held one can never equal a marker. An unterminated tail is not a line.
fn whole_lines(bytes: &[u8]) -> Vec<String> {
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .filter_map(|segment| segment.strip_suffix(b"\n"))
        .map(|line| {
            String::from_utf8_lossy(line)
                .chars()
                .map(|c| if c.is_control() { '\u{fffd}' } else { c })
                .collect()
        })
        .collect()
}

/// Everything the host observed of one launch, the input to [`classify`].
pub struct Observed {
    exit: HelperExit,
    stderr_lines: Vec<String>,
    killed_for: Option<HostKill>,
    chunks: ReceiveOutcome,
}

impl Observed {
    /// The helper's exit, its stderr lines (from [`HelperEnd::stderr_lines`]), why the host killed it, and how
    /// the chunk receiver ended.
    #[must_use]
    pub const fn new(
        exit: HelperExit,
        stderr_lines: Vec<String>,
        killed_for: Option<HostKill>,
        chunks: ReceiveOutcome,
    ) -> Self {
        Self {
            exit,
            stderr_lines,
            killed_for,
            chunks,
        }
    }
}

/// How [`classify`] judged a launch.
pub enum Classification {
    /// A candidate-side ending, with the output stream (empty unless [`Ended::Exited`]).
    Ended(Ended, Vec<u8>),
    /// An infrastructure failure the runner retries, named by a fixed description.
    Infra(&'static str),
}

impl fmt::Debug for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ended(ended, stream) => f
                .debug_tuple("Ended")
                .field(ended)
                .field(&stream.len())
                .finish(),
            Self::Infra(what) => f.debug_tuple("Infra").field(what).finish(),
        }
    }
}

/// Classifies one launch from what the host observed: the refined K7 rule (02-SPIKE-EVIDENCE.md "Classification
/// rule for Phase 5", refined by 05-RESEARCH Pattern 7 and Pitfall 5).
///
/// A host kill decides first, because the host acted on its own observation:
///
/// | Observation | Result |
/// |---|---|
/// | The host killed the group at its deadline | [`Ended::TimedOut`] |
/// | The host killed the group when the console passed its cap (read from the pipe) | [`Ended::Crashed`] ([`CrashCause::ConsoleCap`]) |
/// | The host killed the group on a chunk fault | [`Ended::ChunkProtocol`] |
/// | Exit 0 or exit 124 with the deadline marker, and the chunk receiver faulted | [`Ended::ChunkProtocol`] |
/// | Exit 0 with a complete stream | [`Ended::Exited`], with the stream |
/// | Exit 0 with no END chunk | [`Ended::Crashed`] ([`CrashCause::NoStatusFrame`]) |
/// | Exit 124 with the line [`DEADLINE_MARKER`] | [`Ended::TimedOut`] |
/// | Exit 124 without it, exit 126 (with or without [`PARENT_GONE_MARKER`]), exits 64 to 71, a signal the host did not send, a host kill with no recorded reason, any other code | Infrastructure |
///
/// A marker counts only as an exact whole line together with its exit code. `minos-vmm` writes the markers; the
/// guest reaches only the console, and every line the helper's logger writes starts with `minos-vmm: log `, so no
/// logged text can be a marker line. A spawn failure never reaches here: it is a `LaunchError`.
///
/// A timeout's stage (Pitfall 12): PID 1 enforces its own per-stage deadlines and reports a stage timeout in its
/// status file before the host deadline can fire, so a host [`Ended::TimedOut`] is the backstop only. The runner
/// attributes it to `Run` for a Candidate and `Judge` for a Judge.
#[must_use]
pub fn classify(observed: Observed) -> Classification {
    let Observed {
        exit,
        stderr_lines,
        killed_for,
        chunks,
    } = observed;
    match killed_for {
        Some(HostKill::Deadline) => return ended(Ended::TimedOut),
        Some(HostKill::ConsoleCap) => return ended(Ended::Crashed(CrashCause::ConsoleCap)),
        Some(HostKill::ChunkFault) => return ended(Ended::ChunkProtocol),
        None => {}
    }
    let marked = |marker: &str| stderr_lines.iter().any(|line| line == marker);
    let guest_ran = match exit {
        HelperExit::Code(0) => true,
        HelperExit::Code(124) => marked(DEADLINE_MARKER),
        _ => false,
    };
    if guest_ran && matches!(chunks, ReceiveOutcome::Fault(_)) {
        return ended(Ended::ChunkProtocol);
    }
    match exit {
        HelperExit::Code(0) => match chunks {
            ReceiveOutcome::Complete(stream) => Classification::Ended(Ended::Exited, stream),
            ReceiveOutcome::Incomplete | ReceiveOutcome::Fault(_) => {
                ended(Ended::Crashed(CrashCause::NoStatusFrame))
            }
        },
        HelperExit::Code(124) if guest_ran => ended(Ended::TimedOut),
        HelperExit::Code(124) => {
            Classification::Infra("the helper exited 124 without its deadline marker")
        }
        HelperExit::Code(126) if marked(PARENT_GONE_MARKER) => {
            Classification::Infra("the helper's parent went away")
        }
        HelperExit::Code(126) => {
            Classification::Infra("the helper exited 126 without its parent-gone marker")
        }
        HelperExit::Code(64..=71) => {
            Classification::Infra("the helper failed before the VM started")
        }
        HelperExit::Code(_) => Classification::Infra("the helper exited with an unexpected code"),
        HelperExit::Signal {
            sent_by_host: false,
            ..
        } => Classification::Infra("the helper died of a signal the host did not send"),
        HelperExit::Signal {
            sent_by_host: true, ..
        } => Classification::Infra("the host killed the helper without recording why"),
    }
}

/// A candidate-side ending with no stream.
const fn ended(ended: Ended) -> Classification {
    Classification::Ended(ended, Vec::new())
}

/// Builds the error-mapping closure for a host operation with no path.
fn host(action: &'static str) -> impl FnOnce(io::Error) -> SuperviseError {
    move |source| SuperviseError::Host { action, source }
}

/// Builds the error-mapping closure for an I/O failure on `path`.
fn io_at(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> SuperviseError {
    let path = path.to_path_buf();
    move |source| SuperviseError::Io {
        action,
        path,
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs;
    use std::thread;
    use std::time::{Duration, Instant};

    use minos_core::chunk::ChunkFault;
    use minos_sandbox::spec::BOOT_ALLOWANCE_MS;
    use minos_sandbox::{CrashCause, Ended};
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process_group};

    use super::{
        Classification, DEADLINE_MARKER, HOST_DEADLINE_GRACE, Helper, HelperExit, HostKill,
        Observed, PARENT_GONE_MARKER, SuperviseError, classify, sha256_file, watch, whole_lines,
    };
    use crate::chunks::ReceiveOutcome;

    type TestResult = Result<(), Box<dyn Error>>;

    /// A deadline far past any fake helper's natural end.
    const GENEROUS: Duration = Duration::from_secs(20);

    fn argv(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn later(by: Duration) -> Result<Instant, Box<dyn Error>> {
        Ok(Instant::now().checked_add(by).ok_or("clock overflow")?)
    }

    fn observed(
        exit: HelperExit,
        lines: &[&str],
        killed_for: Option<HostKill>,
        chunks: ReceiveOutcome,
    ) -> Observed {
        Observed::new(
            exit,
            lines.iter().map(|line| (*line).to_owned()).collect(),
            killed_for,
            chunks,
        )
    }

    fn code(code: i32, lines: &[&str]) -> Classification {
        classify(observed(
            HelperExit::Code(code),
            lines,
            None,
            ReceiveOutcome::Incomplete,
        ))
    }

    fn is_ended(classification: &Classification, want: Ended) -> bool {
        matches!(classification, Classification::Ended(got, stream) if *got == want && stream.is_empty())
    }

    fn is_infra(classification: &Classification) -> bool {
        matches!(classification, Classification::Infra(_))
    }

    /// Probes the group with signal 0 until `ESRCH`, the only errno that means no member is left.
    ///
    /// `EPERM` means "keep waiting" only when `ps` shows no member of the group that is not a zombie: on macOS a
    /// group whose remaining members are all zombies awaiting their reap answers `kill(-pgid, 0)` with `EPERM`,
    /// not `ESRCH` (proven in 05-15: a killed, unreaped child probed `EPERM` 5 of 5 times, `ps` showed it as
    /// the group's only member in state `Z`, and the probe turned `ESRCH` after the reap). The loop still ends
    /// only on `ESRCH`; any other errno, or `EPERM` with a live member, fails.
    fn assert_group_gone(pid: u32) -> TestResult {
        let group = Pid::from_raw(i32::try_from(pid)?).ok_or("pid 0")?;
        let gone_by = later(Duration::from_secs(5))?;
        loop {
            match test_kill_process_group(group) {
                Err(Errno::SRCH) => return Ok(()),
                Ok(()) if Instant::now() < gone_by => thread::sleep(Duration::from_millis(20)),
                Ok(()) => return Err("a member of the helper's process group survived".into()),
                Err(Errno::PERM) if only_zombies(pid)? => thread::sleep(Duration::from_millis(20)),
                Err(errno) => return Err(format!("probing the process group: {errno}").into()),
            }
        }
    }

    /// Whether every process `ps` lists in group `pgid` is a zombie (vacuously true when none is listed).
    fn only_zombies(pgid: u32) -> Result<bool, Box<dyn Error>> {
        let ps = std::process::Command::new("/bin/ps")
            .args(["-ax", "-o", "pgid=,stat="])
            .env_clear()
            .output()?;
        let text = String::from_utf8(ps.stdout)?;
        let pgid = pgid.to_string();
        Ok(text
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                (fields.next() == Some(pgid.as_str())).then(|| fields.next())
            })
            .all(|stat| stat.is_some_and(|stat| stat.starts_with('Z'))))
    }

    /// `minos_sandbox::spec::BOOT_ALLOWANCE_MS` documents the same 30 s figure and pins it in
    /// `launcher_bounds_match_minos_vmm` (`crates/minos-sandbox/tests/spec.rs`); change both together.
    #[test]
    fn host_deadline_grace_is_pinned_for_minos_sandbox() {
        assert_eq!(HOST_DEADLINE_GRACE, Duration::from_secs(30));
        assert_eq!(
            HOST_DEADLINE_GRACE,
            Duration::from_millis(BOOT_ALLOWANCE_MS)
        );
    }

    #[test]
    fn supervise_sha256_file_matches_the_abc_vector() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("abc");
        fs::write(&path, b"abc")?;
        assert_eq!(
            sha256_file(&path)?,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        Ok(())
    }

    #[test]
    fn classify_host_deadline_kill_is_timed_out() {
        for exit in [
            HelperExit::Signal {
                signal: 9,
                sent_by_host: true,
            },
            HelperExit::Code(0),
        ] {
            let got = classify(observed(
                exit,
                &[],
                Some(HostKill::Deadline),
                ReceiveOutcome::Complete(b"tar".to_vec()),
            ));
            assert!(is_ended(&got, Ended::TimedOut), "{got:?}");
        }
    }

    #[test]
    fn classify_exit_124_with_the_deadline_line_is_timed_out() {
        let got = code(
            124,
            &[
                "minos-vmm: log warn: x",
                DEADLINE_MARKER,
                "minos-vmm: exit 124",
            ],
        );
        assert!(is_ended(&got, Ended::TimedOut), "{got:?}");
    }

    #[test]
    fn classify_exit_124_without_the_line_is_infra() {
        assert!(is_infra(&code(124, &[])));
        assert!(is_infra(&code(124, &[PARENT_GONE_MARKER])));
    }

    #[test]
    fn classify_console_cap_kill_is_crashed() {
        let got = classify(observed(
            HelperExit::Signal {
                signal: 9,
                sent_by_host: true,
            },
            &[],
            Some(HostKill::ConsoleCap),
            ReceiveOutcome::Incomplete,
        ));
        assert!(
            is_ended(&got, Ended::Crashed(CrashCause::ConsoleCap)),
            "{got:?}"
        );
    }

    #[test]
    fn classify_chunk_fault_kill_is_chunk_protocol() {
        let got = classify(observed(
            HelperExit::Signal {
                signal: 9,
                sent_by_host: true,
            },
            &[],
            Some(HostKill::ChunkFault),
            ReceiveOutcome::Fault(ChunkFault::Token),
        ));
        assert!(is_ended(&got, Ended::ChunkProtocol), "{got:?}");
    }

    #[test]
    fn classify_exit_0_with_a_complete_stream_is_exited_with_the_stream() {
        let got = classify(observed(
            HelperExit::Code(0),
            &[],
            None,
            ReceiveOutcome::Complete(b"the output tar".to_vec()),
        ));
        assert!(
            matches!(&got, Classification::Ended(Ended::Exited, stream) if stream.as_slice() == b"the output tar"),
            "{got:?}"
        );
    }

    #[test]
    fn classify_exit_0_with_no_end_is_crashed_no_status_frame() {
        let got = code(0, &[DEADLINE_MARKER]);
        assert!(
            is_ended(&got, Ended::Crashed(CrashCause::NoStatusFrame)),
            "{got:?}"
        );
    }

    #[test]
    fn classify_exit_0_with_a_fault_is_chunk_protocol() {
        let got = classify(observed(
            HelperExit::Code(0),
            &[],
            None,
            ReceiveOutcome::Fault(ChunkFault::Short),
        ));
        assert!(is_ended(&got, Ended::ChunkProtocol), "{got:?}");
        let got = classify(observed(
            HelperExit::Code(124),
            &[DEADLINE_MARKER],
            None,
            ReceiveOutcome::Fault(ChunkFault::Magic),
        ));
        assert!(is_ended(&got, Ended::ChunkProtocol), "{got:?}");
    }

    #[test]
    fn classify_exit_126_is_infra_with_or_without_its_line() {
        assert!(is_infra(&code(126, &[PARENT_GONE_MARKER])));
        assert!(is_infra(&code(126, &[])));
    }

    #[test]
    fn classify_pre_takeover_codes_are_infra() {
        for exit in [64, 65, 66, 67, 70, 71] {
            assert!(is_infra(&code(exit, &[DEADLINE_MARKER])), "exit {exit}");
        }
    }

    #[test]
    fn classify_other_codes_and_signals_are_infra() {
        assert!(is_infra(&code(3, &[])));
        assert!(is_infra(&code(1, &[])));
        // The pipe console has no 125 console-cap exit: 125 is an unexpected code.
        assert!(is_infra(&code(125, &["minos-vmm: watchdog console-cap"])));
        for sent_by_host in [false, true] {
            let got = classify(observed(
                HelperExit::Signal {
                    signal: 9,
                    sent_by_host,
                },
                &[],
                None,
                ReceiveOutcome::Complete(b"tar".to_vec()),
            ));
            assert!(is_infra(&got), "sent_by_host {sent_by_host}");
        }
        let got = classify(observed(
            HelperExit::Signal {
                signal: 11,
                sent_by_host: false,
            },
            &[],
            None,
            ReceiveOutcome::Incomplete,
        ));
        assert!(is_infra(&got));
    }

    #[test]
    fn markers_are_whole_lines() {
        assert!(is_infra(&code(
            124,
            &["minos-vmm: log warn: x minos-vmm: watchdog deadline"]
        )));
        assert!(is_infra(&code(124, &["minos-vmm: watchdog deadline "])));
        assert!(is_infra(&code(124, &[" minos-vmm: watchdog deadline"])));
        assert!(is_infra(&code(124, &["xminos-vmm: watchdog deadline"])));
        assert!(is_infra(&code(124, &["minos-vmm: watchdog deadlines"])));
        // Bytes the logger wrote with a raw CR or a NUL never become a marker line.
        let lines =
            whole_lines(b"minos-vmm: watchdog deadline\r\nminos-vmm: watchdog dead\0line\n");
        assert_eq!(lines.len(), 2);
        assert!(
            lines.iter().all(|line| line != DEADLINE_MARKER),
            "{lines:?}"
        );
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert!(is_infra(&code(124, &refs)));
    }

    #[test]
    fn whole_lines_drop_an_unterminated_tail() {
        let lines = whole_lines(b"one\ntwo\nminos-vmm: watchdog deadline");
        assert_eq!(lines, ["one", "two"]);
        assert_eq!(
            whole_lines(b"minos-vmm: watchdog deadline\n"),
            [DEADLINE_MARKER]
        );
        assert!(whole_lines(b"").is_empty());
    }

    #[test]
    fn spawn_refuses_an_empty_argv_or_a_relative_program() {
        assert!(matches!(
            Helper::spawn(&[], 16),
            Err(SuperviseError::EmptyArgv)
        ));
        assert!(matches!(
            Helper::spawn(&argv(&["sleep", "1"]), 16),
            Err(SuperviseError::RelativeProgram { .. })
        ));
    }

    #[test]
    fn spawn_failure_is_an_error() {
        assert!(matches!(
            Helper::spawn(&argv(&["/nonexistent/minos-vmm", "x"]), 16),
            Err(SuperviseError::Spawn { .. })
        ));
    }

    #[test]
    fn console_cap_kills_group() -> TestResult {
        let helper = Helper::spawn(&argv(&["/usr/bin/yes"]), 65_536)?;
        let pid = helper.pid();
        let killed_for = watch(&helper, later(GENEROUS)?, || false)?;
        assert_eq!(killed_for, Some(HostKill::ConsoleCap));
        let end = helper.finish()?;
        assert_eq!(end.console().len(), 65_537);
        assert!(matches!(
            end.exit(),
            HelperExit::Signal {
                signal: 9,
                sent_by_host: true
            }
        ));
        assert_group_gone(pid)?;
        let (exit, _, lines) = end.into_parts();
        let got = classify(Observed::new(
            exit,
            lines,
            killed_for,
            ReceiveOutcome::Incomplete,
        ));
        assert!(
            is_ended(&got, Ended::Crashed(CrashCause::ConsoleCap)),
            "{got:?}"
        );
        Ok(())
    }

    #[test]
    fn deadline_kills_group() -> TestResult {
        let start = Instant::now();
        let helper = Helper::spawn(&argv(&["/bin/sleep", "30"]), 1024)?;
        let pid = helper.pid();
        let killed_for = watch(&helper, later(Duration::from_millis(300))?, || false)?;
        assert_eq!(killed_for, Some(HostKill::Deadline));
        assert!(start.elapsed() >= Duration::from_millis(300));
        assert!(start.elapsed() < Duration::from_secs(5));
        let end = helper.finish()?;
        assert!(matches!(
            end.exit(),
            HelperExit::Signal {
                signal: 9,
                sent_by_host: true
            }
        ));
        assert_group_gone(pid)
    }

    #[test]
    fn deadline_kills_every_member_of_the_group() -> TestResult {
        // A fixed test command, not manifest or model input: the shell backgrounds one sleep (a grandchild that
        // survives the leader's death) and execs into another (the leader itself).
        let helper = Helper::spawn(
            &argv(&["/bin/sh", "-c", "/bin/sleep 30 & exec /bin/sleep 30"]),
            1024,
        )?;
        let pid = helper.pid();
        let killed_for = watch(&helper, later(Duration::from_millis(300))?, || false)?;
        assert_eq!(killed_for, Some(HostKill::Deadline));
        helper.finish()?;
        assert_group_gone(pid)
    }

    #[test]
    fn a_member_left_behind_at_exit_is_killed() -> TestResult {
        // The leader exits at once and leaves a sleep holding both pipes; observing the exit kills it.
        let helper = Helper::spawn(&argv(&["/bin/sh", "-c", "/bin/sleep 30 & exit 0"]), 1024)?;
        let pid = helper.pid();
        let killed_for = watch(&helper, later(GENEROUS)?, || false)?;
        assert_eq!(killed_for, None);
        let end = helper.finish()?;
        assert_eq!(end.exit(), HelperExit::Code(0));
        assert_group_gone(pid)
    }

    #[test]
    fn chunk_fault_kills_group() -> TestResult {
        let helper = Helper::spawn(&argv(&["/bin/sleep", "30"]), 1024)?;
        let pid = helper.pid();
        let killed_for = watch(&helper, later(GENEROUS)?, || true)?;
        assert_eq!(killed_for, Some(HostKill::ChunkFault));
        helper.finish()?;
        assert_group_gone(pid)
    }

    #[test]
    fn drop_kills_group() -> TestResult {
        let helper = Helper::spawn(&argv(&["/bin/sleep", "30"]), 1024)?;
        let pid = helper.pid();
        drop(helper);
        assert_group_gone(pid)
    }

    #[test]
    fn env_is_cleared() -> TestResult {
        assert!(
            std::env::vars_os().next().is_some(),
            "the test process has an environment to leak"
        );
        let helper = Helper::spawn(&argv(&["/usr/bin/env"]), 65_536)?;
        assert_eq!(watch(&helper, later(GENEROUS)?, || false)?, None);
        let end = helper.finish()?;
        assert_eq!(end.exit(), HelperExit::Code(0));
        assert!(end.console().is_empty());
        Ok(())
    }

    #[test]
    fn stdin_is_null() -> TestResult {
        let helper = Helper::spawn(&argv(&["/bin/cat"]), 65_536)?;
        assert_eq!(watch(&helper, later(GENEROUS)?, || false)?, None);
        let end = helper.finish()?;
        assert_eq!(end.exit(), HelperExit::Code(0));
        assert!(end.console().is_empty());
        Ok(())
    }

    #[test]
    fn stderr_marker_from_a_real_pipe_classifies() -> TestResult {
        // A fixed test command standing in for minos-vmm's watchdog: a log line, the marker, and exit 124.
        let helper = Helper::spawn(
            &argv(&[
                "/bin/sh",
                "-c",
                "printf 'minos-vmm: log warn: x\\nminos-vmm: watchdog deadline\\n' >&2; printf console; exit 124",
            ]),
            65_536,
        )?;
        let killed_for = watch(&helper, later(GENEROUS)?, || false)?;
        let end = helper.finish()?;
        assert_eq!(end.console(), b"console");
        assert_eq!(
            end.stderr_lines(),
            ["minos-vmm: log warn: x", DEADLINE_MARKER]
        );
        let (exit, _, lines) = end.into_parts();
        let got = classify(Observed::new(
            exit,
            lines,
            killed_for,
            ReceiveOutcome::Incomplete,
        ));
        assert!(is_ended(&got, Ended::TimedOut), "{got:?}");
        Ok(())
    }

    #[test]
    fn stderr_over_its_cap_is_drained() -> TestResult {
        // Three times the cap on stderr: the helper must not block on a full pipe.
        let helper = Helper::spawn(
            &argv(&["/bin/sh", "-c", "/usr/bin/head -c 400000 /dev/zero >&2"]),
            65_536,
        )?;
        assert_eq!(watch(&helper, later(GENEROUS)?, || false)?, None);
        let end = helper.finish()?;
        assert_eq!(end.exit(), HelperExit::Code(0));
        Ok(())
    }
}
