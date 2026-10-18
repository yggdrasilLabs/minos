//! Host side of the libkrun VM backend.
//!
//! Phase 2 holds the spike code that Phase 5 builds on (02-CONTEXT.md D-01): the root-image and initramfs
//! writers, the job input disk, the `minos-vmm` launch config, and a supervisor for one helper process.

pub mod initramfs;
pub mod input_disk;
pub mod launch_config;
pub mod rootfs;
pub mod seatbelt;
pub mod supervise;
