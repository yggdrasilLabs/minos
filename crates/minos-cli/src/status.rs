//! `minos status [--run <prefix>]`: the run's fold plus liveness and health: its ID and start, generations concluded,
//! whether a driver holds the run, the incumbent against the baseline and the noise floor, the cost with token
//! counts, recorded stops, and any ledger problem that needs a human (CLI-07; 08-CONTEXT D-15).
//!
//! [`status`] reads the ledger under the store's shared lock and prints, in order, every line through
//! [`write_line`]:
//!
//! 1. `run <id> started <UTC> (unix <seconds>)`;
//! 2. `generations concluded: <n>`;
//! 3. `driver: running (generation <g> in progress)` or `driver: not running`, from a [`RunLock::lock_run`] probe
//!    whose guard is dropped at once: [`StoreError::RunLocked`] means a driver holds the run;
//! 4. the incumbent's objective against the baseline's median, as a percent, with the noise floor's spread, or
//!    `incumbent: baseline (no generation concluded)`;
//! 5. the run's cost through [`CostLine`], or `cost: unavailable (<why>)` when there is no price table or it cannot
//!    price a recorded spend: a run is never priced at zero (COST-02);
//! 6. one `stop:` line per recorded stop, with its reason's name and its ordinal when it has one;
//! 7. a torn final line, which the store tolerates and the next append truncates.
//!
//! A ledger the store refuses ([`StoreError::Corrupt`], with its 1-based line and kind, or
//! [`StoreError::LedgerTooLarge`]) or one that does not fold is reported as `ledger: ...; the ledger needs a human`,
//! and [`StatusOutcome::is_healthy`] is false, so a damaged ledger is never shown as healthy. A
//! [`StoreError::Conflict`] is never visible here: a refused append writes nothing, so the command whose append hit
//! the conflict is the one that reports it.
//!
//! The ledger records no generation target: no event or field carries one, and Phase 8 changes no wire format. So
//! `status` shows the concluded count and, while a driver holds the run, the generation in progress; D-15's "target
//! when the session recorded one" never applies to this milestone's ledger.
//!
//! `status` never appends to the ledger and never creates a run. Its liveness probe opens the run's `driver.lock`
//! exactly as a driver does (mode 0600, never truncated), so it creates that empty file when no driver has locked
//! the run before; it writes nothing else. The probe holds the lock for an instant, and a driver starting in that
//! instant is refused with [`StoreError::RunLocked`], whose message says to wait (08-RESEARCH Pitfall 7).
//!
//! [`CostLine`]: minos_core::money::CostLine

use std::io::{self, Write};

use minos_core::id::RunId;
use minos_core::money::Prices;
use minos_core::policy::{TrialRule, noise_floor};
use minos_core::state::{RunState, fold};
use minos_loop::DriverError;
use minos_loop::history::recorded_manifest;
use minos_store::{Blobs, Ledger, RunLock, StoreError};

use crate::address::{AddressError, HexPrefix, resolve_run};
use crate::render::{
    IdStyle, cost_line, id_text, objective_of, percent_of, percent_text, run_judge,
    stop_reason_name, utc_text, working_generation, write_line,
};

/// How `status` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusOptions {
    ids: IdStyle,
}

impl StatusOptions {
    /// Options that print IDs in `ids` style (`--full-ids` is [`IdStyle::Full`]).
    #[must_use]
    pub const fn new(ids: IdStyle) -> Self {
        Self { ids }
    }
}

/// What `status` found.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusOutcome {
    healthy: bool,
}

impl StatusOutcome {
    /// Whether the run's ledger was read and folded. False when the ledger is corrupt, too large, or does not
    /// fold: it needs a human, and the caller exits non-zero.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.healthy
    }
}

/// Why `status` could not report. Messages carry IDs and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    /// The run could not be named.
    #[error("the run could not be named")]
    Address(#[source] AddressError),
    /// The store could not be read or the run's lock probed.
    #[error("the store in .minos could not be read")]
    Store(#[source] StoreError),
    /// The manifest the run has in effect could not be read back from its blob.
    #[error("the run's recorded manifest could not be read")]
    Record(#[source] Box<DriverError>),
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Writes the status of the run `run` names in `store`, or of the latest run, to `out`, in the order the
/// [module docs](self) give. `prices` is the parsed `prices.toml`, or `None` when there is none.
///
/// # Errors
///
/// [`StatusError::Address`] when the run cannot be named (no runs, an absent or ambiguous prefix, or no readable
/// run to pick as the latest); [`StatusError::Store`] when the ledger cannot be read for another reason than a
/// refusal, or the lock probe fails other than by finding the run held; [`StatusError::Record`] when the run's
/// recorded manifest cannot be read back; and [`StatusError::Output`] when `out` cannot be written. A corrupt
/// ledger is not an error: it is reported, and the outcome is unhealthy.
pub fn status<S: Ledger + Blobs + RunLock, W: Write>(
    store: &S,
    run: Option<&HexPrefix>,
    prices: Option<&Prices>,
    options: StatusOptions,
    out: &mut W,
) -> Result<StatusOutcome, StatusError> {
    let ids = options.ids;
    let resolved = resolve_run(store, run).map_err(StatusError::Address)?;
    for skipped in resolved.unreadable() {
        say(
            out,
            &format!(
                "warning: run {}: ledger could not be read",
                id_text(&skipped.to_hex(), ids)
            ),
        )?;
    }
    let run = *resolved.run();
    let run_text = id_text(&run.to_hex(), ids);

    let read = match store.read(&run) {
        Ok(read) => read,
        Err(StoreError::Corrupt { line, kind, .. }) => {
            let problem =
                format!("ledger: corrupt at line {line} ({kind}); the ledger needs a human");
            return unhealthy(store, &run, &run_text, &problem, out);
        }
        Err(StoreError::LedgerTooLarge { bytes, max, .. }) => {
            let problem = format!(
                "ledger: {bytes} bytes, over the {max}-byte cap, so it was not read; the ledger needs a human"
            );
            return unhealthy(store, &run, &run_text, &problem, out);
        }
        Err(error) => return Err(StatusError::Store(error)),
    };
    let state = match fold(read.records()) {
        Ok(state) => state,
        Err(error) => {
            let problem = format!("ledger: does not fold ({error}); the ledger needs a human");
            return unhealthy(store, &run, &run_text, &problem, out);
        }
    };

    say(
        out,
        &format!(
            "run {run_text} started {} (unix {})",
            utc_text(state.started_at()),
            state.started_at()
        ),
    )?;
    say(
        out,
        &format!(
            "generations concluded: {}",
            state.last_concluded_generation()
        ),
    )?;
    let driver = if driver_holds(store, &run)? {
        format!(
            "driver: running (generation {} in progress)",
            working_generation(&state)
        )
    } else {
        "driver: not running".to_owned()
    };
    say(out, &driver)?;
    say(out, &incumbent_line(store, &state, ids)?)?;
    let cost = match prices {
        None => "cost: unavailable (no price table was read)".to_owned(),
        Some(prices) => match cost_line(&state, prices) {
            Ok(line) => line.to_string(),
            Err(error) => format!("cost: unavailable ({error})"),
        },
    };
    say(out, &cost)?;
    for stop in state.stops() {
        let ordinal = stop
            .ordinal()
            .map_or_else(String::new, |ordinal| format!(" ordinal {ordinal}"));
        say(
            out,
            &format!(
                "stop: g{} {}{ordinal}",
                stop.generation(),
                stop_reason_name(stop.reason())
            ),
        )?;
    }
    if read.torn_tail() > 0 {
        say(
            out,
            &format!(
                "ledger: {} torn line after the last complete line; the next append truncates it",
                read.torn_tail()
            ),
        )?;
    }
    Ok(StatusOutcome { healthy: true })
}

/// Writes the run, the driver line, and `problem` for a ledger that cannot be shown, and returns unhealthy.
fn unhealthy<S: RunLock, W: Write>(
    store: &S,
    run: &RunId,
    run_text: &str,
    problem: &str,
    out: &mut W,
) -> Result<StatusOutcome, StatusError> {
    say(out, &format!("run {run_text}"))?;
    let driver = if driver_holds(store, run)? {
        "driver: running"
    } else {
        "driver: not running"
    };
    say(out, driver)?;
    say(out, problem)?;
    Ok(StatusOutcome { healthy: false })
}

/// Whether a driver holds `run`: a [`RunLock::lock_run`] probe whose guard is dropped at once.
fn driver_holds<S: RunLock>(store: &S, run: &RunId) -> Result<bool, StatusError> {
    match store.lock_run(run) {
        Ok(guard) => {
            drop(guard);
            Ok(false)
        }
        Err(StoreError::RunLocked { .. }) => Ok(true),
        Err(error) => Err(StatusError::Store(error)),
    }
}

/// The incumbent's line: its objective against the baseline's median and the noise floor's spread.
fn incumbent_line<S: Blobs>(
    store: &S,
    state: &RunState,
    ids: IdStyle,
) -> Result<String, StatusError> {
    if state.decisions().next().is_none() {
        return Ok("incumbent: baseline (no generation concluded)".to_owned());
    }
    let incumbent = state.incumbent();
    let exp = id_text(&incumbent.to_hex(), ids);
    let manifest =
        recorded_manifest(store, state).map_err(|error| StatusError::Record(Box::new(error)))?;
    let metric = manifest.objective().metric();
    let Some(judge) = run_judge(state) else {
        return Ok(format!(
            "incumbent: {exp} (the baseline has no single judge version)"
        ));
    };
    let value = objective_of(state, &judge, &incumbent, metric, TrialRule::ScoredOnly);
    let value_text = value.map_or_else(|| "n/a".to_owned(), |value| value.value().to_string());
    let baseline = match noise_floor(state, &judge, metric) {
        None => "no scored baseline trial".to_owned(),
        Some(floor) => {
            let median = floor.median().value();
            let percent = value.and_then(|value| percent_of(value.value() - median, median));
            format!(
                "baseline median {median} over {} trials, {}; noise floor spread {}",
                floor.trials(),
                percent_text(percent),
                floor.spread()
            )
        }
    };
    Ok(format!(
        "incumbent: {exp} {metric} {value_text} ({baseline})"
    ))
}

/// Writes one line, mapping a write failure.
fn say<W: Write>(out: &mut W, line: &str) -> Result<(), StatusError> {
    write_line(out, line).map_err(StatusError::Output)
}
