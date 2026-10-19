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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::chunk::{ChunkToken, TOKEN_BYTES};
use crate::id::{BlobId, ExperimentId, TreeId};
use crate::job::JobRequest;
use crate::limits::{
    GUEST_RESERVE_MIB, MAX_ARG_BYTES, MAX_ARGV_ENTRIES, MAX_DIFF_BYTES, MAX_MEMORY_MIB,
    MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_STREAM_BYTES, MAX_SCRATCH_MIB,
    MAX_STAGE_MS, MAX_TRIALS, MAX_WRITABLE, MIN_MEMORY_MIB, MIN_SCRATCH_MIB, MIN_STAGE_MS,
    candidate_allowance_mib,
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
    ///
    /// This step precedes the role: when the job did not parse, PID 1 cannot know its role, and the status it
    /// sends carries a placeholder (`candidate`, with apply `baseline`). The host therefore reads `setup` before
    /// `role` for an `Input` failure and does not check the role (review CR-02); every failed setup is an
    /// infrastructure failure whatever the other fields say.
    Input,
    /// Mounting the scratch tmpfs and its bind mounts.
    Mount,
    /// Unpacking the base tarball or the judge's inputs.
    Unpack,
    /// Preparing the tree the diff applies to, or writing the applied files back.
    Apply,
    /// Packing or sending the output.
    Output,
    /// The scratch tmpfs filled while an input was unpacked or the applied diff was written back: the
    /// manifest's `[environment] scratch_mib` is too small for the role's inputs (up to 64 MiB of base tree for
    /// a candidate; up to 96 MiB of judge data plus 16 MiB of candidate output for a judge). The same inputs
    /// fill it on every launch, so this is a configuration error, not an infrastructure failure: Phase 7 must
    /// report it against `scratch_mib` and must not retry it (review WR-02). Added after the first status golden;
    /// an added variant keeps every earlier status parseable, so the version stays 1.
    ScratchFull,
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

// ---------------------------------------------------------------------------------------------------------------
// Input disk (05-CONTEXT D-15)
// ---------------------------------------------------------------------------------------------------------------

/// Magic bytes at the start of the input disk.
pub const DISK_MAGIC: [u8; 8] = *b"MINOSJB2";

/// The input disk layout version this code reads and writes.
pub const DISK_VERSION: u32 = 2;

/// Length of the disk header: the magic, the `u32` version, the `u32` section count, and the 32-byte per-launch
/// output token.
pub const DISK_HEADER_BYTES: usize = 48;

/// Length of one section table entry: the `u32` kind, the `u64` offset, the `u64` length, and the section's
/// SHA-256.
pub const SECTION_ENTRY_BYTES: usize = 52;

/// Every section starts on a boundary of this many bytes, and the disk is a whole number of them.
pub const SECTOR_BYTES: u64 = 512;

/// Most bytes in the BASE section: 80 MiB, above the source snapshot's largest tarball, which is 64 MiB of
/// content plus at most 1 KiB of header and padding for each of 10,000 files, plus the two end blocks (a test in
/// `minos-source` checks the bound against the snapshot caps).
pub const MAX_BASE_TAR_BYTES: u64 = 83_886_080;

/// Most bytes in the `JUDGE_DATA` section: 96 MiB. Judge data is pushed by `minos judge push` in Phase 7, which
/// must refuse larger data.
///
/// 96 MiB keeps the judge role within [`MAX_GUEST_INPUT_BYTES`] with margin. Phase 7's `minos judge push` refuses
/// larger judge data rather than truncating it. A larger cap needs a streaming unpack from the input disk instead
/// of a whole-section read, which plan 05-13 records in CLAUDE.md Open questions with Phase 7 as its owner. This is
/// the smaller fix, and it weakens no invariant: [`read_input_disk`] checks the cap from the table before it reads
/// any section byte, and PID 1 drops each input buffer once it is unpacked (plan 05-10), so the stages run with
/// the whole reserve.
pub const MAX_JUDGE_DATA_BYTES: u64 = 100_663_296;

/// Most bytes in the `CANDIDATE_OUT` section: the candidate's whole output stream, as the judge receives it.
pub const MAX_CANDIDATE_OUT_BYTES: u64 = MAX_OUTPUT_STREAM_BYTES;

/// Most sections on one disk: one of each kind.
const MAX_SECTIONS: u32 = 5;

/// Most bytes PID 1 holds in memory for one role's input sections: 128 MiB, half the guest reserve.
///
/// PID 1 reads every section its role needs whole before unpacking (05-CONTEXT D-07 keeps the decoders
/// byte-oriented), and that memory comes out of the guest reserve (05-CONTEXT D-05: the VM's memory is the
/// reserve, plus the tmpfs scratch cap, plus the candidate allowance), which also holds the kernel, PID 1, and the
/// page cache. Half of [`GUEST_RESERVE_MIB`] is the bound, and the const assertions below hold every role's
/// section caps to it, so a change to a cap or to the reserve that breaks the bound fails to compile.
pub const MAX_GUEST_INPUT_BYTES: u64 = 134_217_728;

const _: () = assert!(
    DISK_HEADER_BYTES == DISK_MAGIC.len() + 4 + 4 + TOKEN_BYTES,
    "the header is the magic, the version, the count, and the token"
);
const _: () = assert!(
    SECTION_ENTRY_BYTES == 4 + 8 + 8 + 32,
    "an entry is the kind, the offset, the length, and the digest"
);
const _: () = assert!(
    usize::BITS <= u64::BITS,
    "a usize widens to a u64 without loss"
);

/// `n` as a `u64`. Lossless: a `usize` is at most 64 bits (asserted above).
const fn widen(n: usize) -> u64 {
    n as u64
}

// Ties the bound to the reserve: a smaller reserve must lower the bound, never leave it stale.
const _: () = assert!(
    MAX_GUEST_INPUT_BYTES == GUEST_RESERVE_MIB as u64 * 1024 * 1024 / 2,
    "PID 1's input bound is half the guest reserve"
);
// A candidate holds the job, the base tarball, and the diff before it unpacks them.
const _: () = assert!(
    widen(MAX_GUEST_JOB_BYTES) + MAX_BASE_TAR_BYTES + widen(MAX_DIFF_BYTES)
        <= MAX_GUEST_INPUT_BYTES,
    "the candidate role's input sections fit half the guest reserve"
);
// A judge holds the job, its private data, and the candidate's output stream before it unpacks them.
const _: () = assert!(
    widen(MAX_GUEST_JOB_BYTES) + MAX_JUDGE_DATA_BYTES + MAX_CANDIDATE_OUT_BYTES
        <= MAX_GUEST_INPUT_BYTES,
    "the judge role's input sections fit half the guest reserve"
);

/// A sector of zeros, for padding.
const ZEROS: [u8; 512] = [0; 512];

/// What one input disk section holds. The discriminant is the kind's code in the section table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectionKind {
    /// The guest job JSON.
    Job = 1,
    /// The base tarball a candidate unpacks.
    Base = 2,
    /// The canonical cumulative diff a candidate applies.
    Diff = 3,
    /// The judge's private data.
    JudgeData = 4,
    /// The candidate's output stream, which the judge scores.
    CandidateOut = 5,
}

impl SectionKind {
    /// Most bytes a section of this kind may hold.
    #[must_use]
    pub const fn cap(self) -> u64 {
        match self {
            Self::Job => widen(MAX_GUEST_JOB_BYTES),
            Self::Base => MAX_BASE_TAR_BYTES,
            Self::Diff => widen(MAX_DIFF_BYTES),
            Self::JudgeData => MAX_JUDGE_DATA_BYTES,
            Self::CandidateOut => MAX_CANDIDATE_OUT_BYTES,
        }
    }

    /// The kind's code in the section table.
    #[must_use]
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// The kind with table code `code`, or `None` for an unknown code.
    #[must_use]
    pub const fn from_u32(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::Job),
            2 => Some(Self::Base),
            3 => Some(Self::Diff),
            4 => Some(Self::JudgeData),
            5 => Some(Self::CandidateOut),
            _ => None,
        }
    }
}

/// Why an input disk could not be written or read.
///
/// The backend writes the disk from blob bytes it has already checked, so every `DiskError` is a host bug or a
/// corrupted disk: an infrastructure failure, never a candidate fault (05-RESEARCH Pattern 4). Payloads are
/// numbers only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum DiskError {
    /// The disk does not start with [`DISK_MAGIC`].
    #[error("the input disk does not start with the disk magic")]
    Magic,
    /// The disk's version is not [`DISK_VERSION`].
    #[error("the input disk version is {found}, not {DISK_VERSION}")]
    Version {
        /// The version found.
        found: u32,
    },
    /// The section count is 0 or more than one section of each kind.
    #[error("the input disk has {count} sections; it holds 1 to {MAX_SECTIONS}")]
    SectionCount {
        /// The count found.
        count: u32,
    },
    /// A section kind this code does not know.
    #[error("the input disk has a section of unknown kind {kind}")]
    UnknownKind {
        /// The kind's code.
        kind: u32,
    },
    /// Two sections of one kind.
    #[error("the input disk has two sections of one kind")]
    DuplicateKind,
    /// The table is not in ascending kind order, a section does not start where the canonical layout puts it
    /// (on the first sector boundary after the table or the section before), or padding is not zero.
    #[error("the input disk layout is not the canonical layout")]
    Layout,
    /// A section is longer than its kind's cap.
    #[error("the input disk section of kind {kind} is {length} bytes; the cap is {cap}")]
    TooLarge {
        /// The kind's code.
        kind: u32,
        /// The section's length.
        length: u64,
        /// The kind's cap.
        cap: u64,
    },
    /// A section's bytes do not hash to its table digest.
    #[error("the input disk section of kind {kind} does not match its digest")]
    Digest {
        /// The kind's code.
        kind: u32,
    },
    /// A section the disk or the job's role needs is missing.
    #[error("the input disk has no section of kind {kind}")]
    MissingSection {
        /// The kind's code.
        kind: u32,
    },
    /// A section the job's role must not see is present.
    #[error("the input disk has a section of kind {kind} that the job's role must not see")]
    UnexpectedSection {
        /// The kind's code.
        kind: u32,
    },
    /// The BASE section is not the job's base tree.
    #[error("the base section is not the job's base tree")]
    BaseMismatch,
    /// The DIFF section is not the job's diff blob.
    #[error("the diff section is not the job's diff")]
    DiffMismatch,
    /// Reading or writing failed, or the disk ended early.
    #[error("input disk I/O failed")]
    Io(#[source] io::Error),
}

/// `n` rounded up to a whole sector, or `None` on overflow.
const fn round_up(n: u64) -> Option<u64> {
    n.checked_next_multiple_of(SECTOR_BYTES)
}

/// Where the first section starts on a disk of `count` sections: the first sector boundary after the table.
fn first_offset(count: usize) -> Option<u64> {
    let table = SECTION_ENTRY_BYTES.checked_mul(count)?;
    round_up(widen(DISK_HEADER_BYTES.checked_add(table)?))
}

/// Writes `length` zero bytes.
fn write_zeros<W: Write>(out: &mut W, mut length: u64) -> Result<(), DiskError> {
    while length > 0 {
        let step = length.min(widen(ZEROS.len()));
        let chunk = usize::try_from(step)
            .ok()
            .and_then(|step| ZEROS.get(..step))
            .ok_or(DiskError::Layout)?;
        out.write_all(chunk).map_err(DiskError::Io)?;
        length -= step;
    }
    Ok(())
}

/// Writes the input disk of `sections` under `token` to `out`, and returns its length in bytes.
///
/// The sections are written in ascending kind order, whatever order they are given in, each on the first sector
/// boundary after the table or the section before, with zero padding between them and after the last to a whole
/// sector. Every check runs before the first byte is written, so a refused disk writes nothing. Core does no file
/// I/O: the caller owns the writer (05-CONTEXT D-15: the backend writes the disk into the job directory, and the
/// disk is never stored as a blob).
///
/// # Errors
///
/// [`DiskError::SectionCount`] for no sections, [`DiskError::DuplicateKind`], [`DiskError::TooLarge`] for a
/// section over its kind's cap, [`DiskError::MissingSection`] without a JOB section, and [`DiskError::Io`] when
/// writing fails.
pub fn write_input_disk<W: Write>(
    out: &mut W,
    token: &ChunkToken,
    sections: &[(SectionKind, &[u8])],
) -> Result<u64, DiskError> {
    if sections.is_empty() {
        return Err(DiskError::SectionCount { count: 0 });
    }
    let mut sorted: BTreeMap<SectionKind, &[u8]> = BTreeMap::new();
    for &(kind, bytes) in sections {
        let length = widen(bytes.len());
        if length > kind.cap() {
            return Err(DiskError::TooLarge {
                kind: kind.code(),
                length,
                cap: kind.cap(),
            });
        }
        if sorted.insert(kind, bytes).is_some() {
            return Err(DiskError::DuplicateKind);
        }
    }
    if !sorted.contains_key(&SectionKind::Job) {
        return Err(DiskError::MissingSection {
            kind: SectionKind::Job.code(),
        });
    }
    let count = u32::try_from(sorted.len()).map_err(|_| DiskError::Layout)?;
    let mut head = Vec::with_capacity(DISK_HEADER_BYTES + SECTION_ENTRY_BYTES * sorted.len());
    head.extend_from_slice(&DISK_MAGIC);
    head.extend_from_slice(&DISK_VERSION.to_be_bytes());
    head.extend_from_slice(&count.to_be_bytes());
    head.extend_from_slice(token.as_bytes());
    let mut offsets = Vec::with_capacity(sorted.len());
    let mut next = first_offset(sorted.len()).ok_or(DiskError::Layout)?;
    for (&kind, bytes) in &sorted {
        let length = widen(bytes.len());
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        head.extend_from_slice(&kind.code().to_be_bytes());
        head.extend_from_slice(&next.to_be_bytes());
        head.extend_from_slice(&length.to_be_bytes());
        head.extend_from_slice(&digest);
        offsets.push(next);
        next = next
            .checked_add(length)
            .and_then(round_up)
            .ok_or(DiskError::Layout)?;
    }
    out.write_all(&head).map_err(DiskError::Io)?;
    let mut position = widen(head.len());
    for (bytes, offset) in sorted.values().zip(offsets) {
        let gap = offset.checked_sub(position).ok_or(DiskError::Layout)?;
        write_zeros(out, gap)?;
        out.write_all(bytes).map_err(DiskError::Io)?;
        position = offset
            .checked_add(widen(bytes.len()))
            .ok_or(DiskError::Layout)?;
    }
    let total = round_up(position).ok_or(DiskError::Layout)?;
    write_zeros(out, total - position)?;
    Ok(total)
}

/// Fixed-width big-endian fields read off the front of a byte slice.
struct Fields<'a>(&'a [u8]);

impl Fields<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DiskError> {
        let (head, rest) = self.0.split_first_chunk::<N>().ok_or(DiskError::Layout)?;
        self.0 = rest;
        Ok(*head)
    }

    fn u32(&mut self) -> Result<u32, DiskError> {
        Ok(u32::from_be_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, DiskError> {
        Ok(u64::from_be_bytes(self.take()?))
    }
}

/// One validated section table entry.
struct Entry {
    kind: SectionKind,
    offset: u64,
    length: u64,
    digest: [u8; 32],
}

/// The validated section table of a disk of `count` sections, read from `table`.
///
/// Every entry is checked here, before any section byte is read: its kind is known and appears once, the kinds
/// ascend, its length is within its kind's cap, and its offset is the canonical one. The table must have a JOB
/// entry.
fn parse_table(count: usize, table: &[u8]) -> Result<Vec<Entry>, DiskError> {
    let mut entries: Vec<Entry> = Vec::with_capacity(count);
    let mut seen = BTreeSet::new();
    let mut expected = first_offset(count).ok_or(DiskError::Layout)?;
    for raw in table.chunks_exact(SECTION_ENTRY_BYTES) {
        let mut fields = Fields(raw);
        let code = fields.u32()?;
        let offset = fields.u64()?;
        let length = fields.u64()?;
        let digest = fields.take::<32>()?;
        let kind = SectionKind::from_u32(code).ok_or(DiskError::UnknownKind { kind: code })?;
        if !seen.insert(kind) {
            return Err(DiskError::DuplicateKind);
        }
        if entries.last().is_some_and(|previous| previous.kind > kind) {
            return Err(DiskError::Layout);
        }
        if length > kind.cap() {
            return Err(DiskError::TooLarge {
                kind: code,
                length,
                cap: kind.cap(),
            });
        }
        if offset != expected {
            return Err(DiskError::Layout);
        }
        expected = offset
            .checked_add(length)
            .and_then(round_up)
            .ok_or(DiskError::Layout)?;
        entries.push(Entry {
            kind,
            offset,
            length,
            digest,
        });
    }
    if !seen.contains(&SectionKind::Job) {
        return Err(DiskError::MissingSection {
            kind: SectionKind::Job.code(),
        });
    }
    Ok(entries)
}

/// The error for a disk that ended early.
fn truncated() -> DiskError {
    DiskError::Io(io::Error::from(io::ErrorKind::UnexpectedEof))
}

/// Reads an input disk from `input`: the header and the table first, then each section in order.
///
/// The header and the whole table are read and every entry is validated (known, unique, ascending, within its
/// kind's cap, at its canonical offset) before any section byte is read. Each section is then read whole after
/// checking that the padding before it is zero, and hashed against its table digest. The reader never reads past
/// the end of the last section, so the trailing padding is left unread.
///
/// # Errors
///
/// [`DiskError::Magic`], [`DiskError::Version`], [`DiskError::SectionCount`], [`DiskError::UnknownKind`],
/// [`DiskError::DuplicateKind`], [`DiskError::Layout`], [`DiskError::TooLarge`], [`DiskError::MissingSection`]
/// without a JOB section, [`DiskError::Digest`], and [`DiskError::Io`] for a failed read or a disk that ends
/// early.
pub fn read_input_disk<R: Read>(input: &mut R) -> Result<InputSections, DiskError> {
    let mut header = [0_u8; DISK_HEADER_BYTES];
    input.read_exact(&mut header).map_err(DiskError::Io)?;
    let mut fields = Fields(&header);
    if fields.take::<8>()? != DISK_MAGIC {
        return Err(DiskError::Magic);
    }
    let version = fields.u32()?;
    if version != DISK_VERSION {
        return Err(DiskError::Version { found: version });
    }
    let count = fields.u32()?;
    if !(1..=MAX_SECTIONS).contains(&count) {
        return Err(DiskError::SectionCount { count });
    }
    let token = ChunkToken::from_bytes(fields.take::<TOKEN_BYTES>()?);
    // count <= MAX_SECTIONS, so neither conversion nor product can overflow; checked anyway.
    let count = usize::try_from(count).map_err(|_| DiskError::Layout)?;
    let table_bytes = SECTION_ENTRY_BYTES
        .checked_mul(count)
        .ok_or(DiskError::Layout)?;
    let mut table = vec![0_u8; table_bytes];
    input.read_exact(&mut table).map_err(DiskError::Io)?;
    let entries = parse_table(count, &table)?;

    let mut position = widen(DISK_HEADER_BYTES + table_bytes);
    let mut padding = ZEROS;
    let mut sections = BTreeMap::new();
    for entry in entries {
        // A canonical offset is the next sector boundary, so the gap is under one sector.
        let gap = entry
            .offset
            .checked_sub(position)
            .and_then(|gap| usize::try_from(gap).ok())
            .ok_or(DiskError::Layout)?;
        let pad = padding.get_mut(..gap).ok_or(DiskError::Layout)?;
        input.read_exact(pad).map_err(DiskError::Io)?;
        if pad.iter().any(|byte| *byte != 0) {
            return Err(DiskError::Layout);
        }
        let length = usize::try_from(entry.length).map_err(|_| DiskError::Layout)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| DiskError::Io(io::Error::from(io::ErrorKind::OutOfMemory)))?;
        Read::take(&mut *input, entry.length)
            .read_to_end(&mut bytes)
            .map_err(DiskError::Io)?;
        if bytes.len() != length {
            return Err(truncated());
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if digest != entry.digest {
            return Err(DiskError::Digest {
                kind: entry.kind.code(),
            });
        }
        sections.insert(entry.kind, bytes);
        position = entry
            .offset
            .checked_add(entry.length)
            .ok_or(DiskError::Layout)?;
    }
    Ok(InputSections { token, sections })
}

/// The sections of one input disk, read and checked against the table, and the per-launch output token.
///
/// `Debug` shows the section kinds and lengths only, never their bytes, and the token stays redacted.
pub struct InputSections {
    token: ChunkToken,
    sections: BTreeMap<SectionKind, Vec<u8>>,
}

impl InputSections {
    /// The per-launch output token from the disk header. It travels nowhere else: not in the job JSON, not in any
    /// blob.
    #[must_use]
    pub const fn token(&self) -> &ChunkToken {
        &self.token
    }

    /// Removes and returns the bytes of the section of `kind`, if the disk has one. PID 1 takes each section once
    /// and drops it after unpacking, so its memory returns to the guest reserve before the stages run.
    pub fn take(&mut self, kind: SectionKind) -> Option<Vec<u8>> {
        self.sections.remove(&kind)
    }

    /// Whether the disk still holds a section of `kind`.
    #[must_use]
    pub fn contains(&self, kind: SectionKind) -> bool {
        self.sections.contains_key(&kind)
    }

    /// Checks that the sections are exactly what `job`'s role needs, before any is taken.
    ///
    /// A candidate needs BASE, hashing to the job's base tree, and DIFF exactly when the job has a diff, hashing
    /// to the job's diff blob; it must not see `JUDGE_DATA` or `CANDIDATE_OUT`. A judge needs `JUDGE_DATA` and
    /// `CANDIDATE_OUT` and must not see BASE or DIFF; those two are bound by their table digests only, since the
    /// job names no ID for them.
    ///
    /// # Errors
    ///
    /// [`DiskError::UnexpectedSection`], [`DiskError::MissingSection`], [`DiskError::BaseMismatch`], or
    /// [`DiskError::DiffMismatch`] for the first failed check.
    pub fn check_role(&self, job: &GuestJob) -> Result<(), DiskError> {
        let (forbidden, needed) = match job.role() {
            GuestRole::Candidate => (
                [SectionKind::JudgeData, SectionKind::CandidateOut],
                [SectionKind::Base].as_slice(),
            ),
            GuestRole::Judge => (
                [SectionKind::Base, SectionKind::Diff],
                [SectionKind::JudgeData, SectionKind::CandidateOut].as_slice(),
            ),
        };
        if let Some(kind) = forbidden.into_iter().find(|kind| self.contains(*kind)) {
            return Err(DiskError::UnexpectedSection { kind: kind.code() });
        }
        if let Some(kind) = needed.iter().find(|kind| !self.contains(**kind)) {
            return Err(DiskError::MissingSection { kind: kind.code() });
        }
        if job.role() == GuestRole::Judge {
            return Ok(());
        }
        let base = self
            .sections
            .get(&SectionKind::Base)
            .ok_or(DiskError::MissingSection {
                kind: SectionKind::Base.code(),
            })?;
        if job.base() != Some(TreeId::of(base)) {
            return Err(DiskError::BaseMismatch);
        }
        match (job.diff(), self.sections.get(&SectionKind::Diff)) {
            (Some(expected), Some(diff)) if BlobId::of(diff) == expected => Ok(()),
            (Some(_), Some(_)) => Err(DiskError::DiffMismatch),
            (Some(_), None) => Err(DiskError::MissingSection {
                kind: SectionKind::Diff.code(),
            }),
            (None, Some(_)) => Err(DiskError::UnexpectedSection {
                kind: SectionKind::Diff.code(),
            }),
            (None, None) => Ok(()),
        }
    }
}

/// The section lengths of an [`InputSections`], by kind.
struct SectionLengths<'a>(&'a BTreeMap<SectionKind, Vec<u8>>);

impl fmt::Debug for SectionLengths<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.0.iter().map(|(kind, bytes)| (kind, bytes.len())))
            .finish()
    }
}

impl fmt::Debug for InputSections {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputSections")
            .field("token", &self.token)
            .field("sections", &SectionLengths(&self.sections))
            .finish()
    }
}
