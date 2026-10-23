//! The loop's stage function (07-CONTEXT D-06, D-07; LOOP-05): one test per row of its table, and properties that
//! a synthetic driver performing only the steps `next_step` returns calibrates first, never repeats a recorded
//! step, and reaches its target, and that any permutation of a ledger prefix gives the same step.
//!
//! Ledgers are built from public constructors only. Candidate experiment IDs are parsed from 64-character hex
//! strings, because the loop's clippy policy bans hashing a bare diff into an `ExperimentId` (D-12), and the fold
//! never recomputes a candidate's ID from its diff.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, RunId, TreeId};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation,
    Origin, Record,
};
use minos_core::manifest::{Arch, JudgeVersion, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Stage};
use minos_core::state::{RunState, fold};
use minos_loop::stage::{CANDIDATE_TRIALS, Step, next_step};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// The most proposals one synthetic propose step records.
const MAX_BATCH: usize = 3;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

fn manifest() -> ManifestId {
    ManifestId::of(b"m")
}

fn base() -> TreeId {
    TreeId::of(b"t")
}

fn run() -> RunId {
    RunId::of(&manifest(), &base(), STARTED_AT)
}

fn baseline() -> ExperimentId {
    ExperimentId::baseline(&base())
}

/// A candidate experiment ID from a distinct number.
fn exp(n: u64) -> Result<ExperimentId, Box<dyn Error>> {
    Ok(ExperimentId::from_str(&format!("{n:064x}"))?)
}

/// The judge version the run is driven under.
fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

/// Another judge version, whose judgments never count for [`judge`].
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

fn scored(value: f64) -> Result<Outcome, Box<dyn Error>> {
    let card = Scorecard::from_entries([(
        MetricName::from_str("throughput_mb_s")?,
        Metric::new(value)?,
        false,
    )])?;
    Ok(Outcome::Scored(card))
}

fn rec(event: Event) -> Record {
    Record::v1(event)
}

fn run_started() -> Record {
    rec(Event::RunStarted {
        run: run(),
        manifest: manifest(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
        root_image: Some(ImageDigest::of(b"root")),
    })
}

fn proposed(exp: ExperimentId, parent: ExperimentId, generation: u32) -> Record {
    rec(Event::Proposed {
        exp,
        parent,
        generation,
        tier: Tier::Sweep,
        diff: BlobId::of(exp.as_bytes()),
        rationale: BlobId::of(b"rationale"),
        origin: Origin::Sweep,
        batch: None,
    })
}

fn judged_under(exp: ExperimentId, judge: JudgeVersion, trial: u32, outcome: Outcome) -> Record {
    rec(Event::Judged {
        exp,
        judge,
        trial,
        outcome,
        backend: backend(),
        logs: None,
    })
}

fn judged(exp: ExperimentId, trial: u32) -> Result<Record, Box<dyn Error>> {
    Ok(judged_under(exp, judge()?, trial, scored(100.0)?))
}

fn selected(generation: u32, kind: DecisionKind, proposed: u32) -> Record {
    rec(Event::Selected {
        generation,
        decision: Decision::new(kind, proposed, 0),
    })
}

/// `RunStarted` and `trials` judged baseline trials.
fn calibrated(trials: u32) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut records = vec![run_started()];
    for trial in 0..trials {
        records.push(judged(baseline(), trial)?);
    }
    Ok(records)
}

fn step_of(records: &[Record], baseline_trials: u32, target: u32) -> Result<Step, Box<dyn Error>> {
    let state = fold(records)?;
    Ok(next_step(
        &state,
        &judge()?,
        baseline_trials,
        CANDIDATE_TRIALS,
        target,
    ))
}

// ---------------------------------------------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn candidates_run_one_trial_in_v1() {
    assert_eq!(CANDIDATE_TRIALS, 1);
}

#[test]
fn calibration_runs_the_lowest_unjudged_baseline_trial_first() -> TestResult {
    let records = vec![run_started()];
    assert_eq!(step_of(&records, 3, 2)?, Step::Calibrate { trial: 0 });

    let records = vec![
        run_started(),
        judged(baseline(), 2)?,
        judged(baseline(), 0)?,
    ];
    assert_eq!(step_of(&records, 3, 2)?, Step::Calibrate { trial: 1 });
    Ok(())
}

#[test]
fn a_run_with_no_baseline_trials_proposes_at_once() -> TestResult {
    assert_eq!(
        step_of(&[run_started()], 0, 1)?,
        Step::Propose { generation: 1 }
    );
    Ok(())
}

#[test]
fn a_calibrated_run_with_target_zero_is_done_at_generation_zero() -> TestResult {
    assert_eq!(
        step_of(&calibrated(3)?, 3, 0)?,
        Step::Done { generation: 0 }
    );
    Ok(())
}

#[test]
fn a_calibrated_run_proposes_generation_one() -> TestResult {
    assert_eq!(
        step_of(&calibrated(3)?, 3, 2)?,
        Step::Propose { generation: 1 }
    );
    Ok(())
}

#[test]
fn trials_run_in_experiment_order_whatever_the_record_order() -> TestResult {
    let (low, high) = (exp(3)?, exp(5)?);
    let mut forward = calibrated(3)?;
    forward.push(proposed(low, baseline(), 1));
    forward.push(proposed(high, baseline(), 1));
    let mut reverse = calibrated(3)?;
    reverse.push(proposed(high, baseline(), 1));
    reverse.push(proposed(low, baseline(), 1));

    let first = Step::RunTrial {
        generation: 1,
        exp: low,
        trial: 0,
    };
    assert_eq!(step_of(&forward, 3, 2)?, first);
    assert_eq!(step_of(&reverse, 3, 2)?, first);

    reverse.push(judged(low, 0)?);
    assert_eq!(
        step_of(&reverse, 3, 2)?,
        Step::RunTrial {
            generation: 1,
            exp: high,
            trial: 0,
        }
    );
    Ok(())
}

#[test]
fn trials_of_one_experiment_run_in_index_order_before_the_next_experiment() -> TestResult {
    let (low, high) = (exp(3)?, exp(5)?);
    let mut records = calibrated(1)?;
    records.push(proposed(high, baseline(), 1));
    records.push(proposed(low, baseline(), 1));
    records.push(judged(low, 1)?);
    let state = fold(&records)?;
    assert_eq!(
        next_step(&state, &judge()?, 1, 2, 1),
        Step::RunTrial {
            generation: 1,
            exp: low,
            trial: 0,
        }
    );

    records.push(judged(low, 0)?);
    let state = fold(&records)?;
    assert_eq!(
        next_step(&state, &judge()?, 1, 2, 1),
        Step::RunTrial {
            generation: 1,
            exp: high,
            trial: 0,
        }
    );
    Ok(())
}

#[test]
fn a_fully_judged_generation_concludes_and_then_is_done_at_its_target() -> TestResult {
    let e1 = exp(1)?;
    let mut records = calibrated(3)?;
    records.push(proposed(e1, baseline(), 1));
    records.push(judged(e1, 0)?);
    assert_eq!(step_of(&records, 3, 1)?, Step::Conclude { generation: 1 });

    records.push(selected(1, DecisionKind::Improved { winner: e1 }, 1));
    assert_eq!(step_of(&records, 3, 1)?, Step::Done { generation: 1 });
    assert_eq!(step_of(&records, 3, 2)?, Step::Propose { generation: 2 });
    Ok(())
}

#[test]
fn an_empty_generation_concluded_in_its_propose_step_moves_on() -> TestResult {
    let mut records = calibrated(3)?;
    records.push(selected(1, DecisionKind::NoValidCandidates, 0));
    assert_eq!(step_of(&records, 3, 2)?, Step::Propose { generation: 2 });
    Ok(())
}

#[test]
fn a_target_below_the_last_concluded_generation_is_done_there() -> TestResult {
    let mut records = calibrated(1)?;
    records.push(selected(1, DecisionKind::NoValidCandidates, 0));
    records.push(selected(2, DecisionKind::NoValidCandidates, 0));
    assert_eq!(step_of(&records, 1, 1)?, Step::Done { generation: 2 });
    Ok(())
}

#[test]
fn judgments_under_another_judge_version_do_not_count() -> TestResult {
    let e1 = exp(1)?;
    let records = vec![
        run_started(),
        judged_under(baseline(), other_judge()?, 0, scored(1.0)?),
    ];
    assert_eq!(step_of(&records, 1, 1)?, Step::Calibrate { trial: 0 });

    let mut records = calibrated(1)?;
    records.push(proposed(e1, baseline(), 1));
    records.push(judged_under(e1, other_judge()?, 0, scored(1.0)?));
    assert_eq!(
        step_of(&records, 1, 1)?,
        Step::RunTrial {
            generation: 1,
            exp: e1,
            trial: 0,
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The synthetic driver
// ---------------------------------------------------------------------------------------------------------------

/// The fixed inputs of one synthetic run, and the choices its driver makes.
#[derive(Debug, Clone)]
struct Plan {
    baseline_trials: u32,
    candidate_trials: u32,
    target: u32,
    /// The size of each propose step's batch, cycled.
    batches: Vec<usize>,
    /// Per conclusion, whether the generation's smallest experiment wins, cycled.
    wins: Vec<bool>,
    /// Per judgment, whether the trial crashes rather than scores, cycled.
    crashes: Vec<bool>,
}

fn plans() -> impl Strategy<Value = Plan> {
    (
        0u32..4,
        1u32..3,
        0u32..5,
        prop::collection::vec(0..=MAX_BATCH, 1..6),
        prop::collection::vec(any::<bool>(), 1..6),
        prop::collection::vec(any::<bool>(), 1..6),
    )
        .prop_map(
            |(baseline_trials, candidate_trials, target, batches, wins, crashes)| Plan {
                baseline_trials,
                candidate_trials,
                target,
                batches,
                wins,
                crashes,
            },
        )
}

fn fail(error: impl std::fmt::Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

/// The `n`th entry of a non-empty cycled choice list.
fn cycled<T: Copy + Default>(choices: &[T], n: usize) -> T {
    choices
        .get(n % choices.len().max(1))
        .copied()
        .unwrap_or_default()
}

/// The most steps a run of `plan` can take before `Done`: every baseline trial, then per generation one propose
/// step, every trial of a full batch, and one conclusion, then `Done` itself.
fn step_bound(plan: &Plan) -> usize {
    let per_generation = 2 + MAX_BATCH * usize::try_from(plan.candidate_trials).unwrap_or(0);
    usize::try_from(plan.baseline_trials).unwrap_or(0)
        + usize::try_from(plan.target).unwrap_or(0) * per_generation
        + 1
}

/// Drives a synthetic run from `RunStarted` by asking `next_step` and appending the one record that completes
/// each step, checking the stage properties along the way. Returns the ledger in append order.
fn drive(plan: &Plan) -> Result<Vec<Record>, TestCaseError> {
    let judge = judge().map_err(fail)?;
    let mut records = vec![run_started()];
    let mut seen: Vec<Step> = Vec::new();
    let mut next_exp = 1u64;
    let (mut proposes, mut conclusions, mut judgments) = (0usize, 0usize, 0usize);

    for _ in 0..step_bound(plan) {
        let state = fold(&records).map_err(fail)?;
        let step = next_step(
            &state,
            &judge,
            plan.baseline_trials,
            plan.candidate_trials,
            plan.target,
        );
        if let Step::Done { generation } = step {
            prop_assert_eq!(generation, plan.target);
            return Ok(records);
        }
        prop_assert!(
            !seen.contains(&step),
            "step {:?} was returned again after its record was appended",
            step
        );
        seen.push(step);

        match step {
            Step::Calibrate { trial } => {
                prop_assert!(trial < plan.baseline_trials);
                prop_assert_eq!(state.proposals_in(1).count(), 0);
                let outcome = outcome_for(plan, judgments)?;
                judgments += 1;
                records.push(judged_under(baseline(), judge, trial, outcome));
            }
            Step::Propose { generation } => {
                for trial in 0..plan.baseline_trials {
                    prop_assert!(
                        state.judged(&baseline(), &judge, trial),
                        "propose before baseline trial {} was judged",
                        trial
                    );
                }
                let parent = state.incumbent_before(generation);
                let size = cycled(&plan.batches, proposes);
                proposes += 1;
                for _ in 0..size {
                    let id = exp(next_exp).map_err(fail)?;
                    next_exp += 1;
                    records.push(proposed(id, parent, generation));
                }
                if size == 0 {
                    records.push(selected(generation, DecisionKind::NoValidCandidates, 0));
                }
            }
            Step::RunTrial { exp: id, trial, .. } => {
                prop_assert!(trial < plan.candidate_trials);
                let outcome = outcome_for(plan, judgments)?;
                judgments += 1;
                records.push(judged_under(id, judge, trial, outcome));
            }
            Step::Conclude { generation } => {
                let proposed: Vec<ExperimentId> =
                    state.proposals_in(generation).map(|(id, _)| *id).collect();
                let count = u32::try_from(proposed.len()).map_err(fail)?;
                let kind = match proposed.first() {
                    Some(winner) if cycled(&plan.wins, conclusions) => {
                        DecisionKind::Improved { winner: *winner }
                    }
                    _ => DecisionKind::Kept {
                        incumbent: state.incumbent_before(generation),
                    },
                };
                conclusions += 1;
                records.push(selected(generation, kind, count));
            }
            Step::Done { .. } => {}
        }
    }
    Err(fail(format!("no Done within {} steps", step_bound(plan))))
}

fn outcome_for(plan: &Plan, judgments: usize) -> Result<Outcome, TestCaseError> {
    if cycled(&plan.crashes, judgments) {
        Ok(Outcome::Crashed { stage: Stage::Run })
    } else {
        let value = f64::from(u32::try_from(judgments % 1000).map_err(fail)?);
        scored(value).map_err(fail)
    }
}

/// The next step of `records` under `plan`'s fixed inputs.
fn step_under(plan: &Plan, records: &[Record]) -> Result<Step, TestCaseError> {
    let state: RunState = fold(records).map_err(fail)?;
    Ok(next_step(
        &state,
        &judge().map_err(fail)?,
        plan.baseline_trials,
        plan.candidate_trials,
        plan.target,
    ))
}

proptest! {
    /// A driver that performs only the steps `next_step` returns calibrates before any proposal, never gets a
    /// step back once its record exists, and reaches `Done` at its target within the bound.
    #[test]
    fn a_synthetic_driver_calibrates_first_never_repeats_and_reaches_its_target(plan in plans()) {
        drive(&plan)?;
    }

    /// Any prefix of a driven ledger, with the records after `RunStarted` in any order, gives the same step.
    #[test]
    fn the_step_is_the_same_under_any_permutation_of_a_ledger_prefix(
        plan in plans(),
        cut in any::<prop::sample::Index>(),
        keys in prop::collection::vec(any::<u64>(), 0..64),
    ) {
        let ledger = drive(&plan)?;
        let prefix_len = 1 + cut.index(ledger.len());
        let prefix: Vec<Record> = ledger.into_iter().take(prefix_len).collect();
        let expected = step_under(&plan, &prefix)?;

        let mut tail: Vec<(u64, usize, Record)> = prefix
            .iter()
            .skip(1)
            .cloned()
            .enumerate()
            .map(|(i, record)| (keys.get(i).copied().unwrap_or(0), i, record))
            .collect();
        tail.sort_by_key(|(key, i, _)| (*key, *i));
        let permuted: Vec<Record> = std::iter::once(run_started())
            .chain(tail.into_iter().map(|(_, _, record)| record))
            .collect();
        prop_assert_eq!(step_under(&plan, &permuted)?, expected);
    }
}
