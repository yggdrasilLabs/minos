//! The runner pipeline end to end over a scripted backend and the memory store (LOOP-01, LOOP-02, LOOP-03,
//! JUDGE-03, JUDGE-04).
//!
//! The fake backend lives here because a runner test cannot link `minos-backend-krun`: `deny.toml` lets only
//! `minos-cli` depend on a backend crate, dev-dependencies included (07-RESEARCH Pitfall 13). The real VMs run the
//! same `Runner` in 07-14's ignored test. `FakeBackend` scripts one `(Ended, stream, elapsed)` per role, chosen by
//! `R::KIND`, can fail a launch or a wait for one role, and records each launch's role, key, input disk, and judge
//! data. Its `wait` builds `Finished` through `Finished::new`, which consumes the launch's `Cleared`, so nothing
//! here launches a plan that did not pass preflight.
//!
//! How the cases map to requirements:
//!
//! - LOOP-01: the order is candidate then judge, the judge is skipped for a failed candidate, and every blame
//!   decision (launch, wait, a guest setup failure, a full scratch, a wrong request) lands on the right side.
//! - LOOP-02: the Candidate plan has no judge data and names the base and diff; the Judge plan has the runner's
//!   judge data, and its `candidate_out` blob, read back and parsed, holds exactly the candidate's `out/` files.
//! - LOOP-03: usage is the host's elapsed time times the plan's machine, whatever timing the candidate forged.
//! - JUDGE-03: a failed gate is `Rejected(GateFailed)`, even with the objective missing.
//! - JUDGE-04: the result carries the backend's provenance, krun on arm64.
//! - D-09: each stage log is stored as a tail of at most `MAX_STAGE_LOG_BYTES`, named by stage.
//! - D-06: `check` launches nothing and refuses a spec over the ceiling or a missing judge data blob.

use std::collections::BTreeSet;
use std::error::Error;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use minos_core::diff::Diff;
use minos_core::diff::cumulative::CumulativeDiff;
use minos_core::experiment::Experiment;
use minos_core::guest::{
    ApplyStatus, GuestRole, GuestStatus, OutputStatus, SetupStatus, SetupStep, StageEnd,
    StageReport,
};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, TreeId};
use minos_core::job::{JobBudget, JobRequest, JobResult};
use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
use minos_core::limits::{Limits, MAX_STAGE_LOG_BYTES, MAX_STAGE_MS};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::metric::Scorecard;
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::usage::Usage;
use minos_runner::{ConfigCause, InfraCause, RUNNER_VCPUS, Runner, RunnerError};
use minos_sandbox::{
    Backend, Capabilities, Cleared, Ended, Finished, InfraError, InputDisk, JudgeData, LaunchError,
    LaunchKey, Machine, Role, RoleKind, RunningVm, SpecError,
};
use minos_store::{Blobs, MemoryStore};
use tar::Header;

type TestResult = Result<(), Box<dyn Error>>;

/// The runner under test: the scripted backend over the memory store.
type TestRunner = Runner<FakeBackend, MemoryStore>;

const GATE: &str = "identical_output";
const OBJECTIVE: &str = "throughput_mb_s";

/// The manifest's `[environment] memory_mib` default, which every plan's machine takes.
const MEMORY_MIB: u64 = 2048;

/// A scorecard that passes the gate and scores the objective.
const SCORED: &[u8] = br#"{"v":1,"metrics":[{"name":"identical_output","value":1,"held_out":false},{"name":"throughput_mb_s","value":12.5,"held_out":false}]}"#;

/// A scorecard whose gate is 0 and whose objective is missing.
const GATE_FAILED: &[u8] =
    br#"{"v":1,"metrics":[{"name":"identical_output","value":0,"held_out":false}]}"#;

/// A scorecard the candidate forges in its own output, claiming a huge objective.
const FORGED: &[u8] = br#"{"v":1,"metrics":[{"name":"identical_output","value":1,"held_out":false},{"name":"throughput_mb_s","value":1e300,"held_out":false}]}"#;

// ---------------------------------------------------------------------------------------------------------------
// The fake backend
// ---------------------------------------------------------------------------------------------------------------

/// How one role's launch ends: the host-observed end, the output stream, and the host-measured elapsed time.
#[derive(Clone)]
struct Script {
    ended: Ended,
    stream: Vec<u8>,
    elapsed: Duration,
}

/// What the fake saw at one launch.
#[derive(Clone, Copy)]
struct Launched {
    role: RoleKind,
    key: LaunchKey,
    input: InputDisk,
    judge_data: Option<JudgeData>,
}

impl Launched {
    /// The plan's judge data, as `LaunchPlan::judge_data` returned it at launch.
    const fn judge_data(&self) -> Option<JudgeData> {
        self.judge_data
    }
}

/// A backend that runs nothing: each launch ends as its role's script says.
struct FakeBackend {
    capabilities: Capabilities,
    candidate: Script,
    judge: Script,
    fail_launch: Option<RoleKind>,
    fail_wait: Option<RoleKind>,
    launched: Mutex<Vec<Launched>>,
}

impl FakeBackend {
    fn new(candidate: Script, judge: Script) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            capabilities: host()?,
            candidate,
            judge,
            fail_launch: None,
            fail_wait: None,
            launched: Mutex::new(Vec::new()),
        })
    }

    /// Every launch so far, in order. A poisoned lock is an error, never a panic.
    fn launched(&self) -> Result<Vec<Launched>, Box<dyn Error>> {
        let guard = self
            .launched
            .lock()
            .map_err(|_| "the launch record lock is poisoned")?;
        Ok(guard.clone())
    }
}

/// A fake running VM: the clearance it was launched with and its role's script.
struct FakeRunning<R: Role> {
    cleared: Cleared<R>,
    script: Script,
    fail: bool,
}

impl<R: Role> RunningVm<R> for FakeRunning<R> {
    fn wait(self) -> Result<Finished<R>, InfraError> {
        if self.fail {
            return Err(InfraError::Backend {
                what: "the fake wait failed",
            });
        }
        Ok(Finished::new(
            self.cleared,
            self.script.ended,
            self.script.stream,
            self.script.elapsed,
            Vec::new(),
        ))
    }
}

impl Backend for FakeBackend {
    type Running<R: Role> = FakeRunning<R>;

    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError> {
        let mut launched = self.launched.lock().map_err(|_| LaunchError::Backend {
            what: "the launch record lock is poisoned",
        })?;
        launched.push(Launched {
            role: R::KIND,
            key: *ok.plan().key(),
            input: *ok.plan().input(),
            judge_data: ok.plan().judge_data().copied(),
        });
        if self.fail_launch == Some(R::KIND) {
            return Err(LaunchError::Backend {
                what: "the fake launch failed",
            });
        }
        let script = match R::KIND {
            RoleKind::Judge => self.judge.clone(),
            _ => self.candidate.clone(),
        };
        Ok(FakeRunning {
            cleared: ok,
            script,
            fail: self.fail_wait == Some(R::KIND),
        })
    }
}

/// An arm64 krun host isolated by hardware, with Seatbelt and rlimits, room for every plan, and no refusals.
fn host() -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        BackendKind::Krun,
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        BTreeSet::new(),
    )
    .with_layers(BTreeSet::from([
        ConfinementLayer::Seatbelt,
        ConfinementLayer::Rlimits,
    ])))
}

// ---------------------------------------------------------------------------------------------------------------
// Output streams
// ---------------------------------------------------------------------------------------------------------------

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// A valid ustar output stream holding `files` in order, then two end blocks.
fn tar(files: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut header = Header::new_ustar();
        header.set_path(name)?;
        header.set_size(u64::try_from(data.len())?);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        out.resize(out.len() + padding, 0);
    }
    out.resize(out.len() + 2 * BLOCK, 0);
    Ok(out)
}

const PACKED: OutputStatus = OutputStatus::Packed {
    files: 4,
    content_bytes: 128,
};

const OK: StageEnd = StageEnd::Exited { code: 0 };

/// A status's JSON, built through `GuestStatus::new`.
fn status(
    role: GuestRole,
    setup: SetupStatus,
    apply: ApplyStatus,
    stages: &[(Stage, StageEnd)],
    output: OutputStatus,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let stages = stages
        .iter()
        .map(|(stage, end)| StageReport::new(*stage, *end))
        .collect();
    Ok(GuestStatus::new(role, setup, apply, stages, output)?.to_json()?)
}

/// A candidate status whose setup succeeded, with `apply` and the given build and run ends.
fn candidate_status(
    apply: ApplyStatus,
    build: StageEnd,
    run: StageEnd,
) -> Result<Vec<u8>, Box<dyn Error>> {
    status(
        GuestRole::Candidate,
        SetupStatus::Ok,
        apply,
        &[(Stage::Build, build), (Stage::Run, run)],
        PACKED,
    )
}

/// A candidate output stream around `status`: the status, both stage logs, two artifacts, and files that must
/// never reach the judge (a forged scorecard, a forged usage file, and a file outside `out/`).
fn candidate_stream(status: &[u8], run_log: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    tar(&[
        (".minos/build.log", b"compiling\n"),
        (".minos/run.log", run_log),
        (".minos/scorecard.json", FORGED),
        (".minos/status.json", status),
        (
            ".minos/usage.json",
            br#"{"vcpu_ms":999999999999,"mem_mib_ms":999999999999}"#,
        ),
        ("notes.txt", b"not an artifact"),
        ("out/result.json", br#"{"ok":true}"#),
        ("out/timing.json", br#"{"elapsed_ms":1}"#),
    ])
}

/// A judge output stream: its status, its log, and `card` as its scorecard.
fn judge_stream(status_json: &[u8], card: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    tar(&[
        (".minos/judge.log", b"judged\n"),
        (".minos/scorecard.json", card),
        (".minos/status.json", status_json),
    ])
}

/// A judge status whose setup succeeded and whose judge stage exited 0 with packed output.
fn judge_ok() -> Result<Vec<u8>, Box<dyn Error>> {
    status(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        &[(Stage::Judge, OK)],
        PACKED,
    )
}

fn exited(stream: Vec<u8>, elapsed_ms: u64) -> Script {
    Script {
        ended: Ended::Exited,
        stream,
        elapsed: Duration::from_millis(elapsed_ms),
    }
}

/// A candidate that applied, built, ran, and packed its output.
fn passing_candidate(elapsed_ms: u64) -> Result<Script, Box<dyn Error>> {
    let status = candidate_status(ApplyStatus::Applied, OK, OK)?;
    Ok(exited(candidate_stream(&status, b"ran\n")?, elapsed_ms))
}

/// A judge that exited 0 with `card` as its scorecard.
fn judge_with(card: &[u8], elapsed_ms: u64) -> Result<Script, Box<dyn Error>> {
    Ok(exited(judge_stream(&judge_ok()?, card)?, elapsed_ms))
}

// ---------------------------------------------------------------------------------------------------------------
// The project
// ---------------------------------------------------------------------------------------------------------------

fn manifest() -> Result<Manifest, Box<dyn Error>> {
    let text = format!(
        r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

[[judge.gate]]
metric = "{GATE}"

[[judge.objective]]
metric = "{OBJECTIVE}"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#
    );
    Ok(Manifest::parse_toml(&text)?)
}

/// The run's fixed inputs: the manifest, the base tree, the root image, the judge, and the stage budgets.
struct Project {
    manifest: Manifest,
    base: TreeId,
    root: ImageDigest,
    judge_data: JudgeData,
    judge: JudgeVersion,
    budget: JobBudget,
}

const BASE_TARBALL: &[u8] = b"the run's base tarball";
const ROOT_IMAGE: &[u8] = b"the root image";
const JUDGE_DATA: &[u8] = b"the judge's private data";

/// The project, with the base tarball and the root image stored in `store`, and the judge data stored when
/// `with_judge_data`.
fn project(store: &MemoryStore, with_judge_data: bool) -> Result<Project, Box<dyn Error>> {
    let manifest = manifest()?;
    let base = TreeId::of(BASE_TARBALL);
    store.put(BASE_TARBALL)?;
    let root = ImageDigest::of(ROOT_IMAGE);
    store.put(ROOT_IMAGE)?;
    let judge_blob = if with_judge_data {
        store.put(JUDGE_DATA)?
    } else {
        BlobId::of(JUDGE_DATA)
    };
    let budget = JobBudget::from_budget(manifest.budget());
    Ok(Project {
        manifest,
        base,
        root,
        judge_data: JudgeData::new(judge_blob),
        judge: JudgeVersion::from_str(&"ab".repeat(32))?,
        budget,
    })
}

/// The canonical diff that sets `parse` to return `value`.
fn diff(project: &Project, value: u32) -> Result<Diff, Box<dyn Error>> {
    let text = format!(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -1,3 +1,3 @@\n fn parse() -> u32 {{\n-    1\n+    {value}\n }}\n"
    );
    Ok(Diff::parse(
        &text,
        project.manifest.writable(),
        &Limits::standard(),
    )?)
}

/// Trial 0 of the candidate whose diff sets `parse` to `value`, with its diff stored in `store`.
fn candidate_request(
    project: &Project,
    store: &MemoryStore,
    value: u32,
) -> Result<JobRequest, Box<dyn Error>> {
    let diff = diff(project, value)?;
    let blob = store.put(diff.as_bytes())?;
    let id = ExperimentId::of(&project.base, &diff);
    let experiment = Experiment::candidate(CumulativeDiff::recorded(project.base, diff, &id)?);
    Ok(JobRequest::candidate(
        &experiment,
        blob,
        project.judge,
        0,
        project.budget,
    )?)
}

/// Trial 0 of the baseline.
fn baseline_request(project: &Project) -> Result<JobRequest, Box<dyn Error>> {
    Ok(JobRequest::baseline(
        project.base,
        project.judge,
        0,
        project.budget,
    )?)
}

fn runner(
    backend: FakeBackend,
    store: MemoryStore,
    project: &Project,
) -> Result<TestRunner, Box<dyn Error>> {
    Ok(Runner::new(
        backend,
        store,
        project.manifest.clone(),
        project.root,
        project.judge_data,
        project.judge,
    )?)
}

/// A passing candidate and a scoring judge, with the candidate request for value 2.
fn scoring_run() -> Result<(TestRunner, Project, JobRequest), Box<dyn Error>> {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    Ok((runner(backend, store, &project)?, project, request))
}

/// The paths in a ustar archive, in archive order.
fn archive_paths(bytes: &[u8]) -> Result<Vec<String>, Box<dyn Error>> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        paths.push(entry.header().path()?.to_string_lossy().into_owned());
    }
    Ok(paths)
}

fn scored() -> Result<Outcome, Box<dyn Error>> {
    Ok(Outcome::Scored(Scorecard::parse_json(
        SCORED,
        &Limits::standard(),
    )?))
}

/// The error of a run that was expected to fail.
fn run_error(runner: &TestRunner, request: &JobRequest) -> Result<RunnerError, Box<dyn Error>> {
    match runner.run(request) {
        Ok(result) => Err(format!("the run returned {:?}", result.outcome()).into()),
        Err(error) => Ok(error),
    }
}

fn launched_roles(runner: &TestRunner) -> Result<Vec<RoleKind>, Box<dyn Error>> {
    Ok(runner
        .backend()
        .launched()?
        .iter()
        .map(|launch| launch.role)
        .collect())
}

fn result_log(
    runner: &TestRunner,
    result: &JobResult,
    stage: Stage,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let blob = result
        .logs()
        .get(&stage)
        .ok_or_else(|| format!("no {stage:?} log"))?;
    Ok(runner.store().get(blob, u64::MAX)?)
}

// ---------------------------------------------------------------------------------------------------------------
// Order and isolation (LOOP-01, LOOP-02)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn passing_candidate_and_scoring_judge_are_scored_after_two_launches_with_distinct_keys()
-> TestResult {
    let (runner, _, request) = scoring_run()?;

    let result = runner.run(&request)?;

    assert_eq!(result.outcome(), &scored()?);
    assert_eq!(result.exp(), request.exp());
    let launched = runner.backend().launched()?;
    let roles: Vec<RoleKind> = launched.iter().map(|launch| launch.role).collect();
    assert_eq!(roles, vec![RoleKind::Candidate, RoleKind::Judge]);
    let keys: BTreeSet<LaunchKey> = launched.iter().map(|launch| launch.key).collect();
    assert_eq!(keys.len(), 2);
    Ok(())
}

#[test]
fn candidate_plan_has_no_judge_data_and_names_the_base_and_diff() -> TestResult {
    let (runner, project, request) = scoring_run()?;

    runner.run(&request)?;

    let launched = runner.backend().launched()?;
    let candidate = launched.first().ok_or("no candidate launch")?;
    assert_eq!(candidate.role, RoleKind::Candidate);
    assert_eq!(candidate.judge_data(), None);
    assert_eq!(candidate.input.base(), Some(project.base));
    assert_eq!(candidate.input.diff(), request.diff());
    assert!(request.diff().is_some());
    assert_eq!(candidate.input.candidate_out(), None);
    Ok(())
}

#[test]
fn judge_plan_carries_the_judge_data_and_only_the_candidates_out_files() -> TestResult {
    let (runner, project, request) = scoring_run()?;

    runner.run(&request)?;

    let launched = runner.backend().launched()?;
    let judge = launched.get(1).ok_or("no judge launch")?;
    assert_eq!(judge.role, RoleKind::Judge);
    assert_eq!(judge.judge_data(), Some(project.judge_data));
    assert_eq!(judge.input.base(), None);
    assert_eq!(judge.input.diff(), None);
    let candidate_out = judge.input.candidate_out().ok_or("no candidate_out")?;
    assert!(!judge.input.blobs().contains(project.judge_data.blob()));
    let archive = runner.store().get(&candidate_out, u64::MAX)?;
    assert_eq!(
        archive_paths(&archive)?,
        vec!["out/result.json".to_owned(), "out/timing.json".to_owned()]
    );
    Ok(())
}

#[test]
fn baseline_launches_with_no_diff_and_scores() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = baseline_request(&project)?;
    let status = candidate_status(ApplyStatus::Baseline, OK, OK)?;
    let candidate = exited(candidate_stream(&status, b"ran\n")?, 1000);
    let backend = FakeBackend::new(candidate, judge_with(SCORED, 500)?)?;
    let runner = runner(backend, store, &project)?;

    let result = runner.run(&request)?;

    assert_eq!(result.outcome(), &scored()?);
    let launched = runner.backend().launched()?;
    let candidate = launched.first().ok_or("no candidate launch")?;
    assert_eq!(candidate.input.diff(), None);
    assert_eq!(candidate.input.base(), Some(project.base));
    assert_eq!(launched.len(), 2);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Metering (LOOP-03)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn usage_is_host_elapsed_times_the_machine_whatever_the_candidate_forged() -> TestResult {
    let (runner, _, request) = scoring_run()?;
    let vcpus = u64::from(RUNNER_VCPUS);

    let result = runner.run(&request)?;

    assert_eq!(
        result.usage(),
        Usage::compute(1500 * vcpus, 1500 * MEMORY_MIB)
    );
    assert_eq!(
        result.judge_usage(),
        Some(Usage::compute(700 * vcpus, 700 * MEMORY_MIB))
    );
    assert_eq!(result.usage().total_tokens(), Some(0));
    Ok(())
}

#[test]
fn failed_build_is_rejected_and_the_judge_never_launches() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let status = candidate_status(
        ApplyStatus::Applied,
        StageEnd::Exited { code: 1 },
        StageEnd::Skipped,
    )?;
    let candidate = exited(candidate_stream(&status, b"")?, 900);
    let backend = FakeBackend::new(candidate, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let result = runner.run(&request)?;

    assert_eq!(result.outcome(), &Outcome::Rejected(Reason::BuildFailed));
    assert_eq!(result.judge_usage(), None);
    assert_eq!(launched_roles(&runner)?, vec![RoleKind::Candidate]);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Gates and provenance (JUDGE-03, JUDGE-04)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn judge_with_a_failed_gate_and_a_missing_objective_is_gate_failed() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(GATE_FAILED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let result = runner.run(&request)?;

    assert_eq!(
        result.outcome(),
        &Outcome::Rejected(Reason::GateFailed {
            gate: MetricName::from_str(GATE)?,
        })
    );
    Ok(())
}

#[test]
fn result_backend_is_the_hosts_provenance_for_the_root_image() -> TestResult {
    let (runner, project, request) = scoring_run()?;

    let result = runner.run(&request)?;

    assert_eq!(
        result.backend(),
        &runner.backend().capabilities().backend_info(project.root)
    );
    assert_eq!(result.backend().backend(), BackendKind::Krun);
    assert_eq!(result.backend().arch(), Arch::Arm64);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Log tails (D-09)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn logs_name_build_run_and_judge_and_a_long_run_log_keeps_its_tail() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let run_log: Vec<u8> = (0..MAX_STAGE_LOG_BYTES + 100)
        .map(|index| u8::try_from(index % 251).unwrap_or(0))
        .collect();
    let status = candidate_status(ApplyStatus::Applied, OK, OK)?;
    let candidate = exited(candidate_stream(&status, &run_log)?, 1500);
    let backend = FakeBackend::new(candidate, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let result = runner.run(&request)?;

    let stages: Vec<Stage> = result.logs().keys().copied().collect();
    assert_eq!(stages, vec![Stage::Build, Stage::Run, Stage::Judge]);
    assert_eq!(
        result_log(&runner, &result, Stage::Build)?,
        b"compiling\n".to_vec()
    );
    assert_eq!(
        result_log(&runner, &result, Stage::Judge)?,
        b"judged\n".to_vec()
    );
    let tail = run_log.get(100..).ok_or("the run log is too short")?;
    let stored = result_log(&runner, &result, Stage::Run)?;
    assert_eq!(stored.len(), MAX_STAGE_LOG_BYTES);
    assert_eq!(stored.as_slice(), tail);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Blame (LOOP-01; D-16)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn launch_error_is_infra_and_nothing_else_launches() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let mut backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    backend.fail_launch = Some(RoleKind::Candidate);
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Infra {
            cause: InfraCause::Launch
        }
    ));
    assert!(error.is_infra());
    assert_eq!(launched_roles(&runner)?, vec![RoleKind::Candidate]);
    Ok(())
}

#[test]
fn judge_wait_error_is_infra() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let mut backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    backend.fail_wait = Some(RoleKind::Judge);
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Infra {
            cause: InfraCause::Wait
        }
    ));
    assert_eq!(
        launched_roles(&runner)?,
        vec![RoleKind::Candidate, RoleKind::Judge]
    );
    Ok(())
}

#[test]
fn judge_input_setup_failure_is_infra_despite_its_placeholder_role() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    // PID 1 could not parse its job, so it sent the placeholder role: a candidate with apply `baseline`.
    let placeholder = status(
        GuestRole::Candidate,
        SetupStatus::Failed {
            step: SetupStep::Input,
        },
        ApplyStatus::Baseline,
        &[],
        OutputStatus::NotPacked,
    )?;
    let judge = exited(tar(&[(".minos/status.json", &placeholder)])?, 300);
    let backend = FakeBackend::new(passing_candidate(1500)?, judge)?;
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Infra {
            cause: InfraCause::GuestSetup {
                step: SetupStep::Input
            }
        }
    ));
    Ok(())
}

#[test]
fn scratch_full_is_a_configuration_error() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = candidate_request(&project, &store, 2)?;
    let full = status(
        GuestRole::Candidate,
        SetupStatus::Failed {
            step: SetupStep::ScratchFull,
        },
        ApplyStatus::Applied,
        &[],
        OutputStatus::NotPacked,
    )?;
    let candidate = exited(tar(&[(".minos/status.json", &full)])?, 400);
    let backend = FakeBackend::new(candidate, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Config {
            cause: ConfigCause::ScratchFull
        }
    ));
    assert_eq!(launched_roles(&runner)?, vec![RoleKind::Candidate]);
    Ok(())
}

#[test]
fn diff_blob_of_another_experiment_is_diff_mismatch_and_nothing_launches() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let ours = candidate_request(&project, &store, 2)?;
    let theirs = candidate_request(&project, &store, 3)?;
    let other_blob = theirs.diff().ok_or("no diff blob")?;
    // A request the loop got wrong: our experiment, their diff blob. The wire form checks only the baseline rule.
    let budget = project.budget;
    let json = format!(
        r#"{{"v":1,"exp":"{}","base":"{}","diff":"{}","judge":"{}","trial":0,"budget":{{"build_ms":{},"run_ms":{},"judge_ms":{}}}}}"#,
        ours.exp(),
        project.base,
        other_blob,
        project.judge,
        budget.build_ms(),
        budget.run_ms(),
        budget.judge_ms(),
    );
    let request = JobRequest::parse_json(json.as_bytes())?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Config {
            cause: ConfigCause::DiffMismatch
        }
    ));
    assert!(runner.backend().launched()?.is_empty());
    Ok(())
}

#[test]
fn request_under_another_judge_version_is_judge_mismatch_and_nothing_launches() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = JobRequest::baseline(
        project.base,
        JudgeVersion::from_str(&"cd".repeat(32))?,
        0,
        project.budget,
    )?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let error = run_error(&runner, &request)?;

    assert!(matches!(
        error,
        RunnerError::Config {
            cause: ConfigCause::JudgeMismatch
        }
    ));
    assert!(runner.backend().launched()?.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Check before generation 0 (D-06)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn check_on_the_baseline_launches_nothing_and_succeeds() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let request = baseline_request(&project)?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    runner.check(&request)?;

    assert!(runner.backend().launched()?.is_empty());
    Ok(())
}

#[test]
fn check_with_a_judge_budget_over_the_launch_ceiling_is_a_spec_error() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, true)?;
    let budget = JobBudget::new(60_000, 120_000, MAX_STAGE_MS)?;
    let request = JobRequest::baseline(project.base, project.judge, 0, budget)?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let error = match runner.check(&request) {
        Ok(()) => return Err("the check passed".into()),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RunnerError::Config {
            cause: ConfigCause::Spec {
                error: SpecError::DurationOverCeiling { .. }
            }
        }
    ));
    assert!(runner.backend().launched()?.is_empty());
    Ok(())
}

#[test]
fn check_without_the_judge_data_blob_is_blob_missing() -> TestResult {
    let store = MemoryStore::new();
    let project = project(&store, false)?;
    let request = baseline_request(&project)?;
    let backend = FakeBackend::new(passing_candidate(1500)?, judge_with(SCORED, 700)?)?;
    let runner = runner(backend, store, &project)?;

    let error = match runner.check(&request) {
        Ok(()) => return Err("the check passed".into()),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RunnerError::Config {
            cause: ConfigCause::BlobMissing
        }
    ));
    assert!(runner.backend().launched()?.is_empty());
    Ok(())
}
