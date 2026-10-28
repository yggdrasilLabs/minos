//! The output helpers every command shares (08-CONTEXT D-10, D-14 to D-17).
//!
//! - Every line a command prints goes through [`write_line`], which passes it through [`SafeText`], so a terminal
//!   escape, a C1 control, or a bidi override never reaches the terminal, whatever its source. Text from a sandbox or
//!   a model has already been sanitized inside the loop; the renderer is applied regardless.
//! - IDs print as [`SHORT_HEX`]-character prefixes unless the user asks for full IDs ([`IdStyle`]), mirroring git's
//!   short hashes.
//! - Outcomes, reasons, stops, and decisions print by their closed-set names, never by free text.
//! - Cost prints only through [`CostLine`], whose dollar figure is written by `ApproxCost` alone (COST-02).
//! - Times are host-measured: a trial's elapsed time is the candidate VM's plus the judge VM's, each recovered from
//!   its recorded vCPU-milliseconds ([`trial_elapsed_ms`]), and dates are UTC.
//!
//! The command modules format through these helpers and add none of their own.

use std::io::{self, Write};

use minos_core::id::{ExperimentId, MetricName};
use minos_core::ledger::{Decision, DecisionKind, SpendSource, StopReason};
use minos_core::manifest::JudgeVersion;
use minos_core::metric::Metric;
use minos_core::money::{CostError, CostLine, Prices};
use minos_core::outcome::{Outcome, Reason};
use minos_core::policy::{TrialRule, reduce_trials, run_cost};
use minos_core::state::RunState;
use minos_core::text::SafeText;
use minos_runner::RUNNER_VCPUS;

/// How many hex characters a short ID prints.
pub const SHORT_HEX: usize = 12;

/// Whether IDs print short or in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdStyle {
    /// The first [`SHORT_HEX`] characters.
    #[default]
    Short,
    /// All 64 characters.
    Full,
}

/// `hex` in `style`: its first [`SHORT_HEX`] characters, or all of it.
#[must_use]
pub fn id_text(hex: &str, style: IdStyle) -> String {
    match style {
        IdStyle::Short => hex.get(..SHORT_HEX).unwrap_or(hex).to_owned(),
        IdStyle::Full => hex.to_owned(),
    }
}

/// The outcome's word: `scored`, `rejected:<reason>`, `crashed:<stage>`, or `timed_out:<stage>`.
#[must_use]
pub fn outcome_word(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Scored(_) => "scored".to_owned(),
        Outcome::Rejected(reason) => format!("rejected:{}", reason_name(reason)),
        Outcome::Crashed { stage } => format!("crashed:{stage}"),
        Outcome::TimedOut { stage } => format!("timed_out:{stage}"),
    }
}

/// The reason's snake-case name, the name its ledger form uses. A variant this build does not know prints as
/// `other`.
#[must_use]
pub const fn reason_name(reason: &Reason) -> &'static str {
    match reason {
        Reason::GateFailed { .. } => "gate_failed",
        Reason::MissingMetric { .. } => "missing_metric",
        Reason::UndeclaredMetric { .. } => "undeclared_metric",
        Reason::HeldOutMismatch { .. } => "held_out_mismatch",
        Reason::ScorecardInvalid => "scorecard_invalid",
        Reason::ApplyFailed { .. } => "apply_failed",
        Reason::BuildFailed => "build_failed",
        Reason::RunFailed => "run_failed",
        Reason::OutputInvalid => "output_invalid",
        Reason::OutputTooLarge => "output_too_large",
        Reason::ChunkProtocol => "chunk_protocol",
        _ => "other",
    }
}

/// The stop reason's snake-case name, the name its ledger form uses. A variant this build does not know prints as
/// `other`.
#[must_use]
pub const fn stop_reason_name(reason: StopReason) -> &'static str {
    match reason {
        StopReason::UserStop => "user_stop",
        StopReason::SpendCap => "spend_cap",
        StopReason::TokenCap => "token_cap",
        StopReason::NoValidCandidates => "no_valid_candidates",
        StopReason::InfraFailures => "infra_failures",
        StopReason::ProviderLimit => "provider_limit",
        StopReason::ProviderAuth => "provider_auth",
        _ => "other",
    }
}

/// The decision's word: `improved`, `kept`, or `no_valid_candidates`.
#[must_use]
pub const fn decision_word(decision: &Decision) -> &'static str {
    match decision.kind() {
        DecisionKind::Improved { .. } => "improved",
        DecisionKind::Kept { .. } => "kept",
        DecisionKind::NoValidCandidates => "no_valid_candidates",
    }
}

/// `delta` as a percentage of the incumbent's magnitude, `delta / |incumbent| * 100`, the formula the report's
/// `SummaryRow::delta_pct` uses, so the percent's sign is the delta's. `None` for a zero incumbent or a result that
/// is not finite. For display only: policy never reads it.
#[must_use]
pub fn percent_of(delta: f64, incumbent: f64) -> Option<f64> {
    if incumbent == 0.0 {
        return None;
    }
    Some(delta / incumbent.abs() * 100.0).filter(|percent| percent.is_finite())
}

/// A percent with its sign and one decimal (`+6.2%`, `-5.0%`), or `n/a`.
#[must_use]
pub fn percent_text(percent: Option<f64>) -> String {
    percent.map_or_else(|| "n/a".to_owned(), |percent| format!("{percent:+.1}%"))
}

/// Milliseconds as whole minutes and seconds, rounded down (`1m42s`, `0m00s`), or `n/a`.
#[must_use]
pub fn elapsed_text(ms: Option<u64>) -> String {
    ms.map_or_else(
        || "n/a".to_owned(),
        |ms| {
            let seconds = ms / 1000;
            format!("{}m{:02}s", seconds / 60, seconds % 60)
        },
    )
}

/// The host-measured wall-clock time of `trial` of `exp`: the candidate VM's recorded vCPU-milliseconds divided by
/// [`RUNNER_VCPUS`], plus the same for the judge VM when it ran. `None` when the candidate's spend is not recorded.
#[must_use]
pub fn trial_elapsed_ms(state: &RunState, exp: &ExperimentId, trial: u32) -> Option<u64> {
    let vcpus = u64::from(RUNNER_VCPUS);
    let mut candidate = None;
    let mut judge = 0_u64;
    for (_, spend) in state.spends() {
        match spend.source() {
            SpendSource::Candidate {
                exp: spent,
                trial: at,
            } if spent == exp && *at == trial => {
                candidate = Some(spend.usage().vcpu_ms() / vcpus);
            }
            SpendSource::Judge {
                exp: spent,
                trial: at,
            } if spent == exp && *at == trial => {
                judge = spend.usage().vcpu_ms() / vcpus;
            }
            _ => {}
        }
    }
    candidate.map(|ms| ms.saturating_add(judge))
}

/// The reduced value of `metric` over the trials of `exp` under `judge`, by the lower median under `rule`: the
/// shape the loop's objective delta uses ([`TrialRule::AllScored`] for a candidate, [`TrialRule::ScoredOnly`] for
/// the incumbent and the baseline).
#[must_use]
pub fn objective_of(
    state: &RunState,
    judge: &JudgeVersion,
    exp: &ExperimentId,
    metric: &MetricName,
    rule: TrialRule,
) -> Option<Metric> {
    reduce_trials(
        state
            .trials(exp, judge)
            .map(|(_, judgment)| judgment.outcome()),
        rule,
    )?
    .get(metric)
}

/// The one judge version the run's baseline was judged under, or `None` when it has none or several.
#[must_use]
pub fn run_judge(state: &RunState) -> Option<JudgeVersion> {
    let versions = state.judge_versions(state.baseline());
    let mut iter = versions.into_iter();
    match (iter.next(), iter.next()) {
        (Some(judge), None) => Some(judge),
        _ => None,
    }
}

/// The generation a running driver is in: 0 while no generation has a decision and generation 1 has no proposal
/// (calibration), else one past the last concluded generation, saturating. `stop` records its stop there, and
/// `status` shows it.
#[must_use]
pub fn working_generation(state: &RunState) -> u32 {
    let calibrating = state.decisions().next().is_none() && state.proposals_in(1).next().is_none();
    if calibrating {
        0
    } else {
        state.last_concluded_generation().saturating_add(1)
    }
}

/// The run's cost line: [`run_cost`] rounded once, beside the run's total token counts.
///
/// # Errors
///
/// [`CostError`] when `prices` cannot price a recorded spend, for example an unpriced model: a run is never priced
/// at zero.
pub fn cost_line(state: &RunState, prices: &Prices) -> Result<CostLine, CostError> {
    Ok(CostLine::new(
        run_cost(state, prices)?.approx(),
        state.total_usage(),
    ))
}

/// The UTC date and time of `unix_seconds`, as `YYYY-MM-DDTHH:MM:SSZ`, by proleptic Gregorian civil-from-days
/// arithmetic in integers only (core has no clock, and the CLI needs no date crate).
#[must_use]
pub fn utc_text(unix_seconds: u64) -> String {
    let days = unix_seconds / 86_400;
    let rest = unix_seconds % 86_400;
    let (hour, minute, second) = (rest / 3600, (rest % 3600) / 60, rest % 60);

    // Days from 0000-03-01, so a leap day is the last day of its year; eras are 400 years of 146,097 days.
    let z = days.saturating_add(719_468);
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era
        .saturating_mul(400)
        .saturating_add(yoe)
        .saturating_add(u64::from(month <= 2));
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Writes `line` through [`SafeText`] and one newline. Control characters (ESC, CR, and the C1 set) are dropped
/// and invisible format characters are shown as `<U+XXXX>`; a newline or tab inside `line` is kept.
///
/// # Errors
///
/// Any error writing to `out`.
pub fn write_line<W: Write>(out: &mut W, line: &str) -> io::Result<()> {
    writeln!(out, "{}", SafeText::new(line))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::str::FromStr;

    use minos_core::id::{ExperimentId, ManifestId, MetricName, RunId, TreeId};
    use minos_core::ledger::{Event, Record};
    use minos_core::metric::{Metric, Scorecard};
    use minos_core::outcome::{Outcome, Reason, Stage};
    use minos_core::state::fold;

    use super::{
        IdStyle, elapsed_text, id_text, outcome_word, percent_of, percent_text, utc_text,
        working_generation, write_line,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn percents_carry_the_delta_sign_and_one_decimal() {
        assert_eq!(percent_text(percent_of(0.62, 10.0)), "+6.2%");
        assert_eq!(percent_text(percent_of(-0.5, -10.0)), "-5.0%");
        assert_eq!(percent_of(1.0, 0.0), None);
        assert_eq!(percent_text(Some(-3.0)), "-3.0%");
        assert_eq!(percent_text(None), "n/a");
    }

    #[test]
    fn elapsed_time_is_whole_minutes_and_seconds() {
        assert_eq!(elapsed_text(Some(102_000)), "1m42s");
        assert_eq!(elapsed_text(Some(999)), "0m00s");
        assert_eq!(elapsed_text(Some(3_600_000)), "60m00s");
        assert_eq!(elapsed_text(None), "n/a");
    }

    #[test]
    fn utc_dates_follow_the_gregorian_calendar() {
        assert_eq!(utc_text(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_text(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_text(1_760_000_000), "2025-10-09T08:53:20Z");
        assert_eq!(utc_text(4_102_444_799), "2099-12-31T23:59:59Z");
        assert_eq!(utc_text(4_107_542_400), "2100-03-01T00:00:00Z");
    }

    #[test]
    fn ids_print_short_or_full() {
        let hex = "0123456789abcdef".repeat(4);
        assert_eq!(id_text(&hex, IdStyle::Short), "0123456789ab");
        assert_eq!(id_text(&hex, IdStyle::Full), hex);
        assert_eq!(id_text("abc", IdStyle::Short), "abc");
    }

    #[test]
    fn outcome_words_name_the_reason_or_the_stage() -> TestResult {
        let gate = Outcome::Rejected(Reason::GateFailed {
            gate: MetricName::from_str("identical_output")?,
        });
        assert_eq!(outcome_word(&gate), "rejected:gate_failed");
        assert_eq!(
            outcome_word(&Outcome::Crashed {
                stage: Stage::Build
            }),
            "crashed:build"
        );
        assert_eq!(
            outcome_word(&Outcome::TimedOut { stage: Stage::Run }),
            "timed_out:run"
        );
        let card =
            Scorecard::from_entries([(MetricName::from_str("x")?, Metric::new(1.0)?, false)])?;
        assert_eq!(outcome_word(&Outcome::Scored(card)), "scored");
        Ok(())
    }

    #[test]
    fn write_line_drops_escapes_and_ends_the_line() -> TestResult {
        let mut out = Vec::new();
        write_line(&mut out, "a\u{1b}[31mb")?;
        assert_eq!(out, b"a[31mb\n");
        Ok(())
    }

    #[test]
    fn a_run_with_only_run_started_is_in_generation_zero() -> TestResult {
        let manifest = ManifestId::of(b"m");
        let base = TreeId::of(b"t");
        let record = Record::v1(Event::RunStarted {
            run: RunId::of(&manifest, &base, 1),
            manifest,
            base,
            baseline: ExperimentId::baseline(&base),
            started_at: 1,
            root_image: None,
        });
        assert_eq!(working_generation(&fold([&record])?), 0);
        Ok(())
    }
}
