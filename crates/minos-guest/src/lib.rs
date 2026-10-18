//! Host-compilable half of the guest that runs as PID 1 inside every VM.
//!
//! PID 1 reads one job through a [`source::JobSource`] and sends one output tar through a [`sink::OutputSink`]
//! (05-CONTEXT D-07). Locally the source is the read-only input disk ([`source::DiskJobSource`]) and the sink is
//! a sequence of short authenticated vsock connections ([`sink::ChunkSink`]); the AWS milestone adds a
//! lifecycle-hook pair implementing the same two traits, without touching the decoders, the applier, or the
//! command runner. Every stage command runs as one fixed unprivileged uid under the D-05 limits, through the
//! `/init --minos-stage` trampoline whose argv [`stage`] encodes and parses (D-08).
//!
//! This library holds the parts the host can compile and test: the source and sink traits and their
//! implementations over generic readers and connectors, the stage limits and trampoline argv, and the Phase 2
//! spike protocol, which is the job decoder ([`job`]), the report and status-frame encoder ([`report`]), and the
//! deterministic byte pattern ([`pattern`]). The Linux-only PID 1 path lives in the binary's `linux` module
//! (02-CONTEXT.md D-03), and only it names `/dev/vdb` and the vsock socket. The spike modules and markers stay
//! until plan 05-12 replaces that path.

pub mod job;
pub mod nofollow;
pub mod pack;
pub mod pattern;
pub mod report;
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

/// Console prefix for a job that could not be read or decoded. No status frame follows it.
pub const JOB_ERROR_MARKER: &str = "MINOS-GUEST-JOB-ERROR";

/// Console prefix printed after the status frame was written to the host.
pub const FRAME_SENT_MARKER: &str = "MINOS-GUEST-FRAME-SENT";

/// Console line printed just before the guest asks the kernel to power off.
pub const POWEROFF_MARKER: &str = "MINOS-GUEST-POWEROFF";

/// Version carried in both the job header and the status-frame header.
pub const PROTOCOL_VERSION: u32 = 1;

/// Length of the job header and of the status-frame header: 8 bytes of magic, a `u32` version, and a `u64`
/// payload length, all big-endian.
pub const HEADER_LEN: usize = 20;

/// Largest job payload the guest reads, in bytes.
pub const MAX_JOB_BYTES: u64 = 1_048_576;

/// Largest status-frame payload the guest sends, in bytes.
pub const MAX_FRAME_BYTES: u64 = 1_048_576;

#[cfg(test)]
pub(crate) mod test_hex {
    //! Hex decoding for golden-byte tests.

    /// Decodes lowercase or uppercase hex with no separators.
    pub(crate) fn decode(text: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        text.as_bytes()
            .chunks(2)
            .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
            .collect()
    }
}
