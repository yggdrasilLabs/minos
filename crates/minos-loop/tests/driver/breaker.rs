//! The circuit breaker and the infrastructure retry streak (07-CONTEXT D-14, D-16; LOOP-08): K empty
//! generations, K consecutive job or provider infrastructure failures, the streak's reset on success, and a
//! store hiccup that retries only the append. Every test runs with no retry pause.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use minos_core::job::JobRequest;
use minos_core::ledger::{DecisionKind, Event, SpendSource, StopReason};
use minos_loop::{Calibrated, Driver, JobFailure, LoopEvent, NoopObserver, RunEnd};
use minos_propose::FakeReply;
use minos_propose::provider::{InfraCause, ProviderError};
use minos_store::{MemoryStore, StoreError};

use crate::support::{
    FakeJobs, FaultStore, Fixture, PRICES, Recorder, STARTED_AT, SWEEP_MANIFEST, TestResult, calls,
    kinds, ledger, options, run_failed, scored, single_shot_proposer, stops, sweep_proposer,
    token_usage, with_budget,
};

/// The infrastructure failure every failing fake job reports.
const INFRA: JobFailure = JobFailure::Infra {
    what: "the fake launch failed",
};

/// A provider infrastructure error with `usage`.
fn provider_infra(with_usage: bool) -> FakeReply {
    FakeReply::Error(ProviderError::Infra {
        cause: InfraCause::Fake,
        usage: with_usage.then(token_usage),
    })
}

/// Every `Retrying` streak and every `Stopped` reason the observer saw, in order.
fn retries_and_stops(events: &[LoopEvent]) -> (Vec<(u32, u32)>, Vec<StopReason>) {
    let retries = events
        .iter()
        .filter_map(|event| match event {
            LoopEvent::Retrying { streak, max } => Some((*streak, *max)),
            _ => None,
        })
        .collect();
    let stopped = events
        .iter()
        .filter_map(|event| match event {
            LoopEvent::Stopped { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect();
    (retries, stopped)
}

#[test]
fn two_empty_generations_trip_the_breaker_with_no_valid_candidates() -> TestResult {
    // Arrange: the breaker at two empty generations, and every candidate's run stage failing.
    let fixture = Fixture::new(
        &with_budget(SWEEP_MANIFEST, 40, "max_empty_generations = 2\n"),
        PRICES,
    )?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| {
        if request.is_baseline() {
            scored(request, 1.0)
        } else {
            run_failed(request)
        }
    });
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let end = driver.run_to(5)?;

    // Assert: generations 1 and 2 concluded empty, and the stop lands before generation 3 proposes anything.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 3,
            reason: StopReason::NoValidCandidates
        }
    );
    let events = ledger(&store, &fixture.run())?;
    let decisions: Vec<(u32, DecisionKind)> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Selected {
                generation,
                decision,
            } => Some((*generation, *decision.kind())),
            _ => None,
        })
        .collect();
    assert_eq!(
        decisions,
        [
            (1, DecisionKind::NoValidCandidates),
            (2, DecisionKind::NoValidCandidates)
        ]
    );
    assert!(
        !events
            .iter()
            .any(|(_, event)| matches!(event, Event::Proposed { generation: 3, .. }))
    );
    assert_eq!(stops(&events), [(3, StopReason::NoValidCandidates)]);
    assert_eq!(kinds(&events).last(), Some(&"stop_requested"));
    Ok(())
}

#[test]
fn three_job_infra_failures_record_nothing_and_stop_with_infra_failures() -> TestResult {
    // Arrange: the default breaker of three, and a job runner that always fails for infrastructure reasons.
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|_request| Err(INFRA));
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
    let end = driver.run_to(1)?;

    // Assert: three attempts at the same trial, nothing recorded for any, then the stop.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 0,
            reason: StopReason::InfraFailures
        }
    );
    let trials: Vec<u32> = jobs.requests()?.iter().map(JobRequest::trial).collect();
    assert_eq!(trials, [0, 0, 0]);
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(kinds(&events), ["run_started", "stop_requested"]);
    assert_eq!(stops(&events), [(0, StopReason::InfraFailures)]);
    let observed = driver.into_observer();
    assert_eq!(
        retries_and_stops(observed.events()),
        (vec![(1, 3), (2, 3)], vec![StopReason::InfraFailures])
    );
    Ok(())
}

#[test]
fn a_success_resets_the_infra_streak_so_a_later_single_failure_does_not_stop() -> TestResult {
    // Arrange: calls 0 and 1 fail, call 2 succeeds, call 3 fails, and every later call succeeds.
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let count = AtomicUsize::new(0);
    let jobs = FakeJobs::new(|request| match count.fetch_add(1, Ordering::SeqCst) {
        0 | 1 | 3 => Err(INFRA),
        _ => scored(request, 1.0),
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
    let calibrated = driver.calibrate()?;

    // Assert: calibration completes; the streak went 1, 2, back to 0, then 1.
    assert!(matches!(calibrated, Calibrated::Ready(_)));
    let trials: Vec<u32> = jobs.requests()?.iter().map(JobRequest::trial).collect();
    assert_eq!(trials, [0, 0, 0, 1, 1, 2]);
    let events = ledger(&store, &fixture.run())?;
    assert!(stops(&events).is_empty());
    assert_eq!(
        kinds(&events)
            .iter()
            .filter(|kind| **kind == "judged")
            .count(),
        3
    );
    let observed = driver.into_observer();
    assert_eq!(
        retries_and_stops(observed.events()),
        (vec![(1, 3), (2, 3), (1, 3)], vec![])
    );
    Ok(())
}

#[test]
fn provider_infra_errors_count_toward_the_streak_and_stop_with_infra_failures() -> TestResult {
    // Arrange: three provider infrastructure failures with no usage.
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![
        provider_infra(false),
        provider_infra(false),
        provider_infra(false),
    ]);
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
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
    let end = driver.run_to(1)?;

    // Assert: three calls, none recorded, then the stop.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::InfraFailures
        }
    );
    assert_eq!(calls(&proposer)?, 3);
    let events = ledger(&store, &fixture.run())?;
    assert!(!events.iter().any(|(_, event)| matches!(
        event,
        Event::Spent {
            source: SpendSource::ModelCall { .. },
            ..
        }
    )));
    assert_eq!(stops(&events), [(1, StopReason::InfraFailures)]);
    let observed = driver.into_observer();
    assert_eq!(
        retries_and_stops(observed.events()),
        (vec![(1, 3), (2, 3)], vec![StopReason::InfraFailures])
    );
    Ok(())
}

#[test]
fn provider_infra_errors_with_usage_record_each_spend_and_still_trip_the_breaker() -> TestResult {
    // Arrange: three provider infrastructure failures that each report usage.
    let fixture = Fixture::single_shot()?;
    let store = MemoryStore::new();
    let proposer = single_shot_proposer(vec![
        provider_infra(true),
        provider_infra(true),
        provider_infra(true),
    ]);
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

    // Act
    let end = driver.run_to(1)?;

    // Assert: each failed call's spend lands under the next call index, and recording it does not reset the
    // streak, so the third failure still stops the run.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::InfraFailures
        }
    );
    let events = ledger(&store, &fixture.run())?;
    let indices: Vec<u32> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Spent {
                source: SpendSource::ModelCall { call, .. },
                ..
            } => Some(*call),
            _ => None,
        })
        .collect();
    assert_eq!(indices, [0, 1, 2]);
    assert_eq!(calls(&proposer)?, 3);
    assert_eq!(stops(&events), [(1, StopReason::InfraFailures)]);
    Ok(())
}

#[test]
fn a_retryable_append_failure_retries_the_append_without_rerunning_the_job() -> TestResult {
    // Arrange: append 3 (baseline trial 0's `Judged`, after RunStarted and its two spends) fails with `Io`.
    let fixture = Fixture::sweep()?;
    let store = FaultStore::new(
        MemoryStore::new(),
        3,
        StoreError::Io {
            op: "the fake append",
            source: io::Error::other("the disk hiccupped"),
        },
    );
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
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
    let calibrated = driver.calibrate()?;

    // Assert: one job per trial, every trial recorded once, and one retry of the append alone.
    assert!(matches!(calibrated, Calibrated::Ready(_)));
    let trials: Vec<u32> = jobs.requests()?.iter().map(JobRequest::trial).collect();
    assert_eq!(trials, [0, 1, 2]);
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(
        kinds(&events).get(1..),
        Some(["spent", "spent", "judged"].repeat(3).as_slice())
    );
    let observed = driver.into_observer();
    assert_eq!(retries_and_stops(observed.events()), (vec![(1, 3)], vec![]));
    Ok(())
}
