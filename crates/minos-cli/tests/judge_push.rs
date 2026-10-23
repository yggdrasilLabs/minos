//! `minos judge push` on a temporary project (07-CONTEXT D-01 to D-04; JUDGE-01, CLI-04): the first push stores
//! the tarball and writes the `[[judge]]` entry, the same push leaves the lock alone, different data under a
//! recorded label is refused without `--update` and replaces only that label with it, a new label adds an entry,
//! a push missing a judge argv path is refused, a judge directory the source snapshot would pick up is refused,
//! and the pack's hygiene (secret scan, denylist, the 96 MiB cap) holds with nothing stored on refusal.

use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use minos_cli::judge_push::{JudgePushError, JudgePushOptions, JudgePushReport, judge_push};
use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::ledger::BackendKind;
use minos_core::lock::{BuilderId, BuilderText, Lock, LockedImage};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::path::RelPath;
use minos_source::SnapshotError;
use minos_store::{Blobs, FileStore, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// A manifest whose judge command names `judge/score`, which lies under the excluded `judge/` directory.
const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/", "private/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// The judge program's bytes.
const SCORE: &[u8] = b"#!/bin/sh\necho '{\"v\":1,\"metrics\":[]}'\n";

/// The expected output's bytes.
const EXPECTED: &[u8] = b"judge-private-expected-output\n";

/// A temporary project beside a judge data directory outside it.
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    project: PathBuf,
    data: PathBuf,
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

/// The manifest with `[judge] version = label`.
fn manifest_text(label: u32) -> String {
    MANIFEST.replace("version = 1", &format!("version = {label}"))
}

/// The fixture: a project with the manifest at label 1, and judge data holding `judge/score` and
/// `judge/expected.txt` in `<root>/judge-data`, outside the project.
fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let root = fs::canonicalize(dir.path())?;
    let project = root.join("project");
    fs::create_dir(&project)?;
    write(&project, "minos.toml", manifest_text(1).as_bytes())?;
    write(&project, "program.md", b"Make src/parse.rs faster.\n")?;
    write(&project, "src/lib.rs", b"pub mod parse;\n")?;
    write(&project, "src/parse.rs", b"pub fn parse() {}\n")?;
    let data = root.join("judge-data");
    write(&data, "judge/score", SCORE)?;
    write(&data, "judge/expected.txt", EXPECTED)?;
    Ok(Fixture {
        _dir: dir,
        root,
        project,
        data,
    })
}

/// `judge push` of `dir` into the fixture's project.
fn push(fixture: &Fixture, dir: &Path, update: bool) -> Result<JudgePushReport, JudgePushError> {
    judge_push(&fixture.project, dir, JudgePushOptions::new(update))
}

/// The fixture's lock, parsed.
fn read_lock(fixture: &Fixture) -> Result<Lock, Box<dyn Error>> {
    let text = fs::read_to_string(fixture.project.join("minos.lock"))?;
    Ok(Lock::parse_toml(&text)?)
}

/// The fixture's lock bytes.
fn lock_bytes(fixture: &Fixture) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(fixture.project.join("minos.lock"))?)
}

/// An image entry for krun on arm64, to prove a judge push leaves it alone.
fn image_entry() -> Result<LockedImage, Box<dyn Error>> {
    let builder = BuilderId::new(
        BuilderText::try_from("fake-builder".to_owned())?,
        BuilderText::try_from("v1".to_owned())?,
    );
    Ok(LockedImage::new(
        BackendKind::Krun,
        Arch::Arm64,
        ImageDigest::from_blob_id(&BlobId::of(b"root image")),
        BlobId::of(b"dockerfile"),
        Vec::new(),
        BlobId::of(b"guest"),
        builder,
    ))
}

#[test]
fn first_push_stores_blob_and_writes_lock() -> TestResult {
    let fixture = fixture()?;

    let report = push(&fixture, &fixture.data, false)?;

    assert_eq!(report.label(), 1);
    assert!(report.lock_written());
    assert_eq!(report.file_count(), 2);
    let content = u64::try_from(SCORE.len() + EXPECTED.len())?;
    assert_eq!(report.bytes(), content);
    assert!(report.denied().is_empty());

    // The tarball is in the store under the tree's blob ID, and hashes to the tree.
    let store = FileStore::open(&fixture.project.join(STORE_DIR))?;
    let tree = *report.tree();
    let tarball = store.get(&tree.to_blob_id(), MAX_JUDGE_DATA_BYTES)?;
    assert_eq!(TreeId::of(&tarball), tree);

    // The lock records the label, the tree, the counts, the pushing minos version, and every file's digest.
    let lock = read_lock(&fixture)?;
    let entry = lock.judge(1).ok_or("no judge entry for label 1")?;
    assert_eq!(*entry.tree(), tree);
    assert_eq!(entry.file_count(), 2);
    assert_eq!(entry.bytes(), content);
    assert_eq!(entry.minos().as_str(), env!("CARGO_PKG_VERSION"));
    let files: Vec<(&str, BlobId)> = entry
        .files()
        .iter()
        .map(|file| (file.path().as_str(), *file.sha256()))
        .collect();
    assert_eq!(
        files,
        [
            ("judge/expected.txt", BlobId::of(EXPECTED)),
            ("judge/score", BlobId::of(SCORE)),
        ]
    );

    // The judge version is the hash of the tree and the manifest's judge spec.
    let manifest = Manifest::parse_toml(&manifest_text(1))?;
    assert_eq!(
        *report.judge_version(),
        JudgeVersion::of(&tree, manifest.judge())
    );
    Ok(())
}

#[test]
fn same_push_keeps_lock() -> TestResult {
    let fixture = fixture()?;
    push(&fixture, &fixture.data, false)?;
    let before = lock_bytes(&fixture)?;

    let report = push(&fixture, &fixture.data, false)?;

    assert!(!report.lock_written());
    assert_eq!(lock_bytes(&fixture)?, before);
    Ok(())
}

#[test]
fn changed_data_refused_without_update() -> TestResult {
    let fixture = fixture()?;
    // A lock with an image entry, which a judge push must leave alone.
    let image = image_entry()?;
    let text = Lock::empty().with_image(image.clone()).to_toml()?;
    write(&fixture.project, "minos.lock", text.as_bytes())?;
    let first = push(&fixture, &fixture.data, false)?;
    let before = lock_bytes(&fixture)?;
    write(&fixture.data, "judge/expected.txt", b"other expected output\n")?;

    let result = push(&fixture, &fixture.data, false);

    let Err(error @ JudgePushError::LabelMismatch { .. }) = result else {
        return Err(format!("expected LabelMismatch, got {result:?}").into());
    };
    let JudgePushError::LabelMismatch {
        label,
        locked,
        pushed,
    } = &error
    else {
        return Err("expected LabelMismatch".into());
    };
    assert_eq!(*label, 1);
    assert_eq!(locked, first.tree());
    assert_ne!(pushed, locked);
    let message = error.to_string();
    assert!(message.contains(&locked.to_string()), "{message}");
    assert!(message.contains(&pushed.to_string()), "{message}");
    assert!(message.contains("rerun with --update"), "{message}");
    assert_eq!(lock_bytes(&fixture)?, before);

    // With --update the label's entry is replaced and the image entry is untouched.
    let report = push(&fixture, &fixture.data, true)?;
    assert!(report.lock_written());
    assert_eq!(report.tree(), pushed);
    let lock = read_lock(&fixture)?;
    let entry = lock.judge(1).ok_or("no judge entry for label 1")?;
    assert_eq!(entry.tree(), pushed);
    assert_eq!(
        lock.image(BackendKind::Krun, Arch::Arm64),
        Some(&image),
        "the image entry changed"
    );
    Ok(())
}

#[test]
fn new_label_adds_second_entry() -> TestResult {
    let fixture = fixture()?;
    let first = push(&fixture, &fixture.data, false)?;
    write(&fixture.project, "minos.toml", manifest_text(2).as_bytes())?;
    write(&fixture.data, "judge/expected.txt", b"label two output\n")?;

    let second = push(&fixture, &fixture.data, false)?;

    assert_eq!(second.label(), 2);
    assert!(second.lock_written());
    let lock = read_lock(&fixture)?;
    assert_eq!(
        lock.judge(1).map(|entry| *entry.tree()),
        Some(*first.tree())
    );
    assert_eq!(
        lock.judge(2).map(|entry| *entry.tree()),
        Some(*second.tree())
    );
    Ok(())
}

#[test]
fn missing_judge_path_refused() -> TestResult {
    let fixture = fixture()?;
    fs::remove_file(fixture.data.join("judge/score"))?;

    let result = push(&fixture, &fixture.data, false);

    let Err(JudgePushError::MissingJudgePath { paths }) = result else {
        return Err(format!("expected MissingJudgePath, got {result:?}").into());
    };
    assert_eq!(paths, [RelPath::try_from("judge/score")?]);
    assert!(!fixture.project.join("minos.lock").exists());
    assert!(!fixture.project.join(STORE_DIR).exists());

    write(&fixture.data, "judge/score", SCORE)?;
    push(&fixture, &fixture.data, false)?;
    Ok(())
}

#[test]
fn dir_in_snapshot_refused() -> TestResult {
    let fixture = fixture()?;
    // `data/` is inside the project and no `[source] exclude` entry covers it.
    let inside = fixture.project.join("data");
    write(&inside, "judge/score", SCORE)?;

    let result = push(&fixture, &inside, false);
    assert!(
        matches!(result, Err(JudgePushError::DirInSnapshot { .. })),
        "{result:?}"
    );

    // The project itself is never judge data.
    let result = push(&fixture, &fixture.project, false);
    assert!(
        matches!(result, Err(JudgePushError::DirInSnapshot { .. })),
        "{result:?}"
    );
    assert!(!fixture.project.join("minos.lock").exists());
    assert!(!fixture.project.join(STORE_DIR).exists());
    Ok(())
}

#[test]
fn dir_under_excluded_path_accepted() -> TestResult {
    let fixture = fixture()?;
    // `private/` is excluded, so the snapshot and the image context never see it.
    let inside = fixture.project.join("private").join("judge-v1");
    write(&inside, "judge/score", SCORE)?;

    let report = push(&fixture, &inside, false)?;

    assert!(report.lock_written());
    assert_eq!(report.file_count(), 1);
    Ok(())
}

#[test]
fn secret_refused_and_nothing_stored() -> TestResult {
    let fixture = fixture()?;
    // An AWS-style access key ID, assembled at run time so no key literal sits in the repository.
    let key = format!("AKIA{}", "Q".repeat(16));
    write(
        &fixture.data,
        "judge/expected.txt",
        format!("expected\n{key}\n").as_bytes(),
    )?;

    let result = push(&fixture, &fixture.data, false);

    assert!(
        matches!(
            result,
            Err(JudgePushError::Pack(SnapshotError::Secrets { .. }))
        ),
        "{result:?}"
    );
    assert!(!fixture.project.join("minos.lock").exists());
    assert!(!fixture.project.join(STORE_DIR).exists());
    Ok(())
}

#[test]
fn over_cap_refused_and_nothing_stored() -> TestResult {
    let fixture = fixture()?;
    // A sparse file one byte over the cap: the walk refuses it from its metadata, before it is read.
    let file = File::create(fixture.data.join("huge.bin"))?;
    file.set_len(MAX_JUDGE_DATA_BYTES + 1)?;

    let result = push(&fixture, &fixture.data, false);

    assert!(
        matches!(
            result,
            Err(JudgePushError::Pack(SnapshotError::Offenders { .. }))
        ),
        "{result:?}"
    );
    assert!(!fixture.project.join("minos.lock").exists());
    assert!(!fixture.project.join(STORE_DIR).exists());
    Ok(())
}

#[test]
fn denylist_drops_are_reported() -> TestResult {
    let fixture = fixture()?;
    write(&fixture.data, ".env", b"MINOS_FIXTURE=1\n")?;

    let report = push(&fixture, &fixture.data, false)?;

    let denied: Vec<&str> = report
        .denied()
        .iter()
        .map(|denied| denied.path().as_str())
        .collect();
    assert_eq!(denied, [".env"]);
    assert_eq!(report.file_count(), 2);
    Ok(())
}

#[test]
fn missing_dir_refused() -> TestResult {
    let fixture = fixture()?;

    let result = push(&fixture, &fixture.root.join("no-such-dir"), false);

    assert!(
        matches!(result, Err(JudgePushError::Dir { .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn cli_wiring() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_minos"))
        .args(["judge", "push", "--help"])
        .output()?;
    assert!(output.status.success(), "minos judge push --help failed");
    let help = String::from_utf8(output.stdout)?;
    for wanted in ["--update", "<dir>"] {
        assert!(help.contains(wanted), "{wanted} missing in:\n{help}");
    }
    Ok(())
}
