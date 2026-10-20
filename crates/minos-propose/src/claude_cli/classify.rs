//! The result classifier: what one `claude -p` call's output means (06-CONTEXT D-12, D-13 amended, D-14, D-15;
//! 06-RESEARCH Pattern 7).
//!
//! [`classify`] is a pure function of the call's capped stdout and how the process ended. It maps every result
//! the CLI can emit to exactly one of a completion with structured output, an empty call (zero valid proposals,
//! never infrastructure), a [`ProviderStop`] that ends the run, or an [`InfraCause`] the loop counts toward its
//! consecutive-failure streak.
//!
//! The envelope is parsed leniently: it declares only the fields the table reads and ignores the rest, because the
//! CLI adds result fields often (`.claude/CLAUDE.md` Question 4). That is the one documented exception to
//! `deny_unknown_fields`. The structured output inside it is model output, still hostile, and is opened only by
//! [`crate::proposal::parse_output`]. No type here declares `total_cost_usd` or `costUSD`: only token counts leave
//! the envelope (D-14).
//!
//! The classification comes from the documented result fields, `subtype`, `is_error`, `api_error_status`, and
//! `stop_reason`, with the documented message prefixes as a fallback. An API error that ends the turn (a usage or
//! rate limit, a rejected login) arrives as `subtype` `success` with `is_error` true, because `api_error_status`
//! exists only on the success variant (06-RESEARCH Finding 1). The exit code is only a consistency check, never
//! the classification (02-SPIKE-EVIDENCE.md C-03, C-11).

use std::collections::BTreeMap;

use minos_core::id::ModelId;
use minos_core::usage::Usage;
use serde::Deserialize;
use serde_json::Value;

use crate::provider::{
    Completion, EmptyCause, InfraCause, ModelOutput, ModelResponse, ProviderError, ProviderStop,
};

/// How the `claude` process ended, as the host observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The process exited with this code.
    Code(i32),
    /// The process was killed by this signal.
    Signal(i32),
}

/// Message prefixes that mark a rate or usage limit (06-RESEARCH Pattern 7), matched after a typographic
/// apostrophe (U+2019) is replaced with an ASCII one.
const LIMIT_PREFIXES: [&str; 3] = [
    "You've hit your",
    "You've reached your",
    "You're out of usage credits",
];

/// Message prefixes that mark a rejected login (06-RESEARCH Pattern 7), matched the same way.
const AUTH_PREFIXES: [&str; 9] = [
    "Failed to authenticate",
    "Not logged in",
    "Login expired",
    "OAuth token revoked",
    "OAuth token has expired",
    "Authentication required",
    "Invalid API key",
    "Claude login not accepted",
    "Your account does not have access to Claude",
];

/// The result fields the classifier reads. Every other field is ignored, never bound.
#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: Option<String>,
    subtype: Option<String>,
    is_error: Option<bool>,
    api_error_status: Option<u64>,
    result: Option<String>,
    errors: Option<Vec<String>>,
    stop_reason: Option<String>,
    structured_output: Option<Value>,
    usage: Option<EnvelopeUsage>,
    #[serde(rename = "modelUsage")]
    model_usage: Option<BTreeMap<String, ModelTokens>>,
}

/// The four token counts of the top-level `usage` object; an absent count is 0.
#[derive(Deserialize)]
struct EnvelopeUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "cache_creation_input_tokens")]
    cache_creation: u64,
    #[serde(default, rename = "cache_read_input_tokens")]
    cache_read: u64,
}

/// The four token counts of one `modelUsage` entry; an absent count is 0.
#[derive(Deserialize)]
struct ModelTokens {
    #[serde(default, rename = "inputTokens")]
    input: u64,
    #[serde(default, rename = "outputTokens")]
    output: u64,
    #[serde(default, rename = "cacheCreationInputTokens")]
    cache_creation: u64,
    #[serde(default, rename = "cacheReadInputTokens")]
    cache_read: u64,
}

impl EnvelopeUsage {
    const fn to_usage(&self) -> Usage {
        Usage::tokens(
            self.input,
            self.output,
            self.cache_creation,
            self.cache_read,
        )
    }
}

impl ModelTokens {
    const fn to_usage(&self) -> Usage {
        Usage::tokens(
            self.input,
            self.output,
            self.cache_creation,
            self.cache_read,
        )
    }
}

/// Classifies one call from its capped `stdout` and how the process ended (06-RESEARCH Pattern 7).
///
/// In order: a signal is [`InfraCause::Signal`]; stdout that is not exactly one JSON object of the envelope's
/// shape (whitespace aside) is [`InfraCause::EnvelopeInvalid`], with no usage; a `type` other than `result` is
/// [`InfraCause::NotAResult`]. Then, by `subtype`:
///
/// - `success` with `is_error` false needs exit code 0 ([`InfraCause::ExitMismatch`]), structured output
///   ([`InfraCause::MissingStructuredOutput`]), and usage ([`InfraCause::MissingUsage`]), and is a completion with
///   [`Completion::Structured`]. A `success` with no `is_error` is [`InfraCause::EnvelopeInvalid`].
/// - `success` with `is_error` true is a [`ProviderStop::Limit`] for status 429, a [`ProviderStop::Auth`] for 401
///   or 403, then, whatever the status, the same stops for a limit or login message prefix on `result`; then an
///   empty call with [`EmptyCause::Refusal`] for `stop_reason` `refusal`; else [`InfraCause::ApiError`] with the
///   status (`None` when absent or outside `u16`).
/// - `error_max_turns` and `error_max_structured_output_retries` are empty calls with [`EmptyCause::MaxTurns`]
///   and [`EmptyCause::StructuredOutputRetries`] (D-12).
/// - `error_during_execution` is the stop whose prefix the first matching `errors` entry carries, else
///   [`InfraCause::ExecutionError`].
/// - `error_max_budget_usd` is [`InfraCause::BudgetExceeded`]; any other or missing subtype is
///   [`InfraCause::UnknownSubtype`].
///
/// Every completion and empty call needs usage. Every stop and infrastructure failure built from a parsed
/// envelope carries its usage when present, so the loop can record what was spent.
///
/// # Errors
///
/// [`ProviderError::Stop`] for a limit or a rejected login, and [`ProviderError::Infra`] for every other failure,
/// as listed above.
pub fn classify(stdout: &[u8], exit: Exit) -> Result<ModelResponse, ProviderError> {
    let code = match exit {
        Exit::Code(code) => code,
        Exit::Signal(signal) => return Err(infra(InfraCause::Signal { signal }, None)),
    };
    let envelope = parse(stdout).ok_or(infra(InfraCause::EnvelopeInvalid, None))?;
    let usage = envelope.usage.as_ref().map(EnvelopeUsage::to_usage);
    if envelope.kind.as_deref() != Some("result") {
        return Err(infra(InfraCause::NotAResult, usage));
    }
    match envelope.subtype.as_deref() {
        Some("success") => success(envelope, code, usage),
        Some("error_max_turns") => empty(EmptyCause::MaxTurns, &envelope, usage),
        Some("error_max_structured_output_retries") => {
            empty(EmptyCause::StructuredOutputRetries, &envelope, usage)
        }
        Some("error_during_execution") => {
            let stop = envelope
                .errors
                .iter()
                .flatten()
                .find_map(|error| message_stop(error));
            Err(
                stop.map_or(infra(InfraCause::ExecutionError, usage), |stop| {
                    ProviderError::Stop { stop, usage }
                }),
            )
        }
        Some("error_max_budget_usd") => Err(infra(InfraCause::BudgetExceeded, usage)),
        _ => Err(infra(InfraCause::UnknownSubtype, usage)),
    }
}

/// The envelope, when `stdout` is exactly one JSON object of its shape, with only whitespace around it.
fn parse(stdout: &[u8]) -> Option<Envelope> {
    // A derived struct also accepts a JSON array of its fields in order; the envelope is always an object.
    if stdout.trim_ascii_start().first() != Some(&b'{') {
        return None;
    }
    serde_json::from_slice(stdout).ok()
}

/// The `success` subtype.
fn success(
    envelope: Envelope,
    code: i32,
    usage: Option<Usage>,
) -> Result<ModelResponse, ProviderError> {
    match envelope.is_error {
        None => Err(infra(InfraCause::EnvelopeInvalid, usage)),
        Some(false) => {
            if code != 0 {
                return Err(infra(InfraCause::ExitMismatch { code: Some(code) }, usage));
            }
            let models = models(envelope.model_usage.as_ref());
            let output = envelope
                .structured_output
                .ok_or(infra(InfraCause::MissingStructuredOutput, usage))?;
            respond(
                Completion::Structured(ModelOutput::from_value(output)),
                usage,
                models,
            )
        }
        Some(true) => {
            let status = envelope
                .api_error_status
                .and_then(|status| u16::try_from(status).ok());
            let stop = match status {
                Some(429) => Some(ProviderStop::Limit),
                Some(401 | 403) => Some(ProviderStop::Auth),
                _ => envelope.result.as_deref().and_then(message_stop),
            };
            if let Some(stop) = stop {
                return Err(ProviderError::Stop { stop, usage });
            }
            if envelope.stop_reason.as_deref() == Some("refusal") {
                return empty(EmptyCause::Refusal, &envelope, usage);
            }
            Err(infra(InfraCause::ApiError { status }, usage))
        }
    }
}

/// An empty call for `cause`, which needs usage.
fn empty(
    cause: EmptyCause,
    envelope: &Envelope,
    usage: Option<Usage>,
) -> Result<ModelResponse, ProviderError> {
    respond(
        Completion::Empty(cause),
        usage,
        models(envelope.model_usage.as_ref()),
    )
}

/// A response with `completion`, or [`InfraCause::MissingUsage`] when the envelope carried no usage.
fn respond(
    completion: Completion,
    usage: Option<Usage>,
    models: BTreeMap<ModelId, Usage>,
) -> Result<ModelResponse, ProviderError> {
    let usage = usage.ok_or(infra(InfraCause::MissingUsage, None))?;
    Ok(ModelResponse::new(completion, usage, models))
}

/// Per-model usage, keyed by the entries whose key parses as a [`ModelId`]; other keys are skipped.
fn models(model_usage: Option<&BTreeMap<String, ModelTokens>>) -> BTreeMap<ModelId, Usage> {
    model_usage
        .into_iter()
        .flatten()
        .filter_map(|(key, tokens)| {
            key.parse::<ModelId>()
                .ok()
                .map(|model| (model, tokens.to_usage()))
        })
        .collect()
}

/// The stop a provider message marks by its prefix, limit prefixes first.
fn message_stop(message: &str) -> Option<ProviderStop> {
    let message = message.replace('\u{2019}', "'");
    if LIMIT_PREFIXES
        .iter()
        .any(|prefix| message.starts_with(prefix))
    {
        Some(ProviderStop::Limit)
    } else if AUTH_PREFIXES
        .iter()
        .any(|prefix| message.starts_with(prefix))
    {
        Some(ProviderStop::Auth)
    } else {
        None
    }
}

const fn infra(cause: InfraCause, usage: Option<Usage>) -> ProviderError {
    ProviderError::Infra { cause, usage }
}
