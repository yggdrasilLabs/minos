//! The assembled prompt value.
//!
//! Deterministic assembly from the manifest, the parent tree, the history, and the steer notes arrives in plan
//! 06-03; this module holds the value it produces. A prompt is one UTF-8 text, a stable prefix then a dynamic
//! suffix (06-CONTEXT D-05 to D-09), sent whole on the provider's stdin (D-06) and hashed with
//! [`PromptHash::of`] over exactly those bytes.

use std::fmt;

use minos_core::id::PromptHash;

use crate::limits::MAX_PROMPT_BYTES;

mod history;

pub use history::{History, HistoryEntry};

/// A prompt: a stable prefix followed by a dynamic suffix, at most [`MAX_PROMPT_BYTES`] bytes.
///
/// `Debug` prints the lengths and the hash only, never the text, so a log line cannot carry a prompt that holds
/// sandbox or model text.
#[derive(Clone, PartialEq, Eq)]
pub struct Prompt {
    text: String,
    prefix_len: usize,
    hash: PromptHash,
}

/// Why a prompt could not be built.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    /// The prefix and suffix together exceed the prompt cap.
    #[error("the prompt is {bytes} bytes, over the {limit}-byte cap")]
    TooLarge {
        /// The prompt's total size, in bytes.
        bytes: usize,
        /// The cap, [`MAX_PROMPT_BYTES`].
        limit: usize,
    },
}

impl Prompt {
    /// The prompt `prefix` then `suffix`.
    ///
    /// # Errors
    ///
    /// [`PromptError::TooLarge`] when the two together exceed [`MAX_PROMPT_BYTES`].
    pub fn new(prefix: &str, suffix: &str) -> Result<Self, PromptError> {
        let bytes = prefix.len().saturating_add(suffix.len());
        if bytes > MAX_PROMPT_BYTES {
            return Err(PromptError::TooLarge {
                bytes,
                limit: MAX_PROMPT_BYTES,
            });
        }
        let mut text = String::with_capacity(bytes);
        text.push_str(prefix);
        text.push_str(suffix);
        let hash = PromptHash::of(text.as_bytes());
        Ok(Self {
            text,
            prefix_len: prefix.len(),
            hash,
        })
    }

    /// The whole prompt text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The whole prompt as bytes, exactly as sent and hashed.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// The prompt's size, in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Whether the prompt is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The stable prefix's size, in bytes.
    #[must_use]
    pub const fn prefix_len(&self) -> usize {
        self.prefix_len
    }

    /// The stable prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        // `prefix_len` is the length of the `&str` the text starts with, so it is always a char boundary.
        self.text.get(..self.prefix_len).unwrap_or_default()
    }

    /// The dynamic suffix.
    #[must_use]
    pub fn suffix(&self) -> &str {
        self.text.get(self.prefix_len..).unwrap_or_default()
    }

    /// The hash of the whole prompt text, as `Origin::Model` records it.
    #[must_use]
    pub const fn hash(&self) -> &PromptHash {
        &self.hash
    }
}

impl fmt::Debug for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Prompt")
            .field("len", &self.text.len())
            .field("prefix_len", &self.prefix_len)
            .field("hash", &self.hash)
            .finish()
    }
}
