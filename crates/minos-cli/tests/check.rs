//! `minos_cli::check::check` on a temporary project over its `FileStore`, with a fake job runner and no provider
//! (CLI-05, JUDGE-02; 08-CONTEXT D-05 to D-09). No test boots a VM or calls a model.
//!
//! The static report:
//!
//! - the project, base, root image, and judge facts, with one line per denylist drop;
//! - the memory budget line on the default 2048 and 1024 MiB;
//! - the judge-scratch line from the lock's judge extent, rounded up to whole MiB;
//! - the route table, one row per proposer with its model and its eligibility or reason;
//! - the single-shot batch advisory at batch 16 over a 4,000-byte writable file, and none at batch 1;
//! - the provider line for each status, where only a refusal sets `refused`.
//!
//! Calibration (D-05, D-07):
//!
//! - a first check starts one run with exactly `[judge] trials` (3) trials, prints each trial's objective, the
//!   noise floor, `min_improvement`, the verdict, and the cost line, and records trials 0 to 2 only;
//! - a second check reuses that run and launches nothing, and prints the same trial lines;
//! - `--fresh` starts a second run with three more trials, and `--fresh` in the first run's second is
//!   `RunExists`;
//! - an edited writable file changes the base, so nothing is reused;
//! - baseline scores 1.0, 1.2, and 0.9 against `5%` print `noise verdict: refuse` with advice, and refuse nothing;
//! - a baseline whose every trial fails its gate is `BaselineUnscored`.
//!
//! Every test also checks that no line contains the word `estimate` or the whole word `exact` (COST-02).

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use minos_cli::address::fold_run;
use minos_cli::check::{CheckError, CheckOptions, CheckOutcome, ProviderStatus, check};
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{Prepared, prepare};
use minos_core::id::{BlobId, ImageDigest, MetricName};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{BackendInfo, BackendKind, EXPECTED_KRUN_LAYERS, Isolation};
use minos_core::limits::judge_scratch_needed_bytes;
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason};
use minos_core::text::SafeText;
use minos_core::usage::Usage;
use minos_loop::{DriverError, DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, Proposer};
use minos_source::CleanPolicy;
use minos_store::{Blobs, FileStore, Ledger, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The first check's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// A sweep param over the writable file's `value = ` line, and a sweep proposer.
const SWEEP: &str = r#"[[param]]
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

/// The manifest with `tail` (params and proposers) and the objective's `min_improvement`. The judge argv is a bare
/// word, so it names no project path; memory and scratch take their defaults (2048 and 1024 MiB).
fn manifest(tail: &str, min_improvement: &str) -> String {
    format!(
        r#"[project]
name = "checked"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/value.txt"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

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
min_improvement = "{min_improvement}"

[policy]
kind = "greedy"
width = 2

{tail}"#
    )
}

/// A single-shot proposer naming `haiku` with `batch`.
fn single_shot(batch: u32) -> String {
    format!("[[proposer]]\nkind = \"single_shot\"\nmodel = \"haiku\"\nbatch = {batch}\n")
}

/// The Dockerfile the lock image entry is written for.
const DOCKERFILE: &[u8] = b"FROM scratch\n";

/// The fake guest binary the lock image entry is written for.
const GUEST: &[u8] = b"a fake minos-guest\n";

/// A priced `prices.toml`, pricing `haiku`.
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

/// The writable file's default contents.
const VALUE: &[u8] = b"value = 5\n";

/// A writable file of exactly 4,000 bytes: the anchored line, then 399 ten-byte padding lines.
fn padded_value() -> Vec<u8> {
    let mut bytes = VALUE.to_vec();
    for _ in 0..399 {
        bytes.extend_from_slice(b"abcdefghi\n");
    }
    bytes
}

/// A temporary project beside its judge data, guest, and prices file, ready to prepare.
struct Fixture {
    _dir: TempDir,
    project: PathBuf,
    guest: PathBuf,
    prices: PathBuf,
}

impl Fixture {
    /// The default project: the sweep manifest at `5%` and the ten-byte writable file.
    fn new() -> Fallible<Self> {
        Self::with(&manifest(SWEEP, "5%"), VALUE)
    }

    /// Writes the project with `manifest` and the writable file `value`, pushes its judge data, stores a root
    /// image, and writes the lock's image entry.
    fn with(manifest: &str, value: &[u8]) -> Fallible<Self> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        write(&project, MANIFEST_FILE, manifest.as_bytes())?;
        write(&project, "program.md", b"Raise the score.\n")?;
        write(&project, "minos.Dockerfile", DOCKERFILE)?;
        write(&project, "src/value.txt", value)?;
        write(&project, "src/lib.rs", b"pub fn f() {}\n")?;
        let data = root.join("judge-data");
        write(&data, "expected.txt", b"check test judge data\n")?;
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

/// What every trial of the fake job runner reports.
enum Script {
    /// The gate passes and the objective is the next value, cycling through the list.
    Scores(Vec<f64>),
    /// The gate fails.
    GateFails,
}

/// A [`JobRunner`] that answers trials from a script and counts them; its preflight always passes.
struct FakeJobs {
    script: Script,
    calls: AtomicUsize,
}

impl FakeJobs {
    /// A runner whose every trial scores `values` in turn.
    fn scoring(values: &[f64]) -> Self {
        Self {
            script: Script::Scores(values.to_vec()),
            calls: AtomicUsize::new(0),
        }
    }

    /// A runner whose every trial scores 1.
    fn steady() -> Self {
        Self::scoring(&[1.0])
    }

    /// A runner whose every trial fails the gate.
    const fn gate_fails() -> Self {
        Self {
            script: Script::GateFails,
            calls: AtomicUsize::new(0),
        }
    }

    /// How many trials ran.
    fn requests(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl JobRunner for FakeJobs {
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let invalid = JobFailure::Config {
            what: "the fake scorecard is invalid",
        };
        let name = |text: &str| MetricName::from_str(text).map_err(|_| invalid);
        let outcome = match &self.script {
            Script::Scores(values) => {
                let value = values
                    .get(call % values.len().max(1))
                    .copied()
                    .unwrap_or(1.0);
                let card = Scorecard::from_entries([
                    (
                        name("identical_output")?,
                        Metric::new(1.0).map_err(|_| invalid)?,
                        false,
                    ),
                    (
                        name("probe_score")?,
                        Metric::new(value).map_err(|_| invalid)?,
                        false,
                    ),
                ])
                .map_err(|_| invalid)?;
                Outcome::Scored(card)
            }
            Script::GateFails => Outcome::Rejected(Reason::GateFailed {
                gate: name("identical_output")?,
            }),
        };
        JobResult::new(
            request,
            outcome,
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

    fn check(&self, _request: &JobRequest) -> Result<(), JobFailure> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Running check
// ---------------------------------------------------------------------------------------------------------------

/// The options of a check at `started_at`, with no retry pause.
fn options(fresh: bool, started_at: u64) -> CheckOptions {
    CheckOptions::new(fresh, started_at).with_driver(DriverOptions {
        retry_pause: Duration::ZERO,
    })
}

/// `check` on `prepared` with `jobs`, `provider`, and `options`: its result and everything it printed. The
/// printed text is checked for the banned words first.
fn run_check(
    prepared: &Prepared,
    jobs: &FakeJobs,
    provider: &ProviderStatus,
    options: CheckOptions,
) -> Fallible<(Result<CheckOutcome, CheckError>, String)> {
    let proposer = Proposer::<FakeProvider>::without_provider();
    let mut out = Vec::new();
    let result = check(prepared, &proposer, jobs, provider, options, &mut out);
    let text = String::from_utf8(out)?;
    assert_no_banned_words(&text);
    Ok((result, text))
}

/// `check` on `prepared` with a steady job runner and no provider needed, which must succeed.
fn checked(prepared: &Prepared) -> Fallible<(CheckOutcome, String)> {
    let (result, text) = run_check(
        prepared,
        &FakeJobs::steady(),
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT),
    )?;
    Ok((result?, text))
}

/// No line says `estimate`, and none has `exact` as a whole word (COST-02): the text is split on characters
/// that are not ASCII alphanumeric and compared lowercase.
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

/// Whether `text` has a line equal to `line`.
fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|candidate| candidate == line)
}

// ---------------------------------------------------------------------------------------------------------------
// The static report
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_project_facts_name_the_base_image_judge_and_denylist_drops() -> TestResult {
    let fixture = Fixture::new()?;
    write(&fixture.project, ".env", b"NOT_A_SECRET=1\n")?;
    let prepared = fixture.prepare()?;
    let (_, text) = checked(&prepared)?;

    let manifest_hex = prepared.manifest_id().to_hex();
    assert!(
        has_line(
            &text,
            &format!(
                "project: checked (manifest {})",
                manifest_hex.get(..12).ok_or("short")?
            )
        ),
        "{text}"
    );
    let tree_hex = prepared.base().id().to_hex();
    let base_line = format!(
        "base: tree {}, {} files, {} bytes packed",
        tree_hex.get(..12).ok_or("short")?,
        prepared.pack().files(),
        prepared.pack().content_bytes()
    );
    assert!(has_line(&text, &base_line), "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("dropped by the denylist: .env (")),
        "{text}"
    );
    let image_hex = prepared.root_image().to_hex();
    assert!(
        has_line(
            &text,
            &format!("root image: {}", image_hex.get(..12).ok_or("short")?)
        ),
        "{text}"
    );
    let extent = prepared.judge_extent();
    let judge_hex = prepared.judge().to_hex();
    assert!(
        has_line(
            &text,
            &format!(
                "judge: version {}, label 1, {} files, {} bytes",
                judge_hex.get(..12).ok_or("short")?,
                extent.files(),
                extent.bytes()
            )
        ),
        "{text}"
    );
    Ok(())
}

#[test]
fn the_memory_budget_is_the_reserve_the_scratch_and_the_allowance() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let (_, text) = checked(&prepared)?;
    assert!(
        has_line(
            &text,
            "memory: 2048 MiB = guest reserve 256 MiB + scratch 1024 MiB + candidate allowance 768 MiB"
        ),
        "{text}"
    );
    Ok(())
}

#[test]
fn the_judge_scratch_need_comes_from_the_lock_extent_in_whole_mib() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let (_, text) = checked(&prepared)?;
    let extent = prepared.judge_extent();
    let needed = judge_scratch_needed_bytes(extent.bytes(), extent.files()).div_ceil(1024 * 1024);
    assert!(
        has_line(
            &text,
            &format!("judge scratch: needs {needed} MiB of 1024 MiB")
        ),
        "{text}"
    );
    Ok(())
}

#[test]
fn the_route_table_shows_each_proposer_with_its_model_and_eligibility() -> TestResult {
    let tail = format!(
        "{SWEEP}\n{}\n[[proposer]]\nkind = \"planned\"\nmodel = \"haiku\"\nbatch = 1\n",
        single_shot(2)
    );
    let fixture = Fixture::with(&manifest(&tail, "5%"), VALUE)?;
    let prepared = fixture.prepare()?;
    let (_, text) = checked(&prepared)?;
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == "routes:")
        .ok_or("no routes line")?;
    let rows: Vec<&str> = lines.iter().skip(at + 1).take(3).copied().collect();
    assert_eq!(
        rows,
        [
            "  0 sweep: eligible",
            "  1 single_shot haiku: eligible",
            "  2 planned haiku: ineligible: this tier is not in this release",
        ],
        "{text}"
    );
    Ok(())
}

#[test]
fn a_large_single_shot_batch_prints_an_advisory_and_refuses_nothing() -> TestResult {
    let fixture = Fixture::with(&manifest(&single_shot(16), "5%"), &padded_value())?;
    let prepared = fixture.prepare()?;
    let (outcome, text) = checked(&prepared)?;
    let advisories: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("advisory: single_shot proposer 0"))
        .collect();
    assert_eq!(advisories.len(), 1, "{text}");
    assert!(
        advisories
            .iter()
            .all(|line| line.contains("(4000 writable bytes x batch 16)")),
        "{text}"
    );
    assert!(!outcome.refused());
    Ok(())
}

#[test]
fn a_small_single_shot_batch_prints_no_advisory() -> TestResult {
    let fixture = Fixture::with(&manifest(&single_shot(1), "5%"), &padded_value())?;
    let prepared = fixture.prepare()?;
    let (_, text) = checked(&prepared)?;
    assert!(
        !text.lines().any(|line| line.starts_with("advisory:")),
        "{text}"
    );
    Ok(())
}

#[test]
fn the_provider_line_reports_each_status_and_only_a_refusal_refuses() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;

    let (outcome, text) = checked(&prepared)?;
    assert!(
        has_line(&text, "provider: none needed (no proposer names a model)"),
        "{text}"
    );
    assert!(!outcome.refused());

    let ready = ProviderStatus::Ready {
        description: SafeText::inline("claude 2.1.304 logged in"),
    };
    let (result, text) = run_check(
        &prepared,
        &FakeJobs::steady(),
        &ready,
        options(false, STARTED_AT + 1),
    )?;
    assert!(
        has_line(&text, "provider: claude 2.1.304 logged in"),
        "{text}"
    );
    assert!(!result?.refused());

    let refused = ProviderStatus::Refused {
        message: SafeText::inline("not logged in: run claude auth login"),
    };
    let (result, text) = run_check(
        &prepared,
        &FakeJobs::steady(),
        &refused,
        options(false, STARTED_AT + 2),
    )?;
    assert!(
        has_line(
            &text,
            "provider: refused: not logged in: run claude auth login"
        ),
        "{text}"
    );
    assert!(result?.refused());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------------------------------------------

/// The trial lines of `text`, in order.
fn trial_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("trial "))
        .collect()
}

#[test]
fn a_first_check_calibrates_a_new_run_with_the_manifest_trials() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::steady();
    let (result, text) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT),
    )?;
    let outcome = result?;

    assert_eq!(jobs.requests(), 3);
    assert!(!outcome.reused());
    assert!(!outcome.refused());
    let run = *outcome.run().ok_or("no run")?;
    assert_eq!(prepared.store().runs()?.len(), 1);
    assert!(
        has_line(
            &text,
            &format!(
                "baseline: run {}, 3 trials (each boots a candidate VM and then a judge VM)",
                run.to_hex().get(..12).ok_or("short")?
            )
        ),
        "{text}"
    );
    assert_eq!(
        trial_lines(&text),
        [
            "trial 0: scored probe_score 1",
            "trial 1: scored probe_score 1",
            "trial 2: scored probe_score 1",
        ],
        "{text}"
    );
    assert!(
        has_line(
            &text,
            "noise floor: median 1, spread 0 over 3 trials (0 skipped)"
        ),
        "{text}"
    );
    assert!(
        has_line(&text, "min_improvement: 5% (threshold 0.05 at the median)"),
        "{text}"
    );
    assert!(has_line(&text, "noise verdict: ok"), "{text}");
    assert!(
        text.lines().any(|line| line.starts_with("approx. cost: $")),
        "{text}"
    );

    let state = fold_run(prepared.store(), &run)?;
    let trials: Vec<u32> = state
        .trials(state.baseline(), &prepared.judge())
        .map(|(trial, _)| trial)
        .collect();
    assert_eq!(trials, [0, 1, 2]);
    Ok(())
}

#[test]
fn a_second_check_reuses_the_calibrated_run_and_launches_nothing() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let (first, first_text) = checked(&prepared)?;
    let run = *first.run().ok_or("no run")?;

    let jobs = FakeJobs::steady();
    let (result, text) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT + 60),
    )?;
    let outcome = result?;
    assert_eq!(jobs.requests(), 0);
    assert!(outcome.reused());
    assert_eq!(outcome.run(), Some(&run));
    assert_eq!(prepared.store().runs()?.len(), 1);
    assert!(
        has_line(
            &text,
            &format!(
                "baseline: reused run {} (no VM launched)",
                run.to_hex().get(..12).ok_or("short")?
            )
        ),
        "{text}"
    );
    assert_eq!(trial_lines(&text), trial_lines(&first_text));
    Ok(())
}

#[test]
fn fresh_starts_a_new_run_and_refuses_one_started_in_the_same_second() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    checked(&prepared)?;

    let jobs = FakeJobs::steady();
    let (result, _) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(true, STARTED_AT + 60),
    )?;
    let outcome = result?;
    assert!(!outcome.reused());
    assert_eq!(jobs.requests(), 3);
    assert_eq!(prepared.store().runs()?.len(), 2);

    let jobs = FakeJobs::steady();
    let (result, _) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(true, STARTED_AT),
    )?;
    assert!(
        matches!(result, Err(CheckError::RunExists { .. })),
        "{result:?}"
    );
    assert_eq!(jobs.requests(), 0);
    assert_eq!(prepared.store().runs()?.len(), 2);
    Ok(())
}

#[test]
fn an_edited_writable_file_changes_the_base_so_nothing_is_reused() -> TestResult {
    let fixture = Fixture::new()?;
    checked(&fixture.prepare()?)?;

    write(&fixture.project, "src/value.txt", b"value = 6\n")?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::steady();
    let (result, text) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT + 60),
    )?;
    let outcome = result?;
    assert!(!outcome.reused());
    assert_eq!(jobs.requests(), 3);
    assert_eq!(prepared.store().runs()?.len(), 2);
    assert!(!text.contains("reused"), "{text}");
    Ok(())
}

#[test]
fn a_noisy_baseline_prints_refuse_with_advice_and_refuses_nothing() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let jobs = FakeJobs::scoring(&[1.0, 1.2, 0.9]);
    let (result, text) = run_check(
        &prepared,
        &jobs,
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT),
    )?;
    let outcome = result?;
    assert!(!outcome.refused());
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == "noise verdict: refuse")
        .ok_or("no refuse verdict")?;
    assert!(
        lines
            .get(at + 1)
            .is_some_and(|line| line.starts_with("advice: ")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("noise floor: median 1, spread ")),
        "{text}"
    );
    Ok(())
}

#[test]
fn a_baseline_that_never_scores_is_baseline_unscored() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let (result, text) = run_check(
        &prepared,
        &FakeJobs::gate_fails(),
        &ProviderStatus::NotNeeded,
        options(false, STARTED_AT),
    )?;
    assert!(
        matches!(
            result,
            Err(CheckError::Driver(DriverError::BaselineUnscored { .. }))
        ),
        "{result:?}\n{text}"
    );
    Ok(())
}
