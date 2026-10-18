//! Sandbox specs and sealed launch plans (SBX-01, SBX-03, SBX-06).
//!
//! A [`SandboxSpec`] is built from a validated [`JobRequest`] and carries IDs and digests only, never bytes: the
//! launch key, the root image digest, the input disk's part IDs, the stage budgets, the machine size, the
//! architecture, and the hard maximum duration. Policy is enforced while the spec is built, and
//! [`SandboxSpec::seal`] is the only constructor of a [`LaunchPlan`], the only thing a backend ever receives.
//!
//! Only `SandboxSpec<Judge>` has a method that attaches judge data ([`SandboxSpec::secrets`]). No role has a
//! method or a field that attaches a network device: no sandbox gets one (CLAUDE.md "Sandbox roles").

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::time::Duration;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::manifest::Arch;

use crate::key::LaunchKey;
use crate::role::{Judge, Role, RoleKind};

// The machine bounds are defined once, in `minos-core::limits` (05-CONTEXT D-06), because `Manifest::try_from`
// range-checks `[environment] memory_mib` against them and core depends on no internal crate; this crate
// re-exports them so existing paths keep working. They are restated only in `crates/minos-vmm/src/config.rs`
// (`MAX_VCPUS`, `MIN_MEMORY_MIB`, `MAX_MEMORY_MIB`, `MAX_DURATION_MS`), because minos-vmm links no internal crate,
// and `BOOT_ALLOWANCE_MS` below restates `HOST_DEADLINE_GRACE` in `crates/minos-backend-krun/src/supervise.rs`. A
// plan that passes here must also pass the launcher's own config validation, so each side pins the same literals
// in a test: `launcher_bounds_match_minos_vmm` in `crates/minos-sandbox/tests/spec.rs`,
// `launcher_bounds_are_pinned_for_minos_sandbox` in `crates/minos-vmm/src/config.rs`, and
// `host_deadline_grace_is_pinned_for_minos_sandbox` in `crates/minos-backend-krun/src/supervise.rs`. Changing one
// side fails the gate until the other side and both tests change with it (review WR-02).

pub use minos_core::limits::{MAX_MEMORY_MIB, MAX_VCPUS, MIN_MEMORY_MIB};

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
    /// The input disk's parts are shaped for another role than the spec's: a runner bug, because a Candidate
    /// input names a base tree and a Judge input names a candidate's output (05-CONTEXT D-15).
    #[error(
        "the input disk is shaped for the {role:?} role, not this spec's; build the role's own input"
    )]
    InputRole {
        /// The role the input is shaped for.
        role: RoleKind,
    },
    /// A Candidate input's base tree or diff blob differs from the job's: a runner bug, because the disk would
    /// carry a different experiment than the launch key names.
    #[error(
        "the input disk's base tree or diff differs from the job's; build the input from the job"
    )]
    InputMismatch,
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

/// The per-job read-only input disk, as the IDs of its parts (05-CONTEXT D-15).
///
/// The input disk is not itself stored as a blob. The backend fetches each part through
/// `BlobSource` under caps and writes the per-job disk itself, without unpacking or applying
/// anything. Every input names the guest job blob, the runner's `GuestJob` JSON. A Candidate input adds the base
/// [`TreeId`] (the source tarball) and, unless the job is the baseline, the canonical diff blob; a Judge input adds
/// the candidate's output blob. Judge data is never a part of an input: it stays [`JudgeData`], attachable only by
/// [`SandboxSpec::secrets`] on a `SandboxSpec<Judge>`. [`SandboxSpec::new`] refuses an input of the other role's
/// shape, and a Candidate input whose base or diff disagrees with the job; `preflight` refuses a Judge plan whose
/// judge data is any of its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputDisk {
    job: BlobId,
    parts: Parts,
}

/// The role-specific parts of an [`InputDisk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parts {
    /// A Candidate's base tree and, unless it is the baseline, its canonical diff.
    Candidate { base: TreeId, diff: Option<BlobId> },
    /// A Judge's view of the candidate: the candidate's output.
    Judge { candidate_out: BlobId },
}

impl InputDisk {
    /// A Candidate input: the guest job blob `job`, the base tree `base`, and the canonical diff blob `diff`, which
    /// is `None` exactly for the baseline.
    #[must_use]
    pub const fn candidate(job: BlobId, base: TreeId, diff: Option<BlobId>) -> Self {
        Self {
            job,
            parts: Parts::Candidate { base, diff },
        }
    }

    /// A Judge input: the guest job blob `job` and the candidate's output blob `candidate_out`.
    #[must_use]
    pub const fn judge(job: BlobId, candidate_out: BlobId) -> Self {
        Self {
            job,
            parts: Parts::Judge { candidate_out },
        }
    }

    /// The guest job blob: the runner's `GuestJob` JSON.
    #[must_use]
    pub const fn job(&self) -> BlobId {
        self.job
    }

    /// The base tree, on a Candidate input only.
    #[must_use]
    pub const fn base(&self) -> Option<TreeId> {
        match self.parts {
            Parts::Candidate { base, .. } => Some(base),
            Parts::Judge { .. } => None,
        }
    }

    /// The canonical diff blob, on a Candidate input that is not the baseline only.
    #[must_use]
    pub const fn diff(&self) -> Option<BlobId> {
        match self.parts {
            Parts::Candidate { diff, .. } => diff,
            Parts::Judge { .. } => None,
        }
    }

    /// The candidate's output blob, on a Judge input only.
    #[must_use]
    pub const fn candidate_out(&self) -> Option<BlobId> {
        match self.parts {
            Parts::Candidate { .. } => None,
            Parts::Judge { candidate_out } => Some(candidate_out),
        }
    }

    /// The role this input is shaped for.
    #[must_use]
    pub const fn role(&self) -> RoleKind {
        match self.parts {
            Parts::Candidate { .. } => RoleKind::Candidate,
            Parts::Judge { .. } => RoleKind::Judge,
        }
    }

    /// Every blob the input names, the base tree as [`TreeId::to_blob_id`]: what the backend fetches.
    #[must_use]
    pub fn blobs(&self) -> BTreeSet<BlobId> {
        let mut blobs = BTreeSet::from([self.job]);
        match self.parts {
            Parts::Candidate { base, diff } => {
                blobs.insert(base.to_blob_id());
                blobs.extend(diff);
            }
            Parts::Judge { candidate_out } => {
                blobs.insert(candidate_out);
            }
        }
        blobs
    }
}

/// The judge's private data, by blob ID, mounted read-only in a Judge sandbox only.
///
/// Provenance rule, owned by the runner: the blob is judge data the user pushed, and it is never given to a
/// Candidate; the type system enforces that half, because only `SandboxSpec<Judge>` has
/// [`SandboxSpec::secrets`]. `preflight` refuses a Judge plan whose judge data is any part of its input disk.
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
    /// [`SpecError::InputRole`] when `input` is shaped for the other role, [`SpecError::InputMismatch`] when a
    /// Candidate input's base or diff differs from the job's, and [`SpecError::DurationOverCeiling`] when the
    /// duration exceeds [`MAX_LAUNCH_MS`]. The input is checked first, and an over-ceiling duration is refused,
    /// never clamped.
    pub fn new(
        job: &JobRequest,
        root: ImageDigest,
        input: InputDisk,
        machine: Machine,
        arch: Arch,
    ) -> Result<Self, SpecError> {
        if input.role() != R::KIND {
            return Err(SpecError::InputRole { role: input.role() });
        }
        if R::KIND == RoleKind::Candidate
            && (input.base() != Some(job.base()) || input.diff() != job.diff())
        {
            return Err(SpecError::InputMismatch);
        }
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
