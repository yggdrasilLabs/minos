//! The model provider abstraction (PROP-01).
//!
//! A provider makes one model call: it takes a [`ModelRequest`] (the model, the prompt, the output schema) and
//! returns a [`ModelResponse`] (the hostile structured output, or the reason there is none, plus token usage), or
//! a [`ProviderError`] that separates blame: a [`ProviderStop`] ends the run, and an [`InfraCause`] is an
//! infrastructure failure that feeds the loop's consecutive-failure streak. No type here holds a dollar figure,
//! model text, or CLI text (06-CONTEXT D-13, D-14).

use std::collections::BTreeMap;
use std::fmt;

use minos_core::id::ModelId;
use minos_core::ledger::StopReason;
use minos_core::usage::Usage;

use crate::prompt::Prompt;
use crate::schema::OutputSchema;

/// One model call.
///
/// Callers are generic over the provider (`fn propose<P: ModelProvider>(provider: &P, ..)`), exactly as the
/// runner is generic over its sandbox backend, and never hold one as a trait object. A later Bedrock or API-key
/// provider implements this same trait, so no caller changes.
pub trait ModelProvider {
    /// Makes one call with `request`'s model, prompt, and schema.
    ///
    /// The provider never retries the call and never interprets proposals: it returns the structured output
    /// still wrapped as hostile [`ModelOutput`], which only [`crate::proposal::parse_output`] opens.
    ///
    /// # Errors
    ///
    /// [`ProviderError::Stop`] when the provider refused in a way that must end the run (a usage limit or a
    /// rejected login), and [`ProviderError::Infra`] for every other failure.
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError>;
}

/// What one model call is asked: the model, the prompt, and the output schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRequest {
    model: ModelId,
    prompt: Prompt,
    schema: OutputSchema,
}

impl ModelRequest {
    /// The request to `model` with `prompt`, answered in the shape of `schema`.
    #[must_use]
    pub const fn new(model: ModelId, prompt: Prompt, schema: OutputSchema) -> Self {
        Self {
            model,
            prompt,
            schema,
        }
    }

    /// The requested model, as the manifest names it (for example the alias `haiku`).
    #[must_use]
    pub const fn model(&self) -> &ModelId {
        &self.model
    }

    /// The prompt, sent whole on the provider's input.
    #[must_use]
    pub const fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    /// The output schema.
    #[must_use]
    pub const fn schema(&self) -> &OutputSchema {
        &self.schema
    }
}

/// Structured output a model wrote: hostile input.
///
/// No derives and no `Debug`, `Display`, `Clone`, or `Deref` implementation (a test proves it at compile time), so
/// the content can be neither printed nor copied out. Any provider may wrap output with [`ModelOutput::from_value`];
/// the only exit is the crate's strict parser, [`crate::proposal::parse_output`].
pub struct ModelOutput(serde_json::Value);

impl ModelOutput {
    /// Wraps a model's structured output, as parsed from the provider's response.
    #[must_use]
    pub const fn from_value(value: serde_json::Value) -> Self {
        Self(value)
    }

    /// The wrapped value. Crate-private: only the strict parser opens it.
    pub(crate) fn into_value(self) -> serde_json::Value {
        self.0
    }
}

/// Why a call completed with no structured output: zero valid proposals for this call, never an infrastructure
/// failure (D-12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmptyCause {
    /// The call hit its turn cap before producing output (`error_max_turns`).
    MaxTurns,
    /// The model failed the schema too many times (`error_max_structured_output_retries`).
    StructuredOutputRetries,
    /// The model refused (`stop_reason` `refusal`).
    Refusal,
    /// The reply was larger than the provider's output cap, so it was never read. Its size is the model's choice,
    /// so this is zero valid proposals for the call and never an infrastructure failure; the usage is unknown,
    /// because the envelope that carries it was never read, and is recorded as zero.
    OutputTooLarge,
}

impl fmt::Display for EmptyCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MaxTurns => "the model call hit its turn cap with no output",
            Self::StructuredOutputRetries => "the model's output failed the schema too many times",
            Self::Refusal => "the model refused",
            Self::OutputTooLarge => {
                "the model's reply was larger than the output cap and was not read"
            }
        })
    }
}

/// What a completed call produced.
///
/// No derives, because [`ModelOutput`] has none.
pub enum Completion {
    /// Structured output, still hostile.
    Structured(ModelOutput),
    /// No output, for a reason that counts as an empty call.
    Empty(EmptyCause),
}

/// A completed model call: its completion and its token usage.
///
/// The tiers record `Origin::Model { model, .. }` with [`ModelResponse::resolved_model`] when there is one (for
/// example `claude-haiku-5-5` for the alias `haiku`), and otherwise with the requested model,
/// [`ModelRequest::model`].
pub struct ModelResponse {
    completion: Completion,
    usage: Usage,
    models: BTreeMap<ModelId, Usage>,
}

impl ModelResponse {
    /// A response with `completion`, the call's top-level token `usage`, and per-model token usage `models`.
    #[must_use]
    pub const fn new(
        completion: Completion,
        usage: Usage,
        models: BTreeMap<ModelId, Usage>,
    ) -> Self {
        Self {
            completion,
            usage,
            models,
        }
    }

    /// What the call produced.
    #[must_use]
    pub const fn completion(&self) -> &Completion {
        &self.completion
    }

    /// What the call produced, by value, for the strict parser.
    #[must_use]
    pub fn into_completion(self) -> Completion {
        self.completion
    }

    /// The call's top-level token usage, which the loop records in `Spent` and checks against the token cap.
    #[must_use]
    pub const fn usage(&self) -> &Usage {
        &self.usage
    }

    /// Token usage per resolved model, for pricing from `prices.toml`. Never a dollar figure (D-14).
    #[must_use]
    pub const fn models(&self) -> &BTreeMap<ModelId, Usage> {
        &self.models
    }

    /// The one model the call ran on, when exactly one per-model usage entry exists; `None` otherwise.
    #[must_use]
    pub fn resolved_model(&self) -> Option<&ModelId> {
        let mut keys = self.models.keys();
        match (keys.next(), keys.next()) {
            (Some(only), None) => Some(only),
            _ => None,
        }
    }
}

/// A provider refusal that ends the run, never retried. [`ProviderStop::stop_reason`] maps each to the ledger's
/// [`StopReason`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderStop {
    /// A rate or usage limit the provider gave up retrying (`StopReason::ProviderLimit`).
    Limit,
    /// The provider rejected the login (`StopReason::ProviderAuth`, D-15).
    Auth,
}

impl ProviderStop {
    /// The ledger's stop reason for this refusal: `ProviderLimit` for a limit and `ProviderAuth` for a rejected
    /// login (06-CONTEXT D-15).
    #[must_use]
    pub const fn stop_reason(self) -> StopReason {
        match self {
            Self::Limit => StopReason::ProviderLimit,
            Self::Auth => StopReason::ProviderAuth,
        }
    }
}

impl fmt::Display for ProviderStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Limit => "the model provider refused for a rate or usage limit",
            Self::Auth => "the model provider rejected the login",
        })
    }
}

/// Why a call failed for infrastructure reasons. Fixed text and numbers only, never model or CLI text.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InfraCause {
    /// The provider process could not be started.
    #[error("the model provider process could not be started")]
    Spawn,
    /// Waiting for the provider process failed.
    #[error("waiting for the model provider process failed")]
    Wait,
    /// Reading from or writing to the provider process failed.
    #[error("input or output with the model provider process failed")]
    Io,
    /// The call ran past its deadline and its process group was killed (D-16).
    #[error("the model call ran past its deadline")]
    Deadline,
    /// The provider process was killed by a signal the provider did not send.
    #[error("the model provider process was killed by signal {signal}")]
    Signal {
        /// The signal number.
        signal: i32,
    },
    /// A preflight query (`--version` or `auth status`) printed more than its cap, which only a broken install
    /// does. A model call's reply over its cap is [`EmptyCause::OutputTooLarge`] instead, never this.
    #[error("the model provider's output exceeded its cap")]
    OutputTooLarge,
    /// The provider's response envelope did not parse.
    #[error("the model provider's response did not parse")]
    EnvelopeInvalid,
    /// The response was not a result message.
    #[error("the model provider's response was not a result")]
    NotAResult,
    /// A successful result carried no structured output.
    #[error("the model provider's result carried no structured output")]
    MissingStructuredOutput,
    /// The result carried no token usage.
    #[error("the model provider's result carried no token usage")]
    MissingUsage,
    /// The provider reported an API error that is neither a limit nor an auth failure.
    #[error("the model provider reported an API error")]
    ApiError {
        /// The HTTP status, when the provider reported one.
        status: Option<u16>,
    },
    /// The provider reported an error during execution.
    #[error("the model provider failed during execution")]
    ExecutionError,
    /// The provider stopped at its own spending budget.
    #[error("the model provider stopped at its own budget")]
    BudgetExceeded,
    /// The result had a subtype this provider does not know.
    #[error("the model provider's result had an unknown subtype")]
    UnknownSubtype,
    /// The process's exit status disagreed with its result.
    #[error("the model provider's exit status disagreed with its result")]
    ExitMismatch {
        /// The exit code, when the process exited with one.
        code: Option<i32>,
    },
    /// The provider binary is missing or no longer where the preflight found it.
    #[error("the model provider binary is missing")]
    BinaryMissing,
    /// A repeated preflight failed during the run.
    #[error("the model provider's preflight failed")]
    Preflight,
    /// The per-call working directory could not be created. A directory that cannot be removed afterwards never
    /// fails the call: the provider records it (review IN-04).
    #[error("the model call's working directory could not be created")]
    WorkDir,
    /// A scripted failure from a test provider.
    #[error("a scripted test failure")]
    Fake,
}

/// A failed model call, with the blame separated: a stop ends the run, an infrastructure failure feeds the loop's
/// consecutive-failure streak. Either carries the call's token usage when the provider reported any, so the loop
/// can still record what was spent.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider refused in a way that ends the run.
    #[error("{stop}")]
    Stop {
        /// Why the run must stop.
        stop: ProviderStop,
        /// Token usage the call reported, if any.
        usage: Option<Usage>,
    },
    /// An infrastructure failure.
    #[error("{cause}")]
    Infra {
        /// What failed.
        cause: InfraCause,
        /// Token usage the call reported, if any.
        usage: Option<Usage>,
    },
}

impl ProviderError {
    /// The token usage the call reported, if any.
    #[must_use]
    pub const fn usage(&self) -> Option<&Usage> {
        match self {
            Self::Stop { usage, .. } | Self::Infra { usage, .. } => usage.as_ref(),
        }
    }

    /// The ledger's stop reason when this failure ends the run, and `None` for an infrastructure failure.
    #[must_use]
    pub const fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stop { stop, .. } => Some(stop.stop_reason()),
            Self::Infra { .. } => None,
        }
    }

    /// Whether this is an infrastructure failure (retried by the loop) rather than a stop.
    #[must_use]
    pub const fn is_infra(&self) -> bool {
        matches!(self, Self::Infra { .. })
    }
}
