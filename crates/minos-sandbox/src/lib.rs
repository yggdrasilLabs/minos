//! Sandbox roles, sealed launch plans, and checks on untrusted output.
//!
//! Roles are types: a sandbox spec is generic over its [`Role`], and policy is enforced when a spec is built, so a
//! spec that breaks a rule cannot be represented. Backends only ever receive a sealed plan. Everything that leaves
//! a sandbox stays wrapped until a validating parser takes it.

pub mod key;
pub mod role;
pub mod spec;

pub use key::LaunchKey;
pub use role::{Candidate, Judge, Role, RoleKind};
pub use spec::{InputDisk, JudgeData, LaunchPlan, Machine, SandboxSpec, SpecError};
