//! The history view (06-CONTEXT D-07, D-08, PROP-06): rows rendered from recorded outcomes, bounded and
//! sanitized, with visible-objective deltas only and no way to carry a held-out value.

use std::error::Error;

use minos_core::id::{ExperimentId, TreeId};
use minos_core::limits::{Limits, RATIONALE_EXCERPT_BYTES};
use minos_core::manifest::{JudgeSpec, Manifest};
use minos_core::metric::Scorecard;
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::path::RelPath;
use minos_core::text::LogExcerpt;
use minos_propose::limits::{EXCERPTS_MAX_BYTES, HISTORY_TABLE_MAX_BYTES};
use minos_propose::prompt::{History, HistoryEntry};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// One maximize objective paired with a held-out metric, and one gate.
const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"
held_out = "held_out_mb_s"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "haiku"
batch = 4
"#;

fn judge() -> Result<JudgeSpec, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?.judge().clone())
}

/// A distinct experiment ID per name.
fn exp(name: &str) -> ExperimentId {
    ExperimentId::baseline(&TreeId::of(name.as_bytes()))
}

/// The first 12 hex characters of `id`, as a row shows it.
fn short(id: &ExperimentId) -> String {
    id.to_hex().chars().take(12).collect()
}

/// A scored outcome built from the scorecard wire form.
fn scored(throughput: f64, gate: f64, held_out: f64) -> Result<Outcome, Box<dyn Error>> {
    let json = format!(
        concat!(
            r#"{{"v":1,"metrics":["#,
            r#"{{"name":"throughput_mb_s","value":{},"held_out":false}},"#,
            r#"{{"name":"identical_output","value":{},"held_out":false}},"#,
            r#"{{"name":"held_out_mb_s","value":{},"held_out":true}}]}}"#
        ),
        throughput, gate, held_out
    );
    Ok(Outcome::Scored(Scorecard::parse_json(
        json.as_bytes(),
        &Limits::standard(),
    )?))
}

/// A scored outcome that lacks the objective metric, so it cannot reduce.
fn scored_without_objective() -> Result<Outcome, Box<dyn Error>> {
    let json = concat!(
        r#"{"v":1,"metrics":["#,
        r#"{"name":"identical_output","value":1,"held_out":false},"#,
        r#"{"name":"held_out_mb_s","value":3,"held_out":true}]}"#
    );
    Ok(Outcome::Scored(Scorecard::parse_json(
        json.as_bytes(),
        &Limits::standard(),
    )?))
}

fn entry(generation: u32, name: &str, accepted: bool, trials: Vec<(u32, Outcome)>) -> HistoryEntry {
    HistoryEntry::new(
        generation,
        exp(name),
        accepted,
        "try a faster loop",
        trials,
        None,
    )
}

/// The table line for `name`, if any.
fn row<'a>(history: &'a History, name: &str) -> Option<&'a str> {
    let key = format!("exp {} ", short(&exp(name)));
    history.table().lines().find(|line| line.contains(&key))
}

fn row_or_err<'a>(history: &'a History, name: &str) -> Result<&'a str, Box<dyn Error>> {
    row(history, name).ok_or_else(|| format!("no row for {name}").into())
}

/// One rejected entry with `reason` in generation 1, rendered against a scored incumbent.
fn rejected_row(reason: Reason) -> Result<String, Box<dyn Error>> {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![entry(1, "a", false, vec![(0, Outcome::Rejected(reason))])],
    );
    Ok(row_or_err(&history, "a")?.to_owned())
}

// ---------------------------------------------------------------------------------------------------------------
// Scored rows
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn scored_row_shows_gates_and_signed_delta() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![
            entry(1, "a", true, vec![(0, scored(105.0, 1.0, 51.0)?)]),
            entry(1, "b", false, vec![(0, scored(95.0, 0.0, 49.0)?)]),
        ],
    );
    let a = row_or_err(&history, "a")?;
    assert_eq!(
        a,
        format!(
            "gen 1 | exp {} | winner | scored | gates: identical_output pass | throughput_mb_s +5.00% \
             | rationale: try a faster loop",
            short(&exp("a"))
        )
    );
    let b = row_or_err(&history, "b")?;
    assert!(b.contains("| candidate | scored | gates: identical_output fail | throughput_mb_s -5.00% |"));
    assert_eq!(history.row_count(), 2);
    assert_eq!(history.omitted_rows(), 0);
    Ok(())
}

#[test]
fn gate_passes_only_at_exactly_one() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![entry(1, "a", false, vec![(0, scored(105.0, 0.999_999, 51.0)?)])],
    );
    assert!(row_or_err(&history, "a")?.contains("gates: identical_output fail"));
    Ok(())
}

#[test]
fn delta_is_not_available_without_a_usable_incumbent() -> TestResult {
    let judge = judge()?;
    let zero = vec![scored(0.0, 1.0, 50.0)?];
    let history = History::new(
        &judge,
        2,
        &zero,
        vec![entry(1, "a", false, vec![(0, scored(105.0, 1.0, 51.0)?)])],
    );
    assert!(row_or_err(&history, "a")?.contains("| throughput_mb_s n/a |"));

    let unscored = vec![Outcome::Crashed { stage: Stage::Run }];
    let history = History::new(
        &judge,
        2,
        &unscored,
        vec![entry(1, "a", false, vec![(0, scored(105.0, 1.0, 51.0)?)])],
    );
    assert!(row_or_err(&history, "a")?.contains("| throughput_mb_s n/a |"));

    let history = History::new(
        &judge,
        2,
        &[],
        vec![entry(1, "a", false, vec![(0, scored(105.0, 1.0, 51.0)?)])],
    );
    assert!(row_or_err(&history, "a")?.contains("| throughput_mb_s n/a |"));
    Ok(())
}

#[test]
fn incumbent_reduces_over_its_scored_trials_only() -> TestResult {
    // The incumbent's one crashed trial is skipped (TrialRule::ScoredOnly), as select_greedy does.
    let incumbent = vec![Outcome::TimedOut { stage: Stage::Judge }, scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![entry(1, "a", false, vec![(0, scored(110.0, 1.0, 51.0)?)])],
    );
    assert!(row_or_err(&history, "a")?.contains("| throughput_mb_s +10.00% |"));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Unscored rows
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn one_unscored_trial_makes_the_row_unscored() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![entry(
            1,
            "a",
            false,
            vec![
                (2, Outcome::TimedOut { stage: Stage::Judge }),
                (0, scored(105.0, 1.0, 51.0)?),
                (1, Outcome::Crashed { stage: Stage::Run }),
            ],
        )],
    );
    let a = row_or_err(&history, "a")?;
    assert!(a.contains("| candidate | crashed in run | rationale: "), "{a}");
    assert!(!a.contains("scored"));
    assert!(!a.contains("throughput_mb_s"));
    Ok(())
}

#[test]
fn build_failure_renders_its_fixed_phrase() -> TestResult {
    assert!(rejected_row(Reason::BuildFailed)?.contains("| rejected: the build failed |"));
    Ok(())
}

#[test]
fn gate_failure_names_the_gate() -> TestResult {
    let gate = "identical_output".parse()?;
    assert!(rejected_row(Reason::GateFailed { gate })?.contains("| rejected: gate identical_output failed |"));
    Ok(())
}

#[test]
fn scorecard_shape_reasons_name_no_metric() -> TestResult {
    let reasons = [
        Reason::MissingMetric {
            metric: "held_out_mb_s".parse()?,
        },
        Reason::UndeclaredMetric {
            metric: "held_out_mb_s".parse()?,
        },
        Reason::HeldOutMismatch {
            metric: "held_out_mb_s".parse()?,
        },
        Reason::ScorecardInvalid,
    ];
    for reason in reasons {
        let text = rejected_row(reason)?;
        assert!(text.contains("| rejected: the judge's scorecard was invalid |"), "{text}");
        assert!(!text.contains("held_out"), "{text}");
    }
    Ok(())
}

#[test]
fn other_reasons_use_their_display_text() -> TestResult {
    let cases = [
        (
            Reason::ApplyFailed {
                path: RelPath::try_from("src/parse.rs")?,
                hunk: 2,
            },
            "rejected: hunk 2 of src/parse.rs does not apply",
        ),
        (Reason::RunFailed, "rejected: the run failed"),
        (Reason::OutputInvalid, "rejected: the output is invalid"),
        (Reason::OutputTooLarge, "rejected: the output is over its cap"),
        (
            Reason::ChunkProtocol,
            "rejected: the output stream broke the chunk protocol",
        ),
    ];
    for (reason, phrase) in cases {
        let text = rejected_row(reason)?;
        assert!(text.contains(&format!("| {phrase} |")), "{text}");
    }
    Ok(())
}

#[test]
fn crash_and_timeout_name_their_stage() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![
            entry(1, "a", false, vec![(0, Outcome::Crashed { stage: Stage::Run })]),
            entry(1, "b", false, vec![(0, Outcome::TimedOut { stage: Stage::Judge })]),
        ],
    );
    assert!(row_or_err(&history, "a")?.contains("| crashed in run |"));
    assert!(row_or_err(&history, "b")?.contains("| timed out in judge |"));
    Ok(())
}

#[test]
fn all_scored_trials_that_do_not_reduce_render_an_invalid_scorecard() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![entry(
            1,
            "a",
            false,
            vec![(0, scored(105.0, 1.0, 51.0)?), (1, scored_without_objective()?)],
        )],
    );
    assert!(row_or_err(&history, "a")?.contains("| rejected: the judge's scorecard was invalid |"));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Rationales
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn rationale_is_inline_sanitized_and_cut() -> TestResult {
    let raw = "line one\nline two \u{1b}[31mred\u{202e}reversed";
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let history = History::new(
        &judge()?,
        2,
        &incumbent,
        vec![HistoryEntry::new(
            1,
            exp("a"),
            false,
            raw,
            vec![(0, Outcome::Rejected(Reason::BuildFailed))],
            None,
        )],
    );
    assert_eq!(history.table().lines().count(), 1);
    let a = row_or_err(&history, "a")?;
    assert!(a.ends_with("| rationale: line one<U+000A>line two [31mred<U+202E>reversed"), "{a}");
    assert!(!history.table().contains('\u{1b}'));
    assert!(!history.table().contains('\u{202e}'));

    // A multi-byte rationale is cut at a char boundary, never inside a character.
    let long = "\u{65e5}".repeat(100);
    let cut = HistoryEntry::new(1, exp("b"), false, &long, Vec::new(), None);
    assert!(cut.rationale().len() <= RATIONALE_EXCERPT_BYTES);
    assert_eq!(cut.rationale().len(), 198);
    assert!(cut.rationale().as_str().chars().all(|c| c == '\u{65e5}'));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Window and order
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn window_keeps_recent_rows_and_every_winner_in_order() -> TestResult {
    let judge = judge()?;
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let rejected = || vec![(0, Outcome::Rejected(Reason::BuildFailed))];
    let entries = vec![
        entry(19, "newest", false, rejected()),
        entry(1, "old-winner", true, vec![(0, scored(101.0, 1.0, 50.0)?)]),
        entry(11, "too-old", false, rejected()),
        entry(12, "edge", false, rejected()),
        entry(2, "old-loser", false, rejected()),
        entry(12, "edge-two", false, rejected()),
    ];
    let mut reversed = entries.clone();
    reversed.reverse();

    let history = History::new(&judge, 20, &incumbent, entries);
    assert!(row(&history, "too-old").is_none());
    assert!(row(&history, "old-loser").is_none());
    assert_eq!(history.row_count(), 4);

    let lines: Vec<&str> = history.table().lines().collect();
    let generations: Vec<&str> = lines
        .iter()
        .filter_map(|line| line.split(" | ").next())
        .collect();
    assert_eq!(generations, ["gen 1", "gen 12", "gen 12", "gen 19"]);

    // Within a generation, rows are in experiment ID order.
    let (first, second) = if exp("edge") < exp("edge-two") {
        ("edge", "edge-two")
    } else {
        ("edge-two", "edge")
    };
    let first_at = history.table().find(&short(&exp(first))).ok_or("first missing")?;
    let second_at = history.table().find(&short(&exp(second))).ok_or("second missing")?;
    assert!(first_at < second_at);

    // Input order does not matter.
    assert_eq!(History::new(&judge, 20, &incumbent, reversed), history);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Caps
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn table_stays_under_its_cap_dropping_the_oldest_non_winners_first() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let long_rationale = "r".repeat(400);
    let mut entries = Vec::new();
    for index in 0..500_u32 {
        let generation = 1 + index / 63;
        entries.push(HistoryEntry::new(
            generation,
            exp(&format!("e{index}")),
            index == 0,
            &long_rationale,
            vec![(0, Outcome::Rejected(Reason::BuildFailed))],
            None,
        ));
    }
    let history = History::new(&judge()?, 9, &incumbent, entries);
    assert!(history.table().len() <= HISTORY_TABLE_MAX_BYTES);
    assert!(history.omitted_rows() > 0);
    assert_eq!(history.row_count() + history.omitted_rows(), 500);
    assert!(
        history
            .table()
            .ends_with(&format!("({} earlier rows omitted)\n", history.omitted_rows()))
    );
    // The oldest row is the only winner, so it stays; the newest row stays too.
    assert!(row(&history, "e0").is_some());
    assert!(row(&history, "e499").is_some());
    assert!(row(&history, "e1").is_none());
    Ok(())
}

#[test]
fn excerpts_keep_the_newest_whole_excerpts_under_their_cap() -> TestResult {
    let incumbent = vec![scored(100.0, 1.0, 50.0)?];
    let log = "x".repeat(10_000);
    let mut entries = Vec::new();
    for index in 0..8_u32 {
        entries.push(HistoryEntry::new(
            index + 1,
            exp(&format!("e{index}")),
            false,
            "build it",
            vec![(0, Outcome::Rejected(Reason::BuildFailed))],
            Some(LogExcerpt::from_bytes(log.as_bytes(), 4096)),
        ));
    }
    // A scored row's excerpt is never shown.
    entries.push(HistoryEntry::new(
        9,
        exp("scored"),
        false,
        "fast",
        vec![(0, scored(105.0, 1.0, 51.0)?)],
        Some(LogExcerpt::from_bytes(b"scored log", 4096)),
    ));
    let history = History::new(&judge()?, 9, &incumbent, entries);
    let total: usize = history.excerpts().iter().map(|(_, text)| text.len()).sum();
    assert!(total <= EXCERPTS_MAX_BYTES);
    let labels: Vec<&str> = history.excerpts().iter().map(|(label, _)| label.as_str()).collect();
    let expected: Vec<String> = (4..8_u32)
        .map(|index| {
            format!(
                "gen {} exp {} rejected: the build failed",
                index + 1,
                short(&exp(&format!("e{index}")))
            )
        })
        .collect();
    assert_eq!(labels, expected);
    assert!(history.excerpts().iter().all(|(_, text)| text.len() == 4096));
    Ok(())
}

#[test]
fn empty_history_has_no_rows() {
    let history = History::empty();
    assert_eq!(history.table(), "");
    assert!(history.excerpts().is_empty());
    assert_eq!(history.row_count(), 0);
    assert_eq!(history.omitted_rows(), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// Held-out values
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn held_out_values_do_not_change_the_history() -> TestResult {
    let judge = judge()?;
    let build = |held_out: f64| -> Result<History, Box<dyn Error>> {
        let incumbent = vec![scored(100.0, 1.0, held_out)?];
        Ok(History::new(
            &judge,
            3,
            &incumbent,
            vec![
                entry(1, "a", true, vec![(0, scored(105.0, 1.0, held_out * 2.0)?)]),
                entry(2, "b", false, vec![(0, scored(90.0, 1.0, held_out + 7.0)?)]),
            ],
        ))
    };
    let low = build(1.0)?;
    let high = build(123_456.789)?;
    assert_eq!(low, high);
    assert!(!low.table().contains("held_out"));
    assert!(!low.table().contains("123456"));
    Ok(())
}
