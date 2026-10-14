//! The fold from ledger records to `RunState` (CORE-09): accessors over a valid ledger, one test per corruption
//! case, and properties for order-insensitivity, duplicate replay, and tamper detection.
//!
//! Ledgers are built from public constructors only. Candidate experiment IDs are parsed from 64-character hex
//! strings: any 32 bytes form an ID, and the fold never recomputes a candidate's ID from its diff.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, ModelId, PromptHash, ReportHash,
    RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation,
    Origin, Record, ReviewAnswer, ReviewId, ReviewKind, SpendSource, StopReason,
};
use minos_core::manifest::{Arch, JudgeVersion, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::state::{FoldError, RunState, fold};
use minos_core::usage::Usage;

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
    Event::RunStarted {
        run: run(),
        manifest: manifest(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
    }
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
