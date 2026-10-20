//! The sweep tier (PROP-10): seeded sampling over the manifest's `[[param]]` tables, with no model.
//!
//! [`propose`] takes no provider, so it cannot call a model. It draws `batch` samples from
//! [`minos_core::sweep::sample`], seeded by the run and the generation, renders each against the parent tree
//! (06-CONTEXT D-05: a sweep after a model improvement keeps the improvement), and turns the result into a diff
//! cumulative from the base through the same helper the model tiers use, so a sample that equals its parent or
//! restores the base is dropped exactly as a model proposal would be. The same run and generation always give the
//! same diffs in the same order.

use std::collections::BTreeMap;

use minos_core::diff::apply::apply;
use minos_core::ledger::Origin;
use minos_core::limits::Limits;
use minos_core::manifest::{ProposerSpec, Tier};
use minos_core::sweep::{SweepError, render, sample};

use crate::limits::MAX_RATIONALE_CHARS;
use crate::prompt::cut;
use crate::proposal::{DropCounts, DropReason, Proposal, finish};
use crate::proposer::{Proposals, ProposeError, ProposeInput, check_input};

/// The sweep's proposals for `input`'s run and generation.
///
/// Each proposal's rationale is the sample's `name=value` description, cut at a char boundary to at most
/// [`MAX_RATIONALE_CHARS`] bytes, which is at most that many characters: Minos's own text, not model output, so
/// cutting it is not a repair. A sample that renders to the parent is a [`DropReason::SameAsParent`] drop, one that
/// cannot be rendered a [`DropReason::Sweep`] drop, one that restores the base a [`DropReason::BackToBase`] drop,
/// and one whose diff equals an earlier survivor's a [`DropReason::DuplicateInBatch`] drop. The result has
/// `Origin::Sweep`, no usage, and no models.
///
/// # Errors
///
/// [`ProposeError::TierMismatch`] when `spec` is not a sweep, [`ProposeError::Generation0`] for generation 0,
/// [`ProposeError::TreeMismatch`] when the base and parent hold different paths, and [`ProposeError::Sweep`] when
/// no samples can be drawn (the manifest declares no params).
pub fn propose(spec: &ProposerSpec, input: &ProposeInput<'_>) -> Result<Proposals, ProposeError> {
    check_input(Tier::Sweep, spec.tier(), input)?;
    let manifest = input.manifest;
    let params = manifest.params();
    let writable = manifest.writable();
    let limits = Limits::standard();
    let samples =
        sample(input.run, input.generation, params, spec.batch()).map_err(ProposeError::Sweep)?;

    let mut items: Vec<Proposal> = Vec::new();
    let mut dropped = DropCounts::default();
    for drawn in &samples {
        let rendered = render(input.parent, params, drawn, writable, &limits)
            .map_err(|error| match error {
                SweepError::NoChange => DropReason::SameAsParent,
                _ => DropReason::Sweep,
            })
            .and_then(|step| apply(input.parent, &step).map_err(|_| DropReason::Compose))
            .and_then(|candidate| finish(input.base, input.parent, &candidate, writable, &limits))
            .and_then(|diff| {
                let description = drawn.describe();
                Proposal::new(diff, cut(&description, MAX_RATIONALE_CHARS).to_owned())
            });
        match rendered {
            Ok(proposal)
                if items
                    .iter()
                    .any(|earlier| earlier.diff() == proposal.diff()) =>
            {
                dropped.add(DropReason::DuplicateInBatch, 1);
            }
            Ok(proposal) => items.push(proposal),
            Err(reason) => dropped.add(reason, 1),
        }
    }

    Ok(Proposals::new(
        Tier::Sweep,
        items,
        dropped,
        None,
        None,
        BTreeMap::new(),
        Origin::Sweep,
    ))
}
