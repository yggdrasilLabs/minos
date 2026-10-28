//! `minos_cli::project` on a temporary project (08-CONTEXT D-06; 08-RESEARCH Pattern 2, G7):
//!
//! - a project with pushed judge data, a lock image entry matching the snapshot's Dockerfile and a fake guest, a
//!   stored root image, and a prices file prepares: the base is the snapshot's tree and is stored under it, and the
//!   run inputs carry the lock's root image and the pushed judge version;
//! - no `minos.lock`, a lock with no image entry, and a deleted root image blob each name `minos env build`;
//! - an edited Dockerfile and a changed guest binary each name `minos env build --update`;
//! - an unpushed `[judge] version` label and a deleted judge data blob each name `minos judge push`;
//! - a missing prices file names `minos init` and the path;
//! - `open_runs` refuses a project with no `.minos` and creates nothing, and opens one that has it;
//! - `base_surface` on a run started from the prepared inputs gives the prepared base back;
//! - `prices_path` is under `~/.config/minos`, and `read_prices` parses a priced file.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use minos_cli::env_build::{LOCK_FILE, MANIFEST_FILE};
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::lock_file::{read_lock, write_lock};
use minos_cli::project::{
    PrepareError, Prepared, base_surface, open_runs, prepare, prices_path, read_prices,
};
use minos_core::id::{BlobId, ExperimentId, ImageDigest, RunId};
use minos_core::ledger::{BackendKind, Event, Record};
use minos_core::lock::{BuilderId, BuilderText, Lock, LockedImage};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::state::fold;
use minos_source::{CleanPolicy, snapshot};
use minos_store::{Blobs, FileStore, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// A sweep project: one writable file with an anchored `value = ` line, one readable file, and a judge argv of
/// bare words, so the judge command names no project path.
const MANIFEST: &str = r#"[project]
name = "prepare"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/value.txt"]
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
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 2

[[param]]
name = "value"
file = "src/value.txt"
anchor = "value = "
min = 0
max = 100
step = 1

[[proposer]]
kind = "sweep"
batch = 2
"#;

/// The Dockerfile the lock image entry is written for.
const DOCKERFILE: &[u8] = b"FROM scratch\n";

/// The fake guest binary the lock image entry is written for. `prepare` only hashes it.
const GUEST: &[u8] = b"a fake minos-guest\n";

/// A priced `prices.toml`.
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models]
"#;

/// A temporary project beside its judge data, guest, and prices file, ready to prepare.
struct Fixture {
    _dir: TempDir,
    project: PathBuf,
    guest: PathBuf,
    prices: PathBuf,
    root_image: ImageDigest,
    judge: JudgeVersion,
}

impl Fixture {
    /// Writes the project, pushes its judge data, stores a root image, and writes the lock's image entry.
    fn new() -> Result<Self, Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        write(&project, MANIFEST_FILE, MANIFEST.as_bytes())?;
        write(&project, "program.md", b"Raise the value.\n")?;
        write(&project, "minos.Dockerfile", DOCKERFILE)?;
        write(&project, "src/value.txt", b"value = 5\n")?;
        write(&project, "src/lib.rs", b"pub fn f() {}\n")?;
        let data = root.join("judge-data");
        write(&data, "expected.txt", b"prepare test judge data\n")?;
        let guest = root.join("minos-guest");
        fs::write(&guest, GUEST)?;
        let prices = root.join("prices.toml");
        fs::write(&prices, PRICES)?;

        let pushed = judge_push(&project, &data, JudgePushOptions::new(false))?;
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
        Ok(Self {
            _dir: dir,
            project,
            guest,
            prices,
            root_image,
            judge: *pushed.judge_version(),
        })
    }

    /// `prepare` on the fixture, with the clean check skipped (the temporary project is not a repository).
    fn prepare(&self) -> Result<Prepared, PrepareError> {
        prepare(
            &self.project,
            &self.guest,
            &self.prices,
            CleanPolicy::AllowDirty,
        )
    }

    /// The fixture's lock.
    fn lock(&self) -> Result<Lock, Box<dyn Error>> {
        Ok(read_lock(&self.project.join(LOCK_FILE))?.ok_or("no lock")?)
    }

    /// Deletes the blob `id` from the project's store.
    fn delete_blob(&self, id: &BlobId) -> TestResult {
        let hex = id.to_hex();
        let (shard, rest) = hex.split_at(2);
        fs::remove_file(
            self.project
                .join(STORE_DIR)
                .join("blobs")
                .join("sha256")
                .join(shard)
                .join(rest),
        )?;
        Ok(())
    }

    /// The text `prepare` refuses with, every source included, as `main` prints it.
    fn refusal(&self) -> Result<String, Box<dyn Error>> {
        match self.prepare() {
            Ok(_) => Err("prepare succeeded".into()),
            Err(error) => Ok(chain(&error)),
        }
    }
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

/// `error` and every source, joined with `: `.
fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}

#[test]
fn prepare_builds_the_run_inputs_from_the_snapshot_and_the_lock() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;

    let manifest = Manifest::parse_toml(MANIFEST)?;
    let snap = snapshot(
        &fixture.project,
        &manifest,
        CleanPolicy::AllowDirty,
        &fixture.lock()?.judge_file_digests(),
    )?;
    assert_eq!(prepared.base().id(), snap.tree());
    assert!(prepared.store().contains(&snap.tree().to_blob_id())?);
    assert_eq!(prepared.manifest_bytes(), MANIFEST.as_bytes());
    assert_eq!(prepared.program(), b"Raise the value.\n");
    assert_eq!(prepared.base().tree().len(), 2);

    let inputs = prepared.run_inputs();
    assert_eq!(inputs.root_image, fixture.root_image);
    assert_eq!(inputs.judge, fixture.judge);
    assert_eq!(inputs.manifest_id(), prepared.manifest_id());
    Ok(())
}

#[test]
fn no_lock_names_env_build() -> TestResult {
    let fixture = Fixture::new()?;
    fs::remove_file(fixture.project.join(LOCK_FILE))?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos env build"), "{text}");
    Ok(())
}

#[test]
fn a_lock_without_an_image_names_env_build() -> TestResult {
    let fixture = Fixture::new()?;
    let judge = fixture.lock()?.judge(1).ok_or("no judge entry")?.clone();
    write_lock(
        &fixture.project.join(STORE_DIR),
        &fixture.project.join(LOCK_FILE),
        Lock::empty().with_judge(judge).to_toml()?.as_bytes(),
    )?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos env build"), "{text}");
    Ok(())
}

#[test]
fn an_edited_dockerfile_names_env_build_update() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(
        fixture.project.join("minos.Dockerfile"),
        b"FROM scratch\n# edited\n",
    )?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos env build --update"), "{text}");
    Ok(())
}

#[test]
fn a_changed_guest_names_env_build_update() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(&fixture.guest, b"another guest\n")?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos env build --update"), "{text}");
    Ok(())
}

#[test]
fn a_missing_root_image_names_env_build() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.delete_blob(&fixture.root_image.to_blob_id())?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos env build"), "{text}");
    assert!(!text.contains("--update"), "{text}");
    Ok(())
}

#[test]
fn an_unpushed_judge_label_names_judge_push() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(
        fixture.project.join(MANIFEST_FILE),
        MANIFEST.replace("version = 1", "version = 2"),
    )?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos judge push"), "{text}");
    Ok(())
}

#[test]
fn a_missing_judge_data_blob_names_judge_push() -> TestResult {
    let fixture = Fixture::new()?;
    let tree = *fixture.lock()?.judge(1).ok_or("no judge entry")?.tree();
    fixture.delete_blob(&tree.to_blob_id())?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos judge push"), "{text}");
    Ok(())
}

#[test]
fn a_missing_prices_file_names_init_and_the_path() -> TestResult {
    let fixture = Fixture::new()?;
    fs::remove_file(&fixture.prices)?;
    let text = fixture.refusal()?;
    assert!(text.contains("minos init"), "{text}");
    assert!(
        text.contains(&fixture.prices.to_string_lossy().into_owned()),
        "{text}"
    );
    Ok(())
}

#[test]
fn open_runs_refuses_a_project_without_a_store_and_creates_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let result = open_runs(dir.path());
    assert!(matches!(result, Err(PrepareError::NoRuns)));
    assert!(fs::symlink_metadata(dir.path().join(STORE_DIR)).is_err());
    Ok(())
}

#[test]
fn open_runs_opens_an_existing_store() -> TestResult {
    let fixture = Fixture::new()?;
    let store = open_runs(&fixture.project)?;
    assert!(store.contains(&fixture.root_image.to_blob_id())?);
    Ok(())
}

#[test]
fn base_surface_reads_the_prepared_base_back_from_the_store() -> TestResult {
    let fixture = Fixture::new()?;
    let prepared = fixture.prepare()?;
    let base = prepared.base().id();
    let manifest = prepared.manifest_id();
    let started_at = 1_760_000_000;
    let record = Record::v1(Event::RunStarted {
        run: RunId::of(&manifest, &base, started_at),
        manifest,
        base,
        baseline: ExperimentId::baseline(&base),
        started_at,
        root_image: Some(prepared.root_image()),
    });
    let state = fold([&record])?;
    let surface = base_surface(prepared.store(), &state, prepared.manifest())?;
    assert!(surface == *prepared.base());
    Ok(())
}

#[test]
fn prices_live_under_the_config_directory_and_parse() -> TestResult {
    let path = prices_path(Path::new("/home/someone"));
    assert_eq!(
        path,
        Path::new("/home/someone/.config/minos/prices.toml").to_path_buf()
    );
    let fixture = Fixture::new()?;
    let prices = read_prices(&fixture.prices)?;
    assert!(
        prices
            .missing_models(&Manifest::parse_toml(MANIFEST)?)
            .is_empty()
    );
    Ok(())
}
