//! Launch config v1: the JSON file named by `minos-vmm`'s only argument (02-CONTEXT.md D-04).
//!
//! The host writes the file, but the launcher validates it as if it were hostile. The size is capped before
//! any parsing, every JSON object rejects unknown fields, and every value is checked by
//! [`VmmConfig::try_from`], the only constructor of [`VmmConfig`]. `minos-backend-krun` serializes the same
//! shape (02-05 E-1), so the two crates share one wire format without sharing a crate.

use std::collections::BTreeMap;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// Largest config file accepted, in bytes. Longer input is refused before JSON parsing.
pub const MAX_CONFIG_BYTES: u64 = 65_536;
/// The only config version this launcher reads.
pub const CONFIG_VERSION: u32 = 1;
/// Longest path accepted in any path field, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Longest vsock socket path, in bytes: the macOS `sun_path` holds 104 bytes including the NUL.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;
/// Largest vsock port. `u32::MAX` is `VMADDR_PORT_ANY`, and `msb_krun` refuses both 0 and `u32::MAX`.
pub const MAX_VSOCK_PORT: u32 = 4_294_967_294;
/// Largest scratch disk cap, in bytes (16 GiB).
pub const MAX_SCRATCH_CAP_BYTES: u64 = 17_179_869_184;
/// Largest console output cap, in bytes (64 MiB).
pub const MAX_CONSOLE_CAP_BYTES: u64 = 67_108_864;
/// Longest hard maximum run time, in milliseconds (one hour).
pub const MAX_DURATION_MS: u64 = 3_600_000;
/// Most vCPUs a VM may have.
pub const MAX_VCPUS: u8 = 8;
/// Least guest memory, in MiB.
pub const MIN_MEMORY_MIB: u16 = 128;
/// Most guest memory, in MiB.
pub const MAX_MEMORY_MIB: u16 = 16_384;
/// Length of a SHA-256 digest written as lowercase hex.
const SHA256_HEX_LEN: usize = 64;

/// Why a launch config was refused. Field names are the dotted JSON key path.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The input is longer than [`MAX_CONFIG_BYTES`]; it was not parsed.
    #[error("config is larger than {MAX_CONFIG_BYTES} bytes")]
    TooLarge,
    /// The input is not launch config JSON: bad syntax, a wrong type, a missing field, or an unknown field.
    #[error("config is not valid launch config JSON")]
    Json(#[source] serde_json::Error),
    /// The `v` tag names a version this launcher does not read.
    #[error("config version {0} is not supported (expected {CONFIG_VERSION})")]
    Version(u32),
    /// A path field breaks a path rule.
    #[error("{field} {reason}")]
    Path {
        /// The JSON key path of the field.
        field: &'static str,
        /// The rule the path breaks.
        reason: PathReason,
    },
    /// Two path fields name the same path.
    #[error("{first} and {second} name the same path")]
    DuplicatePath {
        /// The field that named the path first.
        first: &'static str,
        /// The field that named it again.
        second: &'static str,
    },
    /// A digest field is not 64 lowercase hex characters.
    #[error("{field} must be {SHA256_HEX_LEN} lowercase hex characters")]
    Sha256 {
        /// The JSON key path of the field.
        field: &'static str,
    },
    /// A number is outside its allowed range.
    #[error("{field} is {value}, must be {min} to {max}")]
    OutOfRange {
        /// The JSON key path of the field.
        field: &'static str,
        /// The value found.
        value: u64,
        /// Smallest allowed value.
        min: u64,
        /// Largest allowed value.
        max: u64,
    },
}

/// The path rule a path field breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathReason {
    /// The path contains a NUL byte.
    Nul,
    /// The path is longer than its limit.
    TooLong {
        /// The limit, in bytes.
        max: usize,
    },
    /// The path does not start at the root directory.
    NotAbsolute,
    /// A component after the root is empty, `.`, or `..`, or the path names the root itself.
    NotNormal,
}

impl fmt::Display for PathReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nul => f.write_str("contains a NUL byte"),
            Self::TooLong { max } => write!(f, "is longer than {max} bytes"),
            Self::NotAbsolute => f.write_str("is not an absolute path"),
            Self::NotNormal => f.write_str(
                "must be `/` followed by plain names (no empty, `.`, or `..` component)",
            ),
        }
    }
}

/// Raw top-level object of the launch config, exactly as written. Only [`VmmConfig::try_from`] reads it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVmmConfig {
    v: u32,
    kernel: RawKernel,
    boot: RawBoot,
    disks: RawDisks,
    vsock: RawVsock,
    console: RawConsole,
    max_duration_ms: u64,
    machine: RawMachine,
}

/// Raw `kernel` object: the libkrunfw dylib and its expected SHA-256.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawKernel {
    krunfw: PathBuf,
    sha256: String,
}

/// Raw `boot` value: the string `"root_disk"`, or `{"initramfs":{...}}`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawBoot {
    /// Boot from the read-only root disk on `/dev/vda` (D-18).
    RootDisk,
    /// Attach an initramfs; negative evidence only (D-18).
    Initramfs(RawInitramfs),
}

/// Raw `boot.initramfs` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawInitramfs {
    path: PathBuf,
    sha256: String,
}

/// Raw `disks` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDisks {
    root: PathBuf,
    input: PathBuf,
    scratch: RawScratch,
}

/// Raw `disks.scratch` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScratch {
    path: PathBuf,
    cap_bytes: u64,
}

/// Raw `vsock` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVsock {
    port: u32,
    socket: PathBuf,
}

/// Raw `console` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConsole {
    path: PathBuf,
    cap_bytes: u64,
}

/// Raw `machine` object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMachine {
    vcpus: u32,
    memory_mib: u32,
}

/// How the guest kernel finds `/init`.
pub enum Boot {
    /// The raw read-only `SquashFS` root on `/dev/vda` holds `/init` (D-18).
    RootDisk,
    /// A newc initramfs is attached; recorded as negative evidence only (D-18).
    Initramfs {
        /// Absolute canonical path of the initramfs image.
        path: PathBuf,
        /// Expected SHA-256 of the image.
        sha256: [u8; 32],
    },
}

/// A validated launch config. Private fields, no setters: if a value exists, every E-2 rule held.
#[derive(Deserialize)]
#[serde(try_from = "RawVmmConfig")]
pub struct VmmConfig {
    krunfw: PathBuf,
    krunfw_sha256: [u8; 32],
    boot: Boot,
    root: PathBuf,
    input: PathBuf,
    scratch: PathBuf,
    scratch_cap_bytes: u64,
    vsock_port: u32,
    socket: PathBuf,
    console: PathBuf,
    console_cap_bytes: u64,
    max_duration: Duration,
    vcpus: u8,
    memory_mib: u16,
}

impl VmmConfig {
    /// Path of the libkrunfw dylib that holds the guest kernel.
    pub fn krunfw(&self) -> &Path {
        &self.krunfw
    }

    /// Expected SHA-256 of the libkrunfw dylib.
    pub fn krunfw_sha256(&self) -> &[u8; 32] {
        &self.krunfw_sha256
    }

    /// Boot mode.
    pub fn boot(&self) -> &Boot {
        &self.boot
    }

    /// Read-only root disk image (`/dev/vda`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read-only per-job input disk image (`/dev/vdb`).
    pub fn input(&self) -> &Path {
        &self.input
    }

    /// Read-write scratch disk image (`/dev/vdc`).
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }

    /// Largest scratch image length the launcher will attach, in bytes.
    pub fn scratch_cap_bytes(&self) -> u64 {
        self.scratch_cap_bytes
    }

    /// Guest vsock port routed to the host socket.
    pub fn vsock_port(&self) -> u32 {
        self.vsock_port
    }

    /// Host Unix socket the VMM connects to when the guest connects to the vsock port.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// File that receives the guest console output.
    pub fn console(&self) -> &Path {
        &self.console
    }

    /// Console length past which the watchdog stops the VM, in bytes.
    pub fn console_cap_bytes(&self) -> u64 {
        self.console_cap_bytes
    }

    /// Hard maximum run time, after which the watchdog stops the VM.
    pub fn max_duration(&self) -> Duration {
        self.max_duration
    }

    /// Number of vCPUs.
    pub fn vcpus(&self) -> u8 {
        self.vcpus
    }

    /// Guest memory, in MiB.
    pub fn memory_mib(&self) -> u16 {
        self.memory_mib
    }
}

/// Parses and validates a launch config.
///
/// # Errors
///
/// [`ConfigError::TooLarge`] for input longer than [`MAX_CONFIG_BYTES`] (checked before parsing),
/// [`ConfigError::Json`] for anything serde rejects, including unknown fields, and the other variants for an
/// E-2 rule that failed.
pub fn parse(bytes: &[u8]) -> Result<VmmConfig, ConfigError> {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    let raw: RawVmmConfig = serde_json::from_slice(bytes).map_err(ConfigError::Json)?;
    VmmConfig::try_from(raw)
}

impl TryFrom<RawVmmConfig> for VmmConfig {
    type Error = ConfigError;

    /// Applies every E-2 rule. This is the only way to build a [`VmmConfig`], from serde or by hand.
    fn try_from(raw: RawVmmConfig) -> Result<Self, ConfigError> {
        if raw.v != CONFIG_VERSION {
            return Err(ConfigError::Version(raw.v));
        }

        let mut paths = PathSet::default();
        paths.add("kernel.krunfw", &raw.kernel.krunfw, MAX_PATH_BYTES)?;
        let krunfw_sha256 = sha256("kernel.sha256", &raw.kernel.sha256)?;
        let boot = match raw.boot {
            RawBoot::RootDisk => Boot::RootDisk,
            RawBoot::Initramfs(initramfs) => {
                paths.add("boot.initramfs.path", &initramfs.path, MAX_PATH_BYTES)?;
                let sha256 = sha256("boot.initramfs.sha256", &initramfs.sha256)?;
                Boot::Initramfs {
                    path: initramfs.path,
                    sha256,
                }
            }
        };
        paths.add("disks.root", &raw.disks.root, MAX_PATH_BYTES)?;
        paths.add("disks.input", &raw.disks.input, MAX_PATH_BYTES)?;
        paths.add(
            "disks.scratch.path",
            &raw.disks.scratch.path,
            MAX_PATH_BYTES,
        )?;
        paths.add("vsock.socket", &raw.vsock.socket, MAX_SOCKET_PATH_BYTES)?;
        paths.add("console.path", &raw.console.path, MAX_PATH_BYTES)?;

        let scratch_cap_bytes = in_range(
            "disks.scratch.cap_bytes",
            raw.disks.scratch.cap_bytes,
            1,
            MAX_SCRATCH_CAP_BYTES,
        )?;
        let vsock_port = in_range(
            "vsock.port",
            u64::from(raw.vsock.port),
            1,
            u64::from(MAX_VSOCK_PORT),
        )?;
        let console_cap_bytes = in_range(
            "console.cap_bytes",
            raw.console.cap_bytes,
            1,
            MAX_CONSOLE_CAP_BYTES,
        )?;
        let max_duration_ms: u64 =
            in_range("max_duration_ms", raw.max_duration_ms, 1, MAX_DURATION_MS)?;
        let vcpus = in_range(
            "machine.vcpus",
            u64::from(raw.machine.vcpus),
            1,
            u64::from(MAX_VCPUS),
        )?;
        let memory_mib = in_range(
            "machine.memory_mib",
            u64::from(raw.machine.memory_mib),
            u64::from(MIN_MEMORY_MIB),
            u64::from(MAX_MEMORY_MIB),
        )?;

        Ok(Self {
            krunfw: raw.kernel.krunfw,
            krunfw_sha256,
            boot,
            root: raw.disks.root,
            input: raw.disks.input,
            scratch: raw.disks.scratch.path,
            scratch_cap_bytes,
            vsock_port,
            socket: raw.vsock.socket,
            console: raw.console.path,
            console_cap_bytes,
            max_duration: Duration::from_millis(max_duration_ms),
            vcpus,
            memory_mib,
        })
    }
}

/// Every path field seen so far, keyed by path, so a second field naming the same path is refused.
#[derive(Default)]
struct PathSet {
    seen: BTreeMap<PathBuf, &'static str>,
}

impl PathSet {
    /// Checks one path field against the path rules and against every field added before it.
    fn add(&mut self, field: &'static str, path: &Path, max: usize) -> Result<(), ConfigError> {
        check_path(field, path, max)?;
        match self.seen.insert(path.to_path_buf(), field) {
            Some(first) => Err(ConfigError::DuplicatePath {
                first,
                second: field,
            }),
            None => Ok(()),
        }
    }
}

/// Absolute, every component after the root a plain name, no NUL, at most `max` bytes.
fn check_path(field: &'static str, path: &Path, max: usize) -> Result<(), ConfigError> {
    let fail = |reason| ConfigError::Path { field, reason };
    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&0) {
        return Err(fail(PathReason::Nul));
    }
    if bytes.len() > max {
        return Err(fail(PathReason::TooLong { max }));
    }
    let mut components = path.components();
    let Some(rest) = bytes.strip_prefix(b"/") else {
        return Err(fail(PathReason::NotAbsolute));
    };
    if components.next() != Some(Component::RootDir) {
        return Err(fail(PathReason::NotAbsolute));
    }
    // `Path::components` silently drops `.`, empty, and trailing segments, so the bytes are walked as well:
    // the configured text must already be in the form the preflight compares against `canonicalize`.
    let plain_segments = rest
        .split(|byte| *byte == b'/')
        .all(|segment| !segment.is_empty() && segment != b"." && segment != b"..");
    let plain_components = components.all(|component| matches!(component, Component::Normal(_)));
    if plain_segments && plain_components {
        Ok(())
    } else {
        Err(fail(PathReason::NotNormal))
    }
}

/// Decodes a digest written as exactly 64 lowercase hex characters.
fn sha256(field: &'static str, text: &str) -> Result<[u8; 32], ConfigError> {
    let lower_hex = text.len() == SHA256_HEX_LEN
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    let mut digest = [0_u8; 32];
    if lower_hex && base16ct::lower::decode(text, &mut digest).is_ok() {
        Ok(digest)
    } else {
        Err(ConfigError::Sha256 { field })
    }
}

/// Checks `min <= value <= max` and narrows the value to the field's stored type.
fn in_range<T: TryFrom<u64>>(
    field: &'static str,
    value: u64,
    min: u64,
    max: u64,
) -> Result<T, ConfigError> {
    let out_of_range = ConfigError::OutOfRange {
        field,
        value,
        min,
        max,
    };
    if !(min..=max).contains(&value) {
        return Err(out_of_range);
    }
    T::try_from(value).map_err(|_| out_of_range)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;
    use std::time::Duration;

    use super::{Boot, ConfigError, MAX_CONFIG_BYTES, PathReason, VmmConfig, parse};

    type TestResult = Result<(), Box<dyn Error>>;

    /// E-1 from 02-03-PLAN.md, byte-identical to 02-05's E-1 (one line, no trailing newline).
    const E1: &str = r#"{"v":1,"kernel":{"krunfw":"/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"boot":"root_disk","disks":{"root":"/private/tmp/mns00000000/root.img","input":"/private/tmp/mns00000000/input.img","scratch":{"path":"/private/tmp/mns00000000/scratch.img","cap_bytes":67108864}},"vsock":{"port":1024,"socket":"/private/tmp/mns00000000/o.sock"},"console":{"path":"/private/tmp/mns00000000/console.log","cap_bytes":1048576},"max_duration_ms":60000,"machine":{"vcpus":1,"memory_mib":256}}"#;

    /// E-1b: the `boot` value of initramfs mode.
    const E1B: &str = r#"{"initramfs":{"path":"/private/tmp/mns00000000/initramfs.cpio","sha256":"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"}}"#;

    const ROOT_DISK_BOOT: &str = r#""boot":"root_disk""#;
    const ROOT_PATH: &str = r#""/private/tmp/mns00000000/root.img""#;
    const SOCKET_PATH: &str = r#""/private/tmp/mns00000000/o.sock""#;

    /// `text` with the first `from` replaced by `to`; an error if `from` is missing, so no test is vacuous.
    fn edit(text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        if text.contains(from) {
            Ok(text.replacen(from, to, 1))
        } else {
            Err(format!("the base text does not contain {from}").into())
        }
    }

    fn initramfs_text() -> Result<String, Box<dyn Error>> {
        edit(E1, ROOT_DISK_BOOT, &format!(r#""boot":{E1B}"#))
    }

    fn rejected(text: &str) -> Result<ConfigError, Box<dyn Error>> {
        match parse(text.as_bytes()) {
            Ok(_) => Err("the config was accepted".into()),
            Err(error) => Ok(error),
        }
    }

    fn assert_out_of_range(from: &str, to: &str, expected: &str) -> TestResult {
        let error = rejected(&edit(E1, from, to)?)?;
        assert!(
            matches!(error, ConfigError::OutOfRange { field, .. } if field == expected),
            "expected {expected} out of range, got: {error}"
        );
        Ok(())
    }

    fn assert_path_rule(from: &str, to: &str, expected: &str, reason: PathReason) -> TestResult {
        let error = rejected(&edit(E1, from, to)?)?;
        assert!(
            matches!(error, ConfigError::Path { field, reason: found } if field == expected && found == reason),
            "expected {expected} to break {reason:?}, got: {error}"
        );
        Ok(())
    }

    fn assert_unknown_field(text: &str) -> TestResult {
        let error = rejected(text)?;
        assert!(matches!(error, ConfigError::Json(_)), "got: {error}");
        let shown = match &error {
            ConfigError::Json(source) => source.to_string(),
            other => other.to_string(),
        };
        assert!(shown.contains("unknown field `x`"), "got: {shown}");
        Ok(())
    }

    #[test]
    fn config_e1_parses_in_root_disk_mode() -> TestResult {
        let config = parse(E1.as_bytes())?;
        assert!(matches!(config.boot(), Boot::RootDisk));
        assert_eq!(
            config.krunfw(),
            Path::new("/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib")
        );
        assert_eq!(config.krunfw_sha256().first(), Some(&0x01));
        assert_eq!(config.krunfw_sha256().last(), Some(&0xef));
        assert_eq!(
            config.root(),
            Path::new("/private/tmp/mns00000000/root.img")
        );
        assert_eq!(
            config.input(),
            Path::new("/private/tmp/mns00000000/input.img")
        );
        assert_eq!(
            config.scratch(),
            Path::new("/private/tmp/mns00000000/scratch.img")
        );
        assert_eq!(config.scratch_cap_bytes(), 67_108_864);
        assert_eq!(config.vsock_port(), 1024);
        assert_eq!(
            config.socket(),
            Path::new("/private/tmp/mns00000000/o.sock")
        );
        assert_eq!(
            config.console(),
            Path::new("/private/tmp/mns00000000/console.log")
        );
        assert_eq!(config.console_cap_bytes(), 1_048_576);
        assert_eq!(config.max_duration(), Duration::from_mins(1));
        assert_eq!(config.vcpus(), 1);
        assert_eq!(config.memory_mib(), 256);
        Ok(())
    }

    #[test]
    fn config_e1b_parses_in_initramfs_mode() -> TestResult {
        let config = parse(initramfs_text()?.as_bytes())?;
        let Boot::Initramfs { path, sha256 } = config.boot() else {
            return Err("expected initramfs boot".into());
        };
        assert_eq!(path, Path::new("/private/tmp/mns00000000/initramfs.cpio"));
        assert_eq!(sha256.first(), Some(&0xfe));
        assert_eq!(sha256.last(), Some(&0x10));
        Ok(())
    }

    #[test]
    fn config_serde_try_from_runs_the_same_rules() -> TestResult {
        let config: VmmConfig = serde_json::from_str(E1)?;
        assert_eq!(config.vsock_port(), 1024);
        let refused = serde_json::from_str::<VmmConfig>(&edit(E1, r#""v":1"#, r#""v":2"#)?);
        assert!(refused.is_err());
        Ok(())
    }

    #[test]
    fn config_rejects_version_2() -> TestResult {
        let error = rejected(&edit(E1, r#""v":1"#, r#""v":2"#)?)?;
        assert!(matches!(error, ConfigError::Version(2)), "got: {error}");
        Ok(())
    }

    #[test]
    fn config_rejects_unknown_top_level_key() -> TestResult {
        assert_unknown_field(&edit(E1, r#""v":1,"#, r#""v":1,"x":1,"#)?)
    }

    #[test]
    fn config_rejects_unknown_key_in_every_object() -> TestResult {
        for object in ["kernel", "disks", "scratch", "vsock", "console", "machine"] {
            let from = format!(r#""{object}":{{"#);
            let to = format!(r#""{object}":{{"x":1,"#);
            assert_unknown_field(&edit(E1, &from, &to)?)?;
        }
        Ok(())
    }

    #[test]
    fn config_rejects_unknown_key_in_initramfs() -> TestResult {
        let text = initramfs_text()?;
        assert_unknown_field(&edit(&text, r#""initramfs":{"#, r#""initramfs":{"x":1,"#)?)
    }

    #[test]
    fn config_rejects_unknown_boot_mode() -> TestResult {
        let error = rejected(&edit(E1, ROOT_DISK_BOOT, r#""boot":"virtio_fs""#)?)?;
        assert!(matches!(error, ConfigError::Json(_)), "got: {error}");
        Ok(())
    }

    #[test]
    fn config_rejects_relative_path() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""private/tmp/root.img""#,
            "disks.root",
            PathReason::NotAbsolute,
        )
    }

    #[test]
    fn config_rejects_dot_dot_component() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/../tmp/root.img""#,
            "disks.root",
            PathReason::NotNormal,
        )
    }

    #[test]
    fn config_rejects_dot_component() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/./root.img""#,
            "disks.root",
            PathReason::NotNormal,
        )
    }

    #[test]
    fn config_rejects_empty_component_and_trailing_slash() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""/private//tmp/root.img""#,
            "disks.root",
            PathReason::NotNormal,
        )?;
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/root.img/""#,
            "disks.root",
            PathReason::NotNormal,
        )?;
        assert_path_rule(ROOT_PATH, r#""/""#, "disks.root", PathReason::NotNormal)
    }

    #[test]
    fn config_rejects_nul_in_path() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/ro\u0000ot.img""#,
            "disks.root",
            PathReason::Nul,
        )
    }

    #[test]
    fn config_rejects_1025_byte_path() -> TestResult {
        let long = format!(r#""/{}""#, "a".repeat(1024));
        assert_path_rule(
            ROOT_PATH,
            &long,
            "disks.root",
            PathReason::TooLong { max: 1024 },
        )
    }

    #[test]
    fn config_accepts_1024_byte_path() -> TestResult {
        let longest = format!(r#""/{}""#, "a".repeat(1023));
        parse(edit(E1, ROOT_PATH, &longest)?.as_bytes())?;
        Ok(())
    }

    #[test]
    fn config_rejects_104_byte_socket_path() -> TestResult {
        let long = format!(r#""/{}""#, "s".repeat(103));
        assert_path_rule(
            SOCKET_PATH,
            &long,
            "vsock.socket",
            PathReason::TooLong { max: 103 },
        )
    }

    #[test]
    fn config_accepts_103_byte_socket_path() -> TestResult {
        let longest = format!(r#""/{}""#, "s".repeat(102));
        parse(edit(E1, SOCKET_PATH, &longest)?.as_bytes())?;
        Ok(())
    }

    #[test]
    fn config_rejects_two_fields_with_the_same_path() -> TestResult {
        let error = rejected(&edit(
            E1,
            r#""/private/tmp/mns00000000/input.img""#,
            ROOT_PATH,
        )?)?;
        assert!(
            matches!(
                error,
                ConfigError::DuplicatePath {
                    first: "disks.root",
                    second: "disks.input"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_rejects_initramfs_path_equal_to_root() -> TestResult {
        let text = edit(
            &initramfs_text()?,
            r#""/private/tmp/mns00000000/initramfs.cpio""#,
            ROOT_PATH,
        )?;
        let error = rejected(&text)?;
        assert!(
            matches!(
                error,
                ConfigError::DuplicatePath {
                    first: "boot.initramfs.path",
                    second: "disks.root"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_rejects_63_char_sha256() -> TestResult {
        let error = rejected(&edit(E1, r#"abcdef"}"#, r#"abcde"}"#)?)?;
        assert!(
            matches!(
                error,
                ConfigError::Sha256 {
                    field: "kernel.sha256"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_rejects_uppercase_sha256() -> TestResult {
        let error = rejected(&edit(
            E1,
            r#""sha256":"0123456789abcdef"#,
            r#""sha256":"0123456789ABCDEF"#,
        )?)?;
        assert!(
            matches!(
                error,
                ConfigError::Sha256 {
                    field: "kernel.sha256"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_rejects_bad_initramfs_sha256() -> TestResult {
        let text = edit(&initramfs_text()?, "fedcba", "FEDCBA")?;
        let error = rejected(&text)?;
        assert!(
            matches!(
                error,
                ConfigError::Sha256 {
                    field: "boot.initramfs.sha256"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_rejects_vsock_port_0() -> TestResult {
        assert_out_of_range(r#""port":1024"#, r#""port":0"#, "vsock.port")
    }

    #[test]
    fn config_rejects_vsock_port_u32_max() -> TestResult {
        assert_out_of_range(r#""port":1024"#, r#""port":4294967295"#, "vsock.port")
    }

    #[test]
    fn config_rejects_scratch_cap_0() -> TestResult {
        assert_out_of_range(
            r#""cap_bytes":67108864"#,
            r#""cap_bytes":0"#,
            "disks.scratch.cap_bytes",
        )
    }

    #[test]
    fn config_rejects_scratch_cap_over_16_gib() -> TestResult {
        assert_out_of_range(
            r#""cap_bytes":67108864"#,
            r#""cap_bytes":17179869185"#,
            "disks.scratch.cap_bytes",
        )
    }

    #[test]
    fn config_rejects_console_cap_0() -> TestResult {
        assert_out_of_range(
            r#""cap_bytes":1048576"#,
            r#""cap_bytes":0"#,
            "console.cap_bytes",
        )
    }

    #[test]
    fn config_rejects_console_cap_over_64_mib() -> TestResult {
        assert_out_of_range(
            r#""cap_bytes":1048576"#,
            r#""cap_bytes":67108865"#,
            "console.cap_bytes",
        )
    }

    #[test]
    fn config_rejects_max_duration_0() -> TestResult {
        assert_out_of_range(
            r#""max_duration_ms":60000"#,
            r#""max_duration_ms":0"#,
            "max_duration_ms",
        )
    }

    #[test]
    fn config_rejects_max_duration_over_one_hour() -> TestResult {
        assert_out_of_range(
            r#""max_duration_ms":60000"#,
            r#""max_duration_ms":3600001"#,
            "max_duration_ms",
        )
    }

    #[test]
    fn config_rejects_vcpus_0() -> TestResult {
        assert_out_of_range(r#""vcpus":1"#, r#""vcpus":0"#, "machine.vcpus")
    }

    #[test]
    fn config_rejects_vcpus_9() -> TestResult {
        assert_out_of_range(r#""vcpus":1"#, r#""vcpus":9"#, "machine.vcpus")
    }

    #[test]
    fn config_rejects_memory_127_mib() -> TestResult {
        assert_out_of_range(
            r#""memory_mib":256"#,
            r#""memory_mib":127"#,
            "machine.memory_mib",
        )
    }

    #[test]
    fn config_rejects_memory_16385_mib() -> TestResult {
        assert_out_of_range(
            r#""memory_mib":256"#,
            r#""memory_mib":16385"#,
            "machine.memory_mib",
        )
    }

    #[test]
    fn config_rejects_oversized_input_before_parsing() -> TestResult {
        // Not JSON at all: a size check that ran after parsing would report Json instead.
        let len = usize::try_from(MAX_CONFIG_BYTES + 1)?;
        let error = rejected(&"{".repeat(len))?;
        assert!(matches!(error, ConfigError::TooLarge), "got: {error}");
        Ok(())
    }

    #[test]
    fn config_accepts_input_at_the_size_limit() -> TestResult {
        let len = usize::try_from(MAX_CONFIG_BYTES)?;
        let padded = format!("{E1}{}", " ".repeat(len - E1.len()));
        parse(padded.as_bytes())?;
        Ok(())
    }
}
