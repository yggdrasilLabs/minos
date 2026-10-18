//! The guest wire contract: the guest job, the guest status, and the input disk (GUEST-02, GUEST-03, GUEST-04;
//! 05-CONTEXT D-05, D-07, D-15).
//!
//! Fixtures are built from fixed bytes through public constructors only, so the same code always produces the same
//! values. The five golden texts are insta snapshots under `tests/snapshots/`. `.config/insta.yaml` sets
//! `update: no`, so the plain gate never writes or accepts one, and because insta trims trailing whitespace and
//! normalizes line endings when it compares, `guest_wire_golden_bytes_pinned` also pins the exact bytes of every
//! golden text with one SHA-256 literal.

use std::error::Error;

use minos_core::diff::Diff;
use minos_core::experiment::Experiment;
use minos_core::guest::{
    ApplyStatus, GUEST_JOB_VERSION, GUEST_STATUS_VERSION, GuestJob, GuestJobError, GuestRole,
    GuestStatus, GuestStatusError, MAX_GUEST_JOB_BYTES, MAX_STATUS_BYTES, OUTPUT_VSOCK_PORT,
    OutputStatus, STATUS_PATH, SetupStatus, SetupStep, StageEnd, StageReport,
};
use minos_core::id::{BlobId, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::limits::Limits;
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_core::outcome::Stage;
use minos_core::path::RelPath;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The `minos.toml` example in CLAUDE.md, section "Projects": build 2m, run 5m, the default judge budget, and the
/// default memory (2048 MiB) and scratch (1024 MiB).
const MANIFEST: &str = r#"[project]
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
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// A single-hunk modify of the manifest's one writable file.
const DIFF: &str = "\
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,3 +1,3 @@
 fn parse() {
-    old();
+    new();
 }
";

fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?)
}

fn base() -> TreeId {
    TreeId::of(b"t")
}

fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::of(&TreeId::of(b"judge"), manifest()?.judge()))
}

fn diff() -> Result<Diff, Box<dyn Error>> {
    let manifest = manifest()?;
    Ok(Diff::parse(DIFF, manifest.writable(), &Limits::standard())?)
}

fn diff_blob() -> Result<BlobId, Box<dyn Error>> {
    Ok(BlobId::of(diff()?.as_bytes()))
}

fn budget() -> Result<JobBudget, Box<dyn Error>> {
    Ok(JobBudget::from_budget(manifest()?.budget()))
}

fn baseline_request() -> Result<JobRequest, Box<dyn Error>> {
    Ok(JobRequest::baseline(base(), judge()?, 0, budget()?)?)
}

fn candidate_request() -> Result<JobRequest, Box<dyn Error>> {
    let experiment = Experiment::candidate(base(), diff()?);
    Ok(JobRequest::candidate(
        &experiment,
        diff_blob()?,
        judge()?,
        1,
        budget()?,
    )?)
}

fn candidate_job() -> Result<GuestJob, Box<dyn Error>> {
    Ok(GuestJob::candidate(&manifest()?, &candidate_request()?))
}

fn baseline_job() -> Result<GuestJob, Box<dyn Error>> {
    Ok(GuestJob::candidate(&manifest()?, &baseline_request()?))
}

fn judge_job() -> Result<GuestJob, Box<dyn Error>> {
    Ok(GuestJob::judge(&manifest()?, &candidate_request()?))
}

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

fn candidate_status() -> Result<GuestStatus, Box<dyn Error>> {
    Ok(GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        vec![
            StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
            StageReport::new(Stage::Run, StageEnd::Exited { code: 0 }),
        ],
        OutputStatus::Packed {
            files: 3,
            content_bytes: 4096,
        },
    )?)
}

fn apply_failed_status() -> Result<GuestStatus, Box<dyn Error>> {
    Ok(GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Failed {
            path: path("src/parse.rs")?,
            hunk: 1,
        },
        vec![
            StageReport::new(Stage::Build, StageEnd::Skipped),
            StageReport::new(Stage::Run, StageEnd::Skipped),
        ],
        OutputStatus::Packed {
            files: 1,
            content_bytes: 0,
        },
    )?)
}

fn judge_status() -> Result<GuestStatus, Box<dyn Error>> {
    Ok(GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        vec![StageReport::new(Stage::Judge, StageEnd::Exited { code: 0 })],
        OutputStatus::Packed {
            files: 2,
            content_bytes: 512,
        },
    )?)
}

fn sha256_hex(bytes: &[u8]) -> String {
    base16ct::lower::encode_string(&Sha256::digest(bytes))
}

fn text(bytes: Vec<u8>) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(bytes)?)
}

/// `json` parsed, changed by `change`, and serialized again.
fn mutate(
    json: &[u8],
    change: impl FnOnce(&mut Map<String, Value>) -> Result<(), Box<dyn Error>>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut value: Value = serde_json::from_slice(json)?;
    let object = value.as_object_mut().ok_or("not an object")?;
    change(object)?;
    Ok(serde_json::to_vec(&value)?)
}

/// The first stage object of a job's `stages` array.
fn first_stage(object: &mut Map<String, Value>) -> Result<&mut Map<String, Value>, Box<dyn Error>> {
    object
        .get_mut("stages")
        .and_then(Value::as_array_mut)
        .and_then(|stages| stages.first_mut())
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "no first stage".into())
}

/// The error `GuestJob::parse_json` returns for `bytes`; an error if it parsed. Also checks that the plain serde
/// path refuses the same bytes.
fn job_rejected(bytes: &[u8]) -> Result<GuestJobError, Box<dyn Error>> {
    if serde_json::from_slice::<GuestJob>(bytes).is_ok() {
        return Err("serde accepted the job".into());
    }
    match GuestJob::parse_json(bytes) {
        Ok(_) => Err("the job was accepted".into()),
        Err(error) => Ok(error),
    }
}

/// The error `GuestStatus::parse_json` returns for `bytes`; an error if it parsed.
fn status_rejected(bytes: &[u8]) -> Result<GuestStatusError, Box<dyn Error>> {
    if serde_json::from_slice::<GuestStatus>(bytes).is_ok() {
        return Err("serde accepted the status".into());
    }
    match GuestStatus::parse_json(bytes) {
        Ok(_) => Err("the status was accepted".into()),
        Err(error) => Ok(error),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Guest job
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn guest_constants() {
    assert_eq!(GUEST_JOB_VERSION, 1);
    assert_eq!(GUEST_STATUS_VERSION, 1);
    assert_eq!(MAX_GUEST_JOB_BYTES, 1_048_576);
    assert_eq!(MAX_STATUS_BYTES, 16_384);
    assert_eq!(OUTPUT_VSOCK_PORT, 1024);
    assert_eq!(STATUS_PATH, ".minos/status.json");
}

#[test]
fn guest_job_candidate_from_manifest() -> TestResult {
    let manifest = manifest()?;
    let request = candidate_request()?;
    let job = GuestJob::candidate(&manifest, &request);
    assert_eq!(job.role(), GuestRole::Candidate);
    assert_eq!(job.exp(), request.exp());
    assert_eq!(job.judge(), request.judge());
    assert_eq!(job.trial(), 1);
    assert_eq!(job.base(), Some(base()));
    assert_eq!(job.diff(), Some(diff_blob()?));
    assert_eq!(job.writable(), manifest.writable());
    let stages: Vec<Stage> = job.stages().iter().map(|stage| stage.stage()).collect();
    assert_eq!(stages, vec![Stage::Build, Stage::Run]);
    let build = job.stages().first().ok_or("no build stage")?;
    let run = job.stages().get(1).ok_or("no run stage")?;
    assert_eq!(build.argv(), manifest.commands().build().as_slice());
    assert_eq!(build.budget_ms(), 120_000);
    assert_eq!(run.argv(), manifest.commands().run().as_slice());
    assert_eq!(run.budget_ms(), 300_000);
    assert_eq!(job.memory_mib(), 2048);
    assert_eq!(job.scratch_mib(), 1024);
    assert_eq!(
        job.allowance_mib(),
        manifest.environment().candidate_allowance_mib()
    );
    assert_eq!(job.allowance_mib(), 768);
    Ok(())
}

#[test]
fn guest_job_baseline() -> TestResult {
    let job = baseline_job()?;
    assert_eq!(job.role(), GuestRole::Candidate);
    assert_eq!(job.base(), Some(base()));
    assert_eq!(job.diff(), None);
    assert_eq!(job.trial(), 0);
    Ok(())
}

#[test]
fn guest_job_judge() -> TestResult {
    let manifest = manifest()?;
    let request = candidate_request()?;
    let job = GuestJob::judge(&manifest, &request);
    assert_eq!(job.role(), GuestRole::Judge);
    assert_eq!(job.exp(), request.exp());
    assert_eq!(job.base(), None);
    assert_eq!(job.diff(), None);
    assert!(job.writable().is_empty());
    assert_eq!(job.stages().len(), 1);
    let stage = job.stages().first().ok_or("no judge stage")?;
    assert_eq!(stage.stage(), Stage::Judge);
    assert_eq!(stage.argv(), manifest.judge().command().as_slice());
    assert_eq!(stage.budget_ms(), 120_000);
    assert_eq!(job.memory_mib(), 2048);
    assert_eq!(job.scratch_mib(), 1024);
    Ok(())
}

#[test]
fn guest_job_round_trip() -> TestResult {
    for job in [candidate_job()?, baseline_job()?, judge_job()?] {
        let json = job.to_json()?;
        assert_eq!(GuestJob::parse_json(&json)?, job);
        assert_eq!(serde_json::from_slice::<GuestJob>(&json)?, job);
    }
    Ok(())
}

#[test]
fn guest_job_refuses_another_version() -> TestResult {
    let json = mutate(&candidate_job()?.to_json()?, |object| {
        object.insert("v".to_owned(), Value::from(2));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&json)?, GuestJobError::Version));
    Ok(())
}

#[test]
fn guest_job_refuses_an_unknown_field() -> TestResult {
    let json = mutate(&candidate_job()?.to_json()?, |object| {
        object.insert("extra".to_owned(), Value::from(0));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&json)?, GuestJobError::Json(_)));
    let nested = mutate(&candidate_job()?.to_json()?, |object| {
        first_stage(object)?.insert("extra".to_owned(), Value::from(0));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&nested)?, GuestJobError::Json(_)));
    Ok(())
}

#[test]
fn guest_job_refuses_wrong_stages() -> TestResult {
    let candidate = candidate_job()?.to_json()?;
    // Out of order: run then build.
    let swapped = mutate(&candidate, |object| {
        let stages = object
            .get_mut("stages")
            .and_then(Value::as_array_mut)
            .ok_or("no stages")?;
        stages.reverse();
        Ok(())
    })?;
    assert!(matches!(job_rejected(&swapped)?, GuestJobError::Stages));
    // Missing: build only.
    let missing = mutate(&candidate, |object| {
        let stages = object
            .get_mut("stages")
            .and_then(Value::as_array_mut)
            .ok_or("no stages")?;
        stages.truncate(1);
        Ok(())
    })?;
    assert!(matches!(job_rejected(&missing)?, GuestJobError::Stages));
    // A candidate with a judge stage.
    let judged = mutate(&candidate, |object| {
        first_stage(object)?.insert("stage".to_owned(), Value::from("judge"));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&judged)?, GuestJobError::Stages));
    // A judge with a build stage.
    let judge = mutate(&judge_job()?.to_json()?, |object| {
        first_stage(object)?.insert("stage".to_owned(), Value::from("build"));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&judge)?, GuestJobError::Stages));
    Ok(())
}

#[test]
fn guest_job_refuses_a_judge_with_a_base_diff_or_writable_set() -> TestResult {
    let judge = judge_job()?.to_json()?;
    let with_base = mutate(&judge, |object| {
        object.insert("base".to_owned(), Value::from(base().to_string()));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&with_base)?, GuestJobError::Base));
    let with_diff = mutate(&judge, |object| {
        object.insert("diff".to_owned(), Value::from(diff_blob()?.to_string()));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&with_diff)?, GuestJobError::Diff));
    let with_writable = mutate(&judge, |object| {
        object.insert(
            "writable".to_owned(),
            Value::from(vec![Value::from("src/parse.rs")]),
        );
        Ok(())
    })?;
    assert!(matches!(
        job_rejected(&with_writable)?,
        GuestJobError::Writable
    ));
    Ok(())
}

#[test]
fn guest_job_refuses_a_candidate_without_a_base_or_writable_set() -> TestResult {
    let candidate = candidate_job()?.to_json()?;
    let no_base = mutate(&candidate, |object| {
        object.insert("base".to_owned(), Value::Null);
        Ok(())
    })?;
    assert!(matches!(job_rejected(&no_base)?, GuestJobError::Base));
    let no_writable = mutate(&candidate, |object| {
        object.insert("writable".to_owned(), Value::Array(Vec::new()));
        Ok(())
    })?;
    assert!(matches!(
        job_rejected(&no_writable)?,
        GuestJobError::Writable
    ));
    Ok(())
}

#[test]
fn guest_job_refuses_a_diff_that_disagrees_with_the_baseline() -> TestResult {
    // A candidate experiment with no diff.
    let candidate = mutate(&candidate_job()?.to_json()?, |object| {
        object.insert("diff".to_owned(), Value::Null);
        Ok(())
    })?;
    assert!(matches!(job_rejected(&candidate)?, GuestJobError::Diff));
    // The baseline with a diff.
    let baseline = mutate(&baseline_job()?.to_json()?, |object| {
        object.insert("diff".to_owned(), Value::from(diff_blob()?.to_string()));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&baseline)?, GuestJobError::Diff));
    Ok(())
}

#[test]
fn guest_job_refuses_a_bad_argv() -> TestResult {
    let candidate = candidate_job()?.to_json()?;
    let empty = mutate(&candidate, |object| {
        first_stage(object)?.insert("argv".to_owned(), Value::Array(Vec::new()));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&empty)?, GuestJobError::Argv));
    let nul = mutate(&candidate, |object| {
        first_stage(object)?.insert(
            "argv".to_owned(),
            Value::from(vec![Value::from("cargo"), Value::from("bu\0ild")]),
        );
        Ok(())
    })?;
    assert!(matches!(job_rejected(&nul)?, GuestJobError::Argv));
    let empty_program = mutate(&candidate, |object| {
        first_stage(object)?.insert("argv".to_owned(), Value::from(vec![Value::from("")]));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&empty_program)?, GuestJobError::Argv));
    let long_entry = mutate(&candidate, |object| {
        first_stage(object)?.insert(
            "argv".to_owned(),
            Value::from(vec![Value::from("x".repeat(4097))]),
        );
        Ok(())
    })?;
    assert!(matches!(job_rejected(&long_entry)?, GuestJobError::Argv));
    let too_many = mutate(&candidate, |object| {
        first_stage(object)?.insert(
            "argv".to_owned(),
            Value::from(vec![Value::from("a"); 65]),
        );
        Ok(())
    })?;
    assert!(matches!(job_rejected(&too_many)?, GuestJobError::Argv));
    Ok(())
}

#[test]
fn guest_job_refuses_a_budget_out_of_range() -> TestResult {
    let candidate = candidate_job()?.to_json()?;
    for ms in [0_u64, 999, 3_600_001] {
        let json = mutate(&candidate, |object| {
            first_stage(object)?.insert("budget_ms".to_owned(), Value::from(ms));
            Ok(())
        })?;
        assert!(matches!(job_rejected(&json)?, GuestJobError::Budget), "{ms}");
    }
    Ok(())
}

#[test]
fn guest_job_refuses_memory_that_breaks_the_budget() -> TestResult {
    let candidate = candidate_job()?.to_json()?;
    // 512 MiB less the 256 MiB reserve leaves nothing beside 1024 MiB of scratch.
    for (memory, scratch) in [(512_u16, 1024_u16), (127, 64), (2048, 63), (16_385, 1024)] {
        let json = mutate(&candidate, |object| {
            object.insert("memory_mib".to_owned(), Value::from(memory));
            object.insert("scratch_mib".to_owned(), Value::from(scratch));
            Ok(())
        })?;
        assert!(
            matches!(job_rejected(&json)?, GuestJobError::Memory),
            "{memory} {scratch}"
        );
    }
    Ok(())
}

#[test]
fn guest_job_refuses_a_trial_over_the_cap() -> TestResult {
    let json = mutate(&candidate_job()?.to_json()?, |object| {
        object.insert("trial".to_owned(), Value::from(32));
        Ok(())
    })?;
    assert!(matches!(job_rejected(&json)?, GuestJobError::Trial));
    Ok(())
}

#[test]
fn guest_job_refuses_json_over_the_cap_before_parsing() -> TestResult {
    // Not JSON at all: the cap must refuse it before the parser sees a byte.
    let oversized = vec![b'x'; MAX_GUEST_JOB_BYTES + 1];
    assert!(matches!(
        GuestJob::parse_json(&oversized),
        Err(GuestJobError::TooLarge)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Guest status
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn guest_status_round_trip() -> TestResult {
    let more = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Baseline,
        vec![
            StageReport::new(Stage::Build, StageEnd::Signaled { signal: 9 }),
            StageReport::new(Stage::Run, StageEnd::TimedOut),
        ],
        OutputStatus::TooLarge,
    )?;
    let setup_failed = GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Failed {
            step: SetupStep::Unpack,
        },
        ApplyStatus::NotApplicable,
        vec![StageReport::new(Stage::Judge, StageEnd::SpawnFailed)],
        OutputStatus::NotPacked,
    )?;
    let empty = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Failed {
            step: SetupStep::Input,
        },
        ApplyStatus::Baseline,
        Vec::new(),
        OutputStatus::Invalid,
    )?;
    for status in [
        candidate_status()?,
        apply_failed_status()?,
        judge_status()?,
        more,
        setup_failed,
        empty,
    ] {
        let json = status.to_json()?;
        assert_eq!(GuestStatus::parse_json(&json)?, status);
        assert_eq!(serde_json::from_slice::<GuestStatus>(&json)?, status);
    }
    Ok(())
}

#[test]
fn guest_status_getters() -> TestResult {
    let status = candidate_status()?;
    assert_eq!(status.role(), GuestRole::Candidate);
    assert_eq!(status.setup(), SetupStatus::Ok);
    assert_eq!(status.apply(), &ApplyStatus::Applied);
    assert_eq!(status.stages().len(), 2);
    let run = status.stages().get(1).ok_or("no run report")?;
    assert_eq!(run.stage(), Stage::Run);
    assert_eq!(run.end(), StageEnd::Exited { code: 0 });
    assert_eq!(
        status.output(),
        OutputStatus::Packed {
            files: 3,
            content_bytes: 4096
        }
    );
    Ok(())
}

#[test]
fn guest_status_new_runs_the_checks() {
    let judge_stage = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        vec![StageReport::new(Stage::Judge, StageEnd::Exited { code: 0 })],
        OutputStatus::NotPacked,
    );
    assert!(matches!(judge_stage, Err(GuestStatusError::Stages)));
    let twice = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        vec![
            StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
            StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
        ],
        OutputStatus::NotPacked,
    );
    assert!(matches!(twice, Err(GuestStatusError::Stages)));
    let reversed = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        vec![
            StageReport::new(Stage::Run, StageEnd::Exited { code: 0 }),
            StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
        ],
        OutputStatus::NotPacked,
    );
    assert!(matches!(reversed, Err(GuestStatusError::Stages)));
    for signal in [0, 65, -1] {
        let signaled = GuestStatus::new(
            GuestRole::Judge,
            SetupStatus::Ok,
            ApplyStatus::NotApplicable,
            vec![StageReport::new(Stage::Judge, StageEnd::Signaled { signal })],
            OutputStatus::NotPacked,
        );
        assert!(
            matches!(signaled, Err(GuestStatusError::Signal)),
            "{signal}"
        );
    }
    let judge_applied = GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        Vec::new(),
        OutputStatus::NotPacked,
    );
    assert!(matches!(judge_applied, Err(GuestStatusError::Apply)));
    let candidate_not_applicable = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        Vec::new(),
        OutputStatus::NotPacked,
    );
    assert!(matches!(
        candidate_not_applicable,
        Err(GuestStatusError::Apply)
    ));
    let too_many_files = GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        Vec::new(),
        OutputStatus::Packed {
            files: 1025,
            content_bytes: 0,
        },
    );
    assert!(matches!(too_many_files, Err(GuestStatusError::Output)));
    let too_many_bytes = GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        Vec::new(),
        OutputStatus::Packed {
            files: 1,
            content_bytes: 16 * 1024 * 1024 + 1,
        },
    );
    assert!(matches!(too_many_bytes, Err(GuestStatusError::Output)));
}

#[test]
fn guest_status_refusals() -> TestResult {
    let candidate = candidate_status()?.to_json()?;
    // A candidate status with a judge stage.
    let judged = mutate(&candidate, |object| {
        first_stage(object)?.insert("stage".to_owned(), Value::from("judge"));
        Ok(())
    })?;
    assert!(matches!(status_rejected(&judged)?, GuestStatusError::Stages));
    // A stage listed twice.
    let twice = mutate(&candidate, |object| {
        first_stage(object)?.insert("stage".to_owned(), Value::from("run"));
        Ok(())
    })?;
    assert!(matches!(status_rejected(&twice)?, GuestStatusError::Stages));
    // A signal of 0 and above 64.
    for signal in [0, 65] {
        let json = mutate(&candidate, |object| {
            let mut end = Map::new();
            let mut signaled = Map::new();
            signaled.insert("signal".to_owned(), Value::from(signal));
            end.insert("signaled".to_owned(), Value::Object(signaled));
            first_stage(object)?.insert("end".to_owned(), Value::Object(end));
            Ok(())
        })?;
        assert!(
            matches!(status_rejected(&json)?, GuestStatusError::Signal),
            "{signal}"
        );
    }
    // Unknown fields, at the top and inside a report.
    let extra = mutate(&candidate, |object| {
        object.insert("note".to_owned(), Value::from("free text"));
        Ok(())
    })?;
    assert!(matches!(
        status_rejected(&extra)?,
        GuestStatusError::Json { .. }
    ));
    let nested = mutate(&candidate, |object| {
        first_stage(object)?.insert("note".to_owned(), Value::from("free text"));
        Ok(())
    })?;
    assert!(matches!(
        status_rejected(&nested)?,
        GuestStatusError::Json { .. }
    ));
    // Another version.
    let version = mutate(&candidate, |object| {
        object.insert("v".to_owned(), Value::from(2));
        Ok(())
    })?;
    assert!(matches!(
        status_rejected(&version)?,
        GuestStatusError::Version
    ));
    // A judge status that claims a diff was applied.
    let judge = mutate(&judge_status()?.to_json()?, |object| {
        object.insert("apply".to_owned(), Value::from("applied"));
        Ok(())
    })?;
    assert!(matches!(status_rejected(&judge)?, GuestStatusError::Apply));
    Ok(())
}

#[test]
fn guest_status_refuses_an_unknown_field_in_a_variant() -> TestResult {
    let json = mutate(&apply_failed_status()?.to_json()?, |object| {
        let mut failed = Map::new();
        failed.insert("path".to_owned(), Value::from("src/parse.rs"));
        failed.insert("hunk".to_owned(), Value::from(1));
        failed.insert("why".to_owned(), Value::from("free text"));
        let mut apply = Map::new();
        apply.insert("failed".to_owned(), Value::Object(failed));
        object.insert("apply".to_owned(), Value::Object(apply));
        Ok(())
    })?;
    assert!(matches!(
        status_rejected(&json)?,
        GuestStatusError::Json { .. }
    ));
    Ok(())
}

#[test]
fn guest_status_refuses_a_path_that_is_not_a_rel_path() -> TestResult {
    let json = mutate(&apply_failed_status()?.to_json()?, |object| {
        let mut failed = Map::new();
        failed.insert("path".to_owned(), Value::from("../etc/passwd"));
        failed.insert("hunk".to_owned(), Value::from(1));
        let mut apply = Map::new();
        apply.insert("failed".to_owned(), Value::Object(failed));
        object.insert("apply".to_owned(), Value::Object(apply));
        Ok(())
    })?;
    assert!(matches!(
        status_rejected(&json)?,
        GuestStatusError::Json { .. }
    ));
    Ok(())
}

#[test]
fn guest_status_refuses_bytes_over_the_cap_before_parsing() {
    let oversized = vec![b'x'; MAX_STATUS_BYTES + 1];
    assert!(matches!(
        GuestStatus::parse_json(&oversized),
        Err(GuestStatusError::TooLarge)
    ));
}

#[test]
fn guest_status_error_messages_carry_no_input_text() -> TestResult {
    let json = mutate(&candidate_status()?.to_json()?, |object| {
        object.insert("hostile_name".to_owned(), Value::from("hostile value"));
        Ok(())
    })?;
    let error = status_rejected(&json)?;
    let message = error.to_string();
    assert!(!message.contains("hostile"), "{message}");
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Golden texts
// ---------------------------------------------------------------------------------------------------------------

/// Every golden text, by snapshot name, in pin order.
fn goldens() -> Result<Vec<(&'static str, String)>, Box<dyn Error>> {
    Ok(vec![
        ("guest_job_candidate", text(candidate_job()?.to_json()?)?),
        ("guest_job_judge", text(judge_job()?.to_json()?)?),
        ("guest_status_candidate", text(candidate_status()?.to_json()?)?),
        (
            "guest_status_apply_failed",
            text(apply_failed_status()?.to_json()?)?,
        ),
        ("guest_status_judge", text(judge_status()?.to_json()?)?),
    ])
}

#[test]
fn guest_wire_golden() -> TestResult {
    for (name, golden) in goldens()? {
        assert!(!golden.contains('\n'), "{name} spans lines");
        insta::assert_snapshot!(name, golden);
    }
    Ok(())
}

#[test]
fn guest_wire_golden_bytes_pinned() -> TestResult {
    // The pin was computed outside the code under test, from the accepted snapshot bodies (the text after each
    // file's second `---` line), in the order of `goldens`, each followed by one newline:
    //
    //   cd crates/minos-core/tests/snapshots
    //   for n in guest_job_candidate guest_job_judge guest_status_candidate guest_status_apply_failed \
    //            guest_status_judge; do
    //     awk 'n >= 2 { print } /^---$/ { n++ }' "guest_wire__$n.snap"
    //   done | shasum -a 256
    let mut all = String::new();
    for (_, golden) in goldens()? {
        all.push_str(&golden);
        all.push('\n');
    }
    assert_eq!(sha256_hex(all.as_bytes()), "unpinned");
    Ok(())
}
