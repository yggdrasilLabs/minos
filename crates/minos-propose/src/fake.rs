//! [`FakeProvider`]: the scripted [`ModelProvider`] for every test above the provider, as minos-store's
//! `MemoryStore` is for every test above the store.
//!
//! A fake replays raw JSON and typed errors and records each request. It can never hand back a validated
//! proposal: a scripted output is wrapped as hostile [`ModelOutput`] like any model's, so every test that uses the
//! fake runs the strict parser.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use minos_core::id::ModelId;
use minos_core::usage::Usage;

use crate::provider::{
    Completion, EmptyCause, InfraCause, ModelOutput, ModelProvider, ModelRequest, ModelResponse,
    ProviderError,
};

/// One scripted reply.
pub enum FakeReply {
    /// A completed call with raw structured `output`, top-level `usage`, and per-model usage `models`.
    Structured {
        /// The raw output, parsed by the tier exactly as a real model's would be.
        output: serde_json::Value,
        /// The call's top-level token usage.
        usage: Usage,
        /// Token usage per resolved model.
        models: BTreeMap<ModelId, Usage>,
    },
    /// A completed call with no output.
    Empty {
        /// Why there is no output.
        cause: EmptyCause,
        /// The call's top-level token usage.
        usage: Usage,
    },
    /// A failed call.
    Error(ProviderError),
}

/// One request the fake received: the model, the prompt's exact bytes, and the schema text.
///
/// `Debug` prints the model and the lengths only, never the prompt.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    model: ModelId,
    prompt: Vec<u8>,
    schema: String,
}

impl RecordedRequest {
    /// The requested model.
    #[must_use]
    pub const fn model(&self) -> &ModelId {
        &self.model
    }

    /// The exact prompt bytes the provider would have sent.
    #[must_use]
    pub fn prompt(&self) -> &[u8] {
        &self.prompt
    }

    /// The output schema text.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }
}

impl fmt::Debug for RecordedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordedRequest")
            .field("model", &self.model)
            .field("prompt_bytes", &self.prompt.len())
            .field("schema_bytes", &self.schema.len())
            .finish()
    }
}

/// A provider that answers from a script, in order, and records every request.
///
/// Each list sits behind its own mutex; a lock poisoned by a panic fails closed as an infrastructure failure. No
/// guard is held across anything but the list operation.
pub struct FakeProvider {
    script: Mutex<VecDeque<FakeReply>>,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl FakeProvider {
    /// A fake that answers with `script`, one reply per call, in order.
    #[must_use]
    pub fn new(script: Vec<FakeReply>) -> Self {
        Self {
            script: Mutex::new(VecDeque::from(script)),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every request received so far, in call order.
    ///
    /// # Errors
    ///
    /// [`ProviderError::Infra`] with [`InfraCause::Fake`] when the request list's lock was poisoned.
    pub fn requests(&self) -> Result<Vec<RecordedRequest>, ProviderError> {
        Ok(lock(&self.requests)?.clone())
    }
}

/// The scripted failure: an exhausted script or a poisoned lock.
const fn fake_failure() -> ProviderError {
    ProviderError::Infra {
        cause: InfraCause::Fake,
        usage: None,
    }
}

/// Locks `mutex`, mapping a poisoned lock to the scripted failure.
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, ProviderError> {
    mutex.lock().map_err(|_poisoned| fake_failure())
}

impl ModelProvider for FakeProvider {
    /// Records `request`, then answers with the next scripted reply; an exhausted script is
    /// [`InfraCause::Fake`].
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError> {
        lock(&self.requests)?.push(RecordedRequest {
            model: request.model().clone(),
            prompt: request.prompt().as_bytes().to_vec(),
            schema: request.schema().as_str().to_owned(),
        });
        let reply = lock(&self.script)?.pop_front().ok_or_else(fake_failure)?;
        match reply {
            FakeReply::Structured {
                output,
                usage,
                models,
            } => Ok(ModelResponse::new(
                Completion::Structured(ModelOutput::from_value(output)),
                usage,
                models,
            )),
            FakeReply::Empty { cause, usage } => Ok(ModelResponse::new(
                Completion::Empty(cause),
                usage,
                BTreeMap::new(),
            )),
            FakeReply::Error(error) => Err(error),
        }
    }
}
