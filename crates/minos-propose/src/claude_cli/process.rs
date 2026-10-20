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
//! SIGKILL and the leader is reaped, and only then are the reader threads joined. Killing the group after a normal
//! exit closes any pipe a straggler still holds, so the readers always reach end of file and no process the call
//! started outlives it (06-RESEARCH Anti-Patterns: never join the readers before killing the group).

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use crate::claude_cli::classify::Exit;
use crate::provider::InfraCause;

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

/// Runs `spec` to completion within its bounds.
pub(crate) fn run(spec: &RunSpec<'_>) -> Result<RunOutput, RunError> {
    let _ = spec;
    Err(RunError::Spawn)
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
    use rustix::process::{Pid, test_kill_process};
    use tempfile::TempDir;

    use super::{RunError, RunOutput, RunSpec, run};
    use crate::claude_cli::classify::Exit;

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
        assert!(gone(pid), "the background sleep {raw} outlived the deadline");
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

    #[test]
    fn stdout_over_its_cap_kills_the_run() -> TestResult {
        let dir = TempDir::new()?;
        let mut spec = spec(Path::new("/usr/bin/yes"), &[], dir.path());
        spec.stdout_cap = 1_048_576;
        let start = Instant::now();
        let result = run(&spec);
        assert!(start.elapsed() < PROMPT, "took {:?}", start.elapsed());
        assert!(matches!(result, Err(RunError::StdoutTooLarge)), "{result:?}");
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
        assert!(output.stderr_tail.len() <= 4096, "{}", output.stderr_tail.len());
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
