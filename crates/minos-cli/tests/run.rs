//! `minos_cli::run::run` on a temporary project over its `FileStore`, with a fake job runner and the sweep tier, so
//! no test boots a VM or calls a model (CLI-06, COST-02; 08-CONTEXT D-07, D-14; 08-RESEARCH Open Questions 1, 2).
//!
//! - A new run to generation 2 reaches it and prints one `started` line, three baseline trial lines, one
//!   `proposed` and one `selected` line per generation, one trial line per recorded `Judged` of each generation,
//!   and one cost line for each of generations 0, 1, and 2.
//! - A run to generation 1, then `--run <prefix> --generations 2`, reaches generation 2, and the second session
//!   never runs a generation-1 experiment: the target is absolute.
//! - A changed writable file changes the base, so `--run` is `ResumeRefused`, naming `rerun without --run`.
//! - A new run in the same second as an existing one is `RunExists`, naming `--run`.
//! - A `UserStop` appended during a generation-1 trial ends the run as `Stopped` with a `stopped user_stop` line.
//! - A baseline objective of 1.0 and a candidate of 2.0 print `+100.0%` on the candidate's trial line.
//! - A noisy baseline prints `noise verdict: refuse` as advice and the run still reaches its target.
//! - Zero generations, and a prefix that names no run, are refused before anything is recorded.
//!
//! Every test also checks that no output line contains the word `estimate` or the whole word `exact` (COST-02).

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use minos_cli::address::{HexPrefix, fold_run};
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{Prepared, prepare};
use minos_cli::run::{RunEnded, RunError, RunOptions, RunOutcome, run};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, RunId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, EXPECTED_KRUN_LAYERS, Event, Isolation, Record, StopReason,
};
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::state::RunState;
use minos_core::usage::Usage;
use minos_loop::{DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, Proposer};
use minos_source::CleanPolicy;
use minos_store::{Blobs, FileStore, Ledger, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The first run's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// The manifest: one writable file with a sweep param over its `value = ` line, a sweep proposer, and an objective
/// at `5%`. The judge argv is a bare word, so it names no project path.
const MANIFEST: &str = r#"[project]
name = "running"
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

/// A `prices.toml` that prices compute; the sweep calls no model.
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"
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
        write(&data, "expected.txt", b"run test judge data\n")?;
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

/// The poisoned-lock failure.
const POISONED: JobFailure = JobFailure::Config {
    what: "the fake job runner's request list was poisoned",
};

/// A [`JobRunner`] that answers trials from `run` and records every request; its preflight always passes.
struct FakeJobs<F> {
    run: F,
    requests: Mutex<Vec<JobRequest>>,
}

impl<F> FakeJobs<F>
where
    F: Fn(&JobRequest) -> Result<JobResult, JobFailure>,
{
    /// A runner that answers with `run`.
    const fn new(run: F) -> Self {
        Self {
            run,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every trial request so far, in order.
    fn requests(&self) -> Fallible<Vec<JobRequest>> {
        Ok(self
            .requests
            .lock()
            .map_err(|_poisoned| "the fake job runner's request list was poisoned")?
            .clone())
    }
}

impl<F> JobRunner for FakeJobs<F>
where
    F: Fn(&JobRequest) -> Result<JobResult, JobFailure>,
{
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        self.requests
            .lock()
            .map_err(|_poisoned| POISONED)?
            .push(request.clone());
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
        BTreeMap::new(),
    )
    .map_err(|_| JobFailure::Config {
        what: "the fake result is invalid",
    })
}

/// 1.0 for the baseline, and for a candidate 2.0 plus its ID's first byte over 1,000, so every rerun of one
/// request answers the same.
fn deterministic(request: &JobRequest) -> Result<JobResult, JobFailure> {
    if request.is_baseline() {
        return scored(request, 1.0);
    }
    let byte = request.exp().as_bytes().first().copied().unwrap_or(0);
    scored(request, 2.0 + f64::from(byte) / 1_000.0)
}

// ---------------------------------------------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------------------------------------------

/// The driver options with no retry pause.
const fn driver() -> DriverOptions {
    DriverOptions {
        retry_pause: Duration::ZERO,
    }
}

/// `run` on `prepared` with `jobs` and `options` at `started_at`: its result and everything it printed. The printed
/// text is checked for the banned words first.
fn run_with<J: JobRunner>(
    prepared: &Prepared,
    jobs: &J,
    options: RunOptions,
    started_at: u64,
) -> Fallible<(Result<RunOutcome, RunError>, String)> {
    let proposer = Proposer::<FakeProvider>::without_provider();
    let mut out = Vec::new();
    let result = run(
        prepared,
        &proposer,
        jobs,
        options,
        started_at,
        driver(),
        &mut out,
    );
    let text = String::from_utf8(out)?;
    assert_no_banned_words(&text);
    Ok((result, text))
}

/// A new run to `generations`, which must succeed.
fn new_run<J: JobRunner>(
    prepared: &Prepared,
    jobs: &J,
    generations: u32,
) -> Fallible<(RunOutcome, String)> {
    let (result, text) = run_with(
        prepared,
        jobs,
        RunOptions::new(generations, None),
        STARTED_AT,
    )?;
    Ok((result?, text))
}

/// No line says `estimate`, and none has `exact` as a whole word (COST-02).
fn assert_no_banned_words(text: &str) {
    let lower = text.to_lowercase();
    assert!(!lower.contains("estimate"), "{text}");
    assert!(
        !lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == "exact"),
        "{text}"
    );
}

/// The first 12 hex characters of `exp`.
fn short(exp: &ExperimentId) -> String {
    exp.to_hex().chars().take(12).collect()
}

/// The lines of `text` that start with `prefix`.
fn count_prefixed(text: &str, prefix: &str) -> usize {
    text.lines().filter(|line| line.starts_with(prefix)).count()
}

/// The trial lines of `text` in `generation` whose experiment column is one of `exps`.
fn trial_lines<'t>(text: &'t str, generation: u32, exps: &BTreeSet<String>) -> Vec<&'t str> {
    let tag = format!("g{generation:02}");
    text.lines()
        .filter(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some(tag.as_str()) && words.next().is_some_and(|exp| exps.contains(exp))
        })
        .collect()
}

/// The short IDs of the experiments proposed in `generation`.
fn proposed_in(state: &RunState, generation: u32) -> BTreeSet<String> {
    state
        .proposals_in(generation)
        .map(|(exp, _)| short(exp))
        .collect()
}

/// How many `Judged` records the experiments of `generation` have under `prepared`'s judge.
fn judged_in(state: &RunState, prepared: &Prepared, generation: u32) -> usize {
    state
        .proposals_in(generation)
        .map(|(exp, _)| state.trials(exp, &prepared.judge()).count())
        .sum()
}

/// The run `run` writes for `prepared` at [`STARTED_AT`].
fn run_id(prepared: &Prepared) -> RunId {
    RunId::of(&prepared.manifest_id(), &prepared.base().id(), STARTED_AT)
}

/// The first `len` characters of the run's hex, as a prefix.
fn prefix(run: &RunId, len: usize) -> Fallible<HexPrefix> {
    let hex: String = run.to_hex().chars().take(len).collect();
    Ok(HexPrefix::from_str(&hex)?)
}

// ---------------------------------------------------------------------------------------------------------------
// The cases
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_new_run_reaches_its_target_and_prints_one_line_per_recorded_effect() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::new(deterministic);

    let (outcome, text) = new_run(&prepared, &jobs, 2)?;

    assert_eq!(*outcome.ended(), RunEnded::Reached { generation: 2 });
    let run = *outcome.run();
    assert_eq!(run, run_id(&prepared));
    let state = fold_run(prepared.store(), &run)?;
    let hex = short_run(&run);
    assert_eq!(
        count_prefixed(&text, &format!("run {hex} started; target generation 2")),
        1,
        "{text}"
    );
    assert_eq!(
        count_prefixed(&text, &format!("g00 {} ", short(state.baseline()))),
        3,
        "{text}"
    );
    assert_eq!(count_prefixed(&text, "g00 noise verdict: ok"), 1, "{text}");
    for generation in [1, 2] {
        let tag = format!("g{generation:02}");
        assert_eq!(
            count_prefixed(&text, &format!("{tag} proposed sweep admitted ")),
            1,
            "{text}"
        );
        assert_eq!(
            trial_lines(&text, generation, &proposed_in(&state, generation)).len(),
            judged_in(&state, &prepared, generation),
            "{text}"
        );
        assert_eq!(
            count_prefixed(&text, &format!("{tag} selected ")),
            1,
            "{text}"
        );
    }
    assert!(judged_in(&state, &prepared, 1) > 0, "{text}");
    for generation in [0, 1, 2] {
        assert_eq!(
            count_prefixed(&text, &format!("g{generation:02} approx. cost: $")),
            1,
            "{text}"
        );
    }
    assert_eq!(
        count_prefixed(&text, &format!("run {hex} reached generation 2")),
        1,
        "{text}"
    );
    Ok(())
}

#[test]
fn a_resumed_run_ends_where_an_uninterrupted_one_would() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let first = FakeJobs::new(deterministic);
    let (outcome, _) = new_run(&prepared, &first, 1)?;
    assert_eq!(*outcome.ended(), RunEnded::Reached { generation: 1 });
    let run = *outcome.run();
    let generation_one: BTreeSet<ExperimentId> = fold_run(prepared.store(), &run)?
        .proposals_in(1)
        .map(|(exp, _)| *exp)
        .collect();
    assert!(!generation_one.is_empty());

    let second = FakeJobs::new(deterministic);
    let (result, text) = run_with(
        &prepared,
        &second,
        RunOptions::new(2, Some(prefix(&run, 8)?)),
        STARTED_AT + 60,
    )?;
    let outcome = result?;

    assert_eq!(*outcome.run(), run);
    assert_eq!(*outcome.ended(), RunEnded::Reached { generation: 2 });
    assert_eq!(
        count_prefixed(
            &text,
            &format!("run {} resumed; target generation 2", short_run(&run))
        ),
        1,
        "{text}"
    );
    let requests = second.requests()?;
    assert!(!requests.is_empty());
    assert!(
        requests
            .iter()
            .all(|request| !request.is_baseline() && !generation_one.contains(&request.exp())),
        "the second session re-ran a generation-0 or generation-1 trial"
    );
    assert_eq!(prepared.store().runs()?.len(), 1);
    Ok(())
}

#[test]
fn a_changed_base_refuses_the_resume_and_names_a_new_run() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let (outcome, _) = new_run(&prepared, &FakeJobs::new(deterministic), 1)?;
    let run = *outcome.run();

    write(&fixture.project, "src/value.txt", b"value = 6\n")?;
    let changed = fixture.prepare()?;
    let jobs = FakeJobs::new(deterministic);
    let (result, _) = run_with(
        &changed,
        &jobs,
        RunOptions::new(2, Some(prefix(&run, 12)?)),
        STARTED_AT + 60,
    )?;

    let error = match result {
        Err(error @ RunError::ResumeRefused { .. }) => error,
        other => return Err(format!("expected ResumeRefused, got {other:?}").into()),
    };
    assert!(error.to_string().contains("rerun without --run"), "{error}");
    assert!(jobs.requests()?.is_empty());
    Ok(())
}

#[test]
fn a_new_run_in_the_same_second_as_an_existing_one_is_refused() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    new_run(&prepared, &FakeJobs::new(deterministic), 1)?;

    let jobs = FakeJobs::new(deterministic);
    let (result, text) = run_with(&prepared, &jobs, RunOptions::new(2, None), STARTED_AT)?;

    let error = match result {
        Err(error @ RunError::RunExists { .. }) => error,
        other => return Err(format!("expected RunExists, got {other:?}").into()),
    };
    assert!(error.to_string().contains("--run"), "{error}");
    assert!(jobs.requests()?.is_empty());
    assert!(text.is_empty(), "{text}");
    Ok(())
}

#[test]
fn a_user_stop_during_a_generation_one_trial_ends_the_run() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let run = run_id(&prepared);
    let store = prepared.store();
    let stopped = Mutex::new(false);
    let jobs = FakeJobs::new(|request: &JobRequest| {
        if !request.is_baseline() {
            let mut stopped = stopped.lock().map_err(|_poisoned| POISONED)?;
            if !*stopped {
                *stopped = true;
                store
                    .append(
                        &run,
                        Record::v1(Event::StopRequested {
                            generation: 1,
                            reason: StopReason::UserStop,
                            ordinal: Some(0),
                        }),
                    )
                    .map_err(|_error| JobFailure::Config {
                        what: "the fake could not record the stop",
                    })?;
            }
        }
        deterministic(request)
    });

    let (outcome, text) = new_run(&prepared, &jobs, 3)?;

    assert!(
        matches!(
            outcome.ended(),
            RunEnded::Stopped {
                reason: StopReason::UserStop,
                ..
            }
        ),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with('g') && line.ends_with(" stopped user_stop")),
        "{text}"
    );
    assert_eq!(
        count_prefixed(
            &text,
            &format!("run {} stopped at generation ", short_run(&run))
        ),
        1,
        "{text}"
    );
    Ok(())
}

#[test]
fn a_candidate_twice_the_baseline_prints_plus_one_hundred_percent() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::new(|request: &JobRequest| {
        scored(request, if request.is_baseline() { 1.0 } else { 2.0 })
    });

    let (outcome, text) = new_run(&prepared, &jobs, 1)?;

    let state = fold_run(prepared.store(), outcome.run())?;
    let lines = trial_lines(&text, 1, &proposed_in(&state, 1));
    assert!(!lines.is_empty(), "{text}");
    for line in lines {
        let mut words = line.split_whitespace().skip(4);
        assert_eq!(words.next(), Some("+100.0%"), "{line}");
    }
    let baseline_lines = count_prefixed(&text, &format!("g00 {} ", short(state.baseline())));
    assert_eq!(baseline_lines, 3, "{text}");
    Ok(())
}

#[test]
fn a_noisy_baseline_is_advice_and_the_run_still_reaches_its_target() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let trial = Mutex::new(0_usize);
    let jobs = FakeJobs::new(|request: &JobRequest| {
        if request.is_baseline() {
            let mut count = trial.lock().map_err(|_poisoned| POISONED)?;
            let value = [1.0, 1.2, 0.9].get(*count % 3).copied().unwrap_or(1.0);
            *count += 1;
            return scored(request, value);
        }
        deterministic(request)
    });

    let (outcome, text) = new_run(&prepared, &jobs, 1)?;

    assert_eq!(*outcome.ended(), RunEnded::Reached { generation: 1 });
    assert_eq!(
        count_prefixed(
            &text,
            "g00 noise verdict: refuse (advisory; minos check explains)"
        ),
        1,
        "{text}"
    );
    Ok(())
}

#[test]
fn zero_generations_and_an_unknown_prefix_are_refused_before_anything_is_recorded() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::new(deterministic);

    let (result, _) = run_with(&prepared, &jobs, RunOptions::new(0, None), STARTED_AT)?;
    assert!(
        matches!(result, Err(RunError::ZeroGenerations)),
        "{result:?}"
    );
    assert!(prepared.store().runs()?.is_empty());

    new_run(&prepared, &jobs, 1)?;
    let other = if run_id(&prepared).to_hex().starts_with('0') {
        "f"
    } else {
        "0"
    };
    let (result, _) = run_with(
        &prepared,
        &jobs,
        RunOptions::new(2, Some(HexPrefix::from_str(other)?)),
        STARTED_AT + 60,
    )?;
    assert!(matches!(result, Err(RunError::Address(_))), "{result:?}");
    assert_eq!(prepared.store().runs()?.len(), 1);
    Ok(())
}

/// The first 12 hex characters of `run`.
fn short_run(run: &RunId) -> String {
    run.to_hex().chars().take(12).collect()
}
