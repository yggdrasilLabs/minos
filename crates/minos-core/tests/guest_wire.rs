//! The guest wire contract: the guest job, the guest status, and the input disk (GUEST-02, GUEST-03, GUEST-04;
//! 05-CONTEXT D-05, D-07, D-15).
//!
//! Fixtures are built from fixed bytes through public constructors only, so the same code always produces the same
//! values. The five golden texts are insta snapshots under `tests/snapshots/`. `.config/insta.yaml` sets
//! `update: no`, so the plain gate never writes or accepts one, and because insta trims trailing whitespace and
//! normalizes line endings when it compares, `guest_wire_golden_bytes_pinned` also pins the exact bytes of every
//! golden text with one SHA-256 literal.

use std::error::Error;
use std::io::Cursor;

use minos_core::chunk::ChunkToken;
use minos_core::diff::Diff;
use minos_core::diff::cumulative::CumulativeDiff;
use minos_core::experiment::Experiment;
use minos_core::guest::{
    ApplyStatus, GUEST_JOB_VERSION, GUEST_STATUS_VERSION, GuestJob, GuestJobError, GuestRole,
    GuestStatus, GuestStatusError, MAX_GUEST_JOB_BYTES, MAX_STATUS_BYTES, OUTPUT_VSOCK_PORT,
    OutputStatus, STATUS_PATH, SetupStatus, SetupStep, StageEnd, StageReport, StageSpec,
};
use minos_core::guest::{
    DISK_HEADER_BYTES, DISK_MAGIC, DISK_VERSION, DiskError, InputSections, MAX_BASE_TAR_BYTES,
    MAX_CANDIDATE_OUT_BYTES, MAX_GUEST_INPUT_BYTES, MAX_JUDGE_DATA_BYTES, SECTION_ENTRY_BYTES,
    SECTOR_BYTES, SectionKind, read_input_disk, write_input_disk,
};
use minos_core::id::{BlobId, ExperimentId, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::limits::{GUEST_RESERVE_MIB, Limits, MAX_DIFF_BYTES, MAX_OUTPUT_STREAM_BYTES};
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_core::outcome::Stage;
use minos_core::path::RelPath;
use proptest::prelude::*;
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
    let diff = diff()?;
    let id = ExperimentId::of(&base(), &diff);
    let experiment = Experiment::candidate(CumulativeDiff::recorded(base(), diff, &id)?);
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
    assert_eq!(job.judge_version(), request.judge());
    assert_eq!(job.trial(), 1);
    assert_eq!(job.base(), Some(base()));
    assert_eq!(job.diff(), Some(diff_blob()?));
    assert_eq!(job.writable(), manifest.writable());
    let stages: Vec<Stage> = job.stages().iter().map(StageSpec::stage).collect();
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
        first_stage(object)?.insert("argv".to_owned(), Value::from(vec![Value::from("a"); 65]));
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
        assert!(
            matches!(job_rejected(&json)?, GuestJobError::Budget),
            "{ms}"
        );
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
fn guest_job_refuses_json_over_the_cap_before_parsing() {
    // Not JSON at all: the cap must refuse it before the parser sees a byte.
    let oversized = vec![b'x'; MAX_GUEST_JOB_BYTES + 1];
    assert!(matches!(
        GuestJob::parse_json(&oversized),
        Err(GuestJobError::TooLarge)
    ));
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
fn guest_status_scratch_full_step_is_on_the_wire() -> TestResult {
    // The step added by review WR-02 serializes as snake_case like the others and round-trips under version 1:
    // an added variant keeps every earlier status parseable. Asserted inline, not as an insta golden, so the
    // gate never has to accept a new snapshot for it.
    let status = GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Failed {
            step: SetupStep::ScratchFull,
        },
        ApplyStatus::Baseline,
        Vec::new(),
        OutputStatus::NotPacked,
    )?;
    let json = status.to_json()?;
    let text = std::str::from_utf8(&json)?;
    assert!(
        text.contains(r#""setup":{"failed":{"step":"scratch_full"}}"#),
        "{text}"
    );
    assert_eq!(GuestStatus::parse_json(&json)?, status);
    assert_eq!(
        serde_json::to_value(SetupStep::ScratchFull)?,
        Value::String("scratch_full".to_owned())
    );
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
            vec![StageReport::new(
                Stage::Judge,
                StageEnd::Signaled { signal },
            )],
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
    assert!(matches!(
        status_rejected(&judged)?,
        GuestStatusError::Stages
    ));
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
        (
            "guest_status_candidate",
            text(candidate_status()?.to_json()?)?,
        ),
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
    assert_eq!(
        sha256_hex(all.as_bytes()),
        "cfae3d32369d3ef720e2afc455327bcd30f106a7050227bbc4663aee922b3a4a"
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Input disk
// ---------------------------------------------------------------------------------------------------------------

const TOKEN: [u8; 32] = [0x5a; 32];

fn token() -> ChunkToken {
    ChunkToken::from_bytes(TOKEN)
}

/// The disk of `sections` under the fixed token.
fn write(sections: &[(SectionKind, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    let length = write_input_disk(&mut out, &token(), sections)?;
    assert_eq!(u64::try_from(out.len())?, length);
    Ok(out)
}

/// `bytes` read back as a disk.
fn read(bytes: &[u8]) -> Result<InputSections, DiskError> {
    read_input_disk(&mut Cursor::new(bytes))
}

/// The error reading `bytes` gives; an error if it read.
fn refused(bytes: &[u8]) -> Result<DiskError, Box<dyn Error>> {
    match read(bytes) {
        Ok(_) => Err("the disk was accepted".into()),
        Err(error) => Ok(error),
    }
}

/// `bytes` with `with` written at `at`.
fn patch(bytes: &mut [u8], at: usize, with: &[u8]) -> Result<(), Box<dyn Error>> {
    let end = at.checked_add(with.len()).ok_or("overflow")?;
    bytes
        .get_mut(at..end)
        .ok_or("patch out of range")?
        .copy_from_slice(with);
    Ok(())
}

/// The big-endian `u32` at `at`.
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Box<dyn Error>> {
    let field: [u8; 4] = bytes.get(at..at + 4).ok_or("short")?.try_into()?;
    Ok(u32::from_be_bytes(field))
}

/// The big-endian `u64` at `at`.
fn u64_at(bytes: &[u8], at: usize) -> Result<u64, Box<dyn Error>> {
    let field: [u8; 8] = bytes.get(at..at + 8).ok_or("short")?.try_into()?;
    Ok(u64::from_be_bytes(field))
}

/// Where table entry `index` starts.
const fn entry_at(index: usize) -> usize {
    DISK_HEADER_BYTES + index * SECTION_ENTRY_BYTES
}

/// A header and table with no section bytes: `entries` are `(kind, offset, length)`, each with a zero digest.
fn header_and_table(count: u32, entries: &[(u32, u64, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&DISK_MAGIC);
    out.extend_from_slice(&DISK_VERSION.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&TOKEN);
    for (kind, offset, length) in entries {
        out.extend_from_slice(&kind.to_be_bytes());
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&[0; 32]);
    }
    out
}

fn job_bytes() -> &'static [u8] {
    br#"{"k":1}"#
}

#[test]
fn disk_constants() {
    assert_eq!(&DISK_MAGIC, b"MINOSJB2");
    assert_eq!(DISK_VERSION, 2);
    assert_eq!(DISK_HEADER_BYTES, 48);
    assert_eq!(SECTION_ENTRY_BYTES, 52);
    assert_eq!(SECTOR_BYTES, 512);
    assert_eq!(MAX_BASE_TAR_BYTES, 83_886_080);
    assert_eq!(MAX_CANDIDATE_OUT_BYTES, MAX_OUTPUT_STREAM_BYTES);
    assert_eq!(SectionKind::Job.cap(), 1_048_576);
    assert_eq!(SectionKind::Base.cap(), MAX_BASE_TAR_BYTES);
    assert_eq!(
        SectionKind::Diff.cap(),
        u64::try_from(MAX_DIFF_BYTES).unwrap_or(0)
    );
    assert_eq!(SectionKind::JudgeData.cap(), MAX_JUDGE_DATA_BYTES);
    assert_eq!(SectionKind::CandidateOut.cap(), MAX_CANDIDATE_OUT_BYTES);
    for (code, kind) in [
        (1, SectionKind::Job),
        (2, SectionKind::Base),
        (3, SectionKind::Diff),
        (4, SectionKind::JudgeData),
        (5, SectionKind::CandidateOut),
    ] {
        assert_eq!(SectionKind::from_u32(code), Some(kind));
        assert_eq!(kind.code(), code);
    }
    assert_eq!(SectionKind::from_u32(0), None);
    assert_eq!(SectionKind::from_u32(6), None);
}

#[test]
fn disk_layout() -> TestResult {
    let base = vec![7_u8; 600];
    let diff = b"diff!".to_vec();
    let bytes = write(&[
        (SectionKind::Diff, &diff),
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, &base),
    ])?;
    assert_eq!(bytes.get(..8), Some(&b"MINOSJB2"[..]));
    assert_eq!(u32_at(&bytes, 8)?, 2);
    assert_eq!(u32_at(&bytes, 12)?, 3);
    assert_eq!(bytes.get(16..48), Some(&TOKEN[..]));
    // Ascending kind order, whatever the order given; the first section starts on the sector after the table
    // (48 + 3 * 52 = 204 bytes), and each later one on the sector after the one before.
    let expected: [(u32, u64, &[u8]); 3] =
        [(1, 512, job_bytes()), (2, 1024, &base), (3, 2048, &diff)];
    for (index, (kind, offset, content)) in expected.into_iter().enumerate() {
        let at = entry_at(index);
        assert_eq!(u32_at(&bytes, at)?, kind);
        assert_eq!(u64_at(&bytes, at + 4)?, offset);
        assert_eq!(offset % SECTOR_BYTES, 0);
        let length = u64_at(&bytes, at + 12)?;
        assert_eq!(length, u64::try_from(content.len())?);
        let digest: [u8; 32] = Sha256::digest(content).into();
        assert_eq!(bytes.get(at + 20..at + 52), Some(&digest[..]));
        let start = usize::try_from(offset)?;
        assert_eq!(bytes.get(start..start + content.len()), Some(content));
    }
    // 2048 + 5 rounded up to the sector.
    assert_eq!(bytes.len(), 2560);
    assert_eq!(u64::try_from(bytes.len())? % SECTOR_BYTES, 0);
    // Every byte that is not header, table, or section is zero.
    let padding: usize = [
        bytes.get(204..512),
        bytes.get(512 + 7..1024),
        bytes.get(1624..2048),
        bytes.get(2053..2560),
    ]
    .into_iter()
    .map(|range| range.map_or(1, |range| range.iter().filter(|byte| **byte != 0).count()))
    .sum();
    assert_eq!(padding, 0);
    Ok(())
}

#[test]
fn disk_golden() -> TestResult {
    // The pin was computed outside the code under test, by building the disk from the layout alone:
    //
    //   python3 -c 'import hashlib, struct
    //   job = b"{\"k\":1}"; base = bytes(600); token = bytes([0x5a]) * 32
    //   header = b"MINOSJB2" + struct.pack(">II", 2, 2) + token
    //   table = struct.pack(">IQQ", 1, 512, len(job)) + hashlib.sha256(job).digest() \
    //         + struct.pack(">IQQ", 2, 1024, len(base)) + hashlib.sha256(base).digest()
    //   disk = (header + table).ljust(512, b"\0") + job.ljust(512, b"\0") + base.ljust(1024, b"\0")
    //   print(len(disk), hashlib.sha256(disk).hexdigest())'
    let base = vec![0_u8; 600];
    let bytes = write(&[(SectionKind::Job, job_bytes()), (SectionKind::Base, &base)])?;
    assert_eq!(bytes.len(), 2048);
    assert_eq!(
        sha256_hex(&bytes),
        "f59100e4e4233be146f65fb87860ebea5942b30b426eb3cc4fd90953bc0ef83c"
    );
    Ok(())
}

proptest! {
    #[test]
    fn disk_round_trip(
        token_bytes in any::<[u8; 32]>(),
        job in proptest::collection::vec(any::<u8>(), 0..1500),
        others in proptest::collection::vec(proptest::option::of(proptest::collection::vec(any::<u8>(), 0..1500)), 4),
    ) {
        let kinds = [SectionKind::Base, SectionKind::Diff, SectionKind::JudgeData, SectionKind::CandidateOut];
        let mut sections: Vec<(SectionKind, &[u8])> = vec![(SectionKind::Job, &job)];
        for (kind, content) in kinds.into_iter().zip(&others) {
            if let Some(content) = content {
                sections.push((kind, content));
            }
        }
        // Reverse, so the writer must sort.
        sections.reverse();
        let mut out = Vec::new();
        let written = write_input_disk(&mut out, &ChunkToken::from_bytes(token_bytes), &sections);
        prop_assert!(written.is_ok());
        let mut read = read_input_disk(&mut Cursor::new(&out)).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(read.token().ct_eq(&token_bytes));
        for (kind, content) in &sections {
            prop_assert!(read.contains(*kind));
            let taken = read.take(*kind);
            prop_assert_eq!(taken.as_deref(), Some(*content));
            prop_assert!(!read.contains(*kind));
        }
        for kind in kinds {
            prop_assert_eq!(read.take(kind), None);
        }
    }
}

#[test]
fn disk_reads_exactly_to_the_last_section() -> TestResult {
    let bytes = write(&[(SectionKind::Job, job_bytes()), (SectionKind::Diff, b"abc")])?;
    // Without the trailing padding the disk still reads, and the reader stops at the last section's end.
    let end = 1024 + 3;
    let trimmed = bytes.get(..end).ok_or("short")?;
    let mut cursor = Cursor::new(trimmed);
    read_input_disk(&mut cursor)?;
    assert_eq!(cursor.position(), u64::try_from(end)?);
    let mut cursor = Cursor::new(&bytes);
    read_input_disk(&mut cursor)?;
    assert_eq!(cursor.position(), u64::try_from(end)?);
    Ok(())
}

#[test]
fn disk_sections_debug_shows_no_bytes() -> TestResult {
    let sections = read(&write(&[(SectionKind::Job, b"secret-job")])?)?;
    let debug = format!("{sections:?}");
    assert!(!debug.contains("secret"), "{debug}");
    assert!(!debug.contains("90"), "{debug}");
    Ok(())
}

#[test]
fn disk_writer_refusals() {
    let mut out = Vec::new();
    assert!(matches!(
        write_input_disk(&mut out, &token(), &[]),
        Err(DiskError::SectionCount { count: 0 })
    ));
    assert!(matches!(
        write_input_disk(
            &mut out,
            &token(),
            &[
                (SectionKind::Job, job_bytes()),
                (SectionKind::Job, job_bytes())
            ]
        ),
        Err(DiskError::DuplicateKind)
    ));
    assert!(matches!(
        write_input_disk(&mut out, &token(), &[(SectionKind::Base, b"t")]),
        Err(DiskError::MissingSection { kind: 1 })
    ));
    let oversized = vec![0_u8; 1_048_577];
    assert!(matches!(
        write_input_disk(&mut out, &token(), &[(SectionKind::Job, &oversized)]),
        Err(DiskError::TooLarge {
            kind: 1,
            length: 1_048_577,
            cap: 1_048_576
        })
    ));
    assert!(out.is_empty(), "a refused disk writes nothing");
}

#[test]
fn disk_refusals_header() -> TestResult {
    let good = write(&[(SectionKind::Job, job_bytes()), (SectionKind::Base, b"t")])?;
    read(&good)?;

    let mut magic = good.clone();
    patch(&mut magic, 7, b"1")?;
    assert!(matches!(refused(&magic)?, DiskError::Magic));

    let mut version = good.clone();
    patch(&mut version, 8, &1_u32.to_be_bytes())?;
    assert!(matches!(
        refused(&version)?,
        DiskError::Version { found: 1 }
    ));

    for count in [0_u32, 6] {
        let mut wrong = good.clone();
        patch(&mut wrong, 12, &count.to_be_bytes())?;
        assert!(
            matches!(refused(&wrong)?, DiskError::SectionCount { count: found } if found == count),
            "{count}"
        );
    }

    // A truncated header or table.
    for end in [0, 20, 47, 48 + 51, 48 + 52 + 51] {
        let short = good.get(..end).ok_or("short")?;
        assert!(matches!(refused(short)?, DiskError::Io(_)), "{end}");
    }
    Ok(())
}

#[test]
fn disk_refusals_table() -> TestResult {
    let good = write(&[(SectionKind::Job, job_bytes()), (SectionKind::Base, b"t")])?;

    let mut unknown = good.clone();
    patch(&mut unknown, entry_at(1), &9_u32.to_be_bytes())?;
    assert!(matches!(
        refused(&unknown)?,
        DiskError::UnknownKind { kind: 9 }
    ));

    let mut zero_kind = good.clone();
    patch(&mut zero_kind, entry_at(0), &0_u32.to_be_bytes())?;
    assert!(matches!(
        refused(&zero_kind)?,
        DiskError::UnknownKind { kind: 0 }
    ));

    let mut duplicate = good.clone();
    patch(&mut duplicate, entry_at(1), &1_u32.to_be_bytes())?;
    assert!(matches!(refused(&duplicate)?, DiskError::DuplicateKind));

    // Kinds out of order.
    let mut swapped = good.clone();
    patch(&mut swapped, entry_at(0), &2_u32.to_be_bytes())?;
    patch(&mut swapped, entry_at(1), &1_u32.to_be_bytes())?;
    assert!(matches!(refused(&swapped)?, DiskError::Layout));

    // Offsets: overlapping the header and table, misaligned, overlapping the section before, out of order, and
    // leaving a gap.
    for (index, offset) in [
        (0, 0_u64),
        (0, 48),
        (0, 513),
        (1, 512),
        (1, 0),
        (1, 1536),
        (1, 1025),
    ] {
        let mut wrong = good.clone();
        patch(&mut wrong, entry_at(index) + 4, &offset.to_be_bytes())?;
        assert!(
            matches!(refused(&wrong)?, DiskError::Layout),
            "{index} {offset}"
        );
    }

    // A length over its kind's cap is refused from the table alone: there are no section bytes to read.
    let job_cap = header_and_table(1, &[(1, 512, 1_048_577)]);
    assert!(matches!(
        refused(&job_cap)?,
        DiskError::TooLarge {
            kind: 1,
            length: 1_048_577,
            cap: 1_048_576
        }
    ));
    let base_cap = header_and_table(2, &[(1, 512, 7), (2, 1024, MAX_BASE_TAR_BYTES + 1)]);
    assert!(matches!(
        refused(&base_cap)?,
        DiskError::TooLarge { kind: 2, .. }
    ));

    // No JOB section.
    let mut no_job = write(&[(SectionKind::Job, job_bytes())])?;
    patch(&mut no_job, entry_at(0), &3_u32.to_be_bytes())?;
    assert!(matches!(
        refused(&no_job)?,
        DiskError::MissingSection { kind: 1 }
    ));
    Ok(())
}

#[test]
fn disk_refusals_sections() -> TestResult {
    let good = write(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, b"tree"),
    ])?;

    // A flipped byte in the base section.
    let mut flipped = good.clone();
    patch(&mut flipped, 1024, b"T")?;
    assert!(matches!(refused(&flipped)?, DiskError::Digest { kind: 2 }));

    // A wrong digest in the table.
    let mut digest = good.clone();
    patch(&mut digest, entry_at(0) + 20, &[0; 32])?;
    assert!(matches!(refused(&digest)?, DiskError::Digest { kind: 1 }));

    // A stream cut inside the last section, and one cut in the padding before it.
    for end in [1024 + 3, 800] {
        let short = good.get(..end).ok_or("short")?;
        assert!(matches!(refused(short)?, DiskError::Io(_)), "{end}");
    }

    // Padding that is not zero.
    for at in [300, 600] {
        let mut dirty = good.clone();
        patch(&mut dirty, at, b"x")?;
        assert!(matches!(refused(&dirty)?, DiskError::Layout), "{at}");
    }
    Ok(())
}

/// The sections of a disk holding `sections`.
fn sections_of(sections: &[(SectionKind, &[u8])]) -> Result<InputSections, Box<dyn Error>> {
    Ok(read(&write(sections)?)?)
}

#[test]
fn check_role_candidate() -> TestResult {
    let job = candidate_job()?;
    let diff = diff()?;
    let diff_bytes = diff.as_bytes();
    // `base()` is the tree of the tarball bytes `t`.
    let good = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, b"t"),
        (SectionKind::Diff, diff_bytes),
    ])?;
    good.check_role(&job)?;

    let no_base = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Diff, diff_bytes),
    ])?;
    assert!(matches!(
        no_base.check_role(&job),
        Err(DiskError::MissingSection { kind: 2 })
    ));
    let no_diff = sections_of(&[(SectionKind::Job, job_bytes()), (SectionKind::Base, b"t")])?;
    assert!(matches!(
        no_diff.check_role(&job),
        Err(DiskError::MissingSection { kind: 3 })
    ));
    let other_base = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, b"u"),
        (SectionKind::Diff, diff_bytes),
    ])?;
    assert!(matches!(
        other_base.check_role(&job),
        Err(DiskError::BaseMismatch)
    ));
    let other_diff = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, b"t"),
        (SectionKind::Diff, b"another diff"),
    ])?;
    assert!(matches!(
        other_diff.check_role(&job),
        Err(DiskError::DiffMismatch)
    ));
    for kind in [SectionKind::JudgeData, SectionKind::CandidateOut] {
        let extra = sections_of(&[
            (SectionKind::Job, job_bytes()),
            (SectionKind::Base, b"t"),
            (SectionKind::Diff, diff_bytes),
            (kind, b"x"),
        ])?;
        assert!(
            matches!(extra.check_role(&job), Err(DiskError::UnexpectedSection { kind: found }) if found == kind.code()),
            "{kind:?}"
        );
    }
    Ok(())
}

#[test]
fn check_role_baseline() -> TestResult {
    let job = baseline_job()?;
    let good = sections_of(&[(SectionKind::Job, job_bytes()), (SectionKind::Base, b"t")])?;
    good.check_role(&job)?;
    let with_diff = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::Base, b"t"),
        (SectionKind::Diff, diff()?.as_bytes()),
    ])?;
    assert!(matches!(
        with_diff.check_role(&job),
        Err(DiskError::UnexpectedSection { kind: 3 })
    ));
    Ok(())
}

#[test]
fn check_role_judge() -> TestResult {
    let job = judge_job()?;
    let good = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::JudgeData, b"judge data"),
        (SectionKind::CandidateOut, b"candidate output"),
    ])?;
    good.check_role(&job)?;
    let no_data = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::CandidateOut, b"candidate output"),
    ])?;
    assert!(matches!(
        no_data.check_role(&job),
        Err(DiskError::MissingSection { kind: 4 })
    ));
    let no_output = sections_of(&[
        (SectionKind::Job, job_bytes()),
        (SectionKind::JudgeData, b"judge data"),
    ])?;
    assert!(matches!(
        no_output.check_role(&job),
        Err(DiskError::MissingSection { kind: 5 })
    ));
    for (kind, content) in [(SectionKind::Base, &b"t"[..]), (SectionKind::Diff, b"d")] {
        let extra = sections_of(&[
            (SectionKind::Job, job_bytes()),
            (kind, content),
            (SectionKind::JudgeData, b"judge data"),
            (SectionKind::CandidateOut, b"candidate output"),
        ])?;
        assert!(
            matches!(extra.check_role(&job), Err(DiskError::UnexpectedSection { kind: found }) if found == kind.code()),
            "{kind:?}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// PID 1's in-memory input and the guest reserve (05-CONTEXT D-05)
// ---------------------------------------------------------------------------------------------------------------

/// The sum of the caps of `kinds`.
fn caps(kinds: &[SectionKind]) -> u64 {
    kinds.iter().map(|kind| kind.cap()).sum()
}

#[test]
fn guest_input_fits_reserve() {
    assert_eq!(MAX_GUEST_INPUT_BYTES, 134_217_728);
    assert_eq!(
        MAX_GUEST_INPUT_BYTES,
        u64::from(GUEST_RESERVE_MIB) * 1024 * 1024 / 2
    );
    assert_eq!(MAX_JUDGE_DATA_BYTES, 100_663_296);
    let candidate = caps(&[SectionKind::Job, SectionKind::Base, SectionKind::Diff]);
    assert!(candidate <= MAX_GUEST_INPUT_BYTES, "{candidate}");
    let judge = caps(&[
        SectionKind::Job,
        SectionKind::JudgeData,
        SectionKind::CandidateOut,
    ]);
    assert!(judge <= MAX_GUEST_INPUT_BYTES, "{judge}");
}

#[test]
fn judge_data_cap_is_enforced_from_the_table() -> TestResult {
    // Only the header and the table: a reader that read a section byte before checking the caps would fail with
    // an I/O error instead.
    let over = MAX_JUDGE_DATA_BYTES + 1;
    let table = header_and_table(2, &[(1, 512, 7), (4, 1024, over)]);
    let error = refused(&table)?;
    assert!(
        matches!(
            error,
            DiskError::TooLarge { kind: 4, length, cap } if length == over && cap == MAX_JUDGE_DATA_BYTES
        ),
        "{error}"
    );
    // At the cap, the table is accepted and the reader goes on to the sections, which are not there.
    let at_cap = header_and_table(2, &[(1, 512, 7), (4, 1024, MAX_JUDGE_DATA_BYTES)]);
    assert!(matches!(refused(&at_cap)?, DiskError::Io(_)));
    Ok(())
}
