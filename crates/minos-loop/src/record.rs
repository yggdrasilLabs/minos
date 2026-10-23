//! The one way the driver appends to a run's ledger (07-CONTEXT D-08; 07-RESEARCH Pitfall 1).
//!
//! Every append goes through [`append`], which wraps the event in a version-1 record and classifies the store's
//! answer: a new line or an equal duplicate is success, and a different record under an existing key is
//! [`DriverError::Conflict`] naming the run, the seq, and the event kind. The first write wins on disk and the
//! driver stops, because a conflicting append means a driver bug or a tampered ledger, and the ledger needs a
//! human.
//!
//! # Spends
//!
//! A `Spent` is keyed by its [`SpendId`] (the run, the generation, and the source), not by its usage. A trial
//! re-run after a crash between its `Spent` and its `Judged` measures a different usage, so appending it again
//! would conflict and stop the run. [`spend_once`] therefore appends a `Spent` only when its `SpendId` is absent
//! from the fold: the first measurement stands, and the re-run's usage goes unrecorded. That undercounts the
//! run's spend by at most one job per crash, the counterpart of the one-job overshoot a spend cap allows
//! (07-CONTEXT D-15).

use minos_core::id::RunId;
use minos_core::ledger::{Event, Record, SpendId, SpendSource};
use minos_core::state::RunState;
use minos_core::usage::Usage;
use minos_store::{Ledger, StoreError};

use crate::error::DriverError;

/// The snake-case name of `event`'s kind, as fixed text, for a [`DriverError::Conflict`].
#[must_use]
pub const fn event_kind(event: &Event) -> &'static str {
    match event {
        Event::RunStarted { .. } => "run_started",
        Event::ManifestChanged { .. } => "manifest_changed",
        Event::Proposed { .. } => "proposed",
        Event::Judged { .. } => "judged",
        Event::Selected { .. } => "selected",
        Event::Spent { .. } => "spent",
        Event::Steered { .. } => "steered",
        Event::StopRequested { .. } => "stop_requested",
        Event::ReviewRequested { .. } => "review_requested",
        Event::Reviewed { .. } => "reviewed",
        Event::ReportAcknowledged { .. } => "report_acknowledged",
        // `Event` is non-exhaustive; a variant added later is named generically until it gets its own arm.
        _ => "event",
    }
}

/// Appends `event` to `run`'s ledger as a version-1 record.
///
/// A new line and an equal record already under the event's key are both success: replaying a step changes
/// nothing.
///
/// # Errors
///
/// [`DriverError::Conflict`] naming the run, the existing line's seq, and the event's kind when a different
/// record holds the key (nothing is written; never retried), and [`DriverError::Store`] for every other store
/// error; the caller retries only when [`StoreError::is_retryable`] says so.
pub fn append<L: Ledger>(store: &L, run: &RunId, event: Event) -> Result<(), DriverError> {
    let kind = event_kind(&event);
    match store.append(run, Record::v1(event)) {
        Ok(_) => Ok(()),
        Err(StoreError::Conflict { seq }) => Err(DriverError::Conflict {
            run: *run,
            seq,
            kind,
        }),
        Err(source) => Err(DriverError::Store { run: *run, source }),
    }
}

/// Appends `Spent { generation, source, usage }` unless the fold already records that spend, in which case it
/// attempts nothing, whatever `usage` is (see the [module docs](self)).
///
/// # Errors
///
/// As [`append`].
pub fn spend_once<L: Ledger>(
    store: &L,
    state: &RunState,
    run: &RunId,
    generation: u32,
    source: SpendSource,
    usage: Usage,
) -> Result<(), DriverError> {
    if state.has_spend(&SpendId::of(run, generation, &source)) {
        return Ok(());
    }
    append(
        store,
        run,
        Event::Spent {
            generation,
            source,
            usage,
        },
    )
}
