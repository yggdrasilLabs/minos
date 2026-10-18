//! The file ledger: one append-only JSON Lines file per run, appended under an exclusive per-run lock and read
//! under a shared one (STORE-02, STORE-03; 04-CONTEXT D-11 to D-13).
//!
//! # Reading a ledger
//!
//! Every complete (newline-terminated) line is checked in order, and the first failure refuses the whole ledger
//! with [`StoreError::Corrupt`] naming its 1-based line:
//!
//! 1. A line over [`MAX_LEDGER_LINE_BYTES`] is [`CorruptKind::LineTooLong`], wherever it is. The store never
//!    writes one, so it cannot be a torn write.
//! 2. A version probe reads only `record.v`. Any value but `"1"` is [`CorruptKind::UnknownVersion`], wherever
//!    the line is, so a newer ledger is never mistaken for a torn one (04-RESEARCH Pitfall 3).
//! 3. A line that is not a [`Line`] is [`CorruptKind::BadJson`], wherever it is, the final line included.
//! 4. [`Line::verify`] for the run must pass ([`CorruptKind::KeyMismatch`]).
//! 5. The line's `seq` must equal its 0-based position ([`CorruptKind::SeqGap`]).
//!
//! # The torn tail
//!
//! A crash during an append can leave the end of the file incomplete. [`Ledger::append`] serializes one line,
//! pushes its newline, and issues one `write_all`, and `serde_json` never emits a raw newline inside a line, so a
//! torn write is always a strict prefix with no newline after it. Bytes after the last newline are therefore the
//! one tolerated shape (04-CONTEXT D-13 as amended 2026-10-18; review WR-01). [`Ledger::read`] drops them and
//! reports a count, never the bytes. The next append truncates them with `set_len` under the exclusive lock
//! before writing; that is the only mutation of existing bytes, and it removes only bytes no append ever reported
//! written. Unterminated bytes over the line cap cannot be a torn write of this store either, so they are refused
//! as [`CorruptKind::LineTooLong`].
//!
//! A newline-terminated final line that is not a [`Line`] is not a torn write of this store. The store cannot
//! tell it from damage to a line `sync_data` already reported durable, so it is refused as
//! [`CorruptKind::BadJson`] and needs a human, because truncating it would erase history.
//!
//! The ledger file is never opened with truncation and never rewritten.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use minos_core::id::RunId;
use minos_core::ledger::{Line, Record};
use serde::Deserialize;

use super::{
    FileStore, dir_exists, ensure_dir, fsync_dir, io_error, len_u64, open_existing, open_or_create,
};
use crate::{Appended, CorruptKind, Ledger, LedgerRead, StoreError};

/// The longest ledger line the store reads or writes, without its newline. A `Judged` line with 64 metrics is a
/// few KiB, so 1 MiB is far above any real line and bounds what one line can cost to parse.
pub const MAX_LEDGER_LINE_BYTES: u64 = 1_048_576;

/// The largest ledger file the store reads. The whole file is read under the lock on every append, and a run has
/// hundreds to low thousands of lines, so 64 MiB is far above any real run.
pub const MAX_LEDGER_BYTES: u64 = 67_108_864;

/// The version probe: only `record.v`, read without `deny_unknown_fields`, so it parses lines of any version.
#[derive(Deserialize)]
struct Probe {
    record: ProbeRecord,
}

/// The record part of [`Probe`].
#[derive(Deserialize)]
struct ProbeRecord {
    v: String,
}

/// The verified lines of a ledger, and the byte offset of its torn tail, if it has one.
struct Parsed {
    lines: Vec<Line>,
    torn_start: Option<u64>,
}

/// The [`StoreError::Corrupt`] for `run` at 1-based line `line`.
const fn corrupt(run: &RunId, line: u64, kind: CorruptKind) -> StoreError {
    StoreError::Corrupt {
        run: *run,
        line,
        kind,
    }
}

/// Checks every line of `bytes` (see the module docs) and returns the verified lines and the torn tail's offset.
/// No error carries file bytes.
fn parse(run: &RunId, bytes: &[u8]) -> Result<Parsed, StoreError> {
    let newlines = bytes
        .iter()
        .enumerate()
        .filter_map(|(at, byte)| (*byte == b'\n').then_some(at));
    let mut lines = Vec::new();
    let mut start = 0_usize;
    let mut index = 0_u64;

    for end in newlines {
        let number = index.saturating_add(1);
        let text = bytes
            .get(start..end)
            .ok_or_else(|| corrupt(run, number, CorruptKind::BadJson))?;
        let after = end.saturating_add(1);

        let length = len_u64(text.len());
        if length > MAX_LEDGER_LINE_BYTES {
            return Err(corrupt(
                run,
                number,
                CorruptKind::LineTooLong { bytes: length },
            ));
        }
        if let Ok(probe) = serde_json::from_slice::<Probe>(text)
            && probe.record.v != "1"
        {
            return Err(corrupt(run, number, CorruptKind::UnknownVersion));
        }
        let line = serde_json::from_slice::<Line>(text)
            .map_err(|_not_a_line| corrupt(run, number, CorruptKind::BadJson))?;
        if line.verify(run).is_err() {
            return Err(corrupt(run, number, CorruptKind::KeyMismatch));
        }
        if line.seq() != index {
            return Err(corrupt(
                run,
                number,
                CorruptKind::SeqGap {
                    expected: index,
                    found: line.seq(),
                },
            ));
        }
        lines.push(line);
        start = after;
        index = number;
    }

    let rest = len_u64(bytes.len().saturating_sub(start));
    if rest > MAX_LEDGER_LINE_BYTES {
        return Err(corrupt(
            run,
            index.saturating_add(1),
            CorruptKind::LineTooLong { bytes: rest },
        ));
    }
    let torn_start = (rest > 0).then(|| len_u64(start));
    Ok(Parsed { lines, torn_start })
}

/// Reads the whole ledger file, refusing one over [`MAX_LEDGER_BYTES`] before and while reading.
fn read_capped(run: &RunId, file: &mut File) -> Result<Vec<u8>, StoreError> {
    let size = file
        .metadata()
        .map_err(io_error("inspecting the ledger"))?
        .len();
    if size > MAX_LEDGER_BYTES {
        return Err(StoreError::LedgerTooLarge {
            run: *run,
            bytes: size,
            max: MAX_LEDGER_BYTES,
        });
    }
    let mut bytes = Vec::new();
    file.take(MAX_LEDGER_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error("reading the ledger"))?;
    let read = len_u64(bytes.len());
    if read > MAX_LEDGER_BYTES {
        return Err(StoreError::LedgerTooLarge {
            run: *run,
            bytes: read,
            max: MAX_LEDGER_BYTES,
        });
    }
    Ok(bytes)
}

/// How the lock file is opened: read and write so `flock` works on every platform, never truncated.
fn lock_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).truncate(false);
    options
}

impl FileStore {
    /// Opens the run's lock file, creating it when it is missing. The lock is released when the handle drops.
    fn open_lock(&self, run: &RunId) -> Result<File, StoreError> {
        open_or_create(&self.lock_path(run), "ledger lock file", &lock_options())
            .map(|(file, _created)| file)
    }
}

impl Ledger for FileStore {
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError> {
        let runs_dir = self.runs_dir();
        let run_dir = self.run_dir(run);
        let runs_created = ensure_dir(&runs_dir, "runs directory")?;
        let run_created = ensure_dir(&run_dir, "run directory")?;

        let lock = self.open_lock(run)?;
        lock.lock().map_err(io_error("locking the ledger"))?;

        let mut options = OpenOptions::new();
        options.read(true).append(true);
        let (mut file, file_created) =
            open_or_create(&self.ledger_path(run), "ledger file", &options)?;
        let bytes = read_capped(run, &mut file)?;
        let parsed = parse(run, &bytes)?;
        if let Some(torn_start) = parsed.torn_start {
            file.set_len(torn_start)
                .map_err(io_error("truncating the torn ledger tail"))?;
            file.sync_data().map_err(io_error("syncing the ledger"))?;
        }

        let key = record.event().key(run);
        if let Some(existing) = parsed.lines.iter().find(|line| line.key() == &key) {
            let seq = existing.seq();
            return Ok(if existing.record() == &record {
                Appended::Duplicate { seq }
            } else {
                Appended::Conflict { seq }
            });
        }

        let seq = len_u64(parsed.lines.len());
        let mut encoded =
            serde_json::to_vec(&Line::new(seq, run, record)).map_err(StoreError::Encode)?;
        let length = len_u64(encoded.len());
        if length > MAX_LEDGER_LINE_BYTES {
            return Err(StoreError::LineTooLong {
                bytes: length,
                max: MAX_LEDGER_LINE_BYTES,
            });
        }
        encoded.push(b'\n');
        file.write_all(&encoded)
            .map_err(io_error("writing the ledger"))?;
        file.sync_data().map_err(io_error("syncing the ledger"))?;
        if file_created || run_created {
            fsync_dir(&run_dir)?;
        }
        if run_created {
            fsync_dir(&runs_dir)?;
        }
        if runs_created {
            fsync_dir(&self.root)?;
        }
        drop(lock);
        Ok(Appended::New { seq })
    }

    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError> {
        let path = self.ledger_path(run);
        if !dir_exists(&self.runs_dir(), "runs directory")?
            || !dir_exists(&self.run_dir(run), "run directory")?
        {
            return Ok(LedgerRead::new(Vec::new(), 0));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        if open_existing(&path, "ledger file", &options)?.is_none() {
            return Ok(LedgerRead::new(Vec::new(), 0));
        }

        let lock = self.open_lock(run)?;
        lock.lock_shared().map_err(io_error("locking the ledger"))?;
        let Some(mut file) = open_existing(&path, "ledger file", &options)? else {
            return Ok(LedgerRead::new(Vec::new(), 0));
        };
        let bytes = read_capped(run, &mut file)?;
        drop(lock);
        let parsed = parse(run, &bytes)?;
        let torn_tail = u64::from(parsed.torn_start.is_some());
        Ok(LedgerRead::new(parsed.lines, torn_tail))
    }
}
