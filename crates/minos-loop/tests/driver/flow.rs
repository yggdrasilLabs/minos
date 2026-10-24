//! Whole generations through the driver, for the sweep and the `single_shot` tiers, on fakes (LOOP-04, LOOP-09):
//! the record order (07-CONTEXT D-07, D-18), admission's drop count, an empty propose step, an unscored baseline
//! (07-RESEARCH Pitfall 12), calibration, the start and resume refusals (D-10, D-13, D-19), a provider stop
//! (06-CONTEXT D-13, D-15), a conflicting append (D-08), and the observer's event order.

use std::collections::BTreeSet;

use minos_core::id::{ExperimentId, ImageDigest};
use minos_core::ledger::{DecisionKind, Event, Origin, SpendSource, StopReason};
use minos_core::policy::NoiseVerdict;
use minos_loop::{Calibrated, Driver, DriverError, JobFailure, LoopEvent, NoopObserver, RunEnd};
use minos_propose::FakeReply;
use minos_propose::provider::{EmptyCause, ProviderError, ProviderStop};
use minos_store::{MemoryStore, StoreError};

use crate::support::{
    FMT_FAST, FakeJobs, FaultStore, Fixture, PARSE_FAST, PARSE_FASTER, PRICES_WITHOUT_HAIKU,
    Recorder, SINGLE_SHOT_MANIFEST, STARTED_AT, SWEEP_MANIFEST, TestResult, calls, candidate_usage,
    haiku, judge_usage, kinds, ledger, options, proposal, reply, root, run_failed, scored,
    single_shot_proposer, sweep_proposer, token_usage,
};

/// The seq of the first event `matches` accepts.
fn seq_of(events: &[(u64, Event)], matches: impl Fn(&Event) -> bool) -> Option<u64> {
    events
        .iter()
        .find(|(_, event)| matches(event))
        .map(|(seq, _)| *seq)
}

/// The experiments proposed in `generation`, in seq order, with their parent.
fn proposed_in(events: &[(u64, Event)], generation: u32) -> Vec<(ExperimentId, ExperimentId)> {
    events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Proposed {
                exp,
                parent,
                generation: g,
                ..
            } if *g == generation => Some((*exp, *parent)),
            _ => None,
        })
        .collect()
}

/// `generation`'s decision kind and drop count.
fn selected(events: &[(u64, Event)], generation: u32) -> Option<(DecisionKind, u32)> {
    events.iter().find_map(|(_, event)| match event {
        Event::Selected {
            generation: g,
            decision,
        } if *g == generation => Some((*decision.kind(), decision.dropped())),
        _ => None,
    })
}

/// Record before act: every `Judged` follows its candidate `Spent` and, for a scored trial, its judge `Spent`,
/// every proposal precedes its trials, and every `Selected` follows the last `Judged` of its generation.
fn assert_recorded_before_acted(events: &[(u64, Event)]) -> TestResult {
    for (seq, event) in events {
        match event {
            Event::Judged {
                exp,
                trial,
                outcome,
                ..
            } => {
                let spent = |judge: bool| {
                    seq_of(events, |other| match other {
                        Event::Spent { source, .. } => {
                            *source
                                == if judge {
                                    SpendSource::Judge {
                                        exp: *exp,
                                        trial: *trial,
                                    }
                                } else {
                                    SpendSource::Candidate {
                                        exp: *exp,
                                        trial: *trial,
                                    }
                                }
                        }
                        _ => false,
                    })
                };
                if spent(false).is_none_or(|spent| spent >= *seq) {
                    return Err(
                        format!("judged at seq {seq} has no earlier candidate spend").into(),
                    );
                }
                if outcome.scorecard().is_some() && spent(true).is_none_or(|spent| spent >= *seq) {
                    return Err(format!("judged at seq {seq} has no earlier judge spend").into());
                }
                let proposed = seq_of(
                    events,
                    |other| matches!(other, Event::Proposed { exp: e, .. } if e == exp),
                );
                if proposed.is_some_and(|proposed| proposed >= *seq) {
                    return Err(format!("judged at seq {seq} precedes its proposal").into());
                }
            }
            Event::Selected { generation, .. } => {
                let exps: BTreeSet<ExperimentId> = proposed_in(events, *generation)
                    .into_iter()
                    .map(|(exp, _)| exp)
                    .collect();
                let late = events.iter().any(|(other, event)| {
                    other > seq && matches!(event, Event::Judged { exp, .. } if exps.contains(exp))
                });
                if late {
                    return Err(format!("selected at seq {seq} precedes a judged trial").into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[test]
fn sweep_run_to_two_records_calibration_then_two_generations_in_order() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(2)?, RunEnd::Reached { generation: 2 });

    let events = ledger(&store, &fixture.run())?;
    let Some((
        _,
        Event::RunStarted {
            root_image,
            baseline,
            ..
        },
    )) = events.first()
    else {
        return Err("the ledger does not start with RunStarted".into());
    };
    assert_eq!(*root_image, Some(root()));
    let baseline = *baseline;
    assert_eq!(
        kinds(&events).get(1..10),
        Some(["spent", "spent", "judged"].repeat(3).as_slice())
    );

    let first = proposed_in(&events, 1);
    assert!(!first.is_empty());
    assert!(first.iter().all(|(_, parent)| *parent == baseline));
    let batches: BTreeSet<_> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Proposed {
                generation: 1,
                batch,
                ..
            } => Some(*batch),
            _ => None,
        })
        .collect();
    assert_eq!(batches.len(), 1);
    assert!(batches.iter().all(Option::is_some));

    let Some((DecisionKind::Improved { winner }, _)) = selected(&events, 1) else {
        return Err("generation 1 did not improve".into());
    };
    assert!(first.iter().any(|(exp, _)| *exp == winner));
    let second = proposed_in(&events, 2);
    assert!(!second.is_empty());
    assert!(second.iter().all(|(_, parent)| *parent == winner));
    assert!(selected(&events, 2).is_some());
    assert_recorded_before_acted(&events)?;

    let spends_ok = events.iter().all(|(_, event)| match event {
        Event::Spent {
            source: SpendSource::Candidate { .. },
            usage,
            ..
        } => *usage == candidate_usage(),
        Event::Spent {
            source: SpendSource::Judge { .. },
            usage,
            ..
        } => *usage == judge_usage(),
        _ => true,
    });
    assert!(spends_ok);
    Ok(())
}

#[test]
fn single_shot_records_the_call_spend_before_its_proposals() -> TestResult {
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![reply(vec![
        proposal("faster parse", &[("src/parse.rs", PARSE_FAST)]),
        proposal("faster fmt", &[("src/fmt.rs", FMT_FAST)]),
    ])?]);
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(1)?, RunEnd::Reached { generation: 1 });
    assert_eq!(calls(&proposer)?, 1);

    let events = ledger(&store, &fixture.run())?;
    let model = haiku()?;
    let spend = seq_of(&events, |event| {
        matches!(event, Event::Spent {
            generation: 1,
            source: SpendSource::ModelCall { call: 0, model: m },
            usage,
        } if *m == model && *usage == token_usage())
    })
    .ok_or("no model call spend")?;
    let first_proposed =
        seq_of(&events, |event| matches!(event, Event::Proposed { .. })).ok_or("no proposal")?;
    assert!(spend < first_proposed);

    let origins: Vec<&Origin> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Proposed { origin, .. } => Some(origin),
            _ => None,
        })
        .collect();
    assert_eq!(origins.len(), 2);
    assert!(
        origins
            .iter()
            .all(|origin| matches!(origin, Origin::Model { model: m, .. } if *m == model))
    );
    assert_eq!(jobs.requests()?.len(), 5);
    assert_recorded_before_acted(&events)
}

#[test]
fn single_shot_repeat_of_an_earlier_experiment_is_dropped_and_counted() -> TestResult {
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![
        reply(vec![
            proposal("faster parse", &[("src/parse.rs", PARSE_FAST)]),
            proposal("faster fmt", &[("src/fmt.rs", FMT_FAST)]),
        ])?,
        reply(vec![
            proposal("faster parse again", &[("src/parse.rs", PARSE_FAST)]),
            proposal("even faster parse", &[("src/parse.rs", PARSE_FASTER)]),
        ])?,
    ]);
    // Every candidate ties the baseline, so the baseline is kept and generation 2 proposes from it again.
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(2)?, RunEnd::Reached { generation: 2 });

    let events = ledger(&store, &fixture.run())?;
    assert!(matches!(
        selected(&events, 1),
        Some((DecisionKind::Kept { .. }, 0))
    ));
    let first = proposed_in(&events, 1);
    let second = proposed_in(&events, 2);
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 1);
    assert!(
        second
            .iter()
            .all(|(exp, _)| first.iter().all(|(earlier, _)| earlier != exp))
    );
    assert!(matches!(selected(&events, 2), Some((_, 1))));
    assert_recorded_before_acted(&events)
}

#[test]
fn empty_provider_reply_concludes_with_no_valid_candidates() -> TestResult {
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![FakeReply::Empty {
        cause: EmptyCause::Refusal,
        usage: token_usage(),
    }]);
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(1)?, RunEnd::Reached { generation: 1 });

    let events = ledger(&store, &fixture.run())?;
    assert!(proposed_in(&events, 1).is_empty());
    assert_eq!(
        selected(&events, 1),
        Some((DecisionKind::NoValidCandidates, 0))
    );
    let model = haiku()?;
    let spend = seq_of(
        &events,
        |event| matches!(event, Event::Spent { source: SpendSource::ModelCall { call: 0, model: m }, .. } if *m == model),
    );
    let select = seq_of(&events, |event| matches!(event, Event::Selected { .. }));
    assert!(spend.is_some_and(|spend| select.is_some_and(|select| spend < select)));
    Ok(())
}

#[test]
fn unscored_baseline_stops_before_any_model_call() -> TestResult {
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(Vec::new());
    let jobs = FakeJobs::new(run_failed);
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    let result = driver.run_to(1);
    assert!(
        matches!(result, Err(DriverError::BaselineUnscored { trials: 3 })),
        "{result:?}"
    );
    assert_eq!(calls(&proposer)?, 0);
    let events = ledger(&store, &fixture.run())?;
    assert!(
        !events
            .iter()
            .any(|(_, event)| matches!(event, Event::Proposed { .. } | Event::Selected { .. }))
    );
    Ok(())
}

#[test]
fn calibrate_returns_a_floor_over_three_trials_and_appends_nothing_again() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, f64::from(request.trial()).mul_add(0.1, 1.0)));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    let Calibrated::Ready(calibration) = driver.calibrate()? else {
        return Err("calibration stopped".into());
    };
    assert_eq!(calibration.trials(), 3);
    let floor = calibration.floor().ok_or("no noise floor")?;
    assert_eq!(floor.trials(), 3);
    assert_eq!(floor.skipped(), 0);
    assert!((floor.min().value() - 1.0).abs() < 1e-9);
    assert!((floor.max().value() - 1.2).abs() < 1e-9);
    // A 5% threshold at a median of 1.1 is under the 0.2 spread: improvements would be noise.
    assert_eq!(calibration.verdict(), NoiseVerdict::Refuse);

    let lines = ledger(&store, &fixture.run())?.len();
    assert_eq!(driver.calibrate()?, Calibrated::Ready(calibration));
    assert_eq!(ledger(&store, &fixture.run())?.len(), lines);
    assert_eq!(jobs.requests()?.len(), 3);
    Ok(())
}

#[test]
fn start_with_an_unpriced_model_records_nothing() -> TestResult {
    let fixture = Fixture::new(SINGLE_SHOT_MANIFEST, PRICES_WITHOUT_HAIKU)?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(Vec::new());
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let result = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    );
    let Err(DriverError::PricesMissing { models }) = result else {
        return Err("start did not refuse the unpriced model".into());
    };
    assert_eq!(models, BTreeSet::from([haiku()?]));
    assert!(ledger(&store, &fixture.run())?.is_empty());
    Ok(())
}

#[test]
fn start_refused_by_the_job_check_records_nothing() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let refusal = JobFailure::Config {
        what: "the candidate spec is over the duration ceiling",
    };
    let jobs = FakeJobs::with_check(|request| scored(request, 1.0), Err(refusal));
    let result = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    );
    assert!(
        matches!(
            result,
            Err(DriverError::Job {
                what: "the candidate spec is over the duration ceiling"
            })
        ),
        "start was not refused by the job check"
    );
    assert!(ledger(&store, &fixture.run())?.is_empty());
    assert!(jobs.requests()?.is_empty());
    Ok(())
}

#[test]
fn resume_with_another_root_image_is_refused_naming_both() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let run = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?
    .run_id();

    let rebuilt = ImageDigest::of(b"rebuilt root image");
    let result = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs_with_root(rebuilt),
        options(),
        run,
    );
    let Err(DriverError::RootImageChanged { recorded, current }) = result else {
        return Err("resume did not refuse the rebuilt image".into());
    };
    assert_eq!(recorded, Some(root()));
    assert_eq!(current, rebuilt);
    Ok(())
}

#[test]
fn resume_after_a_budget_edit_records_manifest_changed_and_continues() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(1)?, RunEnd::Reached { generation: 1 });
    let run = driver.run_id();
    drop(driver);

    let edited = Fixture::new(
        &SWEEP_MANIFEST.replace("spend_usd = 40", "spend_usd = 80"),
        crate::support::PRICES,
    )?;
    let mut resumed = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        edited.inputs(),
        options(),
        run,
    )?;
    let events = ledger(&store, &run)?;
    let manifest = edited.inputs().manifest_id();
    assert!(events.iter().any(|(_, event)| matches!(
        event,
        Event::ManifestChanged { manifest: m, from_generation: 2 } if *m == manifest
    )));
    assert_eq!(resumed.run_to(2)?, RunEnd::Reached { generation: 2 });
    Ok(())
}

#[test]
fn resume_after_a_run_command_edit_is_refused_and_records_nothing() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let run = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?
    .run_id();
    let lines = ledger(&store, &run)?.len();

    let edited = Fixture::new(
        &SWEEP_MANIFEST.replace(
            r#"run = ["./target/release/bench"]"#,
            r#"run = ["./target/release/bench", "--fast"]"#,
        ),
        crate::support::PRICES,
    )?;
    let result = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        edited.inputs(),
        options(),
        run,
    );
    assert!(
        matches!(result, Err(DriverError::ManifestChanged { .. })),
        "resume did not refuse the frame edit"
    );
    assert_eq!(ledger(&store, &run)?.len(), lines);
    Ok(())
}

#[test]
fn provider_limit_records_its_spend_then_the_stop_and_ends_the_session() -> TestResult {
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![FakeReply::Error(ProviderError::Stop {
        stop: ProviderStop::Limit,
        usage: Some(token_usage()),
    })]);
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(
        driver.run_to(3)?,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::ProviderLimit
        }
    );
    assert_eq!(calls(&proposer)?, 1);

    let events = ledger(&store, &fixture.run())?;
    let stop = seq_of(&events, |event| {
        matches!(
            event,
            Event::StopRequested {
                generation: 1,
                reason: StopReason::ProviderLimit
            }
        )
    })
    .ok_or("no stop recorded")?;
    let spend = seq_of(&events, |event| {
        matches!(
            event,
            Event::Spent {
                source: SpendSource::ModelCall { call: 0, .. },
                ..
            }
        )
    })
    .ok_or("no model call spend")?;
    assert!(spend < stop);
    assert!(selected(&events, 1).is_none());
    Ok(())
}

#[test]
fn conflicting_append_stops_the_driver_and_records_nothing_more() -> TestResult {
    let fixture = Fixture::sweep()?;
    // Appends 0 to 2 are RunStarted and baseline trial 0's two spends; append 3 is its Judged.
    let store = FaultStore::new(MemoryStore::new(), 3, StoreError::Conflict { seq: 7 });
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    let run = driver.run_id();
    let result = driver.run_to(1);
    let Err(DriverError::Conflict {
        run: conflicted,
        seq,
        kind,
    }) = result
    else {
        return Err("the driver did not stop on the conflict".into());
    };
    assert_eq!((conflicted, seq, kind), (run, 7, "judged"));
    assert_eq!(
        kinds(&ledger(&store, &run)?),
        ["run_started", "spent", "spent"]
    );
    assert_eq!(jobs.requests()?.len(), 1);
    Ok(())
}

#[test]
fn observer_sees_calibration_then_proposed_judged_and_selected_in_record_order() -> TestResult {
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        Recorder::default(),
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(driver.run_to(1)?, RunEnd::Reached { generation: 1 });
    let recorder = driver.into_observer();
    let events = recorder.events();

    let (calibration, rest) = events
        .split_at_checked(3)
        .ok_or("fewer than three events")?;
    assert!(
        calibration
            .iter()
            .zip(0..)
            .all(|(event, t)| matches!(event, LoopEvent::Calibrated { trial, .. } if *trial == t))
    );
    let Some((
        LoopEvent::Proposed {
            generation: 1,
            admitted,
            ..
        },
        rest,
    )) = rest.split_first()
    else {
        return Err("no proposed event after calibration".into());
    };
    let Some((LoopEvent::Selected { generation: 1, .. }, judged)) = rest.split_last() else {
        return Err("the last event is not selected".into());
    };
    assert_eq!(usize::try_from(*admitted)?, judged.len());

    // Judged events arrive in the order their records do, each with the delta against the baseline.
    let recorded: Vec<ExperimentId> = ledger(&store, &fixture.run())?
        .into_iter()
        .filter_map(|(_, event)| match event {
            Event::Judged { exp, .. } if proposed_in_any(&exp, &store, &fixture) => Some(exp),
            _ => None,
        })
        .collect();
    let observed: Vec<ExperimentId> = judged
        .iter()
        .filter_map(|event| match event {
            LoopEvent::Judged { exp, delta, .. } => delta
                .is_some_and(|delta| (delta.value() - 1.0).abs() < 1e-9)
                .then_some(*exp),
            _ => None,
        })
        .collect();
    assert_eq!(observed, recorded);
    Ok(())
}

/// Whether `exp` is a proposed experiment of the fixture's run, not its baseline.
fn proposed_in_any(exp: &ExperimentId, store: &MemoryStore, fixture: &Fixture) -> bool {
    ledger(store, &fixture.run()).is_ok_and(|events| {
        events
            .iter()
            .any(|(_, event)| matches!(event, Event::Proposed { exp: e, .. } if e == exp))
    })
}
