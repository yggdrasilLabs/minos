//! `Experiment`: a base tree plus a parsed cumulative `Diff`, or the baseline itself (03-CONTEXT D-04).
//!
//! An experiment can only be built from a parsed diff, so it can never touch a non-writable path. A parsed
//! [`Diff`] is never empty, so the baseline is the only experiment without a diff, and it has its own constructor
//! and its own ID, [`ExperimentId::baseline`]. A candidate's diff must be cumulative from the run's base tree: the
//! output of [`compose`](crate::diff::compose::compose) or [`diff_trees`](crate::diff::compose::diff_trees), which
//! is what [`ExperimentId::of`] hashes.

use std::fmt;

use crate::diff::Diff;
use crate::id::{ExperimentId, TreeId};

/// One experiment: the run's base tree and, for a candidate, its cumulative diff.
///
/// Fields are private and the ID is computed once at construction, so it always matches the base and diff.
/// Debug prints the ID, the base, and whether a diff is present, never diff text.
#[derive(Clone, PartialEq, Eq)]
pub struct Experiment {
    id: ExperimentId,
    base: TreeId,
    diff: Option<Diff>,
}

impl Experiment {
    /// The baseline of a run: the base tree with no diff, under [`ExperimentId::baseline`].
    #[must_use]
    pub fn baseline(base: TreeId) -> Self {
        Self {
            id: ExperimentId::baseline(&base),
            base,
            diff: None,
        }
    }

    /// A candidate: the base tree and its cumulative diff, under [`ExperimentId::of`].
    ///
    /// `diff` must be cumulative from `base` (see the [module docs](self)).
    #[must_use]
    pub fn candidate(base: TreeId, diff: Diff) -> Self {
        Self {
            id: ExperimentId::of(&base, &diff),
            base,
            diff: Some(diff),
        }
    }

    /// The experiment's ID.
    #[must_use]
    pub const fn id(&self) -> ExperimentId {
        self.id
    }

    /// The run's base tree.
    #[must_use]
    pub const fn base(&self) -> TreeId {
        self.base
    }

    /// The cumulative diff, or `None` for the baseline.
    #[must_use]
    pub const fn diff(&self) -> Option<&Diff> {
        self.diff.as_ref()
    }

    /// Whether this is the baseline.
    #[must_use]
    pub const fn is_baseline(&self) -> bool {
        self.diff.is_none()
    }
}

impl fmt::Debug for Experiment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Experiment")
            .field("id", &self.id)
            .field("base", &self.base)
            .field("has_diff", &self.diff.is_some())
            .finish()
    }
}
