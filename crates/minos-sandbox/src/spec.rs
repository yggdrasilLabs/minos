//! Sandbox specs and sealed launch plans (SBX-01, SBX-03, SBX-06).
//!
//! A [`SandboxSpec`] is built from a validated [`JobRequest`] and carries IDs and digests only, never bytes: the
//! launch key, the root image digest, the input disk blob, the stage budgets, the machine size, the architecture,
//! and the hard maximum duration. Policy is enforced while the spec is built, and [`SandboxSpec::seal`] is the
//! only constructor of a [`LaunchPlan`], the only thing a backend ever receives.
//!
//! Only `SandboxSpec<Judge>` has a method that attaches judge data ([`SandboxSpec::secrets`]). No role has a
//! method or a field that attaches a network device: no sandbox gets one (CLAUDE.md "Sandbox roles").

use std::marker::PhantomData;
use std::time::Duration;

use minos_core::id::{BlobId, ImageDigest};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::manifest::Arch;

use crate::key::LaunchKey;
use crate::role::{Judge, Role, RoleKind};

// The machine and duration bounds below restate `crates/minos-vmm/src/config.rs` (`MAX_VCPUS`, `MIN_MEMORY_MIB`,
// `MAX_MEMORY_MIB`, `MAX_DURATION_MS`), and `BOOT_ALLOWANCE_MS` restates `HOST_DEADLINE_GRACE` in
// `crates/minos-backend-krun/src/supervise.rs`. A plan that passes here must also pass the launcher's own config
// validation, but minos-vmm links no internal crate, so the bounds cannot share a constant. Instead each side
// pins the same literals in a test: `launcher_bounds_match_minos_vmm` in `crates/minos-sandbox/tests/spec.rs`,
// `launcher_bounds_are_pinned_for_minos_sandbox` in `crates/minos-vmm/src/config.rs`, and
// `host_deadline_grace_is_pinned_for_minos_sandbox` in `crates/minos-backend-krun/src/supervise.rs`. Changing one
// side fails the gate until the other side and both tests change with it (review WR-02).

/// Most vCPUs a sandbox may have: the launcher refuses more.
pub const MAX_VCPUS: u8 = 8;
/// Least guest memory a sandbox may have, in MiB: the guest kernel and `/init` need at least this much.
pub const MIN_MEMORY_MIB: u16 = 128;
/// Most guest memory a sandbox may have, in MiB: the launcher refuses more.
pub const MAX_MEMORY_MIB: u16 = 16_384;
/// The launcher's ceiling on a VM's hard maximum duration, in milliseconds (one hour). A plan over it is refused,
/// never clamped, because a clamped plan would time out a job its budgets allow.
pub const MAX_LAUNCH_MS: u64 = 3_600_000;
/// Time allowed for the VM to boot, inside the hard maximum, in milliseconds. The same 30 s figure as
/// `HOST_DEADLINE_GRACE` in `crates/minos-backend-krun/src/supervise.rs`.
pub const BOOT_ALLOWANCE_MS: u64 = 30_000;

/// Why a spec could not be built.
///
/// A `SpecError` is a configuration error. It is never an `Outcome`, because the candidate did nothing, and it is
/// never retried as an infrastructure failure, because a retry builds the same spec and fails the same way.
/// [`SpecError::DurationOverCeiling`] follows from the manifest's budgets alone: `limits::MAX_STAGE_MS` lets each
/// of build, run, and judge reach one hour, so a manifest-valid build plus run plus [`BOOT_ALLOWANCE_MS`], or a
/// one-hour judge plus it, exceeds [`MAX_LAUNCH_MS`], and it would fail every experiment of the run alike.
///
/// Handlers: the Phase 7 runner builds the baseline's Candidate and Judge specs before generation 0 and refuses
/// to start the run on any `SpecError`; Phase 8's `minos check` (CLI-05) reports it with the budgets to lower.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    /// The vCPU count is outside 1 to [`MAX_VCPUS`].
    #[error("a sandbox has 1 to {MAX_VCPUS} vCPUs, not {vcpus}")]
    Vcpus {
        /// The refused vCPU count.
        vcpus: u8,
    },
    /// The memory size is outside [`MIN_MEMORY_MIB`] to [`MAX_MEMORY_MIB`].
    #[error("a sandbox has {MIN_MEMORY_MIB} to {MAX_MEMORY_MIB} MiB of memory, not {mib}")]
    Memory {
        /// The refused memory size, in MiB.
        mib: u16,
    },
    /// The stage budgets plus [`BOOT_ALLOWANCE_MS`] exceed [`MAX_LAUNCH_MS`].
    #[error(
        "the sandbox needs {ms} ms, over the {MAX_LAUNCH_MS} ms launch ceiling; lower the stage budgets"
    )]
    DurationOverCeiling {
        /// The computed hard maximum, in milliseconds.
        ms: u64,
    },
}

/// The size of a sandbox VM: 1 to [`MAX_VCPUS`] vCPUs and [`MIN_MEMORY_MIB`] to [`MAX_MEMORY_MIB`] MiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    vcpus: u8,
    memory_mib: u16,
}

impl Machine {
    /// A machine of `vcpus` vCPUs and `memory_mib` MiB.
    ///
    /// # Errors
    ///
    /// [`SpecError::Vcpus`] when `vcpus` is outside 1 to [`MAX_VCPUS`], and [`SpecError::Memory`] when
    /// `memory_mib` is outside [`MIN_MEMORY_MIB`] to [`MAX_MEMORY_MIB`].
    pub const fn new(vcpus: u8, memory_mib: u16) -> Result<Self, SpecError> {
        if vcpus < 1 || vcpus > MAX_VCPUS {
            return Err(SpecError::Vcpus { vcpus });
        }
        if memory_mib < MIN_MEMORY_MIB || memory_mib > MAX_MEMORY_MIB {
            return Err(SpecError::Memory { mib: memory_mib });
        }
        Ok(Self { vcpus, memory_mib })
    }

    /// The vCPU count.
    #[must_use]
    pub const fn vcpus(&self) -> u8 {
        self.vcpus
    }

    /// The memory size, in MiB.
    #[must_use]
    pub const fn memory_mib(&self) -> u16 {
        self.memory_mib
    }
}

/// The per-job read-only input disk, by blob ID.
///
/// Provenance rule, owned by the runner: the blob is the disk the host wrote from the base tarball, the canonical
/// diff, and the job JSON, without unpacking or applying anything. It is publicly constructible in Phase 4;
/// Phase 5, which owns the input-disk format, may restrict this constructor. `preflight` refuses a Judge plan
/// whose input disk and judge data are the same blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputDisk(BlobId);

impl InputDisk {
    /// The input disk stored as `blob`.
    #[must_use]
    pub const fn new(blob: BlobId) -> Self {
        Self(blob)
    }

    /// The blob ID of the disk.
    #[must_use]
    pub const fn blob(&self) -> &BlobId {
        &self.0
    }
}

/// The judge's private data, by blob ID, mounted read-only in a Judge sandbox only.
///
/// Provenance rule, owned by the runner: the blob is judge data the user pushed, and it is never given to a
/// Candidate; the type system enforces that half, because only `SandboxSpec<Judge>` has
/// [`SandboxSpec::secrets`]. `preflight` refuses a Judge plan whose judge data and input disk are the same blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeData(BlobId);

impl JudgeData {
    /// The judge data stored as `blob`.
    #[must_use]
    pub const fn new(blob: BlobId) -> Self {
        Self(blob)
    }

    /// The blob ID of the judge data.
    #[must_use]
    pub const fn blob(&self) -> &BlobId {
        &self.0
    }
}

/// What one sandbox launch will be, before it is sealed.
///
/// Every field is an ID, a digest, or a bound; no bytes. Only `SandboxSpec<Judge>` can take judge data, through
/// [`SandboxSpec::secrets`]. There is no network field and no method on any role that attaches one, because no
/// role gets a network device. [`SandboxSpec::seal`] turns a spec into the [`LaunchPlan`] a backend receives.
#[derive(Debug)]
pub struct SandboxSpec<R: Role> {
    key: LaunchKey,
    root: ImageDigest,
    input: InputDisk,
    budget: JobBudget,
    machine: Machine,
    arch: Arch,
    max_duration_ms: u64,
    judge_data: Option<JudgeData>,
    role: PhantomData<fn() -> R>,
}

impl<R: Role> SandboxSpec<R> {
    /// The spec of `job` in role `R`, booting the root image `root` with the input disk `input`.
    ///
    /// The launch key is [`LaunchKey::of`] the job's experiment, judge version, and trial in role `R`. The hard
    /// maximum duration is build plus run plus [`BOOT_ALLOWANCE_MS`] for a Candidate, and judge plus
    /// [`BOOT_ALLOWANCE_MS`] for a Judge.
    ///
    /// # Errors
    ///
    /// [`SpecError::DurationOverCeiling`] when that duration exceeds [`MAX_LAUNCH_MS`]. It is refused, never
    /// clamped.
    pub fn new(
        job: &JobRequest,
        root: ImageDigest,
        input: InputDisk,
        machine: Machine,
        arch: Arch,
    ) -> Result<Self, SpecError> {
        let budget = job.budget();
        let ms = match R::KIND {
            RoleKind::Candidate => budget
                .build_ms()
                .saturating_add(budget.run_ms())
                .saturating_add(BOOT_ALLOWANCE_MS),
            RoleKind::Judge => budget.judge_ms().saturating_add(BOOT_ALLOWANCE_MS),
        };
        if ms > MAX_LAUNCH_MS {
            return Err(SpecError::DurationOverCeiling { ms });
        }
        Ok(Self {
            key: LaunchKey::of::<R>(&job.exp(), &job.judge(), job.trial()),
            root,
            input,
            budget,
            machine,
            arch,
            max_duration_ms: ms,
            judge_data: None,
            role: PhantomData,
        })
    }

    /// Seals the spec into a [`LaunchPlan`]: the only constructor of a launch plan.
    #[must_use]
    pub fn seal(self) -> LaunchPlan<R> {
        LaunchPlan {
            key: self.key,
            root: self.root,
            input: self.input,
            budget: self.budget,
            machine: self.machine,
            arch: self.arch,
            max_duration_ms: self.max_duration_ms,
            judge_data: self.judge_data,
            role: PhantomData,
        }
    }
}

impl SandboxSpec<Judge> {
    /// Attaches the judge's private data, mounted read-only in the judge VM.
    ///
    /// The only way judge data enters a spec. No other role has this method; the compile-fail suite (04-08) proves
    /// a Candidate spec cannot call it.
    #[must_use]
    pub fn secrets(mut self, data: JudgeData) -> Self {
        self.judge_data = Some(data);
        self
    }
}

/// A sealed sandbox launch: what a backend receives.
///
/// Built only by [`SandboxSpec::seal`]. Its fields are private and read-only, it has no network field, and it is
/// neither `Clone` nor `Default`, so a backend can neither forge nor duplicate a plan.
#[derive(Debug)]
pub struct LaunchPlan<R: Role> {
    key: LaunchKey,
    root: ImageDigest,
    input: InputDisk,
    budget: JobBudget,
    machine: Machine,
    arch: Arch,
    max_duration_ms: u64,
    judge_data: Option<JudgeData>,
    role: PhantomData<fn() -> R>,
}

impl<R: Role> LaunchPlan<R> {
    /// The launch key, from which a backend derives its idempotency token.
    #[must_use]
    pub const fn key(&self) -> &LaunchKey {
        &self.key
    }

    /// The digest of the read-only root image.
    #[must_use]
    pub const fn root(&self) -> &ImageDigest {
        &self.root
    }

    /// The per-job read-only input disk.
    #[must_use]
    pub const fn input(&self) -> &InputDisk {
        &self.input
    }

    /// The judge data, present only on a Judge plan built with [`SandboxSpec::secrets`].
    #[must_use]
    pub const fn judge_data(&self) -> Option<&JudgeData> {
        self.judge_data.as_ref()
    }

    /// The job's stage budgets.
    #[must_use]
    pub const fn budget(&self) -> &JobBudget {
        &self.budget
    }

    /// The VM size.
    #[must_use]
    pub const fn machine(&self) -> &Machine {
        &self.machine
    }

    /// The guest architecture.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        self.arch
    }

    /// The hard maximum duration, in milliseconds: no launch outlives it, whatever cleanup does.
    #[must_use]
    pub const fn max_duration_ms(&self) -> u64 {
        self.max_duration_ms
    }

    /// The hard maximum duration.
    #[must_use]
    pub const fn max_duration(&self) -> Duration {
        Duration::from_millis(self.max_duration_ms)
    }

    /// The plan's role.
    #[must_use]
    pub const fn role(&self) -> RoleKind {
        R::KIND
    }
}
