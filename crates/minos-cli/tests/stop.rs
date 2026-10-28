//! `minos_cli::stop::stop` over a temporary project's `FileStore`, on runs driven by `minos_cli::run::run` with the
//! sweep tier and a fake job runner, so no test boots a VM or calls a model (CLI-10, COST-02; 08-CONTEXT D-13).
//!
//! - A run no driver holds: `stop` is `StopError::NoDriver`, whose text is `no driver holds run <id>; nothing to
//!   stop`, and the ledger's bytes are unchanged.
//! - While a second `FileStore` handle holds the run's lock: the first `stop` is `Recorded { ordinal: 0 }`, the
//!   second `Recorded { ordinal: 1 }`, and the ledger holds two `UserStop` lines at the working generation. The
//!   output says the driver stops at its `next safe point` and `never kills a VM`.
//! - A store whose first append is raced by an equal append (another `stop`): `AlreadyRecorded`.
//! - A prefix that names no run is `AddressError::NoRun`, and probing it creates no run.
//! - A store with no runs is `AddressError::NoRuns`.
//! - A driver session whose fake job calls `stop` through another handle during a generation-1 trial ends
//!   `RunEnded::Stopped { reason: UserStop, .. }` once that trial is recorded.
//!
//! Every captured output and every provoked error's text passes [`assert_no_banned_words`]: no `estimate`, and no
//! whole word `exact` (COST-02).

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use minos_cli::address::{AddressError, HexPrefix};
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{Prepared, prepare};
use minos_cli::run::{RunEnded, RunOptions, run};
use minos_cli::stop::{StopError, StopOutcome, stop};
use minos_core::id::{BlobId, ImageDigest, MetricName, RunId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, EXPECTED_KRUN_LAYERS, Event, Isolation, Record, StopReason,
};
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::usage::Usage;
use minos_loop::{DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, Proposer};
use minos_source::CleanPolicy;
use minos_store::{
    Appended, Blobs, FileRunGuard, FileStore, Ledger, LedgerRead, RunLock, STORE_DIR, StoreError,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The run's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// One writable file with a sweep param over its `value = ` line, a sweep proposer, and an objective at `5%`. The
/// judge argv is a bare word, so it names no project path.
const MANIFEST: &str = r#"[project]
name = "stopping"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/value.txt"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["score"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "probe_score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 2

[[param]]
name = "value"
file = "src/value.txt"
anchor = "value = "
min = 0
max = 100
step = 1

[[proposer]]
kind = "sweep"
batch = 2
"#;

/// The Dockerfile the lock image entry is written for.
const DOCKERFILE: &[u8] = b"FROM scratch\n";

/// The fake guest binary the lock image entry is written for.
const GUEST: &[u8] = b"a fake minos-guest\n";

/// A `prices.toml` that prices compute and one model.
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

/// A temporary project beside its judge data, guest, and prices file, ready to prepare.
struct Fixture {
    _dir: TempDir,
    project: PathBuf,
    guest: PathBuf,
    prices: PathBuf,
}

impl Fixture {
    /// Writes the project, pushes its judge data, stores a root image, and writes the lock's image entry.
    fn new() -> Fallible<Self> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        write(&project, MANIFEST_FILE, MANIFEST.as_bytes())?;
        write(&project, "program.md", b"Raise the score.\n")?;
        write(&project, "minos.Dockerfile", DOCKERFILE)?;
        write(&project, "src/value.txt", b"value = 5\n")?;
        write(&project, "src/lib.rs", b"pub fn f() {}\n")?;
        let data = root.join("judge-data");
        write(&data, "expected.txt", b"stop test judge data\n")?;
        let guest = root.join("minos-guest");
        fs::write(&guest, GUEST)?;
        let prices = root.join("prices.toml");
        fs::write(&prices, PRICES)?;

        judge_push(&project, &data, JudgePushOptions::new(false))?;
        let store = FileStore::open(&project.join(STORE_DIR))?;
        let root_image = ImageDigest::from_blob_id(&store.put(b"a root image")?);
        let lock = read_lock(&project.join(LOCK_FILE))?.ok_or("judge push wrote no lock")?;
        let lock = lock.with_image(LockedImage::new(
            BackendKind::Krun,
            Arch::Arm64,
            root_image,
            BlobId::of(DOCKERFILE),
            Vec::new(),
            BlobId::of(GUEST),
            BuilderId::new(
                BuilderText::try_from("test-builder".to_owned())?,
                BuilderText::try_from("1".to_owned())?,
            ),
        ));
        write_lock(
            &project.join(STORE_DIR),
            &project.join(LOCK_FILE),
            lock.to_toml()?.as_bytes(),
        )?;
        Ok(Self {
            _dir: dir,
            project,
            guest,
            prices,
        })
    }

    /// `prepare` on the fixture, with the clean check skipped (the temporary project is not a repository).
    fn prepare(&self) -> Fallible<Prepared> {
        Ok(prepare(
            &self.project,
            &self.guest,
            &self.prices,
            CleanPolicy::AllowDirty,
        )?)
    }

    /// The path of `run`'s ledger file.
    fn ledger_path(&self, run: &RunId) -> PathBuf {
        self.project
            .join(STORE_DIR)
            .join("runs")
            .join(run.to_hex())
            .join("ledger.jsonl")
    }

    /// Another handle on the fixture's store.
    fn second_store(&self) -> Fallible<FileStore> {
        Ok(FileStore::open(&self.project.join(STORE_DIR))?)
    }
}

/// Writes `bytes` to `dir/rel`, creating parent directories.
fn write(dir: &Path, rel: &str, bytes: &[u8]) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The fake job runner
// ---------------------------------------------------------------------------------------------------------------

/// A [`JobRunner`] that answers trials from `run`; its preflight always passes.
struct FakeJobs<F> {
    run: F,
}

impl<F> JobRunner for FakeJobs<F>
where
    F: Fn(&JobRequest) -> Result<JobResult, JobFailure>,
{
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        (self.run)(request)
    }

    fn check(&self, _request: &JobRequest) -> Result<(), JobFailure> {
        Ok(())
    }
}

/// A scored result for `request`: the gate passes and the objective is `value`; both VMs ran.
fn scored(request: &JobRequest, value: f64) -> Result<JobResult, JobFailure> {
    let invalid = JobFailure::Config {
        what: "the fake scorecard is invalid",
    };
    let name = |text: &str| MetricName::from_str(text).map_err(|_| invalid);
    let metric = |number: f64| Metric::new(number).map_err(|_| invalid);
    let card = Scorecard::from_entries([
        (name("identical_output")?, metric(1.0)?, false),
        (name("probe_score")?, metric(value)?, false),
    ])
    .map_err(|_| invalid)?;
    JobResult::new(
        request,
        Outcome::Scored(card),
        Usage::compute(1_000, 2_048_000),
        Some(Usage::compute(500, 1_024_000)),
        BackendInfo::new(
            BackendKind::Krun,
            Arch::Arm64,
            Isolation::Hardware,
            EXPECTED_KRUN_LAYERS.iter().copied().collect(),
            ImageDigest::of(b"root image"),
            Some(ImageDigest::of(b"kernel")),
        ),
        std::collections::BTreeMap::new(),
    )
    .map_err(|_| JobFailure::Config {
        what: "the fake result is invalid",
    })
}

/// 1.0 for the baseline and 2.0 for every candidate.
fn constant(request: &JobRequest) -> Result<JobResult, JobFailure> {
    scored(request, if request.is_baseline() { 1.0 } else { 2.0 })
}

/// Drives a new sweep run of `prepared` to `generations` with `jobs`, and returns how it ended.
fn drive<J: JobRunner>(
    prepared: &Prepared,
    jobs: &J,
    generations: u32,
) -> Fallible<(RunId, RunEnded)> {
    let mut out = Vec::new();
    let outcome = run(
        prepared,
        &Proposer::<FakeProvider>::without_provider(),
        jobs,
        RunOptions::new(generations, None),
        STARTED_AT,
        DriverOptions {
            retry_pause: Duration::ZERO,
        },
        &mut out,
    )?;
    Ok((*outcome.run(), *outcome.ended()))
}

/// A run of `prepared` driven to generation 1, which no driver holds afterwards.
fn finished(prepared: &Prepared) -> Fallible<RunId> {
    Ok(drive(prepared, &FakeJobs { run: constant }, 1)?.0)
}

// ---------------------------------------------------------------------------------------------------------------
// Capturing and checking
// ---------------------------------------------------------------------------------------------------------------

/// No `estimate` anywhere, and no `exact` as a whole word (`exactly` passes), in `text` lowercased (COST-02).
fn assert_no_banned_words(text: &str) -> TestResult {
    let lower = text.to_lowercase();
    if lower.contains("estimate") {
        return Err(format!("the text contains the word estimate: {text}").into());
    }
    if lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "exact")
    {
        return Err(format!("the text contains the word exact: {text}").into());
    }
    Ok(())
}

/// `stop` on `store` for `run`: its outcome and everything it printed. The caller checks the text.
fn stop_text<S: Ledger + RunLock>(
    store: &S,
    run: Option<&HexPrefix>,
) -> Result<(StopOutcome, String), StopError> {
    let mut out = Vec::new();
    let outcome = stop(store, run, &mut out)?;
    Ok((outcome, String::from_utf8_lossy(&out).into_owned()))
}

/// The full hex of `run`, as a prefix.
fn prefix_of(run: &RunId) -> Fallible<HexPrefix> {
    Ok(HexPrefix::from_str(&run.to_hex())?)
}

/// The first 12 hex characters of `run`.
fn short(run: &RunId) -> String {
    run.to_hex().chars().take(12).collect()
}

/// Every `StopRequested` of `run`, as generation, reason, and ordinal, in ledger order.
fn stops_of(store: &FileStore, run: &RunId) -> Fallible<Vec<(u32, StopReason, Option<u32>)>> {
    let read = store.read(run)?;
    Ok(read
        .records()
        .filter_map(|record| match record {
            Record::V1(Event::StopRequested {
                generation,
                reason,
                ordinal,
            }) => Some((*generation, *reason, *ordinal)),
            _ => None,
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------------------------
// A store whose first append is raced by an equal one
// ---------------------------------------------------------------------------------------------------------------

/// Forwards to a [`FileStore`], but appends the first record it is given once before appending it again, as a
/// `stop` racing this one would, so that append is a `Duplicate`.
struct RacingStore {
    inner: FileStore,
    raced: Cell<bool>,
}

impl Ledger for RacingStore {
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError> {
        if !self.raced.replace(true) {
            let _first = self.inner.append(run, record.clone())?;
        }
        self.inner.append(run, record)
    }

    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError> {
        self.inner.read(run)
    }

    fn runs(&self) -> Result<std::collections::BTreeSet<RunId>, StoreError> {
        self.inner.runs()
    }
}

impl RunLock for RacingStore {
    type Guard<'a> = FileRunGuard;

    fn lock_run(&self, run: &RunId) -> Result<FileRunGuard, StoreError> {
        self.inner.lock_run(run)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The cases
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn stop_refuses_when_no_driver_holds_the_run_and_appends_nothing() -> TestResult {
    // Arrange: a run whose session ended.
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = finished(&prepared)?;
    let before = fs::read(fixture.ledger_path(&run))?;

    // Act
    let mut out = Vec::new();
    let result = stop(prepared.store(), None, &mut out);

    // Assert
    let Err(error) = result else {
        return Err("stop recorded a stop no driver could honour".into());
    };
    assert!(
        matches!(&error, StopError::NoDriver { run: named } if *named == run),
        "{error}"
    );
    let text = error.to_string();
    assert_eq!(
        text,
        format!("no driver holds run {}; nothing to stop", run.to_hex())
    );
    assert_no_banned_words(&text)?;
    assert!(out.is_empty());
    assert_eq!(fs::read(fixture.ledger_path(&run))?, before);
    // The probe released the run: it can be locked again.
    drop(prepared.store().lock_run(&run)?);
    Ok(())
}

#[test]
fn each_stop_while_a_driver_holds_the_run_records_the_next_ordinal() -> TestResult {
    // Arrange: a run driven to generation 1, held by another handle as a driver would hold it.
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = finished(&prepared)?;
    let holder = fixture.second_store()?;
    let guard = holder.lock_run(&run)?;
    let prefix = prefix_of(&run)?;

    // Act
    let (first, first_text) = stop_text(prepared.store(), Some(&prefix))?;
    let (second, second_text) = stop_text(prepared.store(), None)?;
    drop(guard);

    // Assert: generation 1 concluded, so the working generation is 2.
    assert_eq!(
        first,
        StopOutcome::Recorded {
            run,
            generation: 2,
            ordinal: 0
        }
    );
    assert_eq!(
        second,
        StopOutcome::Recorded {
            run,
            generation: 2,
            ordinal: 1
        }
    );
    assert_eq!(
        stops_of(prepared.store(), &run)?,
        [
            (2, StopReason::UserStop, Some(0)),
            (2, StopReason::UserStop, Some(1)),
        ]
    );
    for text in [&first_text, &second_text] {
        assert!(text.contains("next safe point"), "{text}");
        assert!(text.contains("never kills a VM"), "{text}");
        assert!(text.contains("has no effect"), "{text}");
        assert_no_banned_words(text)?;
    }
    assert!(
        first_text.contains(&format!(
            "stop recorded for run {} at generation 2 (ordinal 0)",
            short(&run)
        )),
        "{first_text}"
    );
    assert!(second_text.contains("(ordinal 1)"), "{second_text}");
    Ok(())
}

#[test]
fn a_raced_stop_is_reported_as_already_recorded() -> TestResult {
    // Arrange: a held run, and a store whose first append is raced by an equal one.
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = finished(&prepared)?;
    let holder = fixture.second_store()?;
    let guard = holder.lock_run(&run)?;
    let racing = RacingStore {
        inner: fixture.second_store()?,
        raced: Cell::new(false),
    };

    // Act
    let (outcome, text) = stop_text(&racing, None)?;
    drop(guard);

    // Assert: one line was written, by the racing append, and this stop says so.
    assert_eq!(
        outcome,
        StopOutcome::AlreadyRecorded {
            run,
            generation: 2,
            ordinal: 0
        }
    );
    assert_eq!(
        stops_of(prepared.store(), &run)?,
        [(2, StopReason::UserStop, Some(0))]
    );
    assert!(
        text.contains(&format!(
            "a stop is already recorded for run {} at generation 2 (ordinal 0)",
            short(&run)
        )),
        "{text}"
    );
    assert!(text.contains("never kills a VM"), "{text}");
    assert_no_banned_words(&text)?;
    Ok(())
}

#[test]
fn a_prefix_that_names_no_run_is_refused_and_creates_no_run() -> TestResult {
    // Arrange
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = finished(&prepared)?;
    let other = if run.to_hex().starts_with('0') {
        "1"
    } else {
        "0"
    };
    let prefix = HexPrefix::from_str(other)?;
    let runs_dir = fixture.project.join(STORE_DIR).join("runs");
    let before = fs::read_dir(&runs_dir)?.count();

    // Act
    let mut out = Vec::new();
    let result = stop(prepared.store(), Some(&prefix), &mut out);

    // Assert
    let Err(error) = result else {
        return Err("stop accepted a prefix that names no run".into());
    };
    assert!(
        matches!(&error, StopError::Address(AddressError::NoRun { .. })),
        "{error}"
    );
    assert_no_banned_words(&error.to_string())?;
    assert!(out.is_empty());
    assert_eq!(fs::read_dir(&runs_dir)?.count(), before);
    assert_eq!(prepared.store().runs()?.len(), 1);
    Ok(())
}

#[test]
fn stop_on_a_store_with_no_runs_names_no_run() -> TestResult {
    // Arrange: a prepared project that has never run.
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;

    // Act
    let mut out = Vec::new();
    let result = stop(prepared.store(), None, &mut out);

    // Assert
    let Err(error) = result else {
        return Err("stop recorded a stop with no run".into());
    };
    assert!(
        matches!(&error, StopError::Address(AddressError::NoRuns)),
        "{error}"
    );
    assert_no_banned_words(&error.to_string())?;
    assert!(prepared.store().runs()?.is_empty());
    Ok(())
}

#[test]
fn a_running_driver_honours_a_stop_from_another_handle() -> TestResult {
    // Arrange: a fake job that, during the first generation-1 trial, calls `stop` through another store handle
    // with the run's full hex, as `minos stop` in another terminal would.
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let recorded: RefCell<Option<(StopOutcome, String)>> = RefCell::new(None);
    let failed = JobFailure::Config {
        what: "the fake could not call stop",
    };
    let jobs = FakeJobs {
        run: |request: &JobRequest| {
            if !request.is_baseline() && recorded.borrow().is_none() {
                let other = fixture.second_store().map_err(|_| failed)?;
                let runs = other.runs().map_err(|_| failed)?;
                let run = runs.first().ok_or(failed)?;
                let prefix = prefix_of(run).map_err(|_| failed)?;
                let result = stop_text(&other, Some(&prefix)).map_err(|_| failed)?;
                *recorded.borrow_mut() = Some(result);
            }
            constant(request)
        },
    };

    // Act
    let (run, ended) = drive(&prepared, &jobs, 3)?;

    // Assert: the stop was recorded at generation 1, and the session ended on it.
    let Some((outcome, text)) = recorded.into_inner() else {
        return Err("no generation-1 trial ran".into());
    };
    assert_eq!(
        outcome,
        StopOutcome::Recorded {
            run,
            generation: 1,
            ordinal: 0
        }
    );
    assert!(
        matches!(
            ended,
            RunEnded::Stopped {
                reason: StopReason::UserStop,
                ..
            }
        ),
        "{ended:?}"
    );
    assert!(text.contains("next safe point"), "{text}");
    assert_no_banned_words(&text)?;
    assert_eq!(
        stops_of(prepared.store(), &run)?,
        [(1, StopReason::UserStop, Some(0))]
    );
    Ok(())
}
