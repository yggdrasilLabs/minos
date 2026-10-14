//! Exact money arithmetic: pico-dollar `Rate`, `Prices`, `Cost`, and the one `ApproxCost` display type.
//!
//! No floats touch a dollar value. A rate is parsed from a decimal string into an exact integer count of
//! pico-dollars (10^-12 USD) per unit, and parsing fails closed: a rate with more than
//! [`RATE_FRACTION_DIGITS`] fractional digits is refused, never rounded. Published AWS rates have 10 fractional
//! digits (for example `0.0000276944` per vCPU-second), which nano-dollars could not represent.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Pico-dollars in one US dollar.
pub const PICO_PER_DOLLAR: u128 = 1_000_000_000_000;
/// Most fractional digits a rate may have: pico-dollar precision.
pub const RATE_FRACTION_DIGITS: usize = 12;
/// Most integer digits a rate may have. No real per-unit rate is near a million dollars, and the bound keeps
/// every rate below 10^18 pico-dollars.
pub const RATE_MAX_INTEGER_DIGITS: usize = 6;

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
