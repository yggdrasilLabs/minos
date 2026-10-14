//! `Metric` (a finite `f64` ordered with `total_cmp`) and `Scorecard`, the judge's validated output.
//!
//! A candidate can influence the numbers a judge emits, so no NaN or infinity ever reaches policy code: a
//! [`Metric`] exists only if its value is finite, and negative zero is stored as zero so that equal metrics have
//! equal bits.
//!
//! [`Scorecard::parse_json`] is the only way sandbox scorecard bytes become a [`Scorecard`]; Phase 4's
//! `Untrusted::into_scorecard` calls it. The wire form is a versioned array of `{name, value, held_out}` entries,
//! never a JSON object, because `serde_json` keeps the last of two equal object keys silently. Here a repeated name
//! is a rejection. The byte cap is checked before any JSON parsing, and a JSON error keeps only its line, column,
//! and category, never `serde_json`'s message, which can echo hostile field names.
//!
//! Example wire form:
//!
//! ```json
//! {"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false}]}
//! ```

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::id::MetricName;
use crate::limits::{Limits, MAX_METRICS};

/// The scorecard wire version this code reads and writes.
pub const SCORECARD_VERSION: u32 = 1;

/// Bits of `-0.0`, which [`Metric::new`] maps to `0.0`.
const NEGATIVE_ZERO_BITS: u64 = 0x8000_0000_0000_0000;

/// Why a value cannot be a [`Metric`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MetricError {
    /// The value is NaN or an infinity.
    #[error("a metric must be a finite number")]
    NotFinite,
}

/// A finite `f64`, with `-0.0` normalized to `0.0`.
///
/// `Eq`, `Ord`, and `Hash` agree with each other and with [`f64::total_cmp`]: NaN cannot occur, and the two zeros
/// are one value.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Metric(f64);

impl Metric {
    /// Zero.
    pub const ZERO: Self = Self(0.0);
    /// One.
    pub const ONE: Self = Self(1.0);

    /// A metric with value `x`. Negative zero becomes zero.
    ///
    /// # Errors
    ///
    /// [`MetricError::NotFinite`] if `x` is NaN or an infinity.
    pub const fn new(x: f64) -> Result<Self, MetricError> {
        if !x.is_finite() {
            return Err(MetricError::NotFinite);
        }
        if x.to_bits() == NEGATIVE_ZERO_BITS {
            return Ok(Self::ZERO);
        }
        Ok(Self(x))
    }

    /// The value, always finite and never `-0.0`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Metric {
    type Error = MetricError;

    fn try_from(x: f64) -> Result<Self, Self::Error> {
        Self::new(x)
    }
}

impl From<Metric> for f64 {
    fn from(metric: Metric) -> Self {
        metric.0
    }
}

impl PartialEq for Metric {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Metric {}

impl PartialOrd for Metric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Metric {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Hash for Metric {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Consistent with `Eq`: `total_cmp` equality is bit equality, and `-0.0` and NaN never occur.
        self.0.to_bits().hash(state);
    }
}

impl fmt::Debug for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// One metric in a [`Scorecard`]: its value and whether it is held out.
///
/// A held-out metric is measured on data the proposer never sees; the report compares it with the visible ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScoredMetric {
    value: Metric,
    held_out: bool,
}

impl ScoredMetric {
    /// The metric's value.
    #[must_use]
    pub const fn value(&self) -> Metric {
        self.value
    }

    /// Whether the metric is held out.
    #[must_use]
    pub const fn held_out(&self) -> bool {
        self.held_out
    }
}

/// Which kind of JSON failure rejected a scorecard. `serde_json`'s message is discarded on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonCategory {
    /// Reading the bytes failed.
    Io,
    /// The bytes are not valid JSON.
    Syntax,
    /// The JSON does not have the scorecard's shape: a missing, unknown, or ill-typed field, or a bad value.
    Data,
    /// The input ended in the middle of a value.
    Eof,
}

impl JsonCategory {
    fn of(error: &serde_json::Error) -> Self {
        match error.classify() {
            serde_json::error::Category::Io => Self::Io,
            serde_json::error::Category::Syntax => Self::Syntax,
            serde_json::error::Category::Data => Self::Data,
            serde_json::error::Category::Eof => Self::Eof,
        }
    }
}

impl fmt::Display for JsonCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "read error",
            Self::Syntax => "syntax error",
            Self::Data => "wrong shape",
            Self::Eof => "unexpected end of input",
        })
    }
}

/// Why scorecard bytes were rejected.
///
/// Every variant is a candidate fault. No variant carries input text: only sizes, positions, a version number,
/// and a validated [`MetricName`] (03-RESEARCH Pitfall 10).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScorecardError {
    /// The bytes are over [`Limits::max_scorecard_bytes`]. Checked before any parsing.
    #[error("the scorecard is {bytes} bytes, over the cap")]
    TooLarge {
        /// Size of the input, in bytes.
        bytes: usize,
    },
    /// The bytes are not a scorecard in JSON. `serde_json`'s message is not kept.
    #[error("the scorecard JSON is invalid at line {line}, column {column}: {category}")]
    Json {
        /// 1-based line of the failure, or 0 if none applies.
        line: usize,
        /// 1-based column of the failure, or 0 if none applies.
        column: usize,
        /// Which kind of failure.
        category: JsonCategory,
    },
    /// The `v` field names a version this code does not read.
    #[error("scorecard version {found} is not supported")]
    Version {
        /// The version found.
        found: u32,
    },
    /// The scorecard has no metrics.
    #[error("the scorecard has no metrics")]
    Empty,
    /// The scorecard has more metrics than the cap.
    #[error("the scorecard has more than {max} metrics")]
    TooMany {
        /// The cap.
        max: usize,
    },
    /// Two entries have the same name.
    #[error("metric {name} appears more than once")]
    DuplicateMetric {
        /// The repeated name.
        name: MetricName,
    },
}

/// One scorecard entry as written. All fields are required and no other field is allowed.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScorecardEntry {
    name: MetricName,
    value: Metric,
    held_out: bool,
}

/// A scorecard as written, before the version, count, and duplicate checks.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScorecard {
    v: u32,
    metrics: Vec<RawScorecardEntry>,
}

/// A validated scorecard: one to [`MAX_METRICS`] metrics with distinct names, kept in name order.
///
/// Whether the names match the manifest's judge is checked by the runner, which holds the manifest; this type
/// checks shape only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawScorecard", into = "RawScorecard")]
pub struct Scorecard {
    metrics: BTreeMap<MetricName, ScoredMetric>,
}

impl Scorecard {
    /// Parses scorecard bytes from a sandbox. The only path from sandbox bytes to a `Scorecard`.
    ///
    /// The byte cap is checked first, then the JSON is parsed into the raw wire form, then the version, count,
    /// and duplicate checks run.
    ///
    /// # Errors
    ///
    /// [`ScorecardError::TooLarge`] over [`Limits::max_scorecard_bytes`]; [`ScorecardError::Json`] for bytes
    /// that are not the wire form (including a bad metric name or a non-finite value); otherwise the errors of the
    /// validation, with [`Limits::max_metrics`] as the count cap.
    pub fn parse_json(bytes: &[u8], limits: &Limits) -> Result<Self, ScorecardError> {
        if bytes.len() > limits.max_scorecard_bytes() {
            return Err(ScorecardError::TooLarge { bytes: bytes.len() });
        }
        let raw: RawScorecard =
            serde_json::from_slice(bytes).map_err(|error| ScorecardError::Json {
                line: error.line(),
                column: error.column(),
                category: JsonCategory::of(&error),
            })?;
        Self::validate(raw, limits.max_metrics())
    }

    /// Builds a scorecard from `(name, value, held_out)` entries through the same validation as
    /// [`Scorecard::parse_json`].
    ///
    /// # Errors
    ///
    /// [`ScorecardError::Empty`], [`ScorecardError::TooMany`], or [`ScorecardError::DuplicateMetric`].
    pub fn from_entries(
        entries: impl IntoIterator<Item = (MetricName, Metric, bool)>,
    ) -> Result<Self, ScorecardError> {
        let metrics = entries
            .into_iter()
            .take(MAX_METRICS.saturating_add(1))
            .map(|(name, value, held_out)| RawScorecardEntry {
                name,
                value,
                held_out,
            })
            .collect();
        Self::validate(
            RawScorecard {
                v: SCORECARD_VERSION,
                metrics,
            },
            MAX_METRICS,
        )
    }

    fn validate(raw: RawScorecard, max_metrics: usize) -> Result<Self, ScorecardError> {
        if raw.v != SCORECARD_VERSION {
            return Err(ScorecardError::Version { found: raw.v });
        }
        if raw.metrics.is_empty() {
            return Err(ScorecardError::Empty);
        }
        let max = max_metrics.min(MAX_METRICS);
        if raw.metrics.len() > max {
            return Err(ScorecardError::TooMany { max });
        }
        let mut metrics = BTreeMap::new();
        for entry in raw.metrics {
            match metrics.entry(entry.name) {
                Entry::Vacant(slot) => {
                    slot.insert(ScoredMetric {
                        value: entry.value,
                        held_out: entry.held_out,
                    });
                }
                Entry::Occupied(slot) => {
                    return Err(ScorecardError::DuplicateMetric {
                        name: slot.key().clone(),
                    });
                }
            }
        }
        Ok(Self { metrics })
    }

    /// The entry for `name`, if the scorecard has one.
    #[must_use]
    pub fn get(&self, name: &MetricName) -> Option<&ScoredMetric> {
        self.metrics.get(name)
    }

    /// The value of `name`, if the scorecard has it.
    #[must_use]
    pub fn value(&self, name: &MetricName) -> Option<Metric> {
        self.metrics.get(name).map(ScoredMetric::value)
    }

    /// The entries in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&MetricName, &ScoredMetric)> {
        self.metrics.iter()
    }

    /// Number of metrics, at least one.
    #[must_use]
    pub fn len(&self) -> usize {
        self.metrics.len()
    }

    /// Always false: a scorecard has at least one metric.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }
}

impl TryFrom<RawScorecard> for Scorecard {
    type Error = ScorecardError;

    fn try_from(raw: RawScorecard) -> Result<Self, Self::Error> {
        Self::validate(raw, MAX_METRICS)
    }
}

impl From<Scorecard> for RawScorecard {
    fn from(card: Scorecard) -> Self {
        Self {
            v: SCORECARD_VERSION,
            metrics: card
                .metrics
                .into_iter()
                .map(|(name, scored)| RawScorecardEntry {
                    name,
                    value: scored.value,
                    held_out: scored.held_out,
                })
                .collect(),
        }
    }
}
