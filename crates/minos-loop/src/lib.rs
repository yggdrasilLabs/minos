//! Local generation driver, resumable from the ledger.
//!
//! # Lane
//!
//! The loop links `minos-core`, `minos-store`, and `minos-propose` only, and `deny.toml` refuses any other
//! internal edge. It never names a sandbox or backend type, so it never sees raw sandbox output: trials run
//! through the [`JobRunner`] seam, which the CLI implements over `minos-runner`, and come back as validated
//! [`minos_core::JobResult`] values.
//!
//! # Shape
//!
//! The loop is synchronous and generic over its store, model provider, job runner, and observer; it never holds
//! any of them as a trait object. It reads no clock, no environment, and no file except through the store
//! traits.
//!
//! # State
//!
//! Run state is never kept in memory between steps. The ledger fold is the only run state: every effect is
//! recorded before the next act, keyed by a content-derived ID (`EventKey`, `SpendId`, `LaunchKey`), so a
//! resumed session re-folds the ledger and performs the same next step an uninterrupted one would
//! (07-CONTEXT D-07).
//!
//! # Output
//!
//! Every value that leaves the loop, in a [`LoopEvent`] or a [`DriverError`], is an ID, a number, a closed enum,
//! or a validated value. No text a sandbox or a model wrote leaves the loop.

pub mod error;
pub mod job;
pub mod observe;
pub mod stage;

pub use error::DriverError;
pub use job::{JobFailure, JobRunner};
pub use observe::{LoopEvent, NoopObserver, Observer};
pub use stage::{CANDIDATE_TRIALS, Step, next_step};
