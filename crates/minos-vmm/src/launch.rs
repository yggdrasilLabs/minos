//! VM assembly: turns a validated [`VmmConfig`] into an `msb_krun` [`Vm`].
//!
//! The device set is fixed here and never comes from the config:
//! - Three virtio-blk disks in attach order: root (`/dev/vda`, read-only), input (`/dev/vdb`, read-only), and
//!   scratch (`/dev/vdc`, read-write). All three are raw images, which is the builder's default, so the launcher
//!   never names an image type and nothing is ever probed.
//! - One vsock route: a guest connection to the configured port is connected to the host Unix socket the parent
//!   already bound. That socket is the only path out of the guest.
//! - The console writes to a file, so neither host stdin nor a host terminal reaches the guest.
//! - The balloon and metrics devices are off. The rng device keeps its default (02-RESEARCH.md open question 3).
//!
//! The VM has no network device and no shared host directory. Without a network device the builder leaves the
//! guest air-gapped, and the clippy bans in this crate's `clippy.toml` keep it that way.

use msb_krun::{KernelBuilder, Vm, VmBuilder};

use crate::config::{Boot, VmmConfig};

/// Kernel arguments appended in root-disk mode (D-18): mount the `SquashFS` root read-only, print every kernel
/// message, and create no dummy network interface. They follow `msb_krun`'s defaults, so they override its
/// `rootfstype=virtiofs rw`. libkrunfw builds the dummy driver in (`CONFIG_DUMMY=y`) and it creates `dummy0` by
/// default; `dummy.numdummies=0` leaves the guest with loopback only (02-06 S01-NETIFACES, user decision).
pub const ROOT_DISK_CMDLINE: &str = "rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0";

/// `init=` value in root-disk mode. `msb_krun` writes `root=/dev/root init=<this>`, so `root=/dev/vda` lands after
/// its `root=/dev/root`, and Linux keeps the last `root=` (02-RESEARCH.md Pattern 1).
pub const ROOT_DISK_INIT: &str = "/init root=/dev/vda";

/// Kernel arguments appended in initramfs mode, the negative-evidence run (D-18).
pub const INITRAMFS_CMDLINE: &str = "ignore_loglevel";

/// Assembles the VM described by `config`. Nothing starts until `Vm::enter`.
///
/// # Errors
///
/// `msb_krun` refused the configuration or failed to build the VM.
pub fn build(config: &VmmConfig) -> Result<Vm, msb_krun::Error> {
    VmBuilder::new()
        .machine(|machine| {
            machine
                .vcpus(config.vcpus())
                .memory_mib(usize::from(config.memory_mib()))
                .balloon(false)
                .msb_metrics(false)
        })
        .kernel(|kernel| kernel_for(kernel, config))
        .disk(|disk| disk.path(config.root()).read_only(true))
        .disk(|disk| disk.path(config.input()).read_only(true))
        .disk(|disk| disk.path(config.scratch()).read_only(false))
        .vsock(|vsock| vsock.unix_connect(config.vsock_port(), config.socket()))
        .console(|console| console.output(config.console()))
        .on_exit(|code| crate::say(format_args!("minos-vmm: exit {code}")))
        .build()
}

/// The libkrunfw path plus the fixed E-4 arguments for the boot mode. Nothing here comes from the config except
/// the two file paths.
fn kernel_for(kernel: KernelBuilder, config: &VmmConfig) -> KernelBuilder {
    let kernel = kernel.krunfw_path(config.krunfw());
    match config.boot() {
        Boot::RootDisk => kernel.cmdline(ROOT_DISK_CMDLINE).init_path(ROOT_DISK_INIT),
        Boot::Initramfs { path, .. } => kernel.cmdline(INITRAMFS_CMDLINE).initramfs_path(path),
    }
}
