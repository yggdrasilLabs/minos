//! `minos host doctor` and `minos host setup` (CLI-03, KRUN-06, KRUN-08; 05-CONTEXT D-01 to D-04).
//!
//! `doctor` finds the binaries at their fixed locations, sweeps job directories a dead supervisor left behind,
//! runs every launch precondition through the backend's [`probe_host`], and prints each check as `ok` or
//! `refused`, every refusal with its fix, every path it looked at, the confinement layers (the deferred
//! dedicated user shown as missing, reduced hardening), and the container builder's state. It exits 0 only when
//! nothing is refused, and it never falls back silently.
//!
//! `setup` runs CLAUDE.md Commands' one documented `codesign` argv from Rust (02-CONTEXT D-11 allowed it; D-02),
//! then verifies with `codesign --display --entitlements - --xml` that both entitlement keys are set, and reports
//! libkrunfw and the builder the way `doctor` does.
//!
//! Every host tool runs by absolute path as an argv array with a cleared environment, stdin from `/dev/null`,
//! capped output, its own process group, and a deadline. Every string from a host tool, and every path, passes
//! through [`SafeText`] before it is printed.

use std::collections::BTreeSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use minos_backend_krun::host::{
    CODESIGN, Check, HostReport, Probe, ProbeOutcome, REQUIRED_ENTITLEMENTS, TOOL_PATH, probe_host,
    signed_with_entitlements,
};
use minos_backend_krun::jobdir::{JOB_ROOT_PARENT, JobRoot, SweepEntry, sweep};
use minos_cli::locate::{Candidate, Located, VMM_NAME, locate, probe_paths};
use minos_core::ledger::{ConfinementLayer, EXPECTED_KRUN_LAYERS, Isolation};
use minos_core::text::SafeText;
use minos_sandbox::Refusal;
use rustix::process::{Pid, Signal, kill_process_group};

/// The committed entitlements plist, `crates/minos-vmm/minos-vmm.entitlements`, byte for byte. `setup` writes a
/// private copy of it, so an installed `minos` signs with exactly the reviewed entitlements and nothing else.
const ENTITLEMENTS: &[u8] = b"";

/// CLAUDE.md Commands' signing flags, between `codesign` and the entitlements and helper paths.
const CODESIGN_FLAGS: [&str; 6] = [
    "--force",
    "--sign",
    "-",
    "--options",
    "runtime",
    "--entitlements",
];

/// How long `codesign` may take to sign the helper.
const SIGN_DEADLINE: Duration = Duration::from_secs(60);

/// How long one `docker` command may take; a daemon that does not answer in time counts as not running.
const DOCKER_DEADLINE: Duration = Duration::from_secs(10);

/// The most stdout bytes read from `codesign` or `docker`.
const TOOL_OUTPUT_CAP: u64 = 4096;

/// The most stderr bytes read from `codesign` or `docker`.
const TOOL_STDERR_CAP: u64 = 4096;

/// The most bytes of a tool's output shown in one line.
const SHOWN_BYTES: usize = 240;

/// Pause between polls of a running tool.
const POLL: Duration = Duration::from_millis(10);

/// The system directories every tool's `PATH` ends with.
const SYSTEM_DIRS: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];

/// Runs `minos host doctor`: locate, sweep, probe, print, and exit 0 only when nothing failed.
///
/// # Errors
///
/// When the running binary's own path cannot be read. Every host problem is a printed refusal, not an error.
pub fn doctor() -> anyhow::Result<ExitCode> {
    let exe = env::current_exe().context("finding the running minos binary")?;
    let located = locate(&exe);
    let paths = probe_paths(&located);
    let swept = sweep_jobs(paths.vmm());
    let host = probe_host(&paths, Path::new(JOB_ROOT_PARENT));
    let report = DoctorReport::new(&located, &host, swept, builder_status());
    print!("{}", render_doctor(&report));
    Ok(report.exit_code())
}

/// Runs `minos host setup`: sign the located helper with the documented argv, verify both entitlements, then
/// report libkrunfw and the builder.
///
/// # Errors
///
/// When the private entitlements copy cannot be written, `codesign` cannot run or fails, or the signed helper does
/// not show both entitlements set to true.
pub fn setup() -> anyhow::Result<ExitCode> {
    let exe = env::current_exe().context("finding the running minos binary")?;
    let located = locate(&exe);
    let Some(vmm) = located.vmm() else {
        let mut out = String::new();
        push(
            &mut out,
            format_args!("refused  helper present  {}", Refusal::VmmMissing),
        );
        for candidate in located
            .checked()
            .iter()
            .filter(|candidate| candidate.path().file_name() == Some(OsStr::new(VMM_NAME)))
        {
            push(
                &mut out,
                format_args!(
                    "  checked: {}  {}",
                    shown(candidate.path()),
                    candidate.reason()
                ),
            );
        }
        print!("{out}");
        return Ok(ExitCode::FAILURE);
    };
    sign(vmm)?;
    let verified = signed_with_entitlements(vmm)
        .with_context(|| format!("verifying the signature of {}", vmm.display()))?;
    if !verified {
        bail!(
            "codesign signed {} but does not show {} set to true",
            vmm.display(),
            REQUIRED_ENTITLEMENTS.join(" and ")
        );
    }
    let mut out = String::new();
    push(&mut out, format_args!("signed: {}", shown(vmm)));
    push(
        &mut out,
        format_args!("verified: {}", REQUIRED_ENTITLEMENTS.join(", ")),
    );
    let host = probe_host(&probe_paths(&located), Path::new(JOB_ROOT_PARENT));
    let kernel: Vec<ProbeLine> = host
        .probes()
        .iter()
        .filter(|probe| probe.check() == Check::Kernel)
        .map(ProbeLine::from)
        .collect();
    for line in &kernel {
        render_probe(&mut out, line);
    }
    push(&mut out, format_args!("builder: {}", builder_status()));
    print!("{out}");
    Ok(if kernel.iter().any(ProbeLine::is_refused) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// The documented signing argv (CLAUDE.md Commands): `/usr/bin/codesign --force --sign - --options runtime
/// --entitlements <entitlements> <vmm>`. Only the entitlements path differs from the documented command, and it
/// names a byte-identical private copy of the committed plist.
pub fn setup_codesign_argv(vmm: &Path, entitlements: &Path) -> Vec<OsString> {
    let _ = (vmm, entitlements);
    Vec::new()
}

/// Writes the committed plist to a fresh 0700 directory as a 0600 file and runs the signing argv on `vmm`.
fn sign(vmm: &Path) -> anyhow::Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("minos-setup-")
        .tempdir()
        .context("creating a private directory for the entitlements")?;
    fs::set_permissions(dir.path(), Permissions::from_mode(0o700))
        .context("making the entitlements directory private")?;
    let plist = dir.path().join("entitlements.plist");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&plist)
        .context("creating the private entitlements copy")?;
    file.write_all(ENTITLEMENTS)
        .and_then(|()| file.sync_all())
        .context("writing the private entitlements copy")?;
    drop(file);

    let argv = setup_codesign_argv(vmm, &plist);
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("the codesign argv is empty"))?;
    let env = [("PATH", OsStr::new(TOOL_PATH))];
    let output = run_tool(
        Path::new(program),
        args,
        &env,
        SIGN_DEADLINE,
        TOOL_OUTPUT_CAP,
    )
    .context("running codesign")?;
    if !output.status.success() {
        bail!(
            "codesign exited {}: {}",
            status_text(output.status),
            excerpt(&output.stderr)
        );
    }
    Ok(())
}

/// Everything `minos host doctor` prints, gathered before rendering so [`render_doctor`] stays pure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    binaries: Vec<BinaryLine>,
    krunfw: PathBuf,
    probes: Vec<ProbeLine>,
    isolation: Isolation,
    layers: BTreeSet<ConfinementLayer>,
    sweep: SweepStatus,
    builder: BuilderStatus,
}

impl DoctorReport {
    /// Gathers what was located, what the probes found, the sweep, and the builder.
    fn new(
        located: &Located,
        host: &HostReport,
        sweep: SweepStatus,
        builder: BuilderStatus,
    ) -> Self {
        Self {
            binaries: located.checked().iter().map(BinaryLine::from).collect(),
            krunfw: located.krunfw().to_path_buf(),
            probes: host.probes().iter().map(ProbeLine::from).collect(),
            isolation: host.capabilities().isolation(),
            layers: host.capabilities().layers().clone(),
            sweep,
            builder,
        }
    }

    /// How many things failed: each refused probe, and a job root that could not be opened.
    fn failures(&self) -> usize {
        let _ = self;
        0
    }

    /// 0 when nothing failed, 1 otherwise.
    fn exit_code(&self) -> ExitCode {
        if self.failures() == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}

/// One path the locator checked, and what it found there.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BinaryLine {
    path: PathBuf,
    accepted: bool,
    reason: &'static str,
}

impl From<&Candidate> for BinaryLine {
    fn from(candidate: &Candidate) -> Self {
        Self {
            path: candidate.path().to_path_buf(),
            accepted: candidate.is_accepted(),
            reason: candidate.reason(),
        }
    }
}

/// One host check as `doctor` prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeLine {
    check: &'static str,
    paths: Vec<PathBuf>,
    outcome: LineOutcome,
}

/// Whether a check passed, with what it saw.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LineOutcome {
    Passed(SafeText),
    Refused { refusal: Refusal, detail: SafeText },
}

impl ProbeLine {
    /// Whether the check refused.
    const fn is_refused(&self) -> bool {
        matches!(self.outcome, LineOutcome::Refused { .. })
    }
}

impl From<&Probe> for ProbeLine {
    fn from(probe: &Probe) -> Self {
        let outcome = match probe.outcome() {
            ProbeOutcome::Passed { detail } => LineOutcome::Passed(detail.clone()),
            ProbeOutcome::Refused { refusal, detail } => LineOutcome::Refused {
                refusal: *refusal,
                detail: detail.clone(),
            },
        };
        Self {
            check: probe.check().name(),
            paths: probe.paths().to_vec(),
            outcome,
        }
    }
}

/// What the sweep of left-over job directories did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SweepStatus {
    /// The job root was opened and swept.
    Swept {
        root: PathBuf,
        entries: Vec<SweepEntry>,
    },
    /// The job root could not be opened; nothing can launch until it can.
    Failed { error: SafeText },
}

/// Opens the job root and sweeps it.
fn sweep_jobs(vmm: &Path) -> SweepStatus {
    match JobRoot::open() {
        Ok(root) => SweepStatus::Swept {
            root: root.path().to_path_buf(),
            entries: sweep(&root, vmm).entries().to_vec(),
        },
        Err(error) => SweepStatus::Failed {
            error: SafeText::inline(&error.to_string()),
        },
    }
}

/// Renders the doctor report: the binaries, each check with its paths, the confinement layers, the sweep, the
/// builder, and the result, in that order. Every path and host-tool string passes through [`SafeText`].
pub fn render_doctor(report: &DoctorReport) -> String {
    let _ = report;
    String::new()
}

/// Renders one check: `ok` or `refused`, its name, what it saw or the refusal with its fix, then its paths.
fn render_probe(out: &mut String, probe: &ProbeLine) {
    match &probe.outcome {
        LineOutcome::Passed(detail) => {
            push(out, format_args!("{:<9}{}  {detail}", "ok", probe.check));
        }
        LineOutcome::Refused { refusal, detail } => {
            push(
                out,
                format_args!(
                    "{:<9}{}  {}",
                    "refused",
                    probe.check,
                    SafeText::inline(&refusal.to_string())
                ),
            );
            push(out, format_args!("  saw: {detail}"));
        }
    }
    for path in &probe.paths {
        push(out, format_args!("  checked: {}", shown(path)));
    }
}

/// What the sweep did with one job directory.
fn sweep_line(entry: &SweepEntry) -> String {
    match entry {
        SweepEntry::Removed { dir } => format!(
            "  removed {}: its supervisor and helper were gone",
            shown(dir)
        ),
        SweepEntry::Killed { dir, pid } => {
            format!("  killed orphaned helper {pid} and removed {}", shown(dir))
        }
        SweepEntry::Live { dir } => format!("  left {}: a running minos owns it", shown(dir)),
        SweepEntry::Unverified { dir, reason } => format!("  left {}: {reason}", shown(dir)),
    }
}

/// The isolation level, as the confinement line names it.
const fn isolation_name(isolation: Isolation) -> &'static str {
    match isolation {
        Isolation::Hardware => "hardware",
        Isolation::WeakProcess => "weak process isolation",
    }
}

/// A confinement layer, as the confinement line names it.
const fn layer_name(layer: ConfinementLayer) -> &'static str {
    match layer {
        ConfinementLayer::Seatbelt => "seatbelt",
        ConfinementLayer::DedicatedUser => "dedicated user",
        ConfinementLayer::Rlimits => "rlimits",
        _ => "another layer",
    }
}

/// The line for an expected layer that is not active.
const fn missing_layer(layer: ConfinementLayer) -> &'static str {
    match layer {
        // D-01: deferred, so every v1 run records it missing and the report shows reduced hardening.
        ConfinementLayer::DedicatedUser => "dedicated user: missing (deferred, reduced hardening)",
        // D-03: mandatory, so the Seatbelt checks above refuse and nothing launches.
        ConfinementLayer::Seatbelt => {
            "seatbelt: missing (required in v1; nothing launches until the Seatbelt checks pass)"
        }
        ConfinementLayer::Rlimits => {
            "rlimits: missing (reduced hardening; fix the helper and guest checks above)"
        }
        _ => "a confinement layer is missing (reduced hardening)",
    }
}

/// Whether the container builder `minos env build` needs is usable. Informational only: never a launch refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderStatus {
    /// `docker buildx version` and `docker info` both answered.
    Ready {
        /// The canonical docker binary.
        docker: PathBuf,
        /// The buildx version.
        buildx: SafeText,
        /// The daemon's server version.
        daemon: SafeText,
    },
    /// docker is on `PATH`, but `docker buildx version` failed.
    NoBuildx {
        /// The canonical docker binary.
        docker: PathBuf,
    },
    /// docker and buildx are present, but the daemon did not answer.
    DaemonDown {
        /// The canonical docker binary.
        docker: PathBuf,
    },
    /// No docker on the user's `PATH`.
    NotFound,
}

impl fmt::Display for BuilderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready {
                docker,
                buildx,
                daemon,
            } => write!(
                f,
                "docker buildx {buildx}, daemon {daemon} ({})",
                shown(docker)
            ),
            Self::NoBuildx { docker } => write!(
                f,
                "docker found at {}, but buildx did not run: install the buildx plugin before minos env build",
                shown(docker)
            ),
            Self::DaemonDown { docker } => write!(
                f,
                "docker found at {}, daemon not running: start Docker Desktop before minos env build",
                shown(docker)
            ),
            Self::NotFound => f.write_str("not found: minos env build needs Docker with buildx"),
        }
    }
}

/// docker as found on the user's `PATH`, with the `PATH` its commands run under.
struct Docker {
    binary: PathBuf,
    search: OsString,
}

/// Finds docker on the user's `PATH` and asks it for its buildx and daemon versions.
///
/// Each command runs by the canonical path of the first executable `docker` in an absolute `PATH` directory,
/// with the environment cleared to `HOME` and a `PATH` of that directory, the binary's canonical directory, and
/// the system directories (02-SPIKE-EVIDENCE.md RI-03), under a 10 s deadline with capped output.
pub fn builder_status() -> BuilderStatus {
    let Some(docker) = find_docker() else {
        return BuilderStatus::NotFound;
    };
    let home = env::var_os("HOME");
    let mut env = vec![("PATH", docker.search.as_os_str())];
    if let Some(home) = &home {
        env.push(("HOME", home.as_os_str()));
    }
    let run = |args: &[&str]| {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        run_tool(
            &docker.binary,
            &args,
            &env,
            DOCKER_DEADLINE,
            TOOL_OUTPUT_CAP,
        )
        .ok()
        .filter(|output| output.status.success())
        .map(|output| first_line(&output.stdout))
        .filter(|line| !line.is_empty())
    };
    let Some(buildx) = run(&["buildx", "version"]) else {
        return BuilderStatus::NoBuildx {
            docker: docker.binary,
        };
    };
    let Some(daemon) = run(&["info", "--format", "{{.ServerVersion}}"]) else {
        return BuilderStatus::DaemonDown {
            docker: docker.binary,
        };
    };
    BuilderStatus::Ready {
        docker: docker.binary,
        buildx: SafeText::inline(buildx_version(&buildx)),
        daemon: SafeText::inline(&daemon),
    }
}

/// The first executable `docker` in an absolute directory of the user's `PATH`.
fn find_docker() -> Option<Docker> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .filter(|dir| dir.is_absolute())
        .find_map(|dir| {
            let candidate = dir.join("docker");
            let meta = fs::metadata(&candidate).ok()?;
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
                return None;
            }
            let binary = fs::canonicalize(&candidate).ok()?;
            let mut dirs = vec![dir.clone()];
            if let Some(parent) = binary.parent()
                && parent != dir
            {
                dirs.push(parent.to_path_buf());
            }
            dirs.extend(SYSTEM_DIRS.iter().map(PathBuf::from));
            let search = env::join_paths(&dirs).ok()?;
            Some(Docker { binary, search })
        })
}

/// The version token of `docker buildx version` (`github.com/docker/buildx v0.29.1 <commit>`), or the whole
/// line when it has none.
fn buildx_version(line: &str) -> &str {
    line.split_whitespace()
        .find(|word| {
            word.strip_prefix('v')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .unwrap_or(line)
}

/// The first line of a tool's output, trimmed, at most [`SHOWN_BYTES`] bytes.
fn first_line(bytes: &[u8]) -> String {
    let head = bytes.get(..SHOWN_BYTES).unwrap_or(bytes);
    String::from_utf8_lossy(head)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// A tool's stderr, for an error message: sanitized, on one line, at most [`SHOWN_BYTES`] bytes.
fn excerpt(bytes: &[u8]) -> SafeText {
    let head = bytes.get(..SHOWN_BYTES).unwrap_or(bytes);
    let text = String::from_utf8_lossy(head);
    let text = text.trim();
    SafeText::inline(if text.is_empty() { "no stderr" } else { text })
}

/// How a tool ended, as text.
fn status_text(status: ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("with code {code}"),
        (None, Some(signal)) => format!("on signal {signal}"),
        (None, None) => "abnormally".to_owned(),
    }
}

/// A path, sanitized for one line.
fn shown(path: &Path) -> SafeText {
    SafeText::inline(&path.to_string_lossy())
}

/// Appends `line` and a newline.
fn push(out: &mut String, line: impl fmt::Display) {
    out.push_str(&line.to_string());
    out.push('\n');
}

/// What a host tool wrote, and how it ended.
struct ToolOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Runs `program` with `args` and exactly `env`: stdin from `/dev/null`, stdout read up to `cap` bytes and stderr
/// up to [`TOOL_STDERR_CAP`], its own process group, killed with that group at `deadline`. Modeled on the krun
/// backend's host-tool runner.
fn run_tool(
    program: &Path,
    args: &[OsString],
    env: &[(&str, &OsStr)],
    deadline: Duration,
    cap: u64,
) -> io::Result<ToolOutput> {
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = Reaper(command.spawn()?);
    let stdout = child.0.stdout.take();
    let stderr = child.0.stderr.take();
    // Both pipes are read while the tool runs, so neither can fill and block it.
    let (status, stdout, stderr) = thread::scope(|scope| {
        let out = scope.spawn(move || read_capped(stdout, cap));
        let err = scope.spawn(move || read_capped(stderr, TOOL_STDERR_CAP));
        let status = wait_until(&mut child.0, deadline);
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
                io::ErrorKind::TimedOut,
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
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    use minos_backend_krun::host::{KRUNFW_PATH, entitlements_present};
    use minos_backend_krun::jobdir::SweepEntry;
    use minos_core::ledger::{ConfinementLayer, Isolation};
    use minos_core::text::SafeText;
    use minos_sandbox::Refusal;

    use super::{
        BinaryLine, BuilderStatus, DoctorReport, ENTITLEMENTS, LineOutcome, ProbeLine, SweepStatus,
        buildx_version, render_doctor, setup_codesign_argv,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// A report with one passed check and two refused ones, Seatbelt and rlimits active, and one swept directory.
    fn report() -> DoctorReport {
        DoctorReport {
            binaries: vec![
                BinaryLine {
                    path: PathBuf::from("/opt/minos/bin/minos-vmm"),
                    accepted: true,
                    reason: "a regular file",
                },
                BinaryLine {
                    path: PathBuf::from("/opt/minos/bin/minos-guest"),
                    accepted: false,
                    reason: "not found",
                },
            ],
            krunfw: PathBuf::from(KRUNFW_PATH),
            probes: vec![
                ProbeLine {
                    check: "helper present",
                    paths: vec![PathBuf::from("/opt/minos/bin/minos-vmm")],
                    outcome: LineOutcome::Passed(SafeText::inline("a regular file of 10 bytes")),
                },
                ProbeLine {
                    check: "guest kernel",
                    paths: vec![PathBuf::from(KRUNFW_PATH)],
                    outcome: LineOutcome::Refused {
                        refusal: Refusal::KernelMissing,
                        detail: SafeText::inline("No such file or directory"),
                    },
                },
                ProbeLine {
                    check: "guest binary",
                    paths: vec![PathBuf::from("/opt/minos/bin/minos-guest")],
                    outcome: LineOutcome::Refused {
                        refusal: Refusal::GuestMissing,
                        detail: SafeText::inline("No such file or directory"),
                    },
                },
            ],
            isolation: Isolation::Hardware,
            layers: BTreeSet::from([ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits]),
            sweep: SweepStatus::Swept {
                root: PathBuf::from("/private/tmp/minos-501"),
                entries: vec![SweepEntry::Removed {
                    dir: PathBuf::from("/private/tmp/minos-501/mnsabcd1234"),
                }],
            },
            builder: BuilderStatus::NotFound,
        }
    }

    /// Asserts each of `parts` appears in `text`, each after the one before.
    fn assert_in_order(text: &str, parts: &[&str]) {
        let mut at = 0;
        for part in parts {
            let rest = text.get(at..).unwrap_or_default();
            match rest.find(part) {
                Some(found) => at = at.saturating_add(found).saturating_add(part.len()),
                None => panic!("{part:?} missing or out of order in:\n{text}"),
            }
        }
    }

    #[test]
    fn render_doctor_lists_everything() {
        let rendered = render_doctor(&report());
        assert_in_order(
            &rendered,
            &[
                "rejected  /opt/minos/bin/minos-guest  not found",
                "ok       helper present  a regular file of 10 bytes",
                "  checked: /opt/minos/bin/minos-vmm",
                "refused  guest kernel  the guest kernel library was not found; install libkrunfw",
                "  saw: No such file or directory",
                &format!("  checked: {KRUNFW_PATH}"),
                "refused  guest binary  no static aarch64 Linux minos-guest was found; build it with cargo build -p minos-guest --release --target aarch64-unknown-linux-musl",
                "  checked: /opt/minos/bin/minos-guest",
                "confinement: hardware, seatbelt, rlimits",
                "dedicated user: missing (deferred, reduced hardening)",
                "job sweep: /private/tmp/minos-501",
                "  removed /private/tmp/minos-501/mnsabcd1234",
                "builder: not found: minos env build needs Docker with buildx",
                "result: 2 failed",
            ],
        );
        let refused = rendered
            .lines()
            .filter(|line| line.starts_with("refused"))
            .count();
        assert_eq!(refused, 2);
    }

    #[test]
    fn render_doctor_strips_escapes() {
        let mut report = report();
        report.probes.push(ProbeLine {
            check: "guest binary",
            paths: vec![PathBuf::from("/tmp/\u{1b}[31mevil")],
            outcome: LineOutcome::Passed(SafeText::inline("saw \u{1b}[31mred")),
        });
        report.binaries.push(BinaryLine {
            path: PathBuf::from("/tmp/\u{1b}]0;title\u{7}minos-vmm"),
            accepted: false,
            reason: "not found",
        });
        report.sweep = SweepStatus::Swept {
            root: PathBuf::from("/private/tmp/minos-501"),
            entries: vec![SweepEntry::Unverified {
                dir: PathBuf::from("/private/tmp/minos-501/\u{1b}[2Jmns"),
                reason: "the pid file is not plain decimal",
            }],
        };
        report.builder = BuilderStatus::Ready {
            docker: PathBuf::from("/usr/local/bin/\u{1b}[31mdocker"),
            buildx: SafeText::inline("v0.29.1\u{1b}[0m"),
            daemon: SafeText::inline("28.5.1"),
        };

        let rendered = render_doctor(&report);

        assert!(
            !rendered.contains('\u{1b}'),
            "an ESC byte survived:\n{rendered}"
        );
        assert!(
            !rendered.contains('\u{7}'),
            "a BEL byte survived:\n{rendered}"
        );
        assert!(rendered.contains("saw [31mred"));
        assert!(rendered.contains("  checked: /tmp/[31mevil"));
    }

    #[test]
    fn doctor_exit_code() {
        let mut report = report();
        assert_eq!(report.failures(), 2);
        assert_eq!(report.exit_code(), ExitCode::FAILURE);

        report.probes.retain(|probe| !probe.is_refused());
        assert_eq!(report.failures(), 0);
        assert_eq!(report.exit_code(), ExitCode::SUCCESS);
        assert!(render_doctor(&report).contains("result: every check passed"));

        report.sweep = SweepStatus::Failed {
            error: SafeText::inline("the job root /private/tmp/minos-501 is a symlink; remove it"),
        };
        assert_eq!(report.failures(), 1);
        assert_eq!(report.exit_code(), ExitCode::FAILURE);
    }

    #[test]
    fn setup_argv_is_documented() {
        let vmm = Path::new("/opt/minos/bin/minos-vmm");
        let entitlements = Path::new("/private/tmp/minos-setup-x/entitlements.plist");
        let expected: Vec<OsString> = [
            "/usr/bin/codesign",
            "--force",
            "--sign",
            "-",
            "--options",
            "runtime",
            "--entitlements",
            "/private/tmp/minos-setup-x/entitlements.plist",
            "/opt/minos/bin/minos-vmm",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        assert_eq!(setup_codesign_argv(vmm, entitlements), expected);
    }

    #[test]
    fn setup_entitlements_are_the_committed_plist() -> TestResult {
        let committed = fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../minos-vmm/minos-vmm.entitlements"
        ))?;
        assert_eq!(ENTITLEMENTS, committed.as_slice());
        assert!(entitlements_present(ENTITLEMENTS));
        Ok(())
    }

    #[test]
    fn buildx_version_picks_the_version_token() {
        assert_eq!(
            buildx_version("github.com/docker/buildx v0.29.1-desktop.1 abc123"),
            "v0.29.1-desktop.1"
        );
        assert_eq!(buildx_version("buildx unknown"), "buildx unknown");
    }
}
