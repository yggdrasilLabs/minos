//! `minos check`: report what a project will run with, before any model is paid for (CLI-05, JUDGE-02; 08-CONTEXT
//! D-05 to D-09).
//!
//! The caller has already prepared the project ([`crate::project::prepare`], which refuses a missing or stale lock,
//! image, or judge push naming the command to run; D-06) and run the provider preflight, whose result it passes in
//! as a [`ProviderStatus`]. [`check`] then prints, in order, every line through [`write_line`]:
//!
//! 1. the project's name and manifest ID, the base tree with its packed size and one line per denylist drop, the
//!    root image, and the judge version with its label and recorded extent;
//! 2. the memory budget, `memory = guest reserve + scratch + candidate allowance` (05-CONTEXT D-05, D-06), and the
//!    judge-scratch requirement the runner checks from the lock's judge extent (07-REVIEW CR-01; D-09);
//! 3. the route table, one row per declared proposer with its model and its eligibility or reason (D-09);
//! 4. for each eligible `single_shot` proposer whose `batch` full-file proposals may reach the `claude` CLI's
//!    default output cap, an advisory (D-08);
//! 5. the provider status; only a refusal makes [`CheckOutcome::refused`] true (D-06);
//! 6. the baseline's diagnostic run (D-05). Without `--fresh`, the latest run by `(started_at, RunId)` whose
//!    generation-0 manifest, base tree, root image, and baseline judge version equal the current ones, whose
//!    `[judge] trials` baseline trials are all judged, and whose baseline is [`baseline_ready`], is reused and no VM
//!    is launched. Otherwise, or with `--fresh`, a new run is named by `RunId::of(manifest, base, started_at)`; a
//!    run of that ID already in the store is refused ([`CheckError::RunExists`]) rather than silently rejoined,
//!    because [`Driver::start`] rejoins a run its inputs name. The new run is started with [`Driver::start`] and
//!    calibrated with [`Driver::calibrate`], which judges exactly `[judge] trials` baseline trials (07-CONTEXT D-05,
//!    D-06); a calibration that stops at a safe point is a refusal;
//! 7. from that run's fold, one code path for reuse and for a new run: each baseline trial's objective value or its
//!    outcome word, the noise floor ([`noise_floor`]), `min_improvement` as written with its threshold at the
//!    median, [`noise_check`]'s verdict with advice when it is `warn` or `refuse`, and the run's cost through
//!    [`CostLine`] (D-07, D-09; COST-02).
//!
//! Every advisory, the D-08 batch advisory and the noise verdict alike, is printed and never refuses (D-09; 08-RESEARCH
//! Open Question 2, decided: the verdict is advisory only). Numbers print with `f64`'s `Display`, and dollars only
//! through [`CostLine`], never as an estimate.
//!
//! [`write_line`]: crate::render::write_line
//! [`CostLine`]: minos_core::money::CostLine

use std::io::{self, Write};

use minos_core::id::RunId;
use minos_core::limits::{
    CLI_DEFAULT_OUTPUT_TOKENS, GUEST_RESERVE_MIB, MIN_NOISE_TRIALS, batch_output_tokens,
    judge_scratch_needed_bytes, near_output_cap,
};
use minos_core::manifest::{Eligibility, Tier};
use minos_core::money::CostError;
use minos_core::policy::{NoiseVerdict, baseline_ready, noise_check, noise_floor};
use minos_core::route::{SurfaceStats, route_table};
use minos_core::text::SafeText;
use minos_loop::{
    Calibrated, Driver, DriverError, DriverOptions, JobRunner, NoopObserver, RETRY_PAUSE,
};
use minos_propose::{ModelProvider, Proposer};
use minos_store::{Ledger, StoreError};

use crate::address::{AddressError, fold_run};
use crate::project::Prepared;
use crate::render::{IdStyle, cost_line, id_text, outcome_word, stop_reason_name, write_line};

/// Bytes in a MiB.
const MIB: u64 = 1024 * 1024;

/// How `check` runs: whether to force a new diagnostic run, the start time a new run is named by, and the driver's
/// session options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckOptions {
    fresh: bool,
    started_at: u64,
    driver: DriverOptions,
}

impl CheckOptions {
    /// Options for a check at `started_at` (Unix seconds, read by the caller), forcing a new run when `fresh`
    /// (`--fresh`), with the production retry pause.
    #[must_use]
    pub const fn new(fresh: bool, started_at: u64) -> Self {
        Self {
            fresh,
            started_at,
            driver: DriverOptions {
                retry_pause: RETRY_PAUSE,
            },
        }
    }

    /// These options with the driver session options `driver`.
    #[must_use]
    pub const fn with_driver(self, driver: DriverOptions) -> Self {
        Self { driver, ..self }
    }

    /// Whether a new diagnostic run is forced.
    #[must_use]
    pub const fn fresh(&self) -> bool {
        self.fresh
    }

    /// The start time a new run is named by.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// The driver's session options.
    #[must_use]
    pub const fn driver(&self) -> DriverOptions {
        self.driver
    }
}

/// The provider preflight's result, which the caller runs before [`check`] (06-CONTEXT D-01 to D-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    /// No proposer names a model, so no provider is needed.
    NotNeeded,
    /// The preflight passed.
    Ready {
        /// What passed, for example the CLI version.
        description: SafeText,
    },
    /// The preflight refused; `check` exits non-zero.
    Refused {
        /// Why, as the refusal's own text.
        message: SafeText,
    },
}

/// What `check` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckOutcome {
    run: Option<RunId>,
    reused: bool,
    refused: bool,
}

impl CheckOutcome {
    /// The diagnostic run the baseline was reported from, if any.
    #[must_use]
    pub const fn run(&self) -> Option<&RunId> {
        self.run.as_ref()
    }

    /// Whether that run was reused, so no VM was launched.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }

    /// Whether something refused, so `minos check` exits non-zero. An advisory never sets it (D-09).
    #[must_use]
    pub const fn refused(&self) -> bool {
        self.refused
    }
}

/// Why `check` could not finish. Messages carry IDs and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    /// A new run's ID is already taken: it was started in this same second with the same manifest and base.
    #[error("run {run} already exists: it started in this same second; run minos check again")]
    RunExists {
        /// The run.
        run: RunId,
    },
    /// The diagnostic run's ledger could not be read or folded.
    #[error("the diagnostic run could not be read")]
    Address(#[source] AddressError),
    /// The driver refused or failed while calibrating.
    #[error("calibration failed")]
    Driver(#[source] DriverError),
    /// The store could not list its runs.
    #[error("the store in .minos could not be read")]
    Store(#[source] StoreError),
    /// The run's recorded spend could not be priced.
    #[error("the diagnostic run's cost could not be priced")]
    Cost(#[source] CostError),
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Checks the prepared project and writes the report to `out`, in the order the [module docs](self) give.
///
/// The caller passes `Proposer::without_provider()`, because generation 0 never calls a model, and has already run
/// the provider preflight, whose result is `provider`. `jobs` runs the baseline trials of a new diagnostic run; a
/// reused run launches nothing.
///
/// # Errors
///
/// [`CheckError::RunExists`] when a new run's ID is already in the store; [`CheckError::Store`] when the store's
/// runs cannot be listed; [`CheckError::Driver`] when the driver refuses to start or calibration fails, for
/// example [`DriverError::BaselineUnscored`]; [`CheckError::Address`] when the run's ledger does not fold;
/// [`CheckError::Cost`] when its spend cannot be priced; and [`CheckError::Output`] when `out` cannot be written.
pub fn check<P: ModelProvider, J: JobRunner, W: Write>(
    prepared: &Prepared,
    proposer: &Proposer<P>,
    jobs: &J,
    provider: &ProviderStatus,
    options: CheckOptions,
    out: &mut W,
) -> Result<CheckOutcome, CheckError> {
    let mut refused = report_facts(prepared, provider, out)?;

    // 6. The diagnostic run: reused when its frame matches and it is calibrated, else new (D-05).
    let reusable = if options.fresh {
        None
    } else {
        reusable_run(prepared)?
    };
    let reused = reusable.is_some();
    let run = if let Some(run) = reusable {
        say(
            out,
            &format!(
                "baseline: reused run {} (no VM launched)",
                id_text(&run.to_hex(), IdStyle::Short)
            ),
        )?;
        run
    } else {
        let run = RunId::of(
            &prepared.manifest_id(),
            &prepared.base().id(),
            options.started_at,
        );
        if prepared
            .store()
            .runs()
            .map_err(CheckError::Store)?
            .contains(&run)
        {
            return Err(CheckError::RunExists { run });
        }
        say(
            out,
            &format!(
                "baseline: run {}, {} trials (each boots a candidate VM and then a judge VM)",
                id_text(&run.to_hex(), IdStyle::Short),
                prepared.manifest().judge().trials()
            ),
        )?;
        let mut driver = Driver::start(
            prepared.store(),
            proposer,
            jobs,
            NoopObserver,
            prepared.run_inputs(),
            options.driver,
            options.started_at,
        )
        .map_err(CheckError::Driver)?;
        if let Calibrated::Stopped { reason } = driver.calibrate().map_err(CheckError::Driver)? {
            say(
                out,
                &format!("calibration stopped: {}", stop_reason_name(reason)),
            )?;
            refused = true;
        }
        run
    };

    // 7. The calibration, from the run's fold.
    report_calibration(prepared, &run, out)?;
    Ok(CheckOutcome {
        run: Some(run),
        reused,
        refused,
    })
}

/// The latest run, by `(started_at, RunId)`, whose generation-0 manifest, base tree, root image, and baseline judge
/// version equal the prepared ones, whose `[judge] trials` baseline trials are all judged, and whose baseline is
/// [`baseline_ready`]. A run whose ledger cannot be read or folded is skipped: it cannot be reused.
fn reusable_run(prepared: &Prepared) -> Result<Option<RunId>, CheckError> {
    let store = prepared.store();
    let manifest = prepared.manifest();
    let manifest_id = prepared.manifest_id();
    let base = prepared.base().id();
    let root_image = prepared.root_image();
    let judge = prepared.judge();
    let mut latest: Option<(u64, RunId)> = None;
    for run in store.runs().map_err(CheckError::Store)? {
        let Ok(state) = fold_run(store, &run) else {
            continue;
        };
        let matches = *state.manifest_at(0) == manifest_id
            && *state.base() == base
            && state.root_image() == Some(&root_image)
            && (0..manifest.judge().trials())
                .all(|trial| state.judged(state.baseline(), &judge, trial))
            && baseline_ready(&state, manifest, &judge);
        let key = (state.started_at(), run);
        if matches && latest.is_none_or(|best| key > best) {
            latest = Some(key);
        }
    }
    Ok(latest.map(|(_, run)| run))
}

/// Writes step 7 of the [module docs](self) from the fold of `run`.
fn report_calibration<W: Write>(
    prepared: &Prepared,
    run: &RunId,
    out: &mut W,
) -> Result<(), CheckError> {
    let state = fold_run(prepared.store(), run).map_err(CheckError::Address)?;
    let judge = prepared.judge();
    let objective = prepared.manifest().objective();
    let metric = objective.metric();

    for (trial, judgment) in state.trials(state.baseline(), &judge) {
        let outcome = judgment.outcome();
        let line = match outcome.scorecard().and_then(|card| card.value(metric)) {
            Some(value) => format!("trial {trial}: scored {metric} {}", value.value()),
            None => format!("trial {trial}: {}", outcome_word(outcome)),
        };
        say(out, &line)?;
    }

    let floor = noise_floor(&state, &judge, metric);
    match floor.filter(|floor| floor.trials() >= MIN_NOISE_TRIALS) {
        Some(floor) => say(
            out,
            &format!(
                "noise floor: median {}, spread {} over {} trials ({} skipped)",
                floor.median().value(),
                floor.spread(),
                floor.trials(),
                floor.skipped()
            ),
        )?,
        None => say(
            out,
            &format!("noise floor: not measured (fewer than {MIN_NOISE_TRIALS} scored trials)"),
        )?,
    }

    let min_improvement = objective.min_improvement();
    let threshold = match floor {
        None => "no baseline median to apply it at".to_owned(),
        Some(floor) => min_improvement
            .threshold_at(floor.median().value())
            .map_or_else(
                || "the threshold at the median is not finite".to_owned(),
                |threshold| format!("threshold {threshold} at the median"),
            ),
    };
    say(
        out,
        &format!("min_improvement: {min_improvement} ({threshold})"),
    )?;

    let verdict = noise_check(floor.as_ref(), objective);
    let word = match verdict {
        NoiseVerdict::Ok => "ok",
        NoiseVerdict::Warn => "warn",
        NoiseVerdict::Refuse => "refuse",
    };
    say(out, &format!("noise verdict: {word}"))?;
    if verdict != NoiseVerdict::Ok {
        say(
            out,
            "advice: raise min_improvement above the spread, or raise [judge] trials so the floor is measured over \
             more boots",
        )?;
    }

    let cost = cost_line(&state, prepared.prices()).map_err(CheckError::Cost)?;
    say(out, &cost.to_string())
}

/// Writes `line` through [`write_line`].
fn say<W: Write>(out: &mut W, line: &str) -> Result<(), CheckError> {
    write_line(out, line).map_err(CheckError::Output)
}

/// Writes the static report, steps 1 to 5 of the [module docs](self), and returns whether the provider refused.
fn report_facts<W: Write>(
    prepared: &Prepared,
    provider: &ProviderStatus,
    out: &mut W,
) -> Result<bool, CheckError> {
    let manifest = prepared.manifest();
    let short = |hex: String| id_text(&hex, IdStyle::Short);

    // 1. The project, base, image, and judge.
    say(
        out,
        &format!(
            "project: {} (manifest {})",
            manifest.name().as_str(),
            short(prepared.manifest_id().to_hex())
        ),
    )?;
    let pack = prepared.pack();
    say(
        out,
        &format!(
            "base: tree {}, {} files, {} bytes packed",
            short(prepared.base().id().to_hex()),
            pack.files(),
            pack.content_bytes()
        ),
    )?;
    for denied in pack.denied() {
        say(
            out,
            &format!(
                "dropped by the denylist: {} ({})",
                denied.path(),
                denied.rule().pattern()
            ),
        )?;
    }
    say(
        out,
        &format!("root image: {}", short(prepared.root_image().to_hex())),
    )?;
    let extent = prepared.judge_extent();
    say(
        out,
        &format!(
            "judge: version {}, label {}, {} files, {} bytes",
            short(prepared.judge().to_hex()),
            manifest.judge().label(),
            extent.files(),
            extent.bytes()
        ),
    )?;

    // 2. The memory budget and the judge scratch.
    let environment = manifest.environment();
    say(
        out,
        &format!(
            "memory: {} MiB = guest reserve {GUEST_RESERVE_MIB} MiB + scratch {} MiB + candidate allowance {} MiB",
            environment.memory_mib(),
            environment.scratch_mib(),
            environment.candidate_allowance_mib()
        ),
    )?;
    let needed = judge_scratch_needed_bytes(extent.bytes(), extent.files()).div_ceil(MIB);
    let scratch = environment.scratch_mib();
    let over = if needed > u64::from(scratch) {
        ": raise [environment] scratch_mib"
    } else {
        ""
    };
    say(
        out,
        &format!("judge scratch: needs {needed} MiB of {scratch} MiB{over}"),
    )?;

    // 3. The route table.
    say(out, "routes:")?;
    let surface = SurfaceStats::new(prepared.run_inputs().surface_bytes());
    let rows = route_table(manifest, &surface);
    for row in &rows {
        let model = manifest
            .proposers()
            .get(row.index())
            .and_then(|spec| spec.model())
            .map(|model| format!(" {model}"))
            .unwrap_or_default();
        let eligibility = match row.eligibility() {
            Eligibility::Eligible => "eligible".to_owned(),
            Eligibility::Ineligible(reason) => format!("ineligible: {reason}"),
        };
        say(
            out,
            &format!("  {} {}{model}: {eligibility}", row.index(), row.tier()),
        )?;
    }

    // 4. The single-shot output advisory (D-08): advisory only.
    let writable_bytes = manifest
        .writable()
        .iter()
        .filter_map(|path| prepared.base().tree().get(path))
        .fold(0_u64, |total, bytes| {
            total.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
        });
    for row in &rows {
        let eligible_single_shot =
            row.tier() == Tier::SingleShot && row.eligibility() == Eligibility::Eligible;
        let Some(spec) = manifest.proposers().get(row.index()) else {
            continue;
        };
        let tokens = batch_output_tokens(writable_bytes, spec.batch());
        if eligible_single_shot && near_output_cap(tokens) {
            say(
                out,
                &format!(
                    "advisory: single_shot proposer {} may return about {tokens} output tokens per call \
                     ({writable_bytes} writable bytes x batch {}), near the claude CLI's default output cap of \
                     {CLI_DEFAULT_OUTPUT_TOKENS}; a smaller batch or writable surface avoids a truncated reply",
                    row.index(),
                    spec.batch()
                ),
            )?;
        }
    }

    // 5. The provider.
    let refused = match provider {
        ProviderStatus::NotNeeded => {
            say(out, "provider: none needed (no proposer names a model)")?;
            false
        }
        ProviderStatus::Ready { description } => {
            say(out, &format!("provider: {description}"))?;
            false
        }
        ProviderStatus::Refused { message } => {
            say(out, &format!("provider: refused: {message}"))?;
            true
        }
    };
    Ok(refused)
}
