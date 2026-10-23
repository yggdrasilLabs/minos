//! Cumulative diffs from the base tree: a child's diff is the parent's cumulative diff plus the proposal,
//! re-derived as one canonical diff against the base (03-CONTEXT D-04 amendment, 2026-10-14).
//!
//! A proposal is written against the parent's state, because that is what the model saw. [`compose`] applies the
//! parent's cumulative diff and then the proposal to the base surface, and [`diff_trees`] re-derives the result
//! from file contents alone: `diffy::create_patch` with its default three lines of context, per changed file, in
//! path order, then [`Diff::parse`]. Two consequences follow:
//!
//! - Equal candidate trees give byte-identical diffs, so two proposals that reach the same tree get the same
//!   [`ExperimentId::of`](crate::id::ExperimentId::of), however their hunks were written.
//! - The cumulative diff always carries context, so the patch `minos adopt` writes stays `git apply` compatible.
//!
//! A proposal that returns the tree to the base is [`ComposeError::NoChange`], so the baseline can never be
//! re-proposed as an experiment (D-03, D-04). Every [`ComposeError`] is a candidate fault: the proposal is dropped,
//! never retried.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::apply::{ApplyError, Tree, apply};
use super::cumulative::{BaseTree, CumulativeDiff, cumulative};
use super::{Diff, DiffError, NO_NEWLINE_MARKER};
use crate::limits::Limits;
use crate::path::RelPath;

/// Why a proposal could not become a cumulative diff. A candidate fault: the proposal is dropped.
///
/// Every field is a validated path or a typed error; no variant carries file content or diff text.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComposeError {
    /// The parent's diff or the proposal did not apply.
    #[error(transparent)]
    Apply(#[from] ApplyError),
    /// A changed file is not UTF-8, so it cannot be carried in a diff.
    #[error("{path} changed but is not UTF-8 text")]
    NotText {
        /// The changed file.
        path: RelPath,
    },
    /// The candidate tree does not have exactly the base tree's paths.
    #[error("{path} is in only one of the base and candidate trees")]
    PathSetChanged {
        /// The first path, in path order, present in only one tree.
        path: RelPath,
    },
    /// The candidate tree equals the base tree.
    #[error("the candidate is identical to the base")]
    NoChange,
    /// The re-derived diff failed validation, for example a cap or a non-writable path.
    #[error(transparent)]
    Invalid(#[from] DiffError),
    /// The parent's diff is cumulative from a base tree other than the run's (07-CONTEXT D-12).
    #[error("the parent's diff is cumulative from another base tree")]
    BaseMismatch,
}

/// The canonical diff that turns `base` into `candidate`, cumulative from `base`.
///
/// Both trees must have the same paths. Each file whose bytes differ, in path order, must be UTF-8 on both sides
/// and becomes one section with diffy's default context; the text then goes through [`Diff::parse`] with
/// `writable` and `limits`, so the result is a validated, canonical [`Diff`].
///
/// The result is cumulative only when `base` is the run's base tree, which is what
/// [`cumulative`](super::cumulative::cumulative) guarantees by taking a [`BaseTree`]; a diff against any other tree,
/// the sweep's render against the parent included, stays a plain [`Diff`] (07-CONTEXT D-12).
///
/// # Errors
///
/// Returns [`ComposeError::PathSetChanged`] if the path sets differ, [`ComposeError::NotText`] if a changed file
/// is not UTF-8, [`ComposeError::NoChange`] if no file changed, and [`ComposeError::Invalid`] if the diff fails
/// [`Diff::parse`].
pub fn diff_trees(
    base: &Tree,
    candidate: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<Diff, ComposeError> {
    if let Some(path) = first_unshared_path(base, candidate) {
        return Err(ComposeError::PathSetChanged { path: path.clone() });
    }
    let mut text = String::new();
    for ((path, old), (_, new)) in base.iter().zip(candidate.iter()) {
        if old == new {
            continue;
        }
        let not_text = || ComposeError::NotText { path: path.clone() };
        let old = std::str::from_utf8(old).map_err(|_| not_text())?;
        let new = std::str::from_utf8(new).map_err(|_| not_text())?;
        write_section(&mut text, path, &diffy::create_patch(old, new));
    }
    if text.is_empty() {
        return Err(ComposeError::NoChange);
    }
    Ok(Diff::parse(&text, writable, limits)?)
}

/// The cumulative diff for `proposal`, written against the state `parent` produces from the run's base (or
/// against the base itself when there is no parent).
///
/// `parent` is the parent experiment's [`CumulativeDiff`], minted against the same [`BaseTree`]. The result's bytes
/// are [`diff_trees`] of the base and the applied candidate, so they equal what the untyped composition gave.
///
/// # Errors
///
/// Returns [`ComposeError::BaseMismatch`] if `parent` is cumulative from another base tree,
/// [`ComposeError::Apply`] if the parent's diff or the proposal does not apply, and any error of [`diff_trees`],
/// including [`ComposeError::NoChange`] when the result equals the base.
pub fn compose(
    base: &BaseTree,
    parent: Option<&CumulativeDiff>,
    proposal: &Diff,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<CumulativeDiff, ComposeError> {
    let candidate = match parent {
        Some(parent) if parent.base() != base.id() => return Err(ComposeError::BaseMismatch),
        Some(parent) => apply(&apply(base.tree(), parent.as_diff())?, proposal)?,
        None => apply(base.tree(), proposal)?,
    };
    cumulative(base, &candidate, writable, limits)
}

/// The first path, in path order, that is in only one of the two trees.
fn first_unshared_path<'a>(base: &'a Tree, candidate: &'a Tree) -> Option<&'a RelPath> {
    let mut left = base.iter().map(|(path, _)| path).peekable();
    let mut right = candidate.iter().map(|(path, _)| path).peekable();
    loop {
        match (left.peek(), right.peek()) {
            (None, None) => return None,
            (Some(path), None) | (None, Some(path)) => return Some(path),
            (Some(old), Some(new)) => match old.cmp(new) {
                Ordering::Equal => {
                    left.next();
                    right.next();
                }
                Ordering::Less => return Some(old),
                Ordering::Greater => return Some(new),
            },
        }
    }
}

/// Appends one section: the three canonical header lines, then diffy's hunks with explicit lengths.
///
/// The text is rendered from diffy's parsed structure rather than its `Display`, so no formatter option decides
/// the bytes; [`Diff::parse`] then validates and canonicalizes them.
fn write_section(out: &mut String, path: &RelPath, patch: &diffy::Patch<'_, str>) {
    let path = path.as_str();
    for piece in [
        "diff --git a/",
        path,
        " b/",
        path,
        "\n--- a/",
        path,
        "\n+++ b/",
        path,
        "\n",
    ] {
        out.push_str(piece);
    }
    for hunk in patch.hunks() {
        let (old, new) = (hunk.old_range(), hunk.new_range());
        for (part, number) in [
            ("@@ -", old.start()),
            (",", old.len()),
            (" +", new.start()),
            (",", new.len()),
        ] {
            out.push_str(part);
            out.push_str(&number.to_string());
        }
        out.push_str(" @@\n");
        for line in hunk.lines() {
            let (sign, content) = match line {
                diffy::Line::Context(content) => (' ', *content),
                diffy::Line::Delete(content) => ('-', *content),
                diffy::Line::Insert(content) => ('+', *content),
            };
            out.push(sign);
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
                out.push_str(NO_NEWLINE_MARKER);
                out.push('\n');
            }
        }
    }
}
