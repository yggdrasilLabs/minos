//! `RawManifest` (the mirror of `minos.toml`) and the validated `Manifest` built from it.
//!
//! Every raw table refuses unknown keys (toml reports the line and column), absent optional fields stay `None`
//! in the raw structs, and [`Manifest::try_from`] is the only constructor of [`Manifest`]: serde reaches it
//! through `try_from`, and [`Manifest::parse_toml`] calls it directly. It applies, in order:
//!
//! 1. Leaf validation, with dotted field paths: commands are argv arrays, budgets lie within `limits`, counts are
//!    capped, and absent optional fields take their defaults (`stall_step`, `max_empty_generations`, and
//!    `max_infra_failures` have minimums of 1, so the circuit breaker cannot be disabled; 03-CONTEXT D-13, D-14).
//!    `[judge] trials`, the baseline trials of generation 0, defaults to 3 and lies within 1 to 32 (07-CONTEXT
//!    D-05).
//!    `[environment] memory_mib` and `scratch_mib` lie within the machine bounds in [`crate::limits`], and the
//!    VM's memory holds the guest reserve, the scratch cap, and the least candidate allowance (05-CONTEXT D-05,
//!    D-06; [`ManifestError::MemoryBudget`]).
//! 2. The surface (03-CONTEXT D-01): `writable` holds 1 to 64 exact files with no duplicates, `readable` has no
//!    duplicates, and the two are disjoint as exact path sets.
//! 3. Judge integrity: `writable` contains no judge path, and neither `program` nor the Dockerfile; `readable`
//!    contains no judge path, so the proposer never receives the scorer's source in its prompt. `writable` holds
//!    neither `minos.toml` nor `minos.lock`, and `readable` not `minos.lock` (compared ASCII
//!    case-insensitively): an adopted patch could otherwise rewrite the judge command, the budgets, the
//!    excludes, or the image pins for the next run.
//! 4. No `writable` or `readable` path lies under a `[source] exclude` entry (compared case-insensitively).
//! 5. No two distinct paths among `writable`, `readable`, `program`, the Dockerfile, and the judge paths are equal
//!    under ASCII case folding, which fails closed on a case-insensitive filesystem.
//! 6. Every judge path lies under a `[source] exclude` entry, so the judge program is never in the source
//!    snapshot the candidate sandbox sees (the judge runs from its own pushed data, never from the tree).
//! 7. The policy is greedy (pareto and review are not in this release) with exactly one objective.
//! 8. No metric name is shared among gates, objectives, and held-out metrics.
//! 9. Params have unique names and live in writable files.
//! 10. Proposers have one entry per tier, and at least one is eligible on manifest facts alone.
//!
//! **Judge paths** are the `[source] exclude` entries (rule 4) plus every judge argv entry that names a
//! project-relative path: the program and every later argument, or the text after the first `=` of an option. An
//! absolute path (`/out/score.json`, a path inside the VM image) contributes nothing, and neither does a bare
//! word, an entry with no `/` or `\` and no leading `./` that is not `.` or `..` (`python3`, `verbose`, the
//! `json` of `--format=json`): it is a program on `PATH` or a plain argument, the same rule as
//! [`CommandArgv::program`]. Otherwise one leading `./` is stripped, and what remains must be a valid
//! [`RelPath`], which is then a judge path. An entry that looks like a path but is not a valid [`RelPath`], and
//! an option without `=` that has a `/` or `\`, is refused rather than ignored. So
//! `["python3", "judge/score.py"]` makes `judge/score.py` a judge path and nothing of `python3`; the judge path
//! can be neither writable nor readable, and must lie under a `[source] exclude` entry (rules 3 and 6). Only the
//! judge command is read this way, never build, run, or check.
//!
//! **The `[judge] version` integer** is a local label naming which pushed judge data the project uses. The
//! recorded identity is [`JudgeVersion`]: the hash of that data's tree and the canonical judge spec. Judge data
//! never lives in the project; it is pushed separately and never mounted for an agent or a candidate.
//!
//! **Images are digests** is enforced by `crate::image`, because the manifest names a Dockerfile and this
//! module reads no file.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::{IdHasher, MetricName, ModelId, TreeId};
use crate::limits::{
    BATCH_MAX, BATCH_MIN, DEFAULT_JUDGE_MS, DEFAULT_MEMORY_MIB, DEFAULT_SCRATCH_MIB,
    EMPTY_GENERATIONS_DEFAULT, EMPTY_GENERATIONS_MAX, EMPTY_GENERATIONS_MIN, GUEST_RESERVE_MIB,
    INFRA_FAILURES_DEFAULT, INFRA_FAILURES_MAX, INFRA_FAILURES_MIN, MAX_ANCHOR_BYTES,
    MAX_ARG_BYTES, MAX_ARGV_ENTRIES, MAX_CHOICE_BYTES, MAX_CHOICES, MAX_EXCLUDES, MAX_GATES,
    MAX_JUDGE_LABEL, MAX_MEMORY_MIB, MAX_MIN_IMPROVEMENT_PERCENT, MAX_NAME_BYTES, MAX_OBJECTIVES,
    MAX_PARAM_POINTS, MAX_PARAMS, MAX_PROPOSERS, MAX_READABLE, MAX_SCRATCH_MIB, MAX_SPEND_USD,
    MAX_STAGE_MS, MAX_TRIALS, MAX_WRITABLE, MIN_CANDIDATE_ALLOWANCE_MIB, MIN_MEMORY_MIB,
    MIN_SCRATCH_MIB, MIN_STAGE_MS, STALL_STEP_DEFAULT, STALL_STEP_MAX, STALL_STEP_MIN,
    TOKEN_CAP_DEFAULT, TOKEN_CAP_MAX, TOKEN_CAP_MIN, TRIALS_DEFAULT, TRIALS_MIN, WIDTH_MAX,
    WIDTH_MIN, candidate_allowance_mib,
};
use crate::path::RelPath;

/// Domain of [`JudgeVersion`].
const JUDGE_DOMAIN: &str = "minos/judge/v1";
/// Project files that are never writable, as ASCII case keys: the manifest and the lockfile.
const RESERVED_WRITABLE: [&str; 2] = ["minos.toml", "minos.lock"];
/// Project files that are never readable, as ASCII case keys: the lockfile.
const RESERVED_READABLE: [&str; 1] = ["minos.lock"];
/// Micro-dollars per whole dollar, for the spend cap.
const MICRO_PER_DOLLAR: u128 = 1_000_000;

/// Longest project name, in bytes: it names directories and appears in reports.
const MAX_PROJECT_NAME_BYTES: usize = 64;
/// Most digits in a stage budget such as `90s`. Six digits cover every value up to the one-hour cap in seconds.
const MAX_DURATION_DIGITS: usize = 6;
/// Longest `min_improvement` text, in bytes. Real thresholds are a few characters; the cap bounds the float parse.
const MAX_MIN_IMPROVEMENT_BYTES: usize = 32;

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// A single value outside its grammar or range, before the manifest knows which field held it.
///
/// It carries the allowed range in words, never the input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the value must be {limit}")]
pub struct ValueError {
    limit: &'static str,
}

impl ValueError {
    /// A value error with the allowed range in words.
    const fn new(limit: &'static str) -> Self {
        Self { limit }
    }

    /// The allowed range, in words.
    #[must_use]
    pub const fn limit(&self) -> &'static str {
        self.limit
    }
}

/// A manifest field whose value is outside its allowed range: the shape `minos-guest`'s job reader uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{field} must be {limit}")]
pub struct InvalidField {
    field: &'static str,
    limit: &'static str,
}

impl InvalidField {
    /// An invalid field with its dotted path and the allowed range in words.
    const fn new(field: &'static str, limit: &'static str) -> Self {
        Self { field, limit }
    }

    /// A [`ValueError`] attributed to the dotted field path `field`.
    const fn at(field: &'static str, error: ValueError) -> Self {
        Self::new(field, error.limit)
    }

    /// Dotted path of the offending field, such as `budget.build`.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// The allowed range, in words.
    #[must_use]
    pub const fn limit(&self) -> &'static str {
        self.limit
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------------------------

/// Checks `text` against a length limit, a first-byte class, and a class for every later byte.
fn check_word(
    text: &str,
    max: usize,
    start: fn(u8) -> bool,
    rest: fn(u8) -> bool,
    limit: &'static str,
) -> Result<(), ValueError> {
    let mut bytes = text.bytes();
    let valid = text.len() <= max && bytes.next().is_some_and(start) && bytes.all(rest);
    if valid {
        Ok(())
    } else {
        Err(ValueError::new(limit))
    }
}

/// Defines a validated word type over a private `String`, parsed and deserialized through one checker.
macro_rules! word_type {
    ($(#[$meta:meta])* $name:ident, $max:expr, $start:expr, $rest:expr, $limit:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The name as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = ValueError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                check_word(&text, $max, $start, $rest, $limit)?;
                Ok(Self(text))
            }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::try_from(text.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(name: $name) -> Self {
                name.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

word_type! {
    /// A project name, such as `logparse`: a lowercase letter or digit, then up to 63 lowercase letters, digits,
    /// `_`, and `-`.
    ProjectName,
    MAX_PROJECT_NAME_BYTES,
    |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit(),
    |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-'),
    "a lowercase letter or digit, then up to 63 lowercase letters, digits, _, and -"
}

word_type! {
    /// A sweep param name, such as `chunk_size`: a lowercase letter, then lowercase letters, digits, and
    /// underscores, at most [`MAX_NAME_BYTES`] bytes.
    ParamName,
    MAX_NAME_BYTES,
    |byte| byte.is_ascii_lowercase(),
    |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_',
    "a lowercase letter, then lowercase letters, digits, and _, at most 64 bytes"
}

/// What kind of project a manifest describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    /// Tuning declared knobs, such as hyperparameters.
    Tune,
    /// Optimizing a measured objective, such as throughput.
    Optimize,
    /// An open-ended design question.
    Explore,
}

/// A guest CPU architecture a project's image is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Arch {
    /// 64-bit Arm (`arm64`): Lambda and Apple silicon.
    #[serde(rename = "arm64")]
    Arm64,
    /// 64-bit x86 (`x86_64`): most GPU machines.
    #[serde(rename = "x86_64")]
    X86_64,
}

// ---------------------------------------------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------------------------------------------

/// A manifest command: an argv array, never a shell string.
///
/// It has 1 to [`MAX_ARGV_ENTRIES`] entries, each at most [`MAX_ARG_BYTES`] bytes with no NUL, and a non-empty
/// first entry. The first entry's project-relative path, if it names one, is computed once here: `./judge` is the
/// path `judge`, `judge/run.sh` is that path, and an absolute path (`/usr/bin/python3`) or a bare name
/// (`python3`) is none. A first entry that looks like a relative path but is not a valid [`RelPath`] is refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Vec<String>")]
pub struct CommandArgv {
    argv: Vec<String>,
    program: Option<RelPath>,
}

impl CommandArgv {
    /// The entries, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.argv
    }

    /// The project-relative path the first entry names, if any.
    #[must_use]
    pub const fn program(&self) -> Option<&RelPath> {
        self.program.as_ref()
    }
}

/// The project-relative path a command's first entry names, if any.
fn program_path(first: &str) -> Result<Option<RelPath>, ValueError> {
    const LIMIT: &str = "a program that is a bare name, an absolute path, or a valid relative path";
    if let Some(rest) = first.strip_prefix("./") {
        return RelPath::try_from(rest)
            .map(Some)
            .map_err(|_| ValueError::new(LIMIT));
    }
    if first.starts_with('/') || !first.contains('/') {
        return Ok(None);
    }
    RelPath::try_from(first)
        .map(Some)
        .map_err(|_| ValueError::new(LIMIT))
}

impl TryFrom<Vec<String>> for CommandArgv {
    type Error = ValueError;

    fn try_from(argv: Vec<String>) -> Result<Self, Self::Error> {
        if !(1..=MAX_ARGV_ENTRIES).contains(&argv.len()) {
            return Err(ValueError::new("an argv array of 1 to 64 entries"));
        }
        if argv
            .iter()
            .any(|entry| entry.len() > MAX_ARG_BYTES || entry.contains('\0'))
        {
            return Err(ValueError::new(
                "argv entries of at most 4096 bytes with no NUL",
            ));
        }
        let first = argv.first().map_or("", String::as_str);
        if first.is_empty() {
            return Err(ValueError::new(
                "an argv array whose first entry is not empty",
            ));
        }
        let program = program_path(first)?;
        Ok(Self { argv, program })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Durations and thresholds
// ---------------------------------------------------------------------------------------------------------------

/// A stage budget in milliseconds, written `<1 to 6 digits><s|m|h>`, from [`MIN_STAGE_MS`] to [`MAX_STAGE_MS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationMs(u64);

impl DurationMs {
    /// A duration the core itself chose, such as a default. It must lie within the stage bounds.
    const fn from_const(ms: u64) -> Self {
        Self(ms)
    }

    /// Parses `90s`, `2m`, or `1h`.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError`] for anything other than 1 to 6 ASCII digits followed by `s`, `m`, or `h`, or for a
    /// duration outside 1 second to 1 hour.
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        const LIMIT: &str = "1 to 6 digits followed by s, m, or h, from 1s to 1h";
        let error = ValueError::new(LIMIT);
        let Some((unit, digits)) = text.as_bytes().split_last() else {
            return Err(error);
        };
        let scale: u64 = match unit {
            b's' => 1_000,
            b'm' => 60_000,
            b'h' => 3_600_000,
            _ => return Err(error),
        };
        if digits.is_empty()
            || digits.len() > MAX_DURATION_DIGITS
            || !digits.iter().all(u8::is_ascii_digit)
        {
            return Err(error);
        }
        let count = digits
            .iter()
            .fold(0_u64, |acc, digit| acc * 10 + u64::from(digit - b'0'));
        let ms = count.checked_mul(scale).ok_or(error)?;
        if (MIN_STAGE_MS..=MAX_STAGE_MS).contains(&ms) {
            Ok(Self(ms))
        } else {
            Err(error)
        }
    }

    /// The duration in milliseconds.
    #[must_use]
    pub const fn as_ms(self) -> u64 {
        self.0
    }
}

/// Which direction of an objective is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Goal {
    /// Larger is better.
    Maximize,
    /// Smaller is better.
    Minimize,
}

impl Goal {
    /// `delta` oriented so that positive is always better: unchanged for maximize, negated for minimize.
    #[must_use]
    pub const fn oriented(self, delta: f64) -> f64 {
        match self {
            Self::Maximize => delta,
            Self::Minimize => -delta,
        }
    }
}

/// The smallest improvement over the incumbent that counts, relative (`5%`) or absolute (`0.25`).
///
/// A relative threshold is stored as a ratio (`5%` is `0.05`), parsed from the shifted decimal text so it is
/// the nearest double to the written value. The threshold is applied by multiplying the incumbent, never by
/// dividing by it (03-RESEARCH Pitfall 7).
#[derive(Debug, Clone, PartialEq)]
pub struct MinImprovement {
    relative: bool,
    amount: f64,
    text: String,
}

/// Splits `digits[.digits]` into its integer and fraction parts, or `None` if the text has another shape.
fn decimal_parts(text: &str) -> Option<(&str, &str)> {
    let (integer, fraction) = text.split_once('.').unwrap_or((text, ""));
    let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    let fraction_ok = !text.contains('.') || !fraction.is_empty();
    (!integer.is_empty() && fraction_ok && all_digits(integer) && all_digits(fraction))
        .then_some((integer, fraction))
}

impl MinImprovement {
    /// Parses `digits[.digits]`, optionally followed by `%`.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError`] for any other text (signs, exponents, spaces, empty parts), a percentage above
    /// [`MAX_MIN_IMPROVEMENT_PERCENT`], or a value that is not finite.
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        const LIMIT: &str = "digits[.digits] with an optional %, at most 1000%";
        let error = ValueError::new(LIMIT);
        if text.len() > MAX_MIN_IMPROVEMENT_BYTES {
            return Err(error);
        }
        let (number, relative) = text
            .strip_suffix('%')
            .map_or((text, false), |number| (number, true));
        let (integer, fraction) = decimal_parts(number).ok_or(error)?;
        let written: f64 = number.parse().map_err(|_| error)?;
        let amount: f64 = if relative {
            if written > f64::from(MAX_MIN_IMPROVEMENT_PERCENT) {
                return Err(error);
            }
            let shift = fraction.len() + 2;
            format!("{integer}{fraction}e-{shift}")
                .parse()
                .map_err(|_| error)?
        } else {
            written
        };
        if !amount.is_finite() || amount < 0.0 {
            return Err(error);
        }
        Ok(Self {
            relative,
            amount,
            text: text.to_owned(),
        })
    }

    /// Whether the threshold is relative to the incumbent.
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        self.relative
    }

    /// The ratio for a relative threshold (`0.05` for `5%`), or the absolute amount.
    #[must_use]
    pub const fn amount(&self) -> f64 {
        self.amount
    }

    /// The absolute improvement required over `incumbent`: `|incumbent| * ratio` when relative, the amount when
    /// absolute, or `None` when the result is not finite.
    #[must_use]
    pub fn threshold_at(&self, incumbent: f64) -> Option<f64> {
        let threshold = if self.relative {
            incumbent.abs() * self.amount
        } else {
            self.amount
        };
        threshold.is_finite().then_some(threshold)
    }
}

impl fmt::Display for MinImprovement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Tiers and eligibility
// ---------------------------------------------------------------------------------------------------------------

/// A proposer tier. Declaration order is the routing rank, cheapest first.
///
/// All four tiers exist now so a recorded tier never needs a format change when the later ones ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Seeded sampling over declared params; no model.
    Sweep,
    /// One model call with no tools.
    SingleShot,
    /// A fixed two-call pipeline. Not in this release.
    Planned,
    /// A tool loop against an agent VM. Not in this release.
    Agentic,
}

impl Tier {
    /// The routing rank: 0 for sweep up to 3 for agentic.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Sweep => 0,
            Self::SingleShot => 1,
            Self::Planned => 2,
            Self::Agentic => 3,
        }
    }

    /// The tier's manifest spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sweep => "sweep",
            Self::SingleShot => "single_shot",
            Self::Planned => "planned",
            Self::Agentic => "agentic",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a declared tier cannot propose. `minos check` prints the reason beside each tier.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IneligibleReason {
    /// A sweep has nothing to sample.
    #[error("sweep needs at least one [[param]]")]
    NoParams,
    /// A model tier names no model.
    #[error("this tier needs a model")]
    NoModel,
    /// The tier is not implemented in this release.
    #[error("this tier is not in this release")]
    NotInThisRelease,
    /// The surface does not fit the single-shot prompt budget (decided by routing, which knows the file sizes).
    #[error("the surface is {bytes} bytes, over the {limit}-byte single-shot prompt budget")]
    SurfaceTooLarge {
        /// Total surface size, in bytes.
        bytes: u64,
        /// The prompt budget, in bytes.
        limit: u64,
    },
}

/// Whether a declared tier can propose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    /// The tier can propose.
    Eligible,
    /// The tier cannot propose, for this reason.
    Ineligible(IneligibleReason),
}

/// One `[[proposer]]` entry: a tier, its model, and how many proposals one step asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposerSpec {
    tier: Tier,
    model: Option<ModelId>,
    batch: u32,
}

impl ProposerSpec {
    /// A proposer entry.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] for a batch outside [`BATCH_MIN`] to [`BATCH_MAX`], or a sweep that names a
    /// model (a sweep calls no model).
    pub fn new(tier: Tier, model: Option<ModelId>, batch: u32) -> Result<Self, InvalidField> {
        if !(BATCH_MIN..=BATCH_MAX).contains(&batch) {
            return Err(InvalidField::new("proposer.batch", "1 to 16"));
        }
        if tier == Tier::Sweep && model.is_some() {
            return Err(InvalidField::new(
                "proposer.model",
                "absent for a sweep, which calls no model",
            ));
        }
        Ok(Self { tier, model, batch })
    }

    /// The tier.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The model, if the entry names one.
    #[must_use]
    pub const fn model(&self) -> Option<&ModelId> {
        self.model.as_ref()
    }

    /// Proposals per step.
    #[must_use]
    pub const fn batch(&self) -> u32 {
        self.batch
    }

    /// Eligibility from manifest facts alone: a sweep needs a param, single-shot needs a model, and planned and
    /// agentic are not in this release. Routing adds the checks that need file sizes.
    #[must_use]
    pub fn declared_eligibility(&self, params: &[SweepParam]) -> Eligibility {
        match self.tier {
            Tier::Sweep if params.is_empty() => Eligibility::Ineligible(IneligibleReason::NoParams),
            Tier::SingleShot if self.model.is_none() => {
                Eligibility::Ineligible(IneligibleReason::NoModel)
            }
            Tier::Sweep | Tier::SingleShot => Eligibility::Eligible,
            Tier::Planned | Tier::Agentic => {
                Eligibility::Ineligible(IneligibleReason::NotInThisRelease)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Sweep params
// ---------------------------------------------------------------------------------------------------------------

/// An integer param domain: `min`, `min + step`, ... up to `max`, at most [`MAX_PARAM_POINTS`] points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntRange {
    min: i64,
    max: i64,
    step: i64,
    points: u64,
}

impl IntRange {
    /// An integer range.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] when `min > max`, `step < 1`, or the range has more than [`MAX_PARAM_POINTS`]
    /// points.
    pub fn new(min: i64, max: i64, step: i64) -> Result<Self, InvalidField> {
        if min > max {
            return Err(InvalidField::new("param.max", "at least param.min"));
        }
        if step < 1 {
            return Err(InvalidField::new("param.step", "at least 1"));
        }
        let span = i128::from(max) - i128::from(min);
        let count = span / i128::from(step) + 1;
        let points = u64::try_from(count)
            .ok()
            .filter(|points| *points <= MAX_PARAM_POINTS)
            .ok_or(InvalidField::new(
                "param",
                "a domain of at most 1000000 points",
            ))?;
        Ok(Self {
            min,
            max,
            step,
            points,
        })
    }

    /// The smallest value.
    #[must_use]
    pub const fn min(&self) -> i64 {
        self.min
    }

    /// The largest value bound (the last point may be below it when the step does not divide the span).
    #[must_use]
    pub const fn max(&self) -> i64 {
        self.max
    }

    /// The distance between points.
    #[must_use]
    pub const fn step(&self) -> i64 {
        self.step
    }

    /// How many points the domain has.
    #[must_use]
    pub const fn points(&self) -> u64 {
        self.points
    }

    /// The point at `index`, or `None` past the end.
    #[must_use]
    pub fn value_at(&self, index: u64) -> Option<i64> {
        if index >= self.points {
            return None;
        }
        let value = i128::from(self.min) + i128::from(index) * i128::from(self.step);
        i64::try_from(value).ok()
    }
}

/// A categorical param domain: 1 to [`MAX_CHOICES`] unique literal strings, each 1 to [`MAX_CHOICE_BYTES`] bytes
/// with no control character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceList(Vec<String>);

impl ChoiceList {
    /// A choice list, in declaration order.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] for an empty or over-long list, a duplicate, an empty or over-long choice, or a
    /// choice with a control character.
    pub fn new(choices: Vec<String>) -> Result<Self, InvalidField> {
        const LIMIT: &str = "1 to 64 unique choices, each 1 to 256 bytes with no control character";
        let error = InvalidField::new("param.choices", LIMIT);
        if !(1..=MAX_CHOICES).contains(&choices.len()) {
            return Err(error);
        }
        let mut seen = BTreeSet::new();
        for choice in &choices {
            if choice.is_empty()
                || choice.len() > MAX_CHOICE_BYTES
                || choice.chars().any(char::is_control)
                || !seen.insert(choice.as_str())
            {
                return Err(error);
            }
        }
        Ok(Self(choices))
    }

    /// How many choices there are (always at least one).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false: a choice list has at least one choice.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The choice at `index`, or `None` past the end.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&str> {
        self.0.get(index).map(String::as_str)
    }

    /// The choices, in declaration order.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// The values a sweep param may take: integer ranges and literal strings only, because float or log ranges would
/// need libm-dependent functions or float formatting (03-RESEARCH Pattern 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamDomain {
    /// An integer range.
    Int(IntRange),
    /// A list of literal strings.
    Choices(ChoiceList),
}

/// One `[[param]]` entry: a named knob in a writable file, located by an anchor line prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepParam {
    name: ParamName,
    file: RelPath,
    anchor: String,
    domain: ParamDomain,
}

impl SweepParam {
    /// A sweep param. The manifest checks that `file` is writable; the sweep checks the anchor against the base
    /// file at plan time.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] for an anchor that is empty, longer than [`MAX_ANCHOR_BYTES`], or contains a
    /// control character.
    pub fn new(
        name: ParamName,
        file: RelPath,
        anchor: String,
        domain: ParamDomain,
    ) -> Result<Self, InvalidField> {
        if anchor.is_empty()
            || anchor.len() > MAX_ANCHOR_BYTES
            || anchor.chars().any(char::is_control)
        {
            return Err(InvalidField::new(
                "param.anchor",
                "1 to 256 bytes with no control character",
            ));
        }
        Ok(Self {
            name,
            file,
            anchor,
            domain,
        })
    }

    /// The param's name.
    #[must_use]
    pub const fn name(&self) -> &ParamName {
        &self.name
    }

    /// The writable file the param lives in.
    #[must_use]
    pub const fn file(&self) -> &RelPath {
        &self.file
    }

    /// The line prefix that locates the param in its file.
    #[must_use]
    pub fn anchor(&self) -> &str {
        &self.anchor
    }

    /// The values the param may take.
    #[must_use]
    pub const fn domain(&self) -> &ParamDomain {
        &self.domain
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Source excludes
// ---------------------------------------------------------------------------------------------------------------

/// One `[source] exclude` entry: a relative path, optionally written with one trailing `/` for a directory.
///
/// It covers a path that equals it or lies under it at a component boundary, compared ASCII case-insensitively,
/// so `judge/` covers `judge/x.py` and `Judge/x.py` but not `judgement.py`. The case-insensitive match fails
/// closed on a case-insensitive filesystem.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct SourceExclude {
    path: RelPath,
    dir: bool,
}

impl SourceExclude {
    /// Parses an exclude entry.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError`] unless the text, with at most one trailing `/` removed, is a valid [`RelPath`].
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let (body, dir) = text
            .strip_suffix('/')
            .map_or((text, false), |body| (body, true));
        let path = RelPath::try_from(body)
            .map_err(|_| ValueError::new("a relative path with at most one trailing /"))?;
        Ok(Self { path, dir })
    }

    /// The excluded path, without the trailing `/`.
    #[must_use]
    pub const fn path(&self) -> &RelPath {
        &self.path
    }

    /// Whether the entry was written with a trailing `/`.
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        self.dir
    }

    /// Whether `path` equals this entry or lies under it, compared ASCII case-insensitively.
    #[must_use]
    pub fn covers(&self, path: &RelPath) -> bool {
        let key = path.ascii_case_key();
        let prefix = self.path.ascii_case_key();
        key.strip_prefix(prefix.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }
}

impl TryFrom<String> for SourceExclude {
    type Error = ValueError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl fmt::Display for SourceExclude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.path.as_str())?;
        if self.dir { f.write_str("/") } else { Ok(()) }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Raw manifest: minos.toml as written
// ---------------------------------------------------------------------------------------------------------------

/// `minos.toml` as written. Every table refuses unknown keys; absent optional tables and fields stay `None`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawManifest {
    project: RawProject,
    surface: RawSurface,
    source: Option<RawSource>,
    environment: RawEnvironment,
    commands: RawCommands,
    budget: RawBudget,
    judge: RawJudge,
    policy: RawPolicy,
    proposer: Vec<RawProposer>,
    param: Option<Vec<RawParam>>,
}

/// `[project]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProject {
    name: ProjectName,
    kind: ProjectKind,
    program: RelPath,
}

/// `[surface]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSurface {
    writable: Vec<RelPath>,
    readable: Option<Vec<RelPath>>,
}

/// `[source]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSource {
    exclude: Option<Vec<String>>,
}

/// `[environment]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEnvironment {
    dockerfile: RelPath,
    requires: RawRequires,
    memory_mib: Option<u16>,
    scratch_mib: Option<u16>,
}

/// `[environment] requires` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRequires {
    arch: Arch,
}

/// `[commands]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCommands {
    build: Vec<String>,
    run: Vec<String>,
    check: Option<Vec<String>>,
}

/// `[budget]` as written. `spend_usd` is a whole-dollar integer, so a float literal is a toml type error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBudget {
    build: String,
    run: String,
    judge: Option<String>,
    spend_usd: u64,
    max_tokens: Option<u64>,
    max_empty_generations: Option<u32>,
    max_infra_failures: Option<u32>,
}

/// `[judge]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJudge {
    version: u32,
    trials: Option<u32>,
    command: Vec<String>,
    gate: Option<Vec<RawGate>>,
    objective: Vec<RawObjective>,
}

/// `[[judge.gate]]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGate {
    metric: MetricName,
}

/// `[[judge.objective]]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawObjective {
    metric: MetricName,
    goal: Goal,
    min_improvement: String,
    held_out: Option<MetricName>,
}

/// `[policy]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPolicy {
    kind: RawPolicyKind,
    width: u32,
    stall_step: Option<u32>,
}

/// `[policy] kind` as written. Only greedy is available in this release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawPolicyKind {
    /// Keep the best candidate that beats the incumbent by `min_improvement`.
    Greedy,
    /// Keep the Pareto front. Not in this release.
    Pareto,
    /// Ask the user each generation. Not in this release.
    Review,
}

/// `[[proposer]]` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProposer {
    kind: Tier,
    model: Option<ModelId>,
    batch: u32,
}

/// `[[param]]` as written: either integer `min`, `max`, and `step`, or `choices`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawParam {
    name: ParamName,
    file: RelPath,
    anchor: String,
    min: Option<i64>,
    max: Option<i64>,
    step: Option<i64>,
    choices: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------------------------------------------
// Validated manifest
// ---------------------------------------------------------------------------------------------------------------

/// Stage budgets, spend, and circuit-breaker limits, all within `limits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    build: DurationMs,
    run: DurationMs,
    judge: DurationMs,
    spend_usd: u64,
    max_tokens: u64,
    max_empty_generations: u32,
    max_infra_failures: u32,
}

impl Budget {
    /// The build stage budget, in milliseconds.
    #[must_use]
    pub const fn build_ms(&self) -> u64 {
        self.build.as_ms()
    }

    /// The run stage budget, in milliseconds.
    #[must_use]
    pub const fn run_ms(&self) -> u64 {
        self.run.as_ms()
    }

    /// The judge stage budget, in milliseconds.
    #[must_use]
    pub const fn judge_ms(&self) -> u64 {
        self.judge.as_ms()
    }

    /// The run's spend cap, in whole US dollars.
    #[must_use]
    pub const fn spend_usd(&self) -> u64 {
        self.spend_usd
    }

    /// The run's spend cap, in micro-dollars, for comparison against metered spend.
    #[must_use]
    pub const fn spend_cap_micro(&self) -> u128 {
        self.spend_usd as u128 * MICRO_PER_DOLLAR
    }

    /// The run's token cap.
    #[must_use]
    pub const fn max_tokens(&self) -> u64 {
        self.max_tokens
    }

    /// Consecutive generations with no valid candidate before the breaker stops the run.
    #[must_use]
    pub const fn max_empty_generations(&self) -> u32 {
        self.max_empty_generations
    }

    /// Consecutive infrastructure failures before the breaker stops the run.
    #[must_use]
    pub const fn max_infra_failures(&self) -> u32 {
        self.max_infra_failures
    }
}

/// The selection policy: greedy, with its width and routing stall step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicySpec {
    width: u32,
    stall_step: u32,
}

impl PolicySpec {
    /// Candidates per generation.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Generations without improvement before routing moves up one tier.
    #[must_use]
    pub const fn stall_step(&self) -> u32 {
        self.stall_step
    }
}

/// One objective: a metric, its direction, its threshold, and an optional held-out metric paired with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    metric: MetricName,
    goal: Goal,
    min_improvement: MinImprovement,
    held_out: Option<MetricName>,
}

impl Objective {
    /// The metric the judge reports.
    #[must_use]
    pub const fn metric(&self) -> &MetricName {
        &self.metric
    }

    /// Which direction is better.
    #[must_use]
    pub const fn goal(&self) -> Goal {
        self.goal
    }

    /// The smallest improvement that counts.
    #[must_use]
    pub const fn min_improvement(&self) -> &MinImprovement {
        &self.min_improvement
    }

    /// The held-out metric paired with this objective, if any.
    #[must_use]
    pub const fn held_out(&self) -> Option<&MetricName> {
        self.held_out.as_ref()
    }
}

/// The judge: its local label, its baseline trials, its argv, the project paths that argv names, its gates, and its
/// objectives.
#[derive(Debug, Clone, PartialEq)]
pub struct JudgeSpec {
    label: u32,
    trials: u32,
    command: CommandArgv,
    paths: BTreeSet<RelPath>,
    gates: BTreeSet<MetricName>,
    objectives: Vec<Objective>,
}

impl JudgeSpec {
    /// The `[judge] version` label: which pushed judge data the project uses. Not part of [`JudgeVersion`].
    #[must_use]
    pub const fn label(&self) -> u32 {
        self.label
    }

    /// The `[judge] trials` value: how many times generation 0 judges the baseline (07-CONTEXT D-05). Not part of
    /// [`JudgeVersion`].
    #[must_use]
    pub const fn trials(&self) -> u32 {
        self.trials
    }

    /// The judge's argv.
    #[must_use]
    pub const fn command(&self) -> &CommandArgv {
        &self.command
    }

    /// The project-relative path the judge's first entry names, if any.
    #[must_use]
    pub const fn program(&self) -> Option<&RelPath> {
        self.command.program()
    }

    /// Every project-relative path the judge command names, the program included.
    #[must_use]
    pub const fn paths(&self) -> &BTreeSet<RelPath> {
        &self.paths
    }

    /// The gate metrics, each of which must pass.
    #[must_use]
    pub const fn gates(&self) -> &BTreeSet<MetricName> {
        &self.gates
    }

    /// The objectives, sorted by metric.
    #[must_use]
    pub fn objectives(&self) -> &[Objective] {
        &self.objectives
    }

    /// The held-out metrics paired with objectives.
    #[must_use]
    pub fn held_out_metrics(&self) -> BTreeSet<MetricName> {
        self.objectives
            .iter()
            .filter_map(|objective| objective.held_out.clone())
            .collect()
    }

    /// Every metric the judge declares: gates, objectives, and held-out metrics.
    #[must_use]
    pub fn declared_metrics(&self) -> BTreeSet<MetricName> {
        let mut metrics = self.gates.clone();
        metrics.extend(
            self.objectives
                .iter()
                .map(|objective| objective.metric.clone()),
        );
        metrics.extend(self.held_out_metrics());
        metrics
    }
}

/// The build environment: the project's Dockerfile, the guest architecture it targets, and the VM's memory.
///
/// `memory_mib` and `scratch_mib` size the VM (05-CONTEXT D-06): both lie within the bounds in
/// [`crate::limits`], and the D-05 budget, guest reserve plus tmpfs scratch cap plus candidate allowance, fits
/// the memory, so [`Environment::candidate_allowance_mib`] is at least [`MIN_CANDIDATE_ALLOWANCE_MIB`]. The
/// Dockerfile's `USER` is ignored: the guest runs the build and run commands as one fixed unprivileged user it
/// creates itself (05-CONTEXT D-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    dockerfile: RelPath,
    arch: Arch,
    memory_mib: u16,
    scratch_mib: u16,
    candidate_allowance_mib: u16,
}

impl Environment {
    /// The Dockerfile, relative to the project.
    #[must_use]
    pub const fn dockerfile(&self) -> &RelPath {
        &self.dockerfile
    }

    /// The guest architecture.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        self.arch
    }

    /// The VM's memory, in MiB.
    #[must_use]
    pub const fn memory_mib(&self) -> u16 {
        self.memory_mib
    }

    /// The tmpfs scratch cap, in MiB.
    #[must_use]
    pub const fn scratch_mib(&self) -> u16 {
        self.scratch_mib
    }

    /// The candidate's memory allowance, in MiB: what the budget leaves after the guest reserve and the scratch
    /// cap. Computed once at parse time; callers read it here and never recompute it.
    #[must_use]
    pub const fn candidate_allowance_mib(&self) -> u16 {
        self.candidate_allowance_mib
    }
}

/// The project's commands, each an argv array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commands {
    build: CommandArgv,
    run: CommandArgv,
    check: Option<CommandArgv>,
}

impl Commands {
    /// The build command.
    #[must_use]
    pub const fn build(&self) -> &CommandArgv {
        &self.build
    }

    /// The run command.
    #[must_use]
    pub const fn run(&self) -> &CommandArgv {
        &self.run
    }

    /// The optional quick check command.
    #[must_use]
    pub const fn check(&self) -> Option<&CommandArgv> {
        self.check.as_ref()
    }
}

/// A validated `minos.toml`. If a value exists, every rule in the [module docs](self) holds.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "RawManifest")]
pub struct Manifest {
    name: ProjectName,
    kind: ProjectKind,
    program: RelPath,
    writable: BTreeSet<RelPath>,
    readable: BTreeSet<RelPath>,
    excludes: Vec<SourceExclude>,
    environment: Environment,
    commands: Commands,
    budget: Budget,
    judge: JudgeSpec,
    objective: Objective,
    policy: PolicySpec,
    proposers: Vec<ProposerSpec>,
    params: Vec<SweepParam>,
}

impl Manifest {
    /// Parses and validates the text of a `minos.toml` file.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Toml`] for a syntax error, a wrong type, a missing field, an unknown key, or a
    /// leaf value that fails its own grammar, and any other [`ManifestError`] for a rule in the
    /// [module docs](self).
    pub fn parse_toml(text: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest = toml::from_str(text).map_err(ManifestError::Toml)?;
        Self::try_from(raw)
    }

    /// The project name.
    #[must_use]
    pub const fn name(&self) -> &ProjectName {
        &self.name
    }

    /// The project kind.
    #[must_use]
    pub const fn kind(&self) -> ProjectKind {
        self.kind
    }

    /// The user's instructions to the proposer, relative to the project.
    #[must_use]
    pub const fn program(&self) -> &RelPath {
        &self.program
    }

    /// The files a proposal may change: exact paths, never prefixes.
    #[must_use]
    pub const fn writable(&self) -> &BTreeSet<RelPath> {
        &self.writable
    }

    /// The files a proposer may read but not change.
    #[must_use]
    pub const fn readable(&self) -> &BTreeSet<RelPath> {
        &self.readable
    }

    /// Every surface file: writable and readable together.
    #[must_use]
    pub fn surface(&self) -> BTreeSet<RelPath> {
        self.writable.union(&self.readable).cloned().collect()
    }

    /// The `[source] exclude` entries, sorted and without duplicates.
    #[must_use]
    pub fn excludes(&self) -> &[SourceExclude] {
        &self.excludes
    }

    /// The build environment.
    #[must_use]
    pub const fn environment(&self) -> &Environment {
        &self.environment
    }

    /// The project's commands.
    #[must_use]
    pub const fn commands(&self) -> &Commands {
        &self.commands
    }

    /// Budgets and breaker limits.
    #[must_use]
    pub const fn budget(&self) -> &Budget {
        &self.budget
    }

    /// The judge.
    #[must_use]
    pub const fn judge(&self) -> &JudgeSpec {
        &self.judge
    }

    /// The single objective the greedy policy optimizes.
    #[must_use]
    pub const fn objective(&self) -> &Objective {
        &self.objective
    }

    /// The selection policy.
    #[must_use]
    pub const fn policy(&self) -> &PolicySpec {
        &self.policy
    }

    /// The declared proposers, one per tier, in tier order.
    #[must_use]
    pub fn proposers(&self) -> &[ProposerSpec] {
        &self.proposers
    }

    /// The declared sweep params, in declaration order.
    #[must_use]
    pub fn params(&self) -> &[SweepParam] {
        &self.params
    }
}

/// Why a manifest was refused. Variants carry field paths, counts, and validated values, never raw text.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The text is not a well-formed manifest: syntax, a type, a missing field, an unknown key, or a leaf
    /// grammar. `minos.toml` is the user's own file, so toml's located message is kept whole.
    #[error("minos.toml: {0}")]
    Toml(#[source] toml::de::Error),
    /// A field is outside its allowed range.
    #[error(transparent)]
    Field(#[from] InvalidField),
    /// A path list names the same path twice.
    #[error("{field} lists {path} twice")]
    DuplicatePath {
        /// Dotted path of the list.
        field: &'static str,
        /// The repeated path.
        path: RelPath,
    },
    /// A writable path is also readable, a judge path, the program, or the Dockerfile.
    #[error("{path} is writable, so it cannot also be in {with}")]
    Overlap {
        /// The writable path.
        path: RelPath,
        /// Dotted path of the field it collides with.
        with: &'static str,
    },
    /// Two distinct paths are equal under ASCII case folding.
    #[error(
        "{first} and {second} differ only in ASCII case, so they collide on a case-insensitive filesystem"
    )]
    CaseFoldCollision {
        /// The path that sorts first.
        first: RelPath,
        /// The path that sorts second.
        second: RelPath,
    },
    /// A surface path lies under a `[source] exclude` entry.
    #[error("{field} lists {path}, which a [source] exclude entry covers")]
    Excluded {
        /// Dotted path of the surface list.
        field: &'static str,
        /// The excluded path.
        path: RelPath,
    },
    /// A surface list names a file Minos reserves: `minos.toml` or `minos.lock` in `writable`, or `minos.lock`
    /// in `readable`.
    #[error(
        "{field} lists {path}, which Minos reserves; the manifest and lockfile are never surface files"
    )]
    Reserved {
        /// Dotted path of the surface list.
        field: &'static str,
        /// The reserved path as written.
        path: RelPath,
    },
    /// A readable path is named by the judge command: the proposer must never see the scorer's source.
    #[error("{path} is named by judge.command, so it cannot be in surface.readable")]
    JudgeReadable {
        /// The judge path.
        path: RelPath,
    },
    /// A path the judge command names is not under any `[source] exclude` entry, so it would be packed into the
    /// source snapshot the candidate sandbox sees.
    #[error(
        "{path} is named by judge.command but no [source] exclude entry covers it; the judge must stay out of the source snapshot"
    )]
    JudgeNotExcluded {
        /// The judge path.
        path: RelPath,
    },
    /// The policy kind is not available in this release.
    #[error("policy kind {kind} is not available in this release; use greedy")]
    PolicyNotAvailable {
        /// The kind as written.
        kind: &'static str,
    },
    /// The greedy policy needs exactly one objective.
    #[error("the greedy policy needs exactly one [[judge.objective]], found {objectives}")]
    PolicyObjectiveMismatch {
        /// How many objectives the manifest declares.
        objectives: usize,
    },
    /// A metric name is declared more than once among gates, objectives, and held-out metrics.
    #[error(
        "metric {metric} is declared more than once among gates, objectives, and held-out metrics"
    )]
    DuplicateMetric {
        /// The repeated metric.
        metric: MetricName,
    },
    /// Two params share a name.
    #[error("param {name} is declared more than once")]
    DuplicateParam {
        /// The repeated name.
        name: ParamName,
    },
    /// A param lives in a file that is not writable.
    #[error("param {name} lives in a file that is not in surface.writable")]
    ParamFileNotWritable {
        /// The param's name.
        name: ParamName,
    },
    /// No declared proposer is eligible on manifest facts alone.
    #[error("no declared proposer is eligible; minos check prints the reason for each tier")]
    NoEligibleTier,
    /// A judge argv entry looks like a path but is not a valid relative path.
    #[error("judge.command entry {index} looks like a path but is not a valid relative path")]
    JudgeArgPath {
        /// Zero-based argv index of the entry.
        index: usize,
    },
    /// The VM's memory cannot hold the guest reserve, the scratch cap, and the least candidate allowance
    /// (05-CONTEXT D-05).
    #[error(
        "environment.memory_mib {memory_mib} cannot hold the {GUEST_RESERVE_MIB} MiB guest reserve, \
         environment.scratch_mib {scratch_mib}, and at least {MIN_CANDIDATE_ALLOWANCE_MIB} MiB for the candidate"
    )]
    MemoryBudget {
        /// The VM's memory, in MiB.
        memory_mib: u16,
        /// The tmpfs scratch cap, in MiB.
        scratch_mib: u16,
    },
}

/// A command field, with a [`ValueError`] attributed to `field`.
fn argv_at(field: &'static str, argv: Vec<String>) -> Result<CommandArgv, InvalidField> {
    CommandArgv::try_from(argv).map_err(|error| InvalidField::at(field, error))
}

/// A duration field, with a [`ValueError`] attributed to `field`.
fn duration_at(field: &'static str, text: &str) -> Result<DurationMs, InvalidField> {
    DurationMs::parse(text).map_err(|error| InvalidField::at(field, error))
}

/// An optional bounded integer: the default when absent, else the value if it lies within `min..=max`.
fn bounded<T: PartialOrd + Copy>(
    field: &'static str,
    value: Option<T>,
    (min, max, default): (T, T, T),
    limit: &'static str,
) -> Result<T, InvalidField> {
    let value = value.unwrap_or(default);
    if min <= value && value <= max {
        Ok(value)
    } else {
        Err(InvalidField::new(field, limit))
    }
}

/// Checks a list's length against `min..=max`.
const fn count(
    field: &'static str,
    len: usize,
    min: usize,
    max: usize,
    limit: &'static str,
) -> Result<(), InvalidField> {
    if min <= len && len <= max {
        Ok(())
    } else {
        Err(InvalidField::new(field, limit))
    }
}

/// A path list as a set, refusing a duplicate.
fn path_set(field: &'static str, paths: Vec<RelPath>) -> Result<BTreeSet<RelPath>, ManifestError> {
    let mut set = BTreeSet::new();
    for path in paths {
        if set.contains(&path) {
            return Err(ManifestError::DuplicatePath { field, path });
        }
        set.insert(path);
    }
    Ok(set)
}

/// The VM's memory, the scratch cap, and the candidate allowance, in MiB: defaults, leaf ranges, then the D-05
/// budget rule (05-CONTEXT D-05, D-06).
fn memory_from(
    memory_mib: Option<u16>,
    scratch_mib: Option<u16>,
) -> Result<(u16, u16, u16), ManifestError> {
    let memory_mib = bounded(
        "environment.memory_mib",
        memory_mib,
        (MIN_MEMORY_MIB, MAX_MEMORY_MIB, DEFAULT_MEMORY_MIB),
        "128 to 16384 MiB",
    )?;
    let scratch_mib = bounded(
        "environment.scratch_mib",
        scratch_mib,
        (MIN_SCRATCH_MIB, MAX_SCRATCH_MIB, DEFAULT_SCRATCH_MIB),
        "64 to 15872 MiB",
    )?;
    let allowance =
        candidate_allowance_mib(memory_mib, scratch_mib).ok_or(ManifestError::MemoryBudget {
            memory_mib,
            scratch_mib,
        })?;
    Ok((memory_mib, scratch_mib, allowance))
}

fn budget_from(raw: RawBudget) -> Result<Budget, InvalidField> {
    let build = duration_at("budget.build", &raw.build)?;
    let run = duration_at("budget.run", &raw.run)?;
    let judge = match raw.judge {
        Some(text) => duration_at("budget.judge", &text)?,
        None => DurationMs::from_const(DEFAULT_JUDGE_MS),
    };
    if raw.spend_usd > MAX_SPEND_USD {
        return Err(InvalidField::new(
            "budget.spend_usd",
            "at most 10000 whole US dollars",
        ));
    }
    Ok(Budget {
        build,
        run,
        judge,
        spend_usd: raw.spend_usd,
        max_tokens: bounded(
            "budget.max_tokens",
            raw.max_tokens,
            (TOKEN_CAP_MIN, TOKEN_CAP_MAX, TOKEN_CAP_DEFAULT),
            "1000 to 1000000000",
        )?,
        max_empty_generations: bounded(
            "budget.max_empty_generations",
            raw.max_empty_generations,
            (
                EMPTY_GENERATIONS_MIN,
                EMPTY_GENERATIONS_MAX,
                EMPTY_GENERATIONS_DEFAULT,
            ),
            "1 to 20",
        )?,
        max_infra_failures: bounded(
            "budget.max_infra_failures",
            raw.max_infra_failures,
            (
                INFRA_FAILURES_MIN,
                INFRA_FAILURES_MAX,
                INFRA_FAILURES_DEFAULT,
            ),
            "1 to 20",
        )?,
    })
}

fn objective_from(raw: RawObjective) -> Result<Objective, InvalidField> {
    let min_improvement = MinImprovement::parse(&raw.min_improvement)
        .map_err(|error| InvalidField::at("judge.objective.min_improvement", error))?;
    Ok(Objective {
        metric: raw.metric,
        goal: raw.goal,
        min_improvement,
        held_out: raw.held_out,
    })
}

fn param_from(raw: RawParam) -> Result<SweepParam, InvalidField> {
    let domain = match (raw.min, raw.max, raw.step, raw.choices) {
        (Some(min), Some(max), Some(step), None) => {
            ParamDomain::Int(IntRange::new(min, max, step)?)
        }
        (None, None, None, Some(choices)) => ParamDomain::Choices(ChoiceList::new(choices)?),
        _ => {
            return Err(InvalidField::new(
                "param",
                "either min, max, and step together, or choices",
            ));
        }
    };
    SweepParam::new(raw.name, raw.file, raw.anchor, domain)
}

/// Every project-relative path the judge command names (see the [module docs](self)).
fn judge_paths(command: &CommandArgv) -> Result<BTreeSet<RelPath>, ManifestError> {
    let path_like = |text: &str| text.contains(['/', '\\']);
    let mut paths = BTreeSet::new();
    for (index, entry) in command.as_slice().iter().enumerate() {
        let candidate = if entry.starts_with('-') {
            match entry.split_once('=') {
                Some((_, value)) => value,
                None if path_like(entry) => return Err(ManifestError::JudgeArgPath { index }),
                None => continue,
            }
        } else {
            entry.as_str()
        };
        if candidate.starts_with('/') {
            continue;
        }
        let dotted = candidate.strip_prefix("./");
        let text = dotted.unwrap_or(candidate);
        // The same rule as `CommandArgv::program`: a bare word names a program on PATH or is a plain argument,
        // never a project path. Anything path-like must then be a valid RelPath.
        if dotted.is_none() && !path_like(candidate) && !matches!(text, "." | "..") {
            continue;
        }
        match RelPath::try_from(text) {
            Ok(path) => {
                paths.insert(path);
            }
            Err(_) => return Err(ManifestError::JudgeArgPath { index }),
        }
    }
    Ok(paths)
}

/// Refuses two distinct paths that are equal under ASCII case folding.
fn check_case_fold<'a>(paths: impl IntoIterator<Item = &'a RelPath>) -> Result<(), ManifestError> {
    let distinct: BTreeSet<&RelPath> = paths.into_iter().collect();
    let mut seen: BTreeMap<String, &RelPath> = BTreeMap::new();
    for path in distinct {
        match seen.entry(path.ascii_case_key()) {
            Entry::Occupied(entry) => {
                return Err(ManifestError::CaseFoldCollision {
                    first: (*entry.get()).clone(),
                    second: path.clone(),
                });
            }
            Entry::Vacant(entry) => {
                entry.insert(path);
            }
        }
    }
    Ok(())
}

impl TryFrom<RawManifest> for Manifest {
    type Error = ManifestError;

    fn try_from(raw: RawManifest) -> Result<Self, Self::Error> {
        let RawManifest {
            project,
            surface,
            source,
            environment,
            commands,
            budget,
            judge,
            policy,
            proposer,
            param,
        } = raw;

        // 1. Leaf validation.
        let commands = Commands {
            build: argv_at("commands.build", commands.build)?,
            run: argv_at("commands.run", commands.run)?,
            check: commands
                .check
                .map(|check| argv_at("commands.check", check))
                .transpose()?,
        };
        let budget = budget_from(budget)?;
        let (memory_mib, scratch_mib, candidate_allowance_mib) =
            memory_from(environment.memory_mib, environment.scratch_mib)?;
        if !(WIDTH_MIN..=WIDTH_MAX).contains(&policy.width) {
            return Err(InvalidField::new("policy.width", "1 to 16").into());
        }
        let policy_spec = PolicySpec {
            width: policy.width,
            stall_step: bounded(
                "policy.stall_step",
                policy.stall_step,
                (STALL_STEP_MIN, STALL_STEP_MAX, STALL_STEP_DEFAULT),
                "1 to 100",
            )?,
        };
        if !(1..=MAX_JUDGE_LABEL).contains(&judge.version) {
            return Err(InvalidField::new("judge.version", "1 to 1000000").into());
        }
        let trials = bounded(
            "judge.trials",
            judge.trials,
            (TRIALS_MIN, MAX_TRIALS, TRIALS_DEFAULT),
            "1 to 32",
        )?;
        let command = argv_at("judge.command", judge.command)?;
        let gates = judge.gate.unwrap_or_default();
        count("judge.gate", gates.len(), 0, MAX_GATES, "at most 16 gates")?;
        count(
            "judge.objective",
            judge.objective.len(),
            0,
            MAX_OBJECTIVES,
            "at most 8 objectives",
        )?;
        let objectives = judge
            .objective
            .into_iter()
            .map(objective_from)
            .collect::<Result<Vec<_>, _>>()?;
        count(
            "proposer",
            proposer.len(),
            1,
            MAX_PROPOSERS,
            "1 to 4 entries",
        )?;
        let mut proposers = proposer
            .into_iter()
            .map(|raw| ProposerSpec::new(raw.kind, raw.model, raw.batch))
            .collect::<Result<Vec<_>, _>>()?;
        let params = param.unwrap_or_default();
        count("param", params.len(), 0, MAX_PARAMS, "at most 32 params")?;
        let params = params
            .into_iter()
            .map(param_from)
            .collect::<Result<Vec<_>, _>>()?;
        let excludes = source.and_then(|source| source.exclude).unwrap_or_default();
        count(
            "source.exclude",
            excludes.len(),
            0,
            MAX_EXCLUDES,
            "at most 256 entries",
        )?;
        let excludes: BTreeSet<SourceExclude> = excludes
            .iter()
            .map(|text| {
                SourceExclude::parse(text)
                    .map_err(|error| InvalidField::at("source.exclude", error))
            })
            .collect::<Result<_, _>>()?;

        // 2. The surface: exact files, no duplicates, writable and readable disjoint (D-01).
        count(
            "surface.writable",
            surface.writable.len(),
            1,
            MAX_WRITABLE,
            "1 to 64 files",
        )?;
        let writable = path_set("surface.writable", surface.writable)?;
        let readable = surface.readable.unwrap_or_default();
        count(
            "surface.readable",
            readable.len(),
            0,
            MAX_READABLE,
            "at most 256 files",
        )?;
        let readable = path_set("surface.readable", readable)?;
        if let Some(path) = writable.intersection(&readable).next() {
            return Err(ManifestError::Overlap {
                path: path.clone(),
                with: "surface.readable",
            });
        }

        // 3. Judge integrity: nothing the judge command names, and neither program nor Dockerfile, is writable,
        //    and nothing the judge command names is readable.
        let paths = judge_paths(&command)?;
        let program = project.program;
        let dockerfile = environment.dockerfile;
        let protected = [
            (paths.iter().collect::<Vec<_>>(), "judge.command"),
            (vec![&program], "project.program"),
            (vec![&dockerfile], "environment.dockerfile"),
        ];
        for (named, with) in protected {
            if let Some(path) = named.into_iter().find(|path| writable.contains(*path)) {
                return Err(ManifestError::Overlap {
                    path: path.clone(),
                    with,
                });
            }
        }
        if let Some(path) = paths.iter().find(|path| readable.contains(*path)) {
            return Err(ManifestError::JudgeReadable { path: path.clone() });
        }
        for (field, set, reserved) in [
            ("surface.writable", &writable, &RESERVED_WRITABLE[..]),
            ("surface.readable", &readable, &RESERVED_READABLE[..]),
        ] {
            if let Some(path) = set
                .iter()
                .find(|path| reserved.contains(&path.ascii_case_key().as_str()))
            {
                return Err(ManifestError::Reserved {
                    field,
                    path: path.clone(),
                });
            }
        }

        // 4. No surface path under an exclude entry.
        for (field, set) in [
            ("surface.writable", &writable),
            ("surface.readable", &readable),
        ] {
            if let Some(path) = set
                .iter()
                .find(|path| excludes.iter().any(|exclude| exclude.covers(path)))
            {
                return Err(ManifestError::Excluded {
                    field,
                    path: path.clone(),
                });
            }
        }

        // 5. Case-fold collisions across every path the project names.
        check_case_fold(
            writable
                .iter()
                .chain(&readable)
                .chain([&program, &dockerfile])
                .chain(&paths),
        )?;

        // 6. Every judge path lies under an exclude entry, so the judge program never enters the snapshot.
        if let Some(path) = paths
            .iter()
            .find(|path| !excludes.iter().any(|exclude| exclude.covers(path)))
        {
            return Err(ManifestError::JudgeNotExcluded { path: path.clone() });
        }

        // 7. Policy.
        match policy.kind {
            RawPolicyKind::Greedy => {}
            RawPolicyKind::Pareto => {
                return Err(ManifestError::PolicyNotAvailable { kind: "pareto" });
            }
            RawPolicyKind::Review => {
                return Err(ManifestError::PolicyNotAvailable { kind: "review" });
            }
        }
        let objective = match objectives.as_slice() {
            [only] => only.clone(),
            other => {
                return Err(ManifestError::PolicyObjectiveMismatch {
                    objectives: other.len(),
                });
            }
        };

        // 8. Metric names are unique across gates, objectives, and held-out metrics.
        let mut metrics = BTreeSet::new();
        let declared = gates
            .iter()
            .map(|gate| &gate.metric)
            .chain(objectives.iter().flat_map(|objective| {
                std::iter::once(&objective.metric).chain(objective.held_out.as_ref())
            }));
        for metric in declared {
            if !metrics.insert(metric) {
                return Err(ManifestError::DuplicateMetric {
                    metric: metric.clone(),
                });
            }
        }

        // 9. Params: unique names, each in a writable file.
        let mut names = BTreeSet::new();
        for param in &params {
            if !names.insert(param.name()) {
                return Err(ManifestError::DuplicateParam {
                    name: param.name().clone(),
                });
            }
            if !writable.contains(param.file()) {
                return Err(ManifestError::ParamFileNotWritable {
                    name: param.name().clone(),
                });
            }
        }

        // 10. Proposers: one per tier, at least one eligible.
        proposers.sort_by_key(ProposerSpec::tier);
        if proposers
            .windows(2)
            .any(|pair| matches!(pair, [first, second] if first.tier() == second.tier()))
        {
            return Err(InvalidField::new("proposer.kind", "one entry per tier").into());
        }
        if !proposers
            .iter()
            .any(|spec| spec.declared_eligibility(&params) == Eligibility::Eligible)
        {
            return Err(ManifestError::NoEligibleTier);
        }

        let mut objectives = objectives;
        objectives.sort_by(|left, right| left.metric.cmp(&right.metric));
        let judge = JudgeSpec {
            label: judge.version,
            trials,
            command,
            paths,
            gates: gates.into_iter().map(|gate| gate.metric).collect(),
            objectives,
        };
        Ok(Self {
            name: project.name,
            kind: project.kind,
            program,
            writable,
            readable,
            excludes: excludes.into_iter().collect(),
            environment: Environment {
                dockerfile,
                arch: environment.requires.arch,
                memory_mib,
                scratch_mib,
                candidate_allowance_mib,
            },
            commands,
            budget,
            judge,
            objective,
            policy: policy_spec,
            proposers,
            params,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JudgeVersion
// ---------------------------------------------------------------------------------------------------------------

crate::id::digest_id! {
    /// The identity of a judge: the judge data tree plus the canonical judge spec, under the domain
    /// `minos/judge/v1` (CORE-01; 03-CONTEXT D-10 amendment, user decision 2026-10-14).
    ///
    /// The spec is the command argv, the gates in sorted order, and the objectives in metric order with each
    /// goal and held-out pairing. The `[judge] version` label, `[judge] trials`, and `min_improvement` are
    /// excluded: they do not change what the judge measures. `Judged` events are keyed by it, so re-scoring under another judge
    /// appends records and never rewrites history.
    JudgeVersion
}

impl JudgeVersion {
    /// The version of `judge` scoring with the pushed judge data `data`.
    ///
    /// Fields, in order: the data digest; the argv count and each entry, length-prefixed; the gate count and each
    /// gate; the objective count and, per objective, its metric, `1` for maximize or `2` for minimize, and `0`
    /// with no held-out pairing or `1` followed by the held-out metric.
    #[must_use]
    pub fn of(data: &TreeId, judge: &JudgeSpec) -> Self {
        let argv = judge.command.as_slice();
        let mut hasher = IdHasher::new(JUDGE_DOMAIN)
            .digest(data.as_bytes())
            .u64(argv.len() as u64);
        for entry in argv {
            hasher = hasher.bytes(entry.as_bytes());
        }
        hasher = hasher.u64(judge.gates.len() as u64);
        for gate in &judge.gates {
            hasher = hasher.bytes(gate.as_str().as_bytes());
        }
        hasher = hasher.u64(judge.objectives.len() as u64);
        for objective in &judge.objectives {
            let goal = match objective.goal {
                Goal::Maximize => 1,
                Goal::Minimize => 2,
            };
            hasher = hasher.bytes(objective.metric.as_str().as_bytes()).u64(goal);
            hasher = match &objective.held_out {
                Some(held_out) => hasher.u64(1).bytes(held_out.as_str().as_bytes()),
                None => hasher.u64(0),
            };
        }
        Self::from_digest(hasher.finish())
    }
}
