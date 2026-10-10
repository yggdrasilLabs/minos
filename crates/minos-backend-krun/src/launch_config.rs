//! The minos-vmm launch config (v1): serialized here, parsed and validated by minos-vmm.
//!
//! The wire shape is fixed by field declaration order, which is the key order of the compact JSON. The same
//! canonical texts (02-05 and 02-03 Exact texts E-1 and E-1b) are embedded in this module's tests and in
//! minos-vmm's parser tests, so the two sides cannot drift apart (02-CONTEXT.md D-04). This crate never links
//! minos-vmm; the shared texts are the only binding. Validation lives in minos-vmm: this type only shapes output.

use std::path::PathBuf;

use serde::Serialize;

/// The launch config version this crate writes.
pub const LAUNCH_CONFIG_VERSION: u32 = 1;

/// One VM launch, as minos-vmm reads it from its single argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchConfig {
    v: u32,
    kernel: Kernel,
    boot: Boot,
    disks: Disks,
    vsock: Vsock,
    console: Console,
    max_duration_ms: u64,
    machine: Machine,
}

impl LaunchConfig {
    /// Assembles a v1 config from its parts.
    #[must_use]
    pub fn new(
        kernel: Kernel,
        boot: Boot,
        disks: Disks,
        vsock: Vsock,
        console: Console,
        max_duration_ms: u64,
        machine: Machine,
    ) -> Self {
        Self {
            v: LAUNCH_CONFIG_VERSION,
            kernel,
            boot,
            disks,
            vsock,
            console,
            max_duration_ms,
            machine,
        }
    }

    /// Serializes the config as compact JSON, the exact bytes minos-vmm parses.
    ///
    /// # Errors
    ///
    /// Returns the [`serde_json::Error`] if a path is not valid UTF-8.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// The libkrunfw dylib and the SHA-256 minos-vmm must find before loading it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Kernel {
    krunfw: PathBuf,
    sha256: String,
}

impl Kernel {
    /// `krunfw` is the absolute dylib path; `sha256_hex` is its digest as 64 lowercase hex characters.
    #[must_use]
    pub fn new(krunfw: impl Into<PathBuf>, sha256_hex: impl Into<String>) -> Self {
        Self {
            krunfw: krunfw.into(),
            sha256: sha256_hex.into(),
        }
    }
}

/// How the guest kernel finds `/init`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Boot {
    /// Boot from the read-only root disk on `/dev/vda` (02-CONTEXT.md D-18). Serializes as `"root_disk"`.
    RootDisk,
    /// Attach an initramfs; negative evidence only (D-18). Serializes as `{"initramfs":{...}}`.
    Initramfs(Initramfs),
}

impl Boot {
    /// The initramfs boot value for the archive at `path` with digest `sha256_hex`.
    #[must_use]
    pub fn initramfs(path: impl Into<PathBuf>, sha256_hex: impl Into<String>) -> Self {
        Self::Initramfs(Initramfs {
            path: path.into(),
            sha256: sha256_hex.into(),
        })
    }
}

/// The `boot.initramfs` object; built through [`Boot::initramfs`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Initramfs {
    path: PathBuf,
    sha256: String,
}

/// The three block devices: read-only root, read-only job input, and the size-capped scratch disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Disks {
    root: PathBuf,
    input: PathBuf,
    scratch: Scratch,
}

impl Disks {
    /// Names the root, input, and scratch images, and the most bytes the scratch image may hold.
    #[must_use]
    pub fn new(
        root: impl Into<PathBuf>,
        input: impl Into<PathBuf>,
        scratch: impl Into<PathBuf>,
        scratch_cap_bytes: u64,
    ) -> Self {
        Self {
            root: root.into(),
            input: input.into(),
            scratch: Scratch {
                path: scratch.into(),
                cap_bytes: scratch_cap_bytes,
            },
        }
    }
}

/// The `disks.scratch` object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Scratch {
    path: PathBuf,
    cap_bytes: u64,
}

/// The guest vsock port and the host Unix socket the VMM connects to when the guest connects on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vsock {
    port: u32,
    socket: PathBuf,
}

impl Vsock {
    /// Routes guest port `port` to the host listener at `socket`.
    #[must_use]
    pub fn new(port: u32, socket: impl Into<PathBuf>) -> Self {
        Self {
            port,
            socket: socket.into(),
        }
    }
}

/// The console log file and the size at which minos-vmm's watchdog stops the VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Console {
    path: PathBuf,
    cap_bytes: u64,
}

impl Console {
    /// The console goes to `path`, a file that must not exist yet; the VM stops at `cap_bytes`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, cap_bytes: u64) -> Self {
        Self {
            path: path.into(),
            cap_bytes,
        }
    }
}

/// vCPU count and guest memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Machine {
    vcpus: u32,
    memory_mib: u32,
}

impl Machine {
    /// `vcpus` virtual CPUs and `memory_mib` MiB of guest memory; minos-vmm range-checks both.
    #[must_use]
    pub const fn new(vcpus: u32, memory_mib: u32) -> Self {
        Self { vcpus, memory_mib }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    use super::{Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock};

    type TestResult = Result<(), Box<dyn Error>>;

    /// 02-05-PLAN.md Exact texts E-1, byte-identical to 02-03's E-1 and to `E1` in minos-vmm's config tests.
    const E1: &str = r#"{"v":1,"kernel":{"krunfw":"/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"boot":"root_disk","disks":{"root":"/private/tmp/mns00000000/root.img","input":"/private/tmp/mns00000000/input.img","scratch":{"path":"/private/tmp/mns00000000/scratch.img","cap_bytes":67108864}},"vsock":{"port":1024,"socket":"/private/tmp/mns00000000/o.sock"},"console":{"path":"/private/tmp/mns00000000/console.log","cap_bytes":1048576},"max_duration_ms":60000,"machine":{"vcpus":1,"memory_mib":256}}"#;

    /// 02-05-PLAN.md Exact texts E-1b: the `boot` value in initramfs mode.
    const E1B: &str = r#"{"initramfs":{"path":"/private/tmp/mns00000000/initramfs.cpio","sha256":"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"}}"#;

    fn e1_config(boot: Boot) -> LaunchConfig {
        LaunchConfig::new(
            Kernel::new(
                "/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            boot,
            Disks::new(
                "/private/tmp/mns00000000/root.img",
                "/private/tmp/mns00000000/input.img",
                "/private/tmp/mns00000000/scratch.img",
                67_108_864,
            ),
            Vsock::new(1024, "/private/tmp/mns00000000/o.sock"),
            Console::new("/private/tmp/mns00000000/console.log", 1_048_576),
            60_000,
            Machine::new(1, 256),
        )
    }

    #[test]
    fn launch_config_root_disk_serializes_to_e1() -> TestResult {
        assert_eq!(e1_config(Boot::RootDisk).to_json()?, E1);
        Ok(())
    }

    #[test]
    fn launch_config_initramfs_serializes_to_e1_with_e1b() -> TestResult {
        let expected = E1.replacen(r#""boot":"root_disk""#, &format!(r#""boot":{E1B}"#), 1);
        assert_ne!(
            expected, E1,
            "the E-1 boot value must be present to substitute"
        );
        let boot = Boot::initramfs(
            "/private/tmp/mns00000000/initramfs.cpio",
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        );
        assert_eq!(e1_config(boot).to_json()?, expected);
        Ok(())
    }

    #[test]
    fn launch_config_refuses_a_non_utf8_path() {
        let path = OsStr::from_bytes(b"/private/tmp/\xff/root.img");
        let config = LaunchConfig::new(
            Kernel::new("/k", "00"),
            Boot::RootDisk,
            Disks::new(path, "/i", "/s", 1),
            Vsock::new(1024, "/o.sock"),
            Console::new("/c", 1),
            1,
            Machine::new(1, 1),
        );
        assert!(config.to_json().is_err());
    }
}
