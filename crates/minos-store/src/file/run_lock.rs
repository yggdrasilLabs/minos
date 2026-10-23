//! [`FileStore`]'s [`RunLock`]: an exclusive `flock` on `runs/<run hex>/driver.lock` (LOOP-06).
//!
//! The driver lock is a separate file from the per-append `ledger.lock` (04-CONTEXT D-11). A driver holds its run
//! for a whole session, across model calls and VM runs, while `ledger.lock` is taken and released inside each
//! [`Ledger::append`](crate::Ledger::append). Holding one file for both would block `minos stop` and
//! `minos steer` for as long as a VM ran (07-RESEARCH Pitfall 11), so a locked run still appends from any handle.
//!
//! The lock is [`File::try_lock`], which is `flock(LOCK_EX | LOCK_NB)`. An `flock` lock belongs to the open file
//! description, so a second open of `driver.lock` is refused whether it comes from another process, another
//! [`FileStore`] in this process, or this same store, and the kernel releases the lock when the holder's
//! descriptor closes, including when the process crashes. The lock is never waited on: a held run is
//! [`StoreError::RunLocked`] at once.
//!
//! `driver.lock` is opened through the store's `open_or_create`, so a symlink or another file type planted at
//! the path is [`StoreError::Unsafe`] and the opened file must be the one the check saw. A new lock file is created
//! with `O_EXCL` and mode 0600; it holds no data and is never truncated.

use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt;

use minos_core::id::RunId;

use super::{FileStore, ensure_dir, fsync_dir, io_error, open_or_create};
use crate::{RunLock, StoreError};

/// Holds one run of a [`FileStore`] locked (LOOP-06). The `flock` lock lives as long as the open file: dropping
/// the guard closes it, and the kernel releases the run.
#[must_use = "the run is unlocked as soon as the guard drops"]
#[derive(Debug)]
pub struct FileRunGuard {
    _file: File,
}

/// How `driver.lock` is opened: read and write so `flock` works on every platform, never truncated, and created
/// with mode 0600.
fn driver_lock_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).truncate(false).mode(0o600);
    options
}

impl RunLock for FileStore {
    type Guard<'a> = FileRunGuard;

    fn lock_run(&self, run: &RunId) -> Result<FileRunGuard, StoreError> {
        let runs_dir = self.runs_dir();
        let run_dir = self.run_dir(run);
        let runs_created = ensure_dir(&runs_dir, "runs directory")?;
        let run_created = ensure_dir(&run_dir, "run directory")?;
        if run_created {
            fsync_dir(&runs_dir)?;
        }
        if runs_created {
            fsync_dir(&self.root)?;
        }

        let (file, _created) = open_or_create(
            &self.driver_lock_path(run),
            "driver lock file",
            &driver_lock_options(),
        )?;
        match file.try_lock() {
            Ok(()) => Ok(FileRunGuard { _file: file }),
            Err(TryLockError::WouldBlock) => Err(StoreError::RunLocked { run: *run }),
            Err(TryLockError::Error(error)) => Err(io_error("locking the run")(error)),
        }
    }
}
