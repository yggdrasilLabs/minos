//! The shared harness of the ignored VM tests (plan 05-17): real VMs on this Mac, through the production path.
//!
//! Every launch goes through the same code the CLI and the runner use: a [`SandboxSpec`] built from a
//! [`JobRequest`], sealed, cleared by [`preflight`] against the backend's own probed capabilities, and launched by
//! [`KrunBackend`] under the embedded deny-default Seatbelt profile, with the chunked output path and the per-job
//! token. Nothing here bypasses a check: the harness only supplies the inputs a project would.
//!
//! The root image is a test image: a tiny rootfs (`/etc`, `/root`, `/usr/local/bin/minos-probe`) packed by the
//! production packer with the static musl `minos-guest` as `/init`, so no Docker daemon is needed. It is stored
//! under the `FileStore` layout `<project>/.minos/blobs/sha256/<2 hex>/<62 hex>` and handed to the backend as a
//! [`VerifiedPath`] (05-CONTEXT D-14), the way `FileBlobSource` does in the CLI; `minos-store` stays out of this
//! crate's graph (`deny.toml`), so the harness writes the layout itself.
//!
//! Every byte that came out of a VM is printed or parsed only through a sanitizing exit: the console through
//! [`Finished::console_excerpt`], and probe results through [`Untrusted::take`] and
//! `Untrusted::into_log_excerpt` before JSON parsing.
//!
//! The `MINOS_*` variables are read only here, by ignored tests (05-CONTEXT D-04). A missing one is an error that
//! carries [`SETUP`], never a skip.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use minos_backend_krun::host::HostPaths;
use minos_backend_krun::jobdir::{JOB_ROOT_PARENT, JobRoot};
use minos_backend_krun::rootfs::{PackLimits, check_guest_elf, pack_root_image};
use minos_backend_krun::{KrunBackend, KrunRunning};
use minos_core::diff::Diff;
use minos_core::diff::cumulative::CumulativeDiff;
use minos_core::experiment::Experiment;
use minos_core::guest::{GuestJob, GuestStatus, StageEnd, StageReport};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::ConfinementLayer;
use minos_core::limits::{Limits, MAX_LOG_EXCERPT_BYTES};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Stage;
use minos_core::path::RelPath;
use minos_sandbox::{
    Backend, BlobSource, BlobSourceError, Candidate, Ended, Finished, GuestOutput, InputDisk,
    Judge, JudgeData, Machine, Role, RunningVm, SandboxSpec, Untrusted, VerifiedPath, preflight,
};
use tar::{Builder, EntryType, Header};

/// The result type of every harness function and VM test.
pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to build, sign, and set before the VM tests run (CLAUDE.md Commands).
pub const SETUP: &str = "\
build and sign first (re-sign after any cargo command that relinks minos-vmm):
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest   (minos-probe must sit beside it)
  MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
and run:
  cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1";

/// Where the probe sits in the test image, and the program the build and run argvs name.
pub const PROBE: &str = "/usr/local/bin/minos-probe";

/// A second copy of the probe in the test image, root-owned with mode 04755 (setuid root), as a stock base
/// image's `su` or `mount` would be. The packer keeps the bit; the guest's `nosuid` root makes it inert
/// (review WR-01).
pub const PROBE_SETUID: &str = "/usr/local/bin/minos-probe-setuid";

/// The mode of [`PROBE_SETUID`] in the test rootfs.
pub const PROBE_SETUID_MODE: u32 = 0o4755;

/// The probe's binary name beside `MINOS_GUEST`, and the bare word a judge argv uses (it is on the stage `PATH`).
pub const PROBE_NAME: &str = "minos-probe";

/// The one writable file of every test manifest.
pub const VALUE_PATH: &str = "src/value.txt";

/// Most bytes read from one guest or probe binary: the CLI locator's cap.
const BINARY_CAP_BYTES: u64 = 64 * 1024 * 1024;

/// Bytes of console tail printed after each launch.
const CONSOLE_PRINT_BYTES: usize = 2048;

/// An absolute path from the required environment variable `name`.
///
/// # Errors
///
/// When `name` is unset or not absolute; the error carries [`SETUP`].
pub fn env_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {SETUP}"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path; {SETUP}").into());
    }
    Ok(fs::canonicalize(&path).map_err(|error| {
        format!(
            "{name}={} cannot be resolved ({error}); {SETUP}",
            path.display()
        )
    })?)
}

/// Reads a binary under [`BINARY_CAP_BYTES`], checking its size before the read.
fn read_binary(path: &Path) -> TestResult<Vec<u8>> {
    let meta = fs::metadata(path)?;
    if !meta.is_file() || meta.len() > BINARY_CAP_BYTES {
        return Err(format!(
            "{} is not a regular file under {BINARY_CAP_BYTES} bytes; {SETUP}",
            path.display()
        )
        .into());
    }
    Ok(fs::read(path)?)
}

// ---------------------------------------------------------------------------------------------------------------
// The blob source
// ---------------------------------------------------------------------------------------------------------------

/// A [`BlobSource`] over an in-memory map and the verified paths of the root images the harness stored.
///
/// Cloning shares the map, so the harness keeps putting blobs after the backend took its copy.
#[derive(Clone)]
pub struct MapBlobSource {
    blobs: Arc<Mutex<BTreeMap<BlobId, Vec<u8>>>>,
    images: BTreeMap<ImageDigest, VerifiedPath>,
}

impl MapBlobSource {
    /// Stores `bytes` and returns their ID.
    ///
    /// # Errors
    ///
    /// When the map's lock is poisoned.
    pub fn put(&self, bytes: &[u8]) -> TestResult<BlobId> {
        let id = BlobId::of(bytes);
        self.blobs
            .lock()
            .map_err(|_| "the blob map lock is poisoned")?
            .insert(id, bytes.to_vec());
        Ok(id)
    }
}

impl BlobSource for MapBlobSource {
    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, BlobSourceError> {
        let blobs = self.blobs.lock().map_err(|_| BlobSourceError::Unsafe {
            what: "blob map lock",
        })?;
        let bytes = blobs.get(id).ok_or(BlobSourceError::Missing { id: *id })?;
        let size =
            u64::try_from(bytes.len()).map_err(|_| BlobSourceError::Unsafe { what: "size" })?;
        if size > max {
            return Err(BlobSourceError::TooLarge { id: *id, size, max });
        }
        Ok(bytes.clone())
    }

    fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
        self.images
            .get(digest)
            .cloned()
            .ok_or(BlobSourceError::RootImageMissing { digest: *digest })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The VM backend
// ---------------------------------------------------------------------------------------------------------------

/// A production [`KrunBackend`] over a test root image, its blob source, and the temporary directory that holds
/// the project store and the job root.
///
/// Fields drop in order: the backend first, the directory with every job and the image last.
pub struct Vm {
    backend: KrunBackend<MapBlobSource>,
    blobs: MapBlobSource,
    image: ImageDigest,
    _dir: tempfile::TempDir,
}

/// A rootfs tar entry: a directory (`bytes` `None`) or a regular file.
fn append(
    builder: &mut Builder<Vec<u8>>,
    path: &str,
    mode: u32,
    bytes: Option<&[u8]>,
) -> TestResult {
    let mut header = Header::new_ustar();
    header.set_path(path)?;
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    let data = bytes.unwrap_or_default();
    header.set_size(u64::try_from(data.len())?);
    header.set_entry_type(if bytes.is_some() {
        EntryType::Regular
    } else {
        EntryType::Directory
    });
    header.set_cksum();
    builder.append(&header, data)?;
    Ok(())
}

/// The test rootfs tar: `/etc`, `/root`, `/usr/local/bin/minos-probe` (0755), and the same binary again as
/// [`PROBE_SETUID`] (04755, uid 0).
fn test_rootfs(probe: &[u8]) -> TestResult<Vec<u8>> {
    let mut builder = Builder::new(Vec::new());
    append(&mut builder, "etc/", 0o755, None)?;
    append(&mut builder, "root/", 0o700, None)?;
    append(&mut builder, "usr/", 0o755, None)?;
    append(&mut builder, "usr/local/", 0o755, None)?;
    append(&mut builder, "usr/local/bin/", 0o755, None)?;
    append(
        &mut builder,
        "usr/local/bin/minos-probe",
        0o755,
        Some(probe),
    )?;
    append(
        &mut builder,
        PROBE_SETUID.trim_start_matches('/'),
        PROBE_SETUID_MODE,
        Some(probe),
    )?;
    Ok(builder.into_inner()?)
}

/// Builds the backend: reads and checks the guest and the probe, packs and stores the test root image, opens a
/// job root in a fresh directory under `/private/tmp`, and builds [`KrunBackend::with_job_root`], whose host
/// probes must all pass with the Seatbelt and rlimits layers present.
///
/// # Errors
///
/// When a variable is missing, a binary is not a static aarch64 ELF, packing fails, or a host probe refuses
/// (the helper is unsigned, the kernel's hash differs, Seatbelt does not deny); each error carries [`SETUP`].
pub fn vm() -> TestResult<Vm> {
    let vmm = env_path("MINOS_VMM")?;
    let guest_path = env_path("MINOS_GUEST")?;
    let krunfw = env_path("MINOS_KRUNFW")?;
    let guest = read_binary(&guest_path)?;
    check_guest_elf(&guest)
        .map_err(|error| format!("MINOS_GUEST is not a static aarch64 ELF ({error}); {SETUP}"))?;
    let probe_path = guest_path.with_file_name(PROBE_NAME);
    let probe = read_binary(&probe_path)?;
    check_guest_elf(&probe).map_err(|error| {
        format!(
            "{} is not a static aarch64 ELF ({error}); {SETUP}",
            probe_path.display()
        )
    })?;

    let image = pack_root_image(
        test_rootfs(&probe)?.as_slice(),
        &guest,
        &PackLimits::default(),
    )?
    .into_bytes();
    let image_id = BlobId::of(&image);
    let digest = ImageDigest::from_blob_id(&image_id);

    let dir = tempfile::Builder::new()
        .prefix("mvt")
        .tempdir_in(JOB_ROOT_PARENT)?;
    let hex = image_id.to_string();
    let (shard, rest) = hex
        .split_at_checked(2)
        .ok_or("a blob ID is 64 hex digits")?;
    let shard_dir = dir.path().join("proj/.minos/blobs/sha256").join(shard);
    fs::create_dir_all(&shard_dir)?;
    let image_path = shard_dir.join(rest);
    fs::write(&image_path, &image)?;
    let verified = VerifiedPath::new(fs::canonicalize(&image_path)?)?;

    let blobs = MapBlobSource {
        blobs: Arc::new(Mutex::new(BTreeMap::new())),
        images: BTreeMap::from([(digest, verified)]),
    };
    let root = JobRoot::open_at(dir.path())?;
    let backend =
        KrunBackend::with_job_root(HostPaths::new(vmm, guest_path, krunfw), blobs.clone(), root)?;
    let caps = backend.capabilities();
    if !caps.host_refusals().is_empty() {
        return Err(format!(
            "the host refused: {:?}; run target/debug/minos host doctor; {SETUP}",
            caps.host_refusals()
        )
        .into());
    }
    for layer in [ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits] {
        if !caps.layers().contains(&layer) {
            return Err(format!("the {layer:?} layer is missing; {SETUP}").into());
        }
    }
    Ok(Vm {
        backend,
        blobs,
        image: digest,
        _dir: dir,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------------------------------------------

/// A TOML array of strings, from JSON (the two agree for the plain strings the tests use).
fn toml_array(argv: &[&str]) -> TestResult<String> {
    Ok(serde_json::to_string(argv)?)
}

/// A whole-second TOML duration from milliseconds.
fn toml_seconds(ms: u64) -> TestResult<String> {
    if !ms.is_multiple_of(1000) {
        return Err(format!("a manifest budget is whole seconds, not {ms} ms").into());
    }
    Ok(format!("{}s", ms / 1000))
}

/// The test project's manifest: writable [`VALUE_PATH`], the given build, run, and judge argvs, memory and
/// scratch, and the build, run, and judge budgets in milliseconds (whole seconds), with a gate on `probe_ok`.
///
/// Build and run may name [`PROBE`] by its absolute path; the judge argv must be bare words, such as
/// `["minos-probe", "scorecard"]`, because the manifest refuses a judge argument that looks like a path and is not
/// a project path under `[source] exclude`.
///
/// # Errors
///
/// When the manifest does not parse.
pub fn manifest(
    build: &[&str],
    run: &[&str],
    judge: &[&str],
    memory_mib: u16,
    scratch_mib: u16,
    budgets_ms: (u64, u64, u64),
) -> TestResult<Manifest> {
    let (build_ms, run_ms, judge_ms) = budgets_ms;
    let text = format!(
        r#"[project]
name = "vmtest"
kind = "optimize"
program = "program.md"

[surface]
writable = ["{VALUE_PATH}"]

[source]
exclude = ["judge/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}
memory_mib = {memory_mib}
scratch_mib = {scratch_mib}

[commands]
build = {build}
run = {run}

[budget]
build = "{build_s}"
run = "{run_s}"
judge = "{judge_s}"
spend_usd = 1

[judge]
version = 1
command = {judge}

[[judge.gate]]
metric = "probe_ok"

[[judge.objective]]
metric = "probe_score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 1

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 1
"#,
        build = toml_array(build)?,
        run = toml_array(run)?,
        judge = toml_array(judge)?,
        build_s = toml_seconds(build_ms)?,
        run_s = toml_seconds(run_ms)?,
        judge_s = toml_seconds(judge_ms)?,
    );
    Ok(Manifest::parse_toml(&text)?)
}

/// A base tarball of regular files `(path, bytes, mode)`, sorted by path, with hand-built ustar headers (mtime,
/// uid, and gid 0), as `minos-source` packs a snapshot.
///
/// # Errors
///
/// When a path does not fit a ustar header.
pub fn base_tar(files: &[(&str, &[u8], u32)]) -> TestResult<Vec<u8>> {
    let mut sorted = files.to_vec();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    let mut builder = Builder::new(Vec::new());
    for (path, bytes, mode) in sorted {
        append(&mut builder, path, mode, Some(bytes))?;
    }
    Ok(builder.into_inner()?)
}

/// A one-hunk diff of the one-line file `path` from `old_line` to `new_line` (each without its newline).
#[must_use]
pub fn diff_text(path: &str, old_line: &str, new_line: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,1 @@\n-{old_line}\n+{new_line}\n"
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Launches
// ---------------------------------------------------------------------------------------------------------------

/// The judge version every test job names.
fn judge_version(manifest: &Manifest) -> JudgeVersion {
    JudgeVersion::of(&TreeId::of(b"minos vm test judge data"), manifest.judge())
}

/// Clears `spec` against the backend's capabilities and launches it, without waiting.
fn start<R: Role>(vm: &Vm, spec: SandboxSpec<R>) -> TestResult<KrunRunning<R>> {
    let cleared = preflight(&vm.backend.capabilities(), spec.seal())
        .map_err(|refusals| format!("preflight refused: {refusals:?}"))?;
    Ok(vm.backend.launch(cleared)?)
}

/// Waits for `running` and prints what the host observed.
fn finish<R: Role>(running: KrunRunning<R>, label: &str) -> TestResult<Finished<R>> {
    let finished = running.wait()?;
    println!(
        "== {label}: ended {:?} after {:?}",
        finished.ended(),
        finished.elapsed()
    );
    if finished.ended() != Ended::Exited {
        let console = finished.console_excerpt(CONSOLE_PRINT_BYTES);
        println!("== {label}: console tail\n{}", console.text().as_str());
    }
    Ok(finished)
}

/// Clears `spec`, launches it, waits, and prints what the host observed.
fn launch<R: Role>(vm: &Vm, spec: SandboxSpec<R>, label: &str) -> TestResult<Finished<R>> {
    finish(start(vm, spec)?, label)
}

/// Runs one candidate: the base tarball `base`, and the diff `diff` (the baseline when `None`), under `manifest`,
/// on one vCPU with the manifest's memory, trial 0.
///
/// # Errors
///
/// When the diff does not parse, a spec or the preflight refuses, the launch fails, or the backend reports an
/// infrastructure failure.
pub fn candidate(
    vm: &Vm,
    manifest: &Manifest,
    base: &[u8],
    diff: Option<&str>,
) -> TestResult<Finished<Candidate>> {
    finish(start_candidate(vm, manifest, base, diff)?, "candidate")
}

/// As [`candidate`], but returns the running VM without waiting, so `exit_kill9` can read the helper's pid and
/// job directory (plan 05-19).
///
/// # Errors
///
/// As [`candidate`], except that nothing has been waited for.
pub fn start_candidate(
    vm: &Vm,
    manifest: &Manifest,
    base: &[u8],
    diff: Option<&str>,
) -> TestResult<KrunRunning<Candidate>> {
    let tree = TreeId::of(base);
    let base_id = vm.blobs.put(base)?;
    if base_id != tree.to_blob_id() {
        return Err("the base blob is not stored under its tree ID".into());
    }
    let budget = JobBudget::from_budget(manifest.budget());
    let request = match diff {
        Some(text) => {
            let diff = Diff::parse(text, manifest.writable(), &Limits::standard())?;
            let diff_id = vm.blobs.put(diff.as_bytes())?;
            let id = ExperimentId::of(&tree, &diff);
            let experiment = Experiment::candidate(CumulativeDiff::recorded(tree, diff, &id)?);
            JobRequest::candidate(&experiment, diff_id, judge_version(manifest), 0, budget)?
        }
        None => JobRequest::baseline(tree, judge_version(manifest), 0, budget)?,
    };
    let job = vm
        .blobs
        .put(&GuestJob::candidate(manifest, &request).to_json()?)?;
    let input = InputDisk::candidate(job, request.base(), request.diff());
    let spec = SandboxSpec::<Candidate>::new(
        &request,
        vm.image,
        input,
        Machine::new(1, manifest.environment().memory_mib())?,
        Arch::Arm64,
    )?;
    start(vm, spec)
}

/// Runs one judge over the judge data tarball `judge_data` and the candidate output tarball `candidate_out`,
/// under `manifest`, on one vCPU with the manifest's memory.
///
/// # Errors
///
/// As [`candidate`].
pub fn judge(
    vm: &Vm,
    manifest: &Manifest,
    judge_data: &[u8],
    candidate_out: &[u8],
) -> TestResult<Finished<Judge>> {
    let request = JobRequest::baseline(
        TreeId::of(b"minos vm test judged tree"),
        judge_version(manifest),
        0,
        JobBudget::from_budget(manifest.budget()),
    )?;
    let job = vm
        .blobs
        .put(&GuestJob::judge(manifest, &request).to_json()?)?;
    let data = vm.blobs.put(judge_data)?;
    let out = vm.blobs.put(candidate_out)?;
    let spec = SandboxSpec::<Judge>::new(
        &request,
        vm.image,
        InputDisk::judge(job, out),
        Machine::new(1, manifest.environment().memory_mib())?,
        Arch::Arm64,
    )?
    .secrets(JudgeData::new(data));
    launch(vm, spec, "judge")
}

// ---------------------------------------------------------------------------------------------------------------
// Reading output
// ---------------------------------------------------------------------------------------------------------------

/// The output of a launch that the host saw end as [`Ended::Exited`], with PID 1's validated status taken.
///
/// # Errors
///
/// When the launch did not end `Exited`, the stream breaks a cap or a rule, or the status is missing or invalid.
pub fn exited<R: Role>(
    finished: Finished<R>,
) -> TestResult<(Untrusted<GuestOutput<R>>, GuestStatus)> {
    if finished.ended() != Ended::Exited {
        return Err(format!("the launch ended {:?}, not Exited", finished.ended()).into());
    }
    let mut output = finished.into_output()?;
    let status = output.take_status()?;
    Ok((output, status))
}

/// How `stage` ended in `status`.
///
/// # Errors
///
/// When the status has no report for `stage`.
pub fn stage_end(status: &GuestStatus, stage: Stage) -> TestResult<StageEnd> {
    status
        .stages()
        .iter()
        .find(|report| report.stage() == stage)
        .map(StageReport::end)
        .ok_or_else(|| format!("the status has no {stage:?} stage").into())
}

/// Removes PID 1's stage logs from `output`, printing each one's sanitized tail, so what remains is the stage
/// output only.
pub fn drop_logs<R: Role>(output: &mut Untrusted<GuestOutput<R>>) -> TestResult {
    for log in [".minos/build.log", ".minos/run.log", ".minos/judge.log"] {
        if let Some(file) = output.take(&RelPath::try_from(log)?) {
            let excerpt = file.into_log_excerpt(512);
            if !excerpt.text().as_str().is_empty() {
                println!("== {log} tail: {}", excerpt.text().as_str());
            }
        }
    }
    Ok(())
}

/// The JSON a probe wrote at output path `path`, read through the sanitizing log excerpt.
///
/// # Errors
///
/// When the file is missing, longer than one excerpt, or not JSON.
pub fn probe_json<R: Role>(
    output: &mut Untrusted<GuestOutput<R>>,
    path: &str,
) -> TestResult<serde_json::Value> {
    let file = output
        .take(&RelPath::try_from(path)?)
        .ok_or_else(|| format!("the output has no {path}"))?;
    let excerpt = file.into_log_excerpt(MAX_LOG_EXCERPT_BYTES);
    if excerpt.is_truncated() {
        return Err(format!("{path} is longer than one log excerpt").into());
    }
    let value: serde_json::Value = serde_json::from_str(excerpt.text().as_str())?;
    println!("== {path}: {value}");
    Ok(value)
}

/// The value of metric `name` in `scorecard`.
///
/// # Errors
///
/// When `name` is not a metric name or not in the scorecard.
pub fn metric(scorecard: &Scorecard, name: &str) -> TestResult<Metric> {
    scorecard
        .value(&MetricName::from_str(name)?)
        .ok_or_else(|| format!("the scorecard has no {name}").into())
}
