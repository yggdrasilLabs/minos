//! CLI-09's golden: the patch file `minos_cli::adopt::adopt` writes, applied by the user's own `/usr/bin/git apply`
//! and `/usr/bin/patch -p1`, yields every proposed file byte for byte (08-CONTEXT D-12; 08-RESEARCH Pitfall 1, E5).
//!
//! The fixture is a `single_shot` project whose writable set is seven text files, one per newline case: a mid-file
//! edit, a changed last line that lacks its newline, a final newline added, a final newline dropped, a line inserted
//! at the head, a line appended at the tail, and a line deleted. A scripted `FakeProvider` proposes all seven new
//! contents in one call, a fake job runner scores the candidate above the baseline, and the run is driven to
//! generation 1 on `FileStore`; the winner's report is built and `adopt` writes its patch with the report's hash.
//!
//! - Outside any repository, `patch -p1 -i <file>` and `git apply <file>` each yield the seven proposed files.
//! - At the root of a fresh `git init` repository, both tools do the same.
//! - In `<repo>/sub/` of a fresh repository, plain `git apply <file>` exits 0 and leaves every file unchanged (the
//!   silent no-op the printed commands avoid), while `git apply --directory=sub/ <file>` and `patch -p1 -i <file>`
//!   each yield the proposed files.
//!
//! Every check compares bytes, never an exit code alone, because `git apply` from a subdirectory succeeds while
//! doing nothing, and every setting checks that no `.orig` or `.rej` file was left. The host tools run as argv
//! arrays with a cleared environment in a fresh temporary directory; `GIT_CEILING_DIRECTORIES` stops git's
//! repository search at the temporary root, so nothing touches the repository this test lives in.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::time::Duration;

use minos_cli::address::HexPrefix;
use minos_cli::adopt::{AdoptOptions, adopt};
use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{Prepared, prepare};
use minos_cli::report::build;
use minos_cli::run::{RunOptions, run};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, MetricName, ModelId, RunId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Isolation,
};
use minos_core::lock::{BuilderId, BuilderText, LockedImage};
use minos_core::manifest::Arch;
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::Outcome;
use minos_core::state::fold;
use minos_core::usage::Usage;
use minos_loop::{DriverOptions, JobFailure, JobRunner};
use minos_propose::{FakeProvider, FakeReply, Proposer};
use minos_source::CleanPolicy;
use minos_store::{Blobs, FileStore, Ledger, STORE_DIR};
use serde_json::json;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The run's start time, in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// The system git.
const GIT: &str = "/usr/bin/git";

/// The system patch.
const PATCH: &str = "/usr/bin/patch";

/// One newline case: the path, the base bytes, and the proposed bytes.
type Case = (&'static str, &'static str, &'static str);

/// The seven newline cases.
const CASES: [Case; 7] = [
    (
        "src/mid.txt",
        "one\ntwo\nthree\nfour\nfive\n",
        "one\ntwo\nTHREE\nfour\nfive\n",
    ),
    (
        "src/last_no_newline.txt",
        "one\ntwo\nthree",
        "one\ntwo\nTHREE",
    ),
    (
        "src/add_newline.txt",
        "one\ntwo\nthree",
        "one\ntwo\nthree\n",
    ),
    (
        "src/drop_newline.txt",
        "one\ntwo\nthree\n",
        "one\ntwo\nthree",
    ),
    (
        "src/head.txt",
        "one\ntwo\nthree\n",
        "zero\none\ntwo\nthree\n",
    ),
    (
        "src/tail.txt",
        "one\ntwo\nthree\n",
        "one\ntwo\nthree\nfour\n",
    ),
    (
        "src/delete.txt",
        "one\ntwo\nthree\nfour\n",
        "one\nthree\nfour\n",
    ),
];

/// The readable file every setting also copies, which the patch never touches.
const READABLE: (&str, &str) = ("src/lib.rs", "pub fn f() {}\n");

/// The manifest: the seven case files writable, a bare-word judge argv, and one `single_shot` proposer.
const MANIFEST: &str = r#"[project]
name = "patching"
kind = "optimize"
program = "program.md"

[surface]
writable = [
    "src/mid.txt",
    "src/last_no_newline.txt",
    "src/add_newline.txt",
    "src/drop_newline.txt",
    "src/head.txt",
    "src/tail.txt",
    "src/delete.txt",
]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["score"]

[[judge.gate]]
metric = "identical_output"

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
"#;

/// The Dockerfile the lock image entry is written for.
const DOCKERFILE: &[u8] = b"FROM scratch\n";

/// The fake guest binary the lock image entry is written for.
const GUEST: &[u8] = b"a fake minos-guest\n";

/// A `prices.toml` that prices compute and the proposer's model.
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

/// The patch `adopt` wrote for the winner of a run driven to generation 1, with the temporary directory that holds
/// the project and every setting's work.
struct Adopted {
    dir: TempDir,
    patch: PathBuf,
}

impl Adopted {
    /// Writes the project, drives it to generation 1, builds the winner's report, and adopts it with its hash.
    fn new() -> Fallible<Self> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        write(&project, MANIFEST_FILE, MANIFEST.as_bytes())?;
        write(&project, "program.md", b"Change the files.\n")?;
        write(&project, "minos.Dockerfile", DOCKERFILE)?;
        write(&project, READABLE.0, READABLE.1.as_bytes())?;
        for (path, base, _) in CASES {
            write(&project, path, base.as_bytes())?;
        }
        let data = root.join("judge-data");
        write(&data, "expected.txt", b"patch test judge data\n")?;
        let guest = root.join("minos-guest");
        fs::write(&guest, GUEST)?;
        let prices = root.join("prices.toml");
        fs::write(&prices, PRICES)?;
        judge_push(&project, &data, JudgePushOptions::new(false))?;
        lock_image(&project)?;

        let prepared = prepare(&project, &guest, &prices, CleanPolicy::AllowDirty)?;
        let (run, winner) = drive(&prepared)?;
        let run_prefix = prefix_of(&run.to_hex())?;
        let exp_prefix = prefix_of(&winner.to_hex())?;
        let built = build(prepared.store(), Some(&run_prefix), &exp_prefix)?;
        let mut out = Vec::new();
        let outcome = adopt(
            prepared.store(),
            &project,
            Some(&run_prefix),
            &exp_prefix,
            AdoptOptions::new(*built.report().hash(), None),
            &mut out,
        )?;
        let patch = outcome.path().to_path_buf();
        let bytes = fs::read(&patch)?;
        if bytes != built.diff().as_diff().as_bytes() {
            return Err("the patch file is not the canonical cumulative diff".into());
        }
        let text = String::from_utf8(bytes)?;
        for (path, _, _) in CASES {
            if !text.contains(&format!("diff --git a/{path} b/{path}\n")) {
                return Err(format!("the patch has no section for {path}").into());
            }
        }
        if !text.contains("\\ No newline at end of file\n") {
            return Err("the patch marks no missing final newline".into());
        }
        Ok(Self { dir, patch })
    }

    /// A new, empty directory `name` under the temporary root.
    fn fresh(&self, name: &str) -> Fallible<PathBuf> {
        let path = fs::canonicalize(self.dir.path())?.join(name);
        fs::create_dir(&path)?;
        Ok(path)
    }

    /// The directory git's repository search stops at: the temporary root's parent.
    fn ceiling(&self) -> Fallible<PathBuf> {
        let root = fs::canonicalize(self.dir.path())?;
        Ok(root
            .parent()
            .ok_or("the temporary root has no parent")?
            .to_path_buf())
    }

    /// Runs `program` with `args` in `dir`, with a cleared environment, and returns its exit status.
    fn tool(&self, program: &str, args: &[&str], dir: &Path) -> Fallible<ExitStatus> {
        let home = self.fresh_home()?;
        Ok(Command::new(program)
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .env("HOME", home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CEILING_DIRECTORIES", self.ceiling()?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?)
    }

    /// The `HOME` the tools see: an empty directory under the temporary root, created once.
    fn fresh_home(&self) -> Fallible<PathBuf> {
        let home = fs::canonicalize(self.dir.path())?.join("home");
        fs::create_dir_all(&home)?;
        Ok(home)
    }

    /// Runs `program` with `args` in `dir` and fails unless it exits 0.
    fn succeed(&self, program: &str, args: &[&str], dir: &Path) -> TestResult {
        let status = self.tool(program, args, dir)?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{program} {args:?} in {} failed: {status}", dir.display()).into())
        }
    }

    /// The patch path as an argument.
    fn patch_arg(&self) -> Fallible<&str> {
        Ok(self.patch.to_str().ok_or("the patch path is not UTF-8")?)
    }

    /// A fresh repository at `name` under the temporary root, created with `git init -q`.
    fn repository(&self, name: &str) -> Fallible<PathBuf> {
        let repo = self.fresh(name)?;
        self.succeed(GIT, &["init", "-q"], &repo)?;
        Ok(repo)
    }
}

/// Stores a root image and writes the lock's image entry for the project at `project`.
fn lock_image(project: &Path) -> TestResult {
    let store = FileStore::open(&project.join(STORE_DIR))?;
    let root_image = ImageDigest::from_blob_id(&store.put(b"a root image")?);
    let lock = read_lock(&project.join(LOCK_FILE))?.ok_or("judge push wrote no lock")?;
    let lock = lock.with_image(LockedImage::new(
        BackendKind::Krun,
        Arch::Arm64,
        root_image,
        BlobId::of(DOCKERFILE),
        Vec::new(),
        BlobId::of(GUEST),
        BuilderId::new(
            BuilderText::try_from("test-builder".to_owned())?,
            BuilderText::try_from("1".to_owned())?,
        ),
    ));
    write_lock(
        &project.join(STORE_DIR),
        &project.join(LOCK_FILE),
        lock.to_toml()?.as_bytes(),
    )?;
    Ok(())
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

/// The first 12 hex characters of `hex`, as a prefix.
fn prefix_of(hex: &str) -> Fallible<HexPrefix> {
    let text: String = hex.chars().take(12).collect();
    Ok(HexPrefix::from_str(&text)?)
}

// ---------------------------------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------------------------------

/// A [`JobRunner`] that scores the baseline 1.0 and every candidate 2.0; its preflight always passes.
struct FakeJobs;

impl JobRunner for FakeJobs {
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        let invalid = JobFailure::Config {
            what: "the fake result is invalid",
        };
        let value = if request.is_baseline() { 1.0 } else { 2.0 };
        let name = |text: &str| MetricName::from_str(text).map_err(|_| invalid);
        let metric = |number: f64| Metric::new(number).map_err(|_| invalid);
        let card = Scorecard::from_entries([
            (name("identical_output")?, metric(1.0)?, false),
            (name("probe_score")?, metric(value)?, false),
        ])
        .map_err(|_| invalid)?;
        JobResult::new(
            request,
            Outcome::Scored(card),
            Usage::compute(1_000, 2_048_000),
            Some(Usage::compute(500, 1_024_000)),
            BackendInfo::new(
                BackendKind::Krun,
                Arch::Arm64,
                Isolation::Hardware,
                EXPECTED_KRUN_LAYERS.iter().copied().collect(),
                ImageDigest::of(b"root image"),
                Some(ImageDigest::of(b"kernel")),
            ),
            BTreeMap::new(),
        )
        .map_err(|_| invalid)
    }

    fn check(&self, _request: &JobRequest) -> Result<(), JobFailure> {
        Ok(())
    }
}

/// Drives a new run of `prepared` to generation 1 with one scripted proposal of all seven new contents, and returns
/// the run and its generation-1 winner.
fn drive(prepared: &Prepared) -> Fallible<(RunId, ExperimentId)> {
    let files: Vec<serde_json::Value> = CASES
        .iter()
        .map(|(path, _, proposed)| json!({"path": path, "content": proposed}))
        .collect();
    let reply = FakeReply::Structured {
        output: json!({
            "proposals": [{
                "rationale": "Change each file in its own way.",
                "files": files,
            }],
        }),
        usage: Usage::tokens(100, 200, 0, 0),
        models: BTreeMap::from([(ModelId::from_str("haiku")?, Usage::tokens(100, 200, 0, 0))]),
    };
    let proposer = Proposer::with_provider(FakeProvider::new(vec![reply]));
    let mut out = Vec::new();
    let outcome = run(
        prepared,
        &proposer,
        &FakeJobs,
        RunOptions::new(1, None),
        STARTED_AT,
        DriverOptions {
            retry_pause: Duration::ZERO,
        },
        &mut out,
    )?;
    let run = *outcome.run();
    let state = fold(prepared.store().read(&run)?.records())?;
    match state.decision(1).map(Decision::kind) {
        Some(DecisionKind::Improved { winner }) => Ok((run, *winner)),
        _ => Err("generation 1 did not improve".into()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Checking a tree
// ---------------------------------------------------------------------------------------------------------------

/// Copies the base files into `dir`.
fn copy_base(dir: &Path) -> TestResult {
    write(dir, READABLE.0, READABLE.1.as_bytes())?;
    for (path, base, _) in CASES {
        write(dir, path, base.as_bytes())?;
    }
    Ok(())
}

/// Fails unless every case file in `dir` holds `pick`'s bytes, the readable file is untouched, and no `.orig` or
/// `.rej` file sits under `dir`.
fn expect_tree(dir: &Path, pick: fn(&Case) -> &'static str) -> TestResult {
    for case in &CASES {
        let found = fs::read(dir.join(case.0))?;
        let want = pick(case).as_bytes();
        if found != want {
            return Err(format!(
                "{} in {}: found {:?}, wanted {:?}",
                case.0,
                dir.display(),
                String::from_utf8_lossy(&found),
                String::from_utf8_lossy(want)
            )
            .into());
        }
    }
    if fs::read(dir.join(READABLE.0))? != READABLE.1.as_bytes() {
        return Err(format!("{} changed in {}", READABLE.0, dir.display()).into());
    }
    no_leftovers(dir)
}

/// The proposed bytes of a case.
const fn proposed(case: &Case) -> &'static str {
    case.2
}

/// The base bytes of a case.
const fn based(case: &Case) -> &'static str {
    case.1
}

/// Fails if any `.orig` or `.rej` file sits under `dir`, outside `.git`.
fn no_leftovers(dir: &Path) -> TestResult {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" {
            continue;
        }
        if name.ends_with(".orig") || name.ends_with(".rej") {
            return Err(format!("{} was left behind", entry.path().display()).into());
        }
        if entry.file_type()?.is_dir() {
            no_leftovers(&entry.path())?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The settings
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn both_tools_apply_the_patch_outside_any_repository() -> TestResult {
    let adopted = Adopted::new()?;
    let patch = adopted.patch_arg()?;

    let by_patch = adopted.fresh("plain-patch")?;
    copy_base(&by_patch)?;
    adopted.succeed(PATCH, &["-p1", "-i", patch], &by_patch)?;
    expect_tree(&by_patch, proposed)?;

    let by_git = adopted.fresh("plain-git")?;
    copy_base(&by_git)?;
    adopted.succeed(GIT, &["apply", patch], &by_git)?;
    expect_tree(&by_git, proposed)
}

#[test]
fn both_tools_apply_the_patch_at_a_repository_root() -> TestResult {
    let adopted = Adopted::new()?;
    let patch = adopted.patch_arg()?;

    let by_patch = adopted.repository("root-patch")?;
    copy_base(&by_patch)?;
    adopted.succeed(PATCH, &["-p1", "-i", patch], &by_patch)?;
    expect_tree(&by_patch, proposed)?;

    let by_git = adopted.repository("root-git")?;
    copy_base(&by_git)?;
    adopted.succeed(GIT, &["apply", patch], &by_git)?;
    expect_tree(&by_git, proposed)
}

#[test]
fn in_a_repository_subdirectory_plain_git_apply_is_a_silent_no_op_and_the_printed_forms_apply()
-> TestResult {
    let adopted = Adopted::new()?;
    let patch = adopted.patch_arg()?;

    let skipped = adopted.repository("sub-plain")?.join("sub");
    copy_base(&skipped)?;
    adopted.succeed(GIT, &["apply", patch], &skipped)?;
    expect_tree(&skipped, based)?;

    let by_directory = adopted.repository("sub-directory")?.join("sub");
    copy_base(&by_directory)?;
    adopted.succeed(GIT, &["apply", "--directory=sub/", patch], &by_directory)?;
    expect_tree(&by_directory, proposed)?;

    let by_patch = adopted.repository("sub-patch")?.join("sub");
    copy_base(&by_patch)?;
    adopted.succeed(PATCH, &["-p1", "-i", patch], &by_patch)?;
    expect_tree(&by_patch, proposed)
}
