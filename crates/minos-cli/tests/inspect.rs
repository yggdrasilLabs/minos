//! `minos_cli::status::status` and `minos_cli::log::log` over a temporary project's `FileStore`, on runs driven by
//! `minos_cli::run::run` with a fake job runner and the sweep tier, so no test boots a VM or calls a model (CLI-07,
//! COST-02; 08-CONTEXT D-10, D-15, D-16).
//!
//! - `status` on a run driven to generation 2 prints its ID and start, `generations concluded: 2`,
//!   `driver: not running`, the incumbent, and the `approx. cost:` line, and is healthy.
//! - While another `FileStore` handle holds the run's lock, `status` prints `driver: running`; once that guard
//!   drops, the run can be locked again, so the probe left nothing held.
//! - A ledger whose middle line is garbage makes `status` report `corrupt` and `needs a human` and return
//!   unhealthy, and makes `log` return an error naming the line.
//! - With no price table, `status` prints `cost: unavailable` and no dollar figure.
//! - `log` prints one line per ledger line, each starting with its `seq`; `--generation 1` keeps generation 1's
//!   events only; `--exp <prefix>` keeps the events that name that experiment only.
//! - `log --json` with no filter writes exactly the bytes of the run's `ledger.jsonl`.
//! - `--full-ids` prints 64-hex IDs, and the default prints 12-hex prefixes.
//!
//! Every captured output and every provoked error's text passes [`assert_no_banned_words`]: no `estimate`, and no
//! whole word `exact` (COST-02). The fixture's files and the sweep's rationales contain neither word.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use minos_cli::address::HexPrefix;
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::log::{LogOptions, log};
use minos_cli::project::{Prepared, prepare};
use minos_cli::render::IdStyle;
use minos_cli::run::{RunOptions, run};
use minos_cli::status::{StatusOptions, status};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, RunId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{BackendInfo, BackendKind, EXPECTED_KRUN_LAYERS, Isolation};
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;
use minos_loop::{DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, ModelProvider, Proposer};
use minos_source::CleanPolicy;
use minos_store::{Blobs, FileStore, Ledger, RunLock, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The run's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// The manifest up to its proposer: one writable file with a sweep param over its `value = ` line and an objective
/// at `5%`. The judge argv is a bare word, so it names no project path.
const MANIFEST_HEAD: &str = r#"[project]
name = "inspecting"
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
"#;

/// The sweep proposer.
const SWEEP: &str = "\n[[proposer]]\nkind = \"sweep\"\nbatch = 2\n";

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
    /// The fixture with the sweep proposer.
    fn new() -> Fallible<Self> {
        Self::with(SWEEP)
    }

    /// Writes the project with `proposer` as its proposer table, pushes its judge data, stores a root image, and
    /// writes the lock's image entry.
    fn with(proposer: &str) -> Fallible<Self> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        write(
            &project,
            MANIFEST_FILE,
            format!("{MANIFEST_HEAD}{proposer}").as_bytes(),
        )?;
        write(&project, "program.md", b"Raise the score.\n")?;
        write(&project, "minos.Dockerfile", DOCKERFILE)?;
        write(&project, "src/value.txt", b"value = 5\n")?;
        write(&project, "src/lib.rs", b"pub fn f() {}\n")?;
        let data = root.join("judge-data");
        write(&data, "expected.txt", b"inspect test judge data\n")?;
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

    /// A second handle on the fixture's store.
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

/// A scored result for `request` with the stage logs `logs`: the gate passes and the objective is `value`; both
/// VMs ran.
fn scored_with_logs(
    request: &JobRequest,
    value: f64,
    logs: BTreeMap<minos_core::outcome::Stage, BlobId>,
) -> Result<JobResult, JobFailure> {
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
        logs,
    )
    .map_err(|_| JobFailure::Config {
        what: "the fake result is invalid",
    })
}

/// 1.0 for the baseline, and for a candidate 2.0 plus its ID's first byte over 1,000, with no logs.
fn deterministic(request: &JobRequest) -> Result<JobResult, JobFailure> {
    if request.is_baseline() {
        return scored_with_logs(request, 1.0, BTreeMap::new());
    }
    let byte = request.exp().as_bytes().first().copied().unwrap_or(0);
    scored_with_logs(request, 2.0 + f64::from(byte) / 1_000.0, BTreeMap::new())
}

/// Drives a new run of `prepared` to `generations` with `proposer` and `jobs`, and returns its ID.
fn drive<P: ModelProvider, J: JobRunner>(
    prepared: &Prepared,
    proposer: &Proposer<P>,
    jobs: &J,
    generations: u32,
) -> Fallible<RunId> {
    let mut out = Vec::new();
    let outcome = run(
        prepared,
        proposer,
        jobs,
        RunOptions::new(generations, None),
        STARTED_AT,
        DriverOptions {
            retry_pause: Duration::ZERO,
        },
        &mut out,
    )?;
    Ok(*outcome.run())
}

/// A sweep run of `prepared` driven to generation 2 with [`deterministic`] results.
fn driven(prepared: &Prepared) -> Fallible<RunId> {
    drive(
        prepared,
        &Proposer::<FakeProvider>::without_provider(),
        &FakeJobs { run: deterministic },
        2,
    )
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

/// `status` on `store` with `prices` and `ids`: its health and everything it printed. The caller checks the text.
fn status_text<S: Ledger + Blobs + RunLock>(
    store: &S,
    run: Option<&HexPrefix>,
    prices: Option<&minos_core::money::Prices>,
    ids: IdStyle,
) -> Fallible<(bool, String)> {
    let mut out = Vec::new();
    let outcome = status(store, run, prices, StatusOptions::new(ids), &mut out)?;
    Ok((outcome.is_healthy(), String::from_utf8(out)?))
}

/// `log` on `store` with `options`: everything it printed. The caller checks the text.
fn log_text<L: Ledger>(
    store: &L,
    run: Option<&HexPrefix>,
    options: LogOptions,
) -> Fallible<String> {
    let mut out = Vec::new();
    log(store, run, options, &mut out)?;
    Ok(String::from_utf8(out)?)
}

/// The default `log` options: no filter, short IDs.
fn plain() -> LogOptions {
    LogOptions::new(None, None, false, IdStyle::Short)
}

/// The first `len` characters of `hex`, as a prefix.
fn prefix_of(hex: &str, len: usize) -> Fallible<HexPrefix> {
    let text: String = hex.chars().take(len).collect();
    Ok(HexPrefix::from_str(&text)?)
}

/// The folded state of `run`.
fn state_of(store: &FileStore, run: &RunId) -> Fallible<RunState> {
    Ok(fold(store.read(run)?.records())?)
}

/// The first 12 hex characters of `exp`.
fn short(exp: &ExperimentId) -> String {
    exp.to_hex().chars().take(12).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn status_prints_the_fold_of_a_run_driven_to_generation_two() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;

    let (healthy, text) = status_text(
        prepared.store(),
        None,
        Some(prepared.prices()),
        IdStyle::Short,
    )?;

    assert_no_banned_words(&text)?;
    assert!(healthy, "{text}");
    let hex: String = run.to_hex().chars().take(12).collect();
    let started = format!("run {hex} started 2025-10-09T08:53:20Z (unix {STARTED_AT})");
    assert!(text.lines().any(|line| line == started), "{text}");
    assert!(
        text.lines().any(|line| line == "generations concluded: 2"),
        "{text}"
    );
    assert!(
        text.lines().any(|line| line == "driver: not running"),
        "{text}"
    );
    assert!(
        text.lines().any(|line| line.starts_with("incumbent: ")),
        "{text}"
    );
    assert!(
        text.lines().any(|line| line.starts_with("approx. cost: $")),
        "{text}"
    );
    assert!(!text.contains("needs a human"), "{text}");
    Ok(())
}

#[test]
fn status_reports_a_held_run_as_running_and_leaves_it_unlocked() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let other = fixture.second_store()?;

    let guard = other.lock_run(&run)?;
    let (healthy, text) = status_text(
        prepared.store(),
        None,
        Some(prepared.prices()),
        IdStyle::Short,
    )?;
    drop(guard);

    assert_no_banned_words(&text)?;
    assert!(healthy, "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("driver: running (generation 3 in progress)")),
        "{text}"
    );
    let (_, after) = status_text(
        prepared.store(),
        None,
        Some(prepared.prices()),
        IdStyle::Short,
    )?;
    assert_no_banned_words(&after)?;
    assert!(
        after.lines().any(|line| line == "driver: not running"),
        "{after}"
    );
    let relocked = other.lock_run(&run)?;
    drop(relocked);
    Ok(())
}

#[test]
fn a_corrupt_ledger_is_reported_by_status_and_refused_by_log() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let path = fixture.ledger_path(&run);
    let original = fs::read_to_string(&path)?;
    let lines: Vec<&str> = original.lines().collect();
    let middle = lines.len() / 2;
    let damaged: String = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index == middle {
                "this is not a ledger line\n".to_owned()
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    fs::write(&path, damaged)?;
    let named = prefix_of(&run.to_hex(), 12)?;

    let (healthy, text) = status_text(
        prepared.store(),
        Some(&named),
        Some(prepared.prices()),
        IdStyle::Short,
    )?;

    assert_no_banned_words(&text)?;
    assert!(!healthy, "{text}");
    let line = middle + 1;
    assert!(
        text.lines().any(|row| row.contains("corrupt")
            && row.contains(&format!("line {line}"))
            && row.contains("needs a human")),
        "{text}"
    );
    let mut out = Vec::new();
    let Err(error) = log(prepared.store(), Some(&named), plain(), &mut out) else {
        return Err("log read a corrupt ledger".into());
    };
    let message = error_chain(&error);
    assert_no_banned_words(&message)?;
    assert!(message.contains(&format!("line {line}")), "{message}");
    Ok(())
}

#[test]
fn status_without_prices_prints_no_dollar_figure() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    driven(&prepared)?;

    let (healthy, text) = status_text(prepared.store(), None, None, IdStyle::Short)?;

    assert_no_banned_words(&text)?;
    assert!(healthy, "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("cost: unavailable")),
        "{text}"
    );
    assert!(!text.contains('$'), "{text}");
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// log
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn log_prints_one_line_per_ledger_line_in_seq_order() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let stored = prepared.store().read(&run)?;

    let text = log_text(prepared.store(), None, plain())?;

    assert_no_banned_words(&text)?;
    let printed: Vec<&str> = text.lines().collect();
    assert_eq!(printed.len(), stored.lines().len(), "{text}");
    for (line, row) in stored.lines().iter().zip(&printed) {
        let mut words = row.split_whitespace();
        assert_eq!(words.next(), Some(line.seq().to_string().as_str()), "{row}");
    }
    assert!(
        printed
            .first()
            .is_some_and(|row| row.starts_with("0 run_started 0 ")),
        "{text}"
    );
    Ok(())
}

#[test]
fn log_keeps_one_generation_with_judged_events_under_their_experiment() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let state = state_of(prepared.store(), &run)?;

    let text = log_text(
        prepared.store(),
        None,
        LogOptions::new(Some(1), None, false, IdStyle::Short),
    )?;

    assert_no_banned_words(&text)?;
    assert!(!text.is_empty());
    for row in text.lines() {
        let generation = row.split_whitespace().nth(2);
        assert_eq!(generation, Some("1"), "{row}");
    }
    let judged = text
        .lines()
        .filter(|row| row.split_whitespace().nth(1) == Some("judged"))
        .count();
    let expected: usize = state
        .proposals_in(1)
        .map(|(exp, _)| state.trials(exp, &prepared.judge()).count())
        .sum();
    assert!(expected > 0);
    assert_eq!(judged, expected, "{text}");
    assert!(
        text.lines()
            .any(|row| row.split_whitespace().nth(1) == Some("selected")),
        "{text}"
    );
    Ok(())
}

#[test]
fn log_keeps_the_events_that_name_one_experiment() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let state = state_of(prepared.store(), &run)?;
    let exp = *state
        .proposals_in(1)
        .next()
        .map(|(exp, _)| exp)
        .ok_or("generation 1 proposed nothing")?;

    let text = log_text(
        prepared.store(),
        None,
        LogOptions::new(
            None,
            Some(prefix_of(&exp.to_hex(), 12)?),
            false,
            IdStyle::Short,
        ),
    )?;

    assert_no_banned_words(&text)?;
    let named = format!("exp={}", short(&exp));
    assert!(!text.is_empty());
    for row in text.lines() {
        let kind = row.split_whitespace().nth(1).unwrap_or_default();
        let names = row.contains(&named) || (kind == "selected" && row.contains(&short(&exp)));
        assert!(names, "{row}");
    }
    assert!(
        text.lines()
            .any(|row| row.split_whitespace().nth(1) == Some("proposed")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|row| row.split_whitespace().nth(1) == Some("judged")),
        "{text}"
    );
    Ok(())
}

#[test]
fn log_json_reproduces_the_stored_ledger_bytes() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;

    let text = log_text(
        prepared.store(),
        None,
        LogOptions::new(None, None, true, IdStyle::Short),
    )?;

    assert_no_banned_words(&text)?;
    let stored = fs::read_to_string(fixture.ledger_path(&run))?;
    assert_eq!(text, stored);
    Ok(())
}

#[test]
fn full_ids_print_all_64_hex_characters() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = driven(&prepared)?;
    let full = run.to_hex();
    let short_run: String = full.chars().take(12).collect();

    let long = log_text(
        prepared.store(),
        None,
        LogOptions::new(None, None, false, IdStyle::Full),
    )?;
    let brief = log_text(prepared.store(), None, plain())?;
    let (_, long_status) = status_text(
        prepared.store(),
        None,
        Some(prepared.prices()),
        IdStyle::Full,
    )?;

    for text in [&long, &brief, &long_status] {
        assert_no_banned_words(text)?;
    }
    assert!(long.contains(&format!("run={full}")), "{long}");
    assert!(brief.contains(&format!("run={short_run} ")), "{brief}");
    assert!(!brief.contains(&full), "{brief}");
    assert!(
        long_status.contains(&format!("run {full} started")),
        "{long_status}"
    );
    Ok(())
}

/// `error`'s message followed by each source's, joined with `: `.
fn error_chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
