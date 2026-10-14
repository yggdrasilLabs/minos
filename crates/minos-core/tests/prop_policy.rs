//! Policy (CORE-10, D-11, D-12): the scorecard gate check, the lower-median trial reduction, the improvement
//! test, greedy selection, the generation conclusion, and the noise check.
//!
//! Every decision is a pure function of recorded results. The properties shuffle candidates, trials, and ledger
//! records and assert the decision never changes. Ledgers are built from public constructors and folded.

use std::error::Error;
use std::fmt::Display;
use std::str::FromStr;

use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, ModelId, PromptHash, RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation,
    Origin, Record,
};
use minos_core::manifest::{Arch, JudgeVersion, Manifest, Objective, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::policy::{
    NoiseVerdict, PolicyError, Selection, check_scorecard, conclude, improves, lower_median,
    noise_check, noise_floor, reduce_trials, select_greedy,
};
use minos_core::state::{RunState, fold};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;
/// The gate every fixture judge declares.
const GATE: &str = "identical_output";
/// The objective every fixture judge declares.
const OBJECTIVE: &str = "throughput_mb_s";

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest with one gate and one objective of `goal` and `min_improvement`, optionally paired with a
/// held-out metric.
fn manifest_text(goal: &str, min_improvement: &str, held_out: Option<&str>) -> String {
    let held = held_out.map_or_else(String::new, |name| format!("held_out = \"{name}\"\n"));
    format!(
        r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

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
goal = "{goal}"
min_improvement = "{min_improvement}"
{held}
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

fn manifest(goal: &str, min_improvement: &str) -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(&manifest_text(
        goal,
        min_improvement,
        None,
    ))?)
}

fn objective(goal: &str, min_improvement: &str) -> Result<Objective, Box<dyn Error>> {
    Ok(manifest(goal, min_improvement)?.objective().clone())
}

fn name(text: &str) -> Result<MetricName, Box<dyn Error>> {
    Ok(MetricName::from_str(text)?)
}

fn metric(value: f64) -> Result<Metric, Box<dyn Error>> {
    Ok(Metric::new(value)?)
}

/// A scorecard from `(name, value, held_out)` entries.
fn card(entries: &[(&str, f64, bool)]) -> Result<Scorecard, Box<dyn Error>> {
    let entries = entries
        .iter()
        .map(|(text, value, held)| Ok((name(text)?, metric(*value)?, *held)))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(Scorecard::from_entries(entries)?)
}

/// A scored trial that passes the gate with objective value `value`.
fn scored(value: f64) -> Result<Outcome, Box<dyn Error>> {
    Ok(Outcome::Scored(card(&[
        (GATE, 1.0, false),
        (OBJECTIVE, value, false),
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

/// A candidate experiment ID from a distinct number; IDs order as their numbers do.
fn exp(n: u64) -> Result<ExperimentId, Box<dyn Error>> {
    Ok(ExperimentId::from_str(&format!("{n:064x}"))?)
}

fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

fn other_judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"cd".repeat(32))?)
}

fn backend() -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        Isolation::Hardware,
        EXPECTED_KRUN_LAYERS.iter().copied().collect(),
        ImageDigest::of(b"root"),
        Some(ImageDigest::of(b"kernel")),
    )
}

fn rec(event: Event) -> Record {
    Record::v1(event)
}

fn run_started() -> Record {
    rec(Event::RunStarted {
        run: run(),
        manifest: manifest_id(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
    })
}

fn proposed(
    exp: ExperimentId,
    parent: ExperimentId,
    generation: u32,
) -> Result<Record, Box<dyn Error>> {
    Ok(rec(Event::Proposed {
        exp,
        parent,
        generation,
        tier: Tier::SingleShot,
        diff: BlobId::of(exp.as_bytes()),
        rationale: BlobId::of(b"rationale"),
        origin: Origin::Model {
            model: ModelId::from_str("mid")?,
            prompt: PromptHash::of(b"prompt"),
        },
    }))
}

fn judged_by(judge: JudgeVersion, exp: ExperimentId, trial: u32, outcome: Outcome) -> Record {
    rec(Event::Judged {
        exp,
        judge,
        trial,
        outcome,
        backend: backend(),
    })
}

fn judged(exp: ExperimentId, trial: u32, outcome: Outcome) -> Result<Record, Box<dyn Error>> {
    Ok(judged_by(judge()?, exp, trial, outcome))
}

fn selected(generation: u32, kind: DecisionKind, proposed: u32) -> Record {
    rec(Event::Selected {
        generation,
        decision: Decision::new(kind, proposed, 0),
    })
}

/// `RunStarted` plus the baseline's trials 100, 101, and 99 (lower median 100).
fn calibrated() -> Result<Vec<Record>, Box<dyn Error>> {
    Ok(vec![
        run_started(),
        judged(baseline(), 0, scored(100.0)?)?,
        judged(baseline(), 1, scored(101.0)?)?,
        judged(baseline(), 2, scored(99.0)?)?,
    ])
}

/// The baseline trial records with these objective values.
fn baseline_trials(values: &[f64]) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut records = vec![run_started()];
    for (trial, value) in (0_u32..).zip(values) {
        records.push(judged(baseline(), trial, scored(*value)?)?);
    }
    Ok(records)
}

fn state(records: &[Record]) -> Result<RunState, Box<dyn Error>> {
    Ok(fold(records)?)
}

fn fail(error: impl Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

// ---------------------------------------------------------------------------------------------------------------
// check_scorecard
// ---------------------------------------------------------------------------------------------------------------

fn rejected(manifest: &Manifest, card: &Scorecard) -> Result<Reason, Box<dyn Error>> {
    match check_scorecard(manifest.judge(), card) {
        Ok(()) => Err("the scorecard passed".into()),
        Err(reason) => Ok(reason),
    }
}

#[test]
fn check_scorecard_passes_a_gate_of_exactly_one() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    check_scorecard(
        manifest.judge(),
        &card(&[(GATE, 1.0, false), (OBJECTIVE, 3.0, false)])?,
    )
    .map_err(|reason| reason.to_string())?;
    Ok(())
}

#[test]
fn check_scorecard_fails_a_gate_that_is_not_one() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    for value in [0.0, 0.999_999, 1.000_001, 2.0, -1.0] {
        let reason = rejected(
            &manifest,
            &card(&[(GATE, value, false), (OBJECTIVE, 3.0, false)])?,
        )?;
        assert_eq!(reason, Reason::GateFailed { gate: name(GATE)? });
    }
    Ok(())
}

#[test]
fn check_scorecard_rejects_a_missing_gate() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(&manifest, &card(&[(OBJECTIVE, 3.0, false)])?)?;
    assert_eq!(
        reason,
        Reason::MissingMetric {
            metric: name(GATE)?
        }
    );
    Ok(())
}

#[test]
fn check_scorecard_checks_gates_before_objectives() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(&manifest, &card(&[(GATE, 0.0, false)])?)?;
    assert_eq!(reason, Reason::GateFailed { gate: name(GATE)? });
    Ok(())
}

#[test]
fn check_scorecard_rejects_a_missing_objective() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(&manifest, &card(&[(GATE, 1.0, false)])?)?;
    assert_eq!(
        reason,
        Reason::MissingMetric {
            metric: name(OBJECTIVE)?
        }
    );
    Ok(())
}

#[test]
fn check_scorecard_rejects_an_undeclared_metric() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(
        &manifest,
        &card(&[
            (GATE, 1.0, false),
            (OBJECTIVE, 3.0, false),
            ("latency_ms", 1.0, false),
        ])?,
    )?;
    assert_eq!(
        reason,
        Reason::UndeclaredMetric {
            metric: name("latency_ms")?
        }
    );
    Ok(())
}

#[test]
fn check_scorecard_rejects_an_objective_marked_held_out() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(
        &manifest,
        &card(&[(GATE, 1.0, false), (OBJECTIVE, 3.0, true)])?,
    )?;
    assert_eq!(
        reason,
        Reason::HeldOutMismatch {
            metric: name(OBJECTIVE)?
        }
    );
    Ok(())
}

#[test]
fn check_scorecard_rejects_a_gate_marked_held_out() -> TestResult {
    let manifest = manifest("maximize", "5%")?;
    let reason = rejected(
        &manifest,
        &card(&[(GATE, 1.0, true), (OBJECTIVE, 3.0, false)])?,
    )?;
    assert_eq!(
        reason,
        Reason::HeldOutMismatch {
            metric: name(GATE)?
        }
    );
    Ok(())
}

#[test]
fn check_scorecard_requires_held_out_pairings_marked_held_out() -> TestResult {
    let manifest = Manifest::parse_toml(&manifest_text("maximize", "5%", Some("hidden_mb_s")))?;
    check_scorecard(
        manifest.judge(),
        &card(&[
            (GATE, 1.0, false),
            (OBJECTIVE, 3.0, false),
            ("hidden_mb_s", 2.0, true),
        ])?,
    )
    .map_err(|reason| reason.to_string())?;
    let unmarked = rejected(
        &manifest,
        &card(&[
            (GATE, 1.0, false),
            (OBJECTIVE, 3.0, false),
            ("hidden_mb_s", 2.0, false),
        ])?,
    )?;
    assert_eq!(
        unmarked,
        Reason::HeldOutMismatch {
            metric: name("hidden_mb_s")?
        }
    );
    let missing = rejected(
        &manifest,
        &card(&[(GATE, 1.0, false), (OBJECTIVE, 3.0, false)])?,
    )?;
    assert_eq!(
        missing,
        Reason::MissingMetric {
            metric: name("hidden_mb_s")?
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// lower_median and reduce_trials
// ---------------------------------------------------------------------------------------------------------------

fn metrics(values: &[f64]) -> Result<Vec<Metric>, Box<dyn Error>> {
    values.iter().map(|value| metric(*value)).collect()
}

#[test]
fn lower_median_takes_the_lower_middle() -> TestResult {
    assert_eq!(lower_median(&metrics(&[3.0])?), Some(metric(3.0)?));
    assert_eq!(lower_median(&metrics(&[1.0, 2.0])?), Some(metric(1.0)?));
    assert_eq!(lower_median(&metrics(&[2.0, 1.0])?), Some(metric(1.0)?));
    assert_eq!(
        lower_median(&metrics(&[5.0, 1.0, 3.0])?),
        Some(metric(3.0)?)
    );
    assert_eq!(
        lower_median(&metrics(&[4.0, 1.0, 3.0, 2.0])?),
        Some(metric(2.0)?)
    );
    assert_eq!(lower_median(&[]), None);
    Ok(())
}

#[test]
fn reduce_trials_takes_the_lower_median_per_metric() -> TestResult {
    let trials = [scored(1.0)?, scored(3.0)?, scored(2.0)?, scored(4.0)?];
    let reduced = reduce_trials(&trials).ok_or("no reduced score")?;
    assert_eq!(reduced.trials(), 4);
    assert_eq!(reduced.get(&name(OBJECTIVE)?), Some(metric(2.0)?));
    assert_eq!(reduced.get(&name(GATE)?), Some(Metric::ONE));
    assert_eq!(reduced.metrics().len(), 2);
    Ok(())
}

#[test]
fn reduce_trials_is_none_for_any_unscored_trial() -> TestResult {
    let unscored = [
        Outcome::Crashed { stage: Stage::Run },
        Outcome::TimedOut {
            stage: Stage::Judge,
        },
        Outcome::Rejected(Reason::BuildFailed),
    ];
    for bad in unscored {
        let trials = [scored(1.0)?, bad, scored(2.0)?];
        assert!(reduce_trials(&trials).is_none());
    }
    assert!(reduce_trials(&[]).is_none());
    Ok(())
}

#[test]
fn reduce_trials_is_none_when_a_metric_is_absent_from_a_trial() -> TestResult {
    let partial = Outcome::Scored(card(&[(GATE, 1.0, false)])?);
    assert!(reduce_trials(&[scored(1.0)?, partial.clone()]).is_none());
    assert!(reduce_trials(&[partial, scored(1.0)?]).is_none());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// improves
// ---------------------------------------------------------------------------------------------------------------

fn improves_at(
    candidate: f64,
    incumbent: f64,
    objective: &Objective,
) -> Result<bool, Box<dyn Error>> {
    Ok(improves(metric(candidate)?, metric(incumbent)?, objective))
}

#[test]
fn improves_needs_the_relative_threshold() -> TestResult {
    let max = objective("maximize", "5%")?;
    assert!(improves_at(105.0, 100.0, &max)?);
    assert!(improves_at(200.0, 100.0, &max)?);
    assert!(!improves_at(104.999, 100.0, &max)?);
    assert!(!improves_at(100.0, 100.0, &max)?);
    assert!(!improves_at(90.0, 100.0, &max)?);
    let min = objective("minimize", "5%")?;
    assert!(improves_at(95.0, 100.0, &min)?);
    assert!(!improves_at(95.001, 100.0, &min)?);
    assert!(!improves_at(105.0, 100.0, &min)?);
    Ok(())
}

#[test]
fn improves_at_a_zero_incumbent_needs_strictly_better() -> TestResult {
    let max = objective("maximize", "5%")?;
    assert!(!improves_at(0.0, 0.0, &max)?);
    assert!(improves_at(1e-300, 0.0, &max)?);
    assert!(!improves_at(-1e-300, 0.0, &max)?);
    let min = objective("minimize", "5%")?;
    assert!(!improves_at(0.0, 0.0, &min)?);
    assert!(improves_at(-1e-300, 0.0, &min)?);
    Ok(())
}

#[test]
fn improves_at_a_negative_incumbent_uses_its_magnitude() -> TestResult {
    let max = objective("maximize", "5%")?;
    assert!(improves_at(-95.0, -100.0, &max)?);
    assert!(!improves_at(-95.5, -100.0, &max)?);
    assert!(!improves_at(-105.0, -100.0, &max)?);
    let min = objective("minimize", "5%")?;
    assert!(improves_at(-105.0, -100.0, &min)?);
    assert!(!improves_at(-104.0, -100.0, &min)?);
    Ok(())
}

#[test]
fn improves_with_an_absolute_threshold() -> TestResult {
    let max = objective("maximize", "0.5")?;
    assert!(improves_at(10.5, 10.0, &max)?);
    assert!(!improves_at(10.4, 10.0, &max)?);
    let min = objective("minimize", "0.5")?;
    assert!(improves_at(9.5, 10.0, &min)?);
    assert!(!improves_at(9.6, 10.0, &min)?);
    Ok(())
}

#[test]
fn improves_never_panics_near_the_largest_finite_value() -> TestResult {
    let max = objective("maximize", "5%")?;
    assert!(!improves_at(f64::MAX, f64::MAX, &max)?);
    // The bound 0.99 MAX + 5% overflows to infinity, so nothing improves.
    assert!(!improves_at(f64::MAX, f64::MAX * 0.99, &max)?);
    let min = objective("minimize", "5%")?;
    assert!(!improves_at(-f64::MAX, -f64::MAX * 0.99, &min)?);
    let absolute = objective("maximize", "0.5")?;
    assert!(!improves_at(f64::MAX, f64::MAX, &absolute)?);
    let percent = objective("maximize", "1000%")?;
    assert!(!improves_at(f64::MAX, f64::MAX / 4.0, &percent)?);
    Ok(())
}

proptest! {
    #[test]
    fn improves_implies_strictly_better(
        candidate in -1e300_f64..1e300,
        incumbent in -1e300_f64..1e300,
        maximize in any::<bool>(),
        relative in any::<bool>(),
    ) {
        let goal = if maximize { "maximize" } else { "minimize" };
        let threshold = if relative { "5%" } else { "0.5" };
        let objective = objective(goal, threshold).map_err(fail)?;
        let (c, i) = (Metric::new(candidate).map_err(fail)?, Metric::new(incumbent).map_err(fail)?);
        prop_assert!(!improves(i, i, &objective));
        if improves(c, i, &objective) {
            if maximize {
                prop_assert!(c > i);
            } else {
                prop_assert!(c < i);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// select_greedy
// ---------------------------------------------------------------------------------------------------------------

fn candidates(pairs: &[(u64, i32)]) -> Result<Vec<(ExperimentId, Metric)>, Box<dyn Error>> {
    pairs
        .iter()
        .map(|(n, value)| Ok((exp(*n)?, metric(f64::from(*value))?)))
        .collect()
}

#[test]
fn select_greedy_keeps_with_no_candidates() -> TestResult {
    let max = objective("maximize", "5%")?;
    assert_eq!(select_greedy(&[], metric(100.0)?, &max), Selection::Kept);
    Ok(())
}

#[test]
fn select_greedy_breaks_ties_on_the_smallest_id() -> TestResult {
    let max = objective("maximize", "5%")?;
    let list = candidates(&[(3, 110), (1, 110), (2, 109)])?;
    assert_eq!(
        select_greedy(&list, metric(100.0)?, &max),
        Selection::Improved(exp(1)?)
    );
    Ok(())
}

#[test]
fn select_greedy_keeps_when_the_best_misses_the_threshold() -> TestResult {
    let max = objective("maximize", "5%")?;
    let list = candidates(&[(1, 104), (2, 90)])?;
    assert_eq!(select_greedy(&list, metric(100.0)?, &max), Selection::Kept);
    Ok(())
}

#[test]
fn select_greedy_minimizes() -> TestResult {
    let min = objective("minimize", "5%")?;
    let list = candidates(&[(1, 94), (2, 80), (3, 120)])?;
    assert_eq!(
        select_greedy(&list, metric(100.0)?, &min),
        Selection::Improved(exp(2)?)
    );
    Ok(())
}

fn arb_candidates() -> impl Strategy<Value = Vec<(u64, i32)>> {
    proptest::collection::vec((0_u64..16, -4_i32..5), 0..8)
}

proptest! {
    #[test]
    fn select_greedy_is_permutation_invariant(
        (original, shuffled) in arb_candidates()
            .prop_flat_map(|list| (Just(list.clone()), Just(list).prop_shuffle())),
        incumbent in -4_i32..5,
        maximize in any::<bool>(),
    ) {
        let goal = if maximize { "maximize" } else { "minimize" };
        let objective = objective(goal, "1").map_err(fail)?;
        let incumbent = Metric::new(f64::from(incumbent)).map_err(fail)?;
        let original = candidates(&original).map_err(fail)?;
        let shuffled = candidates(&shuffled).map_err(fail)?;
        let chosen = select_greedy(&original, incumbent, &objective);
        prop_assert_eq!(chosen, select_greedy(&shuffled, incumbent, &objective));
        if let Selection::Improved(winner) = chosen {
            // An ID may repeat with several values; the winner's is its best one.
            let values = original
                .iter()
                .filter(|(id, _)| *id == winner)
                .map(|(_, value)| *value);
            let best = if maximize { values.max() } else { values.min() }
                .ok_or_else(|| fail("the winner is not a candidate"))?;
            prop_assert!(improves(best, incumbent, &objective));
            for (id, value) in &original {
                let better = if maximize { *value > best } else { *value < best };
                prop_assert!(!better, "a candidate beats the winner");
                prop_assert!(*value != best || *id >= winner, "a smaller ID ties the winner");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// conclude
// ---------------------------------------------------------------------------------------------------------------

fn decide(
    records: &[Record],
    manifest: &Manifest,
    generation: u32,
    dropped: u32,
) -> Result<Decision, Box<dyn Error>> {
    Ok(conclude(
        &state(records)?,
        manifest,
        &judge()?,
        generation,
        dropped,
    )?)
}

#[test]
fn conclude_records_an_improvement_with_counts() -> TestResult {
    let (e1, e2) = (exp(1)?, exp(2)?);
    let mut records = calibrated()?;
    records.extend([
        proposed(e1, baseline(), 1)?,
        proposed(e2, baseline(), 1)?,
        judged(e1, 0, scored(120.0)?)?,
        judged(e2, 0, Outcome::Crashed { stage: Stage::Run })?,
    ]);
    let decision = decide(&records, &manifest("maximize", "5%")?, 1, 3)?;
    assert_eq!(
        decision,
        Decision::new(DecisionKind::Improved { winner: e1 }, 2, 3)
    );
    Ok(())
}

#[test]
fn conclude_keeps_the_incumbent_below_the_threshold() -> TestResult {
    let e1 = exp(1)?;
    let mut records = calibrated()?;
    records.extend([proposed(e1, baseline(), 1)?, judged(e1, 0, scored(104.0)?)?]);
    let decision = decide(&records, &manifest("maximize", "5%")?, 1, 0)?;
    assert_eq!(
        decision,
        Decision::new(
            DecisionKind::Kept {
                incumbent: baseline()
            },
            1,
            0
        )
    );
    Ok(())
}

#[test]
fn conclude_uses_the_median_not_the_best_trial() -> TestResult {
    let e1 = exp(1)?;
    let mut records = calibrated()?;
    records.extend([
        proposed(e1, baseline(), 1)?,
        judged(e1, 0, scored(200.0)?)?,
        judged(e1, 1, scored(50.0)?)?,
        judged(e1, 2, scored(60.0)?)?,
    ]);
    let decision = decide(&records, &manifest("maximize", "5%")?, 1, 0)?;
    assert_eq!(
        *decision.kind(),
        DecisionKind::Kept {
            incumbent: baseline()
        }
    );
    Ok(())
}

#[test]
fn conclude_reports_no_valid_candidates() -> TestResult {
    let (e1, e2, e3) = (exp(1)?, exp(2)?, exp(3)?);
    let mut records = calibrated()?;
    records.extend([
        proposed(e1, baseline(), 1)?,
        proposed(e2, baseline(), 1)?,
        proposed(e3, baseline(), 1)?,
        judged(
            e1,
            0,
            Outcome::Crashed {
                stage: Stage::Build,
            },
        )?,
        judged(e2, 0, scored(500.0)?)?,
        judged(e2, 1, Outcome::Rejected(Reason::RunFailed))?,
        // A recorded Scored trial that fails the gate is not a valid score.
        judged(
            e3,
            0,
            Outcome::Scored(card(&[(GATE, 0.0, false), (OBJECTIVE, 500.0, false)])?),
        )?,
    ]);
    let decision = decide(&records, &manifest("maximize", "5%")?, 1, 1)?;
    assert_eq!(
        decision,
        Decision::new(DecisionKind::NoValidCandidates, 3, 1)
    );
    Ok(())
}

#[test]
fn conclude_with_no_proposals_reports_no_valid_candidates() -> TestResult {
    let decision = decide(&calibrated()?, &manifest("maximize", "5%")?, 1, 4)?;
    assert_eq!(
        decision,
        Decision::new(DecisionKind::NoValidCandidates, 0, 4)
    );
    Ok(())
}

#[test]
fn conclude_reads_only_the_named_judge() -> TestResult {
    let e1 = exp(1)?;
    let mut records = calibrated()?;
    records.extend([
        proposed(e1, baseline(), 1)?,
        judged_by(other_judge()?, e1, 0, scored(500.0)?),
    ]);
    let decision = decide(&records, &manifest("maximize", "5%")?, 1, 0)?;
    assert_eq!(*decision.kind(), DecisionKind::NoValidCandidates);
    Ok(())
}

#[test]
fn conclude_competes_against_the_latest_winner() -> TestResult {
    let (e1, e3, e4) = (exp(1)?, exp(3)?, exp(4)?);
    let mut records = calibrated()?;
    records.extend([
        proposed(e1, baseline(), 1)?,
        judged(e1, 0, scored(120.0)?)?,
        selected(1, DecisionKind::Improved { winner: e1 }, 1),
        proposed(e3, e1, 2)?,
        judged(e3, 0, scored(125.0)?)?,
    ]);
    let manifest = manifest("maximize", "5%")?;
    assert_eq!(
        *decide(&records, &manifest, 2, 0)?.kind(),
        DecisionKind::Kept { incumbent: e1 }
    );
    records.extend([proposed(e4, e1, 2)?, judged(e4, 0, scored(130.0)?)?]);
    assert_eq!(
        *decide(&records, &manifest, 2, 0)?.kind(),
        DecisionKind::Improved { winner: e4 }
    );
    Ok(())
}

#[test]
fn conclude_refuses_an_unscored_incumbent() -> TestResult {
    let e1 = exp(1)?;
    let records = vec![
        run_started(),
        proposed(e1, baseline(), 1)?,
        judged(e1, 0, scored(120.0)?)?,
    ];
    let result = conclude(
        &state(&records)?,
        &manifest("maximize", "5%")?,
        &judge()?,
        1,
        0,
    );
    assert_eq!(
        result,
        Err(PolicyError::IncumbentUnscored { exp: baseline() })
    );
    Ok(())
}

#[test]
fn conclude_refuses_generation_zero() -> TestResult {
    let result = conclude(
        &state(&calibrated()?)?,
        &manifest("maximize", "5%")?,
        &judge()?,
        0,
        0,
    );
    assert_eq!(result, Err(PolicyError::GenerationZero));
    Ok(())
}

fn arb_trial() -> impl Strategy<Value = Option<i32>> {
    prop_oneof![3 => (80_i32..130).prop_map(Some), 1 => Just(None)]
}

/// One experiment's trial values and the same values in another trial order.
fn arb_experiment() -> impl Strategy<Value = (Vec<Option<i32>>, Vec<Option<i32>>)> {
    proptest::collection::vec(arb_trial(), 1..4)
        .prop_flat_map(|trials| (Just(trials.clone()), Just(trials).prop_shuffle()))
}

fn trial_outcome(value: Option<i32>) -> Result<Outcome, Box<dyn Error>> {
    value.map_or(Ok(Outcome::Crashed { stage: Stage::Run }), |value| {
        scored(f64::from(value))
    })
}

/// A calibrated ledger with one generation-1 proposal per entry of `experiments`, each judged with its trials.
fn generation_ledger(experiments: &[Vec<Option<i32>>]) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut records = calibrated()?;
    for (n, trials) in (1_u64..).zip(experiments) {
        let id = exp(n)?;
        records.push(proposed(id, baseline(), 1)?);
        for (trial, value) in (0_u32..).zip(trials) {
            records.push(judged(id, trial, trial_outcome(*value)?)?);
        }
    }
    Ok(records)
}

proptest! {
    #[test]
    fn conclude_ignores_record_order(
        experiments in proptest::collection::vec(arb_experiment(), 1..6),
        keys in proptest::collection::vec(any::<u64>(), 64),
        maximize in any::<bool>(),
    ) {
        let manifest = manifest(if maximize { "maximize" } else { "minimize" }, "5%").map_err(fail)?;
        let original: Vec<Vec<Option<i32>>> = experiments.iter().map(|(trials, _)| trials.clone()).collect();
        let reordered: Vec<Vec<Option<i32>>> = experiments.iter().map(|(_, trials)| trials.clone()).collect();
        let records = generation_ledger(&original).map_err(fail)?;
        let mut permuted = generation_ledger(&reordered).map_err(fail)?.into_iter();
        let first = permuted.next().ok_or_else(|| fail("empty ledger"))?;
        let mut tail: Vec<(u64, Record)> = (0_usize..)
            .zip(permuted)
            .map(|(index, record)| (keys.get(index).copied().unwrap_or_default(), record))
            .collect();
        tail.sort_by_key(|(key, _)| *key);
        let permuted: Vec<Record> = std::iter::once(first).chain(tail.into_iter().map(|(_, record)| record)).collect();
        let judge = judge().map_err(fail)?;
        let expected = conclude(&fold(&records).map_err(fail)?, &manifest, &judge, 1, 0).map_err(fail)?;
        let actual = conclude(&fold(&permuted).map_err(fail)?, &manifest, &judge, 1, 0).map_err(fail)?;
        prop_assert_eq!(expected, actual);
        let count = u32::try_from(original.len()).map_err(fail)?;
        prop_assert_eq!(expected.proposed(), count);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// noise_floor and noise_check
// ---------------------------------------------------------------------------------------------------------------

fn verdict(values: &[f64], goal: &str, threshold: &str) -> Result<NoiseVerdict, Box<dyn Error>> {
    let state = state(&baseline_trials(values)?)?;
    let floor = noise_floor(&state, &judge()?, &name(OBJECTIVE)?);
    Ok(noise_check(floor.as_ref(), &objective(goal, threshold)?))
}

#[test]
fn noise_floor_folds_the_baseline_trials() -> TestResult {
    let state = state(&baseline_trials(&[10.5, 10.0, 10.25])?)?;
    let floor = noise_floor(&state, &judge()?, &name(OBJECTIVE)?).ok_or("no floor")?;
    assert_eq!(floor.median(), metric(10.25)?);
    assert_eq!(floor.min(), metric(10.0)?);
    assert_eq!(floor.max(), metric(10.5)?);
    assert_eq!(floor.trials(), 3);
    assert_eq!(floor.spread().to_bits(), 0.5_f64.to_bits());
    assert!(noise_floor(&state, &judge()?, &name("absent")?).is_none());
    assert!(noise_floor(&state, &other_judge()?, &name(OBJECTIVE)?).is_none());
    Ok(())
}

#[test]
fn noise_floor_skips_unscored_baseline_trials() -> TestResult {
    let mut records = baseline_trials(&[10.0, 10.5])?;
    records.push(judged(
        baseline(),
        7,
        Outcome::Crashed { stage: Stage::Run },
    )?);
    let state = state(&records)?;
    let floor = noise_floor(&state, &judge()?, &name(OBJECTIVE)?).ok_or("no floor")?;
    assert_eq!(floor.trials(), 2);
    Ok(())
}

#[test]
fn noise_check_warns_without_a_floor() -> TestResult {
    let objective = objective("maximize", "0.5")?;
    assert_eq!(noise_check(None, &objective), NoiseVerdict::Warn);
    assert_eq!(verdict(&[10.0], "maximize", "0.5")?, NoiseVerdict::Warn);
    Ok(())
}

#[test]
fn noise_check_is_ok_with_zero_spread() -> TestResult {
    assert_eq!(
        verdict(&[10.0, 10.0, 10.0], "maximize", "0.5")?,
        NoiseVerdict::Ok
    );
    Ok(())
}

#[test]
fn noise_check_compares_the_threshold_with_the_spread() -> TestResult {
    // An absolute threshold of 0.5: refuse at a spread of 0.5, warn at 0.25 (2x), pass at 0.125 (4x).
    assert_eq!(
        verdict(&[10.0, 10.5], "maximize", "0.5")?,
        NoiseVerdict::Refuse
    );
    assert_eq!(
        verdict(&[10.0, 11.0], "minimize", "0.5")?,
        NoiseVerdict::Refuse
    );
    assert_eq!(
        verdict(&[10.0, 10.25], "maximize", "0.5")?,
        NoiseVerdict::Warn
    );
    assert_eq!(
        verdict(&[10.0, 10.125], "maximize", "0.5")?,
        NoiseVerdict::Ok
    );
    // A relative threshold of 5% at the median of 100 is 5.
    assert_eq!(
        verdict(&[100.0, 105.0], "maximize", "5%")?,
        NoiseVerdict::Refuse
    );
    assert_eq!(
        verdict(&[100.0, 103.0], "maximize", "5%")?,
        NoiseVerdict::Warn
    );
    assert_eq!(
        verdict(&[100.0, 101.0], "maximize", "5%")?,
        NoiseVerdict::Ok
    );
    Ok(())
}

#[test]
fn noise_check_refuses_an_unbounded_spread() -> TestResult {
    assert_eq!(
        verdict(&[-f64::MAX, f64::MAX], "maximize", "0.5")?,
        NoiseVerdict::Refuse
    );
    Ok(())
}
