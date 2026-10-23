//! The file ledger on disk (STORE-02, STORE-03; 04-CONTEXT D-11 to D-13): torn tails, every corruption kind,
//! the golden bytes, append-only growth, concurrent writers, symlink refusals, and parity with the memory store.
//!
//! Crash and corruption cases are built by writing raw bytes into the ledger file through `std::fs::OpenOptions`
//! (the crate's clippy bans on `File::create` and `fs::write` apply to tests too). Each refusal test asserts the
//! exact `StoreError::Corrupt` kind and 1-based line, for `read` and for `append`, and that the refused append
//! left the file's bytes unchanged. The golden-byte test ties the file format to core's frozen `line_v1`
//! snapshot (`crates/minos-core/tests/snapshots/golden_wire__line_v1.snap`).

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::thread;

use minos_core::id::{BlobId, ExperimentId, ManifestId, RunId, TreeId};
use minos_core::ledger::{Event, Line, Record};
use minos_store::file::MAX_LEDGER_LINE_BYTES;
use minos_store::{Appended, CorruptKind, FileStore, Ledger, MemoryStore, StoreError};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The start time of the golden run.
const STARTED_AT: u64 = 1_760_000_000;

/// The golden run of `crates/minos-core/tests/golden_wire.rs`: manifest `m`, base tree `t`.
fn run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), STARTED_AT)
}

/// Another run: the same manifest and tree, started a second later.
fn other_run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), STARTED_AT + 1)
}

/// The golden `RunStarted` record, built exactly as core's `golden_line`.
fn run_started() -> Record {
    Record::v1(Event::RunStarted {
        run: run(),
        manifest: ManifestId::of(b"m"),
        base: TreeId::of(b"t"),
        baseline: ExperimentId::baseline(&TreeId::of(b"t")),
        started_at: STARTED_AT,
        root_image: None,
    })
}

/// A `Steered` record, keyed by generation and ordinal only.
fn steered(generation: u32, ordinal: u32, note: &[u8]) -> Record {
    Record::v1(Event::Steered {
        generation,
        ordinal,
        note: BlobId::of(note),
    })
}

/// `<root>/runs/<run hex>/ledger.jsonl`.
fn ledger_path(root: &Path, run: &RunId) -> PathBuf {
    root.join("runs").join(run.to_hex()).join("ledger.jsonl")
}

/// The ledger file's bytes, or none when it does not exist.
fn ledger_bytes(root: &Path, run: &RunId) -> io::Result<Vec<u8>> {
    match fs::read(ledger_path(root, run)) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Replaces the file at `path` with `bytes`, creating its directory.
fn write_raw(path: &Path, bytes: &[u8]) -> TestResult {
    fs::create_dir_all(path.parent().ok_or("a ledger path has a parent")?)?;
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

/// Appends `bytes` to the existing file at `path`.
fn append_raw(path: &Path, bytes: &[u8]) -> TestResult {
    OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

/// One ledger line as the store writes it: the serialized `Line` and its newline.
fn encoded(seq: u64, run: &RunId, record: Record) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = serde_json::to_vec(&Line::new(seq, run, record))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// `encoded`, with the JSON object at `path` (a list of keys from the line's root) given `field` = `value`.
fn edited(
    seq: u64,
    record: Record,
    path: &[&str],
    field: &str,
    value: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut json: serde_json::Value = serde_json::to_value(Line::new(seq, &run(), record))?;
    let mut object = &mut json;
    for key in path {
        object = object
            .as_object_mut()
            .ok_or("a line is a JSON object")?
            .get_mut(*key)
            .ok_or("the edited path exists")?;
    }
    object
        .as_object_mut()
        .ok_or("the edited path is a JSON object")?
        .insert(field.to_owned(), serde_json::Value::from(value));
    let mut bytes = serde_json::to_vec(&json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The concatenation of `lines`.
fn joined(lines: &[Vec<u8>]) -> Vec<u8> {
    lines.concat()
}

/// Asserts that `result` is `StoreError::Corrupt` for the golden run at 1-based `line` with `kind`.
fn expect_corrupt<T>(result: Result<T, StoreError>, line: u64, kind: CorruptKind) -> TestResult {
    match result {
        Err(StoreError::Corrupt {
            run: found_run,
            line: found_line,
            kind: found_kind,
        }) => {
            assert_eq!(found_run, run());
            assert_eq!(found_line, line);
            assert_eq!(found_kind, kind);
            Ok(())
        }
        Err(other) => Err(format!("expected Corrupt {kind:?} at line {line}, got {other}").into()),
        Ok(_) => Err(format!("expected Corrupt {kind:?} at line {line}, got Ok").into()),
    }
}

/// Writes `bytes` as the golden run's ledger and asserts that `read` and `append` both refuse it as `kind` at
/// `line`, and that the refused append left the bytes unchanged.
fn assert_refused(bytes: &[u8], line: u64, kind: CorruptKind) -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    write_raw(&ledger_path(dir.path(), &run()), bytes)?;

    expect_corrupt(store.read(&run()), line, kind)?;
    expect_corrupt(store.append(&run(), steered(9, 9, b"late")), line, kind)?;

    assert_eq!(ledger_bytes(dir.path(), &run())?, bytes);
    Ok(())
}

/// The permission bits of `path`.
fn mode(path: &Path) -> io::Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

// ---------------------------------------------------------------------------------------------------------------
// Golden bytes
// ---------------------------------------------------------------------------------------------------------------

/// The ledger file after one golden append is core's `line_v1` plus a newline. Recipe for the pin:
/// `awk 'n >= 2 { print } /^---$/ { n++ }' golden_wire__line_v1.snap | shasum -a 256`.
#[test]
fn first_line_matches_core_golden_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    assert_eq!(
        store.append(&run(), run_started())?,
        Appended::New { seq: 0 }
    );

    let bytes = ledger_bytes(dir.path(), &run())?;
    assert_eq!(bytes.len(), 445);
    assert_eq!(
        BlobId::of(&bytes).to_hex(),
        "c595be4ad19f70c20e7dd36568d66513721c9c5658908d3b7f93fd933b731909"
    );
    assert_eq!(bytes, encoded(0, &run(), run_started())?);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Torn tails
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn torn_tail_without_newline_is_tolerated() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    for ordinal in 0..3 {
        assert_eq!(
            store.append(&run(), steered(1, ordinal, b"note"))?,
            Appended::New {
                seq: u64::from(ordinal)
            }
        );
    }
    let complete = ledger_bytes(dir.path(), &run())?;
    let fourth = encoded(3, &run(), steered(1, 3, b"note"))?;
    let half = fourth
        .get(..fourth.len() / 2)
        .ok_or("half of a line is in range")?;
    append_raw(&ledger_path(dir.path(), &run()), half)?;
    let torn = ledger_bytes(dir.path(), &run())?;

    let read = store.read(&run())?;

    assert_eq!(read.lines().len(), 3);
    assert_eq!(read.torn_tail(), 1);
    assert_eq!(
        ledger_bytes(dir.path(), &run())?,
        torn,
        "read never truncates"
    );

    assert_eq!(
        store.append(&run(), steered(1, 3, b"note"))?,
        Appended::New { seq: 3 }
    );
    let healed = store.read(&run())?;
    assert_eq!(healed.lines().len(), 4);
    assert_eq!(healed.torn_tail(), 0);
    assert_eq!(
        ledger_bytes(dir.path(), &run())?,
        joined(&[complete, fourth])
    );
    Ok(())
}

#[test]
fn a_duplicate_append_still_truncates_the_torn_tail() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    assert_eq!(
        store.append(&run(), steered(1, 0, b"note"))?,
        Appended::New { seq: 0 }
    );
    let complete = ledger_bytes(dir.path(), &run())?;
    append_raw(&ledger_path(dir.path(), &run()), b"{\"seq\":1")?;

    assert_eq!(
        store.append(&run(), steered(1, 0, b"note"))?,
        Appended::Duplicate { seq: 0 }
    );

    assert_eq!(ledger_bytes(dir.path(), &run())?, complete);
    assert_eq!(store.read(&run())?.torn_tail(), 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn invalid_json_mid_file_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        b"not a ledger line\n".to_vec(),
        encoded(2, &run(), steered(1, 2, b"note"))?,
    ]);
    assert_refused(&bytes, 2, CorruptKind::BadJson)
}

#[test]
fn final_valid_json_that_is_not_a_line_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        b"{\"seq\":1}\n".to_vec(),
    ]);
    assert_refused(&bytes, 2, CorruptKind::BadJson)
}

/// An append writes its line and newline in one `write_all`, so a crash cannot leave a newline-terminated
/// non-line, and truncating one could erase a line already reported durable (review WR-01).
#[test]
fn a_newline_terminated_final_line_that_is_not_a_line_is_refused() -> TestResult {
    let two = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        encoded(1, &run(), steered(1, 1, b"note"))?,
        b"{\"seq\":2,\"key\":\"\n".to_vec(),
    ]);
    assert_refused(&two, 3, CorruptKind::BadJson)?;

    let blank = joined(&[encoded(0, &run(), steered(1, 0, b"note"))?, b"\n".to_vec()]);
    assert_refused(&blank, 2, CorruptKind::BadJson)?;

    let zeroed = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        b"\0\0\0\0\n".to_vec(),
    ]);
    assert_refused(&zeroed, 2, CorruptKind::BadJson)
}

/// An append writes its line and newline in one `write_all`, so a crash cannot leave a newline-terminated
/// non-line, and truncating one could erase a line already reported durable, even on a duplicate append
/// (review WR-01).
#[test]
fn a_duplicate_append_never_erases_a_damaged_final_line() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        b"{\"seq\":1,\"key\":\"\n".to_vec(),
    ]);
    write_raw(&ledger_path(dir.path(), &run()), &bytes)?;

    expect_corrupt(
        store.append(&run(), steered(1, 0, b"note")),
        2,
        CorruptKind::BadJson,
    )?;

    assert_eq!(ledger_bytes(dir.path(), &run())?, bytes);
    Ok(())
}

#[test]
fn an_edited_key_is_refused() -> TestResult {
    let other_key = steered(7, 7, b"other").event().key(&run()).to_hex();
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        edited(1, steered(1, 1, b"note"), &[], "key", &other_key)?,
    ]);
    assert_refused(&bytes, 2, CorruptKind::KeyMismatch)
}

#[test]
fn an_edited_record_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        edited(
            1,
            steered(1, 1, b"note"),
            &["record"],
            "steered",
            "not an event",
        )?,
    ]);
    // The edited `steered` payload is no longer an event, so the line does not parse at all.
    assert_refused(&bytes, 2, CorruptKind::BadJson)
}

#[test]
fn a_line_from_another_run_is_refused() -> TestResult {
    let bytes = encoded(0, &other_run(), steered(1, 0, b"note"))?;
    assert_refused(&bytes, 1, CorruptKind::KeyMismatch)
}

#[test]
fn a_seq_gap_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        encoded(2, &run(), steered(1, 1, b"note"))?,
    ]);
    assert_refused(
        &bytes,
        2,
        CorruptKind::SeqGap {
            expected: 1,
            found: 2,
        },
    )
}

#[test]
fn a_ledger_not_starting_at_seq_zero_is_refused() -> TestResult {
    let bytes = encoded(1, &run(), steered(1, 0, b"note"))?;
    assert_refused(
        &bytes,
        1,
        CorruptKind::SeqGap {
            expected: 0,
            found: 1,
        },
    )
}

#[test]
fn an_unknown_version_mid_file_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        edited(1, steered(1, 1, b"note"), &["record"], "v", "2")?,
        encoded(2, &run(), steered(1, 2, b"note"))?,
    ]);
    assert_refused(&bytes, 2, CorruptKind::UnknownVersion)
}

#[test]
fn an_unknown_version_as_the_final_line_is_refused() -> TestResult {
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        edited(1, steered(1, 1, b"note"), &["record"], "v", "2")?,
    ]);
    assert_refused(&bytes, 2, CorruptKind::UnknownVersion)
}

#[test]
fn a_line_over_the_cap_is_refused() -> TestResult {
    let length = usize::try_from(MAX_LEDGER_LINE_BYTES)? + 1;
    let mut long = vec![b'x'; length];
    long.push(b'\n');
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        long,
        encoded(2, &run(), steered(1, 2, b"note"))?,
    ]);
    assert_refused(
        &bytes,
        2,
        CorruptKind::LineTooLong {
            bytes: MAX_LEDGER_LINE_BYTES + 1,
        },
    )
}

#[test]
fn unterminated_bytes_over_the_line_cap_are_refused() -> TestResult {
    let length = usize::try_from(MAX_LEDGER_LINE_BYTES)? + 1;
    let bytes = joined(&[
        encoded(0, &run(), steered(1, 0, b"note"))?,
        vec![b'{'; length],
    ]);
    assert_refused(
        &bytes,
        2,
        CorruptKind::LineTooLong {
            bytes: MAX_LEDGER_LINE_BYTES + 1,
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn open_refuses_symlinked_root() -> TestResult {
    let dir = tempfile::tempdir()?;
    let real = dir.path().join("real");
    fs::create_dir(&real)?;
    let link = dir.path().join("link");
    symlink(&real, &link)?;

    let result = FileStore::open(&link);

    assert!(
        matches!(result, Err(StoreError::Unsafe { what: "store root" })),
        "a symlinked root must be Unsafe"
    );
    Ok(())
}

#[test]
fn open_refuses_a_root_that_is_a_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("file");
    write_raw(&file, b"not a directory")?;

    let result = FileStore::open(&file);

    assert!(
        matches!(result, Err(StoreError::Unsafe { what: "store root" })),
        "a file root must be Unsafe"
    );
    Ok(())
}

#[test]
fn open_creates_a_missing_root_with_mode_0700() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("project").join(minos_store::STORE_DIR);

    let store = FileStore::open(&root)?;
    assert_eq!(
        store.append(&run(), steered(1, 0, b"note"))?,
        Appended::New { seq: 0 }
    );

    assert_eq!(mode(&root)?, 0o700);
    assert_eq!(mode(&root.join("runs"))?, 0o700);
    assert_eq!(mode(&root.join("runs").join(run().to_hex()))?, 0o700);
    Ok(())
}

#[test]
fn reading_an_unknown_run_creates_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    let read = store.read(&run())?;

    assert!(read.lines().is_empty());
    assert_eq!(read.torn_tail(), 0);
    assert_eq!(fs::read_dir(dir.path())?.count(), 0);

    assert_eq!(
        store.append(&run(), steered(1, 0, b"note"))?,
        Appended::New { seq: 0 }
    );
    assert!(store.read(&other_run())?.lines().is_empty());
    assert!(!dir.path().join("runs").join(other_run().to_hex()).exists());
    Ok(())
}

#[test]
fn a_symlinked_ledger_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    assert_eq!(
        store.append(&run(), steered(1, 0, b"note"))?,
        Appended::New { seq: 0 }
    );
    let path = ledger_path(dir.path(), &run());
    let elsewhere = dir.path().join("elsewhere.jsonl");
    fs::rename(&path, &elsewhere)?;
    symlink(&elsewhere, &path)?;
    let before = fs::read(&elsewhere)?;

    let read = store.read(&run());
    let append = store.append(&run(), steered(1, 1, b"note"));

    assert!(
        matches!(
            read,
            Err(StoreError::Unsafe {
                what: "ledger file"
            })
        ),
        "read must refuse a symlinked ledger"
    );
    assert!(
        matches!(
            append,
            Err(StoreError::Unsafe {
                what: "ledger file"
            })
        ),
        "append must refuse a symlinked ledger"
    );
    assert_eq!(fs::read(&elsewhere)?, before);
    Ok(())
}

#[test]
fn a_symlinked_run_directory_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere)?;
    fs::create_dir(dir.path().join("runs"))?;
    symlink(&elsewhere, dir.path().join("runs").join(run().to_hex()))?;

    let read = store.read(&run());
    let append = store.append(&run(), steered(1, 0, b"note"));

    assert!(
        matches!(
            read,
            Err(StoreError::Unsafe {
                what: "run directory"
            })
        ),
        "read must refuse a symlinked run directory"
    );
    assert!(
        matches!(
            append,
            Err(StoreError::Unsafe {
                what: "run directory"
            })
        ),
        "append must refuse a symlinked run directory"
    );
    assert_eq!(fs::read_dir(&elsewhere)?.count(), 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn concurrent_appends_are_serialized() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();

    let results: Vec<Result<Vec<Appended>, String>> = thread::scope(|scope| {
        let handles: Vec<_> = (0_u32..4)
            .map(|writer| {
                scope.spawn(move || -> Result<Vec<Appended>, StoreError> {
                    let store = FileStore::open(root)?;
                    (0_u32..25)
                        .map(|ordinal| store.append(&run(), steered(writer, ordinal, b"note")))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| match handle.join() {
                Ok(Ok(appended)) => Ok(appended),
                Ok(Err(error)) => Err(error.to_string()),
                Err(_panic) => Err("a writer thread panicked".to_owned()),
            })
            .collect()
    });
    for result in results {
        for appended in result? {
            assert!(
                matches!(appended, Appended::New { .. }),
                "every distinct record is new"
            );
        }
    }

    let read = FileStore::open(root)?.read(&run())?;
    assert_eq!(read.torn_tail(), 0);
    assert_eq!(read.lines().len(), 100);
    for (expected_seq, line) in (0_u64..).zip(read.lines()) {
        assert_eq!(line.seq(), expected_seq);
    }
    let keys: BTreeSet<_> = read.lines().iter().map(Line::key).collect();
    assert_eq!(keys.len(), 100);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------------------------

/// Triples from a small key space (3 generations by 3 ordinals) and 3 notes, so duplicates and conflicts are
/// common.
fn ops() -> impl Strategy<Value = Vec<(u32, u32, u8)>> {
    prop::collection::vec((0_u32..3, 0_u32..3, 0_u8..3), 1..40)
}

/// What one append did, compared across stores: a store error other than a conflict is `Other`, because the two
/// stores need not fail the same way, and a differential property that saw one fails its case.
#[derive(Debug, PartialEq, Eq)]
enum Projected {
    New(u64),
    Duplicate(u64),
    Conflict(u64),
    Other,
}

fn project(result: Result<Appended, StoreError>) -> Projected {
    match result {
        Ok(Appended::New { seq }) => Projected::New(seq),
        Ok(Appended::Duplicate { seq }) => Projected::Duplicate(seq),
        Err(StoreError::Conflict { seq }) => Projected::Conflict(seq),
        Err(_) => Projected::Other,
    }
}

/// Appends with a run choice, so the differential property covers more than one ledger.
fn run_ops() -> impl Strategy<Value = Vec<(bool, u32, u32, u8)>> {
    prop::collection::vec((any::<bool>(), 0_u32..3, 0_u32..3, 0_u8..3), 1..40)
}

proptest! {
    // Every file append ends in sync_data, which is F_FULLFSYNC on macOS, so these properties run fewer cases.
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn append_only_prefix(ops in ops()) {
        let dir = tempfile::tempdir()?;
        let store = FileStore::open(dir.path())?;
        for (generation, ordinal, note) in ops {
            let before = ledger_bytes(dir.path(), &run())?;
            let result = store.append(&run(), steered(generation, ordinal, &[note]));
            let after = ledger_bytes(dir.path(), &run())?;
            prop_assert!(after.starts_with(&before), "an append only adds bytes at the end");
            match result {
                Ok(Appended::New { .. }) => prop_assert!(after.len() > before.len()),
                Ok(Appended::Duplicate { .. }) | Err(StoreError::Conflict { .. }) => {
                    prop_assert_eq!(after, before);
                }
                Err(other) => prop_assert!(false, "an append failed: {}", other),
            }
        }
    }

    #[test]
    fn memory_and_file_agree(ops in run_ops()) {
        let dir = tempfile::tempdir()?;
        let file = FileStore::open(dir.path())?;
        let memory = MemoryStore::new();
        for (first, generation, ordinal, note) in ops {
            let id = if first { run() } else { other_run() };
            let record = steered(generation, ordinal, &[note]);
            let from_file = project(file.append(&id, record.clone()));
            prop_assert_ne!(&from_file, &Projected::Other, "a file append failed");
            prop_assert_eq!(from_file, project(memory.append(&id, record)));
        }
        for id in [run(), other_run()] {
            prop_assert_eq!(file.read(&id)?, memory.read(&id)?);
        }
    }
}
