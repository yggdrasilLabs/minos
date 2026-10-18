//! Watchdog: stops the VM from outside when it runs too long or loses its parent.
//!
//! The thread polls every 50 ms. On a trip it stores the trip's exit code through `Vm::exit_code`, which the VMM
//! prefers over the vCPU exit code, and triggers the `ExitHandle`, so the process exits with that code and
//! `on_exit` prints it (D-05, K7). The parent's own deadline (02-05) stays authoritative; this is the inner one.
//!
//! There is no console-cap trip and the helper no longer exits 125: the console is the helper's stdout pipe, and
//! the supervisor enforces its cap exactly on its side of the pipe, reading through cap + 1 bytes and killing the
//! helper on overflow (05-EVIDENCE.md W0-CONSOLE; 05-RESEARCH Pattern 7).

use std::io;
use std::os::unix::process::parent_id;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::VmmConfig;

/// How often the watchdog checks its limits.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Why the watchdog stopped the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trip {
    /// `max_duration_ms` elapsed since `main` entry.
    Deadline,
    /// The parent process is not the one recorded at `main` entry, or is PID 1: whoever started the launcher is
    /// gone.
    ParentGone,
}

impl Trip {
    /// Exit code the VMM reports for this trip (E-3).
    fn code(self) -> i32 {
        match self {
            Self::Deadline => 124,
            Self::ParentGone => 126,
        }
    }

    /// Word printed after `minos-vmm: watchdog`.
    fn reason(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::ParentGone => "parent-gone",
        }
    }
}

/// The limits the watchdog enforces, taken from the launch config.
pub struct Limits {
    deadline: Duration,
}

impl Limits {
    /// The deadline of `config`.
    pub fn from_config(config: &VmmConfig) -> Self {
        Self::new(config.max_duration())
    }

    /// Limits for a deadline.
    fn new(deadline: Duration) -> Self {
        Self { deadline }
    }

    /// The first limit broken at this moment, checked in order: deadline, then parent.
    fn tripped(&self, start: Instant, parent: u32) -> Option<Trip> {
        if start.elapsed() >= self.deadline {
            Some(Trip::Deadline)
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
    use std::time::{Duration, Instant};

    use super::{LAUNCHD_PID, Limits, Trip, parent_gone};

    #[test]
    fn watchdog_codes_match_e3() {
        assert_eq!(Trip::Deadline.code(), 124);
        assert_eq!(Trip::ParentGone.code(), 126);
        assert_eq!(Trip::Deadline.reason(), "deadline");
        assert_eq!(Trip::ParentGone.reason(), "parent-gone");
    }

    #[test]
    fn watchdog_trips_on_deadline() {
        let tripped = Limits::new(Duration::ZERO).tripped(Instant::now(), parent_id());
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
        let tripped = Limits::new(Duration::from_secs(1)).tripped(started, parent_id());
        assert_eq!(tripped, Some(Trip::Deadline));
        let tripped = Limits::new(Duration::from_hours(1)).tripped(started, parent_id());
        assert_eq!(tripped, None);
        Ok(())
    }

    #[test]
    fn watchdog_does_not_trip_within_its_limits() {
        let tripped = Limits::new(Duration::from_hours(1)).tripped(Instant::now(), parent_id());
        assert_eq!(tripped, None);
    }

    #[test]
    fn watchdog_trips_when_the_parent_changes() {
        let other_parent = parent_id().wrapping_add(1);
        let tripped = Limits::new(Duration::from_hours(1)).tripped(Instant::now(), other_parent);
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
}
