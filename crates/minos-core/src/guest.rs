//! The contract between the backend that writes a job and the guest that runs it (GUEST-02, GUEST-03, GUEST-04).
//!
//! Three wire forms live here, each with one codec that both sides link, so the writer and the reader cannot
//! drift apart (05-RESEARCH Architectural Responsibility Map):
//!
//! - [`GuestJob`]: the manifest subset a guest needs to run one role of one trial, as versioned JSON.
//! - [`GuestStatus`]: PID 1's own report, the file [`STATUS_PATH`] inside the authenticated output stream. It holds
//!   enums, numbers, and [`RelPath`]s only, never free text, so the host parses it without passing hostile text
//!   anywhere (05-RESEARCH Anti-Patterns).
//! - The input disk (05-CONTEXT D-15), written by the backend from blob bytes and read by PID 1. The disk is never
//!   stored as a blob itself.
//!
//! The decoders are byte-oriented (05-CONTEXT D-07): they take byte slices or a [`std::io::Read`], never a path,
//! so the AWS milestone's lifecycle-hook source reuses them unchanged. Core does no I/O of its own; the caller owns
//! every reader and writer.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::id::{BlobId, ExperimentId, TreeId};
use crate::job::JobRequest;
use crate::limits::{
    MAX_ARG_BYTES, MAX_ARGV_ENTRIES, MAX_MEMORY_MIB, MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRIES,
    MAX_SCRATCH_MIB, MAX_STAGE_MS, MAX_TRIALS, MAX_WRITABLE, MIN_MEMORY_MIB, MIN_SCRATCH_MIB,
    MIN_STAGE_MS, candidate_allowance_mib,
};
use crate::manifest::{CommandArgv, JudgeVersion, Manifest};
use crate::outcome::Stage;
use crate::path::RelPath;

/// The guest job wire version this code reads and writes.
pub const GUEST_JOB_VERSION: u32 = 1;

/// The guest status wire version this code reads and writes.
pub const GUEST_STATUS_VERSION: u32 = 1;

/// Most bytes of guest job JSON: 1 MiB, refused before the parser sees a byte.
pub const MAX_GUEST_JOB_BYTES: usize = 1_048_576;

/// Most bytes of guest status JSON: 16 KiB, refused before the parser sees a byte. A status is a handful of enums
/// and numbers plus at most one path.
pub const MAX_STATUS_BYTES: usize = 16_384;

/// The guest's output vsock port. The backend routes it to the job's host socket.
pub const OUTPUT_VSOCK_PORT: u32 = 1024;

/// Where PID 1's status lives inside the output tar, under the `.minos/` prefix the output layout reserves.
pub const STATUS_PATH: &str = ".minos/status.json";

/// Highest signal number a stage can report: Linux has signals 1 to 64.
const MAX_SIGNAL: i32 = 64;

// ---------------------------------------------------------------------------------------------------------------
// Roles and stages
// ---------------------------------------------------------------------------------------------------------------

/// Which role a guest runs: a candidate builds and runs the experiment, a judge scores it.
///
/// The agent role arrives with the agentic tier (TIER-01).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestRole {
    /// The built experiment: the build stage, then the run stage.
    Candidate,
    /// The project's judge command: the judge stage only.
    Judge,
}

impl GuestRole {
    /// The stages a job of this role runs, in order: build then run for a candidate, judge for a judge.
    #[must_use]
    pub const fn stages(self) -> &'static [Stage] {
        match self {
            Self::Candidate => &[Stage::Build, Stage::Run],
            Self::Judge => &[Stage::Judge],
        }
    }
}

/// One stage of a guest job: which stage, its argv, and its budget.
///
/// The argv has 1 to [`MAX_ARGV_ENTRIES`] entries, each at most [`MAX_ARG_BYTES`] bytes with no NUL, and a non-empty
/// first entry, the same rule as a manifest command. The budget is from [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`]
/// milliseconds. PID 1 enforces the budget itself, so a stage its deadline stopped is reported as
/// [`StageEnd::TimedOut`] with its own stage (05-RESEARCH Pitfall 12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawStageSpec", into = "RawStageSpec")]
pub struct StageSpec {
    stage: Stage,
    argv: Vec<String>,
    budget_ms: u64,
}

impl StageSpec {
    /// A stage from a validated manifest command and a validated budget.
    fn of(stage: Stage, argv: &CommandArgv, budget_ms: u64) -> Self {
        Self {
            stage,
            argv: argv.as_slice().to_vec(),
            budget_ms,
        }
    }

    /// Which stage this is.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// The command, as an argv array.
    #[must_use]
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    /// The stage budget, in milliseconds.
    #[must_use]
    pub const fn budget_ms(&self) -> u64 {
        self.budget_ms
    }
}

/// [`StageSpec`] as written, before the checks: `{"stage":..,"argv":[..],"budget_ms":..}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStageSpec {
    stage: Stage,
    argv: Vec<String>,
    budget_ms: u64,
}

impl TryFrom<RawStageSpec> for StageSpec {
    type Error = GuestJobError;

    fn try_from(raw: RawStageSpec) -> Result<Self, Self::Error> {
        check_argv(&raw.argv)?;
        if !(MIN_STAGE_MS..=MAX_STAGE_MS).contains(&raw.budget_ms) {
            return Err(GuestJobError::Budget);
        }
        Ok(Self {
            stage: raw.stage,
            argv: raw.argv,
            budget_ms: raw.budget_ms,
        })
    }
}

impl From<StageSpec> for RawStageSpec {
    fn from(spec: StageSpec) -> Self {
        Self {
            stage: spec.stage,
            argv: spec.argv,
            budget_ms: spec.budget_ms,
        }
    }
}

/// Checks an argv against the manifest command rule.
fn check_argv(argv: &[String]) -> Result<(), GuestJobError> {
    if !(1..=MAX_ARGV_ENTRIES).contains(&argv.len()) {
        return Err(GuestJobError::Argv);
    }
    if argv
        .iter()
        .any(|entry| entry.len() > MAX_ARG_BYTES || entry.contains('\0'))
    {
        return Err(GuestJobError::Argv);
    }
    if argv.first().is_none_or(String::is_empty) {
        return Err(GuestJobError::Argv);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// GuestJob
// ---------------------------------------------------------------------------------------------------------------

/// Why guest job JSON is not valid.
///
/// The job is written by the trusted backend, so a `serde_json` error is kept whole. Every other message is fixed
/// text.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum GuestJobError {
    /// The JSON is longer than [`MAX_GUEST_JOB_BYTES`].
    #[error("the guest job is over {MAX_GUEST_JOB_BYTES} bytes")]
    TooLarge,
    /// The bytes are not the guest job wire form: bad JSON, a missing or unknown field, or a malformed ID or path.
    #[error("the guest job is not valid JSON of the guest job wire form")]
    Json(#[source] serde_json::Error),
    /// The `v` field is not [`GUEST_JOB_VERSION`].
    #[error("the guest job version is not {GUEST_JOB_VERSION}")]
    Version,
    /// The trial index is not below [`MAX_TRIALS`].
    #[error("the trial must be below {MAX_TRIALS}")]
    Trial,
    /// The stages are not exactly the role's: build then run for a candidate, judge for a judge.
    #[error("the stages are not exactly the role's stages in order")]
    Stages,
    /// A candidate without a base tree, or a judge with one.
    #[error("a candidate job needs a base tree and a judge job has none")]
    Base,
    /// A candidate whose diff presence disagrees with its experiment (the baseline has no diff, every other
    /// experiment has one), or a judge with a diff.
    #[error("the diff does not match the experiment and role")]
    Diff,
    /// A candidate whose writable set is empty or over [`MAX_WRITABLE`], or a judge with a writable set.
    #[error("a candidate job needs 1 to {MAX_WRITABLE} writable paths and a judge job has none")]
    Writable,
    /// An argv that breaks the manifest command rule.
    #[error(
        "an argv must have 1 to {MAX_ARGV_ENTRIES} entries of at most {MAX_ARG_BYTES} bytes, no NUL, and a non-empty first entry"
    )]
    Argv,
    /// A stage budget outside [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`].
    #[error("a stage budget must be from {MIN_STAGE_MS} to {MAX_STAGE_MS} ms")]
    Budget,
    /// Memory or scratch outside its range, or the two breaking the memory budget rule (05-CONTEXT D-05).
    #[error("memory and scratch must lie in their ranges and leave the candidate its allowance")]
    Memory,
}

/// One role of one trial, as the guest runs it: the manifest subset PID 1 needs (05-CONTEXT D-06).
///
/// Built only by [`GuestJob::candidate`] or [`GuestJob::judge`] from a validated [`Manifest`] and [`JobRequest`],
/// or by parsing JSON through the same checks. Fields are private and there are no setters, so:
///
/// - a candidate's stages are build then run, a judge's are judge alone;
/// - a candidate has a base tree, and a diff exactly when its experiment is not the baseline of that base;
/// - a judge has no base, no diff, and no writable set, so it never sees the candidate's source;
/// - memory and scratch satisfy the same budget rule as the manifest (05-CONTEXT D-05).
///
/// The per-launch output token is not part of the job: it travels only in the input disk's header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawGuestJob", into = "RawGuestJob")]
pub struct GuestJob {
    role: GuestRole,
    exp: ExperimentId,
    judge: JudgeVersion,
    trial: u32,
    base: Option<TreeId>,
    diff: Option<BlobId>,
    writable: BTreeSet<RelPath>,
    stages: Vec<StageSpec>,
    memory_mib: u16,
    scratch_mib: u16,
    allowance_mib: u16,
}

impl GuestJob {
    /// The candidate job for `request`: the build and run commands and budgets, the writable set, and the memory
    /// budget from `manifest`; the experiment, base, diff, judge, and trial from `request`.
    ///
    /// Infallible: every value comes from an already validated manifest or request.
    #[must_use]
    pub fn candidate(manifest: &Manifest, request: &JobRequest) -> Self {
        let budget = request.budget();
        let commands = manifest.commands();
        let environment = manifest.environment();
        Self {
            role: GuestRole::Candidate,
            exp: request.exp(),
            judge: request.judge(),
            trial: request.trial(),
            base: Some(request.base()),
            diff: request.diff(),
            writable: manifest.writable().clone(),
            stages: vec![
                StageSpec::of(Stage::Build, commands.build(), budget.build_ms()),
                StageSpec::of(Stage::Run, commands.run(), budget.run_ms()),
            ],
            memory_mib: environment.memory_mib(),
            scratch_mib: environment.scratch_mib(),
            allowance_mib: environment.candidate_allowance_mib(),
        }
    }

    /// The judge job for `request`: the judge command and budget and the memory budget from `manifest`; the
    /// experiment, judge, and trial from `request`. No base, no diff, and an empty writable set.
    ///
    /// Infallible: every value comes from an already validated manifest or request.
    #[must_use]
    pub fn judge(manifest: &Manifest, request: &JobRequest) -> Self {
        let environment = manifest.environment();
        Self {
            role: GuestRole::Judge,
            exp: request.exp(),
            judge: request.judge(),
            trial: request.trial(),
            base: None,
            diff: None,
            writable: BTreeSet::new(),
            stages: vec![StageSpec::of(
                Stage::Judge,
                manifest.judge().command(),
                request.budget().judge_ms(),
            )],
            memory_mib: environment.memory_mib(),
            scratch_mib: environment.scratch_mib(),
            allowance_mib: environment.candidate_allowance_mib(),
        }
    }

    /// Parses guest job JSON through the same checks as the constructors, refusing more than
    /// [`MAX_GUEST_JOB_BYTES`] before the parser sees a byte.
    ///
    /// # Errors
    ///
    /// [`GuestJobError::TooLarge`], [`GuestJobError::Json`] for bytes that are not the wire form, otherwise the
    /// error of the first failed check.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, GuestJobError> {
        if bytes.len() > MAX_GUEST_JOB_BYTES {
            return Err(GuestJobError::TooLarge);
        }
        let raw: RawGuestJob = serde_json::from_slice(bytes).map_err(GuestJobError::Json)?;
        Self::try_from(raw)
    }

    /// The job's JSON wire form.
    ///
    /// # Errors
    ///
    /// [`GuestJobError::TooLarge`] when the JSON is over [`MAX_GUEST_JOB_BYTES`] (argv entries full of characters
    /// JSON escapes can get there), so the writer never produces a job the reader refuses; [`GuestJobError::Json`]
    /// if serialization fails.
    pub fn to_json(&self) -> Result<Vec<u8>, GuestJobError> {
        let bytes = serde_json::to_vec(self).map_err(GuestJobError::Json)?;
        if bytes.len() > MAX_GUEST_JOB_BYTES {
            return Err(GuestJobError::TooLarge);
        }
        Ok(bytes)
    }

    /// The role this job runs.
    #[must_use]
    pub const fn role(&self) -> GuestRole {
        self.role
    }

    /// The experiment.
    #[must_use]
    pub const fn exp(&self) -> ExperimentId {
        self.exp
    }

    /// The judge version that scores the trial. Named apart from [`GuestJob::judge`], the judge constructor.
    #[must_use]
    pub const fn judge_version(&self) -> JudgeVersion {
        self.judge
    }

    /// The trial index, from 0.
    #[must_use]
    pub const fn trial(&self) -> u32 {
        self.trial
    }

    /// The base tree a candidate unpacks; `None` for a judge.
    #[must_use]
    pub const fn base(&self) -> Option<TreeId> {
        self.base
    }

    /// The blob of the canonical cumulative diff a candidate applies; `None` for the baseline and for a judge.
    #[must_use]
    pub const fn diff(&self) -> Option<BlobId> {
        self.diff
    }

    /// The paths a diff may touch; empty for a judge.
    #[must_use]
    pub const fn writable(&self) -> &BTreeSet<RelPath> {
        &self.writable
    }

    /// The stages, in the order PID 1 runs them.
    #[must_use]
    pub fn stages(&self) -> &[StageSpec] {
        &self.stages
    }

    /// The VM's memory, in MiB.
    #[must_use]
    pub const fn memory_mib(&self) -> u16 {
        self.memory_mib
    }

    /// The tmpfs scratch cap, in MiB.
    #[must_use]
    pub const fn scratch_mib(&self) -> u16 {
        self.scratch_mib
    }

    /// The candidate allowance, in MiB: the memory less the guest reserve and the scratch cap. PID 1 sets each
    /// stage's `RLIMIT_AS` to it.
    #[must_use]
    pub const fn allowance_mib(&self) -> u16 {
        self.allowance_mib
    }
}

/// [`GuestJob`] as written, before the checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGuestJob {
    v: u32,
    role: GuestRole,
    exp: ExperimentId,
    judge: JudgeVersion,
    trial: u32,
    base: Option<TreeId>,
    diff: Option<BlobId>,
    writable: BTreeSet<RelPath>,
    stages: Vec<RawStageSpec>,
    memory_mib: u16,
    scratch_mib: u16,
}

impl TryFrom<RawGuestJob> for GuestJob {
    type Error = GuestJobError;

    fn try_from(raw: RawGuestJob) -> Result<Self, Self::Error> {
        if raw.v != GUEST_JOB_VERSION {
            return Err(GuestJobError::Version);
        }
        if raw.trial >= MAX_TRIALS {
            return Err(GuestJobError::Trial);
        }
        if !raw
            .stages
            .iter()
            .map(|stage| stage.stage)
            .eq(raw.role.stages().iter().copied())
        {
            return Err(GuestJobError::Stages);
        }
        match raw.role {
            GuestRole::Candidate => {
                let Some(base) = raw.base else {
                    return Err(GuestJobError::Base);
                };
                if raw.writable.is_empty() || raw.writable.len() > MAX_WRITABLE {
                    return Err(GuestJobError::Writable);
                }
                let is_baseline = raw.exp == ExperimentId::baseline(&base);
                if is_baseline != raw.diff.is_none() {
                    return Err(GuestJobError::Diff);
                }
            }
            GuestRole::Judge => {
                if raw.base.is_some() {
                    return Err(GuestJobError::Base);
                }
                if raw.diff.is_some() {
                    return Err(GuestJobError::Diff);
                }
                if !raw.writable.is_empty() {
                    return Err(GuestJobError::Writable);
                }
            }
        }
        let stages = raw
            .stages
            .into_iter()
            .map(StageSpec::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        if !(MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(&raw.memory_mib)
            || !(MIN_SCRATCH_MIB..=MAX_SCRATCH_MIB).contains(&raw.scratch_mib)
        {
            return Err(GuestJobError::Memory);
        }
        let Some(allowance_mib) = candidate_allowance_mib(raw.memory_mib, raw.scratch_mib) else {
            return Err(GuestJobError::Memory);
        };
        Ok(Self {
            role: raw.role,
            exp: raw.exp,
            judge: raw.judge,
            trial: raw.trial,
            base: raw.base,
            diff: raw.diff,
            writable: raw.writable,
            stages,
            memory_mib: raw.memory_mib,
            scratch_mib: raw.scratch_mib,
            allowance_mib,
        })
    }
}

impl From<GuestJob> for RawGuestJob {
    fn from(job: GuestJob) -> Self {
        Self {
            v: GUEST_JOB_VERSION,
            role: job.role,
            exp: job.exp,
            judge: job.judge,
            trial: job.trial,
            base: job.base,
            diff: job.diff,
            writable: job.writable,
            stages: job.stages.into_iter().map(RawStageSpec::from).collect(),
            memory_mib: job.memory_mib,
            scratch_mib: job.scratch_mib,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// GuestStatus
// ---------------------------------------------------------------------------------------------------------------

/// Whether PID 1's setup, everything before the first stage and the final pack, succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupStatus {
    /// Every setup step succeeded.
    Ok,
    /// A setup step failed, so no later stage ran.
    Failed {
        /// The step that failed.
        step: SetupStep,
    },
}

/// One of PID 1's setup steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupStep {
    /// Reading and checking the input disk and the job.
    Input,
    /// Mounting the scratch tmpfs and its bind mounts.
    Mount,
    /// Unpacking the base tarball or the judge's inputs.
    Unpack,
    /// Preparing the tree the diff applies to, or writing the applied files back.
    Apply,
    /// Packing or sending the output.
    Output,
}

/// How the candidate's diff went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplyStatus {
    /// The baseline: there is no diff to apply.
    Baseline,
    /// The diff applied.
    Applied,
    /// The diff did not apply, so no stage ran. The same payload as `Reason::ApplyFailed`.
    Failed {
        /// The file whose hunk did not apply.
        path: RelPath,
        /// 1-based number of the hunk that did not apply; 0 means the file is missing from the base tree.
        hunk: u32,
    },
    /// A judge applies no diff.
    NotApplicable,
}

/// How one stage ended, as PID 1 observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StageEnd {
    /// The stage's process exited with this code.
    Exited {
        /// The exit code.
        code: i32,
    },
    /// The stage's process was killed by this signal, 1 to 64.
    Signaled {
        /// The signal number.
        signal: i32,
    },
    /// PID 1's own deadline for the stage passed, and it killed the stage.
    TimedOut,
    /// The stage's process could not be started.
    SpawnFailed,
    /// The stage did not run because an earlier step failed.
    Skipped,
}

/// How PID 1 packed the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum OutputStatus {
    /// The output was packed: this many files and content bytes, within the output caps.
    Packed {
        /// Files in the output, at most `MAX_OUTPUT_ENTRIES`.
        files: u32,
        /// Content bytes over every file, at most `MAX_OUTPUT_CONTENT_BYTES`.
        content_bytes: u64,
    },
    /// The output was over a cap, so only PID 1's own files were packed.
    TooLarge,
    /// The output held something other than regular files with valid names, so only PID 1's own files were
    /// packed.
    Invalid,
    /// PID 1 did not reach the pack.
    NotPacked,
}

/// How one stage ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageReport {
    stage: Stage,
    end: StageEnd,
}

impl StageReport {
    /// The report that `stage` ended as `end`.
    #[must_use]
    pub const fn new(stage: Stage, end: StageEnd) -> Self {
        Self { stage, end }
    }

    /// Which stage.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// How it ended.
    #[must_use]
    pub const fn end(&self) -> StageEnd {
        self.end
    }
}

/// Why guest status JSON is not valid.
///
/// The status comes from inside the VM, so it is hostile: no message here carries a byte of it, and a JSON error
/// keeps only its line and column.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GuestStatusError {
    /// The JSON is longer than [`MAX_STATUS_BYTES`].
    #[error("the guest status is over {MAX_STATUS_BYTES} bytes")]
    TooLarge,
    /// The bytes are not the status wire form: bad JSON, a missing or unknown field, an unknown variant, or a
    /// path that is not a `RelPath`.
    #[error(
        "the guest status is not valid JSON of the status wire form (line {line}, column {column})"
    )]
    Json {
        /// 1-based line of the error.
        line: usize,
        /// 1-based column of the error.
        column: usize,
    },
    /// The `v` field is not [`GUEST_STATUS_VERSION`].
    #[error("the guest status version is not {GUEST_STATUS_VERSION}")]
    Version,
    /// A stage the role does not run, or stages out of order or listed twice.
    #[error("the stage reports are not in order, unique, and the role's")]
    Stages,
    /// A signal outside 1 to 64.
    #[error("a signal must be from 1 to {MAX_SIGNAL}")]
    Signal,
    /// A judge status whose apply is not `not_applicable`, or a candidate status whose apply is.
    #[error("only a judge status has apply not_applicable")]
    Apply,
    /// A packed output over the output caps.
    #[error(
        "a packed output must have at most {MAX_OUTPUT_ENTRIES} files and {MAX_OUTPUT_CONTENT_BYTES} content bytes"
    )]
    Output,
}

/// PID 1's report of one run, the file [`STATUS_PATH`] inside the authenticated output stream.
///
/// A closed type of enums, numbers, and [`RelPath`]s only, never free text: logs go in their own files. Fields
/// are private, and [`GuestStatus::new`] and the parser run the same checks: the stage reports are in order,
/// unique, and the role's; every signal is 1 to 64; a judge status has [`ApplyStatus::NotApplicable`] and a
/// candidate status does not; a packed output lies within the output caps.
///
/// Phase 7 maps a [`SetupStatus::Failed`] to an infrastructure failure (retried, bounded by the circuit breaker)
/// and each [`StageEnd`] to an `Outcome`. A stage PID 1's own deadline stopped is [`StageEnd::TimedOut`] with its
/// own stage, so the host deadline is only a backstop; when it fires, PID 1 never reported, and the host maps a
/// candidate to `Stage::Run` and a judge to `Stage::Judge` (05-RESEARCH Pitfall 12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawGuestStatus", into = "RawGuestStatus")]
pub struct GuestStatus {
    role: GuestRole,
    setup: SetupStatus,
    apply: ApplyStatus,
    stages: Vec<StageReport>,
    output: OutputStatus,
}

impl GuestStatus {
    /// A status from its parts, through the same checks as the parser.
    ///
    /// # Errors
    ///
    /// [`GuestStatusError::Apply`], [`GuestStatusError::Stages`], [`GuestStatusError::Signal`], or
    /// [`GuestStatusError::Output`] for the first failed check.
    pub fn new(
        role: GuestRole,
        setup: SetupStatus,
        apply: ApplyStatus,
        stages: Vec<StageReport>,
        output: OutputStatus,
    ) -> Result<Self, GuestStatusError> {
        let is_judge = role == GuestRole::Judge;
        if is_judge != (apply == ApplyStatus::NotApplicable) {
            return Err(GuestStatusError::Apply);
        }
        let allowed = role.stages();
        if stages.iter().any(|report| !allowed.contains(&report.stage))
            || stages
                .iter()
                .zip(stages.iter().skip(1))
                .any(|(earlier, later)| earlier.stage >= later.stage)
        {
            return Err(GuestStatusError::Stages);
        }
        if stages.iter().any(|report| {
            matches!(report.end, StageEnd::Signaled { signal } if !(1..=MAX_SIGNAL).contains(&signal))
        }) {
            return Err(GuestStatusError::Signal);
        }
        if let OutputStatus::Packed {
            files,
            content_bytes,
        } = output
        {
            let files_ok = usize::try_from(files).is_ok_and(|files| files <= MAX_OUTPUT_ENTRIES);
            if !files_ok || content_bytes > MAX_OUTPUT_CONTENT_BYTES {
                return Err(GuestStatusError::Output);
            }
        }
        Ok(Self {
            role,
            setup,
            apply,
            stages,
            output,
        })
    }

    /// Parses guest status JSON through the same checks as [`GuestStatus::new`], refusing more than
    /// [`MAX_STATUS_BYTES`] before the parser sees a byte.
    ///
    /// # Errors
    ///
    /// [`GuestStatusError::TooLarge`], [`GuestStatusError::Json`] for bytes that are not the wire form,
    /// [`GuestStatusError::Version`], otherwise the error of the first failed check.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, GuestStatusError> {
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(GuestStatusError::TooLarge);
        }
        let raw: RawGuestStatus =
            serde_json::from_slice(bytes).map_err(|error| GuestStatusError::Json {
                line: error.line(),
                column: error.column(),
            })?;
        Self::try_from(raw)
    }

    /// The status's JSON wire form.
    ///
    /// # Errors
    ///
    /// [`GuestStatusError::TooLarge`] when the JSON is over [`MAX_STATUS_BYTES`], which a valid status cannot
    /// reach; [`GuestStatusError::Json`] if serialization fails.
    pub fn to_json(&self) -> Result<Vec<u8>, GuestStatusError> {
        let bytes = serde_json::to_vec(self).map_err(|error| GuestStatusError::Json {
            line: error.line(),
            column: error.column(),
        })?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(GuestStatusError::TooLarge);
        }
        Ok(bytes)
    }

    /// The role that ran.
    #[must_use]
    pub const fn role(&self) -> GuestRole {
        self.role
    }

    /// How setup went.
    #[must_use]
    pub const fn setup(&self) -> SetupStatus {
        self.setup
    }

    /// How the diff went.
    #[must_use]
    pub const fn apply(&self) -> &ApplyStatus {
        &self.apply
    }

    /// How each stage that PID 1 reached ended, in order.
    #[must_use]
    pub fn stages(&self) -> &[StageReport] {
        &self.stages
    }

    /// How the output was packed.
    #[must_use]
    pub const fn output(&self) -> OutputStatus {
        self.output
    }
}

/// [`GuestStatus`] as written, before the checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGuestStatus {
    v: u32,
    role: GuestRole,
    setup: SetupStatus,
    apply: ApplyStatus,
    stages: Vec<StageReport>,
    output: OutputStatus,
}

impl TryFrom<RawGuestStatus> for GuestStatus {
    type Error = GuestStatusError;

    fn try_from(raw: RawGuestStatus) -> Result<Self, Self::Error> {
        if raw.v != GUEST_STATUS_VERSION {
            return Err(GuestStatusError::Version);
        }
        Self::new(raw.role, raw.setup, raw.apply, raw.stages, raw.output)
    }
}

impl From<GuestStatus> for RawGuestStatus {
    fn from(status: GuestStatus) -> Self {
        Self {
            v: GUEST_STATUS_VERSION,
            role: status.role,
            setup: status.setup,
            apply: status.apply,
            stages: status.stages,
            output: status.output,
        }
    }
}
