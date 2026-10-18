//! Launch key golden vectors and properties, the role tags they hash, and the spec and plan bounds.
//!
//! Every hex literal below was computed outside the code under test with `/usr/bin/shasum -a 256` and `xxd`; the
//! shell recipe sits in a comment above each literal so a reviewer can recompute it. These are plain `assert_eq!`
//! checks on hex literals and never snapshot assertions, so they cannot be auto-accepted.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;

use minos_core::id::{BlobId, ExperimentId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::manifest::{Arch, JudgeVersion};
use minos_sandbox::{
    Candidate, InputDisk, Judge, JudgeData, LaunchKey, Machine, Role, RoleKind, SandboxSpec,
    SpecError,
};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// `ExperimentId::baseline(&TreeId::of(b"t"))`, pinned in `minos-core/tests/golden_ids.rs`.
const BASELINE_OF_T: &str = "6fd8be380eff4cb315d50a3f594a1baa7241164600eb22e68978748be04a75cb";

fn baseline_of_t() -> ExperimentId {
    ExperimentId::baseline(&TreeId::of(b"t"))
}

fn judge_ab() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

#[test]
fn launch_key_golden_vectors() -> TestResult {
    let exp = baseline_of_t();
    assert_eq!(exp.to_hex(), BASELINE_OF_T);
    let judge = judge_ab()?;
    // EXP=6fd8be380eff4cb315d50a3f594a1baa7241164600eb22e68978748be04a75cb; J=$(printf 'ab%.0s' $(seq 32))
    // { printf 'minos/launch/v1\0'; printf '%s' "$EXP" | xxd -r -p; printf '%016x' 1 | xxd -r -p;
    //   printf '%s' "$J" | xxd -r -p; printf '%016x' 0 | xxd -r -p; } | shasum -a 256
    assert_eq!(
        LaunchKey::of::<Candidate>(&exp, &judge, 0).to_hex(),
        "4747473e3894709a175a5f57f4406347fdc40c5b426b6fcb78ecb653c71534fb"
    );
    // { printf 'minos/launch/v1\0'; printf '%s' "$EXP" | xxd -r -p; printf '%016x' 2 | xxd -r -p;
    //   printf '%s' "$J" | xxd -r -p; printf '%016x' 0 | xxd -r -p; } | shasum -a 256
    assert_eq!(
        LaunchKey::of::<Judge>(&exp, &judge, 0).to_hex(),
        "a208611f3b16fd001a6ea2a726d520b2b7cea4c5ba27d75ca56fed2831169d05"
    );
    Ok(())
}

#[test]
fn role_tags_are_fixed() {
    assert_eq!(RoleKind::Candidate.tag(), 1);
    assert_eq!(RoleKind::Judge.tag(), 2);
    assert_eq!(Candidate::KIND, RoleKind::Candidate);
    assert_eq!(Judge::KIND, RoleKind::Judge);
}

#[test]
fn launch_key_prints_hex() -> TestResult {
    let key = LaunchKey::of::<Candidate>(&baseline_of_t(), &judge_ab()?, 0);
    let hex = key.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    );
    assert_eq!(key.to_string(), hex);
    assert!(format!("{key:?}").contains(&hex));
    assert_eq!(base16ct::lower::encode_string(key.as_bytes()), hex);
    Ok(())
}

/// A judge version over 32 arbitrary bytes, through its only public parser.
fn judge_of(bytes: &[u8; 32]) -> Result<JudgeVersion, TestCaseError> {
    JudgeVersion::from_str(&base16ct::lower::encode_string(bytes))
        .map_err(|error| TestCaseError::fail(error.to_string()))
}

proptest! {
    #[test]
    fn launch_key_sensitive_to_every_field(
        tree in proptest::collection::vec(any::<u8>(), 0..64),
        other_tree in proptest::collection::vec(any::<u8>(), 0..64),
        judge_bytes in any::<[u8; 32]>(),
        other_judge_bytes in any::<[u8; 32]>(),
        trial in any::<u32>(),
        other_trial in any::<u32>(),
    ) {
        let exp = ExperimentId::baseline(&TreeId::of(&tree));
        let judge = judge_of(&judge_bytes)?;
        let key = LaunchKey::of::<Candidate>(&exp, &judge, trial);
        prop_assert_eq!(key, LaunchKey::of::<Candidate>(&exp, &judge, trial));
        prop_assert_ne!(key, LaunchKey::of::<Judge>(&exp, &judge, trial));
        if tree != other_tree {
            let other_exp = ExperimentId::baseline(&TreeId::of(&other_tree));
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&other_exp, &judge, trial));
        }
        if judge_bytes != other_judge_bytes {
            let other_judge = judge_of(&other_judge_bytes)?;
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&exp, &other_judge, trial));
        }
        if trial != other_trial {
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&exp, &judge, other_trial));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// SandboxSpec, seal, and LaunchPlan
// ---------------------------------------------------------------------------------------------------------------

/// A baseline job for trial 0 of the tree `t` under the judge `ab..ab`.
fn job(build_ms: u64, run_ms: u64, judge_ms: u64) -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(build_ms, run_ms, judge_ms)?;
    Ok(JobRequest::baseline(
        TreeId::of(b"t"),
        judge_ab()?,
        0,
        budget,
    )?)
}

fn root() -> ImageDigest {
    ImageDigest::of(b"root image")
}

/// The guest job blob every test input names.
fn job_blob() -> BlobId {
    BlobId::of(b"guest job")
}

/// The baseline candidate input of the tree `t`: the job blob and the base tree, no diff.
fn candidate_input() -> InputDisk {
    InputDisk::candidate(job_blob(), TreeId::of(b"t"), None)
}

/// A judge input: the job blob and the candidate's output blob.
fn judge_input() -> InputDisk {
    InputDisk::judge(job_blob(), BlobId::of(b"candidate output"))
}

/// The input of the right shape for role `R`.
fn input<R: Role>() -> InputDisk {
    if R::KIND == RoleKind::Judge {
        judge_input()
    } else {
        candidate_input()
    }
}

fn machine() -> Result<Machine, SpecError> {
    Machine::new(2, 512)
}

#[test]
fn machine_bounds() -> TestResult {
    assert_eq!(Machine::new(0, 512), Err(SpecError::Vcpus { vcpus: 0 }));
    assert_eq!(Machine::new(9, 512), Err(SpecError::Vcpus { vcpus: 9 }));
    assert_eq!(Machine::new(2, 127), Err(SpecError::Memory { mib: 127 }));
    assert_eq!(
        Machine::new(2, 16_385),
        Err(SpecError::Memory { mib: 16_385 })
    );
    let low = Machine::new(1, 128)?;
    assert_eq!((low.vcpus(), low.memory_mib()), (1, 128));
    let high = Machine::new(8, 16_384)?;
    assert_eq!((high.vcpus(), high.memory_mib()), (8, 16_384));
    Ok(())
}

#[test]
fn candidate_duration_is_build_plus_run_plus_boot() -> TestResult {
    let job = job(60_000, 120_000, 120_000)?;
    let spec =
        SandboxSpec::<Candidate>::new(&job, root(), input::<Candidate>(), machine()?, Arch::Arm64)?;
    let plan = spec.seal();
    assert_eq!(plan.max_duration_ms(), 210_000);
    assert_eq!(plan.max_duration(), Duration::from_secs(210));
    Ok(())
}

#[test]
fn judge_duration_is_judge_plus_boot() -> TestResult {
    let job = job(60_000, 120_000, 120_000)?;
    let spec = SandboxSpec::<Judge>::new(&job, root(), input::<Judge>(), machine()?, Arch::Arm64)?;
    assert_eq!(spec.seal().max_duration_ms(), 150_000);
    Ok(())
}

#[test]
fn over_ceiling_is_refused_not_clamped() -> TestResult {
    // build + run + 30 s = 3_630_000 ms, over the 3_600_000 ms launcher ceiling.
    let job = job(1_800_000, 1_800_000, 120_000)?;
    let refused =
        SandboxSpec::<Candidate>::new(&job, root(), input::<Candidate>(), machine()?, Arch::Arm64);
    assert_eq!(
        refused.err(),
        Some(SpecError::DurationOverCeiling { ms: 3_630_000 })
    );
    let judge = SandboxSpec::<Judge>::new(&job, root(), input::<Judge>(), machine()?, Arch::Arm64)?;
    assert_eq!(judge.seal().max_duration_ms(), 150_000);

    // A one-hour judge plus the boot allowance is over the ceiling too.
    let long_judge = self::job(60_000, 60_000, 3_600_000)?;
    let refused = SandboxSpec::<Judge>::new(
        &long_judge,
        root(),
        input::<Judge>(),
        machine()?,
        Arch::Arm64,
    );
    assert_eq!(
        refused.err(),
        Some(SpecError::DurationOverCeiling { ms: 3_630_000 })
    );

    // Exactly at the ceiling is accepted.
    let at_ceiling = self::job(1_785_000, 1_785_000, 60_000)?;
    let plan = SandboxSpec::<Candidate>::new(
        &at_ceiling,
        root(),
        input::<Candidate>(),
        machine()?,
        Arch::Arm64,
    )?
    .seal();
    assert_eq!(plan.max_duration_ms(), 3_600_000);
    Ok(())
}

#[test]
fn seal_preserves_every_field() -> TestResult {
    let job = job(60_000, 120_000, 90_000)?;
    let machine = Machine::new(4, 2048)?;
    let plan =
        SandboxSpec::<Candidate>::new(&job, root(), input::<Candidate>(), machine, Arch::X86_64)?
            .seal();
    assert_eq!(
        *plan.key(),
        LaunchKey::of::<Candidate>(&job.exp(), &job.judge(), job.trial())
    );
    assert_eq!(*plan.root(), root());
    assert_eq!(*plan.input(), candidate_input());
    assert_eq!(*plan.budget(), job.budget());
    assert_eq!(*plan.machine(), machine);
    assert_eq!(plan.arch(), Arch::X86_64);
    assert_eq!(plan.role(), RoleKind::Candidate);

    let judge_plan =
        SandboxSpec::<Judge>::new(&job, root(), input::<Judge>(), machine, Arch::Arm64)?.seal();
    assert_eq!(
        *judge_plan.key(),
        LaunchKey::of::<Judge>(&job.exp(), &job.judge(), job.trial())
    );
    assert_eq!(judge_plan.role(), RoleKind::Judge);
    assert_ne!(judge_plan.key(), plan.key());
    Ok(())
}

#[test]
fn only_judge_secrets_set_judge_data() -> TestResult {
    let job = job(60_000, 120_000, 90_000)?;
    let data = JudgeData::new(BlobId::of(b"judge data"));
    let with = SandboxSpec::<Judge>::new(&job, root(), input::<Judge>(), machine()?, Arch::Arm64)?
        .secrets(data)
        .seal();
    assert_eq!(with.judge_data(), Some(&data));
    assert_eq!(*data.blob(), BlobId::of(b"judge data"));

    let without =
        SandboxSpec::<Judge>::new(&job, root(), input::<Judge>(), machine()?, Arch::Arm64)?.seal();
    assert_eq!(without.judge_data(), None);

    let candidate =
        SandboxSpec::<Candidate>::new(&job, root(), input::<Candidate>(), machine()?, Arch::Arm64)?
            .seal();
    assert_eq!(candidate.judge_data(), None);
    Ok(())
}

/// The sandbox restates launcher bounds it cannot import. The first four are pinned to the same literals by
/// `launcher_bounds_are_pinned_for_minos_sandbox` in `crates/minos-vmm/src/config.rs`, and `BOOT_ALLOWANCE_MS`
/// by `host_deadline_grace_is_pinned_for_minos_sandbox` in `crates/minos-backend-krun/src/supervise.rs`, so
/// changing either side fails the gate until both change (review WR-02).
#[test]
fn launcher_bounds_match_minos_vmm() {
    assert_eq!(minos_sandbox::spec::MAX_VCPUS, 8);
    assert_eq!(minos_sandbox::spec::MIN_MEMORY_MIB, 128);
    assert_eq!(minos_sandbox::spec::MAX_MEMORY_MIB, 16_384);
    assert_eq!(minos_sandbox::spec::MAX_LAUNCH_MS, 3_600_000);
    assert_eq!(minos_sandbox::spec::BOOT_ALLOWANCE_MS, 30_000);
}

/// The machine bounds are core's (05-CONTEXT D-06): the sandbox re-exports them, so the manifest's range check,
/// the plan, and the launcher pins share one definition.
#[test]
fn bounds_come_from_core() {
    assert_eq!(
        minos_sandbox::spec::MAX_VCPUS,
        minos_core::limits::MAX_VCPUS
    );
    assert_eq!(
        minos_sandbox::spec::MIN_MEMORY_MIB,
        minos_core::limits::MIN_MEMORY_MIB
    );
    assert_eq!(
        minos_sandbox::spec::MAX_MEMORY_MIB,
        minos_core::limits::MAX_MEMORY_MIB
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The input disk's parts (05-CONTEXT D-15)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn input_disk_candidate_shape() {
    let base = TreeId::of(b"base tree");
    let diff = BlobId::of(b"canonical diff");
    let input = InputDisk::candidate(job_blob(), base, Some(diff));
    assert_eq!(input.role(), RoleKind::Candidate);
    assert_eq!(input.job(), job_blob());
    assert_eq!(input.base(), Some(base));
    assert_eq!(input.diff(), Some(diff));
    assert_eq!(input.candidate_out(), None);
    assert_eq!(
        input.blobs(),
        BTreeSet::from([job_blob(), base.to_blob_id(), diff])
    );

    let baseline = InputDisk::candidate(job_blob(), base, None);
    assert_eq!(baseline.diff(), None);
    assert_eq!(
        baseline.blobs(),
        BTreeSet::from([job_blob(), base.to_blob_id()])
    );
}

#[test]
fn input_disk_judge_shape() {
    let out = BlobId::of(b"candidate output");
    let input = InputDisk::judge(job_blob(), out);
    assert_eq!(input.role(), RoleKind::Judge);
    assert_eq!(input.job(), job_blob());
    assert_eq!(input.base(), None);
    assert_eq!(input.diff(), None);
    assert_eq!(input.candidate_out(), Some(out));
    assert_eq!(input.blobs(), BTreeSet::from([job_blob(), out]));
}

#[test]
fn spec_refuses_wrong_role_input() -> TestResult {
    let job = job(60_000, 120_000, 90_000)?;
    let refused =
        SandboxSpec::<Candidate>::new(&job, root(), judge_input(), machine()?, Arch::Arm64);
    assert_eq!(
        refused.err(),
        Some(SpecError::InputRole {
            role: RoleKind::Judge
        })
    );
    let refused =
        SandboxSpec::<Judge>::new(&job, root(), candidate_input(), machine()?, Arch::Arm64);
    assert_eq!(
        refused.err(),
        Some(SpecError::InputRole {
            role: RoleKind::Candidate
        })
    );
    Ok(())
}

#[test]
fn spec_refuses_mismatched_parts() -> TestResult {
    let job = job(60_000, 120_000, 90_000)?;
    let other_base = InputDisk::candidate(job_blob(), TreeId::of(b"other tree"), None);
    let refused = SandboxSpec::<Candidate>::new(&job, root(), other_base, machine()?, Arch::Arm64);
    assert_eq!(refused.err(), Some(SpecError::InputMismatch));

    // The baseline job has no diff, so an input naming one disagrees with it.
    let extra_diff = InputDisk::candidate(job_blob(), job.base(), Some(BlobId::of(b"a diff")));
    let refused = SandboxSpec::<Candidate>::new(&job, root(), extra_diff, machine()?, Arch::Arm64);
    assert_eq!(refused.err(), Some(SpecError::InputMismatch));

    // The input checks come before the duration check.
    let over = self::job(1_800_000, 1_800_000, 120_000)?;
    let refused = SandboxSpec::<Candidate>::new(&over, root(), other_base, machine()?, Arch::Arm64);
    assert_eq!(refused.err(), Some(SpecError::InputMismatch));
    let refused =
        SandboxSpec::<Candidate>::new(&over, root(), judge_input(), machine()?, Arch::Arm64);
    assert_eq!(
        refused.err(),
        Some(SpecError::InputRole {
            role: RoleKind::Judge
        })
    );
    Ok(())
}
