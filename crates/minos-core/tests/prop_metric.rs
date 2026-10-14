//! `Metric` (finite, `-0.0` normalized, ordered by `total_cmp`, bit-exact through JSON), `Scorecard` (versioned
//! array wire form, duplicate and oversize rejection, no hostile text in errors), and `Outcome` round trips.
//!
//! Floats are compared with `to_bits`, never `==`.

use std::cmp::Ordering;
use std::error::Error;
use std::str::FromStr;

use minos_core::id::MetricName;
use minos_core::limits::{Limits, MAX_METRICS, MAX_SCORECARD_BYTES};
use minos_core::metric::{JsonCategory, Metric, MetricError, Scorecard, ScorecardError};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const VALID: &str =
    r#"{"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false}]}"#;

fn name(text: &str) -> Result<MetricName, Box<dyn Error>> {
    Ok(MetricName::from_str(text)?)
}

fn parsed(json: &str) -> Result<Scorecard, Box<dyn Error>> {
    Ok(Scorecard::parse_json(json.as_bytes(), &Limits::standard())?)
}

fn rejected_bytes(bytes: &[u8]) -> Result<ScorecardError, Box<dyn Error>> {
    match Scorecard::parse_json(bytes, &Limits::standard()) {
        Ok(_) => Err(format!("{} scorecard bytes accepted", bytes.len()).into()),
        Err(error) => Ok(error),
    }
}

fn rejected(json: &str) -> Result<ScorecardError, Box<dyn Error>> {
    rejected_bytes(json.as_bytes())
}

fn finite_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>().prop_filter("finite", |x| x.is_finite()),
        any::<u64>()
            .prop_map(f64::from_bits)
            .prop_filter("finite", |x| x.is_finite()),
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// Metric
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn metric_rejects_nan_with_a_fixed_message() {
    let result = Metric::new(f64::NAN);
    assert!(matches!(result, Err(MetricError::NotFinite)));
    if let Err(error) = result {
        assert_eq!(error.to_string(), "a metric must be a finite number");
    }
}

#[test]
fn metric_rejects_both_infinities() {
    assert!(matches!(
        Metric::new(f64::INFINITY),
        Err(MetricError::NotFinite)
    ));
    assert!(matches!(
        Metric::new(f64::NEG_INFINITY),
        Err(MetricError::NotFinite)
    ));
}

#[test]
fn metric_maps_negative_zero_to_zero() -> TestResult {
    let metric = Metric::new(-0.0)?;
    assert_eq!(metric.value().to_bits(), 0.0_f64.to_bits());
    assert_eq!(metric, Metric::ZERO);
    assert_eq!(serde_json::to_string(&metric)?, "0.0");
    Ok(())
}

#[test]
fn metric_constants() {
    assert_eq!(Metric::ZERO.value().to_bits(), 0.0_f64.to_bits());
    assert_eq!(Metric::ONE.value().to_bits(), 1.0_f64.to_bits());
    assert_eq!(Metric::ZERO.cmp(&Metric::ONE), Ordering::Less);
}

#[test]
fn metric_display_and_debug_print_the_value() -> TestResult {
    let metric = Metric::new(12.5)?;
    assert_eq!(metric.to_string(), "12.5");
    assert_eq!(format!("{metric:?}"), "12.5");
    Ok(())
}

#[test]
fn metric_json_rejects_non_finite_and_non_numbers() {
    for json in [
        "NaN",
        "Infinity",
        "-Infinity",
        "1e400",
        "-1e400",
        "\"1.0\"",
        "null",
        "true",
    ] {
        assert!(
            serde_json::from_str::<Metric>(json).is_err(),
            "{json} parsed as a metric"
        );
    }
}

#[test]
fn metric_json_accepts_an_integer() -> TestResult {
    let metric: Metric = serde_json::from_str("12")?;
    assert_eq!(metric.value().to_bits(), 12.0_f64.to_bits());
    Ok(())
}

#[test]
fn metric_json_negative_zero_parses_to_zero() -> TestResult {
    let metric: Metric = serde_json::from_str("-0.0")?;
    assert_eq!(metric.value().to_bits(), 0.0_f64.to_bits());
    Ok(())
}

#[test]
fn metric_json_round_trips_extreme_literals() -> TestResult {
    for x in [
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
        0.1,
        969_660.516_061_324_3,
        -1.5e-310,
        1e300,
    ] {
        let metric = Metric::new(x)?;
        let json = serde_json::to_string(&metric)?;
        let back: Metric = serde_json::from_str(&json)?;
        assert_eq!(back.value().to_bits(), x.to_bits(), "{json}");
    }
    Ok(())
}

proptest! {
    #[test]
    fn ord_agrees_with_total_cmp(a in finite_f64(), b in finite_f64()) {
        let left = Metric::new(a).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let right = Metric::new(b).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(left.cmp(&right), left.value().total_cmp(&right.value()));
        prop_assert_eq!(left.partial_cmp(&right), Some(left.cmp(&right)));
        prop_assert_eq!(left == right, left.value().to_bits() == right.value().to_bits());
    }

    #[test]
    fn json_round_trip_is_bit_exact(x in finite_f64()) {
        let metric = Metric::new(x).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let json = serde_json::to_string(&metric).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let back: Metric = serde_json::from_str(&json).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(back.value().to_bits(), metric.value().to_bits());
    }

    #[test]
    fn non_finite_bit_patterns_are_rejected(bits in any::<u64>()) {
        let x = f64::from_bits(bits);
        prop_assert_eq!(Metric::new(x).is_ok(), x.is_finite());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Scorecard
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn scorecard_accepts_a_valid_card() -> TestResult {
    let card = parsed(VALID)?;
    let throughput = name("throughput_mb_s")?;
    assert_eq!(card.len(), 1);
    assert!(!card.is_empty());
    let entry = card.get(&throughput).ok_or("metric missing")?;
    assert_eq!(entry.value().value().to_bits(), 12.5_f64.to_bits());
    assert!(!entry.held_out());
    assert_eq!(card.value(&throughput), Some(Metric::new(12.5)?));
    assert_eq!(card.value(&name("other")?), None);
    Ok(())
}

#[test]
fn scorecard_rejects_a_duplicate_name() -> TestResult {
    let json = r#"{"v":1,"metrics":[{"name":"a","value":1.0,"held_out":false},{"name":"a","value":2.0,"held_out":false}]}"#;
    let error = rejected(json)?;
    assert_eq!(error, ScorecardError::DuplicateMetric { name: name("a")? });
    Ok(())
}

#[test]
fn scorecard_rejects_unknown_fields() -> TestResult {
    for json in [
        r#"{"v":1,"metrics":[{"name":"a","value":1.0,"held_out":false}],"extra":1}"#,
        r#"{"v":1,"metrics":[{"name":"a","value":1.0,"held_out":false,"extra":1}]}"#,
    ] {
        assert!(matches!(
            rejected(json)?,
            ScorecardError::Json {
                category: JsonCategory::Data,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn scorecard_rejects_missing_fields() -> TestResult {
    for json in [
        r#"{"metrics":[{"name":"a","value":1.0,"held_out":false}]}"#,
        r#"{"v":1}"#,
        r#"{"v":1,"metrics":[{"name":"a","value":1.0}]}"#,
        r#"{"v":1,"metrics":[{"name":"a","held_out":false}]}"#,
        r#"{"v":1,"metrics":[{"value":1.0,"held_out":false}]}"#,
    ] {
        assert!(matches!(
            rejected(json)?,
            ScorecardError::Json {
                category: JsonCategory::Data,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn scorecard_rejects_another_version() -> TestResult {
    let json = r#"{"v":2,"metrics":[{"name":"a","value":1.0,"held_out":false}]}"#;
    assert_eq!(rejected(json)?, ScorecardError::Version { found: 2 });
    Ok(())
}

#[test]
fn scorecard_rejects_an_empty_metrics_array() -> TestResult {
    assert_eq!(rejected(r#"{"v":1,"metrics":[]}"#)?, ScorecardError::Empty);
    Ok(())
}

fn card_json(count: usize) -> String {
    let entries: Vec<String> = (0..count)
        .map(|index| format!(r#"{{"name":"m{index}","value":{index},"held_out":false}}"#))
        .collect();
    format!(r#"{{"v":1,"metrics":[{}]}}"#, entries.join(","))
}

#[test]
fn scorecard_accepts_max_metrics_and_rejects_one_more() -> TestResult {
    let full = card_json(MAX_METRICS);
    assert!(full.len() <= MAX_SCORECARD_BYTES);
    assert_eq!(parsed(&full)?.len(), MAX_METRICS);
    let over = card_json(MAX_METRICS + 1);
    assert!(over.len() <= MAX_SCORECARD_BYTES);
    assert_eq!(
        rejected(&over)?,
        ScorecardError::TooMany { max: MAX_METRICS }
    );
    Ok(())
}

#[test]
fn scorecard_rejects_a_bad_name() -> TestResult {
    for bad in ["Bad", "", "1a", "a-b", "a\u{1b}[31m"] {
        let json =
            format!(r#"{{"v":1,"metrics":[{{"name":"{bad}","value":1.0,"held_out":false}}]}}"#);
        assert!(matches!(
            rejected(&json)?,
            ScorecardError::Json {
                category: JsonCategory::Data | JsonCategory::Syntax,
                ..
            }
        ));
    }
    Ok(())
}

#[test]
fn scorecard_rejects_non_finite_values() -> TestResult {
    for value in ["NaN", "1e400", "-1e400", "\"1\""] {
        let json =
            format!(r#"{{"v":1,"metrics":[{{"name":"a","value":{value},"held_out":false}}]}}"#);
        assert!(matches!(rejected(&json)?, ScorecardError::Json { .. }));
    }
    Ok(())
}

#[test]
fn scorecard_rejects_oversize_bytes_before_parsing() -> TestResult {
    // Not JSON at all: a parse would report a Json error, so TooLarge proves the cap runs first.
    let junk = vec![b'x'; MAX_SCORECARD_BYTES + 1];
    assert_eq!(
        rejected_bytes(&junk)?,
        ScorecardError::TooLarge {
            bytes: MAX_SCORECARD_BYTES + 1
        }
    );
    Ok(())
}

#[test]
fn scorecard_at_the_byte_cap_is_parsed() -> TestResult {
    let padding = " ".repeat(MAX_SCORECARD_BYTES - VALID.len());
    let json = format!("{VALID}{padding}");
    assert_eq!(json.len(), MAX_SCORECARD_BYTES);
    assert_eq!(parsed(&json)?.len(), 1);
    let over = format!("{json} ");
    assert!(matches!(rejected(&over)?, ScorecardError::TooLarge { .. }));
    Ok(())
}

#[test]
fn scorecard_rejects_syntax_errors_and_trailing_data() -> TestResult {
    assert!(matches!(
        rejected("{")?,
        ScorecardError::Json {
            category: JsonCategory::Eof,
            ..
        }
    ));
    assert!(matches!(
        rejected(&format!("{VALID} x"))?,
        ScorecardError::Json {
            category: JsonCategory::Syntax,
            ..
        }
    ));
    assert!(matches!(
        rejected(r#"{"v":1,"metrics":{"a":1.0}}"#)?,
        ScorecardError::Json {
            category: JsonCategory::Data,
            ..
        }
    ));
    Ok(())
}

#[test]
fn scorecard_json_errors_carry_no_input_text() -> TestResult {
    let json =
        r#"{"v":1,"metrics":[{"name":"a","value":1.0,"held_out":false,"evil_field_name":1}]}"#;
    let error = rejected(json)?;
    assert!(matches!(error, ScorecardError::Json { line: 1, .. }));
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains("evil"), "{shown}");
    Ok(())
}

#[test]
fn scorecard_serializes_sorted_by_name() -> TestResult {
    let card = Scorecard::from_entries([
        (name("zeta")?, Metric::new(2.5)?, false),
        (name("alpha")?, Metric::ONE, true),
    ])?;
    let expected = r#"{"v":1,"metrics":[{"name":"alpha","value":1.0,"held_out":true},{"name":"zeta","value":2.5,"held_out":false}]}"#;
    assert_eq!(serde_json::to_string(&card)?, expected);

    let reversed = r#"{"v":1,"metrics":[{"name":"zeta","value":2.5,"held_out":false},{"name":"alpha","value":1.0,"held_out":true}]}"#;
    let from_wire = parsed(reversed)?;
    assert_eq!(from_wire, card);
    assert_eq!(serde_json::to_string(&from_wire)?, expected);
    let names: Vec<&str> = card.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(names, ["alpha", "zeta"]);
    let alpha = card.get(&name("alpha")?).ok_or("alpha missing")?;
    assert!(alpha.held_out());
    Ok(())
}

#[test]
fn scorecard_serde_goes_through_validation() -> TestResult {
    let card: Scorecard = serde_json::from_str(VALID)?;
    assert_eq!(card, parsed(VALID)?);
    assert!(serde_json::from_str::<Scorecard>(r#"{"v":1,"metrics":[]}"#).is_err());
    assert!(
        serde_json::from_str::<Scorecard>(
            r#"{"v":2,"metrics":[{"name":"a","value":1.0,"held_out":false}]}"#
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn from_entries_validates_like_parse() -> TestResult {
    let duplicate = Scorecard::from_entries([
        (name("a")?, Metric::ONE, false),
        (name("a")?, Metric::ZERO, true),
    ]);
    assert_eq!(
        duplicate,
        Err(ScorecardError::DuplicateMetric { name: name("a")? })
    );
    assert_eq!(Scorecard::from_entries([]), Err(ScorecardError::Empty));
    let many = (0..=MAX_METRICS)
        .map(|index| Ok((name(&format!("m{index}"))?, Metric::ONE, false)))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    assert_eq!(
        Scorecard::from_entries(many),
        Err(ScorecardError::TooMany { max: MAX_METRICS })
    );
    Ok(())
}
