//! The seeded sweep sampler: value-stable sampling over declared params, with no model and no unseeded randomness
//! (CORE-12).
//!
//! A batch is a pure function of the run, the generation, the declared params, and the batch size, so a resumed
//! run (and, later, a durable replay) draws exactly the samples the first attempt drew:
//!
//! - **Seed.** [`seed`] hashes the run and generation through [`IdHasher`] in the domain `minos/sweep/v1`, and the
//!   32-byte result seeds one [`ChaCha8Rng`] per generation.
//! - **Draw order.** For each batch item in turn, every param is drawn once, in manifest declaration order. An
//!   integer param takes `min + index * step`; a choice param takes the choice at `index`.
//! - **Range mapping.** The index is drawn by our own unbiased rejection sampler over `next_u64`, never by a
//!   `rand` distribution or range helper, because those are not value-stable across crate versions (03-RESEARCH
//!   Pitfall 8): an upgrade would silently change every sample and break golden vectors and replay. The only
//!   thing taken from the RNG crate is the raw `ChaCha8` stream, which the tests pin to a published known-answer
//!   vector.
//! - **No floats.** Params are integer ranges and literal string choices only, so no libm-dependent function and
//!   no float formatting is involved.
//!
//! [`sample`] does not deduplicate: admission (Phase 7) drops a proposal already tried.
//!
//! [`render`] turns a sample into a proposal: in each param's writable file it replaces the rest of the one base
//! line that starts with the param's anchor, then derives the canonical cumulative diff with
//! [`diff_trees`], so a sweep proposal passes the same [`Diff::parse`] validation, writable set included, as a
//! model's.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::diff::Diff;
use crate::diff::apply::Tree;
use crate::diff::compose::{ComposeError, diff_trees};
use crate::id::{IdHasher, RunId};
use crate::limits::Limits;
use crate::manifest::{ParamDomain, ParamName, SweepParam};
use crate::path::RelPath;

/// Domain of [`seed`].
const SWEEP_DOMAIN: &str = "minos/sweep/v1";

/// The seed of generation `generation` of `run`: `IdHasher` in the domain `minos/sweep/v1` over the run ID, then
/// the generation as a `u64`.
#[must_use]
pub fn seed(run: &RunId, generation: u32) -> [u8; 32] {
    IdHasher::new(SWEEP_DOMAIN)
        .digest(run.as_bytes())
        .u64(u64::from(generation))
        .finish()
}

/// An unbiased draw in `[0, n)` by rejection, or `None` for `n == 0`.
///
/// Values below `2^64 mod n` are rejected, so the accepted range is a whole number of copies of `[0, n)` and the
/// remainder is uniform. Each draw is accepted with probability above 1/2, so the loop ends after very few draws.
fn below(rng: &mut ChaCha8Rng, n: u64) -> Option<u64> {
    if n == 0 {
        return None;
    }
    let threshold = n.wrapping_neg() % n;
    loop {
        let x = rng.next_u64();
        if x >= threshold {
            return Some(x % n);
        }
    }
}

/// One sampled value of a param.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParamValue {
    /// A point of an integer range.
    Int(i64),
    /// One of a choice list's literal strings.
    Choice(String),
}

impl ParamValue {
    /// The text written after the param's anchor: the decimal integer, or the choice itself.
    #[must_use]
    pub fn render_text(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::Choice(choice) => choice.clone(),
        }
    }
}

/// One sweep proposal: a value for every declared param.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    values: BTreeMap<ParamName, ParamValue>,
}

impl Sample {
    /// The value of `name`, if it is a declared param.
    #[must_use]
    pub fn get(&self, name: &ParamName) -> Option<&ParamValue> {
        self.values.get(name)
    }

    /// Names and values, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&ParamName, &ParamValue)> {
        self.values.iter()
    }

    /// `name=value` pairs joined by `, `, in name order: the sweep's rationale text.
    #[must_use]
    pub fn describe(&self) -> String {
        self.values
            .iter()
            .map(|(name, value)| format!("{name}={}", value.render_text()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Why a sweep could not produce or render a sample. Every variant is a reason to drop the sample, never an
/// infrastructure error, and none is retried.
///
/// Variants carry validated names and paths, never file content.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SweepError {
    /// The manifest declares no params, so there is nothing to sweep.
    #[error("sweep needs at least one [[param]]")]
    NoParams,
    /// A drawn index, or a value handed to [`render`], is outside the param's domain. Validated params, and samples
    /// drawn from them, never produce this.
    #[error("param {name} has a value outside its domain")]
    Domain {
        /// The param.
        name: ParamName,
    },
    /// The sample has no value for a declared param.
    #[error("the sample assigns no value to param {name}")]
    Unassigned {
        /// The param.
        name: ParamName,
    },
    /// The sample has a value for a name that is not a declared param.
    #[error("the sample assigns {name}, which is not a declared param")]
    Undeclared {
        /// The undeclared name.
        name: ParamName,
    },
    /// No line of the param's base file starts with its anchor.
    #[error("no line starts with the anchor of param {name}")]
    AnchorMissing {
        /// The param.
        name: ParamName,
    },
    /// More than one line of the param's base file starts with its anchor.
    #[error("more than one line starts with the anchor of param {name}")]
    AnchorAmbiguous {
        /// The param.
        name: ParamName,
    },
    /// The param's anchor line is already claimed by a param declared earlier.
    #[error("param {name} anchors a line an earlier param already anchors")]
    AnchorConflict {
        /// The later param.
        name: ParamName,
    },
    /// The param's file is not in the base tree.
    #[error("{path} is not in the base tree")]
    FileMissing {
        /// The param's file.
        path: RelPath,
    },
    /// The param's file is not UTF-8 text.
    #[error("{path} is not UTF-8 text")]
    NotText {
        /// The param's file.
        path: RelPath,
    },
    /// Every rendered value equals the base text, so the sample is the baseline (03-CONTEXT D-03).
    #[error("the sample renders to the base tree")]
    NoChange,
    /// The rendered tree did not become a valid cumulative diff, for example a cap or a non-writable path.
    #[error(transparent)]
    Compose(ComposeError),
}

/// `batch` samples for generation `generation` of `run`, drawn from one `ChaCha8` stream seeded by [`seed`].
///
/// # Errors
///
/// Returns [`SweepError::NoParams`] if `params` is empty, and [`SweepError::Domain`] if a domain has no value at a
/// drawn index, which validated params never cause.
pub fn sample(
    run: &RunId,
    generation: u32,
    params: &[SweepParam],
    batch: u32,
) -> Result<Vec<Sample>, SweepError> {
    if params.is_empty() {
        return Err(SweepError::NoParams);
    }
    let mut rng = ChaCha8Rng::from_seed(seed(run, generation));
    (0..batch)
        .map(|_| {
            let values = params
                .iter()
                .map(|param| Ok((param.name().clone(), draw(&mut rng, param)?)))
                .collect::<Result<BTreeMap<_, _>, SweepError>>()?;
            Ok(Sample { values })
        })
        .collect()
}

/// One value of `param`, drawn from `rng`.
fn draw(rng: &mut ChaCha8Rng, param: &SweepParam) -> Result<ParamValue, SweepError> {
    let missing = || SweepError::Domain {
        name: param.name().clone(),
    };
    match param.domain() {
        ParamDomain::Int(range) => below(rng, range.points())
            .and_then(|index| range.value_at(index))
            .map(ParamValue::Int)
            .ok_or_else(missing),
        ParamDomain::Choices(choices) => {
            let count = u64::try_from(choices.len()).map_err(|_| missing())?;
            below(rng, count)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| choices.get(index))
                .map(|choice| ParamValue::Choice(choice.to_owned()))
                .ok_or_else(missing)
        }
    }
}

/// The canonical cumulative diff that sets every param of `sample` in `base`.
///
/// For each param, in declaration order, the param's file must be UTF-8 text in `base`, and exactly one of its
/// base lines must start with the anchor. Everything after the anchor up to the line's newline is replaced by the
/// value's [`ParamValue::render_text`]; the newline, or its absence on a last line, is kept. Anchors are matched
/// against the base text, never against an earlier param's edit, so the result does not depend on the order of
/// edits. The edited tree then goes through [`diff_trees`] with `writable` and `limits`.
///
/// # Errors
///
/// Returns [`SweepError::Undeclared`] or [`SweepError::Unassigned`] if the sample's names are not exactly the
/// params, [`SweepError::Domain`] for a value outside its param's domain, [`SweepError::FileMissing`],
/// [`SweepError::NotText`], [`SweepError::AnchorMissing`], [`SweepError::AnchorAmbiguous`], or
/// [`SweepError::AnchorConflict`] for a param that cannot be located, [`SweepError::NoChange`] if the result
/// equals the base, and [`SweepError::Compose`] if the diff fails validation.
pub fn render(
    base: &Tree,
    params: &[SweepParam],
    sample: &Sample,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<Diff, SweepError> {
    if let Some(name) = sample
        .values
        .keys()
        .find(|name| !params.iter().any(|param| param.name() == *name))
    {
        return Err(SweepError::Undeclared { name: name.clone() });
    }
    let mut edits: BTreeMap<&RelPath, FileEdit<'_>> = BTreeMap::new();
    for param in params {
        let name = param.name();
        let value = sample
            .get(name)
            .ok_or_else(|| SweepError::Unassigned { name: name.clone() })?;
        if !in_domain(param.domain(), value) {
            return Err(SweepError::Domain { name: name.clone() });
        }
        let path = param.file();
        let edit = match edits.entry(path) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(FileEdit::of(base, path)?),
        };
        edit.set(param, value)?;
    }
    let mut candidate = base.clone();
    for (path, edit) in edits {
        candidate.insert(path.clone(), edit.text().into_bytes());
    }
    diff_trees(base, &candidate, writable, limits).map_err(|error| match error {
        ComposeError::NoChange => SweepError::NoChange,
        other => SweepError::Compose(other),
    })
}

/// Whether `value` is a value of `domain`.
fn in_domain(domain: &ParamDomain, value: &ParamValue) -> bool {
    match (domain, value) {
        (ParamDomain::Int(range), ParamValue::Int(value)) => {
            let offset = i128::from(*value) - i128::from(range.min());
            *value >= range.min() && *value <= range.max() && offset % i128::from(range.step()) == 0
        }
        (ParamDomain::Choices(choices), ParamValue::Choice(value)) => {
            choices.as_slice().iter().any(|choice| choice == value)
        }
        (ParamDomain::Int(_), ParamValue::Choice(_))
        | (ParamDomain::Choices(_), ParamValue::Int(_)) => false,
    }
}

/// The pending edits to one base file: its base lines, each keeping its `\n`, and the replaced lines by index.
struct FileEdit<'a> {
    lines: Vec<&'a str>,
    replaced: BTreeMap<usize, String>,
}

impl<'a> FileEdit<'a> {
    /// The base lines of `path`.
    fn of(base: &'a Tree, path: &RelPath) -> Result<Self, SweepError> {
        let bytes = base
            .get(path)
            .ok_or_else(|| SweepError::FileMissing { path: path.clone() })?;
        let text =
            std::str::from_utf8(bytes).map_err(|_| SweepError::NotText { path: path.clone() })?;
        Ok(Self {
            lines: text.split_inclusive('\n').collect(),
            replaced: BTreeMap::new(),
        })
    }

    /// Replaces the rest of `param`'s anchored base line with `value`.
    fn set(&mut self, param: &SweepParam, value: &ParamValue) -> Result<(), SweepError> {
        let name = || param.name().clone();
        let anchor = param.anchor();
        let matches: Vec<(usize, &str)> = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with(anchor))
            .map(|(index, line)| (index, *line))
            .collect();
        let (index, line) = match matches.as_slice() {
            [] => return Err(SweepError::AnchorMissing { name: name() }),
            [one] => *one,
            [_, _, ..] => return Err(SweepError::AnchorAmbiguous { name: name() }),
        };
        let ending = if line.ends_with('\n') { "\n" } else { "" };
        let replacement = format!("{anchor}{}{ending}", value.render_text());
        match self.replaced.entry(index) {
            Entry::Occupied(_) => Err(SweepError::AnchorConflict { name: name() }),
            Entry::Vacant(entry) => {
                entry.insert(replacement);
                Ok(())
            }
        }
    }

    /// The edited file text.
    fn text(&self) -> String {
        self.lines
            .iter()
            .enumerate()
            .map(|(index, line)| self.replaced.get(&index).map_or(*line, String::as_str))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    use super::below;

    #[test]
    fn below_zero_is_none() {
        let mut rng = ChaCha8Rng::from_seed([7_u8; 32]);
        assert_eq!(below(&mut rng, 0), None);
    }

    #[test]
    fn below_one_is_always_zero() {
        let mut rng = ChaCha8Rng::from_seed([7_u8; 32]);
        assert!((0..1000).all(|_| below(&mut rng, 1) == Some(0)));
    }

    #[test]
    fn below_reaches_every_value_of_a_small_range() {
        let mut rng = ChaCha8Rng::from_seed([7_u8; 32]);
        let mut seen = [false; 3];
        for _ in 0..1000 {
            if let Some(slot) = below(&mut rng, 3)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| seen.get_mut(index))
            {
                *slot = true;
            }
        }
        assert_eq!(seen, [true; 3]);
    }

    proptest! {
        #[test]
        fn below_is_under_n(seed in any::<[u8; 32]>(), n in 1_u64..) {
            let mut rng = ChaCha8Rng::from_seed(seed);
            for _ in 0..64 {
                prop_assert!(below(&mut rng, n).is_some_and(|value| value < n));
            }
        }

        #[test]
        fn below_is_under_n_near_the_top(seed in any::<[u8; 32]>(), gap in 0_u64..1024) {
            let n = u64::MAX - gap;
            let mut rng = ChaCha8Rng::from_seed(seed);
            for _ in 0..64 {
                prop_assert!(below(&mut rng, n).is_some_and(|value| value < n));
            }
        }
    }
}
