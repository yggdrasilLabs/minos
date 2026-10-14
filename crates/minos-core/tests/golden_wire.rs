//! Golden wire formats: the loop-to-runner job contract (CORE-13), every ledger `Record::V1` variant, every
//! `Outcome` variant on the wire (CORE-07), `Line`, and `Scorecard` (CORE-09), plus a byte pin over all of them.
//!
//! Fixtures are built from fixed bytes through public constructors only, so the same code always produces the same
//! values. The job tests below check validation and round trips with plain `assert!`s.

use std::collections::BTreeMap;
use std::error::Error;

use minos_core::diff::Diff;
use minos_core::experiment::Experiment;
use minos_core::id::{BlobId, ExperimentId, ImageDigest, TreeId};
use minos_core::job::{JOB_VERSION, JobBudget, JobError, JobRequest, JobResult};
use minos_core::ledger::{BackendInfo, BackendKind, ConfinementLayer, Isolation};
use minos_core::limits::{MAX_STAGE_MS, MAX_TRIALS, MIN_STAGE_MS};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::outcome::{Outcome, Stage};
use minos_core::usage::Usage;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The `minos.toml` example in CLAUDE.md, section "Projects": build 2m, run 5m, and the default judge budget.
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

/// The base tree of every fixture: the `golden_ids.rs` tree `t`.
fn base() -> TreeId {
    TreeId::of(b"t")
}

/// The manifest's judge over the judge data tree `judge`.
fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::of(&TreeId::of(b"judge"), manifest()?.judge()))
}

fn diff() -> Result<Diff, Box<dyn Error>> {
    let manifest = manifest()?;
    Ok(Diff::parse(DIFF, manifest.writable(), &minos_core::limits::Limits::standard())?)
}

fn candidate() -> Result<Experiment, Box<dyn Error>> {
    Ok(Experiment::candidate(base(), diff()?))
}

/// The content address of the candidate's canonical diff.
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
    Ok(JobRequest::candidate(
        &candidate()?,
        diff_blob()?,
        judge()?,
        1,
        budget()?,
    )?)
}

/// Hardware isolation under Seatbelt and rlimits, without the dedicated user: reduced local hardening.
fn backend() -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        Isolation::Hardware,
        [ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits]
            .into_iter()
            .collect(),
        ImageDigest::of(b"root"),
        Some(ImageDigest::of(b"kernel")),
    )
}

fn logs() -> BTreeMap<Stage, BlobId> {
    [
        (Stage::Build, BlobId::of(b"build log")),
        (Stage::Run, BlobId::of(b"run log")),
        (Stage::Judge, BlobId::of(b"judge log")),
    ]
    .into_iter()
    .collect()
}

fn job_result() -> Result<JobResult, Box<dyn Error>> {
    Ok(JobResult::new(
        &candidate_request()?,
        Outcome::TimedOut { stage: Stage::Run },
        Usage::compute(4_000, 2_048_000),
        backend(),
        logs(),
    ))
}

/// `text` with the first `from` replaced by `to`; an error if `from` is missing, so no test is vacuous.
fn edit(text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
    if text.contains(from) {
        Ok(text.replacen(from, to, 1))
    } else {
        Err(format!("the text does not contain {from:?}").into())
    }
}

/// The error `JobRequest::parse_json` returns for `text`; an error if it parsed. Also checks that the plain serde
/// path refuses the same text.
fn request_rejected(text: &str) -> Result<JobError, Box<dyn Error>> {
    if serde_json::from_str::<JobRequest>(text).is_ok() {
        return Err(format!("serde accepted: {text}").into());
    }
    match JobRequest::parse_json(text.as_bytes()) {
        Ok(_) => Err(format!("request accepted: {text}").into()),
        Err(error) => Ok(error),
    }
}

/// The error `JobResult::parse_json` returns for `text`; an error if it parsed.
fn result_rejected(text: &str) -> Result<JobError, Box<dyn Error>> {
    if serde_json::from_str::<JobResult>(text).is_ok() {
        return Err(format!("serde accepted: {text}").into());
    }
    match JobResult::parse_json(text.as_bytes()) {
        Ok(_) => Err(format!("result accepted: {text}").into()),
        Err(error) => Ok(error),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Job contract
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn job_version_is_one() {
    assert_eq!(JOB_VERSION, 1);
}

#[test]
fn budget_comes_from_the_manifest() -> TestResult {
    let budget = budget()?;
    assert_eq!(budget.build_ms(), 120_000);
    assert_eq!(budget.run_ms(), 300_000);
    assert_eq!(budget.judge_ms(), 120_000);
    Ok(())
}

#[test]
fn budget_new_checks_each_stage() -> TestResult {
    let budget = JobBudget::new(MIN_STAGE_MS, MAX_STAGE_MS, MIN_STAGE_MS)?;
    assert_eq!(budget.build_ms(), MIN_STAGE_MS);
    assert_eq!(budget.run_ms(), MAX_STAGE_MS);
    for (build, run, judge, field) in [
        (MIN_STAGE_MS - 1, MIN_STAGE_MS, MIN_STAGE_MS, "build_ms"),
        (MIN_STAGE_MS, MAX_STAGE_MS + 1, MIN_STAGE_MS, "run_ms"),
        (MIN_STAGE_MS, MIN_STAGE_MS, 0, "judge_ms"),
    ] {
        match JobBudget::new(build, run, judge) {
            Err(JobError::Budget { field: got }) => assert_eq!(got, field),
            other => return Err(format!("{field}: {other:?}").into()),
        }
    }
    Ok(())
}

#[test]
fn baseline_request_has_no_diff() -> TestResult {
    let request = baseline_request()?;
    assert_eq!(request.exp(), ExperimentId::baseline(&base()));
    assert_eq!(request.base(), base());
    assert_eq!(request.diff(), None);
    assert!(request.is_baseline());
    assert_eq!(request.judge(), judge()?);
    assert_eq!(request.trial(), 0);
    assert_eq!(request.budget(), budget()?);
    Ok(())
}

#[test]
fn candidate_request_copies_the_experiment() -> TestResult {
    let experiment = candidate()?;
    let request = candidate_request()?;
    assert_eq!(request.exp(), experiment.id());
    assert_eq!(request.base(), experiment.base());
    assert_eq!(request.diff(), Some(diff_blob()?));
    assert!(!request.is_baseline());
    assert_eq!(request.trial(), 1);
    Ok(())
}

#[test]
fn a_baseline_experiment_is_not_a_candidate() -> TestResult {
    let error = JobRequest::candidate(
        &Experiment::baseline(base()),
        BlobId::of(b"diff"),
        judge()?,
        0,
        budget()?,
    );
    assert!(matches!(error, Err(JobError::BaselineAsCandidate)));
    Ok(())
}

#[test]
fn a_candidate_blob_must_address_its_diff() -> TestResult {
    let error = JobRequest::candidate(
        &candidate()?,
        BlobId::of(b"another diff"),
        judge()?,
        0,
        budget()?,
    );
    assert!(matches!(error, Err(JobError::DiffMismatch)));
    Ok(())
}

#[test]
fn constructors_check_the_trial() -> TestResult {
    let last = MAX_TRIALS - 1;
    assert_eq!(
        JobRequest::baseline(base(), judge()?, last, budget()?)?.trial(),
        last
    );
    assert!(matches!(
        JobRequest::baseline(base(), judge()?, MAX_TRIALS, budget()?),
        Err(JobError::Trial)
    ));
    assert!(matches!(
        JobRequest::candidate(&candidate()?, diff_blob()?, judge()?, MAX_TRIALS, budget()?),
        Err(JobError::Trial)
    ));
    Ok(())
}

#[test]
fn verify_diff_recomputes_the_experiment() -> TestResult {
    let request = candidate_request()?;
    assert!(request.verify_diff(&diff()?));
    let manifest = manifest()?;
    let other = Diff::parse(
        &edit(DIFF, "new();", "newer();")?,
        manifest.writable(),
        &minos_core::limits::Limits::standard(),
    )?;
    assert!(!request.verify_diff(&other));
    assert!(!baseline_request()?.verify_diff(&diff()?));
    Ok(())
}

#[test]
fn requests_round_trip() -> TestResult {
    for request in [baseline_request()?, candidate_request()?] {
        let text = serde_json::to_string(&request)?;
        assert_eq!(serde_json::from_str::<JobRequest>(&text)?, request);
        assert_eq!(JobRequest::parse_json(text.as_bytes())?, request);
    }
    Ok(())
}

#[test]
fn a_candidate_without_a_diff_is_refused() -> TestResult {
    let text = serde_json::to_string(&candidate_request()?)?;
    let blob = format!("\"diff\":\"{}\"", diff_blob()?);
    let error = request_rejected(&edit(&text, &blob, "\"diff\":null")?)?;
    assert!(matches!(error, JobError::DiffMismatch));
    Ok(())
}

#[test]
fn a_baseline_with_a_diff_is_refused() -> TestResult {
    let text = serde_json::to_string(&baseline_request()?)?;
    let blob = format!("\"diff\":\"{}\"", diff_blob()?);
    let error = request_rejected(&edit(&text, "\"diff\":null", &blob)?)?;
    assert!(matches!(error, JobError::DiffMismatch));
    Ok(())
}

#[test]
fn a_request_trial_over_the_cap_is_refused() -> TestResult {
    let text = serde_json::to_string(&baseline_request()?)?;
    let over = format!("\"trial\":{MAX_TRIALS}");
    let error = request_rejected(&edit(&text, "\"trial\":0", &over)?)?;
    assert!(matches!(error, JobError::Trial));
    Ok(())
}

#[test]
fn a_request_budget_out_of_range_is_refused() -> TestResult {
    let text = serde_json::to_string(&baseline_request()?)?;
    for (from, to, field) in [
        ("\"build_ms\":120000", "\"build_ms\":999", "build_ms"),
        ("\"run_ms\":300000", "\"run_ms\":3600001", "run_ms"),
        ("\"judge_ms\":120000", "\"judge_ms\":0", "judge_ms"),
    ] {
        match request_rejected(&edit(&text, from, to)?)? {
            JobError::Budget { field: got } => assert_eq!(got, field),
            other => return Err(format!("{field}: {other:?}").into()),
        }
    }
    Ok(())
}

#[test]
fn a_request_of_another_version_is_refused() -> TestResult {
    let text = serde_json::to_string(&baseline_request()?)?;
    let error = request_rejected(&edit(&text, "\"v\":1", "\"v\":2")?)?;
    assert!(matches!(error, JobError::Version));
    Ok(())
}

#[test]
fn a_request_with_an_unknown_field_is_refused() -> TestResult {
    let text = serde_json::to_string(&baseline_request()?)?;
    let error = request_rejected(&edit(&text, "\"v\":1,", "\"v\":1,\"network\":true,")?)?;
    assert!(matches!(error, JobError::Json(_)));
    let error = request_rejected(&edit(&text, "\"build_ms\"", "\"gpu\":1,\"build_ms\"")?)?;
    assert!(matches!(error, JobError::Json(_)));
    Ok(())
}

#[test]
fn a_result_copies_the_request() -> TestResult {
    let request = candidate_request()?;
    let result = job_result()?;
    assert_eq!(result.exp(), request.exp());
    assert_eq!(result.judge(), request.judge());
    assert_eq!(result.trial(), request.trial());
    assert_eq!(result.outcome(), &Outcome::TimedOut { stage: Stage::Run });
    assert_eq!(result.usage(), Usage::compute(4_000, 2_048_000));
    assert_eq!(result.backend(), &backend());
    assert_eq!(result.logs(), &logs());
    Ok(())
}

#[test]
fn results_round_trip() -> TestResult {
    let result = job_result()?;
    let text = serde_json::to_string(&result)?;
    assert_eq!(serde_json::from_str::<JobResult>(&text)?, result);
    assert_eq!(JobResult::parse_json(text.as_bytes())?, result);
    Ok(())
}

#[test]
fn a_bad_result_is_refused() -> TestResult {
    let text = serde_json::to_string(&job_result()?)?;
    let error = result_rejected(&edit(&text, "\"v\":1", "\"v\":2")?)?;
    assert!(matches!(error, JobError::Version));
    let over = format!("\"trial\":{MAX_TRIALS}");
    let error = result_rejected(&edit(&text, "\"trial\":1", &over)?)?;
    assert!(matches!(error, JobError::Trial));
    let error = result_rejected(&edit(&text, "\"v\":1,", "\"v\":1,\"cost_usd\":3,")?)?;
    assert!(matches!(error, JobError::Json(_)));
    let error = result_rejected(&edit(&text, "\"build\":", "\"setup\":")?)?;
    assert!(matches!(error, JobError::Json(_)));
    Ok(())
}
