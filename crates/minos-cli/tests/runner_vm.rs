//! The production runner on real VMs (plan 07-14; LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04).
//!
//! Each test composes what `minos run` will: `Runner<KrunBackend, FileStore>` behind the loop's [`JobRunner`]
//! seam through [`RunnerJobs`], over a temporary project whose judge data was pushed by [`judge_push`], whose base
//! was packed by [`snapshot`] with the lock's judge file digests, and whose root image is a fixture rootfs
//! (`/usr/local/bin/minos-probe`) packed by the production packer with the static musl `minos-guest` as `/init`, so
//! no Docker daemon is needed. Every trial calls [`JobRunner::check`] first and then [`JobRunner::run`], which boots
//! a Candidate VM and, when the candidate passes, a Judge VM with the judge data attached through `secrets`.
//!
//! They prove, on this Mac's hypervisor: a baseline is built, run, and scored, with krun on arm64 and host-measured
//! usage for both VMs (LOOP-03, JUDGE-04); a candidate's `out/` files are re-packed and reach the judge VM, which
//! scores (LOOP-01, LOOP-02); and a judge whose gate is 0 is `Rejected(GateFailed)` with no scorecard (JUDGE-03).
//! The tests see only [`minos_core::job::JobResult`] values, so they print outcomes and usage and never a guest
//! byte.
//!
//! The test lives in `minos-cli` because only the CLI may link the krun backend (`deny.toml`; 07-RESEARCH
//! Pitfall 13). Every test is ignored: it needs a signed `MINOS_VMM`, `MINOS_GUEST` with `minos-probe` beside it,
//! and `MINOS_KRUNFW`, which only this file reads (05-CONTEXT D-04). A missing one is an error that carries
//! [`SETUP`], never a skip. Build, sign, and run as [`SETUP`] says:
//! `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use minos_backend_krun::KrunBackend;
use minos_backend_krun::host::HostPaths;
use minos_backend_krun::jobdir::JOB_ROOT_PARENT;
use minos_backend_krun::rootfs::{PackLimits, check_guest_elf, pack_root_image};
use minos_cli::blob_source::FileBlobSource;
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE, MAX_ROOT_IMAGE_BYTES};
use minos_cli::jobs::{RunnerJobs, judge_inputs};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::read_lock;
use minos_core::diff::apply::Tree;
use minos_core::diff::cumulative::{BaseTree, cumulative};
use minos_core::experiment::Experiment;
use minos_core::id::{ImageDigest, MetricName};
use minos_core::job::{JobBudget, JobRequest, JobResult};
use minos_core::ledger::{BackendKind, ConfinementLayer};
use minos_core::limits::{Limits, MAX_STAGE_LOG_BYTES};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::metric::Metric;
use minos_core::outcome::{Outcome, Reason};
use minos_core::path::RelPath;
use minos_loop::JobRunner;
use minos_runner::{RUNNER_VCPUS, Runner};
use minos_sandbox::Backend;
use minos_source::{CleanPolicy, snapshot};
use minos_store::{Blobs, FileStore, STORE_DIR};
use tar::{Builder, EntryType, Header};
use tempfile::TempDir;

/// The result type of every helper and test.
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to build, sign, and set before these tests run (CLAUDE.md Commands).
const SETUP: &str = "\
build and sign first (re-sign after any cargo command that relinks minos-vmm):
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest   (minos-probe must sit beside it)
  MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
and run:
  cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1";

/// The probe's binary name beside `MINOS_GUEST`, and the bare word every argv names (it is on the stage `PATH`).
const PROBE_NAME: &str = "minos-probe";

/// The one writable file of the test project.
const VALUE_PATH: &str = "src/value.txt";

/// The writable file's contents in the base snapshot.
const BASE_VALUE: &[u8] = b"1\n";

/// The writable file's contents in the candidate.
const CANDIDATE_VALUE: &[u8] = b"2\n";

/// The score the judge probe reports.
const SCORE: u32 = 7;

/// Most bytes read from one guest or probe binary: the CLI locator's cap.
const BINARY_CAP_BYTES: u64 = 64 * 1024 * 1024;

/// A run stage that does nothing and succeeds.
const NOTHING: [&str; 3] = [PROBE_NAME, "exit", "0"];

/// A run stage that writes 4096 bytes of artifacts under `/out`.
const FILL_OUT: [&str; 4] = [PROBE_NAME, "output-fill", "/out", "4096"];

/// An absolute path from the required environment variable `name`.
///
/// # Errors
///
/// When `name` is unset, not absolute, or cannot be resolved; the error carries [`SETUP`].
fn env_path(name: &str) -> TestResult<PathBuf> {
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

/// Reads a static aarch64 ELF under [`BINARY_CAP_BYTES`], checking its size before the read.
fn read_elf(path: &Path) -> TestResult<Vec<u8>> {
    let meta = fs::metadata(path)?;
    if !meta.is_file() || meta.len() > BINARY_CAP_BYTES {
        return Err(format!(
            "{} is not a regular file under {BINARY_CAP_BYTES} bytes; {SETUP}",
            path.display()
        )
        .into());
    }
    let bytes = fs::read(path)?;
    check_guest_elf(&bytes).map_err(|error| {
        format!(
            "{} is not a static aarch64 ELF ({error}); {SETUP}",
            path.display()
        )
    })?;
    Ok(bytes)
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

/// The fixture rootfs tar: `/usr/local/bin/minos-probe` (0755) and its parent directories.
fn fixture_rootfs(probe: &[u8]) -> TestResult<Vec<u8>> {
    let mut builder = Builder::new(Vec::new());
    append(&mut builder, "usr/", 0o755, None)?;
    append(&mut builder, "usr/local/", 0o755, None)?;
    append(&mut builder, "usr/local/bin/", 0o755, None)?;
    append(
        &mut builder,
        "usr/local/bin/minos-probe",
        0o755,
        Some(probe),
    )?;
    Ok(builder.into_inner()?)
}

/// Writes `bytes` to `dir/rel`, creating parent directories.
fn write(dir: &Path, rel: &str, bytes: &[u8]) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// The test project's manifest, with the run argv `run` and a judge that reports `probe_ok` = `gate` and
/// `probe_score` = [`SCORE`].
fn manifest_text(run: &[&str], gate: u32) -> TestResult<String> {
    let judge = [
        PROBE_NAME,
        "scorecard",
        &gate.to_string(),
        &SCORE.to_string(),
    ]
    .map(str::to_owned);
    Ok(format!(
        r#"[project]
name = "runnervm"
kind = "optimize"
program = "program.md"

[surface]
writable = ["{VALUE_PATH}"]

[source]
exclude = ["target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

[commands]
build = {build}
run = {run}

[budget]
build = "1m"
run = "1m"
judge = "1m"
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
model = "haiku"
batch = 1
"#,
        build = toml_array(&NOTHING.map(str::to_owned))?,
        run = toml_array(
            &run.iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>()
        )?,
        judge = toml_array(&judge)?,
    ))
}

/// A TOML array of plain strings.
fn toml_array(words: &[String]) -> TestResult<String> {
    let quoted: Vec<String> = words
        .iter()
        .map(|word| {
            if word
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
            {
                Ok(format!("\"{word}\""))
            } else {
                Err(format!("{word:?} is not a plain TOML string"))
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(format!("[{}]", quoted.join(", ")))
}

/// A temporary project on real VMs: the production runner behind the loop's seam, and what the tests need to
/// build requests and check results.
///
/// Fields drop in order: the runner (and its backend) first, the directory with the project and its store last.
struct Project {
    jobs: RunnerJobs<KrunBackend<FileBlobSource>>,
    store: FileStore,
    manifest: Manifest,
    base: BaseTree,
    judge: JudgeVersion,
    _dir: TempDir,
}

/// Builds the project and its runner: writes the project with the run argv `run` and the judge gate `gate`,
/// pushes judge data from a sibling directory with [`judge_push`], packs and stores the fixture root image, packs
/// and stores the base with [`snapshot`] under the lock's judge file digests, and composes
/// `Runner<KrunBackend, FileStore>` behind [`RunnerJobs`].
///
/// # Errors
///
/// When a variable is missing, a binary is not a static aarch64 ELF, the project does not push, pack, or store, or
/// the host refuses (the helper is unsigned, the kernel's hash differs, Seatbelt does not deny); host errors carry
/// [`SETUP`].
fn project(run: &[&str], gate: u32) -> TestResult<Project> {
    let vmm = env_path("MINOS_VMM")?;
    let guest_path = env_path("MINOS_GUEST")?;
    let krunfw = env_path("MINOS_KRUNFW")?;
    let guest = read_elf(&guest_path)?;
    let probe = read_elf(&guest_path.with_file_name(PROBE_NAME))?;

    // The project, beside the judge data, in a fresh directory under /private/tmp.
    let dir = tempfile::Builder::new()
        .prefix("mrv")
        .tempdir_in(JOB_ROOT_PARENT)?;
    let root = fs::canonicalize(dir.path())?;
    let project = root.join("project");
    write(
        &project,
        MANIFEST_FILE,
        manifest_text(run, gate)?.as_bytes(),
    )?;
    write(
        &project,
        "program.md",
        b"Raise the value in src/value.txt.\n",
    )?;
    write(&project, "minos.Dockerfile", b"FROM scratch\n")?;
    write(&project, VALUE_PATH, BASE_VALUE)?;
    let data = root.join("judge-data");
    write(&data, "expected.txt", b"runner vm test judge data\n")?;

    // Judge data, pushed as `minos judge push` does, into the project's store and lock.
    let pushed = judge_push(&project, &data, JudgePushOptions::new(false))?;
    let manifest = Manifest::parse_toml(&fs::read_to_string(project.join(MANIFEST_FILE))?)?;
    let lock = read_lock(&project.join(LOCK_FILE))?.ok_or("judge push wrote no minos.lock")?;
    let (judge_data, judge, judge_extent) = judge_inputs(&lock, &manifest)?;
    if judge != *pushed.judge_version() {
        return Err("judge_inputs disagrees with judge push on the judge version".into());
    }

    // The fixture root image, packed by the production packer and stored as `env build` stores one.
    let store = FileStore::open(&project.join(STORE_DIR))?;
    let image = pack_root_image(
        fixture_rootfs(&probe)?.as_slice(),
        &guest,
        &PackLimits::default(),
    )?
    .into_bytes();
    let image_id = store.put_reader(image.as_slice(), MAX_ROOT_IMAGE_BYTES)?;
    let root_image = ImageDigest::from_blob_id(&image_id);

    // The base, packed with the lock's judge file digests so no judge byte can enter it, and stored by tree.
    let snap = snapshot(
        &project,
        &manifest,
        CleanPolicy::AllowDirty,
        &lock.judge_file_digests(),
    )?;
    let tree = snap.tree();
    if store.put(snap.tarball())? != tree.to_blob_id() {
        return Err("the base blob is not stored under its tree ID".into());
    }
    let mut surface = Tree::new();
    surface.insert(RelPath::try_from(VALUE_PATH)?, BASE_VALUE.to_vec());
    let base = BaseTree::new(tree, surface);

    // The production composition: the CLI's blob source feeds the backend, and the runner sits behind the seam.
    let backend = KrunBackend::new(
        HostPaths::new(vmm, guest_path, krunfw),
        FileBlobSource::open(&project)?,
    )?;
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
    let runner = Runner::new(
        backend,
        store.clone(),
        manifest.clone(),
        root_image,
        judge_data,
        judge,
        judge_extent,
    )?;
    Ok(Project {
        jobs: RunnerJobs::new(runner),
        store,
        manifest,
        base,
        judge,
        _dir: dir,
    })
}

impl Project {
    /// Trial 0 of the baseline.
    fn baseline(&self) -> TestResult<JobRequest> {
        Ok(JobRequest::baseline(
            self.base.id(),
            self.judge,
            0,
            JobBudget::from_budget(self.manifest.budget()),
        )?)
    }

    /// Trial 0 of the candidate whose writable file holds `value`, its cumulative diff stored as a blob.
    fn candidate(&self, value: &[u8]) -> TestResult<JobRequest> {
        let mut changed = self.base.tree().clone();
        changed.insert(RelPath::try_from(VALUE_PATH)?, value.to_vec());
        let diff = cumulative(
            &self.base,
            &changed,
            self.manifest.writable(),
            &Limits::standard(),
        )?;
        let diff_blob = self.store.put(diff.as_diff().as_bytes())?;
        let experiment = Experiment::candidate(diff);
        Ok(JobRequest::candidate(
            &experiment,
            diff_blob,
            self.judge,
            0,
            JobBudget::from_budget(self.manifest.budget()),
        )?)
    }

    /// Checks `request` without launching, then runs it on real VMs, printing the outcome and usage.
    fn trial(&self, request: &JobRequest, label: &str) -> TestResult<JobResult> {
        self.jobs.check(request)?;
        let result = self.jobs.run(request)?;
        println!(
            "== {label}: outcome {:?}; candidate usage {} vcpu_ms, {} mem_mib_ms; judge usage {:?}; backend {:?} on {:?}; logs {:?}",
            result.outcome(),
            result.usage().vcpu_ms(),
            result.usage().mem_mib_ms(),
            result
                .judge_usage()
                .map(|usage| (usage.vcpu_ms(), usage.mem_mib_ms())),
            result.backend().backend(),
            result.backend().arch(),
            result.logs().keys().collect::<Vec<_>>(),
        );
        Ok(result)
    }

    /// Checks what every result records: krun on arm64, host-measured usage of [`RUNNER_VCPUS`] and the manifest's
    /// memory for the candidate (and the judge when `judged`), and log tails that are in the store under their cap.
    fn check_provenance(&self, result: &JobResult, judged: bool) -> TestResult {
        assert_eq!(result.backend().backend(), BackendKind::Krun);
        assert_eq!(result.backend().arch(), Arch::Arm64);
        let memory_mib = u64::from(self.manifest.environment().memory_mib());
        let vcpus = u64::from(RUNNER_VCPUS);
        let usage = result.usage();
        assert!(usage.vcpu_ms() > 0, "the candidate's usage is measured");
        assert_eq!(usage.mem_mib_ms(), usage.vcpu_ms() / vcpus * memory_mib);
        assert_eq!(usage.total_tokens(), Some(0));
        match result.judge_usage() {
            Some(judge) => {
                assert!(judged, "a judge ran where none should have");
                assert!(judge.vcpu_ms() > 0, "the judge's usage is measured");
                assert_eq!(judge.mem_mib_ms(), judge.vcpu_ms() / vcpus * memory_mib);
            }
            None => assert!(!judged, "the judge's usage is missing"),
        }
        let cap = u64::try_from(MAX_STAGE_LOG_BYTES)?;
        for blob in result.logs().values() {
            let bytes = self.store.get(blob, cap)?;
            assert!(bytes.len() <= MAX_STAGE_LOG_BYTES);
        }
        Ok(())
    }
}

/// The value of metric `name` in a scored outcome.
fn metric(outcome: &Outcome, name: &str) -> TestResult<Metric> {
    outcome
        .scorecard()
        .ok_or("the outcome is not scored")?
        .value(&MetricName::from_str(name)?)
        .ok_or_else(|| format!("the scorecard has no {name}").into())
}

/// Checks that `outcome` is scored with `probe_ok` 1 and `probe_score` [`SCORE`].
fn assert_scored(outcome: &Outcome) -> TestResult {
    assert!(matches!(outcome, Outcome::Scored(_)), "{outcome:?}");
    assert_eq!(metric(outcome, "probe_ok")?, Metric::try_from(1.0)?);
    assert_eq!(
        metric(outcome, "probe_score")?,
        Metric::try_from(f64::from(SCORE))?
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see SETUP"]
fn baseline_scores_on_real_vms() -> TestResult {
    let project = project(&NOTHING, 1)?;
    let request = project.baseline()?;
    let result = project.trial(&request, "baseline")?;
    assert_eq!(result.exp(), request.exp());
    assert_eq!(result.judge(), project.judge);
    assert_scored(result.outcome())?;
    project.check_provenance(&result, true)?;
    assert!(!result.logs().is_empty(), "the stage logs are stored");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see SETUP"]
fn candidate_artifacts_reach_the_judge_on_real_vms() -> TestResult {
    let project = project(&FILL_OUT, 1)?;
    let request = project.candidate(CANDIDATE_VALUE)?;
    assert!(!request.is_baseline());
    let result = project.trial(&request, "candidate")?;
    assert_eq!(result.exp(), request.exp());
    // The judge VM unpacked the re-packed out/ input and scored; a malformed repack is an infrastructure error.
    assert_scored(result.outcome())?;
    project.check_provenance(&result, true)?;
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see SETUP"]
fn failed_gate_is_rejected_on_real_vms() -> TestResult {
    let project = project(&NOTHING, 0)?;
    let request = project.baseline()?;
    let result = project.trial(&request, "failed gate")?;
    let gate = MetricName::from_str("probe_ok")?;
    assert_eq!(
        result.outcome(),
        &Outcome::Rejected(Reason::GateFailed { gate })
    );
    assert!(result.outcome().scorecard().is_none());
    project.check_provenance(&result, true)?;
    Ok(())
}

/// The fixture manifests parse for every run argv and gate the VM tests use, so a manifest mistake fails the gate
/// rather than a VM run.
#[test]
fn fixture_manifests_parse() -> TestResult {
    for run in [&NOTHING[..], &FILL_OUT[..]] {
        for gate in [0, 1] {
            let manifest = Manifest::parse_toml(&manifest_text(run, gate)?)?;
            assert_eq!(manifest.judge().label(), 1);
        }
    }
    assert!(toml_array(&["a b".to_owned()]).is_err());
    Ok(())
}
