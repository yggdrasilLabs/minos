//! Exact money arithmetic: pico-dollar `Rate`, `Prices`, `Cost`, and the one `ApproxCost` display type.
//!
//! No floats touch a dollar value. A rate is parsed from a decimal string into an exact integer count of
//! pico-dollars (10^-12 USD) per unit, and parsing fails closed: a rate with more than
//! [`RATE_FRACTION_DIGITS`] fractional digits is refused, never rounded. Published AWS rates have 10 fractional
//! digits (for example `0.0000276944` per vCPU-second), which nano-dollars could not represent.
//!
//! The ledger stores [`Usage`] only (COST-01). A [`Cost`] is derived from usage and the user's [`Prices`] table
//! in one scaled integer with a single deferred division, and [`ApproxCost`] is the only type that formats a
//! dollar figure.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::ModelId;
use crate::manifest::Manifest;
use crate::usage::Usage;

/// Pico-dollars in one US dollar.
pub const PICO_PER_DOLLAR: u128 = 1_000_000_000_000;
/// Most fractional digits a rate may have: pico-dollar precision.
pub const RATE_FRACTION_DIGITS: usize = 12;
/// Most integer digits a rate may have. No real per-unit rate is near a million dollars, and the bound keeps
/// every rate below 10^18 pico-dollars.
pub const RATE_MAX_INTEGER_DIGITS: usize = 6;
/// The only `prices.toml` format version this build reads.
pub const PRICES_VERSION: u32 = 1;
/// Scaled cost units per pico-dollar: 1000 milliseconds per second times 1024 MiB per GiB. Keeping both factors
/// in the integer until the end means per-second and per-GiB rates applied to millisecond and MiB usage lose no
/// precision.
const SCALE_PER_PICO: u128 = 1_024_000;
/// Scaled cost units per micro-dollar: 10^6 pico-dollars times [`SCALE_PER_PICO`].
const SCALE_PER_MICRO: u128 = 1_024_000_000_000;
/// Half a micro-dollar in scaled units, the round-half-up threshold.
const HALF_MICRO: u128 = 512_000_000_000;
/// Micro-dollars per cent.
const MICRO_PER_CENT: u128 = 10_000;
/// Half a cent in micro-dollars, the display's round-half-up threshold.
const HALF_CENT: u128 = 5_000;
/// MiB per GiB, the factor that turns a per-GiB-second rate into the scaled unit for vCPU-milliseconds.
const MIB_PER_GIB: u128 = 1_024;

// ---------------------------------------------------------------------------------------------------------------
// Rate
// ---------------------------------------------------------------------------------------------------------------

/// A price per unit, in exact pico-dollars, parsed from a decimal string.
///
/// The grammar is `[0-9]{1,6}(\.[0-9]{1,12})?`: no sign, no exponent, no whitespace, no digit separators. Serde
/// reads and writes it as a string through [`Rate::parse`], so a TOML or JSON number is a type error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rate(u128);

/// Why a rate string did not parse. It carries no input text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RateError {
    /// The rate is empty.
    #[error("a rate cannot be empty")]
    Empty,
    /// The rate has a `+` or `-` sign.
    #[error("a rate has no sign")]
    Sign,
    /// The rate has an exponent (`e` or `E`).
    #[error("a rate is a plain decimal with no exponent")]
    Exponent,
    /// The rate contains whitespace.
    #[error("a rate contains no whitespace")]
    Whitespace,
    /// The rate contains an `_` digit separator.
    #[error("a rate contains no digit separators")]
    Underscore,
    /// A byte other than a digit or one decimal point.
    #[error("the byte at index {index} is not allowed in a rate")]
    BadByte {
        /// Zero-based byte index of the first offending byte.
        index: usize,
    },
    /// The rate starts with the decimal point.
    #[error("a rate needs at least one digit before the decimal point")]
    MissingInteger,
    /// The rate ends with the decimal point.
    #[error("a rate needs at least one digit after the decimal point")]
    EmptyFraction,
    /// More than [`RATE_MAX_INTEGER_DIGITS`] integer digits.
    #[error("a rate has at most 6 integer digits")]
    IntegerTooLong,
    /// More than [`RATE_FRACTION_DIGITS`] fractional digits.
    #[error("a rate has at most 12 fractional digits and is never rounded")]
    FractionTooLong,
    /// The value does not fit. Unreachable within the digit bounds, but checked rather than assumed.
    #[error("a rate is too large")]
    Overflow,
}

impl Rate {
    /// Parses a rate in US dollars per unit, such as `0.0000276944`.
    ///
    /// # Errors
    ///
    /// Returns a [`RateError`] naming the first rule the text breaks. A rate is never rounded.
    pub fn parse(text: &str) -> Result<Self, RateError> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Err(RateError::Empty);
        }
        if bytes.iter().any(u8::is_ascii_whitespace) {
            return Err(RateError::Whitespace);
        }
        if bytes.iter().any(|byte| matches!(byte, b'e' | b'E')) {
            return Err(RateError::Exponent);
        }
        if bytes.iter().any(|byte| matches!(byte, b'+' | b'-')) {
            return Err(RateError::Sign);
        }
        if bytes.contains(&b'_') {
            return Err(RateError::Underscore);
        }
        if let Some(index) = bytes
            .iter()
            .position(|byte| !byte.is_ascii_digit() && *byte != b'.')
        {
            return Err(RateError::BadByte { index });
        }
        let (integer, fraction) = match text.split_once('.') {
            Some((integer, fraction)) => (integer, Some(fraction)),
            None => (text, None),
        };
        if integer.is_empty() {
            return Err(RateError::MissingInteger);
        }
        if let Some(offset) = fraction.and_then(|fraction| fraction.find('.')) {
            return Err(RateError::BadByte {
                index: integer.len() + 1 + offset,
            });
        }
        if integer.len() > RATE_MAX_INTEGER_DIGITS {
            return Err(RateError::IntegerTooLong);
        }
        let fraction = fraction.unwrap_or("");
        if text.ends_with('.') {
            return Err(RateError::EmptyFraction);
        }
        if fraction.len() > RATE_FRACTION_DIGITS {
            return Err(RateError::FractionTooLong);
        }
        let whole = digits_value(integer)?;
        let mut part = digits_value(fraction)?;
        for _ in fraction.len()..RATE_FRACTION_DIGITS {
            part = part.checked_mul(10).ok_or(RateError::Overflow)?;
        }
        whole
            .checked_mul(PICO_PER_DOLLAR)
            .and_then(|pico| pico.checked_add(part))
            .map(Self)
            .ok_or(RateError::Overflow)
    }

    /// The rate in pico-dollars (10^-12 USD) per unit.
    #[must_use]
    pub const fn pico(self) -> u128 {
        self.0
    }
}

/// The value of a run of ASCII digits, already checked to be digits only.
fn digits_value(digits: &str) -> Result<u128, RateError> {
    digits.bytes().try_fold(0_u128, |value, byte| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u128::from(byte - b'0')))
            .ok_or(RateError::Overflow)
    })
}

impl FromStr for Rate {
    type Err = RateError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for Rate {
    type Error = RateError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl From<Rate> for String {
    fn from(rate: Rate) -> Self {
        rate.to_string()
    }
}

impl fmt::Display for Rate {
    /// The canonical decimal: no leading zeros in the integer part, trailing fractional zeros trimmed, and no
    /// decimal point when the fraction is zero. Parsing the output gives the same rate.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / PICO_PER_DOLLAR;
        let part = self.0 % PICO_PER_DOLLAR;
        if part == 0 {
            return write!(f, "{whole}");
        }
        let fraction = format!("{part:012}");
        write!(f, "{whole}.{}", fraction.trim_end_matches('0'))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Prices
// ---------------------------------------------------------------------------------------------------------------

/// Compute rates: US dollars per vCPU-second and per GiB-second of memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeRates {
    vcpu_second: Rate,
    gib_second: Rate,
}

impl ComputeRates {
    /// Dollars per vCPU-second.
    #[must_use]
    pub const fn vcpu_second(&self) -> Rate {
        self.vcpu_second
    }

    /// Dollars per GiB-second of memory.
    #[must_use]
    pub const fn gib_second(&self) -> Rate {
        self.gib_second
    }
}

/// One model's token rates: US dollars per token of each kind in [`Usage`].
///
/// The TOML keys are `input_token`, `output_token`, `cache_creation_input_token`, and `cache_read_input_token`;
/// the private fields drop the shared suffix (pedantic `struct_field_names`) and serde renames them back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenRates {
    #[serde(rename = "input_token")]
    input: Rate,
    #[serde(rename = "output_token")]
    output: Rate,
    #[serde(rename = "cache_creation_input_token")]
    cache_creation_input: Rate,
    #[serde(rename = "cache_read_input_token")]
    cache_read_input: Rate,
}

impl TokenRates {
    /// Dollars per uncached input token.
    #[must_use]
    pub const fn input_token(&self) -> Rate {
        self.input
    }

    /// Dollars per output token.
    #[must_use]
    pub const fn output_token(&self) -> Rate {
        self.output
    }

    /// Dollars per input token written to the prompt cache.
    #[must_use]
    pub const fn cache_creation_input_token(&self) -> Rate {
        self.cache_creation_input
    }

    /// Dollars per input token read from the prompt cache.
    #[must_use]
    pub const fn cache_read_input_token(&self) -> Rate {
        self.cache_read_input
    }
}

/// `prices.toml` as written, before the version check.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPrices {
    v: u32,
    compute: ComputeRates,
    models: BTreeMap<ModelId, TokenRates>,
}

/// The user's price table: compute rates plus token rates per model.
///
/// It lives at `~/.config/minos/prices.toml`, which the CLI reads and passes in as text; core never reads a file.
/// Every rate is a TOML string, so a numeric literal such as `vcpu_second = 0.0000276944` is a type error that
/// names its line and column instead of a float that silently loses digits (COST-01). `PricesId::of` over the
/// file bytes identifies the table a figure was priced with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prices {
    compute: ComputeRates,
    models: BTreeMap<ModelId, TokenRates>,
}

/// Why a price table did not parse.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PricesError {
    /// TOML syntax, a missing or unknown key, a numeric rate, or a rate that breaks the [`Rate`] grammar.
    ///
    /// `prices.toml` is the user's own file, not hostile input, so toml's located message is kept whole.
    #[error("prices.toml: {0}")]
    Toml(#[source] toml::de::Error),
    /// The `v` field names a version this build does not read.
    #[error("prices.toml version {found} is not supported; this build reads version 1")]
    Version {
        /// The version found in the file.
        found: u32,
    },
}

impl TryFrom<RawPrices> for Prices {
    type Error = PricesError;

    fn try_from(raw: RawPrices) -> Result<Self, Self::Error> {
        if raw.v != PRICES_VERSION {
            return Err(PricesError::Version { found: raw.v });
        }
        Ok(Self {
            compute: raw.compute,
            models: raw.models,
        })
    }
}

impl Prices {
    /// Parses the text of a `prices.toml` file.
    ///
    /// # Errors
    ///
    /// Returns [`PricesError::Toml`] for a syntax, key, type, or rate error, with its location, and
    /// [`PricesError::Version`] for an unsupported `v`.
    pub fn parse_toml(text: &str) -> Result<Self, PricesError> {
        let raw: RawPrices = toml::from_str(text).map_err(PricesError::Toml)?;
        Self::try_from(raw)
    }

    /// The compute rates.
    #[must_use]
    pub const fn compute(&self) -> &ComputeRates {
        &self.compute
    }

    /// The token rates of one model, if the table prices it.
    #[must_use]
    pub fn model(&self, model: &ModelId) -> Option<&TokenRates> {
        self.models.get(model)
    }

    /// Every priced model with its rates, in model order.
    pub fn models(&self) -> impl Iterator<Item = (&ModelId, &TokenRates)> {
        self.models.iter()
    }

    /// Every model a `[[proposer]]` of `manifest` names that this table does not price, sorted and without
    /// duplicates (07-CONTEXT D-13).
    ///
    /// The CLI turns a non-empty set into a refusal naming `prices.toml` and each model, before any VM launch or
    /// model call, so a run is never priced at zero for a missing rate. Compute rates need no check here:
    /// [`Prices::parse_toml`] already requires the whole `[compute]` table. The manifest names model aliases,
    /// while `Spent` records the resolved model ID a call reports, so the loop checks again after each call
    /// (07-RESEARCH Pitfall 7).
    #[must_use]
    pub fn missing_models(&self, manifest: &Manifest) -> BTreeSet<ModelId> {
        manifest
            .proposers()
            .iter()
            .filter_map(|proposer| proposer.model())
            .filter(|model| self.model(model).is_none())
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Cost
// ---------------------------------------------------------------------------------------------------------------

/// A metered cost, held exactly as one scaled integer.
///
/// The arithmetic, with every multiplication and addition checked:
///
/// ```text
/// scaled = vcpu_second_pico * vcpu_ms * 1024
///        + gib_second_pico * mem_mib_ms
///        + sum over the four token kinds of token_rate_pico * tokens * 1_024_000
/// ```
///
/// The unit of `scaled` is 10^-12 USD divided by `1_024_000` (1000 ms per second times 1024 MiB per GiB), so a
/// per-second rate times milliseconds and a per-GiB rate times MiB are both exact. The only division happens once,
/// in [`Cost::micro_dollars`]: `(scaled + 512_000_000_000) / 1_024_000_000_000`, rounding half up. Costs add in
/// the scaled unit, so a sum of many small costs is rounded once, not once per term.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cost {
    scaled: u128,
}

/// Why a cost could not be computed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CostError {
    /// The usage has tokens but names no model to price them with.
    #[error("token usage names no model to price it with")]
    TokensWithoutModel,
    /// The price table has no rates for the model.
    #[error("prices.toml has no rates for model {model}")]
    UnpricedModel {
        /// The model the table does not price.
        model: ModelId,
    },
    /// The cost does not fit in 128 bits.
    #[error("the cost overflows")]
    Overflow,
}

/// `a * b * c`, checked.
fn product(rate: Rate, count: u64, scale: u128) -> Result<u128, CostError> {
    rate.pico()
        .checked_mul(u128::from(count))
        .and_then(|value| value.checked_mul(scale))
        .ok_or(CostError::Overflow)
}

impl Cost {
    /// No cost.
    pub const ZERO: Self = Self { scaled: 0 };

    /// Prices a usage: compute time always, and tokens with the named model's rates.
    ///
    /// # Errors
    ///
    /// Returns [`CostError::TokensWithoutModel`] when the usage has tokens and `model` is `None`,
    /// [`CostError::UnpricedModel`] when `model` is not in the table, and [`CostError::Overflow`] when the cost
    /// does not fit. Nothing panics or wraps.
    pub fn of(prices: &Prices, model: Option<&ModelId>, usage: &Usage) -> Result<Self, CostError> {
        let compute = prices.compute();
        let mut cost = Self {
            scaled: product(compute.vcpu_second(), usage.vcpu_ms(), MIB_PER_GIB)?,
        }
        .checked_add(Self {
            scaled: product(compute.gib_second(), usage.mem_mib_ms(), 1)?,
        })?;
        let counts = [
            usage.input_tokens(),
            usage.output_tokens(),
            usage.cache_creation_input_tokens(),
            usage.cache_read_input_tokens(),
        ];
        let Some(model) = model else {
            if counts.iter().any(|count| *count != 0) {
                return Err(CostError::TokensWithoutModel);
            }
            return Ok(cost);
        };
        let rates = prices
            .model(model)
            .ok_or_else(|| CostError::UnpricedModel {
                model: model.clone(),
            })?;
        let rates = [
            rates.input_token(),
            rates.output_token(),
            rates.cache_creation_input_token(),
            rates.cache_read_input_token(),
        ];
        for (rate, count) in rates.into_iter().zip(counts) {
            cost = cost.checked_add(Self {
                scaled: product(rate, count, SCALE_PER_PICO)?,
            })?;
        }
        Ok(cost)
    }

    /// The sum of two costs, still unrounded.
    ///
    /// # Errors
    ///
    /// Returns [`CostError::Overflow`] when the sum does not fit.
    pub const fn checked_add(self, other: Self) -> Result<Self, CostError> {
        match self.scaled.checked_add(other.scaled) {
            Some(scaled) => Ok(Self { scaled }),
            None => Err(CostError::Overflow),
        }
    }

    /// The cost in whole micro-dollars, rounded half up. The remainder form cannot overflow.
    #[must_use]
    pub const fn micro_dollars(&self) -> u128 {
        let whole = self.scaled / SCALE_PER_MICRO;
        if self.scaled % SCALE_PER_MICRO >= HALF_MICRO {
            whole + 1
        } else {
            whole
        }
    }

    /// The cost as the one displayable dollar figure.
    #[must_use]
    pub const fn approx(&self) -> ApproxCost {
        ApproxCost::from_micro(self.micro_dollars())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// ApproxCost
// ---------------------------------------------------------------------------------------------------------------

/// A measured cost ready to show: `approx. cost: $31.40`.
///
/// This is the only type in the workspace that formats a dollar amount, so no command prints a bare dollar
/// figure. It always says "approx." because the price table is the user's own and may lag the provider; it never
/// prints an estimate and never uses the word "exact". [`CostLine`] puts the token counts beside it (COST-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApproxCost {
    micro: u128,
}

impl ApproxCost {
    /// A cost of `micro` micro-dollars.
    #[must_use]
    pub const fn from_micro(micro: u128) -> Self {
        Self { micro }
    }

    /// The cost in micro-dollars.
    #[must_use]
    pub const fn micro(&self) -> u128 {
        self.micro
    }
}

impl fmt::Display for ApproxCost {
    /// Rounds to cents half up, then writes `approx. cost: $D.CC`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cents = self.micro / MICRO_PER_CENT;
        if self.micro % MICRO_PER_CENT >= HALF_CENT {
            cents += 1;
        }
        write!(f, "approx. cost: ${}.{:02}", cents / 100, cents % 100)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// CostLine
// ---------------------------------------------------------------------------------------------------------------

/// The one cost line every command prints (COST-02): `approx. cost: $31.40 (in 1500 tokens, out 456 tokens)`.
///
/// The dollar figure is written by [`ApproxCost`]'s own `Display`, so `ApproxCost` stays the only type that
/// formats a dollar amount; this type only adds the token counts beside it. "in" counts every input-side token
/// the provider metered, cached or not (input, cache creation, and cache read), and "out" counts output tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostLine {
    cost: ApproxCost,
    input_tokens: u64,
    output_tokens: u64,
}

impl CostLine {
    /// The line for `cost` and the tokens of `usage`. The input-side sum saturates rather than wraps.
    #[must_use]
    pub const fn new(cost: ApproxCost, usage: &Usage) -> Self {
        Self {
            cost,
            input_tokens: usage
                .input_tokens()
                .saturating_add(usage.cache_creation_input_tokens())
                .saturating_add(usage.cache_read_input_tokens()),
            output_tokens: usage.output_tokens(),
        }
    }

    /// The dollar figure.
    #[must_use]
    pub const fn cost(&self) -> ApproxCost {
        self.cost
    }

    /// Every input-side token: input, cache creation, and cache read, saturating.
    #[must_use]
    pub const fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    /// Output tokens.
    #[must_use]
    pub const fn output_tokens(&self) -> u64 {
        self.output_tokens
    }
}

impl fmt::Display for CostLine {
    /// Writes the [`ApproxCost`] through its own `Display`, then ` (in N tokens, out M tokens)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (in {} tokens, out {} tokens)",
            self.cost, self.input_tokens, self.output_tokens
        )
    }
}
