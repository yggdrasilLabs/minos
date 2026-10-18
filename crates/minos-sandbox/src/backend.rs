//! The backend contract: how a cleared plan becomes a running VM (SBX-03, SBX-06, D-17).
//!
//! A [`Backend`] turns a [`Cleared`] plan into a running VM and reports what the host can run. The runner is
//! generic over `B: Backend`, and each binary picks its backend concretely. A binary that chooses among several
//! backends at runtime defines its own enum outside this crate. Never use a trait object, and never put a backend
//! enum in `minos-sandbox`.
//!
//! The flow is `preflight(&backend.capabilities(), plan)`, then [`Backend::launch`], then [`RunningVm::wait`],
//! which returns a [`Finished`]. A candidate fault is a value inside `Finished` (its [`crate::Ended`] and its
//! untrusted output), never an `Err`. [`LaunchError`] and [`InfraError`] are infrastructure failures only, and the
//! runner retries them.
//!
//! The trait takes no blob store. `deny.toml` keeps `minos-store` out of backend crates, so a backend is
//! constructed with a [`BlobSource`](crate::BlobSource), through which it fetches the bytes of the IDs a plan
//! names under caps and the verified root image path (05-CONTEXT D-13, resolving 04-RESEARCH Open Question 7).
//! Its failures reach the runner as [`LaunchError::Blob`].

use crate::blob_source::BlobSourceError;
use crate::finished::Finished;
use crate::preflight::{Capabilities, Cleared};
use crate::role::Role;

/// Why a backend could not launch a cleared plan. An infrastructure failure that the runner retries, never an
/// `Outcome`: the candidate has not run yet.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    /// A host I/O operation failed while preparing or starting the VM.
    #[error("launch failed at {op}")]
    Io {
        /// The operation that failed, as a fixed name.
        op: &'static str,
        /// The I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The backend failed for a reason of its own.
    #[error("the backend failed to launch: {what}")]
    Backend {
        /// What failed, as a fixed description.
        what: &'static str,
    },
    /// The [`BlobSource`](crate::BlobSource) could not supply a blob or the root image path the plan names. An
    /// infrastructure failure the runner retries, except [`BlobSourceError::RootImageMissing`], which
    /// `minos check` (Phase 8) reports as "run minos env build".
    #[error("a blob the plan names could not be read")]
    Blob(#[from] BlobSourceError),
}

/// Why a backend could not observe how a launch ended. An infrastructure failure that the runner retries, never
/// an `Outcome`: a crash, a timeout, or bad output is reported inside [`Finished`] instead.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum InfraError {
    /// A host I/O operation failed while supervising the VM.
    #[error("supervising the launch failed at {op}")]
    Io {
        /// The operation that failed, as a fixed name.
        op: &'static str,
        /// The I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The backend failed for a reason of its own.
    #[error("the backend failed while the launch ran: {what}")]
    Backend {
        /// What failed, as a fixed description.
        what: &'static str,
    },
}

/// A VM of role `R` that a backend launched.
///
/// Dropping the value destroys the VM. Nothing depends on that drop running, because every launch carries the
/// plan's hard maximum duration.
pub trait RunningVm<R: Role> {
    /// Blocks until the guest ends or the plan's hard maximum duration passes, and returns how it ended.
    ///
    /// A candidate fault is reported inside the [`Finished`]: its host-observed [`crate::Ended`] (`Crashed`,
    /// `TimedOut`) and its untrusted output stream. It is never an `Err`. An implementer moves the launch's
    /// [`crate::Cleared`] into [`Finished::new`], which consumes it.
    ///
    /// # Errors
    ///
    /// An [`InfraError`] when the host failed to supervise the launch. The runner retries it.
    fn wait(self) -> Result<Finished<R>, InfraError>;
}

/// A way to run sandboxes: exactly two methods and one generic associated type.
///
/// The runner is generic over `B: Backend`; never a trait object.
pub trait Backend {
    /// A running VM of role `R`.
    type Running<R: Role>: RunningVm<R>;

    /// What this backend's host can run, as the backend checked it. The runner passes it to
    /// [`crate::preflight()`], and `minos host doctor` reports its host refusals.
    fn capabilities(&self) -> Capabilities;

    /// Launches the cleared plan `ok`.
    ///
    /// Takes the clearance by value, so one clearance authorizes one launch. The backend derives its idempotency
    /// token from `ok.plan().key()` and enforces `ok.plan().max_duration()` as a hard limit on every launch, so no
    /// VM outlives its plan even when nothing drops the running value.
    ///
    /// # Errors
    ///
    /// A [`LaunchError`] when the host failed to start the VM. The runner retries it.
    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>;
}
