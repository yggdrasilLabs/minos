//! `RawManifest` (the mirror of `minos.toml`) and the validated `Manifest` built from it.
//!
//! This module holds the manifest's leaf types. Each has private fields and one validating constructor, which
//! serde reaches through `try_from`, so a value that exists is valid.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::ModelId;
use crate::limits::{
    BATCH_MAX, BATCH_MIN, MAX_ANCHOR_BYTES, MAX_ARG_BYTES, MAX_ARGV_ENTRIES, MAX_CHOICE_BYTES,
    MAX_CHOICES, MAX_MIN_IMPROVEMENT_PERCENT, MAX_NAME_BYTES, MAX_PARAM_POINTS, MAX_STAGE_MS,
    MIN_STAGE_MS,
};
use crate::path::RelPath;

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
    pub const fn declared_eligibility(&self, params: &[SweepParam]) -> Eligibility {
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
