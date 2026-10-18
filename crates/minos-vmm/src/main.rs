//! Per-VM launcher: the only binary that links the VMM crate.
//!
//! `minos-vmm <config.json>` sets its own resource limits, installs its capped stderr logger, reads a launch
//! config (02-CONTEXT.md D-04; v2, `golden/config-v2.json`), checks the environment, its stdout, and the host
//! files the config names, builds the VM with `msb_krun`, starts a watchdog, and hands the process to the VMM,
//! which exits it when the guest stops. stdin is never read; the parent gives the helper `/dev/null`, its stdout
//! is the console pipe, and its stderr is a second pipe (KRUN-01).
//!
//! Exit codes (02-03-PLAN.md E-3, as amended by 05-07):
//!
//! | Code | Meaning |
//! |---|---|
//! | 0 | the VMM exited on its own (guest power-off, guest reboot or panic, SIGTERM) |
//! | 64 | not exactly one argument |
//! | 65 | config unreadable, too large, not JSON, an unknown field, or a failed rule |
//! | 66 | setting its own rlimits or installing its logger; preflight: a forbidden environment variable, stdout not a pipe, or a named file missing, not canonical, or the wrong type |
//! | 67 | libkrunfw SHA-256 does not match the config |
//! | 70 | `msb_krun` refused to build or start the VM before takeover |
//! | 71 | the watchdog thread could not be started (a host thread or memory limit), so the VM was never entered |
//! | 124, 126 | watchdog: deadline, parent gone |
//!
//! The helper no longer exits 125: the console is its stdout pipe, and the supervisor enforces the console cap on
//! its side of the pipe and kills the helper on overflow (05-EVIDENCE.md W0-CONSOLE).
//!
//! A guest exit code is never visible here: with no virtio-fs device the guest has no way to set one, so the
//! status of a run comes from its vsock status frame, not from this process (02-RESEARCH.md Pattern 3).

mod config;
mod launch;
mod log;
mod preflight;
mod watchdog;

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use rustix::process::{Resource, Rlimit};

use crate::config::{ConfigError, MAX_CONFIG_BYTES, VmmConfig};
use crate::preflight::PreflightError;

/// Not exactly one argument.
const EXIT_USAGE: u8 = 64;
/// The config could not be read or failed validation.
const EXIT_CONFIG: u8 = 65;
/// The helper could not confine itself, or a preflight check failed.
const EXIT_PREFLIGHT: u8 = 66;
/// The libkrunfw digest did not match.
const EXIT_HASH: u8 = 67;
/// `msb_krun` refused to build or start the VM before takeover.
const EXIT_VMM: u8 = 70;
/// The watchdog thread could not be started, so the VM was never entered.
const EXIT_WATCHDOG: u8 = 71;

/// The helper's open-file limit. Lowered to the inherited hard limit if that is smaller, because an unprivileged
/// process cannot raise its hard limit.
///
/// The VMM's two disks, its threads, and its pipes need a few dozen descriptors, but the guest's chunked output
/// needs far more: the pinned `msb_krun_devices` 0.1.41 vsock reaper keeps every released connection's proxy,
/// and with it the host socket, for 5 s after the connection closes (`virtio/vsock/reaper.rs`, `TIMEOUT`), and a
/// full output stream is about 4353 connections of one chunk each (`MAX_OUTPUT_STREAM_BYTES` over
/// `MAX_CHUNK_PAYLOAD`, plus the END). At the earlier 1024, every thousand or so connections exhausted the
/// table, PID 1's connects were reset until the reaper freed them, and a 16 MiB output took 20.5 s instead of
/// 0.9 s (05-EVIDENCE.md "05-17 helper `RLIMIT_NOFILE` and the vsock reaper"). 8192 holds a whole stream inside one
/// reaper window with room to spare, and still bounds the helper's descriptors.
const NOFILE_LIMIT: u64 = 8192;

/// The usage line printed for exit code 64.
const USAGE: &str = "minos-vmm: usage: minos-vmm <config.json>";

/// Every way the launcher fails before the VMM takes over the process.
#[derive(Debug, thiserror::Error)]
enum LaunchError {
    /// Not exactly one argument.
    #[error("expected exactly one argument, the launch config path")]
    Usage,
    /// The config file could not be opened or read.
    #[error("reading launch config {}", .path.display())]
    Read {
        /// The path from argv.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The config failed to parse or validate.
    #[error("launch config {}", .path.display())]
    Config {
        /// The path from argv.
        path: PathBuf,
        /// What was wrong with it.
        #[source]
        source: ConfigError,
    },
    /// The helper could not set its own rlimits, or a preflight check failed.
    #[error("preflight")]
    Preflight(#[source] PreflightError),
    /// The stderr logger could not be installed. `log` builds `SetLoggerError` without its `Error` impl here
    /// (the `std` feature is off), so it is shown, not chained.
    #[error("installing the stderr logger: {0}")]
    Logger(::log::SetLoggerError),
    /// `msb_krun` refused to build the VM.
    #[error("building the VM")]
    Build(#[source] msb_krun::Error),
    /// The watchdog thread could not be started.
    #[error("starting the watchdog")]
    Watchdog(#[source] io::Error),
    /// `msb_krun` failed to start the VM before takeover.
    #[error("starting the VM")]
    Enter(#[source] msb_krun::Error),
}

impl LaunchError {
    /// The documented exit code for this failure (E-3).
    fn exit_code(&self) -> u8 {
        match self {
            Self::Usage => EXIT_USAGE,
            Self::Read { .. } | Self::Config { .. } => EXIT_CONFIG,
            Self::Preflight(PreflightError::Hash { .. }) => EXIT_HASH,
            Self::Preflight(_) | Self::Logger(_) => EXIT_PREFLIGHT,
            Self::Build(_) | Self::Enter(_) => EXIT_VMM,
            Self::Watchdog(_) => EXIT_WATCHDOG,
        }
    }
}

fn main() -> ExitCode {
    // First statements, before argument parsing, the config read, preflight, or the VM build: a parent that exits
    // during any of those must still trip the watchdog (DEFECT-01), and `max_duration_ms` counts from here, so it
    // bounds the helper's whole lifetime rather than the VM's run time alone (IN-04).
    let parent = std::os::unix::process::parent_id();
    let started = Instant::now();
    match run(parent, started) {
        Ok(never) => match never {},
        Err(LaunchError::Usage) => {
            say(format_args!("{USAGE}"));
            ExitCode::from(EXIT_USAGE)
        }
        Err(error) => {
            say(format_args!("minos-vmm: error: {}", chain(&error)));
            ExitCode::from(error.exit_code())
        }
    }
}

/// Everything up to and including `enter`, which never returns on success. `parent` and `started` are the parent
/// PID and the instant recorded at `main` entry.
///
/// The helper confines itself first, before it reads its arguments or its config: its own rlimits, then the
/// logger, so `msb_krun`'s warnings reach stderr capped and sanitized from the first VMM call on.
fn run(parent: u32, started: Instant) -> Result<Infallible, LaunchError> {
    apply_rlimits().map_err(LaunchError::Preflight)?;
    crate::log::install().map_err(LaunchError::Logger)?;
    let path = config_path()?;
    let config = read_config(path)?;
    preflight::check(&config).map_err(LaunchError::Preflight)?;
    let vm = launch::build(&config).map_err(LaunchError::Build)?;
    // Both handles must be taken before `enter`, which consumes the VM.
    let exit_handle = vm.exit_handle();
    let exit_code = vm.exit_code();
    watchdog::spawn(
        exit_handle,
        exit_code,
        watchdog::Limits::from_config(&config),
        parent,
        started,
    )
    .map_err(LaunchError::Watchdog)?;
    vm.enter().map_err(LaunchError::Enter)
}

/// Sets the helper's own resource limits, each with its current and maximum value equal, so nothing it runs can
/// raise them again (D-01, the host half of `ConfinementLayer::Rlimits`; the guest half is the trampoline's
/// per-stage limits on the candidate):
///
/// - `RLIMIT_CORE` 0: a crash leaves no core file of the entitled process on the host.
/// - `RLIMIT_NOFILE` [`NOFILE_LIMIT`], or the inherited hard limit if smaller.
/// - `RLIMIT_FSIZE` 0: the helper writes no regular file. Its console and stderr are pipes, which the limit does
///   not cover, and `msb_krun` creates no file of its own (05-EVIDENCE.md W0-CONSOLE, assumption A4). A write to
///   a regular file would raise `SIGXFSZ` and kill the helper, which the supervisor classifies as an
///   infrastructure failure, so the limit fails closed.
///
/// # Errors
///
/// [`PreflightError::Rlimit`] naming the first limit that could not be set.
fn apply_rlimits() -> Result<(), PreflightError> {
    let nofile = rustix::process::getrlimit(Resource::Nofile)
        .maximum
        .map_or(NOFILE_LIMIT, |hard| hard.min(NOFILE_LIMIT));
    for (resource, name, value) in [
        (Resource::Core, "RLIMIT_CORE", 0),
        (Resource::Nofile, "RLIMIT_NOFILE", nofile),
        (Resource::Fsize, "RLIMIT_FSIZE", 0),
    ] {
        let limit = Rlimit {
            current: Some(value),
            maximum: Some(value),
        };
        rustix::process::setrlimit(resource, limit).map_err(|errno| PreflightError::Rlimit {
            resource: name,
            source: io::Error::from(errno),
        })?;
    }
    Ok(())
}

/// The only argument: the launch config path.
fn config_path() -> Result<PathBuf, LaunchError> {
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next()) {
        (Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err(LaunchError::Usage),
    }
}

/// Reads at most one byte past the cap, so [`config::parse`] can refuse an oversized file without reading it all.
fn read_config(path: PathBuf) -> Result<VmmConfig, LaunchError> {
    let mut bytes = Vec::new();
    let read =
        File::open(&path).and_then(|file| file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes));
    if let Err(source) = read {
        return Err(LaunchError::Read { path, source });
    }
    config::parse(&bytes).map_err(|source| LaunchError::Config { path, source })
}

/// The error and its sources joined by `: `. A source whose text already ends the line is skipped, because some
/// errors (for example `msb_krun::Error::Io`) print their source in their own message as well.
fn chain(error: &dyn Error) -> String {
    let mut line = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !line.ends_with(&text) {
            line.push_str(": ");
            line.push_str(&text);
        }
        source = cause.source();
    }
    line
}

/// Writes one line to stderr. A stderr write error has nowhere to be reported, so it is dropped rather than
/// allowed to panic the way `eprintln!` would.
fn say(line: fmt::Arguments<'_>) {
    let mut stderr = io::stderr().lock();
    let written = writeln!(stderr, "{line}").and_then(|()| stderr.flush());
    drop(written);
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::{EXIT_CONFIG, EXIT_HASH, EXIT_PREFLIGHT, EXIT_USAGE, EXIT_WATCHDOG, LaunchError};
    use crate::config::ConfigError;
    use crate::preflight::PreflightError;

    /// Every pre-takeover failure the launcher can build without `msb_krun` maps to its documented code, and
    /// the watchdog spawn failure has its own, distinct from the 70 reserved for `msb_krun`.
    #[test]
    fn launch_error_codes_match_the_table() {
        assert_eq!(LaunchError::Usage.exit_code(), EXIT_USAGE);
        assert_eq!(
            LaunchError::Read {
                path: PathBuf::from("/private/tmp/mns00000000/vmm.json"),
                source: io::Error::from(io::ErrorKind::NotFound),
            }
            .exit_code(),
            EXIT_CONFIG
        );
        assert_eq!(
            LaunchError::Config {
                path: PathBuf::from("/private/tmp/mns00000000/vmm.json"),
                source: ConfigError::TooLarge,
            }
            .exit_code(),
            EXIT_CONFIG
        );
        assert_eq!(
            LaunchError::Preflight(PreflightError::Hash {
                field: "kernel.krunfw",
                path: PathBuf::from("/opt/homebrew/lib/libkrunfw.5.dylib"),
                expected: "00".repeat(32),
                actual: "ff".repeat(32),
            })
            .exit_code(),
            EXIT_HASH
        );
        assert_eq!(
            LaunchError::Preflight(PreflightError::ConsoleNotPipe {
                found: "regular file"
            })
            .exit_code(),
            EXIT_PREFLIGHT
        );
        assert_eq!(
            LaunchError::Preflight(PreflightError::Rlimit {
                resource: "RLIMIT_FSIZE",
                source: io::Error::from(io::ErrorKind::PermissionDenied),
            })
            .exit_code(),
            EXIT_PREFLIGHT
        );
        assert_eq!(EXIT_PREFLIGHT, 66);
        let watchdog = LaunchError::Watchdog(io::Error::from(io::ErrorKind::WouldBlock));
        assert_eq!(watchdog.exit_code(), EXIT_WATCHDOG);
        assert_eq!(EXIT_WATCHDOG, 71);
        assert_ne!(EXIT_WATCHDOG, super::EXIT_VMM);
    }
}
