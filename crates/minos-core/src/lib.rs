//! Pure core of Minos: validated types, policy, and reports, with no I/O.
//!
//! This crate has no filesystem, no clock, no environment, no network, and no unseeded randomness. Time, file
//! contents, and blob bytes arrive as arguments, so every function here is a pure function of its inputs and is
//! safe to call during replay.
//!
//! Every parser treats its input as hostile: manifests, diffs, ledger rows, and sandbox output all go through the
//! same validating constructors. Validated types have private fields and no setters, and they deserialize through
//! the same constructor as hand-built values, so if a value exists, it is valid.
//!
//! The entry points are [`Manifest::parse_toml`] (a project manifest), [`Diff::parse`] (a proposed change),
//! [`fold`] (ledger records into a [`RunState`]), [`policy::conclude`] (one generation's decision),
//! [`route::route`] (which proposer tier runs next), and [`build_report`] (the review artifact). The principal
//! types are re-exported at the crate root; every module stays public, so the module paths keep working.

pub mod limits;

pub mod id;

pub mod path;

pub mod text;

pub mod metric;

pub mod outcome;

pub mod usage;

pub mod money;

pub mod image;

pub mod manifest;

pub mod diff;

pub mod experiment;

pub mod ledger;

pub mod state;

pub mod policy;

pub mod route;

pub mod sweep;

pub mod job;

pub mod chunk;

pub mod lock;

pub mod guest;

pub mod report;

// id: SHA-256 identifiers and validated names
pub use id::{
    BlobId, ExperimentId, IdHasher, ImageDigest, ManifestId, MetricName, ModelId, PricesId,
    PromptHash, ReportHash, RunId, TreeId,
};

// path: relative paths with only normal components
pub use path::RelPath;

// limits: every cap and default
pub use limits::Limits;

// text: the allowlist renderer for hostile text
pub use text::{LogExcerpt, SafeText};

// metric: finite metrics and scorecards
pub use metric::{Metric, Scorecard};

// outcome: experiment failure as a value
pub use outcome::{Outcome, Reason, Stage};

// usage: metered usage, never dollars
pub use usage::Usage;

// money: exact pico-dollar rates and the one dollar formatter
pub use money::{ApproxCost, Cost, Prices, Rate};

// image: digest-pinned image references
pub use image::ImageRef;

// manifest: the validated project manifest
pub use manifest::{Goal, JudgeSpec, JudgeVersion, Manifest, Objective, Tier};

// diff: the parsed canonical diff, the tree it applies to, and the diff cumulative from the run's base
pub use diff::Diff;
pub use diff::apply::Tree;
pub use diff::cumulative::{BaseTree, CumulativeDiff};

// experiment: a base tree plus an optional diff
pub use experiment::Experiment;

// ledger: the frozen event set and its wire records
pub use ledger::{BackendInfo, Decision, Event, EventKey, Line, Record, SpendId, StopReason};

// state: run state folded from ledger records
pub use state::{RunState, fold};

// chunk: the authenticated vsock output codec
pub use chunk::{ChunkToken, Reassembler};

// lock: the minos.lock wire type
pub use lock::Lock;

// guest: the backend-to-guest contract
pub use guest::{GuestJob, GuestStatus};

// job: the loop-to-runner contract
pub use job::{JobRequest, JobResult};

// report: the review artifact
pub use report::{Report, build_report};
