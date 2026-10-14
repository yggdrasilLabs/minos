//! Integer `Usage`: vCPU and memory time and model tokens, never dollars.
//!
//! The ledger stores usage only, never a dollar figure (COST-01). Dollars are derived later from usage and the
//! user's price table in [`crate::money`], so a price change re-prices history without rewriting it.
//!
//! The token field names follow the `usage` object of `claude -p` (02-SPIKE-EVIDENCE.md SPIKE-07 contract):
//! `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, and `cache_read_input_tokens`. Local VMs cost
//! nothing in v1, but the compute dimensions (`vcpu_ms`, `mem_mib_ms`) exist now so the AWS milestone prices
//! Lambda microVM time without a wire-format change.
//!
//! Every field is a `u64`. A 128-bit field would not survive serde's internally tagged enums, which buffer integers as
//! at most 64 bits, and the ledger's `Event` enum is internally tagged.

use serde::{Deserialize, Serialize};

/// Metered quantities for one step: tokens in and out, and compute time. Never a dollar amount.
///
/// Every combination of `u64` values is a valid usage, so the derived `Deserialize` is the whole validation;
/// `deny_unknown_fields` refuses an extra field such as a dollar estimate, and every field is required.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    vcpu_ms: u64,
    mem_mib_ms: u64,
}

impl Usage {
    /// Token usage of one model call, with no compute time.
    #[must_use]
    pub const fn tokens(
        input: u64,
        output: u64,
        cache_creation_input: u64,
        cache_read_input: u64,
    ) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            cache_creation_input_tokens: cache_creation_input,
            cache_read_input_tokens: cache_read_input,
            vcpu_ms: 0,
            mem_mib_ms: 0,
        }
    }

    /// Compute usage of one VM run, with no tokens: vCPU-milliseconds and MiB-milliseconds of memory.
    #[must_use]
    pub const fn compute(vcpu_ms: u64, mem_mib_ms: u64) -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            vcpu_ms,
            mem_mib_ms,
        }
    }

    /// The field-by-field sum, or `None` if any field overflows.
    #[must_use]
    pub const fn checked_add(&self, other: &Self) -> Option<Self> {
        let Some(input_tokens) = self.input_tokens.checked_add(other.input_tokens) else {
            return None;
        };
        let Some(output_tokens) = self.output_tokens.checked_add(other.output_tokens) else {
            return None;
        };
        let Some(cache_creation_input_tokens) = self
            .cache_creation_input_tokens
            .checked_add(other.cache_creation_input_tokens)
        else {
            return None;
        };
        let Some(cache_read_input_tokens) = self
            .cache_read_input_tokens
            .checked_add(other.cache_read_input_tokens)
        else {
            return None;
        };
        let Some(vcpu_ms) = self.vcpu_ms.checked_add(other.vcpu_ms) else {
            return None;
        };
        let Some(mem_mib_ms) = self.mem_mib_ms.checked_add(other.mem_mib_ms) else {
            return None;
        };
        Some(Self {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            vcpu_ms,
            mem_mib_ms,
        })
    }

    /// The sum of the four token counts, or `None` if it overflows.
    #[must_use]
    pub const fn total_tokens(&self) -> Option<u64> {
        let Some(sum) = self.input_tokens.checked_add(self.output_tokens) else {
            return None;
        };
        let Some(sum) = sum.checked_add(self.cache_creation_input_tokens) else {
            return None;
        };
        sum.checked_add(self.cache_read_input_tokens)
    }

    /// Uncached input tokens.
    #[must_use]
    pub const fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    /// Output tokens.
    #[must_use]
    pub const fn output_tokens(&self) -> u64 {
        self.output_tokens
    }

    /// Input tokens written to the prompt cache.
    #[must_use]
    pub const fn cache_creation_input_tokens(&self) -> u64 {
        self.cache_creation_input_tokens
    }

    /// Input tokens read from the prompt cache.
    #[must_use]
    pub const fn cache_read_input_tokens(&self) -> u64 {
        self.cache_read_input_tokens
    }

    /// vCPU time, in vCPU-milliseconds.
    #[must_use]
    pub const fn vcpu_ms(&self) -> u64 {
        self.vcpu_ms
    }

    /// Memory time, in MiB-milliseconds.
    #[must_use]
    pub const fn mem_mib_ms(&self) -> u64 {
        self.mem_mib_ms
    }
}
