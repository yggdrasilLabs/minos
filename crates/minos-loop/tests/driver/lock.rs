//! One driver per run (LOOP-06): while a session holds a run's lock, a second `Driver::resume` or `Driver::start`
//! of that run is refused with `DriverError::RunLocked` naming the run, before it reads or records anything; once
//! the first session is dropped, a resume takes the lock and drives the run.
//!
//! On `MemoryStore` the lock is an in-process set of held runs. On `FileStore` it is `runs/<run-hex>/driver.lock`
//! under `File::try_lock`, so the file cases open two `FileStore` handles over one directory, as two `minos`
//! processes would.

use std::path::Path;

use minos_core::id::RunId;
use minos_loop::{Driver, DriverError, NoopObserver, RunEnd};
use minos_store::{Blobs, FileStore, Ledger, MemoryStore, RunLock};

use crate::support::{
    FakeJobs, Fixture, STARTED_AT, TestResult, deterministic, ledger, options, sweep_proposer,
};

/// Fails unless `result` is a refusal naming `run` as held by another driver.
fn assert_run_locked<T>(result: Result<T, DriverError>, run: RunId) -> TestResult {
    match result {
        Err(DriverError::RunLocked { run: locked }) if locked == run => Ok(()),
        Err(error) => Err(format!("expected the run lock refusal, got: {error}").into()),
        Ok(_) => Err("a second driver was allowed on a held run".into()),
    }
}

/// Drives `first` through a refused resume on `second`, then a resume on `second` after `first`'s session ends.
/// `first` and `second` may be one store or two handles over one directory.
fn refused_while_held_then_resumed<A, B>(first: &A, second: &B) -> TestResult
where
    A: Ledger + Blobs + RunLock,
    B: Ledger + Blobs + RunLock,
{
    // Arrange: a session that holds the run after reaching generation 1.
    let fixture = Fixture::sweep()?;
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(deterministic);
    let mut holder = Driver::start(
        first,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(holder.run_to(1)?, RunEnd::Reached { generation: 1 });
    let run = holder.run_id();
    let recorded = ledger(second, &run)?;

    // Act: a second driver on the held run.
    let refused = Driver::resume(
        second,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    );

    // Assert: refused, and nothing recorded.
    assert_run_locked(refused, run)?;
    assert_eq!(ledger(second, &run)?, recorded);

    // Act: the holder's session ends; a resume takes the run and drives it on.
    drop(holder);
    let mut resumed = Driver::resume(
        second,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    )?;

    // Assert
    assert_eq!(resumed.run_to(2)?, RunEnd::Reached { generation: 2 });
    Ok(())
}

/// The store root inside a temporary project directory.
fn store_root(project: &Path) -> std::path::PathBuf {
    project.join(".minos")
}

#[test]
fn memory_store_refuses_a_second_driver_until_the_first_is_dropped() -> TestResult {
    let store = MemoryStore::new();
    refused_while_held_then_resumed(&store, &store)
}

#[test]
fn two_file_store_handles_refuse_a_second_driver_until_the_first_is_dropped() -> TestResult {
    let project = tempfile::tempdir()?;
    let first = FileStore::open(&store_root(project.path()))?;
    let second = FileStore::open(&store_root(project.path()))?;
    refused_while_held_then_resumed(&first, &second)
}

#[test]
fn start_with_the_same_inputs_while_a_driver_holds_the_run_is_refused_and_appends_nothing()
-> TestResult {
    for store in [
        Store::Memory(MemoryStore::new()),
        Store::File(tempfile::tempdir()?),
    ] {
        match &store {
            Store::Memory(memory) => start_refused(memory, memory)?,
            Store::File(project) => {
                let first = FileStore::open(&store_root(project.path()))?;
                let second = FileStore::open(&store_root(project.path()))?;
                start_refused(&first, &second)?;
            }
        }
    }
    Ok(())
}

/// The two store kinds the start refusal is checked on.
enum Store {
    /// One in-memory store.
    Memory(MemoryStore),
    /// A temporary project directory holding a file store.
    File(tempfile::TempDir),
}

/// A `Driver::start` on `second` of the run a live session on `first` holds is refused and appends nothing.
fn start_refused<A, B>(first: &A, second: &B) -> TestResult
where
    A: Ledger + Blobs + RunLock,
    B: Ledger + Blobs + RunLock,
{
    // Arrange: a live session that has calibrated.
    let fixture = Fixture::sweep()?;
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(deterministic);
    let mut holder = Driver::start(
        first,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(holder.run_to(0)?, RunEnd::Reached { generation: 0 });
    let run = holder.run_id();
    let recorded = ledger(second, &run)?;

    // Act: the same inputs and the same start time name the same run.
    let refused = Driver::start(
        second,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    );

    // Assert
    assert_run_locked(refused, run)?;
    assert_eq!(ledger(second, &run)?, recorded);
    assert_eq!(jobs.requests()?.len(), 3);
    drop(holder);
    Ok(())
}

#[test]
fn the_refusal_names_the_run_and_says_to_wait_or_run_minos_stop() -> TestResult {
    // Arrange
    let fixture = Fixture::sweep()?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(deterministic);
    let holder = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    let run = holder.run_id();

    // Act
    let Err(error) = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    ) else {
        return Err("a second driver was allowed on a held run".into());
    };

    // Assert
    let message = error.to_string();
    assert!(message.contains(&run.to_hex()), "{message}");
    assert!(message.contains("wait"), "{message}");
    assert!(message.contains("minos stop"), "{message}");
    drop(holder);
    Ok(())
}
