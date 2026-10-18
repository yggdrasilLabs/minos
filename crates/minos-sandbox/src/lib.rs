//! Sandbox roles, sealed launch plans, and checks on untrusted output.
//!
//! Roles are types: a sandbox spec is generic over its [`Role`], and policy is enforced when a spec is built, so a
//! spec that breaks a rule cannot be represented. Backends only ever receive a sealed plan. Everything that leaves
//! a sandbox stays wrapped until a validating parser takes it.
//!
//! # Flow
//!
//! 1. [`SandboxSpec::new`] builds a spec of role `R` from a job; only a `SandboxSpec<Judge>` can take judge data,
//!    through [`SandboxSpec::secrets`], and no role can attach a network.
//! 2. [`SandboxSpec::seal`] turns it into a [`LaunchPlan`], the only constructor of one.
//! 3. [`preflight()`] checks the plan against [`Backend::capabilities`] and returns a [`Cleared`], the only
//!    constructor of one, which owns the plan.
//! 4. [`Backend::launch`] takes the `Cleared` by value and starts the VM under the plan's [`LaunchKey`] and hard
//!    maximum duration. The backend fetches the bytes of the IDs the plan names, and the verified root image path,
//!    through the [`BlobSource`] it was constructed with.
//! 5. [`RunningVm::wait`] returns a [`Finished`]: the host-observed [`Ended`] and elapsed time, the key, the
//!    hostile stream, and the hostile console, whose only exit is [`Finished::console_excerpt`]. The `Finished` is
//!    built by [`Finished::new`], which consumes the launch's `Cleared`.
//! 6. [`Finished::into_output`] reads the stream under caps into an [`Untrusted`] [`GuestOutput`] of role `R`.
//! 7. The exits: [`Untrusted::take_status`] returns PID 1's validated status; [`Untrusted::take`] projects one
//!    file, whose only exit is [`Untrusted::into_log_excerpt`]; [`Untrusted::into_diff_texts`] returns the
//!    submitted diffs; and [`Untrusted::into_scorecard`] exists only on judge output.

pub mod backend;
pub mod blob_source;
pub mod finished;
pub mod key;
pub mod output;
pub mod preflight;
pub mod role;
pub mod spec;
pub mod untrusted;

pub use backend::{Backend, InfraError, LaunchError, RunningVm};
pub use blob_source::{BlobSource, BlobSourceError, VerifiedPath};
pub use finished::{CrashCause, Ended, Finished};
pub use key::LaunchKey;
pub use output::{OutputCap, OutputFault, OutputInvalid};
pub use preflight::{Capabilities, Cleared, Refusal, preflight};
pub use role::{Candidate, Judge, Role, RoleKind};
pub use spec::{InputDisk, JudgeData, LaunchPlan, Machine, SandboxSpec, SpecError};
pub use untrusted::{
    BUILD_LOG_PATH, DIFFS_DIR, GuestOutput, JUDGE_LOG_PATH, OUT_DIR, OutputFile, RUN_LOG_PATH,
    SCORECARD_PATH, STATUS_PATH, ScorecardFault, StatusFault, Untrusted,
};
