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

use minos_core::id::ImageDigest;
use minos_core::ledger::{BackendInfo, BackendKind, ConfinementLayer, Isolation};
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
    /// The VMM helper lacks one of its two entitlements, `com.apple.security.hypervisor` and
    /// `com.apple.security.cs.disable-library-validation` (05-CONTEXT D-02). Re-sign it with `minos host setup`.
    #[error(
        "the VMM helper lacks com.apple.security.hypervisor or com.apple.security.cs.disable-library-validation; run minos host setup"
    )]
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
    /// `/usr/bin/sandbox-exec` is missing. Seatbelt confinement is mandatory in v1 (05-CONTEXT D-03), so no krun
    /// plan launches without it; preflight also refuses when the host's layers lack Seatbelt.
    #[error(
        "/usr/bin/sandbox-exec is missing, and Seatbelt confinement is required to launch; restore the system tool"
    )]
    SeatbeltMissing,
    /// The Seatbelt profile no longer denies what it must: `minos host doctor`'s probes found a loopback connection
    /// or an unlisted file read allowed under it (05-CONTEXT D-03). Nothing launches until the probes pass again.
    #[error(
        "the Seatbelt profile no longer denies a loopback connection or an unlisted file; do not launch until minos host doctor passes"
    )]
    SeatbeltNotEnforcing,
    /// The `minos-vmm` helper binary was not found. Build it.
    #[error(
        "the minos-vmm helper was not found next to minos or in the cargo target directory; build it with cargo build -p minos-vmm"
    )]
    VmmMissing,
    /// No static aarch64 Linux `minos-guest` binary was found to write into the root image as `/init`. Build it.
    #[error(
        "no static aarch64 Linux minos-guest was found; build it with cargo build -p minos-guest --release --target aarch64-unknown-linux-musl"
    )]
    GuestMissing,
}

/// What a host can run, as a backend checked it.
///
/// A backend probes the host once and returns this value from `capabilities()` (05-RESEARCH Pattern 9), so the
/// runner learns the confinement layers through the two-method `Backend` trait and records them through
/// [`Capabilities::backend_info`] (KRUN-07). Host refusals are the facts `minos host doctor` reports (KRUN-06):
/// each refuses every plan.
///
/// The layers are the confinement around the guest's code beyond the VM itself: [`ConfinementLayer::Seatbelt`],
/// the deny-default profile around the VMM process on the host, and [`ConfinementLayer::Rlimits`], which covers
/// both halves 05-CONTEXT D-01 names (the helper's own limits, plan 05-07, and the guest's per-stage limits on the
/// candidate, plans 05-08 and 05-12) and is listed only when both are applied (plan 05-11). A krun host never
/// lists [`ConfinementLayer::DedicatedUser`] in v1 (D-01, deferred), so every krun run shows it as reduced
/// hardening. Seatbelt is never optional: [`preflight`] refuses every krun plan on a host whose layers lack it
/// (D-03), so no run is ever recorded without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    backend: BackendKind,
    arch: Arch,
    max: Machine,
    max_duration_ms: u64,
    isolation: Isolation,
    host_refusals: BTreeSet<Refusal>,
    layers: BTreeSet<ConfinementLayer>,
    kernel: Option<ImageDigest>,
}

impl Capabilities {
    /// A `backend` host of `arch` that runs VMs up to `max` for at most `max_duration_ms`, isolated at
    /// `isolation`, with the host checks that failed in `host_refusals`.
    ///
    /// It starts with no confinement layers and no kernel digest; add them with [`Capabilities::with_layers`] and
    /// [`Capabilities::with_kernel`]. A krun host with no layers clears no plan.
    #[must_use]
    pub const fn new(
        backend: BackendKind,
        arch: Arch,
        max: Machine,
        max_duration_ms: u64,
        isolation: Isolation,
        host_refusals: BTreeSet<Refusal>,
    ) -> Self {
        Self {
            backend,
            arch,
            max,
            max_duration_ms,
            isolation,
            host_refusals,
            layers: BTreeSet::new(),
            kernel: None,
        }
    }

    /// The same host with `layers` as its active confinement layers, replacing any set before.
    #[must_use]
    pub fn with_layers(self, layers: BTreeSet<ConfinementLayer>) -> Self {
        Self { layers, ..self }
    }

    /// The same host with `kernel` as the digest of the guest kernel it boots.
    #[must_use]
    pub fn with_kernel(self, kernel: ImageDigest) -> Self {
        Self {
            kernel: Some(kernel),
            ..self
        }
    }

    /// The backend that checked the host.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
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

    /// The confinement layers active around every launch on this host.
    #[must_use]
    pub const fn layers(&self) -> &BTreeSet<ConfinementLayer> {
        &self.layers
    }

    /// The digest of the guest kernel, when the backend reports one.
    #[must_use]
    pub const fn kernel(&self) -> Option<&ImageDigest> {
        self.kernel.as_ref()
    }

    /// The provenance a run on this host records for a VM that booted `root_image`: the backend, architecture,
    /// isolation level, active layers, and kernel digest, so the report shows any layer that was missing.
    #[must_use]
    pub fn backend_info(&self, root_image: ImageDigest) -> BackendInfo {
        BackendInfo::new(
            self.backend,
            self.arch,
            self.isolation,
            self.layers.clone(),
            root_image,
            self.kernel,
        )
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
/// duration exceed the host's, when the host isolates below [`Isolation::Hardware`], when a krun host's layers lack
/// [`ConfinementLayer::Seatbelt`] (05-CONTEXT D-03: there is no fallback without it), when a Judge plan has no
/// judge data or its judge data is any part of its input disk, and for every host refusal. A missing
/// [`ConfinementLayer::DedicatedUser`] is not a refusal: it is deferred (D-01) and recorded as missing.
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
    // Seatbelt is the krun backend's host layer; a later backend brings its own mandatory layers.
    if host.backend == BackendKind::Krun && !host.layers.contains(&ConfinementLayer::Seatbelt) {
        refusals.insert(Refusal::SeatbeltMissing);
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
