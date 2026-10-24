//! Every way the driver stops without an outcome.
//!
//! A candidate's failure is never here: it is an [`minos_core::Outcome`], recorded in a `Judged` event. An
//! infrastructure failure is retried inside the driver, at the same step after a fixed pause, until the streak
//! reaches `max_infra_failures`; the driver then records `StopRequested { InfraFailures }` and exits normally
//! (07-CONTEXT D-16). So a [`DriverError`] means one of three things, and each variant says which:
//!
//! - **configuration:** the run cannot continue as configured, and a human changes something (a rebuilt image
//!   means a new run, a missing price means editing `prices.toml`);
//! - **corruption:** the ledger or a recorded blob contradicts itself, and a human must look at it;
//! - **infrastructure that could not be recorded:** the driver could not even record its own stop.
//!
//! Every message names IDs, numbers, and fixed text only, never bytes from a sandbox or a model, so a caller
//! may print it as it is.

use std::collections::BTreeSet;

use minos_core::id::{ExperimentId, ImageDigest, ManifestId, ModelId, RunId, TreeId};
use minos_core::job::JobError;
use minos_core::ledger::BackendKind;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::money::CostError;
use minos_core::policy::PolicyError;
use minos_core::route::RouteError;
use minos_core::state::FoldError;
use minos_propose::ProposeError;
use minos_store::StoreError;

/// Why the driver stopped without recording an outcome.
///
/// Infrastructure failures are retried inside the driver and surface here only as [`DriverError::Infra`], when
/// even the stop that ends the retries could not be recorded. Every other variant is a configuration error or a
/// corrupt record, never retried, and each says whether a human must act.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// Another driver holds the run's lock (LOOP-06). Configuration: wait for that driver to finish or stop it
    /// with `minos stop`; nothing needs repair.
    #[error(
        "run {run} is being driven by another minos process; wait for it to finish or stop it with minos stop"
    )]
    RunLocked {
        /// The run.
        run: RunId,
    },
    /// The store holds no ledger for the run. Configuration: the run ID names no run in this project.
    #[error("no ledger exists for run {run}")]
    RunNotFound {
        /// The run.
        run: RunId,
    },
    /// An append found a different record under the same key, so nothing was written and the first write won
    /// (07-CONTEXT D-08). Corruption or a driver bug: never retried, and the ledger needs a human.
    #[error(
        "the ledger of run {run} already holds a different {kind} at seq {seq}; it needs a human"
    )]
    Conflict {
        /// The run.
        run: RunId,
        /// The existing line's position.
        seq: u64,
        /// The event kind the driver tried to append, as fixed text.
        kind: &'static str,
    },
    /// A store operation failed in a way that is not retried: a corrupt ledger, a tampered or oversized blob, an
    /// unsafe store path. A human must look at the store.
    #[error("the store failed for run {run}")]
    Store {
        /// The run.
        run: RunId,
        /// The store's error.
        source: StoreError,
    },
    /// The manifest's experiment frame differs from the one the run recorded between sessions. Configuration: the
    /// frame (surface, commands, judge spec, environment) defines what an experiment means, so a changed frame
    /// means a new run. A budget, policy, proposer, or param edit is not refused (07-CONTEXT D-19).
    #[error(
        "run recorded manifest {recorded}, but the current manifest is {current}; start a new run"
    )]
    ManifestChanged {
        /// The manifest the run recorded.
        recorded: ManifestId,
        /// The current manifest.
        current: ManifestId,
    },
    /// The manifest blob the run recorded is missing from the store, over its cap, tampered, or no longer parses
    /// as a manifest, so the experiment frame cannot be compared (07-REVIEW WR-03). Corruption: the store needs a
    /// human, and it is never reported as a manifest edit.
    #[error(
        "the manifest blob {manifest} recorded for run {run} is missing, tampered, or no longer parses; the store needs a human"
    )]
    ManifestRecordUnreadable {
        /// The run.
        run: RunId,
        /// The manifest the run recorded, whose blob could not be read back.
        manifest: ManifestId,
    },
    /// The source tree supplied for the session is not the run's base tree. Configuration: the working tree
    /// changed since the run started.
    #[error("run recorded base tree {recorded}, but the supplied tree is {supplied}")]
    BaseMismatch {
        /// The base tree the run recorded.
        recorded: TreeId,
        /// The tree supplied for this session.
        supplied: TreeId,
    },
    /// The lock's root image differs from the one the run recorded, or the run recorded none (07-CONTEXT D-10).
    /// Configuration: a rebuilt image means a new run.
    #[error(
        "run recorded root image {}, but minos.lock names {current}; a rebuilt image means a new run",
        recorded.map_or_else(|| String::from("none"), |digest| digest.to_string())
    )]
    RootImageChanged {
        /// The root image the run recorded, or `None` for a run started before Phase 7.
        recorded: Option<ImageDigest>,
        /// The root image `minos.lock` names now.
        current: ImageDigest,
    },
    /// The judge version the run's trials were recorded under is not the current one. Configuration: judge data
    /// pushed again under the same label changes what a score means, so it needs a new run.
    #[error("run recorded judge version {recorded}, but the current judge version is {current}")]
    JudgeChanged {
        /// The judge version the run recorded.
        recorded: JudgeVersion,
        /// The current judge version.
        current: JudgeVersion,
    },
    /// `prices.toml` has no rate for models the manifest's proposers name, checked before any call
    /// (07-CONTEXT D-13). Configuration: add the rates; a missing rate is never priced at zero.
    #[error("prices.toml has no rate for {} model(s) the proposers name", models.len())]
    PricesMissing {
        /// The models with no rate, in order.
        models: BTreeSet<ModelId>,
    },
    /// A model call was recorded under a resolved model ID that `prices.toml` does not price (07-CONTEXT D-13,
    /// 07-RESEARCH Pitfall 7). Configuration: add the rate, then resume; the call's spend is already recorded.
    #[error("prices.toml has no rate for model {model}, which a recorded model call used")]
    UnpricedModel {
        /// The resolved model ID.
        model: ModelId,
    },
    /// Pricing the recorded usage failed. Configuration: the price table cannot price the run.
    #[error(transparent)]
    Cost(CostError),
    /// A trial ran on a backend class other than the run's first (07-CONTEXT D-11). Configuration: a run never
    /// mixes backends, so its speed numbers all come from one class; nothing was recorded.
    #[error("run is pinned to backend {recorded:?} on {recorded_arch:?}, but a trial ran on {found:?} on {found_arch:?}",
        recorded = recorded.0, recorded_arch = recorded.1, found = found.0, found_arch = found.1)]
    BackendChanged {
        /// The backend class of the run's first trial.
        recorded: (BackendKind, Arch),
        /// The backend class the refused trial ran on.
        found: (BackendKind, Arch),
    },
    /// No baseline trial scored, so no candidate can ever be compared with it (07-RESEARCH Pitfall 12).
    /// Configuration: the baseline does not build, run, or pass its gates as configured; a human fixes the project
    /// before any model call is paid for. No `StopReason` is added for it.
    #[error("none of the {trials} baseline trials scored; fix the baseline before proposing")]
    BaselineUnscored {
        /// How many baseline trials ran.
        trials: u32,
    },
    /// The job runner refused a job as configured ([`crate::job::JobFailure::Config`]). Configuration: nothing
    /// was recorded.
    #[error("the job cannot run as configured: {what}")]
    Job {
        /// What is wrong, as fixed text.
        what: &'static str,
    },
    /// An infrastructure failure the driver could not retry or record, such as a failed append of the breaker's
    /// own `StopRequested`. Infrastructure: resuming the run retries from the ledger.
    #[error("an infrastructure failure could not be recorded: {what}")]
    Infra {
        /// What failed, as fixed text.
        what: &'static str,
    },
    /// The propose step failed in a way that is neither a provider stop nor an infrastructure failure: a
    /// proposer misconfigured for its tier, a prompt or schema that could not be built. Configuration.
    #[error(transparent)]
    Propose(ProposeError),
    /// No proposer tier could be routed. Configuration.
    #[error(transparent)]
    Route(RouteError),
    /// Selection failed on recorded results. Configuration or corruption, as the inner error says.
    #[error(transparent)]
    Policy(PolicyError),
    /// The ledger does not fold. Corruption: a human must look at it.
    #[error(transparent)]
    Fold(FoldError),
    /// A job request could not be built from recorded values. Corruption or a driver bug.
    #[error(transparent)]
    Request(JobError),
    /// A batch blob does not parse or does not match the `Proposed` records that name it (07-CONTEXT D-18).
    /// Corruption: the ledger needs a human.
    #[error(
        "the batch of generation {generation} in run {run} does not match its records; it needs a human"
    )]
    Batch {
        /// The run.
        run: RunId,
        /// The generation whose batch is inconsistent.
        generation: u32,
    },
    /// A recorded diff blob does not hash to its experiment. Corruption: the store or the ledger needs a human.
    #[error("the recorded diff of experiment {exp} does not hash to its id; it needs a human")]
    DiffRecord {
        /// The experiment.
        exp: ExperimentId,
    },
}
