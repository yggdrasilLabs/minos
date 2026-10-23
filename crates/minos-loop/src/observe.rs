//! The observer seam: one synchronous event per recorded effect, so a caller can show progress without the loop
//! knowing about terminals.
//!
//! No event carries free text. Every field is an ID, a number, a closed enum, or a validated value
//! ([`Outcome`], [`Decision`], [`Metric`]), so nothing a sandbox or a model wrote can reach a terminal through
//! an event. Elapsed time is not an event field: the loop reads no clock, and a caller that shows durations uses
//! its own.

use minos_core::id::ExperimentId;
use minos_core::ledger::{Decision, StopReason};
use minos_core::manifest::Tier;
use minos_core::metric::Metric;
use minos_core::outcome::Outcome;

/// Receives the loop's events, in the order the driver records the effects they describe.
///
/// The driver calls [`Observer::observe`] after the record lands, so an observer never sees an effect the
/// ledger does not hold. The driver is generic over its observer and never holds one as a trait object.
pub trait Observer {
    /// Receives one event.
    fn observe(&mut self, event: &LoopEvent);
}

impl<O: Observer + ?Sized> Observer for &mut O {
    fn observe(&mut self, event: &LoopEvent) {
        (**self).observe(event);
    }
}

/// An observer that ignores every event, for callers that show no progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoopObserver;

impl Observer for NoopObserver {
    fn observe(&mut self, _event: &LoopEvent) {}
}

/// One recorded effect of the loop.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopEvent {
    /// A baseline trial of generation 0 was judged.
    Calibrated {
        /// The baseline trial's index.
        trial: u32,
        /// How it ended.
        outcome: Outcome,
    },
    /// A propose step recorded its proposals.
    Proposed {
        /// The generation.
        generation: u32,
        /// The tier that proposed.
        tier: Tier,
        /// How many proposals were admitted and recorded.
        admitted: u32,
        /// How many were dropped: invalid, a duplicate, or already proposed in an earlier generation.
        dropped: u32,
    },
    /// A candidate's trial was judged.
    Judged {
        /// The generation.
        generation: u32,
        /// The experiment.
        exp: ExperimentId,
        /// The trial's index.
        trial: u32,
        /// The tier that proposed the experiment.
        tier: Tier,
        /// How the trial ended.
        outcome: Outcome,
        /// The experiment's reduced objective minus the incumbent's, or `None` when either is unscored or the
        /// difference is not finite.
        delta: Option<Metric>,
    },
    /// A generation concluded.
    Selected {
        /// The generation.
        generation: u32,
        /// What it decided.
        decision: Decision,
    },
    /// An infrastructure failure was not recorded and the step will be retried after the pause (07-CONTEXT D-16).
    Retrying {
        /// Consecutive infrastructure failures so far, this one included.
        streak: u32,
        /// The streak at which the run stops (`max_infra_failures`).
        max: u32,
    },
    /// The driver recorded or honored a stop and exits.
    Stopped {
        /// The generation at which it stopped.
        generation: u32,
        /// Why.
        reason: StopReason,
    },
}
