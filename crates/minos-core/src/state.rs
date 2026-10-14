//! The fold from ledger records to [`RunState`]: same events, same state, under any replay (CORE-09).
//!
//! Run state is never stored. [`fold`] rebuilds it from the ledger's records every time, and nothing else in
//! core holds run state, so resume (LOOP-05), policy, routing, the circuit breaker, and the report all read the
//! same value for the same ledger. [`RunState`] holds only `BTreeMap`, `BTreeSet`, and values derived from them,
//! so its contents and its iteration order depend on the records' contents and never on the order they were
//! appended in.
//!
//! A ledger that fails validation is corrupt, and [`FoldError`] is an infrastructure fault: the driver stops and
//! reports it. It is never turned into an [`Outcome`], because no candidate is to blame for it.

use std::collections::{BTreeMap, BTreeSet};

use crate::id::{BlobId, ExperimentId, ManifestId, ReportHash, RunId, TreeId};
use crate::ledger::{
    BackendInfo, Decision, DecisionKind, Event, EventKey, Origin, Record, ReviewAnswer, ReviewId,
    ReviewKind, SpendId, SpendSource, StopReason,
};
use crate::manifest::{JudgeVersion, Tier};
use crate::outcome::Outcome;
use crate::usage::Usage;

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// Why a ledger does not fold: it is corrupt.
///
/// Ledger corruption is an infrastructure fault. The driver stops and reports it; it is never retried as if a
/// candidate had failed, and it is never recorded as an [`Outcome`]. Every variant carries IDs only, never text
/// from a record.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FoldError {
    /// The ledger has no records.
    #[error("the ledger is empty")]
    Empty,
    /// The first record is not `RunStarted`.
    #[error("the first ledger record is not RunStarted")]
    FirstNotRunStarted,
    /// A second `RunStarted` differs from the first.
    #[error("the ledger has more than one RunStarted record")]
    DuplicateRunStarted,
    /// `RunStarted` names a run ID that its manifest, base, and start time do not hash to.
    #[error("RunStarted records run {recorded}, but its fields give run {computed}")]
    RunIdMismatch {
        /// The run ID in the record.
        recorded: RunId,
        /// `RunId::of(manifest, base, started_at)`.
        computed: RunId,
    },
    /// `RunStarted` names a baseline that its base tree does not hash to.
    #[error("RunStarted records baseline {recorded}, but its base tree gives {computed}")]
    BaselineMismatch {
        /// The baseline in the record.
        recorded: ExperimentId,
        /// `ExperimentId::baseline(base)`.
        computed: ExperimentId,
    },
    /// Two different events share one key: a record was rewritten.
    #[error("two different records share key {key}")]
    KeyConflict {
        /// The shared key.
        key: EventKey,
    },
    /// A record refers to an experiment that is neither the baseline nor proposed.
    #[error("a record refers to unknown experiment {exp}")]
    UnknownExperiment {
        /// The unknown experiment.
        exp: ExperimentId,
    },
    /// A proposal's parent is unknown or not from an earlier generation, or the proposal claims the baseline,
    /// which has no parent.
    #[error("experiment {exp} has an invalid parent {parent}")]
    BadParent {
        /// The proposed experiment.
        exp: ExperimentId,
        /// Its recorded parent.
        parent: ExperimentId,
    },
    /// A proposal at generation 0, which is baseline calibration only.
    #[error("experiment {exp} is proposed at generation 0")]
    ProposedAtGenerationZero {
        /// The proposed experiment.
        exp: ExperimentId,
    },
    /// A sweep-tier proposal with a model origin, or a model-tier proposal with a sweep origin.
    #[error("experiment {exp} has an origin that disagrees with its tier")]
    OriginTierMismatch {
        /// The proposed experiment.
        exp: ExperimentId,
    },
    /// A decision at generation 0, a winner not proposed in the decision's generation, or a kept incumbent that
    /// is not the incumbent before that generation.
    #[error("the decision for generation {generation} is inconsistent with the ledger")]
    BadDecision {
        /// The decision's generation.
        generation: u32,
    },
    /// An answer to a review that was never requested.
    #[error("review {request} is answered but was never requested")]
    AnswerWithoutRequest {
        /// The answered request.
        request: ReviewId,
    },
    /// Two different manifests take effect from one generation, or a generation-0 manifest differs from the one
    /// `RunStarted` names.
    #[error("two different manifests take effect from generation {generation}")]
    ManifestConflict {
        /// The generation.
        generation: u32,
    },
    /// The recorded usage does not fit in 64-bit counters.
    #[error("the recorded usage overflows")]
    UsageOverflow,
}

// ---------------------------------------------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------------------------------------------

/// One recorded proposal: an experiment, where it came from, and the blobs that describe it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    parent: ExperimentId,
    generation: u32,
    tier: Tier,
    diff: BlobId,
    rationale: BlobId,
    origin: Origin,
}

impl Proposal {
    /// The experiment it was proposed from: the baseline or an experiment of an earlier generation.
    #[must_use]
    pub const fn parent(&self) -> &ExperimentId {
        &self.parent
    }

    /// The generation that proposed it, from 1.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// The tier that proposed it.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// The blob of the canonical cumulative diff from the base tree.
    #[must_use]
    pub const fn diff(&self) -> &BlobId {
        &self.diff
    }

    /// The blob of the proposer's rationale.
    #[must_use]
    pub const fn rationale(&self) -> &BlobId {
        &self.rationale
    }

    /// The sweep or the model call that produced it.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }
}

/// How one trial of an experiment under one judge version ended, and where it ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgment {
    outcome: Outcome,
    backend: BackendInfo,
}

impl Judgment {
    /// How the trial ended.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Where and how the VMs ran.
    #[must_use]
    pub const fn backend(&self) -> &BackendInfo {
        &self.backend
    }
}

/// One metered step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spend {
    generation: u32,
    source: SpendSource,
    usage: Usage,
}

impl Spend {
    /// The generation it belongs to; 0 for baseline calibration.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// What was metered.
    #[must_use]
    pub const fn source(&self) -> &SpendSource {
        &self.source
    }

    /// The metered quantities.
    #[must_use]
    pub const fn usage(&self) -> &Usage {
        &self.usage
    }
}

/// A review request and, once the user has answered, the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Review {
    kind: ReviewKind,
    answer: Option<ReviewAnswer>,
}

impl Review {
    /// What the request asks the user to approve.
    #[must_use]
    pub const fn kind(&self) -> ReviewKind {
        self.kind
    }

    /// The user's answer, or `None` while the request waits in the inbox.
    #[must_use]
    pub const fn answer(&self) -> Option<ReviewAnswer> {
        self.answer
    }
}

/// The state of a run, rebuilt from its ledger by [`fold`] and never stored.
///
/// Every collection is a `BTreeMap` or `BTreeSet`, so every accessor iterates in key order, whatever order the
/// records were appended in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    run: RunId,
    initial_manifest: ManifestId,
    manifests: BTreeMap<u32, ManifestId>,
    base: TreeId,
    baseline: ExperimentId,
    started_at: u64,
    proposals: BTreeMap<ExperimentId, Proposal>,
    judgments: BTreeMap<(ExperimentId, JudgeVersion), BTreeMap<u32, Judgment>>,
    decisions: BTreeMap<u32, Decision>,
    spends: BTreeMap<SpendId, Spend>,
    total_usage: Usage,
    notes: BTreeMap<(u32, u32), BlobId>,
    stops: BTreeSet<(u32, StopReason)>,
    reviews: BTreeMap<ReviewId, Review>,
    acknowledged: BTreeSet<(ExperimentId, ReportHash)>,
}

// ---------------------------------------------------------------------------------------------------------------
// The fold
// ---------------------------------------------------------------------------------------------------------------

/// Rebuilds a run's state from its ledger records.
///
/// The fold has two phases, and that shape is what makes the result independent of record order:
///
/// 1. The first record must be `RunStarted`. Its `run` and `baseline` are recomputed from its other fields. Then
///    every record, the first included, is inserted into a map under [`Event::key`]. An event equal to the one
///    already under its key is a duplicate and is skipped, so replaying a ledger, or appending a resumed step
///    twice, changes nothing. A different event under an existing key is a rewritten record.
/// 2. Only after every record is in, the state is built by iterating that map, whose order is the key order and
///    never the input order, and every cross-reference is checked against the complete state.
///
/// So any permutation of the records after `RunStarted`, with or without duplicates, gives an equal state or the
/// same kind of error.
///
/// # Errors
///
/// A [`FoldError`] when the ledger is corrupt: empty, not starting with a single consistent `RunStarted`, a key
/// shared by two different events, or a dangling or inconsistent cross-reference. Corruption is an infrastructure
/// fault, never an [`Outcome`].
pub fn fold<'a>(records: impl IntoIterator<Item = &'a Record>) -> Result<RunState, FoldError> {
    let mut records = records.into_iter();
    let first = records.next().ok_or(FoldError::Empty)?.event();
    let Event::RunStarted {
        run,
        manifest,
        base,
        baseline,
        started_at,
    } = first
    else {
        return Err(FoldError::FirstNotRunStarted);
    };

    let computed_run = RunId::of(manifest, base, *started_at);
    if *run != computed_run {
        return Err(FoldError::RunIdMismatch {
            recorded: *run,
            computed: computed_run,
        });
    }
    let computed = ExperimentId::baseline(base);
    if *baseline != computed {
        return Err(FoldError::BaselineMismatch {
            recorded: *baseline,
            computed,
        });
    }

    // Phase 1: key every record.
    let mut keyed: BTreeMap<EventKey, &Event> = BTreeMap::new();
    for event in std::iter::once(first).chain(records.map(Record::event)) {
        let key = event.key(&computed_run);
        match keyed.get(&key) {
            None => {
                keyed.insert(key, event);
            }
            Some(existing) if *existing == event => {}
            Some(_) if matches!(event, Event::RunStarted { .. }) => {
                return Err(FoldError::DuplicateRunStarted);
            }
            Some(_) => return Err(FoldError::KeyConflict { key }),
        }
    }

    // Phase 2: build from the key-ordered map, then check cross-references against the complete state.
    let mut state = RunState {
        run: *run,
        initial_manifest: *manifest,
        manifests: BTreeMap::new(),
        base: *base,
        baseline: *baseline,
        started_at: *started_at,
        proposals: BTreeMap::new(),
        judgments: BTreeMap::new(),
        decisions: BTreeMap::new(),
        spends: BTreeMap::new(),
        total_usage: Usage::default(),
        notes: BTreeMap::new(),
        stops: BTreeSet::new(),
        reviews: BTreeMap::new(),
        acknowledged: BTreeSet::new(),
    };
    let mut answers: BTreeMap<ReviewId, ReviewAnswer> = BTreeMap::new();
    for event in keyed.into_values() {
        state.insert(event, &mut answers)?;
    }
    for (request, answer) in answers {
        let review = state
            .reviews
            .get_mut(&request)
            .ok_or(FoldError::AnswerWithoutRequest { request })?;
        review.answer = Some(answer);
    }
    state.validate()?;
    Ok(state)
}

impl RunState {
    /// Places one keyed event into the state. Keys are unique here, so each collection receives each of its
    /// entries once; only the manifest schedule can still collide, because its key includes the manifest.
    fn insert(
        &mut self,
        event: &Event,
        answers: &mut BTreeMap<ReviewId, ReviewAnswer>,
    ) -> Result<(), FoldError> {
        match event {
            Event::RunStarted { .. } => {}
            Event::ManifestChanged {
                manifest,
                from_generation,
            } => {
                let clashes = (*from_generation == 0 && *manifest != self.initial_manifest)
                    || self
                        .manifests
                        .get(from_generation)
                        .is_some_and(|existing| existing != manifest);
                if clashes {
                    return Err(FoldError::ManifestConflict {
                        generation: *from_generation,
                    });
                }
                self.manifests.insert(*from_generation, *manifest);
            }
            Event::Proposed {
                exp,
                parent,
                generation,
                tier,
                diff,
                rationale,
                origin,
            } => {
                self.proposals.insert(
                    *exp,
                    Proposal {
                        parent: *parent,
                        generation: *generation,
                        tier: *tier,
                        diff: *diff,
                        rationale: *rationale,
                        origin: origin.clone(),
                    },
                );
            }
            Event::Judged {
                exp,
                judge,
                trial,
                outcome,
                backend,
            } => {
                self.judgments.entry((*exp, *judge)).or_default().insert(
                    *trial,
                    Judgment {
                        outcome: outcome.clone(),
                        backend: backend.clone(),
                    },
                );
            }
            Event::Selected {
                generation,
                decision,
            } => {
                self.decisions.insert(*generation, *decision);
            }
            Event::Spent {
                generation,
                source,
                usage,
            } => {
                self.total_usage = self
                    .total_usage
                    .checked_add(usage)
                    .ok_or(FoldError::UsageOverflow)?;
                self.spends.insert(
                    SpendId::of(&self.run, *generation, source),
                    Spend {
                        generation: *generation,
                        source: source.clone(),
                        usage: *usage,
                    },
                );
            }
            Event::Steered {
                generation,
                ordinal,
                note,
            } => {
                self.notes.insert((*generation, *ordinal), *note);
            }
            Event::StopRequested { generation, reason } => {
                self.stops.insert((*generation, *reason));
            }
            Event::ReviewRequested { request, kind } => {
                self.reviews.insert(
                    *request,
                    Review {
                        kind: *kind,
                        answer: None,
                    },
                );
            }
            Event::Reviewed { request, answer } => {
                answers.insert(*request, *answer);
            }
            Event::ReportAcknowledged { exp, report } => {
                self.acknowledged.insert((*exp, *report));
            }
        }
        Ok(())
    }

    /// Checks every cross-reference against the complete state, in key order.
    fn validate(&self) -> Result<(), FoldError> {
        for (exp, proposal) in &self.proposals {
            if proposal.generation == 0 {
                return Err(FoldError::ProposedAtGenerationZero { exp: *exp });
            }
            let sweep_tier = proposal.tier == Tier::Sweep;
            let sweep_origin = matches!(proposal.origin, Origin::Sweep);
            if sweep_tier != sweep_origin {
                return Err(FoldError::OriginTierMismatch { exp: *exp });
            }
            let parent_ok = *exp != self.baseline
                && (proposal.parent == self.baseline
                    || self
                        .proposals
                        .get(&proposal.parent)
                        .is_some_and(|parent| parent.generation < proposal.generation));
            if !parent_ok {
                return Err(FoldError::BadParent {
                    exp: *exp,
                    parent: proposal.parent,
                });
            }
        }
        for (exp, _) in self.judgments.keys() {
            self.known(exp)?;
        }
        for (&generation, decision) in &self.decisions {
            let consistent = generation != 0
                && match decision.kind() {
                    DecisionKind::Improved { winner } => self
                        .proposals
                        .get(winner)
                        .is_some_and(|proposal| proposal.generation == generation),
                    DecisionKind::Kept { incumbent } => {
                        *incumbent == self.incumbent_before(generation)
                    }
                    DecisionKind::NoValidCandidates => true,
                };
            if !consistent {
                return Err(FoldError::BadDecision { generation });
            }
        }
        for spend in self.spends.values() {
            if let Some(exp) = spend.source.exp() {
                self.known(exp)?;
            }
        }
        for (exp, _) in &self.acknowledged {
            self.known(exp)?;
        }
        Ok(())
    }

    /// `Ok` when `exp` is the baseline or a proposed experiment.
    fn known(&self, exp: &ExperimentId) -> Result<(), FoldError> {
        if *exp == self.baseline || self.proposals.contains_key(exp) {
            Ok(())
        } else {
            Err(FoldError::UnknownExperiment { exp: *exp })
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------------------------------------------

    /// The run.
    #[must_use]
    pub const fn run(&self) -> &RunId {
        &self.run
    }

    /// The manifest in effect now: the one from the latest `ManifestChanged`, else the one the run started with.
    #[must_use]
    pub fn manifest(&self) -> &ManifestId {
        self.manifests
            .last_key_value()
            .map_or(&self.initial_manifest, |(_, manifest)| manifest)
    }

    /// The manifest in effect at `generation`: the latest change from that generation or earlier, else the one
    /// the run started with.
    #[must_use]
    pub fn manifest_at(&self, generation: u32) -> &ManifestId {
        self.manifests
            .range(..=generation)
            .next_back()
            .map_or(&self.initial_manifest, |(_, manifest)| manifest)
    }

    /// The base source tree.
    #[must_use]
    pub const fn base(&self) -> &TreeId {
        &self.base
    }

    /// The baseline experiment: the base tree with no diff.
    #[must_use]
    pub const fn baseline(&self) -> &ExperimentId {
        &self.baseline
    }

    /// The start time in Unix seconds.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// The proposal of `exp`, or `None` for the baseline and for an unknown experiment.
    #[must_use]
    pub fn proposal(&self, exp: &ExperimentId) -> Option<&Proposal> {
        self.proposals.get(exp)
    }

    /// The proposals of `generation`, in [`ExperimentId`] order.
    pub fn proposals_in(
        &self,
        generation: u32,
    ) -> impl Iterator<Item = (&ExperimentId, &Proposal)> {
        self.proposals
            .iter()
            .filter(move |(_, proposal)| proposal.generation == generation)
    }

    /// The trials of `exp` under `judge`, in trial order. Empty when there are none.
    pub fn trials(
        &self,
        exp: &ExperimentId,
        judge: &JudgeVersion,
    ) -> impl Iterator<Item = (u32, &Judgment)> {
        self.judgments
            .get(&(*exp, *judge))
            .into_iter()
            .flat_map(|trials| trials.iter().map(|(trial, judgment)| (*trial, judgment)))
    }

    /// The decision of `generation`, if it concluded.
    #[must_use]
    pub fn decision(&self, generation: u32) -> Option<&Decision> {
        self.decisions.get(&generation)
    }

    /// Every decision, in generation order.
    pub fn decisions(&self) -> impl Iterator<Item = (u32, &Decision)> {
        self.decisions
            .iter()
            .map(|(generation, decision)| (*generation, decision))
    }

    /// The highest generation with a decision, or 0 when none has concluded.
    #[must_use]
    pub fn last_concluded_generation(&self) -> u32 {
        self.decisions
            .last_key_value()
            .map_or(0, |(generation, _)| *generation)
    }

    /// The incumbent that `generation` competes against: the winner of the latest improvement at a lower
    /// generation, else the baseline.
    #[must_use]
    pub fn incumbent_before(&self, generation: u32) -> ExperimentId {
        latest_winner(self.decisions.range(..generation)).unwrap_or(self.baseline)
    }

    /// The incumbent now: the winner of the latest improvement, else the baseline.
    #[must_use]
    pub fn incumbent(&self) -> ExperimentId {
        latest_winner(self.decisions.iter()).unwrap_or(self.baseline)
    }

    /// The steering notes for `generation`, in ordinal order (03-CONTEXT D-07).
    #[must_use]
    pub fn notes(&self, generation: u32) -> Vec<BlobId> {
        self.notes
            .range((generation, 0)..=(generation, u32::MAX))
            .map(|(_, note)| *note)
            .collect()
    }

    /// The ordinal for the next note of `generation`: one past the highest recorded, or 0 when there is none.
    /// `None` when the highest recorded ordinal is `u32::MAX`, so no further note can be keyed apart from it.
    #[must_use]
    pub fn next_note_ordinal(&self, generation: u32) -> Option<u32> {
        self.notes
            .range((generation, 0)..=(generation, u32::MAX))
            .next_back()
            .map_or(Some(0), |((_, ordinal), _)| ordinal.checked_add(1))
    }

    /// The earliest stop request, by generation and then reason, if any.
    #[must_use]
    pub fn stop_requested(&self) -> Option<(u32, StopReason)> {
        self.stops.first().copied()
    }

    /// Every metered step, in [`SpendId`] order.
    pub fn spends(&self) -> impl Iterator<Item = (&SpendId, &Spend)> {
        self.spends.iter()
    }

    /// The sum of every recorded usage.
    #[must_use]
    pub const fn total_usage(&self) -> &Usage {
        &self.total_usage
    }

    /// Every review request with its answer, in [`ReviewId`] order.
    pub fn reviews(&self) -> impl Iterator<Item = (&ReviewId, &Review)> {
        self.reviews.iter()
    }

    /// Whether the user acknowledged `report` for `exp`.
    #[must_use]
    pub fn is_acknowledged(&self, exp: &ExperimentId, report: &ReportHash) -> bool {
        self.acknowledged.contains(&(*exp, *report))
    }

    /// The chain of experiments from the baseline to `exp`, baseline first and `exp` last, or `None` for an
    /// unknown experiment.
    ///
    /// The fold has checked that every parent is the baseline or a proposal of a strictly earlier generation, so
    /// the walk ends at the baseline; it is still bounded by the number of proposals.
    #[must_use]
    pub fn lineage(&self, exp: &ExperimentId) -> Option<Vec<ExperimentId>> {
        let mut chain = vec![*exp];
        let mut current = *exp;
        for _ in 0..=self.proposals.len() {
            if current == self.baseline {
                chain.reverse();
                return Some(chain);
            }
            current = *self.proposals.get(&current)?.parent();
            chain.push(current);
        }
        None
    }
}

/// The winner of the last improvement among `decisions`, walking back from the highest generation.
fn latest_winner<'a>(
    decisions: impl DoubleEndedIterator<Item = (&'a u32, &'a Decision)>,
) -> Option<ExperimentId> {
    decisions
        .rev()
        .find_map(|(_, decision)| match decision.kind() {
            DecisionKind::Improved { winner } => Some(*winner),
            DecisionKind::Kept { .. } | DecisionKind::NoValidCandidates => None,
        })
}
