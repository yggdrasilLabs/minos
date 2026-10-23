//! The store: an append-only event ledger per run and a content-addressed blob store (STORE-01).
//!
//! Three traits, [`Ledger`], [`Blobs`], and [`RunLock`], are the whole contract. They are synchronous, with
//! `&self` methods and no supertraits, because the local milestone has no async runtime. Every implementation
//! keeps the same contract, and one test file (`tests/contract.rs`) runs every case against each of them:
//!
//! - [`MemoryStore`] holds everything in memory, for every test above the store.
//! - [`FileStore`] keeps one append-only JSON Lines ledger per run and a content-addressed blob tree under a
//!   store root, which Phase 8 opens at `<project>/`[`STORE_DIR`]. Appends are locked per run and durable before
//!   they return, a torn final line left by a crash is tolerated, and every blob is rehashed on read.
//! - The AWS milestone's table and object stores implement the same traits.
//!
//! Run state is never stored. It is rebuilt by folding the records [`Ledger::read`] returns
//! (`minos_core::state::fold`), so the same lines always give the same state.
//!
//! # The contract
//!
//! - [`Ledger::read`] returns a run's lines in `seq` order, starting at 0 with no gap, and every line passes
//!   [`Line::verify`] for its run.
//! - An append whose [`EventKey`](minos_core::ledger::EventKey) already exists in the run writes nothing
//!   (STORE-03): [`Appended::Duplicate`] when the stored record is equal, and the error
//!   [`StoreError::Conflict`] when it is not (07-CONTEXT D-08). The first write wins, and a caller cannot carry
//!   on past a conflict with `?` as if it were a success.
//! - An append is durable before it returns. For the memory store that is a no-op.
//! - A blob is addressed by the plain SHA-256 of its bytes ([`BlobId::of`]) and is rehashed on every read.
//!   [`Blobs::put_reader`] streams a blob in under a cap and names it exactly as [`Blobs::put`] would.
//! - [`RunLock::lock_run`] hands out at most one guard per run at a time (LOOP-06); a second request while it
//!   lives is [`StoreError::RunLocked`], and the run is free again once the guard drops. The run lock is not the
//!   append lock: appends and reads on a locked run still succeed, so `minos stop` works while a driver runs.
//!
//! # Errors
//!
//! [`StoreError`] separates infrastructure faults, which are retried ([`StoreError::is_retryable`]), from a
//! corrupt ledger, which is refused and needs a human, and from blob faults. No variant is an
//! `Outcome`: a store fault is never blamed on a candidate. No variant carries file bytes; each names IDs,
//! numbers, and fixed text only.

pub mod file;
pub mod memory;

use minos_core::id::{BlobId, RunId};
use minos_core::ledger::{Line, Record};

pub use file::{FileRunGuard, FileStore, STORE_DIR};
pub use memory::{MemoryRunGuard, MemoryStore};

/// What a successful [`Ledger::append`] did. Every variant carries the `seq` of the line that holds the key. A
/// same-key append with a different record is not a success: it is [`StoreError::Conflict`] (07-CONTEXT D-08).
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appended {
    /// The record was written as a new line at `seq`.
    New {
        /// The new line's position.
        seq: u64,
    },
    /// A line with the same key and an equal record already sits at `seq`, so nothing was written. This is the
    /// STORE-03 no-op that makes a resumed step's re-append harmless.
    Duplicate {
        /// The existing line's position.
        seq: u64,
    },
}

impl Appended {
    /// The `seq` of the line that holds the key: the new line, or the existing one.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        match *self {
            Self::New { seq } | Self::Duplicate { seq } => seq,
        }
    }
}

/// A run's ledger as [`Ledger::read`] found it: its complete, verified lines in `seq` order, and how many
/// incomplete tail lines were dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRead {
    lines: Vec<Line>,
    torn_tail: u64,
}

impl LedgerRead {
    /// A read of `lines`, which the store has verified, with `torn_tail` incomplete tail lines dropped.
    pub(crate) const fn new(lines: Vec<Line>, torn_tail: u64) -> Self {
        Self { lines, torn_tail }
    }

    /// The lines, in `seq` order from 0.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The lines, by value.
    #[must_use]
    pub fn into_lines(self) -> Vec<Line> {
        self.lines
    }

    /// The records of the lines, in `seq` order: the input of the fold.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.lines.iter().map(Line::record)
    }

    /// The number of incomplete tail lines dropped, 0 or 1. Never their bytes (04-CONTEXT D-13). Always 0 for
    /// the memory store.
    #[must_use]
    pub const fn torn_tail(&self) -> u64 {
        self.torn_tail
    }
}

/// Why a ledger was refused as corrupt. Each kind needs a human; none is retried.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CorruptKind {
    /// A record's `v` tag names a version this build does not read.
    #[error("a record has a version this build of minos does not read; upgrade minos")]
    UnknownVersion,
    /// A complete line is not a ledger line.
    #[error("a complete line is not a ledger line")]
    BadJson,
    /// [`Line::verify`] failed: the line was edited, or it belongs to another run.
    #[error("a line's key does not match its record in this run")]
    KeyMismatch,
    /// A line's `seq` is not its position in the ledger.
    #[error("expected seq {expected}, found {found}")]
    SeqGap {
        /// The line's position, which its `seq` must equal.
        expected: u64,
        /// The `seq` the line holds.
        found: u64,
    },
    /// A line is longer than the store's line cap; it was not parsed.
    #[error("a line is {bytes} bytes, over the line cap")]
    LineTooLong {
        /// The line's length in bytes.
        bytes: u64,
    },
}

/// Why a store operation failed. No variant is an `Outcome`, and none carries file bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// An I/O operation failed. Infrastructure: retried.
    #[error("store i/o failed while {op}")]
    Io {
        /// What the store was doing, as fixed text.
        op: &'static str,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A run's ledger is corrupt and is refused; it needs a human (04-CONTEXT D-13).
    #[error("the ledger of run {run} is corrupt at line {line}: {kind}")]
    Corrupt {
        /// The run.
        run: RunId,
        /// The 1-based line number.
        line: u64,
        /// What is wrong with the line.
        kind: CorruptKind,
    },
    /// A run's ledger is larger than the store reads; it was not read.
    #[error("the ledger of run {run} is {bytes} bytes, over the {max}-byte cap")]
    LedgerTooLarge {
        /// The run.
        run: RunId,
        /// The ledger's size in bytes.
        bytes: u64,
        /// The cap.
        max: u64,
    },
    /// Serializing a ledger line failed.
    #[error("encoding a ledger line failed")]
    Encode(#[source] serde_json::Error),
    /// A record serializes to a line longer than the store's line cap, so it was not written.
    #[error("a ledger line would be {bytes} bytes, over the {max}-byte line cap")]
    LineTooLong {
        /// The serialized line's length in bytes, without its newline.
        bytes: u64,
        /// The cap.
        max: u64,
    },
    /// No blob has this ID.
    #[error("blob {id} is not in the store")]
    BlobMissing {
        /// The blob asked for.
        id: BlobId,
    },
    /// The stored blob is larger than the caller's cap. Checked before any byte is copied.
    #[error("blob {id} is {size} bytes, over the {max}-byte cap")]
    BlobTooLarge {
        /// The blob asked for.
        id: BlobId,
        /// The stored size in bytes.
        size: u64,
        /// The caller's cap.
        max: u64,
    },
    /// A streamed put received more than its cap, so nothing was stored. Not retried: the same stream is the same
    /// size again.
    #[error("the streamed blob is over its {max}-byte cap")]
    PutTooLarge {
        /// The caller's cap.
        max: u64,
    },
    /// The stored bytes do not hash to their ID.
    #[error("blob {id} does not hash to its id")]
    BlobTampered {
        /// The blob asked for.
        id: BlobId,
    },
    /// A store path is a symlink or is not the expected file type.
    #[error("the store's {what} is a symlink or not the expected file type")]
    Unsafe {
        /// Which store path, as fixed text.
        what: &'static str,
    },
    /// An in-memory lock was poisoned by a panic in another thread. The store fails closed.
    #[error("an in-memory store lock was poisoned by a panic")]
    Poisoned,
    /// A line with the same key and a different record already sits at `seq`, so nothing was written and the
    /// first write wins (07-CONTEXT D-08). The caller has a bug, such as a re-run trial that produced different
    /// metrics (04-RESEARCH Pitfall 4). Never retried, because the same append conflicts again, and never a
    /// candidate's fault: the ledger needs a human.
    #[error(
        "a different record already holds this key at seq {seq}; the first write wins and the ledger needs a human"
    )]
    Conflict {
        /// The existing line's position.
        seq: u64,
    },
    /// Another driver holds the run's lock (LOOP-06). Never retried: the holder runs until it finishes or is
    /// stopped, and the caller tells the user rather than waiting.
    #[error(
        "run {run} is being driven by another minos process; wait for it to finish or stop it with minos stop"
    )]
    RunLocked {
        /// The run.
        run: RunId,
    },
}

impl StoreError {
    /// Whether the operation may be retried: true only for [`StoreError::Io`], which is infrastructure.
    /// Corruption needs a human, and the blob faults do not change on a retry.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Io { .. })
    }
}

/// An append-only event ledger per run.
pub trait Ledger {
    /// Appends `record` to `run` unless a line with the same key exists, and says which happened. A same-key
    /// append writes nothing: [`Appended::Duplicate`] for an equal record, [`StoreError::Conflict`] for a
    /// different one. A [`Appended::New`] line is durable before this returns.
    ///
    /// # Errors
    ///
    /// [`StoreError::Conflict`] when a line with the same key holds a different record (nothing is written; never
    /// retried), [`StoreError::Io`] when the ledger cannot be read or written, [`StoreError::Corrupt`] or
    /// [`StoreError::LedgerTooLarge`] when the existing ledger is refused, [`StoreError::Encode`] when the line
    /// cannot be serialized, [`StoreError::LineTooLong`] when it is over the store's line cap,
    /// [`StoreError::Unsafe`] when a store path is not the expected file, and
    /// [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError>;

    /// Every complete, verified line of `run` in `seq` order from 0, and the number of torn tail lines dropped.
    /// A run that was never appended to reads as zero lines and a torn tail of 0.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when the ledger cannot be read, [`StoreError::Corrupt`] or
    /// [`StoreError::LedgerTooLarge`] when it is refused, [`StoreError::Unsafe`] when a store path is not the
    /// expected file, and [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError>;
}

/// A whole-run exclusive lock (LOOP-06): at most one driver acts on a run at a time.
///
/// The lock is synchronous and takes `&self`. [`RunLock::lock_run`] returns a guard that holds the run until it
/// drops (RAII), so a driver that returns, fails, or panics releases it. The driver takes the guard before its
/// first ledger read of a session, before appending `RunStarted` for a new run, and holds it for the whole
/// session. It is separate from the per-append lock: [`Ledger::append`] and [`Ledger::read`] on a locked run still
/// succeed, from any handle, so `minos stop` and `minos steer` can record while a driver runs (07-RESEARCH
/// Pitfall 11).
///
/// The guard's type is generic over the borrow of the store, so the memory guard can point back into it; it is
/// used as a bound (`S: RunLock`), never as a trait object.
pub trait RunLock {
    /// Holds the run locked until it drops.
    type Guard<'a>: Sized
    where
        Self: 'a;

    /// Locks `run` for the caller, or refuses at once when anyone else holds it. It never waits.
    ///
    /// # Errors
    ///
    /// [`StoreError::RunLocked`] when another guard holds `run` (never retried), [`StoreError::Io`] when the lock
    /// file cannot be created, opened, or locked, [`StoreError::Unsafe`] when a store path is not the expected
    /// file, and [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn lock_run(&self, run: &RunId) -> Result<Self::Guard<'_>, StoreError>;
}

/// A content-addressed blob store. A blob goes in whole ([`Blobs::put`]) or streamed from a reader
/// ([`Blobs::put_reader`], for root images of hundreds of MB; 05-CONTEXT D-09), and either way it is named by
/// [`BlobId::of`] its bytes.
///
/// The trait has a generic method, so it is used as a bound (`S: Blobs`), never as a trait object.
pub trait Blobs {
    /// Stores `bytes` and returns [`BlobId::of`] them. Storing the same bytes again returns the same ID and
    /// changes nothing.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when the blob cannot be written, [`StoreError::Unsafe`] when a store path is not the
    /// expected file, and [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError>;

    /// Streams `reader` into the store, hashing with [`minos_core::id::BlobHasher`] as it writes (05-CONTEXT
    /// D-09), and returns the ID. The ID equals [`BlobId::of`] the bytes read, and so equals what [`Blobs::put`]
    /// of the same bytes returns. At most `max` bytes are accepted: once more arrive the put is refused and
    /// nothing is stored. A reader of exactly `max` bytes is stored.
    ///
    /// # Errors
    ///
    /// [`StoreError::PutTooLarge`] when the reader yields more than `max` bytes, [`StoreError::Io`] when the
    /// reader fails or the blob cannot be written (retryable; nothing is stored), [`StoreError::Unsafe`] when a
    /// store path is not the expected file, and [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn put_reader<R: std::io::Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError>;

    /// The bytes of blob `id`, refused before any byte is copied when the stored size is over `max`, and
    /// rehashed before they are returned.
    ///
    /// # Errors
    ///
    /// [`StoreError::BlobMissing`] when no blob has the ID, [`StoreError::BlobTooLarge`] when the stored size is
    /// over `max`, [`StoreError::BlobTampered`] when the bytes do not hash to `id`, [`StoreError::Io`] when the
    /// blob cannot be read, [`StoreError::Unsafe`] when a store path is not the expected file, and
    /// [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, StoreError>;

    /// Whether a blob with this ID is stored. The bytes are not read or rehashed.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when the store cannot be inspected, [`StoreError::Unsafe`] when a store path is not
    /// the expected file, and [`StoreError::Poisoned`] when an in-memory lock was poisoned.
    fn contains(&self, id: &BlobId) -> Result<bool, StoreError>;
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use minos_core::id::{BlobId, ManifestId, RunId, TreeId};

    use super::{Appended, CorruptKind, StoreError};

    type TestResult = Result<(), Box<dyn Error>>;

    fn run() -> RunId {
        RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
    }

    fn json_error() -> Result<serde_json::Error, Box<dyn Error>> {
        Ok(serde_json::from_str::<u8>("x")
            .err()
            .ok_or("parsing `x` as a u8 unexpectedly succeeded")?)
    }

    #[test]
    fn appended_seq_returns_the_carried_seq() {
        assert_eq!(Appended::New { seq: 0 }.seq(), 0);
        assert_eq!(Appended::Duplicate { seq: 7 }.seq(), 7);
        assert_eq!(Appended::Duplicate { seq: u64::MAX }.seq(), u64::MAX);
    }

    #[test]
    fn only_io_is_retryable() -> TestResult {
        let id = BlobId::of(b"blob");
        let io_error = StoreError::Io {
            op: "reading the ledger",
            source: io::Error::from(io::ErrorKind::Interrupted),
        };
        assert!(io_error.is_retryable());
        let others = [
            StoreError::Corrupt {
                run: run(),
                line: 1,
                kind: CorruptKind::BadJson,
            },
            StoreError::LedgerTooLarge {
                run: run(),
                bytes: 2,
                max: 1,
            },
            StoreError::Encode(json_error()?),
            StoreError::LineTooLong { bytes: 2, max: 1 },
            StoreError::BlobMissing { id },
            StoreError::BlobTooLarge {
                id,
                size: 2,
                max: 1,
            },
            StoreError::PutTooLarge { max: 1 },
            StoreError::BlobTampered { id },
            StoreError::Unsafe { what: "blob file" },
            StoreError::Poisoned,
            StoreError::Conflict { seq: 0 },
            StoreError::RunLocked { run: run() },
        ];
        for error in &others {
            assert!(!error.is_retryable(), "{error} must not be retryable");
        }
        Ok(())
    }

    #[test]
    fn corrupt_display_names_the_run_line_and_kind() {
        let error = StoreError::Corrupt {
            run: run(),
            line: 3,
            kind: CorruptKind::SeqGap {
                expected: 2,
                found: 5,
            },
        };
        let text = error.to_string();
        assert!(text.contains(&run().to_hex()), "{text}");
        assert!(text.contains("line 3"), "{text}");
        assert!(text.contains("expected seq 2, found 5"), "{text}");
    }

    #[test]
    fn blob_too_large_display_names_the_id_size_and_max() {
        let id = BlobId::of(b"blob");
        let text = StoreError::BlobTooLarge {
            id,
            size: 4096,
            max: 1024,
        }
        .to_string();
        assert!(text.contains(&id.to_hex()), "{text}");
        assert!(text.contains("4096"), "{text}");
        assert!(text.contains("1024"), "{text}");
    }

    #[test]
    fn put_too_large_display_names_the_cap() {
        let text = StoreError::PutTooLarge { max: 1024 }.to_string();
        assert_eq!(text, "the streamed blob is over its 1024-byte cap");
    }

    #[test]
    fn conflict_display_names_the_seq() {
        let text = StoreError::Conflict { seq: 4 }.to_string();
        assert!(text.contains("seq 4"), "{text}");
        assert!(text.contains("first write wins"), "{text}");
    }

    #[test]
    fn run_locked_display_names_the_run_and_the_way_out() {
        let text = StoreError::RunLocked { run: run() }.to_string();
        assert!(text.contains(&run().to_hex()), "{text}");
        assert!(text.contains("minos stop"), "{text}");
    }

    #[test]
    fn unknown_version_display_says_to_upgrade() {
        let text = CorruptKind::UnknownVersion.to_string();
        assert!(text.contains("upgrade minos"), "{text}");
    }
}
