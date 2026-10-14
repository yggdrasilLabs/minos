//! Money: the `Rate` grammar (exact pico-dollars from strings, failing closed), integer `Usage`, `prices.toml`,
//! exact `Cost` products against published AWS rates, round half up, and the one `ApproxCost` format.
//!
//! Every expected product carries its hand arithmetic in a comment.

use std::error::Error;
use std::str::FromStr;

use minos_core::money::{Rate, RateError};
use minos_core::usage::Usage;
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

fn rejected(text: &str) -> Result<RateError, Box<dyn Error>> {
    match Rate::parse(text) {
        Ok(rate) => Err(format!("{text:?} parsed as {} pico", rate.pico()).into()),
        Err(error) => Ok(error),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Rate grammar
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn rate_parses_a_published_aws_rate_exactly() -> TestResult {
    // 0.0000276944 USD = 27_694_400 * 10^-12 USD.
    assert_eq!(Rate::parse("0.0000276944")?.pico(), 27_694_400);
    Ok(())
}

#[test]
fn rate_parses_one_dollar() -> TestResult {
    assert_eq!(Rate::parse("1")?.pico(), 1_000_000_000_000);
    Ok(())
}

#[test]
fn rate_parses_the_largest_value() -> TestResult {
    assert_eq!(
        Rate::parse("999999.999999999999")?.pico(),
        999_999_999_999_999_999
    );
    Ok(())
}

#[test]
fn rate_parses_one_pico_dollar() -> TestResult {
    assert_eq!(Rate::parse("0.000000000001")?.pico(), 1);
    Ok(())
}

#[test]
fn rate_from_str_matches_parse() -> TestResult {
    assert_eq!(Rate::from_str("0.5")?, Rate::parse("0.5")?);
    Ok(())
}

#[test]
fn rate_rejects_empty() -> TestResult {
    assert!(matches!(rejected("")?, RateError::Empty));
    Ok(())
}

#[test]
fn rate_rejects_signs() -> TestResult {
    assert!(matches!(rejected("-1")?, RateError::Sign));
    assert!(matches!(rejected("+1")?, RateError::Sign));
    Ok(())
}

#[test]
fn rate_rejects_exponents() -> TestResult {
    assert!(matches!(rejected("1e-6")?, RateError::Exponent));
    assert!(matches!(rejected("1E3")?, RateError::Exponent));
    Ok(())
}

#[test]
fn rate_rejects_whitespace() -> TestResult {
    assert!(matches!(rejected(" 1")?, RateError::Whitespace));
    assert!(matches!(rejected("1 ")?, RateError::Whitespace));
    Ok(())
}

#[test]
fn rate_rejects_underscores() -> TestResult {
    assert!(matches!(rejected("1_000")?, RateError::Underscore));
    Ok(())
}

#[test]
fn rate_rejects_an_empty_fraction() -> TestResult {
    assert!(matches!(rejected("1.")?, RateError::EmptyFraction));
    Ok(())
}

#[test]
fn rate_rejects_a_missing_integer() -> TestResult {
    assert!(matches!(rejected(".5")?, RateError::MissingInteger));
    Ok(())
}

#[test]
fn rate_rejects_a_thirteenth_fractional_digit_without_rounding() -> TestResult {
    // Deliberate violation: 13 fractional digits. The rate is refused, never rounded to 1.000000000000.
    let error = rejected("1.0000000000001")?;
    assert!(matches!(error, RateError::FractionTooLong));
    assert_eq!(
        error.to_string(),
        "a rate has at most 12 fractional digits and is never rounded"
    );
    Ok(())
}

#[test]
fn rate_rejects_a_seventh_integer_digit() -> TestResult {
    assert!(matches!(rejected("1234567")?, RateError::IntegerTooLong));
    Ok(())
}

#[test]
fn rate_rejects_other_bytes() -> TestResult {
    assert!(matches!(rejected("0x10")?, RateError::BadByte { index: 1 }));
    assert!(matches!(rejected("1,5")?, RateError::BadByte { index: 1 }));
    assert!(matches!(
        rejected("1.5.3")?,
        RateError::BadByte { index: 3 }
    ));
    Ok(())
}

#[test]
fn rate_display_is_canonical() -> TestResult {
    assert_eq!(Rate::parse("0.0000276944")?.to_string(), "0.0000276944");
    assert_eq!(Rate::parse("1.500000")?.to_string(), "1.5");
    assert_eq!(Rate::parse("007")?.to_string(), "7");
    assert_eq!(Rate::parse("0")?.to_string(), "0");
    assert_eq!(Rate::parse("0.000000000001")?.to_string(), "0.000000000001");
    Ok(())
}

#[test]
fn rate_serde_is_a_string_through_parse() -> TestResult {
    let rate: Rate = serde_json::from_str(r#""0.000003""#)?;
    assert_eq!(rate.pico(), 3_000_000);
    assert_eq!(serde_json::to_string(&rate)?, r#""0.000003""#);
    assert!(serde_json::from_str::<Rate>(r#""1e-6""#).is_err());
    assert!(serde_json::from_str::<Rate>("0.000003").is_err());
    Ok(())
}

proptest! {
    #[test]
    fn rate_display_round_trips(pico in 0_u128..=999_999_999_999_999_999_u128) {
        let shown = Rate::parse(&pico_text(pico)).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(shown.pico(), pico);
        let again = Rate::parse(&shown.to_string()).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(again, shown);
    }
}

/// An independent decimal rendering of a pico value, with all twelve fractional digits.
fn pico_text(pico: u128) -> String {
    format!(
        "{}.{:012}",
        pico / 1_000_000_000_000,
        pico % 1_000_000_000_000
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn usage_adds_tokens_and_compute() -> TestResult {
    let sum = Usage::tokens(1, 2, 3, 4)
        .checked_add(&Usage::compute(5, 6))
        .ok_or("usage addition overflowed")?;
    assert_eq!(sum.input_tokens(), 1);
    assert_eq!(sum.output_tokens(), 2);
    assert_eq!(sum.cache_creation_input_tokens(), 3);
    assert_eq!(sum.cache_read_input_tokens(), 4);
    assert_eq!(sum.vcpu_ms(), 5);
    assert_eq!(sum.mem_mib_ms(), 6);
    assert_eq!(sum.total_tokens(), Some(10));
    Ok(())
}

#[test]
fn usage_addition_overflow_is_none() {
    let full = Usage::tokens(u64::MAX, 0, 0, 0);
    assert_eq!(full.checked_add(&Usage::tokens(1, 0, 0, 0)), None);
    let compute = Usage::compute(0, u64::MAX);
    assert_eq!(compute.checked_add(&Usage::compute(0, 1)), None);
}

#[test]
fn usage_total_tokens_overflow_is_none() {
    assert_eq!(Usage::tokens(u64::MAX, 1, 0, 0).total_tokens(), None);
}

#[test]
fn usage_default_is_zero() {
    assert_eq!(Usage::default(), Usage::tokens(0, 0, 0, 0));
    assert_eq!(Usage::default().total_tokens(), Some(0));
}

#[test]
fn usage_json_has_exactly_the_six_fields() -> TestResult {
    let usage = Usage::tokens(1, 2, 3, 4)
        .checked_add(&Usage::compute(5, 6))
        .ok_or("usage addition overflowed")?;
    let json = serde_json::to_string(&usage)?;
    assert_eq!(
        json,
        r#"{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"vcpu_ms":5,"mem_mib_ms":6}"#
    );
    let back: Usage = serde_json::from_str(&json)?;
    assert_eq!(back, usage);
    Ok(())
}

#[test]
fn usage_rejects_an_unknown_field() {
    let json = r#"{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"vcpu_ms":5,"mem_mib_ms":6,"usd":1}"#;
    assert!(serde_json::from_str::<Usage>(json).is_err());
}

#[test]
fn usage_rejects_a_missing_field() {
    let json = r#"{"input_tokens":1,"output_tokens":2}"#;
    assert!(serde_json::from_str::<Usage>(json).is_err());
}
