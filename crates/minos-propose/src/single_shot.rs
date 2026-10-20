//! The single-shot tier (PROP-07): one model call with no tools, returning up to `batch` proposals.
//!
//! [`propose`] assembles the prompt, builds the output schema for the spec's batch, and makes exactly one
//! [`ModelProvider::complete`] call. The structured output reaches proposals only through the strict parser,
//! [`parse_output`], which drops an invalid item and counts it rather than repairing it; zero survivors is an
//! empty call, never an error (06-CONTEXT D-10, D-12). The tier never retries: a provider stop or infrastructure
//! failure is returned to the loop as [`ProposeError::Provider`] (PROP-09), and the call's token usage is returned
//! with every result for the loop's `Spent` event (PROP-08).

use minos_core::ledger::Origin;
use minos_core::limits::Limits;
use minos_core::manifest::{ProposerSpec, Tier};

use crate::prompt::{PromptInputs, assemble};
use crate::proposal::{DropCounts, parse_output};
use crate::proposer::{Proposals, ProposeError, ProposeInput, check_input};
use crate::provider::{Completion, ModelProvider, ModelRequest};
use crate::schema::OutputSchema;

/// The single-shot tier's proposals for `input`, from one call to `provider`.
///
/// The origin is `Origin::Model` with the hash of the exact prompt sent and, as documented on
/// [`crate::provider::ModelResponse`], the resolved model when the response names exactly one, else the spec's
/// requested model. An empty completion gives no proposals and sets [`Proposals::empty`].
///
/// # Errors
///
/// [`ProposeError::TierMismatch`] when `spec` is not single-shot, [`ProposeError::NoModel`] when it names no model,
/// [`ProposeError::Generation0`] and [`ProposeError::TreeMismatch`] for an invalid input,
/// [`ProposeError::Prompt`] and [`ProposeError::Schema`] when the call cannot be built (none of these makes a
/// call), and [`ProposeError::Provider`] when the call fails.
pub fn propose<P: ModelProvider>(
    provider: &P,
    spec: &ProposerSpec,
    input: &ProposeInput<'_>,
) -> Result<Proposals, ProposeError> {
    check_input(Tier::SingleShot, spec.tier(), input)?;
    let model = spec.model().ok_or(ProposeError::NoModel)?;
    let manifest = input.manifest;
    let prompt = assemble(&PromptInputs {
        manifest,
        program: input.program,
        parent: input.parent,
        history: input.history,
        notes: input.notes,
        generation: input.generation,
        batch: spec.batch(),
    })
    .map_err(ProposeError::Prompt)?;
    let schema =
        OutputSchema::for_batch(spec.batch(), manifest.writable()).map_err(ProposeError::Schema)?;
    let prompt_hash = *prompt.hash();
    let request = ModelRequest::new(model.clone(), prompt, schema);

    let response = provider
        .complete(&request)
        .map_err(ProposeError::Provider)?;

    let origin = Origin::Model {
        model: response.resolved_model().unwrap_or(model).clone(),
        prompt: prompt_hash,
    };
    let usage = *response.usage();
    let models = response.models().clone();
    let (items, dropped, empty) = match response.into_completion() {
        Completion::Structured(output) => {
            let (items, dropped) = parse_output(
                output,
                spec.batch(),
                input.base,
                input.parent,
                manifest.writable(),
                &Limits::standard(),
            )
            .into_parts();
            (items, dropped, None)
        }
        Completion::Empty(cause) => (Vec::new(), DropCounts::default(), Some(cause)),
    };

    Ok(Proposals::new(
        Tier::SingleShot,
        items,
        dropped,
        empty,
        Some(usage),
        models,
        origin,
    ))
}
