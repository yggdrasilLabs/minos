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
// `crates/minos-sandbox/src/spec.rs` restates the next four bounds so preflight never clears a plan this launcher
// refuses; minos-vmm links no internal crate, so both sides pin the same literals in tests (review WR-02).
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
/// The console path that names the helper's own stdout: the helper's own stdout, a pipe the supervisor reads;
/// Seatbelt matches this literal (05-RESEARCH Pattern 7).
///
/// Only this exact byte string is treated as stdout. Preflight skips its console-absent and parent checks for it,
/// and the watchdog does not measure it, because the supervisor reads the pipe through its own cap (KRUN-01).
/// Every other spelling, `/dev/stdout` and `/dev/fd/2` included, is an ordinary console path that must not exist.
pub const STDOUT_CONSOLE: &str = "/dev/fd/1";

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

    /// File that receives the guest console output, or [`STDOUT_CONSOLE`] for the helper's own stdout.
    pub fn console(&self) -> &Path {
        &self.console
    }

    /// Console length past which the watchdog stops the VM, in bytes.
    pub fn console_cap_bytes(&self) -> u64 {
        self.console_cap_bytes
    }

    /// Hard maximum lifetime of the launcher, counted from `main` entry, after which the watchdog stops the VM.
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

    use sha2::{Digest, Sha256};

    use super::{ConfigError, MAX_CONFIG_BYTES, PathReason, VmmConfig, parse};

    type TestResult = Result<(), Box<dyn Error>>;

    /// The golden v2 launch config: one line of compact JSON in the serializer's field order, then a newline.
    /// 05-21 binds `minos-backend-krun`'s serializer to the same bytes (02-CONTEXT.md D-04).
    const GOLDEN: &str = include_str!("../golden/config-v2.json");

    /// SHA-256 of `golden/config-v2.json`, from `shasum -a 256 crates/minos-vmm/golden/config-v2.json`.
    const GOLDEN_SHA256: &str = "0b1a1208888bbb6da4d77d0947a24993b31210aef4f788fb366549ee445f4a7b";

    const ROOT_PATH: &str = r#""/Users/example/project/.minos/blobs/sha256/ab/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd""#;
    const INPUT_PATH: &str = r#""/private/tmp/minos-501/mnsAAAAAAAA/input.img""#;
    const SOCKET_PATH: &str = r#""/private/tmp/minos-501/mnsAAAAAAAA/o.sock""#;

    /// `crates/minos-sandbox/src/spec.rs` restates these four bounds (`MAX_DURATION_MS` as `MAX_LAUNCH_MS`) so
    /// preflight never clears a plan this launcher refuses with exit 64 or 65. minos-vmm may not link an internal
    /// crate, so both sides pin the same literals, and the sandbox side is `launcher_bounds_match_minos_vmm` in
    /// `crates/minos-sandbox/tests/spec.rs`. Change both files and both tests together (review WR-02).
    #[test]
    fn launcher_bounds_are_pinned_for_minos_sandbox() {
        assert_eq!(super::MAX_VCPUS, 8);
        assert_eq!(super::MIN_MEMORY_MIB, 128);
        assert_eq!(super::MAX_MEMORY_MIB, 16_384);
        assert_eq!(super::MAX_DURATION_MS, 3_600_000);
    }

    /// `text` with the first `from` replaced by `to`; an error if `from` is missing, so no test is vacuous.
    fn edit(text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        if text.contains(from) {
            Ok(text.replacen(from, to, 1))
        } else {
            Err(format!("the base text does not contain {from}").into())
        }
    }

    fn rejected(text: &str) -> Result<ConfigError, Box<dyn Error>> {
        match parse(text.as_bytes()) {
            Ok(_) => Err("the config was accepted".into()),
            Err(error) => Ok(error),
        }
    }

    fn assert_out_of_range(from: &str, to: &str, expected: &str) -> TestResult {
        let error = rejected(&edit(GOLDEN, from, to)?)?;
        assert!(
            matches!(error, ConfigError::OutOfRange { field, .. } if field == expected),
            "expected {expected} out of range, got: {error}"
        );
        Ok(())
    }

    fn assert_path_rule(from: &str, to: &str, expected: &str, reason: PathReason) -> TestResult {
        let error = rejected(&edit(GOLDEN, from, to)?)?;
        assert!(
            matches!(error, ConfigError::Path { field, reason: found } if field == expected && found == reason),
            "expected {expected} to break {reason:?}, got: {error}"
        );
        Ok(())
    }

    /// `text` is refused by serde as carrying the unknown field `name`.
    fn assert_unknown_field(text: &str, name: &str) -> TestResult {
        let error = rejected(text)?;
        let shown = match &error {
            ConfigError::Json(source) => source.to_string(),
            other => return Err(format!("expected a JSON error, got: {other}").into()),
        };
        assert!(
            shown.contains(&format!("unknown field `{name}`")),
            "got: {shown}"
        );
        Ok(())
    }

    #[test]
    fn config_v2_golden() -> TestResult {
        let digest: [u8; 32] = Sha256::digest(GOLDEN.as_bytes()).into();
        assert_eq!(base16ct::lower::encode_string(&digest), GOLDEN_SHA256);
        assert_eq!(GOLDEN.lines().count(), 1);
        assert!(GOLDEN.ends_with("}\n"));

        let config = parse(GOLDEN.as_bytes())?;
        assert_eq!(
            config.krunfw(),
            Path::new("/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib")
        );
        assert_eq!(
            base16ct::lower::encode_string(config.krunfw_sha256()),
            "59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217"
        );
        assert_eq!(
            config.root(),
            Path::new(
                "/Users/example/project/.minos/blobs/sha256/ab/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd"
            )
        );
        assert_eq!(
            config.input(),
            Path::new("/private/tmp/minos-501/mnsAAAAAAAA/input.img")
        );
        assert_eq!(config.vsock_port(), 1024);
        assert_eq!(
            config.socket(),
            Path::new("/private/tmp/minos-501/mnsAAAAAAAA/o.sock")
        );
        assert_eq!(config.max_duration(), Duration::from_mins(10));
        assert_eq!(config.vcpus(), 2);
        assert_eq!(config.memory_mib(), 2048);
        Ok(())
    }

    #[test]
    fn config_v2_serde_try_from_runs_the_same_rules() -> TestResult {
        let config: VmmConfig = serde_json::from_str(GOLDEN)?;
        assert_eq!(config.vsock_port(), 1024);
        let refused = serde_json::from_str::<VmmConfig>(&edit(GOLDEN, r#""v":2"#, r#""v":1"#)?);
        assert!(refused.is_err());
        Ok(())
    }

    #[test]
    fn config_v2_refuses_v1() -> TestResult {
        let error = rejected(&edit(GOLDEN, r#""v":2"#, r#""v":1"#)?)?;
        assert!(matches!(error, ConfigError::Version(1)), "got: {error}");
        let error = rejected(&edit(GOLDEN, r#""v":2"#, r#""v":3"#)?)?;
        assert!(matches!(error, ConfigError::Version(3)), "got: {error}");
        Ok(())
    }

    /// D-05 moved scratch into guest memory and 05-RESEARCH Open Question 7 removed the initramfs mode, so the
    /// v1 scratch object and boot field are unknown fields in v2.
    #[test]
    fn config_v2_refuses_removed_fields() -> TestResult {
        let scratch = edit(
            GOLDEN,
            INPUT_PATH,
            &format!(
                r#"{INPUT_PATH},"scratch":{{"path":"/private/tmp/minos-501/mnsAAAAAAAA/scratch.img","cap_bytes":67108864}}"#
            ),
        )?;
        assert_unknown_field(&scratch, "scratch")?;
        assert_unknown_field(
            &edit(GOLDEN, r#""v":2,"#, r#""v":2,"boot":"root_disk","#)?,
            "boot",
        )?;
        let initramfs = r#""boot":{"initramfs":{"path":"/private/tmp/minos-501/mnsAAAAAAAA/initramfs.cpio","sha256":"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"}},"#;
        assert_unknown_field(
            &edit(GOLDEN, r#""v":2,"#, &format!(r#""v":2,{initramfs}"#))?,
            "boot",
        )
    }

    /// The console is the helper's stdout pipe, a constant, so a console object is an unknown field.
    #[test]
    fn config_v2_has_no_console_object() -> TestResult {
        let console = r#""console":{"path":"/dev/fd/1","cap_bytes":1048576},"#;
        let text = edit(
            GOLDEN,
            r#""max_duration_ms":"#,
            &format!(r#"{console}"max_duration_ms":"#),
        )?;
        assert_unknown_field(&text, "console")
    }

    #[test]
    fn config_v2_rejects_unknown_key_in_every_object() -> TestResult {
        assert_unknown_field(&edit(GOLDEN, r#""v":2,"#, r#""v":2,"x":1,"#)?, "x")?;
        for object in ["kernel", "disks", "vsock", "machine"] {
            let from = format!(r#""{object}":{{"#);
            let to = format!(r#""{object}":{{"x":1,"#);
            assert_unknown_field(&edit(GOLDEN, &from, &to)?, "x")?;
        }
        Ok(())
    }

    #[test]
    fn config_v2_keeps_path_rules() -> TestResult {
        assert_path_rule(
            ROOT_PATH,
            r#""private/tmp/root.img""#,
            "disks.root",
            PathReason::NotAbsolute,
        )?;
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/../tmp/root.img""#,
            "disks.root",
            PathReason::NotNormal,
        )?;
        assert_path_rule(
            ROOT_PATH,
            r#""/private/tmp/./root.img""#,
            "disks.root",
            PathReason::NotNormal,
        )?;
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
        assert_path_rule(ROOT_PATH, r#""/""#, "disks.root", PathReason::NotNormal)?;
        assert_path_rule(
            INPUT_PATH,
            r#""/private/tmp/in\u0000put.img""#,
            "disks.input",
            PathReason::Nul,
        )?;
        assert_path_rule(
            SOCKET_PATH,
            r#""relative.sock""#,
            "vsock.socket",
            PathReason::NotAbsolute,
        )
    }

    #[test]
    fn config_v2_path_length_limits() -> TestResult {
        let long = format!(r#""/{}""#, "a".repeat(1024));
        assert_path_rule(
            ROOT_PATH,
            &long,
            "disks.root",
            PathReason::TooLong { max: 1024 },
        )?;
        let longest = format!(r#""/{}""#, "a".repeat(1023));
        parse(edit(GOLDEN, ROOT_PATH, &longest)?.as_bytes())?;

        let long = format!(r#""/{}""#, "s".repeat(103));
        assert_path_rule(
            SOCKET_PATH,
            &long,
            "vsock.socket",
            PathReason::TooLong { max: 103 },
        )?;
        let longest = format!(r#""/{}""#, "s".repeat(102));
        parse(edit(GOLDEN, SOCKET_PATH, &longest)?.as_bytes())?;
        Ok(())
    }

    #[test]
    fn config_v2_rejects_two_fields_with_the_same_path() -> TestResult {
        let error = rejected(&edit(GOLDEN, INPUT_PATH, ROOT_PATH)?)?;
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
        let error = rejected(&edit(GOLDEN, SOCKET_PATH, INPUT_PATH)?)?;
        assert!(
            matches!(
                error,
                ConfigError::DuplicatePath {
                    first: "disks.input",
                    second: "vsock.socket"
                }
            ),
            "got: {error}"
        );
        Ok(())
    }

    #[test]
    fn config_v2_rejects_bad_sha256() -> TestResult {
        for (from, to) in [(r#"8e5217""#, r#"8e521""#), (r#"bbdd5218"#, r#"BBDD5218"#)] {
            let error = rejected(&edit(GOLDEN, from, to)?)?;
            assert!(
                matches!(
                    error,
                    ConfigError::Sha256 {
                        field: "kernel.sha256"
                    }
                ),
                "got: {error}"
            );
        }
        Ok(())
    }

    #[test]
    fn config_v2_rejects_out_of_range_numbers() -> TestResult {
        assert_out_of_range(r#""port":1024"#, r#""port":0"#, "vsock.port")?;
        assert_out_of_range(r#""port":1024"#, r#""port":4294967295"#, "vsock.port")?;
        assert_out_of_range(
            r#""max_duration_ms":600000"#,
            r#""max_duration_ms":0"#,
            "max_duration_ms",
        )?;
        assert_out_of_range(
            r#""max_duration_ms":600000"#,
            r#""max_duration_ms":3600001"#,
            "max_duration_ms",
        )?;
        assert_out_of_range(r#""vcpus":2"#, r#""vcpus":0"#, "machine.vcpus")?;
        assert_out_of_range(r#""vcpus":2"#, r#""vcpus":9"#, "machine.vcpus")?;
        assert_out_of_range(
            r#""memory_mib":2048"#,
            r#""memory_mib":127"#,
            "machine.memory_mib",
        )?;
        assert_out_of_range(
            r#""memory_mib":2048"#,
            r#""memory_mib":16385"#,
            "machine.memory_mib",
        )
    }

    #[test]
    fn config_v2_accepts_the_range_bounds() -> TestResult {
        for (from, to) in [
            (r#""port":1024"#, r#""port":1"#),
            (r#""port":1024"#, r#""port":4294967294"#),
            (r#""max_duration_ms":600000"#, r#""max_duration_ms":1"#),
            (
                r#""max_duration_ms":600000"#,
                r#""max_duration_ms":3600000"#,
            ),
            (r#""vcpus":2"#, r#""vcpus":1"#),
            (r#""vcpus":2"#, r#""vcpus":8"#),
            (r#""memory_mib":2048"#, r#""memory_mib":128"#),
            (r#""memory_mib":2048"#, r#""memory_mib":16384"#),
        ] {
            parse(edit(GOLDEN, from, to)?.as_bytes())?;
        }
        Ok(())
    }

    #[test]
    fn config_v2_rejects_oversized_input_before_parsing() -> TestResult {
        // Not JSON at all: a size check that ran after parsing would report Json instead.
        let len = usize::try_from(MAX_CONFIG_BYTES + 1)?;
        let error = rejected(&"{".repeat(len))?;
        assert!(matches!(error, ConfigError::TooLarge), "got: {error}");
        Ok(())
    }

    #[test]
    fn config_v2_accepts_input_at_the_size_limit() -> TestResult {
        let len = usize::try_from(MAX_CONFIG_BYTES)?;
        let padding = len
            .checked_sub(GOLDEN.len())
            .ok_or("the golden config is longer than the size limit")?;
        let padded = format!("{GOLDEN}{}", " ".repeat(padding));
        parse(padded.as_bytes())?;
        Ok(())
    }
}
