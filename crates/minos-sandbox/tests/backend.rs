//! The `Backend` contract with an in-file fake backend (SBX-03, SBX-06, D-17).
//!
//! The fake records every launch it receives, so the tests can show that only a cleared plan launches, that the
//! plan's launch key and hard maximum duration reach the backend, and that a candidate fault or a host-observed
//! end (`TimedOut`) comes back as a value inside `Finished`, never as an `Err`.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::Isolation;
use minos_core::limits::Limits;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::metric::Scorecard;
use minos_core::path::RelPath;
use minos_sandbox::{
    Backend, Candidate, Capabilities, Cleared, Ended, Finished, InfraError, InputDisk,
    JUDGE_LOG_PATH, Judge, JudgeData, LaunchError, LaunchKey, LaunchPlan, Machine, Refusal, Role,
    RoleKind, RunningVm, SCORECARD_PATH, SandboxSpec, preflight,
};
use tar::Header;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// The fake backend
// ---------------------------------------------------------------------------------------------------------------

/// What the fake saw at one launch: the plan's key and its hard maximum duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Launched {
    key: LaunchKey,
    max_duration: Duration,
}

/// A backend that runs nothing: every launch "ends" with `ended` and the output `stream`.
struct FakeBackend {
    capabilities: Capabilities,
    stream: Vec<u8>,
    ended: Ended,
    launched: Mutex<Vec<Launched>>,
}

impl FakeBackend {
    fn new(capabilities: Capabilities, ended: Ended, stream: Vec<u8>) -> Self {
        Self {
            capabilities,
            stream,
            ended,
            launched: Mutex::new(Vec::new()),
        }
    }

    /// Every launch so far, in order. A poisoned lock is an error, never a panic.
    fn launched(&self) -> Result<Vec<Launched>, Box<dyn Error>> {
        let guard = self
            .launched
            .lock()
            .map_err(|_| "the launch record lock is poisoned")?;
        Ok(guard.clone())
    }
}

/// A fake running VM: the clearance it was launched with, and the end it will report.
struct FakeRunning<R: Role> {
    cleared: Cleared<R>,
    stream: Vec<u8>,
    ended: Ended,
}

impl<R: Role> RunningVm<R> for FakeRunning<R> {
    fn wait(self) -> Result<Finished<R>, InfraError> {
        Ok(Finished::new(self.cleared, self.ended, self.stream))
    }
}

impl Backend for FakeBackend {
    type Running<R: Role> = FakeRunning<R>;

    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError> {
        let mut launched = self.launched.lock().map_err(|_| LaunchError::Backend {
            what: "the launch record lock is poisoned",
        })?;
        launched.push(Launched {
            key: *ok.plan().key(),
            max_duration: ok.plan().max_duration(),
        });
        Ok(FakeRunning {
            cleared: ok,
            stream: self.stream.clone(),
            ended: self.ended,
        })
    }
}

/// The runner's shape: clear `plan` against the backend's capabilities, launch it, and wait. Generic over the
/// backend, never a trait object.
fn run<B: Backend, R: Role>(
    backend: &B,
    plan: LaunchPlan<R>,
) -> Result<Finished<R>, Box<dyn Error>> {
    let cleared =
        preflight(&backend.capabilities(), plan).map_err(|refusals| format!("{refusals:?}"))?;
    Ok(backend.launch(cleared)?.wait()?)
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// A valid scorecard in the wire form.
const SCORECARD: &[u8] = br#"{"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false},{"name":"identical_output","value":1,"held_out":true}]}"#;

/// A baseline job: build 60 s, run 120 s, judge 90 s.
fn job() -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(60_000, 120_000, 90_000)?;
    let judge = JudgeVersion::from_str(&"ab".repeat(32))?;
    Ok(JobRequest::baseline(TreeId::of(b"t"), judge, 0, budget)?)
}

/// An arm64 Hardware host with room for every test plan and the given host refusals.
fn host(refusals: BTreeSet<Refusal>) -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        refusals,
    ))
}

/// The input disk of the right shape for role `R`: the baseline's candidate parts, or a judge's parts.
fn input<R: Role>() -> InputDisk {
    if R::KIND == RoleKind::Judge {
        InputDisk::judge(BlobId::of(b"guest job"), BlobId::of(b"candidate output"))
    } else {
        InputDisk::candidate(BlobId::of(b"guest job"), TreeId::of(b"t"), None)
    }
}

/// A spec of role `R` for the baseline job.
fn spec<R: Role>() -> Result<SandboxSpec<R>, Box<dyn Error>> {
    Ok(SandboxSpec::<R>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        input::<R>(),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?)
}

/// The sealed Judge plan for the baseline job, with judge data attached.
fn judge_plan() -> Result<LaunchPlan<Judge>, Box<dyn Error>> {
    Ok(spec::<Judge>()?
        .secrets(JudgeData::new(BlobId::of(b"judge data")))
        .seal())
}

/// A valid output tar holding `files` in the given order, then two end blocks.
fn tar(files: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut header = Header::new_ustar();
        header.set_path(name)?;
        header.set_size(u64::try_from(data.len())?);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        out.resize(out.len() + padding, 0);
    }
    out.resize(out.len() + 2 * BLOCK, 0);
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Flows
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn judge_flow_end_to_end() -> TestResult {
    // Arrange
    let limits = Limits::standard();
    let stream = tar(&[
        (SCORECARD_PATH, SCORECARD),
        (JUDGE_LOG_PATH, b"judged 3 cases\n"),
    ])?;
    let backend = FakeBackend::new(host(BTreeSet::new())?, Ended::Exited, stream);
    let plan = judge_plan()?;
    let key = *plan.key();

    // Act
    let finished = run(&backend, plan)?;

    // Assert
    assert_eq!(finished.ended(), Ended::Exited);
    assert_eq!(finished.key(), &key);
    let mut output = finished.into_output()?;
    let log = output
        .take(&RelPath::try_from(JUDGE_LOG_PATH)?)
        .ok_or("the judge log is missing")?;
    assert_eq!(
        log.into_log_excerpt(4096).text().as_str(),
        "judged 3 cases\n"
    );
    assert_eq!(
        output.into_scorecard(&limits)?,
        Scorecard::parse_json(SCORECARD, &limits)?
    );
    Ok(())
}

#[test]
fn candidate_flow_end_to_end() -> TestResult {
    // Arrange
    let first = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-a\n+b\n";
    let second = "--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-c\n+d\n";
    let stream = tar(&[
        ("diffs/0001.diff", first.as_bytes()),
        ("diffs/0002.diff", second.as_bytes()),
        ("out/result.json", b"{}"),
    ])?;
    let backend = FakeBackend::new(host(BTreeSet::new())?, Ended::Exited, stream);
    let plan = spec::<Candidate>()?.seal();
    let key = *plan.key();

    // Act
    let finished = run(&backend, plan)?;

    // Assert
    assert_eq!(finished.ended(), Ended::Exited);
    assert_eq!(finished.key(), &key);
    let texts = finished
        .into_output()?
        .into_diff_texts(&Limits::standard())?;
    assert_eq!(texts, vec![first.to_owned(), second.to_owned()]);
    Ok(())
}

#[test]
fn refused_plan_never_launches() -> TestResult {
    // Arrange
    let refusals = BTreeSet::from([Refusal::KernelMissing]);
    let backend = FakeBackend::new(host(refusals)?, Ended::Exited, tar(&[])?);

    // Act
    let result = run(&backend, spec::<Candidate>()?.seal());

    // Assert
    assert!(result.is_err(), "a refused plan produced a Finished");
    assert!(
        preflight(&backend.capabilities(), spec::<Candidate>()?.seal()).is_err(),
        "a refused plan was cleared"
    );
    assert_eq!(backend.launched()?, Vec::new());
    Ok(())
}

#[test]
fn launch_key_is_the_idempotency_token() -> TestResult {
    // Arrange
    let first = spec::<Candidate>()?.seal();
    let second = spec::<Candidate>()?.seal();
    let judge = judge_plan()?;
    let backend = FakeBackend::new(host(BTreeSet::new())?, Ended::Exited, tar(&[])?);
    let key = *first.key();

    // Act
    let finished = run(&backend, first)?;

    // Assert
    assert_eq!(&key, second.key());
    assert_ne!(&key, judge.key());
    assert_eq!(finished.key(), &key);
    let launched = backend.launched()?;
    assert_eq!(launched.len(), 1);
    assert_eq!(launched.first().map(|l| l.key), Some(key));
    Ok(())
}

#[test]
fn timed_out_is_a_host_fact() -> TestResult {
    // Arrange
    let stream = tar(&[("diffs/0001.diff", b"partial")])?;
    let backend = FakeBackend::new(host(BTreeSet::new())?, Ended::TimedOut, stream);

    // Act
    let finished = run(&backend, spec::<Candidate>()?.seal())?;

    // Assert
    assert_eq!(finished.ended(), Ended::TimedOut);
    let output = finished.into_output()?;
    assert_eq!(output.entry_count(), 1);
    assert_eq!(output.content_bytes(), 7);
    Ok(())
}

#[test]
fn every_launch_carries_a_hard_limit() -> TestResult {
    // Arrange
    let candidate = spec::<Candidate>()?.seal();
    let judge = judge_plan()?;
    let expected = vec![
        Launched {
            key: *candidate.key(),
            max_duration: candidate.max_duration(),
        },
        Launched {
            key: *judge.key(),
            max_duration: judge.max_duration(),
        },
    ];
    let backend = FakeBackend::new(host(BTreeSet::new())?, Ended::Exited, tar(&[])?);

    // Act
    run(&backend, candidate)?;
    run(&backend, judge)?;

    // Assert: build 60 s + run 120 s + 30 s boot, and judge 90 s + 30 s boot.
    let launched = backend.launched()?;
    assert_eq!(launched, expected);
    assert_eq!(
        launched.iter().map(|l| l.max_duration).collect::<Vec<_>>(),
        vec![Duration::from_secs(210), Duration::from_mins(2)]
    );
    Ok(())
}
