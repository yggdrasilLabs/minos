//! What a propose step reads from the run: the parent tree, the prompt's history, the steer notes, and the
//! recorded manifest (06-CONTEXT D-05, D-07, D-08, D-09; 07-CONTEXT D-09, D-12; 07-REVIEW WR-03).
//!
//! Each reader is a pure function of the ledger fold and the blob store, so a resumed run builds the same parent
//! tree, history, and notes as an uninterrupted one at the same generation.
//!
//! # Hostile bytes
//!
//! Stage logs, rationales, and notes are blobs that may hold hostile bytes: a candidate wrote the logs, a model
//! wrote the rationales, and a user wrote the notes. They reach the prompt only through core's sanitizing types:
//! a log through [`LogExcerpt::from_bytes`], read under [`MAX_STAGE_LOG_BYTES`] and cut to
//! [`MAX_LOG_EXCERPT_BYTES`]; a rationale through `HistoryEntry::new`, which renders it with `SafeText`; and a note
//! through [`SteerNote::new`], which does the same. No reader here returns blob bytes.
//!
//! # Recorded diffs
//!
//! A recorded diff is re-read through [`CumulativeDiff::recorded`], which accepts it only when it hashes to the
//! experiment the ledger recorded it under, so a tampered or substituted diff blob never becomes a parent tree.
//!
//! # Recorded manifests
//!
//! A run's manifest is read back from the blob of its exact bytes by [`recorded_manifest`] (the one in effect
//! now) and [`recorded_manifest_at`] (the one in effect at a generation). The driver and the CLI share this one
//! implementation, so a missing, over-cap, or tampered blob is always [`DriverError::ManifestRecordUnreadable`]
//! and never reported as a manifest edit.

use minos_core::diff::Diff;
use minos_core::diff::apply::{Tree, apply};
use minos_core::diff::cumulative::{BaseTree, CumulativeDiff};
use minos_core::id::{BlobId, ExperimentId, ManifestId, RunId};
use minos_core::ledger::DecisionKind;
use minos_core::limits::{
    Limits, MAX_LOG_EXCERPT_BYTES, MAX_MANIFEST_BYTES, MAX_STAGE_LOG_BYTES, MAX_STEER_NOTE_BYTES,
};
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::state::{Judgment, RunState};
use minos_core::text::LogExcerpt;
use minos_propose::limits::MAX_RATIONALE_BYTES;
use minos_propose::{History, HistoryEntry, SteerNote};
use minos_store::{Blobs, StoreError};

use crate::byte_cap;
use crate::error::DriverError;

/// The recorded cumulative diff of `exp`, re-read from its blob and checked against its ID.
///
/// The blob is read under the standard diff cap, decoded as UTF-8, parsed against `manifest`'s writable set, and
/// accepted only if it hashes to `exp` with `base`'s ID.
///
/// # Errors
///
/// [`DriverError::DiffRecord`] when `exp` has no recorded proposal, or its diff blob is over the cap, is not
/// UTF-8, does not parse, or does not hash to `exp`; [`DriverError::Store`] when the blob cannot be read.
pub fn recorded_diff<S: Blobs>(
    store: &S,
    state: &RunState,
    manifest: &Manifest,
    base: &BaseTree,
    exp: &ExperimentId,
) -> Result<CumulativeDiff, DriverError> {
    let unrecorded = || DriverError::DiffRecord { exp: *exp };
    let proposal = state.proposal(exp).ok_or_else(unrecorded)?;
    let limits = Limits::standard();
    let bytes = store
        .get(proposal.diff(), byte_cap(limits.max_diff_bytes()))
        .map_err(|source| match source {
            StoreError::BlobTooLarge { .. } => unrecorded(),
            source => DriverError::Store {
                run: *state.run(),
                source,
            },
        })?;
    let text = String::from_utf8(bytes).map_err(|_| unrecorded())?;
    let diff = Diff::parse(&text, manifest.writable(), &limits).map_err(|_| unrecorded())?;
    CumulativeDiff::recorded(base.id(), diff, exp).map_err(|_| unrecorded())
}

/// The incumbent `generation` proposes from, and its surface tree: the base tree for the baseline, else the base
/// with the incumbent's recorded diff applied (06-CONTEXT D-05).
///
/// # Errors
///
/// As [`recorded_diff`], and [`DriverError::DiffRecord`] when the recorded diff does not apply to the base.
pub fn parent_tree<S: Blobs>(
    store: &S,
    state: &RunState,
    manifest: &Manifest,
    base: &BaseTree,
    generation: u32,
) -> Result<(ExperimentId, Tree), DriverError> {
    let incumbent = state.incumbent_before(generation);
    if incumbent == *state.baseline() {
        return Ok((incumbent, base.tree().clone()));
    }
    let recorded = recorded_diff(store, state, manifest, base, &incumbent)?;
    let tree = apply(base.tree(), recorded.as_diff())
        .map_err(|_| DriverError::DiffRecord { exp: incumbent })?;
    Ok((incumbent, tree))
}

/// The stage whose log explains `outcome`, or `None` when no log does, from the outcome alone.
///
/// A build failure is the build's; a run failure and every output fault are the run's; a scorecard or gate fault
/// is the judge's; a crash or timeout names its stage. A diff that did not apply ran no stage, and a scored
/// trial needs no excerpt. [`excerpt_log`] refines the output faults against the logs a trial recorded (IN-05).
#[must_use]
pub const fn excerpt_stage(outcome: &Outcome) -> Option<Stage> {
    match outcome {
        Outcome::Scored(_) => None,
        Outcome::Crashed { stage } | Outcome::TimedOut { stage } => Some(*stage),
        Outcome::Rejected(reason) => match reason {
            Reason::BuildFailed => Some(Stage::Build),
            Reason::RunFailed
            | Reason::OutputInvalid
            | Reason::OutputTooLarge
            | Reason::ChunkProtocol => Some(Stage::Run),
            Reason::GateFailed { .. }
            | Reason::MissingMetric { .. }
            | Reason::UndeclaredMetric { .. }
            | Reason::HeldOutMismatch { .. }
            | Reason::ScorecardInvalid => Some(Stage::Judge),
            // `Reason` is non-exhaustive; a reason added later shows no excerpt until it is mapped here.
            _ => None,
        },
    }
}

/// The prompt's history for `generation`: one entry per experiment proposed in an earlier generation, with its
/// trials under `judge`, whether it won its generation, its rationale, and the excerpt of its first failed trial's
/// log, rendered against the incumbent's trials (06-CONTEXT D-07, D-08; 07-CONTEXT D-09).
///
/// # Errors
///
/// [`DriverError::Store`] when a rationale or log blob cannot be read under its cap.
pub fn history<S: Blobs>(
    store: &S,
    state: &RunState,
    manifest: &Manifest,
    judge: &JudgeVersion,
    generation: u32,
) -> Result<History, DriverError> {
    let store_error = |source: StoreError| DriverError::Store {
        run: *state.run(),
        source,
    };
    let mut entries = Vec::new();
    for earlier in 1..generation {
        let winner = state
            .decision(earlier)
            .and_then(|decision| match decision.kind() {
                DecisionKind::Improved { winner } => Some(*winner),
                DecisionKind::Kept { .. } | DecisionKind::NoValidCandidates => None,
            });
        for (exp, proposal) in state.proposals_in(earlier) {
            let rationale = store
                .get(proposal.rationale(), byte_cap(MAX_RATIONALE_BYTES))
                .map_err(store_error)?;
            let judgments: Vec<(u32, &Judgment)> = state.trials(exp, judge).collect();
            let excerpt = excerpt(store, &judgments).map_err(store_error)?;
            let trials = judgments
                .into_iter()
                .map(|(trial, judgment)| (trial, judgment.outcome().clone()))
                .collect();
            entries.push(HistoryEntry::new(
                earlier,
                *exp,
                winner == Some(*exp),
                &String::from_utf8_lossy(&rationale),
                trials,
                excerpt,
            ));
        }
    }
    let incumbent: Vec<Outcome> = state
        .trials(&state.incumbent_before(generation), judge)
        .map(|(_, judgment)| judgment.outcome().clone())
        .collect();
    Ok(History::new(
        manifest.judge(),
        generation,
        &incumbent,
        entries,
    ))
}

/// The recorded log that explains `judgment`, if any: the log of its [`excerpt_stage`], except that an output
/// fault (`OutputInvalid`, `OutputTooLarge`, `ChunkProtocol`) is the judge VM's when the trial recorded a judge
/// log (07-REVIEW IN-05).
///
/// The runner stores a judge log only for a launch the candidate passed into, and the same three reasons come
/// from the judge VM's stream and status as from the candidate's. So a judge log beside one of them says the
/// candidate passed and the judge's output was at fault, and the judge log is the one that explains it; without
/// a judge log the fault is the candidate's own, or the judge's stream was unreadable and no log explains it, and
/// the run log is shown as before.
#[must_use]
pub fn excerpt_log(judgment: &Judgment) -> Option<&BlobId> {
    let logs = judgment.logs()?;
    let stage = excerpt_stage(judgment.outcome())?;
    let output_fault = matches!(
        judgment.outcome(),
        Outcome::Rejected(Reason::OutputInvalid | Reason::OutputTooLarge | Reason::ChunkProtocol)
    );
    if output_fault && let Some(log) = logs.get(&Stage::Judge) {
        return Some(log);
    }
    logs.get(&stage)
}

/// The excerpt of the first trial, in trial order, that has an [`excerpt_log`], read under
/// [`MAX_STAGE_LOG_BYTES`] and passed only through [`LogExcerpt::from_bytes`].
fn excerpt<S: Blobs>(
    store: &S,
    judgments: &[(u32, &Judgment)],
) -> Result<Option<LogExcerpt>, StoreError> {
    let log: Option<&BlobId> = judgments
        .iter()
        .find_map(|(_, judgment)| excerpt_log(judgment));
    log.map(|log| {
        let bytes = store.get(log, byte_cap(MAX_STAGE_LOG_BYTES))?;
        Ok(LogExcerpt::from_bytes(&bytes, MAX_LOG_EXCERPT_BYTES))
    })
    .transpose()
}

/// The steer notes for `generation`, in ordinal order, each read under [`MAX_STEER_NOTE_BYTES`], decoded lossily,
/// and rendered through [`SteerNote::new`] under its recorded ordinal (06-CONTEXT D-09).
///
/// # Errors
///
/// [`DriverError::Store`] when a note blob cannot be read under its cap.
pub fn notes<S: Blobs>(
    store: &S,
    state: &RunState,
    generation: u32,
) -> Result<Vec<SteerNote>, DriverError> {
    state
        .notes_by_ordinal(generation)
        .into_iter()
        .map(|(ordinal, note)| {
            let bytes = store
                .get(&note, byte_cap(MAX_STEER_NOTE_BYTES))
                .map_err(|source| DriverError::Store {
                    run: *state.run(),
                    source,
                })?;
            Ok(SteerNote::new(ordinal, &String::from_utf8_lossy(&bytes)))
        })
        .collect()
}

/// The manifest the run has in effect now, read back from its blob.
///
/// # Errors
///
/// [`DriverError::ManifestRecordUnreadable`] when the blob of [`RunState::manifest`] is absent, over
/// [`MAX_MANIFEST_BYTES`], or tampered, or its bytes are not a manifest: a store problem a human must look at,
/// never reported as a manifest edit (07-REVIEW WR-03). [`DriverError::Store`] for any other store error.
pub fn recorded_manifest<S: Blobs>(store: &S, state: &RunState) -> Result<Manifest, DriverError> {
    manifest_blob(store, state.run(), state.manifest())
}

/// The manifest the run had in effect at `generation`, read back from its blob: the latest `ManifestChanged`
/// from that generation or earlier, else the manifest the run started with.
///
/// # Errors
///
/// As [`recorded_manifest`], for the blob of [`RunState::manifest_at`] `generation`.
pub fn recorded_manifest_at<S: Blobs>(
    store: &S,
    state: &RunState,
    generation: u32,
) -> Result<Manifest, DriverError> {
    manifest_blob(store, state.run(), state.manifest_at(generation))
}

/// The manifest whose exact bytes are the blob with `manifest`'s digest, read under [`MAX_MANIFEST_BYTES`].
fn manifest_blob<S: Blobs>(
    store: &S,
    run: &RunId,
    manifest: &ManifestId,
) -> Result<Manifest, DriverError> {
    let unreadable = || DriverError::ManifestRecordUnreadable {
        run: *run,
        manifest: *manifest,
    };
    // A manifest's ID is the plain SHA-256 of its bytes, so the blob of those bytes has the same digest.
    let blob = manifest
        .to_hex()
        .parse::<BlobId>()
        .map_err(|_| unreadable())?;
    // The same cap the CLI read the manifest under, so an accepted manifest is never unreadable here (IN-01).
    let bytes = store
        .get(&blob, MAX_MANIFEST_BYTES)
        .map_err(|source| match source {
            StoreError::BlobMissing { .. }
            | StoreError::BlobTooLarge { .. }
            | StoreError::BlobTampered { .. } => unreadable(),
            source => DriverError::Store { run: *run, source },
        })?;
    let text = std::str::from_utf8(&bytes).map_err(|_| unreadable())?;
    Manifest::parse_toml(text).map_err(|_| unreadable())
}
