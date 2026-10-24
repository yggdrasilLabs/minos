//! Session-relative stops (07-CONTEXT D-14, D-19): a stop from an earlier session does not block a resume, a
//! stop added while a VM runs ends the session after that trial is recorded, and a cap stop resumes once the cap
//! is raised and re-trips while it is not. One test pins the gap 07-REVIEW WR-01 names: a `minos stop` repeated
//! at a generation an earlier session already stopped in is a store duplicate the resumed session never sees.

use minos_core::ledger::{Event, Record, SpendSource, StopReason};
use minos_loop::{Driver, JobFailure, LoopEvent, NoopObserver, RunEnd};
use minos_store::{Ledger, MemoryStore};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::support::{
    FakeJobs, Fixture, PRICES_DEAR, Recorder, STARTED_AT, SWEEP_MANIFEST, TestResult, kinds,
    ledger, options, scored, stops, sweep_proposer, with_budget,
};

/// The failure a fake job reports when it cannot append its stop.
const STOP_FAILED: JobFailure = JobFailure::Config {
    what: "the fake could not append its stop",
};

#[test]
fn an_old_user_stop_does_not_block_a_resume_from_reaching_its_target() -> TestResult {
    // Arrange: a run that reached generation 1, then a `minos stop` recorded after that session ended.
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let run = {
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
        driver.run_id()
    };
    let _ = store.append(
        &run,
        Record::v1(Event::StopRequested {
            generation: 1,
            reason: StopReason::UserStop,
        }),
    )?;

    // Act
    let mut driver = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    )?;
    let end = driver.run_to(2)?;

    // Assert: the resume reached its target, and the old stop is the only one recorded.
    assert_eq!(end, RunEnd::Reached { generation: 2 });
    assert_eq!(stops(&ledger(&store, &run)?), [(1, StopReason::UserStop)]);
    Ok(())
}

#[test]
fn a_user_stop_during_a_launch_ends_the_session_after_that_trial_is_recorded() -> TestResult {
    // Arrange: while baseline trial 1 runs, `minos stop` appends `StopRequested { 0, UserStop }`.
    let fixture = Fixture::sweep()?;
    let run = fixture.run();
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let count = AtomicUsize::new(0);
    let jobs = FakeJobs::new(|request| {
        if count.fetch_add(1, Ordering::SeqCst) == 1 {
            let _ = store
                .append(
                    &run,
                    Record::v1(Event::StopRequested {
                        generation: 0,
                        reason: StopReason::UserStop,
                    }),
                )
                .map_err(|_error| STOP_FAILED)?;
        }
        scored(request, 1.0)
    });
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        Recorder::default(),
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let end = driver.run_to(2)?;

    // Assert: trial 1 was awaited and recorded after the stop, and nothing else was launched.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 0,
            reason: StopReason::UserStop
        }
    );
    assert_eq!(jobs.requests()?.len(), 2);
    let events = ledger(&store, &run)?;
    assert_eq!(
        kinds(&events),
        [
            "run_started",
            "spent",
            "spent",
            "judged",
            "stop_requested",
            "spent",
            "spent",
            "judged"
        ]
    );
    assert!(matches!(
        events.last(),
        Some((_, Event::Judged { trial: 1, .. }))
    ));
    assert!(events.iter().any(|(_, event)| matches!(
        event,
        Event::Spent {
            source: SpendSource::Candidate { trial: 1, .. },
            ..
        }
    )));
    let observed = driver.into_observer();
    let stopped: Vec<&LoopEvent> = observed
        .events()
        .iter()
        .filter(|event| matches!(event, LoopEvent::Stopped { .. }))
        .collect();
    assert_eq!(
        stopped,
        [&LoopEvent::Stopped {
            generation: 0,
            reason: StopReason::UserStop
        }]
    );
    assert!(matches!(
        observed.events().last(),
        Some(LoopEvent::Stopped { .. })
    ));
    Ok(())
}

/// Pins the gap 07-REVIEW WR-01 names, open in CLAUDE.md ("Open questions", owner Phase 8): `StopRequested` is
/// keyed by `(generation, reason)`, so a second `minos stop` at a generation an earlier session already stopped in
/// is a store duplicate that writes nothing, and the resumed session, which snapshotted the first stop at its
/// start (D-19), runs every remaining trial of that generation and sees a new stop only once the generation
/// advances. Closing it needs a per-request key (a `"v":"2"` wire decision) or a `minos stop` that refuses when
/// the pair already exists. When that lands, this test must flip to a `Stopped` end.
#[test]
fn a_repeated_user_stop_at_a_generation_an_earlier_session_stopped_in_is_a_duplicate_the_resume_never_sees()
-> TestResult {
    // Arrange: `minos stop` appends `StopRequested { 1, UserStop }` during the first candidate trial of each
    // session: session A's first, which A honours, and session B's first, which is a duplicate of it.
    let fixture = Fixture::sweep()?;
    let run = fixture.run();
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let candidates = AtomicUsize::new(0);
    let jobs = FakeJobs::new(|request| {
        if !request.is_baseline() && candidates.fetch_add(1, Ordering::SeqCst) < 2 {
            let _ = store
                .append(
                    &run,
                    Record::v1(Event::StopRequested {
                        generation: 1,
                        reason: StopReason::UserStop,
                    }),
                )
                .map_err(|_error| STOP_FAILED)?;
        }
        scored(request, if request.is_baseline() { 1.0 } else { 2.0 })
    });

    // Act: session A stops after the trial the stop arrived in; session B resumes at generation 1.
    let ended_a = {
        let mut driver = Driver::start(
            &store,
            &proposer,
            &jobs,
            NoopObserver,
            fixture.inputs(),
            options(),
            STARTED_AT,
        )?;
        driver.run_to(2)?
    };
    let mut resumed = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    )?;
    let ended_b = resumed.run_to(2)?;

    // Assert: A honoured the first stop; B never saw the second, which left no line, and reached its target.
    assert_eq!(
        ended_a,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::UserStop
        }
    );
    // Both stops were appended (the second during B's first candidate trial) and B launched more after it.
    assert!(candidates.load(Ordering::SeqCst) > 2);
    assert_eq!(ended_b, RunEnd::Reached { generation: 2 });
    assert_eq!(stops(&ledger(&store, &run)?), [(1, StopReason::UserStop)]);
    Ok(())
}

#[test]
fn a_spend_cap_stop_resumes_and_proceeds_once_the_cap_is_raised() -> TestResult {
    // Arrange: a run stopped at its $1 cap after one $1.50 trial.
    let capped = Fixture::new(&with_budget(SWEEP_MANIFEST, 1, ""), PRICES_DEAR)?;
    let raised = Fixture::new(&with_budget(SWEEP_MANIFEST, 100, ""), PRICES_DEAR)?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let run = {
        let mut driver = Driver::start(
            &store,
            &proposer,
            &jobs,
            NoopObserver,
            capped.inputs(),
            options(),
            STARTED_AT,
        )?;
        assert!(matches!(
            driver.run_to(1)?,
            RunEnd::Stopped {
                reason: StopReason::SpendCap,
                ..
            }
        ));
        driver.run_id()
    };

    // Act: resume under a manifest whose only change is a $100 cap.
    let mut driver = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        raised.inputs(),
        options(),
        run,
    )?;
    let end = driver.run_to(1)?;

    // Assert: the edit was recorded and the run reached its target with no further stop.
    assert_eq!(end, RunEnd::Reached { generation: 1 });
    let events = ledger(&store, &run)?;
    assert!(events.iter().any(|(_, event)| matches!(
        event,
        Event::ManifestChanged {
            from_generation: 1,
            ..
        }
    )));
    assert_eq!(stops(&events), [(0, StopReason::SpendCap)]);
    Ok(())
}

#[test]
fn a_spend_cap_stop_re_trips_on_resume_when_the_cap_is_unchanged() -> TestResult {
    // Arrange: a run stopped at its $1 cap after one $1.50 trial.
    let fixture = Fixture::new(&with_budget(SWEEP_MANIFEST, 1, ""), PRICES_DEAR)?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let run = {
        let mut driver = Driver::start(
            &store,
            &proposer,
            &jobs,
            NoopObserver,
            fixture.inputs(),
            options(),
            STARTED_AT,
        )?;
        let _ = driver.run_to(1)?;
        driver.run_id()
    };
    let before = ledger(&store, &run)?;

    // Act: resume under the same manifest.
    let mut driver = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    )?;
    let end = driver.run_to(1)?;

    // Assert: the cap re-trips before any launch, and its stop is a duplicate, so the ledger is unchanged.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 0,
            reason: StopReason::SpendCap
        }
    );
    assert_eq!(jobs.requests()?.len(), 1);
    assert_eq!(ledger(&store, &run)?, before);
    Ok(())
}
