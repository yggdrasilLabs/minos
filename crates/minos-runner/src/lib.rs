//! Runs the candidate VM and then the judge VM for one experiment.
//!
//! The runner takes one `JobRequest` and runs it: the candidate VM builds and runs the experiment, and, if the
//! candidate passed, the judge VM scores its output. It is generic over its `B: Backend` and its blob store, and
//! never names a backend crate, uses a trait object, or defines a backend enum: each binary picks its backend.
//!
//! Three guarantees hold here:
//!
//! - **The runner is the only crate that opens sandbox output** (LOOP-01). Every byte a VM returns stays inside
//!   `Untrusted` until one of its validating exits takes it, and the `map` module turns what the host observed into an
//!   `Outcome`, an infrastructure failure, or a configuration error, by one pure, exhaustively tested table.
//! - **Usage is measured by the host** (LOOP-03). [`usage::host_usage`] multiplies the backend's measured
//!   elapsed time by the plan's machine. The runner reads no clock and no number the guest reported.
//! - **The judge is separate** (LOOP-02). The judge receives the judge data read-only, attached only through
//!   `SandboxSpec<Judge>::secrets`, plus only the candidate's `out/` files. The two VMs share no process and no
//!   disk.
//!
//! Errors carry blame by type ([`error`]): a candidate fault is an `Outcome`, never an error.

pub mod error;
pub mod usage;

pub use error::{ConfigCause, InfraCause, RunnerError};
pub use usage::{RUNNER_VCPUS, host_usage};
