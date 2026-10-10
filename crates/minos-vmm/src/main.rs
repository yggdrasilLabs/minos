//! Per-VM launcher: the only binary that links the VMM crate.
//!
//! `minos-vmm <config.json>` reads a launch config (02-CONTEXT.md D-04), checks the environment and the host files
//! it names, builds the VM with `msb_krun`, starts a watchdog, and hands the process to the VMM, which exits it
//! when the guest stops. stdin is never read; the parent gives the helper `/dev/null` (KRUN-01).
//!
//! Exit codes (02-03-PLAN.md E-3):
//!
//! | Code | Meaning |
//! |---|---|
//! | 0 | the VMM exited on its own (guest power-off, guest reboot or panic, SIGTERM) |
//! | 64 | not exactly one argument |
//! | 65 | config unreadable, too large, not JSON, an unknown field, or a failed rule |
//! | 66 | preflight: a named file missing, not canonical, or the wrong type; scratch over its cap; console path exists; a forbidden environment variable |
//! | 67 | libkrunfw or initramfs SHA-256 does not match the config |
//! | 70 | `msb_krun` refused to build or start the VM before takeover |
//! | 124, 125, 126 | watchdog: deadline, console cap, parent gone |
//!
//! A guest exit code is never visible here: with no virtio-fs device the guest has no way to set one, so the
//! status of a run comes from its vsock status frame, not from this process (02-RESEARCH.md Pattern 3).

mod config;
mod launch;
mod preflight;
mod watchdog;

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::config::{ConfigError, MAX_CONFIG_BYTES, VmmConfig};
use crate::preflight::PreflightError;

/// Not exactly one argument.
const EXIT_USAGE: u8 = 64;
/// The config could not be read or failed validation.
const EXIT_CONFIG: u8 = 65;
/// A preflight check failed.
const EXIT_PREFLIGHT: u8 = 66;
/// A libkrunfw or initramfs digest did not match.
const EXIT_HASH: u8 = 67;
/// `msb_krun` refused to build or start the VM before takeover.
const EXIT_VMM: u8 = 70;

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
    /// A preflight check failed.
    #[error("preflight")]
    Preflight(#[source] PreflightError),
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
            Self::Preflight(_) => EXIT_PREFLIGHT,
            Self::Build(_) | Self::Watchdog(_) | Self::Enter(_) => EXIT_VMM,
        }
    }
}

fn main() -> ExitCode {
    // First statement, before argument parsing, the config read, preflight, or the VM build: a parent that exits
    // during any of those must still trip the watchdog (DEFECT-01).
    let parent = std::os::unix::process::parent_id();
    match run(parent) {
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

/// Everything up to and including `enter`, which never returns on success. `parent` is the parent PID recorded at
/// `main` entry.
fn run(parent: u32) -> Result<Infallible, LaunchError> {
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
    )
    .map_err(LaunchError::Watchdog)?;
    vm.enter().map_err(LaunchError::Enter)
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
