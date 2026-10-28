//! Naming runs and experiments by unique lowercase hex prefix, git-style (08-CONTEXT D-10).
//!
//! Every command that takes a run (`status`, `log`, `stop`, and `show`, `report`, `adopt` through `--run`) resolves
//! it here, and `show`, `report`, and `adopt` resolve their experiment in that run. A prefix must match exactly one
//! ID: an absent prefix and an ambiguous one are named errors, and an ambiguous one lists every candidate's full ID,
//! so a prefix never silently picks the wrong run. With no prefix, the run is the latest by `(started_at, RunId)`.
//!
//! Resolution by name never reads or creates a run: it lists names through [`Ledger::runs`] and matches hex, and
//! only the latest-run choice reads ledgers, through [`Ledger::read`]. Nothing here takes a run lock, because
//! `FileStore::lock_run` creates the run's directory (08-RESEARCH Pitfall 8).

use std::fmt;
use std::str::FromStr;

use minos_core::id::{ExperimentId, RunId};
use minos_core::state::{FoldError, RunState, fold};
use minos_core::text::SafeText;
use minos_store::{Ledger, StoreError};

/// The longest prefix: a whole SHA-256 ID in hex.
const MAX_PREFIX_CHARS: usize = 64;

/// A prefix of an ID's lowercase hex: 1 to 64 characters of `[0-9a-f]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HexPrefix(String);

impl HexPrefix {
    /// The prefix.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the lowercase hex `hex` starts with this prefix.
    #[must_use]
    pub fn matches(&self, hex: &str) -> bool {
        hex.starts_with(self.0.as_str())
    }
}

impl FromStr for HexPrefix {
    type Err = AddressError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let valid = !text.is_empty()
            && text.len() <= MAX_PREFIX_CHARS
            && text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(AddressError::BadPrefix {
                prefix: SafeText::inline(text),
            })
        }
    }
}

impl fmt::Display for HexPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A resolved run, with every run the latest-run choice could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    run: RunId,
    unreadable: Vec<RunId>,
}

impl Resolved {
    /// The run.
    #[must_use]
    pub const fn run(&self) -> &RunId {
        &self.run
    }

    /// The runs whose ledgers could not be read or folded while choosing the latest run, in [`RunId`] order.
    /// Always empty when a prefix was given, because a prefix is matched by name alone.
    #[must_use]
    pub fn unreadable(&self) -> &[RunId] {
        &self.unreadable
    }
}

/// Why a run or an experiment could not be named. Messages carry IDs, validated prefixes, and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    /// The argument is not a lowercase hex prefix.
    #[error("{prefix} is not a lowercase hex prefix of 1 to 64 characters")]
    BadPrefix {
        /// The argument, sanitized.
        prefix: SafeText,
    },
    /// The store has no runs.
    #[error("no runs recorded; run `minos check` or `minos run`")]
    NoRuns,
    /// No run's ID starts with the prefix.
    #[error("no run starts with {prefix}")]
    NoRun {
        /// The prefix.
        prefix: HexPrefix,
    },
    /// No experiment of the run starts with the prefix.
    #[error("no experiment of this run starts with {prefix}")]
    NoExperiment {
        /// The prefix.
        prefix: HexPrefix,
    },
    /// More than one ID starts with the prefix.
    #[error("{prefix} names more than one {what}: {}; give more characters", candidates.join(", "))]
    Ambiguous {
        /// `run` or `experiment`.
        what: &'static str,
        /// The prefix.
        prefix: HexPrefix,
        /// Every matching ID's full hex, sorted.
        candidates: Vec<String>,
    },
    /// The run has no recorded lines.
    #[error("run {run} has no recorded lines")]
    EmptyRun {
        /// The run.
        run: RunId,
    },
    /// No run's ledger could be read, so there is no latest run.
    #[error("no run's ledger could be read or folded: {}", list_runs(runs))]
    Unreadable {
        /// Every run, in order.
        runs: Vec<RunId>,
    },
    /// The store could not be listed or read.
    #[error("the store in .minos could not be read")]
    Store(#[source] StoreError),
    /// The run's ledger does not fold: it is corrupt and needs a human.
    #[error("the ledger of run {run} does not fold")]
    Fold {
        /// The run.
        run: RunId,
        /// Why.
        #[source]
        source: FoldError,
    },
}

/// The runs' hex joined with `, `.
fn list_runs(runs: &[RunId]) -> String {
    runs.iter()
        .map(RunId::to_hex)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The run `prefix` names in `store`, or with no prefix the latest run by `(started_at, RunId)`.
///
/// A prefix is matched against the names [`Ledger::runs`] lists, and no ledger is read. With no prefix, every run
/// is read and folded; a run whose ledger cannot be read or folded is skipped and listed in
/// [`Resolved::unreadable`], so one corrupt run never hides the others.
///
/// # Errors
///
/// [`AddressError::NoRuns`] when the store has no runs, [`AddressError::NoRun`] or [`AddressError::Ambiguous`] when
/// the prefix matches no run or several, [`AddressError::Unreadable`] when no run folds, and
/// [`AddressError::Store`] when the runs cannot be listed.
pub fn resolve_run<L: Ledger>(
    store: &L,
    prefix: Option<&HexPrefix>,
) -> Result<Resolved, AddressError> {
    let runs = store.runs().map_err(AddressError::Store)?;
    if runs.is_empty() {
        return Err(AddressError::NoRuns);
    }
    if let Some(prefix) = prefix {
        let matched: Vec<RunId> = runs
            .into_iter()
            .filter(|run| prefix.matches(&run.to_hex()))
            .collect();
        return match matched.as_slice() {
            [] => Err(AddressError::NoRun {
                prefix: prefix.clone(),
            }),
            [run] => Ok(Resolved {
                run: *run,
                unreadable: Vec::new(),
            }),
            _ => Err(AddressError::Ambiguous {
                what: "run",
                prefix: prefix.clone(),
                candidates: matched.iter().map(RunId::to_hex).collect(),
            }),
        };
    }

    let mut latest: Option<(u64, RunId)> = None;
    let mut unreadable = Vec::new();
    for run in runs {
        match fold_run(store, &run) {
            Ok(state) => {
                let key = (state.started_at(), run);
                if latest.is_none_or(|best| key > best) {
                    latest = Some(key);
                }
            }
            Err(AddressError::EmptyRun { .. }) => {}
            Err(_) => unreadable.push(run),
        }
    }
    match latest {
        Some((_, run)) => Ok(Resolved { run, unreadable }),
        None if unreadable.is_empty() => Err(AddressError::NoRuns),
        None => Err(AddressError::Unreadable { runs: unreadable }),
    }
}

/// The folded state of `run`.
///
/// # Errors
///
/// [`AddressError::Store`] when the ledger cannot be read, [`AddressError::EmptyRun`] when it has no lines, and
/// [`AddressError::Fold`] when it does not fold.
pub fn fold_run<L: Ledger>(store: &L, run: &RunId) -> Result<RunState, AddressError> {
    let read = store.read(run).map_err(AddressError::Store)?;
    if read.lines().is_empty() {
        return Err(AddressError::EmptyRun { run: *run });
    }
    fold(read.records()).map_err(|source| AddressError::Fold { run: *run, source })
}

/// The experiment of the run `state` that `prefix` names: the baseline or a proposal.
///
/// # Errors
///
/// [`AddressError::NoExperiment`] when no experiment matches, and [`AddressError::Ambiguous`] listing every match
/// when several do.
pub fn resolve_exp(state: &RunState, prefix: &HexPrefix) -> Result<ExperimentId, AddressError> {
    let mut matched: Vec<ExperimentId> = std::iter::once(state.baseline())
        .chain(state.proposals().map(|(exp, _)| exp))
        .filter(|exp| prefix.matches(&exp.to_hex()))
        .copied()
        .collect();
    matched.sort();
    match matched.as_slice() {
        [] => Err(AddressError::NoExperiment {
            prefix: prefix.clone(),
        }),
        [exp] => Ok(*exp),
        _ => Err(AddressError::Ambiguous {
            what: "experiment",
            prefix: prefix.clone(),
            candidates: matched.iter().map(ExperimentId::to_hex).collect(),
        }),
    }
}
