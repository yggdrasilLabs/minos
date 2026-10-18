//! The krun backend's host probes: every launch precondition, checked once, as [`Capabilities`] and the
//! [`Refusal`]s that `minos host doctor` explains (KRUN-06, KRUN-07, KRUN-08; 05-RESEARCH Pattern 9).
//!
//! [`probe_host`] runs at backend construction and in `minos host doctor`, not per launch, so the runner does not
//! re-run `codesign` and re-hash a 24 MB kernel for every VM. Each failed check is one [`Refusal`] whose `Display`
//! carries the fix, and every check is listed in the [`HostReport`] with the paths it looked at and what it saw
//! (05-CONTEXT D-04: `doctor` names every path it checked). `minos-vmm` re-hashes libkrunfw before every `dlopen`
//! anyway (02-SPIKE-EVIDENCE.md K5), so this probe is the early, explained refusal, not the only integrity check.
//!
//! Every host tool runs as an absolute-path argv with the environment cleared to [`TOOL_PATH`], stdin from
//! `/dev/null`, capped stdout and stderr, its own process group, and [`TOOL_DEADLINE`]. Production code reads no
//! environment variable: the CLI locates the binaries and passes them in [`HostPaths`] (D-04). Tool output that
//! reaches a [`ProbeOutcome`] detail passes through [`SafeText`] first.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use minos_core::id::ImageDigest;
use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
use minos_core::manifest::Arch;
use minos_core::text::SafeText;
use minos_sandbox::spec::{MAX_LAUNCH_MS, MAX_MEMORY_MIB, MAX_VCPUS, MIN_MEMORY_MIB, Machine};
use minos_sandbox::{Capabilities, Refusal};
use rustix::process::{Pid, Signal, kill_process_group};
use tempfile::TempDir;

use crate::jobdir::{JOB_PREFIX, JOB_RANDOM_CHARS, ROOT_PREFIX, SOCKET_FILE};
use crate::rootfs::check_guest_elf;
use crate::seatbelt::{self, ProfileParams, SANDBOX_EXEC};
use crate::supervise::{MAX_SOCKET_PATH_BYTES, sha256_file};

/// The pinned libkrunfw dylib, at its Homebrew Cellar path (05-CONTEXT D-04; 02-SPIKE-EVIDENCE.md H-01).
pub const KRUNFW_PATH: &str = "/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib";

/// SHA-256 of libkrunfw 5.6.2 at [`KRUNFW_PATH`] (02-01; re-verified 2026-10-18).
pub const KRUNFW_SHA256: &str = "59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217";

/// The Homebrew link that names [`KRUNFW_PATH`], which the Seatbelt profile also allows the helper to read.
pub const KRUNFW_LINK: &str = "/opt/homebrew/lib/libkrunfw.5.dylib";

/// The entitlements the signed `minos-vmm` must carry, each set to true (05-CONTEXT D-02; 02-SPIKE-EVIDENCE.md
/// S04-SIGN, K5): without the first `hv_vm_create` fails, and without the second the hardened runtime refuses
/// the Homebrew libkrunfw dylib.
pub const REQUIRED_ENTITLEMENTS: [&str; 2] = [
    "com.apple.security.hypervisor",
    "com.apple.security.cs.disable-library-validation",
];

/// The only `PATH` a host tool sees.
pub const TOOL_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// How long one host tool may run before it is killed and its probe fails.
pub const TOOL_DEADLINE: Duration = Duration::from_secs(5);

/// The most stdout bytes read from `codesign`; more is a failed check.
pub const CODESIGN_OUTPUT_CAP: u64 = 65_536;

/// `codesign`, by absolute path.
pub const CODESIGN: &str = "/usr/bin/codesign";

/// `sysctl`, by absolute path.
pub const SYSCTL: &str = "/usr/sbin/sysctl";

/// `nc`, the program the network denial probe runs under the profile.
pub const NC: &str = "/usr/bin/nc";

/// `cat`, the program the file denial probe runs under the profile.
pub const CAT: &str = "/bin/cat";

/// The most stdout bytes read from `sysctl`, `nc`, or `cat`.
const TOOL_OUTPUT_CAP: u64 = 4096;

/// The most stderr bytes read from any host tool; stderr only feeds a probe detail.
const TOOL_STDERR_CAP: u64 = 4096;

/// The most stderr bytes shown in a probe detail.
const DETAIL_STDERR_BYTES: usize = 240;

/// The largest guest binary the guest probe reads.
const GUEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Pause between polls of a running tool.
const POLL: Duration = Duration::from_millis(5);

/// How long the network probe waits for the control connection to reach the listener.
const CONTROL_ACCEPT_WINDOW: Duration = Duration::from_secs(1);

/// The bytes of the file the profile lists, which the file probe must read back exactly.
const LISTED_BYTES: &[u8] = b"minos-seatbelt-listed\n";

/// The bytes of the file the profile does not list, which must never come back.
const UNLISTED_BYTES: &[u8] = b"minos-seatbelt-unlisted\n";

/// The smallest machine, used as the host maximum when the hypervisor probe could not read the host's size; the
/// probe then refuses, so nothing launches on it.
const MIN_MACHINE: Machine = match Machine::new(1, MIN_MEMORY_MIB) {
    Ok(machine) => machine,
    Err(_) => panic!("1 vCPU and MIN_MEMORY_MIB is a valid machine"),
};

/// Where the backend's binaries and kernel are, as the CLI found them (05-CONTEXT D-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPaths {
    vmm: PathBuf,
    guest: PathBuf,
    krunfw: PathBuf,
}

impl HostPaths {
    /// The `minos-vmm` helper, the static aarch64 `minos-guest`, and the libkrunfw dylib. Production passes
    /// [`KRUNFW_PATH`]; only the ignored VM tests take these from `MINOS_*` variables.
    #[must_use]
    pub fn new(
        vmm: impl Into<PathBuf>,
        guest: impl Into<PathBuf>,
        krunfw: impl Into<PathBuf>,
    ) -> Self {
        Self {
            vmm: vmm.into(),
            guest: guest.into(),
            krunfw: krunfw.into(),
        }
    }

    /// The `minos-vmm` helper.
    #[must_use]
    pub fn vmm(&self) -> &Path {
        &self.vmm
    }

    /// The `minos-guest` binary written into every root image as `/init`.
    #[must_use]
    pub fn guest(&self) -> &Path {
        &self.guest
    }

    /// The libkrunfw dylib.
    #[must_use]
    pub fn krunfw(&self) -> &Path {
        &self.krunfw
    }
}

/// One host check, in the order [`probe_host`] runs them.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Check {
    /// The helper is a regular file.
    Vmm,
    /// The helper is signed with both [`REQUIRED_ENTITLEMENTS`].
    Entitlements,
    /// libkrunfw is present and matches [`KRUNFW_SHA256`].
    Kernel,
    /// `kern.hv_support` is 1, and the host's size sets the largest VM.
    Hypervisor,
    /// The longest job socket path fits a Unix socket address.
    SocketPath,
    /// `/usr/bin/sandbox-exec` is present.
    SandboxExec,
    /// The profile denies a loopback TCP connect.
    SeatbeltNetwork,
    /// The profile denies an unlisted file read and allows a listed one.
    SeatbeltFile,
    /// The guest is a static aarch64 ELF.
    Guest,
}

impl Check {
    /// A short name for the check, as `minos host doctor` prints it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vmm => "helper present",
            Self::Entitlements => "helper entitlements",
            Self::Kernel => "guest kernel",
            Self::Hypervisor => "hypervisor",
            Self::SocketPath => "job socket path",
            Self::SandboxExec => "sandbox-exec present",
            Self::SeatbeltNetwork => "Seatbelt denies network",
            Self::SeatbeltFile => "Seatbelt denies unlisted files",
            Self::Guest => "guest binary",
        }
    }
}

/// What one check found. The detail is sanitized host-tool output and the probe's own observations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// The check passed.
    Passed {
        /// What the check saw.
        detail: SafeText,
    },
    /// The check failed, refusing every plan on this host.
    Refused {
        /// The refusal, whose `Display` carries the fix.
        refusal: Refusal,
        /// What the check saw.
        detail: SafeText,
    },
}

/// One check, the paths it looked at, and its outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    check: Check,
    paths: Vec<PathBuf>,
    outcome: ProbeOutcome,
}

impl Probe {
    /// A passed check.
    fn passed(check: Check, paths: Vec<PathBuf>, detail: &str) -> Self {
        Self {
            check,
            paths,
            outcome: ProbeOutcome::Passed {
                detail: SafeText::inline(detail),
            },
        }
    }

    /// A failed check.
    fn refused(check: Check, paths: Vec<PathBuf>, refusal: Refusal, detail: &str) -> Self {
        Self {
            check,
            paths,
            outcome: ProbeOutcome::Refused {
                refusal,
                detail: SafeText::inline(detail),
            },
        }
    }

    /// A check that passed with `Ok(detail)` or failed with `refusal` and `Err(detail)`.
    fn from_result(
        check: Check,
        paths: Vec<PathBuf>,
        refusal: Refusal,
        result: Result<String, String>,
    ) -> Self {
        match result {
            Ok(detail) => Self::passed(check, paths, &detail),
            Err(detail) => Self::refused(check, paths, refusal, &detail),
        }
    }

    /// Which check this is.
    #[must_use]
    pub const fn check(&self) -> Check {
        self.check
    }

    /// The files and tools the check looked at.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// What the check found.
    #[must_use]
    pub const fn outcome(&self) -> &ProbeOutcome {
        &self.outcome
    }

    /// Whether the check passed.
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self.outcome, ProbeOutcome::Passed { .. })
    }

    /// The refusal, when the check failed.
    #[must_use]
    pub const fn refusal(&self) -> Option<Refusal> {
        match self.outcome {
            ProbeOutcome::Passed { .. } => None,
            ProbeOutcome::Refused { refusal, .. } => Some(refusal),
        }
    }
}

/// Every check [`probe_host`] ran, and the [`Capabilities`] they add up to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostReport {
    probes: Vec<Probe>,
    capabilities: Capabilities,
}

impl HostReport {
    /// Every check, in the order it ran.
    #[must_use]
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// What the host can run: its refusals, layers, largest VM, and kernel digest.
    #[must_use]
    pub const fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// The capabilities, for a backend that keeps them and drops the report.
    #[must_use]
    pub fn into_capabilities(self) -> Capabilities {
        self.capabilities
    }
}

/// Checks every launch precondition once and returns each check with the [`Capabilities`] it implies.
///
/// The checks, in order: the helper is a regular file ([`Refusal::VmmMissing`]); it is signed with both
/// [`REQUIRED_ENTITLEMENTS`] ([`Refusal::EntitlementMissing`]); libkrunfw exists ([`Refusal::KernelMissing`]) and
/// matches [`KRUNFW_SHA256`] ([`Refusal::KernelHashMismatch`]); `kern.hv_support` is 1
/// ([`Refusal::HypervisorUnsupported`]), with `hw.ncpu` and `hw.memsize` setting the largest VM; the longest job
/// socket path under `job_root` fits [`MAX_SOCKET_PATH_BYTES`] ([`Refusal::SocketPathTooLong`]);
/// [`SANDBOX_EXEC`] exists ([`Refusal::SeatbeltMissing`]); the production profile denies a loopback TCP connect
/// and an unlisted file read ([`Refusal::SeatbeltNotEnforcing`]); and the guest is a static aarch64 ELF
/// ([`Refusal::GuestMissing`]). `job_root` is the directory the job root lives in (`/private/tmp` in
/// production); the denial probes work in a private directory there and remove it.
#[must_use]
pub fn probe_host(paths: &HostPaths, job_root: &Path) -> HostReport {
    let job_root = fs::canonicalize(job_root).unwrap_or_else(|_| job_root.to_path_buf());
    let mut probes = Vec::with_capacity(9);
    let vmm = probe_vmm(paths.vmm());
    let vmm_present = vmm.is_passed();
    probes.push(vmm);
    probes.push(probe_entitlements(paths.vmm(), vmm_present));
    let (kernel, digest) = probe_kernel(paths.krunfw(), KRUNFW_SHA256);
    probes.push(kernel);
    let (hypervisor, max) = probe_hypervisor();
    probes.push(hypervisor);
    probes.push(probe_socket_path(
        &job_root,
        rustix::process::getuid().as_raw(),
    ));
    let sandbox_exec = probe_sandbox_exec(Path::new(SANDBOX_EXEC));
    let sandbox_exec_present = sandbox_exec.is_passed();
    probes.push(sandbox_exec);
    probes.extend(probe_seatbelt(&job_root, sandbox_exec_present));
    probes.push(probe_guest(paths.guest()));
    report(probes, max, digest)
}

/// Builds the report: every refused check's refusal, the layers [`layers_for`] allows, and the kernel digest.
fn report(probes: Vec<Probe>, max: Machine, kernel: Option<ImageDigest>) -> HostReport {
    let refusals: BTreeSet<Refusal> = probes.iter().filter_map(Probe::refusal).collect();
    let layers = layers_for(&probes);
    let capabilities = Capabilities::new(
        BackendKind::Krun,
        Arch::Arm64,
        max,
        MAX_LAUNCH_MS,
        Isolation::Hardware,
        refusals,
    )
    .with_layers(layers);
    let capabilities = match kernel {
        Some(digest) => capabilities.with_kernel(digest),
        None => capabilities,
    };
    HostReport {
        probes,
        capabilities,
    }
}

/// The confinement layers the checks prove are active.
///
/// - [`ConfinementLayer::Seatbelt`] when [`SANDBOX_EXEC`] is present and both denial probes passed: the profile
///   is applied to every helper and was just seen denying a loopback connect and an unlisted read.
/// - [`ConfinementLayer::Rlimits`] when the helper checks (present, signed with both keys) and the guest check
///   (a static aarch64 ELF) passed. The layer covers both halves 05-CONTEXT D-01 names ("in-guest rlimits"): the
///   helper's own limits, which it sets before reading its config and without which it exits 66 (plan 05-07),
///   and the guest's per-stage limits on the candidate, which the trampoline sets before exec and without which
///   it never execs (plans 05-08, 05-12). Listing it only when both binaries are the verified ones means a
///   recorded `Rlimits` always has both halves applied.
/// - Never [`ConfinementLayer::DedicatedUser`]: the `_minos` user is deferred (D-01), so every v1 run records it
///   as missing.
///
/// A check counts as passed only when it ran and every probe of it passed.
fn layers_for(probes: &[Probe]) -> BTreeSet<ConfinementLayer> {
    let passed = |check: Check| {
        let mut runs = probes
            .iter()
            .filter(|probe| probe.check == check)
            .peekable();
        runs.peek().is_some() && runs.all(Probe::is_passed)
    };
    let mut layers = BTreeSet::new();
    if passed(Check::SandboxExec) && passed(Check::SeatbeltNetwork) && passed(Check::SeatbeltFile) {
        layers.insert(ConfinementLayer::Seatbelt);
    }
    if passed(Check::Vmm) && passed(Check::Entitlements) && passed(Check::Guest) {
        layers.insert(ConfinementLayer::Rlimits);
    }
    layers
}

/// Whether codesign's XML entitlements set every one of [`REQUIRED_ENTITLEMENTS`] to true.
///
/// Each key must appear as a `<key>…</key>` element, and every appearance must be followed, after any ASCII
/// whitespace, by `<true/>`; a key that is absent, false, or present once true and once false fails. This reads
/// the signature of the helper Minos signs itself, so it is a check against a mis-signed build, not a parser for
/// hostile plists.
#[must_use]
pub fn entitlements_present(xml: &[u8]) -> bool {
    REQUIRED_ENTITLEMENTS
        .iter()
        .all(|key| key_is_true(xml, key))
}

/// Whether `key` appears in `xml` and every appearance is followed by `<true/>`.
fn key_is_true(xml: &[u8], key: &str) -> bool {
    const TRUE: &[u8] = b"<true/>";
    let element = format!("<key>{key}</key>");
    let element = element.as_bytes();
    let mut seen = false;
    for (at, window) in xml.windows(element.len()).enumerate() {
        if window != element {
            continue;
        }
        seen = true;
        let rest = xml
            .get(at.saturating_add(element.len())..)
            .unwrap_or_default();
        if !rest.trim_ascii_start().starts_with(TRUE) {
            return false;
        }
    }
    seen
}

/// Whether the binary at `vmm` is signed with both [`REQUIRED_ENTITLEMENTS`] set to true, by running
/// `codesign --display --entitlements - --xml <vmm>` (05-CONTEXT D-02). An unsigned binary, which codesign
/// reports with a non-zero exit, is `false`.
///
/// # Errors
///
/// The `io::Error` when codesign cannot start, passes [`TOOL_DEADLINE`], or writes more than
/// [`CODESIGN_OUTPUT_CAP`] bytes.
pub fn signed_with_entitlements(vmm: &Path) -> io::Result<bool> {
    let args = [
        OsStr::new("--display"),
        OsStr::new("--entitlements"),
        OsStr::new("-"),
        OsStr::new("--xml"),
        vmm.as_os_str(),
    ];
    let output = run_tool(Path::new(CODESIGN), &args, CODESIGN_OUTPUT_CAP)?;
    Ok(output.status.success() && entitlements_present(&output.stdout))
}

/// The helper is a regular file.
fn probe_vmm(vmm: &Path) -> Probe {
    let paths = vec![vmm.to_path_buf()];
    match fs::metadata(vmm) {
        Ok(meta) if meta.is_file() => Probe::passed(
            Check::Vmm,
            paths,
            &format!("a regular file of {} bytes", meta.len()),
        ),
        Ok(_) => Probe::refused(Check::Vmm, paths, Refusal::VmmMissing, "not a regular file"),
        Err(error) => Probe::refused(Check::Vmm, paths, Refusal::VmmMissing, &error.to_string()),
    }
}

/// The helper is signed with both entitlement keys. When the helper is missing, nothing is checked and the
/// check fails closed.
fn probe_entitlements(vmm: &Path, vmm_present: bool) -> Probe {
    let paths = vec![vmm.to_path_buf(), PathBuf::from(CODESIGN)];
    let refused = |detail: &str| {
        Probe::refused(
            Check::Entitlements,
            paths.clone(),
            Refusal::EntitlementMissing,
            detail,
        )
    };
    if !vmm_present {
        return refused("not checked: the helper was not found");
    }
    match signed_with_entitlements(vmm) {
        Ok(true) => Probe::passed(
            Check::Entitlements,
            paths.clone(),
            "both entitlements are set to true",
        ),
        Ok(false) => refused("codesign does not show both entitlements set to true"),
        Err(error) => refused(&format!("codesign failed: {error}")),
    }
}

/// libkrunfw exists and its SHA-256 is `pinned`; the digest is returned only when it matches.
fn probe_kernel(krunfw: &Path, pinned: &str) -> (Probe, Option<ImageDigest>) {
    let paths = vec![krunfw.to_path_buf()];
    let missing =
        |detail: &str| Probe::refused(Check::Kernel, paths.clone(), Refusal::KernelMissing, detail);
    match fs::metadata(krunfw) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return (missing("not a regular file"), None),
        Err(error) => return (missing(&error.to_string()), None),
    }
    let actual = match sha256_file(krunfw) {
        Ok(actual) => actual,
        Err(error) => return (missing(&error.to_string()), None),
    };
    let mismatch = |detail: &str| {
        Probe::refused(
            Check::Kernel,
            paths.clone(),
            Refusal::KernelHashMismatch,
            detail,
        )
    };
    if actual != pinned {
        return (mismatch(&format!("sha256 {actual}, pinned {pinned}")), None);
    }
    match actual.parse::<ImageDigest>() {
        Ok(digest) => (
            Probe::passed(Check::Kernel, paths.clone(), &format!("sha256 {actual}")),
            Some(digest),
        ),
        Err(_) => (
            mismatch("the digest is not 64 lowercase hex characters"),
            None,
        ),
    }
}

/// `kern.hv_support` is 1, and `hw.ncpu` and `hw.memsize` give the largest VM. When the host's size cannot be
/// read the check fails and the largest VM is the smallest one.
fn probe_hypervisor() -> (Probe, Machine) {
    let paths = vec![PathBuf::from(SYSCTL)];
    let refused = |detail: &str| {
        Probe::refused(
            Check::Hypervisor,
            paths.clone(),
            Refusal::HypervisorUnsupported,
            detail,
        )
    };
    let args = ["-n", "kern.hv_support", "hw.ncpu", "hw.memsize"];
    let output = match run_tool(Path::new(SYSCTL), &args, TOOL_OUTPUT_CAP) {
        Ok(output) => output,
        Err(error) => return (refused(&format!("sysctl failed: {error}")), MIN_MACHINE),
    };
    if !output.status.success() {
        let detail = format!(
            "sysctl exited {}: {}",
            status_text(output.status),
            excerpt(&output.stderr)
        );
        return (refused(&detail), MIN_MACHINE);
    }
    let Some((hv_support, ncpu, memsize)) = parse_sysctl(&output.stdout) else {
        return (
            refused("sysctl printed something other than three values"),
            MIN_MACHINE,
        );
    };
    let max = max_machine(ncpu, memsize);
    if !hv_support {
        return (refused("kern.hv_support is not 1"), max);
    }
    let detail = format!(
        "kern.hv_support 1, hw.ncpu {ncpu}, hw.memsize {memsize}; largest VM {} vCPUs and {} MiB, leaving half the \
         host's memory to the host",
        max.vcpus(),
        max.memory_mib()
    );
    (Probe::passed(Check::Hypervisor, paths, &detail), max)
}

/// The output of `sysctl -n kern.hv_support hw.ncpu hw.memsize`: exactly three lines, the first `1` or not.
fn parse_sysctl(stdout: &[u8]) -> Option<(bool, u64, u64)> {
    let text = stdout.strip_suffix(b"\n").unwrap_or(stdout);
    let mut lines = text.split(|byte| *byte == b'\n');
    let hv = lines.next()?;
    let ncpu = parse_u64(lines.next()?)?;
    let memsize = parse_u64(lines.next()?)?;
    if lines.next().is_some() {
        return None;
    }
    Some((parse_hv_support(hv), ncpu, memsize))
}

/// Whether one `kern.hv_support` value is exactly `1`, with an optional trailing newline.
fn parse_hv_support(value: &[u8]) -> bool {
    value.strip_suffix(b"\n").unwrap_or(value) == b"1"
}

/// A plain decimal `u64` with an optional trailing newline: digits only, no sign, no space.
fn parse_u64(value: &[u8]) -> Option<u64> {
    let digits = value.strip_suffix(b"\n").unwrap_or(value);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// The largest VM on a host with `ncpu` CPUs and `memsize` bytes: at most [`MAX_VCPUS`] vCPUs, and half the
/// host's memory (leave half to the host) capped at [`MAX_MEMORY_MIB`], never below the smallest machine.
fn max_machine(ncpu: u64, memsize: u64) -> Machine {
    let vcpus = u8::try_from(ncpu.clamp(1, u64::from(MAX_VCPUS))).unwrap_or(MAX_VCPUS);
    let half_mib = memsize / (1024 * 1024) / 2;
    let memory =
        u16::try_from(half_mib.clamp(u64::from(MIN_MEMORY_MIB), u64::from(MAX_MEMORY_MIB)))
            .unwrap_or(MAX_MEMORY_MIB);
    Machine::new(vcpus, memory).unwrap_or(MIN_MACHINE)
}

/// The longest socket path a job under `job_root` gets: `<job_root>/minos-<uid>/mnsXXXXXXXX/o.sock`.
fn longest_socket_path(job_root: &Path, uid: u32) -> PathBuf {
    let job = format!("{JOB_PREFIX}{}", "X".repeat(JOB_RANDOM_CHARS));
    job_root
        .join(format!("{ROOT_PREFIX}{uid}"))
        .join(job)
        .join(SOCKET_FILE)
}

/// The longest job socket path fits [`MAX_SOCKET_PATH_BYTES`].
fn probe_socket_path(job_root: &Path, uid: u32) -> Probe {
    let socket = longest_socket_path(job_root, uid);
    let len = socket.as_os_str().len();
    let detail = format!("{len} of at most {MAX_SOCKET_PATH_BYTES} bytes");
    if len <= MAX_SOCKET_PATH_BYTES {
        Probe::passed(Check::SocketPath, vec![socket], &detail)
    } else {
        Probe::refused(
            Check::SocketPath,
            vec![socket],
            Refusal::SocketPathTooLong,
            &detail,
        )
    }
}

/// `sandbox-exec` is a regular file at `path`.
fn probe_sandbox_exec(path: &Path) -> Probe {
    let paths = vec![path.to_path_buf()];
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Probe::passed(Check::SandboxExec, paths, "a regular file"),
        Ok(_) => Probe::refused(
            Check::SandboxExec,
            paths,
            Refusal::SeatbeltMissing,
            "not a regular file",
        ),
        Err(error) => Probe::refused(
            Check::SandboxExec,
            paths,
            Refusal::SeatbeltMissing,
            &error.to_string(),
        ),
    }
}

/// The two Seatbelt denial probes (05-CONTEXT D-03), run offline against the production profile in a private
/// directory under `job_root` that is removed afterwards.
///
/// The file probe runs first: under the profile with `VMM=/bin/cat` and `CONFIG` naming a listed file, `cat` must
/// read the listed file back exactly and must fail on an unlisted file in the same directory. The listed read is
/// the proof that `sandbox-exec` applied the profile and ran the program, so the network probe runs only after
/// it. The network probe binds a probe-owned listener on 127.0.0.1; an unsandboxed `nc -z` must connect to it
/// first, then the same `nc` under the profile (`VMM=/usr/bin/nc`) must exit non-zero while the listener accepts
/// nothing. A failed control or a failed listed read is inconclusive and fails closed as
/// [`Refusal::SeatbeltNotEnforcing`], as does a probe that could not run at all.
fn probe_seatbelt(job_root: &Path, sandbox_exec_present: bool) -> [Probe; 2] {
    let paths = vec![
        PathBuf::from(SANDBOX_EXEC),
        PathBuf::from(NC),
        PathBuf::from(CAT),
    ];
    let both = |detail: &str| {
        [
            Probe::refused(
                Check::SeatbeltNetwork,
                paths.clone(),
                Refusal::SeatbeltNotEnforcing,
                detail,
            ),
            Probe::refused(
                Check::SeatbeltFile,
                paths.clone(),
                Refusal::SeatbeltNotEnforcing,
                detail,
            ),
        ]
    };
    if !sandbox_exec_present {
        return both("not run: sandbox-exec is missing");
    }
    let dir = match probe_dir(job_root) {
        Ok(dir) => dir,
        Err(error) => {
            return both(&format!(
                "inconclusive: could not create a probe directory in {}: {error}",
                job_root.display()
            ));
        }
    };
    let file = file_probe(&dir.1);
    let network = match file {
        Ok(_) => network_probe(&dir.1),
        Err(_) => Err(
            "inconclusive: the profile did not run a listed read (see the file probe)".to_owned(),
        ),
    };
    [
        Probe::from_result(
            Check::SeatbeltNetwork,
            paths.clone(),
            Refusal::SeatbeltNotEnforcing,
            network,
        ),
        Probe::from_result(
            Check::SeatbeltFile,
            paths,
            Refusal::SeatbeltNotEnforcing,
            file,
        ),
    ]
}

/// A fresh 0700 directory `mnpXXXXXXXX` under `job_root`, with its canonical path, which Seatbelt rules compare
/// against. Dropping the [`TempDir`] removes it.
fn probe_dir(job_root: &Path) -> io::Result<(TempDir, PathBuf)> {
    let dir = tempfile::Builder::new()
        .prefix("mnp")
        .rand_bytes(8)
        .permissions(Permissions::from_mode(0o700))
        .tempdir_in(job_root)?;
    let canonical = fs::canonicalize(dir.path())?;
    Ok((dir, canonical))
}

/// The profile parameters of a denial probe: `program` as `VMM`, `config` as `CONFIG`, and paths in `dir` for the
/// rest, none of which the probe reads.
fn probe_params(dir: &Path, program: &Path, config: &Path) -> Result<ProfileParams, String> {
    ProfileParams::new(
        program,
        config,
        &dir.join("krunfw"),
        &dir.join("krunfw-link"),
        &dir.join("root.img"),
        &dir.join("input.img"),
        &dir.join("o.sock"),
    )
    .map_err(|error| format!("inconclusive: {error}"))
}

/// Runs `program` with `args` under the production profile through [`seatbelt::argv`].
fn run_confined(params: &ProfileParams, program: &Path, args: &[&OsStr]) -> io::Result<ToolOutput> {
    let line = seatbelt::argv(params, program, args);
    let (first, rest) = line
        .split_first()
        .ok_or_else(|| io::Error::other("the sandbox-exec argv is empty"))?;
    run_tool(Path::new(first), rest, TOOL_OUTPUT_CAP)
}

/// Under the profile `cat` reads a listed file back exactly and fails on an unlisted one.
fn file_probe(dir: &Path) -> Result<String, String> {
    let listed = dir.join("listed");
    let unlisted = dir.join("unlisted");
    write_new(&listed, LISTED_BYTES)
        .map_err(|error| format!("inconclusive: could not write the listed file: {error}"))?;
    write_new(&unlisted, UNLISTED_BYTES)
        .map_err(|error| format!("inconclusive: could not write the unlisted file: {error}"))?;
    let cat = Path::new(CAT);
    let params = probe_params(dir, cat, &listed)?;
    let read = run_confined(&params, cat, &[listed.as_os_str()])
        .map_err(|error| format!("inconclusive: the listed read could not run: {error}"))?;
    if !read.status.success() || read.stdout != LISTED_BYTES {
        return Err(format!(
            "inconclusive: under the profile cat did not read the listed file (exited {}): {}",
            status_text(read.status),
            excerpt(&read.stderr)
        ));
    }
    let denied = run_confined(&params, cat, &[unlisted.as_os_str()])
        .map_err(|error| format!("inconclusive: the unlisted read could not run: {error}"))?;
    if denied.status.success() || contains(&denied.stdout, UNLISTED_BYTES) {
        return Err("the profile allowed cat to read an unlisted file".to_owned());
    }
    Ok(format!(
        "the listed file read back; the unlisted read exited {}: {}",
        status_text(denied.status),
        excerpt(&denied.stderr)
    ))
}

/// An unsandboxed `nc -z` reaches a probe-owned 127.0.0.1 listener, and the same `nc` under the profile does not.
fn network_probe(dir: &Path) -> Result<String, String> {
    let inconclusive = |what: &str, error: io::Error| format!("inconclusive: {what}: {error}");
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| inconclusive("could not bind a loopback listener", error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| inconclusive("could not make the listener non-blocking", error))?;
    let port = listener
        .local_addr()
        .map_err(|error| inconclusive("could not read the listener's port", error))?
        .port()
        .to_string();
    let nc = Path::new(NC);
    let args = [
        OsStr::new("-z"),
        OsStr::new("-G"),
        OsStr::new("2"),
        OsStr::new("127.0.0.1"),
        OsStr::new(&port),
    ];
    let control = run_tool(nc, &args, TOOL_OUTPUT_CAP)
        .map_err(|error| inconclusive("the control connect could not run", error))?;
    if !control.status.success() {
        return Err(format!(
            "inconclusive: the unsandboxed control connect exited {}: {}",
            status_text(control.status),
            excerpt(&control.stderr)
        ));
    }
    if accept_within(&listener, CONTROL_ACCEPT_WINDOW)? == 0 {
        return Err("inconclusive: the listener saw no control connection".to_owned());
    }
    let params = probe_params(dir, nc, &dir.join("vmm.json"))?;
    let confined = run_confined(&params, nc, &args)
        .map_err(|error| inconclusive("the sandboxed connect could not run", error))?;
    let seen = accept_within(&listener, Duration::ZERO)?;
    if confined.status.success() || seen > 0 {
        return Err(format!(
            "the profile allowed a loopback TCP connect: nc exited {} and the listener accepted {seen}",
            status_text(confined.status)
        ));
    }
    Ok(format!(
        "the control connected; under the profile nc exited {} and the listener accepted nothing: {}",
        status_text(confined.status),
        excerpt(&confined.stderr)
    ))
}

/// Accepts every pending connection on the non-blocking `listener`, polling for up to `window` while none has
/// arrived, and returns how many it accepted.
fn accept_within(listener: &TcpListener, window: Duration) -> Result<usize, String> {
    let start = Instant::now();
    let mut count: usize = 0;
    loop {
        match listener.accept() {
            Ok(_) => count = count.saturating_add(1),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if count > 0 || start.elapsed() >= window {
                    return Ok(count);
                }
                thread::sleep(POLL);
            }
            Err(error) => return Err(format!("inconclusive: accept failed: {error}")),
        }
    }
}

/// The guest is a static aarch64 ELF of at most [`GUEST_MAX_BYTES`] bytes.
fn probe_guest(guest: &Path) -> Probe {
    let paths = vec![guest.to_path_buf()];
    let result = read_guest(guest).and_then(|bytes| {
        check_guest_elf(&bytes)
            .map(|()| format!("a static aarch64 ELF of {} bytes", bytes.len()))
            .map_err(|error| error.to_string())
    });
    Probe::from_result(Check::Guest, paths, Refusal::GuestMissing, result)
}

/// Reads the guest binary, refusing a non-file or one over [`GUEST_MAX_BYTES`].
fn read_guest(guest: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(guest).map_err(|error| error.to_string())?;
    let meta = file.metadata().map_err(|error| error.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(GUEST_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > GUEST_MAX_BYTES) {
        return Err(format!("larger than {GUEST_MAX_BYTES} bytes"));
    }
    Ok(bytes)
}

/// Creates `path` with mode 0600 and writes `bytes`.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(bytes)
}

/// Whether `haystack` contains `needle`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// How a tool ended, for a detail: `0`, `1`, or `signal 9`.
fn status_text(status: ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => code.to_string(),
        (None, Some(signal)) => format!("signal {signal}"),
        (None, None) => "unknown".to_owned(),
    }
}

/// The first [`DETAIL_STDERR_BYTES`] bytes of a tool's stderr, decoded lossily and trimmed, or `no stderr`. The
/// caller's [`SafeText`] removes control characters.
fn excerpt(stderr: &[u8]) -> String {
    let head = stderr.get(..DETAIL_STDERR_BYTES).unwrap_or(stderr);
    let text = String::from_utf8_lossy(head).trim().to_owned();
    if text.is_empty() {
        "no stderr".to_owned()
    } else {
        text
    }
}

/// What a host tool wrote, and how it ended.
pub(crate) struct ToolOutput {
    /// How the tool ended.
    pub(crate) status: ExitStatus,
    /// At most the caller's cap of stdout bytes.
    pub(crate) stdout: Vec<u8>,
    /// At most `TOOL_STDERR_CAP` plus one stderr bytes.
    pub(crate) stderr: Vec<u8>,
}

/// Runs `program` with `args` as a host tool: the environment cleared to `PATH=`[`TOOL_PATH`], stdin from
/// `/dev/null`, stdout read up to `cap` bytes and stderr up to `TOOL_STDERR_CAP`, its own process group, and
/// killed with that group at [`TOOL_DEADLINE`]. Modeled on `minos-source`'s `run_capped`.
pub(crate) fn run_tool<A: AsRef<OsStr>>(
    program: &Path,
    args: &[A],
    cap: u64,
) -> io::Result<ToolOutput> {
    let child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", TOOL_PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut child = Reaper(child);
    let stdout = child.0.stdout.take();
    let stderr = child.0.stderr.take();
    // Both pipes are read while the tool runs, so neither can fill and block it.
    let (status, stdout, stderr) = thread::scope(|scope| {
        let out = scope.spawn(move || read_capped(stdout, cap));
        let err = scope.spawn(move || read_capped(stderr, TOOL_STDERR_CAP));
        let status = wait_until(&mut child.0, TOOL_DEADLINE);
        (status, out.join(), err.join())
    });
    let status = status?;
    let stdout = stdout.map_err(|_| io::Error::other("the stdout reader panicked"))??;
    let stderr = stderr.map_err(|_| io::Error::other("the stderr reader panicked"))??;
    if u64::try_from(stdout.len()).map_or(true, |len| len > cap) {
        return Err(io::Error::other(format!(
            "{} wrote more than {cap} bytes",
            program.display()
        )));
    }
    Ok(ToolOutput {
        status,
        stdout,
        stderr,
    })
}

/// Reads at most `cap + 1` bytes from `pipe`, so more than `cap` bytes is detectable.
fn read_capped<R: Read>(pipe: Option<R>, cap: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(pipe) = pipe {
        pipe.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

/// Waits for `child` until `deadline` has passed, then kills its process group and reaps it.
fn wait_until(child: &mut Child, deadline: Duration) -> io::Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= deadline {
            kill_group(child);
            child.wait().ok();
            return Err(io::Error::new(
                ErrorKind::TimedOut,
                "the host tool ran past its deadline",
            ));
        }
        thread::sleep(POLL);
    }
}

/// Sends SIGKILL to the unreaped `child`'s process group, which it leads, then to the child itself.
fn kill_group(child: &mut Child) {
    kill_process_group(Pid::from_child(child), Signal::KILL).ok();
    child.kill().ok();
}

/// Kills the tool's process group and reaps it on drop, so no early return leaves a tool running. The group is
/// signalled only while the leader is unreaped, so its pid still names the tool.
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            kill_group(&mut self.0);
        }
        self.0.wait().ok();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::error::Error;
    use std::fmt::Write as _;
    use std::fs;
    use std::path::{Path, PathBuf};

    use minos_core::id::ImageDigest;
    use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
    use minos_core::manifest::Arch;
    use minos_sandbox::Refusal;
    use minos_sandbox::spec::{MAX_LAUNCH_MS, MIN_MEMORY_MIB};
    use sha2::{Digest, Sha256};

    use super::{
        Check, HostPaths, KRUNFW_PATH, KRUNFW_SHA256, Probe, ProbeOutcome, REQUIRED_ENTITLEMENTS,
        entitlements_present, longest_socket_path, max_machine, parse_hv_support, parse_sysctl,
        parse_u64, probe_entitlements, probe_guest, probe_host, probe_kernel, probe_sandbox_exec,
        probe_seatbelt, probe_socket_path, probe_vmm, report,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    const HYPERVISOR: &str = "com.apple.security.hypervisor";
    const LIBRARY: &str = "com.apple.security.cs.disable-library-validation";

    /// An entitlements plist with `entries` as `(key, value element)` pairs, in order.
    fn plist(entries: &[(&str, &str)]) -> Result<Vec<u8>, std::fmt::Error> {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n",
        );
        for (key, value) in entries {
            writeln!(xml, "<key>{key}</key>{value}")?;
        }
        xml.push_str("</dict>\n</plist>\n");
        Ok(xml.into_bytes())
    }

    /// The smallest static aarch64 ELF header `check_guest_elf` accepts: no program headers.
    fn static_elf() -> Vec<u8> {
        let mut header = vec![0_u8; 64];
        for (slot, byte) in header.iter_mut().zip([0x7f, b'E', b'L', b'F', 2, 1, 1]) {
            *slot = byte;
        }
        if let Some(machine) = header.get_mut(18..20) {
            machine.copy_from_slice(&183_u16.to_le_bytes());
        }
        header
    }

    /// The refusal a failed `check` records.
    fn refusal_for(check: Check) -> Refusal {
        match check {
            Check::Vmm => Refusal::VmmMissing,
            Check::Entitlements => Refusal::EntitlementMissing,
            Check::Kernel => Refusal::KernelHashMismatch,
            Check::Hypervisor => Refusal::HypervisorUnsupported,
            Check::SocketPath => Refusal::SocketPathTooLong,
            Check::SandboxExec => Refusal::SeatbeltMissing,
            Check::SeatbeltNetwork | Check::SeatbeltFile => Refusal::SeatbeltNotEnforcing,
            Check::Guest => Refusal::GuestMissing,
        }
    }

    #[test]
    fn entitlements_both_keys() -> TestResult {
        assert_eq!(REQUIRED_ENTITLEMENTS, [HYPERVISOR, LIBRARY]);
        assert!(entitlements_present(&plist(&[
            (HYPERVISOR, "<true/>"),
            (LIBRARY, "<true/>")
        ])?));
        assert!(entitlements_present(&plist(&[
            (LIBRARY, "\n\t<true/>"),
            (HYPERVISOR, " <true/>")
        ])?));
        assert!(!entitlements_present(&plist(&[(HYPERVISOR, "<true/>")])?));
        assert!(!entitlements_present(&plist(&[(LIBRARY, "<true/>")])?));
        assert!(!entitlements_present(&plist(&[
            (HYPERVISOR, "<true/>"),
            (LIBRARY, "<false/>")
        ])?));
        assert!(!entitlements_present(&plist(&[
            (HYPERVISOR, "<false/>"),
            (LIBRARY, "<true/>")
        ])?));
        assert!(!entitlements_present(&plist(&[])?));
        assert!(!entitlements_present(b""));
        let repeated = plist(&[
            (HYPERVISOR, "<true/>"),
            (LIBRARY, "<true/>"),
            (HYPERVISOR, "<false/>"),
        ])?;
        assert!(!entitlements_present(&repeated));
        Ok(())
    }

    #[test]
    fn sysctl_parsing() {
        assert!(parse_hv_support(b"1\n"));
        assert!(parse_hv_support(b"1"));
        assert!(!parse_hv_support(b"0\n"));
        assert!(!parse_hv_support(b"garbage"));
        assert!(!parse_hv_support(b""));
        assert!(!parse_hv_support(b"11\n"));
        assert_eq!(parse_u64(b"16\n"), Some(16));
        assert_eq!(parse_u64(b"51539607552"), Some(51_539_607_552));
        assert_eq!(parse_u64(b"+16"), None);
        assert_eq!(parse_u64(b" 16"), None);
        assert_eq!(parse_u64(b"-1"), None);
        assert_eq!(parse_u64(b""), None);
        assert_eq!(parse_u64(b"99999999999999999999999"), None);
        assert_eq!(
            parse_sysctl(b"1\n16\n51539607552\n"),
            Some((true, 16, 51_539_607_552))
        );
        assert_eq!(
            parse_sysctl(b"0\n16\n51539607552\n"),
            Some((false, 16, 51_539_607_552))
        );
        assert_eq!(parse_sysctl(b"1\n16\n"), None);
        assert_eq!(parse_sysctl(b"1\n16\n5\n7\n"), None);
        assert_eq!(parse_sysctl(b"1\nsixteen\n5\n"), None);

        let big = max_machine(16, 51_539_607_552);
        assert_eq!((big.vcpus(), big.memory_mib()), (8, 16_384));
        let small = max_machine(4, 8_589_934_592);
        assert_eq!((small.vcpus(), small.memory_mib()), (4, 4096));
        let tiny = max_machine(0, 0);
        assert_eq!((tiny.vcpus(), tiny.memory_mib()), (1, MIN_MEMORY_MIB));
        let huge = max_machine(u64::MAX, u64::MAX);
        assert_eq!((huge.vcpus(), huge.memory_mib()), (8, 16_384));
    }

    #[test]
    fn socket_path_boundary() {
        // "/" + n bytes, then "/minos-501" (10), "/mnsXXXXXXXX" (12), and "/o.sock" (7).
        let at = PathBuf::from(format!("/{}", "a".repeat(73)));
        assert_eq!(longest_socket_path(&at, 501).as_os_str().len(), 103);
        let fits = probe_socket_path(&at, 501);
        assert!(fits.is_passed());
        assert_eq!(fits.check(), Check::SocketPath);
        let over = PathBuf::from(format!("/{}", "a".repeat(74)));
        assert_eq!(longest_socket_path(&over, 501).as_os_str().len(), 104);
        assert_eq!(
            probe_socket_path(&over, 501).refusal(),
            Some(Refusal::SocketPathTooLong)
        );
        assert!(probe_socket_path(Path::new("/private/tmp"), u32::MAX).is_passed());
        assert_eq!(
            longest_socket_path(Path::new("/private/tmp"), 501),
            Path::new("/private/tmp/minos-501/mnsXXXXXXXX/o.sock")
        );
    }

    #[test]
    fn missing_files_refuse() -> TestResult {
        let dir = tempfile::tempdir()?;
        let absent = dir.path().join("absent");

        assert_eq!(probe_vmm(&absent).refusal(), Some(Refusal::VmmMissing));
        assert_eq!(probe_vmm(dir.path()).refusal(), Some(Refusal::VmmMissing));
        let vmm = dir.path().join("minos-vmm");
        fs::write(&vmm, b"helper")?;
        assert!(probe_vmm(&vmm).is_passed());
        assert_eq!(
            probe_entitlements(&absent, false).refusal(),
            Some(Refusal::EntitlementMissing)
        );

        let (kernel, digest) = probe_kernel(&absent, KRUNFW_SHA256);
        assert_eq!(kernel.refusal(), Some(Refusal::KernelMissing));
        assert!(digest.is_none());
        let (kernel, digest) = probe_kernel(dir.path(), KRUNFW_SHA256);
        assert_eq!(kernel.refusal(), Some(Refusal::KernelMissing));
        assert!(digest.is_none());
        let other = dir.path().join("libkrunfw.5.dylib");
        let bytes = b"not the pinned kernel";
        fs::write(&other, bytes)?;
        let (kernel, digest) = probe_kernel(&other, KRUNFW_SHA256);
        assert_eq!(kernel.refusal(), Some(Refusal::KernelHashMismatch));
        assert!(digest.is_none());
        let pinned: [u8; 32] = Sha256::digest(bytes).into();
        let pinned = base16ct::lower::encode_string(&pinned);
        let (kernel, digest) = probe_kernel(&other, &pinned);
        assert!(kernel.is_passed());
        assert_eq!(digest, Some(ImageDigest::of(bytes)));

        let guest = dir.path().join("minos-guest");
        fs::write(
            &guest,
            [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01, 0, 0, 0, 0],
        )?;
        assert_eq!(probe_guest(&guest).refusal(), Some(Refusal::GuestMissing));
        assert_eq!(probe_guest(&absent).refusal(), Some(Refusal::GuestMissing));
        assert_eq!(
            probe_guest(dir.path()).refusal(),
            Some(Refusal::GuestMissing)
        );
        let elf = dir.path().join("minos-guest-elf");
        fs::write(&elf, static_elf())?;
        assert!(probe_guest(&elf).is_passed());

        assert_eq!(
            probe_sandbox_exec(&absent).refusal(),
            Some(Refusal::SeatbeltMissing)
        );
        let [network, file] = probe_seatbelt(dir.path(), false);
        assert_eq!(network.refusal(), Some(Refusal::SeatbeltNotEnforcing));
        assert_eq!(file.refusal(), Some(Refusal::SeatbeltNotEnforcing));
        Ok(())
    }

    #[test]
    fn layers_never_include_dedicated_user() {
        let checks = [
            Check::Vmm,
            Check::Entitlements,
            Check::Guest,
            Check::SandboxExec,
            Check::SeatbeltNetwork,
            Check::SeatbeltFile,
        ];
        let max = max_machine(8, 1 << 34);
        for mask in 0_u32..64 {
            let on = |bit: u32| mask & (1 << bit) != 0;
            let probes: Vec<Probe> = (0_u32..)
                .zip(checks)
                .map(|(bit, check)| {
                    if on(bit) {
                        Probe::passed(check, Vec::new(), "ok")
                    } else {
                        Probe::refused(check, Vec::new(), refusal_for(check), "no")
                    }
                })
                .collect();
            let expected: BTreeSet<Refusal> = probes.iter().filter_map(Probe::refusal).collect();
            let host = report(probes, max, None);
            let caps = host.capabilities();
            let layers = caps.layers();
            assert!(
                !layers.contains(&ConfinementLayer::DedicatedUser),
                "mask {mask:06b}"
            );
            assert_eq!(
                layers.contains(&ConfinementLayer::Rlimits),
                on(0) && on(1) && on(2),
                "mask {mask:06b}"
            );
            assert_eq!(
                layers.contains(&ConfinementLayer::Seatbelt),
                on(3) && on(4) && on(5),
                "mask {mask:06b}"
            );
            assert_eq!(caps.host_refusals(), &expected, "mask {mask:06b}");
        }

        // A check that never ran is not passed, and one refused run of a check outweighs a passed one.
        assert!(
            report(Vec::new(), max, None)
                .capabilities()
                .layers()
                .is_empty()
        );
        let mut doubled: Vec<Probe> = checks
            .iter()
            .map(|check| Probe::passed(*check, Vec::new(), "ok"))
            .collect();
        doubled.push(Probe::refused(
            Check::SeatbeltFile,
            Vec::new(),
            Refusal::SeatbeltNotEnforcing,
            "no",
        ));
        let host = report(doubled, max, None);
        assert_eq!(
            host.capabilities().layers(),
            &BTreeSet::from([ConfinementLayer::Rlimits])
        );
    }

    #[test]
    fn report_shape() {
        let max = max_machine(16, 51_539_607_552);
        let kernel = ImageDigest::of(b"kernel");
        let host = report(
            vec![Probe::passed(Check::Kernel, Vec::new(), "ok")],
            max,
            Some(kernel),
        );
        let caps = host.capabilities();
        assert_eq!(caps.backend(), BackendKind::Krun);
        assert_eq!(caps.arch(), Arch::Arm64);
        assert_eq!(caps.isolation(), Isolation::Hardware);
        assert_eq!(caps.max_duration_ms(), MAX_LAUNCH_MS);
        assert_eq!(caps.max(), &max);
        assert_eq!(caps.kernel(), Some(&kernel));
        assert!(caps.host_refusals().is_empty());
        assert_eq!(host.probes().len(), 1);
        assert!(
            report(Vec::new(), max, None)
                .capabilities()
                .kernel()
                .is_none()
        );
    }

    #[test]
    fn probe_details_are_sanitized() -> TestResult {
        let probe = Probe::passed(Check::Vmm, Vec::new(), "line\u{1b}[31m\nnext");
        let ProbeOutcome::Passed { detail } = probe.outcome() else {
            return Err("a passed probe".into());
        };
        assert!(!detail.as_str().contains('\u{1b}'));
        assert!(!detail.as_str().contains('\n'));
        Ok(())
    }

    /// Runs every probe on this Mac. Build first: `cargo build -p minos-vmm`, sign it with the CLAUDE.md codesign
    /// argv, and `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`.
    #[test]
    #[ignore = "needs MINOS_VMM (a signed minos-vmm) and MINOS_GUEST (the static musl minos-guest); MINOS_KRUNFW is optional"]
    fn probe_host_on_this_mac() -> TestResult {
        let vmm = std::env::var_os("MINOS_VMM").ok_or("set MINOS_VMM to the signed minos-vmm")?;
        let guest =
            std::env::var_os("MINOS_GUEST").ok_or("set MINOS_GUEST to the musl minos-guest")?;
        let krunfw = std::env::var_os("MINOS_KRUNFW")
            .map_or_else(|| PathBuf::from(KRUNFW_PATH), PathBuf::from);
        let host = probe_host(
            &HostPaths::new(vmm, guest, krunfw),
            Path::new("/private/tmp"),
        );
        for probe in host.probes() {
            println!(
                "{:<32} {:?} {:?}",
                probe.check().name(),
                probe.paths(),
                probe.outcome()
            );
        }
        let caps = host.capabilities();
        assert!(
            caps.host_refusals().is_empty(),
            "refusals: {:?}",
            caps.host_refusals()
        );
        assert_eq!(
            caps.layers(),
            &BTreeSet::from([ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits])
        );
        assert!(caps.kernel().is_some());
        assert_eq!(host.probes().len(), 9);
        Ok(())
    }

    /// The two Seatbelt denial probes against the production profile, offline.
    #[test]
    #[ignore = "needs /usr/bin/sandbox-exec, /usr/bin/nc, and /bin/cat on macOS"]
    fn seatbelt_denial_probes_hold() -> TestResult {
        let root = tempfile::tempdir_in("/private/tmp")?;
        let [network, file] = probe_seatbelt(root.path(), true);
        println!("{:?}\n{:?}", network.outcome(), file.outcome());
        assert!(file.is_passed(), "{:?}", file.outcome());
        assert!(network.is_passed(), "{:?}", network.outcome());
        assert_eq!(
            fs::read_dir(root.path())?.count(),
            0,
            "the probe directory is removed"
        );
        Ok(())
    }
}
