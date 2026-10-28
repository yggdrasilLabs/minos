//! `minos_cli::report::{build, report}` and `minos_cli::adopt::adopt` over a temporary project's `FileStore`, on a
//! sweep run driven to generation 1 by `minos_cli::run::run` with a fake job runner that scores every candidate above
//! the baseline, so generation 1 is `Improved` and no test boots a VM or calls a model (CLI-08, CLI-09, COST-02;
//! 08-CONTEXT D-11, D-12).
//!
//! - `report <winner>` prints every line of `Report::text()`, then `report hash: <64 hex>`, then the `minos adopt`
//!   command naming the experiment, the run, and the hash; the ledger has the same lines before and after.
//! - `report <baseline>` is refused: the baseline has no diff to review.
//! - `adopt` with a well-formed hash that is not the report's is `HashMismatch`, appends nothing, and writes no
//!   patch file.
//! - `adopt` with the report's hash records the acknowledgment, writes `<project>/minos-<12 hex>.patch` holding the
//!   recorded cumulative diff's canonical bytes, and prints the `patch -p1` and `git apply --directory=` commands.
//! - A second identical `adopt` reports the acknowledgment as already recorded, then refuses the existing file and
//!   leaves it unchanged.
//! - `--out` naming an existing file is `Exists`; `--out` naming a new path writes there.
//! - A symlink at the patch path is `Exists`, and its target is never created.
//! - After a second `Judged` trial for the winner is appended, the old hash is `HashMismatch`.
//!
//! Every captured output, `Report::text()`, and every provoked error's text passes [`assert_no_banned_words`]: no
//! `estimate`, and no whole word `exact` (COST-02). The fixture's files and the sweep's rationales contain neither
//! word, so a hit can only come from Minos's own text.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use minos_cli::address::HexPrefix;
use minos_cli::adopt::{AdoptError, AdoptOptions, adopt};
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{Prepared, base_surface, prepare};
use minos_cli::report::{BuiltReport, ReportCmdError, build, report};
use minos_cli::run::{RunOptions, run};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, ReportHash, RunId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation, Record,
};
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;
use minos_loop::history::{recorded_diff, recorded_manifest_at};
use minos_loop::{DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, Proposer};
use minos_source::CleanPolicy;
use minos_store::{Appended, Blobs, FileStore, Ledger, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The run's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// A sweep manifest: one writable file with a param over its `value = ` line and an objective at `5%`. The judge
/// argv is a bare word, so it names no project path.
const MANIFEST: &str = r#"[project]
name = "reviewing"
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
    root: PathBuf,
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
        write(&data, "expected.txt", b"report test judge data\n")?;
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
            root,
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

    /// The bytes of `run`'s ledger file.
    fn ledger_bytes(&self, run: &RunId) -> Fallible<Vec<u8>> {
        Ok(fs::read(
            self.project
                .join(STORE_DIR)
                .join("runs")
                .join(run.to_hex())
                .join("ledger.jsonl"),
        )?)
    }

    /// The default patch path for `exp`.
    fn default_patch(&self, exp: &ExperimentId) -> PathBuf {
        self.project.join(format!("minos-{}.patch", short(exp)))
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

/// The backend every fake result records.
fn backend() -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        Isolation::Hardware,
        EXPECTED_KRUN_LAYERS.iter().copied().collect(),
        ImageDigest::of(b"root image"),
        Some(ImageDigest::of(b"kernel")),
    )
}

/// A scored outcome whose gate passes and whose objective is `value`.
fn scored(value: f64) -> Fallible<Outcome> {
    let card = Scorecard::from_entries([
        (
            MetricName::from_str("identical_output")?,
            Metric::new(1.0)?,
            false,
        ),
        (
            MetricName::from_str("probe_score")?,
            Metric::new(value)?,
            false,
        ),
    ])?;
    Ok(Outcome::Scored(card))
}

/// 1.0 for the baseline, and for a candidate 2.0 plus its ID's first byte over 1,000; both VMs ran.
fn deterministic(request: &JobRequest) -> Result<JobResult, JobFailure> {
    let invalid = JobFailure::Config {
        what: "the fake result is invalid",
    };
    let value = if request.is_baseline() {
        1.0
    } else {
        let byte = request.exp().as_bytes().first().copied().unwrap_or(0);
        2.0 + f64::from(byte) / 1_000.0
    };
    let outcome = scored(value).map_err(|_| invalid)?;
    JobResult::new(
        request,
        outcome,
        Usage::compute(1_000, 2_048_000),
        Some(Usage::compute(500, 1_024_000)),
        backend(),
        BTreeMap::new(),
    )
    .map_err(|_| invalid)
}

/// A run of the fixture driven to generation 1, with its folded state and its generation-1 winner.
struct Driven {
    fixture: Fixture,
    prepared: Prepared,
    run: RunId,
    winner: ExperimentId,
}

impl Driven {
    /// Prepares the fixture and drives a sweep run to generation 1 with [`deterministic`] results.
    fn new() -> Fallible<Self> {
        let fixture = Fixture::new()?;
        let prepared = fixture.prepare()?;
        let mut out = Vec::new();
        let outcome = run(
            &prepared,
            &Proposer::<FakeProvider>::without_provider(),
            &FakeJobs { run: deterministic },
            RunOptions::new(1, None),
            STARTED_AT,
            DriverOptions {
                retry_pause: Duration::ZERO,
            },
            &mut out,
        )?;
        let run = *outcome.run();
        let state = state_of(prepared.store(), &run)?;
        let winner = match state.decision(1).map(minos_core::ledger::Decision::kind) {
            Some(DecisionKind::Improved { winner }) => *winner,
            _ => return Err("generation 1 did not improve".into()),
        };
        Ok(Self {
            fixture,
            prepared,
            run,
            winner,
        })
    }

    /// The fixture's store.
    const fn store(&self) -> &FileStore {
        self.prepared.store()
    }

    /// The run's folded state now.
    fn state(&self) -> Fallible<RunState> {
        state_of(self.store(), &self.run)
    }
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

/// The first `len` characters of `hex`, as a prefix.
fn prefix_of(hex: &str, len: usize) -> Fallible<HexPrefix> {
    let text: String = hex.chars().take(len).collect();
    Ok(HexPrefix::from_str(&text)?)
}

/// The 12-character prefix of `exp`.
fn exp_prefix(exp: &ExperimentId) -> Fallible<HexPrefix> {
    prefix_of(&exp.to_hex(), 12)
}

/// The folded state of `run`.
fn state_of(store: &FileStore, run: &RunId) -> Fallible<RunState> {
    Ok(fold(store.read(run)?.records())?)
}

/// The first 12 hex characters of `exp`.
fn short(exp: &ExperimentId) -> String {
    exp.to_hex().chars().take(12).collect()
}

/// `report` on the driven run's `exp`: its result and everything it printed. The caller checks both.
fn report_text(
    driven: &Driven,
    exp: &ExperimentId,
) -> Fallible<(Result<BuiltReport, ReportCmdError>, String)> {
    let mut out = Vec::new();
    let result = report(
        driven.store(),
        Some(&prefix_of(&driven.run.to_hex(), 12)?),
        &exp_prefix(exp)?,
        &mut out,
    );
    Ok((result, String::from_utf8(out)?))
}

/// `adopt` of the driven run's winner with `options`: its result and everything it printed. The caller checks both.
fn adopt_text(
    driven: &Driven,
    options: AdoptOptions,
) -> Fallible<(Result<minos_cli::adopt::AdoptOutcome, AdoptError>, String)> {
    let mut out = Vec::new();
    let result = adopt(
        driven.store(),
        &driven.fixture.project,
        Some(&prefix_of(&driven.run.to_hex(), 12)?),
        &exp_prefix(&driven.winner)?,
        options,
        &mut out,
    );
    Ok((result, String::from_utf8(out)?))
}

/// The winner's built report, with its text checked for banned words.
fn built(driven: &Driven) -> Fallible<BuiltReport> {
    let built = build(
        driven.store(),
        Some(&prefix_of(&driven.run.to_hex(), 12)?),
        &exp_prefix(&driven.winner)?,
    )?;
    assert_no_banned_words(built.report().text())?;
    Ok(built)
}

/// The winner's recorded cumulative diff bytes, re-read through the loop's own reader.
fn recorded_bytes(driven: &Driven) -> Fallible<Vec<u8>> {
    let state = driven.state()?;
    let generation = state
        .proposal(&driven.winner)
        .ok_or("the winner has no proposal")?
        .generation();
    let manifest = recorded_manifest_at(driven.store(), &state, generation)?;
    let base = base_surface(driven.store(), &state, &manifest)?;
    let diff = recorded_diff(driven.store(), &state, &manifest, &base, &driven.winner)?;
    Ok(diff.as_diff().as_bytes().to_vec())
}

/// A hash that is well formed but is not any report's.
fn other_hash() -> ReportHash {
    ReportHash::of(b"a report nobody rendered")
}

// ---------------------------------------------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn report_prints_the_text_the_hash_and_the_adopt_command_and_appends_nothing() -> TestResult {
    let driven = Driven::new()?;
    let before = driven.fixture.ledger_bytes(&driven.run)?;

    let (result, text) = report_text(&driven, &driven.winner)?;
    let built = result?;

    assert_no_banned_words(&text)?;
    assert_no_banned_words(built.report().text())?;
    assert_eq!(built.exp(), &driven.winner);
    assert_eq!(built.run(), &driven.run);
    let hash = built.report().hash().to_hex();
    for line in built.report().text().lines() {
        assert!(
            text.lines().any(|printed| printed == line),
            "{line}\n{text}"
        );
    }
    assert!(text.starts_with("minos report v1\n"), "{text}");
    assert!(
        text.lines()
            .any(|line| line == format!("report hash: {hash}")),
        "{text}"
    );
    let command = format!(
        "minos adopt {} --run {} --reviewed {hash}",
        short(&driven.winner),
        prefix_of(&driven.run.to_hex(), 12)?
    );
    assert!(
        text.lines().any(|line| line.contains(&command)),
        "{command}\n{text}"
    );
    assert_eq!(driven.fixture.ledger_bytes(&driven.run)?, before);
    Ok(())
}

#[test]
fn report_refuses_the_baseline() -> TestResult {
    let driven = Driven::new()?;
    let state = driven.state()?;

    let (result, text) = report_text(&driven, state.baseline())?;

    assert_no_banned_words(&text)?;
    match result {
        Err(error @ ReportCmdError::Baseline) => {
            assert_no_banned_words(&error.to_string())?;
            assert!(error.to_string().contains("no diff"), "{error}");
        }
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("the baseline was reported".into()),
    }
    assert!(text.is_empty(), "{text}");
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// adopt
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn adopt_refuses_a_hash_that_is_not_the_reports_and_records_and_writes_nothing() -> TestResult {
    let driven = Driven::new()?;
    let before = driven.fixture.ledger_bytes(&driven.run)?;

    let (result, text) = adopt_text(&driven, AdoptOptions::new(other_hash(), None))?;

    assert_no_banned_words(&text)?;
    match result {
        Err(error @ AdoptError::HashMismatch { .. }) => {
            assert_no_banned_words(&error.to_string())?;
            assert!(error.to_string().contains("minos report"), "{error}");
        }
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("a wrong hash was adopted".into()),
    }
    assert_eq!(driven.fixture.ledger_bytes(&driven.run)?, before);
    assert!(!driven.fixture.default_patch(&driven.winner).exists());
    assert!(
        !driven
            .state()?
            .is_acknowledged(&driven.winner, &other_hash())
    );
    Ok(())
}

#[test]
fn adopt_with_the_reports_hash_records_it_and_writes_the_canonical_patch() -> TestResult {
    let driven = Driven::new()?;
    let built = built(&driven)?;
    let hash = *built.report().hash();

    let (result, text) = adopt_text(&driven, AdoptOptions::new(hash, None))?;
    let outcome = result?;

    assert_no_banned_words(&text)?;
    assert!(driven.state()?.is_acknowledged(&driven.winner, &hash));
    let path = driven.fixture.default_patch(&driven.winner);
    assert_eq!(outcome.path(), path.as_path());
    assert!(outcome.newly_acknowledged());
    let bytes = fs::read(&path)?;
    assert_eq!(bytes, recorded_bytes(&driven)?);
    assert_eq!(bytes, built.diff().as_diff().as_bytes());
    assert!(text.contains("(recorded)"), "{text}");
    assert!(text.contains("patch -p1"), "{text}");
    assert!(text.contains("git apply --directory="), "{text}");
    assert!(text.contains("never applies a patch"), "{text}");
    assert!(text.contains("src/value.txt"), "{text}");
    Ok(())
}

#[test]
fn a_second_adopt_reports_the_acknowledgment_as_recorded_and_refuses_the_existing_file()
-> TestResult {
    let driven = Driven::new()?;
    let hash = *built(&driven)?.report().hash();
    let (first, _) = adopt_text(&driven, AdoptOptions::new(hash, None))?;
    first?;
    let path = driven.fixture.default_patch(&driven.winner);
    let written = fs::read(&path)?;
    let before = driven.fixture.ledger_bytes(&driven.run)?;

    let (result, text) = adopt_text(&driven, AdoptOptions::new(hash, None))?;

    assert_no_banned_words(&text)?;
    assert!(text.contains("(already recorded)"), "{text}");
    match result {
        Err(error @ AdoptError::Exists { .. }) => {
            assert_no_banned_words(&error.to_string())?;
            assert!(error.to_string().contains("never overwrites"), "{error}");
        }
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("an existing patch was overwritten".into()),
    }
    assert_eq!(fs::read(&path)?, written);
    assert_eq!(driven.fixture.ledger_bytes(&driven.run)?, before);
    Ok(())
}

#[test]
fn adopt_out_refuses_an_existing_file_and_leaves_it_unchanged() -> TestResult {
    let driven = Driven::new()?;
    let hash = *built(&driven)?.report().hash();
    let path = driven.fixture.root.join("mine.patch");
    fs::write(&path, b"the user's own file\n")?;

    let (result, text) = adopt_text(&driven, AdoptOptions::new(hash, Some(path.clone())))?;

    assert_no_banned_words(&text)?;
    match result {
        Err(error @ AdoptError::Exists { .. }) => assert_no_banned_words(&error.to_string())?,
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("an existing file was overwritten".into()),
    }
    assert_eq!(fs::read(&path)?, b"the user's own file\n");
    assert!(!driven.fixture.default_patch(&driven.winner).exists());
    Ok(())
}

#[test]
fn adopt_out_writes_a_new_path() -> TestResult {
    let driven = Driven::new()?;
    let hash = *built(&driven)?.report().hash();
    let path = driven.fixture.root.join("elsewhere.patch");

    let (result, text) = adopt_text(&driven, AdoptOptions::new(hash, Some(path.clone())))?;
    let outcome = result?;

    assert_no_banned_words(&text)?;
    assert_eq!(outcome.path(), path.as_path());
    assert_eq!(fs::read(&path)?, recorded_bytes(&driven)?);
    assert!(!driven.fixture.default_patch(&driven.winner).exists());
    Ok(())
}

#[test]
fn adopt_refuses_a_symlink_at_the_patch_path_and_never_creates_its_target() -> TestResult {
    let driven = Driven::new()?;
    let hash = *built(&driven)?.report().hash();
    let target = driven.fixture.root.join("target-of-link");
    let path = driven.fixture.default_patch(&driven.winner);
    std::os::unix::fs::symlink(&target, &path)?;

    let (result, text) = adopt_text(&driven, AdoptOptions::new(hash, None))?;

    assert_no_banned_words(&text)?;
    match result {
        Err(error @ AdoptError::Exists { .. }) => assert_no_banned_words(&error.to_string())?,
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("a symlink was followed".into()),
    }
    assert!(!target.exists());
    Ok(())
}

#[test]
fn a_new_trial_makes_the_old_hash_stale() -> TestResult {
    let driven = Driven::new()?;
    let old = *built(&driven)?.report().hash();
    let state = driven.state()?;
    let judge = state
        .judge_versions(&driven.winner)
        .into_iter()
        .next()
        .ok_or("the winner has no judge version")?;
    let appended = driven.store().append(
        &driven.run,
        Record::v1(Event::Judged {
            exp: driven.winner,
            judge,
            trial: 1,
            outcome: scored(2.5)?,
            backend: backend(),
            logs: None,
        }),
    )?;
    assert!(matches!(appended, Appended::New { .. }));
    let before = driven.fixture.ledger_bytes(&driven.run)?;

    let (result, text) = adopt_text(&driven, AdoptOptions::new(old, None))?;

    assert_no_banned_words(&text)?;
    match result {
        Err(error @ AdoptError::HashMismatch { .. }) => {
            assert_no_banned_words(&error.to_string())?;
        }
        Err(error) => return Err(format!("the wrong error: {error}").into()),
        Ok(_) => return Err("a stale report was adopted".into()),
    }
    let now = *built(&driven)?.report().hash();
    assert_ne!(now, old);
    assert_eq!(driven.fixture.ledger_bytes(&driven.run)?, before);
    assert!(!driven.fixture.default_patch(&driven.winner).exists());
    Ok(())
}
