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

use std::collections::BTreeMap;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::id::{IdHasher, RunId};
use crate::manifest::{ParamDomain, ParamName, SweepParam};

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

/// Why a sweep could not produce a sample. Every variant is a reason to drop the sample, never an
/// infrastructure error, and none is retried.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SweepError {
    /// The manifest declares no params, so there is nothing to sweep.
    #[error("sweep needs at least one [[param]]")]
    NoParams,
    /// A drawn index had no value in the param's domain. Validated params never produce this.
    #[error("param {name} has no value at the drawn index")]
    Domain {
        /// The param.
        name: ParamName,
    },
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
