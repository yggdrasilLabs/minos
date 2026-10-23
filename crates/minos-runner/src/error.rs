//! The runner's errors, which carry blame by type (CLAUDE.md "Errors separate blame"; 07-CONTEXT D-16).
//!
//! A run step can end three ways, and only two of them are errors:
//!
//! - **The candidate did something wrong.** That is never an error. It is an [`Outcome`](minos_core::Outcome),
//!   recorded in a `Judged` event and never retried; the `map` module decides which.
//! - **Infrastructure failed** ([`RunnerError::Infra`]): a launch, a wait, a store write, or a guest setup step
//!   that has nothing to do with the candidate's code. The loop retries the same step after its pause and counts
//!   the failure toward `[budget] max_infra_failures`; nothing is recorded for the failed attempt.
//! - **The configuration cannot work** ([`RunnerError::Config`]): a spec over the launch ceiling, a host refusal,
//!   a scratch disk too small for the inputs, a missing blob, or a request that does not match the runner. A retry
//!   would fail the same way for every experiment of the run, so the loop stops the run and records nothing.
//!
//! Every cause is a closed enum with fixed text. No variant carries a byte of sandbox output, model output, or a
//! path the guest chose: the payloads are a [`SetupStep`], a [`SpecError`] (numbers and enums), or a set of
//! [`Refusal`]s (numbers and enums).

use std::collections::BTreeSet;
use std::fmt;

use minos_core::guest::SetupStep;
use minos_sandbox::{Refusal, SpecError};

/// Why a run step failed for infrastructure reasons: retried by the loop after its pause and counted toward the
/// circuit breaker (D-16). Never the candidate's fault, so never an `Outcome`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InfraCause {
    /// `Backend::launch` failed for a host reason: an I/O error or a backend fault while starting the VM.
    Launch,
    /// `RunningVm::wait` failed: the host could not observe how the launch ended.
    Wait,
    /// A store read or write failed with a retryable I/O error.
    Store,
    /// The backend's blob source hit an I/O error while fetching a part of the plan.
    BlobIo,
    /// PID 1 reported a failed setup step that is not the candidate's doing: reading the input disk, mounting the
    /// scratch tmpfs, unpacking the inputs, preparing the tree the diff applies to, or packing the output.
    ///
    /// An input failure is infrastructure whatever role the status names, because PID 1 may not have parsed its
    /// job and the role can be a placeholder (review CR-02). [`SetupStep::ScratchFull`] never appears here: a
    /// full scratch is [`ConfigCause::ScratchFull`] (guest.rs WR-02).
    GuestSetup {
        /// The setup step that failed.
        step: SetupStep,
    },
    /// A sink exit's store returned another digest than the bytes' own SHA-256.
    DigestMismatch,
    /// The guest job did not encode.
    GuestJob,
    /// The host observation has no row in the runner's blame table: the runner was handed a launch end it does
    /// not know. Fails closed: retried and counted toward the breaker, never blamed on the candidate.
    Unmapped,
}

impl InfraCause {
    /// A fixed description of the cause, the same text as its `Display`.
    #[must_use]
    pub const fn what(&self) -> &'static str {
        match self {
            Self::Launch => "the sandbox VM could not be launched",
            Self::Wait => "the host could not observe how the sandbox VM ended",
            Self::Store => "a store read or write failed",
            Self::BlobIo => "the backend could not read a blob for the launch",
            Self::GuestSetup { step } => match step {
                SetupStep::Input => "the guest could not read its input disk",
                SetupStep::Mount => "the guest could not mount its scratch space",
                SetupStep::Unpack => "the guest could not unpack its inputs",
                SetupStep::Apply => "the guest could not prepare the tree the diff applies to",
                SetupStep::Output => "the guest could not pack or send its output",
                SetupStep::ScratchFull => "the guest's scratch space filled",
            },
            Self::DigestMismatch => "the store returned another digest than the bytes' own",
            Self::GuestJob => "the guest job did not encode",
            Self::Unmapped => "the runner has no mapping for how the sandbox VM ended",
        }
    }
}

impl fmt::Display for InfraCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.what())
    }
}

/// Why the run cannot proceed with this configuration: the loop stops the run and records nothing (D-16). A retry
/// would build the same plan and fail the same way for every experiment, and the candidate did nothing.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigCause {
    /// A sandbox spec could not be built, such as stage budgets over the launch ceiling. Holds the spec error's
    /// kind and numbers, never text.
    Spec {
        /// The spec error.
        #[source]
        error: SpecError,
    },
    /// The host refused the plan in preflight.
    Preflight {
        /// Every refusal, sorted.
        refusals: BTreeSet<Refusal>,
    },
    /// The build or run filled `[environment] scratch_mib`. The same inputs fill it on every launch, so it is
    /// never retried (guest.rs WR-02).
    ScratchFull,
    /// The fetched diff does not hash to the request's experiment: the loop built a wrong request.
    DiffMismatch,
    /// The fetched diff blob did not parse against the manifest's writable set: the loop stored a wrong blob.
    DiffParse,
    /// The request names another judge version than the runner holds.
    JudgeMismatch,
    /// The manifest's `[environment] memory_mib` does not fit a sandbox machine.
    Machine,
    /// A blob the launch needs (the root image, the judge data, the base tarball, or the diff) is absent from the
    /// store or over its cap.
    BlobMissing,
}

impl ConfigCause {
    /// A fixed description of the cause, the same text as its `Display`.
    #[must_use]
    pub const fn what(&self) -> &'static str {
        match self {
            Self::Spec { .. } => "the sandbox spec could not be built; lower the stage budgets",
            Self::Preflight { .. } => "the host refused the sandbox plan",
            Self::ScratchFull => "the build or run filled [environment] scratch_mib; raise it",
            Self::DiffMismatch => "the diff does not hash to the requested experiment",
            Self::DiffParse => "the stored diff does not parse against the writable set",
            Self::JudgeMismatch => "the request names another judge version than the runner holds",
            Self::Machine => "[environment] memory_mib does not fit a sandbox machine",
            Self::BlobMissing => {
                "a blob the launch needs is missing from the store or over its cap"
            }
        }
    }
}

impl fmt::Display for ConfigCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.what())
    }
}

/// A run step that did not end in an `Outcome`, with its blame in the variant.
///
/// A candidate fault is never a `RunnerError`: it is an `Outcome` the runner returns in its result.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    /// Infrastructure failed: the loop retries the step after its pause and counts it toward the breaker.
    #[error("an infrastructure failure, retried")]
    Infra {
        /// What failed.
        #[source]
        cause: InfraCause,
    },
    /// The configuration cannot work: the loop stops the run and records nothing.
    #[error("a configuration error; the run stops")]
    Config {
        /// What is wrong.
        #[source]
        cause: ConfigCause,
    },
}

impl RunnerError {
    /// Whether the error is an infrastructure failure, which the loop retries. Every other error stops the run.
    #[must_use]
    pub const fn is_infra(&self) -> bool {
        matches!(self, Self::Infra { .. })
    }

    /// The cause's fixed description, which the CLI adapter passes into the loop's job failure.
    #[must_use]
    pub const fn what(&self) -> &'static str {
        match self {
            Self::Infra { cause } => cause.what(),
            Self::Config { cause } => cause.what(),
        }
    }
}

impl From<InfraCause> for RunnerError {
    fn from(cause: InfraCause) -> Self {
        Self::Infra { cause }
    }
}

impl From<ConfigCause> for RunnerError {
    fn from(cause: ConfigCause) -> Self {
        Self::Config { cause }
    }
}
