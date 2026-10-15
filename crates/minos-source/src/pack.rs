//! The deterministic ustar tarball (CLI-11).
//!
//! Every header is built by hand from a fresh `Header::new_ustar()`: the path (split into ustar prefix and name by
//! `set_path`, which fails if it cannot fit), the size, mode 0644 or 0755 (only the executable bit is kept), uid
//! and gid 0, mtime 0, typeflag Regular, empty user and group names, and the checksum last. Entries follow the
//! files map, which is sorted by path, and `Builder::append` writes each header exactly as given.
//!
//! Not the tar crate's deterministic header mode: it sets mtime to 1153704088, not 0, and normalizes modes
//! differently. Not the builder's path, file, directory, or data conveniences: they copy mtime, owner, and mode
//! from disk, or fall back to a GNU long-name entry for a long path. `crates/minos-source/clippy.toml` bans them.

use std::collections::BTreeMap;

use minos_core::path::RelPath;
use minos_core::text::SafeText;
use tar::{Builder, EntryType, Header};

use crate::walk::FileEntry;
use crate::{Offender, OffenderKind, SnapshotError};

/// Writes `files` in map order as a ustar tarball ending with the two zero blocks, with no record padding.
///
/// # Errors
///
/// [`SnapshotError::Offenders`] if a path no longer fits a ustar header or a size does not fit `u64` (the walk has
/// already refused both), and [`SnapshotError::Io`] if writing to memory fails.
pub(crate) fn tarball(files: &BTreeMap<RelPath, FileEntry>) -> Result<Vec<u8>, SnapshotError> {
    let mut builder = Builder::new(Vec::new());
    for (path, file) in files {
        let header = header(path, file)?;
        builder
            .append(&header, file.bytes.as_slice())
            .map_err(|source| SnapshotError::Io {
                op: "pack",
                path: SafeText::inline(path.as_str()),
                source,
            })?;
    }
    builder.into_inner().map_err(|source| SnapshotError::Io {
        op: "finish",
        path: SafeText::inline("the tarball"),
        source,
    })
}

/// The header of one file, built with exactly these calls in this order.
fn header(path: &RelPath, file: &FileEntry) -> Result<Header, SnapshotError> {
    let refuse = |kind| SnapshotError::Offenders {
        shown: vec![Offender {
            path: SafeText::inline(path.as_str()),
            kind,
        }],
        more: 0,
    };
    let size = u64::try_from(file.bytes.len())
        .map_err(|_| refuse(OffenderKind::Oversize { bytes: u64::MAX }))?;
    let mut header = Header::new_ustar();
    header
        .set_path(path.as_str())
        .map_err(|_| refuse(OffenderKind::UstarPath))?;
    header.set_size(size);
    if file.executable {
        header.set_mode(0o755);
    } else {
        header.set_mode(0o644);
    }
    // Set explicitly: they write seven `0` digits and a NUL, where new_ustar leaves zero bytes.
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    Ok(header)
}
