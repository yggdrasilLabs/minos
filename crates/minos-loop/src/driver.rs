//! The generation driver: starts or resumes a run under its lock, calibrates generation 0, and drives
//! generations to an absolute target (07-CONTEXT D-05 to D-10, D-12, D-13, D-18, D-19; LOOP-04, LOOP-05).
//!
//! # Record before act
//!
//! Every effect is recorded in the ledger before the driver acts on it, keyed by its content-derived ID: a job's
//! `Spent` records precede its `Judged`, a model call's `Spent` records precede its batch, the batch blob is
//! stored before the batch's first `Proposed`, and a generation's `Selected` follows its last `Judged`. A `Spent`
//! is appended only when its `SpendId` is absent ([`spend_once`]), and a conflicting append stops the driver with
//! [`DriverError::Conflict`] and records nothing more (D-08; 07-RESEARCH Pitfall 1).
//!
//! # The fold is the only state
//!
//! The driver keeps no position. Before every step it reads and folds the ledger and performs exactly the step
//! [`next_step`] returns, so a session resumed after a crash at any point performs the same next step an
//! uninterrupted one would have (D-07, LOOP-05). The one thing it remembers within a session is which
//! generations' batches it has already completed, so it reads a batch blob once per generation rather than once
//! per trial; forgetting it costs a re-read, never a different record.
//!
//! # The target is absolute
//!
//! [`Driver::run_to`] takes the generation to reach, not a number of generations to add, so a resumed session
//! computes the same [`RunEnd::Reached`] as an uninterrupted one. Phase 8 maps `--generations N` onto it.
//!
//! # Calibration
//!
//! [`Driver::calibrate`] performs generation 0 on its own: the `[judge] trials` baseline trials, then the
//! [`baseline_ready`] check, returning the baseline's noise floor and the noise verdict (D-05, D-06). It is
//! idempotent, so Phase 8's `minos check` can call it on a diagnostic run, and [`Driver::run_to`] calls it first,
//! so no model call or candidate VM is paid for on a run whose baseline cannot score (07-RESEARCH Pitfall 12).
//!
//! # Phase 8 interface points
//!
//! The CLI builds the [`RunInputs`], picks the concrete store, provider, job runner, and observer, and calls
//! [`Driver::start`] or [`Driver::resume`], then [`Driver::calibrate`] (`minos check`) or [`Driver::run_to`]
//! (`minos run`). Safe points, spend caps, the circuit breaker, infrastructure retries, and the backend pin arrive
//! with 07-15; until then an infrastructure failure surfaces as [`DriverError::Infra`] with nothing recorded.

use std::collections::BTreeSet;
use std::time::Duration;

use minos_core::experiment::Experiment;
use minos_core::id::{BlobId, ExperimentId, RunId};
use minos_core::job::{JobBudget, JobRequest, JobResult};
use minos_core::ledger::{Event, SpendSource, StopReason};
use minos_core::manifest::{JudgeVersion, Manifest, ProposerSpec};
use minos_core::metric::Metric;
use minos_core::policy::{
    NoiseFloor, NoiseVerdict, TrialRule, baseline_ready, conclude, noise_check, noise_floor,
    reduce_trials,
};
use minos_core::route::{RouteError, SurfaceStats, route};
use minos_core::state::{RunState, fold};
use minos_propose::{ModelProvider, ProposeError, ProposeInput, Proposer};
use minos_store::{Blobs, Ledger, RunLock, StoreError};

use crate::admit::{AdmitStep, admit, load_batch, missing_proposed};
use crate::byte_cap;
use crate::error::DriverError;
use crate::history::{history, notes, parent_tree, recorded_diff};
use crate::inputs::{Resume, RunInputs, check_resume, check_start};
use crate::job::{JobFailure, JobRunner};
use crate::observe::{LoopEvent, Observer};
use crate::record::{append, spend_once};
use crate::stage::{CANDIDATE_TRIALS, Step, next_step};

/// The fixed pause before an infrastructure retry (D-16): long enough for a transient host fault to clear, short
/// against a stage budget; tests pass zero.
pub const RETRY_PAUSE: Duration = Duration::from_secs(5);

/// The cap a recorded manifest blob is read under: the CLI's `MAX_MANIFEST_BYTES`, restated because the loop
/// cannot link the CLI. A manifest the CLI accepted is never larger.
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

/// How a driver session behaves, apart from the run itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverOptions {
    /// The pause before an infrastructure retry; [`RETRY_PAUSE`] in production, zero in tests.
    pub retry_pause: Duration,
}

impl Default for DriverOptions {
    fn default() -> Self {
        Self {
            retry_pause: RETRY_PAUSE,
        }
    }
}

/// The result of generation 0: how many baseline trials were judged, the baseline's noise floor on the objective,
/// and whether the objective's `min_improvement` stands clear of it (D-05, D-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calibration {
    trials: u32,
    floor: Option<NoiseFloor>,
    verdict: NoiseVerdict,
}

impl Calibration {
    /// The number of baseline trials, `[judge] trials`.
    #[must_use]
    pub const fn trials(&self) -> u32 {
        self.trials
    }

    /// The baseline's noise floor on the objective metric, or `None` when no trial reported it.
    #[must_use]
    pub const fn floor(&self) -> Option<NoiseFloor> {
        self.floor
    }

    /// The advisory noise verdict for the objective; `minos check` prints it (D-12).
    #[must_use]
    pub const fn verdict(&self) -> NoiseVerdict {
        self.verdict
    }
}

/// How [`Driver::calibrate`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calibrated {
    /// Every baseline trial is judged and the baseline has a usable score.
    Ready(Calibration),
    /// The session stopped during generation 0 (07-15's safe points).
    Stopped {
        /// Why.
        reason: StopReason,
    },
}

/// How [`Driver::run_to`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    /// The last concluded generation is at least the target.
    Reached {
        /// The last concluded generation.
        generation: u32,
    },
    /// The session recorded or honored a stop.
    Stopped {
        /// The generation at which it stopped.
        generation: u32,
        /// Why.
        reason: StopReason,
    },
}

/// One session of the generation driver over one run, holding the run's lock until it drops.
///
/// Generic over its store, model provider, job runner, and observer, never a trait object. It holds the borrowed
/// inputs, the session options, the run ID, and the set of generations whose batch it completed this session;
/// everything else is re-derived from the ledger before every step.
pub struct Driver<'a, S, P, J, O>
where
    S: Ledger + Blobs + RunLock + 'a,
    P: ModelProvider,
    J: JobRunner,
    O: Observer,
{
    store: &'a S,
    /// Held for the whole session and released on drop (LOOP-06); never read.
    _guard: S::Guard<'a>,
    proposer: &'a Proposer<P>,
    jobs: &'a J,
    observer: O,
    inputs: RunInputs<'a>,
    options: DriverOptions,
    run: RunId,
    completed: BTreeSet<u32>,
}

impl<'a, S, P, J, O> Driver<'a, S, P, J, O>
where
    S: Ledger + Blobs + RunLock + 'a,
    P: ModelProvider,
    J: JobRunner,
    O: Observer,
{
    /// Starts a new run at `started_at` (Unix seconds, read by the caller), or rejoins the run those inputs name
    /// if it was already started.
    ///
    /// In order, recording nothing before every check has passed: the prices check (D-13), the baseline's job
    /// preflight through [`JobRunner::check`], the run lock (LOOP-06), the manifest bytes stored as a blob (its ID
    /// has the manifest ID's digest, so a resume can compare experiment frames, D-19), `RunStarted` with the
    /// lock's root image (D-10), and [`check_resume`] against what the ledger now holds.
    ///
    /// # Errors
    ///
    /// [`DriverError::PricesMissing`] when a proposer's model is unpriced; [`DriverError::Job`] when the baseline
    /// can never run as configured, and [`DriverError::Infra`] when its check could not complete;
    /// [`DriverError::RunLocked`] when another driver holds the run; [`DriverError::Store`] and
    /// [`DriverError::Conflict`] from the store; and every refusal of [`check_resume`] for a run that already
    /// exists under other inputs.
    pub fn start(
        store: &'a S,
        proposer: &'a Proposer<P>,
        jobs: &'a J,
        observer: O,
        inputs: RunInputs<'a>,
        options: DriverOptions,
        started_at: u64,
    ) -> Result<Self, DriverError> {
        // 1. Every proposer model is priced (D-13).
        check_start(&inputs)?;

        // 2. The baseline's plans pass preflight, before anything is recorded.
        check_job(jobs, &baseline_request(&inputs, 0)?)?;

        // 3. The run's lock, held for the session.
        let manifest = inputs.manifest_id();
        let base = inputs.base.id();
        let run = RunId::of(&manifest, &base, started_at);
        let guard = lock(store, &run)?;

        // 4. The manifest bytes, so a later resume can compare experiment frames (D-19).
        store
            .put(inputs.manifest_bytes)
            .map_err(|source| DriverError::Store { run, source })?;

        // 5. RunStarted with the lock's root image (D-10); an equal record already there is a duplicate.
        append(
            store,
            &run,
            Event::RunStarted {
                run,
                manifest,
                base,
                baseline: ExperimentId::baseline(&base),
                started_at,
                root_image: Some(inputs.root_image),
            },
        )?;

        // 6. The ledger agrees with the inputs.
        let driver = Self {
            store,
            _guard: guard,
            proposer,
            jobs,
            observer,
            inputs,
            options,
            run,
            completed: BTreeSet::new(),
        };
        let state = driver.state()?;
        check_resume(&state, Some(inputs.manifest), &inputs)?;
        Ok(driver)
    }

    /// Resumes `run`.
    ///
    /// In order: the run lock (LOOP-06), the ledger fold, the recorded manifest read back from its blob,
    /// [`check_resume`] (a changed base, root image, judge, or experiment frame is refused), and the baseline's
    /// job preflight. Only then, when the manifest changed outside the experiment frame, are the new manifest
    /// bytes stored and `ManifestChanged` appended from the next generation (D-19).
    ///
    /// # Errors
    ///
    /// [`DriverError::RunLocked`] when another driver holds the run; [`DriverError::RunNotFound`] when the store
    /// holds no ledger for it; [`DriverError::Fold`] when the ledger does not fold; every refusal of
    /// [`check_resume`]; [`DriverError::Job`] and [`DriverError::Infra`] from the baseline's preflight; and
    /// [`DriverError::Store`] and [`DriverError::Conflict`] from the store.
    pub fn resume(
        store: &'a S,
        proposer: &'a Proposer<P>,
        jobs: &'a J,
        observer: O,
        inputs: RunInputs<'a>,
        options: DriverOptions,
        run: RunId,
    ) -> Result<Self, DriverError> {
        // 1. The run's lock, before the first read.
        let guard = lock(store, &run)?;
        let driver = Self {
            store,
            _guard: guard,
            proposer,
            jobs,
            observer,
            inputs,
            options,
            run,
            completed: BTreeSet::new(),
        };

        // 2. The fold; an empty ledger names no run.
        let read = store
            .read(&run)
            .map_err(|source| DriverError::Store { run, source })?;
        if read.records().next().is_none() {
            return Err(DriverError::RunNotFound { run });
        }
        let state = fold(read.records()).map_err(DriverError::Fold)?;

        // 3. The inputs against the record (D-10, D-19).
        let recorded = recorded_manifest(store, &state)?;
        let resume = check_resume(&state, recorded.as_ref(), &inputs)?;

        // 4. The baseline's plans pass preflight.
        check_job(jobs, &baseline_request(&inputs, 0)?)?;

        // 5. A manifest edit outside the experiment frame takes effect from the next generation.
        if let Resume::ManifestChanged { from_generation } = resume {
            store
                .put(inputs.manifest_bytes)
                .map_err(|source| DriverError::Store { run, source })?;
            append(
                store,
                &run,
                Event::ManifestChanged {
                    manifest: inputs.manifest_id(),
                    from_generation,
                },
            )?;
        }
        Ok(driver)
    }

    /// The run this session drives.
    #[must_use]
    pub const fn run_id(&self) -> RunId {
        self.run
    }

    /// The session's options.
    #[must_use]
    pub const fn options(&self) -> DriverOptions {
        self.options
    }

    /// The observer.
    #[must_use]
    pub const fn observer(&self) -> &O {
        &self.observer
    }

    /// Ends the session, releasing the run's lock, and returns the observer.
    #[must_use]
    pub fn into_observer(self) -> O {
        self.observer
    }

    /// Performs generation 0: every unjudged baseline trial, each recorded as its `Spent` records and then
    /// `Judged`, then the [`baseline_ready`] check (D-05, D-06). Idempotent: on a calibrated run it records
    /// nothing and returns the same calibration.
    ///
    /// # Errors
    ///
    /// [`DriverError::BaselineUnscored`] when no baseline trial has a usable score; [`DriverError::Job`] and
    /// [`DriverError::Infra`] from the job runner; [`DriverError::Request`] when a request cannot be built; and
    /// [`DriverError::Store`], [`DriverError::Conflict`], and [`DriverError::Fold`] from the ledger.
    pub fn calibrate(&mut self) -> Result<Calibrated, DriverError> {
        let manifest = self.inputs.manifest;
        let trials = manifest.judge().trials();
        let state = loop {
            let state = self.state()?;
            // With target 0, the only step after calibration is `Done`.
            let Step::Calibrate { trial } =
                next_step(&state, &self.inputs.judge, trials, CANDIDATE_TRIALS, 0)
            else {
                break state;
            };
            self.calibrate_trial(&state, trial)?;
        };

        if !baseline_ready(&state, manifest, &self.inputs.judge) {
            return Err(DriverError::BaselineUnscored { trials });
        }
        let objective = manifest.objective();
        let floor = noise_floor(&state, &self.inputs.judge, objective.metric());
        let verdict = noise_check(floor.as_ref(), objective);
        Ok(Calibrated::Ready(Calibration {
            trials,
            floor,
            verdict,
        }))
    }

    /// Drives the run until its last concluded generation is at least `target`, an absolute generation, by
    /// performing exactly the step [`next_step`] returns each time (D-07, LOOP-04). Calibration comes first.
    ///
    /// - A propose step records each model call's `Spent`, then the batch blob, then one `Proposed` per admitted
    ///   experiment; a step that admits nothing concludes its generation at once as `NoValidCandidates` (D-18).
    /// - A trial runs through [`JobRunner::run`] and is recorded as its `Spent` records and then `Judged`.
    /// - A conclusion appends `Selected`, with the drop count from the generation's batch.
    /// - A provider stop (`ProviderLimit`, `ProviderAuth`) appends the call's `Spent` if it had usage, then
    ///   `StopRequested`, and ends the session; it is never retried (06-CONTEXT D-13, D-15).
    ///
    /// # Errors
    ///
    /// Every error of [`Driver::calibrate`]; [`DriverError::Route`] when no proposer can be routed;
    /// [`DriverError::Propose`] for a proposer misconfigured for its tier; [`DriverError::Infra`] for a provider or
    /// job infrastructure failure (retried from 07-15 on); [`DriverError::Policy`] when a generation cannot be
    /// concluded; and [`DriverError::Batch`] and [`DriverError::DiffRecord`] for an inconsistent record.
    pub fn run_to(&mut self, target: u32) -> Result<RunEnd, DriverError> {
        if let Calibrated::Stopped { reason } = self.calibrate()? {
            return Ok(RunEnd::Stopped {
                generation: 0,
                reason,
            });
        }
        let trials = self.inputs.manifest.judge().trials();
        loop {
            let state = self.state()?;
            match next_step(&state, &self.inputs.judge, trials, CANDIDATE_TRIALS, target) {
                Step::Calibrate { trial } => self.calibrate_trial(&state, trial)?,
                Step::Propose { generation } => {
                    if let Some(end) = self.propose(&state, generation)? {
                        return Ok(end);
                    }
                }
                Step::RunTrial {
                    generation,
                    exp,
                    trial,
                } => {
                    // A batch a crash left partly recorded is completed first, and the step asked again, so the
                    // trials run in the order an uninterrupted session would run them (D-18).
                    if !self.complete_batch(&state, generation)? {
                        self.run_trial(&state, generation, exp, trial)?;
                    }
                }
                Step::Conclude { generation } => {
                    if !self.complete_batch(&state, generation)? {
                        self.conclude_generation(&state, generation)?;
                    }
                }
                Step::Done { generation } => return Ok(RunEnd::Reached { generation }),
            }
        }
    }

    /// Reads and folds the run's ledger. Called before every step; never cached.
    fn state(&self) -> Result<RunState, DriverError> {
        let read = self
            .store
            .read(&self.run)
            .map_err(|source| self.store_error(source))?;
        fold(read.records()).map_err(DriverError::Fold)
    }

    /// A store error of this run.
    const fn store_error(&self, source: StoreError) -> DriverError {
        DriverError::Store {
            run: self.run,
            source,
        }
    }

    /// Runs baseline trial `trial` and records it.
    fn calibrate_trial(&mut self, state: &RunState, trial: u32) -> Result<(), DriverError> {
        let request = baseline_request(&self.inputs, trial)?;
        let result = run_job(self.jobs, &request)?;
        self.record_result(state, 0, &request, &result)?;
        self.observer.observe(&LoopEvent::Calibrated {
            trial,
            outcome: result.outcome().clone(),
        });
        Ok(())
    }

    /// Records one job's result for `request` in `generation`: the candidate's `Spent`, the judge's `Spent` when
    /// the judge ran, each only if absent, then `Judged` with the stage logs the runner stored (D-09).
    fn record_result(
        &self,
        state: &RunState,
        generation: u32,
        request: &JobRequest,
        result: &JobResult,
    ) -> Result<(), DriverError> {
        let exp = request.exp();
        let trial = request.trial();
        spend_once(
            self.store,
            state,
            &self.run,
            generation,
            SpendSource::Candidate { exp, trial },
            result.usage(),
        )?;
        if let Some(usage) = result.judge_usage() {
            spend_once(
                self.store,
                state,
                &self.run,
                generation,
                SpendSource::Judge { exp, trial },
                usage,
            )?;
        }
        let logs = (!result.logs().is_empty()).then(|| result.logs().clone());
        append(
            self.store,
            &self.run,
            Event::Judged {
                exp,
                judge: request.judge(),
                trial,
                outcome: result.outcome().clone(),
                backend: result.backend().clone(),
                logs,
            },
        )
    }

    /// Performs the propose step of `generation`. `Some` when a provider stop ended the session.
    fn propose(
        &mut self,
        state: &RunState,
        generation: u32,
    ) -> Result<Option<RunEnd>, DriverError> {
        let manifest = self.inputs.manifest;
        let base = self.inputs.base;
        let judge = self.inputs.judge;

        // 1. The incumbent and its surface tree (06-CONTEXT D-05).
        let (parent, tree) = parent_tree(self.store, state, manifest, base, generation)?;

        // 2. The routed proposer (D-13).
        let surface = SurfaceStats::new(surface_bytes(&self.inputs));
        let routed = route(manifest, state, &surface).map_err(DriverError::Route)?;
        let spec = manifest
            .proposers()
            .get(routed.proposer())
            .ok_or(DriverError::Route(RouteError::NoEligibleTier))?;

        // 3. The prompt's history and notes, and the call's index in the generation.
        let history = history(self.store, state, manifest, &judge, generation)?;
        let notes = notes(self.store, state, generation)?;
        let call = state.next_model_call(generation);

        // 4. The call.
        let input = ProposeInput {
            manifest,
            run: &self.run,
            generation,
            base,
            parent: &tree,
            program: self.inputs.program,
            history: &history,
            notes: &notes,
        };
        let proposals = match self.proposer.propose(spec, &input) {
            Ok(proposals) => proposals,
            Err(error) => {
                return self
                    .propose_failed(state, generation, call, spec, error)
                    .map(Some);
            }
        };

        // 5. The call's spend, once per resolved model, or once under the spec's model; a sweep has neither.
        if proposals.models().is_empty() {
            if let (Some(usage), Some(model)) = (proposals.usage(), spec.model()) {
                self.spend_call(state, generation, call, model, *usage)?;
            }
        } else {
            for (model, usage) in proposals.models() {
                self.spend_call(state, generation, call, model, *usage)?;
            }
        }

        // 6. Admission: the batch blob, then one `Proposed` per admitted experiment (D-12, D-18).
        let tier = proposals.tier();
        let origin = proposals.origin().clone();
        let dropped = proposals.dropped().total();
        let admission = admit(
            self.store,
            state,
            AdmitStep {
                generation,
                parent,
                tier,
                origin: &origin,
                dropped,
            },
            proposals.into_items(),
        )?;
        match admission.batch() {
            // 7a. Nothing survived: the generation concludes in the same step.
            None => self.select(state, generation, admission.dropped())?,
            // 7b. Every experiment of the batch, in batch order.
            Some((blob, batch)) => {
                let events = missing_proposed(state, blob, batch);
                let admitted = u32::try_from(events.len()).unwrap_or(u32::MAX);
                for event in events {
                    append(self.store, &self.run, event)?;
                }
                self.completed.insert(generation);
                self.observer.observe(&LoopEvent::Proposed {
                    generation,
                    tier,
                    admitted,
                    dropped: admission.dropped(),
                });
            }
        }
        Ok(None)
    }

    /// Records a failed propose step: the call's `Spent` when it reported usage, then, for a provider stop,
    /// `StopRequested`, which ends the session. Any other failure records nothing more and is an error.
    fn propose_failed(
        &mut self,
        state: &RunState,
        generation: u32,
        call: u32,
        spec: &ProposerSpec,
        error: ProposeError,
    ) -> Result<RunEnd, DriverError> {
        if let (Some(usage), Some(model)) = (error.usage(), spec.model()) {
            self.spend_call(state, generation, call, model, *usage)?;
        }
        if let Some(reason) = error.stop_reason() {
            append(
                self.store,
                &self.run,
                Event::StopRequested { generation, reason },
            )?;
            self.observer
                .observe(&LoopEvent::Stopped { generation, reason });
            return Ok(RunEnd::Stopped { generation, reason });
        }
        if error.is_infra() {
            return Err(DriverError::Infra {
                what: "the model provider failed",
            });
        }
        Err(DriverError::Propose(error))
    }

    /// Records one model call's spend under `model`, only if absent.
    fn spend_call(
        &self,
        state: &RunState,
        generation: u32,
        call: u32,
        model: &minos_core::id::ModelId,
        usage: minos_core::usage::Usage,
    ) -> Result<(), DriverError> {
        spend_once(
            self.store,
            state,
            &self.run,
            generation,
            SpendSource::ModelCall {
                call,
                model: model.clone(),
            },
            usage,
        )
    }

    /// Appends the `Proposed` records a crash left out of `generation`'s batch, once per session. `true` when it
    /// appended any, so the caller asks [`next_step`] again.
    fn complete_batch(&mut self, state: &RunState, generation: u32) -> Result<bool, DriverError> {
        if self.completed.contains(&generation) {
            return Ok(false);
        }
        let missing = load_batch(self.store, state, &self.run, generation)?
            .map(|(blob, batch)| missing_proposed(state, &blob, &batch))
            .unwrap_or_default();
        let appended = !missing.is_empty();
        for event in missing {
            append(self.store, &self.run, event)?;
        }
        self.completed.insert(generation);
        Ok(appended)
    }

    /// Runs trial `trial` of `exp`, proposed in `generation`, and records it.
    fn run_trial(
        &mut self,
        state: &RunState,
        generation: u32,
        exp: ExperimentId,
        trial: u32,
    ) -> Result<(), DriverError> {
        let manifest = self.inputs.manifest;
        let judge = self.inputs.judge;
        let proposal = state
            .proposal(&exp)
            .ok_or(DriverError::DiffRecord { exp })?;
        let tier = proposal.tier();
        let cumulative = recorded_diff(self.store, state, manifest, self.inputs.base, &exp)?;
        let request = JobRequest::candidate(
            &Experiment::candidate(cumulative),
            *proposal.diff(),
            judge,
            trial,
            JobBudget::from_budget(manifest.budget()),
        )
        .map_err(DriverError::Request)?;
        let result = run_job(self.jobs, &request)?;
        self.record_result(state, generation, &request, &result)?;

        let state = self.state()?;
        let delta = objective_delta(&state, manifest, &judge, &exp, generation);
        self.observer.observe(&LoopEvent::Judged {
            generation,
            exp,
            trial,
            tier,
            outcome: result.outcome().clone(),
            delta,
        });
        Ok(())
    }

    /// Concludes `generation` with the drop count its batch recorded, 0 when it has no batch.
    fn conclude_generation(
        &mut self,
        state: &RunState,
        generation: u32,
    ) -> Result<(), DriverError> {
        let dropped = load_batch(self.store, state, &self.run, generation)?
            .map_or(0, |(_, batch)| batch.dropped());
        self.select(state, generation, dropped)
    }

    /// Appends `generation`'s `Selected` as [`conclude`] decides it.
    fn select(
        &mut self,
        state: &RunState,
        generation: u32,
        dropped: u32,
    ) -> Result<(), DriverError> {
        let decision = conclude(
            state,
            self.inputs.manifest,
            &self.inputs.judge,
            generation,
            dropped,
        )
        .map_err(DriverError::Policy)?;
        append(
            self.store,
            &self.run,
            Event::Selected {
                generation,
                decision,
            },
        )?;
        self.observer.observe(&LoopEvent::Selected {
            generation,
            decision,
        });
        Ok(())
    }
}

/// Takes `run`'s lock, naming a held run as [`DriverError::RunLocked`].
fn lock<'s, S: RunLock>(store: &'s S, run: &RunId) -> Result<S::Guard<'s>, DriverError> {
    store.lock_run(run).map_err(|source| match source {
        StoreError::RunLocked { run } => DriverError::RunLocked { run },
        source => DriverError::Store { run: *run, source },
    })
}

/// The manifest the run has in effect, read back from its blob, or `None` when the blob is absent, over the cap,
/// tampered, or does not parse.
fn recorded_manifest<S: Blobs>(
    store: &S,
    state: &RunState,
) -> Result<Option<Manifest>, DriverError> {
    // A manifest's ID is the plain SHA-256 of its bytes, so the blob of those bytes has the same digest.
    let Ok(blob) = state.manifest().to_hex().parse::<BlobId>() else {
        return Ok(None);
    };
    let bytes = match store.get(&blob, byte_cap(MAX_MANIFEST_BYTES)) {
        Ok(bytes) => bytes,
        Err(
            StoreError::BlobMissing { .. }
            | StoreError::BlobTooLarge { .. }
            | StoreError::BlobTampered { .. },
        ) => return Ok(None),
        Err(source) => {
            return Err(DriverError::Store {
                run: *state.run(),
                source,
            });
        }
    };
    Ok(std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| Manifest::parse_toml(text).ok()))
}

/// The request for baseline trial `trial` under the session's judge and the manifest's stage budgets.
fn baseline_request(inputs: &RunInputs<'_>, trial: u32) -> Result<JobRequest, DriverError> {
    JobRequest::baseline(
        inputs.base.id(),
        inputs.judge,
        trial,
        JobBudget::from_budget(inputs.manifest.budget()),
    )
    .map_err(DriverError::Request)
}

/// The blame of a job failure: a configuration refusal stops the driver, and an infrastructure failure surfaces
/// as [`DriverError::Infra`] until 07-15 retries it. Nothing is recorded for either.
const fn job_error(failure: JobFailure) -> DriverError {
    match failure {
        JobFailure::Config { what } => DriverError::Job { what },
        JobFailure::Infra { what } => DriverError::Infra { what },
    }
}

/// Preflights `request` through the job runner.
fn check_job<J: JobRunner>(jobs: &J, request: &JobRequest) -> Result<(), DriverError> {
    jobs.check(request).map_err(job_error)
}

/// Runs `request` through the job runner.
fn run_job<J: JobRunner>(jobs: &J, request: &JobRequest) -> Result<JobResult, DriverError> {
    jobs.run(request).map_err(job_error)
}

/// The size a single-shot prompt must carry: the program file plus every base surface file (`SurfaceStats`).
fn surface_bytes(inputs: &RunInputs<'_>) -> u64 {
    let size = |len: usize| u64::try_from(len).unwrap_or(u64::MAX);
    inputs
        .base
        .tree()
        .iter()
        .fold(size(inputs.program.len()), |total, (_, bytes)| {
            total.saturating_add(size(bytes.len()))
        })
}

/// `exp`'s reduced objective (every trial scored) minus the incumbent's before `generation` (scored trials only),
/// or `None` when either is unscored or the difference is not finite.
fn objective_delta(
    state: &RunState,
    manifest: &Manifest,
    judge: &JudgeVersion,
    exp: &ExperimentId,
    generation: u32,
) -> Option<Metric> {
    let metric = manifest.objective().metric();
    let reduced = |exp: &ExperimentId, rule: TrialRule| {
        reduce_trials(
            state
                .trials(exp, judge)
                .map(|(_, judgment)| judgment.outcome()),
            rule,
        )
        .and_then(|score| score.get(metric))
    };
    let candidate = reduced(exp, TrialRule::AllScored)?;
    let incumbent = reduced(&state.incumbent_before(generation), TrialRule::ScoredOnly)?;
    Metric::new(candidate.value() - incumbent.value()).ok()
}
