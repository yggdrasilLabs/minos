//! Admission: how one propose step's proposals become experiments, and the batch blob that lets a resumed run
//! finish the step exactly as an uninterrupted one would (07-CONTEXT D-07, D-12, D-18; 07-RESEARCH Pattern 6,
//! Pitfall 2).
//!
//! # Admission
//!
//! [`admit`] names each proposal through [`Experiment::candidate`] over its [`CumulativeDiff`], never by hashing a
//! bare diff (D-12). A proposal whose experiment the run already recorded, in an earlier generation or earlier in
//! the same step, is dropped and counted: a second `Proposed` for one experiment with other fields would fail the
//! fold. Each surviving proposal's canonical diff and rationale are stored as blobs.
//!
//! # The batch blob
//!
//! `Proposed` lines are appended one at a time, and the step's drop count is otherwise written only in the
//! generation's `Selected`. A crash between the first `Proposed` and `Selected` would lose the unrecorded items
//! and the drop count, and D-07 forbids proposing the generation again, so the resumed run would differ from an
//! uninterrupted one. So before the first `Proposed`, [`admit`] stores one [`Batch`]: the generation, the parent,
//! the tier, the origin, the total drop count, and every admitted item's experiment, diff blob, and rationale
//! blob, in proposal order. Every `Proposed` of the step names it. On resume, [`load_batch`] reads it back from
//! any recorded `Proposed` of the generation and checks every recorded `Proposed` against it, and
//! [`missing_proposed`] yields the items still to append. A crash before the first `Proposed` leaves no record
//! that names the batch, so the step proposes again under the next model call index (D-07).
//!
//! The blob is versioned JSON whose parser refuses unknown keys, another version, an empty or over-cap item
//! list, a repeated experiment, generation 0, a tier that disagrees with its origin, and text over
//! [`MAX_BATCH_BYTES`], checked before parsing.

use std::collections::BTreeSet;

use minos_core::experiment::Experiment;
use minos_core::id::{BlobId, ExperimentId, RunId};
use minos_core::ledger::{Event, Origin};
use minos_core::limits::BATCH_MAX;
use minos_core::manifest::Tier;
use minos_core::state::RunState;
use minos_propose::Proposal;
use minos_store::{Blobs, StoreError};
use serde::{Deserialize, Serialize};

use crate::byte_cap;
use crate::error::DriverError;

/// The batch blob's wire version, the `v` field.
pub const BATCH_VERSION: u32 = 1;

/// The largest batch blob, in bytes, written or read. At most [`BATCH_MAX`] items of three 64-character IDs, plus
/// the parent, the tier, and the origin, is under 4 KiB; the cap leaves room without letting a tampered blob be
/// large.
pub const MAX_BATCH_BYTES: usize = 65_536;

/// One admitted experiment of a batch: its ID and the blobs of its canonical cumulative diff and its rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchItem {
    exp: ExperimentId,
    diff: BlobId,
    rationale: BlobId,
}

impl BatchItem {
    /// An item for `exp`, whose diff and rationale are stored as `diff` and `rationale`.
    #[must_use]
    pub const fn new(exp: ExperimentId, diff: BlobId, rationale: BlobId) -> Self {
        Self {
            exp,
            diff,
            rationale,
        }
    }

    /// The experiment.
    #[must_use]
    pub const fn exp(&self) -> &ExperimentId {
        &self.exp
    }

    /// The blob of the canonical cumulative diff.
    #[must_use]
    pub const fn diff(&self) -> &BlobId {
        &self.diff
    }

    /// The blob of the rationale.
    #[must_use]
    pub const fn rationale(&self) -> &BlobId {
        &self.rationale
    }
}

/// One propose step's admitted experiments and drop count (see the [module docs](self)).
///
/// Fields are private and every value passed [`Batch::new`], the deserializer included, so if a batch exists:
/// its generation is at least 1, it lists 1 to [`BATCH_MAX`] items with no experiment twice, and its tier agrees
/// with its origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawBatch", into = "RawBatch")]
pub struct Batch {
    generation: u32,
    parent: ExperimentId,
    tier: Tier,
    origin: Origin,
    dropped: u32,
    items: Vec<BatchItem>,
}

impl Batch {
    /// The batch of `generation`, proposed from `parent` by `tier` with `origin`, that dropped `dropped`
    /// proposals and admitted `items`, in proposal order.
    ///
    /// # Errors
    ///
    /// [`BatchError::Generation`] for generation 0, [`BatchError::Empty`] for no item, [`BatchError::TooMany`] for
    /// more than [`BATCH_MAX`] items, [`BatchError::RepeatedExperiment`] when an experiment is listed twice, and
    /// [`BatchError::OriginTier`] when exactly one of the tier and the origin is the sweep.
    pub fn new(
        generation: u32,
        parent: ExperimentId,
        tier: Tier,
        origin: Origin,
        dropped: u32,
        items: Vec<BatchItem>,
    ) -> Result<Self, BatchError> {
        if generation == 0 {
            return Err(BatchError::Generation);
        }
        if items.is_empty() {
            return Err(BatchError::Empty);
        }
        if !u32::try_from(items.len()).is_ok_and(|len| len <= BATCH_MAX) {
            return Err(BatchError::TooMany);
        }
        let mut seen = BTreeSet::new();
        if !items.iter().all(|item| seen.insert(item.exp)) {
            return Err(BatchError::RepeatedExperiment);
        }
        if (tier == Tier::Sweep) != matches!(origin, Origin::Sweep) {
            return Err(BatchError::OriginTier);
        }
        Ok(Self {
            generation,
            parent,
            tier,
            origin,
            dropped,
            items,
        })
    }

    /// The canonical JSON bytes of the batch, as stored.
    ///
    /// # Errors
    ///
    /// [`BatchError::Json`] if serialization fails and [`BatchError::TooLarge`] if the bytes exceed
    /// [`MAX_BATCH_BYTES`].
    pub fn to_json(&self) -> Result<Vec<u8>, BatchError> {
        let bytes = serde_json::to_vec(self).map_err(|_| BatchError::Json)?;
        if bytes.len() > MAX_BATCH_BYTES {
            return Err(BatchError::TooLarge);
        }
        Ok(bytes)
    }

    /// Parses a stored batch, refusing text over [`MAX_BATCH_BYTES`] before reading it.
    ///
    /// # Errors
    ///
    /// [`BatchError::TooLarge`] for over-cap text, [`BatchError::Json`] for text that is not JSON of the batch shape
    /// (an unknown key included), [`BatchError::Version`] for a `v` other than [`BATCH_VERSION`], and every error
    /// of [`Batch::new`].
    pub fn parse_json(bytes: &[u8]) -> Result<Self, BatchError> {
        if bytes.len() > MAX_BATCH_BYTES {
            return Err(BatchError::TooLarge);
        }
        let raw: RawBatch = serde_json::from_slice(bytes).map_err(|_| BatchError::Json)?;
        Self::try_from(raw)
    }

    /// The generation, from 1.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// The experiment the batch was proposed from.
    #[must_use]
    pub const fn parent(&self) -> &ExperimentId {
        &self.parent
    }

    /// The tier that proposed.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The sweep or the model call that produced every item.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The step's total drop count: the proposer's drops and the repeats admission dropped.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }

    /// The admitted items, in proposal order.
    #[must_use]
    pub fn items(&self) -> &[BatchItem] {
        &self.items
    }
}

/// [`Batch`] as written: `{"v":1,"generation":..,"parent":..,"tier":..,"origin":..,"dropped":..,"items":[..]}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBatch {
    v: u32,
    generation: u32,
    parent: ExperimentId,
    tier: Tier,
    origin: Origin,
    dropped: u32,
    items: Vec<RawBatchItem>,
}

/// [`BatchItem`] as written: `{"exp":..,"diff":..,"rationale":..}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBatchItem {
    exp: ExperimentId,
    diff: BlobId,
    rationale: BlobId,
}

impl TryFrom<RawBatch> for Batch {
    type Error = BatchError;

    fn try_from(raw: RawBatch) -> Result<Self, Self::Error> {
        if raw.v != BATCH_VERSION {
            return Err(BatchError::Version);
        }
        let items = raw
            .items
            .into_iter()
            .map(|item| BatchItem::new(item.exp, item.diff, item.rationale))
            .collect();
        Self::new(
            raw.generation,
            raw.parent,
            raw.tier,
            raw.origin,
            raw.dropped,
            items,
        )
    }
}

impl From<Batch> for RawBatch {
    fn from(batch: Batch) -> Self {
        Self {
            v: BATCH_VERSION,
            generation: batch.generation,
            parent: batch.parent,
            tier: batch.tier,
            origin: batch.origin,
            dropped: batch.dropped,
            items: batch
                .items
                .into_iter()
                .map(|item| RawBatchItem {
                    exp: item.exp,
                    diff: item.diff,
                    rationale: item.rationale,
                })
                .collect(),
        }
    }
}

/// Why a batch was refused. Carries no input text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BatchError {
    /// The text is not JSON of the batch shape, or has an unknown key.
    #[error("the batch is not JSON of the batch shape")]
    Json,
    /// The `v` field is not [`BATCH_VERSION`].
    #[error("the batch version is not {BATCH_VERSION}")]
    Version,
    /// The batch lists no item; a step that admits nothing stores no batch.
    #[error("the batch lists no experiment")]
    Empty,
    /// The batch lists more than [`BATCH_MAX`] items.
    #[error("the batch lists more than {BATCH_MAX} experiments")]
    TooMany,
    /// The batch lists one experiment twice.
    #[error("the batch lists an experiment twice")]
    RepeatedExperiment,
    /// The batch's generation is 0, which is baseline calibration.
    #[error("a batch cannot belong to generation 0")]
    Generation,
    /// Exactly one of the tier and the origin is the sweep.
    #[error("the batch's tier disagrees with its origin")]
    OriginTier,
    /// The text is over [`MAX_BATCH_BYTES`].
    #[error("the batch is over its {MAX_BATCH_BYTES}-byte cap")]
    TooLarge,
}

/// What one propose step's records are made of: a parameter bundle the driver builds, not a validated type.
#[derive(Debug, Clone, Copy)]
pub struct AdmitStep<'a> {
    /// The generation being proposed, from 1.
    pub generation: u32,
    /// The incumbent the step proposed from.
    pub parent: ExperimentId,
    /// The tier that proposed.
    pub tier: Tier,
    /// The step's origin.
    pub origin: &'a Origin,
    /// The proposer's drop count for the step.
    pub dropped: u32,
}

/// What admission produced: the stored batch, if any item survived, and the step's total drop count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    batch: Option<(BlobId, Batch)>,
    dropped: u32,
}

impl Admission {
    /// The stored batch and its blob, or `None` when every proposal was dropped and nothing was stored.
    #[must_use]
    pub fn batch(&self) -> Option<(&BlobId, &Batch)> {
        self.batch.as_ref().map(|(blob, batch)| (blob, batch))
    }

    /// The step's total drop count: the proposer's drops and the repeats admission dropped. Equal to the batch's
    /// [`Batch::dropped`] when there is a batch.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }
}

/// Admits `items`, in order, as experiments of `step.generation` (see the [module docs](self)).
///
/// A proposal whose experiment `state` already records, or that an earlier item of `items` already named, is
/// dropped and counted. Every survivor's diff and rationale are stored, then the [`Batch`]; when nothing
/// survives, nothing is stored. The caller appends [`missing_proposed`] for the returned batch.
///
/// # Errors
///
/// [`DriverError::Store`] when a blob cannot be stored, and [`DriverError::Batch`] when the survivors cannot form
/// a batch (more than [`BATCH_MAX`], generation 0, or a tier that disagrees with the origin).
pub fn admit<S: Blobs>(
    store: &S,
    state: &RunState,
    step: AdmitStep<'_>,
    items: Vec<Proposal>,
) -> Result<Admission, DriverError> {
    let run = *state.run();
    let mut dropped = step.dropped;
    let mut seen = BTreeSet::new();
    let mut admitted = Vec::new();
    for proposal in items {
        let (cumulative, rationale) = proposal.into_parts();
        let exp = Experiment::candidate(cumulative.clone()).id();
        if state.proposal(&exp).is_some() || !seen.insert(exp) {
            dropped = dropped.saturating_add(1);
        } else {
            admitted.push((exp, cumulative, rationale));
        }
    }
    if admitted.is_empty() {
        return Ok(Admission {
            batch: None,
            dropped,
        });
    }

    let store_error = |source: StoreError| DriverError::Store { run, source };
    let mut batch_items = Vec::with_capacity(admitted.len());
    for (exp, cumulative, rationale) in admitted {
        let diff = store
            .put(cumulative.as_diff().as_bytes())
            .map_err(store_error)?;
        let rationale = store.put(rationale.as_bytes()).map_err(store_error)?;
        batch_items.push(BatchItem::new(exp, diff, rationale));
    }
    let inconsistent = |_: BatchError| DriverError::Batch {
        run,
        generation: step.generation,
    };
    let batch = Batch::new(
        step.generation,
        step.parent,
        step.tier,
        step.origin.clone(),
        dropped,
        batch_items,
    )
    .map_err(inconsistent)?;
    let blob = store
        .put(&batch.to_json().map_err(inconsistent)?)
        .map_err(store_error)?;
    Ok(Admission {
        batch: Some((blob, batch)),
        dropped,
    })
}

/// The `Proposed` events of `batch`, stored as `batch_blob`, that `state` does not record yet, in batch order.
#[must_use]
pub fn missing_proposed(state: &RunState, batch_blob: &BlobId, batch: &Batch) -> Vec<Event> {
    batch
        .items()
        .iter()
        .filter(|item| state.proposal(item.exp()).is_none())
        .map(|item| Event::Proposed {
            exp: item.exp,
            parent: batch.parent,
            generation: batch.generation,
            tier: batch.tier,
            diff: item.diff,
            rationale: item.rationale,
            origin: batch.origin.clone(),
            batch: Some(*batch_blob),
        })
        .collect()
}

/// The batch of `generation` and its blob, read back through the `Proposed` records that name it, or `None` when
/// the generation records no `Proposed` naming a batch.
///
/// Every recorded `Proposed` of the generation must name the same batch and equal its item there, with the
/// batch's parent, tier, and origin.
///
/// # Errors
///
/// [`DriverError::Batch`] when the generation's `Proposed` records name different batches or only some name one,
/// when the batch is over [`MAX_BATCH_BYTES`], does not parse, or belongs to another generation, or when a
/// recorded `Proposed` disagrees with it; [`DriverError::Store`] when the blob cannot be read.
pub fn load_batch<S: Blobs>(
    store: &S,
    state: &RunState,
    run: &RunId,
    generation: u32,
) -> Result<Option<(BlobId, Batch)>, DriverError> {
    let inconsistent = || DriverError::Batch {
        run: *run,
        generation,
    };
    let named: BTreeSet<Option<BlobId>> = state
        .proposals_in(generation)
        .map(|(_, proposal)| proposal.batch().copied())
        .collect();
    let mut named = named.into_iter();
    let blob = match (named.next(), named.next()) {
        (None | Some(None), None) => return Ok(None),
        (Some(Some(blob)), None) => blob,
        _ => return Err(inconsistent()),
    };

    let bytes = store
        .get(&blob, byte_cap(MAX_BATCH_BYTES))
        .map_err(|source| match source {
            StoreError::BlobTooLarge { .. } => inconsistent(),
            source => DriverError::Store { run: *run, source },
        })?;
    let batch = Batch::parse_json(&bytes).map_err(|_| inconsistent())?;
    if batch.generation() != generation {
        return Err(inconsistent());
    }
    for (exp, proposal) in state.proposals_in(generation) {
        let matches = batch.items().iter().any(|item| {
            item.exp == *exp
                && item.diff == *proposal.diff()
                && item.rationale == *proposal.rationale()
        }) && proposal.parent() == batch.parent()
            && proposal.tier() == batch.tier()
            && proposal.origin() == batch.origin();
        if !matches {
            return Err(inconsistent());
        }
    }
    Ok(Some((blob, batch)))
}
