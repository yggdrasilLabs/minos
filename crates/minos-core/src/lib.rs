//! Pure core of Minos: validated types, policy, and reports, with no I/O.
//!
//! This crate has no filesystem, no clock, no environment, no network, and no unseeded randomness. Time, file
//! contents, and blob bytes arrive as arguments, so every function here is a pure function of its inputs and is
//! safe to call during replay.
//!
//! Every parser treats its input as hostile: manifests, diffs, ledger rows, and sandbox output all go through the
//! same validating constructors. Validated types have private fields and no setters, and they deserialize through
//! the same constructor as hand-built values, so if a value exists, it is valid.

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

pub mod report;
