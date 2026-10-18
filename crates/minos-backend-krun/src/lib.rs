//! Host side of the libkrun VM backend: [`KrunBackend`], the [`minos_sandbox::Backend`] the CLI and the runner use.
//!
//! A launch fetches the plan's blobs through a [`minos_sandbox::BlobSource`] ([`input_disk`]), writes a per-job
//! input disk into a private job directory ([`jobdir`]), names the store's verified root image under a read-only
//! raw disk, writes the `minos-vmm` launch config ([`launch_config`]), binds the vsock listener and starts the
//! chunk receiver ([`chunks`]), and spawns the helper under the embedded Seatbelt profile ([`seatbelt`]). The
//! supervisor ([`supervise`]) caps the console and stderr pipes, kills the helper's process group at the host
//! deadline, the console cap, or a chunk fault, and classifies how the launch ended. The host probes ([`host`])
//! run once, when the backend is built, and [`rootfs`] packs root images for `minos env build`.

pub mod backend;
pub mod chunks;
pub mod host;
pub mod input_disk;
pub mod jobdir;
pub mod launch_config;
pub mod rootfs;
pub mod seatbelt;
pub mod supervise;

pub use backend::{KrunBackend, KrunRunning};
