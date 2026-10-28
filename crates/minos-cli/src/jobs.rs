//! The runner behind the loop's job seam (07-CONTEXT D-16; 07-RESEARCH Pattern 1, Pitfall 13).
//!
//! `minos-loop` defines [`JobRunner`] and `minos-runner` implements the real pipeline, and `deny.toml` keeps either
//! crate from linking the other. The CLI is the only crate where they meet: [`RunnerJobs`] wraps the production
//! [`Runner`] over the project's [`FileStore`] and hands the loop its results, mapping each [`RunnerError`] onto a
//! [`JobFailure`] by its blame alone. The runner stays generic over its backend `B`, never a trait object, so the
//! binary picks `RunnerJobs<KrunBackend>` concretely and the tests can pick another backend.
//!
//! [`judge_inputs`] turns the `[[judge]]` lock entry for the manifest's `[judge] version` label into the judge data
//! and the [`JudgeVersion`] the runner is built with (07-CONTEXT D-02).
//!
//! [`krun_jobs`] is the production composition `check` and `run` share: [`KrunBackend`] over the project's
//! [`FileBlobSource`], refused when the host probes failed (naming `minos host doctor`), and the [`Runner`] built
//! from a [`Prepared`] project behind [`RunnerJobs`]. It is the only path from the CLI to a VM launch.

use minos_backend_krun::KrunBackend;
use minos_backend_krun::host::HostPaths;
use minos_core::job::{JobRequest, JobResult};
use minos_core::lock::{Lock, LockMismatch};
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_loop::{JobFailure, JobRunner};
use minos_runner::{JudgeExtent, Runner, RunnerError};
use minos_sandbox::{Backend, JudgeData, LaunchError};
use minos_store::{FileStore, StoreError};

use crate::blob_source::FileBlobSource;
use crate::project::Prepared;

/// The production runner over the project's file store, as the loop's [`JobRunner`].
pub struct RunnerJobs<B: Backend> {
    runner: Runner<B, FileStore>,
}

impl<B: Backend> RunnerJobs<B> {
    /// The job runner that runs every trial through `runner`.
    #[must_use]
    pub const fn new(runner: Runner<B, FileStore>) -> Self {
        Self { runner }
    }

    /// The wrapped runner, with its store and backend.
    #[must_use]
    pub const fn runner(&self) -> &Runner<B, FileStore> {
        &self.runner
    }
}

impl<B: Backend> JobRunner for RunnerJobs<B> {
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        self.runner.run(request).map_err(|error| failure(&error))
    }

    fn check(&self, request: &JobRequest) -> Result<(), JobFailure> {
        self.runner.check(request).map_err(|error| failure(&error))
    }
}

/// The loop's view of a runner error: an infrastructure failure is retried and counted toward the breaker, and
/// every other error stops the run (D-16). Only [`RunnerError::is_infra`] decides, and the text is the cause's fixed
/// description, never bytes from a sandbox.
fn failure(error: &RunnerError) -> JobFailure {
    let what = error.what();
    if error.is_infra() {
        JobFailure::Infra { what }
    } else {
        JobFailure::Config { what }
    }
}

/// The judge data, judge version, and judge extent for `manifest`, from the `[[judge]]` entry `lock` records
/// under its `[judge] version` label: the tarball's blob, [`JudgeVersion::of`] its tree and the manifest's judge
/// spec, and the entry's recorded bytes and file count, which [`Runner::new`] checks `[environment] scratch_mib`
/// against (review CR-01).
///
/// # Errors
///
/// [`LockMismatch::NoJudge`] when the lock has no entry for the manifest's label, so a run cannot start until
/// `minos judge push` records one.
pub fn judge_inputs(
    lock: &Lock,
    manifest: &Manifest,
) -> Result<(JudgeData, JudgeVersion, JudgeExtent), LockMismatch> {
    let entry = lock.check_judge(manifest.judge().label())?;
    let tree = entry.tree();
    Ok((
        JudgeData::new(tree.to_blob_id()),
        JudgeVersion::of(tree, manifest.judge()),
        JudgeExtent::new(entry.bytes(), entry.file_count()),
    ))
}

/// Why the production runner could not be composed. Messages are fixed text; a runner refusal carries the
/// cause's fixed description only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    /// The project's store could not be opened for the backend's blob source.
    #[error("the store in .minos could not be opened")]
    Store(#[source] StoreError),
    /// The backend could not open its job root.
    #[error("the krun backend could not start")]
    Backend(#[source] LaunchError),
    /// The host probes refused, so every launch would be refused (KRUN-06, KRUN-07).
    #[error(
        "this host refuses {count} launch checks: run `minos host doctor` to see each one and its fix"
    )]
    HostRefused {
        /// How many host checks failed.
        count: usize,
    },
    /// The runner refused the project's configuration, for example a `scratch_mib` too small for the judge data.
    #[error("the runner refuses this project: {what}")]
    Runner {
        /// The runner cause's fixed description.
        what: &'static str,
    },
}

/// The production job runner for `prepared`: [`KrunBackend`] at `paths` over the project's [`FileBlobSource`],
/// and the [`Runner`] over the project's store with the prepared manifest, root image, and judge inputs, behind
/// [`RunnerJobs`]. The backend sweeps its job root and probes the host once, here.
///
/// # Errors
///
/// [`ComposeError::Store`] when the store cannot be opened, [`ComposeError::Backend`] when the job root cannot,
/// [`ComposeError::HostRefused`] when any host probe failed, and [`ComposeError::Runner`] when [`Runner::new`]
/// refuses the configuration.
pub fn krun_jobs(
    prepared: &Prepared,
    paths: HostPaths,
) -> Result<RunnerJobs<KrunBackend<FileBlobSource>>, ComposeError> {
    let source = FileBlobSource::open(prepared.project()).map_err(ComposeError::Store)?;
    let backend = KrunBackend::new(paths, source).map_err(ComposeError::Backend)?;
    let count = backend.capabilities().host_refusals().len();
    if count > 0 {
        return Err(ComposeError::HostRefused { count });
    }
    let runner = Runner::new(
        backend,
        prepared.store().clone(),
        prepared.manifest().clone(),
        prepared.root_image(),
        prepared.judge_data(),
        prepared.judge(),
        prepared.judge_extent(),
    )
    .map_err(|error| ComposeError::Runner { what: error.what() })?;
    Ok(RunnerJobs::new(runner))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::error::Error;

    use minos_core::id::{BlobId, TreeId};
    use minos_core::lock::{BuilderText, Lock, LockMismatch, LockedJudge};
    use minos_core::manifest::{JudgeVersion, Manifest};
    use minos_core::path::RelPath;
    use minos_loop::JobFailure;
    use minos_runner::{ConfigCause, InfraCause, JudgeExtent, RunnerError};
    use minos_sandbox::JudgeData;

    use super::{failure, judge_inputs};

    type TestResult = Result<(), Box<dyn Error>>;

    /// A manifest whose `[judge] version` is `label`.
    fn manifest(label: u32) -> Result<Manifest, Box<dyn Error>> {
        let text = format!(
            r#"[project]
name = "jobs"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/value.txt"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

[commands]
build = ["minos-probe", "exit", "0"]
run = ["minos-probe", "exit", "0"]

[budget]
build = "1m"
run = "1m"
spend_usd = 1

[judge]
version = {label}
command = ["minos-probe", "scorecard", "1", "7"]

[[judge.gate]]
metric = "probe_ok"

[[judge.objective]]
metric = "probe_score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 1

[[proposer]]
kind = "single_shot"
model = "haiku"
batch = 1
"#
        );
        Ok(Manifest::parse_toml(&text)?)
    }

    /// A lock with one `[[judge]]` entry under label 1 for `tree`.
    fn lock(tree: TreeId) -> Result<Lock, Box<dyn Error>> {
        let files = BTreeMap::from([(
            RelPath::try_from("expected.txt")?,
            BlobId::of(b"expected\n"),
        )]);
        let entry = LockedJudge::new(
            1,
            tree,
            9,
            BuilderText::try_from("0.1.0".to_owned())?,
            files,
        )?;
        Ok(Lock::empty().with_judge(entry))
    }

    #[test]
    fn infra_runner_error_is_an_infra_job_failure() {
        let error = RunnerError::from(InfraCause::Launch);
        assert_eq!(
            failure(&error),
            JobFailure::Infra {
                what: InfraCause::Launch.what()
            }
        );
        assert!(failure(&error).is_infra());
    }

    #[test]
    fn config_runner_error_is_a_config_job_failure() {
        let error = RunnerError::from(ConfigCause::ScratchFull);
        assert_eq!(
            failure(&error),
            JobFailure::Config {
                what: ConfigCause::ScratchFull.what()
            }
        );
        assert!(!failure(&error).is_infra());
    }

    #[test]
    fn judge_inputs_come_from_the_label_entry() -> TestResult {
        let tree = TreeId::of(b"judge data tarball");
        let manifest = manifest(1)?;
        let (data, version, extent) = judge_inputs(&lock(tree)?, &manifest)?;
        assert_eq!(data, JudgeData::new(tree.to_blob_id()));
        assert_eq!(version, JudgeVersion::of(&tree, manifest.judge()));
        // The entry records 9 bytes in one file.
        assert_eq!(extent, JudgeExtent::new(9, 1));
        Ok(())
    }

    #[test]
    fn judge_inputs_refuse_a_label_with_no_entry() -> TestResult {
        let tree = TreeId::of(b"judge data tarball");
        let result = judge_inputs(&lock(tree)?, &manifest(2)?);
        assert_eq!(result.err(), Some(LockMismatch::NoJudge { label: 2 }));
        Ok(())
    }
}
