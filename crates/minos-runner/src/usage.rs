//! Host-measured usage of one VM (LOOP-03).
//!
//! The candidate reports nothing about itself. A VM's usage is the time the host measured, from the backend's own
//! clock, times the size of the machine the plan asked for. No number from the guest's output, its status, its
//! logs, or any file it wrote, is ever read here, and the runner reads no clock of its own (its `clippy.toml`
//! bans `SystemTime::now` and `Instant::now`).

use std::time::Duration;

use minos_core::limits::MAX_VCPUS;
use minos_core::usage::Usage;
use minos_sandbox::Machine;

/// vCPUs of every candidate and judge VM in v1.
///
/// A build parallelizes somewhat, so one vCPU would make builds slow, and metered usage scales with the count.
/// The manifest has no field for it, so it is one constant for both roles (07-RESEARCH Assumption A5), within
/// [`MAX_VCPUS`].
pub const RUNNER_VCPUS: u8 = 2;

const _: () = assert!(RUNNER_VCPUS >= 1 && RUNNER_VCPUS <= MAX_VCPUS);

/// The usage of one VM that ran for `elapsed` on `machine`: vCPU-milliseconds and MiB-milliseconds of memory, with
/// no tokens.
///
/// `elapsed` is [`Finished::elapsed`](minos_sandbox::Finished::elapsed), the backend's host `Instant` from the
/// helper's spawn to its exit, so it includes boot: the VM held its vCPUs and memory for all of it. `machine` is
/// the plan's own machine. Nothing from the guest's output is read.
///
/// Every step saturates: a duration whose milliseconds do not fit a `u64`, or a product that overflows, becomes
/// `u64::MAX`, which is over every spend cap and so fails closed.
#[must_use]
pub fn host_usage(elapsed: Duration, machine: &Machine) -> Usage {
    let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    Usage::compute(
        ms.saturating_mul(u64::from(machine.vcpus())),
        ms.saturating_mul(u64::from(machine.memory_mib())),
    )
}
