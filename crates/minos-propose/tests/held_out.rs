//! Held-out non-interference (PROP-06, PITFALLS 11): no held-out metric's value or name can reach a prompt.
//!
//! The proof is a property: substituting arbitrary finite values for every held-out metric, in every trial of
//! every entry and of the incumbent, leaves the assembled prompt byte-identical, with an equal `PromptHash`. A
//! direct test also checks that a held-out name and value are absent, including from a rejection whose reason
//! names the held-out metric.
//!
//! This target builds its own fixture rather than sharing one with `tests/prompt.rs`, because a shared test
//! module must have every item used by each target that declares it.

use std::error::Error;

use minos_core::diff::apply::Tree;
use minos_core::id::{ExperimentId, TreeId};
use minos_core::limits::Limits;
use minos_core::manifest::Manifest;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::path::RelPath;
use minos_core::text::LogExcerpt;
use minos_propose::prompt::{History, HistoryEntry, Prompt, PromptInputs, SteerNote, assemble};
use proptest::prelude::*;
use serde_json::{Map, Value};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------------------------------

/// One maximize objective, `throughput_mb_s`, paired with the held-out `held_out_mb_s`, and one gate.
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

const PROGRAM: &str = "Make the log parser faster without changing its output.\n";
const PARSE: &str = "pub fn parse(input: &str) -> usize {\n    input.len()\n}\n";
const LIB: &str = "pub mod parse;\n";

/// Held-out values per prompt: two incumbent trials, two winner trials, and two candidate trials.
const HELD_OUT_SLOTS: usize = 6;

/// The fixed held-out value of the reference prompt.
const REFERENCE_HELD_OUT: f64 = 50.0;

fn exp(name: &str) -> ExperimentId {
    ExperimentId::baseline(&TreeId::of(name.as_bytes()))
}

/// One scorecard entry in the wire form.
fn metric(name: &str, value: Metric, held_out: bool) -> Value {
    let mut entry = Map::new();
    entry.insert("name".to_owned(), Value::from(name));
    entry.insert("value".to_owned(), Value::from(value.value()));
    entry.insert("held_out".to_owned(), Value::from(held_out));
    Value::Object(entry)
}

/// A scored outcome built from the scorecard wire form, its held-out entry marked `held_out: true`.
fn scored(throughput: f64, held_out: Metric) -> Result<Outcome, Box<dyn Error>> {
    let mut card = Map::new();
    card.insert("v".to_owned(), Value::from(1));
    card.insert(
        "metrics".to_owned(),
        Value::Array(vec![
            metric("throughput_mb_s", Metric::new(throughput)?, false),
            metric("identical_output", Metric::ONE, false),
            metric("held_out_mb_s", held_out, true),
        ]),
    );
    let bytes = serde_json::to_vec(&Value::Object(card))?;
    Ok(Outcome::Scored(Scorecard::parse_json(
        &bytes,
        &Limits::standard(),
    )?))
}

/// The value for held-out slot `index`.
fn slot(values: &[Metric], index: usize) -> Result<Metric, Box<dyn Error>> {
    Ok(*values.get(index).ok_or("missing held-out slot")?)
}

/// The prompt for generation 4 with `values` as every held-out value, in slot order.
fn prompt(values: &[Metric]) -> Result<Prompt, Box<dyn Error>> {
    let manifest = Manifest::parse_toml(MANIFEST)?;
    let mut parent = Tree::new();
    parent.insert(
        RelPath::try_from("src/parse.rs")?,
        PARSE.as_bytes().to_vec(),
    );
    parent.insert(RelPath::try_from("src/lib.rs")?, LIB.as_bytes().to_vec());

    let incumbent = vec![
        scored(100.0, slot(values, 0)?)?,
        scored(102.0, slot(values, 1)?)?,
    ];
    let entries = vec![
        HistoryEntry::new(
            1,
            exp("winner"),
            true,
            "Count bytes instead of chars.",
            vec![
                (0, scored(110.0, slot(values, 2)?)?),
                (1, scored(111.0, slot(values, 3)?)?),
            ],
            None,
        ),
        HistoryEntry::new(
            2,
            exp("candidate"),
            false,
            "Use a lookup table.",
            vec![
                (0, scored(95.0, slot(values, 4)?)?),
                (1, scored(97.0, slot(values, 5)?)?),
            ],
            None,
        ),
        HistoryEntry::new(
            2,
            exp("mismatch"),
            false,
            "Report the held-out metric too.",
            vec![(
                0,
                Outcome::Rejected(Reason::HeldOutMismatch {
                    metric: "held_out_mb_s".parse()?,
                }),
            )],
            Some(LogExcerpt::from_bytes(
                b"judge: wrote the scorecard\n",
                4096,
            )),
        ),
        HistoryEntry::new(
            3,
            exp("crash"),
            false,
            "Unroll the loop.",
            vec![(0, Outcome::Crashed { stage: Stage::Run })],
            Some(LogExcerpt::from_bytes(b"thread 'main' panicked\n", 4096)),
        ),
    ];
    let history = History::new(manifest.judge(), 4, &incumbent, entries);
    let notes = [SteerNote::new(0, "Keep the output identical.")];
    Ok(assemble(&PromptInputs {
        manifest: &manifest,
        program: PROGRAM.as_bytes(),
        parent: &parent,
        history: &history,
        notes: &notes,
        generation: 4,
        batch: 4,
    })?)
}

fn reference() -> Result<Prompt, Box<dyn Error>> {
    prompt(&[Metric::new(REFERENCE_HELD_OUT)?; HELD_OUT_SLOTS])
}

// ---------------------------------------------------------------------------------------------------------------
// Properties and tests
// ---------------------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn held_out_values_never_change_the_prompt(
        raw in prop::collection::vec(
            prop::num::f64::NORMAL | prop::num::f64::ZERO,
            HELD_OUT_SLOTS,
        ),
    ) {
        let values: Vec<Metric> = raw
            .iter()
            .map(|value| Metric::new(*value))
            .collect::<Result<_, _>>()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let reference = reference().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let changed = prompt(&values).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(changed.as_bytes(), reference.as_bytes());
        prop_assert_eq!(changed.hash(), reference.hash());
    }
}

#[test]
fn held_out_names_and_values_are_absent() -> TestResult {
    let prompt = prompt(&[Metric::new(123_456.789)?; HELD_OUT_SLOTS])?;
    let text = prompt.as_str();
    assert!(!text.contains("held_out_mb_s"));
    assert!(!text.contains("123456"));
    assert!(!text.contains("1.23456789e5"));
    // The rejection whose reason names the held-out metric is still shown, without the name.
    assert!(text.contains("| rejected: the judge's scorecard was invalid |"));
    // The visible objective and its deltas are shown.
    assert!(text.contains("- throughput_mb_s: maximize\n"));
    assert!(text.contains("| throughput_mb_s "));
    Ok(())
}
