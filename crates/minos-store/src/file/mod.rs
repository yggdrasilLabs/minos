//! [`FileStore`]: the file-backed [`Ledger`](crate::Ledger) and [`Blobs`](crate::Blobs) (STORE-02, STORE-03,
//! STORE-04).
//!
//! # Layout
//!
//! Under the store root (04-CONTEXT D-10; Phase 8 opens `<project>/`[`STORE_DIR`]):
//!
//! ```text
//! runs/<run hex>/ledger.jsonl    one append-only JSON Lines ledger per run
//! runs/<run hex>/ledger.lock     the per-run lock file; it holds no data
//! blobs/sha256/<2 hex>/<62 hex>  one file per blob, shared by every run
//! ```
//!
//! Every directory the store creates has mode 0700. The root, every directory under it, and every ledger, lock,
//! and blob file is checked with `symlink_metadata` before use: a symlink or an unexpected file type is
//! [`StoreError::Unsafe`]. A file is opened only after that check, and the opened handle's device and inode
//! must match what the check saw, so a path swapped for a symlink between the check and the open is refused too.
//!
//! # Durability
//!
//! An append writes its line with the trailing newline and calls `sync_data` before it returns (D-12); on Apple
//! platforms std implements that with `F_FULLFSYNC`. When an append or a put creates a file or a directory, the
//! directory that holds the new entry is fsynced as well. A blob is written to a temporary file in its shard
//! directory, synced, and renamed over its final path, so a reader sees the whole blob or none of it.

mod ledger;

pub use ledger::{MAX_LEDGER_BYTES, MAX_LEDGER_LINE_BYTES};

use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

use minos_core::id::RunId;

use crate::StoreError;

/// The store root's directory name inside a project. Phase 8 opens `<project>/.minos`; the source denylist never
/// packs it (04-CONTEXT D-07, D-10), so a project's history never enters its own snapshot.
pub const STORE_DIR: &str = ".minos";

/// The ledger file's name inside a run directory.
const LEDGER_FILE: &str = "ledger.jsonl";

/// The lock file's name inside a run directory.
const LOCK_FILE: &str = "ledger.lock";

/// A ledger and blob store in a directory on the local filesystem.
///
/// Every operation re-reads what it needs from disk under the run's lock, so several `FileStore` values, in one
/// process or many, may share a root.
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    /// Opens the store at `root`, creating the directory (and any missing parent) with mode 0700 when it does not
    /// exist.
    ///
    /// # Errors
    ///
    /// [`StoreError::Unsafe`] when `root` is a symlink or not a directory, and [`StoreError::Io`] when it cannot
    /// be inspected or created.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        match fs::symlink_metadata(root) {
            Ok(meta) => require_dir(&meta, "store root")?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(root)
                    .map_err(io_error("creating the store root"))?;
                let meta =
                    fs::symlink_metadata(root).map_err(io_error("inspecting the store root"))?;
                require_dir(&meta, "store root")?;
            }
            Err(error) => return Err(io_error("inspecting the store root")(error)),
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// `root/runs`.
    fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    /// `root/runs/<run hex>`.
    fn run_dir(&self, run: &RunId) -> PathBuf {
        self.runs_dir().join(run.to_hex())
    }

    /// `root/runs/<run hex>/ledger.jsonl`.
    fn ledger_path(&self, run: &RunId) -> PathBuf {
        self.run_dir(run).join(LEDGER_FILE)
    }

    /// `root/runs/<run hex>/ledger.lock`.
    fn lock_path(&self, run: &RunId) -> PathBuf {
        self.run_dir(run).join(LOCK_FILE)
    }
}

/// Maps an I/O error to [`StoreError::Io`] with fixed context.
fn io_error(op: &'static str) -> impl FnOnce(io::Error) -> StoreError {
    move |source| StoreError::Io { op, source }
}

/// A length as a `u64`, saturating: a length that does not fit is larger than any cap.
fn len_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Refuses anything but a real directory (a symlink to a directory is refused).
fn require_dir(meta: &Metadata, what: &'static str) -> Result<(), StoreError> {
    if meta.file_type().is_dir() {
        Ok(())
    } else {
        Err(StoreError::Unsafe { what })
    }
}

/// Whether the directory at `path` exists: `false` when it is missing, [`StoreError::Unsafe`] when it is a
/// symlink or another file type.
fn dir_exists(path: &Path, what: &'static str) -> Result<bool, StoreError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => require_dir(&meta, what).map(|()| true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error("inspecting a store directory")(error)),
    }
}

/// Makes sure the directory at `path` exists, creating it with mode 0700 when it is missing, and says whether it
/// was created here. A directory created concurrently by another store counts as existing. `mkdir` never follows
/// a symlink at `path`, and an existing entry is checked with `symlink_metadata`.
fn ensure_dir(path: &Path, what: &'static str) -> Result<bool, StoreError> {
    if dir_exists(path, what)? {
        return Ok(false);
    }
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            dir_exists(path, what).map(|_exists| false)
        }
        Err(error) => Err(io_error("creating a store directory")(error)),
    }
}

/// Fsyncs the directory at `path`, so an entry created in it survives a power failure.
fn fsync_dir(path: &Path) -> Result<(), StoreError> {
    File::open(path)
        .and_then(|dir| dir.sync_all())
        .map_err(io_error("syncing a store directory"))
}

/// Opens the existing regular file at `path` with `options`. `None` when nothing is there;
/// [`StoreError::Unsafe`] when it is a symlink or another file type, or when the opened file is not the one the
/// check saw.
fn open_existing(
    path: &Path,
    what: &'static str,
    options: &OpenOptions,
) -> Result<Option<File>, StoreError> {
    let seen = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("inspecting a store file")(error)),
    };
    if !seen.file_type().is_file() {
        return Err(StoreError::Unsafe { what });
    }
    let file = options
        .open(path)
        .map_err(io_error("opening a store file"))?;
    let opened = file
        .metadata()
        .map_err(io_error("inspecting a store file"))?;
    if opened.dev() == seen.dev() && opened.ino() == seen.ino() {
        Ok(Some(file))
    } else {
        Err(StoreError::Unsafe { what })
    }
}

/// Opens the regular file at `path` with `options`, creating it when it is missing, and says whether it was
/// created here. Creation uses `create_new` (`O_EXCL`), which never follows a symlink; losing a creation race to
/// another store falls back to opening the existing file under the same checks.
fn open_or_create(
    path: &Path,
    what: &'static str,
    options: &OpenOptions,
) -> Result<(File, bool), StoreError> {
    if let Some(file) = open_existing(path, what, options)? {
        return Ok((file, false));
    }
    match options.clone().create_new(true).open(path) {
        Ok(file) => Ok((file, true)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            match open_existing(path, what, options)? {
                Some(file) => Ok((file, false)),
                None => Err(io_error("opening a store file")(io::Error::from(
                    io::ErrorKind::NotFound,
                ))),
            }
        }
        Err(error) => Err(io_error("creating a store file")(error)),
    }
}
