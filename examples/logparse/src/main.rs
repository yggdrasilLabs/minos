//! The `logparse` sample candidate for Minos.
//!
//! `logparse <input.log> <output.tsv>` reads the input as UTF-8 text and writes one row per input line:
//! the 1-based line number, a tab, and the four fields [`parse::parse_line`] extracts, joined by tabs with `-`
//! for a missing field, each row ending in a line feed. `program.md` states the full output spec.
//!
//! Only `src/parse.rs` is writable for a proposer; this file and `src/record.rs` are readable context.

mod parse;
mod record;

use std::ffi::OsString;
use std::fmt::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: logparse <input.log> <output.tsv>");
        return ExitCode::from(2);
    };
    let Ok(text) = std::fs::read_to_string(input) else {
        eprintln!("logparse: cannot read the input file as UTF-8 text");
        return ExitCode::from(1);
    };
    let Ok(rows) = render(&text) else {
        eprintln!("logparse: cannot render the output rows");
        return ExitCode::from(1);
    };
    if std::fs::write(output, rows).is_err() {
        eprintln!("logparse: cannot write the output file");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// One `<line number>\t<fields>` row per input line, each ending in a line feed.
fn render(text: &str) -> Result<String, std::fmt::Error> {
    let mut rows = String::new();
    for (number, line) in (1_u64..).zip(text.lines()) {
        writeln!(rows, "{number}\t{}", parse::parse_line(line).to_tsv())?;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::render;

    type TestResult = Result<(), Box<dyn Error>>;

    /// The sample's own input, as the run command reads it.
    const INPUT: &str = include_str!("../data/input.log");

    #[test]
    fn the_sample_input_yields_one_numbered_row_per_line() -> TestResult {
        // Arrange
        let expected_rows = 48;

        // Act
        let rows = render(INPUT)?;

        // Assert
        assert_eq!(rows.lines().count(), expected_rows);
        for (number, row) in (1_u64..).zip(rows.lines()) {
            let prefix = format!("{number}\t");
            assert!(
                row.starts_with(&prefix),
                "row {number} is not numbered {number}"
            );
            assert_eq!(row.split('\t').count(), 5, "row {number} has five fields");
        }
        Ok(())
    }

    #[test]
    fn every_row_ends_in_a_line_feed() -> TestResult {
        // Arrange
        let text = "one\ntwo";

        // Act
        let rows = render(text)?;

        // Assert
        assert_eq!(rows, "1\t-\t-\t-\t-\n2\t-\t-\t-\t-\n");
        Ok(())
    }
}
