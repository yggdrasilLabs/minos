//! The `logparse` sample judge for Minos.
//!
//! `logparse-judge <answers.tsv> <candidate-output.tsv> <scorecard.json>` scores the candidate's output file
//! against the private answer key and writes the scorecard Minos reads:
//! `{"v":1,"metrics":[{"name":"rows_match",...},{"name":"accuracy",...},{"name":"accuracy_held_out",...}]}`.
//!
//! - `rows_match`, the gate, is 1 exactly when the output has one row per key row, the Nth row's first field is
//!   `N`, and every row has exactly five tab-separated fields; else 0.
//! - `accuracy`, the objective, is the fraction of the four fields per row that equal the key byte for byte,
//!   over the visible rows (the ones `data/examples.tsv` shows the proposer).
//! - `accuracy_held_out` is the same fraction over the held-out rows, whose answers the proposer never sees.
//!
//! The judge reads only its key and the candidate's output file, never a number the candidate reports about
//! itself. An output file that is missing, unreadable, too large, or not UTF-8 counts as no rows, so a broken
//! candidate is scored, never a judge crash. It is compiled inside the judge VM from this pushed source (D-04).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read as _;
use std::path::Path;
use std::process::ExitCode;

/// The largest candidate output the judge reads: the cap on a candidate's whole `out/` directory.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;

/// The most rows an answer key may hold.
const MAX_KEY_ROWS: usize = 100_000;

/// The fields each row carries after its line number.
const FIELDS: usize = 4;

/// Which half of the input a key row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Split {
    /// Its answer is shown to the proposer in `data/examples.tsv`.
    Visible,
    /// Its answer is never shown to the proposer.
    HeldOut,
}

/// One key row; its line number is its 1-based position in the key.
#[derive(Debug)]
struct Answer {
    split: Split,
    fields: Vec<String>,
}

/// The three metrics, each in `[0, 1]`.
#[derive(Debug, PartialEq)]
struct Scorecard {
    rows_match: f64,
    accuracy: f64,
    accuracy_held_out: f64,
}

/// Why the judge could not write a scorecard.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// Not exactly three arguments.
    Usage,
    /// The answer key could not be read as UTF-8 text.
    KeyUnreadable,
    /// The answer key does not have the expected shape.
    KeyMalformed,
    /// The scorecard could not be written.
    ScorecardUnwritable,
}

impl Failure {
    /// The fixed message and the exit code for this failure.
    fn report(&self) -> (&'static str, u8) {
        match self {
            Self::Usage => (
                "usage: logparse-judge <answers.tsv> <candidate-output.tsv> <scorecard.json>",
                2,
            ),
            Self::KeyUnreadable => ("logparse-judge: cannot read the answer key", 1),
            Self::KeyMalformed => ("logparse-judge: the answer key is malformed", 1),
            Self::ScorecardUnwritable => ("logparse-judge: cannot write the scorecard", 1),
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            let (message, code) = failure.report();
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}

/// Scores the output named by `args` against the key and writes the scorecard.
fn run(args: &[OsString]) -> Result<(), Failure> {
    let [answers, output, scorecard] = args else {
        return Err(Failure::Usage);
    };
    let key_text = std::fs::read_to_string(answers).map_err(|_| Failure::KeyUnreadable)?;
    let key = parse_key(&key_text).ok_or(Failure::KeyMalformed)?;
    let output = read_output(Path::new(output));
    let card = score(&key, output.as_deref());
    std::fs::write(scorecard, card.to_json()).map_err(|_| Failure::ScorecardUnwritable)
}

/// The key rows, in line order; `None` when the key is malformed.
///
/// Lines starting with `#` are comments. Every other line is `<N>\t<split>\t<4 fields>`, numbered from 1 with
/// no gap, and both splits must be present.
fn parse_key(text: &str) -> Option<Vec<Answer>> {
    let mut key: Vec<Answer> = Vec::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        if key.len() >= MAX_KEY_ROWS {
            return None;
        }
        let mut columns = line.split('\t');
        let number: usize = columns.next()?.parse().ok()?;
        if number != key.len().checked_add(1)? {
            return None;
        }
        let split = match columns.next()? {
            "visible" => Split::Visible,
            "held_out" => Split::HeldOut,
            _ => return None,
        };
        let fields: Vec<String> = columns.map(str::to_owned).collect();
        if fields.len() != FIELDS {
            return None;
        }
        key.push(Answer { split, fields });
    }
    let has = |split: Split| key.iter().any(|answer| answer.split == split);
    (has(Split::Visible) && has(Split::HeldOut)).then_some(key)
}

/// The candidate's output, or `None` when it is missing, unreadable, over the cap, or not UTF-8.
fn read_output(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut text = String::new();
    file.take(MAX_OUTPUT_BYTES.checked_add(1)?)
        .read_to_string(&mut text)
        .ok()?;
    let within_cap = u64::try_from(text.len()).is_ok_and(|len| len <= MAX_OUTPUT_BYTES);
    within_cap.then_some(text)
}

/// Scores the candidate's output text against the key.
fn score(key: &[Answer], output: Option<&str>) -> Scorecard {
    let rows: Vec<&str> = output
        .map(|text| text.split_terminator('\n').collect())
        .unwrap_or_default();

    let rows_match = rows.len() == key.len()
        && (1_usize..).zip(&rows).all(|(number, row)| {
            let mut columns = row.split('\t');
            columns.next() == Some(number.to_string().as_str()) && columns.count() == FIELDS
        });

    // Each row's fields after its line number, by line number; a number written twice is ambiguous and matches
    // nothing.
    let mut by_number: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    for row in &rows {
        let (number, fields) = row.split_once('\t').unwrap_or((row, ""));
        by_number
            .entry(number)
            .and_modify(|seen| *seen = None)
            .or_insert(Some(fields));
    }

    let mut visible = Tally::default();
    let mut held_out = Tally::default();
    for (number, answer) in (1_usize..).zip(key) {
        let tally = match answer.split {
            Split::Visible => &mut visible,
            Split::HeldOut => &mut held_out,
        };
        let candidate = by_number
            .get(number.to_string().as_str())
            .copied()
            .flatten();
        let matched = candidate.map_or(0, |fields| {
            fields
                .split('\t')
                .zip(&answer.fields)
                .filter(|(given, expected)| given == expected)
                .count()
        });
        tally.add(matched);
    }

    Scorecard {
        rows_match: if rows_match { 1.0 } else { 0.0 },
        accuracy: visible.fraction(),
        accuracy_held_out: held_out.fraction(),
    }
}

/// Matched fields over total fields for one split.
#[derive(Default)]
struct Tally {
    matched: u32,
    total: u32,
}

impl Tally {
    /// Counts one key row, of which `matched` fields were right.
    fn add(&mut self, matched: usize) {
        // The key holds at most MAX_KEY_ROWS rows of FIELDS fields, so neither count can overflow a u32.
        self.matched = self
            .matched
            .saturating_add(u32::try_from(matched).unwrap_or(u32::MAX));
        self.total = self
            .total
            .saturating_add(u32::try_from(FIELDS).unwrap_or(u32::MAX));
    }

    /// The fraction in `[0, 1]`; 0 for a split with no rows, so the result is always finite.
    fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            f64::from(self.matched) / f64::from(self.total)
        }
    }
}

impl Scorecard {
    /// The scorecard in Minos's wire form. Every value is a finite fraction, and Rust's `f64` `Display` writes it
    /// as a plain decimal JSON number.
    fn to_json(&self) -> String {
        format!(
            "{{\"v\":1,\"metrics\":[\
             {{\"name\":\"rows_match\",\"value\":{},\"held_out\":false}},\
             {{\"name\":\"accuracy\",\"value\":{},\"held_out\":false}},\
             {{\"name\":\"accuracy_held_out\",\"value\":{},\"held_out\":true}}]}}",
            self.rows_match, self.accuracy, self.accuracy_held_out
        )
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{Failure, Scorecard, parse_key, run, score};

    type TestResult = Result<(), Box<dyn Error>>;

    /// The sample's own answer key, as `minos judge push` uploads it.
    const ANSWERS: &str = include_str!("../answers.tsv");

    /// A small key: two visible rows and two held-out rows.
    const SMALL_KEY: &str = "# line\tsplit\ttimestamp\tlevel\tcomponent\tlatency_ms\n\
        1\tvisible\t2026-10-28T14:03:11Z\tINFO\tauth\t12\n\
        2\tvisible\t2026-10-28T14:03:12Z\t-\tnginx\t15\n\
        3\theld_out\t2026-10-29T08:00:01Z\tINFO\tapi\t27\n\
        4\theld_out\t2026-10-29T08:00:02Z\t-\tnginx\t-\n";

    /// The output a perfect candidate writes for `key_text`: each key row without its split column.
    fn perfect_output(key_text: &str) -> String {
        key_text
            .lines()
            .filter(|line| !line.starts_with('#'))
            .flat_map(|line| {
                let (number, rest) = line.split_once('\t').unwrap_or((line, ""));
                let (_split, fields) = rest.split_once('\t').unwrap_or((rest, ""));
                [number, "\t", fields, "\n"]
            })
            .collect()
    }

    /// A path under the system temporary directory that is unique to this process and test.
    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("logparse-judge-{}-{name}", std::process::id()))
    }

    #[test]
    fn a_perfect_output_for_the_sample_key_scores_one_everywhere() -> TestResult {
        // Arrange
        let key = parse_key(ANSWERS).ok_or("the sample key is malformed")?;
        let output = perfect_output(ANSWERS);

        // Act
        let card = score(&key, Some(&output));

        // Assert
        assert_eq!(key.len(), 48);
        assert!((card.rows_match - 1.0).abs() < f64::EPSILON);
        assert!((card.accuracy - 1.0).abs() < f64::EPSILON);
        assert!((card.accuracy_held_out - 1.0).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn a_missing_row_fails_the_gate_and_costs_four_fields() -> TestResult {
        // Arrange
        let key = parse_key(SMALL_KEY).ok_or("the small key is malformed")?;
        let output: String = perfect_output(SMALL_KEY)
            .lines()
            .take(3)
            .flat_map(|row| [row, "\n"])
            .collect();

        // Act
        let card = score(&key, Some(&output));

        // Assert
        assert!(card.rows_match.abs() < f64::EPSILON);
        assert!((card.accuracy - 1.0).abs() < f64::EPSILON);
        assert!((card.accuracy_held_out - 0.5).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn a_misnumbered_row_fails_the_gate() -> TestResult {
        // Arrange
        let key = parse_key(SMALL_KEY).ok_or("the small key is malformed")?;
        let output = perfect_output(SMALL_KEY).replacen("3\t", "4\t", 1);

        // Act
        let card = score(&key, Some(&output));

        // Assert
        assert!(card.rows_match.abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn a_row_with_extra_fields_fails_the_gate() -> TestResult {
        // Arrange
        let key = parse_key(SMALL_KEY).ok_or("the small key is malformed")?;
        let output = perfect_output(SMALL_KEY).replacen("\tauth\t12\n", "\tauth\t12\textra\n", 1);

        // Act
        let card = score(&key, Some(&output));

        // Assert
        assert!(card.rows_match.abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn a_wrong_field_costs_one_quarter_of_a_row() -> TestResult {
        // Arrange
        let key = parse_key(SMALL_KEY).ok_or("the small key is malformed")?;
        let output = perfect_output(SMALL_KEY).replacen("\tauth\t12\n", "\tauth\t13\n", 1);

        // Act
        let card = score(&key, Some(&output));

        // Assert
        assert!((card.rows_match - 1.0).abs() < f64::EPSILON);
        assert!((card.accuracy - 7.0 / 8.0).abs() < f64::EPSILON);
        assert!((card.accuracy_held_out - 1.0).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn an_absent_output_scores_zero_everywhere() -> TestResult {
        // Arrange
        let key = parse_key(SMALL_KEY).ok_or("the small key is malformed")?;

        // Act
        let card = score(&key, None);

        // Assert
        assert!(card.rows_match.abs() < f64::EPSILON);
        assert!(card.accuracy.abs() < f64::EPSILON);
        assert!(card.accuracy_held_out.abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn the_scorecard_json_has_the_wire_form() {
        // Arrange
        let card = Scorecard {
            rows_match: 1.0,
            accuracy: 0.5,
            accuracy_held_out: 0.25,
        };

        // Act
        let json = card.to_json();

        // Assert
        assert_eq!(
            json,
            "{\"v\":1,\"metrics\":[\
             {\"name\":\"rows_match\",\"value\":1,\"held_out\":false},\
             {\"name\":\"accuracy\",\"value\":0.5,\"held_out\":false},\
             {\"name\":\"accuracy_held_out\",\"value\":0.25,\"held_out\":true}]}"
        );
    }

    #[test]
    fn a_key_with_a_gap_or_an_unknown_split_is_malformed() {
        // Arrange
        let gap = SMALL_KEY.replacen("2\tvisible", "5\tvisible", 1);
        let unknown = SMALL_KEY.replacen("2\tvisible", "2\thidden", 1);

        // Act
        let parsed = (parse_key(&gap), parse_key(&unknown));

        // Assert
        assert!(parsed.0.is_none());
        assert!(parsed.1.is_none());
    }

    #[test]
    fn an_unreadable_answer_key_is_a_failure() {
        // Arrange
        let args: Vec<OsString> = vec![
            temp_path("missing-answers.tsv").into(),
            temp_path("missing-output.tsv").into(),
            temp_path("unused-scorecard.json").into(),
        ];

        // Act
        let result = run(&args);

        // Assert
        assert_eq!(result, Err(Failure::KeyUnreadable));
    }

    #[test]
    fn an_unwritable_scorecard_path_is_a_failure() -> TestResult {
        // Arrange
        let answers = temp_path("answers.tsv");
        std::fs::write(&answers, SMALL_KEY)?;
        let args: Vec<OsString> = vec![
            answers.clone().into(),
            temp_path("missing-output.tsv").into(),
            temp_path("no-such-directory").join("scorecard.json").into(),
        ];

        // Act
        let result = run(&args);
        std::fs::remove_file(&answers)?;

        // Assert
        assert_eq!(result, Err(Failure::ScorecardUnwritable));
        Ok(())
    }

    #[test]
    fn a_missing_output_still_writes_a_scorecard() -> TestResult {
        // Arrange
        let answers = temp_path("answers-for-missing.tsv");
        let scorecard = temp_path("scorecard-for-missing.json");
        std::fs::write(&answers, SMALL_KEY)?;
        let args: Vec<OsString> = vec![
            answers.clone().into(),
            temp_path("absent-output.tsv").into(),
            scorecard.clone().into(),
        ];

        // Act
        let result = run(&args);
        let written = std::fs::read_to_string(&scorecard);
        std::fs::remove_file(&answers)?;
        std::fs::remove_file(&scorecard)?;

        // Assert
        assert_eq!(result, Ok(()));
        assert_eq!(
            written?,
            Scorecard {
                rows_match: 0.0,
                accuracy: 0.0,
                accuracy_held_out: 0.0,
            }
            .to_json()
        );
        Ok(())
    }

    #[test]
    fn the_wrong_argument_count_is_a_usage_failure() {
        // Arrange
        let args: Vec<OsString> = vec!["answers.tsv".into()];

        // Act
        let result = run(&args);

        // Assert
        assert_eq!(result, Err(Failure::Usage));
    }
}
