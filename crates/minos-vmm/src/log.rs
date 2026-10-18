//! The helper's stderr logger (DIAG-01).
//!
//! `msb_krun` and its device crates log through the `log` facade. With no logger installed every message was
//! discarded, including the vsock device's "error reading TX packet" that DIAG-01 needed. This logger forwards
//! warnings and errors to stderr, where the supervisor reads them, under three rules (05-RESEARCH Pitfall 5):
//!
//! - Every line is `minos-vmm: log <level>: ` followed by the message with every character outside printable
//!   ASCII (0x20 to 0x7e) replaced by `?`. A message can carry guest-influenced bytes, but it can never contain a
//!   newline, a carriage return, or an escape sequence, so it is always exactly one line.
//! - The supervisor matches the watchdog markers (`minos-vmm: watchdog deadline`, `minos-vmm: watchdog
//!   parent-gone`) as exact whole lines, and every logged line starts with `minos-vmm: log `, so no logged line
//!   can equal a marker.
//! - At most [`LOG_CAP_BYTES`] bytes of log lines are written, newlines included. The first line that would pass
//!   the cap is replaced by one [`TRUNCATED_LINE`], and nothing is logged after it.
//!
//! Only `Warn` and `Error` records are written. A stderr write error is dropped, never a panic.

use std::fmt::{self, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};

use ::log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

/// Most bytes of log lines the helper writes to stderr, newlines included, before [`TRUNCATED_LINE`].
pub const LOG_CAP_BYTES: u64 = 65_536;
/// The one line written in place of the first line that would pass [`LOG_CAP_BYTES`].
pub const TRUNCATED_LINE: &str = "minos-vmm: log truncated";
/// The prefix of every logged line; no watchdog marker starts with it.
const PREFIX: &str = "minos-vmm: log ";
/// The most verbose level written.
const MAX_LEVEL: Level = Level::Warn;
/// Most message characters rendered. A longer line could never fit under [`LOG_CAP_BYTES`] anyway.
const MAX_MESSAGE_CHARS: usize = 65_536;
/// [`LineBudget`]'s state once [`TRUNCATED_LINE`] has been admitted.
const CLOSED: u64 = u64::MAX;

/// What [`LineBudget::admit`] allows for one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admit {
    /// Write the line.
    Line,
    /// Write [`TRUNCATED_LINE`] instead; returned exactly once.
    Truncated,
    /// Write nothing: the budget is closed.
    Drop,
}

/// One log line for `level` and `message`: the prefix, the level word, and the sanitized message, no newline.
pub fn format_line(level: Level, message: &str) -> String {
    drop((level, message));
    String::new()
}

/// The byte budget shared by every logged line.
pub struct LineBudget {
    used: AtomicU64,
}

impl LineBudget {
    /// An empty budget.
    pub const fn new() -> Self {
        Self {
            used: AtomicU64::new(0),
        }
    }

    /// Whether a line of `len` bytes, its newline included, may be written.
    pub fn admit(&self, len: u64) -> Admit {
        drop((len, self.used.load(Ordering::SeqCst), CLOSED, LOG_CAP_BYTES));
        Admit::Drop
    }
}

/// The installed logger.
struct Logger {
    budget: LineBudget,
}

/// The process-wide logger `install` hands to `log`.
static LOGGER: Logger = Logger {
    budget: LineBudget::new(),
};

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        drop((metadata, MAX_LEVEL));
        true
    }

    fn log(&self, record: &Record<'_>) {
        drop(record);
    }

    fn flush(&self) {}
}

/// Installs the logger and sets the maximum level to `Warn`.
///
/// # Errors
///
/// A logger was already installed.
pub fn install() -> Result<(), SetLoggerError> {
    ::log::set_logger(&LOGGER)?;
    ::log::set_max_level(LevelFilter::Warn);
    Ok(())
}

#[cfg(test)]
mod tests {
    use ::log::{Level, Log, Metadata};

    use super::{Admit, LOG_CAP_BYTES, LOGGER, LineBudget, TRUNCATED_LINE, format_line};

    /// A message that tries to start a new line holding a watchdog marker, then a carriage return and an ANSI
    /// escape, stays one line under the log prefix.
    #[test]
    fn log_line_is_one_sanitized_line() {
        let line = format_line(Level::Warn, "x\nminos-vmm: watchdog parent-gone\r\x1b[31m");
        assert!(line.starts_with("minos-vmm: log warn: "), "got: {line}");
        assert!(!line.contains(['\n', '\r', '\x1b']), "got: {line}");
        assert_eq!(
            line,
            "minos-vmm: log warn: x?minos-vmm: watchdog parent-gone??[31m"
        );
        assert_eq!(line.lines().count(), 1);
        assert_ne!(line, "minos-vmm: watchdog parent-gone");
    }

    #[test]
    fn log_line_replaces_everything_outside_printable_ascii() {
        let line = format_line(Level::Error, "caf\u{e9}\t\u{7f}\u{202e}~ ");
        assert_eq!(line, "minos-vmm: log error: caf????~ ");
        assert!(line.bytes().all(|byte| (0x20..=0x7e).contains(&byte)));
    }

    /// Lines totalling more than the cap are cut: what is admitted stays within the cap, the truncated line is
    /// admitted exactly once, and nothing follows it.
    #[test]
    fn log_budget_caps_total() {
        let budget = LineBudget::new();
        let line_len = 1000;
        let mut written = 0_u64;
        let mut truncated = 0;
        let mut after_truncation = 0;
        for _ in 0..200 {
            match budget.admit(line_len) {
                Admit::Line => {
                    assert_eq!(truncated, 0, "a line was admitted after the truncated line");
                    written += line_len;
                }
                Admit::Truncated => truncated += 1,
                Admit::Drop => after_truncation += 1,
            }
        }
        assert!(written <= LOG_CAP_BYTES, "wrote {written} bytes");
        assert_eq!(written, 65_000);
        assert_eq!(truncated, 1);
        assert_eq!(after_truncation, 200 - 65 - 1);
        assert_eq!(TRUNCATED_LINE, "minos-vmm: log truncated");
    }

    /// A line that fills the budget exactly is admitted; the next is the truncated line, even a short one.
    #[test]
    fn log_budget_admits_up_to_the_cap_exactly() {
        let budget = LineBudget::new();
        assert_eq!(budget.admit(LOG_CAP_BYTES), Admit::Line);
        assert_eq!(budget.admit(1), Admit::Truncated);
        assert_eq!(budget.admit(1), Admit::Drop);

        let budget = LineBudget::new();
        assert_eq!(budget.admit(LOG_CAP_BYTES + 1), Admit::Truncated);
        assert_eq!(budget.admit(0), Admit::Drop);
    }

    #[test]
    fn log_level_is_warn() {
        let enabled = |level| LOGGER.enabled(&Metadata::builder().level(level).build());
        assert!(enabled(Level::Error));
        assert!(enabled(Level::Warn));
        assert!(!enabled(Level::Info));
        assert!(!enabled(Level::Debug));
        assert!(!enabled(Level::Trace));
    }
}
