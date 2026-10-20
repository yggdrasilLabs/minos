//! The strict parser that turns a model's hostile structured output into validated proposals (06-CONTEXT D-10,
//! D-12, PROP-07).
//!
//! A model returns each proposal as `{rationale, files: [{path, content}]}`, the full content of every writable
//! file it changes. [`parse_output`] parses the output into closed types that refuse unknown keys, checks every
//! rule, builds the candidate tree from the parent tree plus the returned files, and derives the canonical diff,
//! cumulative from the base, with [`diff_trees`]. An item that breaks a rule is dropped and counted by its
//! [`DropReason`]; nothing is repaired, trimmed, or normalized. Zero survivors is an empty call, which the loop
//! records as `Selected { NoValidCandidates }`, never an infrastructure failure (D-12).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use minos_core::diff::Diff;
use minos_core::diff::apply::Tree;
use minos_core::diff::compose::{ComposeError, diff_trees};
use minos_core::limits::Limits;
use minos_core::path::RelPath;
use serde::Deserialize;

use crate::limits::{MAX_RATIONALE_BYTES, MAX_RETURNED_FILE_BYTES};
use crate::provider::ModelOutput;

/// Why one proposal, or a whole output, was dropped. A closed set with fixed text; no variant carries model text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, thiserror::Error)]
pub enum DropReason {
    /// The output is not `{"proposals": [...]}` with 1 to `batch` items and no other key.
    #[error("the output is not a list of 1 to batch proposals")]
    OutputShape,
    /// An item is not `{rationale, files: [{path, content}]}` with no other key.
    #[error("a proposal is not a rationale and a list of files")]
    ItemShape,
    /// The rationale is empty, over its cap, or holds a NUL byte.
    #[error("a proposal's rationale is empty, too long, or holds a NUL byte")]
    RationaleInvalid,
    /// The proposal lists no file.
    #[error("a proposal lists no file")]
    NoFiles,
    /// The proposal lists more files than the surface has writable files.
    #[error("a proposal lists more files than are writable")]
    TooManyFiles,
    /// A returned path is not a valid relative path.
    #[error("a returned path is not a valid relative path")]
    PathInvalid,
    /// A returned path is not byte for byte a writable file.
    #[error("a returned path is not a writable file")]
    NotWritable,
    /// A returned path appears twice in one proposal.
    #[error("a returned path appears twice in one proposal")]
    DuplicatePath,
    /// A returned writable path is not in the parent tree.
    #[error("a returned path is not in the parent tree")]
    NotInParent,
    /// Returned content holds a CR or NUL byte.
    #[error("returned content holds a CR or NUL byte")]
    ForbiddenByte,
    /// Returned content is over the per-file cap.
    #[error("returned content is over the per-file cap")]
    FileTooLarge,
    /// The proposal changes nothing against the parent tree.
    #[error("the proposal changes nothing against the parent")]
    SameAsParent,
    /// The proposal returns the tree to the base, which is the baseline, not an experiment.
    #[error("the proposal returns the tree to the base")]
    BackToBase,
    /// The derived diff failed validation, for example over a diff cap.
    #[error("the derived diff failed validation")]
    Compose,
    /// A sweep sample could not be rendered (used by the sweep tier).
    #[error("a sweep sample could not be rendered")]
    Sweep,
    /// The proposal reaches the same tree as an earlier proposal in the same batch.
    #[error("the proposal duplicates an earlier one in the batch")]
    DuplicateInBatch,
}

/// Dropped items counted by reason, in reason order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropCounts {
    counts: BTreeMap<DropReason, u32>,
}

impl DropCounts {
    /// Counts `n` more drops for `reason`, saturating.
    pub fn add(&mut self, reason: DropReason, n: u32) {
        if n == 0 {
            return;
        }
        let count = self.counts.entry(reason).or_insert(0);
        *count = count.saturating_add(n);
    }

    /// The drops counted for `reason`.
    #[must_use]
    pub fn get(&self, reason: DropReason) -> u32 {
        self.counts.get(&reason).copied().unwrap_or(0)
    }

    /// Each reason with a non-zero count, in reason order.
    pub fn iter(&self) -> impl Iterator<Item = (DropReason, u32)> + '_ {
        self.counts.iter().map(|(reason, count)| (*reason, *count))
    }

    /// The total number of drops, saturating: the number the loop records in `Decision::dropped`.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.counts
            .values()
            .fold(0_u32, |total, count| total.saturating_add(*count))
    }
}

/// A validated proposal: a canonical diff cumulative from the base tree, and the model's rationale.
///
/// `Debug` prints the diff's own `Debug` (paths and lengths) and the rationale's length only.
#[derive(Clone, PartialEq, Eq)]
pub struct Proposal {
    diff: Diff,
    rationale: String,
}

impl Proposal {
    /// The proposal `diff` with `rationale`. Crate-private: only the tiers build one, so a test double can hand back
    /// raw output only and every test runs the strict parser.
    pub(crate) fn new(diff: Diff, rationale: String) -> Result<Self, DropReason> {
        check_rationale(&rationale)?;
        Ok(Self { diff, rationale })
    }

    /// The canonical diff, cumulative from the base tree: what `Proposed` records and `ExperimentId::of` hashes.
    #[must_use]
    pub const fn diff(&self) -> &Diff {
        &self.diff
    }

    /// The rationale, 1 to [`MAX_RATIONALE_BYTES`] bytes with no NUL. Model text: shown only through `SafeText`.
    #[must_use]
    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    /// The diff and the rationale, by value.
    #[must_use]
    pub fn into_parts(self) -> (Diff, String) {
        (self.diff, self.rationale)
    }
}

impl fmt::Debug for Proposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proposal")
            .field("diff", &self.diff)
            .field("rationale_bytes", &self.rationale.len())
            .finish()
    }
}

/// What one call's output gave: the surviving proposals, in output order, and the drops by reason.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    proposals: Vec<Proposal>,
    dropped: DropCounts,
}

impl Parsed {
    /// The surviving proposals, in output order.
    #[must_use]
    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    /// The drops by reason.
    #[must_use]
    pub const fn dropped(&self) -> &DropCounts {
        &self.dropped
    }

    /// The proposals and the drop counts, by value.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Proposal>, DropCounts) {
        (self.proposals, self.dropped)
    }
}

/// The canonical diff, cumulative from `base`, that turns `base` into `candidate`, where `candidate` is `parent`
/// with a proposal's edit applied.
///
/// Shared by every tier, so a proposal that equals its parent or returns to the base is dropped the same way
/// whichever tier made it (03-CONTEXT D-04 amendment).
///
/// # Errors
///
/// [`DropReason::SameAsParent`] when `candidate` equals `parent`, [`DropReason::BackToBase`] when it equals
/// `base`, and [`DropReason::Compose`] for every other [`ComposeError`].
pub(crate) fn finish(
    base: &Tree,
    parent: &Tree,
    candidate: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<Diff, DropReason> {
    if candidate == parent {
        return Err(DropReason::SameAsParent);
    }
    diff_trees(base, candidate, writable, limits).map_err(|error| match error {
        ComposeError::NoChange => DropReason::BackToBase,
        _ => DropReason::Compose,
    })
}

/// The whole output, as the model wrote it. Every key is declared and unknown keys are refused.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    proposals: Vec<serde_json::Value>,
}

/// One proposal, as the model wrote it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProposal {
    rationale: String,
    files: Vec<RawFile>,
}

/// One returned file, as the model wrote it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    path: String,
    content: String,
}

/// Parses one call's structured `output` into proposals against `parent`, each with a diff cumulative from
/// `base`.
///
/// The output must be `{"proposals": [...]}` with 1 to `batch` items and no other key; otherwise the call counts
/// one [`DropReason::OutputShape`] drop (or, for an over-batch list, one per item) and yields nothing. Each item
/// is then checked in this order, and its first failing rule decides its one drop: the item shape, the rationale,
/// at least one file, at most `writable.len()` files, and per file in order a valid path, a path byte-equal to a
/// `writable` entry, no repeated path, a path in `parent`, no CR or NUL byte, and the per-file cap; then the
/// candidate (`parent` with every returned file written) must differ from `parent` and from `base` and give a
/// diff within `limits`. A survivor whose diff equals an earlier survivor's is a
/// [`DropReason::DuplicateInBatch`] drop.
#[must_use]
pub fn parse_output(
    output: ModelOutput,
    batch: u32,
    base: &Tree,
    parent: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Parsed {
    let mut parsed = Parsed::default();
    let Ok(raw) = serde_json::from_value::<RawOutput>(output.into_value()) else {
        parsed.dropped.add(DropReason::OutputShape, 1);
        return parsed;
    };
    if raw.proposals.is_empty() {
        parsed.dropped.add(DropReason::OutputShape, 1);
        return parsed;
    }
    let batch_len = usize::try_from(batch).unwrap_or(usize::MAX);
    if raw.proposals.len() > batch_len {
        let count = u32::try_from(raw.proposals.len()).unwrap_or(u32::MAX);
        parsed.dropped.add(DropReason::OutputShape, count);
        return parsed;
    }
    for item in raw.proposals {
        match parse_item(item, base, parent, writable, limits) {
            Ok(proposal) => {
                if parsed
                    .proposals
                    .iter()
                    .any(|earlier| earlier.diff == proposal.diff)
                {
                    parsed.dropped.add(DropReason::DuplicateInBatch, 1);
                } else {
                    parsed.proposals.push(proposal);
                }
            }
            Err(reason) => parsed.dropped.add(reason, 1),
        }
    }
    parsed
}

/// One item into a proposal, or the first rule it breaks.
fn parse_item(
    item: serde_json::Value,
    base: &Tree,
    parent: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<Proposal, DropReason> {
    let raw: RawProposal = serde_json::from_value(item).map_err(|_| DropReason::ItemShape)?;
    check_rationale(&raw.rationale)?;
    if raw.files.is_empty() {
        return Err(DropReason::NoFiles);
    }
    if raw.files.len() > writable.len() {
        return Err(DropReason::TooManyFiles);
    }
    let mut seen = BTreeSet::new();
    let mut candidate = parent.clone();
    for file in raw.files {
        let path = RelPath::try_from(file.path).map_err(|_| DropReason::PathInvalid)?;
        if !writable.contains(&path) {
            return Err(DropReason::NotWritable);
        }
        if seen.contains(&path) {
            return Err(DropReason::DuplicatePath);
        }
        if !parent.contains(&path) {
            return Err(DropReason::NotInParent);
        }
        if file.content.bytes().any(|byte| byte == b'\r' || byte == 0) {
            return Err(DropReason::ForbiddenByte);
        }
        if file.content.len() > MAX_RETURNED_FILE_BYTES {
            return Err(DropReason::FileTooLarge);
        }
        seen.insert(path.clone());
        candidate.insert(path, file.content.into_bytes());
    }
    let diff = finish(base, parent, &candidate, writable, limits)?;
    Proposal::new(diff, raw.rationale)
}

/// A rationale must be 1 to [`MAX_RATIONALE_BYTES`] bytes with no NUL byte.
fn check_rationale(rationale: &str) -> Result<(), DropReason> {
    if rationale.is_empty() || rationale.len() > MAX_RATIONALE_BYTES || rationale.contains('\0') {
        return Err(DropReason::RationaleInvalid);
    }
    Ok(())
}
