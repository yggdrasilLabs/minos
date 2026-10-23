//! The loop's stage function (07-CONTEXT D-06, D-07).

use minos_core::id::ExperimentId;
use minos_core::manifest::JudgeVersion;
use minos_core::state::RunState;

/// Trials per candidate in v1.
pub const CANDIDATE_TRIALS: u32 = 1;

/// The driver's next action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Run a baseline trial.
    Calibrate {
        /// The trial.
        trial: u32,
    },
    /// Propose a generation.
    Propose {
        /// The generation.
        generation: u32,
    },
    /// Run a candidate trial.
    RunTrial {
        /// The generation.
        generation: u32,
        /// The experiment.
        exp: ExperimentId,
        /// The trial.
        trial: u32,
    },
    /// Conclude a generation.
    Conclude {
        /// The generation.
        generation: u32,
    },
    /// The run reached its target.
    Done {
        /// The last concluded generation.
        generation: u32,
    },
}

/// The next step. Not yet implemented.
#[must_use]
pub fn next_step(
    state: &RunState,
    _judge: &JudgeVersion,
    _baseline_trials: u32,
    _candidate_trials: u32,
    _target: u32,
) -> Step {
    Step::Done {
        generation: state.last_concluded_generation(),
    }
}
