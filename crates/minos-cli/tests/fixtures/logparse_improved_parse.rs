//! The parser: the one file a Minos proposer may change.
//!
//! This is the improved parser for the `examples/logparse` sample, kept as a test fixture: the full-file content
//! the Phase 8 VM test's scripted provider proposes in place of `src/parse.rs`. It is not compiled by the Minos
//! workspace; it compiles only as `src/parse.rs` of the sample crate, under that crate's lints, and implements
//! the whole output spec in the sample's `program.md`:
//!
//! - format A: `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`, latency from `latency_ms=<n>`;
//! - format B: `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`, year 2026, no level, latency from a
//!   final `<n>ms` token;
//! - format C: `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`, latency from `took <n> ms`.
//!
//! Tokens may be separated by runs of spaces, and a line may carry leading or trailing spaces.

use crate::record::Record;

/// The year a format B line carries implicitly.
const SYSLOG_YEAR: &str = "2026";

/// The three-letter month names format B uses, in calendar order.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Extracts the four fields from one input line.
#[must_use]
pub fn parse_line(line: &str) -> Record {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    format_a(&tokens)
        .or_else(|| format_c(&tokens))
        .or_else(|| format_b(&tokens))
        .unwrap_or_default()
}

/// `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`.
fn format_a(tokens: &[&str]) -> Option<Record> {
    let [stamp, level, component, message @ ..] = tokens else {
        return None;
    };
    let (date, time) = stamp.strip_suffix('Z')?.split_once('T')?;
    let timestamp = iso_timestamp(date, '-', time)?;
    let component = component_name(component.strip_suffix(':')?)?;
    let latency_ms = message
        .iter()
        .find_map(|token| token.strip_prefix("latency_ms="))
        .and_then(number);
    Some(Record::new(
        Some(timestamp),
        canonical_level(level),
        Some(component),
        latency_ms,
    ))
}

/// `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`.
fn format_c(tokens: &[&str]) -> Option<Record> {
    let [date, time, level, component, message @ ..] = tokens else {
        return None;
    };
    let date = date.strip_prefix('[')?;
    let time = time.strip_suffix(']')?;
    let timestamp = iso_timestamp(date, '/', time)?;
    let component = component_name(component.strip_suffix(':')?)?;
    let latency_ms = message.windows(3).find_map(|window| match window {
        ["took", value, "ms"] => number(value),
        _ => None,
    });
    Some(Record::new(
        Some(timestamp),
        canonical_level(level),
        Some(component),
        latency_ms,
    ))
}

/// `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`.
fn format_b(tokens: &[&str]) -> Option<Record> {
    let [month, day, time, _host, tag, message @ ..] = tokens else {
        return None;
    };
    let month = (1_u8..)
        .zip(MONTHS)
        .find_map(|(number, name)| (name == *month).then_some(number))?;
    let day: u8 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
    let time = clock(time)?;
    let (component, pid) = tag.strip_suffix("]:")?.split_once('[')?;
    number(pid)?;
    let component = component_name(component)?;
    let latency_ms = message
        .last()
        .and_then(|token| token.strip_suffix("ms"))
        .and_then(number);
    Some(Record::new(
        Some(format!("{SYSLOG_YEAR}-{month:02}-{day:02}T{time}Z")),
        None,
        Some(component),
        latency_ms,
    ))
}

/// `YYYY<sep>MM<sep>DD` and `HH:MM:SS` as `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_timestamp(date: &str, separator: char, time: &str) -> Option<String> {
    let mut parts = date.split(separator);
    let year = digits(parts.next()?, 4)?;
    let month = digits(parts.next()?, 2)?;
    let day = digits(parts.next()?, 2)?;
    if parts.next().is_some() {
        return None;
    }
    let time = clock(time)?;
    Some(format!("{year}-{month}-{day}T{time}Z"))
}

/// `HH:MM:SS`, two digits each.
fn clock(time: &str) -> Option<&str> {
    let mut parts = time.split(':');
    let all_two_digits = (0..3).all(|_| parts.next().and_then(|part| digits(part, 2)).is_some());
    (all_two_digits && parts.next().is_none()).then_some(time)
}

/// `text` when it is exactly `count` ASCII digits.
fn digits(text: &str, count: usize) -> Option<&str> {
    (text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(text)
}

/// A non-negative integer written in ASCII digits only.
fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A component name: non-empty letters, digits, `-`, `_`, or `.`.
fn component_name(text: &str) -> Option<String> {
    let valid = !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    valid.then(|| text.to_owned())
}

/// A level spelling, case-insensitive, as its canonical name; `warning` is `WARN` and `err` is `ERROR`.
fn canonical_level(text: &str) -> Option<String> {
    let canonical = match text.to_ascii_uppercase().as_str() {
        "DEBUG" => "DEBUG",
        "INFO" => "INFO",
        "WARN" | "WARNING" => "WARN",
        "ERROR" | "ERR" => "ERROR",
        _ => return None,
    };
    Some(canonical.to_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_line;

    #[test]
    fn a_format_a_line_with_runs_of_spaces_is_parsed() {
        // Arrange
        let line = "2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "2026-10-28T14:03:40Z\tERROR\tbilling\t910");
    }

    #[test]
    fn a_format_b_line_has_no_level() {
        // Arrange
        let line = "Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "2026-11-03T09:15:02Z\t-\tcron\t120");
    }

    #[test]
    fn a_format_c_line_takes_its_latency_from_took() {
        // Arrange
        let line = "[2026/10/28 14:03:21] warning db: slow query took 1250 ms";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "2026-10-28T14:03:21Z\tWARN\tdb\t1250");
    }

    #[test]
    fn an_unrecognized_line_yields_an_empty_record() {
        // Arrange
        let line = "not a log line";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "-\t-\t-\t-");
    }
}
