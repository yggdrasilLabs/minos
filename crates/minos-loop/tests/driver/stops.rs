//! Session-relative stops (07-CONTEXT D-14, D-19): a stop from an earlier session does not block a resume, a
//! stop added while a VM runs ends the session after that trial is recorded, and a cap stop resumes once the cap
//! is raised and re-trips while it is not. One test proves the gap 07-REVIEW WR-01 named is closed: a `minos stop`
//! repeated at a generation an earlier session already stopped in carries the next free ordinal, so it is a new
//! line that the resumed session honours at its next safe point.

use minos_core::ledger::{Event, Record, SpendSource, StopReason};
use minos_loop::{Driver, JobFailure, LoopEvent, NoopObserver, RunEnd};
use minos_store::{Ledger, MemoryStore};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::support::{
    FakeJobs, Fixture, PRICES_DEAR, Recorder, STARTED_AT, SWEEP_MANIFEST, TestResult, kinds,
    ledger, options, scored, stop_identities, stops, sweep_proposer, user_stop, with_budget,
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
            ordinal: Some(0),
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
                        ordinal: Some(0),
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

/// Proves the gap 07-REVIEW WR-01 named is closed (user decision 2026-10-24): each `minos stop` carries the next
/// free ordinal from `RunState::next_user_stop_ordinal`, so a second stop at a generation an earlier session
/// already stopped in is a new ledger line, not a store duplicate. The resumed session's D-19 snapshot of stop
/// identities, taken at its start, holds only the first stop, so it honours the second at its next safe point
/// instead of running every remaining trial of that generation.
#[test]
fn a_repeated_user_stop_at_a_generation_an_earlier_session_stopped_in_ends_the_resumed_session_at_its_next_safe_point()
-> TestResult {
    // Arrange: `minos stop` records a stop at generation 1 during the first candidate trial of each session:
    // session A's first, with ordinal 0, and session B's first, with ordinal 1.
    let fixture = Fixture::sweep()?;
    let run = fixture.run();
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let candidates = AtomicUsize::new(0);
    let jobs = FakeJobs::new(|request| {
        if !request.is_baseline() && candidates.fetch_add(1, Ordering::SeqCst) < 2 {
            user_stop(&store, &run, 1)?;
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

    // Assert: each session honoured its own stop after the trial it arrived in, and launched nothing after it.
    assert_eq!(
        ended_a,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::UserStop
        }
    );
    assert_eq!(
        ended_b,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::UserStop
        }
    );
    assert_eq!(candidates.load(Ordering::SeqCst), 2);
    let events = ledger(&store, &run)?;
    assert_eq!(
        stop_identities(&events),
        [
            (1, StopReason::UserStop, Some(0)),
            (1, StopReason::UserStop, Some(1))
        ]
    );
    assert!(matches!(events.last(), Some((_, Event::Judged { .. }))));
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
    let after = ledger(&store, &run)?;
    assert_eq!(after, before);
    // The driver's own stop keeps no ordinal, so its key is `(generation, reason)` and the re-trip is a duplicate.
    assert_eq!(stop_identities(&after), [(0, StopReason::SpendCap, None)]);
    Ok(())
}
