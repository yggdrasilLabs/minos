//! `minos_cli::address` on a `MemoryStore` (08-CONTEXT D-10; 08-RESEARCH Pitfall 8):
//!
//! - `HexPrefix` accepts 1 to 64 lowercase hex characters and refuses the empty string, uppercase, non-hex, and 65
//!   characters;
//! - `resolve_run` refuses a store with no runs, resolves a unique prefix, names both full IDs in order for an
//!   ambiguous one, refuses a prefix no run has, and with no prefix picks the latest `(started_at, RunId)`, listing
//!   runs whose ledgers do not fold and refusing when no run folds;
//! - `fold_run` refuses a run with no lines;
//! - `resolve_exp` resolves a prefix of the baseline of a run with only `RunStarted` and refuses another prefix.

use std::error::Error;

use minos_cli::address::{AddressError, HexPrefix, fold_run, resolve_exp, resolve_run};
use minos_core::id::{BlobId, ExperimentId, ManifestId, RunId, TreeId};
use minos_core::ledger::{Event, Record};
use minos_store::{Appended, Ledger, MemoryStore};

type TestResult = Result<(), Box<dyn Error>>;

/// The run of `manifest` started at `started_at` on the fixture tree, and its `RunStarted` record, with `run` and
/// `baseline` computed as the fold recomputes them.
fn run_started(manifest: &[u8], started_at: u64) -> (RunId, Record) {
    let manifest = ManifestId::of(manifest);
    let base = TreeId::of(b"address test tree");
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

/// Appends a started run to `store` and returns its ID.
fn start(store: &MemoryStore, manifest: &[u8], started_at: u64) -> Result<RunId, Box<dyn Error>> {
    let (run, record) = run_started(manifest, started_at);
    assert_eq!(store.append(&run, record)?, Appended::New { seq: 0 });
    Ok(run)
}

/// Two runs of one manifest whose IDs share their first two hex characters, earliest start times first.
fn twins() -> Result<(u64, u64), Box<dyn Error>> {
    let mut seen: Vec<(String, u64)> = Vec::new();
    for started_at in 0..10_000 {
        let hex = run_started(b"twins", started_at).0.to_hex();
        let head = hex.get(..2).ok_or("short hex")?.to_owned();
        if let Some((_, earlier)) = seen.iter().find(|(other, _)| *other == head) {
            return Ok((*earlier, started_at));
        }
        seen.push((head, started_at));
    }
    Err("no two runs share a two-character prefix".into())
}

/// The length of the common prefix of `a` and `b`.
fn common(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

#[test]
fn hex_prefixes_are_lowercase_hex_of_one_to_sixty_four_characters() -> TestResult {
    assert_eq!("ab".parse::<HexPrefix>()?.as_str(), "ab");
    let full = "0123456789abcdef".repeat(4);
    assert_eq!(full.parse::<HexPrefix>()?.as_str(), full);
    for bad in ["", "AB", "xyz", "0g", " ab"] {
        assert!(
            matches!(
                bad.parse::<HexPrefix>(),
                Err(AddressError::BadPrefix { .. })
            ),
            "{bad:?}"
        );
    }
    let long = format!("{full}0");
    assert!(matches!(
        long.parse::<HexPrefix>(),
        Err(AddressError::BadPrefix { .. })
    ));
    Ok(())
}

#[test]
fn a_store_with_no_runs_names_check_and_run() {
    let store = MemoryStore::new();
    let result = resolve_run(&store, None);
    assert!(matches!(result, Err(AddressError::NoRuns)));
    let text = result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(text.contains("minos check"), "{text}");
}

#[test]
fn prefixes_resolve_uniquely_or_name_every_candidate() -> TestResult {
    let (first_at, second_at) = twins()?;
    let store = MemoryStore::new();
    let first = start(&store, b"twins", first_at)?;
    let second = start(&store, b"twins", second_at)?;
    let (first_hex, second_hex) = (first.to_hex(), second.to_hex());
    let shared = common(&first_hex, &second_hex);

    // A prefix one character past the shared part names only the first run.
    let unique: HexPrefix = first_hex.get(..=shared).ok_or("short hex")?.parse()?;
    let resolved = resolve_run(&store, Some(&unique))?;
    assert_eq!(*resolved.run(), first);
    assert!(resolved.unreadable().is_empty());

    // The shared two characters name both, sorted, by their full IDs.
    let both: HexPrefix = first_hex.get(..2).ok_or("short hex")?.parse()?;
    match resolve_run(&store, Some(&both)) {
        Err(AddressError::Ambiguous {
            what, candidates, ..
        }) => {
            assert_eq!(what, "run");
            let mut expected = vec![first_hex.clone(), second_hex.clone()];
            expected.sort();
            assert_eq!(candidates, expected);
        }
        other => return Err(format!("expected Ambiguous, got {:?}", other.err()).into()),
    }
    let text = resolve_run(&store, Some(&both))
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        text.contains(&first_hex) && text.contains(&second_hex),
        "{text}"
    );

    // A two-character prefix neither run has names no run.
    let absent = (0..=255_u8)
        .map(|byte| format!("{byte:02x}"))
        .find(|head| {
            !first_hex.starts_with(head.as_str()) && !second_hex.starts_with(head.as_str())
        })
        .ok_or("every prefix is taken")?;
    let absent: HexPrefix = absent.parse()?;
    assert!(matches!(
        resolve_run(&store, Some(&absent)),
        Err(AddressError::NoRun { .. })
    ));
    Ok(())
}

#[test]
fn no_prefix_picks_the_latest_start_then_the_larger_run_id() -> TestResult {
    let store = MemoryStore::new();
    start(&store, b"early", 10)?;
    let late = start(&store, b"late", 20)?;
    assert_eq!(*resolve_run(&store, None)?.run(), late);

    let tied = start(&store, b"tied", 20)?;
    let expected = if tied > late { tied } else { late };
    assert_eq!(*resolve_run(&store, None)?.run(), expected);
    Ok(())
}

#[test]
fn runs_that_do_not_fold_are_listed_and_refused_when_alone() -> TestResult {
    let store = MemoryStore::new();
    let broken = RunId::of(&ManifestId::of(b"broken"), &TreeId::of(b"t"), 5);
    let appended = store.append(
        &broken,
        Record::v1(Event::Steered {
            generation: 1,
            ordinal: 0,
            note: BlobId::of(b"note"),
        }),
    )?;
    assert_eq!(appended, Appended::New { seq: 0 });
    match resolve_run(&store, None) {
        Err(AddressError::Unreadable { runs }) => assert_eq!(runs, vec![broken]),
        other => return Err(format!("expected Unreadable, got {:?}", other.err()).into()),
    }

    let good = start(&store, b"good", 1)?;
    let resolved = resolve_run(&store, None)?;
    assert_eq!(*resolved.run(), good);
    assert_eq!(resolved.unreadable(), [broken]);
    Ok(())
}

#[test]
fn fold_run_refuses_a_run_with_no_lines() {
    let store = MemoryStore::new();
    let (run, _) = run_started(b"never appended", 1);
    assert!(matches!(
        fold_run(&store, &run),
        Err(AddressError::EmptyRun { .. })
    ));
}

#[test]
fn experiments_resolve_against_the_baseline() -> TestResult {
    let store = MemoryStore::new();
    let run = start(&store, b"experiments", 7)?;
    let state = fold_run(&store, &run)?;
    let baseline = *state.baseline();
    let hex = baseline.to_hex();

    let prefix: HexPrefix = hex.get(..6).ok_or("short hex")?.parse()?;
    assert_eq!(resolve_exp(&state, &prefix)?, baseline);

    let other = if hex.starts_with('0') { "1" } else { "0" };
    let other: HexPrefix = other.parse()?;
    assert!(matches!(
        resolve_exp(&state, &other),
        Err(AddressError::NoExperiment { .. })
    ));
    Ok(())
}
