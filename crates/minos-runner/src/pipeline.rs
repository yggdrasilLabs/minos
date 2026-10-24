//! The runner pipeline: one `JobRequest` in, one `JobResult` out (LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04).
//!
//! [`Runner::run`] runs one trial in a fixed order:
//!
//! 1. A request under another judge version than the runner holds is refused.
//! 2. A candidate's diff blob is read back under the diff cap, parsed against the manifest's writable set, and
//!    accepted only through [`CumulativeDiff::recorded`]: it must hash to the request's experiment. A mismatch is
//!    the loop's bug, never the candidate's fault, so nothing launches.
//! 3. The candidate's guest job is stored, and its input disk names the job, the base tree, and the diff. No judge
//!    data: an input disk has no part for it.
//! 4. The Candidate spec is built, sealed, cleared by `preflight` against the backend's capabilities, launched, and
//!    waited for.
//! 5. The candidate's usage is [`host_usage`] of the backend's measured elapsed time and the plan's machine,
//!    and nothing else: no number the guest wrote is read (LOOP-03).
//! 6. What the host observed is read in order (the end, then the output stream, then PID 1's status) into a
//!    [`Seen`], and the build and run logs are stored as capped tail blobs.
//! 7. [`candidate_verdict`] decides: a decided outcome is the result, an infrastructure or configuration failure is
//!    the error, and only a pass continues.
//! 8. The candidate's `out/` files, and only those, are re-packed into the `candidate_out` blob by
//!    `store_judge_input`. PID 1's status, the stage logs, and anything else the candidate wrote, a forged
//!    scorecard included, never reach the judge (LOOP-02).
//! 9. The judge's guest job is stored, and the Judge spec is built with `InputDisk::judge(job, candidate_out)`. The
//!    judge data is attached only here, through `SandboxSpec<Judge>::secrets`, the one method that can attach it.
//!    The Judge VM is launched under its own launch key, job directory, and input disk, and shares no process with
//!    the candidate. Its usage is measured the same way.
//! 10. The judge's end is observed the same way, its log tail stored, and [`judge_verdict`] decides. A judge that
//!     passed hands its scorecard to [`judged_outcome`], which checks the gates first, so a failed gate is
//!     `Rejected(GateFailed)` and the objectives are never read (JUDGE-03).
//! 11. The result carries the outcome, both usages, the stored log tails by stage, and the backend's provenance for
//!     the root image, with the backend and architecture (JUDGE-04).
//!
//! [`Runner::check`] runs steps 1 to 4 for the baseline without launching, so a configuration that can never work
//! stops the run before generation 0. [`Runner::new`] refuses, earlier still, a `[environment] scratch_mib` that
//! the judge data plus a full candidate output could fill, because the judge VM unpacks both into one scratch and
//! PID 1 reports either filling it as `ScratchFull`, a configuration error: with the check passed, that error is
//! never a candidate's lever (review CR-01).
//!
//! This module is the only place in production code where a sandbox's output leaves [`Untrusted`]: through
//! `take_status`, `into_scorecard`, and the two sink exits, `store_judge_input` and `store_tail`, which hand capped
//! bytes only to the store and return only a [`BlobId`] (LOOP-01, 07-CONTEXT D-17). Every blame decision is the
//! pure table in [`crate::map`].
//!
//! [`host_usage`]: crate::usage::host_usage
//! [`Seen`]: crate::map::Seen
//! [`candidate_verdict`]: crate::map::candidate_verdict
//! [`judge_verdict`]: crate::map::judge_verdict
//! [`judged_outcome`]: crate::map::judged_outcome

use std::collections::BTreeMap;
use std::str::FromStr;

use minos_core::diff::Diff;
use minos_core::diff::cumulative::CumulativeDiff;
use minos_core::guest::GuestJob;
use minos_core::id::{BlobId, ImageDigest};
use minos_core::job::{JobRequest, JobResult};
use minos_core::limits::{Limits, MAX_STAGE_LOG_BYTES, judge_scratch_needed_bytes};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::path::RelPath;
use minos_core::usage::Usage;
use minos_sandbox::{
    BUILD_LOG_PATH, Backend, BlobSink, BlobSourceError, Candidate, Ended, Finished, GuestOutput,
    InputDisk, JUDGE_LOG_PATH, Judge, JudgeData, LaunchError, LaunchPlan, Machine, RUN_LOG_PATH,
    Refusal, Role, RunningVm, SandboxSpec, SinkError, Untrusted, preflight,
};
use minos_store::{Blobs, StoreError};

use crate::error::{ConfigCause, InfraCause, RunnerError};
use crate::map::{Seen, Verdict, candidate_verdict, judge_verdict, judged_outcome};
use crate::usage::{RUNNER_VCPUS, host_usage};

/// The empty ustar archive, two 512-byte zero blocks: what `store_judge_input` makes of an output with no
/// artifact. [`Runner::check`] names its blob as the judge's placeholder `candidate_out`, because no candidate has
/// run yet.
const EMPTY_USTAR: [u8; 1024] = [0; 1024];

/// The candidate's stage logs, stored as tail blobs under their stage.
const CANDIDATE_LOGS: [(Stage, &str); 2] =
    [(Stage::Build, BUILD_LOG_PATH), (Stage::Run, RUN_LOG_PATH)];

/// The judge's stage log, stored as a tail blob under its stage.
const JUDGE_LOGS: [(Stage, &str); 1] = [(Stage::Judge, JUDGE_LOG_PATH)];

/// One MiB, in bytes: the unit of `[environment] scratch_mib`.
const MIB: u64 = 1024 * 1024;

/// The size of the judge data a runner's judge VM unpacks: the content bytes and the file count `minos.lock`
/// records for the manifest's `[judge] version` label (`LockedJudge::bytes`, `LockedJudge::file_count`).
///
/// [`Runner::new`] checks it against `[environment] scratch_mib` with [`judge_scratch_needed_bytes`], so a
/// judge-side `ScratchFull` is never something a candidate's output can cause (review CR-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeExtent {
    bytes: u64,
    files: u64,
}

impl JudgeExtent {
    /// Judge data of `bytes` content bytes in `files` files.
    #[must_use]
    pub const fn new(bytes: u64, files: u64) -> Self {
        Self { bytes, files }
    }

    /// The summed length of the judge files, headers and padding excluded.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// How many files the judge data holds.
    #[must_use]
    pub const fn files(&self) -> u64 {
        self.files
    }
}

/// Runs one trial of one experiment: the candidate VM, then, if the candidate passed, the judge VM.
///
/// Generic over its backend `B` and its blob store `S`, and never a trait object or a backend enum: each binary
/// picks both concretely (the CLI builds `Runner<KrunBackend, FileStore>`), and the tests run the same code over a
/// scripted backend and the memory store. See the [module docs](self) for the order of a run.
///
/// A runner holds one manifest, one root image, and one judge version with its data, so every trial of a run
/// launches the same machine, the same image, and the same judge.
pub struct Runner<B: Backend, S: Blobs> {
    backend: B,
    store: S,
    manifest: Manifest,
    root: ImageDigest,
    judge_data: JudgeData,
    judge: JudgeVersion,
    machine: Machine,
    arch: Arch,
}

impl<B: Backend, S: Blobs> Runner<B, S> {
    /// A runner that launches on `backend`, stores through `store`, builds every VM from `manifest` with the root
    /// image `root`, and scores with the judge version `judge`, whose data is stored as `judge_data` and measures
    /// `judge_extent`.
    ///
    /// Every VM gets [`RUNNER_VCPUS`] vCPUs and the manifest's `[environment] memory_mib`, on the manifest's
    /// architecture. The manifest's `[environment] scratch_mib` must hold the judge data together with the largest
    /// candidate output the runner can hand the judge, so that a judge VM whose scratch fills is a configuration
    /// error and never a candidate's doing (review CR-01).
    ///
    /// # Errors
    ///
    /// [`ConfigCause::Machine`] when the manifest's memory does not fit a sandbox machine, and
    /// [`ConfigCause::JudgeScratch`] when its scratch is below
    /// [`judge_scratch_needed_bytes`] for `judge_extent`.
    pub fn new(
        backend: B,
        store: S,
        manifest: Manifest,
        root: ImageDigest,
        judge_data: JudgeData,
        judge: JudgeVersion,
        judge_extent: JudgeExtent,
    ) -> Result<Self, RunnerError> {
        let environment = manifest.environment();
        let machine = Machine::new(RUNNER_VCPUS, environment.memory_mib())
            .map_err(|_| ConfigCause::Machine)?;
        let scratch = u64::from(environment.scratch_mib()).saturating_mul(MIB);
        let needed = judge_scratch_needed_bytes(judge_extent.bytes(), judge_extent.files());
        if scratch < needed {
            return Err(ConfigCause::JudgeScratch { scratch, needed }.into());
        }
        let arch = environment.arch();
        Ok(Self {
            backend,
            store,
            manifest,
            root,
            judge_data,
            judge,
            machine,
            arch,
        })
    }

    /// The blob store the runner reads diffs from and writes guest jobs, judge inputs, and log tails to.
    #[must_use]
    pub const fn store(&self) -> &S {
        &self.store
    }

    /// The backend the runner launches on.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Runs `request`: the candidate VM, then, if the candidate passed, the judge VM, and returns how the trial
    /// ended. A candidate fault is an `Outcome` inside the result, never an error.
    ///
    /// # Errors
    ///
    /// [`RunnerError::Infra`], which the loop retries, when a launch or a wait fails, a store read or write fails
    /// with a retryable error, the store returns another digest than the bytes' own, the backend cannot read a blob,
    /// a guest job does not encode, PID 1 reports a failed setup step other than a full scratch, or the host
    /// observed an end this runner has no mapping for.
    ///
    /// [`RunnerError::Config`], which stops the run, when the request names another judge version, its diff blob
    /// does not parse or does not hash to its experiment, a spec cannot be built, the host refuses a plan, a blob
    /// is missing, over its cap, tampered, or unsafe, the scratch space fills, or the result cannot be built.
    pub fn run(&self, request: &JobRequest) -> Result<JobResult, RunnerError> {
        // 1. A request under another judge version would launch a judge the runner does not hold.
        self.require_judge(request)?;
        // 2. The diff must be the request's experiment, cumulative from the run's base, before anything launches.
        self.verify_diff(request)?;
        // 3. Store the candidate's guest job; its input names the job, the base tree, and the diff, never judge data.
        let job = self.put(&encode(&GuestJob::candidate(&self.manifest, request))?)?;
        // 4. Build, seal, clear, launch, and wait for the Candidate VM.
        let finished = self.launch(self.candidate_plan(request, job)?)?;
        // 5. Usage is the host's measured time on the plan's machine, and nothing the guest wrote (LOOP-03).
        let candidate_usage = host_usage(finished.elapsed(), &self.machine);
        // 6. Observe the end, the stream, and the status in order, and store the build and run log tails.
        let mut logs = BTreeMap::new();
        let (seen, mut output) = observe(finished);
        if let Some(output) = output.as_mut() {
            self.store_logs(output, &CANDIDATE_LOGS, &mut logs)?;
        }
        // 7. The blame table decides, with the apply status checked against the request (IN-03); only a passing
        //    candidate reaches the judge.
        match candidate_verdict(&seen, request.diff().is_some()) {
            Verdict::Pass => {}
            Verdict::Done(outcome) => {
                return self.result(request, outcome, candidate_usage, None, logs);
            }
            Verdict::Infra(cause) => return Err(cause.into()),
            Verdict::Config(cause) => return Err(cause.into()),
        }
        // A pass is decided only from a status taken from a read output, so the output is always here; fail
        // closed as infrastructure if it ever is not.
        let Some(output) = output else {
            return Err(InfraCause::Unmapped.into());
        };
        // 8. Re-pack only the candidate's out/ files as the judge's input (LOOP-02).
        let candidate_out = match output.store_judge_input(&self.sink()) {
            Ok(blob) => blob,
            Err(SinkError::Repack) => {
                let outcome = Outcome::Rejected(Reason::OutputInvalid);
                return self.result(request, outcome, candidate_usage, None, logs);
            }
            Err(error) => return Err(sink_error(&error)),
        };
        // 9. Store the judge's guest job; the judge data is attached only through `secrets` on the Judge spec.
        let job = self.put(&encode(&GuestJob::judge(&self.manifest, request))?)?;
        let finished = self.launch(self.judge_plan(request, job, candidate_out)?)?;
        let judge_usage = host_usage(finished.elapsed(), &self.machine);
        // 10. Observe the judge the same way, store its log tail, and score it gates first (JUDGE-03).
        let (seen, mut output) = observe(finished);
        if let Some(output) = output.as_mut() {
            self.store_logs(output, &JUDGE_LOGS, &mut logs)?;
        }
        let outcome = match judge_verdict(&seen) {
            Verdict::Pass => {
                let Some(output) = output else {
                    return Err(InfraCause::Unmapped.into());
                };
                judged_outcome(
                    output.into_scorecard(&Limits::standard()),
                    self.manifest.judge(),
                )
            }
            Verdict::Done(outcome) => outcome,
            Verdict::Infra(cause) => return Err(cause.into()),
            Verdict::Config(cause) => return Err(cause.into()),
        };
        // 11. The result names the trial, both usages, the log tails, and where the VMs ran (JUDGE-04).
        self.result(request, outcome, candidate_usage, Some(judge_usage), logs)
    }

    /// Checks that `request` could run, without launching anything: what the loop calls on the baseline's first
    /// trial before generation 0 (CLAUDE.md "minos-sandbox"; 07-CONTEXT D-06).
    ///
    /// It requires the root image, the base tarball, the judge data, and (for a candidate) the diff in the store,
    /// then builds, seals, and preflights the Candidate and Judge plans against the backend's capabilities. The
    /// Judge plan names the empty ustar archive as a placeholder `candidate_out`, because no candidate has run. The
    /// guest jobs are encoded and hashed but not stored, so a check writes nothing.
    ///
    /// # Errors
    ///
    /// [`RunnerError::Config`] when the request names another judge version, a blob is missing or unsafe, a spec
    /// cannot be built (such as stage budgets over the one-hour launch ceiling), or the host refuses a plan.
    /// [`RunnerError::Infra`] when the store cannot be inspected or a guest job does not encode.
    pub fn check(&self, request: &JobRequest) -> Result<(), RunnerError> {
        self.require_judge(request)?;
        let mut needed = vec![
            self.root.to_blob_id(),
            request.base().to_blob_id(),
            *self.judge_data.blob(),
        ];
        needed.extend(request.diff());
        for blob in &needed {
            if !self
                .store
                .contains(blob)
                .map_err(|error| store_error(&error))?
            {
                return Err(ConfigCause::BlobMissing.into());
            }
        }
        let host = self.backend.capabilities();
        let job = BlobId::of(&encode(&GuestJob::candidate(&self.manifest, request))?);
        let _candidate = preflight(&host, self.candidate_plan(request, job)?).map_err(refused)?;
        let job = BlobId::of(&encode(&GuestJob::judge(&self.manifest, request))?);
        let placeholder = BlobId::of(&EMPTY_USTAR);
        let _judge =
            preflight(&host, self.judge_plan(request, job, placeholder)?).map_err(refused)?;
        Ok(())
    }

    /// Refuses a request under another judge version than the runner holds.
    fn require_judge(&self, request: &JobRequest) -> Result<(), RunnerError> {
        if request.judge() == self.judge {
            Ok(())
        } else {
            Err(ConfigCause::JudgeMismatch.into())
        }
    }

    /// For a candidate, reads its diff blob back and requires that it is the canonical diff of the request's
    /// experiment, cumulative from the run's base. A baseline has no diff and passes.
    fn verify_diff(&self, request: &JobRequest) -> Result<(), RunnerError> {
        let Some(blob) = request.diff() else {
            return Ok(());
        };
        let limits = Limits::standard();
        let max = u64::try_from(limits.max_diff_bytes()).unwrap_or(u64::MAX);
        let bytes = self
            .store
            .get(&blob, max)
            .map_err(|error| store_error(&error))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| ConfigCause::DiffParse)?;
        let diff = Diff::parse(text, self.manifest.writable(), &limits)
            .map_err(|_| ConfigCause::DiffParse)?;
        // The guest receives these exact bytes, so they must be the canonical form the experiment ID hashes.
        if BlobId::of(diff.as_bytes()) != blob {
            return Err(ConfigCause::DiffMismatch.into());
        }
        CumulativeDiff::recorded(request.base(), diff, &request.exp())
            .map_err(|_| ConfigCause::DiffMismatch)?;
        Ok(())
    }

    /// The sealed Candidate plan of `request` with the guest job blob `job`. No judge data: a Candidate spec has no
    /// method that attaches it.
    fn candidate_plan(
        &self,
        request: &JobRequest,
        job: BlobId,
    ) -> Result<LaunchPlan<Candidate>, RunnerError> {
        let input = InputDisk::candidate(job, request.base(), request.diff());
        let spec =
            SandboxSpec::<Candidate>::new(request, self.root, input, self.machine, self.arch)
                .map_err(|error| ConfigCause::Spec { error })?;
        Ok(spec.seal())
    }

    /// The sealed Judge plan of `request` with the guest job blob `job` and the candidate's re-packed output
    /// `candidate_out`, carrying the judge data through `secrets`, the only way it enters a plan.
    fn judge_plan(
        &self,
        request: &JobRequest,
        job: BlobId,
        candidate_out: BlobId,
    ) -> Result<LaunchPlan<Judge>, RunnerError> {
        let input = InputDisk::judge(job, candidate_out);
        let spec = SandboxSpec::<Judge>::new(request, self.root, input, self.machine, self.arch)
            .map_err(|error| ConfigCause::Spec { error })?;
        Ok(spec.secrets(self.judge_data).seal())
    }

    /// Clears `plan` against the backend's capabilities, launches it, and waits for it to end.
    fn launch<R: Role>(&self, plan: LaunchPlan<R>) -> Result<Finished<R>, RunnerError> {
        let cleared = preflight(&self.backend.capabilities(), plan).map_err(refused)?;
        let running = self
            .backend
            .launch(cleared)
            .map_err(|error| launch_error(&error))?;
        running
            .wait()
            .map_err(|_| RunnerError::from(InfraCause::Wait))
    }

    /// Stores each log in `stages` that `output` holds as a tail blob of at most [`MAX_STAGE_LOG_BYTES`], under its
    /// stage in `logs`. A log the guest did not write is simply absent.
    ///
    /// # Errors
    ///
    /// [`ConfigCause::LogPath`] when a stage log path constant is not a valid [`RelPath`]: a runner defect that
    /// would otherwise drop the log silently, so it fails loud (07-REVIEW IN-04).
    fn store_logs<R: Role>(
        &self,
        output: &mut Untrusted<GuestOutput<R>>,
        stages: &[(Stage, &str)],
        logs: &mut BTreeMap<Stage, BlobId>,
    ) -> Result<(), RunnerError> {
        for (stage, path) in stages {
            // The paths are minos-sandbox constants; the pipeline tests pin that every stage's log is stored, and
            // one that stopped parsing fails the run loudly as well, never by quietly storing no log (IN-04).
            let path = RelPath::from_str(path).map_err(|_| ConfigCause::LogPath)?;
            if let Some(file) = output.take(&path) {
                let blob = file
                    .store_tail(MAX_STAGE_LOG_BYTES, &self.sink())
                    .map_err(|error| sink_error(&error))?;
                logs.insert(*stage, blob);
            }
        }
        Ok(())
    }

    /// Stores `bytes` the runner built itself (a guest job), refusing a store that names them by another digest.
    fn put(&self, bytes: &[u8]) -> Result<BlobId, RunnerError> {
        let stored = self.store.put(bytes).map_err(|error| store_error(&error))?;
        if stored == BlobId::of(bytes) {
            Ok(stored)
        } else {
            Err(InfraCause::DigestMismatch.into())
        }
    }

    /// The store as the sink the two `Untrusted` sink exits hand their capped bytes to.
    const fn sink(&self) -> StoreSink<'_, S> {
        StoreSink(&self.store)
    }

    /// The result of `request`, with the backend's provenance for the root image.
    fn result(
        &self,
        request: &JobRequest,
        outcome: Outcome,
        usage: Usage,
        judge_usage: Option<Usage>,
        logs: BTreeMap<Stage, BlobId>,
    ) -> Result<JobResult, RunnerError> {
        let backend = self.backend.capabilities().backend_info(self.root);
        JobResult::new(request, outcome, usage, judge_usage, backend, logs)
            .map_err(|_| RunnerError::from(ConfigCause::JobResult))
    }
}

/// The runner's adapter from the store's [`Blobs::put`] to the sandbox's [`BlobSink`]: the production
/// implementor the sink exits name (07-04).
struct StoreSink<'a, S>(&'a S);

impl<S: Blobs> BlobSink for StoreSink<'_, S> {
    type Error = StoreError;

    fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError> {
        self.0.put(bytes)
    }
}

/// What the host observed of one launch, read in the blame table's order: the end, then the output stream, then
/// PID 1's status. The output is returned only when the stream was read, with the status already taken.
fn observe<R: Role>(finished: Finished<R>) -> (Seen, Option<Untrusted<GuestOutput<R>>>) {
    let ended = finished.ended();
    if ended != Ended::Exited {
        return (Seen::NotExited(ended), None);
    }
    let mut output = match finished.into_output() {
        Ok(output) => output,
        Err(fault) => return (Seen::Output(fault), None),
    };
    let seen = match output.take_status() {
        Ok(status) => Seen::Guest(status),
        Err(fault) => Seen::Status(fault),
    };
    (seen, Some(output))
}

/// A guest job's JSON. A job that does not encode is infrastructure (07-09).
fn encode(job: &GuestJob) -> Result<Vec<u8>, RunnerError> {
    job.to_json()
        .map_err(|_| RunnerError::from(InfraCause::GuestJob))
}

/// Preflight's refusals as a configuration error: the host refuses every launch of this plan alike.
fn refused(refusals: Vec<Refusal>) -> RunnerError {
    ConfigCause::Preflight {
        refusals: refusals.into_iter().collect(),
    }
    .into()
}

/// A store error's blame: a retryable I/O error is infrastructure; a missing or over-cap blob is configuration;
/// anything else (a tampered blob, an unsafe path, a poisoned lock) is configuration that needs a human.
fn store_error(error: &StoreError) -> RunnerError {
    if error.is_retryable() {
        return InfraCause::Store.into();
    }
    match error {
        StoreError::BlobMissing { .. } | StoreError::BlobTooLarge { .. } => {
            ConfigCause::BlobMissing.into()
        }
        _ => ConfigCause::BlobUnsafe.into(),
    }
}

/// A sink exit's error, other than the candidate's own `Repack` fault, which the caller records as an outcome.
fn sink_error(error: &SinkError<StoreError>) -> RunnerError {
    match error {
        SinkError::DigestMismatch => InfraCause::DigestMismatch.into(),
        SinkError::Store(error) => store_error(error),
        // `Repack` never comes from `store_tail`, and the judge-input caller handles it first. A variant this
        // runner does not know fails closed as infrastructure, never blamed on the candidate.
        _ => InfraCause::Store.into(),
    }
}

/// A launch error's blame: a missing or unsafe blob is configuration, a blob read I/O error and every host fault
/// are infrastructure (07-RESEARCH Pattern 1 step 5).
fn launch_error(error: &LaunchError) -> RunnerError {
    match error {
        LaunchError::Blob(source) => blob_source_error(source),
        _ => InfraCause::Launch.into(),
    }
}

/// A blob source error's blame, as [`store_error`] decides it for the store's own errors.
fn blob_source_error(error: &BlobSourceError) -> RunnerError {
    match error {
        BlobSourceError::Missing { .. }
        | BlobSourceError::TooLarge { .. }
        | BlobSourceError::RootImageMissing { .. } => ConfigCause::BlobMissing.into(),
        BlobSourceError::Tampered { .. } | BlobSourceError::Unsafe { .. } => {
            ConfigCause::BlobUnsafe.into()
        }
        // `Io`, and a variant this runner does not know: infrastructure, retried.
        _ => InfraCause::BlobIo.into(),
    }
}
