//! The backend-class pin (07-CONTEXT D-11, JUDGE-04): a trial on another class is refused with nothing
//! recorded for it, in calibration and in a candidate generation, and a run on one class records every trial.

use std::sync::atomic::{AtomicUsize, Ordering};

use minos_core::ledger::{BackendKind, Event};
use minos_core::manifest::Arch;
use minos_core::state::fold;
use minos_loop::{Driver, DriverError, NoopObserver, RunEnd};
use minos_store::{Ledger, MemoryStore};

use crate::support::{
    FakeJobs, Fixture, STARTED_AT, TestResult, backend, backend_on, kinds, ledger, options, scored,
    scored_on, sweep_proposer,
};

/// Asserts `result` is `BackendChanged` from krun on arm64 to krun on `x86_64`.
fn assert_changed_to_x86<T: std::fmt::Debug>(result: &Result<T, DriverError>) -> TestResult {
    let Err(DriverError::BackendChanged { recorded, found }) = result else {
        return Err(format!("expected BackendChanged, got {result:?}").into());
    };
    assert_eq!(*recorded, (BackendKind::Krun, Arch::Arm64));
    assert_eq!(*found, (BackendKind::Krun, Arch::X86_64));
    Ok(())
}

#[test]
fn a_baseline_trial_on_another_backend_class_is_refused_and_records_nothing() -> TestResult {
    // Arrange: baseline trial 1 reports krun on x86_64.
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let count = AtomicUsize::new(0);
    let jobs = FakeJobs::new(|request| {
        if count.fetch_add(1, Ordering::SeqCst) == 1 {
            scored_on(request, 1.0, backend_on(Arch::X86_64))
        } else {
            scored(request, 1.0)
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
    let result = driver.run_to(1);

    // Assert: refused with both classes, and only trial 0 is in the ledger.
    assert_changed_to_x86(&result)?;
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(kinds(&events), ["run_started", "spent", "spent", "judged"]);
    assert!(matches!(
        events.last(),
        Some((_, Event::Judged { trial: 0, .. }))
    ));
    Ok(())
}

#[test]
fn a_candidate_trial_on_another_backend_class_is_refused_and_records_nothing() -> TestResult {
    // Arrange: every baseline trial is krun on arm64; every candidate reports krun on x86_64.
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| {
        if request.is_baseline() {
            scored(request, 1.0)
        } else {
            scored_on(request, 2.0, backend_on(Arch::X86_64))
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
    let result = driver.run_to(1);

    // Assert: the generation's proposals are recorded, but no candidate spend or judgment.
    assert_changed_to_x86(&result)?;
    let events = ledger(&store, &fixture.run())?;
    assert!(kinds(&events).contains(&"proposed"));
    assert_eq!(kinds(&events).last(), Some(&"proposed"));
    assert_eq!(
        kinds(&events)
            .iter()
            .filter(|kind| **kind == "judged")
            .count(),
        3
    );
    Ok(())
}

#[test]
fn a_run_whose_every_trial_is_krun_on_arm64_records_them_all() -> TestResult {
    // Arrange
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

    // Act
    let end = driver.run_to(2)?;

    // Assert: every trial that ran is judged, all under the one class.
    assert_eq!(end, RunEnd::Reached { generation: 2 });
    let events = ledger(&store, &fixture.run())?;
    let judged: Vec<&Event> = events
        .iter()
        .map(|(_, event)| event)
        .filter(|event| matches!(event, Event::Judged { .. }))
        .collect();
    assert_eq!(judged.len(), jobs.requests()?.len());
    assert!(
        judged
            .iter()
            .all(|event| matches!(event, Event::Judged { backend: b, .. } if *b == backend()))
    );
    let state = fold(store.read(&fixture.run())?.records())?;
    assert_eq!(
        state.backend_classes().into_iter().collect::<Vec<_>>(),
        [(BackendKind::Krun, Arch::Arm64)]
    );
    Ok(())
}
