//! Money: the `Rate` grammar (exact pico-dollars from strings, failing closed), integer `Usage`, `prices.toml`,
//! exact `Cost` products against published AWS rates, round half up, and the one `ApproxCost` format.
//!
//! Every expected product carries its hand arithmetic in a comment.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::ModelId;
use minos_core::money::{ApproxCost, Cost, CostError, Prices, PricesError, Rate, RateError};
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

// ---------------------------------------------------------------------------------------------------------------
// prices.toml
// ---------------------------------------------------------------------------------------------------------------

const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000003"
output_token = "0.000015"
cache_creation_input_token = "0.00000375"
cache_read_input_token = "0.0000003"
"#;

fn prices(text: &str) -> Result<Prices, Box<dyn Error>> {
    Ok(Prices::parse_toml(text)?)
}

fn prices_rejected(text: &str) -> Result<PricesError, Box<dyn Error>> {
    match Prices::parse_toml(text) {
        Ok(_) => Err("the price table was accepted".into()),
        Err(error) => Ok(error),
    }
}

fn model(text: &str) -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str(text)?)
}

/// A price table with one compute rate pair and one model whose four token rates are all `token`.
fn table(vcpu: &str, gib: &str, token: &str) -> Result<Prices, Box<dyn Error>> {
    prices(&format!(
        "v = 1\n[compute]\nvcpu_second = \"{vcpu}\"\ngib_second = \"{gib}\"\n\
         [models.m]\ninput_token = \"{token}\"\noutput_token = \"{token}\"\n\
         cache_creation_input_token = \"{token}\"\ncache_read_input_token = \"{token}\"\n"
    ))
}

#[test]
fn prices_parse_a_complete_table() -> TestResult {
    let table = prices(PRICES)?;
    assert_eq!(table.compute().vcpu_second().pico(), 27_694_400);
    assert_eq!(table.compute().gib_second().pico(), 3_666_700);
    let haiku = table.model(&model("haiku")?).ok_or("haiku is missing")?;
    assert_eq!(haiku.input_token().pico(), 3_000_000);
    assert_eq!(haiku.output_token().pico(), 15_000_000);
    assert_eq!(haiku.cache_creation_input_token().pico(), 3_750_000);
    assert_eq!(haiku.cache_read_input_token().pico(), 300_000);
    assert_eq!(table.models().count(), 1);
    assert!(table.model(&model("opus")?).is_none());
    Ok(())
}

#[test]
fn numeric_rate_is_a_located_type_error() -> TestResult {
    let text = PRICES.replace(r#"vcpu_second = "0.0000276944""#, "vcpu_second = 0.0000276944");
    let error = prices_rejected(&text)?;
    assert!(matches!(error, PricesError::Toml(_)), "got: {error}");
    let shown = match &error {
        PricesError::Toml(source) => source.to_string(),
        other => other.to_string(),
    };
    assert!(shown.contains("line 4"), "got: {shown}");
    assert!(shown.contains("floating point"), "got: {shown}");
    assert!(shown.contains("expected a string"), "got: {shown}");
    Ok(())
}

#[test]
fn prices_reject_an_unknown_key() -> TestResult {
    let text = PRICES.replace("gib_second", "gib_second_x = \"1\"\ngib_second");
    let error = prices_rejected(&text)?;
    assert!(error.to_string().contains("unknown field"), "got: {error}");
    Ok(())
}

#[test]
fn prices_reject_another_version() -> TestResult {
    let error = prices_rejected(&PRICES.replacen("v = 1", "v = 2", 1))?;
    assert!(matches!(error, PricesError::Version { found: 2 }), "got: {error}");
    Ok(())
}

#[test]
fn prices_reject_a_duplicate_model_table() -> TestResult {
    let text = format!("{PRICES}\n[models.\"haiku\"]\ninput_token = \"1\"\n");
    assert!(matches!(prices_rejected(&text)?, PricesError::Toml(_)));
    Ok(())
}

#[test]
fn prices_reject_a_bad_rate_string() -> TestResult {
    let text = PRICES.replace("0.0000276944", "1.0000000000001");
    assert!(matches!(prices_rejected(&text)?, PricesError::Toml(_)));
    Ok(())
}

#[test]
fn prices_reject_a_bad_model_id() -> TestResult {
    let text = PRICES.replace(r#"[models."haiku"]"#, r#"[models."-bad"]"#);
    assert!(matches!(prices_rejected(&text)?, PricesError::Toml(_)));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Cost
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn aws_rate_products_are_exact() -> TestResult {
    // vCPU: 27_694_400 pico/s * 3_600 s = 99_699_840_000 pico = 99_699.84 micro, rounds half up to 99_700.
    let table = table("0.0000276944", "0", "0")?;
    let vcpu = Cost::of(&table, None, &Usage::compute(3_600_000, 0))?;
    assert_eq!(vcpu.micro_dollars(), 99_700);
    assert_eq!(vcpu.approx().to_string(), "approx. cost: $0.10");

    // Memory: one GiB-second (1_024 MiB * 1_000 ms = 1_024_000 MiB-ms) at 3_666_700 pico = 3.6667 micro, so 4.
    let table = table("0", "0.0000036667", "0")?;
    let one = Cost::of(&table, None, &Usage::compute(0, 1_024_000))?;
    assert_eq!(one.micro_dollars(), 4);

    // Lambda: 1 GiB for 3_600 s = 3_600 GiB-s * 16_666_700 pico = 60_000_120_000 pico = 60_000.12 micro, so 60_000.
    let table = table("0", "0.0000166667", "0")?;
    let hour = Cost::of(&table, None, &Usage::compute(0, 1_024 * 3_600_000))?;
    assert_eq!(hour.micro_dollars(), 60_000);

    // Tokens: 1_000_000 input tokens * 3_000_000 pico = 3 * 10^12 pico = 3_000_000 micro, exactly $3.
    let table = table("0", "0", "0.000003")?;
    let tokens = Cost::of(&table, Some(&model("m")?), &Usage::tokens(1_000_000, 0, 0, 0))?;
    assert_eq!(tokens.micro_dollars(), 3_000_000);
    assert_eq!(tokens.approx().to_string(), "approx. cost: $3.00");
    Ok(())
}

#[test]
fn every_token_kind_is_priced() -> TestResult {
    // input 1 * 3 + output 2 * 15 + cache creation 4 * 3.75 + cache read 10 * 0.3 = 3 + 30 + 15 + 3 = 51 micro.
    let table = prices(PRICES)?;
    let cost = Cost::of(&table, Some(&model("haiku")?), &Usage::tokens(1, 2, 4, 10))?;
    assert_eq!(cost.micro_dollars(), 51);
    Ok(())
}

#[test]
fn rounds_half_up() -> TestResult {
    // A token rate of 500_000 pico is exactly half a micro-dollar per token: rounds up to 1.
    let half = Cost::of(&table("0", "0", "0.0000005")?, Some(&model("m")?), &Usage::tokens(1, 0, 0, 0))?;
    assert_eq!(half.micro_dollars(), 1);
    // A GiB-second rate of 1 pico times 512_000_000_000 MiB-ms is 500_000 pico, again exactly half: 1.
    let table = table("0", "0.000000000001", "0")?;
    let also_half = Cost::of(&table, None, &Usage::compute(0, 512_000_000_000))?;
    assert_eq!(also_half.micro_dollars(), 1);
    // One scaled unit less than half (1/1_024_000 pico below 500_000 pico) rounds down to 0.
    let below = Cost::of(&table, None, &Usage::compute(0, 511_999_999_999))?;
    assert_eq!(below.micro_dollars(), 0);
    Ok(())
}

#[test]
fn zero_usage_costs_zero() -> TestResult {
    let cost = Cost::of(&prices(PRICES)?, None, &Usage::default())?;
    assert_eq!(cost, Cost::ZERO);
    assert_eq!(cost.micro_dollars(), 0);
    assert_eq!(cost.approx().to_string(), "approx. cost: $0.00");
    Ok(())
}

#[test]
fn tokens_without_a_model_are_refused() -> TestResult {
    let result = Cost::of(&prices(PRICES)?, None, &Usage::tokens(0, 1, 0, 0));
    assert!(matches!(result, Err(CostError::TokensWithoutModel)));
    Ok(())
}

#[test]
fn an_unpriced_model_is_refused() -> TestResult {
    let opus = model("opus")?;
    let result = Cost::of(&prices(PRICES)?, Some(&opus), &Usage::tokens(1, 0, 0, 0));
    assert!(matches!(result, Err(CostError::UnpricedModel { model }) if model == opus));
    Ok(())
}

#[test]
fn extreme_inputs_overflow_without_panic() -> TestResult {
    let table = table("999999.999999999999", "999999.999999999999", "999999.999999999999")?;
    let tokens = Usage::tokens(u64::MAX, u64::MAX, u64::MAX, u64::MAX);
    let result = Cost::of(&table, Some(&model("m")?), &tokens);
    assert!(matches!(result, Err(CostError::Overflow)));
    let compute = Usage::compute(u64::MAX, u64::MAX);
    assert!(matches!(Cost::of(&table, None, &compute), Err(CostError::Overflow)));
    Ok(())
}

#[test]
fn cost_addition_overflow_is_an_error() -> TestResult {
    let table = table("999999.999999999999", "0", "0")?;
    let big = Cost::of(&table, None, &Usage::compute(u64::MAX, 0))?;
    let mut total = Cost::ZERO;
    let mut overflowed = false;
    for _ in 0..1_000_000 {
        match total.checked_add(big) {
            Ok(next) => total = next,
            Err(CostError::Overflow) => {
                overflowed = true;
                break;
            }
            Err(other) => return Err(other.into()),
        }
    }
    assert!(overflowed);
    Ok(())
}

#[test]
fn approx_cost_rounds_cents_half_up() {
    assert_eq!(ApproxCost::from_micro(31_400_000).to_string(), "approx. cost: $31.40");
    assert_eq!(ApproxCost::from_micro(4_999).to_string(), "approx. cost: $0.00");
    assert_eq!(ApproxCost::from_micro(5_000).to_string(), "approx. cost: $0.01");
    assert_eq!(ApproxCost::from_micro(995_000).to_string(), "approx. cost: $1.00");
    assert_eq!(ApproxCost::from_micro(5_000).micro(), 5_000);
    let max = ApproxCost::from_micro(u128::MAX).to_string();
    assert!(max.starts_with("approx. cost: $"), "got: {max}");
}

fn small_usage() -> impl Strategy<Value = Usage> {
    (
        0_u64..1 << 40,
        0_u64..1 << 40,
        0_u64..1 << 40,
        0_u64..1 << 40,
        0_u64..1 << 40,
        0_u64..1 << 40,
    )
        .prop_map(|(input, output, creation, read, vcpu, mem)| {
            let tokens = Usage::tokens(input, output, creation, read);
            tokens.checked_add(&Usage::compute(vcpu, mem)).unwrap_or(tokens)
        })
}

fn priced(usage: &Usage) -> Result<Cost, TestCaseError> {
    let table = prices(PRICES).map_err(|error| TestCaseError::fail(error.to_string()))?;
    let haiku = model("haiku").map_err(|error| TestCaseError::fail(error.to_string()))?;
    Cost::of(&table, Some(&haiku), usage).map_err(|error| TestCaseError::fail(error.to_string()))
}

proptest! {
    #[test]
    fn micro_dollars_monotone(a in small_usage(), b in small_usage()) {
        let sum = a.checked_add(&b).ok_or_else(|| TestCaseError::fail("usage overflowed"))?;
        prop_assert!(priced(&a)?.micro_dollars() <= priced(&sum)?.micro_dollars());
        prop_assert!(priced(&a)? <= priced(&sum)?);
    }

    #[test]
    fn cost_addition_commutes(a in small_usage(), b in small_usage(), c in small_usage()) {
        let (a, b, c) = (priced(&a)?, priced(&b)?, priced(&c)?);
        let fail = |error: CostError| TestCaseError::fail(error.to_string());
        prop_assert_eq!(a.checked_add(b).map_err(fail)?, b.checked_add(a).map_err(fail)?);
        let left = a.checked_add(b).and_then(|ab| ab.checked_add(c)).map_err(fail)?;
        let right = b.checked_add(c).and_then(|bc| a.checked_add(bc)).map_err(fail)?;
        prop_assert_eq!(left, right);
    }

    #[test]
    fn approx_cost_format(micro in any::<u128>()) {
        let shown = ApproxCost::from_micro(micro).to_string();
        let amount = shown.strip_prefix("approx. cost: $").ok_or_else(|| TestCaseError::fail(shown.clone()))?;
        let (dollars, cents) = amount.split_once('.').ok_or_else(|| TestCaseError::fail(shown.clone()))?;
        prop_assert!(!dollars.is_empty() && dollars.bytes().all(|byte| byte.is_ascii_digit()), "{}", shown);
        prop_assert!(cents.len() == 2 && cents.bytes().all(|byte| byte.is_ascii_digit()), "{}", shown);
    }
}
