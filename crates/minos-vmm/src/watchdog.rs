//! Watchdog: stops the VM from outside when it runs too long, floods the console, or loses its parent.
//!
//! The thread polls every 50 ms. On a trip it stores the trip's exit code through `Vm::exit_code`, which the VMM
//! prefers over the vCPU exit code, and triggers the `ExitHandle`, so the process exits with that code and
//! `on_exit` prints it (D-05, K7). The parent's own deadline (02-05) stays authoritative; this is the inner one.

use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::parent_id;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{STDOUT_CONSOLE, VmmConfig};

/// How often the watchdog checks its limits.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Why the watchdog stopped the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trip {
    /// `max_duration_ms` elapsed since `main` entry.
    Deadline,
    /// The console file passed `console.cap_bytes`. Never for a console on the helper's stdout.
    ConsoleCap,
    /// The parent process is not the one recorded at `main` entry, or is PID 1: whoever started the launcher is
    /// gone.
    ParentGone,
}

impl Trip {
    /// Exit code the VMM reports for this trip (E-3).
    fn code(self) -> i32 {
        match self {
            Self::Deadline => 124,
            Self::ConsoleCap => 125,
            Self::ParentGone => 126,
        }
    }

    /// Word printed after `minos-vmm: watchdog`.
    fn reason(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::ConsoleCap => "console-cap",
            Self::ParentGone => "parent-gone",
        }
    }
}

/// The limits the watchdog enforces, taken from the launch config.
pub struct Limits {
    deadline: Duration,
    /// The console file to measure; `None` when the console is the helper's stdout pipe, whose cap the parent
    /// enforces on its side of the pipe (KRUN-01, 05-RESEARCH Pattern 7).
    console: Option<PathBuf>,
    console_cap_bytes: u64,
}

impl Limits {
    /// The deadline, console path, and console cap of `config`.
    pub fn from_config(config: &VmmConfig) -> Self {
        Self::new(
            config.max_duration(),
            config.console(),
            config.console_cap_bytes(),
        )
    }

    /// Limits for a deadline, a console path, and a console cap. A console path equal to [`STDOUT_CONSOLE`] byte
    /// for byte is never measured, so the console cap never trips for it; the deadline and the parent check do.
    fn new(deadline: Duration, console: &Path, console_cap_bytes: u64) -> Self {
        let is_stdout = console.as_os_str().as_bytes() == STDOUT_CONSOLE.as_bytes();
        Self {
            deadline,
            console: (!is_stdout).then(|| console.to_path_buf()),
            console_cap_bytes,
        }
    }

    /// The first limit broken at this moment, checked in order: deadline, console cap, parent.
    fn tripped(&self, start: Instant, parent: u32) -> Option<Trip> {
        if start.elapsed() >= self.deadline {
            Some(Trip::Deadline)
        } else if self
            .console
            .as_deref()
            .is_some_and(|console| console_len(console) > self.console_cap_bytes)
        {
            Some(Trip::ConsoleCap)
        } else if parent_gone(parent, parent_id()) {
            Some(Trip::ParentGone)
        } else {
            None
        }
    }
}

/// The PID an orphaned process is reparented to on macOS (launchd).
const LAUNCHD_PID: u32 = 1;

/// Whether the parent that started the launcher is gone: the current parent differs from the one recorded as the
/// first statement of `main`, or the current parent is launchd. launchd never legitimately starts `minos-vmm`, so a
/// PID 1 parent also covers a parent that exited between `fork` and the launcher's first instruction (DEFECT-01).
fn parent_gone(expected: u32, current: u32) -> bool {
    current != expected || current == LAUNCHD_PID
}

/// Length of the console file; a file that does not exist yet counts as empty.
fn console_len(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

/// Starts the watchdog thread. `parent` and `started` are the parent PID and the instant `main` recorded before
/// doing anything else, so a parent that exits while the config is read, the kernel is hashed, or the VM is built
/// is still caught (DEFECT-01), and the deadline counts that time too: `max_duration_ms` bounds the helper's whole
/// lifetime, not only the VM's run time, so it stays one clock with the parent's host deadline (IN-04).
///
/// The thread is detached: it ends when it trips, or with the process when the VM exits first.
///
/// # Errors
///
/// The thread could not be spawned.
pub fn spawn(
    exit_handle: msb_krun::ExitHandle,
    exit_code: Arc<AtomicI32>,
    limits: Limits,
    parent: u32,
    started: Instant,
) -> io::Result<()> {
    thread::Builder::new()
        .name("minos-vmm-watchdog".to_owned())
        .spawn(move || {
            let trip = loop {
                thread::sleep(POLL_INTERVAL);
                if let Some(trip) = limits.tripped(started, parent) {
                    break trip;
                }
            };
            crate::say(format_args!("minos-vmm: watchdog {}", trip.reason()));
            exit_code.store(trip.code(), Ordering::SeqCst);
            exit_handle.trigger();
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::parent_id;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::{LAUNCHD_PID, Limits, Trip, parent_gone};
    use crate::config::STDOUT_CONSOLE;

    fn limits(deadline: Duration, console_cap_bytes: u64) -> Limits {
        Limits::new(
            deadline,
            Path::new("/private/tmp/mns00000000/console.log"),
            console_cap_bytes,
        )
    }

    #[test]
    fn watchdog_codes_match_e3() {
        assert_eq!(Trip::Deadline.code(), 124);
        assert_eq!(Trip::ConsoleCap.code(), 125);
        assert_eq!(Trip::ParentGone.code(), 126);
    }

    #[test]
    fn watchdog_trips_on_deadline() {
        let tripped = limits(Duration::ZERO, 1).tripped(Instant::now(), parent_id());
        assert_eq!(tripped, Some(Trip::Deadline));
    }

    /// The deadline is measured from the instant `main` recorded, not from when the thread started, so time
    /// already spent before `spawn` counts against it.
    #[test]
    fn watchdog_deadline_counts_from_the_recorded_start() -> Result<(), Box<dyn std::error::Error>>
    {
        let started = Instant::now()
            .checked_sub(Duration::from_secs(2))
            .ok_or("the monotonic clock has not run for two seconds")?;
        let tripped = limits(Duration::from_secs(1), 1).tripped(started, parent_id());
        assert_eq!(tripped, Some(Trip::Deadline));
        let tripped = limits(Duration::from_hours(1), 1).tripped(started, parent_id());
        assert_eq!(tripped, None);
        Ok(())
    }

    #[test]
    fn watchdog_counts_a_missing_console_as_empty() {
        let tripped = limits(Duration::from_hours(1), 1).tripped(Instant::now(), parent_id());
        assert_eq!(tripped, None);
    }

    #[test]
    fn watchdog_trips_when_the_parent_changes() {
        let other_parent = parent_id().wrapping_add(1);
        let tripped = limits(Duration::from_hours(1), 1).tripped(Instant::now(), other_parent);
        assert_eq!(tripped, Some(Trip::ParentGone));
    }

    #[test]
    fn parent_gone_when_the_recorded_parent_differs_from_the_current_one() {
        assert!(parent_gone(4242, 4243));
        assert!(!parent_gone(4242, 4242));
    }

    #[test]
    fn parent_gone_when_the_current_parent_is_launchd() {
        assert!(parent_gone(LAUNCHD_PID, LAUNCHD_PID));
        assert!(parent_gone(4242, LAUNCHD_PID));
    }

    /// A console file longer than its cap trips the console cap.
    #[test]
    fn watchdog_trips_on_console_file_over_cap() -> Result<(), Box<dyn std::error::Error>> {
        let this_binary = std::env::current_exe()?;
        let tripped = Limits::new(Duration::from_hours(1), &this_binary, 1)
            .tripped(Instant::now(), parent_id());
        assert_eq!(tripped, Some(Trip::ConsoleCap));
        Ok(())
    }

    /// With the console on the helper's stdout pipe, the parent enforces the cap, so the watchdog never trips
    /// console-cap; the deadline and parent-gone still trip.
    #[test]
    fn watchdog_ignores_stdout_console_length() {
        let stdout = |deadline| Limits::new(deadline, Path::new(STDOUT_CONSOLE), 1);
        assert_eq!(
            stdout(Duration::from_hours(1)).tripped(Instant::now(), parent_id()),
            None
        );
        assert_eq!(
            stdout(Duration::ZERO).tripped(Instant::now(), parent_id()),
            Some(Trip::Deadline)
        );
        assert_eq!(
            stdout(Duration::from_hours(1)).tripped(Instant::now(), parent_id().wrapping_add(1)),
            Some(Trip::ParentGone)
        );
    }
}
