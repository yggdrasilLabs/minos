//! Reading and writing `minos.lock`, shared by `minos env build` and `minos judge push` (05-CONTEXT D-10;
//! 07-CONTEXT D-01, D-02).
//!
//! [`read_lock`] reads at most one byte over [`MAX_LOCK_BYTES`], so an over-cap file is refused as
//! [`LockError::TooLarge`] rather than parsed. [`write_lock`] writes a synced temporary file in `.minos/` (the
//! store root, on the source denylist), renames it over the lock, and syncs the lock's directory, so a reader sees
//! the old lock or the new one, never part of one. Both commands store their blob first and write the lock last.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use minos_core::lock::{Lock, LockError, MAX_LOCK_BYTES};

/// Why `minos.lock` could not be read. Messages never print the file's bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum LockFileError {
    /// The file could not be read, or is not UTF-8 text.
    #[error("could not read minos.lock")]
    Read {
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The text is over [`MAX_LOCK_BYTES`] or is not a valid lock.
    #[error("minos.lock is refused")]
    Lock(#[source] LockError),
}

/// The lock at `path`, or `None` when there is no file there.
///
/// # Errors
///
/// [`LockFileError::Read`] when the file exists but cannot be read or is not UTF-8, and [`LockFileError::Lock`]
/// when it is over [`MAX_LOCK_BYTES`] or does not parse.
pub fn read_lock(path: &Path) -> Result<Option<Lock>, LockFileError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(LockFileError::Read { source }),
        Ok(_) => {}
    }
    // One byte over the cap is read, so an over-cap file is refused as too large, never parsed.
    let max = u64::try_from(MAX_LOCK_BYTES).unwrap_or(u64::MAX);
    let file = File::open(path).map_err(|source| LockFileError::Read { source })?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| LockFileError::Read { source })?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max) {
        return Err(LockFileError::Lock(LockError::TooLarge));
    }
    let text = String::from_utf8(bytes).map_err(|_| LockFileError::Read {
        source: io::Error::new(io::ErrorKind::InvalidData, "not UTF-8 text"),
    })?;
    Lock::parse_toml(&text)
        .map(Some)
        .map_err(LockFileError::Lock)
}

/// Writes `bytes` to a temporary file in `staging` (the store root, on the source denylist), syncs it, renames it
/// over `lock`, and syncs the lock's directory, so `lock` is the old file or the new one, never part of one.
///
/// # Errors
///
/// Any I/O error creating, writing, syncing, or renaming the temporary file, or syncing the directory.
pub fn write_lock(staging: &Path, lock: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = tempfile::Builder::new()
        .prefix("minos.lock.")
        .tempfile_in(staging)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(lock).map_err(|error| error.error)?;
    if let Some(dir) = lock.parent() {
        File::open(dir)?.sync_all()?;
    }
    Ok(())
}
