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
/// Most message characters kept. A longer line could never fit under [`LOG_CAP_BYTES`] anyway.
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

/// One log line for `level` and `message`: the prefix, the level word, and the message with every character
/// outside printable ASCII replaced by `?`, at most [`MAX_MESSAGE_CHARS`] message characters, no newline.
pub fn format_line(level: Level, message: &str) -> String {
    let word = level_word(level);
    let mut line = String::with_capacity(
        PREFIX
            .len()
            .saturating_add(word.len())
            .saturating_add(2)
            .saturating_add(message.len().min(MAX_MESSAGE_CHARS)),
    );
    line.push_str(PREFIX);
    line.push_str(word);
    line.push_str(": ");
    line.extend(message.chars().take(MAX_MESSAGE_CHARS).map(|character| {
        if matches!(character, ' '..='~') {
            character
        } else {
            '?'
        }
    }));
    line
}

/// The record's message, formatted into a string of at most [`MAX_MESSAGE_CHARS`] characters, so a huge message
/// is never held whole.
fn message_text(message: fmt::Arguments<'_>) -> String {
    let mut text = Bounded {
        text: String::new(),
        room: MAX_MESSAGE_CHARS,
    };
    // A full writer stops formatting with `fmt::Error`; what was written so far is the message.
    text.write_fmt(message).unwrap_or(());
    text.text
}

/// The lowercase word for `level`.
fn level_word(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

/// A `fmt::Write` sink that stops after `room` characters.
struct Bounded {
    text: String,
    room: usize,
}

impl fmt::Write for Bounded {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for character in text.chars() {
            let Some(room) = self.room.checked_sub(1) else {
                return Err(fmt::Error);
            };
            self.room = room;
            self.text.push(character);
        }
        Ok(())
    }
}

/// The byte budget shared by every logged line.
pub struct LineBudget {
    /// Bytes admitted so far, or [`CLOSED`] once the truncated line was admitted.
    used: AtomicU64,
}

impl LineBudget {
    /// An empty budget.
    pub const fn new() -> Self {
        Self {
            used: AtomicU64::new(0),
        }
    }

    /// Whether a line of `len` bytes, its newline included, may be written: [`Admit::Line`] while the total stays
    /// within [`LOG_CAP_BYTES`], [`Admit::Truncated`] once for the first line that would pass it, and
    /// [`Admit::Drop`] for everything after.
    pub fn admit(&self, len: u64) -> Admit {
        let mut used = self.used.load(Ordering::SeqCst);
        loop {
            if used == CLOSED {
                return Admit::Drop;
            }
            let (next, verdict) = match used.checked_add(len) {
                Some(total) if total <= LOG_CAP_BYTES => (total, Admit::Line),
                _ => (CLOSED, Admit::Truncated),
            };
            match self
                .used
                .compare_exchange_weak(used, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return verdict,
                Err(actual) => used = actual,
            }
        }
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
        metadata.level() <= MAX_LEVEL
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format_line(record.level(), &message_text(*record.args()));
        let len = u64::try_from(line.len().saturating_add(1)).unwrap_or(u64::MAX);
        match self.budget.admit(len) {
            Admit::Line => crate::say(format_args!("{line}")),
            Admit::Truncated => crate::say(format_args!("{TRUNCATED_LINE}")),
            Admit::Drop => {}
        }
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

    use super::{
        Admit, LOG_CAP_BYTES, LOGGER, LineBudget, MAX_MESSAGE_CHARS, TRUNCATED_LINE, format_line,
        message_text,
    };

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

    /// A message is formatted into at most `MAX_MESSAGE_CHARS` characters, and a line never carries more.
    #[test]
    fn log_message_is_bounded() {
        let long = "a".repeat(MAX_MESSAGE_CHARS + 10);
        let text = message_text(format_args!("{long}"));
        assert_eq!(text.chars().count(), MAX_MESSAGE_CHARS);
        let line = format_line(Level::Warn, &long);
        assert_eq!(
            line.len(),
            "minos-vmm: log warn: ".len() + MAX_MESSAGE_CHARS
        );
        assert_eq!(message_text(format_args!("{} {}", 1, "two")), "1 two");
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
