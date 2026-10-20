//! Prompt assembly, model providers, and proposer tiers.
//!
//! # Trust boundary
//!
//! Model output and every string that came from a sandbox are hostile input. Model output enters as an opaque
//! value with no display or copy path and leaves only through this crate's strict parser, which drops an invalid
//! item rather than repairing it; text shown to a user or placed in a prompt goes through
//! [`minos_core::text::SafeText`].
//!
//! The crate never reads, stores, or forwards a model credential. It reads no process environment: a caller that
//! needs the parent environment checked passes a snapshot of it as a value. It spawns only the user's own
//! `claude` binary, always as an argv array, never through a shell.
//!
//! Providers are used through generics, never trait objects, exactly as the runner is generic over its sandbox
//! backend: each binary picks its provider concretely.

pub mod claude_cli;
pub mod fake;
pub mod limits;
pub mod prompt;
pub mod proposal;
pub mod proposer;
pub mod provider;
pub mod schema;
pub mod single_shot;
pub mod sweep;

pub use fake::{FakeProvider, FakeReply};
pub use prompt::{History, HistoryEntry, Prompt, SteerNote};
pub use proposal::{DropCounts, DropReason, Proposal};
pub use proposer::{Proposals, ProposeError, ProposeInput, Proposer};
pub use provider::{
    Completion, EmptyCause, InfraCause, ModelOutput, ModelProvider, ModelRequest, ModelResponse,
    ProviderError, ProviderStop,
};
pub use schema::OutputSchema;
