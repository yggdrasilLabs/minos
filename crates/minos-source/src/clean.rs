//! The clean-tree check (04-CONTEXT D-02, CLI-11): the one place Minos runs `git`.
//!
//! The rule "no git in the trusted path" exists because git on a candidate-controlled tree brings that tree's
//! hooks and config into the trusted path. This check runs the user's own `git` on the user's own working tree,
//! which no candidate ever writes (`adopt` writes a patch file only), so the rule's reason does not apply. The
//! moment a tree could be candidate-controlled, the rule applies in full.
//!
//! It runs two read-only commands as argv arrays, never through a shell, from the project directory:
//! [`STATUS_ARGS`] (`git status --porcelain`) and [`TRACKED_ARGS`] (`git ls-files`). The environment is cleared
//! to `PATH` ([`GIT_PATH`]), `LC_ALL=C`, `GIT_CONFIG_NOSYSTEM=1`, and `GIT_CONFIG_GLOBAL=/dev/null`, so neither
//! system nor global config applies and there is no `HOME`; `core.fsmonitor` is off, so `status` starts no
//! configured program, and `core.hooksPath` is `/dev/null`. Git never contributes a byte to the tarball: the
//! tarball is built from the filesystem walk alone.
//!
//! The tree is clean only when `status` prints nothing for the project directory and every path to be packed is
//! tracked. The second condition closes the gap between the walk and git: a file ignored only through
//! `.git/info/exclude` or a global excludes file is invisible to `status` but would still be packed
//! (04-RESEARCH Pitfall 1). Every doubt counts as dirty: not a repository, no `git` on that `PATH`, a non-zero
//! exit, a timeout, or over-cap output. [`CleanPolicy::AllowDirty`] is then the only way to proceed.
//!
//! Each git run has a [`GIT_DEADLINE`], after which it is killed and reaped, and its stdout and stderr are read
//! on scoped threads under [`GIT_STDOUT_CAP`] and [`GIT_STDERR_CAP`], so a full pipe cannot deadlock it. stderr is
//! drained only and never shown: it can hold paths with control characters.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use minos_core::path::RelPath;

/// The program run, resolved through [`GIT_PATH`] only, so a `git` elsewhere is not found and the tree is dirty.
pub const GIT_PROGRAM: &str = "git";

/// The only `PATH` git sees: the system directories, never the user's shell `PATH`.
pub const GIT_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// How long one git run may take before it is killed and the tree counts as dirty.
pub const GIT_DEADLINE: Duration = Duration::from_secs(30);

/// The most stdout read from one git run (16 MiB); more counts as dirty.
pub const GIT_STDOUT_CAP: u64 = 16 << 20;

/// The most stderr drained from one git run (64 KiB). It is never shown.
pub const GIT_STDERR_CAP: u64 = 64 << 10;

/// How often a running git is polled against its deadline.
const POLL: Duration = Duration::from_millis(5);

/// The most packed-but-untracked paths a [`Dirty::Untracked`] lists; its count is always the full total.
const SHOWN_UNTRACKED: usize = 20;

/// `git status` for the project directory, after the program name. `-z` gives NUL-separated, unquoted output.
pub const STATUS_ARGS: [&str; 11] = [
    "--no-optional-locks",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--",
    ".",
];

/// `git ls-files` for the project directory, after the program name. It prints paths relative to the project
/// directory, NUL-separated and unquoted.
pub const TRACKED_ARGS: [&str; 9] = [
    "--no-optional-locks",
    "-c",
    "core.fsmonitor=false",
    "ls-files",
    "-z",
    "--cached",
    "--recurse-submodules",
    "--",
    ".",
];

/// Whether a snapshot requires a clean tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanPolicy {
    /// Run [`check_clean`] and refuse a dirty tree (the default).
    RequireClean,
    /// Skip [`check_clean`] (`--allow-dirty`). The denylist and the secret scan always run (04-CONTEXT D-06).
    AllowDirty,
}

/// Why the tree does not count as clean.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Dirty {
    /// git could not be started.
    #[error(
        "git could not be started from /usr/bin:/bin:/usr/sbin:/sbin; install git, or pass --allow-dirty"
    )]
    GitUnavailable,
    /// git exited unsuccessfully, for example because the project is not inside a repository.
    #[error(
        "git exited unsuccessfully ({}); is the project inside a git repository? commit it, or pass --allow-dirty",
        exit_text(*.code)
    )]
    GitFailed {
        /// The exit code, or `None` if git was ended by a signal or could not be waited on.
        code: Option<i32>,
    },
    /// git did not finish within [`GIT_DEADLINE`].
    #[error("git did not finish within 30 s; check the repository, or pass --allow-dirty")]
    TimedOut,
    /// git printed more than [`GIT_STDOUT_CAP`].
    #[error("git printed more than 16 MiB; commit or clean the tree, or pass --allow-dirty")]
    OutputTooLarge,
    /// `git status` lists changes in the project directory.
    #[error("the project has uncommitted changes; commit them, or pass --allow-dirty")]
    Changes,
    /// Paths that would be packed but git does not track, typically ignored only through `.git/info/exclude` or a
    /// global excludes file.
    #[error(
        "{count} path(s) would be packed but are not tracked by git: {}; add them to .minosignore, commit them, or pass --allow-dirty",
        path_list(.shown, *.count)
    )]
    Untracked {
        /// How many packed paths git does not track.
        count: u64,
        /// The first of them in path order, at most 20.
        shown: Vec<RelPath>,
    },
}

/// The exit code as text.
fn exit_text(code: Option<i32>) -> String {
    code.map_or_else(
        || "no exit code".to_owned(),
        |code| format!("exit code {code}"),
    )
}

/// The shown paths, comma-separated, noting how many more there are. Paths are printable by construction.
fn path_list(shown: &[RelPath], count: u64) -> String {
    let mut text = String::new();
    for (index, path) in shown.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push_str(path.as_str());
    }
    let listed = u64::try_from(shown.len()).unwrap_or(u64::MAX);
    if count > listed {
        write!(text, ", and {} more", count.saturating_sub(listed)).ok();
    }
    text
}

/// Whether the project at `root` is committed: `git status` prints nothing for it, and every path in `packed`
/// (relative to `root`) is tracked by git.
///
/// # Errors
///
/// - [`Dirty::GitUnavailable`] if git cannot be started (no `git` on [`GIT_PATH`], or `root` does not exist).
/// - [`Dirty::GitFailed`] if git exits unsuccessfully, as it does outside a repository, or cannot be waited on.
/// - [`Dirty::TimedOut`] if a git run exceeds [`GIT_DEADLINE`].
/// - [`Dirty::OutputTooLarge`] if a git run prints more than [`GIT_STDOUT_CAP`].
/// - [`Dirty::Changes`] if `git status` lists any change in the project directory.
/// - [`Dirty::Untracked`] if a packed path is not tracked.
pub fn check_clean(root: &Path, packed: &BTreeSet<RelPath>) -> Result<(), Dirty> {
    if !run_git(GIT_PROGRAM, root, &STATUS_ARGS)?.is_empty() {
        return Err(Dirty::Changes);
    }
    let listed = run_git(GIT_PROGRAM, root, &TRACKED_ARGS)?;
    // A tracked path that does not fit the path grammar can never be packed, so dropping it changes nothing.
    let tracked: BTreeSet<RelPath> = listed
        .split(|byte| *byte == 0)
        .filter_map(|entry| std::str::from_utf8(entry).ok())
        .filter_map(|text| RelPath::try_from(text).ok())
        .collect();
    let mut untracked = packed.iter().filter(|path| !tracked.contains(*path));
    let shown: Vec<RelPath> = untracked.by_ref().take(SHOWN_UNTRACKED).cloned().collect();
    if shown.is_empty() {
        return Ok(());
    }
    let rest = untracked.count();
    let count = u64::try_from(shown.len().saturating_add(rest)).unwrap_or(u64::MAX);
    Err(Dirty::Untracked { count, shown })
}

/// Runs `program` with `args` in `root` under the git deadline and caps, returning its stdout.
fn run_git(program: &str, root: &Path, args: &[&str]) -> Result<Vec<u8>, Dirty> {
    run_capped(program, root, args, GIT_DEADLINE, GIT_STDOUT_CAP)
}

/// Runs `program` with `args` in `root`, with the fixed environment, a `deadline`, and a `stdout_cap`.
fn run_capped(
    program: &str,
    root: &Path,
    args: &[&str],
    deadline: Duration,
    stdout_cap: u64,
) -> Result<Vec<u8>, Dirty> {
    let child = Command::new(program)
        .args(args)
        .current_dir(root)
        .env_clear()
        .env("PATH", GIT_PATH)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Dirty::GitUnavailable)?;
    let mut child = Reaper(child);
    let stdout = child.0.stdout.take();
    let stderr = child.0.stderr.take();
    // Both pipes are read while the child runs, so neither can fill and block it. A reader that reaches its cap
    // stops and closes its pipe, and git then fails on its next write instead of blocking.
    let (exit, output) = thread::scope(|scope| {
        let out = scope.spawn(move || read_capped(stdout, stdout_cap));
        let err = scope.spawn(move || read_capped(stderr, GIT_STDERR_CAP));
        let exit = wait_until(&mut child.0, deadline);
        let output = out.join();
        err.join().ok();
        (exit, output)
    });
    let status = exit?;
    let output = output
        .ok()
        .and_then(Result::ok)
        .ok_or(Dirty::GitFailed { code: None })?;
    if u64::try_from(output.len()).map_or(true, |len| len > stdout_cap) {
        return Err(Dirty::OutputTooLarge);
    }
    if !status.success() {
        return Err(Dirty::GitFailed {
            code: status.code(),
        });
    }
    Ok(output)
}

/// Reads at most `cap + 1` bytes from `pipe`, so a reader of more than `cap` bytes is detectable.
fn read_capped<R: Read>(pipe: Option<R>, cap: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(pipe) = pipe {
        pipe.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

/// Waits for `child` until `deadline` has passed since the call, then kills and reaps it.
fn wait_until(child: &mut Child, deadline: Duration) -> Result<ExitStatus, Dirty> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if start.elapsed() < deadline => thread::sleep(POLL),
            Ok(None) => {
                child.kill().ok();
                child.wait().ok();
                return Err(Dirty::TimedOut);
            }
            Err(_) => {
                child.kill().ok();
                child.wait().ok();
                return Err(Dirty::GitFailed { code: None });
            }
        }
    }
}

/// Kills and reaps the child on every exit path, including a panic.
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::{Dirty, run_capped, run_git};

    #[test]
    fn a_program_that_does_not_exist_is_unavailable() {
        let result = run_git("/nonexistent/minos-test-git", Path::new("/"), &["status"]);
        assert!(matches!(result, Err(Dirty::GitUnavailable)), "{result:?}");
    }

    #[test]
    fn a_missing_working_directory_is_unavailable() {
        let result = run_git(
            "git",
            Path::new("/nonexistent/minos-test-project"),
            &["status"],
        );
        assert!(matches!(result, Err(Dirty::GitUnavailable)), "{result:?}");
    }

    #[test]
    fn a_run_past_its_deadline_is_killed_and_timed_out() {
        let start = Instant::now();
        let result = run_capped(
            "/bin/sleep",
            Path::new("/"),
            &["10"],
            Duration::from_millis(100),
            1024,
        );
        assert!(matches!(result, Err(Dirty::TimedOut)), "{result:?}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn output_over_the_cap_is_too_large() {
        let start = Instant::now();
        let result = run_capped(
            "/usr/bin/yes",
            Path::new("/"),
            &[],
            Duration::from_secs(10),
            4096,
        );
        assert!(matches!(result, Err(Dirty::OutputTooLarge)), "{result:?}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_non_zero_exit_is_failed_with_its_code() {
        let result = run_capped(
            "/usr/bin/false",
            Path::new("/"),
            &[],
            Duration::from_secs(10),
            1024,
        );
        assert!(
            matches!(result, Err(Dirty::GitFailed { code: Some(1) })),
            "{result:?}"
        );
    }

    #[test]
    fn the_program_sees_only_the_allowlisted_environment() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = run_capped(
            "/usr/bin/env",
            Path::new("/"),
            &[],
            Duration::from_secs(10),
            1024,
        )
        .map_err(|dirty| dirty.to_string())?;
        let mut lines: Vec<&str> = std::str::from_utf8(&output)?.lines().collect();
        lines.sort_unstable();
        assert_eq!(
            lines,
            [
                "GIT_CONFIG_GLOBAL=/dev/null",
                "GIT_CONFIG_NOSYSTEM=1",
                "LC_ALL=C",
                "PATH=/usr/bin:/bin:/usr/sbin:/sbin",
            ]
        );
        Ok(())
    }
}
