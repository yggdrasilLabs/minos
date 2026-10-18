//! The minos-vmm launch config (v2): serialized here, parsed and validated by minos-vmm.
//!
//! v2 names two read-only disks, the root image and the job input, and nothing else: no scratch disk (scratch is a
//! size-capped tmpfs in guest memory, 05-CONTEXT D-05), no initramfs mode, and no console file, because the
//! console is the helper's stdout pipe (05-RESEARCH Open Question 7; 05-EVIDENCE.md Mechanism selections,
//! `console: pipe`). minos-vmm v2 refuses any of those fields.
//!
//! The wire shape is fixed by field declaration order, which is the key order of the compact JSON. This crate
//! never links minos-vmm; one golden file is the binding (02-CONTEXT.md D-04). This module's tests serialize a
//! fixed config and compare it byte for byte with `crates/minos-vmm/golden/config-v2.json`, whose SHA-256 both
//! crates pin, and minos-vmm's tests parse the same file. Validation lives in minos-vmm: this type only shapes
//! output.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

/// The launch config version this crate writes.
pub const LAUNCH_CONFIG_VERSION: u32 = 2;

/// One VM launch, as minos-vmm reads it from its single argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchConfig {
    v: u32,
    kernel: Kernel,
    disks: Disks,
    vsock: Vsock,
    max_duration_ms: u64,
    machine: Machine,
}

impl LaunchConfig {
    /// Assembles a v2 config from its parts.
    #[must_use]
    pub const fn new(
        kernel: Kernel,
        disks: Disks,
        vsock: Vsock,
        max_duration_ms: u64,
        machine: Machine,
    ) -> Self {
        Self {
            v: LAUNCH_CONFIG_VERSION,
            kernel,
            disks,
            vsock,
            max_duration_ms,
            machine,
        }
    }

    /// The hard maximum lifetime minos-vmm's watchdog enforces, from `max_duration_ms`.
    #[must_use]
    pub const fn max_duration(&self) -> Duration {
        Duration::from_millis(self.max_duration_ms)
    }

    /// Serializes the config as one line of compact JSON followed by a newline, the exact bytes minos-vmm parses
    /// and the form of its golden file.
    ///
    /// # Errors
    ///
    /// Returns the [`serde_json::Error`] if a path is not valid UTF-8.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut json = serde_json::to_string(self)?;
        json.push('\n');
        Ok(json)
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

/// The two block devices, both attached read-only: the root image on `/dev/vda` and the job input on `/dev/vdb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Disks {
    root: PathBuf,
    input: PathBuf,
}

impl Disks {
    /// Names the root and input images.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, input: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            input: input.into(),
        }
    }
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

    use sha2::{Digest, Sha256};

    use super::{Disks, Kernel, LAUNCH_CONFIG_VERSION, LaunchConfig, Machine, Vsock};

    type TestResult = Result<(), Box<dyn Error>>;

    /// minos-vmm's golden v2 launch config. Both sides are bound to these bytes (02-CONTEXT.md D-04): minos-vmm's
    /// `config_v2_golden` parses them, and [`launch_config_matches_golden`] serializes them.
    const GOLDEN: &str = include_str!("../../minos-vmm/golden/config-v2.json");

    /// The same pin as `GOLDEN_SHA256` in minos-vmm's config tests, from
    /// `shasum -a 256 crates/minos-vmm/golden/config-v2.json`. Change both together.
    const GOLDEN_SHA256: &str = "0b1a1208888bbb6da4d77d0947a24993b31210aef4f788fb366549ee445f4a7b";

    /// The config whose serialization is the golden file.
    fn golden_config() -> LaunchConfig {
        LaunchConfig::new(
            Kernel::new(
                "/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib",
                "59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217",
            ),
            Disks::new(
                "/Users/example/project/.minos/blobs/sha256/ab/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd",
                "/private/tmp/minos-501/mnsAAAAAAAA/input.img",
            ),
            Vsock::new(1024, "/private/tmp/minos-501/mnsAAAAAAAA/o.sock"),
            600_000,
            Machine::new(2, 2048),
        )
    }

    #[test]
    fn launch_config_matches_golden() -> TestResult {
        let digest: [u8; 32] = Sha256::digest(GOLDEN.as_bytes()).into();
        assert_eq!(base16ct::lower::encode_string(&digest), GOLDEN_SHA256);
        let json = golden_config().to_json()?;
        assert_eq!(json, GOLDEN);
        let digest: [u8; 32] = Sha256::digest(json.as_bytes()).into();
        assert_eq!(base16ct::lower::encode_string(&digest), GOLDEN_SHA256);
        Ok(())
    }

    #[test]
    fn launch_config_field_order() -> TestResult {
        assert_eq!(LAUNCH_CONFIG_VERSION, 2);
        let json = golden_config().to_json()?;
        assert!(json.starts_with(r#"{"v":2,"kernel":{"krunfw":"#));
        let keys = [
            r#""v":"#,
            r#""kernel":"#,
            r#""disks":{"root":"#,
            r#","input":"#,
            r#""vsock":{"port":"#,
            r#""max_duration_ms":"#,
            r#""machine":{"vcpus":"#,
        ];
        let mut last = 0;
        for key in keys {
            let at = json.find(key).ok_or_else(|| format!("{key} is missing"))?;
            assert!(at >= last, "{key} is out of order");
            last = at;
        }
        // The console is the helper's stdout pipe (05-EVIDENCE.md Mechanism selections), and v2 has no boot mode,
        // scratch disk, or initramfs (05-CONTEXT D-05).
        for gone in [
            r#""console""#,
            r#""boot""#,
            r#""scratch""#,
            r#""initramfs""#,
        ] {
            assert!(!json.contains(gone), "{gone} must not be written");
        }
        assert_eq!(json.lines().count(), 1);
        assert!(json.ends_with("}\n"));
        Ok(())
    }

    #[test]
    fn launch_config_refuses_a_non_utf8_path() {
        let path = OsStr::from_bytes(b"/private/tmp/\xff/root.img");
        let config = LaunchConfig::new(
            Kernel::new("/k", "00"),
            Disks::new(path, "/i"),
            Vsock::new(1024, "/o.sock"),
            1,
            Machine::new(1, 1),
        );
        assert!(config.to_json().is_err());
    }
}
