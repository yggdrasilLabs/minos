//! The parser: the one file a Minos proposer may change.
//!
//! This baseline is imperfect on purpose. It handles format A (`<ISO timestamp> <LEVEL> <component>: <message>`)
//! only when its tokens are separated by single spaces and its level is already one of the four canonical
//! spellings, and yields an empty record for every other line. `program.md` states the full spec.

use crate::record::Record;

/// The canonical level names.
const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];

/// Extracts the four fields from one input line.
#[must_use]
pub fn parse_line(line: &str) -> Record {
    let mut tokens = line.split(' ');
    let (Some(timestamp), Some(level), Some(component)) =
        (tokens.next(), tokens.next(), tokens.next())
    else {
        return Record::default();
    };
    let Some(component) = component.strip_suffix(':') else {
        return Record::default();
    };
    if !timestamp.ends_with('Z') || component.is_empty() {
        return Record::default();
    }
    let level = LEVELS.contains(&level).then(|| level.to_owned());
    let latency_ms = tokens
        .find_map(|token| token.strip_prefix("latency_ms="))
        .and_then(|value| value.parse().ok());
    Record::new(
        Some(timestamp.to_owned()),
        level,
        Some(component.to_owned()),
        latency_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::parse_line;

    #[test]
    fn a_single_spaced_format_a_line_is_parsed() {
        // Arrange
        let line = "2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
    }

    #[test]
    fn the_baseline_yields_nothing_for_format_b() {
        // Arrange
        let line = "Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "-\t-\t-\t-");
    }

    #[test]
    fn the_baseline_yields_nothing_for_format_c() {
        // Arrange
        let line = "[2026/10/28 14:03:13] INFO db: query took 42 ms";

        // Act
        let row = parse_line(line).to_tsv();

        // Assert
        assert_eq!(row, "-\t-\t-\t-");
    }
}
