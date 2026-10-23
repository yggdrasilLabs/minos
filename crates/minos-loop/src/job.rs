//! The job seam: how the loop runs one trial without knowing about sandboxes (07-CONTEXT D-16).
//!
//! `minos-loop` defines [`JobRunner`] and `minos-runner` implements the real pipeline; the CLI adapts one to the
//! other, so neither crate depends on the other and the loop never links a sandbox or backend type. A fake job
//! runner implements the same trait for the loop's tests (LOOP-09).

use minos_core::job::{JobRequest, JobResult};

/// Runs trials for the loop.
///
/// Synchronous and `&self`, like [`minos_propose::ModelProvider`]: the driver is generic over its job runner and
/// never holds one as a trait object. A candidate's failure is an [`minos_core::Outcome`] inside the
/// [`JobResult`], never a [`JobFailure`]; a [`JobFailure`] means no outcome exists for the trial.
pub trait JobRunner {
    /// Runs one trial of `request`: the candidate VM, then the judge VM if the candidate passed.
    ///
    /// Every usage in the result is measured by the host, never reported by the guest (LOOP-03).
    ///
    /// # Errors
    ///
    /// [`JobFailure::Infra`] when the trial could not run for infrastructure reasons (a launch error, a VM that
    /// ended in a way the host attributes to itself, a store I/O error), and [`JobFailure::Config`] when it can
    /// never run as configured (a spec over the duration ceiling, a preflight refusal, a missing root image or
    /// judge data blob, a diff that does not match the request).
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure>;

    /// Builds and preflights the candidate and judge plans of `request` without launching anything.
    ///
    /// The driver calls it on the baseline's request before generation 0, so a spec over the duration ceiling or
    /// a host refusal stops the run before any VM or model call is paid for.
    ///
    /// # Errors
    ///
    /// The same split as [`JobRunner::run`]: [`JobFailure::Config`] for a plan that can never launch, and
    /// [`JobFailure::Infra`] when the check itself could not complete.
    fn check(&self, request: &JobRequest) -> Result<(), JobFailure>;
}

/// Why a job produced no result. Never an [`minos_core::Outcome`]: no candidate is to blame.
///
/// Each variant carries fixed text only, never bytes from a sandbox, a log, or a model.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JobFailure {
    /// The job failed for infrastructure reasons. The driver records nothing, pauses, retries the same step at
    /// the same `LaunchKey`, and counts the failure toward `max_infra_failures`; any success resets the streak
    /// (07-CONTEXT D-16).
    #[error("the job failed for infrastructure reasons: {what}")]
    Infra {
        /// What failed, as fixed text.
        what: &'static str,
    },
    /// The job can never run as configured. The driver stops with nothing recorded and is never retried, because
    /// the same request fails the same way again; a human changes the configuration.
    #[error("the job cannot run as configured: {what}")]
    Config {
        /// What is wrong, as fixed text.
        what: &'static str,
    },
}

impl JobFailure {
    /// Whether the failure is infrastructure: retried by the driver and counted by its streak.
    #[must_use]
    pub const fn is_infra(&self) -> bool {
        matches!(self, Self::Infra { .. })
    }
}
