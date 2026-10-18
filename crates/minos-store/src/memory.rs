//! [`MemoryStore`]: the in-memory [`Ledger`] and [`Blobs`], for every test above the store (STORE-01, LOOP-09).
//!
//! Nothing is durable: the store lives and dies with the value. Its semantics match the file store's because
//! both run the same contract tests (`tests/contract.rs`), including the same-key no-op, the blob size cap, and
//! the rehash on read.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Mutex, MutexGuard};

use minos_core::id::{BlobId, RunId};
use minos_core::ledger::{Line, Record};

use crate::{Appended, Blobs, Ledger, LedgerRead, StoreError};

/// An in-memory ledger and blob store. Each map sits behind its own mutex; a lock poisoned by a panic fails
/// closed as [`StoreError::Poisoned`]. No guard is held across anything but the map operation.
#[derive(Debug, Default)]
pub struct MemoryStore {
    runs: Mutex<BTreeMap<RunId, Vec<Line>>>,
    blobs: Mutex<BTreeMap<BlobId, Vec<u8>>>,
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
            return Ok(if existing.record() == &record {
                Appended::Duplicate { seq }
            } else {
                Appended::Conflict { seq }
            });
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
