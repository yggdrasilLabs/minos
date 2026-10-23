//! The runner's blame table (LOOP-01, JUDGE-03): one test per row of 07-RESEARCH Pattern 2, the gate short-circuit
//! of `judged_outcome`, and a property that no generated observation panics either verdict function.
//!
//! The rows follow the refined K7 rule (02-SPIKE-EVIDENCE.md K7 table; `minos_core::outcome` module docs), 05-RESEARCH
//! Pitfall 12 (a host timeout is the backstop, attributed to `Run` for a candidate and `Judge` for a judge), review
//! CR-02 (setup is read before role, and an input failure is infrastructure whatever role it names), guest.rs WR-02
//! (a full scratch is configuration), and 07-RESEARCH Assumptions A2 (a failed judge stage is `Crashed { Judge }`)
//! and A3 (a signaled or unspawnable stage is `BuildFailed` or `RunFailed`). Statuses are built through
//! `GuestStatus::new` and scorecards through `Scorecard::parse_json`, so every input passed the same checks as real
//! guest output.

use std::error::Error;
use std::str::FromStr;

use minos_core::guest::{
    ApplyStatus, GuestRole, GuestStatus, GuestStatusError, OutputStatus, SetupStatus, SetupStep,
    StageEnd, StageReport,
};
use minos_core::id::MetricName;
use minos_core::limits::Limits;
use minos_core::metric::Scorecard;
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::path::RelPath;
use minos_core::{JudgeSpec, Manifest};
use minos_runner::map::{Seen, Verdict, candidate_verdict, judge_verdict, judged_outcome};
use minos_runner::{ConfigCause, InfraCause};
use minos_sandbox::{
    CrashCause, Ended, OutputCap, OutputFault, OutputInvalid, ScorecardFault, StatusFault,
};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const GATE: &str = "identical_output";
const OBJECTIVE: &str = "throughput_mb_s";

const PACKED: OutputStatus = OutputStatus::Packed {
    files: 3,
    content_bytes: 4096,
};

const OK: StageEnd = StageEnd::Exited { code: 0 };

fn status(
    role: GuestRole,
    setup: SetupStatus,
    apply: ApplyStatus,
    stages: &[(Stage, StageEnd)],
    output: OutputStatus,
) -> Result<GuestStatus, GuestStatusError> {
    GuestStatus::new(
        role,
        setup,
        apply,
        stages
            .iter()
            .map(|(stage, end)| StageReport::new(*stage, *end))
            .collect(),
        output,
    )
}

/// A candidate status whose setup succeeded and whose diff applied, with the given stage ends and output.
fn candidate(stages: &[(Stage, StageEnd)], output: OutputStatus) -> Result<Seen, GuestStatusError> {
    Ok(Seen::Guest(status(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Applied,
        stages,
        output,
    )?))
}

/// A judge status whose setup succeeded, with the given judge stage end and output.
fn judge(end: StageEnd, output: OutputStatus) -> Result<Seen, GuestStatusError> {
    Ok(Seen::Guest(status(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        &[(Stage::Judge, end)],
        output,
    )?))
}

/// A status whose setup failed at `step`, shaped as PID 1 sends it: no stage ran and nothing was packed.
fn setup_failed(role: GuestRole, step: SetupStep) -> Result<Seen, GuestStatusError> {
    let apply = match role {
        GuestRole::Candidate => ApplyStatus::Baseline,
        GuestRole::Judge => ApplyStatus::NotApplicable,
    };
    Ok(Seen::Guest(status(
        role,
        SetupStatus::Failed { step },
        apply,
        &[],
        OutputStatus::NotPacked,
    )?))
}

fn rejected(reason: Reason) -> Verdict {
    Verdict::Done(Outcome::Rejected(reason))
}

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::from_str(text)?)
}

fn name(text: &str) -> Result<MetricName, Box<dyn Error>> {
    Ok(MetricName::from_str(text)?)
}

fn judge_spec() -> Result<JudgeSpec, Box<dyn Error>> {
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
command = ["./judge"]

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
    Ok(Manifest::parse_toml(&text)?.judge().clone())
}

fn card(metrics: &[(&str, &str)]) -> Result<Scorecard, Box<dyn Error>> {
    let entries: Vec<String> = metrics
        .iter()
        .map(|(metric, value)| format!(r#"{{"name":"{metric}","value":{value},"held_out":false}}"#))
        .collect();
    let json = format!(r#"{{"v":1,"metrics":[{}]}}"#, entries.join(","));
    Ok(Scorecard::parse_json(json.as_bytes(), &Limits::standard())?)
}

// ---------------------------------------------------------------------------------------------------------------
// Host-observed ends (K7; 05-RESEARCH Pitfall 12)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn host_timeout_is_a_candidate_run_timeout() {
    assert_eq!(
        candidate_verdict(&Seen::NotExited(Ended::TimedOut)),
        Verdict::Done(Outcome::TimedOut { stage: Stage::Run })
    );
}

#[test]
fn host_timeout_is_a_judge_timeout() {
    assert_eq!(
        judge_verdict(&Seen::NotExited(Ended::TimedOut)),
        Verdict::Done(Outcome::TimedOut {
            stage: Stage::Judge
        })
    );
}

#[test]
fn missing_status_frame_is_a_candidate_run_crash() {
    assert_eq!(
        candidate_verdict(&Seen::NotExited(Ended::Crashed(CrashCause::NoStatusFrame))),
        Verdict::Done(Outcome::Crashed { stage: Stage::Run })
    );
}

#[test]
fn console_cap_is_a_candidate_run_crash() {
    assert_eq!(
        candidate_verdict(&Seen::NotExited(Ended::Crashed(CrashCause::ConsoleCap))),
        Verdict::Done(Outcome::Crashed { stage: Stage::Run })
    );
}

#[test]
fn host_crash_is_a_judge_crash() {
    for cause in [CrashCause::NoStatusFrame, CrashCause::ConsoleCap] {
        assert_eq!(
            judge_verdict(&Seen::NotExited(Ended::Crashed(cause))),
            Verdict::Done(Outcome::Crashed {
                stage: Stage::Judge
            })
        );
    }
}

#[test]
fn chunk_protocol_is_rejected_for_both_roles() {
    let seen = Seen::NotExited(Ended::ChunkProtocol);
    assert_eq!(candidate_verdict(&seen), rejected(Reason::ChunkProtocol));
    assert_eq!(judge_verdict(&seen), rejected(Reason::ChunkProtocol));
}

#[test]
fn an_exited_end_handed_in_as_not_exited_fails_closed_as_infrastructure() {
    let seen = Seen::NotExited(Ended::Exited);
    assert_eq!(
        candidate_verdict(&seen),
        Verdict::Infra(InfraCause::Unmapped)
    );
    assert_eq!(judge_verdict(&seen), Verdict::Infra(InfraCause::Unmapped));
}

// ---------------------------------------------------------------------------------------------------------------
// Output and status faults
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn output_over_a_cap_is_rejected_as_too_large() {
    let seen = Seen::Output(OutputFault::TooLarge {
        cap: OutputCap::Total,
    });
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputTooLarge));
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputTooLarge));
}

#[test]
fn output_breaking_a_rule_is_rejected_as_invalid() {
    let seen = Seen::Output(OutputFault::Invalid {
        why: OutputInvalid::EntryType,
    });
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputInvalid));
}

#[test]
fn missing_status_is_rejected_as_invalid_output() {
    let seen = Seen::Status(StatusFault::Missing);
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputInvalid));
}

#[test]
fn malformed_status_is_rejected_as_invalid_output() {
    let seen = Seen::Status(StatusFault::Invalid(GuestStatusError::Version));
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputInvalid));
}

#[test]
fn wrong_role_status_is_rejected_as_invalid_output() {
    let seen = Seen::Status(StatusFault::WrongRole);
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputInvalid));
}

#[test]
fn a_judge_status_handed_to_the_candidate_map_is_invalid_output() -> TestResult {
    let seen = judge(OK, PACKED)?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

#[test]
fn a_candidate_status_handed_to_the_judge_map_is_invalid_output() -> TestResult {
    let seen = candidate(&[(Stage::Build, OK), (Stage::Run, OK)], PACKED)?;
    assert_eq!(judge_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Setup, read before anything else (CR-02, WR-02)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn input_setup_failure_is_infrastructure_even_with_a_placeholder_role() -> TestResult {
    // PID 1 sends a candidate placeholder when it could not parse its job, whatever role it ran as.
    let seen = setup_failed(GuestRole::Candidate, SetupStep::Input)?;
    let expected = Verdict::Infra(InfraCause::GuestSetup {
        step: SetupStep::Input,
    });
    assert_eq!(candidate_verdict(&seen), expected);
    assert_eq!(judge_verdict(&seen), expected);
    Ok(())
}

#[test]
fn mount_setup_failure_is_infrastructure() -> TestResult {
    for role in [GuestRole::Candidate, GuestRole::Judge] {
        let seen = setup_failed(role, SetupStep::Mount)?;
        let expected = Verdict::Infra(InfraCause::GuestSetup {
            step: SetupStep::Mount,
        });
        assert_eq!(candidate_verdict(&seen), expected);
        assert_eq!(judge_verdict(&seen), expected);
    }
    Ok(())
}

#[test]
fn unpack_setup_failure_is_infrastructure() -> TestResult {
    for role in [GuestRole::Candidate, GuestRole::Judge] {
        let seen = setup_failed(role, SetupStep::Unpack)?;
        let expected = Verdict::Infra(InfraCause::GuestSetup {
            step: SetupStep::Unpack,
        });
        assert_eq!(candidate_verdict(&seen), expected);
        assert_eq!(judge_verdict(&seen), expected);
    }
    Ok(())
}

#[test]
fn apply_step_setup_failure_is_infrastructure() -> TestResult {
    for role in [GuestRole::Candidate, GuestRole::Judge] {
        let seen = setup_failed(role, SetupStep::Apply)?;
        let expected = Verdict::Infra(InfraCause::GuestSetup {
            step: SetupStep::Apply,
        });
        assert_eq!(candidate_verdict(&seen), expected);
        assert_eq!(judge_verdict(&seen), expected);
    }
    Ok(())
}

#[test]
fn output_setup_failure_is_infrastructure() -> TestResult {
    for role in [GuestRole::Candidate, GuestRole::Judge] {
        let seen = setup_failed(role, SetupStep::Output)?;
        let expected = Verdict::Infra(InfraCause::GuestSetup {
            step: SetupStep::Output,
        });
        assert_eq!(candidate_verdict(&seen), expected);
        assert_eq!(judge_verdict(&seen), expected);
    }
    Ok(())
}

#[test]
fn full_scratch_is_a_configuration_error_never_retried() -> TestResult {
    for role in [GuestRole::Candidate, GuestRole::Judge] {
        let seen = setup_failed(role, SetupStep::ScratchFull)?;
        let expected = Verdict::Config(ConfigCause::ScratchFull);
        assert_eq!(candidate_verdict(&seen), expected);
        assert_eq!(judge_verdict(&seen), expected);
    }
    Ok(())
}

#[test]
fn setup_is_read_before_apply_and_stages() -> TestResult {
    // A status that also claims a failed apply and failed stages: setup decides.
    let seen = Seen::Guest(status(
        GuestRole::Candidate,
        SetupStatus::Failed {
            step: SetupStep::Unpack,
        },
        ApplyStatus::Failed {
            path: path("src/parse.rs")?,
            hunk: 2,
        },
        &[(Stage::Build, StageEnd::Exited { code: 1 })],
        OutputStatus::TooLarge,
    )?);
    assert_eq!(
        candidate_verdict(&seen),
        Verdict::Infra(InfraCause::GuestSetup {
            step: SetupStep::Unpack
        })
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The candidate's apply, build, run, and output
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn failed_apply_is_rejected_with_its_path_and_hunk() -> TestResult {
    let seen = Seen::Guest(status(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Failed {
            path: path("src/parse.rs")?,
            hunk: 3,
        },
        &[],
        OutputStatus::NotPacked,
    )?);
    assert_eq!(
        candidate_verdict(&seen),
        rejected(Reason::ApplyFailed {
            path: path("src/parse.rs")?,
            hunk: 3,
        })
    );
    Ok(())
}

#[test]
fn build_timeout_is_a_build_timeout() -> TestResult {
    let seen = candidate(&[(Stage::Build, StageEnd::TimedOut)], PACKED)?;
    assert_eq!(
        candidate_verdict(&seen),
        Verdict::Done(Outcome::TimedOut {
            stage: Stage::Build
        })
    );
    Ok(())
}

#[test]
fn build_nonzero_exit_is_rejected_as_build_failed() -> TestResult {
    let seen = candidate(
        &[
            (Stage::Build, StageEnd::Exited { code: 101 }),
            (Stage::Run, StageEnd::Skipped),
        ],
        PACKED,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::BuildFailed));
    Ok(())
}

#[test]
fn build_killed_by_a_signal_is_rejected_as_build_failed() -> TestResult {
    let seen = candidate(&[(Stage::Build, StageEnd::Signaled { signal: 9 })], PACKED)?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::BuildFailed));
    Ok(())
}

#[test]
fn build_that_could_not_spawn_is_rejected_as_build_failed() -> TestResult {
    let seen = candidate(&[(Stage::Build, StageEnd::SpawnFailed)], PACKED)?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::BuildFailed));
    Ok(())
}

#[test]
fn run_timeout_is_a_run_timeout() -> TestResult {
    let seen = candidate(
        &[(Stage::Build, OK), (Stage::Run, StageEnd::TimedOut)],
        PACKED,
    )?;
    assert_eq!(
        candidate_verdict(&seen),
        Verdict::Done(Outcome::TimedOut { stage: Stage::Run })
    );
    Ok(())
}

#[test]
fn run_nonzero_exit_is_rejected_as_run_failed() -> TestResult {
    let seen = candidate(
        &[
            (Stage::Build, OK),
            (Stage::Run, StageEnd::Exited { code: 2 }),
        ],
        PACKED,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::RunFailed));
    Ok(())
}

#[test]
fn run_killed_by_a_signal_is_rejected_as_run_failed() -> TestResult {
    let seen = candidate(
        &[
            (Stage::Build, OK),
            (Stage::Run, StageEnd::Signaled { signal: 11 }),
        ],
        PACKED,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::RunFailed));
    Ok(())
}

#[test]
fn run_that_could_not_spawn_is_rejected_as_run_failed() -> TestResult {
    let seen = candidate(
        &[(Stage::Build, OK), (Stage::Run, StageEnd::SpawnFailed)],
        PACKED,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::RunFailed));
    Ok(())
}

#[test]
fn output_over_its_cap_after_passing_stages_is_rejected_as_too_large() -> TestResult {
    let seen = candidate(
        &[(Stage::Build, OK), (Stage::Run, OK)],
        OutputStatus::TooLarge,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputTooLarge));
    Ok(())
}

#[test]
fn invalid_output_after_passing_stages_is_rejected_as_invalid() -> TestResult {
    let seen = candidate(
        &[(Stage::Build, OK), (Stage::Run, OK)],
        OutputStatus::Invalid,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

#[test]
fn unpacked_output_after_passing_stages_is_rejected_as_invalid() -> TestResult {
    let seen = candidate(
        &[(Stage::Build, OK), (Stage::Run, OK)],
        OutputStatus::NotPacked,
    )?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

#[test]
fn a_missing_build_stage_is_rejected_as_invalid_output() -> TestResult {
    let seen = candidate(&[], PACKED)?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

#[test]
fn a_missing_run_stage_after_a_passing_build_is_rejected_as_invalid_output() -> TestResult {
    let seen = candidate(&[(Stage::Build, OK)], PACKED)?;
    assert_eq!(candidate_verdict(&seen), rejected(Reason::OutputInvalid));
    Ok(())
}

#[test]
fn a_skipped_stage_where_a_passing_candidate_needs_it_is_rejected_as_invalid_output() -> TestResult
{
    let skipped_build = candidate(
        &[(Stage::Build, StageEnd::Skipped), (Stage::Run, OK)],
        PACKED,
    )?;
    assert_eq!(
        candidate_verdict(&skipped_build),
        rejected(Reason::OutputInvalid)
    );
    let skipped_run = candidate(
        &[(Stage::Build, OK), (Stage::Run, StageEnd::Skipped)],
        PACKED,
    )?;
    assert_eq!(
        candidate_verdict(&skipped_run),
        rejected(Reason::OutputInvalid)
    );
    Ok(())
}

#[test]
fn a_candidate_whose_diff_applied_and_stages_passed_with_packed_output_passes() -> TestResult {
    let seen = candidate(&[(Stage::Build, OK), (Stage::Run, OK)], PACKED)?;
    assert_eq!(candidate_verdict(&seen), Verdict::Pass);
    Ok(())
}

#[test]
fn the_baseline_with_passing_stages_and_packed_output_passes() -> TestResult {
    let seen = Seen::Guest(status(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Baseline,
        &[(Stage::Build, OK), (Stage::Run, OK)],
        OutputStatus::Packed {
            files: 0,
            content_bytes: 0,
        },
    )?);
    assert_eq!(candidate_verdict(&seen), Verdict::Pass);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The judge stage (A2)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn judge_stage_timeout_is_a_judge_timeout() -> TestResult {
    assert_eq!(
        judge_verdict(&judge(StageEnd::TimedOut, PACKED)?),
        Verdict::Done(Outcome::TimedOut {
            stage: Stage::Judge
        })
    );
    Ok(())
}

#[test]
fn judge_stage_nonzero_exit_is_a_judge_crash() -> TestResult {
    assert_eq!(
        judge_verdict(&judge(StageEnd::Exited { code: 1 }, PACKED)?),
        Verdict::Done(Outcome::Crashed {
            stage: Stage::Judge
        })
    );
    Ok(())
}

#[test]
fn judge_stage_killed_by_a_signal_is_a_judge_crash() -> TestResult {
    assert_eq!(
        judge_verdict(&judge(StageEnd::Signaled { signal: 6 }, PACKED)?),
        Verdict::Done(Outcome::Crashed {
            stage: Stage::Judge
        })
    );
    Ok(())
}

#[test]
fn judge_stage_that_could_not_spawn_is_a_judge_crash() -> TestResult {
    assert_eq!(
        judge_verdict(&judge(StageEnd::SpawnFailed, PACKED)?),
        Verdict::Done(Outcome::Crashed {
            stage: Stage::Judge
        })
    );
    Ok(())
}

#[test]
fn a_missing_or_skipped_judge_stage_is_rejected_as_invalid_output() -> TestResult {
    let missing = Seen::Guest(status(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        &[],
        PACKED,
    )?);
    assert_eq!(judge_verdict(&missing), rejected(Reason::OutputInvalid));
    assert_eq!(
        judge_verdict(&judge(StageEnd::Skipped, PACKED)?),
        rejected(Reason::OutputInvalid)
    );
    Ok(())
}

#[test]
fn judge_output_that_was_not_packed_is_rejected() -> TestResult {
    assert_eq!(
        judge_verdict(&judge(OK, OutputStatus::TooLarge)?),
        rejected(Reason::OutputTooLarge)
    );
    assert_eq!(
        judge_verdict(&judge(OK, OutputStatus::Invalid)?),
        rejected(Reason::OutputInvalid)
    );
    assert_eq!(
        judge_verdict(&judge(OK, OutputStatus::NotPacked)?),
        rejected(Reason::OutputInvalid)
    );
    Ok(())
}

#[test]
fn a_judge_that_exited_zero_with_packed_output_passes_to_the_scorecard() -> TestResult {
    assert_eq!(judge_verdict(&judge(OK, PACKED)?), Verdict::Pass);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The scorecard (JUDGE-03)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_missing_scorecard_is_rejected_as_invalid() -> TestResult {
    assert_eq!(
        judged_outcome(Err(ScorecardFault::Missing), &judge_spec()?),
        Outcome::Rejected(Reason::ScorecardInvalid)
    );
    Ok(())
}

#[test]
fn an_unparseable_scorecard_is_rejected_as_invalid() -> TestResult {
    let error = Scorecard::parse_json(b"not a scorecard", &Limits::standard())
        .err()
        .ok_or("the bytes parsed as a scorecard")?;
    assert_eq!(
        judged_outcome(Err(ScorecardFault::Invalid(error)), &judge_spec()?),
        Outcome::Rejected(Reason::ScorecardInvalid)
    );
    Ok(())
}

#[test]
fn gate_failure_short_circuits_before_a_missing_objective_is_read() -> TestResult {
    // The objective is absent, but the failed gate is the reason: objectives are never read (JUDGE-03).
    let scorecard = card(&[(GATE, "0")])?;
    assert_eq!(
        judged_outcome(Ok(scorecard), &judge_spec()?),
        Outcome::Rejected(Reason::GateFailed { gate: name(GATE)? })
    );
    Ok(())
}

#[test]
fn a_gate_near_one_but_not_exactly_one_fails() -> TestResult {
    let scorecard = card(&[(GATE, "0.9999999"), (OBJECTIVE, "12.5")])?;
    assert_eq!(
        judged_outcome(Ok(scorecard), &judge_spec()?),
        Outcome::Rejected(Reason::GateFailed { gate: name(GATE)? })
    );
    Ok(())
}

#[test]
fn a_passing_gate_with_a_missing_objective_is_rejected_as_missing_metric() -> TestResult {
    let scorecard = card(&[(GATE, "1")])?;
    assert_eq!(
        judged_outcome(Ok(scorecard), &judge_spec()?),
        Outcome::Rejected(Reason::MissingMetric {
            metric: name(OBJECTIVE)?
        })
    );
    Ok(())
}

#[test]
fn an_undeclared_metric_is_rejected() -> TestResult {
    let scorecard = card(&[(GATE, "1"), (OBJECTIVE, "12.5"), ("extra", "3")])?;
    assert_eq!(
        judged_outcome(Ok(scorecard), &judge_spec()?),
        Outcome::Rejected(Reason::UndeclaredMetric {
            metric: name("extra")?
        })
    );
    Ok(())
}

#[test]
fn a_valid_scorecard_is_scored() -> TestResult {
    let scorecard = card(&[(GATE, "1"), (OBJECTIVE, "12.5")])?;
    assert_eq!(
        judged_outcome(Ok(scorecard.clone()), &judge_spec()?),
        Outcome::Scored(scorecard)
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Property: every observation maps without panicking, and only a full pass is `Pass`
// ---------------------------------------------------------------------------------------------------------------

fn any_end() -> impl Strategy<Value = Ended> {
    prop_oneof![
        Just(Ended::Exited),
        Just(Ended::Crashed(CrashCause::NoStatusFrame)),
        Just(Ended::Crashed(CrashCause::ConsoleCap)),
        Just(Ended::TimedOut),
        Just(Ended::ChunkProtocol),
    ]
}

fn any_output_fault() -> impl Strategy<Value = OutputFault> {
    prop_oneof![
        Just(OutputFault::TooLarge {
            cap: OutputCap::Stream
        }),
        Just(OutputFault::TooLarge {
            cap: OutputCap::Entries
        }),
        Just(OutputFault::TooLarge {
            cap: OutputCap::Total
        }),
        Just(OutputFault::Invalid {
            why: OutputInvalid::Path
        }),
        Just(OutputFault::Invalid {
            why: OutputInvalid::Truncated
        }),
        Just(OutputFault::Invalid {
            why: OutputInvalid::Malformed
        }),
    ]
}

fn any_status_fault() -> impl Strategy<Value = StatusFault> {
    prop_oneof![
        Just(StatusFault::Missing),
        Just(StatusFault::Invalid(GuestStatusError::TooLarge)),
        Just(StatusFault::Invalid(GuestStatusError::Json {
            line: 1,
            column: 1
        })),
        Just(StatusFault::WrongRole),
    ]
}

fn any_setup() -> impl Strategy<Value = SetupStatus> {
    prop_oneof![
        3 => Just(SetupStatus::Ok),
        1 => prop_oneof![
            Just(SetupStep::Input),
            Just(SetupStep::Mount),
            Just(SetupStep::Unpack),
            Just(SetupStep::Apply),
            Just(SetupStep::Output),
            Just(SetupStep::ScratchFull),
        ]
        .prop_map(|step| SetupStatus::Failed { step }),
    ]
}

fn any_apply() -> impl Strategy<Value = ApplyStatus> {
    prop_oneof![
        Just(ApplyStatus::Baseline),
        Just(ApplyStatus::Applied),
        Just(ApplyStatus::NotApplicable),
        (
            0_u32..4,
            prop_oneof![Just("src/parse.rs"), Just("src/lib.rs")]
        )
            .prop_filter_map("a valid path", |(hunk, text)| RelPath::from_str(text)
                .ok()
                .map(|path| ApplyStatus::Failed { path, hunk })),
    ]
}

fn any_stage_end() -> impl Strategy<Value = StageEnd> {
    prop_oneof![
        4 => Just(OK),
        1 => (-3_i32..256).prop_map(|code| StageEnd::Exited { code }),
        1 => (-2_i32..70).prop_map(|signal| StageEnd::Signaled { signal }),
        1 => Just(StageEnd::TimedOut),
        1 => Just(StageEnd::SpawnFailed),
        1 => Just(StageEnd::Skipped),
    ]
}

fn any_stages() -> impl Strategy<Value = Vec<StageReport>> {
    let stage = prop_oneof![Just(Stage::Build), Just(Stage::Run), Just(Stage::Judge)];
    prop::collection::vec((stage, any_stage_end()), 0..4).prop_map(|reports| {
        reports
            .into_iter()
            .map(|(stage, end)| StageReport::new(stage, end))
            .collect()
    })
}

fn any_output_status() -> impl Strategy<Value = OutputStatus> {
    prop_oneof![
        3 => (0_u32..2000, 0_u64..20_000_000)
            .prop_map(|(files, content_bytes)| OutputStatus::Packed { files, content_bytes }),
        1 => Just(OutputStatus::TooLarge),
        1 => Just(OutputStatus::Invalid),
        1 => Just(OutputStatus::NotPacked),
    ]
}

/// Every status `GuestStatus::new` accepts, and nothing it refuses.
fn any_status() -> impl Strategy<Value = GuestStatus> {
    (
        prop_oneof![Just(GuestRole::Candidate), Just(GuestRole::Judge)],
        any_setup(),
        any_apply(),
        any_stages(),
        any_output_status(),
    )
        .prop_filter_map(
            "a status GuestStatus::new accepts",
            |(role, setup, apply, stages, output)| {
                GuestStatus::new(role, setup, apply, stages, output).ok()
            },
        )
}

fn any_seen() -> impl Strategy<Value = Seen> {
    prop_oneof![
        1 => any_end().prop_map(Seen::NotExited),
        1 => any_output_fault().prop_map(Seen::Output),
        1 => any_status_fault().prop_map(Seen::Status),
        4 => any_status().prop_map(Seen::Guest),
    ]
}

fn stage_end(status: &GuestStatus, stage: Stage) -> Option<StageEnd> {
    status
        .stages()
        .iter()
        .find(|report| report.stage() == stage)
        .map(StageReport::end)
}

fn candidate_should_pass(seen: &Seen) -> bool {
    let Seen::Guest(status) = seen else {
        return false;
    };
    status.role() == GuestRole::Candidate
        && status.setup() == SetupStatus::Ok
        && matches!(status.apply(), ApplyStatus::Baseline | ApplyStatus::Applied)
        && stage_end(status, Stage::Build) == Some(OK)
        && stage_end(status, Stage::Run) == Some(OK)
        && matches!(status.output(), OutputStatus::Packed { .. })
}

fn judge_should_pass(seen: &Seen) -> bool {
    let Seen::Guest(status) = seen else {
        return false;
    };
    status.role() == GuestRole::Judge
        && status.setup() == SetupStatus::Ok
        && stage_end(status, Stage::Judge) == Some(OK)
        && matches!(status.output(), OutputStatus::Packed { .. })
}

/// A failed setup step must never be blamed on the candidate: it is infrastructure or configuration.
fn setup_failed_is_never_an_outcome(seen: &Seen, verdict: &Verdict) -> bool {
    match seen {
        Seen::Guest(status) if status.setup() != SetupStatus::Ok => {
            matches!(verdict, Verdict::Infra(_) | Verdict::Config(_))
        }
        _ => true,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn every_observation_maps_and_only_a_full_pass_is_pass(seen in any_seen()) {
        let candidate = candidate_verdict(&seen);
        let judge = judge_verdict(&seen);
        prop_assert_eq!(candidate == Verdict::Pass, candidate_should_pass(&seen));
        prop_assert_eq!(judge == Verdict::Pass, judge_should_pass(&seen));
        prop_assert!(setup_failed_is_never_an_outcome(&seen, &candidate));
        prop_assert!(setup_failed_is_never_an_outcome(&seen, &judge));
    }
}
