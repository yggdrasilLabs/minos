//! Preflight: the fail-closed check between a sealed plan and a launch (SBX-03, D-17).
//!
//! [`preflight`] is the only constructor of [`Cleared`], and a backend's launch takes a `Cleared` rather than a
//! bare plan. `Cleared` owns the plan it cleared, so a token for plan A can never authorize plan B, and no backend
//! crate can mint one.
//!
//! Preflight takes the backend's [`Capabilities`] (the "host" of D-17) rather than the backend itself, so the
//! sandbox types need no `Backend` trait to clear a plan; the runner calls `preflight(&backend.capabilities(),
//! plan)`.

use std::collections::BTreeSet;

use minos_core::ledger::Isolation;
use minos_core::manifest::Arch;

use crate::key::LaunchKey;
use crate::role::{Role, RoleKind};
use crate::spec::{LaunchPlan, Machine};

/// Why a plan may not launch on a host. Payloads are numbers and enums only.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, thiserror::Error)]
pub enum Refusal {
    /// The plan's image is built for another architecture than the host runs. Build the environment for the
    /// host's architecture, or run on a matching host.
    #[error(
        "the plan is for {plan:?} but the host runs {host:?}; build the image for the host's architecture"
    )]
    ArchMismatch {
        /// The plan's architecture.
        plan: Arch,
        /// The host's architecture.
        host: Arch,
    },
    /// The plan asks for more vCPUs than the host offers. Ask for fewer.
    #[error("the plan asks for {plan} vCPUs but the host offers {host}; ask for fewer")]
    TooManyVcpus {
        /// The plan's vCPU count.
        plan: u8,
        /// The host's maximum.
        host: u8,
    },
    /// The plan asks for more memory than the host offers. Ask for less.
    #[error("the plan asks for {plan} MiB but the host offers {host} MiB; ask for less")]
    TooMuchMemory {
        /// The plan's memory, in MiB.
        plan: u16,
        /// The host's maximum, in MiB.
        host: u16,
    },
    /// The plan's hard maximum duration is longer than the host allows. Lower the stage budgets.
    #[error(
        "the plan needs {plan_ms} ms but the host allows {host_ms} ms; lower the stage budgets"
    )]
    DurationOverHost {
        /// The plan's hard maximum, in milliseconds.
        plan_ms: u64,
        /// The host's maximum, in milliseconds.
        host_ms: u64,
    },
    /// The host isolates below hardware virtualization. v1 launches only on hardware isolation; the labelled weak
    /// dev mode arrives with LNX-01 in a later milestone.
    #[error("the host does not provide hardware virtualization")]
    WeakIsolation,
    /// A Judge plan carries no judge data. The runner must attach the pushed judge data with `secrets`.
    #[error("the judge plan carries no judge data; push judge data and attach it")]
    JudgeDataMissing,
    /// A Judge plan's judge data is one of its input disk's parts: a runner bug that would hand judge data in as
    /// ordinary input. Fix the runner.
    #[error("the judge plan's judge data is part of its input disk; fix the runner")]
    JudgeDataIsInput,
    /// The VMM helper lacks the hypervisor entitlement. Re-sign it as `minos host setup` describes.
    #[error("the VMM helper lacks the hypervisor entitlement; re-sign it")]
    EntitlementMissing,
    /// The guest kernel library was not found. Install libkrunfw.
    #[error("the guest kernel library was not found; install libkrunfw")]
    KernelMissing,
    /// The guest kernel library does not match its pinned SHA-256. Reinstall the pinned libkrunfw.
    #[error(
        "the guest kernel library does not match its pinned hash; reinstall the pinned libkrunfw"
    )]
    KernelHashMismatch,
    /// The host has no usable hypervisor. Run on hardware with Hypervisor.framework support.
    #[error("the host has no usable hypervisor")]
    HypervisorUnsupported,
    /// The vsock socket path would not fit in a Unix socket address. Use a shorter data directory.
    #[error("the vsock socket path is too long for a Unix socket; use a shorter data directory")]
    SocketPathTooLong,
}

/// What a host can run, as a backend checked it.
///
/// A backend builds this from what it checked on the host; the `Backend` trait (04-08) returns it from
/// `capabilities()`. Host refusals are the facts `minos host doctor` reports (KRUN-06): each refuses every plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    arch: Arch,
    max: Machine,
    max_duration_ms: u64,
    isolation: Isolation,
    host_refusals: BTreeSet<Refusal>,
}

impl Capabilities {
    /// A host of `arch` that runs VMs up to `max` for at most `max_duration_ms`, isolated at `isolation`, with the
    /// host checks that failed in `host_refusals`.
    #[must_use]
    pub const fn new(
        arch: Arch,
        max: Machine,
        max_duration_ms: u64,
        isolation: Isolation,
        host_refusals: BTreeSet<Refusal>,
    ) -> Self {
        Self {
            arch,
            max,
            max_duration_ms,
            isolation,
            host_refusals,
        }
    }

    /// The host's architecture.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        self.arch
    }

    /// The largest VM the host runs.
    #[must_use]
    pub const fn max(&self) -> &Machine {
        &self.max
    }

    /// The longest hard maximum duration the host accepts, in milliseconds.
    #[must_use]
    pub const fn max_duration_ms(&self) -> u64 {
        self.max_duration_ms
    }

    /// The isolation level the host provides.
    #[must_use]
    pub const fn isolation(&self) -> Isolation {
        self.isolation
    }

    /// The host checks that failed, each of which refuses every plan.
    #[must_use]
    pub const fn host_refusals(&self) -> &BTreeSet<Refusal> {
        &self.host_refusals
    }
}

/// Proof that this exact plan passed [`preflight`].
///
/// It owns the plan, so a `Cleared` for plan A can never authorize plan B. Its field is private and it is neither
/// `Clone` nor `Default`, so no backend crate can mint or duplicate one (ARCHITECTURE.md Pattern 4, Amendment A5).
pub struct Cleared<R: Role> {
    plan: LaunchPlan<R>,
}

impl<R: Role> Cleared<R> {
    /// The plan that was cleared.
    #[must_use]
    pub const fn plan(&self) -> &LaunchPlan<R> {
        &self.plan
    }

    /// The plan's launch key, consuming the clearance. Only [`Finished::new`](crate::Finished::new) calls it, so
    /// a clearance ends in at most one `Finished`.
    #[must_use]
    pub(crate) fn into_key(self) -> LaunchKey {
        *self.plan.key()
    }
}

/// Checks `plan` against `host` and clears it, failing closed.
///
/// The only constructor of [`Cleared`]. Refuses when the architecture differs, when the plan's vCPUs, memory, or
/// duration exceed the host's, when the host isolates below [`Isolation::Hardware`], when a Judge plan has no
/// judge data or its judge data is any part of its input disk, and for every host refusal.
///
/// # Errors
///
/// Every [`Refusal`] that applies, sorted and without duplicates. Nothing launches.
pub fn preflight<R: Role>(
    host: &Capabilities,
    plan: LaunchPlan<R>,
) -> Result<Cleared<R>, Vec<Refusal>> {
    let mut refusals = BTreeSet::new();
    if plan.arch() != host.arch {
        refusals.insert(Refusal::ArchMismatch {
            plan: plan.arch(),
            host: host.arch,
        });
    }
    if plan.machine().vcpus() > host.max.vcpus() {
        refusals.insert(Refusal::TooManyVcpus {
            plan: plan.machine().vcpus(),
            host: host.max.vcpus(),
        });
    }
    if plan.machine().memory_mib() > host.max.memory_mib() {
        refusals.insert(Refusal::TooMuchMemory {
            plan: plan.machine().memory_mib(),
            host: host.max.memory_mib(),
        });
    }
    if plan.max_duration_ms() > host.max_duration_ms {
        refusals.insert(Refusal::DurationOverHost {
            plan_ms: plan.max_duration_ms(),
            host_ms: host.max_duration_ms,
        });
    }
    if host.isolation < Isolation::Hardware {
        refusals.insert(Refusal::WeakIsolation);
    }
    if R::KIND == RoleKind::Judge {
        match plan.judge_data() {
            None => {
                refusals.insert(Refusal::JudgeDataMissing);
            }
            Some(data) if plan.input().blobs().contains(data.blob()) => {
                refusals.insert(Refusal::JudgeDataIsInput);
            }
            Some(_) => {}
        }
    }
    refusals.extend(host.host_refusals.iter().copied());
    if refusals.is_empty() {
        Ok(Cleared { plan })
    } else {
        Err(refusals.into_iter().collect())
    }
}
