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
//!
//! # Sink exits
//!
//! Two exits release bytes rather than validated values, and each hands them only to a [`BlobSink`], a
//! content-addressed blob put, returning nothing but the [`BlobId`] the sink named them by (07-CONTEXT D-17,
//! approved by the user at the 07-04 checkpoint on 2026-10-22, trait form):
//!
//! - [`Untrusted::store_judge_input`], on candidate output only, re-packs the files under [`OUT_DIR`] as a
//!   canonical ustar archive. It is the only way to make the judge's `candidate_out` blob (LOOP-02).
//! - [`Untrusted::store_tail`], on one file of any role, keeps at most [`MAX_STAGE_LOG_BYTES`] raw bytes of a
//!   stage log (D-09). Readers turn such a blob into text only through [`LogExcerpt::from_bytes`].
//!
//! The bytes go to a sink and never back to the caller as a value, so the only things that ever hold them are a
//! blob store and, for the judge input, another VM. The sink can see the bytes, as any put interface must; the
//! guarantee is that the exit returns none.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use minos_core::guest::{
    GuestRole, GuestStatus, GuestStatusError, MAX_CANDIDATE_OUT_BYTES, MAX_STATUS_BYTES,
    SetupStatus, SetupStep,
};
use minos_core::id::BlobId;
use minos_core::limits::{Limits, MAX_STAGE_LOG_BYTES};
use minos_core::metric::{Scorecard, ScorecardError};
use minos_core::outcome::Reason;
use minos_core::path::RelPath;
use minos_core::text::LogExcerpt;
use tar::{Builder, EntryType, Header};

use crate::output::{MAX_DIFF_TEXTS, OutputCap, OutputFault, OutputInvalid};
use crate::role::{Candidate, Judge, Role, RoleKind};

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
    ///
    /// Not raised for a status whose setup failed at [`SetupStep::Input`]: PID 1 sends that status when it could
    /// not read or parse its job, so the role it carries may be a placeholder. The host reads `setup` before
    /// `role` for that one case, and Phase 7 maps every failed setup to an infrastructure failure before it reads
    /// any other field (review CR-02).
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

/// Whether `path` lies under [`OUT_DIR`]: the `/` after `out` is required, so a file named `out` and a path under
/// `outx/` are not artifacts.
fn is_artifact(path: &RelPath) -> bool {
    path.as_str()
        .strip_prefix(OUT_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Where the two sink exits put the bytes they release: a content-addressed blob put (07-CONTEXT D-17).
///
/// The production implementor is `minos-runner`'s adapter over `minos-store`'s `Blobs::put`; tests implement it
/// over a recorder. The trait is not sealed and cannot be: this crate does not link `minos-store`, so the store's
/// adapter must be able to implement it from outside. Sealing would add nothing against the guest, whose bytes
/// reach a sink only through an exit that checks the returned digest.
pub trait BlobSink {
    /// The store's own error.
    type Error: std::error::Error + 'static;

    /// Stores `bytes` as one blob and returns the ID it is stored under, which must be the SHA-256 of `bytes`
    /// ([`BlobId::of`]); an exit refuses any other ID with [`SinkError::DigestMismatch`].
    ///
    /// # Errors
    ///
    /// The store's own error when it could not store the blob.
    fn put(&self, bytes: &[u8]) -> Result<BlobId, Self::Error>;
}

/// Why a sink exit returned no [`BlobId`]. No variant carries a byte of the output.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SinkError<E> {
    /// The candidate's `out/` files could not be re-packed as a ustar archive. A candidate fault: the runner
    /// records it as `Rejected(OutputInvalid)` and never retries it.
    #[error("the candidate's out/ files could not be re-packed")]
    Repack,
    /// The sink named the blob by a digest other than its content's. An infrastructure fault.
    #[error("the store named the blob by a digest other than its content's")]
    DigestMismatch,
    /// The sink failed. The store's own error, retried if the store says it is retryable.
    #[error("the blob store failed")]
    Store(#[source] E),
}

/// Hands `bytes` to `sink` once and returns the blob's ID, only after checking it is the content's own digest.
fn sink_put<S: BlobSink>(sink: &S, bytes: &[u8]) -> Result<BlobId, SinkError<S::Error>> {
    let expected = BlobId::of(bytes);
    let stored = sink.put(bytes).map_err(SinkError::Store)?;
    if stored == expected {
        Ok(expected)
    } else {
        Err(SinkError::DigestMismatch)
    }
}

/// The canonical header of one re-packed artifact, built with exactly these calls in this order (the recipe of
/// `minos-source`'s packer, with the mode fixed at 0644).
fn artifact_header(path: &RelPath, size: usize) -> Option<Header> {
    let size = u64::try_from(size).ok()?;
    let mut header = Header::new_ustar();
    header.set_path(path.as_str()).ok()?;
    header.set_size(size);
    header.set_mode(0o644);
    // Set explicitly: they write seven `0` digits and a NUL, where new_ustar leaves zero bytes.
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    Some(header)
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
    /// The role check is skipped for a status whose setup failed at [`SetupStep::Input`]: an input failure
    /// precedes the role, because PID 1 may not have parsed its job, so the role in that status can be a
    /// placeholder. Such a status is an infrastructure failure whatever its role says, and the caller must map
    /// it to one before reading any other field (review CR-02).
    ///
    /// # Errors
    ///
    /// [`StatusFault::Missing`] when there is no status file; [`StatusFault::Invalid`] when it is over the cap or
    /// not a guest status; [`StatusFault::WrongRole`] when its setup did not fail at the input step and it names
    /// a role other than `R`.
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
        let input_failed = matches!(
            status.setup(),
            SetupStatus::Failed {
                step: SetupStep::Input
            }
        );
        if !input_failed && status.role() != guest_role(R::KIND) {
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

impl Untrusted<GuestOutput<Candidate>> {
    /// Re-packs the candidate's artifacts, the files under [`OUT_DIR`], and stores them through `sink` as the
    /// judge's `candidate_out` blob. The only way to make that blob (LOOP-02, 07-CONTEXT D-17).
    ///
    /// Implemented for candidate output only. The archive holds only files whose path starts with `out/`, in path
    /// order; everything else is dropped: PID 1's status, the stage logs, the diffs, a scorecard-shaped file, a
    /// file named `out`, and any path under `outx/` or another directory. Each entry is a regular file with a
    /// hand-built ustar header: the path, the size, mode 0644, uid, gid, and mtime 0, empty user and group names,
    /// and the checksum. The archive ends with two 512-byte zero blocks and nothing else, so an output with no
    /// artifact becomes 1024 zero bytes. The judge VM's PID 1 unpacks it under `/scratch/candidate`, so the judge
    /// reads `/scratch/candidate/out/<file>`.
    ///
    /// The archive is a subset of a stream the output reader already held to [`MAX_CANDIDATE_OUT_BYTES`]: the
    /// same names, so every header fits, and the same sizes and padding. It is checked against that cap again
    /// before it is stored.
    ///
    /// Calls [`BlobSink::put`] exactly once on success of the re-pack, and returns the archive's [`BlobId`] only
    /// after checking `sink` returned that same ID. Returns no byte.
    ///
    /// # Errors
    ///
    /// [`SinkError::Repack`] when an artifact's header cannot be written as ustar or the archive is over
    /// [`MAX_CANDIDATE_OUT_BYTES`] (a candidate fault, recorded as `Rejected(OutputInvalid)`);
    /// [`SinkError::DigestMismatch`] when `sink` returned another ID; [`SinkError::Store`] when `sink` failed.
    pub fn store_judge_input<S: BlobSink>(self, sink: &S) -> Result<BlobId, SinkError<S::Error>> {
        let mut builder = Builder::new(Vec::new());
        for (path, bytes) in self.0.files.iter().filter(|(path, _)| is_artifact(path)) {
            let header = artifact_header(path, bytes.len()).ok_or(SinkError::Repack)?;
            builder
                .append(&header, bytes.as_slice())
                .map_err(|_| SinkError::Repack)?;
        }
        let archive = builder.into_inner().map_err(|_| SinkError::Repack)?;
        let size = u64::try_from(archive.len()).map_err(|_| SinkError::Repack)?;
        if size > MAX_CANDIDATE_OUT_BYTES {
            return Err(SinkError::Repack);
        }
        sink_put(sink, &archive)
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

    /// Stores the last `min(max, MAX_STAGE_LOG_BYTES)` bytes of the file through `sink`, unsanitized, and returns
    /// only the blob's [`BlobId`] (07-CONTEXT D-09, D-17).
    ///
    /// The bytes are a stage log and stay hostile in the store: every reader turns the blob into text only through
    /// [`LogExcerpt::from_bytes`]. Calls [`BlobSink::put`] exactly once, and returns the ID only after checking
    /// `sink` returned the tail's own digest. Returns no byte.
    ///
    /// # Errors
    ///
    /// [`SinkError::DigestMismatch`] when `sink` returned another ID, and [`SinkError::Store`] when `sink` failed.
    pub fn store_tail<S: BlobSink>(
        self,
        max: usize,
        sink: &S,
    ) -> Result<BlobId, SinkError<S::Error>> {
        let bytes = self.0.bytes.as_slice();
        let start = bytes.len().saturating_sub(max.min(MAX_STAGE_LOG_BYTES));
        let tail = bytes
            .split_at_checked(start)
            .map(|(_, tail)| tail)
            .unwrap_or_default();
        sink_put(sink, tail)
    }
}
