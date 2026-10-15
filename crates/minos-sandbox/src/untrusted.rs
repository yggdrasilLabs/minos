//! Hostile sandbox output, wrapped so it cannot be used unvalidated (SBX-04, D-14).
//!
//! Everything that leaves a sandbox is hostile: file names, sizes, contents, logs, and diffs. [`Untrusted`] holds
//! such a value and has no `Debug`, `Display`, `Deref`, `Clone`, or `Default` (the precedent is `GuestBytes` in
//! `minos-backend-krun`), so its contents leave only through the validating exits below, each capped. Only this
//! crate constructs one.
//!
//! [`GuestOutput`] carries the role of the sandbox that wrote it in its type. That is why a candidate's bytes
//! cannot become a scorecard: the scorecard exit exists only for `Untrusted<GuestOutput<Judge>>`, so the compiler
//! refuses the call on candidate output and no runtime role check exists to get wrong.
//!
//! # Layout
//!
//! The constants here are the layout Phase 5's guest writes into its output tar. The run's result travels in the
//! status frame over vsock (02-SPIKE-EVIDENCE.md K7); these files carry only data: the judge's scorecard, the
//! stage logs, the diffs a proposer sandbox submits, and the artifacts under [`OUT_DIR`].

use std::collections::BTreeMap;
use std::marker::PhantomData;

use minos_core::path::RelPath;

use crate::role::Role;

/// The judge's scorecard, read by `Untrusted::<GuestOutput<Judge>>::into_scorecard`.
pub const SCORECARD_PATH: &str = ".minos/scorecard.json";
/// The build stage's log.
pub const BUILD_LOG_PATH: &str = ".minos/build.log";
/// The run stage's log.
pub const RUN_LOG_PATH: &str = ".minos/run.log";
/// The judge stage's log.
pub const JUDGE_LOG_PATH: &str = ".minos/judge.log";
/// The directory whose files are diff texts.
pub const DIFFS_DIR: &str = "diffs";
/// The directory of artifacts the run or the judge produced.
pub const OUT_DIR: &str = "out";

/// A value that came out of a sandbox and has not been validated.
///
/// One private field, and no `Debug`, `Display`, `Deref`, `Clone`, or `Default` implementation. Only this crate
/// constructs one, and the contents leave only through validating, capped exits.
pub struct Untrusted<T>(T);

impl<T> Untrusted<T> {
    /// Wraps `value`. Crate-private, so no other crate can label bytes as sandbox output.
    pub(crate) const fn new(value: T) -> Self {
        Self(value)
    }
}

/// The files one sandbox of role `R` wrote, read in memory from its output tar.
///
/// Paths are validated [`RelPath`]s, kept in path order. No derives, and only this crate constructs one.
pub struct GuestOutput<R: Role> {
    files: BTreeMap<RelPath, Vec<u8>>,
    role: PhantomData<fn() -> R>,
}

impl<R: Role> GuestOutput<R> {
    /// The output holding `files`. Crate-private: only the output reader builds one.
    pub(crate) const fn new(files: BTreeMap<RelPath, Vec<u8>>) -> Self {
        Self {
            files,
            role: PhantomData,
        }
    }
}

impl<R: Role> Untrusted<GuestOutput<R>> {
    /// How many files the output holds. Reveals no content.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.0.files.len()
    }

    /// The total size of the files, in bytes. Reveals no content.
    #[must_use]
    pub fn content_bytes(&self) -> u64 {
        self.0
            .files
            .values()
            .map(|bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            .fold(0, u64::saturating_add)
    }
}
