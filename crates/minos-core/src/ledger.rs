//! The frozen ledger wire format: the eleven-variant [`Event`] set, the versioned [`Record`], the JSONL [`Line`],
//! and one content-derived [`EventKey`] per event (CORE-08).
//!
//! The ledger is append-only JSON Lines, one [`Line`] `{ seq, key, record }` per line. Run state is never stored:
//! it is only ever rebuilt by folding the records (`crate::state`), so the same lines always give the same state.
//! The store (Phase 4) calls [`Line::verify`] on every line it reads, and treats an append whose key already
//! exists as a no-op (STORE-03), so a resumed step that re-appends an event changes nothing.
//!
//! # Keys
//!
//! [`Event::key`] is a pure function of the event and the run. Every key starts
//! `IdHasher::new("minos/event/v1").digest(run).u64(tag)`, then hashes the event's key fields:
//!
//! | Event | tag | then hashed | Why these fields |
//! |---|---|---|---|
//! | `RunStarted` | 1 | nothing | One per run; the run digest is already hashed |
//! | `ManifestChanged` | 2 | `digest(manifest)`, `u64(from_generation)` | `from_generation`, so a change A to B to A is three records, not one |
//! | `Proposed` | 3 | `digest(exp)` | A re-proposal of the same experiment is the same record. The other fields are not in the key, so the loop must check `RunState::proposal(&exp).is_none()` before appending and drop a repeat at admission; a second proposal with different fields fails the fold with `FoldError::DuplicateProposal` |
//! | `Judged` | 4 | `digest(exp)`, `digest(judge)`, `u64(trial)` | `trial`, so repeated noise-floor measurements never collapse; `judge`, so re-scoring appends |
//! | `Selected` | 5 | `u64(generation)` | One decision per generation |
//! | `Spent` | 6 | `digest(SpendId)` | Through [`SpendId`], so a resumed model call with a new index is neither double-counted nor dropped |
//! | `Steered` | 7 | `u64(generation)`, `u64(ordinal)` | `ordinal`, so identical notes for one generation stay distinct. The note is not in the key, so `minos steer` must read `RunState::next_note_ordinal` and append under the ledger lock; two different notes at one ordinal fail the fold with `FoldError::NoteConflict` |
//! | `StopRequested` | 8 | `u64(generation)`, `u64(reason tag)` | `generation`, so a repeated stop is a no-op but a resumed run can stop again later |
//! | `ReviewRequested` | 9 | `digest(request)` | Its own tag, so a request and its answer never share a key |
//! | `Reviewed` | 10 | `digest(request)` | Its own tag, distinct from `ReviewRequested` |
//! | `ReportAcknowledged` | 11 | `digest(exp)`, `digest(report)` | One acknowledgement per experiment and report |
//!
//! [`SpendId`] is `IdHasher::new("minos/spend/v1").digest(run).u64(generation)`, then for
//! [`SpendSource::ModelCall`] `u64(1).u64(call).bytes(model)`, for [`SpendSource::Candidate`]
//! `u64(2).digest(exp).u64(trial)`, and for [`SpendSource::Judge`] `u64(3).digest(exp).u64(trial)`.
//!
//! [`ReviewId`] is `IdHasher::new("minos/review/v1").digest(run).u64(kind tag).digest(subject)`.
//!
//! [`StopReason`] tags: `UserStop` 1, `SpendCap` 2, `TokenCap` 3, `NoValidCandidates` 4, `InfraFailures` 5,
//! `ProviderLimit` 6. [`ReviewKind`] tags: `JudgeDesign` 1, `Params` 2, `Dependencies` 3. Tags are explicit match
//! arms, never enum discriminant casts, so reordering a declaration cannot change a key.
//!
//! # Generations
//!
//! Generation 0 is baseline calibration: the noise-floor `Judged` trials of the baseline and their `Spent`
//! records. Proposals and `Selected` decisions start at generation 1.
//!
//! # Evolution
//!
//! A [`Record`] is `{"v":"1","<snake_case_variant>":{...}}`, and every event and payload struct refuses unknown
//! fields. Adding an enum variant, or adding an `Option` field to an existing variant (an absent field reads as
//! `None`), keeps every existing line parseable. A required new field, a renamed field, or a changed type needs a
//! new record version, `"v":"2"`.
//!
//! No ledger type holds a 128-bit integer or a dollar amount: serde cannot carry a 128-bit integer inside an
//! internally tagged enum, and dollars are derived from [`Usage`] and the user's price table, never stored
//! (COST-01).

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::{
    BlobId, ExperimentId, IdHasher, ImageDigest, ManifestId, ModelId, PromptHash, ReportHash,
    RunId, TreeId,
};
use crate::manifest::{Arch, JudgeVersion, Tier};
use crate::outcome::Outcome;
use crate::usage::Usage;

/// Domain of [`EventKey`].
const EVENT_DOMAIN: &str = "minos/event/v1";
/// Domain of [`SpendId`].
const SPEND_DOMAIN: &str = "minos/spend/v1";
/// Domain of [`ReviewId`].
const REVIEW_DOMAIN: &str = "minos/review/v1";

// ---------------------------------------------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------------------------------------------

crate::id::digest_id! {
    /// The content-derived key of one ledger event, under the domain `minos/event/v1`. See the module docs for
    /// the key table.
    EventKey
}

crate::id::digest_id! {
    /// One metered step of a run: the run, the generation, and the [`SpendSource`], under the domain
    /// `minos/spend/v1`.
    SpendId
}

impl SpendId {
    /// The ID of the spend of `source` in `generation` of `run`.
    #[must_use]
    pub fn of(run: &RunId, generation: u32, source: &SpendSource) -> Self {
        let hasher = IdHasher::new(SPEND_DOMAIN)
            .digest(run.as_bytes())
            .u64(u64::from(generation));
        let hasher = match source {
            SpendSource::ModelCall { call, model } => hasher
                .u64(1)
                .u64(u64::from(*call))
                .bytes(model.as_str().as_bytes()),
            SpendSource::Candidate { exp, trial } => {
                hasher.u64(2).digest(exp.as_bytes()).u64(u64::from(*trial))
            }
            SpendSource::Judge { exp, trial } => {
                hasher.u64(3).digest(exp.as_bytes()).u64(u64::from(*trial))
            }
        };
        Self::from_digest(hasher.finish())
    }
}

crate::id::digest_id! {
    /// A review request: the run, the [`ReviewKind`], and the blob under review, under the domain
    /// `minos/review/v1`.
    ReviewId
}

impl ReviewId {
    /// The ID of a request of `kind` about `subject` in `run`.
    #[must_use]
    pub fn of(run: &RunId, kind: ReviewKind, subject: &BlobId) -> Self {
        Self::from_digest(
            IdHasher::new(REVIEW_DOMAIN)
                .digest(run.as_bytes())
                .u64(kind.tag())
                .digest(subject.as_bytes())
                .finish(),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Payloads
// ---------------------------------------------------------------------------------------------------------------

/// Where a proposal came from. A sweep proposal has no model and no prompt.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    /// Seeded sampling over declared params; no model was called.
    Sweep,
    /// A model call.
    Model {
        /// The model that wrote the proposal.
        model: ModelId,
        /// The hash of the exact prompt sent.
        prompt: PromptHash,
    },
}

/// Why a run stopped. A closed set with fixed display strings, so no shell or model text enters the ledger
/// through a stop (03-CONTEXT D-05).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StopReason {
    /// `minos stop`.
    UserStop,
    /// Metered spend passed the manifest's spend cap.
    SpendCap,
    /// Total tokens passed the manifest's token cap.
    TokenCap,
    /// The circuit breaker: consecutive generations with no valid candidate.
    NoValidCandidates,
    /// The circuit breaker: consecutive infrastructure failures.
    InfraFailures,
    /// The model provider refused for a rate or usage limit.
    ProviderLimit,
}

impl StopReason {
    /// The key tag (module docs): explicit arms, never a discriminant cast.
    const fn tag(self) -> u64 {
        match self {
            Self::UserStop => 1,
            Self::SpendCap => 2,
            Self::TokenCap => 3,
            Self::NoValidCandidates => 4,
            Self::InfraFailures => 5,
            Self::ProviderLimit => 6,
        }
    }
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UserStop => "stopped by the user",
            Self::SpendCap => "spend cap reached",
            Self::TokenCap => "token cap reached",
            Self::NoValidCandidates => "no valid candidate in consecutive generations",
            Self::InfraFailures => "consecutive infrastructure failures",
            Self::ProviderLimit => "model provider rate or usage limit",
        })
    }
}

/// What a review request asks the user to approve. The agent may propose these; it never approves them.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewKind {
    /// A drafted judge design.
    JudgeDesign,
    /// New sweep params.
    Params,
    /// New dependencies.
    Dependencies,
}

impl ReviewKind {
    /// The [`ReviewId`] tag (module docs): explicit arms, never a discriminant cast.
    const fn tag(self) -> u64 {
        match self {
            Self::JudgeDesign => 1,
            Self::Params => 2,
            Self::Dependencies => 3,
        }
    }
}

/// The user's answer to a review request.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewAnswer {
    /// Approved.
    Approved,
    /// Declined.
    Declined,
}

/// What one [`Event::Spent`] record metered: one model call or one VM run (03-CONTEXT D-09).
///
/// The model is on the source because prices are per model (COST-01).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SpendSource {
    /// The `call`-th model call of the generation.
    ModelCall {
        /// Index of the call within the generation, from 0.
        call: u32,
        /// The model called.
        model: ModelId,
    },
    /// One candidate VM run.
    Candidate {
        /// The experiment run.
        exp: ExperimentId,
        /// The trial, from 0.
        trial: u32,
    },
    /// One judge VM run.
    Judge {
        /// The experiment judged.
        exp: ExperimentId,
        /// The trial, from 0.
        trial: u32,
    },
}

impl SpendSource {
    /// The experiment this spend belongs to, if any. A model call belongs to the generation, not one experiment.
    #[must_use]
    pub const fn exp(&self) -> Option<&ExperimentId> {
        match self {
            Self::ModelCall { .. } => None,
            Self::Candidate { exp, .. } | Self::Judge { exp, .. } => Some(exp),
        }
    }
}

/// The kind of a generation's selection decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DecisionKind {
    /// A candidate beat the incumbent by at least `min_improvement` and becomes the incumbent.
    Improved {
        /// The new incumbent.
        winner: ExperimentId,
    },
    /// No valid candidate cleared the threshold; the incumbent stays.
    Kept {
        /// The incumbent that stays.
        incumbent: ExperimentId,
    },
    /// No candidate of the generation produced a valid score.
    NoValidCandidates,
}

/// A generation's selection decision, with the counts the circuit breaker and `minos status` read.
///
/// Every combination of kind and counts is a valid record, so the derived `Deserialize` is the whole validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    kind: DecisionKind,
    proposed: u32,
    dropped: u32,
}

impl Decision {
    /// A decision of `kind` over `proposed` recorded proposals, with `dropped` proposals rejected at parse or
    /// compose (PITFALLS Pitfall 34, the invalid-proposal rate).
    #[must_use]
    pub const fn new(kind: DecisionKind, proposed: u32, dropped: u32) -> Self {
        Self {
            kind,
            proposed,
            dropped,
        }
    }

    /// The decision's kind.
    #[must_use]
    pub const fn kind(&self) -> &DecisionKind {
        &self.kind
    }

    /// The number of proposals the generation recorded.
    #[must_use]
    pub const fn proposed(&self) -> u32 {
        self.proposed
    }

    /// The number of proposals dropped at parse or compose, which never became experiments.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }
}

/// Which backend ran a VM.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BackendKind {
    /// libkrun through `minos-vmm`, on macOS arm64.
    Krun,
}

/// The isolation level of a run. Ordered weakest first, so the minimum over a lineage is its weakest level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Isolation {
    /// Process-only isolation, the labelled dev mode (`WeakIsolation` in the report).
    WeakProcess,
    /// Hardware virtualization.
    Hardware,
}

/// A host-side confinement layer around the VMM process (KRUN-07).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfinementLayer {
    /// The Seatbelt deny-default profile, through `/usr/bin/sandbox-exec`.
    Seatbelt,
    /// The dedicated unprivileged `_minos` user.
    DedicatedUser,
    /// Resource limits on the helper process.
    Rlimits,
}

/// The layers a krun run is expected to have. A layer absent from a run's [`BackendInfo::layers`] is reduced
/// hardening, which the report shows (03-CONTEXT D-18).
pub const EXPECTED_KRUN_LAYERS: [ConfinementLayer; 3] = [
    ConfinementLayer::Seatbelt,
    ConfinementLayer::DedicatedUser,
    ConfinementLayer::Rlimits,
];

/// Where and how a VM ran: recorded on every `Judged` event so the report can show provenance (D-18, KRUN-07).
///
/// Power source and thermal state can be added later as `Option` fields under the evolution rule.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawBackendInfo", into = "RawBackendInfo")]
pub struct BackendInfo {
    backend: BackendKind,
    arch: Arch,
    isolation: Isolation,
    layers: BTreeSet<ConfinementLayer>,
    root_image: ImageDigest,
    kernel: Option<ImageDigest>,
}

impl BackendInfo {
    /// The provenance of one VM run: `layers` are the confinement layers that were active.
    #[must_use]
    pub const fn new(
        backend: BackendKind,
        arch: Arch,
        isolation: Isolation,
        layers: BTreeSet<ConfinementLayer>,
        root_image: ImageDigest,
        kernel: Option<ImageDigest>,
    ) -> Self {
        Self {
            backend,
            arch,
            isolation,
            layers,
            root_image,
            kernel,
        }
    }

    /// The backend.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
    }

    /// The guest architecture.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        self.arch
    }

    /// The isolation level.
    #[must_use]
    pub const fn isolation(&self) -> Isolation {
        self.isolation
    }

    /// The confinement layers that were active.
    #[must_use]
    pub const fn layers(&self) -> &BTreeSet<ConfinementLayer> {
        &self.layers
    }

    /// The digest of the root image the VM booted.
    #[must_use]
    pub const fn root_image(&self) -> &ImageDigest {
        &self.root_image
    }

    /// The digest of the guest kernel, when the backend reports one.
    #[must_use]
    pub const fn kernel(&self) -> Option<&ImageDigest> {
        self.kernel.as_ref()
    }
}

/// [`BackendInfo`] as written: `layers` is an array that must be strictly ascending, so each value has exactly
/// one wire form.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBackendInfo {
    backend: BackendKind,
    arch: Arch,
    isolation: Isolation,
    layers: Vec<ConfinementLayer>,
    root_image: ImageDigest,
    kernel: Option<ImageDigest>,
}

/// Why a [`RawBackendInfo`] is not a [`BackendInfo`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackendInfoError {
    /// The layers are repeated or out of order.
    #[error("backend layers must be listed once each, in ascending order")]
    LayersNotCanonical,
}

impl TryFrom<RawBackendInfo> for BackendInfo {
    type Error = BackendInfoError;

    fn try_from(raw: RawBackendInfo) -> Result<Self, Self::Error> {
        let ascending = raw
            .layers
            .iter()
            .zip(raw.layers.iter().skip(1))
            .all(|(before, after)| before < after);
        if !ascending {
            return Err(BackendInfoError::LayersNotCanonical);
        }
        Ok(Self::new(
            raw.backend,
            raw.arch,
            raw.isolation,
            raw.layers.into_iter().collect(),
            raw.root_image,
            raw.kernel,
        ))
    }
}

impl From<BackendInfo> for RawBackendInfo {
    fn from(info: BackendInfo) -> Self {
        Self {
            backend: info.backend,
            arch: info.arch,
            isolation: info.isolation,
            layers: info.layers.into_iter().collect(),
            root_image: info.root_image,
            kernel: info.kernel,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------------------------------

/// One ledger event. The set is frozen (CORE-08); each event's key is in the module docs.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// A run began. Always the first record; the fold recomputes `run` and `baseline` from the other fields.
    RunStarted {
        /// `RunId::of(manifest, base, started_at)`.
        run: RunId,
        /// The manifest the run started with.
        manifest: ManifestId,
        /// The base source tree.
        base: TreeId,
        /// `ExperimentId::baseline(base)`.
        baseline: ExperimentId,
        /// The start time in Unix seconds, read by the shell.
        started_at: u64,
    },
    /// The run uses a new manifest from `from_generation` on.
    ManifestChanged {
        /// The new manifest.
        manifest: ManifestId,
        /// The first generation that uses it.
        from_generation: u32,
    },
    /// A proposal became an experiment.
    Proposed {
        /// The experiment.
        exp: ExperimentId,
        /// The experiment it was proposed from: lineage only, since the diff is cumulative from the base.
        parent: ExperimentId,
        /// The generation that proposed it, from 1.
        generation: u32,
        /// The tier that proposed it.
        tier: Tier,
        /// The blob of the canonical cumulative diff from the base tree.
        diff: BlobId,
        /// The blob of the proposer's rationale.
        rationale: BlobId,
        /// The sweep or the model call that produced it.
        origin: Origin,
    },
    /// One trial of an experiment under a judge version ended.
    Judged {
        /// The experiment.
        exp: ExperimentId,
        /// The judge version that scored it.
        judge: JudgeVersion,
        /// The trial, from 0.
        trial: u32,
        /// How the trial ended.
        outcome: Outcome,
        /// Where and how the VMs ran.
        backend: BackendInfo,
    },
    /// A generation concluded.
    Selected {
        /// The generation.
        generation: u32,
        /// The decision.
        decision: Decision,
    },
    /// One metered step. Usage only, never dollars (COST-01).
    Spent {
        /// The generation the spend belongs to; 0 for baseline calibration.
        generation: u32,
        /// What was metered.
        source: SpendSource,
        /// The metered quantities.
        usage: Usage,
    },
    /// A note from `minos steer` for a generation. The text is a blob, never inline.
    Steered {
        /// The generation whose propose step reads the note.
        generation: u32,
        /// The note's position among that generation's notes, from 0.
        ordinal: u32,
        /// The blob of the note text.
        note: BlobId,
    },
    /// The run should stop before `generation`.
    StopRequested {
        /// The generation at which the stop was requested.
        generation: u32,
        /// Why.
        reason: StopReason,
    },
    /// A decision now waits on the user.
    ReviewRequested {
        /// The request.
        request: ReviewId,
        /// What is to be reviewed.
        kind: ReviewKind,
    },
    /// The user answered a review request.
    Reviewed {
        /// The request answered.
        request: ReviewId,
        /// The answer.
        answer: ReviewAnswer,
    },
    /// The user acknowledged the report for an experiment.
    ReportAcknowledged {
        /// The experiment.
        exp: ExperimentId,
        /// The hash of the report the user acknowledged.
        report: ReportHash,
    },
}

impl Event {
    /// The event's key in `run`, as the module-level key table defines it.
    #[must_use]
    pub fn key(&self, run: &RunId) -> EventKey {
        let hasher = IdHasher::new(EVENT_DOMAIN).digest(run.as_bytes());
        let hasher = match self {
            Self::RunStarted { .. } => hasher.u64(1),
            Self::ManifestChanged {
                manifest,
                from_generation,
            } => hasher
                .u64(2)
                .digest(manifest.as_bytes())
                .u64(u64::from(*from_generation)),
            Self::Proposed { exp, .. } => hasher.u64(3).digest(exp.as_bytes()),
            Self::Judged {
                exp, judge, trial, ..
            } => hasher
                .u64(4)
                .digest(exp.as_bytes())
                .digest(judge.as_bytes())
                .u64(u64::from(*trial)),
            Self::Selected { generation, .. } => hasher.u64(5).u64(u64::from(*generation)),
            Self::Spent {
                generation, source, ..
            } => hasher
                .u64(6)
                .digest(SpendId::of(run, *generation, source).as_bytes()),
            Self::Steered {
                generation,
                ordinal,
                ..
            } => hasher
                .u64(7)
                .u64(u64::from(*generation))
                .u64(u64::from(*ordinal)),
            Self::StopRequested { generation, reason } => {
                hasher.u64(8).u64(u64::from(*generation)).u64(reason.tag())
            }
            Self::ReviewRequested { request, .. } => hasher.u64(9).digest(request.as_bytes()),
            Self::Reviewed { request, .. } => hasher.u64(10).digest(request.as_bytes()),
            Self::ReportAcknowledged { exp, report } => hasher
                .u64(11)
                .digest(exp.as_bytes())
                .digest(report.as_bytes()),
        };
        EventKey::from_digest(hasher.finish())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Record and Line
// ---------------------------------------------------------------------------------------------------------------

/// A versioned event: `{"v":"1","<snake_case_variant>":{...}}`. The `v` tag is how the wire format changes.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum Record {
    /// Version 1.
    #[serde(rename = "1")]
    V1(Event),
}

impl Record {
    /// A version-1 record of `event`.
    #[must_use]
    pub const fn v1(event: Event) -> Self {
        Self::V1(event)
    }

    /// The event.
    #[must_use]
    pub const fn event(&self) -> &Event {
        match self {
            Self::V1(event) => event,
        }
    }
}

/// Why a ledger line failed verification.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LineError {
    /// The stored key is not the key of the stored record in this run: the line was tampered with or belongs to
    /// another run.
    #[error("the line's key does not match its record")]
    KeyMismatch,
}

/// One JSON Lines row of the ledger: a sequence number, the record's key, and the record.
///
/// The store (Phase 4) calls [`Line::verify`] on every line it reads and treats an append whose key already
/// exists as a no-op (STORE-03).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    seq: u64,
    key: EventKey,
    record: Record,
}

impl Line {
    /// The line at position `seq` holding `record`, keyed in `run`.
    #[must_use]
    pub fn new(seq: u64, run: &RunId, record: Record) -> Self {
        let key = record.event().key(run);
        Self { seq, key, record }
    }

    /// The line's position in the ledger.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The stored key.
    #[must_use]
    pub const fn key(&self) -> &EventKey {
        &self.key
    }

    /// The record.
    #[must_use]
    pub const fn record(&self) -> &Record {
        &self.record
    }

    /// Recomputes the record's key in `run` and compares it with the stored key.
    ///
    /// # Errors
    ///
    /// [`LineError::KeyMismatch`] when they differ.
    pub fn verify(&self, run: &RunId) -> Result<(), LineError> {
        if self.record.event().key(run) == self.key {
            Ok(())
        } else {
            Err(LineError::KeyMismatch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EVENT_DOMAIN, REVIEW_DOMAIN, SPEND_DOMAIN};

    #[test]
    fn domains_contain_no_nul() {
        for domain in [EVENT_DOMAIN, SPEND_DOMAIN, REVIEW_DOMAIN] {
            assert!(!domain.as_bytes().contains(&0));
        }
    }
}
