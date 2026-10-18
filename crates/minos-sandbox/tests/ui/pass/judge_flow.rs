// The whole judge flow compiles and runs: spec, secrets, seal, preflight, launch, wait, into_output, take,
// into_log_excerpt, and into_scorecard, against an in-file fake backend. It proves the compile-fail fixtures'
// types and calls are sound, so a fail case cannot pass for the wrong reason (SBX-02).
use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
use minos_core::limits::Limits;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::metric::Scorecard;
use minos_core::path::RelPath;
use minos_sandbox::{
    Backend, Capabilities, Cleared, Ended, Finished, InfraError, InputDisk, JUDGE_LOG_PATH, Judge,
    JudgeData, LaunchError, Machine, Role, RunningVm, SCORECARD_PATH, SandboxSpec, preflight,
};
use tar::Header;

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// A valid scorecard in the wire form.
const SCORECARD: &[u8] =
    br#"{"v":1,"metrics":[{"name":"identical_output","value":1,"held_out":false}]}"#;

/// A backend that runs nothing: every launch exits with `stream` as its output.
struct FakeBackend {
    capabilities: Capabilities,
    stream: Vec<u8>,
}

/// A fake running VM holding its clearance and its output.
struct FakeRunning<R: Role> {
    cleared: Cleared<R>,
    stream: Vec<u8>,
}

impl<R: Role> RunningVm<R> for FakeRunning<R> {
    fn wait(self) -> Result<Finished<R>, InfraError> {
        Ok(Finished::new(
            self.cleared,
            Ended::Exited,
            self.stream,
            Duration::ZERO,
            Vec::new(),
        ))
    }
}

impl Backend for FakeBackend {
    type Running<R: Role> = FakeRunning<R>;

    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError> {
        Ok(FakeRunning {
            cleared: ok,
            stream: self.stream.clone(),
        })
    }
}

/// A valid output tar holding `files`, then two end blocks.
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

fn main() -> Result<(), Box<dyn Error>> {
    let limits = Limits::standard();
    let backend = FakeBackend {
        capabilities: Capabilities::new(
            BackendKind::Krun,
            Arch::Arm64,
            Machine::new(8, 16_384)?,
            3_600_000,
            Isolation::Hardware,
            BTreeSet::new(),
        )
        .with_layers(BTreeSet::from([
            ConfinementLayer::Seatbelt,
            ConfinementLayer::Rlimits,
        ])),
        stream: tar(&[(SCORECARD_PATH, SCORECARD), (JUDGE_LOG_PATH, b"ok\n")])?,
    };
    let job = JobRequest::baseline(
        TreeId::of(b"t"),
        JudgeVersion::from_str(&"ab".repeat(32))?,
        0,
        JobBudget::new(60_000, 120_000, 90_000)?,
    )?;

    let plan = SandboxSpec::<Judge>::new(
        &job,
        ImageDigest::of(b"root image"),
        InputDisk::judge(BlobId::of(b"guest job"), BlobId::of(b"candidate output")),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?
    .secrets(JudgeData::new(BlobId::of(b"judge data")))
    .seal();
    let key = *plan.key();
    let cleared =
        preflight(&backend.capabilities(), plan).map_err(|refusals| format!("{refusals:?}"))?;
    let finished = backend.launch(cleared)?.wait()?;
    if finished.ended() != Ended::Exited || finished.key() != &key {
        return Err("the launch did not end as the fake reported".into());
    }

    let mut output = finished.into_output()?;
    let log = output
        .take(&RelPath::try_from(JUDGE_LOG_PATH)?)
        .ok_or("the judge log is missing")?;
    if log.into_log_excerpt(4096).text().as_str() != "ok\n" {
        return Err("the judge log changed".into());
    }
    if output.into_scorecard(&limits)? != Scorecard::parse_json(SCORECARD, &limits)? {
        return Err("the scorecard changed".into());
    }
    Ok(())
}
