//! The permanent exit tests of Phase 5 (plan 05-19), promoted from the Phase 2 spikes onto the production backend
//! and guest (02-CONTEXT D-02).
//!
//! Success criterion 1 of Phase 5 is never traded for schedule: egress fails closed (`exit_egress`, KRUN-03), no
//! host value reaches the guest's environment or command line (`exit_env`, GUEST-05), `kill -9` of the process
//! that launched a VM leaves no helper and no job directory (`exit_kill9`, KRUN-05), and a candidate cannot forge
//! the output stream or read its token (`exit_forge`, GUEST-04; 05-RESEARCH Pitfall 1, Assumption A1).
//!
//! Every test is ignored: each needs the signed helper, the musl guest with `minos-probe` beside it, and libkrunfw
//! ([`harness::SETUP`]); `exit_env` also needs `MINOS_EXIT_CANARY`, and `exit_egress` reads the optional
//! `MINOS_EXIT_HOST_IP` for a LAN target. Every launch goes through [`harness::vm`]'s production `KrunBackend`,
//! and every guest-derived value is read only through parsed probe JSON or a log excerpt. Run:
//! `MINOS_EXIT_CANARY=$(uuidgen) cargo test -p minos-backend-krun --test exit -- --ignored --nocapture
//! --test-threads=1`, with the three `MINOS_*` paths set.

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use minos_backend_krun::KrunBackend;
use minos_backend_krun::host::HostPaths;
use minos_backend_krun::jobdir::{JOB_ROOT_PARENT, JobRoot, SweepEntry};
use minos_core::guest::StageEnd;
use minos_core::id::{BlobId, ImageDigest};
use minos_core::limits::Limits;
use minos_core::manifest::Manifest;
use minos_core::metric::Metric;
use minos_core::outcome::Stage;
use minos_sandbox::{BlobSource, BlobSourceError, Ended, VerifiedPath};
use serde_json::Value;

use harness::{
    PROBE, PROBE_NAME, SETUP, TestResult, VALUE_PATH, base_tar, candidate, diff_text, drop_logs,
    env_path, exited, judge, manifest, metric, probe_json, stage_end, start_candidate, vm,
};

/// The default VM memory.
const MEMORY_MIB: u16 = 2048;
/// The default scratch cap.
const SCRATCH_MIB: u16 = 1024;
/// Build, run, and judge budgets: one minute each.
const BUDGETS: (u64, u64, u64) = (60_000, 60_000, 60_000);
/// The judge argv: bare words, so the manifest's judge-path rule holds.
const JUDGE: [&str; 2] = [PROBE_NAME, "scorecard"];
/// A stage that does nothing and succeeds.
const NOTHING: [&str; 3] = [PROBE, "exit", "0"];
/// The virtio device ids that must never appear: net (0x0001) and fs (0x001a).
const FORBIDDEN_VIRTIO: [&str; 2] = ["0x0001", "0x001a"];
/// The shortest canary `exit_env` accepts.
const MIN_CANARY_CHARS: usize = 16;
/// The key of the one host-derived `/proc/cmdline` token the user allowed at the 05-19 checkpoint.
const BOOT_START_KEY: &str = "KRUN_BOOT_START_NS=";
/// The guest's root-disk `/proc/cmdline`, split on whitespace (02-SPIKE-EVIDENCE.md S01-CMDLINE, with
/// `dummy.numdummies=0` from S01-NETIFACES); `None` marks the one allowed `KRUN_BOOT_START_NS=<digits>` token.
const EXPECTED_CMDLINE: [Option<&str>; 18] = [
    Some("reboot=k"),
    Some("panic=-1"),
    Some("panic_print=0"),
    Some("nomodule"),
    Some("console=hvc0"),
    Some("rootfstype=virtiofs"),
    Some("rw"),
    Some("quiet"),
    Some("no-kvmapf"),
    Some("rootfstype=squashfs"),
    Some("ro"),
    Some("ignore_loglevel"),
    Some("dummy.numdummies=0"),
    Some("root=/dev/root"),
    Some("init=/init"),
    Some("root=/dev/vda"),
    None,
    Some("--"),
];
/// Every stage's whole environment (`minos_guest::stage::STAGE_ENV`).
const STAGE_ENV: [(&str, &str); 4] = [
    (
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    ),
    ("HOME", "/scratch/home"),
    ("TMPDIR", "/tmp"),
    ("LANG", "C"),
];
/// The environment variable that turns `exit_kill9` into the launcher the parent kills.
const KILL9_CHILD: &str = "MINOS_EXIT_KILL9_CHILD";
/// The prefix of the launcher's report line.
const HELPER_PID: &str = "HELPER_PID=";
/// The separator before the job directory in the launcher's report line.
const JOB_DIR: &str = " JOB_DIR=";
/// How long the parent waits for the launcher's report line.
const REPORT_TIMEOUT: Duration = Duration::from_mins(1);
/// How long the helper may outlive its killed launcher: the parent-death watchdog's bound.
const HELPER_EXIT_BOUND: Duration = Duration::from_secs(5);
/// How often the parent polls `ps` for the helper.
const POLL: Duration = Duration::from_millis(50);
/// How long the launcher sleeps with its VM running before it gives up on being killed.
const LAUNCHER_SLEEP: Duration = Duration::from_mins(2);
/// The prefix of the harness's temporary directories under [`JOB_ROOT_PARENT`].
const HARNESS_DIR_PREFIX: &str = "mvt";

/// The base tree every candidate starts from.
fn base() -> TestResult<Vec<u8>> {
    base_tar(&[(VALUE_PATH, b"old\n", 0o644)])
}

/// A manifest with the default memory and the given budgets.
fn project(build: &[&str], run: &[&str], budgets_ms: (u64, u64, u64)) -> TestResult<Manifest> {
    manifest(build, run, &JUDGE, MEMORY_MIB, SCRATCH_MIB, budgets_ms)
}

/// The field `key` of a probe result.
fn field<'a>(value: &'a Value, key: &str) -> TestResult<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| format!("the probe result has no {key}").into())
}

/// The string field `key`.
fn text<'a>(value: &'a Value, key: &str) -> TestResult<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("{key} is not a string").into())
}

/// The array field `key`.
fn list<'a>(value: &'a Value, key: &str) -> TestResult<&'a [Value]> {
    Ok(field(value, key)?
        .as_array()
        .ok_or_else(|| format!("{key} is not an array"))?
        .as_slice())
}

/// Each probe attempt under `key`: its target, whether it worked, and its OS error.
fn attempts(value: &Value, key: &str) -> TestResult<Vec<(String, bool, Option<i64>)>> {
    list(value, key)?
        .iter()
        .map(|attempt| {
            Ok((
                text(attempt, "target")?.to_owned(),
                field(attempt, "ok")?
                    .as_bool()
                    .ok_or("ok is not a boolean")?,
                field(attempt, "os_error")?.as_i64(),
            ))
        })
        .collect()
}

/// Asserts no connection reached `listener`: a nonblocking accept must find nothing.
fn assert_untouched(listener: &TcpListener, label: &str) -> TestResult {
    match listener.accept() {
        Ok((_, peer)) => {
            Err(format!("the {label} canary accepted a connection from {peer}").into())
        }
        Err(error) if error.kind() == ErrorKind::WouldBlock => {
            println!("== {label} canary: no connection arrived");
            Ok(())
        }
        Err(error) => Err(format!("the {label} canary's accept failed: {error}").into()),
    }
}

/// A nonblocking canary listener on `addr`.
fn canary(addr: SocketAddr) -> TestResult<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn exit_egress() -> TestResult {
    let v4 = canary(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    let v6 = canary(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0))?;
    let v4_port = v4.local_addr()?.port().to_string();
    let v6_port = v6.local_addr()?.port().to_string();
    let host_ip: Option<IpAddr> = match std::env::var("MINOS_EXIT_HOST_IP") {
        Ok(value) => Some(value.parse()?),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(format!("MINOS_EXIT_HOST_IP: {error}").into()),
    };
    let lan = match host_ip {
        Some(ip) => Some(canary(SocketAddr::new(ip, v4.local_addr()?.port()))?),
        None => None,
    };
    let host_text = host_ip.map(|ip| ip.to_string());
    match &host_text {
        Some(ip) => println!("== LAN target: runs against {ip}"),
        None => println!("== LAN target: not run (MINOS_EXIT_HOST_IP is unset)"),
    }

    let mut run = vec![PROBE, "egress", "/out/egress.json", &v4_port, &v6_port];
    if let Some(ip) = &host_text {
        run.push(ip);
    }
    let manifest = project(&NOTHING, &run, BUDGETS)?;
    let (mut output, status) = exited(candidate(&vm()?, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    drop_logs(&mut output)?;
    let egress = probe_json(&mut output, "out/egress.json")?;

    let tried = attempts(&egress, "attempts")?;
    assert_eq!(tried.len(), if host_text.is_some() { 6 } else { 5 });
    for (target, ok, os_error) in &tried {
        println!("== connect {target}: connected {ok}, errno {os_error:?}");
        assert!(!ok, "the guest connected to {target}");
        assert!(os_error.is_some(), "{target} failed with no OS error");
    }
    assert_untouched(&v4, "127.0.0.1")?;
    assert_untouched(&v6, "[::1]")?;
    if let Some(listener) = &lan {
        assert_untouched(listener, "LAN")?;
    }

    let ifaces: Vec<&str> = list(&egress, "net_ifaces")?
        .iter()
        .map(|name| name.as_str().ok_or("an interface name is not a string"))
        .collect::<Result<_, _>>()?;
    println!("== interfaces: {ifaces:?}");
    assert!(
        ifaces.is_empty() || ifaces == ["lo"],
        "the guest sees interfaces {ifaces:?}"
    );
    for device in list(&egress, "virtio_devices")? {
        let name = text(device, "name")?;
        let id = text(device, "device")?;
        println!("== virtio {name}: {id}");
        assert!(
            !FORBIDDEN_VIRTIO.contains(&id),
            "the guest has virtio device {name} with id {id}"
        );
    }
    Ok(())
}

/// Whether `token` is the one allowed host-derived token: `KRUN_BOOT_START_NS=` and one or more ASCII digits.
fn is_boot_start(token: &str) -> bool {
    token.strip_prefix(BOOT_START_KEY).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// GUEST-05: the candidate's environment and `/proc/cmdline` hold no host value.
///
/// One stated exception, the user's decision at the 05-19 checkpoint on 2026-10-19 (05-RESEARCH Open Question
/// 2): `/proc/cmdline` may hold exactly one `KRUN_BOOT_START_NS=` token whose value is ASCII digits only. `msb_krun`
/// 0.1.41 writes it unconditionally (`build_kernel_cmdline`, from the host's `SystemTime::now()` at VM build) with
/// no builder method, feature, or option to drop it, and no newer release exists. It reveals the host's clock at
/// boot, which the guest's own clock also reveals. It is a recorded exception, not an endorsement: CLAUDE.md Open
/// questions names the removal path the user would accept. Every other token is matched exactly.
#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW, and MINOS_EXIT_CANARY"]
fn exit_env() -> TestResult {
    let canary = std::env::var("MINOS_EXIT_CANARY").map_err(|_| {
        format!(
            "MINOS_EXIT_CANARY is not set; run with MINOS_EXIT_CANARY=$(uuidgen) (at least {MIN_CANARY_CHARS} \
             characters); {SETUP}"
        )
    })?;
    if canary.chars().count() < MIN_CANARY_CHARS {
        return Err(format!(
            "MINOS_EXIT_CANARY must be at least {MIN_CANARY_CHARS} characters; use $(uuidgen)"
        )
        .into());
    }
    let host_home = std::env::var("HOME")?;
    let host_user = std::env::var("USER")?;
    if host_home.is_empty() || host_user.is_empty() {
        return Err("the test process's HOME and USER must be set and non-empty".into());
    }

    let manifest = project(&[PROBE, "env", "/out/env.json"], &NOTHING, BUDGETS)?;
    let diff = diff_text(VALUE_PATH, "old", "new");
    let (mut output, status) = exited(candidate(&vm()?, &manifest, &base()?, Some(&diff))?)?;
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );
    let env = probe_json(&mut output, "out/env.json")?;

    let vars: BTreeMap<&str, &str> = field(&env, "vars")?
        .as_object()
        .ok_or("vars is not an object")?
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.as_str(), value))
                .ok_or("a variable's value is not a string")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(vars, BTreeMap::from(STAGE_ENV), "the stage environment");

    let cmdline = text(&env, "cmdline")?;
    let tokens: Vec<&str> = cmdline.split_whitespace().collect();
    assert_eq!(
        tokens.len(),
        EXPECTED_CMDLINE.len(),
        "the /proc/cmdline tokens {tokens:?}"
    );
    for (token, expected) in tokens.iter().zip(EXPECTED_CMDLINE) {
        match expected {
            Some(expected) => assert_eq!(*token, expected, "a /proc/cmdline token"),
            None => assert!(
                is_boot_start(token),
                "the allowed token is not {BOOT_START_KEY}<digits>: {token:?}"
            ),
        }
    }
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.starts_with("KRUN_"))
            .count(),
        1,
        "exactly one KRUN_ token"
    );

    let whole = env.to_string();
    for (label, value) in [
        ("the canary", canary.as_str()),
        ("the host HOME", host_home.as_str()),
        ("the host USER", host_user.as_str()),
    ] {
        assert!(!whole.contains(value), "{label} appears in env.json");
        println!("== {label} appears nowhere in env.json");
    }
    Ok(())
}

/// A [`BlobSource`] with no blobs: the sweeping backend of `exit_kill9` never launches.
struct NoBlobs;

impl BlobSource for NoBlobs {
    fn get(&self, id: &BlobId, _max: u64) -> Result<Vec<u8>, BlobSourceError> {
        Err(BlobSourceError::Missing { id: *id })
    }

    fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
        Err(BlobSourceError::RootImageMissing { digest: *digest })
    }
}

/// The killed launcher: SIGKILL and reap on drop, so a failing parent never leaves it running. It kills only
/// this one child process, by its own handle.
struct Launcher(Child);

impl Drop for Launcher {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// The launcher half of `exit_kill9`: starts a VM that hangs, reports its helper and job directory, and sleeps.
fn kill9_launcher() -> TestResult {
    let vm = vm()?;
    let manifest = project(&NOTHING, &[PROBE, "hang"], (60_000, 600_000, 60_000))?;
    let running = start_candidate(&vm, &manifest, &base()?, None)?;
    let mut stdout = io::stdout();
    writeln!(
        stdout,
        "\n{HELPER_PID}{}{JOB_DIR}{}",
        running.helper_pid(),
        running.job_dir().display()
    )?;
    stdout.flush()?;
    thread::sleep(LAUNCHER_SLEEP);
    drop(running);
    Err("the launcher was not killed".into())
}

/// The helper pid and job directory from the launcher's report line.
fn parse_report(line: &str) -> TestResult<(u32, PathBuf)> {
    let (_, rest) = line
        .split_once(HELPER_PID)
        .ok_or("the report has no helper pid")?;
    let (pid, dir) = rest
        .split_once(JOB_DIR)
        .ok_or("the report has no job directory")?;
    let dir = PathBuf::from(dir.trim_end());
    if !dir.is_absolute() {
        return Err(format!("the job directory {} is not absolute", dir.display()).into());
    }
    Ok((pid.parse()?, dir))
}

/// Whether `ps` still shows the process `pid`.
fn process_exists(pid: u32) -> TestResult<bool> {
    let out = Command::new("/bin/ps")
        .args(["-o", "pid=", "-p", &pid.to_string()])
        .output()?;
    Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

/// The harness directory that holds the job root `<dir>/minos-<uid>/mnsXXXXXXXX`, checked to be a direct child of
/// [`JOB_ROOT_PARENT`] named `mvt*`, so the cleanup can never remove anything else.
fn harness_dir(job_dir: &Path) -> TestResult<PathBuf> {
    let dir = job_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("the job directory has no grandparent")?;
    let parent = fs::canonicalize(JOB_ROOT_PARENT)?;
    let named = dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(HARNESS_DIR_PREFIX));
    if dir.parent() != Some(parent.as_path()) || !named {
        return Err(format!(
            "{} is not a harness directory under {}",
            dir.display(),
            parent.display()
        )
        .into());
    }
    Ok(dir.to_path_buf())
}

/// KRUN-05: after SIGKILL of the process that launched a VM, the helper exits on its own within 5 s, no process
/// names the job, and a new backend's sweep leaves no job directory.
#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn exit_kill9() -> TestResult {
    if std::env::var(KILL9_CHILD).is_ok_and(|value| value == "1") {
        return kill9_launcher();
    }
    let paths = ["MINOS_VMM", "MINOS_GUEST", "MINOS_KRUNFW"]
        .into_iter()
        .map(|name| Ok((name, env_path(name)?)))
        .collect::<TestResult<Vec<_>>>()?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "exit_kill9",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(KILL9_CHILD, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (name, path) in &paths {
        command.env(name, path);
    }
    let mut launcher = Launcher(command.spawn()?);
    let stdout = launcher
        .0
        .stdout
        .take()
        .ok_or("the launcher has no stdout")?;
    let (sender, receiver) = mpsc::channel();
    // The reader drains the launcher's stdout until it closes, so the launcher never blocks on a full pipe.
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            println!("launcher: {line}");
            if line.contains(HELPER_PID) && sender.send(line).is_err() {
                break;
            }
        }
    });
    let line = receiver.recv_timeout(REPORT_TIMEOUT).map_err(|error| {
        format!("no report from the launcher within {REPORT_TIMEOUT:?}: {error}")
    })?;
    let (pid, job_dir) = parse_report(&line)?;
    let harness_dir = harness_dir(&job_dir)?;
    assert!(process_exists(pid)?, "the helper {pid} is not running");
    assert!(job_dir.is_dir(), "{} is not a directory", job_dir.display());

    // SIGKILL of the launcher alone, by its own child handle; the helper is never signalled by this test.
    launcher.0.kill()?;
    let killed = Instant::now();
    let status = launcher.0.wait()?;
    println!("== launcher {} killed: {status}", launcher.0.id());
    loop {
        if !process_exists(pid)? {
            break;
        }
        if killed.elapsed() > HELPER_EXIT_BOUND {
            return Err(format!(
                "the helper {pid} outlived its killed launcher by more than {HELPER_EXIT_BOUND:?}"
            )
            .into());
        }
        thread::sleep(POLL);
    }
    println!(
        "== the helper {pid} exited {:?} after the launcher's SIGKILL",
        killed.elapsed()
    );

    let ps = Command::new("/bin/ps")
        .args(["-A", "-ww", "-o", "command="])
        .output()?;
    let job_text = job_dir.to_string_lossy().into_owned();
    let naming: Vec<String> = String::from_utf8_lossy(&ps.stdout)
        .lines()
        .filter(|line| line.contains(&job_text))
        .map(str::to_owned)
        .collect();
    assert!(naming.is_empty(), "processes name the job: {naming:?}");

    let job_root = job_dir.parent().ok_or("the job directory has no parent")?;
    let root = JobRoot::open_at(&harness_dir)?;
    assert_eq!(root.path(), job_root, "the job root");
    let backend = KrunBackend::with_job_root(
        HostPaths::new(
            env_path("MINOS_VMM")?,
            env_path("MINOS_GUEST")?,
            env_path("MINOS_KRUNFW")?,
        ),
        NoBlobs,
        root,
    )?;
    let entries = backend.sweep_report().entries();
    println!("== sweep: {entries:?}");
    assert!(
        entries.contains(&SweepEntry::Removed {
            dir: job_dir.clone()
        }),
        "the sweep did not remove {} as gone",
        job_dir.display()
    );
    assert!(
        fs::symlink_metadata(&job_dir).is_err_and(|error| error.kind() == ErrorKind::NotFound),
        "{} still exists",
        job_dir.display()
    );
    drop(backend);

    // The killed launcher's own temporary directory (its root image and job root), which its drop never removed.
    fs::remove_dir_all(&harness_dir)?;
    println!("== removed {}", harness_dir.display());
    Ok(())
}

/// GUEST-04: the candidate cannot read the output token's disk, and a stream with a guessed token is
/// `ChunkProtocol`; PID 1's own stream, for a judge, is accepted as a control.
#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn exit_forge() -> TestResult {
    let vm = vm()?;

    let reads = project(&NOTHING, &[PROBE, "write-outside", "/out/w.json"], BUDGETS)?;
    let (mut output, status) = exited(candidate(&vm, &reads, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    let result = probe_json(&mut output, "out/w.json")?;
    let opened: BTreeSet<String> = attempts(&result, "reads")?
        .into_iter()
        .map(|(target, ok, os_error)| {
            println!("== open {target}: opened {ok}, errno {os_error:?}");
            assert!(!ok, "the candidate opened {target}");
            assert!(os_error.is_some(), "{target} failed with no OS error");
            target
        })
        .collect();
    assert_eq!(
        opened,
        BTreeSet::from(["/dev/vdb".to_owned(), "/proc/1/environ".to_owned()])
    );

    let forge = project(&NOTHING, &[PROBE, "forge", "/out/f.json"], BUDGETS)?;
    let finished = candidate(&vm, &forge, &base()?, None)?;
    assert_eq!(
        finished.ended(),
        Ended::ChunkProtocol,
        "a stream under a guessed token"
    );

    // Control: the same receiver accepts PID 1's genuine stream and its scorecard.
    let judge_data = base_tar(&[("expected.txt", b"1\n", 0o644)])?;
    let candidate_out = base_tar(&[("out/result.json", b"{}\n", 0o644)])?;
    let (output, status) = exited(judge(&vm, &reads, &judge_data, &candidate_out)?)?;
    assert_eq!(
        stage_end(&status, Stage::Judge)?,
        StageEnd::Exited { code: 0 }
    );
    let scorecard = output.into_scorecard(&Limits::standard())?;
    assert_eq!(metric(&scorecard, "probe_ok")?, Metric::ONE);
    Ok(())
}
