//! Sandbox roles, sealed launch plans, and checks on untrusted output.
//!
//! Roles are types: a sandbox spec is generic over its [`Role`], and policy is enforced when a spec is built, so a
//! spec that breaks a rule cannot be represented. Backends only ever receive a sealed plan. Everything that leaves
//! a sandbox stays wrapped until a validating parser takes it.

pub mod finished;
pub mod key;
pub mod output;
pub mod preflight;
pub mod role;
pub mod spec;
pub mod untrusted;

pub use finished::{CrashCause, Ended, Finished};
pub use key::LaunchKey;
pub use output::{OutputCap, OutputFault, OutputInvalid};
pub use preflight::{Capabilities, Cleared, Refusal, preflight};
pub use role::{Candidate, Judge, Role, RoleKind};
pub use spec::{InputDisk, JudgeData, LaunchPlan, Machine, SandboxSpec, SpecError};
pub use untrusted::{GuestOutput, Untrusted};
