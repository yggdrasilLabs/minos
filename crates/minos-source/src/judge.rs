//! The judge-data pack: a judge directory packed with the source snapshot's hygiene (07-CONTEXT D-03, JUDGE-01).
//!
//! Judge data sits unencrypted in `.minos/blobs` and inside every judge VM, so [`pack_judge_data`] fails closed
//! exactly as [`crate::snapshot`] does. Shared with the snapshot, with no override:
//!
//! - The walker, with the `.gitignore` and `.minosignore` files inside the judge directory and nothing outside it,
//!   and the refusal of a `.minosignore` negation.
//! - The hard denylist, every drop listed in the [`PackReport`].
//! - The offender rules: a symlink, hardlink, FIFO, socket, device, a name outside the [`RelPath`] grammar, a path
//!   that does not fit ustar, an oversize file, and a file that changes while it is read all refuse the pack, every
//!   one listed in one error; nothing is skipped or followed.
//! - The secret scan over exactly the bytes packed, binary files included.
//! - The hand-built ustar packer, so the tarball is deterministic and its SHA-256 is the [`TreeId`] that
//!   `JudgeVersion::of` hashes.
//!
//! Different, and why:
//!
//! - No manifest: a judge directory has no `[source] exclude`, no surface, and no program file to require.
//! - No judge-path drop: the snapshot drops every path the judge command names, which here would drop the judge
//!   program from its own data.
//! - No clean check: judge data is private and usually not committed, and D-03 lists the hygiene, not the check.
//! - The caps: [`MAX_JUDGE_FILES`] files, and [`MAX_JUDGE_DATA_BYTES`] (96 MiB) per file, in total, and for the
//!   tarball itself, because the tarball is the judge section of the input disk and PID 1 reads that section whole.
//!   Data over the cap is refused naming its size, never truncated; larger judge data waits for a streaming unpack
//!   from the input disk (CLAUDE.md Open questions).
//!
//! [`MAX_JUDGE_FILES`]: minos_core::limits::MAX_JUDGE_FILES

use std::collections::BTreeMap;
use std::path::Path;

use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::id::{BlobId, TreeId};
use minos_core::path::RelPath;

use crate::walk::{self, WalkRules};
use crate::{PackReport, SnapshotError, pack, scan_files};

/// A packed judge directory: the deterministic tarball, its [`TreeId`], each file's SHA-256, and the pack report.
///
/// The caller stores the tarball as one blob, whose ID equals [`TreeId::to_blob_id`] of [`JudgePack::tree`], and
/// records [`JudgePack::files`] in `minos.lock`, so every later source snapshot can refuse a file equal to judge
/// data (07-CONTEXT D-02, D-04).
#[derive(Clone, Debug)]
pub struct JudgePack {
    tarball: Vec<u8>,
    tree: TreeId,
    files: BTreeMap<RelPath, BlobId>,
    report: PackReport,
}

impl JudgePack {
    /// The tarball bytes.
    #[must_use]
    pub fn tarball(&self) -> &[u8] {
        &self.tarball
    }

    /// The tarball bytes, consuming the pack.
    #[must_use]
    pub fn into_tarball(self) -> Vec<u8> {
        self.tarball
    }

    /// The data tree's ID: the plain SHA-256 of the tarball, the bytes `JudgeVersion::of` hashes.
    #[must_use]
    pub const fn tree(&self) -> TreeId {
        self.tree
    }

    /// Every packed file's path and the SHA-256 of its bytes, in path order.
    #[must_use]
    pub const fn files(&self) -> &BTreeMap<RelPath, BlobId> {
        &self.files
    }

    /// What was packed and what the denylist dropped.
    #[must_use]
    pub const fn report(&self) -> &PackReport {
        &self.report
    }
}

/// Packs the judge directory at `root` into a deterministic, secret-scanned tarball under the judge caps.
///
/// In order: the walk under the ignore files and the hard denylist with the judge caps, which refuses every
/// offender and every over-cap tree; the secret scan over exactly the bytes to be packed; the tarball; and the
/// check that the tarball fits [`MAX_JUDGE_DATA_BYTES`].
///
/// # Errors
///
/// - [`SnapshotError::Walk`], [`SnapshotError::Io`]: the walk or a filesystem operation failed.
/// - [`SnapshotError::Offenders`]: entries that are not packable regular files, all listed, a file over the 96 MiB
///   per-file cap among them.
/// - [`SnapshotError::TooManyFiles`], [`SnapshotError::TooLarge`]: the tree is over the file-count or content cap.
/// - [`SnapshotError::MinosignoreNegation`], [`SnapshotError::IgnoreFileTooLarge`]: a `.minosignore` is refused.
/// - [`SnapshotError::Scanner`], [`SnapshotError::Secrets`]: the scanner failed, or found a secret.
/// - [`SnapshotError::JudgeTooLarge`]: the tarball, headers and padding included, is over the judge cap.
pub fn pack_judge_data(root: &Path) -> Result<JudgePack, SnapshotError> {
    let collected = walk::collect(root, &WalkRules::judge())?;
    scan_files(&collected.files)?;

    let tarball = pack::tarball(&collected.files)?;
    let bytes = u64::try_from(tarball.len()).unwrap_or(u64::MAX);
    if bytes > MAX_JUDGE_DATA_BYTES {
        return Err(SnapshotError::JudgeTooLarge {
            bytes,
            max: MAX_JUDGE_DATA_BYTES,
        });
    }
    let tree = TreeId::of(&tarball);
    let files: BTreeMap<RelPath, BlobId> = collected
        .files
        .iter()
        .map(|(path, file)| (path.clone(), BlobId::of(&file.bytes)))
        .collect();
    Ok(JudgePack {
        tarball,
        tree,
        report: PackReport {
            files: u64::try_from(files.len()).unwrap_or(u64::MAX),
            content_bytes: collected.content_bytes,
            denied: collected.denied,
        },
        files,
    })
}
