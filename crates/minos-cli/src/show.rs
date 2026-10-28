//! `minos show <exp>`: one experiment's header, every trial's outcome with its reason and delta, the lineage from the
//! baseline, the rationale under a fixed cap, and the cumulative diff from the base, with stage logs only behind
//! `--logs` (CLI-07; 08-CONTEXT D-17).
//!
//! [`show`] names the run (the latest when `--run` is omitted) and the experiment by prefix, and prints, in order,
//! every line through [`write_line`]:
//!
//! 1. `experiment <id>`, with ` (baseline)` for the baseline, and `run <id>`;
//! 2. for a proposal, `generation <g>  tier <tier>  origin <sweep | model <model id>>  parent <id>`;
//! 3. one line per trial under the run's judge version: `trial <i>: <outcome word>`, the rejection's reason in
//!    parentheses, the trial's objective as a percent of the incumbent's before its generation (`n/a` for the
//!    baseline or an unscored trial), and the host-measured elapsed time;
//! 4. `lineage: <baseline> -> ... -> <exp>`;
//! 5. for a proposal, `rationale:` and the rationale blob, read under [`MAX_RATIONALE_BYTES`] and rendered only
//!    through [`LogExcerpt::from_bytes`] under [`MAX_LOG_EXCERPT_BYTES`], with a `[truncated]` line when its head
//!    was cut; then `diff:` and the cumulative diff from the base, re-read through [`recorded_diff`] (which accepts
//!    it only when it hashes to the experiment) against the manifest recorded for its generation and the base
//!    rebuilt from the stored base tarball, rendered in full through [`SafeText`] (it is bounded by the diff caps);
//! 6. with `--logs` only, for each trial and each stage log it recorded, `log trial <i> <stage>:` and the log's
//!    tail, read under [`MAX_STAGE_LOG_BYTES`] and rendered only through [`LogExcerpt::from_bytes`].
//!
//! A rationale and a stage log are hostile bytes, from a model and a candidate. Neither reaches the output except
//! through [`LogExcerpt`], which drops control characters and shows invisible format characters as `<U+XXXX>`, and
//! a stage log prints only when asked for, so a hostile log never prints unasked (T-08-09-01, T-08-09-02). Their
//! lines are indented by two spaces, so no rationale or log line can pose as one of the command's own lines. The
//! diff is canonical text Minos derived itself, and it still passes [`SafeText`].
//!
//! `show` reads only: it never appends to the ledger, never takes the run lock, reads nothing from the working
//! tree, and creates nothing.
//!
//! [`recorded_diff`]: minos_loop::history::recorded_diff

use std::io::{self, Write};

use minos_core::id::{BlobId, ExperimentId};
use minos_core::ledger::Origin;
use minos_core::limits::{MAX_LOG_EXCERPT_BYTES, MAX_STAGE_LOG_BYTES};
use minos_core::outcome::Outcome;
use minos_core::policy::TrialRule;
use minos_core::state::Proposal;
use minos_core::text::{LogExcerpt, SafeText};
use minos_loop::DriverError;
use minos_loop::history::{recorded_diff, recorded_manifest_at};
use minos_propose::limits::MAX_RATIONALE_BYTES;
use minos_store::{Blobs, Ledger, StoreError};

use crate::address::{AddressError, HexPrefix, fold_run, resolve_exp, resolve_run};
use crate::project::{PrepareError, base_surface};
use crate::render::{
    IdStyle, elapsed_text, id_text, objective_of, outcome_word, percent_of, percent_text,
    run_judge, trial_elapsed_ms, write_line,
};

/// The indent of a rationale or log line.
const INDENT: &str = "  ";

/// What `show` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShowOptions {
    logs: bool,
    ids: IdStyle,
}

impl ShowOptions {
    /// Options that print the stage logs when `logs` (`--logs`) and IDs in `ids` style (`--full-ids`).
    #[must_use]
    pub const fn new(logs: bool, ids: IdStyle) -> Self {
        Self { logs, ids }
    }
}

/// Why `show` could not print. Messages carry IDs and fixed text only, never blob bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ShowError {
    /// The run or the experiment could not be named, or the run's ledger does not fold.
    #[error("the run or experiment could not be named")]
    Address(#[source] AddressError),
    /// A rationale or log blob could not be read under its cap.
    #[error("a recorded blob could not be read")]
    Store(#[source] StoreError),
    /// The run's base surface could not be rebuilt from its stored base tarball.
    #[error("the run's base could not be read back")]
    Prepare(#[source] Box<PrepareError>),
    /// The experiment's recorded diff or the manifest of its generation could not be read back and checked.
    #[error("the experiment's recorded diff or manifest could not be read back")]
    Record(#[source] Box<DriverError>),
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Writes the experiment `exp` names, in the run `run` names in `store` or the latest run, to `out`, in the order
/// the [module docs](self) give.
///
/// # Errors
///
/// [`ShowError::Address`] when the run or the experiment cannot be named (an absent or ambiguous prefix) or the run
/// does not fold; [`ShowError::Record`] when the manifest recorded for the experiment's generation or its diff
/// cannot be read back and checked; [`ShowError::Prepare`] when the base cannot be rebuilt from its stored tarball;
/// [`ShowError::Store`] when a rationale or log blob is missing, over its cap, or tampered; and
/// [`ShowError::Output`] when `out` cannot be written.
pub fn show<S: Ledger + Blobs, W: Write>(
    store: &S,
    run: Option<&HexPrefix>,
    exp: &HexPrefix,
    options: ShowOptions,
    out: &mut W,
) -> Result<(), ShowError> {
    let ids = options.ids;
    let resolved = resolve_run(store, run).map_err(ShowError::Address)?;
    let state = fold_run(store, resolved.run()).map_err(ShowError::Address)?;
    let exp = resolve_exp(&state, exp).map_err(ShowError::Address)?;
    let id = |exp: &ExperimentId| id_text(&exp.to_hex(), ids);
    let proposal = state.proposal(&exp);
    let generation = proposal.map_or(0, Proposal::generation);
    let manifest = recorded_manifest_at(store, &state, generation).map_err(record)?;

    if proposal.is_some() {
        say(out, &format!("experiment {}", id(&exp)))?;
    } else {
        say(out, &format!("experiment {} (baseline)", id(&exp)))?;
    }
    say(out, &format!("run {}", id_text(&state.run().to_hex(), ids)))?;
    if let Some(proposal) = proposal {
        let origin = match proposal.origin() {
            Origin::Sweep => "sweep".to_owned(),
            Origin::Model { model, .. } => format!("model {model}"),
        };
        say(
            out,
            &format!(
                "generation {generation}  tier {}  origin {origin}  parent {}",
                proposal.tier().as_str(),
                id(proposal.parent())
            ),
        )?;
    }

    let judge = run_judge(&state);
    match judge {
        None => say(out, "trials: the baseline has no single judge version")?,
        Some(judge) => {
            let metric = manifest.objective().metric();
            let incumbent = proposal.and_then(|_| {
                objective_of(
                    &state,
                    &judge,
                    &state.incumbent_before(generation),
                    metric,
                    TrialRule::ScoredOnly,
                )
            });
            let mut any = false;
            for (trial, judgment) in state.trials(&exp, &judge) {
                any = true;
                let outcome = judgment.outcome();
                let reason = match outcome {
                    Outcome::Rejected(reason) => format!(" ({reason})"),
                    _ => String::new(),
                };
                let percent = outcome
                    .scorecard()
                    .and_then(|card| card.value(metric))
                    .zip(incumbent)
                    .and_then(|(value, incumbent)| {
                        percent_of(value.value() - incumbent.value(), incumbent.value())
                    });
                say(
                    out,
                    &format!(
                        "trial {trial}: {}{reason} {} {}",
                        outcome_word(outcome),
                        percent_text(percent),
                        elapsed_text(trial_elapsed_ms(&state, &exp, trial))
                    ),
                )?;
            }
            if !any {
                say(out, "trials: none recorded")?;
            }
        }
    }

    let lineage = state.lineage(&exp).map_or_else(
        || "unknown".to_owned(),
        |chain| chain.iter().map(id).collect::<Vec<_>>().join(" -> "),
    );
    say(out, &format!("lineage: {lineage}"))?;

    if let Some(proposal) = proposal {
        say(out, "rationale:")?;
        let bytes = blob(store, proposal.rationale(), MAX_RATIONALE_BYTES)?;
        excerpt(out, &LogExcerpt::from_bytes(&bytes, MAX_LOG_EXCERPT_BYTES))?;

        say(out, "diff:")?;
        let base = base_surface(store, &state, &manifest)
            .map_err(|error| ShowError::Prepare(Box::new(error)))?;
        let diff = recorded_diff(store, &state, &manifest, &base, &exp).map_err(record)?;
        let text = SafeText::from_utf8_lossy(diff.as_diff().as_bytes());
        for line in text.as_str().lines() {
            say(out, line)?;
        }
    }

    if options.logs
        && let Some(judge) = judge
    {
        for (trial, judgment) in state.trials(&exp, &judge) {
            for (stage, log) in judgment.logs().into_iter().flatten() {
                say(out, &format!("log trial {trial} {stage}:"))?;
                let bytes = blob(store, log, MAX_STAGE_LOG_BYTES)?;
                excerpt(out, &LogExcerpt::from_bytes(&bytes, MAX_STAGE_LOG_BYTES))?;
            }
        }
    }
    Ok(())
}

/// The bytes of blob `id`, read under `max`.
fn blob<S: Blobs>(store: &S, id: &BlobId, max: usize) -> Result<Vec<u8>, ShowError> {
    store
        .get(id, u64::try_from(max).unwrap_or(u64::MAX))
        .map_err(ShowError::Store)
}

/// Writes `excerpt` indented, line by line, after a `[truncated]` line when its head was cut.
fn excerpt<W: Write>(out: &mut W, excerpt: &LogExcerpt) -> Result<(), ShowError> {
    if excerpt.is_truncated() {
        say(out, &format!("{INDENT}[truncated]"))?;
    }
    for line in excerpt.text().as_str().lines() {
        say(out, &format!("{INDENT}{line}"))?;
    }
    Ok(())
}

/// A driver error from re-reading a record, boxed.
fn record(error: DriverError) -> ShowError {
    ShowError::Record(Box::new(error))
}

/// Writes one line, mapping a write failure.
fn say<W: Write>(out: &mut W, line: &str) -> Result<(), ShowError> {
    write_line(out, line).map_err(ShowError::Output)
}
