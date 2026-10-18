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
//! The constants here are the layout Phase 5's guest writes into its output tar: PID 1's own status at
//! [`STATUS_PATH`], which leaves only through [`Untrusted::take_status`], the judge's scorecard, the stage logs, the
//! diffs a proposer sandbox submits, and the artifacts under [`OUT_DIR`].

use std::collections::BTreeMap;
use std::marker::PhantomData;

use minos_core::guest::{GuestRole, GuestStatus, GuestStatusError, MAX_STATUS_BYTES};
use minos_core::limits::Limits;
use minos_core::metric::{Scorecard, ScorecardError};
use minos_core::outcome::Reason;
use minos_core::path::RelPath;
use minos_core::text::LogExcerpt;

use crate::output::{MAX_DIFF_TEXTS, OutputCap, OutputFault, OutputInvalid};
use crate::role::{Judge, Role, RoleKind};

pub use minos_core::guest::STATUS_PATH;

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

/// One file of a role-`R` sandbox's output, still hostile. No trait implementations, and only this crate
/// constructs one.
pub struct OutputFile<R: Role> {
    bytes: Vec<u8>,
    role: PhantomData<fn() -> R>,
}

/// Why judge output did not yield a scorecard. A candidate fault: the judge ran the candidate's artifacts, so the
/// experiment is rejected, never retried.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ScorecardFault {
    /// The output has no file at [`SCORECARD_PATH`].
    #[error("the judge wrote no scorecard")]
    Missing,
    /// The scorecard file did not parse as a scorecard.
    #[error("the judge's scorecard is invalid")]
    Invalid(#[source] ScorecardError),
}

impl ScorecardFault {
    /// The ledger reason: [`Reason::ScorecardInvalid`] for every variant.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        match self {
            Self::Missing | Self::Invalid(_) => Reason::ScorecardInvalid,
        }
    }
}

/// Why the guest's status file did not yield a [`GuestStatus`]. A candidate fault: the status came from inside the
/// VM, so a missing, malformed, or wrong-role status rejects the experiment and is never retried.
///
/// No variant carries a byte of the file: [`GuestStatusError`] keeps only a line and column.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StatusFault {
    /// The output has no file at [`STATUS_PATH`].
    #[error("the guest wrote no status")]
    Missing,
    /// The status file is over [`MAX_STATUS_BYTES`] or did not parse as a guest status.
    #[error("the guest's status is invalid")]
    Invalid(#[source] GuestStatusError),
    /// The status parsed but names a role other than the sandbox's own.
    #[error("the guest's status names another role")]
    WrongRole,
}

impl StatusFault {
    /// The ledger reason: [`Reason::OutputInvalid`] for every variant.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        match self {
            Self::Missing | Self::Invalid(_) | Self::WrongRole => Reason::OutputInvalid,
        }
    }
}

/// The status role a sandbox of role kind `kind` must report.
const fn guest_role(kind: RoleKind) -> GuestRole {
    match kind {
        RoleKind::Candidate => GuestRole::Candidate,
        RoleKind::Judge => GuestRole::Judge,
    }
}

/// Whether `path` lies under [`DIFFS_DIR`].
fn is_diff(path: &RelPath) -> bool {
    path.as_str()
        .strip_prefix(DIFFS_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
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

    /// Removes the file at `path` and returns it, still wrapped and still of role `R`.
    ///
    /// A projection, not an exit: the bytes stay hostile, and leave only through an exit on
    /// [`Untrusted<OutputFile<R>>`]. Returns `None` when there is no such file, including on a second call.
    pub fn take(&mut self, path: &RelPath) -> Option<Untrusted<OutputFile<R>>> {
        self.0.files.remove(path).map(|bytes| {
            Untrusted::new(OutputFile {
                bytes,
                role: PhantomData,
            })
        })
    }

    /// Removes PID 1's status, the file at [`STATUS_PATH`], and returns it validated: the only exit for that file.
    ///
    /// The status is authenticated by the chunk token, but a guest that exploited its kernel could forge it, so it
    /// is parsed as hostile: refused over [`MAX_STATUS_BYTES`] before the parser sees a byte, parsed by
    /// [`GuestStatus::parse_json`] (unknown fields refused, every check of [`GuestStatus::new`] applied), and
    /// refused when its role is not `R`. The file is removed whether or not it validates, so a later call
    /// returns [`StatusFault::Missing`].
    ///
    /// # Errors
    ///
    /// [`StatusFault::Missing`] when there is no status file; [`StatusFault::Invalid`] when it is over the cap or
    /// not a guest status; [`StatusFault::WrongRole`] when it names a role other than `R`.
    pub fn take_status(&mut self) -> Result<GuestStatus, StatusFault> {
        let path = self
            .0
            .files
            .keys()
            .find(|path| path.as_str() == STATUS_PATH)
            .cloned()
            .ok_or(StatusFault::Missing)?;
        let bytes = self.0.files.remove(&path).ok_or(StatusFault::Missing)?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusFault::Invalid(GuestStatusError::TooLarge));
        }
        let status = GuestStatus::parse_json(&bytes).map_err(StatusFault::Invalid)?;
        if status.role() != guest_role(R::KIND) {
            return Err(StatusFault::WrongRole);
        }
        Ok(status)
    }

    /// The diff texts under [`DIFFS_DIR`], in path order. Files elsewhere are ignored.
    ///
    /// This exit checks shape and caps only: at most [`MAX_DIFF_TEXTS`] texts, each at most
    /// [`Limits::max_diff_bytes`], valid UTF-8 with no NUL and no carriage return. The texts are still model or
    /// sandbox output; `Diff::parse` validates their content against the writable set.
    ///
    /// # Errors
    ///
    /// [`OutputFault::TooLarge`] with [`OutputCap::DiffCount`] for more than [`MAX_DIFF_TEXTS`] texts, or with
    /// [`OutputCap::DiffBytes`] for a text over the byte cap; [`OutputFault::Invalid`] with
    /// [`OutputInvalid::DiffNotText`] for a text that is not UTF-8 or holds a NUL or a carriage return.
    pub fn into_diff_texts(self, limits: &Limits) -> Result<Vec<String>, OutputFault> {
        let diffs: Vec<Vec<u8>> = self
            .0
            .files
            .into_iter()
            .filter(|(path, _)| is_diff(path))
            .map(|(_, bytes)| bytes)
            .collect();
        if diffs.len() > MAX_DIFF_TEXTS {
            return Err(OutputFault::TooLarge {
                cap: OutputCap::DiffCount,
            });
        }
        diffs
            .into_iter()
            .map(|bytes| {
                if bytes.len() > limits.max_diff_bytes() {
                    return Err(OutputFault::TooLarge {
                        cap: OutputCap::DiffBytes,
                    });
                }
                String::from_utf8(bytes)
                    .ok()
                    .filter(|text| !text.contains(['\0', '\r']))
                    .ok_or(OutputFault::Invalid {
                        why: OutputInvalid::DiffNotText,
                    })
            })
            .collect()
    }
}

impl Untrusted<GuestOutput<Judge>> {
    /// The judge's scorecard, from the file at [`SCORECARD_PATH`], through [`Scorecard::parse_json`] (which checks
    /// the byte cap before parsing).
    ///
    /// Implemented for judge output only, so candidate bytes have no path to a scorecard: the compiler refuses the
    /// call on `Untrusted<GuestOutput<Candidate>>` (D-14; GUEST-04 enforced by the type system). The compile-fail
    /// proof lands with plan 04-08.
    ///
    /// # Errors
    ///
    /// [`ScorecardFault::Missing`] when there is no scorecard file, and [`ScorecardFault::Invalid`] when its bytes
    /// are not a scorecard.
    pub fn into_scorecard(self, limits: &Limits) -> Result<Scorecard, ScorecardFault> {
        let bytes = self
            .0
            .files
            .into_iter()
            .find(|(path, _)| path.as_str() == SCORECARD_PATH)
            .map(|(_, bytes)| bytes)
            .ok_or(ScorecardFault::Missing)?;
        Scorecard::parse_json(&bytes, limits).map_err(ScorecardFault::Invalid)
    }
}

impl<R: Role> Untrusted<OutputFile<R>> {
    /// The sanitized tail of the file as a log excerpt, through [`LogExcerpt::from_bytes`]: at most `max` bytes,
    /// clamped to 4096, with control characters dropped and invisible format characters shown as `<U+XXXX>`.
    #[must_use]
    pub fn into_log_excerpt(self, max: usize) -> LogExcerpt {
        LogExcerpt::from_bytes(&self.0.bytes, max)
    }
}
