//! The loop-facing entry point: what one propose step reads, what it returns, and how it fails (PROP-01, PROP-08,
//! PROP-09).
//!
//! The Phase 7 loop builds a [`ProposeInput`] from the folded run state and receives [`Proposals`]: validated
//! proposals whose diffs are cumulative from the base, the drops counted by closed reason, the call's token usage,
//! and the proposals' [`Origin`]. A [`ProposeError`] is never an `Outcome`: a provider stop ends the run, a
//! provider infrastructure failure feeds the loop's consecutive-failure streak, and every other variant is a
//! configuration error the loop reports and never retries.

use std::collections::BTreeMap;

use minos_core::diff::apply::Tree;
use minos_core::diff::cumulative::BaseTree;
use minos_core::id::{ModelId, RunId};
use minos_core::ledger::{Origin, StopReason};
use minos_core::manifest::{Manifest, ProposerSpec, Tier};
use minos_core::sweep::SweepError;
use minos_core::usage::Usage;

use crate::prompt::{History, PromptError, SteerNote};
use crate::proposal::{DropCounts, Proposal};
use crate::provider::{EmptyCause, ModelProvider, ProviderError};
use crate::schema::SchemaError;
use crate::{single_shot, sweep};

/// Everything one propose step reads: a parameter bundle the loop builds from the folded run state, not a
/// validated type. The tiers check its invariants (a generation of at least 1, and a parent with the base's
/// paths).
#[derive(Debug, Clone, Copy)]
pub struct ProposeInput<'a> {
    /// The validated manifest.
    pub manifest: &'a Manifest,
    /// The run, which seeds the sweep.
    pub run: &'a RunId,
    /// The generation being proposed, at least 1 (generation 0 is baseline calibration).
    pub generation: u32,
    /// The run's base surface; every proposal's diff is minted cumulative from it (07-CONTEXT D-12).
    pub base: &'a BaseTree,
    /// The incumbent's surface tree: the base with the incumbent's cumulative diff applied by the loop (D-05).
    /// Every proposal is an edit of it.
    pub parent: &'a Tree,
    /// The bytes of the manifest's program file.
    pub program: &'a [u8],
    /// The rendered history.
    pub history: &'a History,
    /// The user's steer notes.
    pub notes: &'a [SteerNote],
}

/// What one propose step produced.
///
/// The loop records one `Proposed` event per item with [`Proposals::origin`], adds [`DropCounts::total`] of
/// [`Proposals::dropped`] to `Decision::dropped`, records `Spent { SpendSource::ModelCall }` once per entry of
/// [`Proposals::models`], or once from [`Proposals::usage`] under the requested model when `models` is empty, and
/// treats an empty [`Proposals::items`] as a generation with no valid candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposals {
    tier: Tier,
    items: Vec<Proposal>,
    dropped: DropCounts,
    empty: Option<EmptyCause>,
    usage: Option<Usage>,
    models: BTreeMap<ModelId, Usage>,
    origin: Origin,
}

impl Proposals {
    /// The step's result. Crate-private: only the tiers build one, from proposals that passed the strict checks.
    pub(crate) const fn new(
        tier: Tier,
        items: Vec<Proposal>,
        dropped: DropCounts,
        empty: Option<EmptyCause>,
        usage: Option<Usage>,
        models: BTreeMap<ModelId, Usage>,
        origin: Origin,
    ) -> Self {
        Self {
            tier,
            items,
            dropped,
            empty,
            usage,
            models,
            origin,
        }
    }

    /// The tier that proposed.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The surviving proposals, in the order they were produced.
    #[must_use]
    pub fn items(&self) -> &[Proposal] {
        &self.items
    }

    /// The drops by reason.
    #[must_use]
    pub const fn dropped(&self) -> &DropCounts {
        &self.dropped
    }

    /// Why a model call completed with no output, when it did; `None` for a sweep and for a call with output.
    #[must_use]
    pub const fn empty(&self) -> Option<EmptyCause> {
        self.empty
    }

    /// The call's top-level token usage; `None` for a sweep, which calls no model.
    #[must_use]
    pub const fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// Token usage per resolved model, for pricing. Never a dollar figure.
    #[must_use]
    pub const fn models(&self) -> &BTreeMap<ModelId, Usage> {
        &self.models
    }

    /// Where every proposal came from: `Origin::Sweep`, or the model and the hash of the exact prompt sent.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The proposals, by value.
    #[must_use]
    pub fn into_items(self) -> Vec<Proposal> {
        self.items
    }
}

/// Why a propose step produced nothing at all. Never an `Outcome`.
///
/// [`ProposeError::Provider`] carries the provider's blame: [`ProposeError::stop_reason`] is the run's stop
/// reason for a provider stop, and [`ProposeError::is_infra`] is true for a provider infrastructure failure, which
/// the loop counts toward `max_infra_failures`. Every other variant is a configuration error the loop reports and
/// never retries.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProposeError {
    /// Generation 0 is baseline calibration; proposals start at generation 1.
    #[error("generation 0 is baseline calibration and takes no proposals")]
    Generation0,
    /// The base and parent trees do not hold the same paths.
    #[error("the base and parent trees do not hold the same paths")]
    TreeMismatch,
    /// The spec's tier is not the tier that was called.
    #[error("a {found} proposer was handed to the {expected} tier")]
    TierMismatch {
        /// The tier that was called.
        expected: Tier,
        /// The spec's tier.
        found: Tier,
    },
    /// A model tier's spec names no model.
    #[error("this proposer names no model")]
    NoModel,
    /// A model tier was asked to propose with no provider.
    #[error("this proposer needs a model provider and none was given")]
    NoProvider,
    /// The tier is not implemented in this release.
    #[error("the {tier} tier is not in this release")]
    TierNotInThisRelease {
        /// The tier.
        tier: Tier,
    },
    /// The sweep could not draw samples.
    #[error(transparent)]
    Sweep(SweepError),
    /// The prompt could not be built.
    #[error(transparent)]
    Prompt(PromptError),
    /// The output schema could not be built.
    #[error(transparent)]
    Schema(SchemaError),
    /// The model call failed.
    #[error(transparent)]
    Provider(ProviderError),
}

impl ProposeError {
    /// The run's stop reason when the provider refused in a way that ends the run; `None` otherwise.
    #[must_use]
    pub const fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Provider(error) => error.stop_reason(),
            _ => None,
        }
    }

    /// Whether the provider failed for infrastructure reasons (retried by the loop, counted by its streak).
    #[must_use]
    pub const fn is_infra(&self) -> bool {
        match self {
            Self::Provider(error) => error.is_infra(),
            _ => false,
        }
    }

    /// The token usage the failed call reported, if any, so the loop can still record what was spent.
    #[must_use]
    pub const fn usage(&self) -> Option<&Usage> {
        match self {
            Self::Provider(error) => error.usage(),
            _ => None,
        }
    }
}

/// Checks that the spec's tier, `found`, is the tier that was called, `expected`, then the input's invariants: a generation of at least 1
/// and a parent with exactly the base's paths.
pub(crate) fn check_input(
    expected: Tier,
    found: Tier,
    input: &ProposeInput<'_>,
) -> Result<(), ProposeError> {
    if expected != found {
        return Err(ProposeError::TierMismatch { expected, found });
    }
    if input.generation == 0 {
        return Err(ProposeError::Generation0);
    }
    let same_paths = input
        .base
        .tree()
        .iter()
        .map(|(path, _)| path)
        .eq(input.parent.iter().map(|(path, _)| path));
    if !same_paths {
        return Err(ProposeError::TreeMismatch);
    }
    Ok(())
}

/// The loop's one entry point per generation: it dispatches a proposer spec to its tier.
///
/// Generic over the provider, exactly as the runner is generic over its sandbox backend, and never a trait object:
/// each binary picks its provider concretely, so the same tier code runs over [`crate::fake::FakeProvider`] in
/// tests and the `claude` CLI provider in production. Routing (`minos_core::route`) picks which spec to pass.
#[derive(Debug, Clone)]
pub struct Proposer<P> {
    provider: Option<P>,
}

impl<P> Proposer<P> {
    /// A proposer whose model tiers call `provider`.
    #[must_use]
    pub const fn with_provider(provider: P) -> Self {
        Self {
            provider: Some(provider),
        }
    }

    /// A proposer with no provider: only the sweep tier can propose, so a sweep-only project needs no model login.
    #[must_use]
    pub const fn without_provider() -> Self {
        Self { provider: None }
    }

    /// The provider, when there is one.
    #[must_use]
    pub const fn provider(&self) -> Option<&P> {
        self.provider.as_ref()
    }
}

impl<P: ModelProvider> Proposer<P> {
    /// The proposals of `spec`'s tier for `input`: a sweep through [`sweep::propose`], which calls no model, and a
    /// single-shot proposer through [`single_shot::propose`] with this proposer's provider.
    ///
    /// # Errors
    ///
    /// [`ProposeError::NoProvider`] for a single-shot spec when there is no provider,
    /// [`ProposeError::TierNotInThisRelease`] for a planned or agentic spec, and every error of the tier called.
    pub fn propose(
        &self,
        spec: &ProposerSpec,
        input: &ProposeInput<'_>,
    ) -> Result<Proposals, ProposeError> {
        match spec.tier() {
            Tier::Sweep => sweep::propose(spec, input),
            Tier::SingleShot => {
                let provider = self.provider.as_ref().ok_or(ProposeError::NoProvider)?;
                single_shot::propose(provider, spec, input)
            }
            tier @ (Tier::Planned | Tier::Agentic) => {
                Err(ProposeError::TierNotInThisRelease { tier })
            }
        }
    }
}
