//! Proposer tier eligibility, with reasons, and routing (CORE-11; 03-CONTEXT D-13).
//!
//! Routing is a pure function of the manifest, the ledger, and the surface size the shell measured: no clock,
//! no randomness, and no state outside the folded [`RunState`]. The ladder is the declared proposers that are
//! eligible, ordered by [`Tier::rank`] and then by declaration index. A generation starts on the lowest rung and
//! moves up one rung for every `stall_step` concluded generations since the last improving `Selected` decision,
//! clamped to the top rung. An improvement resets the stall count, so the cheap tiers get another chance from
//! the new incumbent (D-13).
//!
//! [`route_table`] is what `minos check` prints: every declared proposer with its eligibility, and a reason for
//! each one that cannot propose.

use crate::ledger::DecisionKind;
use crate::limits::SINGLE_SHOT_SURFACE_MAX_BYTES;
use crate::manifest::{Eligibility, IneligibleReason, Manifest, ProposerSpec, SweepParam, Tier};
use crate::state::RunState;

/// The size of what a proposer prompt must carry, measured by the shell: `program.md` plus every surface file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceStats {
    bytes: u64,
}

impl SurfaceStats {
    /// A surface of `bytes` total bytes.
    #[must_use]
    pub const fn new(bytes: u64) -> Self {
        Self { bytes }
    }

    /// The total size, in bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl ProposerSpec {
    /// Whether this proposer can propose: [`ProposerSpec::declared_eligibility`] first, then the checks that need
    /// file sizes. A single-shot proposer is [`IneligibleReason::SurfaceTooLarge`] when the surface is over
    /// [`SINGLE_SHOT_SURFACE_MAX_BYTES`], because the whole surface must fit in one prompt.
    #[must_use]
    pub fn eligible(&self, params: &[SweepParam], surface: &SurfaceStats) -> Eligibility {
        match self.declared_eligibility(params) {
            Eligibility::Eligible
                if self.tier() == Tier::SingleShot
                    && surface.bytes() > SINGLE_SHOT_SURFACE_MAX_BYTES =>
            {
                Eligibility::Ineligible(IneligibleReason::SurfaceTooLarge {
                    bytes: surface.bytes(),
                    limit: SINGLE_SHOT_SURFACE_MAX_BYTES,
                })
            }
            eligibility => eligibility,
        }
    }
}

/// One declared proposer and whether it can propose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteRow {
    index: usize,
    tier: Tier,
    eligibility: Eligibility,
}

impl RouteRow {
    /// The proposer's position in [`Manifest::proposers`].
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The proposer's tier.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// Whether it can propose, with the reason when it cannot.
    #[must_use]
    pub const fn eligibility(&self) -> Eligibility {
        self.eligibility
    }
}

/// Every declared proposer, in the order of [`Manifest::proposers`], with its eligibility.
#[must_use]
pub fn route_table(manifest: &Manifest, surface: &SurfaceStats) -> Vec<RouteRow> {
    manifest
        .proposers()
        .iter()
        .enumerate()
        .map(|(index, spec)| RouteRow {
            index,
            tier: spec.tier(),
            eligibility: spec.eligible(manifest.params(), surface),
        })
        .collect()
}

/// Why no route exists.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    /// No declared proposer is eligible for this surface; [`route_table`] gives the reason for each.
    #[error("no declared proposer is eligible; minos check prints the reason for each tier")]
    NoEligibleTier,
}

/// The proposer chosen for the next generation, and how it was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    proposer: usize,
    tier: Tier,
    stall: u32,
    step: u32,
}

impl Route {
    /// The chosen proposer's position in [`Manifest::proposers`].
    #[must_use]
    pub const fn proposer(&self) -> usize {
        self.proposer
    }

    /// The chosen proposer's tier.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The number of concluded generations since the last improving decision, or since the run started.
    #[must_use]
    pub const fn stall(&self) -> u32 {
        self.stall
    }

    /// `stall / stall_step`: the rung asked for. The chosen rung is this, clamped to the top of the ladder.
    #[must_use]
    pub const fn step(&self) -> u32 {
        self.step
    }
}

/// The proposer for the next generation (D-13).
///
/// The ladder is the eligible rows of [`route_table`], sorted by ([`Tier::rank`], index). `stall` counts the
/// recorded decisions with a generation above the most recent [`DecisionKind::Improved`] decision, or every
/// decision when none improved; a `Kept` and a `NoValidCandidates` decision both count. The chosen rung is
/// `min(stall / stall_step, top)`. All arithmetic saturates or is checked, so no ledger can make it panic.
///
/// # Errors
///
/// [`RouteError::NoEligibleTier`] when no declared proposer is eligible for `surface`.
pub fn route(
    manifest: &Manifest,
    state: &RunState,
    surface: &SurfaceStats,
) -> Result<Route, RouteError> {
    let mut ladder: Vec<RouteRow> = route_table(manifest, surface)
        .into_iter()
        .filter(|row| row.eligibility == Eligibility::Eligible)
        .collect();
    ladder.sort_by_key(|row| (row.tier.rank(), row.index));
    let last_improved = state
        .decisions()
        .filter(|(_, decision)| matches!(decision.kind(), DecisionKind::Improved { .. }))
        .map(|(generation, _)| generation)
        .max();
    let stalled = state
        .decisions()
        .filter(|(generation, _)| last_improved.is_none_or(|improved| *generation > improved))
        .count();
    let stall = u32::try_from(stalled).unwrap_or(u32::MAX);
    let step = stall / manifest.policy().stall_step().max(1);
    let top = ladder.len().saturating_sub(1);
    let rung = usize::try_from(step).unwrap_or(usize::MAX).min(top);
    let row = ladder.get(rung).ok_or(RouteError::NoEligibleTier)?;
    Ok(Route {
        proposer: row.index,
        tier: row.tier,
        stall,
        step,
    })
}
