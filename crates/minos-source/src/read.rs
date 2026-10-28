//! The read-back of the tarballs [`snapshot`](crate::snapshot) writes (08-RESEARCH G3).
//!
//! The CLI uses it twice: at `check` and `run`, through [`Snapshot::surface`](crate::Snapshot::surface), to build
//! the run's base surface and program bytes from the bytes that were packed, never from the working tree; and at
//! `report`, `show`, and `adopt`, through [`read_files`] on the stored base blob, so a report needs no working tree.
//!
//! The reader is strict even though Minos wrote the tarball, because a stored blob is still bytes from disk. It
//! works in memory and never extracts anything: the stream must be a whole number of 512-byte blocks no longer than
//! [`MAX_BASE_TAR_BYTES`]; every header must be POSIX ustar with typeflag `0` (a regular file; links, directories,
//! devices, and extension headers are refused); every name must be UTF-8 in the [`RelPath`] grammar; no path may
//! repeat; the snapshot caps bound the entry count, each entry (checked from the header before any data is read),
//! and the content total; and the archive must end with two zero blocks and nothing else. Errors name paths
//! through [`RelPath`] or give fixed text, never file bytes.

use std::collections::BTreeSet;
use std::io::{Cursor, ErrorKind, Read};

use minos_core::diff::apply::Tree;
use minos_core::guest::MAX_BASE_TAR_BYTES;
use minos_core::path::RelPath;

use crate::listing;
use crate::walk::{MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_FILE_BYTES, MAX_SNAPSHOT_FILES};

/// Bytes in one tar block.
const BLOCK: u64 = 512;

/// Bytes in the end-of-archive marker: two zero blocks.
const END_MARKER: usize = 1024;

/// Most bytes preallocated for one entry's data, whatever its header claims.
const PREALLOC_CAP: u64 = 64 * 1024;

/// Why a snapshot tarball could not be read back. No variant carries file bytes.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    /// Requested paths the tarball does not hold.
    #[error("the tarball is missing requested paths:{}", listing(.paths, 0))]
    Missing {
        /// Every requested path not found, sorted.
        paths: Vec<RelPath>,
    },
    /// The bytes are not a canonical snapshot tarball.
    #[error("the tarball is malformed: {what}")]
    Malformed {
        /// The rule broken, as fixed text.
        what: &'static str,
    },
}

/// Shorthand for a [`ReadError::Malformed`].
const fn malformed(what: &'static str) -> ReadError {
    ReadError::Malformed { what }
}

/// Maps an error from the tar crate. The input is in memory, so an unexpected end is a truncation and anything
/// else is a header the crate refused.
fn tar_error(error: &std::io::Error) -> ReadError {
    if error.kind() == ErrorKind::UnexpectedEof {
        malformed("an entry's data is truncated")
    } else {
        malformed("not a ustar archive")
    }
}

/// Reads the files at `paths` from `tarball`, a snapshot tarball, in memory.
///
/// Nothing is extracted. The stream must be a whole number of 512-byte blocks no longer than
/// [`MAX_BASE_TAR_BYTES`]; every header must be POSIX ustar with typeflag `0`, a regular file, so links,
/// directories, devices, and extension headers are refused; every name must be UTF-8 in the [`RelPath`] grammar;
/// no path may repeat; the snapshot caps bound the entry count (10,000), each entry (16 MiB, checked from the
/// header before any data is read), and the content total (64 MiB); and the archive must end with two zero blocks
/// and nothing else. Every entry is checked, requested or not, so a malformed archive is refused whatever is asked
/// of it; only the requested entries' bytes are kept, and the result holds exactly the requested paths.
///
/// # Errors
///
/// - [`ReadError::Malformed`]: the stream breaks a rule or a cap, with fixed text naming which.
/// - [`ReadError::Missing`]: the archive is well formed but lacks requested paths, all of them listed in order.
pub fn read_files(tarball: &[u8], paths: &BTreeSet<RelPath>) -> Result<Tree, ReadError> {
    let len = u64::try_from(tarball.len()).unwrap_or(u64::MAX);
    if len > MAX_BASE_TAR_BYTES {
        return Err(malformed("the archive is over its cap"));
    }
    if len % BLOCK != 0 {
        return Err(malformed("not a ustar archive"));
    }

    let mut seen: BTreeSet<RelPath> = BTreeSet::new();
    let mut tree = Tree::new();
    let mut entries_read: u64 = 0;
    let mut total: u64 = 0;
    let mut end: u64 = 0;
    let mut archive = tar::Archive::new(Cursor::new(tarball));
    let entries = archive
        .entries()
        .map_err(|error| tar_error(&error))?
        .raw(true);
    for entry in entries {
        let mut entry = entry.map_err(|error| tar_error(&error))?;
        if entries_read >= MAX_SNAPSHOT_FILES {
            return Err(malformed("too many entries"));
        }
        entries_read = entries_read.saturating_add(1);
        let header = entry.header();
        if header.as_ustar().is_none() {
            return Err(malformed("not a ustar archive"));
        }
        if !matches!(header.as_old().linkflag, [b'0']) {
            return Err(malformed("an entry is not a regular file"));
        }
        let size = entry.size();
        if size > MAX_SNAPSHOT_FILE_BYTES {
            return Err(malformed("an entry is over its cap"));
        }
        let path = std::str::from_utf8(&header.path_bytes())
            .ok()
            .and_then(|name| RelPath::try_from(name).ok())
            .ok_or(malformed("an entry name is not a project path"))?;
        total = total
            .checked_add(size)
            .filter(|sum| *sum <= MAX_SNAPSHOT_BYTES)
            .ok_or(malformed("the archive is over its cap"))?;
        if !seen.insert(path.clone()) {
            return Err(malformed("a path appears twice"));
        }

        if paths.contains(&path) {
            let capacity = usize::try_from(size.min(PREALLOC_CAP)).unwrap_or(0);
            let mut data = Vec::with_capacity(capacity);
            (&mut entry)
                .take(size.saturating_add(1))
                .read_to_end(&mut data)
                .map_err(|error| tar_error(&error))?;
            if u64::try_from(data.len()).unwrap_or(u64::MAX) != size {
                return Err(malformed("an entry's data is truncated"));
            }
            tree.insert(path, data);
        }
        end = size
            .checked_next_multiple_of(BLOCK)
            .and_then(|padded| entry.raw_file_position().checked_add(padded))
            .ok_or(malformed("not a ustar archive"))?;
    }

    // An entry's data that runs past the stream leaves no room for the end marker, so truncation lands here too.
    let rest = usize::try_from(end)
        .ok()
        .and_then(|start| tarball.get(start..))
        .unwrap_or_default();
    if rest.len() < END_MARKER {
        return Err(malformed("the archive has no end marker"));
    }
    if rest.iter().any(|byte| *byte != 0) {
        return Err(malformed("data follows the end marker"));
    }

    let missing: Vec<RelPath> = paths
        .iter()
        .filter(|path| !tree.contains(path))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(ReadError::Missing { paths: missing });
    }
    Ok(tree)
}
