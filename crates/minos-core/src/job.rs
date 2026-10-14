//! `JobRequest` and `JobResult`: the contract between the loop and the runner for one experiment trial (CORE-13).
//!
//! `minos-loop` sends a [`JobRequest`] and `minos-runner` answers with a [`JobResult`], through the loop's
//! `JobRunner` trait (Phase 7). The loop never links the sandbox crate or a backend, so this pair of types is the
//! whole of what it knows about a VM run.
//!
//! A request names its diff by blob, never inline. The runner fetches that blob, parses it with `Diff::parse`, and
//! must call [`JobRequest::verify_diff`] before it builds the candidate VM, so a runner never applies a diff that is
//! not the experiment's. The baseline of a base tree is the only experiment without a diff (03-CONTEXT D-04), so a
//! request has `diff: None` exactly when `exp` is [`ExperimentId::baseline`] of `base`.
//!
//! In v1 the result returns in-process; only the scorecard inside it crossed the vsock boundary, and that went
//! through `Scorecard::parse_json` under its own cap. Both types are still versioned wire formats (`"v":1`) that
//! refuse unknown fields, so the AWS milestone can carry them through a queue or a checkpoint unchanged.
//!
//! No field here is a 128-bit integer or a dollar amount: usage is metered in [`Usage`] and priced later.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diff::Diff;
use crate::experiment::Experiment;
use crate::id::{BlobId, ExperimentId, TreeId};
use crate::ledger::BackendInfo;
use crate::limits::{MAX_STAGE_MS, MAX_TRIALS, MIN_STAGE_MS};
use crate::manifest::{Budget, JudgeVersion};
use crate::outcome::{Outcome, Stage};
use crate::usage::Usage;

/// The job wire version this code reads and writes.
pub const JOB_VERSION: u32 = 1;

/// Why a job request or result is not valid.
///
/// Job JSON is produced by trusted code on both sides, so a `serde_json` error is kept whole.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// The bytes are not the job wire form: bad JSON, a missing or unknown field, or a malformed ID.
    #[error("the job is not valid JSON of the job wire form")]
    Json(#[source] serde_json::Error),
    /// The `v` field is not [`JOB_VERSION`].
    #[error("the job version is not {JOB_VERSION}")]
    Version,
    /// The trial index is not below [`MAX_TRIALS`].
    #[error("the trial must be below {MAX_TRIALS}")]
    Trial,
    /// A stage budget is outside [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`].
    #[error("budget {field} must be from {MIN_STAGE_MS} to {MAX_STAGE_MS} ms")]
    Budget {
        /// The budget field.
        field: &'static str,
    },
    /// The diff does not match the experiment: a baseline with a diff, a candidate without one, or a diff blob
    /// that is not the content address of the experiment's canonical diff.
    #[error("the diff does not match the experiment")]
    DiffMismatch,
    /// A candidate request was built from the baseline experiment.
    #[error("the baseline is not a candidate")]
    BaselineAsCandidate,
    /// A result's usage carries tokens. A job is a VM run with no model call, so its usage is compute time only;
    /// tokens here would be an impossible ledger record that `Cost::of` could not price.
    #[error("a job result carries compute usage only, never tokens")]
    TokensInJob,
}

/// Checks that a job's usage carries no tokens: a VM run makes no model call.
const fn check_usage(usage: Usage) -> Result<Usage, JobError> {
    match usage.total_tokens() {
        Some(0) => Ok(usage),
        Some(_) | None => Err(JobError::TokensInJob),
    }
}

/// Checks that `trial` is a valid trial index.
const fn check_trial(trial: u32) -> Result<u32, JobError> {
    if trial < MAX_TRIALS {
        Ok(trial)
    } else {
        Err(JobError::Trial)
    }
}

/// Checks that one stage budget lies within the stage bounds.
const fn check_stage(field: &'static str, ms: u64) -> Result<u64, JobError> {
    if ms >= MIN_STAGE_MS && ms <= MAX_STAGE_MS {
        Ok(ms)
    } else {
        Err(JobError::Budget { field })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JobBudget
// ---------------------------------------------------------------------------------------------------------------

/// The build, run, and judge budgets of one job, each from [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`] milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobBudget {
    build: u64,
    run: u64,
    judge: u64,
}

impl JobBudget {
    /// A budget from three stage limits in milliseconds.
    ///
    /// # Errors
    ///
    /// [`JobError::Budget`] naming the first stage outside [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`].
    pub fn new(build_ms: u64, run_ms: u64, judge_ms: u64) -> Result<Self, JobError> {
        Ok(Self {
            build: check_stage("build_ms", build_ms)?,
            run: check_stage("run_ms", run_ms)?,
            judge: check_stage("judge_ms", judge_ms)?,
        })
    }

    /// The manifest's stage budgets. Infallible: a manifest's budgets already lie within the stage bounds.
    #[must_use]
    pub const fn from_budget(budget: &Budget) -> Self {
        Self {
            build: budget.build_ms(),
            run: budget.run_ms(),
            judge: budget.judge_ms(),
        }
    }

    /// The build stage budget, in milliseconds.
    #[must_use]
    pub const fn build_ms(&self) -> u64 {
        self.build
    }

    /// The run stage budget, in milliseconds.
    #[must_use]
    pub const fn run_ms(&self) -> u64 {
        self.run
    }

    /// The judge stage budget, in milliseconds.
    #[must_use]
    pub const fn judge_ms(&self) -> u64 {
        self.judge
    }
}

/// [`JobBudget`] as written, before range checks: `{"build_ms":..,"run_ms":..,"judge_ms":..}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJobBudget {
    #[serde(rename = "build_ms")]
    build: u64,
    #[serde(rename = "run_ms")]
    run: u64,
    #[serde(rename = "judge_ms")]
    judge: u64,
}

impl TryFrom<RawJobBudget> for JobBudget {
    type Error = JobError;

    fn try_from(raw: RawJobBudget) -> Result<Self, Self::Error> {
        Self::new(raw.build, raw.run, raw.judge)
    }
}

impl From<JobBudget> for RawJobBudget {
    fn from(budget: JobBudget) -> Self {
        Self {
            build: budget.build,
            run: budget.run,
            judge: budget.judge,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JobRequest
// ---------------------------------------------------------------------------------------------------------------

/// One trial of one experiment under one judge version, as the loop asks the runner for it.
///
/// Fields are private, and every constructor and the deserializer run the same checks, so `diff` is `None`
/// exactly when `exp` is the baseline of `base`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawJobRequest", into = "RawJobRequest")]
pub struct JobRequest {
    exp: ExperimentId,
    base: TreeId,
    diff: Option<BlobId>,
    judge: JudgeVersion,
    trial: u32,
    budget: JobBudget,
}

impl JobRequest {
    /// A request for trial `trial` of the baseline of `base`: no diff, under [`ExperimentId::baseline`].
    ///
    /// # Errors
    ///
    /// [`JobError::Trial`] when `trial` is not below [`MAX_TRIALS`].
    pub fn baseline(
        base: TreeId,
        judge: JudgeVersion,
        trial: u32,
        budget: JobBudget,
    ) -> Result<Self, JobError> {
        Ok(Self {
            exp: ExperimentId::baseline(&base),
            base,
            diff: None,
            judge,
            trial: check_trial(trial)?,
            budget,
        })
    }

    /// A request for trial `trial` of a candidate experiment, whose canonical diff is stored as `diff_blob`.
    ///
    /// # Errors
    ///
    /// [`JobError::BaselineAsCandidate`] for the baseline experiment, [`JobError::DiffMismatch`] when `diff_blob`
    /// is not the content address of the experiment's canonical diff, and [`JobError::Trial`] when `trial` is not
    /// below [`MAX_TRIALS`].
    pub fn candidate(
        experiment: &Experiment,
        diff_blob: BlobId,
        judge: JudgeVersion,
        trial: u32,
        budget: JobBudget,
    ) -> Result<Self, JobError> {
        let Some(diff) = experiment.diff() else {
            return Err(JobError::BaselineAsCandidate);
        };
        if BlobId::of(diff.as_bytes()) != diff_blob {
            return Err(JobError::DiffMismatch);
        }
        Ok(Self {
            exp: experiment.id(),
            base: experiment.base(),
            diff: Some(diff_blob),
            judge,
            trial: check_trial(trial)?,
            budget,
        })
    }

    /// Parses job request JSON through the same checks as the constructors.
    ///
    /// # Errors
    ///
    /// [`JobError::Json`] for bytes that are not the wire form, otherwise the error of the first failed check.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, JobError> {
        let raw: RawJobRequest = serde_json::from_slice(bytes).map_err(JobError::Json)?;
        Self::try_from(raw)
    }

    /// Whether `diff` is this request's diff: `ExperimentId::of(base, diff)` equals `exp`. Always false for a
    /// baseline request. The runner calls this on the fetched diff before it builds the candidate VM.
    #[must_use]
    pub fn verify_diff(&self, diff: &Diff) -> bool {
        self.diff.is_some() && ExperimentId::of(&self.base, diff) == self.exp
    }

    /// The experiment.
    #[must_use]
    pub const fn exp(&self) -> ExperimentId {
        self.exp
    }

    /// The run's base tree.
    #[must_use]
    pub const fn base(&self) -> TreeId {
        self.base
    }

    /// The blob of the experiment's canonical cumulative diff, or `None` for the baseline.
    #[must_use]
    pub const fn diff(&self) -> Option<BlobId> {
        self.diff
    }

    /// Whether this request is for the baseline.
    #[must_use]
    pub const fn is_baseline(&self) -> bool {
        self.diff.is_none()
    }

    /// The judge version that scores the trial.
    #[must_use]
    pub const fn judge(&self) -> JudgeVersion {
        self.judge
    }

    /// The trial index, from 0.
    #[must_use]
    pub const fn trial(&self) -> u32 {
        self.trial
    }

    /// The stage budgets.
    #[must_use]
    pub const fn budget(&self) -> JobBudget {
        self.budget
    }
}

/// [`JobRequest`] as written, before the checks.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJobRequest {
    v: u32,
    exp: ExperimentId,
    base: TreeId,
    diff: Option<BlobId>,
    judge: JudgeVersion,
    trial: u32,
    budget: RawJobBudget,
}

impl TryFrom<RawJobRequest> for JobRequest {
    type Error = JobError;

    fn try_from(raw: RawJobRequest) -> Result<Self, Self::Error> {
        if raw.v != JOB_VERSION {
            return Err(JobError::Version);
        }
        let trial = check_trial(raw.trial)?;
        let budget = JobBudget::try_from(raw.budget)?;
        let is_baseline = raw.exp == ExperimentId::baseline(&raw.base);
        if is_baseline != raw.diff.is_none() {
            return Err(JobError::DiffMismatch);
        }
        Ok(Self {
            exp: raw.exp,
            base: raw.base,
            diff: raw.diff,
            judge: raw.judge,
            trial,
            budget,
        })
    }
}

impl From<JobRequest> for RawJobRequest {
    fn from(request: JobRequest) -> Self {
        Self {
            v: JOB_VERSION,
            exp: request.exp,
            base: request.base,
            diff: request.diff,
            judge: request.judge,
            trial: request.trial,
            budget: request.budget.into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JobResult
// ---------------------------------------------------------------------------------------------------------------

/// How one requested trial ended: the outcome, the metered usage, where the VMs ran, and the log blob of each stage
/// that produced one.
///
/// A trial that ended in an infrastructure failure has no `JobResult`; that is an error on the runner's own path,
/// and it is retried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawJobResult", into = "RawJobResult")]
pub struct JobResult {
    exp: ExperimentId,
    judge: JudgeVersion,
    trial: u32,
    outcome: Outcome,
    usage: Usage,
    backend: BackendInfo,
    logs: BTreeMap<Stage, BlobId>,
}

impl JobResult {
    /// The result of `request`: the experiment, judge, and trial are copied from it, so a result always names the
    /// trial it answers. `usage` is the VMs' compute time.
    ///
    /// # Errors
    ///
    /// [`JobError::TokensInJob`] if `usage` carries any tokens: a job makes no model call.
    pub fn new(
        request: &JobRequest,
        outcome: Outcome,
        usage: Usage,
        backend: BackendInfo,
        logs: BTreeMap<Stage, BlobId>,
    ) -> Result<Self, JobError> {
        let usage = check_usage(usage)?;
        Ok(Self {
            exp: request.exp,
            judge: request.judge,
            trial: request.trial,
            outcome,
            usage,
            backend,
            logs,
        })
    }

    /// Parses job result JSON through the same checks as the deserializer.
    ///
    /// # Errors
    ///
    /// [`JobError::Json`] for bytes that are not the wire form, [`JobError::Version`], [`JobError::Trial`], or
    /// [`JobError::TokensInJob`].
    pub fn parse_json(bytes: &[u8]) -> Result<Self, JobError> {
        let raw: RawJobResult = serde_json::from_slice(bytes).map_err(JobError::Json)?;
        Self::try_from(raw)
    }

    /// The experiment.
    #[must_use]
    pub const fn exp(&self) -> ExperimentId {
        self.exp
    }

    /// The judge version that scored the trial.
    #[must_use]
    pub const fn judge(&self) -> JudgeVersion {
        self.judge
    }

    /// The trial index, from 0.
    #[must_use]
    pub const fn trial(&self) -> u32 {
        self.trial
    }

    /// How the trial ended.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// The metered usage of the trial's VM runs.
    #[must_use]
    pub const fn usage(&self) -> Usage {
        self.usage
    }

    /// Where and how the VMs ran.
    #[must_use]
    pub const fn backend(&self) -> &BackendInfo {
        &self.backend
    }

    /// The log blob of each stage that produced one, in stage order.
    #[must_use]
    pub const fn logs(&self) -> &BTreeMap<Stage, BlobId> {
        &self.logs
    }
}

/// [`JobResult`] as written, before the checks. `logs` is a JSON object keyed by stage name.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJobResult {
    v: u32,
    exp: ExperimentId,
    judge: JudgeVersion,
    trial: u32,
    outcome: Outcome,
    usage: Usage,
    backend: BackendInfo,
    logs: BTreeMap<Stage, BlobId>,
}

impl TryFrom<RawJobResult> for JobResult {
    type Error = JobError;

    fn try_from(raw: RawJobResult) -> Result<Self, Self::Error> {
        if raw.v != JOB_VERSION {
            return Err(JobError::Version);
        }
        Ok(Self {
            exp: raw.exp,
            judge: raw.judge,
            trial: check_trial(raw.trial)?,
            outcome: raw.outcome,
            usage: check_usage(raw.usage)?,
            backend: raw.backend,
            logs: raw.logs,
        })
    }
}

impl From<JobResult> for RawJobResult {
    fn from(result: JobResult) -> Self {
        Self {
            v: JOB_VERSION,
            exp: result.exp,
            judge: result.judge,
            trial: result.trial,
            outcome: result.outcome,
            usage: result.usage,
            backend: result.backend,
            logs: result.logs,
        }
    }
}
