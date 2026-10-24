//! `minos judge push <dir> [--update]`: a directory of judge data becomes one blob in `.minos/blobs`, recorded in
//! `minos.lock` under the manifest's `[judge] version` label (JUDGE-01, CLI-04; 07-CONTEXT D-01 to D-04).
//!
//! [`judge_push`] runs, in order:
//!
//! 1. the manifest, `<project>/minos.toml`, read under [`MAX_MANIFEST_BYTES`] and validated; its `[judge] version`
//!    is the label, so there is no `--version` flag and no auto-increment (D-01);
//! 2. the directory's placement: when it lies inside the project it must lie under a `[source] exclude` entry, so
//!    the source snapshot and the image build context never pack it (JUDGE-01). The check compares path components
//!    ASCII case-insensitively, so a case-insensitive filesystem cannot hide containment;
//! 3. the pack, `minos_source::pack_judge_data`, with the source snapshot's hygiene and no override: the walker
//!    with the ignore files inside the directory, the hard denylist (every drop is reported), the offender rules,
//!    the secret scan over exactly the packed bytes, and the 96 MiB cap ([`MAX_JUDGE_DATA_BYTES`]), over which the
//!    data is refused and never truncated (D-03);
//! 4. every path the judge command names must be in the pushed tree, because the judge stage runs from the
//!    unpacked data root and a missing program would fail every judge VM (07-RESEARCH A9);
//! 5. the lock decision, made before anything is stored: an equal entry leaves the lock alone, different data under
//!    a recorded label is refused naming both tree digests unless `--update` (D-01), and anything else is written;
//! 6. the tarball stored with `Blobs::put_reader` under [`MAX_JUDGE_DATA_BYTES`]; its blob ID must equal the
//!    tree's, the bytes `JudgeVersion::of` hashes (D-03). An unchanged push stores it too, so a fresh clone whose
//!    committed lock already names the data gets the blob back;
//! 7. `minos.lock`, read again and decided again, then written through the shared synced temporary file in
//!    `.minos/` ([`write_lock`]): blob first, lock last.
//!
//! The data itself stays in `.minos/blobs` (04-CONTEXT D-10) and is never written to the lock, which holds only
//! the label, the tree digest, the counts, the pushing `minos` version, and each file's path and SHA-256. Committing
//! the lock therefore exposes the judge file paths, and a low-entropy file's content can be guessed from its digest;
//! it never reveals the data. `.minos/` is not committed, so a fresh clone must push again.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::id::{BlobId, TreeId};
use minos_core::limits::MAX_MANIFEST_BYTES;
use minos_core::lock::{BuilderText, Lock, LockError, LockedJudge};
use minos_core::manifest::{JudgeVersion, Manifest, ManifestError};
use minos_core::path::RelPath;
use minos_core::text::SafeText;
use minos_source::{Denied, SnapshotError, pack_judge_data};
use minos_store::{Blobs, FileStore, STORE_DIR, StoreError};

use crate::env_build::{LOCK_FILE, MANIFEST_FILE};
use crate::lock_file::{LockFileError, read_lock, write_lock};

/// The flags of one `judge push`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JudgePushOptions {
    update: bool,
}

impl JudgePushOptions {
    /// `update`: replace the data already recorded for the manifest's label (`--update`).
    #[must_use]
    pub const fn new(update: bool) -> Self {
        Self { update }
    }
}

/// What one `judge push` recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgePushReport {
    label: u32,
    tree: TreeId,
    file_count: u64,
    bytes: u64,
    judge_version: JudgeVersion,
    lock_written: bool,
    denied: Vec<Denied>,
}

impl JudgePushReport {
    /// The manifest's `[judge] version` label the data was pushed under.
    #[must_use]
    pub const fn label(&self) -> u32 {
        self.label
    }

    /// The judge data tree's digest, equal to its blob ID in `.minos/blobs`.
    #[must_use]
    pub const fn tree(&self) -> &TreeId {
        &self.tree
    }

    /// How many files were pushed.
    #[must_use]
    pub const fn file_count(&self) -> u64 {
        self.file_count
    }

    /// The summed length of the pushed files, headers and padding excluded.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The judge version a run with this data and the manifest's judge spec records: `JudgeVersion::of(tree,
    /// spec)`.
    #[must_use]
    pub const fn judge_version(&self) -> &JudgeVersion {
        &self.judge_version
    }

    /// Whether `minos.lock` was written; `false` when it already recorded exactly this push.
    #[must_use]
    pub const fn lock_written(&self) -> bool {
        self.lock_written
    }

    /// Every entry the hard denylist dropped, sorted by path; each path is a [`SafeText`].
    #[must_use]
    pub fn denied(&self) -> &[Denied] {
        &self.denied
    }
}

/// Why `judge push` stopped. Messages name paths through [`SafeText`] or [`RelPath`] and digests only, and never
/// print file bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum JudgePushError {
    /// A file or directory could not be read.
    #[error("could not read {what}")]
    Read {
        /// Which file or directory.
        what: &'static str,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A file is over its cap.
    #[error("{what} is over {max} bytes")]
    TooLarge {
        /// Which file.
        what: &'static str,
        /// The cap.
        max: u64,
    },
    /// `minos.toml` is not a valid manifest.
    #[error("minos.toml is refused")]
    Manifest(#[source] ManifestError),
    /// The judge data directory cannot be resolved, or is not a directory.
    #[error("could not read the judge data directory")]
    Dir {
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The judge data directory lies inside the project where the source snapshot and the image build context would
    /// pack it (JUDGE-01).
    #[error(
        "the judge data directory {dir} lies inside the project and no [source] exclude entry covers it, so the source snapshot would pack it; move it outside the project or add it to [source] exclude"
    )]
    DirInSnapshot {
        /// The directory, as resolved.
        dir: SafeText,
    },
    /// The judge data directory is the project itself (07-REVIEW IN-06). Judge data is never the project: the
    /// snapshot would pack every file of it, and the judge's own program would sit in the surface.
    #[error(
        "the judge data directory {dir} is the project itself; push a directory holding only the judge data, outside the project or under a [source] exclude entry"
    )]
    DirIsProject {
        /// The directory, as resolved.
        dir: SafeText,
    },
    /// The judge data pack was refused.
    #[error("the judge data is refused")]
    Pack(#[source] SnapshotError),
    /// The pushed tree lacks paths the judge command names (07-RESEARCH A9).
    #[error(
        "the judge data lacks {}, which the judge command names; the judge runs from the data root, so push a directory holding each at that path",
        list_paths(paths)
    )]
    MissingJudgePath {
        /// Every missing path, in path order.
        paths: Vec<RelPath>,
    },
    /// The judge data could not be stored.
    #[error("the judge data could not be stored in .minos/blobs")]
    Store(#[source] StoreError),
    /// The stored blob's ID is not the tree's digest.
    #[error("the stored judge data blob sha256 {stored} is not the packed tree sha256 {tree}")]
    StoredDigest {
        /// The packed tree's digest.
        tree: TreeId,
        /// The blob ID the store returned.
        stored: BlobId,
    },
    /// The running `minos` version cannot be recorded in the lock.
    #[error("the minos version {} cannot be recorded in minos.lock", env!("CARGO_PKG_VERSION"))]
    Version,
    /// The lock entry or the lock text could not be built.
    #[error("minos.lock is refused")]
    Lock(#[source] LockError),
    /// `minos.lock` could not be read or parsed.
    #[error("minos.lock could not be read")]
    LockFile(#[source] LockFileError),
    /// `minos.lock` could not be written.
    #[error("could not write minos.lock")]
    LockWrite(#[source] io::Error),
    /// Different data under a label the lock already records, and `--update` was not passed (D-01). Nothing was
    /// stored and the lock is unchanged.
    #[error(
        "minos.lock records judge data sha256 {locked} for label {label}, but the pushed directory packs to sha256 {pushed}; bump [judge] version in minos.toml for new data, or rerun with --update to replace label {label}"
    )]
    LabelMismatch {
        /// The manifest's `[judge] version` label.
        label: u32,
        /// The tree digest the lock records for the label.
        locked: TreeId,
        /// The tree digest of the pushed directory.
        pushed: TreeId,
    },
}

/// The paths joined with `, ` for a message.
fn list_paths(paths: &[RelPath]) -> String {
    paths
        .iter()
        .map(RelPath::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Pushes the judge data in `dir` for the project at `project`, in the order the [module docs](self) give.
///
/// # Errors
///
/// A [`JudgePushError`] for the first step that fails. Every refusal before step 6 stores nothing, and
/// [`JudgePushError::LabelMismatch`] leaves `minos.lock` unchanged.
pub fn judge_push(
    project: &Path,
    dir: &Path,
    options: JudgePushOptions,
) -> Result<JudgePushReport, JudgePushError> {
    // 1. The manifest; its `[judge] version` is the label.
    let manifest_text = read_text(
        &project.join(MANIFEST_FILE),
        MANIFEST_FILE,
        MAX_MANIFEST_BYTES,
    )?;
    let manifest = Manifest::parse_toml(&manifest_text).map_err(JudgePushError::Manifest)?;
    let label = manifest.judge().label();

    // 2. The directory's placement: never where the source snapshot would pack it.
    let dir = fs::canonicalize(dir).map_err(|source| JudgePushError::Dir { source })?;
    if !fs::metadata(&dir)
        .map_err(|source| JudgePushError::Dir { source })?
        .is_dir()
    {
        return Err(JudgePushError::Dir {
            source: io::Error::from(io::ErrorKind::NotADirectory),
        });
    }
    let project_root = fs::canonicalize(project).map_err(|source| JudgePushError::Read {
        what: "the project directory",
        source,
    })?;
    check_placement(&project_root, &dir, &manifest)?;

    // 3. The pack, with the snapshot's hygiene and the judge caps.
    let pack = pack_judge_data(&dir).map_err(JudgePushError::Pack)?;
    let tree = pack.tree();

    // 4. Every path the judge command names is in the pushed tree.
    let missing: Vec<RelPath> = manifest
        .judge()
        .paths()
        .iter()
        .filter(|path| !pack.files().contains_key(*path))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(JudgePushError::MissingJudgePath { paths: missing });
    }

    // 5. The lock decision, before anything is stored, so a refused push stores nothing.
    let minos = BuilderText::try_from(env!("CARGO_PKG_VERSION").to_owned())
        .map_err(|_| JudgePushError::Version)?;
    let entry = LockedJudge::new(
        label,
        tree,
        pack.report().content_bytes(),
        minos,
        pack.files().clone(),
    )
    .map_err(JudgePushError::Lock)?;
    let lock_path = project.join(LOCK_FILE);
    let lock = read_lock(&lock_path).map_err(JudgePushError::LockFile)?;
    decide(lock.as_ref(), &entry, options.update)?;

    // 6. The tarball into the store; its blob ID is the tree's.
    let store = FileStore::open(&project.join(STORE_DIR)).map_err(JudgePushError::Store)?;
    let stored = store
        .put_reader(pack.tarball(), MAX_JUDGE_DATA_BYTES)
        .map_err(JudgePushError::Store)?;
    if stored != tree.to_blob_id() {
        return Err(JudgePushError::StoredDigest { tree, stored });
    }

    // 7. The lock, only after the blob is stored. It is read and decided again, so an entry written meanwhile (an
    // `env build`) is kept.
    let lock = read_lock(&lock_path).map_err(JudgePushError::LockFile)?;
    let lock_written = if decide(lock.as_ref(), &entry, options.update)? {
        let text = lock
            .unwrap_or_else(Lock::empty)
            .with_judge(entry.clone())
            .to_toml()
            .map_err(JudgePushError::Lock)?;
        write_lock(&project.join(STORE_DIR), &lock_path, text.as_bytes())
            .map_err(JudgePushError::LockWrite)?;
        true
    } else {
        false
    };

    Ok(JudgePushReport {
        label,
        tree,
        file_count: entry.file_count(),
        bytes: entry.bytes(),
        judge_version: JudgeVersion::of(&tree, manifest.judge()),
        lock_written,
        denied: pack.report().denied().to_vec(),
    })
}

/// Whether `entry` must be written to `lock`: `false` when the lock already holds exactly it, `true` for a new
/// label, a changed `minos` version or file list under the same tree, or a new tree with `update`.
///
/// # Errors
///
/// [`JudgePushError::LabelMismatch`] when the lock records a different tree for the label and `update` is off.
fn decide(lock: Option<&Lock>, entry: &LockedJudge, update: bool) -> Result<bool, JudgePushError> {
    match lock.and_then(|lock| lock.judge(entry.label())) {
        Some(locked) if locked == entry => Ok(false),
        Some(locked) if locked.tree() != entry.tree() && !update => {
            Err(JudgePushError::LabelMismatch {
                label: entry.label(),
                locked: *locked.tree(),
                pushed: *entry.tree(),
            })
        }
        _ => Ok(true),
    }
}

/// Refuses a judge directory `dir` that lies inside `project` (both canonical) unless a `[source] exclude` entry
/// covers its project-relative path. The project itself is refused by name (IN-06), and a path that is not a
/// valid [`RelPath`] is refused as uncovered.
///
/// # Errors
///
/// [`JudgePushError::DirIsProject`] when `dir` is `project`; [`JudgePushError::DirInSnapshot`] naming `dir`
/// otherwise.
fn check_placement(project: &Path, dir: &Path, manifest: &Manifest) -> Result<(), JudgePushError> {
    let Some(rel) = relative_to(project, dir) else {
        return Ok(());
    };
    if rel.as_os_str().is_empty() {
        return Err(JudgePushError::DirIsProject {
            dir: SafeText::inline(&dir.to_string_lossy()),
        });
    }
    let covered = rel
        .to_str()
        .and_then(|text| RelPath::try_from(text).ok())
        .is_some_and(|rel| {
            manifest
                .excludes()
                .iter()
                .any(|exclude| exclude.covers(&rel))
        });
    if covered {
        Ok(())
    } else {
        Err(JudgePushError::DirInSnapshot {
            dir: SafeText::inline(&dir.to_string_lossy()),
        })
    }
}

/// `dir` relative to `project` when every component of `project` begins `dir`, compared ASCII
/// case-insensitively; `None` when `dir` lies outside `project`. The project itself gives an empty path.
fn relative_to(project: &Path, dir: &Path) -> Option<PathBuf> {
    let mut rest = dir.components();
    for part in project.components() {
        let next = rest.next()?;
        let same = next
            .as_os_str()
            .as_encoded_bytes()
            .eq_ignore_ascii_case(part.as_os_str().as_encoded_bytes());
        if !same {
            return None;
        }
    }
    Some(rest.as_path().to_path_buf())
}

/// Reads `path` as UTF-8 text of at most `max` bytes.
fn read_text(path: &Path, what: &'static str, max: u64) -> Result<String, JudgePushError> {
    let file = File::open(path).map_err(|source| JudgePushError::Read { what, source })?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| JudgePushError::Read { what, source })?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max) {
        return Err(JudgePushError::TooLarge { what, max });
    }
    String::from_utf8(bytes).map_err(|_| JudgePushError::Read {
        what,
        source: io::Error::new(io::ErrorKind::InvalidData, "not UTF-8 text"),
    })
}
