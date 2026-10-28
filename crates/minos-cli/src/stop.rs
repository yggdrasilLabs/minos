//! `minos stop [<prefix>]`: when a driver holds the run, record a user stop at the run's working generation under
//! the next free ordinal, which a running driver honours at its next safe point (CLI-10; 08-CONTEXT D-13).
//!
//! [`stop`] takes these steps in order:
//!
//! 1. It names the run: the run whose ID starts with the prefix, matched against the names [`Ledger::runs`] lists,
//!    or with no prefix the latest run ([`resolve_run`]). It never names a run the store does not list, because the
//!    probe below creates a run's directory (08-RESEARCH Pitfall 8).
//! 2. It probes the run's `driver.lock` with [`RunLock::lock_run`]. When the lock can be taken, no driver holds
//!    the run, so the guard is dropped at once and the result is [`StopError::NoDriver`]: nothing is appended,
//!    because under 07-CONTEXT D-19 a stop recorded with no session running would never take effect. When the
//!    lock is held ([`StoreError::RunLocked`]), it goes on.
//! 3. It folds the ledger and appends `StopRequested { generation, reason: UserStop, ordinal }` at the run's
//!    working generation ([`working_generation`]) under [`RunState::next_user_stop_ordinal`], so each `stop` is
//!    its own line and a stop repeated at a generation an earlier session already stopped in is still honoured
//!    (07-REVIEW WR-01). The append takes the ledger's own append lock only, never the driver lock, so it works
//!    while a driver holds the run; the probe's guard was dropped before it.
//! 4. A [`Appended::Duplicate`] append, an equal stop recorded by a `stop` racing this one, is reported as a stop
//!    already recorded ([`StopOutcome::AlreadyRecorded`]); any store error, a `Conflict` included, is
//!    [`StopError::Store`].
//!
//! The driver never kills a VM: a stop that arrives during a trial takes effect after that trial is recorded, and
//! `stop` prints so. Two races are inherent and documented in its output (08-RESEARCH Pitfall 7): the driver can
//! end between the probe and the append, and then the stop belongs to no session and has no effect on a later
//! one; and the probe holds `driver.lock` for an instant, so a `minos run` starting at that moment is refused with
//! [`StoreError::RunLocked`], whose message says to wait.
//!
//! [`RunState::next_user_stop_ordinal`]: minos_core::state::RunState::next_user_stop_ordinal

use std::io::{self, Write};

use minos_core::id::RunId;
use minos_core::ledger::{Event, Record, StopReason};
use minos_store::{Appended, Ledger, RunLock, StoreError};

use crate::address::{AddressError, HexPrefix, fold_run, resolve_run};
use crate::render::{IdStyle, id_text, working_generation, write_line};

/// What `stop` recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// This call appended a new stop line.
    Recorded {
        /// The run.
        run: RunId,
        /// The working generation the stop is recorded under.
        generation: u32,
        /// The stop's ordinal at that generation.
        ordinal: u32,
    },
    /// An equal stop was already recorded, by a `stop` racing this one, so nothing was written.
    AlreadyRecorded {
        /// The run.
        run: RunId,
        /// The working generation the stop is recorded under.
        generation: u32,
        /// The stop's ordinal at that generation.
        ordinal: u32,
    },
}

/// Why `stop` refused or failed. Messages carry IDs and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum StopError {
    /// No driver holds the run, so a stop could never take effect (08-CONTEXT D-13).
    #[error("no driver holds run {run}; nothing to stop")]
    NoDriver {
        /// The run.
        run: RunId,
    },
    /// Every ordinal at the generation is taken, so no further stop can be keyed apart from the last.
    #[error("run {run} has no free stop ordinal at generation {generation}")]
    OrdinalsExhausted {
        /// The run.
        run: RunId,
        /// The working generation.
        generation: u32,
    },
    /// The run could not be named, or its ledger could not be read or folded.
    #[error("the run to stop could not be named")]
    Address(#[source] AddressError),
    /// The store refused the probe or the append.
    #[error("the store in .minos refused the stop")]
    Store(#[source] StoreError),
    /// The output could not be written.
    #[error("writing the output failed")]
    Output(#[source] io::Error),
}

/// Asks the driver holding the run that `run` names (or the latest run) to stop at its next safe point, by
/// appending a `UserStop` at the run's working generation under the next free ordinal, and prints what it did.
///
/// # Errors
///
/// [`StopError::Address`] when the run cannot be named or its ledger cannot be folded; [`StopError::NoDriver`]
/// when no driver holds the run, with nothing appended; [`StopError::OrdinalsExhausted`] when no ordinal is free;
/// [`StopError::Store`] when the probe or the append fails, a `Conflict` included; and [`StopError::Output`]
/// when `out` cannot be written.
pub fn stop<S: Ledger + RunLock, W: Write>(
    store: &S,
    run: Option<&HexPrefix>,
    out: &mut W,
) -> Result<StopOutcome, StopError> {
    let run = *resolve_run(store, run).map_err(StopError::Address)?.run();

    // The probe: the guard is dropped at once and never held across the append.
    match store.lock_run(&run) {
        Ok(guard) => {
            drop(guard);
            return Err(StopError::NoDriver { run });
        }
        Err(StoreError::RunLocked { .. }) => {}
        Err(error) => return Err(StopError::Store(error)),
    }

    let state = fold_run(store, &run).map_err(StopError::Address)?;
    let generation = working_generation(&state);
    let ordinal = state
        .next_user_stop_ordinal(generation)
        .ok_or(StopError::OrdinalsExhausted { run, generation })?;
    let record = Record::v1(Event::StopRequested {
        generation,
        reason: StopReason::UserStop,
        ordinal: Some(ordinal),
    });
    let outcome = match store.append(&run, record).map_err(StopError::Store)? {
        Appended::New { .. } => StopOutcome::Recorded {
            run,
            generation,
            ordinal,
        },
        Appended::Duplicate { .. } => StopOutcome::AlreadyRecorded {
            run,
            generation,
            ordinal,
        },
    };

    let short = id_text(&run.to_hex(), IdStyle::Short);
    let head = match outcome {
        StopOutcome::Recorded { .. } => {
            format!("stop recorded for run {short} at generation {generation} (ordinal {ordinal})")
        }
        StopOutcome::AlreadyRecorded { .. } => format!(
            "a stop is already recorded for run {short} at generation {generation} (ordinal {ordinal})"
        ),
    };
    for line in [
        head.as_str(),
        "the driver stops at its next safe point; it never kills a VM, so a running trial finishes and is recorded first",
        "if the run ends before that, this stop has no effect on a later session",
    ] {
        write_line(out, line).map_err(StopError::Output)?;
    }
    Ok(outcome)
}
