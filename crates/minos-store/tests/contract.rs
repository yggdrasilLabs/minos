//! The Ledger and Blobs contract, run against every implementation (STORE-01): the memory store and the file
//! store, each file case on its own fresh temporary store root.
//!
//! Each case is one generic function over `L: Ledger`, `B: Blobs`, or `S: RunLock`. `ledger_contract!`,
//! `blob_contract!`, and `run_lock_contract!` expand a case into a module with one `#[test]` per implementation,
//! so the memory store and the file store are held to exactly the same assertions. A few run-lock cases that
//! only the file store can have (its lock file's mode, a planted symlink, two handles) follow the arms.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{PermissionsExt, symlink};

use minos_core::id::{BlobId, ExperimentId, ManifestId, RunId, TreeId};
use minos_core::ledger::{Event, EventKey, Record};
use minos_store::{Appended, Blobs, FileStore, Ledger, MemoryStore, RunLock, StoreError};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The run every ledger case appends to.
fn run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
}

/// A second run: the same manifest and tree, started a second later.
fn other_run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_001)
}

/// The `RunStarted` record of the run started at `started_at` on the fixture manifest and tree, with its run.
fn run_started(started_at: u64) -> (RunId, Record) {
    let manifest = ManifestId::of(b"m");
    let base = TreeId::of(b"t");
    let run = RunId::of(&manifest, &base, started_at);
    let record = Record::v1(Event::RunStarted {
        run,
        manifest,
        base,
        baseline: ExperimentId::baseline(&base),
        started_at,
        root_image: None,
    });
    (run, record)
}

/// A `Steered` record. Its key is the generation and the ordinal only, so two notes at one ordinal conflict.
fn steered(generation: u32, ordinal: u32, note: &[u8]) -> Record {
    Record::v1(Event::Steered {
        generation,
        ordinal,
        note: BlobId::of(note),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Ledger cases
// ---------------------------------------------------------------------------------------------------------------

fn append_then_read_round_trips<L: Ledger>(ledger: &L) -> TestResult {
    let records: Vec<Record> = (0..3).map(|ordinal| steered(1, ordinal, b"note")).collect();
    for (expected_seq, record) in (0_u64..).zip(&records) {
        assert_eq!(
            ledger.append(&run(), record.clone())?,
            Appended::New { seq: expected_seq }
        );
    }

    let read = ledger.read(&run())?;
    assert_eq!(read.torn_tail(), 0);
    assert_eq!(read.lines().len(), records.len());
    for ((expected_seq, line), record) in (0_u64..).zip(read.lines()).zip(&records) {
        assert_eq!(line.seq(), expected_seq);
        assert_eq!(line.record(), record);
        line.verify(&run())?;
    }
    assert!(read.records().eq(records.iter()));
    Ok(())
}

fn duplicate_append_is_a_no_op<L: Ledger>(ledger: &L) -> TestResult {
    assert_eq!(
        ledger.append(&run(), steered(1, 0, b"first"))?,
        Appended::New { seq: 0 }
    );
    let record = steered(1, 1, b"second");
    assert_eq!(
        ledger.append(&run(), record.clone())?,
        Appended::New { seq: 1 }
    );
    let before = ledger.read(&run())?;

    assert_eq!(
        ledger.append(&run(), record)?,
        Appended::Duplicate { seq: 1 }
    );

    assert_eq!(ledger.read(&run())?, before);
    Ok(())
}

fn conflicting_append_is_not_written<L: Ledger>(ledger: &L) -> TestResult {
    let first = steered(2, 0, b"first note");
    assert_eq!(
        ledger.append(&run(), first.clone())?,
        Appended::New { seq: 0 }
    );
    let before = ledger.read(&run())?;

    let result = ledger.append(&run(), steered(2, 0, b"a different note"));

    match result {
        Err(error @ StoreError::Conflict { seq: 0 }) => assert!(!error.is_retryable()),
        Err(other) => {
            return Err(format!("a conflicting append must be Conflict, got {other}").into());
        }
        Ok(appended) => return Err(format!("a conflicting append returned {appended:?}").into()),
    }

    let after = ledger.read(&run())?;
    assert_eq!(after, before);
    assert!(after.records().eq([&first]));
    Ok(())
}

fn runs_are_independent<L: Ledger>(ledger: &L) -> TestResult {
    let record = steered(1, 0, b"note");
    assert_eq!(
        ledger.append(&run(), record.clone())?,
        Appended::New { seq: 0 }
    );
    assert_eq!(
        ledger.append(&other_run(), record)?,
        Appended::New { seq: 0 }
    );

    for id in [run(), other_run()] {
        let read = ledger.read(&id)?;
        assert_eq!(read.lines().len(), 1);
        for line in read.lines() {
            assert_eq!(line.seq(), 0);
            line.verify(&id)?;
        }
    }
    Ok(())
}

fn unknown_run_reads_empty<L: Ledger>(ledger: &L) -> TestResult {
    assert_eq!(
        ledger.append(&run(), steered(1, 0, b"note"))?,
        Appended::New { seq: 0 }
    );

    let read = ledger.read(&other_run())?;

    assert!(read.lines().is_empty());
    assert_eq!(read.torn_tail(), 0);
    Ok(())
}

fn runs_lists_appended_runs<L: Ledger>(ledger: &L) -> TestResult {
    // Arrange
    let (first, first_started) = run_started(1_760_000_000);
    let (second, second_started) = run_started(1_760_000_001);
    assert_eq!(
        ledger.append(&second, second_started)?,
        Appended::New { seq: 0 }
    );
    assert_eq!(
        ledger.append(&first, first_started)?,
        Appended::New { seq: 0 }
    );
    assert_eq!(
        ledger.append(&first, steered(1, 0, b"note"))?,
        Appended::New { seq: 1 }
    );

    // Act
    let runs = ledger.runs()?;

    // Assert
    assert_eq!(runs, BTreeSet::from([first, second]));
    let mut sorted = [first, second];
    sorted.sort();
    assert!(runs.iter().eq(sorted.iter()), "runs come in RunId order");
    Ok(())
}

fn runs_of_an_empty_store_is_empty<L: Ledger>(ledger: &L) -> TestResult {
    // Arrange: a fresh store, and a read of a run that was never appended to.
    assert!(ledger.read(&run())?.lines().is_empty());

    // Act
    let runs = ledger.runs()?;

    // Assert
    assert!(runs.is_empty(), "a fresh store lists no runs");
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Blob cases
// ---------------------------------------------------------------------------------------------------------------

fn put_returns_the_content_address<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = b"the content".as_slice();

    let id = blobs.put(bytes)?;

    assert_eq!(id, BlobId::of(bytes));
    let max = u64::try_from(bytes.len())?;
    assert_eq!(blobs.get(&id, max)?, bytes);
    assert_eq!(blobs.get(&id, u64::MAX)?, bytes);
    Ok(())
}

fn get_over_max_is_refused<B: Blobs>(blobs: &B) -> TestResult {
    let id = blobs.put(b"12345")?;

    let result = blobs.get(&id, 4);

    assert!(
        matches!(
            result,
            Err(StoreError::BlobTooLarge { id: refused, size: 5, max: 4 }) if refused == id
        ),
        "a 5-byte blob under a 4-byte cap must be BlobTooLarge"
    );
    Ok(())
}

fn get_missing_is_refused<B: Blobs>(blobs: &B) -> TestResult {
    let absent = BlobId::of(b"never stored");
    blobs.put(b"something else")?;

    let result = blobs.get(&absent, u64::MAX);

    assert!(
        matches!(result, Err(StoreError::BlobMissing { id }) if id == absent),
        "an absent blob must be BlobMissing"
    );
    Ok(())
}

fn contains_tracks_put<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = b"tracked".as_slice();
    let id = BlobId::of(bytes);
    assert!(!blobs.contains(&id)?);

    blobs.put(bytes)?;

    assert!(blobs.contains(&id)?);
    assert!(!blobs.contains(&BlobId::of(b"other"))?);
    Ok(())
}

fn put_is_idempotent<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = b"stored twice".as_slice();

    let first = blobs.put(bytes)?;
    let second = blobs.put(bytes)?;

    assert_eq!(first, second);
    assert_eq!(blobs.get(&first, u64::MAX)?, bytes);
    Ok(())
}

/// A reader that yields `prefix` once and then fails, as a pipe from a dying builder would.
struct FailsAfter<'a> {
    prefix: &'a [u8],
}

impl Read for FailsAfter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.prefix.is_empty() {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let n = self.prefix.read(buf)?;
        Ok(n)
    }
}

/// Bytes larger than one 64 KiB copy buffer, so a streamed put crosses buffer boundaries.
fn streamed_bytes() -> Vec<u8> {
    (0_u32..200_000)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect()
}

fn put_reader_equals_put<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = streamed_bytes();
    let max = u64::try_from(bytes.len())?;

    let streamed = blobs.put_reader(bytes.as_slice(), max)?;
    let whole = blobs.put(&bytes)?;

    assert_eq!(streamed, whole);
    assert_eq!(streamed, BlobId::of(&bytes));
    assert_eq!(blobs.get(&streamed, max)?, bytes);
    Ok(())
}

fn put_reader_exact_max<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = b"exactly at the cap".as_slice();
    let max = u64::try_from(bytes.len())?;

    let id = blobs.put_reader(bytes, max)?;

    assert_eq!(id, BlobId::of(bytes));
    assert_eq!(blobs.get(&id, max)?, bytes);
    Ok(())
}

fn put_reader_over_max<B: Blobs>(blobs: &B) -> TestResult {
    let bytes = streamed_bytes();
    let max = u64::try_from(bytes.len())?.saturating_sub(1);

    let result = blobs.put_reader(bytes.as_slice(), max);

    assert!(
        matches!(result, Err(StoreError::PutTooLarge { max: refused }) if refused == max),
        "a stream one byte over its cap must be PutTooLarge"
    );
    assert!(!blobs.contains(&BlobId::of(&bytes))?);
    Ok(())
}

fn put_reader_reader_error<B: Blobs>(blobs: &B) -> TestResult {
    let prefix = b"half a root image".as_slice();

    let result = blobs.put_reader(FailsAfter { prefix }, u64::MAX);

    match result {
        Err(error @ StoreError::Io { .. }) => assert!(error.is_retryable()),
        Err(other) => return Err(format!("a reader error must be Io, got {other}").into()),
        Ok(id) => return Err(format!("a failing reader stored blob {id}").into()),
    }
    assert!(!blobs.contains(&BlobId::of(prefix))?);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Run-lock cases (LOOP-06)
// ---------------------------------------------------------------------------------------------------------------

/// Checks that `result` is `StoreError::RunLocked` naming `expected`, and that it is not retryable.
fn expect_run_locked<G>(result: Result<G, StoreError>, expected: &RunId) -> TestResult {
    match result {
        Err(error) => {
            assert!(
                matches!(error, StoreError::RunLocked { run: locked } if locked == *expected),
                "a held run must be RunLocked naming the run, got {error}"
            );
            assert!(!error.is_retryable(), "RunLocked must not be retryable");
            Ok(())
        }
        Ok(_guard) => Err("a second lock on a held run was granted".into()),
    }
}

fn second_lock_is_refused<S: RunLock>(store: &S) -> TestResult {
    let _held = store.lock_run(&run())?;

    let result = store.lock_run(&run());

    expect_run_locked(result, &run())
}

fn lock_after_drop_succeeds<S: RunLock>(store: &S) -> TestResult {
    let held = store.lock_run(&run())?;
    expect_run_locked(store.lock_run(&run()), &run())?;

    drop(held);

    let _again = store.lock_run(&run())?;
    expect_run_locked(store.lock_run(&run()), &run())
}

fn runs_lock_independently<S: RunLock>(store: &S) -> TestResult {
    let _first = store.lock_run(&run())?;

    let _second = store.lock_run(&other_run())?;

    expect_run_locked(store.lock_run(&run()), &run())?;
    expect_run_locked(store.lock_run(&other_run()), &other_run())
}

fn locked_run_still_appends<S: RunLock + Ledger>(store: &S) -> TestResult {
    let _held = store.lock_run(&run())?;

    let appended = store.append(&run(), steered(1, 0, b"stop note"))?;

    assert_eq!(appended, Appended::New { seq: 0 });
    assert_eq!(store.read(&run())?.lines().len(), 1);
    Ok(())
}

fn a_locked_run_is_not_listed<S: RunLock + Ledger>(store: &S) -> TestResult {
    // Arrange: lock one run and release it with nothing appended; append to another.
    drop(store.lock_run(&run())?);
    let (started, record) = run_started(1_760_000_002);
    assert_eq!(store.append(&started, record)?, Appended::New { seq: 0 });

    // Act
    let runs = store.runs()?;

    // Assert
    assert_eq!(runs, BTreeSet::from([started]));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Arms
// ---------------------------------------------------------------------------------------------------------------

/// Runs a ledger case against every implementation. The file arm keeps its `TempDir` alive until the case returns.
macro_rules! ledger_contract {
    ($case:ident) => {
        mod $case {
            #[test]
            fn memory() -> super::TestResult {
                super::$case(&minos_store::MemoryStore::new())
            }

            #[test]
            fn file() -> super::TestResult {
                let dir = tempfile::tempdir()?;
                super::$case(&minos_store::FileStore::open(dir.path())?)
            }
        }
    };
}

/// Runs a blob case against every implementation. The file arm keeps its `TempDir` alive until the case returns.
macro_rules! blob_contract {
    ($case:ident) => {
        mod $case {
            #[test]
            fn memory() -> super::TestResult {
                super::$case(&minos_store::MemoryStore::new())
            }

            #[test]
            fn file() -> super::TestResult {
                let dir = tempfile::tempdir()?;
                super::$case(&minos_store::FileStore::open(dir.path())?)
            }
        }
    };
}

/// Runs a run-lock case against every implementation. The file arm keeps its `TempDir` alive until the case
/// returns.
macro_rules! run_lock_contract {
    ($case:ident) => {
        mod $case {
            #[test]
            fn memory() -> super::TestResult {
                super::$case(&minos_store::MemoryStore::new())
            }

            #[test]
            fn file() -> super::TestResult {
                let dir = tempfile::tempdir()?;
                super::$case(&minos_store::FileStore::open(dir.path())?)
            }
        }
    };
}

ledger_contract!(append_then_read_round_trips);
ledger_contract!(duplicate_append_is_a_no_op);
ledger_contract!(conflicting_append_is_not_written);
ledger_contract!(runs_are_independent);
ledger_contract!(unknown_run_reads_empty);
ledger_contract!(runs_lists_appended_runs);
ledger_contract!(runs_of_an_empty_store_is_empty);

blob_contract!(put_returns_the_content_address);
blob_contract!(get_over_max_is_refused);
blob_contract!(get_missing_is_refused);
blob_contract!(contains_tracks_put);
blob_contract!(put_is_idempotent);
blob_contract!(put_reader_equals_put);
blob_contract!(put_reader_exact_max);
blob_contract!(put_reader_over_max);
blob_contract!(put_reader_reader_error);

run_lock_contract!(second_lock_is_refused);
run_lock_contract!(lock_after_drop_succeeds);
run_lock_contract!(runs_lock_independently);
run_lock_contract!(locked_run_still_appends);
run_lock_contract!(a_locked_run_is_not_listed);

// ---------------------------------------------------------------------------------------------------------------
// File-only run-lock cases
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn driver_lock_is_created_with_mode_0600() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    let _held = store.lock_run(&run())?;

    let path = dir
        .path()
        .join("runs")
        .join(run().to_hex())
        .join("driver.lock");
    let meta = fs::symlink_metadata(&path)?;
    assert!(
        meta.file_type().is_file(),
        "driver.lock must be a regular file"
    );
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(
        fs::symlink_metadata(dir.path().join("runs").join(run().to_hex()))?
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    Ok(())
}

#[test]
fn a_symlink_at_driver_lock_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let run_dir = dir.path().join("runs").join(run().to_hex());
    fs::create_dir_all(&run_dir)?;
    let target = dir.path().join("elsewhere");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)?;
    symlink(&target, run_dir.join("driver.lock"))?;

    let result = store.lock_run(&run());

    assert!(
        matches!(
            result,
            Err(StoreError::Unsafe {
                what: "driver lock file"
            })
        ),
        "a symlink planted at driver.lock must be Unsafe"
    );
    Ok(())
}

#[test]
fn two_file_handles_refuse_each_others_lock() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = FileStore::open(dir.path())?;
    let second = FileStore::open(dir.path())?;

    let held = first.lock_run(&run())?;
    expect_run_locked(second.lock_run(&run()), &run())?;
    assert_eq!(
        second.append(&run(), steered(1, 0, b"stop note"))?,
        Appended::New { seq: 0 }
    );

    drop(held);

    let _taken = second.lock_run(&run())?;
    expect_run_locked(first.lock_run(&run()), &run())
}

// ---------------------------------------------------------------------------------------------------------------
// File-only run listing cases
// ---------------------------------------------------------------------------------------------------------------

/// Checks that `result` is `StoreError::Unsafe` naming `what`.
fn expect_unsafe(result: Result<BTreeSet<RunId>, StoreError>, what: &str) -> TestResult {
    match result {
        Err(StoreError::Unsafe { what: refused }) if refused == what => Ok(()),
        Err(other) => Err(format!("expected Unsafe naming the {what}, got {other}").into()),
        Ok(runs) => Err(format!("a foreign entry was listed as {} runs", runs.len()).into()),
    }
}

/// Creates the file at `path`, which must not exist, holding `bytes`: a planted entry the store did not write.
fn plant(path: impl AsRef<std::path::Path>, bytes: &[u8]) -> TestResult {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

/// The names of the entries directly under `path`, sorted.
fn entry_names(path: &std::path::Path) -> Result<BTreeSet<std::ffi::OsString>, Box<dyn Error>> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(path)? {
        names.insert(entry?.file_name());
    }
    Ok(names)
}

/// A file store with one appended run, so `runs/` exists, and the path of its `runs/` directory.
fn store_with_one_run(
    dir: &tempfile::TempDir,
) -> Result<(FileStore, std::path::PathBuf), Box<dyn Error>> {
    let store = FileStore::open(dir.path())?;
    let (started, record) = run_started(1_760_000_000);
    assert_eq!(store.append(&started, record)?, Appended::New { seq: 0 });
    Ok((store, dir.path().join("runs")))
}

#[test]
fn runs_of_a_fresh_file_store_creates_nothing() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    // Act
    let runs = store.runs()?;

    // Assert
    assert!(runs.is_empty());
    assert!(
        entry_names(dir.path())?.is_empty(),
        "listing runs must not create runs/"
    );
    Ok(())
}

#[test]
fn listing_a_locked_run_creates_nothing_in_it() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    drop(store.lock_run(&run())?);
    let run_dir = dir.path().join("runs").join(run().to_hex());
    let before = entry_names(&run_dir)?;

    // Act
    let runs = store.runs()?;

    // Assert
    assert!(runs.is_empty(), "a run that was only locked is not a run");
    assert_eq!(entry_names(&run_dir)?, before);
    Ok(())
}

#[test]
fn a_ledger_without_a_complete_line_is_not_listed() -> TestResult {
    // Arrange: one run whose ledger is empty and one whose ledger holds only a torn tail, which `read` reads as
    // zero lines.
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    let (listed, _record) = run_started(1_760_000_000);
    let (empty, _record) = run_started(1_760_000_003);
    let (torn, _record) = run_started(1_760_000_004);
    for (run, bytes) in [(empty, b"".as_slice()), (torn, b"{\"seq\":0".as_slice())] {
        let run_dir = runs_dir.join(run.to_hex());
        fs::create_dir(&run_dir)?;
        plant(run_dir.join("ledger.jsonl"), bytes)?;
    }

    // Act
    let runs = store.runs()?;

    // Assert
    assert!(store.read(&empty)?.lines().is_empty());
    assert_eq!(store.read(&torn)?.torn_tail(), 1);
    assert_eq!(runs, BTreeSet::from([listed]));
    Ok(())
}

#[test]
fn a_ledger_read_would_refuse_is_listed() -> TestResult {
    // Arrange: one run whose complete first line is not a ledger line, and one whose unterminated first line is
    // over the line cap, so neither can be a torn append; `read` refuses both, and listing them lets the refusal
    // reach a human.
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    let (listed, _record) = run_started(1_760_000_000);
    let (garbled, _record) = run_started(1_760_000_003);
    let (oversized, _record) = run_started(1_760_000_004);
    let over_cap =
        vec![b'x'; usize::try_from(minos_store::file::MAX_LEDGER_LINE_BYTES)?.saturating_add(1)];
    for (run, bytes) in [
        (garbled, b"not a line\n".as_slice()),
        (oversized, over_cap.as_slice()),
    ] {
        let run_dir = runs_dir.join(run.to_hex());
        fs::create_dir(&run_dir)?;
        plant(run_dir.join("ledger.jsonl"), bytes)?;
    }

    // Act
    let runs = store.runs()?;

    // Assert
    assert!(matches!(
        store.read(&garbled),
        Err(StoreError::Corrupt { .. })
    ));
    assert!(matches!(
        store.read(&oversized),
        Err(StoreError::Corrupt { .. })
    ));
    assert_eq!(runs, BTreeSet::from([listed, garbled, oversized]));
    Ok(())
}

#[test]
fn a_symlink_under_runs_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    let target = dir.path().join("elsewhere");
    fs::create_dir(&target)?;
    plant(target.join("ledger.jsonl"), b"")?;
    symlink(&target, runs_dir.join(other_run().to_hex()))?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "run directory")
}

#[test]
fn a_regular_file_under_runs_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    plant(runs_dir.join(other_run().to_hex()), b"not a directory")?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "run directory")
}

#[test]
fn a_directory_not_named_by_a_run_id_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    fs::create_dir(runs_dir.join("not-a-run"))?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "run directory")
}

#[test]
fn an_uppercase_run_directory_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    fs::create_dir(runs_dir.join(other_run().to_hex().to_uppercase()))?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "run directory")
}

#[test]
fn a_symlinked_ledger_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let (store, runs_dir) = store_with_one_run(&dir)?;
    let target = dir.path().join("elsewhere.jsonl");
    plant(&target, b"")?;
    let run_dir = runs_dir.join(other_run().to_hex());
    fs::create_dir(&run_dir)?;
    symlink(&target, run_dir.join("ledger.jsonl"))?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "ledger file")
}

#[test]
fn a_symlinked_runs_directory_is_refused() -> TestResult {
    // Arrange
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let target = dir.path().join("elsewhere");
    fs::create_dir(&target)?;
    symlink(&target, dir.path().join("runs"))?;

    // Act
    let result = store.runs();

    // Assert
    expect_unsafe(result, "runs directory")
}

fn is_send_sync<T: Send + Sync>() {}

#[test]
fn memory_store_is_send_and_sync() {
    is_send_sync::<MemoryStore>();
}

#[test]
fn file_store_is_send_and_sync() {
    is_send_sync::<FileStore>();
}

// ---------------------------------------------------------------------------------------------------------------
// First write wins
// ---------------------------------------------------------------------------------------------------------------

/// Appends one `Steered` record per `(generation, ordinal, note)` triple and checks every result against a model
/// in which the first record under each key wins: `New` at the next seq for a new key, `Duplicate` at the first
/// seq for an equal record, `Err(StoreError::Conflict)` at the first seq for a different one; then `read` returns
/// exactly one line per distinct key, in first-append order, with contiguous seq.
fn keeps_first_write<L: Ledger>(ledger: &L, ops: &[(u32, u32, u8)]) -> Result<(), TestCaseError> {
    let id = run();
    let mut first: BTreeMap<EventKey, (u64, Record)> = BTreeMap::new();
    let mut order: Vec<Record> = Vec::new();

    for &(generation, ordinal, note) in ops {
        let record = steered(generation, ordinal, &[note]);
        let key = record.event().key(&id);
        let result = ledger.append(&id, record.clone());
        match first.get(&key) {
            Some((seq, kept)) if *kept == record => {
                prop_assert_eq!(result?, Appended::Duplicate { seq: *seq });
            }
            Some((seq, _)) => {
                let conflict =
                    matches!(result, Err(StoreError::Conflict { seq: at }) if at == *seq);
                prop_assert!(
                    conflict,
                    "a different record under a stored key must be Conflict at the first seq"
                );
            }
            None => {
                let seq = u64::try_from(order.len())?;
                prop_assert_eq!(result?, Appended::New { seq });
                first.insert(key, (seq, record.clone()));
                order.push(record);
            }
        }
    }

    let read = ledger.read(&id)?;
    prop_assert_eq!(read.torn_tail(), 0);
    prop_assert_eq!(read.lines().len(), first.len());
    prop_assert_eq!(read.lines().len(), order.len());
    for ((expected_seq, line), record) in (0_u64..).zip(read.lines()).zip(&order) {
        prop_assert_eq!(line.seq(), expected_seq);
        prop_assert_eq!(line.record(), record);
        line.verify(&id)?;
    }
    Ok(())
}

/// Triples from a small key space (3 generations by 3 ordinals) and 3 notes, so duplicates and conflicts are
/// common.
fn ops() -> impl Strategy<Value = Vec<(u32, u32, u8)>> {
    prop::collection::vec((0_u32..3, 0_u32..3, 0_u8..3), 0..48)
}

proptest! {
    #[test]
    fn append_sequence_keeps_first_write(ops in ops()) {
        keeps_first_write(&MemoryStore::new(), &ops)?;
    }
}

proptest! {
    // Every file append ends in sync_data, which is F_FULLFSYNC on macOS, so the file arm runs fewer cases.
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn append_sequence_keeps_first_write_on_file(ops in ops()) {
        let dir = tempfile::tempdir()?;
        keeps_first_write(&FileStore::open(dir.path())?, &ops)?;
    }
}
