//! The generation driver: starts or resumes a run under its lock, calibrates generation 0, and drives
//! generations to an absolute target (07-CONTEXT D-05 to D-16, D-18, D-19; LOOP-04, LOOP-05, LOOP-07, LOOP-08,
//! JUDGE-04).
//!
//! # Record before act
//!
//! Every effect is recorded in the ledger before the driver acts on it, keyed by its content-derived ID: a job's
//! `Spent` records precede its `Judged`, a model call's `Spent` records precede its batch, the batch blob is
//! stored before the batch's first `Proposed`, and a generation's `Selected` follows its last `Judged`. A `Spent`
//! is appended only when its `SpendId` is absent ([`spend_once`]), and a conflicting append stops the driver with
//! [`DriverError::Conflict`] and records nothing more (D-08; 07-RESEARCH Pitfall 1). Every stop the driver
//! takes is likewise recorded as `StopRequested` before the session ends, so a resumed session reaches the same
//! stop an uninterrupted one did.
//!
//! # The fold is the only run state
//!
//! The driver keeps no position. Before every step it reads and folds the ledger and performs exactly the step
//! [`next_step`] returns, so a session resumed after a crash at any point performs the same next step an
//! uninterrupted one would have (D-07, LOOP-05). Within a session it remembers three things only: which
//! generations' batches it has already completed (so it reads a batch blob once per generation), the stops the
//! ledger held when the session started, and the streak of consecutive infrastructure failures. Forgetting the
//! first costs a re-read; the other two are session-relative by design, so a restart resets them.
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
//! # Safe points (D-14)
//!
//! The driver checks whether the session must end at four kinds of safe point. Each check calls the pure core
//! function that owns the decision; the driver only chooses where:
//!
//! | Safe point | When | New stops | [`circuit_breaker`] | [`spend_ok`] |
//! |---|---|---|---|---|
//! | Generation | Before generation 0's first launch in a session, and before every later step of [`Driver::run_to`] | yes | yes | yes |
//! | Model call | Before every propose call, and again after every model call that reported usage, before its proposals are admitted | yes | no | yes |
//! | Launch | Before every [`JobRunner::run`], baseline or candidate | yes | no | no |
//! | After trial | After a trial's `Spent` and `Judged` are recorded | yes | no | yes |
//!
//! A breaker or cap verdict appends `StopRequested { generation, reason }` and ends the session; the key
//! `(generation, reason)` makes a repeat a duplicate, so a cap that re-trips on resume records nothing new. A
//! resolved model ID that `prices.toml` does not price ends the session with [`DriverError::UnpricedModel`] right
//! after the first call that used it, its spend already recorded (D-13 extended; 07-RESEARCH Pitfall 7).
//!
//! # Stops are session-relative (D-19)
//!
//! A `StopRequested` stays in the ledger forever, so the driver snapshots the recorded stops when its session
//! starts and honours only a stop added after that, whether by `minos stop` or by itself. Cap and breaker stops
//! are re-derived from the fold at every safe point rather than read from old records, so a run stopped by a cap
//! resumes once the cap is raised, and an old `UserStop`, `ProviderLimit`, or `ProviderAuth` never blocks a
//! resume by itself. An honoured stop records nothing.
//!
//! The driver never kills a VM. [`JobRunner::run`] is synchronous and each VM has a hard maximum duration, so a
//! stop that arrives while a trial runs takes effect at the next safe point, after that trial's `Spent` and
//! `Judged` are recorded.
//!
//! # Caps are metered only (D-15)
//!
//! [`spend_ok`] prices the recorded `Spent` events; nothing is estimated before a job or a call runs. A run can
//! therefore overshoot its spend or token cap by at most one VM job (the trial whose `Spent` crossed it) or one
//! model call, and never by more, because the next safe point sees the recorded spend.
//!
//! # Infrastructure retries (D-16)
//!
//! A [`JobFailure::Infra`], a provider infrastructure error, or a retryable store error on an append records
//! nothing for the failed attempt, adds one to the session's streak, and is retried at the same step after
//! [`DriverOptions::retry_pause`]: the same `LaunchKey` is re-derived from the fold, and a model call takes the
//! next call index only if the failed call's spend was recorded. A failed append is retried on its own while the
//! driver holds the result it already obtained, so a store hiccup never re-runs a VM or a model call. A successful
//! job, a successful model call, and an append that lands after failing reset the streak to 0. When the streak
//! reaches `max_infra_failures` the driver appends `StopRequested { InfraFailures }` and ends the session.
//!
//! # The backend pin (D-11, JUDGE-04)
//!
//! A run never mixes backend classes. Before recording a trial, the driver refuses a [`JobResult`] whose
//! `(backend, arch)` differs from the class of the run's recorded trials with [`DriverError::BackendChanged`],
//! recording nothing for that trial.
//!
//! # Phase 8 interface points
//!
//! The CLI builds the [`RunInputs`], picks the concrete store, provider, job runner, and observer, and calls
//! [`Driver::start`] or [`Driver::resume`], then [`Driver::calibrate`] (`minos check`) or [`Driver::run_to`]
//! (`minos run`). `minos stop` appends `StopRequested { UserStop }`, which a running session honours at its
//! next safe point.

use std::collections::BTreeSet;
use std::time::Duration;

use minos_core::experiment::Experiment;
use minos_core::id::{BlobId, ExperimentId, ModelId, RunId};
use minos_core::job::{JobBudget, JobRequest, JobResult};
use minos_core::ledger::{Event, SpendSource, StopReason};
use minos_core::manifest::{JudgeVersion, Manifest, ProposerSpec};
use minos_core::metric::Metric;
use minos_core::money::CostError;
use minos_core::policy::{
    NoiseFloor, NoiseVerdict, SpendCheck, TrialRule, baseline_ready, circuit_breaker, conclude,
    noise_check, noise_floor, reduce_trials, spend_ok,
};
use minos_core::route::{RouteError, SurfaceStats, route};
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;
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

/// The fixed pause before an infrastructure retry (D-16): long enough for a transient fault on the host to clear,
/// and short against a stage budget, so even a full streak of `max_infra_failures` pauses costs little next to one
/// trial. It is one constant rather than a backoff schedule because the breaker, not the pause, bounds the
/// retries. Tests pass zero.
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
    /// The session stopped during generation 0 at a safe point: a cap, the breaker, or a stop added during the
    /// session (D-14, D-19).
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
    /// The session recorded or honoured a stop.
    Stopped {
        /// The generation the stop is recorded under.
        generation: u32,
        /// Why.
        reason: StopReason,
    },
}

/// The safe points at which the driver checks whether the session must end (D-14; module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafePoint {
    /// Before a generation's work: new stops, the circuit breaker, and the caps.
    Generation,
    /// Before a model call, and after one that reported usage: new stops and the caps.
    ModelCall,
    /// Before a VM launch: new stops only; compute spend is checked once its `Spent` lands.
    Launch,
    /// After a trial's records land: new stops and the caps.
    AfterTrial,
}

impl SafePoint {
    /// Whether this safe point runs [`circuit_breaker`].
    const fn checks_breaker(self) -> bool {
        matches!(self, Self::Generation)
    }

    /// Whether this safe point runs [`spend_ok`].
    const fn checks_spend(self) -> bool {
        !matches!(self, Self::Launch)
    }
}

/// Why an internal step did not finish normally: the session ended at a stop, or the driver failed.
///
/// Internal steps return [`Flow`], so `?` carries a stop as readily as an error up to the public entry point,
/// which turns it back into [`RunEnd::Stopped`] or [`Calibrated::Stopped`].
#[derive(Debug)]
enum Halt {
    /// The session ended at a stop it recorded or honoured; [`LoopEvent::Stopped`] was already emitted.
    Stopped {
        /// The generation the stop is recorded under.
        generation: u32,
        /// Why.
        reason: StopReason,
    },
    /// The driver failed.
    Failed(DriverError),
}

impl From<DriverError> for Halt {
    fn from(error: DriverError) -> Self {
        Self::Failed(error)
    }
}

/// The result of an internal step.
type Flow<T> = Result<T, Halt>;

/// One session of the generation driver over one run, holding the run's lock until it drops.
///
/// Generic over its store, model provider, job runner, and observer, never a trait object. It holds the borrowed
/// inputs, the session options, the run ID, the set of generations whose batch it completed this session, the
/// stops the ledger held when the session started, and the streak of consecutive infrastructure failures;
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
    /// The stops recorded when the session started; only a stop outside this set ends the session (D-19).
    session_stops: BTreeSet<(u32, StopReason)>,
    /// Consecutive infrastructure failures in this session (D-16); a restart resets it.
    streak: u32,
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
    /// lock's root image (D-10), and [`check_resume`] against what the ledger now holds. The stops the ledger then
    /// holds are the session's starting stops (D-19).
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

        // 6. The ledger agrees with the inputs, and its stops are the session's starting stops.
        let mut driver = Self {
            store,
            _guard: guard,
            proposer,
            jobs,
            observer,
            inputs,
            options,
            run,
            completed: BTreeSet::new(),
            session_stops: BTreeSet::new(),
            streak: 0,
        };
        let state = driver.state()?;
        check_resume(&state, Some(inputs.manifest), &inputs)?;
        driver.session_stops = state.stops().collect();
        Ok(driver)
    }

    /// Resumes `run`.
    ///
    /// In order: the run lock (LOOP-06), the ledger fold, the recorded manifest read back from its blob,
    /// [`check_resume`] (a changed base, root image, judge, or experiment frame is refused), and the baseline's
    /// job preflight. Only then, when the manifest changed outside the experiment frame, are the new manifest
    /// bytes stored and `ManifestChanged` appended from the next generation (D-19). The stops the ledger holds
    /// are the session's starting stops, so none of them ends this session by itself (D-19).
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
        let mut driver = Self {
            store,
            _guard: guard,
            proposer,
            jobs,
            observer,
            inputs,
            options,
            run,
            completed: BTreeSet::new(),
            session_stops: BTreeSet::new(),
            streak: 0,
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

        // 6. Every stop recorded so far belongs to an earlier session (D-19).
        driver.session_stops = state.stops().collect();
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
    /// Before the session's first baseline launch it passes the generation safe point, before every launch the
    /// launch safe point, and after every recorded trial the after-trial safe point (module docs); a stop at any
    /// of them is [`Calibrated::Stopped`]. An infrastructure failure of a trial is retried after the pause
    /// (D-16).
    ///
    /// # Errors
    ///
    /// [`DriverError::BaselineUnscored`] when no baseline trial has a usable score; [`DriverError::Job`] from the
    /// job runner; [`DriverError::BackendChanged`] when a trial ran on another backend class (D-11);
    /// [`DriverError::UnpricedModel`] and [`DriverError::Cost`] when the recorded spend cannot be priced;
    /// [`DriverError::Infra`] when the stop that ends the infrastructure retries could not be recorded;
    /// [`DriverError::Request`] when a request cannot be built; and [`DriverError::Store`],
    /// [`DriverError::Conflict`], and [`DriverError::Fold`] from the ledger.
    pub fn calibrate(&mut self) -> Result<Calibrated, DriverError> {
        match self.calibrate_flow() {
            Ok(calibration) => Ok(Calibrated::Ready(calibration)),
            Err(Halt::Stopped { reason, .. }) => Ok(Calibrated::Stopped { reason }),
            Err(Halt::Failed(error)) => Err(error),
        }
    }

    /// Drives the run until its last concluded generation is at least `target`, an absolute generation, by
    /// performing exactly the step [`next_step`] returns each time (D-07, LOOP-04). Calibration comes first, and
    /// every step after it passes the generation safe point (module docs).
    ///
    /// - A propose step passes the model-call safe point, then records each model call's `Spent`, passes the
    ///   model-call safe point again, then records the batch blob and one `Proposed` per admitted experiment; a
    ///   step that admits nothing concludes its generation at once as `NoValidCandidates` (D-18).
    /// - A trial passes the launch safe point, runs through [`JobRunner::run`], is recorded as its `Spent`
    ///   records and then `Judged`, and passes the after-trial safe point.
    /// - A conclusion appends `Selected`, with the drop count from the generation's batch.
    /// - A provider stop (`ProviderLimit`, `ProviderAuth`) appends the call's `Spent` if it had usage, then
    ///   `StopRequested`, and ends the session; it is never retried (06-CONTEXT D-13, D-15).
    /// - A cap, the breaker, or a stop added during the session ends it as [`RunEnd::Stopped`] (D-14, D-19).
    /// - An infrastructure failure is retried at the same step after the pause, until the streak reaches
    ///   `max_infra_failures` (D-16).
    ///
    /// # Errors
    ///
    /// Every error of [`Driver::calibrate`]; [`DriverError::Route`] when no proposer can be routed;
    /// [`DriverError::Propose`] for a proposer misconfigured for its tier; [`DriverError::UnpricedModel`] right
    /// after a model call whose resolved model `prices.toml` does not price; [`DriverError::Policy`] when a
    /// generation cannot be concluded; and [`DriverError::Batch`] and [`DriverError::DiffRecord`] for an
    /// inconsistent record.
    pub fn run_to(&mut self, target: u32) -> Result<RunEnd, DriverError> {
        match self.drive(target) {
            Ok(generation) => Ok(RunEnd::Reached { generation }),
            Err(Halt::Stopped { generation, reason }) => Ok(RunEnd::Stopped { generation, reason }),
            Err(Halt::Failed(error)) => Err(error),
        }
    }

    /// [`Driver::calibrate`]'s body: the baseline trials, then the readiness check and the noise verdict.
    fn calibrate_flow(&mut self) -> Flow<Calibration> {
        let manifest = self.inputs.manifest;
        let trials = manifest.judge().trials();
        let mut entered = false;
        let state = loop {
            let state = self.state()?;
            // With target 0, the only step after calibration is `Done`.
            let Step::Calibrate { trial } =
                next_step(&state, &self.inputs.judge, trials, CANDIDATE_TRIALS, 0)
            else {
                break state;
            };
            // Generation 0 is a generation: its safe point comes before the session's first baseline launch, so
            // a resumed run whose cap is still exceeded launches nothing more (D-14, D-15).
            if !entered {
                self.safe_point(SafePoint::Generation, 0)?;
                entered = true;
            }
            self.calibrate_trial(&state, trial)?;
        };

        if !baseline_ready(&state, manifest, &self.inputs.judge) {
            return Err(DriverError::BaselineUnscored { trials }.into());
        }
        let objective = manifest.objective();
        let floor = noise_floor(&state, &self.inputs.judge, objective.metric());
        let verdict = noise_check(floor.as_ref(), objective);
        Ok(Calibration {
            trials,
            floor,
            verdict,
        })
    }

    /// [`Driver::run_to`]'s body; `Ok` holds the last concluded generation once it reaches `target`.
    fn drive(&mut self, target: u32) -> Flow<u32> {
        self.calibrate_flow()?;
        let trials = self.inputs.manifest.judge().trials();
        loop {
            let state = self.state()?;
            let step = next_step(&state, &self.inputs.judge, trials, CANDIDATE_TRIALS, target);
            if let Step::Done { generation } = step {
                return Ok(generation);
            }
            // Every step of generation g = last concluded + 1 passes the generation safe point first. A passed
            // safe point records nothing, so `state` still describes the ledger.
            let current = state.last_concluded_generation().saturating_add(1);
            self.safe_point(SafePoint::Generation, current)?;
            match step {
                Step::Calibrate { trial } => self.calibrate_trial(&state, trial)?,
                Step::Propose { generation } => self.propose(&state, generation)?,
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
                Step::Done { generation } => return Ok(generation),
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

    /// Checks whether the session must end at safe point `at` of `generation` (D-14, D-19; module docs).
    ///
    /// In order, over a fresh fold: a stop recorded since the session started ends it and records nothing; at a
    /// generation, a [`circuit_breaker`] verdict appends `StopRequested` and ends it; at every safe point but a
    /// launch, a [`spend_ok`] cap verdict does the same, and an unpriced spend is an error.
    fn safe_point(&mut self, at: SafePoint, generation: u32) -> Flow<()> {
        let state = self.state()?;

        // 1. A stop added since the session started: honoured, never recorded again (D-19).
        if let Some((recorded, reason)) = state
            .stops()
            .find(|stop| !self.session_stops.contains(stop))
        {
            return Err(self.stopped(recorded, reason));
        }

        // 2. The breaker: K empty generations in a row (D-14, LOOP-08).
        if at.checks_breaker()
            && let Some(reason) =
                circuit_breaker(&state, self.inputs.manifest.budget(), self.streak)
        {
            return Err(self.stop(generation, reason));
        }

        // 3. The metered caps (D-15, LOOP-07).
        if at.checks_spend() {
            match spend_ok(&state, self.inputs.manifest, self.inputs.prices) {
                Ok(SpendCheck::Within { .. }) => {}
                Ok(SpendCheck::Stop(reason)) => return Err(self.stop(generation, reason)),
                Err(CostError::UnpricedModel { model }) => {
                    return Err(DriverError::UnpricedModel { model }.into());
                }
                Err(error) => return Err(DriverError::Cost(error).into()),
            }
        }
        Ok(())
    }

    /// Records `StopRequested { generation, reason }` and ends the session; a halt while recording it wins.
    fn stop(&mut self, generation: u32, reason: StopReason) -> Halt {
        match self.record(generation, Event::StopRequested { generation, reason }) {
            Ok(()) => self.stopped(generation, reason),
            Err(halt) => halt,
        }
    }

    /// Ends the session at a stop already in the ledger, emitting [`LoopEvent::Stopped`] once.
    fn stopped(&mut self, generation: u32, reason: StopReason) -> Halt {
        self.observer
            .observe(&LoopEvent::Stopped { generation, reason });
        Halt::Stopped { generation, reason }
    }

    /// Counts one infrastructure failure of `generation`'s current step, of which `what` is fixed text (D-16).
    ///
    /// Below `max_infra_failures` it emits [`LoopEvent::Retrying`], pauses for the session's retry pause, and
    /// returns `Ok`, so the caller re-derives the same step. At the limit it appends
    /// `StopRequested { InfraFailures }` and ends the session; when even that append fails on a retryable store
    /// error, the failure is [`DriverError::Infra`] naming `what`, and resuming the run retries from the ledger.
    fn retry(&mut self, generation: u32, what: &'static str) -> Flow<()> {
        self.streak = self.streak.saturating_add(1);
        let max = self.inputs.manifest.budget().max_infra_failures();
        if self.streak >= max {
            let reason = StopReason::InfraFailures;
            return match append(
                self.store,
                &self.run,
                Event::StopRequested { generation, reason },
            ) {
                Ok(()) => Err(self.stopped(generation, reason)),
                Err(DriverError::Store { source, .. }) if source.is_retryable() => {
                    Err(DriverError::Infra { what }.into())
                }
                Err(error) => Err(error.into()),
            };
        }
        self.observer.observe(&LoopEvent::Retrying {
            streak: self.streak,
            max,
        });
        std::thread::sleep(self.options.retry_pause);
        Ok(())
    }

    /// Runs `attempt`, one append, until it lands, retrying only the append on a retryable store error while the
    /// caller keeps the result it already obtained, so a store hiccup never re-runs a VM or a model call (D-16).
    ///
    /// An append that lands after failing resets the streak. One that lands at once leaves it alone, because the
    /// step it belongs to has not necessarily succeeded: a failed model call's spend is recorded before its retry
    /// is counted.
    fn retry_append(
        &mut self,
        generation: u32,
        mut attempt: impl FnMut() -> Result<(), DriverError>,
    ) -> Flow<()> {
        let mut failed = false;
        loop {
            match attempt() {
                Ok(()) => {
                    if failed {
                        self.streak = 0;
                    }
                    return Ok(());
                }
                Err(DriverError::Store { source, .. }) if source.is_retryable() => {
                    failed = true;
                    self.retry(generation, "a ledger append failed")?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Appends `event` for `generation`, retrying only the append (D-16).
    fn record(&mut self, generation: u32, event: Event) -> Flow<()> {
        let (store, run) = (self.store, self.run);
        self.retry_append(generation, move || append(store, &run, event.clone()))
    }

    /// Appends `Spent { generation, source, usage }` unless the fold already records it, retrying only the
    /// append (D-16).
    fn spend(
        &mut self,
        state: &RunState,
        generation: u32,
        source: &SpendSource,
        usage: Usage,
    ) -> Flow<()> {
        let (store, run) = (self.store, self.run);
        self.retry_append(generation, || {
            spend_once(store, state, &run, generation, source.clone(), usage)
        })
    }

    /// Runs `request` for `generation` through the job runner and checks its backend class against the run's
    /// (D-11). `None` when an infrastructure failure was counted and the step is to be retried after the pause;
    /// nothing is recorded for it (D-16).
    fn run_job(
        &mut self,
        state: &RunState,
        request: &JobRequest,
        generation: u32,
    ) -> Flow<Option<JobResult>> {
        match self.jobs.run(request) {
            Ok(result) => {
                self.streak = 0;
                pin_backend(state, &result)?;
                Ok(Some(result))
            }
            Err(JobFailure::Infra { what }) => {
                self.retry(generation, what)?;
                Ok(None)
            }
            Err(JobFailure::Config { what }) => Err(DriverError::Job { what }.into()),
        }
    }

    /// Runs baseline trial `trial` and records it, between the launch and after-trial safe points.
    fn calibrate_trial(&mut self, state: &RunState, trial: u32) -> Flow<()> {
        let request = baseline_request(&self.inputs, trial)?;
        self.safe_point(SafePoint::Launch, 0)?;
        let Some(result) = self.run_job(state, &request, 0)? else {
            return Ok(());
        };
        self.record_result(state, 0, &request, &result)?;
        self.observer.observe(&LoopEvent::Calibrated {
            trial,
            outcome: result.outcome().clone(),
        });
        self.safe_point(SafePoint::AfterTrial, 0)
    }

    /// Records one job's result for `request` in `generation`: the candidate's `Spent`, the judge's `Spent` when
    /// the judge ran, each only if absent, then `Judged` with the stage logs the runner stored (D-09).
    fn record_result(
        &mut self,
        state: &RunState,
        generation: u32,
        request: &JobRequest,
        result: &JobResult,
    ) -> Flow<()> {
        let exp = request.exp();
        let trial = request.trial();
        self.spend(
            state,
            generation,
            &SpendSource::Candidate { exp, trial },
            result.usage(),
        )?;
        if let Some(usage) = result.judge_usage() {
            self.spend(state, generation, &SpendSource::Judge { exp, trial }, usage)?;
        }
        let logs = (!result.logs().is_empty()).then(|| result.logs().clone());
        self.record(
            generation,
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

    /// Performs the propose step of `generation`, between the model-call safe points.
    fn propose(&mut self, state: &RunState, generation: u32) -> Flow<()> {
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

        // 4. The model-call safe point, then the call (D-14).
        self.safe_point(SafePoint::ModelCall, generation)?;
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
            Ok(proposals) => {
                self.streak = 0;
                proposals
            }
            Err(error) => return self.propose_failed(state, generation, call, spec, error),
        };

        // 5. The call's spend, once per resolved model, or once under the spec's model; a sweep has neither.
        let mut paid = false;
        if proposals.models().is_empty() {
            if let (Some(usage), Some(model)) = (proposals.usage(), spec.model()) {
                self.spend_call(state, generation, call, model, *usage)?;
                paid = true;
            }
        } else {
            for (model, usage) in proposals.models() {
                self.spend_call(state, generation, call, model, *usage)?;
                paid = true;
            }
        }

        // 6. The spend is priced before anything is admitted, so a cap the call crossed or a resolved model
        //    `prices.toml` does not price stops the run right after this one paid call (D-13 extended, D-15).
        if paid {
            self.safe_point(SafePoint::ModelCall, generation)?;
        }

        // 7. Admission: the batch blob, then one `Proposed` per admitted experiment (D-12, D-18).
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
            // 8a. Nothing survived: the generation concludes in the same step.
            None => self.select(state, generation, admission.dropped())?,
            // 8b. Every experiment of the batch, in batch order.
            Some((blob, batch)) => {
                let events = missing_proposed(state, blob, batch);
                let admitted = u32::try_from(events.len()).unwrap_or(u32::MAX);
                for event in events {
                    self.record(generation, event)?;
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
        Ok(())
    }

    /// Records a failed propose step: the call's `Spent` when it reported usage, followed by the model-call safe
    /// point; then, for a provider stop, `StopRequested`, which ends the session (06-CONTEXT D-13, D-15). An
    /// infrastructure failure is counted and retried after the pause (D-16); any other failure records nothing
    /// more and is an error.
    fn propose_failed(
        &mut self,
        state: &RunState,
        generation: u32,
        call: u32,
        spec: &ProposerSpec,
        error: ProposeError,
    ) -> Flow<()> {
        if let (Some(usage), Some(model)) = (error.usage(), spec.model()) {
            self.spend_call(state, generation, call, model, *usage)?;
            self.safe_point(SafePoint::ModelCall, generation)?;
        }
        if let Some(reason) = error.stop_reason() {
            return Err(self.stop(generation, reason));
        }
        if error.is_infra() {
            return self.retry(generation, "the model provider failed");
        }
        Err(DriverError::Propose(error).into())
    }

    /// Records one model call's spend under `model`, only if absent.
    fn spend_call(
        &mut self,
        state: &RunState,
        generation: u32,
        call: u32,
        model: &ModelId,
        usage: Usage,
    ) -> Flow<()> {
        self.spend(
            state,
            generation,
            &SpendSource::ModelCall {
                call,
                model: model.clone(),
            },
            usage,
        )
    }

    /// Appends the `Proposed` records a crash left out of `generation`'s batch, once per session. `true` when it
    /// appended any, so the caller asks [`next_step`] again.
    fn complete_batch(&mut self, state: &RunState, generation: u32) -> Flow<bool> {
        if self.completed.contains(&generation) {
            return Ok(false);
        }
        let missing = load_batch(self.store, state, &self.run, generation)?
            .map(|(blob, batch)| missing_proposed(state, &blob, &batch))
            .unwrap_or_default();
        let appended = !missing.is_empty();
        for event in missing {
            self.record(generation, event)?;
        }
        self.completed.insert(generation);
        Ok(appended)
    }

    /// Runs trial `trial` of `exp`, proposed in `generation`, and records it, between the launch and after-trial
    /// safe points.
    fn run_trial(
        &mut self,
        state: &RunState,
        generation: u32,
        exp: ExperimentId,
        trial: u32,
    ) -> Flow<()> {
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
        self.safe_point(SafePoint::Launch, generation)?;
        let Some(result) = self.run_job(state, &request, generation)? else {
            return Ok(());
        };
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
        self.safe_point(SafePoint::AfterTrial, generation)
    }

    /// Concludes `generation` with the drop count its batch recorded, 0 when it has no batch.
    fn conclude_generation(&mut self, state: &RunState, generation: u32) -> Flow<()> {
        let dropped = load_batch(self.store, state, &self.run, generation)?
            .map_or(0, |(_, batch)| batch.dropped());
        self.select(state, generation, dropped)
    }

    /// Appends `generation`'s `Selected` as [`conclude`] decides it.
    fn select(&mut self, state: &RunState, generation: u32, dropped: u32) -> Flow<()> {
        let decision = conclude(
            state,
            self.inputs.manifest,
            &self.inputs.judge,
            generation,
            dropped,
        )
        .map_err(DriverError::Policy)?;
        self.record(
            generation,
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

/// Refuses `result` when its backend class differs from the class of the run's recorded trials (D-11,
/// JUDGE-04). A run with no recorded trial accepts any class; the first `Judged` pins it.
fn pin_backend(state: &RunState, result: &JobResult) -> Result<(), DriverError> {
    let backend = result.backend();
    let found = (backend.backend(), backend.arch());
    match state
        .backend_classes()
        .into_iter()
        .find(|recorded| *recorded != found)
    {
        Some(recorded) => Err(DriverError::BackendChanged { recorded, found }),
        None => Ok(()),
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

/// Preflights `request` through the job runner, before the session starts. Nothing is recorded and nothing is
/// retried: a configuration refusal is [`DriverError::Job`], and a check that could not complete is
/// [`DriverError::Infra`], which the caller may retry by starting the session again.
fn check_job<J: JobRunner>(jobs: &J, request: &JobRequest) -> Result<(), DriverError> {
    jobs.check(request).map_err(|failure| match failure {
        JobFailure::Config { what } => DriverError::Job { what },
        JobFailure::Infra { what } => DriverError::Infra { what },
    })
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
