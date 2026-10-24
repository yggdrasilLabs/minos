//! The runner's whole blame table: what the host observed of one VM, mapped to a verdict (LOOP-01, JUDGE-03).
//!
//! The backend has already classified the helper's exit by the refined K7 rule (02-SPIKE-EVIDENCE.md K7 table;
//! the krun backend's `supervise::classify`), so the runner maps only what it receives, as a pure function with
//! one tested row per observation (07-RESEARCH Pattern 2). The runner observes a launch in a fixed order, and
//! [`Seen`] holds the first observation that decides: the host's [`Ended`]; then, for a launch that exited, the
//! output stream's [`OutputFault`] from `into_output`; then the status's [`StatusFault`] from `take_status`; then
//! the validated [`GuestStatus`] itself.
//!
//! | Observation | Candidate | Judge |
//! |---|---|---|
//! | `Ended::TimedOut` (the host deadline, a backstop) | `TimedOut { Run }` | `TimedOut { Judge }` |
//! | `Ended::Crashed(_)` (no status frame, console cap) | `Crashed { Run }` | `Crashed { Judge }` |
//! | `Ended::ChunkProtocol` | `Rejected(ChunkProtocol)` | `Rejected(ChunkProtocol)` |
//! | An `OutputFault` | `Rejected(fault.reason())` | same |
//! | A `StatusFault` (missing, invalid, wrong role) | `Rejected(OutputInvalid)` | same |
//! | Setup failed at input, mount, unpack, apply, or output | infrastructure, retried | same |
//! | Setup failed with `ScratchFull` | configuration, never retried | same: `Runner::new` proved the scratch holds the judge data and any candidate output (CR-01) |
//! | A status of the other role | `Rejected(OutputInvalid)` | same |
//! | Apply failed | `Rejected(ApplyFailed { path, hunk })` | n/a |
//! | Build timed out | `TimedOut { Build }` | n/a |
//! | Build non-zero, signaled, or not spawned | `Rejected(BuildFailed)` | n/a |
//! | Run timed out | `TimedOut { Run }` | n/a |
//! | Run non-zero, signaled, or not spawned | `Rejected(RunFailed)` | n/a |
//! | Judge timed out | n/a | `TimedOut { Judge }` |
//! | Judge non-zero, signaled, or not spawned | n/a | `Crashed { Judge }` |
//! | A needed stage missing or skipped | `Rejected(OutputInvalid)` | same |
//! | Output `TooLarge` after passing stages | `Rejected(OutputTooLarge)` | same |
//! | Output `Invalid` or `NotPacked` after passing stages | `Rejected(OutputInvalid)` | same |
//! | Everything passed, output `Packed` | [`Verdict::Pass`]: launch the judge | [`Verdict::Pass`]: read the scorecard |
//!
//! A judge that passed hands its scorecard to [`judged_outcome`], which checks it gates first (JUDGE-03).
//!
//! Setup is read before every other field of a status, the role included (review CR-02): PID 1 sends a placeholder
//! role when it could not parse its job, so an input failure is infrastructure whatever role it names.

use minos_core::guest::{
    ApplyStatus, GuestRole, GuestStatus, OutputStatus, SetupStatus, SetupStep, StageEnd,
    StageReport,
};
use minos_core::manifest::JudgeSpec;
use minos_core::metric::Scorecard;
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::policy::check_scorecard;
use minos_sandbox::{Ended, OutputFault, ScorecardFault, StatusFault};

use crate::error::{ConfigCause, InfraCause};

/// The first observation of one launch that decides its verdict, in the order the runner observes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// The host observed an end other than [`Ended::Exited`], so the runner reads no output. Trusted.
    NotExited(Ended),
    /// The launch exited, and `into_output` refused the output stream.
    Output(OutputFault),
    /// The output stream was read, and `take_status` refused PID 1's status.
    Status(StatusFault),
    /// PID 1's validated status. Hostile in origin, validated in shape: a guest that exploited its kernel could
    /// still forge it, so it decides only the candidate's or the judge's own outcome.
    Guest(GuestStatus),
}

/// What the runner does with one launch.
///
/// Not `#[non_exhaustive]`: every caller must handle every verdict, and a new one must break the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every pass condition held: launch the judge for a candidate, read the scorecard for a judge.
    Pass,
    /// The experiment is decided: record this outcome. A candidate fault, never retried.
    Done(Outcome),
    /// Infrastructure failed: retry the step and count it toward the breaker.
    Infra(InfraCause),
    /// The configuration cannot work: stop the run and record nothing.
    Config(ConfigCause),
}

/// The verdict on a candidate launch.
#[must_use]
pub fn candidate_verdict(seen: &Seen) -> Verdict {
    match seen {
        Seen::NotExited(ended) => not_exited(*ended, Stage::Run, Stage::Run),
        Seen::Output(fault) => rejected(fault.reason()),
        Seen::Status(fault) => rejected(fault.reason()),
        Seen::Guest(status) => candidate_status(status),
    }
}

/// The verdict on a judge launch. [`Verdict::Pass`] means the judge exited 0 with packed output, and the runner
/// reads its scorecard and maps it with [`judged_outcome`].
#[must_use]
pub fn judge_verdict(seen: &Seen) -> Verdict {
    match seen {
        Seen::NotExited(ended) => not_exited(*ended, Stage::Judge, Stage::Judge),
        Seen::Output(fault) => rejected(fault.reason()),
        Seen::Status(fault) => rejected(fault.reason()),
        Seen::Guest(status) => judge_status(status),
    }
}

/// The outcome of a judge that passed, from its scorecard (JUDGE-03).
///
/// A scorecard that could not be read is `Rejected(ScorecardInvalid)`. A read scorecard goes through
/// [`check_scorecard`], which checks the gates first: a gate that is not exactly 1 is `Rejected(GateFailed)`, and
/// the objectives are never read, even when one is missing. Only a scorecard that passes every check is
/// `Scored`.
#[must_use]
pub fn judged_outcome(card: Result<Scorecard, ScorecardFault>, judge: &JudgeSpec) -> Outcome {
    match card {
        Err(fault) => Outcome::Rejected(fault.reason()),
        Ok(card) => match check_scorecard(judge, &card) {
            Err(reason) => Outcome::Rejected(reason),
            Ok(()) => Outcome::Scored(card),
        },
    }
}

/// A host-observed end other than `Exited` (K7). A host timeout is the backstop, because PID 1 enforces each
/// stage's deadline itself, so it is attributed to `timeout_stage` (05-RESEARCH Pitfall 12); a host crash is
/// attributed to `crash_stage` (07-RESEARCH A2).
const fn not_exited(ended: Ended, timeout_stage: Stage, crash_stage: Stage) -> Verdict {
    match ended {
        Ended::TimedOut => Verdict::Done(Outcome::TimedOut {
            stage: timeout_stage,
        }),
        Ended::Crashed(_) => Verdict::Done(Outcome::Crashed { stage: crash_stage }),
        Ended::ChunkProtocol => rejected(Reason::ChunkProtocol),
        // `Exited` is never `NotExited` in the pipeline, and a backend end this runner does not know has no row:
        // either fails closed as infrastructure, never blamed on the candidate.
        _ => Verdict::Infra(InfraCause::Unmapped),
    }
}

const fn rejected(reason: Reason) -> Verdict {
    Verdict::Done(Outcome::Rejected(reason))
}

/// A failed setup step: configuration for a full scratch (guest.rs WR-02), infrastructure for every other step,
/// whatever role the status names (review CR-02). `None` when setup succeeded.
const fn setup_verdict(setup: SetupStatus) -> Option<Verdict> {
    match setup {
        SetupStatus::Ok => None,
        SetupStatus::Failed {
            step: SetupStep::ScratchFull,
        } => Some(Verdict::Config(ConfigCause::ScratchFull)),
        SetupStatus::Failed {
            step:
                step @ (SetupStep::Input
                | SetupStep::Mount
                | SetupStep::Unpack
                | SetupStep::Apply
                | SetupStep::Output),
        } => Some(Verdict::Infra(InfraCause::GuestSetup { step })),
    }
}

/// How `stage` ended, if PID 1 reported it.
fn stage_end(status: &GuestStatus, stage: Stage) -> Option<StageEnd> {
    status
        .stages()
        .iter()
        .find(|report| report.stage() == stage)
        .map(StageReport::end)
}

/// The verdict on a candidate stage that must exit 0 for the candidate to pass. `None` when it did.
///
/// A stage PID 1's own deadline stopped is `TimedOut` with its own stage; a non-zero exit, a signal, or a failed
/// spawn is `failed` (07-RESEARCH A3); a missing or skipped stage, where a passing candidate needs it, means the
/// status is inconsistent and is `OutputInvalid`.
fn candidate_stage(status: &GuestStatus, stage: Stage, failed: Reason) -> Option<Verdict> {
    match stage_end(status, stage) {
        Some(StageEnd::Exited { code: 0 }) => None,
        Some(StageEnd::TimedOut) => Some(Verdict::Done(Outcome::TimedOut { stage })),
        Some(StageEnd::Exited { .. } | StageEnd::Signaled { .. } | StageEnd::SpawnFailed) => {
            Some(rejected(failed))
        }
        Some(StageEnd::Skipped) | None => Some(rejected(Reason::OutputInvalid)),
    }
}

/// The verdict on the output status of a role whose stages passed: `Pass` only for packed output.
const fn output_verdict(output: OutputStatus) -> Verdict {
    match output {
        OutputStatus::Packed { .. } => Verdict::Pass,
        OutputStatus::TooLarge => rejected(Reason::OutputTooLarge),
        OutputStatus::Invalid | OutputStatus::NotPacked => rejected(Reason::OutputInvalid),
    }
}

/// A candidate's validated status: setup, then role, then apply, then build, then run, then output.
fn candidate_status(status: &GuestStatus) -> Verdict {
    if let Some(verdict) = setup_verdict(status.setup()) {
        return verdict;
    }
    if status.role() != GuestRole::Candidate {
        return rejected(Reason::OutputInvalid);
    }
    match status.apply() {
        ApplyStatus::Baseline | ApplyStatus::Applied => {}
        ApplyStatus::Failed { path, hunk } => {
            return rejected(Reason::ApplyFailed {
                path: path.clone(),
                hunk: *hunk,
            });
        }
        // `GuestStatus::new` refuses a candidate status with this apply; refuse it here too, fail closed.
        ApplyStatus::NotApplicable => return rejected(Reason::OutputInvalid),
    }
    if let Some(verdict) = candidate_stage(status, Stage::Build, Reason::BuildFailed) {
        return verdict;
    }
    if let Some(verdict) = candidate_stage(status, Stage::Run, Reason::RunFailed) {
        return verdict;
    }
    output_verdict(status.output())
}

/// A judge's validated status: setup, then role, then the judge stage, then output.
///
/// A judge stage that failed is `Crashed { Judge }`, because no `Reason` names a judge failure (07-RESEARCH A2).
/// A judge that exited 0 passes only with packed output, because otherwise PID 1 packed only its own files and the
/// scorecard is not among them.
fn judge_status(status: &GuestStatus) -> Verdict {
    if let Some(verdict) = setup_verdict(status.setup()) {
        return verdict;
    }
    if status.role() != GuestRole::Judge {
        return rejected(Reason::OutputInvalid);
    }
    match stage_end(status, Stage::Judge) {
        Some(StageEnd::Exited { code: 0 }) => output_verdict(status.output()),
        Some(StageEnd::TimedOut) => Verdict::Done(Outcome::TimedOut {
            stage: Stage::Judge,
        }),
        Some(StageEnd::Exited { .. } | StageEnd::Signaled { .. } | StageEnd::SpawnFailed) => {
            Verdict::Done(Outcome::Crashed {
                stage: Stage::Judge,
            })
        }
        Some(StageEnd::Skipped) | None => rejected(Reason::OutputInvalid),
    }
}
