//! `Outcome`, `Reason`, and `Stage`: an experiment's failure is a value, never an error.
//!
//! Every experiment that ran ends in an [`Outcome`], which the ledger records in a `Judged` event. A candidate
//! fault becomes [`Outcome::Rejected`] or [`Outcome::Crashed`] and is never retried. An infrastructure failure is
//! not an `Outcome` at all: it is an error on a separate path, and the runner retries it. No core function
//! returns an `Outcome` inside `Err`.
//!
//! The runner maps what the host observed of a VM run to an outcome by the Phase 2 K7 classification rule
//! (02-SPIKE-EVIDENCE.md, "Classification rule for Phase 5"), as refined in Phase 5 for the console pipe and the
//! chunked output stream (the krun backend's `supervise::classify`). The helper's exit code alone never carries
//! the guest's result:
//!
//! | Host observation | Result |
//! |---|---|
//! | Exit 0 and a complete output stream (its END chunk arrived) | The guest decided; its status file says what happened |
//! | Exit 0 with no END chunk | [`Outcome::Crashed`] |
//! | The console passed its cap, read by the host from the console pipe, and the host killed the VM | [`Outcome::Crashed`], never retried |
//! | The output stream broke the chunk protocol (a wrong token, a gap, a duplicate, a bad END, an over-cap chunk) | [`Outcome::Rejected`] with [`Reason::ChunkProtocol`], never retried |
//! | The host deadline, or exit 124 with `minos-vmm: watchdog deadline` | [`Outcome::TimedOut`] |
//! | Exit 64 to 71, exit 126 (with or without `minos-vmm: watchdog parent-gone`), exit 124 without its marker, a spawn failure, a signal the host did not send, any other code | Infrastructure failure, not an `Outcome`; retried |
//!
//! A marker counts only as an exact whole line of the helper's stderr together with its exit code. `minos-vmm`
//! writes the markers, the guest reaches only the console, and every line the helper logs starts with
//! `minos-vmm: log `, so no logged text can be a marker. With the console on a pipe the helper no longer exits 125
//! with a `console-cap` marker; the console cap is observed by the host.
//!
//! [`Outcome::Crashed`] and [`Outcome::TimedOut`] carry the [`Stage`] they happened in.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::MetricName;
use crate::metric::Scorecard;
use crate::path::RelPath;

/// Which VM step an outcome refers to. Also the key of a job result's log blobs (plan 03-09).
///
/// A complete set: an experiment runs a build, a run, and a judge step, in that order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The project's build command.
    Build,
    /// The project's run command.
    Run,
    /// The judge command.
    Judge,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Build => "build",
            Self::Run => "run",
            Self::Judge => "judge",
        })
    }
}

/// Why a candidate was rejected. Every variant is a candidate fault, never retried.
///
/// Every payload is a validated type ([`MetricName`], [`RelPath`], or a number), never free text, so a reason
/// can be shown without passing hostile text to a terminal.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Reason {
    /// A `[[judge.gate]]` metric did not pass.
    GateFailed {
        /// The gate's metric.
        gate: MetricName,
    },
    /// The scorecard lacks a metric the judge declares.
    MissingMetric {
        /// The missing metric.
        metric: MetricName,
    },
    /// The scorecard has a metric the judge does not declare.
    UndeclaredMetric {
        /// The undeclared metric.
        metric: MetricName,
    },
    /// A metric's `held_out` flag differs from the judge's declaration.
    HeldOutMismatch {
        /// The metric.
        metric: MetricName,
    },
    /// The judge's scorecard bytes did not parse as a scorecard.
    ScorecardInvalid,
    /// The diff did not apply to the base tree: `ApplyFailed { path: RelPath, hunk: u32 }`.
    ApplyFailed {
        /// The file whose hunk did not apply.
        path: RelPath,
        /// 1-based number of the hunk that did not apply; 0 means the file is missing from the base tree.
        hunk: u32,
    },
    /// The build command failed.
    BuildFailed,
    /// The run command failed.
    RunFailed,
    /// The candidate's output tree is not valid: a bad path, entry type, or shape.
    OutputInvalid,
    /// The candidate's output is over its cap.
    OutputTooLarge,
    /// The vsock output broke the chunk protocol: a gap, a duplicate, a length mismatch, or an over-cap chunk
    /// (02-SPIKE-EVIDENCE.md S03-ANSWER).
    ChunkProtocol,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GateFailed { gate } => write!(f, "gate {gate} failed"),
            Self::MissingMetric { metric } => write!(f, "the scorecard has no metric {metric}"),
            Self::UndeclaredMetric { metric } => {
                write!(f, "the scorecard has undeclared metric {metric}")
            }
            Self::HeldOutMismatch { metric } => {
                write!(f, "metric {metric} has the wrong held_out flag")
            }
            Self::ScorecardInvalid => f.write_str("the scorecard is invalid"),
            Self::ApplyFailed { path, hunk: 0 } => write!(f, "{path} is not in the base tree"),
            Self::ApplyFailed { path, hunk } => write!(f, "hunk {hunk} of {path} does not apply"),
            Self::BuildFailed => f.write_str("the build failed"),
            Self::RunFailed => f.write_str("the run failed"),
            Self::OutputInvalid => f.write_str("the output is invalid"),
            Self::OutputTooLarge => f.write_str("the output is over its cap"),
            Self::ChunkProtocol => f.write_str("the output stream broke the chunk protocol"),
        }
    }
}

/// How an experiment ended. Failure is a value: the ledger records it and policy reads it.
///
/// A complete set fixed by CORE-07:
/// `Scored(Scorecard) | Rejected(Reason) | Crashed { stage: Stage } | TimedOut { stage: Stage }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    /// The judge produced a valid scorecard.
    Scored(Scorecard),
    /// The candidate was rejected for a stated reason.
    Rejected(Reason),
    /// The VM ended without a valid result, no status frame or the console cap: `Crashed { stage: Stage }`.
    Crashed {
        /// The step that crashed.
        stage: Stage,
    },
    /// A deadline fired: `TimedOut { stage: Stage }`.
    TimedOut {
        /// The step whose deadline fired.
        stage: Stage,
    },
}

impl Outcome {
    /// Whether the outcome is [`Outcome::Scored`].
    #[must_use]
    pub const fn is_scored(&self) -> bool {
        matches!(self, Self::Scored(_))
    }

    /// The scorecard, if the outcome is [`Outcome::Scored`].
    #[must_use]
    pub const fn scorecard(&self) -> Option<&Scorecard> {
        match self {
            Self::Scored(card) => Some(card),
            Self::Rejected(_) | Self::Crashed { .. } | Self::TimedOut { .. } => None,
        }
    }
}
