//! `build_report` and its flags: a pure function of ledger and blobs, so the same inputs give the same report and
//! the report can be hashed (CORE-16; 03-CONTEXT D-15 to D-18).
//!
//! The inputs are a [`ReportInput`]: the folded [`RunState`], the manifest, the judge version, the experiment
//! under review, the fetched blobs (diffs and rationales), and the base surface files as a [`Tree`]. Judge data is
//! never an input, so nothing the judge keeps private can reach the report. Nothing here reads a clock, draws a
//! random number, or depends on the order records were appended in: every collection is ordered by key.
//!
//! [`build_report`] renders the review artifact as text and hashes exactly those bytes with [`ReportHash::of`]:
//! the user acknowledges what they read, and `minos adopt` (Phase 8) rebuilds the report from the same ledger and
//! blobs and refuses unless the hash matches the acknowledged one. The text has a fixed section order (header,
//! summary, flags, provenance, lineage, diff), LF line endings, and every string that originated in a sandbox or a
//! model (rationales, diff lines, literals) passed through [`SafeText`]. Numbers are printed by this module's own
//! formatter, never by a serializer, so a dependency upgrade cannot change a report's bytes.
//!
//! The summary has one [`SummaryRow`] per objective, gate, and held-out metric (D-17). The flags are pure functions
//! of those rows, the ledger, the target's diff, and the surface tree. Flags direct attention; they are not
//! verdicts. A report with no flag is not a claim that the change is safe, and a flag is not a claim that it is
//! malicious.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::diff::apply::Tree;
use crate::diff::{Diff, HunkLine};
use crate::id::{BlobId, ExperimentId, ImageDigest, MetricName, PromptHash, ReportHash};
use crate::ledger::{BackendKind, ConfinementLayer, EXPECTED_KRUN_LAYERS, Isolation, Origin};
use crate::limits::{
    ANOMALY_MULTIPLE, ENCODED_RUN_MIN, LITERAL_MIN_BYTES, Limits, MIN_NOISE_TRIALS,
    NOISE_WARN_MULTIPLE, RATIONALE_EXCERPT_BYTES,
};
use crate::manifest::{Arch, JudgeVersion, Manifest, Objective};
use crate::metric::Metric;
use crate::path::RelPath;
use crate::policy::{NoiseFloor, ReducedScore, improves, noise_floor, reduce_trials};
use crate::state::{Proposal, RunState};
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
    /// `manifest` is the manifest in effect for the run. `blobs` must hold the target's diff blob and the rationale
    /// blob of every proposal in its lineage, keyed by [`BlobId::of`] their bytes. `surface` holds the base tree's
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
    lineage: Vec<ExperimentId>,
    diff: Diff,
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
        lineage,
        diff,
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

// ---------------------------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------------------------

/// The review artifact: its text, the hash of exactly that text, and the flags and summary it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    text: String,
    hash: ReportHash,
    flags: Vec<Flag>,
    summary: Vec<SummaryRow>,
}

impl Report {
    /// The rendered text: what the user reads, LF only, ending with a newline.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// [`ReportHash::of`] the text's bytes: what the user acknowledges and `minos adopt` requires.
    #[must_use]
    pub const fn hash(&self) -> &ReportHash {
        &self.hash
    }

    /// The flags, sorted.
    #[must_use]
    pub fn flags(&self) -> &[Flag] {
        &self.flags
    }

    /// The summary rows, in report order.
    #[must_use]
    pub fn summary(&self) -> &[SummaryRow] {
        &self.summary
    }
}

/// A metric value as the report prints it: `0` for zero, Rust's shortest round-trip `{}` form for magnitudes in
/// `[1e-4, 1e15)`, and its `{:e}` form otherwise. Both forms are defined by std and independent of the platform,
/// so a value always prints the same bytes.
fn format_metric(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if (1e-4..1e15).contains(&value.abs()) {
        format!("{value}")
    } else {
        format!("{value:e}")
    }
}

/// A signed delta: a leading `+` when positive, then [`format_metric`] and `suffix`; `n/a` when absent.
fn format_delta(delta: Option<f64>, suffix: &str) -> String {
    match delta {
        Some(delta) if delta > 0.0 => format!("+{}{suffix}", format_metric(delta)),
        Some(delta) => format!("{}{suffix}", format_metric(delta)),
        None => "n/a".to_owned(),
    }
}

/// `yes`, `no`, or `n/a`.
const fn format_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "n/a",
    }
}

/// `items` joined with `, `, or `none`.
fn join_or_none(items: impl IntoIterator<Item = String>) -> String {
    let items: Vec<String> = items.into_iter().collect();
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

const fn backend_text(backend: BackendKind) -> &'static str {
    match backend {
        BackendKind::Krun => "krun",
    }
}

const fn arch_text(arch: Arch) -> &'static str {
    match arch {
        Arch::Arm64 => "arm64",
        Arch::X86_64 => "x86_64",
    }
}

const fn layer_text(layer: ConfinementLayer) -> &'static str {
    match layer {
        ConfinementLayer::Seatbelt => "seatbelt",
        ConfinementLayer::DedicatedUser => "dedicated_user",
        ConfinementLayer::Rlimits => "rlimits",
    }
}

/// The confinement layers a backend is expected to have.
const fn expected_layers(backend: BackendKind) -> &'static [ConfinementLayer] {
    match backend {
        BackendKind::Krun => &EXPECTED_KRUN_LAYERS,
    }
}

/// `text` cut to at most `max` bytes at a char boundary, and whether anything was cut.
fn excerpt(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let end = (0..=max)
        .rev()
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(0);
    (text.get(..end).unwrap_or_default(), true)
}

/// Appends one formatted line and its LF.
fn put(out: &mut String, line: fmt::Arguments<'_>) -> Result<(), ReportError> {
    fmt::Write::write_fmt(out, line).map_err(|_| ReportError::Format)?;
    out.push('\n');
    Ok(())
}

/// The summary section (D-17).
fn render_summary(
    out: &mut String,
    input: &ReportInput<'_>,
    rows: &[SummaryRow],
) -> Result<(), ReportError> {
    put(out, format_args!("summary"))?;
    let objectives = input.manifest.judge().objectives();
    for row in rows {
        let threshold = match objectives
            .iter()
            .find(|objective| objective.metric() == row.metric())
            .filter(|_| row.role() == MetricRole::Objective)
        {
            Some(objective) => format!(
                ", clears min_improvement {}: {}",
                objective.min_improvement(),
                format_bool(row.clears_threshold())
            ),
            None => String::new(),
        };
        put(
            out,
            format_args!(
                "  {} {}: baseline {} ({} to {}, {} trials), winner {} ({} trials), delta {} ({}){threshold}, \
                 clears noise: {}",
                row.role(),
                row.metric(),
                format_metric(row.baseline_median().value()),
                format_metric(row.baseline_min().value()),
                format_metric(row.baseline_max().value()),
                row.baseline_trials(),
                format_metric(row.winner_median().value()),
                row.winner_trials(),
                format_delta(row.delta_abs(), ""),
                format_delta(row.delta_pct(), "%"),
                format_bool(row.clears_noise()),
            ),
        )?;
    }
    Ok(())
}

/// The provenance section (D-18): isolation and reduced hardening across the lineage, the judge, the backends
/// and images, and the models and prompts.
fn render_provenance(
    out: &mut String,
    input: &ReportInput<'_>,
    lineage: &[ExperimentId],
) -> Result<(), ReportError> {
    let state = input.state;
    let mut backends: BTreeSet<(BackendKind, Arch)> = BTreeSet::new();
    let mut roots: BTreeSet<ImageDigest> = BTreeSet::new();
    let mut kernels: BTreeSet<ImageDigest> = BTreeSet::new();
    let mut missing: BTreeSet<ConfinementLayer> = BTreeSet::new();
    for exp in lineage {
        for (_, judgment) in state.trials(exp, input.judge) {
            let backend = judgment.backend();
            backends.insert((backend.backend(), backend.arch()));
            roots.insert(*backend.root_image());
            kernels.extend(backend.kernel().copied());
            missing.extend(
                expected_layers(backend.backend())
                    .iter()
                    .filter(|layer| !backend.layers().contains(layer))
                    .copied(),
            );
        }
    }
    let mut models: BTreeSet<String> = BTreeSet::new();
    let mut prompts: BTreeSet<PromptHash> = BTreeSet::new();
    for exp in lineage {
        match state.proposal(exp).map(Proposal::origin) {
            Some(Origin::Sweep) => {
                models.insert("sweep".to_owned());
            }
            Some(Origin::Model { model, prompt }) => {
                models.insert(SafeText::inline(model.as_str()).as_str().to_owned());
                prompts.insert(*prompt);
            }
            None => {}
        }
    }
    let isolation = weakest_isolation(input, lineage).map_or("none recorded", isolation_text);
    put(out, format_args!("provenance"))?;
    put(out, format_args!("  isolation: {isolation}"))?;
    put(
        out,
        format_args!(
            "  reduced hardening: {}",
            join_or_none(missing.iter().map(|layer| layer_text(*layer).to_owned()))
        ),
    )?;
    put(out, format_args!("  judge: {}", input.judge))?;
    put(
        out,
        format_args!(
            "  backends: {}",
            join_or_none(backends.iter().map(|(backend, arch)| format!(
                "{} {}",
                backend_text(*backend),
                arch_text(*arch)
            )))
        ),
    )?;
    put(
        out,
        format_args!(
            "  root images: {}",
            join_or_none(roots.iter().map(ToString::to_string))
        ),
    )?;
    put(
        out,
        format_args!(
            "  kernels: {}",
            join_or_none(kernels.iter().map(ToString::to_string))
        ),
    )?;
    put(out, format_args!("  models: {}", join_or_none(models)))?;
    put(
        out,
        format_args!(
            "  prompts: {}",
            join_or_none(prompts.iter().map(ToString::to_string))
        ),
    )?;
    Ok(())
}

/// The lineage section: the baseline, then each proposal with its generation, tier, ID, and rationale excerpt.
fn render_lineage(
    out: &mut String,
    input: &ReportInput<'_>,
    lineage: &[ExperimentId],
) -> Result<(), ReportError> {
    put(out, format_args!("lineage"))?;
    for exp in lineage {
        let Some(proposal) = input.state.proposal(exp) else {
            put(out, format_args!("  baseline {exp}"))?;
            continue;
        };
        let rationale = text_blob(input, proposal.rationale())?;
        let (shown, truncated) = excerpt(rationale, RATIONALE_EXCERPT_BYTES);
        put(
            out,
            format_args!(
                "  generation {} {} {exp}: {}{}",
                proposal.generation(),
                proposal.tier(),
                SafeText::inline(shown),
                if truncated { " [truncated]" } else { "" },
            ),
        )?;
    }
    Ok(())
}

/// Builds the review artifact for `input` (CORE-16).
///
/// The text is, in order: the header (report format, run, experiment, judge version, base tree); the summary
/// rows (D-17); the flags, or `none`; the provenance (D-18); the lineage from the baseline to the target; and the
/// target's cumulative canonical diff from the base, re-parsed with [`Diff::parse`] against the manifest's
/// writable set and checked to hash to the target. Sections are separated by one empty line. The result is a pure
/// function of `input`: the same ledger, in any record order, and the same blobs give the same bytes and the same
/// [`ReportHash`].
///
/// # Errors
///
/// A [`ReportError`] when the target is the baseline or unknown, a blob is missing, tampered, or not text, the
/// diff is invalid for the run, or the target or baseline has no complete score under the judge.
pub fn build_report(input: &ReportInput<'_>) -> Result<Report, ReportError> {
    let analysis = analyze(input)?;
    let state = input.state;
    let mut out = String::new();

    put(&mut out, format_args!("minos report v1"))?;
    put(&mut out, format_args!("  run: {}", state.run()))?;
    put(&mut out, format_args!("  experiment: {}", input.target))?;
    put(&mut out, format_args!("  judge: {}", input.judge))?;
    put(&mut out, format_args!("  base: {}", state.base()))?;
    out.push('\n');

    render_summary(&mut out, input, &analysis.rows)?;
    out.push('\n');

    put(&mut out, format_args!("flags"))?;
    if analysis.flags.is_empty() {
        put(&mut out, format_args!("  none"))?;
    }
    for flag in &analysis.flags {
        put(
            &mut out,
            format_args!("  {}", SafeText::inline(&flag.to_string())),
        )?;
    }
    out.push('\n');

    render_provenance(&mut out, input, &analysis.lineage)?;
    out.push('\n');

    render_lineage(&mut out, input, &analysis.lineage)?;
    out.push('\n');

    put(&mut out, format_args!("diff"))?;
    for line in analysis.diff.as_str().split_terminator('\n') {
        put(&mut out, format_args!("{}", SafeText::new(line)))?;
    }

    let hash = ReportHash::of(out.as_bytes());
    Ok(Report {
        text: out,
        hash,
        flags: analysis.flags,
        summary: analysis.rows,
    })
}

#[cfg(test)]
mod tests {
    use super::format_metric;

    #[test]
    fn format_metric_uses_plain_form_in_range() {
        assert_eq!(format_metric(0.5), "0.5");
        assert_eq!(format_metric(12.25), "12.25");
        assert_eq!(format_metric(-120.0), "-120");
        assert_eq!(format_metric(1e-4), "0.0001");
    }

    #[test]
    fn format_metric_uses_exponent_form_outside_range() {
        assert_eq!(format_metric(1e-7), "1e-7");
        assert_eq!(format_metric(2e20), "2e20");
        assert_eq!(format_metric(1e15), "1e15");
        assert_eq!(format_metric(-3.5e-9), "-3.5e-9");
    }

    #[test]
    fn format_metric_prints_zero_as_zero() {
        assert_eq!(format_metric(0.0), "0");
        assert_eq!(format_metric(-0.0), "0");
    }
}
