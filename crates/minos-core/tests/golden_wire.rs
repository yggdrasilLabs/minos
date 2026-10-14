//! Golden wire formats: the loop-to-runner job contract (CORE-13), every ledger `Record::V1` variant, every
//! `Outcome` variant on the wire (CORE-07), `Line`, and `Scorecard` (CORE-09), plus a byte pin over all of them.
//!
//! Fixtures are built from fixed bytes through public constructors only, so the same code always produces the same
//! values.
//!
//! Each golden line is an insta snapshot under `tests/snapshots/`. `.config/insta.yaml` sets `update: no`, so the
//! plain gate never writes or accepts one: a snapshot is created deliberately with
//! `INSTA_UPDATE=always cargo test -p minos-core --test golden_wire` and reviewed before it is committed. insta
//! trims trailing whitespace and normalizes line endings when it compares, so `all_golden_bytes_pinned` also pins
//! the exact bytes of every golden line with one SHA-256 literal (03-RESEARCH Pitfall 4).

use std::collections::BTreeMap;
use std::error::Error;
use std::str::FromStr;

use minos_core::diff::Diff;
use minos_core::experiment::Experiment;
use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, MetricName, ModelId, PromptHash, ReportHash,
    RunId, TreeId,
};
use minos_core::job::{JOB_VERSION, JobBudget, JobError, JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, ConfinementLayer, Decision, DecisionKind, Event, Isolation, Line,
    Origin, Record, ReviewAnswer, ReviewId, ReviewKind, SpendSource, StopReason,
};
use minos_core::limits::{Limits, MAX_STAGE_MS, MAX_TRIALS, MIN_STAGE_MS};
use minos_core::manifest::{Arch, JudgeVersion, Manifest, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::path::RelPath;
use minos_core::usage::Usage;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

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
    Ok(Diff::parse(DIFF, manifest.writable(), &Limits::standard())?)
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
        &Limits::standard(),
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

// ---------------------------------------------------------------------------------------------------------------
// Golden records
// ---------------------------------------------------------------------------------------------------------------

/// The start time of the `golden_ids.rs` `RunId`.
const STARTED_AT: u64 = 1_760_000_000;

/// The `RunId` of `golden_ids.rs`: manifest `m`, base tree `t`, started at [`STARTED_AT`].
fn run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &base(), STARTED_AT)
}

fn baseline() -> ExperimentId {
    ExperimentId::baseline(&base())
}

fn haiku() -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str("haiku")?)
}

fn metric(name: &str) -> Result<MetricName, Box<dyn Error>> {
    Ok(MetricName::from_str(name)?)
}

/// The manifest's gate and objective, plus one held-out metric, with exactly representable values.
fn scorecard() -> Result<Scorecard, Box<dyn Error>> {
    Ok(Scorecard::from_entries([
        (metric("identical_output")?, Metric::new(1.0)?, false),
        (metric("throughput_mb_s")?, Metric::new(12.25)?, false),
        (metric("held_out_mb_s")?, Metric::new(0.5)?, true),
    ])?)
}

fn review_request() -> ReviewId {
    ReviewId::of(&run(), ReviewKind::Params, &BlobId::of(b"params"))
}

fn run_started() -> Event {
    Event::RunStarted {
        run: run(),
        manifest: ManifestId::of(b"m"),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
    }
}

fn judged(trial: u32, outcome: Outcome) -> Result<Event, Box<dyn Error>> {
    Ok(Event::Judged {
        exp: candidate()?.id(),
        judge: judge()?,
        trial,
        outcome,
        backend: backend(),
    })
}

fn spent(source: SpendSource, usage: Usage) -> Event {
    Event::Spent {
        generation: 1,
        source,
        usage,
    }
}

/// One record per event shape, each with the name of its snapshot, in the snapshot order of [`goldens`].
fn golden_records() -> Result<Vec<(&'static str, Record)>, Box<dyn Error>> {
    let exp = candidate()?.id();
    let proposed = |tier: Tier, origin: Origin| -> Result<Event, Box<dyn Error>> {
        Ok(Event::Proposed {
            exp,
            parent: baseline(),
            generation: 1,
            tier,
            diff: diff_blob()?,
            rationale: BlobId::of(b"rationale"),
            origin,
        })
    };
    let selected = |generation: u32, kind: DecisionKind, dropped: u32| Event::Selected {
        generation,
        decision: Decision::new(kind, 4, dropped),
    };
    let events = vec![
        ("record_v1_run_started", run_started()),
        (
            "record_v1_manifest_changed",
            Event::ManifestChanged {
                manifest: ManifestId::of(b"m2"),
                from_generation: 2,
            },
        ),
        (
            "record_v1_proposed_sweep",
            proposed(Tier::Sweep, Origin::Sweep)?,
        ),
        (
            "record_v1_proposed_model",
            proposed(
                Tier::SingleShot,
                Origin::Model {
                    model: haiku()?,
                    prompt: PromptHash::of(b"prompt"),
                },
            )?,
        ),
        (
            "record_v1_judged_scored",
            judged(0, Outcome::Scored(scorecard()?))?,
        ),
        (
            "record_v1_judged_rejected",
            judged(
                1,
                Outcome::Rejected(Reason::ApplyFailed {
                    path: RelPath::try_from("src/parse.rs")?,
                    hunk: 1,
                }),
            )?,
        ),
        (
            "record_v1_judged_crashed",
            judged(
                2,
                Outcome::Crashed {
                    stage: Stage::Build,
                },
            )?,
        ),
        (
            "record_v1_judged_timed_out",
            judged(3, Outcome::TimedOut { stage: Stage::Run })?,
        ),
        (
            "record_v1_selected_improved",
            selected(1, DecisionKind::Improved { winner: exp }, 1),
        ),
        (
            "record_v1_selected_kept",
            selected(
                2,
                DecisionKind::Kept {
                    incumbent: baseline(),
                },
                0,
            ),
        ),
        (
            "record_v1_selected_no_valid",
            selected(3, DecisionKind::NoValidCandidates, 4),
        ),
        (
            "record_v1_spent_model_call",
            spent(
                SpendSource::ModelCall {
                    call: 0,
                    model: haiku()?,
                },
                Usage::tokens(1_200, 340, 56, 78),
            ),
        ),
        (
            "record_v1_spent_candidate",
            spent(
                SpendSource::Candidate { exp, trial: 0 },
                Usage::compute(4_000, 2_048_000),
            ),
        ),
        (
            "record_v1_spent_judge",
            spent(
                SpendSource::Judge { exp, trial: 0 },
                Usage::compute(1_500, 768_000),
            ),
        ),
        (
            "record_v1_steered",
            Event::Steered {
                generation: 2,
                ordinal: 0,
                note: BlobId::of(b"note"),
            },
        ),
        (
            "record_v1_stop_requested",
            Event::StopRequested {
                generation: 3,
                reason: StopReason::SpendCap,
            },
        ),
        (
            "record_v1_review_requested",
            Event::ReviewRequested {
                request: review_request(),
                kind: ReviewKind::Params,
            },
        ),
        (
            "record_v1_reviewed",
            Event::Reviewed {
                request: review_request(),
                answer: ReviewAnswer::Approved,
            },
        ),
        (
            "record_v1_report_acknowledged",
            Event::ReportAcknowledged {
                exp,
                report: ReportHash::of(b"report"),
            },
        ),
    ];
    Ok(events
        .into_iter()
        .map(|(name, event)| (name, Record::v1(event)))
        .collect())
}

fn golden_line() -> Line {
    Line::new(0, &run(), Record::v1(run_started()))
}

/// Every golden line with its snapshot name, in the order the byte pin hashes them.
fn goldens() -> Result<Vec<(&'static str, String)>, Box<dyn Error>> {
    let mut lines = Vec::new();
    for (name, record) in golden_records()? {
        lines.push((name, serde_json::to_string(&record)?));
    }
    lines.push(("line_v1", serde_json::to_string(&golden_line())?));
    lines.push(("scorecard_v1", serde_json::to_string(&scorecard()?)?));
    lines.push((
        "job_request_baseline",
        serde_json::to_string(&baseline_request()?)?,
    ));
    lines.push((
        "job_request_candidate",
        serde_json::to_string(&candidate_request()?)?,
    ));
    lines.push(("job_result", serde_json::to_string(&job_result()?)?));
    Ok(lines)
}

/// The golden line named `name`.
fn golden(name: &str) -> Result<String, Box<dyn Error>> {
    goldens()?
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, line)| line)
        .ok_or_else(|| format!("no golden line {name}").into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

fn record_rejected(text: &str) -> TestResult {
    match serde_json::from_str::<Record>(text) {
        Ok(_) => Err(format!("record accepted: {text}").into()),
        Err(_) => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Golden tests
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn golden_snapshots() -> TestResult {
    let lines = goldens()?;
    assert_eq!(lines.len(), 24);
    for (name, line) in lines {
        insta::assert_snapshot!(name, line);
    }
    Ok(())
}

#[test]
fn all_golden_bytes_pinned() -> TestResult {
    // The pin was computed outside the code under test, from the accepted snapshot bodies (the text after each
    // file's second `---` line), in the order of `goldens`, each followed by one newline:
    //
    //   cd crates/minos-core/tests/snapshots
    //   for n in record_v1_run_started record_v1_manifest_changed record_v1_proposed_sweep \
    //            record_v1_proposed_model record_v1_judged_scored record_v1_judged_rejected \
    //            record_v1_judged_crashed record_v1_judged_timed_out record_v1_selected_improved \
    //            record_v1_selected_kept record_v1_selected_no_valid record_v1_spent_model_call \
    //            record_v1_spent_candidate record_v1_spent_judge record_v1_steered record_v1_stop_requested \
    //            record_v1_review_requested record_v1_reviewed record_v1_report_acknowledged line_v1 \
    //            scorecard_v1 job_request_baseline job_request_candidate job_result; do
    //     awk 'n >= 2 { print } /^---$/ { n++ }' "golden_wire__$n.snap"
    //   done | shasum -a 256
    let mut all = String::new();
    for (_, line) in goldens()? {
        all.push_str(&line);
        all.push('\n');
    }
    assert_eq!(
        sha256_hex(all.as_bytes()),
        "8020cfdeb958e6e74187ffafc315a7aea756daef4011aceeb61a647d9f2b1039"
    );
    Ok(())
}

#[test]
fn every_golden_line_is_one_json_line() -> TestResult {
    for (name, line) in goldens()? {
        assert!(!line.contains('\n'), "{name} spans lines");
        assert!(!line.contains('\r'), "{name} has a carriage return");
        assert!(line.starts_with('{') && line.ends_with('}'), "{name}");
    }
    Ok(())
}

#[test]
fn every_event_variant_and_outcome_has_a_golden_line() -> TestResult {
    let records = golden_records()?;
    let has = |wanted: fn(&Event) -> bool| records.iter().any(|(_, record)| wanted(record.event()));
    assert!(has(|event| matches!(event, Event::RunStarted { .. })));
    assert!(has(|event| matches!(event, Event::ManifestChanged { .. })));
    assert!(has(|event| matches!(event, Event::Proposed { .. })));
    assert!(has(|event| matches!(event, Event::Selected { .. })));
    assert!(has(|event| matches!(event, Event::Spent { .. })));
    assert!(has(|event| matches!(event, Event::Steered { .. })));
    assert!(has(|event| matches!(event, Event::StopRequested { .. })));
    assert!(has(|event| matches!(event, Event::ReviewRequested { .. })));
    assert!(has(|event| matches!(event, Event::Reviewed { .. })));
    assert!(has(|event| matches!(
        event,
        Event::ReportAcknowledged { .. }
    )));
    let judged = |wanted: fn(&Outcome) -> bool| {
        records.iter().any(|(_, record)| {
            matches!(record.event(), Event::Judged { outcome, .. } if wanted(outcome))
        })
    };
    assert!(judged(|outcome| matches!(outcome, Outcome::Scored(_))));
    assert!(judged(|outcome| matches!(outcome, Outcome::Rejected(_))));
    assert!(judged(|outcome| matches!(outcome, Outcome::Crashed { .. })));
    assert!(judged(|outcome| matches!(
        outcome,
        Outcome::TimedOut { .. }
    )));
    Ok(())
}

#[test]
fn golden_lines_parse_back_to_equal_values() -> TestResult {
    for (name, record) in golden_records()? {
        let text = golden(name)?;
        assert_eq!(serde_json::from_str::<Record>(&text)?, record, "{name}");
    }
    let line = serde_json::from_str::<Line>(&golden("line_v1")?)?;
    assert_eq!(line, golden_line());
    assert!(line.verify(&run()).is_ok());
    assert_eq!(
        serde_json::from_str::<Scorecard>(&golden("scorecard_v1")?)?,
        scorecard()?
    );
    assert_eq!(
        JobRequest::parse_json(golden("job_request_baseline")?.as_bytes())?,
        baseline_request()?
    );
    assert_eq!(
        JobRequest::parse_json(golden("job_request_candidate")?.as_bytes())?,
        candidate_request()?
    );
    assert_eq!(
        JobResult::parse_json(golden("job_result")?.as_bytes())?,
        job_result()?
    );
    Ok(())
}

#[test]
fn records_carry_the_string_version_tag() -> TestResult {
    for (name, _) in golden_records()? {
        assert!(golden(name)?.starts_with("{\"v\":\"1\","), "{name}");
    }
    Ok(())
}

#[test]
fn a_record_of_another_version_is_refused() -> TestResult {
    for (name, _) in golden_records()? {
        record_rejected(&edit(&golden(name)?, "\"v\":\"1\"", "\"v\":\"2\"")?)?;
    }
    Ok(())
}

#[test]
fn an_unknown_event_variant_is_refused() -> TestResult {
    let text = golden("record_v1_run_started")?;
    record_rejected(&edit(&text, "\"run_started\"", "\"run_restarted\"")?)?;
    let text = golden("record_v1_judged_scored")?;
    record_rejected(&edit(&text, "\"scored\"", "\"perfect\"")?)?;
    Ok(())
}

#[test]
fn an_unknown_field_inside_a_variant_is_refused() -> TestResult {
    for (name, _) in golden_records()? {
        // The first `":{` follows the variant name, so this adds a field to the event's own struct.
        record_rejected(&edit(&golden(name)?, "\":{", "\":{\"extra\":1,")?)?;
    }
    let text = golden("record_v1_spent_model_call")?;
    record_rejected(&edit(
        &text,
        "\"input_tokens\"",
        "\"cost_usd\":1,\"input_tokens\"",
    )?)?;
    let text = golden("record_v1_judged_scored")?;
    record_rejected(&edit(
        &text,
        "\"isolation\"",
        "\"network\":true,\"isolation\"",
    )?)?;
    Ok(())
}

#[test]
fn a_missing_field_is_refused() -> TestResult {
    let text = golden("record_v1_run_started")?;
    record_rejected(&edit(&text, ",\"started_at\":1760000000", "")?)?;
    let text = golden("record_v1_judged_crashed")?;
    record_rejected(&edit(&text, "\"trial\":2,", "")?)?;
    let text = golden("record_v1_spent_candidate")?;
    record_rejected(&edit(&text, "\"vcpu_ms\":4000,", "")?)?;
    let text = golden("line_v1")?;
    if serde_json::from_str::<Line>(&edit(&text, "\"seq\":0,", "")?).is_ok() {
        return Err("a line without seq was accepted".into());
    }
    Ok(())
}

#[test]
fn spent_carries_no_dollar_field() -> TestResult {
    for name in [
        "record_v1_spent_model_call",
        "record_v1_spent_candidate",
        "record_v1_spent_judge",
    ] {
        let text = golden(name)?.to_ascii_lowercase();
        for word in ["usd", "cost", "dollar", "price"] {
            assert!(!text.contains(word), "{name} contains {word}");
        }
    }
    Ok(())
}

fn fail<E: std::fmt::Display>(error: E) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

/// Any finite `f64`, drawn from all bit patterns.
fn finite() -> impl Strategy<Value = f64> {
    any::<u64>()
        .prop_map(f64::from_bits)
        .prop_filter("finite", |value| value.is_finite())
}

proptest! {
    #[test]
    fn metric_bits_survive_record_round_trip(values in prop::collection::vec(finite(), 1..8)) {
        let mut entries = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let name = MetricName::from_str(&format!("m{index}")).map_err(fail)?;
            entries.push((name, Metric::new(*value).map_err(fail)?, index % 2 == 0));
        }
        let card = Scorecard::from_entries(entries).map_err(fail)?;
        let event = judged(0, Outcome::Scored(card.clone())).map_err(fail)?;
        let text = serde_json::to_string(&Record::v1(event)).map_err(fail)?;
        let back = serde_json::from_str::<Record>(&text).map_err(fail)?;
        let Event::Judged { outcome: Outcome::Scored(read), .. } = back.event() else {
            return Err(fail("the record did not read back as a scored Judged event"));
        };
        prop_assert_eq!(read.len(), card.len());
        for ((name, written), (read_name, read_metric)) in card.iter().zip(read.iter()) {
            prop_assert_eq!(name, read_name);
            prop_assert_eq!(written.value().value().to_bits(), read_metric.value().value().to_bits());
        }
    }
}
