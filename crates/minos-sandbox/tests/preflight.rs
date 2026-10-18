//! Every preflight refusal, alone and together, and the admitted path.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::Isolation;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_sandbox::{
    Candidate, Capabilities, InputDisk, Judge, JudgeData, LaunchPlan, Machine, Refusal, Role,
    RoleKind, SandboxSpec, preflight,
};

type TestResult = Result<(), Box<dyn Error>>;

/// A baseline job: build 60 s, run 120 s, judge 90 s.
fn job() -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(60_000, 120_000, 90_000)?;
    let judge = JudgeVersion::from_str(&"ab".repeat(32))?;
    Ok(JobRequest::baseline(TreeId::of(b"t"), judge, 0, budget)?)
}

/// The guest job blob every test input names.
fn job_blob() -> BlobId {
    BlobId::of(b"guest job")
}

/// The candidate output blob a judge input names.
fn candidate_out() -> BlobId {
    BlobId::of(b"candidate output")
}

/// The input of the right shape for role `R`: the baseline's candidate parts, or a judge's parts.
fn input<R: Role>() -> InputDisk {
    if R::KIND == RoleKind::Judge {
        InputDisk::judge(job_blob(), candidate_out())
    } else {
        InputDisk::candidate(job_blob(), TreeId::of(b"t"), None)
    }
}

/// A sealed plan in role `R` for 2 vCPUs and 512 MiB on arm64.
fn plan<R: Role>() -> Result<LaunchPlan<R>, Box<dyn Error>> {
    let spec = SandboxSpec::<R>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        input::<R>(),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?;
    Ok(spec.seal())
}

/// A sealed Judge plan carrying `data`.
fn judge_plan(data: JudgeData) -> Result<LaunchPlan<Judge>, Box<dyn Error>> {
    let spec = SandboxSpec::<Judge>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        input::<Judge>(),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?;
    Ok(spec.secrets(data).seal())
}

/// An arm64 Hardware host with room for every test plan and no host refusals.
fn host() -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        BTreeSet::new(),
    ))
}

/// The refusals of `plan` on `host`, or an error when it was cleared.
fn refusals<R: Role>(host: &Capabilities, plan: LaunchPlan<R>) -> Result<Vec<Refusal>, String> {
    match preflight(host, plan) {
        Ok(_) => Err("the plan was cleared".to_owned()),
        Err(refusals) => Ok(refusals),
    }
}

#[test]
fn admitted_plan_is_cleared() -> TestResult {
    let plan = plan::<Candidate>()?;
    let key = *plan.key();
    let cleared = preflight(&host()?, plan).map_err(|refusals| format!("{refusals:?}"))?;
    assert_eq!(*cleared.plan().key(), key);

    let data = JudgeData::new(BlobId::of(b"judge data"));
    let judge = judge_plan(data)?;
    let cleared = preflight(&host()?, judge).map_err(|refusals| format!("{refusals:?}"))?;
    assert_eq!(cleared.plan().judge_data(), Some(&data));
    Ok(())
}

#[test]
fn each_mismatch_is_refused() -> TestResult {
    let none = BTreeSet::new;

    let x86 = Capabilities::new(
        Arch::X86_64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        none(),
    );
    assert_eq!(
        refusals(&x86, plan::<Candidate>()?)?,
        vec![Refusal::ArchMismatch {
            plan: Arch::Arm64,
            host: Arch::X86_64
        }]
    );

    let one_cpu = Capabilities::new(
        Arch::Arm64,
        Machine::new(1, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        none(),
    );
    assert_eq!(
        refusals(&one_cpu, plan::<Candidate>()?)?,
        vec![Refusal::TooManyVcpus { plan: 2, host: 1 }]
    );

    let small = Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 256)?,
        3_600_000,
        Isolation::Hardware,
        none(),
    );
    assert_eq!(
        refusals(&small, plan::<Candidate>()?)?,
        vec![Refusal::TooMuchMemory {
            plan: 512,
            host: 256
        }]
    );

    let short = Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        200_000,
        Isolation::Hardware,
        none(),
    );
    assert_eq!(
        refusals(&short, plan::<Candidate>()?)?,
        vec![Refusal::DurationOverHost {
            plan_ms: 210_000,
            host_ms: 200_000
        }]
    );

    let weak = Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::WeakProcess,
        none(),
    );
    assert_eq!(
        refusals(&weak, plan::<Candidate>()?)?,
        vec![Refusal::WeakIsolation]
    );
    Ok(())
}

#[test]
fn judge_without_data_is_refused() -> TestResult {
    let found = refusals(&host()?, plan::<Judge>()?)?;
    assert!(found.contains(&Refusal::JudgeDataMissing));
    assert_eq!(found, vec![Refusal::JudgeDataMissing]);
    Ok(())
}

#[test]
fn preflight_judge_data_is_any_input_part() -> TestResult {
    for part in [job_blob(), candidate_out()] {
        let found = refusals(&host()?, judge_plan(JudgeData::new(part))?)?;
        assert_eq!(found, vec![Refusal::JudgeDataIsInput]);
    }
    Ok(())
}

#[test]
fn host_refusals_refuse_every_plan() -> TestResult {
    let host_refusals = BTreeSet::from([Refusal::SocketPathTooLong, Refusal::EntitlementMissing]);
    let broken = Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        host_refusals,
    );
    let expected = vec![Refusal::EntitlementMissing, Refusal::SocketPathTooLong];
    assert_eq!(refusals(&broken, plan::<Candidate>()?)?, expected);
    let data = JudgeData::new(BlobId::of(b"judge data"));
    assert_eq!(refusals(&broken, judge_plan(data)?)?, expected);
    Ok(())
}

#[test]
fn refusals_are_sorted_and_unique() -> TestResult {
    let host_refusals = BTreeSet::from([
        Refusal::KernelHashMismatch,
        Refusal::HypervisorUnsupported,
        Refusal::KernelMissing,
    ]);
    let everything_wrong = Capabilities::new(
        Arch::X86_64,
        Machine::new(1, 128)?,
        1_000,
        Isolation::WeakProcess,
        host_refusals,
    );
    let found = refusals(&everything_wrong, plan::<Judge>()?)?;
    let mut sorted = found.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(found, sorted);
    assert_eq!(
        found,
        vec![
            Refusal::ArchMismatch {
                plan: Arch::Arm64,
                host: Arch::X86_64
            },
            Refusal::TooManyVcpus { plan: 2, host: 1 },
            Refusal::TooMuchMemory {
                plan: 512,
                host: 128
            },
            Refusal::DurationOverHost {
                plan_ms: 120_000,
                host_ms: 1_000
            },
            Refusal::WeakIsolation,
            Refusal::JudgeDataMissing,
            Refusal::KernelMissing,
            Refusal::KernelHashMismatch,
            Refusal::HypervisorUnsupported,
        ]
    );
    Ok(())
}
