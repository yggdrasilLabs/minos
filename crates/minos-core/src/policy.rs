//! Pure selection policy: the scorecard gate check, the lower-median reduction of trials, the improvement test,
//! greedy selection, the generation conclusion, the noise check, the circuit breaker, and the spend check
//! (CORE-10, CORE-17; 03-CONTEXT D-09, D-11, D-12, D-14).
//!
//! Every `if` that decides an experiment's fate lives here, and every function is a pure function of recorded
//! results: no clock, no randomness, and no dependence on the order of its input. Trials are sorted before they
//! are reduced, candidates are compared by value and then by [`ExperimentId`], never by position, and every
//! collection is a `BTreeMap`. So resume, review, and re-scoring all see the same decisions for the same ledger.
//!
//! Decisions use only IEEE-754 add, subtract, multiply, and compare, which are correctly rounded on every
//! platform Rust supports. No decision divides by the incumbent (03-RESEARCH Pitfall 7), and none calls a
//! libm-dependent function whose result can differ between platforms.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::id::{ExperimentId, MetricName};
use crate::ledger::{Decision, DecisionKind, SpendSource, StopReason};
use crate::limits::{MIN_NOISE_TRIALS, NOISE_REFUSE_MULTIPLE, NOISE_WARN_MULTIPLE};
use crate::manifest::{Budget, Goal, JudgeSpec, JudgeVersion, Manifest, Objective};
use crate::metric::{Metric, Scorecard};
use crate::money::{ApproxCost, Cost, CostError, Prices};
use crate::outcome::{Outcome, Reason};
use crate::state::RunState;

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// Why a generation could not be concluded. Each is a fault of the driver or the ledger, never of a candidate:
/// a candidate's failure is an [`Outcome`] and only ever makes it invalid for selection.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// The incumbent the generation competes against has no valid reduced score under the judge: the baseline
    /// was never calibrated, or was not re-scored under this judge version.
    #[error("incumbent {exp} has no valid score under this judge")]
    IncumbentUnscored {
        /// The incumbent.
        exp: ExperimentId,
    },
    /// The generation recorded more proposals than a `u32` counts.
    #[error("the generation has more proposals than a decision can count")]
    TooManyProposals,
    /// Generation 0 is baseline calibration and has no decision.
    #[error("generation 0 is baseline calibration and is never concluded")]
    GenerationZero,
}

// ---------------------------------------------------------------------------------------------------------------
// Scorecard gate check
// ---------------------------------------------------------------------------------------------------------------

/// Checks a scorecard against the judge's declarations, gates first (JUDGE-03).
///
/// The rules, in order:
///
/// 1. Each gate, in sorted order, must be present ([`Reason::MissingMetric`]), not marked held-out
///    ([`Reason::HeldOutMismatch`]), and pass. A gate passes only when its value equals exactly 1
///    ([`Metric::ONE`]); any other value is [`Reason::GateFailed`].
/// 2. Each objective metric must be present and not marked held-out.
/// 3. Each held-out metric paired with an objective must be present and marked held-out.
/// 4. Every metric on the card must be declared by the judge ([`Reason::UndeclaredMetric`]).
///
/// Phase 7's runner calls this on every judge scorecard before recording it as [`Outcome::Scored`], and
/// [`conclude`] calls it again on every recorded trial, so a scorecard that fails a gate never reaches selection.
///
/// # Errors
///
/// The first broken rule's [`Reason`]. Every payload is a validated [`MetricName`].
pub fn check_scorecard(judge: &JudgeSpec, card: &Scorecard) -> Result<(), Reason> {
    for gate in judge.gates() {
        let scored = present(card, gate, false)?;
        if scored != Metric::ONE {
            return Err(Reason::GateFailed { gate: gate.clone() });
        }
    }
    for objective in judge.objectives() {
        present(card, objective.metric(), false)?;
    }
    for held_out in judge.held_out_metrics() {
        present(card, &held_out, true)?;
    }
    let declared = judge.declared_metrics();
    if let Some((metric, _)) = card.iter().find(|(name, _)| !declared.contains(*name)) {
        return Err(Reason::UndeclaredMetric {
            metric: metric.clone(),
        });
    }
    Ok(())
}

/// The value of `metric` on `card`, which must be present with the `held_out` flag the judge declares.
fn present(card: &Scorecard, metric: &MetricName, held_out: bool) -> Result<Metric, Reason> {
    let scored = card.get(metric).ok_or_else(|| Reason::MissingMetric {
        metric: metric.clone(),
    })?;
    if scored.held_out() != held_out {
        return Err(Reason::HeldOutMismatch {
            metric: metric.clone(),
        });
    }
    Ok(scored.value())
}

// ---------------------------------------------------------------------------------------------------------------
// Trial reduction
// ---------------------------------------------------------------------------------------------------------------

/// The lower median of `values`: the middle value of a sorted copy, and the lower of the two middle values for an
/// even count, so two metrics are never averaged (D-11). `None` for no values.
///
/// The sort uses [`Metric`]'s total order, so the result does not depend on the order of `values`.
#[must_use]
pub fn lower_median(values: &[Metric]) -> Option<Metric> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len().checked_sub(1)? / 2;
    sorted.get(middle).copied()
}

/// How a reduction treats a trial that is not [`Outcome::Scored`] (or, in [`conclude`], one whose scorecard
/// fails [`check_scorecard`]).
///
/// A candidate is judged under [`TrialRule::AllScored`]: one failed trial makes it invalid, because a change
/// that crashes or times out once is not an improvement. The incumbent and the baseline are judged under
/// [`TrialRule::ScoredOnly`]: they are already accepted code, a trial that did not score measured nothing, and
/// one busy-host timeout during calibration must not block every later generation. The number of trials skipped
/// is carried on the result so the report can say "3 of 4 trials scored".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialRule {
    /// Every trial must be scored; otherwise the experiment has no score.
    AllScored,
    /// Unscored trials are skipped and counted; at least one scored trial is needed.
    ScoredOnly,
}

/// One experiment's trials under one judge version, reduced to one value per metric by the lower median.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedScore {
    metrics: BTreeMap<MetricName, Metric>,
    trials: u32,
    skipped: u32,
}

impl ReducedScore {
    /// The reduced value of every metric, in name order.
    #[must_use]
    pub const fn metrics(&self) -> &BTreeMap<MetricName, Metric> {
        &self.metrics
    }

    /// The reduced value of `metric`, if every trial reported it.
    #[must_use]
    pub fn get(&self, metric: &MetricName) -> Option<Metric> {
        self.metrics.get(metric).copied()
    }

    /// The number of trials reduced.
    #[must_use]
    pub const fn trials(&self) -> u32 {
        self.trials
    }

    /// The number of unscored trials skipped under [`TrialRule::ScoredOnly`]; always 0 under
    /// [`TrialRule::AllScored`].
    #[must_use]
    pub const fn skipped(&self) -> u32 {
        self.skipped
    }
}

/// Reduces an experiment's trials to one score per metric by the lower median (D-11).
///
/// `None`, meaning the experiment has no score, when there are no scored trials, when a metric is absent from
/// some scored trial, when there are more trials than a `u32` counts, or, under [`TrialRule::AllScored`], when
/// any trial is not [`Outcome::Scored`]. Under [`TrialRule::ScoredOnly`] unscored trials are skipped and
/// counted. A single trial is its own median. The result does not depend on the order of `outcomes`.
pub fn reduce_trials<'a>(
    outcomes: impl IntoIterator<Item = &'a Outcome>,
    rule: TrialRule,
) -> Option<ReducedScore> {
    reduce_cards(outcomes.into_iter().map(Outcome::scorecard), rule)
}

/// [`reduce_trials`] over the trials' scorecards, `None` standing for an unscored trial.
fn reduce_cards<'a>(
    cards: impl IntoIterator<Item = Option<&'a Scorecard>>,
    rule: TrialRule,
) -> Option<ReducedScore> {
    let mut values: BTreeMap<MetricName, Vec<Metric>> = BTreeMap::new();
    let mut count: usize = 0;
    let mut skipped: u32 = 0;
    for card in cards {
        let Some(card) = card else {
            match rule {
                TrialRule::AllScored => return None,
                TrialRule::ScoredOnly => {
                    skipped = skipped.checked_add(1)?;
                    continue;
                }
            }
        };
        count = count.checked_add(1)?;
        for (name, scored) in card.iter() {
            values.entry(name.clone()).or_default().push(scored.value());
        }
    }
    let trials = u32::try_from(count).ok().filter(|trials| *trials > 0)?;
    let mut metrics = BTreeMap::new();
    for (name, list) in values {
        if list.len() != count {
            return None;
        }
        metrics.insert(name, lower_median(&list)?);
    }
    Some(ReducedScore {
        metrics,
        trials,
        skipped,
    })
}

/// The reduced score of `exp` under `version`, counting only trials whose scorecard passes [`check_scorecard`]
/// against `spec`. A recorded trial that fails the check is treated as unscored: under
/// [`TrialRule::AllScored`] it makes the experiment invalid, under [`TrialRule::ScoredOnly`] it is skipped.
fn valid_score(
    state: &RunState,
    spec: &JudgeSpec,
    version: &JudgeVersion,
    exp: &ExperimentId,
    rule: TrialRule,
) -> Option<ReducedScore> {
    reduce_cards(
        state.trials(exp, version).map(|(_, judgment)| {
            judgment
                .outcome()
                .scorecard()
                .filter(|card| check_scorecard(spec, card).is_ok())
        }),
        rule,
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Improvement and greedy selection
// ---------------------------------------------------------------------------------------------------------------

/// Whether `candidate` beats `incumbent` by at least the objective's `min_improvement`.
///
/// With `t = min_improvement.threshold_at(incumbent)` (`|incumbent| * ratio` when relative, the amount when
/// absolute), a maximize objective improves iff `candidate > incumbent` and `candidate >= incumbent + t`, and a
/// minimize objective iff `candidate < incumbent` and `candidate <= incumbent - t`. The threshold is applied by
/// multiplying and comparing, never by dividing by the incumbent, so a zero or negative incumbent is handled
/// exactly: at zero a relative threshold is zero and the candidate must be strictly better. A threshold or bound
/// that is not finite means no improvement, so an incumbent near the largest finite value never panics and never
/// improves by overflow.
#[must_use]
pub fn improves(candidate: Metric, incumbent: Metric, objective: &Objective) -> bool {
    let (candidate, incumbent) = (candidate.value(), incumbent.value());
    let Some(threshold) = objective.min_improvement().threshold_at(incumbent) else {
        return false;
    };
    match objective.goal() {
        Goal::Maximize => {
            let bound = incumbent + threshold;
            bound.is_finite() && candidate > incumbent && candidate >= bound
        }
        Goal::Minimize => {
            let bound = incumbent - threshold;
            bound.is_finite() && candidate < incumbent && candidate <= bound
        }
    }
}

/// The result of greedy selection over one generation's valid candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// This candidate beat the incumbent by at least `min_improvement`.
    Improved(ExperimentId),
    /// No candidate cleared the threshold; the incumbent stays.
    Kept,
}

/// `Greater` when `left` is the better candidate: the better objective value, then the smaller ID.
fn rank(goal: Goal, left: &(ExperimentId, Metric), right: &(ExperimentId, Metric)) -> Ordering {
    let by_value = match goal {
        Goal::Maximize => left.1.cmp(&right.1),
        Goal::Minimize => right.1.cmp(&left.1),
    };
    by_value.then_with(|| right.0.cmp(&left.0))
}

/// Greedy selection: the best candidate by the objective's direction, ties broken on the smallest
/// [`ExperimentId`], is [`Selection::Improved`] when it [`improves`] on `incumbent`; otherwise, and for no
/// candidates, [`Selection::Kept`].
///
/// The best candidate is the maximum of a total order over `(value, ID)` pairs, never the first seen, so the
/// result is identical under any permutation of `candidates` (CORE-10).
#[must_use]
pub fn select_greedy(
    candidates: &[(ExperimentId, Metric)],
    incumbent: Metric,
    objective: &Objective,
) -> Selection {
    let goal = objective.goal();
    match candidates
        .iter()
        .max_by(|left, right| rank(goal, left, right))
    {
        Some((winner, value)) if improves(*value, incumbent, objective) => {
            Selection::Improved(*winner)
        }
        Some(_) | None => Selection::Kept,
    }
}

/// Concludes a generation: the [`Decision`] its proposals and recorded trials give.
///
/// A candidate is a proposal of `generation` whose trials under `judge` all pass [`check_scorecard`] against the
/// manifest's judge ([`TrialRule::AllScored`]) and reduce, by [`reduce_trials`], to a score that has the
/// objective metric. The incumbent is [`RunState::incumbent_before`] the generation, reduced over its scored
/// trials alone ([`TrialRule::ScoredOnly`]), so one failed calibration trial of the baseline does not block the
/// run; the report shows how many of its trials were skipped. No candidate gives
/// [`DecisionKind::NoValidCandidates`]; otherwise [`select_greedy`] gives [`DecisionKind::Improved`] or
/// [`DecisionKind::Kept`] with the incumbent.
/// `proposed` is the number of recorded proposals, and `dropped` the number the caller rejected at parse or
/// compose, which never became experiments.
///
/// `manifest` must be the manifest in effect at `generation`, and `judge` the [`JudgeVersion`] of its judge.
/// The result depends only on the ledger's contents, never on record or trial order.
///
/// # Errors
///
/// [`PolicyError::GenerationZero`] for generation 0, [`PolicyError::IncumbentUnscored`] when the incumbent has
/// no scored trial with the objective metric, and [`PolicyError::TooManyProposals`] when the count does not fit.
pub fn conclude(
    state: &RunState,
    manifest: &Manifest,
    judge: &JudgeVersion,
    generation: u32,
    dropped: u32,
) -> Result<Decision, PolicyError> {
    if generation == 0 {
        return Err(PolicyError::GenerationZero);
    }
    let spec = manifest.judge();
    let objective = manifest.objective();
    let incumbent = state.incumbent_before(generation);
    let incumbent_value = valid_score(state, spec, judge, &incumbent, TrialRule::ScoredOnly)
        .and_then(|score| score.get(objective.metric()))
        .ok_or(PolicyError::IncumbentUnscored { exp: incumbent })?;
    let mut proposed: usize = 0;
    let mut candidates = Vec::new();
    for (exp, _) in state.proposals_in(generation) {
        proposed = proposed
            .checked_add(1)
            .ok_or(PolicyError::TooManyProposals)?;
        if let Some(value) = valid_score(state, spec, judge, exp, TrialRule::AllScored)
            .and_then(|score| score.get(objective.metric()))
        {
            candidates.push((*exp, value));
        }
    }
    let proposed = u32::try_from(proposed).map_err(|_| PolicyError::TooManyProposals)?;
    let kind = if candidates.is_empty() {
        DecisionKind::NoValidCandidates
    } else {
        match select_greedy(&candidates, incumbent_value, objective) {
            Selection::Improved(winner) => DecisionKind::Improved { winner },
            Selection::Kept => DecisionKind::Kept { incumbent },
        }
    };
    Ok(Decision::new(kind, proposed, dropped))
}

// ---------------------------------------------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------------------------------------------

/// The spread of the baseline's repeated measurements of one metric under one judge (D-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseFloor {
    median: Metric,
    min: Metric,
    max: Metric,
    trials: u32,
    skipped: u32,
}

impl NoiseFloor {
    /// The lower median of the trials.
    #[must_use]
    pub const fn median(&self) -> Metric {
        self.median
    }

    /// The smallest trial value.
    #[must_use]
    pub const fn min(&self) -> Metric {
        self.min
    }

    /// The largest trial value.
    #[must_use]
    pub const fn max(&self) -> Metric {
        self.max
    }

    /// The number of trials measured.
    #[must_use]
    pub const fn trials(&self) -> u32 {
        self.trials
    }

    /// The number of baseline trials under the judge that did not score and were skipped.
    #[must_use]
    pub const fn skipped(&self) -> u32 {
        self.skipped
    }

    /// `max - min`. Never negative; infinite when the difference overflows.
    #[must_use]
    pub fn spread(&self) -> f64 {
        self.max.value() - self.min.value()
    }
}

/// The noise floor of `metric`, folded from the baseline's [`Outcome::Scored`] trials under `judge` that report
/// it. `None` when there are none. Trials that did not score are skipped and counted, the same
/// [`TrialRule::ScoredOnly`] rule [`conclude`] applies to the incumbent: they measured nothing.
#[must_use]
pub fn noise_floor(
    state: &RunState,
    judge: &JudgeVersion,
    metric: &MetricName,
) -> Option<NoiseFloor> {
    let mut values: Vec<Metric> = Vec::new();
    let mut skipped: u32 = 0;
    for (_, judgment) in state.trials(state.baseline(), judge) {
        match judgment.outcome().scorecard() {
            Some(card) => values.extend(card.value(metric)),
            None => skipped = skipped.checked_add(1)?,
        }
    }
    values.sort_unstable();
    let trials = u32::try_from(values.len()).ok()?;
    Some(NoiseFloor {
        median: lower_median(&values)?,
        min: *values.first()?,
        max: *values.last()?,
        trials,
        skipped,
    })
}

/// Whether an objective's `min_improvement` stands clear of the baseline's noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseVerdict {
    /// The threshold is more than [`NOISE_WARN_MULTIPLE`] times the spread, or the spread is zero.
    Ok,
    /// No floor could be measured (fewer than [`MIN_NOISE_TRIALS`] trials), the threshold is not finite, or the
    /// threshold is at most [`NOISE_WARN_MULTIPLE`] times the spread.
    Warn,
    /// The threshold is at most [`NOISE_REFUSE_MULTIPLE`] times the spread: improvements would be noise.
    Refuse,
}

/// Compares the objective's threshold at the baseline median with the baseline's spread (D-12).
///
/// The verdict is advisory and never changes the manifest: `objective` is read through a shared reference, and
/// selection always applies the manifest's own `min_improvement`. `minos check` prints the verdict, and
/// `minos run` refuses a [`NoiseVerdict::Refuse`] unless the user overrides it (Phase 8).
#[must_use]
pub fn noise_check(floor: Option<&NoiseFloor>, objective: &Objective) -> NoiseVerdict {
    let Some(floor) = floor.filter(|floor| floor.trials() >= MIN_NOISE_TRIALS) else {
        return NoiseVerdict::Warn;
    };
    let spread = floor.spread();
    if spread <= 0.0 {
        return NoiseVerdict::Ok;
    }
    let Some(threshold) = objective
        .min_improvement()
        .threshold_at(floor.median().value())
    else {
        return NoiseVerdict::Warn;
    };
    if threshold <= NOISE_REFUSE_MULTIPLE * spread {
        NoiseVerdict::Refuse
    } else if threshold <= NOISE_WARN_MULTIPLE * spread {
        NoiseVerdict::Warn
    } else {
        NoiseVerdict::Ok
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Circuit breaker
// ---------------------------------------------------------------------------------------------------------------

/// The circuit breaker (CORE-17, D-14): whether the run must stop before its next generation.
///
/// It counts the consecutive [`DecisionKind::NoValidCandidates`] decisions walking down from
/// [`RunState::last_concluded_generation`]; the walk ends at any other decision or at a generation with no
/// decision. At least `max_empty_generations` of them gives [`StopReason::NoValidCandidates`]. Otherwise an
/// `infra_streak` of at least `max_infra_failures` gives [`StopReason::InfraFailures`]. The empty streak is
/// checked first, so when both limits are met the result is always [`StopReason::NoValidCandidates`]. Both limits
/// are at least 1 (`Manifest::try_from`), so the breaker cannot be disabled, and it trips at exactly the limit,
/// never a generation early.
///
/// Infrastructure failures are retried and are not [`Outcome`]s, so the ledger does not record them: the loop
/// supplies the current streak from memory and records a trip as `StopRequested { reason: InfraFailures }`. A
/// restart resets the streak, which is acceptable because a restart is a human action (03-RESEARCH Open
/// Question 8).
#[must_use]
pub fn circuit_breaker(state: &RunState, budget: &Budget, infra_streak: u32) -> Option<StopReason> {
    let limit = budget.max_empty_generations();
    let mut empty: u32 = 0;
    let mut generation = state.last_concluded_generation();
    while generation > 0 && empty < limit {
        match state.decision(generation).map(Decision::kind) {
            Some(DecisionKind::NoValidCandidates) => {
                empty = empty.saturating_add(1);
                generation = generation.saturating_sub(1);
            }
            Some(DecisionKind::Improved { .. } | DecisionKind::Kept { .. }) | None => break,
        }
    }
    if empty >= limit {
        Some(StopReason::NoValidCandidates)
    } else if infra_streak >= budget.max_infra_failures() {
        Some(StopReason::InfraFailures)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Spend check
// ---------------------------------------------------------------------------------------------------------------

/// The result of [`spend_ok`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendCheck {
    /// Spend and tokens are within the manifest's caps.
    Within {
        /// The metered spend so far.
        spent: ApproxCost,
        /// The tokens used so far.
        tokens: u64,
    },
    /// A cap is exceeded: [`StopReason::SpendCap`] or [`StopReason::TokenCap`].
    Stop(StopReason),
}

/// Prices every recorded spend and compares the totals with the manifest's caps (D-09, COST-01).
///
/// Each [`SpendSource::ModelCall`] is priced with its model's rates, and each candidate and judge run with the
/// compute rates alone, through [`Cost::of`], and the costs are summed with checked arithmetic. Spend strictly
/// above [`Budget::spend_cap_micro`] stops with [`StopReason::SpendCap`]; otherwise total tokens strictly above
/// [`Budget::max_tokens`] stop with [`StopReason::TokenCap`], and a token total that overflows counts as above.
/// Dollars are derived here from usage and the user's price table and are never stored. The loop checks this
/// before each generation and each model call (LOOP-07).
///
/// # Errors
///
/// A [`CostError`] when a recorded model has no rates in `prices` (even at zero tokens), or when a cost
/// overflows. The loop stops and reports it: a spend it cannot price is never treated as free.
pub fn spend_ok(
    state: &RunState,
    manifest: &Manifest,
    prices: &Prices,
) -> Result<SpendCheck, CostError> {
    let mut total = Cost::ZERO;
    for (_, spend) in state.spends() {
        let model = match spend.source() {
            SpendSource::ModelCall { model, .. } => Some(model),
            SpendSource::Candidate { .. } | SpendSource::Judge { .. } => None,
        };
        total = total.checked_add(Cost::of(prices, model, spend.usage())?)?;
    }
    let budget = manifest.budget();
    if total.micro_dollars() > budget.spend_cap_micro() {
        return Ok(SpendCheck::Stop(StopReason::SpendCap));
    }
    match state.total_usage().total_tokens() {
        Some(tokens) if tokens <= budget.max_tokens() => Ok(SpendCheck::Within {
            spent: total.approx(),
            tokens,
        }),
        Some(_) | None => Ok(SpendCheck::Stop(StopReason::TokenCap)),
    }
}
