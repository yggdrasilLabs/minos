//! Host-compilable half of the guest that runs as PID 1 inside every VM.
//!
//! PID 1 reads one job through a [`source::JobSource`], runs it with [`run::run_job`], and sends one
//! authenticated output tar through a [`sink::OutputSink`] (05-CONTEXT D-07). Locally the source is the read-only
//! input disk ([`source::DiskJobSource`]) and the sink is a sequence of short authenticated vsock connections
//! ([`sink::ChunkSink`]); the AWS milestone adds a lifecycle-hook pair implementing the same two traits, without
//! touching the decoders, the applier, or the command runner. Every stage command runs as one fixed unprivileged
//! uid under the D-05 limits, through the `/init --minos-stage` trampoline whose argv [`stage`] encodes and parses
//! (D-08).
//!
//! [`run::run_job`] runs one job for either role in a fixed order: read the job, create the per-run directories,
//! unpack the inputs ([`workspace::unpack_tar`]) and apply the diff with core's strict applier
//! ([`workspace::apply_diff`]), drop the input buffers, hand the writable directories to the stage uid, run each
//! stage until one does not exit 0, pack the output tar without following a link ([`pack`]), and send it. Its
//! source, sink, stage executor, and handover are traits, so the host tests the whole flow with fakes.
//!
//! This library holds the parts the host can compile and test. The Linux half, PID 1 itself, the trampoline,
//! and the Linux implementations of the four traits, lives in the binary's `linux` module (02-CONTEXT.md D-03),
//! and only it names `/dev/vdb` and the vsock socket. The `minos-probe` binary is a test fixture for the VM
//! tests, built with the guest and never packed into a user image; it uses the deterministic byte pattern in
//! [`pattern`].

pub mod nofollow;
pub mod pack;
pub mod pattern;
pub mod run;
pub mod sink;
pub mod source;
pub mod stage;
pub mod workspace;

#[cfg(test)]
pub(crate) mod test_fixture;

/// First line the guest prints on the console, before anything else, so a boot is visible even if the job
/// is bad.
pub const BOOT_MARKER: &str = "MINOS-GUEST-BOOT-OK";

/// Console prefix for a setup failure PID 1 could not report to the host: the input disk could not be read, so
/// there is no token to authenticate a status with, or nothing could be packed.
pub const SETUP_ERROR_MARKER: &str = "MINOS-GUEST-SETUP-ERROR";

/// Console prefix printed after the output tar was sent, followed by the number of connections it took.
pub const OUTPUT_SENT_MARKER: &str = "MINOS-GUEST-OUTPUT-SENT";

/// Console line printed just before the guest asks the kernel to power off.
pub const POWEROFF_MARKER: &str = "MINOS-GUEST-POWEROFF";
