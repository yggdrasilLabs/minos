//! The container builder that turns a project's Dockerfile into a rootfs tar (05-RESEARCH Pattern 10; 02-CONTEXT
//! D-14; 02-SPIKE-EVIDENCE.md RI-03).
//!
//! [`RootfsBuilder`] is the seam `minos env build` is written against, so its pipeline is tested with a fake
//! builder and runs with [`DockerBuildx`] for real. A builder takes the build context as bytes (the source
//! snapshot's tarball, and nothing else) and streams back the rootfs tar, which is hostile input to the trusted
//! packer.
//!
//! [`DockerBuildx`] runs
//! `docker buildx build --platform linux/arm64 --provenance=false --sbom=false --output type=tar,dest=- -f
//! <dockerfile> -` as an argv array, never a shell string, by the canonical path of the first executable `docker`
//! in an absolute directory of the user's `PATH`. The environment is cleared to `HOME` and a `PATH` of that
//! directory, the binary's canonical directory, and the system directories (RI-03), so proxies, credentials, and
//! `DOCKER_*` overrides in the user's shell do not reach it. Attestations are off because the tar exporter would
//! otherwise write their JSON files into the export root. `-f` is relative to the root of the piped context; that
//! buildx accepts a piped tar context is verified with Docker running in plan 05-18.
//!
//! The context is written to the builder's stdin by its own thread, the rootfs tar is read from its stdout, and
//! its stderr is kept up to [`BUILDER_STDERR_CAP`] bytes (the rest drained) and shown only through
//! [`SafeText`]. The builder runs in its own process group, which a watchdog thread kills at
//! [`BUILD_DEADLINE`], or as soon as the [`RunningBuild`] is dropped without [`RunningBuild::finish`]. The
//! watchdog is the only code that reaps the child, so a kill can never reach a reused process ID.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use minos_core::lock::{BuilderId, BuilderText};
use minos_core::path::RelPath;
use minos_core::text::SafeText;
use rustix::process::{Pid, Signal, kill_process_group};

/// How long one image build may run before its process group is killed.
pub const BUILD_DEADLINE: Duration = Duration::from_mins(30);

/// How long `docker buildx version` may take.
pub const IDENTITY_DEADLINE: Duration = Duration::from_secs(10);

/// The most stderr bytes kept from a builder run; the rest is drained and dropped.
pub const BUILDER_STDERR_CAP: u64 = 64 * 1024;

/// The most stdout bytes read from `docker buildx version`.
const IDENTITY_STDOUT_CAP: u64 = 4096;

/// The name recorded in `minos.lock` for this builder.
pub const DOCKER_BUILDX_NAME: &str = "docker-buildx";

/// The directories every builder `PATH` ends with.
const SYSTEM_DIRS: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];

/// How often the watchdog polls the builder.
const POLL: Duration = Duration::from_millis(20);

/// Why the builder could not produce a rootfs tar.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    /// No executable `docker` in an absolute directory of the user's `PATH`.
    #[error(
        "docker was not found on PATH: minos env build needs Docker with buildx (run minos host doctor)"
    )]
    NotFound,
    /// The builder process could not be started.
    #[error("the builder could not be started")]
    Spawn(#[source] io::Error),
    /// The builder exited unsuccessfully.
    #[error("the builder exited {}: {stderr}", exit_text(*.code))]
    Failed {
        /// The exit code, or `None` when a signal ended it.
        code: Option<i32>,
        /// The start of its stderr, sanitized.
        stderr: SafeText,
    },
    /// `docker buildx version` did not answer with one line holding a version token.
    #[error("docker buildx version did not report a version: is the buildx plugin installed?")]
    Identity,
    /// The builder ran past its deadline and was killed.
    #[error("the builder ran past its deadline and was killed")]
    TimedOut,
    /// The build context could not be written to the builder's stdin while the builder reported success.
    #[error("the build context could not be written to the builder")]
    Context(#[source] io::Error),
    /// The builder could not be waited on.
    #[error("the builder could not be waited on")]
    Wait(#[source] io::Error),
}

/// An exit code as text.
fn exit_text(code: Option<i32>) -> String {
    code.map_or_else(
        || "on a signal".to_owned(),
        |code| format!("with code {code}"),
    )
}

/// A builder that turns a build context and a Dockerfile path into a streamed rootfs tar.
pub trait RootfsBuilder {
    /// The stream the rootfs tar is read from.
    type Rootfs: Read;

    /// The builder's name and version, recorded in `minos.lock` (05-CONTEXT D-11).
    ///
    /// # Errors
    ///
    /// A [`BuilderError`] when the builder cannot be asked or does not answer with a version.
    fn identity(&self) -> Result<BuilderId, BuilderError>;

    /// Starts a build of `dockerfile` (relative to the context's root) with `context`, a tarball, as the whole
    /// build context.
    ///
    /// # Errors
    ///
    /// A [`BuilderError`] when the build cannot be started.
    fn start(
        &self,
        context: Vec<u8>,
        dockerfile: &RelPath,
    ) -> Result<RunningBuild<Self::Rootfs>, BuilderError>;
}

/// A build in progress: its rootfs stream, and how to wait for its result.
///
/// Dropping it without [`RunningBuild::finish`] kills a process-backed build.
#[must_use = "a running build is killed when dropped; call finish"]
pub struct RunningBuild<R> {
    rootfs: R,
    wait: BuildWait,
}

/// How a [`RunningBuild`] ends.
enum BuildWait {
    /// An in-process builder that already knows its result.
    Completed(Result<(), BuilderError>),
    /// A builder process under a watchdog.
    Process(Box<BuildProcess>),
}

impl<R> RunningBuild<R> {
    /// A build whose result is already known, for builders that run in-process (tests use one).
    pub const fn completed(rootfs: R, outcome: Result<(), BuilderError>) -> Self {
        Self {
            rootfs,
            wait: BuildWait::Completed(outcome),
        }
    }

    /// The rootfs tar stream.
    pub const fn rootfs_mut(&mut self) -> &mut R {
        &mut self.rootfs
    }

    /// Closes the rootfs stream, waits for the builder, and requires that it succeeded.
    ///
    /// # Errors
    ///
    /// [`BuilderError::Failed`] for an unsuccessful exit, [`BuilderError::TimedOut`] past the deadline,
    /// [`BuilderError::Context`] when the context could not be written to a builder that then succeeded, and
    /// [`BuilderError::Wait`] when the builder could not be waited on.
    pub fn finish(self) -> Result<(), BuilderError> {
        let Self { rootfs, wait } = self;
        // Closing the stream first lets a builder still writing fail on its next write instead of blocking.
        drop(rootfs);
        match wait {
            BuildWait::Completed(outcome) => outcome,
            BuildWait::Process(process) => process.finish().map(|_| ()),
        }
    }
}

/// A builder child process: the watchdog that owns it, and the threads feeding and draining its pipes.
struct BuildProcess {
    /// Kept alive while the build should run; dropping it tells the watchdog to kill the build.
    keep: Sender<()>,
    watchdog: JoinHandle<Result<ExitStatus, BuilderError>>,
    stdin: Option<JoinHandle<io::Result<()>>>,
    stderr: JoinHandle<Vec<u8>>,
}

impl BuildProcess {
    /// Spawns `program` with `args`, exactly `env`, its own process group, stdout and stderr piped, and stdin fed
    /// `input` from a thread (or `/dev/null` when `None`), under a watchdog with `deadline`.
    fn spawn(
        program: &Path,
        args: &[OsString],
        env: &[(OsString, OsString)],
        input: Option<Vec<u8>>,
        deadline: Duration,
    ) -> Result<(ChildStdout, Self), BuilderError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .env_clear()
            .envs(env.iter().map(|(key, value)| (key, value)))
            .current_dir("/")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn().map_err(BuilderError::Spawn)?;
        let started = Instant::now();
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            kill_and_reap(&mut child);
            return Err(BuilderError::Spawn(io::Error::other(
                "the builder's pipes were not created",
            )));
        };
        let stdin = match (input, child.stdin.take()) {
            (Some(bytes), Some(mut pipe)) => Some(thread::spawn(move || {
                // Dropping the pipe at the end of the thread closes it, which ends the context.
                pipe.write_all(&bytes)
            })),
            _ => None,
        };
        let stderr = thread::spawn(move || drain_capped(stderr, BUILDER_STDERR_CAP));
        let (keep, stop) = mpsc::channel();
        let watchdog = thread::spawn(move || watch(child, started, deadline, &stop));
        Ok((
            stdout,
            Self {
                keep,
                watchdog,
                stdin,
                stderr,
            },
        ))
    }

    /// Waits for the watchdog's verdict, then classifies it with the stderr and stdin threads' results.
    fn finish(self) -> Result<ExitStatus, BuilderError> {
        let Self {
            keep,
            watchdog,
            stdin,
            stderr,
        } = self;
        let verdict = watchdog.join().unwrap_or_else(|_| {
            Err(BuilderError::Wait(io::Error::other(
                "the watchdog panicked",
            )))
        });
        drop(keep);
        let stderr = stderr.join().unwrap_or_default();
        let written = stdin.map_or(Ok(()), |writer| {
            writer
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("the context writer panicked")))
        });
        let status = verdict?;
        if !status.success() {
            return Err(BuilderError::Failed {
                code: status.code(),
                stderr: excerpt(&stderr),
            });
        }
        written.map_err(BuilderError::Context)?;
        Ok(status)
    }
}

/// Owns `child` until it exits: kills its process group at `deadline` after `started`, or as soon as `stop`'s
/// sender is dropped, and reaps it in every case.
fn watch(
    mut child: Child,
    started: Instant,
    deadline: Duration,
    stop: &Receiver<()>,
) -> Result<ExitStatus, BuilderError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                kill_and_reap(&mut child);
                return Err(BuilderError::Wait(error));
            }
        }
        if started.elapsed() >= deadline {
            kill_and_reap(&mut child);
            return Err(BuilderError::TimedOut);
        }
        match stop.recv_timeout(POLL) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                kill_and_reap(&mut child);
                return Err(BuilderError::Wait(io::Error::other(
                    "the build was abandoned and killed",
                )));
            }
        }
    }
}

/// Sends SIGKILL to the unreaped `child`'s process group, which it leads, and to the child, then reaps it.
fn kill_and_reap(child: &mut Child) {
    kill_process_group(Pid::from_child(child), Signal::KILL).ok();
    child.kill().ok();
    child.wait().ok();
}

/// Reads up to `cap` bytes from `pipe`, then drains and drops the rest so the writer never blocks on it.
fn drain_capped(pipe: ChildStderr, cap: u64) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut pipe = pipe;
    if pipe.by_ref().take(cap).read_to_end(&mut kept).is_ok() {
        io::copy(&mut pipe, &mut io::sink()).ok();
    }
    kept
}

/// The end of a builder's stderr, where the error is, sanitized on one line and at most 2 KiB.
fn excerpt(bytes: &[u8]) -> SafeText {
    const SHOWN: usize = 2048;
    let start = bytes.len().saturating_sub(SHOWN);
    let tail = bytes.get(start..).unwrap_or(bytes);
    let text = String::from_utf8_lossy(tail);
    let text = text.trim();
    SafeText::inline(if text.is_empty() { "no stderr" } else { text })
}

/// Docker's buildx plugin as the builder (RI-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerBuildx {
    docker: PathBuf,
    search: Vec<PathBuf>,
    home: Option<OsString>,
}

impl DockerBuildx {
    /// The first executable `docker` in an absolute directory of the user's `PATH`, by its canonical path.
    ///
    /// # Errors
    ///
    /// [`BuilderError::NotFound`] when no `PATH` directory holds one.
    pub fn find() -> Result<Self, BuilderError> {
        let path = env::var_os("PATH").ok_or(BuilderError::NotFound)?;
        env::split_paths(&path)
            .find_map(|dir| Self::in_dir(&dir))
            .ok_or(BuilderError::NotFound)
    }

    /// The executable `docker` in `dir`, which must be absolute, with `HOME` taken from this process's
    /// environment, or `None` when there is none.
    #[must_use]
    pub fn in_dir(dir: &Path) -> Option<Self> {
        if !dir.is_absolute() {
            return None;
        }
        let candidate = dir.join("docker");
        let meta = fs::metadata(&candidate).ok()?;
        if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
            return None;
        }
        let docker = fs::canonicalize(&candidate).ok()?;
        let mut search = vec![dir.to_path_buf()];
        if let Some(parent) = docker.parent()
            && parent != dir
        {
            search.push(parent.to_path_buf());
        }
        search.extend(SYSTEM_DIRS.iter().map(PathBuf::from));
        Some(Self {
            docker,
            search,
            home: env::var_os("HOME"),
        })
    }

    /// The canonical docker binary.
    #[must_use]
    pub fn docker(&self) -> &Path {
        &self.docker
    }

    /// The build's argv, program first: build `dockerfile` for `linux/arm64` from the context on stdin, with
    /// attestations off, exporting the rootfs as a tar on stdout.
    #[must_use]
    pub fn build_argv(&self, dockerfile: &RelPath) -> Vec<OsString> {
        let mut argv = vec![self.docker.clone().into_os_string()];
        argv.extend(
            [
                "buildx",
                "build",
                "--platform",
                "linux/arm64",
                "--provenance=false",
                "--sbom=false",
                "--output",
                "type=tar,dest=-",
                "-f",
                dockerfile.as_str(),
                "-",
            ]
            .map(OsString::from),
        );
        argv
    }

    /// The builder's whole environment: `HOME` when this process has one, and a `PATH` of the docker directory,
    /// its canonical directory, and the system directories.
    #[must_use]
    pub fn child_env(&self) -> Vec<(OsString, OsString)> {
        let mut vars = Vec::with_capacity(2);
        if let Some(home) = &self.home {
            vars.push((OsString::from("HOME"), home.clone()));
        }
        let path = env::join_paths(&self.search).unwrap_or_default();
        vars.push((OsString::from("PATH"), path));
        vars
    }

    /// Runs the docker binary with `args` after the program name.
    fn spawn(
        &self,
        args: &[OsString],
        input: Option<Vec<u8>>,
        deadline: Duration,
    ) -> Result<(ChildStdout, BuildProcess), BuilderError> {
        BuildProcess::spawn(&self.docker, args, &self.child_env(), input, deadline)
    }
}

impl RootfsBuilder for DockerBuildx {
    type Rootfs = ChildStdout;

    fn identity(&self) -> Result<BuilderId, BuilderError> {
        let args = [OsString::from("buildx"), OsString::from("version")];
        let (stdout, process) = self.spawn(&args, None, IDENTITY_DEADLINE)?;
        let mut text = Vec::new();
        let mut stdout = stdout;
        let read = stdout
            .by_ref()
            .take(IDENTITY_STDOUT_CAP.saturating_add(1))
            .read_to_end(&mut text);
        drop(stdout);
        process.finish()?;
        read.map_err(|_| BuilderError::Identity)?;
        if u64::try_from(text.len()).map_or(true, |len| len > IDENTITY_STDOUT_CAP) {
            return Err(BuilderError::Identity);
        }
        let text = String::from_utf8(text).map_err(|_| BuilderError::Identity)?;
        parse_buildx_version(&text)
    }

    fn start(
        &self,
        context: Vec<u8>,
        dockerfile: &RelPath,
    ) -> Result<RunningBuild<ChildStdout>, BuilderError> {
        let argv = self.build_argv(dockerfile);
        let args = argv.get(1..).unwrap_or_default();
        let (stdout, process) = self.spawn(args, Some(context), BUILD_DEADLINE)?;
        Ok(RunningBuild {
            rootfs: stdout,
            wait: BuildWait::Process(Box::new(process)),
        })
    }
}

/// Parses `docker buildx version` output (`github.com/docker/buildx v0.35.0-desktop.2 <commit>`) into the
/// builder identity `docker-buildx` and the version token: the first word that is `v` followed by a digit.
///
/// # Errors
///
/// [`BuilderError::Identity`] when the text, apart from one trailing newline, holds a control character or has no
/// version token, or the token is not a valid [`BuilderText`].
pub fn parse_buildx_version(text: &str) -> Result<BuilderId, BuilderError> {
    let line = text.strip_suffix('\n').unwrap_or(text);
    if line.chars().any(char::is_control) {
        return Err(BuilderError::Identity);
    }
    let version = line
        .split(' ')
        .find(|word| {
            word.strip_prefix('v')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .ok_or(BuilderError::Identity)?;
    let name =
        BuilderText::try_from(DOCKER_BUILDX_NAME.to_owned()).map_err(|_| BuilderError::Identity)?;
    let version = BuilderText::try_from(version.to_owned()).map_err(|_| BuilderError::Identity)?;
    Ok(BuilderId::new(name, version))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use minos_core::path::RelPath;

    use super::{BuilderError, DockerBuildx, RootfsBuilder, parse_buildx_version};

    type TestResult = Result<(), Box<dyn Error>>;

    /// A temporary directory holding an executable file named `docker`.
    fn docker_dir() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let canonical = fs::canonicalize(dir.path())?;
        let docker = canonical.join("docker");
        fs::write(&docker, b"#!/bin/sh\nexit 0\n")?;
        fs::set_permissions(&docker, fs::Permissions::from_mode(0o755))?;
        Ok((dir, canonical))
    }

    #[test]
    fn builder_docker_argv() -> TestResult {
        let (_dir, canonical) = docker_dir()?;
        let buildx = DockerBuildx::in_dir(&canonical).ok_or("no docker found")?;
        let argv = buildx.build_argv(&RelPath::try_from("minos.Dockerfile")?);
        let docker = canonical.join("docker").into_os_string();
        let expected: Vec<OsString> = [
            docker.as_os_str(),
            "buildx".as_ref(),
            "build".as_ref(),
            "--platform".as_ref(),
            "linux/arm64".as_ref(),
            "--provenance=false".as_ref(),
            "--sbom=false".as_ref(),
            "--output".as_ref(),
            "type=tar,dest=-".as_ref(),
            "-f".as_ref(),
            "minos.Dockerfile".as_ref(),
            "-".as_ref(),
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert_eq!(argv, expected);
        Ok(())
    }

    #[test]
    fn builder_docker_env() -> TestResult {
        let (_dir, canonical) = docker_dir()?;
        // The binary is reached through a symlink in a second directory, so the PATH holds both directories.
        let link_dir = tempfile::tempdir()?;
        let link_dir = fs::canonicalize(link_dir.path())?;
        std::os::unix::fs::symlink(canonical.join("docker"), link_dir.join("docker"))?;
        let buildx = DockerBuildx::in_dir(&link_dir).ok_or("no docker found")?;
        assert_eq!(buildx.docker(), canonical.join("docker"));

        let env = buildx.child_env();
        let keys: Vec<&str> = env.iter().filter_map(|(key, _)| key.to_str()).collect();
        let expected_keys: &[&str] = if std::env::var_os("HOME").is_some() {
            &["HOME", "PATH"]
        } else {
            &["PATH"]
        };
        assert_eq!(keys, expected_keys);
        let path = env
            .iter()
            .find(|(key, _)| key == "PATH")
            .map(|(_, value)| value.clone())
            .ok_or("no PATH")?;
        let expected = format!(
            "{}:{}:/usr/bin:/bin:/usr/sbin:/sbin",
            link_dir.display(),
            canonical.display()
        );
        assert_eq!(path, OsString::from(expected));
        Ok(())
    }

    #[test]
    fn builder_relative_or_empty_dir_is_not_a_docker() -> TestResult {
        assert!(DockerBuildx::in_dir(std::path::Path::new("bin")).is_none());
        let empty = tempfile::tempdir()?;
        assert!(DockerBuildx::in_dir(empty.path()).is_none());
        Ok(())
    }

    /// A directory holding a `docker` that runs `script` under `/bin/sh`.
    fn scripted_docker(script: &str) -> Result<(tempfile::TempDir, DockerBuildx), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let canonical = fs::canonicalize(dir.path())?;
        let docker = canonical.join("docker");
        fs::write(&docker, format!("#!/bin/sh\n{script}\n"))?;
        fs::set_permissions(&docker, fs::Permissions::from_mode(0o755))?;
        let buildx = DockerBuildx::in_dir(&canonical).ok_or("no docker found")?;
        Ok((dir, buildx))
    }

    #[test]
    fn builder_process_streams_the_context_back_and_succeeds() -> TestResult {
        // The script echoes its stdin, the context, to stdout, where the rootfs tar would be.
        let (_dir, buildx) = scripted_docker("exec /bin/cat")?;
        let mut build = buildx.start(
            b"context bytes".to_vec(),
            &RelPath::try_from("minos.Dockerfile")?,
        )?;
        let mut out = Vec::new();
        std::io::Read::read_to_end(build.rootfs_mut(), &mut out)?;
        build.finish()?;
        assert_eq!(out, b"context bytes");
        Ok(())
    }

    #[test]
    fn builder_process_failure_carries_code_and_sanitized_stderr() -> TestResult {
        let (_dir, buildx) =
            scripted_docker("/bin/cat >/dev/null; printf 'boom\\033[31m\\n' >&2; exit 3")?;
        let mut build = buildx.start(Vec::new(), &RelPath::try_from("minos.Dockerfile")?)?;
        let mut out = Vec::new();
        std::io::Read::read_to_end(build.rootfs_mut(), &mut out)?;
        match build.finish() {
            Err(BuilderError::Failed { code, stderr }) => {
                assert_eq!(code, Some(3));
                assert!(stderr.as_str().starts_with("boom"), "{}", stderr.as_str());
                assert!(!stderr.as_str().contains('\u{1b}'));
            }
            other => return Err(format!("expected Failed, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn builder_identity_runs_buildx_version() -> TestResult {
        let (_dir, buildx) = scripted_docker(
            "[ \"$1 $2\" = 'buildx version' ] || exit 9; echo 'github.com/docker/buildx v0.35.0 abc'",
        )?;
        let id = buildx.identity()?;
        assert_eq!(id.name().as_str(), "docker-buildx");
        assert_eq!(id.version().as_str(), "v0.35.0");
        Ok(())
    }

    #[test]
    fn builder_buildx_version_parse() -> TestResult {
        let id = parse_buildx_version("github.com/docker/buildx v0.35.0-desktop.2 abc123\n")?;
        assert_eq!(id.name().as_str(), "docker-buildx");
        assert_eq!(id.version().as_str(), "v0.35.0-desktop.2");

        for refused in [
            "github.com/docker/buildx v0.35.0\u{1b}[31m abc123\n",
            "github.com/docker/buildx 0.35.0 abc123\n",
            "",
            "github.com/docker/buildx v0.35.0\nsecond line\n",
        ] {
            let result = parse_buildx_version(refused);
            assert!(
                matches!(result, Err(BuilderError::Identity)),
                "{refused:?}: {result:?}"
            );
        }
        Ok(())
    }
}
