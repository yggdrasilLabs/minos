//! The report (CORE-16; 03-CONTEXT D-15 to D-18): summary rows, every flag on a fixture that fires it and on a
//! clean one that does not, and the rendered, hashed review artifact.
//!
//! Fixtures are built from fixed bytes through public constructors only: a manifest from inline TOML, a ledger
//! folded from records, blobs keyed by `BlobId::of`, and the base surface as a `Tree`.

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use minos_core::diff::Diff;
use minos_core::diff::apply::Tree;
use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, ModelId, PromptHash, ReportHash,
    RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, ConfinementLayer, EXPECTED_KRUN_LAYERS, Event, Isolation, Origin,
    Record,
};
use minos_core::limits::Limits;
use minos_core::manifest::{Arch, JudgeVersion, Manifest, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::path::RelPath;
use minos_core::report::{
    DEPENDENCY_FILE_NAMES, Flag, MetricRole, Report, ReportError, ReportInput, SummaryRow,
    build_report, flags, summary,
};
use minos_core::state::{RunState, fold};
use minos_core::text::SafeText;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;
/// The gate every fixture judge declares.
const GATE: &str = "identical_output";
/// The objective every fixture judge declares.
const OBJECTIVE: &str = "throughput_mb_s";
/// The held-out metric paired with the objective.
const HELD_OUT: &str = "holdout_mb_s";
/// The writable surface file every fixture diff edits.
const PARSE: &str = "src/parse.rs";
/// The readable surface file.
const LIB: &str = "src/lib.rs";

/// The base `src/parse.rs`: line 2 is the one every fixture diff replaces.
const PARSE_RS: &str = "pub fn parse(input: &str) -> usize {\n    input.len()\n}\n";
/// The base `src/lib.rs`, holding a visible benchmark input on line 2.
const LIB_RS: &str = "pub mod parse;\npub const EXPECTED: &str = \"expected_output_42\";\n";
/// A `src/lib.rs` with no literal.
const LIB_PLAIN: &str = "pub mod parse;\n";

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest with `writable` as the writable set, one gate, and one maximize objective with a held-out
/// pair.
fn manifest_text(writable: &str) -> String {
    format!(
        r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = [{writable}]
readable = ["{LIB}"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "{GATE}"

[[judge.objective]]
metric = "{OBJECTIVE}"
goal = "maximize"
min_improvement = "5%"
held_out = "{HELD_OUT}"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#
    )
}

fn name(text: &str) -> Result<MetricName, Box<dyn Error>> {
    Ok(MetricName::from_str(text)?)
}

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text.to_owned())?)
}

fn metric(value: f64) -> Result<Metric, Box<dyn Error>> {
    Ok(Metric::new(value)?)
}

/// A scored trial that passes the gate, with objective `value` and held-out `held`.
fn scored(value: f64, held: f64) -> Result<Outcome, Box<dyn Error>> {
    Ok(Outcome::Scored(Scorecard::from_entries([
        (name(GATE)?, Metric::ONE, false),
        (name(OBJECTIVE)?, metric(value)?, false),
        (name(HELD_OUT)?, metric(held)?, true),
    ])?))
}

fn manifest_id() -> ManifestId {
    ManifestId::of(b"m")
}

fn base() -> TreeId {
    TreeId::of(b"t")
}

fn run() -> RunId {
    RunId::of(&manifest_id(), &base(), STARTED_AT)
}

fn baseline() -> ExperimentId {
    ExperimentId::baseline(&base())
}

fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

/// A krun backend with `isolation` and the given active layers.
fn backend_with(isolation: Isolation, layers: &[ConfinementLayer]) -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        isolation,
        layers.iter().copied().collect(),
        ImageDigest::of(b"root"),
        Some(ImageDigest::of(b"kernel")),
    )
}

/// A krun backend with hardware isolation and every expected layer.
fn hardware() -> BackendInfo {
    backend_with(Isolation::Hardware, &EXPECTED_KRUN_LAYERS)
}

/// A diff of `file` that replaces line `line` (`old`) with `new`, each given without its newline.
fn replace_line(file: &str, line: u32, old: &str, new: &str) -> String {
    format!(
        "diff --git a/{file} b/{file}\n--- a/{file}\n+++ b/{file}\n@@ -{line},1 +{line},1 @@\n-{old}\n+{new}\n"
    )
}

/// A diff of `src/parse.rs` that replaces its line 2 with `new`.
fn parse_line(new: &str) -> String {
    replace_line(PARSE, 2, "    input.len()", new)
}

/// One proposal in a fixture lineage: each step's parent is the step before it, the first step's the baseline.
struct Step {
    diff: String,
    rationale: String,
    trials: Vec<(f64, f64)>,
    backend: BackendInfo,
}

impl Step {
    /// A step with a hardware backend.
    fn new(diff: String, rationale: &str, trials: &[(f64, f64)]) -> Self {
        Self {
            diff,
            rationale: rationale.to_owned(),
            trials: trials.to_vec(),
            backend: hardware(),
        }
    }

    /// The same step judged on `backend`.
    fn on(mut self, backend: BackendInfo) -> Self {
        self.backend = backend;
        self
    }
}

/// A complete report fixture.
struct Fixture {
    manifest: Manifest,
    records: Vec<Record>,
    blobs: BTreeMap<BlobId, Vec<u8>>,
    surface: Tree,
    target: ExperimentId,
    judge: JudgeVersion,
}

/// Options that vary between fixtures; the defaults are the clean fixture's.
struct Spec<'a> {
    writable: &'a str,
    parse_rs: &'a str,
    lib_rs: &'a str,
    baseline: &'a [(f64, f64)],
    baseline_backend: BackendInfo,
}

impl Default for Spec<'_> {
    fn default() -> Self {
        Self {
            writable: "\"src/parse.rs\"",
            parse_rs: PARSE_RS,
            lib_rs: LIB_RS,
            baseline: &[(99.0, 49.0), (100.0, 50.0), (101.0, 51.0)],
            baseline_backend: hardware(),
        }
    }
}

fn rec(event: Event) -> Record {
    Record::v1(event)
}

impl Fixture {
    /// Builds a fixture from `spec` and the lineage `steps`; the last step is the target, or the baseline when
    /// there are none.
    fn build(spec: &Spec<'_>, steps: Vec<Step>) -> Result<Self, Box<dyn Error>> {
        let manifest = Manifest::parse_toml(&manifest_text(spec.writable))?;
        let judge = judge()?;
        let mut blobs = BTreeMap::new();
        let mut records = vec![rec(Event::RunStarted {
            run: run(),
            manifest: manifest_id(),
            base: base(),
            baseline: baseline(),
            started_at: STARTED_AT,
        })];
        for (trial, (value, held)) in (0_u32..).zip(spec.baseline) {
            records.push(rec(Event::Judged {
                exp: baseline(),
                judge,
                trial,
                outcome: scored(*value, *held)?,
                backend: spec.baseline_backend.clone(),
            }));
        }
        let mut parent = baseline();
        for (generation, step) in (1_u32..).zip(steps) {
            let diff = Diff::parse(&step.diff, manifest.writable(), &Limits::standard())?;
            let exp = ExperimentId::of(&base(), &diff);
            let diff_blob = BlobId::of(diff.as_bytes());
            let rationale_blob = BlobId::of(step.rationale.as_bytes());
            blobs.insert(diff_blob, diff.as_bytes().to_vec());
            blobs.insert(rationale_blob, step.rationale.into_bytes());
            records.push(rec(Event::Proposed {
                exp,
                parent,
                generation,
                tier: Tier::SingleShot,
                diff: diff_blob,
                rationale: rationale_blob,
                origin: Origin::Model {
                    model: ModelId::from_str("mid")?,
                    prompt: PromptHash::of(format!("prompt {generation}").as_bytes()),
                },
            }));
            for (trial, (value, held)) in (0_u32..).zip(&step.trials) {
                records.push(rec(Event::Judged {
                    exp,
                    judge,
                    trial,
                    outcome: scored(*value, *held)?,
                    backend: step.backend.clone(),
                }));
            }
            parent = exp;
        }
        let mut surface = Tree::new();
        surface.insert(path(PARSE)?, spec.parse_rs.as_bytes().to_vec());
        surface.insert(path(LIB)?, spec.lib_rs.as_bytes().to_vec());
        Ok(Self {
            manifest,
            records,
            blobs,
            surface,
            target: parent,
            judge,
        })
    }

    fn state(&self) -> Result<RunState, Box<dyn Error>> {
        Ok(fold(&self.records)?)
    }

    fn input<'a>(&'a self, state: &'a RunState) -> ReportInput<'a> {
        ReportInput::new(
            state,
            &self.manifest,
            &self.judge,
            &self.target,
            &self.blobs,
            &self.surface,
        )
    }

    fn flags(&self) -> Result<Vec<Flag>, Box<dyn Error>> {
        let state = self.state()?;
        Ok(flags(&self.input(&state))?)
    }

    fn summary(&self) -> Result<Vec<SummaryRow>, Box<dyn Error>> {
        let state = self.state()?;
        Ok(summary(&self.input(&state))?)
    }

    fn report(&self) -> Result<Report, Box<dyn Error>> {
        let state = self.state()?;
        Ok(build_report(&self.input(&state))?)
    }

    /// The report error for this fixture, or an error when the report builds.
    fn report_error(&self) -> Result<ReportError, Box<dyn Error>> {
        let state = self.state()?;
        match build_report(&self.input(&state)) {
            Ok(_) => Err("the report built".into()),
            Err(error) => Ok(error),
        }
    }
}

/// The clean fixture: two modest steps over a noisy baseline, hardware isolation, no suspicious text.
fn clean() -> Result<Fixture, Box<dyn Error>> {
    Fixture::build(
        &Spec::default(),
        vec![
            Step::new(
                parse_line("    input.trim().len()"),
                "Trim before counting.",
                &[(108.0, 54.0), (110.0, 55.0), (112.0, 56.0)],
            ),
            Step::new(
                parse_line("    input.as_bytes().len()"),
                "Count bytes directly.",
                &[(118.0, 59.0), (120.0, 60.0), (122.0, 61.0)],
            ),
        ],
    )
}

/// A one-step fixture over the default spec whose target replaces line 2 of `src/parse.rs` with `line`.
fn one_step(line: &str, trials: &[(f64, f64)]) -> Result<Fixture, Box<dyn Error>> {
    Fixture::build(
        &Spec::default(),
        vec![Step::new(parse_line(line), "One step.", trials)],
    )
}

fn row<'a>(rows: &'a [SummaryRow], metric: &str) -> Result<&'a SummaryRow, Box<dyn Error>> {
    let metric = name(metric)?;
    rows.iter()
        .find(|row| *row.metric() == metric)
        .ok_or_else(|| format!("no summary row for {metric}").into())
}

// ---------------------------------------------------------------------------------------------------------------
// The clean fixture
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn clean_fixture_raises_no_flag() -> TestResult {
    assert_eq!(clean()?.flags()?, Vec::new());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Anomalous jump (D-16)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn anomalous_jump_fires_over_the_baseline_spread() -> TestResult {
    let spec = Spec {
        baseline: &[(99.5, 49.5), (100.0, 50.0), (100.5, 50.5)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    input.as_bytes().len()"),
            "Jump.",
            &[(150.0, 75.0)],
        )],
    )?;
    assert!(fixture.flags()?.contains(&Flag::AnomalousJump {
        metric: name(OBJECTIVE)?
    }));
    Ok(())
}

#[test]
fn anomalous_jump_is_quiet_within_the_spread() -> TestResult {
    // Spread 2 (99 to 101), delta 9 over the baseline median 100: below 10 times the spread.
    let fixture = one_step("    input.as_bytes().len()", &[(109.0, 54.5)])?;
    let flags = fixture.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::AnomalousJump { .. }))
    );
    Ok(())
}

#[test]
fn anomalous_jump_falls_back_to_the_largest_earlier_delta() -> TestResult {
    // One baseline trial, so no noise floor: the earlier accepted delta is 2, and the target's delta is 50.
    let spec = Spec {
        baseline: &[(100.0, 50.0)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![
            Step::new(
                parse_line("    input.trim().len()"),
                "Small.",
                &[(102.0, 51.0)],
            ),
            Step::new(
                parse_line("    input.as_bytes().len()"),
                "Huge.",
                &[(152.0, 76.0)],
            ),
        ],
    )?;
    assert!(fixture.flags()?.contains(&Flag::AnomalousJump {
        metric: name(OBJECTIVE)?
    }));
    Ok(())
}

#[test]
fn anomalous_jump_fallback_is_quiet_for_a_comparable_delta() -> TestResult {
    let spec = Spec {
        baseline: &[(100.0, 50.0)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![
            Step::new(
                parse_line("    input.trim().len()"),
                "Small.",
                &[(102.0, 51.0)],
            ),
            Step::new(
                parse_line("    input.as_bytes().len()"),
                "Also small.",
                &[(110.0, 55.0)],
            ),
        ],
    )?;
    let flags = fixture.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::AnomalousJump { .. }))
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Held-out divergence (D-17)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn held_out_divergence_fires_when_the_held_out_pair_falls() -> TestResult {
    let fixture = one_step("    input.as_bytes().len()", &[(112.0, 45.0)])?;
    assert!(fixture.flags()?.contains(&Flag::HeldOutDivergence {
        metric: name(OBJECTIVE)?
    }));
    Ok(())
}

#[test]
fn held_out_divergence_is_quiet_when_both_improve() -> TestResult {
    let flags = clean()?.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::HeldOutDivergence { .. }))
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Literal match (D-15)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn literal_match_fires_on_a_visible_input() -> TestResult {
    let fixture = one_step(
        "    input.len() + \"expected_output_42\".len()",
        &[(110.0, 55.0)],
    )?;
    assert!(fixture.flags()?.contains(&Flag::LiteralMatch {
        path: path(PARSE)?,
        line: 2,
        literal: SafeText::inline("expected_output_42"),
    }));
    Ok(())
}

#[test]
fn literal_match_is_quiet_inside_the_hunks_own_old_lines() -> TestResult {
    // The literal is on the line the hunk replaces, and nowhere else in the surface.
    let spec = Spec {
        parse_rs: "pub fn parse(input: &str) -> usize {\n    \"expected_output_42\".len()\n}\n",
        lib_rs: LIB_PLAIN,
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            replace_line(
                PARSE,
                2,
                "    \"expected_output_42\".len()",
                "    \"expected_output_42\".len() + 1",
            ),
            "Off by one.",
            &[(110.0, 55.0)],
        )],
    )?;
    let flags = fixture.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::LiteralMatch { .. }))
    );
    Ok(())
}

#[test]
fn literal_match_ignores_short_literals() -> TestResult {
    // "short" is five bytes, below LITERAL_MIN_BYTES, even though lib.rs could hold it.
    let spec = Spec {
        lib_rs: "pub const S: &str = \"short\";\n",
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    input.len() + \"short\".len()"),
            "Short.",
            &[(110.0, 55.0)],
        )],
    )?;
    let flags = fixture.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::LiteralMatch { .. }))
    );
    Ok(())
}

#[test]
fn literal_match_finds_digit_runs() -> TestResult {
    let spec = Spec {
        lib_rs: "pub const N: f64 = 3.14159265;\n",
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    (input.len() as f64 * 3.14159265) as usize"),
            "Pi.",
            &[(110.0, 55.0)],
        )],
    )?;
    assert!(fixture.flags()?.contains(&Flag::LiteralMatch {
        path: path(PARSE)?,
        line: 2,
        literal: SafeText::inline("3.14159265"),
    }));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// New dependency, encoded blob, hidden characters
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn dependency_names_include_the_common_manifests() {
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "go.mod",
        "uv.lock",
    ] {
        assert!(DEPENDENCY_FILE_NAMES.contains(&file), "{file}");
    }
}

#[test]
fn new_dependency_fires_on_cargo_toml() -> TestResult {
    let spec = Spec {
        writable: "\"src/parse.rs\", \"Cargo.toml\"",
        ..Spec::default()
    };
    let diff = "diff --git a/Cargo.toml b/Cargo.toml\n--- a/Cargo.toml\n+++ b/Cargo.toml\n\
                @@ -7,1 +7,2 @@\n [dependencies]\n+regex = \"1\"\n";
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(diff.to_owned(), "Add regex.", &[(110.0, 55.0)])],
    )?;
    assert!(fixture.flags()?.contains(&Flag::NewDependency {
        path: path("Cargo.toml")?
    }));
    Ok(())
}

#[test]
fn new_dependency_is_quiet_for_source_files() -> TestResult {
    let flags = clean()?.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::NewDependency { .. }))
    );
    Ok(())
}

#[test]
fn encoded_blob_fires_on_a_long_base64_run() -> TestResult {
    let blob = "QUJD".repeat(20);
    let fixture = one_step(
        &format!("    input.len() + \"{blob}\".len()"),
        &[(110.0, 55.0)],
    )?;
    assert!(fixture.flags()?.contains(&Flag::EncodedBlob {
        path: path(PARSE)?,
        line: 2,
    }));
    Ok(())
}

#[test]
fn encoded_blob_is_quiet_for_ordinary_code() -> TestResult {
    let flags = clean()?.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::EncodedBlob { .. }))
    );
    Ok(())
}

#[test]
fn hidden_characters_fire_on_a_bidi_override() -> TestResult {
    let fixture = one_step("    input.len() // \u{202e}note", &[(110.0, 55.0)])?;
    assert!(fixture.flags()?.contains(&Flag::HiddenCharacters {
        path: path(PARSE)?,
        line: 2,
    }));
    Ok(())
}

#[test]
fn hidden_characters_are_quiet_for_plain_text() -> TestResult {
    let flags = clean()?.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::HiddenCharacters { .. }))
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Weak isolation (D-18)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn weak_isolation_fires_anywhere_in_the_lineage() -> TestResult {
    let fixture = Fixture::build(
        &Spec::default(),
        vec![
            Step::new(
                parse_line("    input.trim().len()"),
                "Weak step.",
                &[(108.0, 54.0)],
            )
            .on(backend_with(Isolation::WeakProcess, &[])),
            Step::new(
                parse_line("    input.as_bytes().len()"),
                "Hardware step.",
                &[(118.0, 59.0)],
            ),
        ],
    )?;
    assert!(fixture.flags()?.contains(&Flag::WeakIsolation {
        level: Isolation::WeakProcess
    }));
    Ok(())
}

#[test]
fn weak_isolation_is_quiet_for_hardware() -> TestResult {
    let flags = clean()?.flags()?;
    assert!(
        !flags
            .iter()
            .any(|flag| matches!(flag, Flag::WeakIsolation { .. }))
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Flag order
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn flags_are_sorted() -> TestResult {
    let blob = "QUJD".repeat(20);
    let fixture = Fixture::build(
        &Spec::default(),
        vec![
            Step::new(
                parse_line(&format!(
                    "    input.len() + \"expected_output_42\".len() + \"{blob}\".len() // \u{200b}"
                )),
                "Everything.",
                &[(112.0, 45.0)],
            )
            .on(backend_with(Isolation::WeakProcess, &[])),
        ],
    )?;
    let flags = fixture.flags()?;
    assert!(flags.len() >= 4, "{flags:?}");
    assert!(flags.is_sorted());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Summary rows (D-17)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn summary_has_one_row_per_objective_gate_and_held_out_metric() -> TestResult {
    let rows = clean()?.summary()?;
    assert_eq!(rows.len(), 3);
    assert_eq!(row(&rows, OBJECTIVE)?.role(), MetricRole::Objective);
    assert_eq!(row(&rows, GATE)?.role(), MetricRole::Gate);
    assert_eq!(row(&rows, HELD_OUT)?.role(), MetricRole::HeldOut);
    Ok(())
}

#[test]
fn summary_objective_row_carries_the_d17_fields() -> TestResult {
    let rows = clean()?.summary()?;
    let objective = row(&rows, OBJECTIVE)?;
    assert_eq!(objective.baseline_median(), metric(100.0)?);
    assert_eq!(objective.baseline_min(), metric(99.0)?);
    assert_eq!(objective.baseline_max(), metric(101.0)?);
    assert_eq!(objective.baseline_trials(), 3);
    assert_eq!(objective.winner_median(), metric(120.0)?);
    assert_eq!(objective.winner_trials(), 3);
    assert_eq!(objective.delta_abs(), Some(20.0));
    assert_eq!(objective.delta_pct(), Some(20.0));
    assert_eq!(objective.clears_threshold(), Some(true));
    assert_eq!(objective.clears_noise(), Some(true));
    let gate = row(&rows, GATE)?;
    assert_eq!(gate.clears_threshold(), None);
    assert_eq!(gate.delta_abs(), Some(0.0));
    assert_eq!(row(&rows, HELD_OUT)?.clears_threshold(), None);
    Ok(())
}

#[test]
fn summary_threshold_uses_min_improvement() -> TestResult {
    // 104 over a median of 100 is a 4% gain, below the 5% threshold.
    let rows = one_step("    input.as_bytes().len()", &[(104.0, 52.0)])?.summary()?;
    assert_eq!(row(&rows, OBJECTIVE)?.clears_threshold(), Some(false));
    Ok(())
}

#[test]
fn summary_percent_is_absent_at_a_zero_baseline() -> TestResult {
    let spec = Spec {
        baseline: &[(0.0, 0.0), (0.0, 0.0)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    input.as_bytes().len()"),
            "From zero.",
            &[(1.0, 1.0)],
        )],
    )?;
    let rows = fixture.summary()?;
    let objective = row(&rows, OBJECTIVE)?;
    assert_eq!(objective.delta_abs(), Some(1.0));
    assert_eq!(objective.delta_pct(), None);
    Ok(())
}

#[test]
fn summary_noise_is_absent_without_a_spread() -> TestResult {
    // Two equal baseline trials: a spread of zero.
    let spec = Spec {
        baseline: &[(100.0, 50.0), (100.0, 50.0)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    input.as_bytes().len()"),
            "Flat.",
            &[(110.0, 55.0)],
        )],
    )?;
    assert_eq!(row(&fixture.summary()?, OBJECTIVE)?.clears_noise(), None);
    Ok(())
}

#[test]
fn summary_noise_is_absent_with_one_baseline_trial() -> TestResult {
    let spec = Spec {
        baseline: &[(100.0, 50.0)],
        ..Spec::default()
    };
    let fixture = Fixture::build(
        &spec,
        vec![Step::new(
            parse_line("    input.as_bytes().len()"),
            "Single.",
            &[(110.0, 55.0)],
        )],
    )?;
    assert_eq!(row(&fixture.summary()?, OBJECTIVE)?.clears_noise(), None);
    Ok(())
}

#[test]
fn summary_noise_is_false_inside_the_noise_multiple() -> TestResult {
    // Spread 2; a delta of 3 is not above NOISE_WARN_MULTIPLE (2) times the spread.
    let rows = one_step("    input.as_bytes().len()", &[(103.0, 51.5)])?.summary()?;
    assert_eq!(row(&rows, OBJECTIVE)?.clears_noise(), Some(false));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The rendered report
// ---------------------------------------------------------------------------------------------------------------

/// The section headings, in report order.
const SECTIONS: [&str; 6] = [
    "minos report v1",
    "summary",
    "flags",
    "provenance",
    "lineage",
    "diff",
];

/// The fixture for the weak-isolation golden report: a weak first step without the dedicated user or rlimits,
/// then a hardware step without the dedicated user.
fn weak() -> Result<Fixture, Box<dyn Error>> {
    Fixture::build(
        &Spec::default(),
        vec![
            Step::new(
                parse_line("    input.trim().len()"),
                "Trim before counting.",
                &[(108.0, 54.0), (110.0, 55.0), (112.0, 56.0)],
            )
            .on(backend_with(
                Isolation::WeakProcess,
                &[ConfinementLayer::Seatbelt],
            )),
            Step::new(
                parse_line("    input.as_bytes().len()"),
                "Count bytes directly.",
                &[(118.0, 59.0), (120.0, 60.0), (122.0, 61.0)],
            )
            .on(backend_with(
                Isolation::Hardware,
                &[ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits],
            )),
        ],
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

fn fail(error: impl std::fmt::Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

#[test]
fn sections_are_in_order() -> TestResult {
    let report = clean()?.report()?;
    let lines: Vec<&str> = report.text().lines().collect();
    let mut last = None;
    for heading in SECTIONS {
        let at = lines
            .iter()
            .position(|line| *line == heading)
            .ok_or_else(|| format!("no section {heading}"))?;
        assert!(
            last.is_none_or(|before| at > before),
            "{heading} is out of order"
        );
        last = Some(at);
    }
    Ok(())
}

#[test]
fn clean_report_flags_section_says_none() -> TestResult {
    let report = clean()?.report()?;
    assert!(report.flags().is_empty());
    assert!(report.text().contains("\nflags\n  none\n"));
    Ok(())
}

#[test]
fn report_is_lf_only_and_ends_with_a_newline() -> TestResult {
    for fixture in [clean()?, weak()?] {
        let report = fixture.report()?;
        assert!(!report.text().contains('\r'));
        assert!(report.text().ends_with('\n'));
    }
    Ok(())
}

#[test]
fn hostile_rationale_renders_inert() -> TestResult {
    let fixture = Fixture::build(
        &Spec::default(),
        vec![Step::new(
            parse_line("    input.as_bytes().len() // \u{1b}[31mred\u{1b}[0m"),
            "Faster.\u{1b}]52;c;bad\u{7} then \u{202e}reversed\nsecond line",
            &[(110.0, 55.0)],
        )],
    )?;
    let text = fixture.report()?.text().to_owned();
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{7}'));
    assert!(!text.contains('\u{202e}'));
    assert!(text.contains("<U+202E>reversed<U+000A>second line"));
    assert!(text.contains("]52;c;bad"));
    Ok(())
}

#[test]
fn long_rationale_is_truncated_at_a_char_boundary() -> TestResult {
    let rationale = "\u{e9}".repeat(300);
    let fixture = Fixture::build(
        &Spec::default(),
        vec![Step::new(
            parse_line("    input.as_bytes().len()"),
            &rationale,
            &[(110.0, 55.0)],
        )],
    )?;
    let text = fixture.report()?.text().to_owned();
    assert!(text.contains(&format!("{} [truncated]", "\u{e9}".repeat(100))));
    assert!(!text.contains(&"\u{e9}".repeat(101)));
    Ok(())
}

#[test]
fn hash_covers_the_rendered_bytes() -> TestResult {
    let report = clean()?.report()?;
    assert_eq!(*report.hash(), ReportHash::of(report.text().as_bytes()));
    Ok(())
}

#[test]
fn same_inputs_give_the_same_report() -> TestResult {
    let first = clean()?.report()?;
    let second = clean()?.report()?;
    assert_eq!(first.text(), second.text());
    assert_eq!(first.hash(), second.hash());
    assert_eq!(first.summary(), second.summary());
    Ok(())
}

#[test]
fn different_reports_have_different_hashes() -> TestResult {
    assert_ne!(clean()?.report()?.hash(), weak()?.report()?.hash());
    Ok(())
}

#[test]
fn a_report_on_the_baseline_is_refused() -> TestResult {
    let fixture = Fixture::build(&Spec::default(), Vec::new())?;
    assert_eq!(fixture.report_error()?, ReportError::Baseline);
    Ok(())
}

#[test]
fn a_report_on_an_unknown_experiment_is_refused() -> TestResult {
    let mut fixture = clean()?;
    fixture.target = ExperimentId::from_str(&"cd".repeat(32))?;
    assert_eq!(
        fixture.report_error()?,
        ReportError::NotInLedger {
            exp: fixture.target
        }
    );
    Ok(())
}

#[test]
fn a_missing_diff_blob_is_refused() -> TestResult {
    let mut fixture = clean()?;
    let state = fixture.state()?;
    let diff = *state
        .proposal(&fixture.target)
        .ok_or("no target proposal")?
        .diff();
    fixture.blobs.remove(&diff);
    assert_eq!(
        fixture.report_error()?,
        ReportError::MissingBlob { blob: diff }
    );
    Ok(())
}

#[test]
fn a_blob_that_does_not_match_its_id_is_refused() -> TestResult {
    let mut fixture = clean()?;
    let state = fixture.state()?;
    let rationale = *state
        .proposal(&fixture.target)
        .ok_or("no target proposal")?
        .rationale();
    fixture.blobs.insert(rationale, b"tampered".to_vec());
    assert_eq!(
        fixture.report_error()?,
        ReportError::MissingBlob { blob: rationale }
    );
    Ok(())
}

#[test]
fn weak_isolation_report_shows_the_weakest_level_and_missing_layers() -> TestResult {
    let report = weak()?.report()?;
    let text = report.text();
    assert!(text.contains("  isolation: weak (process only)\n"));
    assert!(text.contains("  reduced hardening: dedicated_user, rlimits\n"));
    assert!(report.flags().contains(&Flag::WeakIsolation {
        level: Isolation::WeakProcess
    }));
    Ok(())
}

#[test]
fn clean_report_shows_hardware_isolation() -> TestResult {
    let text = clean()?.report()?.text().to_owned();
    assert!(text.contains("  isolation: hardware virtualization\n"));
    assert!(text.contains("  reduced hardening: none\n"));
    Ok(())
}

#[test]
fn golden_reports() -> TestResult {
    insta::assert_snapshot!("clean_report", clean()?.report()?.text());
    insta::assert_snapshot!("weak_isolation_report", weak()?.report()?.text());
    Ok(())
}

#[test]
fn golden_report_bytes_pinned() -> TestResult {
    // insta trims trailing whitespace and normalizes line endings when it compares (03-RESEARCH Pitfall 4), so
    // this pins the exact bytes of both golden texts, concatenated in this order. The pin was computed outside the
    // code under test, from the accepted snapshot bodies (the text after each file's second `---` line), each of
    // which insta stores without the report's final newline:
    //
    //   cd crates/minos-core/tests/snapshots
    //   for n in clean_report weak_isolation_report; do
    //     awk 'n >= 2 { print } /^---$/ { n++ }' "report__$n.snap"
    //   done | shasum -a 256
    let mut all = String::new();
    all.push_str(clean()?.report()?.text());
    all.push_str(weak()?.report()?.text());
    assert_eq!(sha256_hex(all.as_bytes()), GOLDEN_REPORT_SHA256);
    Ok(())
}

/// See `golden_report_bytes_pinned`.
const GOLDEN_REPORT_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

proptest! {
    #[test]
    fn report_is_order_insensitive(seed in any::<u64>()) {
        let fixture = clean().map_err(fail)?;
        let expected = fixture.report().map_err(fail)?;
        let (first, rest) = fixture.records.split_first().ok_or_else(|| fail("no records"))?;
        // A deterministic shuffle of every record after RunStarted, keyed by the seed and each record's position.
        let mut keyed: Vec<(String, Record)> = rest
            .iter()
            .enumerate()
            .map(|(index, record)| (sha256_hex(format!("{seed}/{index}").as_bytes()), record.clone()))
            .collect();
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        let mut shuffled = vec![first.clone()];
        shuffled.extend(keyed.into_iter().map(|(_, record)| record));
        let state = fold(&shuffled).map_err(fail)?;
        let report = build_report(&fixture.input(&state)).map_err(fail)?;
        prop_assert_eq!(report.text(), expected.text());
        prop_assert_eq!(report.hash(), expected.hash());
    }
}
