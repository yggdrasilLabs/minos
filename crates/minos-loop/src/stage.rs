//! The loop's stage function: the next step of a run, as a pure function of its ledger (07-CONTEXT D-06, D-07).
//!
//! Resume is a fold, not a journal. The driver never remembers where it was: before every step it folds the
//! ledger and asks [`next_step`], then performs that one step and records the event that completes it. Because
//! [`RunState`] does not depend on the order records were appended in, and [`next_step`] reads nothing else, a
//! resumed session chooses exactly the step an uninterrupted one would have (LOOP-05), and a step whose record
//! exists is never performed again.

use minos_core::id::ExperimentId;
use minos_core::manifest::JudgeVersion;
use minos_core::state::RunState;

/// Trials per candidate in v1: one, because champion re-measurement is NEXT-05 and `min_improvement` with the
/// baseline noise check guards selection (07-RESEARCH A4).
pub const CANDIDATE_TRIALS: u32 = 1;

/// The driver's next action, and the record that completes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Run baseline trial `trial` of generation 0. Completed by `Judged { baseline, judge, trial }`.
    Calibrate {
        /// The baseline trial's index.
        trial: u32,
    },
    /// Propose generation `generation`. Completed by its `Proposed` records, or, when the propose step admits
    /// nothing, by the generation's `Selected` with no valid candidates, appended in the same step.
    Propose {
        /// The generation, from 1.
        generation: u32,
    },
    /// Run trial `trial` of `exp`, proposed in `generation`. Completed by `Judged { exp, judge, trial }`.
    RunTrial {
        /// The generation that proposed `exp`.
        generation: u32,
        /// The experiment.
        exp: ExperimentId,
        /// The trial's index.
        trial: u32,
    },
    /// Conclude `generation`: every trial of every proposal is judged. Completed by its `Selected`.
    Conclude {
        /// The generation, from 1.
        generation: u32,
    },
    /// The run has reached its target; nothing is recorded. `generation` is the last concluded generation, or 0
    /// when none has concluded.
    Done {
        /// The last concluded generation.
        generation: u32,
    },
}

/// The next step of the run whose ledger folds to `state`, driven under `judge` with `baseline_trials` baseline
/// trials, `candidate_trials` trials per candidate, and the absolute target generation `target`.
///
/// In order:
///
/// 1. The lowest baseline trial below `baseline_trials` with no `Judged` under `judge` is [`Step::Calibrate`], so
///    calibration precedes every proposal (D-06). Judgments under another judge version do not count.
/// 2. Once the last concluded generation is at least `target`, [`Step::Done`]. The target is absolute, not "N
///    more", so a resumed session computes the same `Done` as an uninterrupted one.
/// 3. The working generation `g` is one past the last concluded one. Generation 0 is calibration and is never
///    concluded, so the first working generation is 1. With no proposal in `g`, [`Step::Propose`].
/// 4. The first `(exp, trial)` of `g`, in [`ExperimentId`] order and then trial order, with `trial` below
///    `candidate_trials` and no `Judged` under `judge`, is [`Step::RunTrial`], so the order of launches is a pure
///    function of the ledger.
/// 5. Otherwise every trial of `g` is judged: [`Step::Conclude`].
///
/// A propose step that admits nothing appends the generation's `Selected` itself, so this function never needs
/// to tell an empty generation from an unproposed one.
#[must_use]
pub fn next_step(
    state: &RunState,
    judge: &JudgeVersion,
    baseline_trials: u32,
    candidate_trials: u32,
    target: u32,
) -> Step {
    let baseline = state.baseline();
    if let Some(trial) = (0..baseline_trials).find(|trial| !state.judged(baseline, judge, *trial)) {
        return Step::Calibrate { trial };
    }

    let concluded = state.last_concluded_generation();
    if concluded >= target {
        return Step::Done {
            generation: concluded,
        };
    }

    // `concluded < target <= u32::MAX`, so this never saturates; saturating keeps the arithmetic total.
    let generation = concluded.saturating_add(1);
    let mut proposals = state.proposals_in(generation).peekable();
    if proposals.peek().is_none() {
        return Step::Propose { generation };
    }

    proposals
        .flat_map(|(exp, _)| (0..candidate_trials).map(move |trial| (*exp, trial)))
        .find(|(exp, trial)| !state.judged(exp, judge, *trial))
        .map_or(Step::Conclude { generation }, |(exp, trial)| {
            Step::RunTrial {
                generation,
                exp,
                trial,
            }
        })
}
