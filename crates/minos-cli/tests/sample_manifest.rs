//! The shipped sample project, `examples/logparse`, checked on the host in the workspace gate (DOCS-01).
//!
//! - `sample_manifest_parses`: `minos.toml` validates, its one writable file is `src/parse.rs`, its judge argv
//!   names no project path, its objective `accuracy` is paired with the held-out `accuracy_held_out`, and the
//!   Dockerfile's only base image is pinned by digest.
//! - `sample_crates_have_no_dependencies`: the candidate and judge crates are std only and forbid `unwrap`, so
//!   both build offline with `--frozen` under the repository's own lint levels.
//! - `sample_snapshot_holds_no_judge_file`: `judge push judge` accepts the judge directory, and the source snapshot
//!   packed under the lock's judge file digests holds no `judge/` path and does hold the build inputs.
//! - `sample_scores_on_the_host`: built with the toolchain running this test, the baseline passes the gate and
//!   scores below 1 on both accuracies, and the improved parser fixture scores 1 on both.
//!
//! Every test that writes works on a copy of the sample in a temporary directory, so no test writes into the
//! repository, and the copy builds outside the repository's workspace as it does in the VM.

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::read_lock;
use minos_core::id::MetricName;
use minos_core::image::base_images;
use minos_core::limits::Limits;
use minos_core::manifest::Manifest;
use minos_core::metric::{Metric, Scorecard};
use minos_core::path::RelPath;
use minos_source::{CleanPolicy, snapshot};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The improved parser: a complete replacement for the sample's `src/parse.rs`.
const IMPROVED_PARSE: &str = include_str!("fixtures/logparse_improved_parse.rs");

/// The sample's base image: `rust:1.97.1-slim-trixie`, pinned by its index digest alone.
const BASE_IMAGE: &str =
    "rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c";

/// The sample's directory in this repository.
fn sample_dir() -> TestResult<PathBuf> {
    Ok(fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/logparse"),
    )?)
}

/// The sample's manifest, parsed.
fn sample_manifest() -> TestResult<Manifest> {
    let text = fs::read_to_string(sample_dir()?.join(MANIFEST_FILE))?;
    Ok(Manifest::parse_toml(&text)?)
}

/// Copies the directory tree `from` into `to`: regular files and directories only, skipping every directory named
/// `target` or `.minos`. A symlink or any other kind of entry is an error, never followed or skipped.
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
            fs::copy(&source, to.join(&name))?;
        } else {
            return Err(format!("{} is not a regular file or directory", source.display()).into());
        }
    }
    Ok(())
}

/// A temporary directory holding a copy of the sample at `<dir>/logparse`, returned with that path.
fn sample_copy() -> TestResult<(TempDir, PathBuf)> {
    let dir = TempDir::new()?;
    let copy = fs::canonicalize(dir.path())?.join("logparse");
    copy_tree(&sample_dir()?, &copy)?;
    Ok((dir, copy))
}

/// A `toml::Table` read from `path`.
fn read_table(path: &Path) -> TestResult<toml::Table> {
    Ok(fs::read_to_string(path)?.parse::<toml::Table>()?)
}

/// The cargo running this test and the `rustc` beside it, so the copies build with the pinned toolchain even
/// outside the repository, where `rust-toolchain.toml` does not apply.
struct Toolchain {
    cargo: PathBuf,
    rustc: PathBuf,
}

impl Toolchain {
    /// Reads `CARGO`, which cargo sets for the tests it runs; an absent value is an error, never a skip.
    fn from_env() -> TestResult<Self> {
        let cargo = PathBuf::from(std::env::var_os("CARGO").ok_or("CARGO is not set")?);
        let rustc = cargo.with_file_name("rustc");
        if !rustc.is_file() {
            return Err(format!("no rustc beside {}", cargo.display()).into());
        }
        Ok(Self { cargo, rustc })
    }

    /// `cargo build --release --frozen` of the crate at `manifest_path` into `target_dir`.
    fn build(&self, manifest_path: &Path, target_dir: &Path) -> TestResult {
        let mut command = Command::new(&self.cargo);
        command
            .args([
                "build",
                "--release",
                "--frozen",
                "--quiet",
                "--manifest-path",
            ])
            .arg(manifest_path)
            .arg("--target-dir")
            .arg(target_dir)
            .env("RUSTC", &self.rustc);
        run(&mut command)
    }
}

/// Runs `command` with no stdin and requires it to exit 0.
fn run(command: &mut Command) -> TestResult {
    let output = command.stdin(Stdio::null()).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{command:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

/// The value of metric `name` on `card`.
fn metric(card: &Scorecard, name: &str) -> TestResult<Metric> {
    Ok(card
        .value(&MetricName::from_str(name)?)
        .ok_or_else(|| format!("the scorecard has no {name}"))?)
}

/// Runs the built candidate on the copy's input, then the built judge on its output, and parses the scorecard.
fn score(copy: &Path, scratch: &Path, target: &Path) -> TestResult<Scorecard> {
    let fields = scratch.join("fields.tsv");
    let card = scratch.join("scorecard.json");
    run(Command::new(target.join("release/logparse"))
        .arg(copy.join("data/input.log"))
        .arg(&fields))?;
    run(Command::new(target.join("release/logparse-judge"))
        .arg(copy.join("judge/answers.tsv"))
        .arg(&fields)
        .arg(&card))?;
    Ok(Scorecard::parse_json(
        &fs::read(&card)?,
        &Limits::standard(),
    )?)
}

#[test]
fn sample_manifest_parses() -> TestResult {
    // Arrange
    let sample = sample_dir()?;

    // Act
    let manifest = sample_manifest()?;
    let images = base_images(&fs::read_to_string(sample.join("minos.Dockerfile"))?)?;

    // Assert
    let writable: Vec<&RelPath> = manifest.writable().iter().collect();
    if writable != [&RelPath::try_from("src/parse.rs")?] {
        return Err("the writable set is not exactly src/parse.rs".into());
    }
    let judge = manifest.judge();
    if !judge.paths().is_empty() {
        return Err("the judge argv names a project path".into());
    }
    if judge.gates().iter().collect::<Vec<_>>() != [&MetricName::from_str("rows_match")?] {
        return Err("the gate is not exactly rows_match".into());
    }
    let [objective] = judge.objectives() else {
        return Err("the sample does not have exactly one objective".into());
    };
    if *objective.metric() != MetricName::from_str("accuracy")? {
        return Err("the objective is not accuracy".into());
    }
    if objective.held_out() != Some(&MetricName::from_str("accuracy_held_out")?) {
        return Err("the objective is not paired with accuracy_held_out".into());
    }
    let [image] = images.as_slice() else {
        return Err("the Dockerfile does not name exactly one pinned base image".into());
    };
    if image.to_string() != BASE_IMAGE {
        return Err(format!("the Dockerfile's base image is {image}, not {BASE_IMAGE}").into());
    }
    Ok(())
}

#[test]
fn sample_crates_have_no_dependencies() -> TestResult {
    // Arrange
    let sample = sample_dir()?;

    for manifest in [sample.join("Cargo.toml"), sample.join("judge/Cargo.toml")] {
        // Act
        let table = read_table(&manifest)?;

        // Assert
        for key in [
            "dependencies",
            "dev-dependencies",
            "build-dependencies",
            "workspace",
        ] {
            if table.contains_key(key) {
                return Err(format!("{} has a [{key}] table", manifest.display()).into());
            }
        }
        let unwrap = table
            .get("lints")
            .and_then(toml::Value::as_table)
            .and_then(|lints| lints.get("clippy"))
            .and_then(toml::Value::as_table)
            .and_then(|clippy| clippy.get("unwrap_used"))
            .and_then(toml::Value::as_str);
        if unwrap != Some("forbid") {
            return Err(
                format!("{} does not forbid clippy::unwrap_used", manifest.display()).into(),
            );
        }
    }
    Ok(())
}

#[test]
fn sample_snapshot_holds_no_judge_file() -> TestResult {
    // Arrange
    let (_dir, copy) = sample_copy()?;
    let manifest = sample_manifest()?;

    // Act
    judge_push(&copy, &copy.join("judge"), JudgePushOptions::new(false))?;
    let lock = read_lock(&copy.join(LOCK_FILE))?.ok_or("judge push wrote no minos.lock")?;
    let snap = snapshot(
        &copy,
        &manifest,
        CleanPolicy::AllowDirty,
        &lock.judge_file_digests(),
    )?;

    // Assert
    let mut archive = tar::Archive::new(snap.tarball());
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        paths.push(entry?.path()?.into_owned());
    }
    if let Some(judge) = paths.iter().find(|path| path.starts_with("judge")) {
        return Err(format!("the snapshot holds the judge path {}", judge.display()).into());
    }
    for needed in ["minos.Dockerfile", "src/parse.rs", "Cargo.lock"] {
        if !paths.iter().any(|path| path == Path::new(needed)) {
            return Err(format!("the snapshot does not hold {needed}").into());
        }
    }
    Ok(())
}

#[test]
fn sample_scores_on_the_host() -> TestResult {
    // Arrange
    let (dir, copy) = sample_copy()?;
    let scratch = fs::canonicalize(dir.path())?;
    let target = scratch.join("t");
    let toolchain = Toolchain::from_env()?;
    toolchain.build(&copy.join("judge/Cargo.toml"), &target)?;

    // Act: the baseline.
    toolchain.build(&copy.join("Cargo.toml"), &target)?;
    let baseline = score(&copy, &scratch, &target)?;

    // Assert
    if metric(&baseline, "rows_match")? != Metric::ONE {
        return Err("the baseline fails the rows_match gate".into());
    }
    for name in ["accuracy", "accuracy_held_out"] {
        if metric(&baseline, name)? >= Metric::ONE {
            return Err(format!("the baseline already scores 1 on {name}").into());
        }
    }

    // Act: the improved parser in place of src/parse.rs.
    fs::write(copy.join("src/parse.rs"), IMPROVED_PARSE)?;
    toolchain.build(&copy.join("Cargo.toml"), &target)?;
    let improved = score(&copy, &scratch, &target)?;

    // Assert
    for name in ["rows_match", "accuracy", "accuracy_held_out"] {
        if metric(&improved, name)? != Metric::ONE {
            return Err(format!("the improved parser does not score 1 on {name}").into());
        }
    }
    Ok(())
}
