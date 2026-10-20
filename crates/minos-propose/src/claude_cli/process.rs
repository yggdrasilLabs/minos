//! The capped subprocess runner every `claude` invocation goes through (06-CONTEXT D-16; 06-RESEARCH Pattern 3
//! steps 3 to 5).
//!
//! [`run`] starts a program by its path with an argv array, a cleared environment holding exactly the given
//! variables, the given working directory, and its own process group. Its input is written on stdin by a thread,
//! its stdout is read up to a cap (one byte more is an overflow, which ends the run at once), and its stderr is
//! drained with only a capped tail kept. A watcher polls the leader with `waitid(WNOWAIT)`, which observes its
//! exit without reaping it, so its pid, and so its process group id, cannot be reused while the group is killed.
//!
//! In every case, after the leader exits, after an overflow, or at the deadline, the whole process group is sent
//! SIGKILL and the leader is reaped, and only then are the pipe threads joined. Killing the group after a normal
//! exit closes any pipe a straggler in the group still holds, so the readers reach end of file (06-RESEARCH
//! Anti-Patterns: never join the readers before killing the group). A descendant that left the group on its own
//! (`setsid`, `setpgid`) is not killed and can hold a pipe open; so that it cannot hold the call open, every pipe
//! thread blocks only in `poll`, and once the group has been killed and [`DRAIN_GRACE`] has passed it abandons
//! its pipe and returns what it has (review WR-04). A call therefore returns within its deadline plus the grace.

use std::ffi::OsString;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::{Errno, ioctl_fionbio};
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};

use crate::claude_cli::classify::Exit;
use crate::limits::{DRAIN_GRACE, POLL_INTERVAL};
use crate::provider::InfraCause;

/// The readers' chunk size.
const PIPE_CHUNK_BYTES: usize = 8192;

/// How long one `poll` on a pipe waits before the thread checks whether the group was killed. Only the latency
/// of noticing the kill, not a bound on anything.
const DRAIN_POLL: Duration = Duration::from_millis(100);

/// One subprocess to run: what to start, with what, and within which bounds.
pub(crate) struct RunSpec<'a> {
    /// The program, by absolute path; never looked up on `PATH`.
    pub(crate) program: &'a Path,
    /// The arguments after the program.
    pub(crate) args: &'a [OsString],
    /// The child's whole environment; nothing is inherited.
    pub(crate) env: &'a [(OsString, OsString)],
    /// The child's working directory.
    pub(crate) cwd: &'a Path,
    /// Bytes written to the child's stdin, which is then closed.
    pub(crate) stdin: &'a [u8],
    /// How long after the spawn the process group is killed.
    pub(crate) deadline: Duration,
    /// Most stdout bytes accepted; one more ends the run as [`RunError::StdoutTooLarge`].
    pub(crate) stdout_cap: u64,
    /// Most stderr bytes kept, the last ones written; the rest is drained and dropped.
    pub(crate) stderr_cap: u64,
}

/// What a run that ended on its own produced.
#[derive(Debug)]
pub(crate) struct RunOutput {
    /// How the leader ended.
    pub(crate) exit: Exit,
    /// Its whole stdout, at most the cap.
    pub(crate) stdout: Vec<u8>,
    /// The last bytes of its stderr, at most the cap. Hostile text: shown only through `SafeText`.
    pub(crate) stderr_tail: Vec<u8>,
}

/// Why a run produced no output. Unit variants: the OS error is not surfaced, because every provider error is a
/// closed cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunError {
    /// The process could not be started.
    Spawn,
    /// Observing or reaping the process failed.
    Wait,
    /// Reading the process's stdout failed.
    Io,
    /// The deadline passed before the leader exited; the group was killed.
    Deadline,
    /// The process wrote more stdout than its cap; the group was killed.
    StdoutTooLarge,
}

impl RunError {
    /// The infrastructure cause this failure is reported as.
    pub(crate) const fn cause(self) -> InfraCause {
        match self {
            Self::Spawn => InfraCause::Spawn,
            Self::Wait => InfraCause::Wait,
            Self::Io => InfraCause::Io,
            Self::Deadline => InfraCause::Deadline,
            Self::StdoutTooLarge => InfraCause::OutputTooLarge,
        }
    }
}

/// Why the watcher stopped polling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    /// The leader exited; it is still unreaped.
    Exited,
    /// The stdout reader saw more than its cap.
    Overflow,
    /// The deadline passed first.
    Deadline,
    /// `waitid` failed, so the leader's state is unknown.
    WaitFailed,
}

/// Runs `spec` to completion within its bounds.
///
/// The process group is killed and the leader reaped before this returns, whatever happened, so no process the
/// run started is left behind except one that left the group on its own; such a process is not killed, and a
/// pipe it holds is abandoned [`DRAIN_GRACE`] after the kill, so it cannot hold the call open past the deadline
/// plus the grace.
///
/// # Errors
///
/// [`RunError::Spawn`] when the process could not be started, [`RunError::Deadline`] when the deadline passed
/// before the leader exited, [`RunError::StdoutTooLarge`] when stdout passed its cap, [`RunError::Wait`] when the
/// leader could not be observed or reaped, and [`RunError::Io`] when its stdout could not be read.
pub(crate) fn run(spec: &RunSpec<'_>) -> Result<RunOutput, RunError> {
    let mut child = Command::new(spec.program)
        .args(spec.args)
        .env_clear()
        .envs(spec.env.iter().map(|(name, value)| (name, value)))
        .current_dir(spec.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| RunError::Spawn)?;
    let started = Instant::now();
    let pid = Pid::from_child(&child);
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        kill_and_reap(&mut child, pid).ok();
        return Err(RunError::Spawn);
    };
    let overflow = AtomicBool::new(false);
    let killed = AtomicBool::new(false);
    let killed = &killed;
    let (ended, reaped, stdout, stderr_tail) = thread::scope(|scope| {
        let input = spec.stdin;
        // The pipe moves into the thread, so it is closed when the writer is done, not when the scope ends.
        scope.spawn(move || write_input(stdin, input, killed));
        let out = scope.spawn(|| read_capped(stdout, spec.stdout_cap, &overflow, killed));
        let err = scope.spawn(|| keep_tail(stderr, spec.stderr_cap, killed));
        let ended = watch(pid, started, spec.deadline, &overflow);
        // Before any pipe thread is joined: a straggler in the group holding a pipe would otherwise block the
        // join; one outside the group is bounded by the grace that starts when `killed` is set.
        let reaped = kill_and_reap(&mut child, pid);
        killed.store(true, Ordering::SeqCst);
        let stdout = out
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("the stdout reader panicked")));
        let stderr_tail = err.join().unwrap_or_default();
        (ended, reaped, stdout, stderr_tail)
    });
    match ended {
        Ended::Deadline => return Err(RunError::Deadline),
        Ended::WaitFailed => return Err(RunError::Wait),
        Ended::Exited | Ended::Overflow => {}
    }
    if overflow.load(Ordering::SeqCst) {
        return Err(RunError::StdoutTooLarge);
    }
    let status = reaped.map_err(|_| RunError::Wait)?;
    let stdout = stdout.map_err(|_| RunError::Io)?;
    Ok(RunOutput {
        exit: exit_of(status).ok_or(RunError::Wait)?,
        stdout,
        stderr_tail,
    })
}

/// Polls the unreaped leader every [`POLL_INTERVAL`] until it exits, `overflow` is set, or `deadline` has passed
/// since `started`.
fn watch(pid: Pid, started: Instant, deadline: Duration, overflow: &AtomicBool) -> Ended {
    loop {
        match exited_unreaped(pid) {
            Ok(true) => return Ended::Exited,
            Ok(false) => {}
            Err(_) => return Ended::WaitFailed,
        }
        if overflow.load(Ordering::SeqCst) {
            return Ended::Overflow;
        }
        if started.elapsed() >= deadline {
            return Ended::Deadline;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Whether the unreaped leader `pid` has exited, observed with `waitid(WNOWAIT)` so it stays a zombie and keeps
/// its pid, and so its process group id, from being reused (the `minos-backend-krun` supervisor's pattern).
fn exited_unreaped(pid: Pid) -> Result<bool, Errno> {
    match waitid(
        WaitId::Pid(pid),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    ) {
        Ok(status) => Ok(status.is_some()),
        Err(Errno::INTR) => Ok(false),
        Err(errno) => Err(errno),
    }
}

/// Sends SIGKILL to the group the unreaped `child` leads and to the child, then reaps it.
///
/// `ESRCH` (an empty group) and `EPERM` (on macOS, a group whose only members are zombies, proven in 05-15) both
/// mean nothing is left to kill, and any other error still leaves the reap below to run, so the result is ignored.
fn kill_and_reap(child: &mut Child, pid: Pid) -> io::Result<ExitStatus> {
    kill_process_group(pid, Signal::KILL).ok();
    child.kill().ok();
    child.wait()
}

/// A bounded wait on one end of a pipe. Every pipe thread blocks only here, in `poll` with a short timeout, so it
/// can notice that the group was killed; [`DRAIN_GRACE`] after it first sees that, it abandons the pipe, which
/// bounds the call even when a process outside the group still holds the other end.
struct Drain<'a> {
    killed: &'a AtomicBool,
    /// When this thread first saw the kill.
    seen: Option<Instant>,
}

impl<'a> Drain<'a> {
    const fn new(killed: &'a AtomicBool) -> Self {
        Self { killed, seen: None }
    }

    /// Waits until `fd` is ready for `flags` (a hang-up counts as ready: the read or write then reports it), or
    /// `false` once the pipe is abandoned.
    fn ready(&mut self, fd: &impl AsFd, flags: PollFlags) -> io::Result<bool> {
        let timeout = Timespec::try_from(DRAIN_POLL).map_err(io::Error::other)?;
        loop {
            if self.killed.load(Ordering::SeqCst) {
                let seen = self.seen.get_or_insert_with(Instant::now);
                if seen.elapsed() >= DRAIN_GRACE {
                    return Ok(false);
                }
            }
            let mut fds = [PollFd::new(fd, flags)];
            match rustix::event::poll(&mut fds, Some(&timeout)) {
                Ok(0) | Err(Errno::INTR) => {}
                Ok(_) => return Ok(true),
                Err(errno) => return Err(errno.into()),
            }
        }
    }
}

/// Writes `input` to the child's stdin, then closes it by dropping the pipe at the end.
///
/// A write error, `BrokenPipe` included, is not a failure: the child may exit without reading its input, and its
/// output says what happened. The pipe is made non-blocking, because a blocking write of more than one pipe
/// buffer would not return until the child read it all, which a child that never reads its input would prevent
/// forever; a full pipe is waited on through [`Drain`], so an abandoned pipe is simply closed unread.
fn write_input(pipe: ChildStdin, input: &[u8], killed: &AtomicBool) {
    let mut pipe = pipe;
    if ioctl_fionbio(&pipe, true).is_err() {
        return;
    }
    let mut drain = Drain::new(killed);
    let mut rest = input;
    while !rest.is_empty() {
        match drain.ready(&pipe, PollFlags::OUT) {
            Ok(true) => {}
            Ok(false) | Err(_) => return,
        }
        match pipe.write(rest) {
            Ok(0) => return,
            Ok(written) => rest = rest.get(written..).unwrap_or_default(),
            Err(error)
                if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {}
            Err(_) => return,
        }
    }
}

/// Reads `pipe` up to `cap` bytes. On one byte more, sets `overflow`, drops what was kept, and keeps draining
/// (so the writer never blocks while the watcher kills the group) until end of file or the pipe is abandoned.
fn read_capped(
    pipe: ChildStdout,
    cap: u64,
    overflow: &AtomicBool,
    killed: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let mut pipe = pipe;
    let mut drain = Drain::new(killed);
    let mut kept = Vec::new();
    let mut chunk = [0_u8; PIPE_CHUNK_BYTES];
    loop {
        if !drain.ready(&pipe, PollFlags::IN)? {
            break;
        }
        // After `poll` reported the pipe readable or hung up, this read returns without blocking.
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(_) if overflow.load(Ordering::SeqCst) => {}
            Ok(read) => {
                let total = u64::try_from(kept.len().saturating_add(read)).unwrap_or(u64::MAX);
                if total > cap {
                    overflow.store(true, Ordering::SeqCst);
                    kept = Vec::new();
                } else {
                    kept.extend_from_slice(chunk.get(..read).unwrap_or_default());
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(kept)
}

/// Reads `pipe` to its end, or until it is abandoned, and keeps only the last `cap` bytes.
///
/// A read error ends the reading and keeps what was read: the tail is a diagnostic only, never a result.
fn keep_tail(pipe: ChildStderr, cap: u64, killed: &AtomicBool) -> Vec<u8> {
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut pipe = pipe;
    let mut drain = Drain::new(killed);
    let mut kept = Vec::new();
    let mut chunk = [0_u8; PIPE_CHUNK_BYTES];
    // An abandoned pipe or a poll error ends the reading too.
    while let Ok(true) = drain.ready(&pipe, PollFlags::IN) {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                kept.extend_from_slice(chunk.get(..read).unwrap_or_default());
                // Trimmed at twice the cap, so the copy is amortized.
                if kept.len() > cap.saturating_mul(2) {
                    let excess = kept.len().saturating_sub(cap);
                    kept.drain(..excess);
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    let excess = kept.len().saturating_sub(cap);
    kept.drain(..excess);
    kept
}

/// How `status` ended: an exit code or a signal. `None` for a status that is neither, which Unix never reports.
fn exit_of(status: ExitStatus) -> Option<Exit> {
    status
        .code()
        .map(Exit::Code)
        .or_else(|| status.signal().map(Exit::Signal))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;
    use std::thread;
    use std::time::{Duration, Instant};

    use rustix::io::Errno;
    use rustix::process::{Pid, Signal, kill_process, test_kill_process};
    use tempfile::TempDir;

    use super::{RunError, RunOutput, RunSpec, run};
    use crate::claude_cli::classify::Exit;
    use crate::limits::DRAIN_GRACE;

    type TestResult = Result<(), Box<dyn Error>>;

    /// How long a test waits for anything that should be prompt.
    const PROMPT: Duration = Duration::from_secs(5);

    /// A spec for `program` with `args` in `cwd`, no environment, no input, a 30 s deadline, and 64 KiB caps.
    fn spec<'a>(program: &'a Path, args: &'a [OsString], cwd: &'a Path) -> RunSpec<'a> {
        RunSpec {
            program,
            args,
            env: &[],
            cwd,
            stdin: &[],
            deadline: Duration::from_secs(30),
            stdout_cap: 65_536,
            stderr_cap: 65_536,
        }
    }

    /// The argv `-c <script>` for `/bin/sh`; every script is fixed test text.
    fn shell(script: &str) -> Vec<OsString> {
        vec![OsString::from("-c"), OsString::from(script)]
    }

    fn expect_output(result: Result<RunOutput, RunError>) -> Result<RunOutput, Box<dyn Error>> {
        result.map_err(|error| format!("expected output, got {error:?}").into())
    }

    /// Polls `pid` until it no longer exists, for at most [`PROMPT`].
    fn gone(pid: Pid) -> bool {
        let start = Instant::now();
        while start.elapsed() < PROMPT {
            if test_kill_process(pid) == Err(Errno::SRCH) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn stdin_larger_than_a_pipe_buffer_round_trips() -> TestResult {
        let dir = TempDir::new()?;
        let input: Vec<u8> = (0..102_400_u32)
            .map(|i| u8::try_from(i % 251).unwrap_or_default())
            .collect();
        let mut spec = spec(Path::new("/bin/cat"), &[], dir.path());
        spec.stdin = &input;
        spec.stdout_cap = 1_048_576;
        let output = expect_output(run(&spec))?;
        assert_eq!(output.exit, Exit::Code(0));
        assert_eq!(output.stdout, input);
        Ok(())
    }

    #[test]
    fn the_child_sees_exactly_the_given_environment() -> TestResult {
        let dir = TempDir::new()?;
        let env = [
            (OsString::from("A"), OsString::from("1")),
            (OsString::from("B"), OsString::from("2")),
        ];
        let mut spec = spec(Path::new("/usr/bin/env"), &[], dir.path());
        spec.env = &env;
        let output = expect_output(run(&spec))?;
        assert_eq!(output.exit, Exit::Code(0));
        let mut lines: Vec<&str> = std::str::from_utf8(&output.stdout)?.lines().collect();
        lines.sort_unstable();
        assert_eq!(lines, ["A=1", "B=2"]);
        Ok(())
    }

    #[test]
    fn the_child_runs_in_the_given_directory() -> TestResult {
        let dir = TempDir::new()?;
        let args = [OsString::from("-P")];
        let output = expect_output(run(&spec(Path::new("/bin/pwd"), &args, dir.path())))?;
        assert_eq!(output.exit, Exit::Code(0));
        let printed = std::str::from_utf8(&output.stdout)?.trim_end();
        assert_eq!(Path::new(printed), fs::canonicalize(dir.path())?.as_path());
        Ok(())
    }

    #[test]
    fn an_exit_code_is_reported() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("exit 3");
        let output = expect_output(run(&spec(Path::new("/bin/sh"), &args, dir.path())))?;
        assert_eq!(output.exit, Exit::Code(3));
        Ok(())
    }

    #[test]
    fn a_signal_is_reported() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("kill -9 $$");
        let output = expect_output(run(&spec(Path::new("/bin/sh"), &args, dir.path())))?;
        assert_eq!(output.exit, Exit::Signal(9));
        Ok(())
    }

    #[test]
    fn the_deadline_kills_the_whole_process_group() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("/bin/sleep 30 & echo $! > straggler.pid; /bin/sleep 30");
        let mut spec = spec(Path::new("/bin/sh"), &args, dir.path());
        spec.deadline = Duration::from_millis(300);
        let start = Instant::now();
        let result = run(&spec);
        assert!(start.elapsed() < PROMPT, "took {:?}", start.elapsed());
        assert!(matches!(result, Err(RunError::Deadline)), "{result:?}");
        let raw: i32 = fs::read_to_string(dir.path().join("straggler.pid"))?
            .trim()
            .parse()?;
        let pid = Pid::from_raw(raw).ok_or("the straggler pid is not positive")?;
        assert!(
            gone(pid),
            "the background sleep {raw} outlived the deadline"
        );
        Ok(())
    }

    #[test]
    fn a_straggler_holding_the_pipes_does_not_hang_the_run() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("/bin/sleep 30 & exit 0");
        let start = Instant::now();
        let output = expect_output(run(&spec(Path::new("/bin/sh"), &args, dir.path())))?;
        assert!(start.elapsed() < PROMPT, "took {:?}", start.elapsed());
        assert_eq!(output.exit, Exit::Code(0));
        Ok(())
    }

    /// The pid the script wrote to `straggler.pid` in `dir`.
    fn straggler(dir: &Path) -> Result<Pid, Box<dyn Error>> {
        let raw: i32 = fs::read_to_string(dir.join("straggler.pid"))?
            .trim()
            .parse()?;
        Pid::from_raw(raw).ok_or_else(|| "the straggler pid is not positive".into())
    }

    /// A background job under `set -m` gets its own process group (verified on this host: `ps -o pgid` shows the
    /// job's own pid), so it survives the group kill and keeps every inherited pipe open. The leader exits at
    /// once with nothing read from stdin; the run returns once the grace passes, with the leader's result.
    #[test]
    fn a_straggler_outside_the_group_cannot_hold_the_run_past_the_grace() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("set -m; /bin/sleep 30 & echo $! > straggler.pid; printf ok; exit 0");
        // More than a pipe buffer, so the writer would block forever on the unread input without the bound.
        let input = vec![b'x'; 1_048_576];
        let mut spec = spec(Path::new("/bin/sh"), &args, dir.path());
        spec.stdin = &input;
        let start = Instant::now();
        let output = expect_output(run(&spec))?;
        let took = start.elapsed();
        let pid = straggler(dir.path())?;
        // Alive: the group kill did not reach it, which is exactly the case the grace bounds.
        let alive = test_kill_process(pid) == Ok(());
        kill_process(pid, Signal::KILL).ok();
        assert!(alive, "the straggler {pid:?} did not escape the group");
        assert!(took >= DRAIN_GRACE, "took {took:?}, less than the grace");
        assert!(
            took < DRAIN_GRACE.saturating_add(PROMPT),
            "took {took:?}, more than the grace"
        );
        assert_eq!(output.exit, Exit::Code(0));
        assert_eq!(output.stdout, b"ok");
        Ok(())
    }

    /// The same straggler under a leader that runs past a short deadline: the deadline kills the group and the
    /// grace bounds the drain, so the deadline failure arrives within the deadline plus the grace. The leader
    /// waits with the `wait` builtin, because under `set -m` a foreground job would also get its own group and
    /// outlive the test.
    #[test]
    fn the_deadline_holds_with_a_straggler_outside_the_group() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("set -m; /bin/sleep 30 & echo $! > straggler.pid; wait");
        let mut spec = spec(Path::new("/bin/sh"), &args, dir.path());
        spec.deadline = Duration::from_millis(300);
        let start = Instant::now();
        let result = run(&spec);
        let took = start.elapsed();
        let pid = straggler(dir.path())?;
        let alive = test_kill_process(pid) == Ok(());
        kill_process(pid, Signal::KILL).ok();
        assert!(alive, "the straggler {pid:?} did not escape the group");
        assert!(matches!(result, Err(RunError::Deadline)), "{result:?}");
        assert!(
            took < DRAIN_GRACE.saturating_add(PROMPT),
            "took {took:?}, more than the deadline plus the grace"
        );
        Ok(())
    }

    #[test]
    fn stdout_over_its_cap_kills_the_run() -> TestResult {
        let dir = TempDir::new()?;
        let mut spec = spec(Path::new("/usr/bin/yes"), &[], dir.path());
        spec.stdout_cap = 1_048_576;
        let start = Instant::now();
        let result = run(&spec);
        assert!(start.elapsed() < PROMPT, "took {:?}", start.elapsed());
        assert!(
            matches!(result, Err(RunError::StdoutTooLarge)),
            "{result:?}"
        );
        Ok(())
    }

    #[test]
    fn a_flood_of_stderr_keeps_only_a_capped_tail() -> TestResult {
        let dir = TempDir::new()?;
        let args = shell("/usr/bin/head -c 1000000 /dev/zero >&2; printf ok");
        let mut spec = spec(Path::new("/bin/sh"), &args, dir.path());
        spec.stderr_cap = 4096;
        let start = Instant::now();
        let output = expect_output(run(&spec))?;
        assert!(start.elapsed() < PROMPT, "took {:?}", start.elapsed());
        assert_eq!(output.exit, Exit::Code(0));
        assert_eq!(output.stdout, b"ok");
        assert!(
            output.stderr_tail.len() <= 4096,
            "{}",
            output.stderr_tail.len()
        );
        assert!(!output.stderr_tail.is_empty());
        Ok(())
    }

    #[test]
    fn a_missing_program_is_a_spawn_failure() -> TestResult {
        let dir = TempDir::new()?;
        let result = run(&spec(
            Path::new("/nonexistent/minos-test-claude"),
            &[],
            dir.path(),
        ));
        assert!(matches!(result, Err(RunError::Spawn)), "{result:?}");
        Ok(())
    }
}
