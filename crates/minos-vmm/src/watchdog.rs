//! Watchdog: stops the VM from outside when it runs too long, floods the console, or loses its parent.
//!
//! The thread polls every 50 ms. On a trip it stores the trip's exit code through `Vm::exit_code`, which the VMM
//! prefers over the vCPU exit code, and triggers the `ExitHandle`, so the process exits with that code and
//! `on_exit` prints it (D-05, K7). The parent's own deadline (02-05) stays authoritative; this is the inner one.

use std::fs;
use std::io;
use std::os::unix::process::parent_id;
use std::path::PathBuf;
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
    /// `max_duration_ms` elapsed.
    Deadline,
    /// The console file passed `console.cap_bytes`.
    ConsoleCap,
    /// The parent process changed: the launcher was reparented, so whoever started it is gone.
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
    console: PathBuf,
    console_cap_bytes: u64,
}

impl Limits {
    /// The deadline, console path, and console cap of `config`.
    pub fn from_config(config: &VmmConfig) -> Self {
        Self {
            deadline: config.max_duration(),
            console: config.console().to_path_buf(),
            console_cap_bytes: config.console_cap_bytes(),
        }
    }

    /// The first limit broken at this moment, checked in order: deadline, console cap, parent.
    fn tripped(&self, start: Instant, parent: u32) -> Option<Trip> {
        if start.elapsed() >= self.deadline {
            Some(Trip::Deadline)
        } else if console_len(&self.console) > self.console_cap_bytes {
            Some(Trip::ConsoleCap)
        } else if parent_id() != parent {
            Some(Trip::ParentGone)
        } else {
            None
        }
    }
}

/// Length of the console file; a file that does not exist yet counts as empty.
fn console_len(path: &std::path::Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

/// Starts the watchdog thread. The clock and the parent id are read now, just before `enter`.
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
) -> io::Result<()> {
    let start = Instant::now();
    let parent = parent_id();
    thread::Builder::new()
        .name("minos-vmm-watchdog".to_owned())
        .spawn(move || {
            let trip = loop {
                thread::sleep(POLL_INTERVAL);
                if let Some(trip) = limits.tripped(start, parent) {
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
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::{Limits, Trip};

    fn limits(deadline: Duration, console_cap_bytes: u64) -> Limits {
        Limits {
            deadline,
            console: PathBuf::from("/private/tmp/mns00000000/console.log"),
            console_cap_bytes,
        }
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
}
