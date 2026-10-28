//! The shipped sample project, `examples/logparse`, end to end on real VMs, and the three measurements Phase 8 owns
//! (plan 08-13; DOCS-02, JUDGE-02; 08-CONTEXT D-02, D-04, D-07).
//!
//! - `sample_end_to_end_on_real_vms` (DOCS-02, JUDGE-02): on a copy of the sample, `env build` builds the root image
//!   with Docker buildx and the production packer, `judge push judge` records the judge data, `check` runs the
//!   baseline's `[judge] trials` trials (each a candidate VM then a judge VM, with the trial index on every
//!   `Judged`), a second `check` reuses that calibration without a VM, `run --generations 2` drives a new run with a
//!   scripted `single_shot` provider that proposes the improved parser in generation 1, `report` builds the winner's
//!   report, and `adopt` writes its patch with the report's hash. `/usr/bin/patch -p1` applies that patch to a fresh
//!   copy of the sample's `src/parse.rs` and yields the improved parser byte for byte, and every `Judged` of the
//!   check run and of the run records krun on arm64.
//! - `sample_measurements_on_real_vms` (D-02, D-04, D-07): the baseline at candidate allowances of 384, 448, 512,
//!   640, and 768 MiB (`memory_mib = 512 + allowance`, `scratch_mib = 256`, so the 256 MiB guest reserve and the
//!   scratch leave exactly the allowance as the candidate's `RLIMIT_AS`), one row each; then five boots of the
//!   baseline at the sample's own configuration, one row each with the objective values and the host-measured
//!   candidate and judge VM times. Only the five default boots are asserted, each `Scored`; the allowance rows are
//!   data.
//!
//! Both compose exactly what `minos check` and `minos run` do: [`prepare`] and then [`krun_jobs`], the production
//! `Runner<KrunBackend, FileStore>` behind the loop's job seam. Only the provider is scripted ([`FakeProvider`]), so
//! no test calls a model.
//!
//! Every test is ignored. Each needs a signed `MINOS_VMM`, a static musl `MINOS_GUEST`, `MINOS_KRUNFW`, and a
//! running Docker daemon with buildx; only this file (and the other ignored VM tests) reads the three variables
//! (05-CONTEXT D-04). A missing one is an error that carries [`SETUP`], never a skip. Build, sign, and run:
//!
//! ```text
//! cargo build -p minos-vmm
//! codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
//! cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
//! MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The tests print only command output the commands already rendered through `SafeText`, stage log tails through
//! [`LogExcerpt`], and numbers; never a raw guest or model byte.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

use minos_backend_krun::KrunBackend;
use minos_backend_krun::host::HostPaths;
use minos_backend_krun::jobdir::JOB_ROOT_PARENT;
use minos_cli::address::{HexPrefix, fold_run};
use minos_cli::adopt::{AdoptOptions, adopt};
use minos_cli::blob_source::FileBlobSource;
use minos_cli::builder::DockerBuildx;
use minos_cli::check::{CheckOptions, ProviderStatus, check};
use minos_cli::env_build::{EnvBuildOptions, MANIFEST_FILE, env_build};
use minos_cli::jobs::{RunnerJobs, krun_jobs};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::project::{Prepared, prepare};
use minos_cli::render::outcome_word;
use minos_cli::report::report;
use minos_cli::run::{RunEnded, RunOptions, run};
use minos_core::id::{MetricName, ModelId, RunId};
use minos_core::job::{JobBudget, JobRequest, JobResult};
use minos_core::ledger::{BackendKind, Decision, DecisionKind};
use minos_core::limits::MAX_STAGE_LOG_BYTES;
use minos_core::manifest::Arch;
use minos_core::outcome::{Outcome, Stage};
use minos_core::text::{LogExcerpt, SafeText};
use minos_core::usage::Usage;
use minos_loop::{DriverOptions, JobRunner};
use minos_propose::{FakeProvider, FakeReply, Proposer};
use minos_runner::RUNNER_VCPUS;
use minos_source::CleanPolicy;
use minos_store::Blobs;
use serde_json::json;
use tempfile::TempDir;

/// The result type of every helper and test.
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to build, sign, start, and set before these tests run (CLAUDE.md Commands).
const SETUP: &str = "\
build and sign first (re-sign after any cargo command that relinks minos-vmm):
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
start Docker, and check that `docker buildx version` exits 0 (env build runs docker buildx; the base image pull needs
network, the VMs never do), then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
and run:
  cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1";

/// The improved parser: a complete replacement for the sample's `src/parse.rs` (08-04).
const IMPROVED_PARSE: &str = include_str!("fixtures/logparse_improved_parse.rs");

/// The generation-2 proposal: the improved parser with one comment line added, so it scores the same and is kept.
const SECOND_LOOK: &str = "// A second look in generation 2 found nothing more to change.\n";

/// The one writable file of the sample.
const PARSE_PATH: &str = "src/parse.rs";

/// The model the sample's proposer names.
const ALIAS: &str = "haiku";

/// The resolved model ID the scripted provider reports, as the `claude` CLI reports one in `modelUsage`.
const RESOLVED: &str = "claude-haiku-5-5";

/// A `prices.toml` that prices compute, the alias, and the resolved ID (CLAUDE.md Cost).
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"

[models."claude-haiku-5-5"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

/// The start time of the first `check`, in Unix seconds; later commands use later seconds.
const STARTED_AT: u64 = 1_761_600_000;

/// The candidate allowances the `RLIMIT_AS` bisection steps through, in MiB (08-RESEARCH Pitfall 5).
const ALLOWANCES_MIB: [u16; 5] = [384, 448, 512, 640, 768];

/// The scratch cap of every bisection row, in MiB.
const BISECT_SCRATCH_MIB: u16 = 256;

/// The guest reserve plus [`BISECT_SCRATCH_MIB`]: `memory_mib` less this is the candidate allowance.
const BISECT_FIXED_MIB: u16 = 512;

/// How many baseline boots the variance measurement records at the sample's own configuration (D-07).
const DEFAULT_BOOTS: u32 = 5;

/// Most bytes of a stage log tail a non-scored row prints.
const LOG_TAIL_BYTES: usize = 2048;

/// The system patch.
const PATCH: &str = "/usr/bin/patch";

/// The line of the sample manifest that the bisection appends its `[environment]` values after.
const REQUIRES_LINE: &str = "requires = { arch = \"arm64\" }\n";

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

/// The sample's directory in this repository.
fn sample_dir() -> TestResult<PathBuf> {
    Ok(fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/logparse"),
    )?)
}

/// Copies the directory tree `from` into `to`: regular files and directories only, skipping every directory named
/// `target` or `.minos` and a top-level `minos.lock` (each records this machine's state, and the copy starts clean).
/// A symlink or any other kind of entry is an error, never followed or skipped.
fn copy_tree(from: &Path, to: &Path) -> TestResult {
    fs::create_dir_all(to)?;
    let mut entries = fs::read_dir(from)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let source = entry.path();
        let kind = fs::symlink_metadata(&source)?.file_type();
        if kind.is_dir() {
            if name == OsStr::new("target") || name == OsStr::new(".minos") {
                continue;
            }
            copy_tree(&source, &to.join(&name))?;
        } else if kind.is_file() {
            if name == OsStr::new("minos.lock") {
                continue;
            }
            fs::copy(&source, to.join(&name))?;
        } else {
            return Err(format!("{} is not a regular file or directory", source.display()).into());
        }
    }
    Ok(())
}

/// Prints `bytes`, the output a command wrote through `SafeText`, under the heading `label`, rendered through
/// `SafeText` once more so nothing but printable text and newlines reaches the terminal.
fn print_output(label: &str, bytes: &[u8]) {
    println!("== {label}");
    print!("{}", SafeText::from_utf8_lossy(bytes));
}

/// The sample, copied, built, and pushed, with the production runner composed over it.
///
/// Fields drop in order: the job runner (and its backend) first, the directory with the project and its store last.
struct Sample {
    jobs: RunnerJobs<KrunBackend<FileBlobSource>>,
    prepared: Prepared,
    paths: (PathBuf, PathBuf, PathBuf),
    prices: PathBuf,
    project: PathBuf,
    manifest_text: String,
    root: PathBuf,
    _dir: TempDir,
}

impl Sample {
    /// Copies the sample into a fresh directory under [`JOB_ROOT_PARENT`], writes `prices.toml` beside it, runs
    /// `env build` (Docker buildx and the production packer) and `judge push judge`, then prepares the project and
    /// composes [`krun_jobs`], as `minos check` and `minos run` do.
    ///
    /// # Errors
    ///
    /// When a variable is missing, Docker or buildx cannot build, the push or the preparation refuses, or the host
    /// refuses a launch check; host errors carry [`SETUP`].
    fn new() -> TestResult<Self> {
        let vmm = env_path("MINOS_VMM")?;
        let guest = env_path("MINOS_GUEST")?;
        let krunfw = env_path("MINOS_KRUNFW")?;

        let dir = tempfile::Builder::new()
            .prefix("msv")
            .tempdir_in(JOB_ROOT_PARENT)?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("logparse");
        copy_tree(&sample_dir()?, &project)?;
        let prices = root.join("prices.toml");
        fs::write(&prices, PRICES)?;
        let manifest_text = fs::read_to_string(project.join(MANIFEST_FILE))?;

        let docker = DockerBuildx::find().map_err(|error| format!("{error}; {SETUP}"))?;
        let built = env_build(&project, &guest, &docker, EnvBuildOptions::new(false, true))
            .map_err(|error| format!("env build failed: {error}; {SETUP}"))?;
        println!(
            "== env build: root image {}, lock written {}, {} files, {} directories, {} symlinks in the image, {} base images",
            built.image().to_hex(),
            built.lock_written(),
            built.pack().files(),
            built.pack().dirs(),
            built.pack().symlinks(),
            built.base_images().len(),
        );
        let pushed = judge_push(
            &project,
            &project.join("judge"),
            JudgePushOptions::new(false),
        )?;
        println!(
            "== judge push judge: label {}, tree {}, {} files, {} bytes, judge version {}",
            pushed.label(),
            pushed.tree().to_hex(),
            pushed.file_count(),
            pushed.bytes(),
            pushed.judge_version().to_hex(),
        );

        let paths = (vmm, guest, krunfw);
        let (prepared, jobs) = compose(&project, &paths, &prices)?;
        Ok(Self {
            jobs,
            prepared,
            paths,
            prices,
            project,
            manifest_text,
            root,
            _dir: dir,
        })
    }

    /// Rewrites the copy's manifest to `text`, then prepares it and composes the job runner again.
    fn reconfigure(&mut self, text: &str) -> TestResult {
        fs::write(self.project.join(MANIFEST_FILE), text)?;
        let (prepared, jobs) = compose(&self.project, &self.paths, &self.prices)?;
        self.jobs = jobs;
        self.prepared = prepared;
        Ok(())
    }

    /// Trial `trial` of the baseline under the prepared manifest's budget.
    fn baseline(&self, trial: u32) -> TestResult<JobRequest> {
        Ok(JobRequest::baseline(
            self.prepared.base().id(),
            self.prepared.judge(),
            trial,
            JobBudget::from_budget(self.prepared.manifest().budget()),
        )?)
    }

    /// Checks `request` without launching, then runs it on real VMs.
    fn trial(&self, request: &JobRequest) -> TestResult<JobResult> {
        self.jobs.check(request)?;
        Ok(self.jobs.run(request)?)
    }

    /// The tail of `result`'s `stage` log, through [`LogExcerpt`], or `None` when it recorded none.
    fn log_tail(&self, result: &JobResult, stage: Stage) -> TestResult<Option<LogExcerpt>> {
        let Some(blob) = result.logs().get(&stage) else {
            return Ok(None);
        };
        let bytes = self
            .prepared
            .store()
            .get(blob, u64::try_from(MAX_STAGE_LOG_BYTES)?)?;
        Ok(Some(LogExcerpt::from_bytes(&bytes, LOG_TAIL_BYTES)))
    }

    /// Prints every stage log tail `result` recorded, for a row that did not score.
    fn print_logs(&self, result: &JobResult) -> TestResult {
        for stage in [Stage::Build, Stage::Run, Stage::Judge] {
            if let Some(excerpt) = self.log_tail(result, stage)? {
                println!(
                    "-- {stage} log tail ({} bytes shown, head dropped: {}):",
                    excerpt.text().len(),
                    excerpt.is_truncated()
                );
                println!("{}", excerpt.text());
            }
        }
        Ok(())
    }
}

/// Prepares the project at `project` and composes the production job runner at the host `paths`.
fn compose(
    project: &Path,
    paths: &(PathBuf, PathBuf, PathBuf),
    prices: &Path,
) -> TestResult<(Prepared, RunnerJobs<KrunBackend<FileBlobSource>>)> {
    let (vmm, guest, krunfw) = paths;
    let prepared = prepare(project, guest, prices, CleanPolicy::AllowDirty)?;
    let jobs = krun_jobs(&prepared, HostPaths::new(vmm, guest, krunfw))
        .map_err(|error| format!("{error}; {SETUP}"))?;
    Ok((prepared, jobs))
}

/// The host-measured wall time of a VM, in milliseconds: its vCPU milliseconds over [`RUNNER_VCPUS`].
fn vm_ms(usage: Usage) -> u64 {
    usage.vcpu_ms() / u64::from(RUNNER_VCPUS)
}

/// The judge VM's wall time in milliseconds, or `n/a` when no judge ran.
fn judge_ms_text(result: &JobResult) -> String {
    result
        .judge_usage()
        .map_or_else(|| "n/a".to_owned(), |usage| vm_ms(usage).to_string())
}

/// The value of metric `name` in a scored outcome, or `n/a`.
fn metric_text(outcome: &Outcome, name: &str) -> TestResult<String> {
    let name = MetricName::from_str(name)?;
    Ok(outcome
        .scorecard()
        .and_then(|card| card.value(&name))
        .map_or_else(|| "n/a".to_owned(), |value| value.value().to_string()))
}

/// The sample manifest with `[environment]` set to `memory_mib` and `scratch_mib`.
fn with_environment(text: &str, memory_mib: u16, scratch_mib: u16) -> TestResult<String> {
    if text.matches(REQUIRES_LINE).count() != 1 {
        return Err("the sample manifest's [environment] requires line is not unique".into());
    }
    Ok(text.replacen(
        REQUIRES_LINE,
        &format!("{REQUIRES_LINE}memory_mib = {memory_mib}\nscratch_mib = {scratch_mib}\n"),
        1,
    ))
}

/// One scripted `single_shot` reply proposing `content` as the whole of `src/parse.rs`, with token usage reported
/// under the resolved model ID.
fn reply(rationale: &str, content: &str) -> TestResult<FakeReply> {
    let usage = Usage::tokens(1_000, 2_000, 0, 0);
    Ok(FakeReply::Structured {
        output: json!({
            "proposals": [{
                "rationale": rationale,
                "files": [{"path": PARSE_PATH, "content": content}],
            }],
        }),
        usage,
        models: BTreeMap::from([(ModelId::from_str(RESOLVED)?, usage)]),
    })
}

/// The first 12 hex characters of `hex`, as a prefix.
fn prefix_of(hex: &str) -> TestResult<HexPrefix> {
    let text: String = hex.chars().take(12).collect();
    Ok(HexPrefix::from_str(&text)?)
}

/// Fails unless every recorded trial of `run` records krun on arm64.
fn assert_krun_arm64(sample: &Sample, run: &RunId) -> TestResult {
    let state = fold_run(sample.prepared.store(), run)?;
    assert_eq!(
        state.backend_classes(),
        BTreeSet::from([(BackendKind::Krun, Arch::Arm64)]),
        "every Judged of run {} records krun on arm64",
        run.to_hex()
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and Docker buildx; see SETUP"]
fn sample_end_to_end_on_real_vms() -> TestResult {
    let sample = Sample::new()?;
    let prepared = &sample.prepared;
    let quick = DriverOptions {
        retry_pause: Duration::ZERO,
    };
    let ready = ProviderStatus::Ready {
        description: SafeText::new("scripted test provider"),
    };
    let no_model = Proposer::<FakeProvider>::without_provider();

    // check: the baseline's trials, each a candidate VM then a judge VM (JUDGE-02).
    let mut out = Vec::new();
    let checked = check(
        prepared,
        &no_model,
        &sample.jobs,
        &ready,
        CheckOptions::new(false, STARTED_AT).with_driver(quick),
        &mut out,
    )?;
    print_output("minos check", &out);
    assert!(!checked.refused(), "check refused");
    assert!(!checked.reused(), "the first check launched its own trials");
    let text = String::from_utf8(out)?;
    for needle in [
        "trial 0:",
        "trial 1:",
        "trial 2:",
        "noise floor:",
        "noise verdict:",
        "approx. cost:",
    ] {
        assert!(text.contains(needle), "check printed no {needle:?}");
    }
    let check_run = *checked.run().ok_or("check named no run")?;
    let state = fold_run(prepared.store(), &check_run)?;
    let judge = prepared.judge();
    let trials: Vec<u32> = state
        .trials(state.baseline(), &judge)
        .map(|(trial, _)| trial)
        .collect();
    assert_eq!(trials, vec![0, 1, 2], "the baseline's Judged trial indexes");
    assert_krun_arm64(&sample, &check_run)?;

    // A second check reuses that calibration and launches nothing (08-CONTEXT D-05).
    let mut out = Vec::new();
    let again = check(
        prepared,
        &no_model,
        &sample.jobs,
        &ready,
        CheckOptions::new(false, STARTED_AT + 1).with_driver(quick),
        &mut out,
    )?;
    print_output("minos check (again)", &out);
    assert!(again.reused(), "the second check reused the first run");
    assert_eq!(again.run(), Some(&check_run));
    assert!(String::from_utf8(out)?.contains("no VM launched"));

    // run --generations 2: generation 1 proposes the improved parser, generation 2 adds one comment line.
    let proposer = Proposer::with_provider(FakeProvider::new(vec![
        reply(
            "Parse all three formats and normalize the level.",
            IMPROVED_PARSE,
        )?,
        reply(
            "Keep the parser; note the second look.",
            &format!("{IMPROVED_PARSE}{SECOND_LOOK}"),
        )?,
    ]));
    let mut out = Vec::new();
    let ran = run(
        prepared,
        &proposer,
        &sample.jobs,
        RunOptions::new(2, None),
        STARTED_AT + 2,
        quick,
        &mut out,
    )?;
    print_output("minos run --generations 2", &out);
    assert_eq!(ran.ended(), &RunEnded::Reached { generation: 2 });
    let run_id = *ran.run();
    let state = fold_run(prepared.store(), &run_id)?;
    let winner = match state.decision(1).map(Decision::kind) {
        Some(DecisionKind::Improved { winner }) => *winner,
        other => return Err(format!("generation 1 did not improve: {other:?}").into()),
    };
    assert_krun_arm64(&sample, &run_id)?;
    let requests = proposer
        .provider()
        .ok_or("the proposer has no provider")?
        .requests()?;
    assert_eq!(requests.len(), 2, "one model call per generation");
    let alias = ModelId::from_str(ALIAS)?;
    assert!(requests.iter().all(|request| request.model() == &alias));

    // report, then adopt with the report's hash.
    let run_prefix = prefix_of(&run_id.to_hex())?;
    let exp_prefix = prefix_of(&winner.to_hex())?;
    let mut out = Vec::new();
    let built = report(prepared.store(), Some(&run_prefix), &exp_prefix, &mut out)?;
    print_output("minos report", &out);
    let mut out = Vec::new();
    let adopted = adopt(
        prepared.store(),
        &sample.project,
        Some(&run_prefix),
        &exp_prefix,
        AdoptOptions::new(*built.report().hash(), None),
        &mut out,
    )?;
    print_output("minos adopt", &out);
    assert!(adopted.newly_acknowledged());

    // patch -p1 on a fresh copy of the sample's parser yields the improved parser byte for byte.
    let applied = sample.root.join("applied");
    fs::create_dir_all(applied.join("src"))?;
    fs::copy(sample_dir()?.join(PARSE_PATH), applied.join(PARSE_PATH))?;
    let home = sample.root.join("home");
    fs::create_dir_all(&home)?;
    let patched = Command::new(PATCH)
        .arg("-p1")
        .arg("-i")
        .arg(adopted.path())
        .current_dir(&applied)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("HOME", &home)
        .stdin(Stdio::null())
        .output()?;
    print_output("patch -p1 stdout", &patched.stdout);
    print_output("patch -p1 stderr", &patched.stderr);
    assert!(
        patched.status.success(),
        "patch -p1 failed: {}",
        patched.status
    );
    assert_eq!(
        fs::read(applied.join(PARSE_PATH))?,
        IMPROVED_PARSE.as_bytes(),
        "the patched parser is the improved parser"
    );
    for leftover in ["src/parse.rs.orig", "src/parse.rs.rej"] {
        assert!(!applied.join(leftover).exists(), "{leftover} was left");
    }
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and Docker buildx; see SETUP"]
fn sample_measurements_on_real_vms() -> TestResult {
    let mut sample = Sample::new()?;
    let original = sample.manifest_text.clone();

    // D-02: the baseline's release build at each candidate allowance. Data, not assertions.
    println!("== RLIMIT_AS bisection (D-02)");
    for allowance in ALLOWANCES_MIB {
        let memory_mib = BISECT_FIXED_MIB + allowance;
        sample.reconfigure(&with_environment(
            &original,
            memory_mib,
            BISECT_SCRATCH_MIB,
        )?)?;
        let environment = sample.prepared.manifest().environment();
        assert_eq!(environment.candidate_allowance_mib(), allowance);
        let request = sample.baseline(0)?;
        let row = format!(
            "allowance_mib={allowance} memory_mib={memory_mib} scratch_mib={BISECT_SCRATCH_MIB}"
        );
        sample.jobs.check(&request)?;
        match sample.jobs.run(&request) {
            Ok(result) => {
                println!(
                    "{row} outcome={} candidate_ms={} judge_ms={} accuracy={}",
                    outcome_word(result.outcome()),
                    vm_ms(result.usage()),
                    judge_ms_text(&result),
                    metric_text(result.outcome(), "accuracy")?,
                );
                if !matches!(result.outcome(), Outcome::Scored(_)) {
                    sample.print_logs(&result)?;
                }
            }
            Err(failure) => println!("{row} outcome=error ({failure})"),
        }
    }

    // D-04 and D-07: five boots of the baseline at the sample's own configuration.
    sample.reconfigure(&original)?;
    let environment = sample.prepared.manifest().environment();
    let budget = JobBudget::from_budget(sample.prepared.manifest().budget());
    println!(
        "== default configuration: memory_mib={} scratch_mib={} allowance_mib={}; budget build {} ms, run {} ms, judge {} ms",
        environment.memory_mib(),
        environment.scratch_mib(),
        environment.candidate_allowance_mib(),
        budget.build_ms(),
        budget.run_ms(),
        budget.judge_ms(),
    );
    for boot in 0..DEFAULT_BOOTS {
        let result = sample.trial(&sample.baseline(boot)?)?;
        println!(
            "boot={boot} outcome={} accuracy={} accuracy_held_out={} candidate_ms={} judge_ms={}",
            outcome_word(result.outcome()),
            metric_text(result.outcome(), "accuracy")?,
            metric_text(result.outcome(), "accuracy_held_out")?,
            vm_ms(result.usage()),
            judge_ms_text(&result),
        );
        if !matches!(result.outcome(), Outcome::Scored(_)) {
            sample.print_logs(&result)?;
        }
        assert!(
            matches!(result.outcome(), Outcome::Scored(_)),
            "boot {boot} at the default configuration scored"
        );
        assert_eq!(result.backend().backend(), BackendKind::Krun);
        assert_eq!(result.backend().arch(), Arch::Arm64);
    }
    Ok(())
}
