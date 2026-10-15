//! The hostile output tar reader (SBX-05, D-16).
//!
//! A sandbox returns its output as a tar stream, reassembled by the backend from vsock chunks. Every byte of it is
//! attacker-controlled: names, types, sizes, headers, padding, and trailing data. The crate-private `read_output` parses it in
//! memory under caps and returns the files inside an [`Untrusted`]. It writes nothing to disk: there is no
//! `unpack`, and this crate's `clippy.toml` bans the tar unpack methods and the `File` and `OpenOptions` types.
//!
//! # Steps
//!
//! 1. The stream is refused over [`MAX_OUTPUT_STREAM_BYTES`], then if its length is not a whole number of 512-byte
//!    blocks, before any parsing.
//! 2. Entries are iterated in raw mode. Per entry, before reading any of its data: the entry count against
//!    [`MAX_OUTPUT_ENTRIES`]; the header must be POSIX ustar (magic `ustar\0`, version `00`); the typeflag must be
//!    exactly `b'0'`, a regular file; the size against [`MAX_OUTPUT_ENTRY_BYTES`]; the name must be valid UTF-8
//!    and a [`RelPath`] (the D-01 grammar, so `..`, absolute, empty, backslash, NUL, and non-ASCII names are
//!    refused); the running total against [`MAX_OUTPUT_CONTENT_BYTES`]; and the name must not repeat. Then the
//!    data is read through `take(size)`, and fewer than `size` bytes is a truncation.
//! 3. After the last entry, the rest of the stream must be at least two blocks long and all zero, so a truncated
//!    archive and data hidden after the end marker are both refused.
//!
//! # Why raw mode
//!
//! Our guest writes plain ustar, so refusing every extension type (GNU long names and long links, PAX records,
//! sparse files) is simpler and stronger than processing them. In raw mode the tar crate returns every header as
//! an entry and applies no extension, so the PAX size-desync class of parser differential cannot occur: the only
//! size we ever use is the ustar header's own, and an extension header is refused by its typeflag.
//!
//! # Allocation bound
//!
//! The stream is a slice already refused above [`MAX_OUTPUT_STREAM_BYTES`] before parsing. Raw mode never buffers
//! an extension record. Each entry's data is read through `take(size)` only after the per-entry cap, into a buffer
//! preallocated with at most 64 KiB, so the files held at once never exceed [`MAX_OUTPUT_CONTENT_BYTES`]. No
//! allocation counter can measure this directly, because a global allocator needs `unsafe` code, which the
//! workspace forbids (04-RESEARCH Pitfall 9); the bound is structural, and the property tests check its outcome.
//!
//! Every fault is a candidate fault (an `Outcome`, never retried): the input is already in memory, so nothing here
//! can be an infrastructure failure.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Cursor, ErrorKind, Read};

use minos_core::outcome::Reason;
use minos_core::path::RelPath;

use crate::role::Role;
use crate::untrusted::{GuestOutput, Untrusted};

/// Most content bytes, summed over every file of one output: 16 MiB. Logs are capped by the guest, a scorecard is
/// 4 KiB, and a diff 1 MiB, so this leaves room for artifacts. It is 4096 vsock chunks of 4 KiB, whose throughput
/// is unmeasured (02-SPIKE-EVIDENCE.md S03-ANSWER).
pub const MAX_OUTPUT_CONTENT_BYTES: u64 = 16 * 1024 * 1024;

/// Most bytes in one file: 8 MiB, so one artifact cannot take the whole budget.
pub const MAX_OUTPUT_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

/// Most files in one output: bounds the per-entry work and the size of the map.
pub const MAX_OUTPUT_ENTRIES: usize = 1024;

/// Most bytes in one output stream: the content cap, plus one header block and up to 511 padding bytes per entry,
/// plus the two end blocks. A stream any longer cannot be a valid output.
pub const MAX_OUTPUT_STREAM_BYTES: u64 =
    MAX_OUTPUT_CONTENT_BYTES + (MAX_OUTPUT_ENTRIES as u64 + 2) * 1024;

/// Most diff texts one output may carry: one proposal batch (`limits::BATCH_MAX`).
pub const MAX_DIFF_TEXTS: usize = minos_core::limits::BATCH_MAX as usize;

/// Bytes in one tar block.
const BLOCK: u64 = 512;

/// Bytes in the end-of-archive marker: two zero blocks.
const END_MARKER: usize = 1024;

/// Most bytes preallocated for one entry's data, whatever its header claims.
const PREALLOC_CAP: u64 = 64 * 1024;

/// Which cap an output exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputCap {
    /// The whole stream, [`MAX_OUTPUT_STREAM_BYTES`].
    Stream,
    /// The entry count, [`MAX_OUTPUT_ENTRIES`].
    Entries,
    /// One entry's size, [`MAX_OUTPUT_ENTRY_BYTES`].
    Entry,
    /// The content total, [`MAX_OUTPUT_CONTENT_BYTES`].
    Total,
    /// The number of diff texts, [`MAX_DIFF_TEXTS`].
    DiffCount,
    /// One diff text's size, `Limits::max_diff_bytes`.
    DiffBytes,
}

impl fmt::Display for OutputCap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stream => "stream size",
            Self::Entries => "entry count",
            Self::Entry => "entry size",
            Self::Total => "content total",
            Self::DiffCount => "diff count",
            Self::DiffBytes => "diff size",
        })
    }
}

/// Which rule an output broke.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputInvalid {
    /// The stream is not a whole number of 512-byte blocks.
    NotBlockAligned,
    /// A header is not POSIX ustar (a GNU or v7 header, or no magic).
    NotUstar,
    /// An entry is not a regular file with typeflag `b'0'`: a link, device, FIFO, directory, legacy regular file,
    /// or extension header.
    EntryType,
    /// An entry's name is not valid UTF-8 or not a relative path in the D-01 grammar.
    Path,
    /// An entry's data is shorter than its header's size.
    Truncated,
    /// Two entries have the same name.
    Duplicate,
    /// The stream ends without two zero blocks after the last entry.
    NoEndMarker,
    /// Non-zero bytes follow the end marker.
    TrailingData,
    /// The tar crate refused a header: a bad checksum or a corrupt numeric field.
    Malformed,
    /// A diff text is not valid UTF-8, or contains a NUL or a carriage return.
    DiffNotText,
}

impl fmt::Display for OutputInvalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotBlockAligned => "the stream is not block-aligned",
            Self::NotUstar => "a header is not ustar",
            Self::EntryType => "an entry is not a regular file",
            Self::Path => "an entry name is not a valid relative path",
            Self::Truncated => "an entry's data is truncated",
            Self::Duplicate => "an entry name repeats",
            Self::NoEndMarker => "the archive has no end marker",
            Self::TrailingData => "data follows the end marker",
            Self::Malformed => "a header is malformed",
            Self::DiffNotText => "a diff is not text",
        })
    }
}

/// Why sandbox output was refused. A candidate fault: it becomes an `Outcome` and is never retried.
///
/// The messages name the cap or the rule, never input bytes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OutputFault {
    /// The output exceeded a cap.
    #[error("the output is over its {cap} cap")]
    TooLarge {
        /// The cap exceeded.
        cap: OutputCap,
    },
    /// The output broke a rule.
    #[error("the output is invalid: {why}")]
    Invalid {
        /// The rule broken.
        why: OutputInvalid,
    },
}

impl OutputFault {
    /// The ledger reason: [`Reason::OutputTooLarge`] for a cap, [`Reason::OutputInvalid`] for a rule.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        match self {
            Self::TooLarge { .. } => Reason::OutputTooLarge,
            Self::Invalid { .. } => Reason::OutputInvalid,
        }
    }
}

/// Shorthand for a cap fault.
const fn too_large(cap: OutputCap) -> OutputFault {
    OutputFault::TooLarge { cap }
}

/// Shorthand for a rule fault.
const fn invalid(why: OutputInvalid) -> OutputFault {
    OutputFault::Invalid { why }
}

/// Maps an error from the tar crate. The input is in memory, so no error is infrastructure: an unexpected end is
/// a truncation and anything else is a malformed header.
fn tar_fault(error: &std::io::Error) -> OutputFault {
    if error.kind() == ErrorKind::UnexpectedEof {
        invalid(OutputInvalid::Truncated)
    } else {
        invalid(OutputInvalid::Malformed)
    }
}

/// Reads a role-`R` sandbox's output stream in memory, under caps, in the order the [module docs](self) give.
///
/// Crate-private: only `Finished::into_output`, which requires a `Cleared<R>`, calls it, so no code outside this
/// crate can label arbitrary bytes as a role's output.
pub(crate) fn read_output<R: Role>(
    stream: &[u8],
) -> Result<Untrusted<GuestOutput<R>>, OutputFault> {
    let len = u64::try_from(stream.len()).unwrap_or(u64::MAX);
    if len > MAX_OUTPUT_STREAM_BYTES {
        return Err(too_large(OutputCap::Stream));
    }
    if len % BLOCK != 0 {
        return Err(invalid(OutputInvalid::NotBlockAligned));
    }

    let mut files: BTreeMap<RelPath, Vec<u8>> = BTreeMap::new();
    let mut total: u64 = 0;
    let mut end: u64 = 0;
    let mut archive = tar::Archive::new(Cursor::new(stream));
    let entries = archive
        .entries()
        .map_err(|error| tar_fault(&error))?
        .raw(true);
    for entry in entries {
        let mut entry = entry.map_err(|error| tar_fault(&error))?;
        if files.len() >= MAX_OUTPUT_ENTRIES {
            return Err(too_large(OutputCap::Entries));
        }
        let header = entry.header();
        if header.as_ustar().is_none() {
            return Err(invalid(OutputInvalid::NotUstar));
        }
        if !matches!(header.as_old().linkflag, [b'0']) {
            return Err(invalid(OutputInvalid::EntryType));
        }
        let size = entry.size();
        if size > MAX_OUTPUT_ENTRY_BYTES {
            return Err(too_large(OutputCap::Entry));
        }
        let path = std::str::from_utf8(&header.path_bytes())
            .ok()
            .and_then(|name| RelPath::try_from(name).ok())
            .ok_or(invalid(OutputInvalid::Path))?;
        total = total
            .checked_add(size)
            .filter(|sum| *sum <= MAX_OUTPUT_CONTENT_BYTES)
            .ok_or(too_large(OutputCap::Total))?;
        if files.contains_key(&path) {
            return Err(invalid(OutputInvalid::Duplicate));
        }

        let capacity = usize::try_from(size.min(PREALLOC_CAP)).unwrap_or(0);
        let mut data = Vec::with_capacity(capacity);
        (&mut entry)
            .take(size)
            .read_to_end(&mut data)
            .map_err(|error| tar_fault(&error))?;
        if u64::try_from(data.len()).unwrap_or(u64::MAX) != size {
            return Err(invalid(OutputInvalid::Truncated));
        }
        end = size
            .checked_next_multiple_of(BLOCK)
            .and_then(|padded| entry.raw_file_position().checked_add(padded))
            .ok_or(invalid(OutputInvalid::Malformed))?;
        files.insert(path, data);
    }

    let rest = usize::try_from(end)
        .ok()
        .and_then(|start| stream.get(start..))
        .unwrap_or_default();
    if rest.len() < END_MARKER {
        return Err(invalid(OutputInvalid::NoEndMarker));
    }
    if rest.iter().any(|byte| *byte != 0) {
        return Err(invalid(OutputInvalid::TrailingData));
    }
    Ok(Untrusted::new(GuestOutput::new(files)))
}
