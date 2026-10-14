//! `build_report` and its flags: a pure function of ledger and blobs, so the same inputs give the same report and
//! the report can be hashed (CORE-16; 03-CONTEXT D-15 to D-18).
//!
//! The inputs are a [`ReportInput`]: the folded [`RunState`], the manifest, the judge version, the experiment
//! under review, the fetched blobs (diffs and rationales), and the base surface files as a [`Tree`]. Judge data is
//! never an input, so nothing the judge keeps private can reach the report. Nothing here reads a clock, draws a
//! random number, or depends on the order records were appended in: every collection is ordered by key.
//!
//! The summary has one [`SummaryRow`] per objective, gate, and held-out metric (D-17). The flags are pure functions
//! of those rows, the ledger, the target's diff, and the surface tree. Flags direct attention; they are not
//! verdicts. A report with no flag is not a claim that the change is safe, and a flag is not a claim that it is
//! malicious.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::diff::apply::Tree;
use crate::diff::{Diff, HunkLine};
use crate::id::{BlobId, ExperimentId, MetricName};
use crate::ledger::Isolation;
use crate::limits::{
    ANOMALY_MULTIPLE, ENCODED_RUN_MIN, LITERAL_MIN_BYTES, Limits, MIN_NOISE_TRIALS,
    NOISE_WARN_MULTIPLE,
};
use crate::manifest::{JudgeVersion, Manifest, Objective};
use crate::metric::Metric;
use crate::path::RelPath;
use crate::policy::{NoiseFloor, ReducedScore, improves, noise_floor, reduce_trials};
use crate::state::RunState;
use crate::text::{SafeText, is_invisible_format};

/// File names of dependency manifests and lockfiles. A diff that touches a file with one of these names, matched
/// against [`RelPath::file_name`], raises [`Flag::NewDependency`].
pub const DEPENDENCY_FILE_NAMES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "requirements.txt",
    "pyproject.toml",
    "poetry.lock",
    "uv.lock",
    "Pipfile",
    "Pipfile.lock",
    "setup.py",
    "setup.cfg",
    "environment.yml",
    "go.mod",
    "go.sum",
    "Gemfile",
    "Gemfile.lock",
    "build.gradle",
    "build.gradle.kts",
    "pom.xml",
    "composer.json",
    "composer.lock",
    "mix.exs",
    "mix.lock",
    "Package.swift",
    "Package.resolved",
    "flake.nix",
    "flake.lock",
];

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// Why a report could not be built. Every variant carries IDs only, never text from a blob.
///
/// None of these is a candidate fault: each means the caller asked about the wrong experiment, or the ledger or
/// blob store does not hold what the report needs. The CLI reports the error and shows nothing, because a partial
/// report could be acknowledged as if it were complete.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReportError {
    /// The experiment is neither the baseline nor a recorded proposal.
    #[error("experiment {exp} is not in the ledger")]
    NotInLedger {
        /// The experiment asked about.
        exp: ExperimentId,
    },
    /// The baseline has no diff, so there is nothing to review or adopt.
    #[error("the baseline has no diff to review")]
    Baseline,
    /// A blob the report needs is absent, or its bytes do not hash to its ID.
    #[error("blob {blob} is missing or does not match its ID")]
    MissingBlob {
        /// The blob.
        blob: BlobId,
    },
    /// A diff or rationale blob is not UTF-8 text.
    #[error("blob {blob} is not UTF-8 text")]
    NotText {
        /// The blob.
        blob: BlobId,
    },
    /// The experiment's diff blob does not parse against the manifest's writable set, or does not hash to the
    /// experiment with the run's base tree.
    #[error("the diff of experiment {exp} is not valid for this run")]
    InvalidDiff {
        /// The experiment.
        exp: ExperimentId,
    },
    /// The experiment has no complete score under the judge: no trial, a trial that did not score, or a declared
    /// metric missing from a trial.
    #[error("experiment {exp} has no complete score under this judge")]
    Unscored {
        /// The experiment: the target or the baseline.
        exp: ExperimentId,
    },
    /// Writing the report text failed. Writing to a `String` cannot fail, so this is never expected.
    #[error("the report text could not be written")]
    Format,
}

// ---------------------------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------------------------

/// Everything a report is a function of. Judge data is not here, and cannot be added without changing this type.
#[derive(Clone, Copy)]
pub struct ReportInput<'a> {
    state: &'a RunState,
    manifest: &'a Manifest,
    judge: &'a JudgeVersion,
    target: &'a ExperimentId,
    blobs: &'a BTreeMap<BlobId, Vec<u8>>,
    surface: &'a Tree,
}

impl<'a> ReportInput<'a> {
    /// The report inputs for `target` under `judge`.
    ///
    /// `manifest` is the manifest in effect for the run. `blobs` must hold the diff and rationale blob of every
    /// proposal in the target's lineage, keyed by [`BlobId::of`] their bytes. `surface` holds the base tree's
    /// `[surface]` files (readable and writable), which the literal-match flag searches (D-15).
    #[must_use]
    pub const fn new(
        state: &'a RunState,
        manifest: &'a Manifest,
        judge: &'a JudgeVersion,
        target: &'a ExperimentId,
        blobs: &'a BTreeMap<BlobId, Vec<u8>>,
        surface: &'a Tree,
    ) -> Self {
        Self {
            state,
            manifest,
            judge,
            target,
            blobs,
            surface,
        }
    }
}

impl fmt::Debug for ReportInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportInput")
            .field("target", self.target)
            .field("judge", self.judge)
            .field("blobs", &self.blobs.len())
            .field("surface", self.surface)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Summary rows (D-17)
// ---------------------------------------------------------------------------------------------------------------

/// What a summary row's metric is to the judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricRole {
    /// An objective the policy optimizes.
    Objective,
    /// A gate that must equal 1.
    Gate,
    /// A held-out metric paired with an objective, measured on data the proposer never sees.
    HeldOut,
}

impl fmt::Display for MetricRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Objective => "objective",
            Self::Gate => "gate",
            Self::HeldOut => "held-out",
        })
    }
}

/// One metric of the summary: the baseline against the experiment under review (D-17).
///
/// The baseline side is the noise floor of the baseline's trials; the winner side is the lower median of the
/// target's trials. The delta is `winner - baseline`, in the metric's own direction.
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryRow {
    metric: MetricName,
    role: MetricRole,
    baseline_median: Metric,
    baseline_min: Metric,
    baseline_max: Metric,
    baseline_trials: u32,
    winner_median: Metric,
    winner_trials: u32,
    delta_abs: Option<f64>,
    delta_pct: Option<f64>,
    clears_threshold: Option<bool>,
    clears_noise: Option<bool>,
}

impl SummaryRow {
    /// The metric.
    #[must_use]
    pub const fn metric(&self) -> &MetricName {
        &self.metric
    }

    /// What the metric is to the judge.
    #[must_use]
    pub const fn role(&self) -> MetricRole {
        self.role
    }

    /// The lower median of the baseline's trials.
    #[must_use]
    pub const fn baseline_median(&self) -> Metric {
        self.baseline_median
    }

    /// The smallest baseline trial.
    #[must_use]
    pub const fn baseline_min(&self) -> Metric {
        self.baseline_min
    }

    /// The largest baseline trial.
    #[must_use]
    pub const fn baseline_max(&self) -> Metric {
        self.baseline_max
    }

    /// The number of baseline trials that reported the metric.
    #[must_use]
    pub const fn baseline_trials(&self) -> u32 {
        self.baseline_trials
    }

    /// The lower median of the target's trials.
    #[must_use]
    pub const fn winner_median(&self) -> Metric {
        self.winner_median
    }

    /// The number of the target's trials.
    #[must_use]
    pub const fn winner_trials(&self) -> u32 {
        self.winner_trials
    }

    /// `winner - baseline`, or `None` when the difference is not finite.
    #[must_use]
    pub const fn delta_abs(&self) -> Option<f64> {
        self.delta_abs
    }

    /// The delta as a percentage of the baseline median's magnitude, for display only. `None` when the baseline
    /// median is zero or the result is not finite.
    #[must_use]
    pub const fn delta_pct(&self) -> Option<f64> {
        self.delta_pct
    }

    /// For an objective row, whether the winner beats the baseline by at least `min_improvement`
    /// ([`improves`]); `None` for gate and held-out rows.
    #[must_use]
    pub const fn clears_threshold(&self) -> Option<bool> {
        self.clears_threshold
    }

    /// Whether the delta's magnitude is above [`NOISE_WARN_MULTIPLE`] times the baseline spread. `None` when
    /// fewer than [`MIN_NOISE_TRIALS`] baseline trials exist or the spread is zero, so there is no noise to
    /// compare against.
    #[must_use]
    pub const fn clears_noise(&self) -> Option<bool> {
        self.clears_noise
    }
}

/// A usable spread: at least [`MIN_NOISE_TRIALS`] trials and a finite spread above zero.
fn measured_spread(floor: &NoiseFloor) -> Option<f64> {
    let spread = floor.spread();
    (floor.trials() >= MIN_NOISE_TRIALS && spread > 0.0 && spread.is_finite()).then_some(spread)
}

/// The summary row of `metric`, against the baseline's noise floor and the target's reduced `winner`.
fn summary_row(
    state: &RunState,
    judge: &JudgeVersion,
    target: ExperimentId,
    winner: &ReducedScore,
    metric: &MetricName,
    role: MetricRole,
    objective: Option<&Objective>,
) -> Result<SummaryRow, ReportError> {
    let floor = noise_floor(state, judge, metric).ok_or(ReportError::Unscored {
        exp: *state.baseline(),
    })?;
    let winner_median = winner
        .get(metric)
        .ok_or(ReportError::Unscored { exp: target })?;
    let base = floor.median().value();
    let delta = winner_median.value() - base;
    let delta_abs = delta.is_finite().then_some(delta);
    let delta_pct = delta_abs
        .filter(|_| base != 0.0)
        .map(|delta| delta / base.abs() * 100.0)
        .filter(|percent| percent.is_finite());
    let clears_threshold =
        objective.map(|objective| improves(winner_median, floor.median(), objective));
    let clears_noise = measured_spread(&floor)
        .and_then(|spread| delta_abs.map(|delta| delta.abs() > NOISE_WARN_MULTIPLE * spread));
    Ok(SummaryRow {
        metric: metric.clone(),
        role,
        baseline_median: floor.median(),
        baseline_min: floor.min(),
        baseline_max: floor.max(),
        baseline_trials: floor.trials(),
        winner_median,
        winner_trials: winner.trials(),
        delta_abs,
        delta_pct,
        clears_threshold,
        clears_noise,
    })
}

/// The reduced score of `exp` under `judge`, or `None` when it has no complete score.
fn score(state: &RunState, judge: &JudgeVersion, exp: &ExperimentId) -> Option<ReducedScore> {
    reduce_trials(
        state
            .trials(exp, judge)
            .map(|(_, judgment)| judgment.outcome()),
    )
}

/// The rows in report order: objectives in metric order, then gates, then held-out metrics, each sorted.
fn summary_rows(input: &ReportInput<'_>) -> Result<Vec<SummaryRow>, ReportError> {
    let state = input.state;
    let target = *input.target;
    let winner = score(state, input.judge, &target).ok_or(ReportError::Unscored { exp: target })?;
    let spec = input.manifest.judge();
    let mut rows = Vec::new();
    for objective in spec.objectives() {
        rows.push(summary_row(
            state,
            input.judge,
            target,
            &winner,
            objective.metric(),
            MetricRole::Objective,
            Some(objective),
        )?);
    }
    for gate in spec.gates() {
        rows.push(summary_row(
            state,
            input.judge,
            target,
            &winner,
            gate,
            MetricRole::Gate,
            None,
        )?);
    }
    for held_out in spec.held_out_metrics() {
        rows.push(summary_row(
            state,
            input.judge,
            target,
            &winner,
            &held_out,
            MetricRole::HeldOut,
            None,
        )?);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------------------------------------------

/// Something in the experiment that deserves the reviewer's attention. Flags direct attention; they are not
/// verdicts.
///
/// Flags are ordered by variant and then by field, and a report lists them in that order.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    /// The step from the parent to the target is more than [`ANOMALY_MULTIPLE`] times the baseline spread, or,
    /// with no measured spread, that multiple of the largest earlier step in the lineage (D-16).
    AnomalousJump {
        /// The objective metric.
        metric: MetricName,
    },
    /// The visible objective improved over the baseline while its held-out pair did not (D-17).
    HeldOutDivergence {
        /// The visible objective metric.
        metric: MetricName,
    },
    /// An added literal also appears in a readable or writable base surface file, outside the hunk's own old-side
    /// lines (D-15).
    LiteralMatch {
        /// The file the literal was added to.
        path: RelPath,
        /// The 1-based new-side line it was added on.
        line: u32,
        /// The literal, sanitized for one line.
        literal: SafeText,
    },
    /// The diff touches a dependency manifest or lockfile ([`DEPENDENCY_FILE_NAMES`]).
    NewDependency {
        /// The file.
        path: RelPath,
    },
    /// An added line holds a run of at least [`ENCODED_RUN_MIN`] base64 or hex characters.
    EncodedBlob {
        /// The file.
        path: RelPath,
        /// The 1-based new-side line.
        line: u32,
    },
    /// An added line holds a bidi control, zero-width, or tag character ([`is_invisible_format`]).
    HiddenCharacters {
        /// The file.
        path: RelPath,
        /// The 1-based new-side line.
        line: u32,
    },
    /// Some trial in the lineage ran with isolation weaker than hardware virtualization (D-18).
    WeakIsolation {
        /// The weakest level seen.
        level: Isolation,
    },
}

/// The report's text for an isolation level.
const fn isolation_text(level: Isolation) -> &'static str {
    match level {
        Isolation::WeakProcess => "weak (process only)",
        Isolation::Hardware => "hardware virtualization",
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnomalousJump { metric } => write!(
                f,
                "anomalous jump: the step in {metric} from the parent is large next to the baseline noise or \
                 earlier steps"
            ),
            Self::HeldOutDivergence { metric } => write!(
                f,
                "held-out divergence: {metric} improved over the baseline but its held-out pair did not"
            ),
            Self::LiteralMatch {
                path,
                line,
                literal,
            } => write!(
                f,
                "literal match: {path}:{line} adds {literal}, which also appears in a visible surface file"
            ),
            Self::NewDependency { path } => write!(
                f,
                "new dependency: {path} is a dependency manifest or lockfile"
            ),
            Self::EncodedBlob { path, line } => write!(
                f,
                "encoded blob: {path}:{line} adds a long base64 or hex run"
            ),
            Self::HiddenCharacters { path, line } => write!(
                f,
                "hidden characters: {path}:{line} adds an invisible or bidi character"
            ),
            Self::WeakIsolation { level } => write!(
                f,
                "weak isolation: a trial in the lineage ran with {} isolation",
                isolation_text(*level)
            ),
        }
    }
}

/// One added line of the target's diff, with where it sits.
struct Added<'a> {
    path: &'a RelPath,
    /// 1-based new-side line number.
    line: u32,
    text: &'a str,
    /// The hunk's old side: its first line and its length.
    old_start: u32,
    old_len: u32,
}

/// Every inserted line of `diff`, in file and hunk order, numbered from each hunk's new-side start.
fn added_lines(diff: &Diff) -> Vec<Added<'_>> {
    let mut added = Vec::new();
    for file in diff.files() {
        for hunk in file.hunks() {
            let mut line = hunk.new_start();
            for body in hunk.lines() {
                match body {
                    HunkLine::Context(_) => line = line.saturating_add(1),
                    HunkLine::Insert(_) => {
                        added.push(Added {
                            path: file.path(),
                            line,
                            text: body.text(),
                            old_start: hunk.old_start(),
                            old_len: hunk.old_len(),
                        });
                        line = line.saturating_add(1);
                    }
                    HunkLine::Delete(_) => {}
                }
            }
        }
    }
    added
}

/// The literals in one line: the contents of double- and single-quoted strings, and runs of ASCII digits with at
/// most one `.`, each at least [`LITERAL_MIN_BYTES`] long.
fn literals(line: &str) -> BTreeSet<&str> {
    let mut found = BTreeSet::new();
    let mut chars = line.char_indices();
    while let Some((start, quote)) = chars.next() {
        if quote != '"' && quote != '\'' {
            continue;
        }
        let mut escaped = false;
        let mut end = None;
        for (index, c) in chars.by_ref() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                end = Some(index);
                break;
            }
        }
        if let Some(inner) = end.and_then(|end| line.get(start + 1..end))
            && inner.len() >= LITERAL_MIN_BYTES
        {
            found.insert(inner);
        }
    }
    let mut run: Option<(usize, bool)> = None;
    for (index, c) in line
        .char_indices()
        .chain(std::iter::once((line.len(), ' ')))
    {
        match run {
            None if c.is_ascii_digit() => run = Some((index, false)),
            None => {}
            Some(_) if c.is_ascii_digit() => {}
            Some((start, false)) if c == '.' => run = Some((start, true)),
            Some((start, _)) => {
                if let Some(digits) = line.get(start..index).map(|run| run.trim_end_matches('.'))
                    && digits.len() >= LITERAL_MIN_BYTES
                {
                    found.insert(digits);
                }
                run = None;
            }
        }
    }
    found
}

/// Whether `literal` appears on a line of a visible surface file, other than the lines of `added`'s own hunk on
/// its old side in its own file.
fn appears_outside(visible: &[(&RelPath, &str)], added: &Added<'_>, literal: &str) -> bool {
    let old_start = u64::from(added.old_start);
    let old_end = old_start + u64::from(added.old_len);
    visible.iter().any(|(path, text)| {
        (1_u64..).zip(text.split('\n')).any(|(number, line)| {
            let own_hunk = *path == added.path && (old_start..old_end).contains(&number);
            !own_hunk && line.contains(literal)
        })
    })
}

/// [`Flag::LiteralMatch`]: one flag per (path, literal), at the first line that adds it (D-15).
fn literal_flags(input: &ReportInput<'_>, added: &[Added<'_>], flags: &mut BTreeSet<Flag>) {
    let surface = input.manifest.surface();
    let visible: Vec<(&RelPath, &str)> = input
        .surface
        .iter()
        .filter(|(path, _)| surface.contains(*path))
        .filter_map(|(path, bytes)| std::str::from_utf8(bytes).ok().map(|text| (path, text)))
        .collect();
    let mut seen: BTreeSet<(&RelPath, &str)> = BTreeSet::new();
    for line in added {
        for literal in literals(line.text) {
            if !seen.contains(&(line.path, literal)) && appears_outside(&visible, line, literal) {
                seen.insert((line.path, literal));
                flags.insert(Flag::LiteralMatch {
                    path: line.path.clone(),
                    line: line.line,
                    literal: SafeText::inline(literal),
                });
            }
        }
    }
}

/// Whether `c` can be part of a base64, base64url, or hex encoding.
const fn is_encoding_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
}

/// Whether `text` holds a run of at least [`ENCODED_RUN_MIN`] encoding characters.
fn has_encoded_run(text: &str) -> bool {
    let mut run: usize = 0;
    for c in text.chars() {
        if is_encoding_char(c) {
            run = run.saturating_add(1);
            if run >= ENCODED_RUN_MIN {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// [`Flag::AnomalousJump`] for one objective (D-16).
fn anomaly_flag(
    input: &ReportInput<'_>,
    lineage: &[ExperimentId],
    objective: &Objective,
    flags: &mut BTreeSet<Flag>,
) {
    let state = input.state;
    let metric = objective.metric();
    let goal = objective.goal();
    let value = |exp: &ExperimentId| {
        score(state, input.judge, exp)
            .and_then(|score| score.get(metric))
            .map(Metric::value)
    };
    let step = |pair: &[ExperimentId]| match pair {
        [from, to] => {
            Some(goal.oriented(value(to)? - value(from)?)).filter(|step| step.is_finite())
        }
        _ => None,
    };
    let steps: Vec<&[ExperimentId]> = lineage.windows(2).collect();
    let Some((last, earlier)) = steps.split_last() else {
        return;
    };
    let Some(jump) = step(last) else {
        return;
    };
    let spread = noise_floor(state, input.judge, metric)
        .as_ref()
        .and_then(measured_spread);
    // With no measured spread, the yardstick is the largest earlier positive step in the lineage.
    let Some(yardstick) = spread.or_else(|| {
        earlier
            .iter()
            .filter_map(|pair| step(pair))
            .filter(|step| *step > 0.0)
            .max_by(f64::total_cmp)
    }) else {
        return;
    };
    let bound = ANOMALY_MULTIPLE * yardstick;
    if bound.is_finite() && jump > bound {
        flags.insert(Flag::AnomalousJump {
            metric: metric.clone(),
        });
    }
}

/// [`Flag::HeldOutDivergence`] for one objective, read from the summary rows (D-17).
fn divergence_flag(rows: &[SummaryRow], objective: &Objective, flags: &mut BTreeSet<Flag>) {
    let Some(held_out) = objective.held_out() else {
        return;
    };
    let delta = |metric: &MetricName| {
        rows.iter()
            .find(|row| row.metric() == metric)
            .and_then(SummaryRow::delta_abs)
            .map(|delta| objective.goal().oriented(delta))
    };
    if let (Some(visible), Some(hidden)) = (delta(objective.metric()), delta(held_out))
        && visible > 0.0
        && hidden <= 0.0
    {
        flags.insert(Flag::HeldOutDivergence {
            metric: objective.metric().clone(),
        });
    }
}

/// The weakest isolation level over every trial of every lineage experiment under the judge, or `None` when the
/// lineage has no trial.
fn weakest_isolation(input: &ReportInput<'_>, lineage: &[ExperimentId]) -> Option<Isolation> {
    lineage
        .iter()
        .flat_map(|exp| input.state.trials(exp, input.judge))
        .map(|(_, judgment)| judgment.backend().isolation())
        .min()
}

// ---------------------------------------------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------------------------------------------

/// What the flags and the rendering share: computed once per report.
struct Analysis {
    rows: Vec<SummaryRow>,
    flags: Vec<Flag>,
}

/// The bytes of `blob`, checked against its ID.
fn blob<'a>(input: &ReportInput<'a>, blob: &BlobId) -> Result<&'a [u8], ReportError> {
    input
        .blobs
        .get(blob)
        .map(Vec::as_slice)
        .filter(|bytes| BlobId::of(bytes) == *blob)
        .ok_or(ReportError::MissingBlob { blob: *blob })
}

/// The text of `blob`, which must be UTF-8.
fn text_blob<'a>(input: &ReportInput<'a>, id: &BlobId) -> Result<&'a str, ReportError> {
    std::str::from_utf8(blob(input, id)?).map_err(|_| ReportError::NotText { blob: *id })
}

/// Checks the target, re-parses its diff, and computes the summary rows and the flags.
fn analyze(input: &ReportInput<'_>) -> Result<Analysis, ReportError> {
    let state = input.state;
    let target = *input.target;
    if target == *state.baseline() {
        return Err(ReportError::Baseline);
    }
    let proposal = state
        .proposal(&target)
        .ok_or(ReportError::NotInLedger { exp: target })?;
    let lineage = state
        .lineage(&target)
        .ok_or(ReportError::NotInLedger { exp: target })?;
    let text = text_blob(input, proposal.diff())?;
    let diff = Diff::parse(text, input.manifest.writable(), &Limits::standard())
        .map_err(|_| ReportError::InvalidDiff { exp: target })?;
    if ExperimentId::of(state.base(), &diff) != target {
        return Err(ReportError::InvalidDiff { exp: target });
    }
    let rows = summary_rows(input)?;

    let mut flags = BTreeSet::new();
    for objective in input.manifest.judge().objectives() {
        anomaly_flag(input, &lineage, objective, &mut flags);
        divergence_flag(&rows, objective, &mut flags);
    }
    let added = added_lines(&diff);
    literal_flags(input, &added, &mut flags);
    for file in diff.files() {
        if DEPENDENCY_FILE_NAMES.contains(&file.path().file_name()) {
            flags.insert(Flag::NewDependency {
                path: file.path().clone(),
            });
        }
    }
    for line in &added {
        if has_encoded_run(line.text) {
            flags.insert(Flag::EncodedBlob {
                path: line.path.clone(),
                line: line.line,
            });
        }
        if line.text.chars().any(is_invisible_format) {
            flags.insert(Flag::HiddenCharacters {
                path: line.path.clone(),
                line: line.line,
            });
        }
    }
    if let Some(level @ Isolation::WeakProcess) = weakest_isolation(input, &lineage) {
        flags.insert(Flag::WeakIsolation { level });
    }

    Ok(Analysis {
        rows,
        flags: flags.into_iter().collect(),
    })
}

/// The flags the report on `input` raises, sorted.
///
/// # Errors
///
/// A [`ReportError`] when the target is the baseline or unknown, a lineage blob is missing or not text, the diff
/// is invalid for the run, or the target or baseline has no complete score under the judge.
pub fn flags(input: &ReportInput<'_>) -> Result<Vec<Flag>, ReportError> {
    analyze(input).map(|analysis| analysis.flags)
}

/// The summary rows of the report on `input`: objectives, then gates, then held-out metrics, each in metric
/// order (D-17).
///
/// # Errors
///
/// As for [`flags`].
pub fn summary(input: &ReportInput<'_>) -> Result<Vec<SummaryRow>, ReportError> {
    analyze(input).map(|analysis| analysis.rows)
}
