//! The fold from ledger records to `RunState` (CORE-09): accessors over a valid ledger, one test per corruption
//! case, and properties for order-insensitivity, duplicate replay, and tamper detection.
//!
//! Ledgers are built from public constructors only. Candidate experiment IDs are parsed from 64-character hex
//! strings: any 32 bytes form an ID, and the fold never recomputes a candidate's ID from its diff.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::str::FromStr;

use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, ModelId, PromptHash, ReportHash,
    RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation,
    Origin, Record, ReviewAnswer, ReviewId, ReviewKind, SpendId, SpendSource, StopReason,
};
use minos_core::manifest::{Arch, JudgeVersion, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::state::{FoldError, RunState, fold};
use minos_core::usage::Usage;
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

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

fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

fn haiku() -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str("haiku")?)
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

fn run_started() -> Event {
    run_started_with(None)
}

/// The run's `RunStarted`, with the lock's root image digest when `root_image` is `Some`.
fn run_started_with(root_image: Option<ImageDigest>) -> Event {
    Event::RunStarted {
        run: run(),
        manifest: manifest(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
        root_image,
    }
}

/// A build and a run log blob for `exp`'s `trial`.
fn stage_logs(exp: ExperimentId, trial: u32) -> BTreeMap<Stage, BlobId> {
    [
        (
            Stage::Build,
            BlobId::of(format!("build {exp} {trial}").as_bytes()),
        ),
        (
            Stage::Run,
            BlobId::of(format!("run {exp} {trial}").as_bytes()),
        ),
    ]
    .into_iter()
    .collect()
}

/// The origin that agrees with `tier`: sweep for the sweep tier, a model call otherwise.
fn origin_for(tier: Tier) -> Result<Origin, Box<dyn Error>> {
    Ok(match tier {
        Tier::Sweep => Origin::Sweep,
        Tier::SingleShot | Tier::Planned | Tier::Agentic => Origin::Model {
            model: haiku()?,
            prompt: PromptHash::of(b"prompt"),
        },
    })
}

fn proposed_with(
    exp: ExperimentId,
    parent: ExperimentId,
    generation: u32,
    tier: Tier,
    origin: Origin,
) -> Event {
    Event::Proposed {
        exp,
        parent,
        generation,
        tier,
        diff: BlobId::of(exp.as_bytes()),
        rationale: BlobId::of(b"rationale"),
        origin,
        batch: None,
    }
}

fn proposed(
    exp: ExperimentId,
    parent: ExperimentId,
    generation: u32,
    tier: Tier,
) -> Result<Event, Box<dyn Error>> {
    Ok(proposed_with(
        exp,
        parent,
        generation,
        tier,
        origin_for(tier)?,
    ))
}

fn judged(exp: ExperimentId, trial: u32, outcome: Outcome) -> Result<Event, Box<dyn Error>> {
    Ok(Event::Judged {
        exp,
        judge: judge()?,
        trial,
        outcome,
        backend: backend(),
        logs: None,
    })
}

fn selected(generation: u32, kind: DecisionKind, proposed: u32) -> Event {
    Event::Selected {
        generation,
        decision: Decision::new(kind, proposed, 0),
    }
}

fn review_id(kind: ReviewKind) -> ReviewId {
    ReviewId::of(&run(), kind, &BlobId::of(b"subject"))
}

/// A valid two-generation ledger, in append order, with every event kind.
fn valid_ledger() -> Result<Vec<Record>, Box<dyn Error>> {
    let (e1, e2, e3) = (exp(1)?, exp(2)?, exp(3)?);
    let request = review_id(ReviewKind::Params);
    Ok(vec![
        rec(run_started()),
        rec(judged(baseline(), 0, scored(100.0)?)?),
        rec(judged(baseline(), 2, scored(101.0)?)?),
        rec(judged(baseline(), 1, scored(99.0)?)?),
        rec(Event::Spent {
            generation: 0,
            source: SpendSource::Judge {
                exp: baseline(),
                trial: 0,
            },
            usage: Usage::compute(10, 20),
        }),
        rec(proposed(e2, baseline(), 1, Tier::Sweep)?),
        rec(proposed(e1, baseline(), 1, Tier::SingleShot)?),
        rec(Event::Spent {
            generation: 1,
            source: SpendSource::ModelCall {
                call: 0,
                model: haiku()?,
            },
            usage: Usage::tokens(100, 20, 3, 4),
        }),
        rec(judged(e1, 0, scored(120.0)?)?),
        rec(judged(e2, 0, Outcome::Crashed { stage: Stage::Run })?),
        rec(Event::Spent {
            generation: 1,
            source: SpendSource::Candidate { exp: e1, trial: 0 },
            usage: Usage::compute(5, 7),
        }),
        rec(selected(1, DecisionKind::Improved { winner: e1 }, 2)),
        rec(Event::Steered {
            generation: 2,
            ordinal: 1,
            note: BlobId::of(b"note one"),
        }),
        rec(Event::Steered {
            generation: 2,
            ordinal: 0,
            note: BlobId::of(b"note zero"),
        }),
        rec(Event::ManifestChanged {
            manifest: ManifestId::of(b"m2"),
            from_generation: 2,
        }),
        rec(proposed(e3, e1, 2, Tier::SingleShot)?),
        rec(judged(e3, 0, Outcome::Rejected(Reason::BuildFailed))?),
        rec(selected(2, DecisionKind::Kept { incumbent: e1 }, 1)),
        rec(Event::ReviewRequested {
            request,
            kind: ReviewKind::Params,
        }),
        rec(Event::Reviewed {
            request,
            answer: ReviewAnswer::Approved,
        }),
        rec(Event::ReportAcknowledged {
            exp: e1,
            report: ReportHash::of(b"report"),
        }),
        rec(Event::StopRequested {
            generation: 3,
            reason: StopReason::UserStop,
        }),
    ])
}

/// The valid ledger with `extra` appended.
fn with(extra: Event) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut records = valid_ledger()?;
    records.push(rec(extra));
    Ok(records)
}

fn fold_err(records: &[Record]) -> Result<FoldError, Box<dyn Error>> {
    match fold(records) {
        Ok(_) => Err("the ledger folded, but a FoldError was expected".into()),
        Err(error) => Ok(error),
    }
}

fn folded() -> Result<RunState, Box<dyn Error>> {
    Ok(fold(&valid_ledger()?)?)
}

// ---------------------------------------------------------------------------------------------------------------
// A valid ledger
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_valid_ledger_folds_with_its_identity() -> TestResult {
    let state = folded()?;
    assert_eq!(state.run(), &run());
    assert_eq!(state.base(), &base());
    assert_eq!(state.baseline(), &baseline());
    assert_eq!(state.started_at(), STARTED_AT);
    assert_eq!(state.manifest(), &ManifestId::of(b"m2"));
    assert_eq!(state.manifest_at(0), &manifest());
    assert_eq!(state.manifest_at(1), &manifest());
    assert_eq!(state.manifest_at(2), &ManifestId::of(b"m2"));
    assert_eq!(state.manifest_at(9), &ManifestId::of(b"m2"));
    Ok(())
}

#[test]
fn proposals_and_trials_come_back_in_key_order() -> TestResult {
    let state = folded()?;
    let (e1, e2, e3) = (exp(1)?, exp(2)?, exp(3)?);

    let gen1: Vec<ExperimentId> = state.proposals_in(1).map(|(id, _)| *id).collect();
    assert_eq!(gen1, vec![e1, e2]);
    let gen2: Vec<ExperimentId> = state.proposals_in(2).map(|(id, _)| *id).collect();
    assert_eq!(gen2, vec![e3]);
    assert_eq!(state.proposals_in(3).count(), 0);

    let proposal = state.proposal(&e3).ok_or("e3 is proposed")?;
    assert_eq!(proposal.parent(), &e1);
    assert_eq!(proposal.generation(), 2);
    assert_eq!(proposal.tier(), Tier::SingleShot);
    assert_eq!(proposal.diff(), &BlobId::of(e3.as_bytes()));
    assert_eq!(proposal.rationale(), &BlobId::of(b"rationale"));
    assert_eq!(proposal.origin(), &origin_for(Tier::SingleShot)?);
    assert!(state.proposal(&baseline()).is_none());

    let trials: Vec<u32> = state
        .trials(&baseline(), &judge()?)
        .map(|(trial, _)| trial)
        .collect();
    assert_eq!(trials, vec![0, 1, 2]);
    let first = state
        .trials(&baseline(), &judge()?)
        .next()
        .ok_or("a baseline trial")?;
    assert_eq!(first.1.outcome(), &scored(100.0)?);
    assert_eq!(first.1.backend(), &backend());
    assert_eq!(state.trials(&exp(9)?, &judge()?).count(), 0);
    Ok(())
}

#[test]
fn decisions_and_incumbents() -> TestResult {
    let state = folded()?;
    let e1 = exp(1)?;
    assert_eq!(
        state.decision(1).map(Decision::kind),
        Some(&DecisionKind::Improved { winner: e1 })
    );
    assert_eq!(
        state.decision(2).map(Decision::kind),
        Some(&DecisionKind::Kept { incumbent: e1 })
    );
    assert!(state.decision(3).is_none());
    let generations: Vec<u32> = state
        .decisions()
        .map(|(generation, _)| generation)
        .collect();
    assert_eq!(generations, vec![1, 2]);
    assert_eq!(state.last_concluded_generation(), 2);
    assert_eq!(state.incumbent_before(0), baseline());
    assert_eq!(state.incumbent_before(1), baseline());
    assert_eq!(state.incumbent_before(2), e1);
    assert_eq!(state.incumbent_before(3), e1);
    assert_eq!(state.incumbent(), e1);
    Ok(())
}

#[test]
fn a_run_with_no_decisions_has_the_baseline_as_incumbent() -> TestResult {
    let state = fold(&[rec(run_started())])?;
    assert_eq!(state.last_concluded_generation(), 0);
    assert_eq!(state.incumbent(), baseline());
    assert_eq!(state.lineage(&baseline()), Some(vec![baseline()]));
    assert_eq!(state.total_usage(), &Usage::default());
    assert!(state.stop_requested().is_none());
    Ok(())
}

#[test]
fn notes_come_back_in_ordinal_order() -> TestResult {
    let state = folded()?;
    assert_eq!(
        state.notes(2),
        vec![BlobId::of(b"note zero"), BlobId::of(b"note one")]
    );
    assert_eq!(
        state.notes_by_ordinal(2),
        vec![(0, BlobId::of(b"note zero")), (1, BlobId::of(b"note one"))]
    );
    assert_eq!(state.next_note_ordinal(2), Some(2));
    assert!(state.notes(1).is_empty());
    assert_eq!(state.next_note_ordinal(1), Some(0));
    Ok(())
}

#[test]
fn the_last_note_ordinal_has_no_successor() -> TestResult {
    let records = with(Event::Steered {
        generation: 5,
        ordinal: u32::MAX,
        note: BlobId::of(b"last"),
    })?;
    let state = fold(&records)?;
    assert_eq!(state.notes(5), vec![BlobId::of(b"last")]);
    assert_eq!(
        state.notes_by_ordinal(5),
        vec![(u32::MAX, BlobId::of(b"last"))]
    );
    assert_eq!(state.next_note_ordinal(5), None);
    Ok(())
}

#[test]
fn stops_spends_reviews_and_acknowledgements() -> TestResult {
    let records = with(Event::StopRequested {
        generation: 7,
        reason: StopReason::SpendCap,
    })?;
    let state = fold(&records)?;
    assert_eq!(state.stop_requested(), Some((3, StopReason::UserStop)));

    let expected = Usage::compute(10, 20)
        .checked_add(&Usage::tokens(100, 20, 3, 4))
        .and_then(|sum| sum.checked_add(&Usage::compute(5, 7)))
        .ok_or("no overflow")?;
    assert_eq!(state.total_usage(), &expected);
    assert_eq!(state.spends().count(), 3);
    let baseline_spend = state
        .spends()
        .find(|(_, spend)| spend.generation() == 0)
        .ok_or("the baseline spend")?;
    assert_eq!(baseline_spend.1.usage(), &Usage::compute(10, 20));
    assert_eq!(
        baseline_spend.1.source(),
        &SpendSource::Judge {
            exp: baseline(),
            trial: 0
        }
    );

    let reviews: Vec<(ReviewId, ReviewKind, Option<ReviewAnswer>)> = state
        .reviews()
        .map(|(id, review)| (*id, review.kind(), review.answer()))
        .collect();
    assert_eq!(
        reviews,
        vec![(
            review_id(ReviewKind::Params),
            ReviewKind::Params,
            Some(ReviewAnswer::Approved)
        )]
    );

    assert!(state.is_acknowledged(&exp(1)?, &ReportHash::of(b"report")));
    assert!(!state.is_acknowledged(&exp(1)?, &ReportHash::of(b"other")));
    assert!(!state.is_acknowledged(&exp(3)?, &ReportHash::of(b"report")));
    Ok(())
}

#[test]
fn an_unanswered_review_has_no_answer() -> TestResult {
    let request = review_id(ReviewKind::Dependencies);
    let state = fold(&with(Event::ReviewRequested {
        request,
        kind: ReviewKind::Dependencies,
    })?)?;
    let review = state
        .reviews()
        .find(|(id, _)| **id == request)
        .ok_or("the request")?;
    assert_eq!(review.1.kind(), ReviewKind::Dependencies);
    assert_eq!(review.1.answer(), None);
    Ok(())
}

#[test]
fn lineage_runs_from_the_baseline_to_the_experiment() -> TestResult {
    let state = folded()?;
    let (e1, e2, e3) = (exp(1)?, exp(2)?, exp(3)?);
    assert_eq!(state.lineage(&e3), Some(vec![baseline(), e1, e3]));
    assert_eq!(state.lineage(&e2), Some(vec![baseline(), e2]));
    assert_eq!(state.lineage(&baseline()), Some(vec![baseline()]));
    assert_eq!(state.lineage(&exp(9)?), None);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Phase 7 accessors
// ---------------------------------------------------------------------------------------------------------------

/// The valid ledger with its `RunStarted` replaced by `first` and `extra` appended.
fn starting_with(first: Event, extra: Vec<Event>) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut records = vec![rec(first)];
    records.extend(valid_ledger()?.into_iter().skip(1));
    records.extend(extra.into_iter().map(rec));
    Ok(records)
}

#[test]
fn root_image_is_absent_on_a_ledger_that_records_none() -> TestResult {
    assert_eq!(folded()?.root_image(), None);
    Ok(())
}

#[test]
fn root_image_is_the_recorded_digest() -> TestResult {
    let digest = ImageDigest::of(b"locked root");
    let state = fold(&starting_with(run_started_with(Some(digest)), Vec::new())?)?;
    assert_eq!(state.root_image(), Some(&digest));
    Ok(())
}

#[test]
fn judge_versions_lists_every_version_an_experiment_was_judged_under() -> TestResult {
    // Arrange
    let other = JudgeVersion::from_str(&"cd".repeat(32))?;
    let records = with(Event::Judged {
        exp: baseline(),
        judge: other,
        trial: 0,
        outcome: scored(100.0)?,
        backend: backend(),
        logs: None,
    })?;

    // Act
    let state = fold(&records)?;

    // Assert
    let expected: BTreeSet<JudgeVersion> = [judge()?, other].into();
    assert_eq!(state.judge_versions(&baseline()), expected);
    assert!(state.judge_versions(&exp(9)?).is_empty());
    Ok(())
}

#[test]
fn backend_classes_is_empty_with_no_judgment() -> TestResult {
    let state = fold(&[rec(run_started())])?;
    assert!(state.backend_classes().is_empty());
    Ok(())
}

#[test]
fn backend_classes_holds_one_class_for_one_backend() -> TestResult {
    let expected: BTreeSet<(BackendKind, Arch)> = [(BackendKind::Krun, Arch::Arm64)].into();
    assert_eq!(folded()?.backend_classes(), expected);
    Ok(())
}

#[test]
fn backend_classes_holds_two_classes_for_a_mixed_ledger() -> TestResult {
    let other = BackendInfo::new(
        BackendKind::Krun,
        Arch::X86_64,
        Isolation::Hardware,
        EXPECTED_KRUN_LAYERS.iter().copied().collect(),
        ImageDigest::of(b"root"),
        None,
    );
    let records = with(Event::Judged {
        exp: baseline(),
        judge: judge()?,
        trial: 3,
        outcome: scored(100.0)?,
        backend: other,
        logs: None,
    })?;
    let expected: BTreeSet<(BackendKind, Arch)> = [
        (BackendKind::Krun, Arch::Arm64),
        (BackendKind::Krun, Arch::X86_64),
    ]
    .into();
    assert_eq!(fold(&records)?.backend_classes(), expected);
    Ok(())
}

#[test]
fn has_spend_is_true_exactly_for_recorded_spends() -> TestResult {
    let state = folded()?;
    let (e1, e2) = (exp(1)?, exp(2)?);
    let recorded = [
        (
            0,
            SpendSource::Judge {
                exp: baseline(),
                trial: 0,
            },
        ),
        (
            1,
            SpendSource::ModelCall {
                call: 0,
                model: haiku()?,
            },
        ),
        (1, SpendSource::Candidate { exp: e1, trial: 0 }),
    ];
    for (generation, source) in &recorded {
        assert!(state.has_spend(&SpendId::of(&run(), *generation, source)));
    }
    let absent = [
        (1, SpendSource::Candidate { exp: e2, trial: 0 }),
        (2, SpendSource::Candidate { exp: e1, trial: 0 }),
        (
            1,
            SpendSource::ModelCall {
                call: 1,
                model: haiku()?,
            },
        ),
        (
            0,
            SpendSource::Candidate {
                exp: baseline(),
                trial: 0,
            },
        ),
    ];
    for (generation, source) in &absent {
        assert!(!state.has_spend(&SpendId::of(&run(), *generation, source)));
    }
    Ok(())
}

#[test]
fn next_model_call_is_zero_without_a_model_call() -> TestResult {
    let state = folded()?;
    // Generation 0 has a judge spend only, generation 2 none.
    assert_eq!(state.next_model_call(0), 0);
    assert_eq!(state.next_model_call(2), 0);
    assert_eq!(state.next_model_call(9), 0);
    Ok(())
}

#[test]
fn next_model_call_is_one_past_the_highest_call_of_its_generation() -> TestResult {
    let model_call = |generation: u32, call: u32| -> Result<Event, Box<dyn Error>> {
        Ok(Event::Spent {
            generation,
            source: SpendSource::ModelCall {
                call,
                model: haiku()?,
            },
            usage: Usage::tokens(1, 1, 0, 0),
        })
    };
    let records = starting_with(
        run_started(),
        vec![
            model_call(2, 5)?,
            model_call(2, 3)?,
            model_call(3, 9)?,
            Event::Spent {
                generation: 2,
                source: SpendSource::Candidate {
                    exp: exp(3)?,
                    trial: 7,
                },
                usage: Usage::compute(1, 1),
            },
        ],
    )?;
    let state = fold(&records)?;
    assert_eq!(state.next_model_call(1), 1);
    assert_eq!(state.next_model_call(2), 6);
    assert_eq!(state.next_model_call(3), 10);
    assert_eq!(state.next_model_call(4), 0);
    Ok(())
}

#[test]
fn next_model_call_saturates_at_the_last_index() -> TestResult {
    let records = with(Event::Spent {
        generation: 4,
        source: SpendSource::ModelCall {
            call: u32::MAX,
            model: haiku()?,
        },
        usage: Usage::tokens(1, 1, 0, 0),
    })?;
    assert_eq!(fold(&records)?.next_model_call(4), u32::MAX);
    Ok(())
}

#[test]
fn judged_is_true_exactly_for_recorded_trials() -> TestResult {
    let state = folded()?;
    let judge = judge()?;
    for trial in 0..3 {
        assert!(state.judged(&baseline(), &judge, trial));
    }
    assert!(!state.judged(&baseline(), &judge, 3));
    assert!(state.judged(&exp(2)?, &judge, 0));
    assert!(!state.judged(&exp(2)?, &judge, 1));
    assert!(!state.judged(&exp(9)?, &judge, 0));
    let other = JudgeVersion::from_str(&"cd".repeat(32))?;
    assert!(!state.judged(&baseline(), &other, 0));
    Ok(())
}

#[test]
fn stops_yields_every_recorded_stop_in_key_order() -> TestResult {
    let records = starting_with(
        run_started(),
        vec![
            Event::StopRequested {
                generation: 7,
                reason: StopReason::SpendCap,
            },
            Event::StopRequested {
                generation: 3,
                reason: StopReason::ProviderAuth,
            },
        ],
    )?;
    let state = fold(&records)?;
    assert_eq!(
        state.stops().collect::<Vec<_>>(),
        vec![
            (3, StopReason::UserStop),
            (3, StopReason::ProviderAuth),
            (7, StopReason::SpendCap),
        ]
    );
    assert_eq!(state.stop_requested(), Some((3, StopReason::UserStop)));
    assert_eq!(fold(&[rec(run_started())])?.stops().count(), 0);
    Ok(())
}

#[test]
fn judgment_logs_and_proposal_batch_are_the_recorded_values() -> TestResult {
    let e1 = exp(1)?;
    let logs = stage_logs(baseline(), 3);
    let records = with(Event::Judged {
        exp: baseline(),
        judge: judge()?,
        trial: 3,
        outcome: scored(100.0)?,
        backend: backend(),
        logs: Some(logs.clone()),
    })?;
    let state = fold(&records)?;
    let judgments: BTreeMap<u32, Option<BTreeMap<Stage, BlobId>>> = state
        .trials(&baseline(), &judge()?)
        .map(|(trial, judgment)| (trial, judgment.logs().cloned()))
        .collect();
    assert_eq!(judgments.get(&3), Some(&Some(logs)));
    assert_eq!(judgments.get(&0), Some(&None));
    assert_eq!(state.proposal(&e1).and_then(|p| p.batch()), None);

    let batch = BlobId::of(b"batch");
    let records = with(Event::Proposed {
        exp: exp(10)?,
        parent: e1,
        generation: 3,
        tier: Tier::Sweep,
        diff: BlobId::of(b"diff"),
        rationale: BlobId::of(b"rationale"),
        origin: Origin::Sweep,
        batch: Some(batch),
    })?;
    let state = fold(&records)?;
    assert_eq!(
        state.proposal(&exp(10)?).and_then(|p| p.batch()),
        Some(&batch)
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Corruption
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn an_empty_ledger_is_refused() -> TestResult {
    assert!(matches!(fold_err(&[])?, FoldError::Empty));
    Ok(())
}

#[test]
fn the_first_record_must_be_run_started() -> TestResult {
    let records = [
        rec(judged(baseline(), 0, scored(1.0)?)?),
        rec(run_started()),
    ];
    assert!(matches!(fold_err(&records)?, FoldError::FirstNotRunStarted));
    Ok(())
}

#[test]
fn a_second_different_run_started_is_refused() -> TestResult {
    let records = with(Event::RunStarted {
        run: RunId::of(&manifest(), &base(), STARTED_AT + 1),
        manifest: manifest(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT + 1,
        root_image: None,
    })?;
    let error = fold_err(&records)?;
    assert!(matches!(error, FoldError::DuplicateRunStarted));
    // Deliberate-violation evidence: the exact message an operator sees.
    assert_eq!(
        error.to_string(),
        "the ledger has more than one RunStarted record"
    );
    Ok(())
}

#[test]
fn a_wrong_run_id_is_refused() -> TestResult {
    let records = [rec(Event::RunStarted {
        run: RunId::of(&manifest(), &base(), STARTED_AT + 1),
        manifest: manifest(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
        root_image: None,
    })];
    let error = fold_err(&records)?;
    assert!(matches!(
        error,
        FoldError::RunIdMismatch { recorded, computed }
            if recorded == RunId::of(&manifest(), &base(), STARTED_AT + 1) && computed == run()
    ));
    Ok(())
}

#[test]
fn a_wrong_baseline_is_refused() -> TestResult {
    let records = [rec(Event::RunStarted {
        run: run(),
        manifest: manifest(),
        base: base(),
        baseline: ExperimentId::baseline(&TreeId::of(b"other")),
        started_at: STARTED_AT,
        root_image: None,
    })];
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BaselineMismatch { computed, .. } if computed == baseline()
    ));
    Ok(())
}

#[test]
fn the_same_key_with_different_content_conflicts() -> TestResult {
    let records = with(judged(exp(1)?, 0, scored(1.0)?)?)?;
    assert!(matches!(fold_err(&records)?, FoldError::KeyConflict { .. }));
    Ok(())
}

#[test]
fn a_second_proposal_of_one_experiment_is_a_duplicate_proposal() -> TestResult {
    // `Proposed` is keyed by experiment alone: the same experiment proposed again in a later generation, from
    // another parent, is the loop's admission failure, named as such rather than as a rewritten record.
    let e1 = exp(1)?;
    let records = with(proposed(e1, baseline(), 2, Tier::SingleShot)?)?;
    assert_eq!(
        fold_err(&records)?,
        FoldError::DuplicateProposal { exp: e1 }
    );
    // An identical re-proposal is the same record and folds.
    let records = with(proposed(e1, baseline(), 1, Tier::SingleShot)?)?;
    assert!(fold(&records).is_ok());
    Ok(())
}

#[test]
fn two_different_notes_at_one_ordinal_are_a_note_conflict() -> TestResult {
    let records = with(Event::Steered {
        generation: 2,
        ordinal: 0,
        note: BlobId::of(b"another note zero"),
    })?;
    assert_eq!(
        fold_err(&records)?,
        FoldError::NoteConflict {
            generation: 2,
            ordinal: 0
        }
    );
    Ok(())
}

#[test]
fn judged_for_an_unknown_experiment_is_refused() -> TestResult {
    let records = with(judged(exp(9)?, 0, scored(1.0)?)?)?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::UnknownExperiment { exp } if exp == exp_or_zero(9)
    ));
    Ok(())
}

/// `exp(n)`, for pattern guards; falls back to the baseline only if hex parsing were ever to fail, which would
/// make the guard false.
fn exp_or_zero(n: u64) -> ExperimentId {
    exp(n).unwrap_or_else(|_| baseline())
}

#[test]
fn spent_for_an_unknown_experiment_is_refused() -> TestResult {
    let records = with(Event::Spent {
        generation: 1,
        source: SpendSource::Candidate {
            exp: exp(9)?,
            trial: 0,
        },
        usage: Usage::compute(1, 1),
    })?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::UnknownExperiment { .. }
    ));
    Ok(())
}

#[test]
fn an_acknowledgement_for_an_unknown_experiment_is_refused() -> TestResult {
    let records = with(Event::ReportAcknowledged {
        exp: exp(9)?,
        report: ReportHash::of(b"report"),
    })?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::UnknownExperiment { .. }
    ));
    Ok(())
}

#[test]
fn a_parent_that_is_unknown_is_refused() -> TestResult {
    let records = with(proposed(exp(10)?, exp(9)?, 3, Tier::SingleShot)?)?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadParent { exp, parent } if exp == exp_or_zero(10) && parent == exp_or_zero(9)
    ));
    Ok(())
}

#[test]
fn a_parent_from_the_same_generation_is_refused() -> TestResult {
    let records = with(proposed(exp(10)?, exp(1)?, 1, Tier::SingleShot)?)?;
    assert!(matches!(fold_err(&records)?, FoldError::BadParent { .. }));
    Ok(())
}

#[test]
fn a_parent_from_a_later_generation_is_refused() -> TestResult {
    let records = with(proposed(exp(10)?, exp(3)?, 1, Tier::SingleShot)?)?;
    assert!(matches!(fold_err(&records)?, FoldError::BadParent { .. }));
    Ok(())
}

#[test]
fn proposing_the_baseline_is_refused() -> TestResult {
    let records = with(proposed(baseline(), exp(1)?, 3, Tier::SingleShot)?)?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadParent { exp, .. } if exp == baseline()
    ));
    Ok(())
}

#[test]
fn a_proposal_at_generation_zero_is_refused() -> TestResult {
    let records = with(proposed(exp(10)?, baseline(), 0, Tier::SingleShot)?)?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::ProposedAtGenerationZero { .. }
    ));
    Ok(())
}

#[test]
fn a_sweep_tier_with_a_model_origin_is_refused() -> TestResult {
    let records = with(proposed_with(
        exp(10)?,
        baseline(),
        3,
        Tier::Sweep,
        origin_for(Tier::SingleShot)?,
    ))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::OriginTierMismatch { .. }
    ));
    Ok(())
}

#[test]
fn a_model_tier_with_a_sweep_origin_is_refused() -> TestResult {
    let records = with(proposed_with(
        exp(10)?,
        baseline(),
        3,
        Tier::SingleShot,
        Origin::Sweep,
    ))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::OriginTierMismatch { .. }
    ));
    Ok(())
}

#[test]
fn a_winner_from_another_generation_is_refused() -> TestResult {
    let records = with(selected(3, DecisionKind::Improved { winner: exp(1)? }, 0))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadDecision { generation: 3 }
    ));
    Ok(())
}

#[test]
fn a_winner_never_proposed_is_refused() -> TestResult {
    let records = with(selected(3, DecisionKind::Improved { winner: exp(9)? }, 0))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadDecision { generation: 3 }
    ));
    Ok(())
}

#[test]
fn a_kept_incumbent_that_is_not_the_incumbent_is_refused() -> TestResult {
    let records = with(selected(
        3,
        DecisionKind::Kept {
            incumbent: baseline(),
        },
        0,
    ))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadDecision { generation: 3 }
    ));
    Ok(())
}

#[test]
fn a_decision_at_generation_zero_is_refused() -> TestResult {
    let records = with(selected(0, DecisionKind::NoValidCandidates, 0))?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::BadDecision { generation: 0 }
    ));
    Ok(())
}

#[test]
fn an_answer_without_a_request_is_refused() -> TestResult {
    let request = review_id(ReviewKind::JudgeDesign);
    let records = with(Event::Reviewed {
        request,
        answer: ReviewAnswer::Declined,
    })?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::AnswerWithoutRequest { request: found } if found == request
    ));
    Ok(())
}

#[test]
fn two_manifests_for_one_generation_are_refused() -> TestResult {
    let records = with(Event::ManifestChanged {
        manifest: ManifestId::of(b"m3"),
        from_generation: 2,
    })?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::ManifestConflict { generation: 2 }
    ));
    Ok(())
}

#[test]
fn a_generation_zero_manifest_must_match_run_started() -> TestResult {
    let records = with(Event::ManifestChanged {
        manifest: ManifestId::of(b"m3"),
        from_generation: 0,
    })?;
    assert!(matches!(
        fold_err(&records)?,
        FoldError::ManifestConflict { generation: 0 }
    ));
    Ok(())
}

#[test]
fn usage_overflow_is_refused() -> TestResult {
    let records = with(Event::Spent {
        generation: 1,
        source: SpendSource::ModelCall {
            call: 1,
            model: haiku()?,
        },
        usage: Usage::tokens(u64::MAX, 0, 0, 0),
    })?;
    assert!(matches!(fold_err(&records)?, FoldError::UsageOverflow));
    Ok(())
}

#[test]
fn an_exact_duplicate_is_ignored() -> TestResult {
    let mut records = valid_ledger()?;
    records.extend(valid_ledger()?);
    assert_eq!(fold(&records)?, folded()?);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Properties over generated valid ledgers
// ---------------------------------------------------------------------------------------------------------------

/// One generated proposal: a parent choice among the baseline and every earlier experiment, whether it is a
/// sweep proposal, and one outcome choice per trial.
type ProposalPlan = (usize, bool, Vec<u8>);

/// One generated generation.
#[derive(Clone, Debug)]
struct GenerationPlan {
    proposals: Vec<ProposalPlan>,
    /// 0 to 2 picks `Improved`, `Kept`, or `NoValidCandidates`; 3 records no decision, as for a generation in progress.
    decision: u8,
    winner: usize,
    model_calls: u32,
    notes: u32,
    /// Whether the generation's proposals carry a batch blob.
    batch: bool,
}

/// One generated ledger.
#[derive(Clone, Debug)]
struct LedgerPlan {
    baseline_trials: Vec<u8>,
    generations: Vec<GenerationPlan>,
    /// Distinct numbers for candidate experiment IDs, in random order.
    ids: Vec<u64>,
    manifest_change: bool,
    review: Option<bool>,
    /// Stop requests, each a generation index and a reason choice.
    stops: Vec<(u8, u8)>,
    acknowledge: bool,
    /// Whether `RunStarted` records a root image.
    root_image: bool,
}

/// Every generated ledger has at most 4 generations of at most 4 proposals.
const MAX_EXPERIMENTS: usize = 16;

fn generation_plan() -> impl Strategy<Value = GenerationPlan> {
    (
        prop::collection::vec(
            (
                any::<usize>(),
                any::<bool>(),
                prop::collection::vec(any::<u8>(), 1..=3),
            ),
            1..=4,
        ),
        0_u8..=3,
        any::<usize>(),
        0_u32..=2,
        0_u32..=2,
        any::<bool>(),
    )
        .prop_map(
            |(proposals, decision, winner, model_calls, notes, batch)| GenerationPlan {
                proposals,
                decision,
                winner,
                model_calls,
                notes,
                batch,
            },
        )
}

fn ledger_plan() -> impl Strategy<Value = LedgerPlan> {
    (
        prop::collection::vec(any::<u8>(), 1..=3),
        prop::collection::vec(generation_plan(), 1..=4),
        prop::collection::btree_set(any::<u64>(), MAX_EXPERIMENTS)
            .prop_map(|ids| ids.into_iter().collect::<Vec<u64>>())
            .prop_shuffle(),
        any::<bool>(),
        proptest::option::of(any::<bool>()),
        prop::collection::vec((any::<u8>(), any::<u8>()), 0..=3),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(
                baseline_trials,
                generations,
                ids,
                manifest_change,
                review,
                stops,
                acknowledge,
                root_image,
            )| {
                LedgerPlan {
                    baseline_trials,
                    generations,
                    ids,
                    manifest_change,
                    review,
                    stops,
                    acknowledge,
                    root_image,
                }
            },
        )
}

/// A trial outcome from a small fixed set. No generated outcome uses `Stage::Build`, which the tamper property
/// relies on.
fn outcome_of(choice: u8) -> Result<Outcome, Box<dyn Error>> {
    Ok(match choice % 6 {
        0 => scored(1.0)?,
        1 => scored(2.5)?,
        2 => scored(100.0)?,
        3 => Outcome::Crashed { stage: Stage::Run },
        4 => Outcome::TimedOut {
            stage: Stage::Judge,
        },
        _ => Outcome::Rejected(Reason::BuildFailed),
    })
}

fn pick<T: Copy>(items: &[T], choice: usize) -> Result<T, Box<dyn Error>> {
    choice
        .checked_rem(items.len())
        .and_then(|index| items.get(index))
        .copied()
        .ok_or_else(|| "nothing to pick from".into())
}

/// The stop reason a generated choice names.
const fn reason_of(choice: u8) -> StopReason {
    match choice % 4 {
        0 => StopReason::UserStop,
        1 => StopReason::SpendCap,
        2 => StopReason::ProviderLimit,
        _ => StopReason::ProviderAuth,
    }
}

/// Appends the Judged and Spent records of `exp`'s trials. A trial whose outcome choice has an even sixth carries
/// stage log blobs, so generated ledgers mix `Judged` records with and without `logs`.
fn push_trials(
    records: &mut Vec<Record>,
    exp: ExperimentId,
    generation: u32,
    outcomes: &[u8],
) -> Result<(), Box<dyn Error>> {
    for (trial, choice) in (0_u32..).zip(outcomes) {
        let logs = (choice / 6 % 2 == 0).then(|| stage_logs(exp, trial));
        records.push(rec(Event::Judged {
            exp,
            judge: judge()?,
            trial,
            outcome: outcome_of(*choice)?,
            backend: backend(),
            logs,
        }));
        let source = if exp == baseline() {
            SpendSource::Judge { exp, trial }
        } else {
            SpendSource::Candidate { exp, trial }
        };
        records.push(rec(Event::Spent {
            generation,
            source,
            usage: Usage::compute(u64::from(trial) + 1, u64::from(*choice) + 1),
        }));
    }
    Ok(())
}

/// Builds a valid ledger from `plan`, in append order: parents are always from an earlier generation, tiers
/// and origins agree, and every decision is consistent with the generation's proposals and incumbent.
fn build(plan: &LedgerPlan) -> Result<Vec<Record>, Box<dyn Error>> {
    let root_image = plan.root_image.then(|| ImageDigest::of(b"locked root"));
    let mut records = vec![rec(run_started_with(root_image))];
    push_trials(&mut records, baseline(), 0, &plan.baseline_trials)?;

    let mut ids = plan.ids.iter();
    let mut earlier = vec![baseline()];
    let mut incumbent = baseline();
    for (generation, gen_plan) in (1_u32..).zip(&plan.generations) {
        let mut this_generation = Vec::new();
        for (parent_choice, sweep, outcomes) in &gen_plan.proposals {
            let id = exp(*ids.next().ok_or("ran out of experiment IDs")?)?;
            let parent = pick(&earlier, *parent_choice)?;
            let tier = if *sweep {
                Tier::Sweep
            } else {
                Tier::SingleShot
            };
            let batch = gen_plan
                .batch
                .then(|| BlobId::of(format!("batch {generation}").as_bytes()));
            records.push(rec(Event::Proposed {
                exp: id,
                parent,
                generation,
                tier,
                diff: BlobId::of(id.as_bytes()),
                rationale: BlobId::of(b"rationale"),
                origin: origin_for(tier)?,
                batch,
            }));
            push_trials(&mut records, id, generation, outcomes)?;
            this_generation.push(id);
        }
        for call in 0..gen_plan.model_calls {
            records.push(rec(Event::Spent {
                generation,
                source: SpendSource::ModelCall {
                    call,
                    model: haiku()?,
                },
                usage: Usage::tokens(u64::from(call) + 10, 5, 1, 2),
            }));
        }
        for ordinal in 0..gen_plan.notes {
            records.push(rec(Event::Steered {
                generation,
                ordinal,
                note: BlobId::of(format!("note {generation} {ordinal}").as_bytes()),
            }));
        }
        let proposed_count = u32::try_from(this_generation.len())?;
        let kind = match gen_plan.decision {
            0 => Some(DecisionKind::Improved {
                winner: pick(&this_generation, gen_plan.winner)?,
            }),
            1 => Some(DecisionKind::Kept { incumbent }),
            2 => Some(DecisionKind::NoValidCandidates),
            _ => None,
        };
        if let Some(kind) = kind {
            if let DecisionKind::Improved { winner } = kind {
                incumbent = winner;
            }
            records.push(rec(selected(generation, kind, proposed_count)));
        }
        earlier.extend(this_generation);
    }

    if plan.manifest_change {
        records.push(rec(Event::ManifestChanged {
            manifest: ManifestId::of(b"m2"),
            from_generation: 1,
        }));
    }
    if let Some(answered) = plan.review {
        let request = review_id(ReviewKind::Params);
        records.push(rec(Event::ReviewRequested {
            request,
            kind: ReviewKind::Params,
        }));
        if answered {
            records.push(rec(Event::Reviewed {
                request,
                answer: ReviewAnswer::Declined,
            }));
        }
    }
    for (generation, reason) in &plan.stops {
        records.push(rec(Event::StopRequested {
            generation: u32::from(*generation % 6),
            reason: reason_of(*reason),
        }));
    }
    if plan.acknowledge {
        records.push(rec(Event::ReportAcknowledged {
            exp: incumbent,
            report: ReportHash::of(b"report"),
        }));
    }
    Ok(records)
}

/// A generated ledger, the records after `RunStarted` shuffled, and a mask choosing records to duplicate.
fn shuffled_ledger() -> impl Strategy<Value = (Result<Vec<Record>, String>, Vec<Record>, Vec<bool>)>
{
    ledger_plan().prop_flat_map(|plan| {
        let built = build(&plan).map_err(|error| error.to_string());
        let rest: Vec<Record> = built
            .as_ref()
            .map(|records| records.iter().skip(1).cloned().collect())
            .unwrap_or_default();
        let len = rest.len();
        (
            Just(built),
            Just(rest).prop_shuffle(),
            prop::collection::vec(any::<bool>(), len),
        )
    })
}

/// The `RunStarted` record of a built ledger.
fn head(records: &[Record]) -> Result<&Record, TestCaseError> {
    records
        .first()
        .ok_or_else(|| TestCaseError::fail("a built ledger starts with RunStarted"))
}

proptest! {
    #[test]
    fn generated_ledgers_fold((built, _rest, _mask) in shuffled_ledger()) {
        let records = built.map_err(TestCaseError::fail)?;
        let state = fold(&records)?;
        prop_assert_eq!(state.baseline(), &baseline());
        let judge = judge().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let baseline_trials = state.trials(&baseline(), &judge).count();
        prop_assert!(baseline_trials >= 1, "the baseline has {} trials", baseline_trials);
    }

    #[test]
    fn fold_is_permutation_invariant((built, rest, _mask) in shuffled_ledger()) {
        let records = built.map_err(TestCaseError::fail)?;
        let expected = fold(&records)?;
        let shuffled = fold(std::iter::once(head(&records)?).chain(&rest))?;
        prop_assert_eq!(shuffled.root_image(), expected.root_image());
        prop_assert_eq!(shuffled.backend_classes(), expected.backend_classes());
        prop_assert_eq!(shuffled.stops().collect::<Vec<_>>(), expected.stops().collect::<Vec<_>>());
        prop_assert_eq!(shuffled.stop_requested(), expected.stop_requested());
        for generation in 0..=6 {
            prop_assert_eq!(shuffled.next_model_call(generation), expected.next_model_call(generation));
        }
        for (id, spend) in expected.spends() {
            prop_assert!(shuffled.has_spend(id), "spend {} of generation {}", id, spend.generation());
        }
        let judge = judge().map_err(|error| TestCaseError::fail(error.to_string()))?;
        let exps: Vec<ExperimentId> = std::iter::once(baseline())
            .chain((1..=4).flat_map(|generation| expected.proposals_in(generation).map(|(id, _)| *id)))
            .collect();
        for exp in exps {
            for (trial, judgment) in expected.trials(&exp, &judge) {
                prop_assert!(shuffled.judged(&exp, &judge, trial));
                let other = shuffled.trials(&exp, &judge).find(|(t, _)| *t == trial).map(|(_, j)| j.logs().cloned());
                prop_assert_eq!(other, Some(judgment.logs().cloned()));
            }
            let batch = expected.proposal(&exp).map(|p| p.batch().copied());
            prop_assert_eq!(shuffled.proposal(&exp).map(|p| p.batch().copied()), batch);
        }
        prop_assert_eq!(shuffled, expected);
    }

    #[test]
    fn duplicate_replay_is_idempotent((built, rest, mask) in shuffled_ledger()) {
        let records = built.map_err(TestCaseError::fail)?;
        let expected = fold(&records)?;

        // A copy of a chosen subset, appended after the ledger.
        let chosen = records
            .iter()
            .skip(1)
            .zip(&mask)
            .filter(|(_, keep)| **keep)
            .map(|(record, _)| record);
        prop_assert_eq!(&fold(records.iter().chain(chosen))?, &expected);

        // A copy of every record, RunStarted included.
        prop_assert_eq!(&fold(records.iter().chain(&records))?, &expected);

        // A shuffled copy interleaved after RunStarted.
        let replayed = std::iter::once(head(&records)?)
            .chain(&rest)
            .chain(records.iter().skip(1));
        prop_assert_eq!(&fold(replayed)?, &expected);
    }

    #[test]
    fn tampered_record_conflicts(
        (built, _rest, _mask) in shuffled_ledger(),
        choice in any::<usize>(),
    ) {
        let records = built.map_err(TestCaseError::fail)?;
        let judged_records: Vec<&Record> = records
            .iter()
            .filter(|record| matches!(record.event(), Event::Judged { .. }))
            .collect();
        let target = pick(&judged_records, choice).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let Event::Judged { exp, judge, trial, outcome, backend, logs } = target.event().clone() else {
            return Err(TestCaseError::fail("the filter keeps Judged records only"));
        };
        let replacement = Outcome::Crashed { stage: Stage::Build };
        prop_assert_ne!(&outcome, &replacement);
        let tampered = rec(Event::Judged { exp, judge, trial, outcome: replacement, backend, logs });

        let result = fold(records.iter().chain(std::iter::once(&tampered)));
        let conflicts = matches!(result, Err(FoldError::KeyConflict { .. }));
        prop_assert!(conflicts, "a tampered Judged record must be a KeyConflict");
    }
}
