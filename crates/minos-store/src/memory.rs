//! [`MemoryStore`]: the in-memory [`Ledger`], [`Blobs`], and [`RunLock`], for every test above the store
//! (STORE-01, LOOP-06, LOOP-09).
//!
//! Nothing is durable: the store lives and dies with the value. Its semantics match the file store's because
//! both run the same contract tests (`tests/contract.rs`), including the same-key no-op, the blob size cap, and
//! the rehash on read.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::{Mutex, MutexGuard};

use minos_core::id::{BlobId, RunId};
use minos_core::ledger::{Line, Record};

use crate::{Appended, Blobs, Ledger, LedgerRead, RunLock, StoreError};

/// An in-memory ledger, blob store, and run lock. Each map and set sits behind its own mutex; a lock poisoned by a
/// panic fails closed as [`StoreError::Poisoned`]. No mutex guard is held across anything but the map or set
/// operation; a [`MemoryRunGuard`] holds a run, not the mutex.
#[derive(Debug, Default)]
pub struct MemoryStore {
    runs: Mutex<BTreeMap<RunId, Vec<Line>>>,
    blobs: Mutex<BTreeMap<BlobId, Vec<u8>>>,
    run_locks: Mutex<BTreeSet<RunId>>,
}

/// Holds one run of a [`MemoryStore`] locked (LOOP-06). Dropping it removes the run from the store's locked set.
#[must_use = "the run is unlocked as soon as the guard drops"]
#[derive(Debug)]
pub struct MemoryRunGuard<'a> {
    locks: &'a Mutex<BTreeSet<RunId>>,
    run: RunId,
}

impl Drop for MemoryRunGuard<'_> {
    /// Removes the run from the locked set. A poisoned mutex is left as it is, never a panic in `drop`: every
    /// later `lock_run` on the store already fails closed as [`StoreError::Poisoned`].
    fn drop(&mut self) {
        if let Ok(mut locks) = self.locks.lock() {
            locks.remove(&self.run);
        }
    }
}

impl MemoryStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Locks `mutex`, mapping a poisoned lock to [`StoreError::Poisoned`].
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, StoreError> {
    mutex.lock().map_err(|_poisoned| StoreError::Poisoned)
}

/// A length as a `u64`, saturating: a length that does not fit is larger than any cap.
fn len_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

impl Ledger for MemoryStore {
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError> {
        let key = record.event().key(run);
        let mut runs = lock(&self.runs)?;
        let lines = runs.entry(*run).or_default();
        if let Some(existing) = lines.iter().find(|line| line.key() == &key) {
            let seq = existing.seq();
            return if existing.record() == &record {
                Ok(Appended::Duplicate { seq })
            } else {
                Err(StoreError::Conflict { seq })
            };
        }
        let seq = len_u64(lines.len());
        lines.push(Line::new(seq, run, record));
        Ok(Appended::New { seq })
    }

    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError> {
        let lines = lock(&self.runs)?.get(run).cloned().unwrap_or_default();
        Ok(LedgerRead::new(lines, 0))
    }
}

impl RunLock for MemoryStore {
    type Guard<'a> = MemoryRunGuard<'a>;

    fn lock_run(&self, run: &RunId) -> Result<MemoryRunGuard<'_>, StoreError> {
        if lock(&self.run_locks)?.insert(*run) {
            Ok(MemoryRunGuard {
                locks: &self.run_locks,
                run: *run,
            })
        } else {
            Err(StoreError::RunLocked { run: *run })
        }
    }
}

impl Blobs for MemoryStore {
    fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError> {
        let id = BlobId::of(bytes);
        lock(&self.blobs)?
            .entry(id)
            .or_insert_with(|| bytes.to_vec());
        Ok(id)
    }

    fn put_reader<R: Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError> {
        let mut bytes = Vec::new();
        reader
            .take(max.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| StoreError::Io {
                op: "reading a streamed blob",
                source,
            })?;
        if len_u64(bytes.len()) > max {
            return Err(StoreError::PutTooLarge { max });
        }
        self.put(&bytes)
    }

    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, StoreError> {
        let copy = {
            let blobs = lock(&self.blobs)?;
            let stored = blobs.get(id).ok_or(StoreError::BlobMissing { id: *id })?;
            let size = len_u64(stored.len());
            if size > max {
                return Err(StoreError::BlobTooLarge { id: *id, size, max });
            }
            stored.clone()
        };
        if BlobId::of(&copy) == *id {
            Ok(copy)
        } else {
            Err(StoreError::BlobTampered { id: *id })
        }
    }

    fn contains(&self, id: &BlobId) -> Result<bool, StoreError> {
        Ok(lock(&self.blobs)?.contains_key(id))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::thread;

    use minos_core::id::{ManifestId, RunId, TreeId};

    use super::MemoryStore;
    use crate::{RunLock, StoreError};

    type TestResult = Result<(), Box<dyn Error>>;

    fn run() -> RunId {
        RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
    }

    /// Poisons the store's run-lock mutex by panicking in a thread that holds it.
    fn poison_run_locks(store: &MemoryStore) -> TestResult {
        let joined = thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _held = store.run_locks.lock();
                    panic!("poisoning the run-lock mutex on purpose");
                })
                .join()
        });
        if joined.is_ok() {
            return Err("the poisoning thread did not panic".into());
        }
        if !store.run_locks.is_poisoned() {
            return Err("the run-lock mutex is not poisoned".into());
        }
        Ok(())
    }

    #[test]
    fn a_poisoned_run_lock_fails_closed_and_the_guard_drops_quietly() -> TestResult {
        let store = MemoryStore::new();
        let held = store.lock_run(&run())?;
        poison_run_locks(&store)?;

        drop(held);

        assert!(
            matches!(store.lock_run(&run()), Err(StoreError::Poisoned)),
            "a poisoned run-lock mutex must be Poisoned"
        );
        Ok(())
    }
}
