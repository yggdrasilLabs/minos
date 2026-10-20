//! The history view: what the prompt's suffix says about earlier experiments (06-CONTEXT D-07, D-08).
//!
//! A [`History`] holds rendered text only, never a scorecard, an outcome, or a metric value, so nothing it
//! carries into a prompt can be a held-out value. Rows read exactly two kinds of metric name from the judge: its
//! gates and each objective's own metric. The held-out metric paired with an objective is never read, and a
//! rejection whose reason names a metric for a scorecard-shape fault renders a fixed phrase without the name.
//!
//! Every string that came from a model or a sandbox is sanitized before it is kept: a rationale with
//! [`SafeText::inline`], so it cannot break its row, and a failure excerpt is a [`LogExcerpt`], which core has
//! already sanitized.

use minos_core::id::ExperimentId;
use minos_core::limits::RATIONALE_EXCERPT_BYTES;
use minos_core::manifest::JudgeSpec;
use minos_core::metric::Metric;
use minos_core::outcome::{Outcome, Reason};
use minos_core::policy::{ReducedScore, TrialRule, reduce_trials};
use minos_core::text::{LogExcerpt, SafeText};

use crate::limits::{EXCERPTS_MAX_BYTES, HISTORY_GENERATIONS, HISTORY_TABLE_MAX_BYTES};

/// Hex characters of an experiment ID shown in a row: enough to tell the rows of one run apart.
const SHORT_ID_CHARS: usize = 12;

/// The phrase for every scorecard-shape rejection. It names no metric, because the metric could be held out.
const SCORECARD_INVALID: &str = "the judge's scorecard was invalid";

/// One recorded experiment, as the loop hands it to the history view.
///
/// The rationale is sanitized and cut when the entry is built; the raw text is never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    generation: u32,
    exp: ExperimentId,
    accepted: bool,
    rationale: SafeText,
    trials: Vec<(u32, Outcome)>,
    excerpt: Option<LogExcerpt>,
}

impl HistoryEntry {
    /// An experiment of `generation`, whether it was accepted as a winner, its rationale, its trials as
    /// `(trial, outcome)` pairs, and optionally the tail of its failed stage's log.
    ///
    /// The rationale is rendered with [`SafeText::inline`] and cut at a char boundary to at most
    /// [`RATIONALE_EXCERPT_BYTES`]. Which log the excerpt comes from is the loop's choice; an entry may carry
    /// none, and only an unscored row's excerpt is ever shown.
    #[must_use]
    pub fn new(
        generation: u32,
        exp: ExperimentId,
        accepted: bool,
        rationale: &str,
        trials: Vec<(u32, Outcome)>,
        excerpt: Option<LogExcerpt>,
    ) -> Self {
        let inline = SafeText::inline(rationale);
        // A cut of sanitized text holds no hidden character, so sanitizing it again leaves it unchanged.
        let rationale = SafeText::inline(cut(inline.as_str(), RATIONALE_EXCERPT_BYTES));
        Self {
            generation,
            exp,
            accepted,
            rationale,
            trials,
            excerpt,
        }
    }

    /// The generation the experiment was proposed in.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// The experiment.
    #[must_use]
    pub const fn exp(&self) -> &ExperimentId {
        &self.exp
    }

    /// Whether the experiment was accepted as a winner.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    /// The sanitized rationale excerpt, at most [`RATIONALE_EXCERPT_BYTES`].
    #[must_use]
    pub const fn rationale(&self) -> &SafeText {
        &self.rationale
    }
}

/// The rendered history: the table's text, the failure excerpts, and the row counts.
///
/// Holds no scorecard, outcome, or metric value (PROP-06).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    table: String,
    excerpts: Vec<(String, String)>,
    rows: usize,
    omitted: usize,
}

/// One rendered row before the caps apply.
struct Row {
    accepted: bool,
    line: String,
    /// The excerpt label and text, for an unscored row that carries one.
    excerpt: Option<(String, String)>,
}

impl History {
    /// A history with no rows, as before the first proposal.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Renders `entries` for a prompt of `generation`, with deltas against the incumbent's trials.
    ///
    /// Entries are ordered by `(generation, experiment)` whatever their input order. An entry is kept when it is
    /// from the last [`HISTORY_GENERATIONS`] generations or was accepted. The incumbent reduces over its scored
    /// trials ([`TrialRule::ScoredOnly`]) and each candidate only when every trial scored
    /// ([`TrialRule::AllScored`]), as `select_greedy` does.
    ///
    /// When the table would exceed [`HISTORY_TABLE_MAX_BYTES`], rows are dropped one at a time, the oldest
    /// non-winner first and then the oldest winner, and the table ends with a line counting them. Excerpts come
    /// from the kept unscored rows that carry one, the newest first while their total stays within
    /// [`EXCERPTS_MAX_BYTES`], and are stored oldest first.
    #[must_use]
    pub fn new(
        judge: &JudgeSpec,
        generation: u32,
        incumbent: &[Outcome],
        mut entries: Vec<HistoryEntry>,
    ) -> Self {
        entries.sort_by_key(|entry| (entry.generation, entry.exp));
        let incumbent = reduce_trials(incumbent, TrialRule::ScoredOnly);
        let mut rows: Vec<Row> = entries
            .iter_mut()
            .filter(|entry| {
                entry.accepted || entry.generation.saturating_add(HISTORY_GENERATIONS) >= generation
            })
            .map(|entry| render_row(judge, incumbent.as_ref(), entry))
            .collect();

        let omitted = fit_table(&mut rows);
        let mut table = String::new();
        for row in &rows {
            table.push_str(&row.line);
            table.push('\n');
        }
        if omitted > 0 {
            table.push_str(&omitted_line(omitted));
        }

        let mut excerpts = Vec::new();
        let mut total: usize = 0;
        for (label, text) in rows.iter().rev().filter_map(|row| row.excerpt.as_ref()) {
            let next = total.saturating_add(text.len());
            if next > EXCERPTS_MAX_BYTES {
                break;
            }
            total = next;
            excerpts.push((label.clone(), text.clone()));
        }
        excerpts.reverse();

        Self {
            table,
            excerpts,
            rows: rows.len(),
            omitted,
        }
    }

    /// The table: one line per row, each ending with a newline, then the omitted-rows line when rows were
    /// dropped. Empty when there are no rows.
    #[must_use]
    pub fn table(&self) -> &str {
        &self.table
    }

    /// The failure excerpts as `(label, sanitized text)`, in `(generation, experiment)` order.
    #[must_use]
    pub fn excerpts(&self) -> &[(String, String)] {
        &self.excerpts
    }

    /// The number of rows in the table.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.rows
    }

    /// The number of rows dropped to keep the table under [`HISTORY_TABLE_MAX_BYTES`].
    #[must_use]
    pub const fn omitted_rows(&self) -> usize {
        self.omitted
    }
}

/// The table's last line when `omitted` rows were dropped.
fn omitted_line(omitted: usize) -> String {
    format!("({omitted} earlier rows omitted)\n")
}

/// Drops rows until the table fits [`HISTORY_TABLE_MAX_BYTES`], the oldest non-winner first, then the oldest
/// winner. Returns how many were dropped.
fn fit_table(rows: &mut Vec<Row>) -> usize {
    let mut size: usize = rows
        .iter()
        .map(|row| row.line.len().saturating_add(1))
        .fold(0, usize::saturating_add);
    let fits = |size: usize, omitted: usize| {
        let footer = if omitted == 0 {
            0
        } else {
            omitted_line(omitted).len()
        };
        size.saturating_add(footer) <= HISTORY_TABLE_MAX_BYTES
    };
    let mut keep = vec![true; rows.len()];
    let mut omitted: usize = 0;
    for winners in [false, true] {
        for (row, kept) in rows.iter().zip(keep.iter_mut()) {
            if fits(size, omitted) {
                break;
            }
            if row.accepted == winners && *kept {
                *kept = false;
                size = size.saturating_sub(row.line.len().saturating_add(1));
                omitted = omitted.saturating_add(1);
            }
        }
    }
    let mut flags = keep.into_iter();
    rows.retain(|_| flags.next().unwrap_or(true));
    omitted
}

/// Renders one entry's row and, when it is unscored and carries one, its excerpt.
fn render_row(
    judge: &JudgeSpec,
    incumbent: Option<&ReducedScore>,
    entry: &mut HistoryEntry,
) -> Row {
    entry.trials.sort_by_key(|(trial, _)| *trial);
    let outcomes = entry.trials.iter().map(|(_, outcome)| outcome);
    let reduced = reduce_trials(outcomes, TrialRule::AllScored);
    let short: String = entry.exp.to_hex().chars().take(SHORT_ID_CHARS).collect();

    let mut line = format!(
        "gen {} | exp {short} | {}",
        entry.generation,
        if entry.accepted {
            "winner"
        } else {
            "candidate"
        }
    );
    let unscored = match (&reduced, first_unscored(&entry.trials)) {
        (_, Some(outcome)) => Some(unscored_text(outcome)),
        (None, None) if entry.trials.is_empty() => Some("not judged".to_owned()),
        (None, None) => Some(format!("rejected: {SCORECARD_INVALID}")),
        (Some(score), None) => {
            line.push_str(" | scored");
            push_scored(&mut line, judge, score, incumbent);
            None
        }
    };
    let excerpt = unscored.map(|text| {
        line.push_str(" | ");
        line.push_str(&text);
        let label = format!("gen {} exp {short} {text}", entry.generation);
        entry
            .excerpt
            .as_ref()
            .map(|excerpt| (label, excerpt.text().as_str().to_owned()))
    });
    line.push_str(" | rationale: ");
    line.push_str(entry.rationale.as_str());
    Row {
        accepted: entry.accepted,
        line,
        excerpt: excerpt.flatten(),
    }
}

/// The first trial, in trial order, that did not score.
fn first_unscored(trials: &[(u32, Outcome)]) -> Option<&Outcome> {
    trials
        .iter()
        .map(|(_, outcome)| outcome)
        .find(|outcome| !outcome.is_scored())
}

/// The gates and the visible-objective deltas of a scored row.
fn push_scored(
    line: &mut String,
    judge: &JudgeSpec,
    score: &ReducedScore,
    incumbent: Option<&ReducedScore>,
) {
    let mut gates = judge.gates().iter().peekable();
    if gates.peek().is_some() {
        line.push_str(" | gates: ");
        let mut first = true;
        for gate in gates {
            if !first {
                line.push_str(", ");
            }
            first = false;
            line.push_str(gate.as_str());
            line.push_str(if score.get(gate) == Some(Metric::ONE) {
                " pass"
            } else {
                " fail"
            });
        }
    }
    for objective in judge.objectives() {
        let metric = objective.metric();
        line.push_str(" | ");
        line.push_str(metric.as_str());
        line.push(' ');
        let current = score.get(metric);
        let base = incumbent.and_then(|incumbent| incumbent.get(metric));
        line.push_str(&delta(current, base));
    }
}

/// The signed percent change from `incumbent` to `candidate`, or `n/a` when it is not defined.
fn delta(candidate: Option<Metric>, incumbent: Option<Metric>) -> String {
    let (Some(candidate), Some(incumbent)) = (candidate, incumbent) else {
        return "n/a".to_owned();
    };
    let base = incumbent.value();
    if base == 0.0 {
        return "n/a".to_owned();
    }
    let percent = (candidate.value() - base) / base.abs() * 100.0;
    if percent.is_finite() {
        format!("{percent:+.2}%")
    } else {
        "n/a".to_owned()
    }
}

/// The text of an outcome that did not score.
fn unscored_text(outcome: &Outcome) -> String {
    let text = match outcome {
        Outcome::Crashed { stage } => format!("crashed in {stage}"),
        Outcome::TimedOut { stage } => format!("timed out in {stage}"),
        Outcome::Rejected(reason) => match reason_phrase(reason) {
            Some(phrase) => format!("rejected: {phrase}"),
            None => "rejected".to_owned(),
        },
        Outcome::Scored(_) => "scored".to_owned(),
    };
    // Every payload is a validated name, path, or number already; sanitizing again is a backstop.
    SafeText::inline(&text).as_str().to_owned()
}

/// The fixed phrase for a rejection reason, or `None` for a reason this crate does not know yet.
fn reason_phrase(reason: &Reason) -> Option<String> {
    match reason {
        // A gate is never a held-out metric: the manifest refuses that pairing.
        Reason::GateFailed { .. }
        | Reason::ApplyFailed { .. }
        | Reason::BuildFailed
        | Reason::RunFailed
        | Reason::OutputInvalid
        | Reason::OutputTooLarge
        | Reason::ChunkProtocol => Some(reason.to_string()),
        // These may name a held-out metric, so the name is never rendered.
        Reason::MissingMetric { .. }
        | Reason::UndeclaredMetric { .. }
        | Reason::HeldOutMismatch { .. }
        | Reason::ScorecardInvalid => Some(SCORECARD_INVALID.to_owned()),
        _ => None,
    }
}

/// `text` cut to at most `max` bytes at a char boundary.
pub(super) fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let end = (0..=max)
        .rev()
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(0);
    text.get(..end).unwrap_or_default()
}
