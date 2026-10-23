//! A diff cumulative from the run's base tree, as a type (07-CONTEXT D-12, 03-REVIEW WR-03).
//!
//! An experiment's ID hashes the run's base [`TreeId`] and a diff that turns the base into the candidate
//! ([`ExperimentId::of`]; 03-CONTEXT D-04 amendment, 2026-10-14). A [`Diff`] alone does not say what it is
//! relative to: the sweep renders against the parent tree, a model writes against the parent's state, and
//! [`diff_trees`] accepts any two trees. Before this module, a diff relative to the parent could become an
//! [`ExperimentId`] by mistake, and the ID would name a candidate that was never run (WR-03).
//!
//! [`CumulativeDiff`] closes that. It has private fields and no conversion from a bare [`Diff`], and
//! [`Experiment::candidate`](crate::experiment::Experiment::candidate) takes only it. It is minted in three ways:
//!
//! - [`cumulative`], which re-derives the diff against a [`BaseTree`], the run's base surface under its
//!   [`TreeId`]. A parent tree is a plain [`Tree`], so it cannot be passed where the base handle is required.
//! - [`compose`](super::compose::compose), which applies a parent's `CumulativeDiff` and a proposal to the
//!   [`BaseTree`] and refuses a parent minted against another base.
//! - [`CumulativeDiff::recorded`], the re-read path for a diff loaded from its blob: it accepts the diff only when
//!   it hashes to the experiment ID the ledger recorded for it.
//!
//! [`diff_trees`] keeps its signature and its plain [`Diff`] result on purpose: the sweep's render against the
//! parent tree goes through it, and that diff is relative to the parent, so labelling `diff_trees`' output
//! cumulative would mislabel exactly the case WR-03 is about (07-RESEARCH Pattern 10). The type changes no wire
//! byte and no ID: a `CumulativeDiff` holds the same canonical [`Diff`], and its ID is [`ExperimentId::of`].

use std::collections::BTreeSet;
use std::fmt;

use super::Diff;
use super::apply::Tree;
use super::compose::{ComposeError, diff_trees};
use crate::id::{ExperimentId, TreeId};
use crate::limits::Limits;
use crate::path::RelPath;

/// The run's base surface: the [`Tree`] of the source snapshot whose tarball hashes to [`BaseTree::id`].
///
/// It is the handle every [`CumulativeDiff`] is minted against, so a parent tree, which is a plain [`Tree`],
/// cannot take its place. Debug prints the ID and the file count only, never content.
#[derive(Clone, PartialEq, Eq)]
pub struct BaseTree {
    id: TreeId,
    tree: Tree,
}

impl BaseTree {
    /// The run's base: `tree` is the surface read from the snapshot whose tarball hashes to `id`.
    ///
    /// The caller builds it once per run from the base snapshot; core has no filesystem, so it cannot check that
    /// `tree` came from the tarball `id` names.
    #[must_use]
    pub const fn new(id: TreeId, tree: Tree) -> Self {
        Self { id, tree }
    }

    /// The base tree's ID, the [`TreeId`] every experiment of the run hashes.
    #[must_use]
    pub const fn id(&self) -> TreeId {
        self.id
    }

    /// The base surface.
    #[must_use]
    pub const fn tree(&self) -> &Tree {
        &self.tree
    }
}

impl fmt::Debug for BaseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaseTree")
            .field("id", &self.id)
            .field("files", &self.tree.len())
            .finish()
    }
}

/// A canonical [`Diff`] that is cumulative from the run's base tree, with that tree's ID.
///
/// Fields are private and there is no conversion from a bare [`Diff`]: a value exists only through [`cumulative`],
/// [`compose`](super::compose::compose), or [`CumulativeDiff::recorded`] (see the [module docs](self)). Debug
/// prints the base ID and the diff's Debug, which shows paths and lengths only, never content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CumulativeDiff {
    base: TreeId,
    diff: Diff,
}

impl CumulativeDiff {
    /// Wraps a diff that is cumulative from `base`. Private: only this module's minting paths call it.
    pub(super) const fn minted(base: TreeId, diff: Diff) -> Self {
        Self { base, diff }
    }

    /// A diff re-read from its blob, accepted only if it hashes to `exp`, the experiment the ledger recorded it
    /// under, with the run's base `base`.
    ///
    /// This is the path for a diff loaded back from the store: the loop's parent tree and trial requests, and the
    /// runner's check. A diff that is not the recorded experiment's, a diff relative to the parent included, hashes
    /// to another ID and is refused.
    ///
    /// # Errors
    ///
    /// Returns [`NotRecorded`] naming `exp` if [`ExperimentId::of`]`(&base, &diff)` is not `exp`.
    pub fn recorded(base: TreeId, diff: Diff, exp: &ExperimentId) -> Result<Self, NotRecorded> {
        if ExperimentId::of(&base, &diff) == *exp {
            Ok(Self::minted(base, diff))
        } else {
            Err(NotRecorded { exp: *exp })
        }
    }

    /// The ID of the base tree the diff is cumulative from.
    #[must_use]
    pub const fn base(&self) -> TreeId {
        self.base
    }

    /// The canonical diff.
    #[must_use]
    pub const fn as_diff(&self) -> &Diff {
        &self.diff
    }

    /// The canonical diff, consuming the wrapper.
    #[must_use]
    pub fn into_diff(self) -> Diff {
        self.diff
    }

    /// The experiment this diff is: [`ExperimentId::of`] the base and the diff.
    #[must_use]
    pub fn exp(&self) -> ExperimentId {
        ExperimentId::of(&self.base, &self.diff)
    }
}

/// A re-read diff does not hash to the experiment it was recorded under, so it is refused.
///
/// The value names only the recorded experiment, never diff text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the diff does not hash to experiment {exp}")]
pub struct NotRecorded {
    exp: ExperimentId,
}

impl NotRecorded {
    /// The recorded experiment the diff failed to match.
    #[must_use]
    pub const fn exp(&self) -> ExperimentId {
        self.exp
    }
}

/// The canonical diff that turns the run's base into `candidate`, cumulative from `base` by construction.
///
/// The bytes are exactly [`diff_trees`]`(base.tree(), candidate, writable, limits)`; the result also carries
/// `base.id()`.
///
/// # Errors
///
/// Returns every error of [`diff_trees`], including [`ComposeError::NoChange`] when `candidate` equals the base.
pub fn cumulative(
    base: &BaseTree,
    candidate: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<CumulativeDiff, ComposeError> {
    let diff = diff_trees(base.tree(), candidate, writable, limits)?;
    Ok(CumulativeDiff::minted(base.id(), diff))
}
