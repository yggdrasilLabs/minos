//! `minos run --generations N [--run <prefix>]`: start a new run, or with `--run` resume one, and drive it to the
//! absolute generation N, printing one fixed-width progress line per recorded trial, decision, retry, and generation
//! cost (CLI-06; 08-CONTEXT D-14; 08-RESEARCH Open Question 1).
//!
//! # The progress format
//!
//! Plain text, fixed width, no color (D-14). The formatters below are pure and are the only place a progress line
//! is laid out; `tests/progress_golden.rs` pins them byte for byte. A trial line has six columns separated by one
//! space:
//!
//! | Column | Width | Content |
//! |---|---|---|
//! | generation | `g` and at least two digits | `g03`; a generation past 99 widens on its own |
//! | experiment | 12, left-aligned | the experiment ID's first 12 hex characters |
//! | tier | 11, left-aligned | `sweep`, `single_shot`, or `baseline` in generation 0 |
//! | outcome | 26, left-aligned | `scored`, `rejected:<reason>`, `crashed:<stage>`, `timed_out:<stage>`; 26 fits the widest, `rejected:held_out_mismatch` |
//! | delta | 7, right-aligned | the delta as a signed percent of the incumbent's objective (`+6.2%`), or `n/a` |
//! | elapsed | 7, right-aligned | the candidate VM's plus the judge VM's host-measured time (`1m42s`), or `n/a` |
//!
//! For example `g03 3f9a2c1d7e4b single_shot scored                       +6.2%   1m42s`. The other lines are
//! `g03 proposed single_shot admitted 4 dropped 1`, `g03 selected improved 3f9a2c1d7e4b` (or `kept <incumbent>`,
//! or `no_valid_candidates`), `retrying after an infrastructure failure (1 of 3)`, `g03 stopped user_stop`, and
//! `g03 approx. cost: $0.42 (in 1500 tokens, out 456 tokens)`, the cost line for the run so far, written only by
//! [`CostLine`] (COST-02). No line ends in a space, and every printed line still passes through
//! [`crate::render::write_line`].
//!
//! # The command
//!
//! [`run`] refuses zero generations. Without `--run` it names a new run by `RunId::of(manifest, base,
//! started_at)`, refuses one already in the store ([`RunError::RunExists`]) rather than silently rejoining it, and
//! starts it with [`Driver::start`]. With `--run <prefix>` it resolves the run ([`resolve_run`]) and resumes it with
//! [`Driver::resume`], whose checks stay in force: a changed base, root image, judge version, or experiment frame is
//! [`RunError::ResumeRefused`], naming both values and saying a new run is needed. In both cases N is the absolute
//! generation to reach (08-RESEARCH Open Question 1, decided; CLAUDE.md "The target is absolute"), so a resumed
//! session ends where an uninterrupted one would.
//!
//! The session then calibrates generation 0 (idempotent on a calibrated run) and prints the noise verdict as advice
//! only, never a refusal (08-CONTEXT D-07; 08-RESEARCH Open Question 2, decided), and the cost line for the run so
//! far. Then it drives the run with [`Driver::run_to`]. [`ProgressObserver`] prints one line per recorded effect as
//! the driver records it, re-folding the ledger for a trial's delta percent and its host-measured elapsed time, and
//! one cost line after every concluded generation. The driver's own session-relative stops (a `minos stop`, a cap,
//! the breaker) end the session, and `run` reports where it stopped.
//!
//! The observer and `run`'s own lines share one writer through a private [`RefCell`] handle, because the driver
//! owns its observer for the whole session while `run` still prints between its calls; the borrow is taken for one
//! write at a time and a failed borrow is an I/O error, never a panic.

use std::cell::RefCell;
use std::io::{self, Write};

use minos_core::id::{ExperimentId, RunId};
use minos_core::ledger::{DecisionKind, StopReason};
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_core::metric::Metric;
use minos_core::money::{CostLine, Prices};
use minos_core::policy::{NoiseVerdict, TrialRule};
use minos_core::state::RunState;
use minos_loop::{
    Calibrated, Driver, DriverError, DriverOptions, JobRunner, LoopEvent, Observer, RunEnd,
};
use minos_propose::{ModelProvider, Proposer};
use minos_store::{Ledger, StoreError};

use crate::address::{AddressError, HexPrefix, fold_run, resolve_run};
use crate::project::Prepared;
use crate::render::{
    IdStyle, cost_line, decision_word, elapsed_text, id_text, objective_of, outcome_word,
    percent_of, percent_text, stop_reason_name, trial_elapsed_ms, write_line,
};

/// A trial's progress line: generation, experiment prefix, tier, outcome word, delta percent, and elapsed time, laid
/// out as the [module docs](self) give.
#[must_use]
pub fn trial_line(
    generation: u32,
    exp: &str,
    tier: &str,
    outcome: &str,
    percent: Option<f64>,
    elapsed_ms: Option<u64>,
) -> String {
    let percent = percent_text(percent);
    let elapsed = elapsed_text(elapsed_ms);
    format!("g{generation:02} {exp:<12} {tier:<11} {outcome:<26} {percent:>7} {elapsed:>7}")
}

/// A propose step's line: `g03 proposed single_shot admitted 4 dropped 1`.
#[must_use]
pub fn proposed_line(generation: u32, tier: &str, admitted: u32, dropped: u32) -> String {
    format!("g{generation:02} proposed {tier} admitted {admitted} dropped {dropped}")
}

/// A decision's line: `g03 selected improved 3f9a2c1d7e4b`, or without an experiment
/// `g03 selected no_valid_candidates`.
#[must_use]
pub fn selected_line(generation: u32, decision: &str, exp: Option<&str>) -> String {
    match exp {
        Some(exp) => format!("g{generation:02} selected {decision} {exp}"),
        None => format!("g{generation:02} selected {decision}"),
    }
}

/// An infrastructure retry's line: `retrying after an infrastructure failure (1 of 3)`.
#[must_use]
pub fn retrying_line(streak: u32, max: u32) -> String {
    format!("retrying after an infrastructure failure ({streak} of {max})")
}

/// A stop's line: `g03 stopped user_stop`.
#[must_use]
pub fn stopped_line(generation: u32, reason: &str) -> String {
    format!("g{generation:02} stopped {reason}")
}

/// A generation's cost line for the run so far: `g03 ` and then the [`CostLine`] text.
#[must_use]
pub fn cost_text(generation: u32, cost: &CostLine) -> String {
    format!("g{generation:02} {cost}")
}

/// A generation's cost line for the run `state` so far, or `g<N> cost: unavailable (<why>)` when the recorded spend
/// cannot be priced; a run is never priced at zero.
fn cost_or_unavailable(generation: u32, state: &RunState, prices: &Prices) -> String {
    match cost_line(state, prices) {
        Ok(cost) => cost_text(generation, &cost),
        Err(error) => format!("g{generation:02} cost: unavailable ({error})"),
    }
}

/// The short hex of `exp`.
fn short_exp(exp: &ExperimentId) -> String {
    id_text(&exp.to_hex(), IdStyle::Short)
}

/// The short hex of `run`.
fn short_run(run: &RunId) -> String {
    id_text(&run.to_hex(), IdStyle::Short)
}

// ---------------------------------------------------------------------------------------------------------------
// The progress observer
// ---------------------------------------------------------------------------------------------------------------

/// Prints one progress line per loop event, as the [module docs](self) lay out, through [`write_line`].
///
/// On a trial, a decision, or a baseline trial it re-reads and folds the run's ledger, which already holds the
/// effect the event describes (the driver observes after the record lands), for the trial's delta percent, its
/// host-measured elapsed time, and the run's cost so far. A read or fold failure prints the line with `n/a`
/// columns and never panics. The first write error is kept, later lines are skipped, and [`ProgressObserver::finish`]
/// returns it.
pub struct ProgressObserver<'a, L: Ledger, W: Write> {
    ledger: &'a L,
    run: RunId,
    manifest: &'a Manifest,
    judge: JudgeVersion,
    prices: &'a Prices,
    out: W,
    error: Option<io::Error>,
}

impl<'a, L: Ledger, W: Write> ProgressObserver<'a, L, W> {
    /// An observer of `run` in `ledger`, reducing the objective of `manifest` under `judge`, pricing with `prices`,
    /// and writing to `out`.
    #[must_use]
    pub const fn new(
        ledger: &'a L,
        run: RunId,
        manifest: &'a Manifest,
        judge: JudgeVersion,
        prices: &'a Prices,
        out: W,
    ) -> Self {
        Self {
            ledger,
            run,
            manifest,
            judge,
            prices,
            out,
            error: None,
        }
    }

    /// Ends observation and returns the writer.
    ///
    /// # Errors
    ///
    /// The first error writing a line, if any.
    pub fn finish(self) -> Result<W, io::Error> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.out),
        }
    }

    /// The run's folded state, or `None` when its ledger cannot be read or folded.
    fn state(&self) -> Option<RunState> {
        fold_run(self.ledger, &self.run).ok()
    }

    /// Writes `line`, unless an earlier write failed; keeps the first failure.
    fn emit(&mut self, line: &str) {
        if self.error.is_none()
            && let Err(error) = write_line(&mut self.out, line)
        {
            self.error = Some(error);
        }
    }

    /// The baseline trial's line.
    fn calibrated(&self, trial: u32, outcome: &minos_core::outcome::Outcome) -> String {
        let state = self.state();
        let baseline = state.as_ref().map(RunState::baseline);
        let exp = baseline.map_or_else(|| "n/a".to_owned(), short_exp);
        let elapsed = state
            .as_ref()
            .zip(baseline)
            .and_then(|(state, baseline)| trial_elapsed_ms(state, baseline, trial));
        trial_line(0, &exp, "baseline", &outcome_word(outcome), None, elapsed)
    }

    /// A candidate trial's line: the delta as a percent of the incumbent's reduced objective before `generation`.
    fn judged(
        &self,
        generation: u32,
        exp: &ExperimentId,
        trial: u32,
        tier: &str,
        outcome: &minos_core::outcome::Outcome,
        delta: Option<f64>,
    ) -> String {
        let state = self.state();
        let metric = self.manifest.objective().metric();
        let incumbent = state.as_ref().and_then(|state| {
            objective_of(
                state,
                &self.judge,
                &state.incumbent_before(generation),
                metric,
                TrialRule::ScoredOnly,
            )
        });
        let percent = delta
            .zip(incumbent)
            .and_then(|(delta, incumbent)| percent_of(delta, incumbent.value()));
        let elapsed = state
            .as_ref()
            .and_then(|state| trial_elapsed_ms(state, exp, trial));
        trial_line(
            generation,
            &short_exp(exp),
            tier,
            &outcome_word(outcome),
            percent,
            elapsed,
        )
    }

    /// A decision's line and the cost line for the run so far.
    fn selected(&self, generation: u32, decision: &minos_core::ledger::Decision) -> [String; 2] {
        let exp = match decision.kind() {
            DecisionKind::Improved { winner } => Some(short_exp(winner)),
            DecisionKind::Kept { incumbent } => Some(short_exp(incumbent)),
            DecisionKind::NoValidCandidates => None,
        };
        let line = selected_line(generation, decision_word(decision), exp.as_deref());
        let cost = self.state().map_or_else(
            || format!("g{generation:02} cost: unavailable (the ledger could not be read)"),
            |state| cost_or_unavailable(generation, &state, self.prices),
        );
        [line, cost]
    }
}

impl<L: Ledger, W: Write> Observer for ProgressObserver<'_, L, W> {
    fn observe(&mut self, event: &LoopEvent) {
        let lines = match event {
            LoopEvent::Calibrated { trial, outcome } => vec![self.calibrated(*trial, outcome)],
            LoopEvent::Proposed {
                generation,
                tier,
                admitted,
                dropped,
            } => vec![proposed_line(
                *generation,
                tier.as_str(),
                *admitted,
                *dropped,
            )],
            LoopEvent::Judged {
                generation,
                exp,
                trial,
                tier,
                outcome,
                delta,
            } => vec![self.judged(
                *generation,
                exp,
                *trial,
                tier.as_str(),
                outcome,
                delta.map(Metric::value),
            )],
            LoopEvent::Selected {
                generation,
                decision,
            } => self.selected(*generation, decision).to_vec(),
            LoopEvent::Retrying { streak, max } => vec![retrying_line(*streak, *max)],
            LoopEvent::Stopped { generation, reason } => {
                vec![stopped_line(*generation, stop_reason_name(*reason))]
            }
            _ => Vec::new(),
        };
        for line in &lines {
            self.emit(line);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------------------------------------------

/// What `minos run` was asked to do: the absolute generation to reach, and the run to resume, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    generations: u32,
    resume: Option<HexPrefix>,
}

impl RunOptions {
    /// Run to generation `generations` (`--generations N`), resuming the run `resume` names (`--run <prefix>`) or,
    /// with `None`, starting a new run.
    #[must_use]
    pub const fn new(generations: u32, resume: Option<HexPrefix>) -> Self {
        Self {
            generations,
            resume,
        }
    }

    /// The absolute generation to reach.
    #[must_use]
    pub const fn generations(&self) -> u32 {
        self.generations
    }

    /// The prefix of the run to resume, if any.
    #[must_use]
    pub const fn resume(&self) -> Option<&HexPrefix> {
        self.resume.as_ref()
    }
}

/// How the session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnded {
    /// The last concluded generation reached the target.
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

/// The run a session drove and how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOutcome {
    run: RunId,
    ended: RunEnded,
}

impl RunOutcome {
    /// The run.
    #[must_use]
    pub const fn run(&self) -> &RunId {
        &self.run
    }

    /// How the session ended.
    #[must_use]
    pub const fn ended(&self) -> &RunEnded {
        &self.ended
    }
}

/// Why `run` could not start, resume, or finish. Messages carry IDs and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// `--generations 0` names no generation to reach.
    #[error("--generations must be at least 1")]
    ZeroGenerations,
    /// A new run's ID is already taken: it was started in this same second with the same manifest and base.
    #[error("run {run} already exists: it started in this same second; resume it with --run {run}")]
    RunExists {
        /// The run.
        run: RunId,
    },
    /// The run to resume could not be named.
    #[error("the run to resume could not be named")]
    Address(#[source] AddressError),
    /// The store could not list its runs.
    #[error("the store in .minos could not be read")]
    Store(#[source] StoreError),
    /// The resume was refused because the run's frame changed.
    #[error(
        "cannot resume run {run}: {source}; a changed base, root image, judge version, or experiment frame needs a new run: rerun without --run"
    )]
    ResumeRefused {
        /// The run.
        run: RunId,
        /// The driver's refusal, which names both values.
        source: Box<DriverError>,
    },
    /// The driver refused or failed.
    #[error("driving run {run} failed")]
    Driver {
        /// The run.
        run: RunId,
        /// Why.
        source: Box<DriverError>,
    },
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// A writer over a shared cell, so the observer the driver owns and `run` itself write to one output. The borrow is
/// held for one call; a failed borrow is an I/O error.
struct Shared<'c, T: Write>(&'c RefCell<T>);

impl<T: Write> Shared<'_, T> {
    /// The cell, borrowed for one write.
    fn with<R>(&self, call: impl FnOnce(&mut T) -> io::Result<R>) -> io::Result<R> {
        let mut out = self
            .0
            .try_borrow_mut()
            .map_err(|_borrowed| io::Error::other("the output is already being written"))?;
        call(&mut out)
    }
}

impl<T: Write> Write for Shared<'_, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.with(|out| out.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.with(Write::flush)
    }
}

/// Writes `line` to the shared output through [`write_line`].
fn say<T: Write>(cell: &RefCell<T>, line: &str) -> Result<(), RunError> {
    write_line(&mut Shared(cell), line).map_err(RunError::Output)
}

/// Starts or resumes a run of the prepared project and drives it to the absolute generation
/// [`RunOptions::generations`], writing progress to `out` as the [module docs](self) describe.
///
/// `proposer` proposes from generation 1, and `jobs` runs every trial. `started_at` (Unix seconds, read by the
/// caller) names a new run and is unused on a resume. The noise verdict is printed as advice and never refuses.
///
/// # Errors
///
/// [`RunError::ZeroGenerations`] for a target of 0; [`RunError::RunExists`] when a new run's ID is already in the
/// store; [`RunError::Store`] when the store's runs cannot be listed; [`RunError::Address`] when `--run` names no
/// run or several, or the run's ledger cannot be folded; [`RunError::ResumeRefused`] when the base, root image,
/// judge version, or experiment frame changed since the run started; [`RunError::Driver`] for every other driver
/// refusal or failure, for example [`DriverError::RunLocked`]; and [`RunError::Output`] when `out` cannot be
/// written.
pub fn run<P: ModelProvider, J: JobRunner, W: Write>(
    prepared: &Prepared,
    proposer: &Proposer<P>,
    jobs: &J,
    options: RunOptions,
    started_at: u64,
    driver: DriverOptions,
    out: &mut W,
) -> Result<RunOutcome, RunError> {
    let RunOptions {
        generations: target,
        resume,
    } = options;
    if target == 0 {
        return Err(RunError::ZeroGenerations);
    }
    let store = prepared.store();
    let cell = RefCell::new(out);
    let observer = |run: RunId| {
        ProgressObserver::new(
            store,
            run,
            prepared.manifest(),
            prepared.judge(),
            prepared.prices(),
            Shared(&cell),
        )
    };

    let (run, mut session) = if let Some(prefix) = resume.as_ref() {
        let run = *resolve_run(store, Some(prefix))
            .map_err(RunError::Address)?
            .run();
        let session = Driver::resume(
            store,
            proposer,
            jobs,
            observer(run),
            prepared.run_inputs(),
            driver,
            run,
        )
        .map_err(|source| match source {
            DriverError::BaseMismatch { .. }
            | DriverError::RootImageChanged { .. }
            | DriverError::JudgeChanged { .. }
            | DriverError::ManifestChanged { .. } => RunError::ResumeRefused {
                run,
                source: Box::new(source),
            },
            source => RunError::Driver {
                run,
                source: Box::new(source),
            },
        })?;
        say(
            &cell,
            &format!(
                "run {} resumed; target generation {target}",
                short_run(&run)
            ),
        )?;
        (run, session)
    } else {
        let run = RunId::of(&prepared.manifest_id(), &prepared.base().id(), started_at);
        if store.runs().map_err(RunError::Store)?.contains(&run) {
            return Err(RunError::RunExists { run });
        }
        let session = Driver::start(
            store,
            proposer,
            jobs,
            observer(run),
            prepared.run_inputs(),
            driver,
            started_at,
        )
        .map_err(|source| RunError::Driver {
            run,
            source: Box::new(source),
        })?;
        say(
            &cell,
            &format!(
                "run {} started; target generation {target}",
                short_run(&run)
            ),
        )?;
        (run, session)
    };
    let failed = |source: DriverError| RunError::Driver {
        run,
        source: Box::new(source),
    };

    // Generation 0: the noise verdict is advice only (D-07), then the cost so far.
    let ended = match session.calibrate().map_err(failed)? {
        Calibrated::Stopped { reason } => RunEnded::Stopped {
            generation: 0,
            reason,
        },
        Calibrated::Ready(calibration) => {
            let word = match calibration.verdict() {
                NoiseVerdict::Ok => "ok",
                NoiseVerdict::Warn => "warn",
                NoiseVerdict::Refuse => "refuse",
            };
            say(
                &cell,
                &format!("g00 noise verdict: {word} (advisory; minos check explains)"),
            )?;
            let state = fold_run(store, &run).map_err(RunError::Address)?;
            say(
                &cell,
                &cost_or_unavailable(state.last_concluded_generation(), &state, prepared.prices()),
            )?;
            match session.run_to(target).map_err(failed)? {
                RunEnd::Reached { generation } => RunEnded::Reached { generation },
                RunEnd::Stopped { generation, reason } => RunEnded::Stopped { generation, reason },
            }
        }
    };

    // Release the run, then report where the session ended.
    session.into_observer().finish().map_err(RunError::Output)?;
    let last = match ended {
        RunEnded::Reached { generation } => {
            format!("run {} reached generation {generation}", short_run(&run))
        }
        RunEnded::Stopped { generation, reason } => format!(
            "run {} stopped at generation {generation}: {reason}",
            short_run(&run)
        ),
    };
    say(&cell, &last)?;
    Ok(RunOutcome { run, ended })
}
