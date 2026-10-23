//! The judge-data pack (07-CONTEXT D-03): the source snapshot's hygiene under the judge caps, with no manifest
//! excludes, no judge-path drop, and no clean check. Then the source snapshot's judge-overlap refusal (D-04,
//! D-20): a non-empty packed file equal to pushed judge data refuses the snapshot, and an empty one never does.
//!
//! No complete secret-shaped value appears in this file. The AWS-shaped key is joined at run time from pieces that
//! do not match on their own (04-RESEARCH Pitfall 5), so the self-scan in tests/scan.rs still passes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::id::{BlobId, TreeId};
use minos_core::limits::MAX_JUDGE_FILES;
use minos_core::manifest::Manifest;
use minos_core::path::RelPath;
use minos_source::{
    CleanPolicy, DenyRule, JudgePack, OffenderKind, Snapshot, SnapshotError, pack_judge_data,
    snapshot,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The expected output the judge compares against.
const EXPECTED: &[u8] = b"42\n";

/// The judge program.
const JUDGE: &[u8] = b"#!/bin/sh\necho judged\n";

/// The `minos.toml` example in CLAUDE.md, section "Projects", copied byte for byte from `CLAUDE_EXAMPLE` in
/// crates/minos-core/tests/manifest.rs.
const CLAUDE_EXAMPLE: &str = r#"[project]
name = "logparse"
kind = "optimize"            # tune | optimize | explore
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

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
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"              # greedy | pareto | review
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// The five project files: path, bytes, and whether the file is executable.
const PROJECT: [(&str, &[u8], bool); 5] = [
    ("minos.Dockerfile", b"FROM scratch\n", false),
    ("minos.toml", CLAUDE_EXAMPLE.as_bytes(), false),
    (
        "program.md",
        b"Make src/parse.rs faster without changing its output.\n",
        false,
    ),
    ("src/lib.rs", b"pub mod parse;\n", false),
    (
        "src/parse.rs",
        b"pub fn parse(input: &str) -> usize {\n    input.len()\n}\n",
        true,
    ),
];

/// Writes `bytes` to `dir/rel`, creating parent directories, with mode 0755 when `executable` and 0644 otherwise.
fn write_file(dir: &Path, rel: &str, bytes: &[u8], executable: bool) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes)?;
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// A temporary judge directory holding `expected.txt` and the executable `bin/judge`.
fn judge_dir() -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write_file(dir.path(), "expected.txt", EXPECTED, false)?;
    write_file(dir.path(), "bin/judge", JUDGE, true)?;
    Ok(dir)
}

/// A sparse file of `len` bytes at `dir/rel`, created with `set_len` so no data is written.
fn sparse(dir: &Path, rel: &str, len: u64) -> TestResult {
    let file = File::create(dir.join(rel))?;
    file.set_len(len)?;
    Ok(())
}

/// Every entry of `tarball` as (path, mode).
fn entries(tarball: &[u8]) -> Result<Vec<(String, u32)>, Box<dyn Error>> {
    let mut archive = tar::Archive::new(tarball);
    let mut listed = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        listed.push((path, entry.header().mode()?));
    }
    Ok(listed)
}

/// The packed paths, in order.
fn packed(pack: &JudgePack) -> Vec<&str> {
    pack.files().keys().map(RelPath::as_str).collect()
}

/// `n` characters cycling through `[A-Z]`.
fn upper(n: usize) -> String {
    ('A'..='Z').cycle().take(n).collect()
}

/// An AWS-shaped access key ID, joined at run time.
fn aws_key() -> String {
    format!("{}{}{}", "AK", "IA", upper(16))
}

#[test]
fn judge_dir_packs_deterministically_with_file_digests() -> TestResult {
    let dir = judge_dir()?;
    let pack = pack_judge_data(dir.path())?;
    assert_eq!(pack.tree(), TreeId::of(pack.tarball()));
    assert_eq!(pack.tree().to_blob_id(), BlobId::of(pack.tarball()));
    let digests: Vec<(&str, BlobId)> = pack
        .files()
        .iter()
        .map(|(path, id)| (path.as_str(), *id))
        .collect();
    assert_eq!(
        digests,
        [
            ("bin/judge", BlobId::of(JUDGE)),
            ("expected.txt", BlobId::of(EXPECTED)),
        ]
    );
    assert_eq!(
        entries(pack.tarball())?,
        [
            ("bin/judge".to_owned(), 0o755),
            ("expected.txt".to_owned(), 0o644),
        ]
    );
    assert_eq!(pack.report().files(), 2);
    let content = u64::try_from(JUDGE.len() + EXPECTED.len())?;
    assert_eq!(pack.report().content_bytes(), content);
    assert!(pack.report().denied().is_empty());

    let again = pack_judge_data(dir.path())?;
    assert_eq!(again.tarball(), pack.tarball());
    assert_eq!(again.into_tarball(), pack.into_tarball());
    Ok(())
}

#[test]
fn denylist_drops_are_reported() -> TestResult {
    let dir = judge_dir()?;
    write_file(dir.path(), ".env", b"TOKEN=x\n", false)?;
    write_file(dir.path(), "keys/id_rsa", b"not a key\n", false)?;
    let pack = pack_judge_data(dir.path())?;
    assert_eq!(packed(&pack), ["bin/judge", "expected.txt"]);
    let denied: Vec<(&str, DenyRule)> = pack
        .report()
        .denied()
        .iter()
        .map(|drop| (drop.path().as_str(), drop.rule()))
        .collect();
    assert_eq!(
        denied,
        [
            (".env", DenyRule::EnvFile),
            ("keys/id_rsa", DenyRule::SshKey)
        ]
    );
    Ok(())
}

#[test]
fn links_and_special_files_are_listed_together() -> TestResult {
    let dir = judge_dir()?;
    let root = dir.path();
    std::os::unix::fs::symlink("expected.txt", root.join("link"))?;
    let status = Command::new("/usr/bin/mkfifo")
        .arg(root.join("pipe"))
        .env_clear()
        .status()?;
    assert!(status.success(), "mkfifo failed: {status}");
    write_file(root, "a.txt", b"linked\n", false)?;
    fs::hard_link(root.join("a.txt"), root.join("b.txt"))?;

    let Err(SnapshotError::Offenders { shown, more }) = pack_judge_data(root) else {
        return Err("expected Offenders".into());
    };
    assert_eq!(more, 0);
    let listed: Vec<(&str, OffenderKind)> = shown
        .iter()
        .map(|offender| (offender.path().as_str(), offender.kind()))
        .collect();
    assert_eq!(
        listed,
        [
            ("a.txt", OffenderKind::Hardlink),
            ("b.txt", OffenderKind::Hardlink),
            ("link", OffenderKind::Symlink),
            ("pipe", OffenderKind::Special),
        ]
    );
    Ok(())
}

#[test]
fn secret_in_judge_data_is_refused_without_its_bytes() -> TestResult {
    let dir = judge_dir()?;
    let data = format!("case one\ncase two {}\n", aws_key());
    write_file(dir.path(), "cases.txt", data.as_bytes(), false)?;
    let result = pack_judge_data(dir.path());
    let Err(error @ SnapshotError::Secrets { .. }) = result else {
        return Err("expected Secrets".into());
    };
    if let SnapshotError::Secrets { shown, total } = &error {
        assert_eq!(*total, 1);
        let hits: Vec<(&str, &str, u64)> = shown
            .iter()
            .map(|hit| (hit.path().as_str(), hit.pattern(), hit.line()))
            .collect();
        assert_eq!(hits, [("cases.txt", "aws-access-key-id", 2)]);
    }
    let text = error.to_string();
    assert!(!text.contains(&aws_key()), "{text:?}");
    assert!(
        text.contains("cases.txt:2: matches aws-access-key-id"),
        "{text:?}"
    );
    Ok(())
}

#[test]
fn ignore_files_inside_the_judge_dir_apply() -> TestResult {
    let dir = judge_dir()?;
    write_file(dir.path(), ".minosignore", b"scratch.txt\n", false)?;
    write_file(dir.path(), ".gitignore", b"*.tmp\n", false)?;
    write_file(dir.path(), "scratch.txt", b"draft\n", false)?;
    write_file(dir.path(), "run.tmp", b"draft\n", false)?;
    let pack = pack_judge_data(dir.path())?;
    // The ignore files are regular files in the tree, so they are packed like any other.
    assert_eq!(
        packed(&pack),
        [".gitignore", ".minosignore", "bin/judge", "expected.txt"]
    );
    Ok(())
}

#[test]
fn minosignore_negation_is_refused() -> TestResult {
    let dir = judge_dir()?;
    write_file(dir.path(), ".minosignore", b"a.log\n!keep.log\n", false)?;
    let Err(SnapshotError::MinosignoreNegation { file, line }) = pack_judge_data(dir.path()) else {
        return Err("expected MinosignoreNegation".into());
    };
    assert_eq!(file.as_str(), ".minosignore");
    assert_eq!(line, 2);
    Ok(())
}

#[test]
fn file_over_the_judge_cap_is_refused_before_it_is_read() -> TestResult {
    let dir = judge_dir()?;
    let len = MAX_JUDGE_DATA_BYTES + 1;
    sparse(dir.path(), "huge.bin", len)?;
    let Err(SnapshotError::Offenders { shown, more }) = pack_judge_data(dir.path()) else {
        return Err("expected Offenders".into());
    };
    assert_eq!(more, 0);
    let listed: Vec<(&str, OffenderKind)> = shown
        .iter()
        .map(|offender| (offender.path().as_str(), offender.kind()))
        .collect();
    assert_eq!(
        listed,
        [("huge.bin", OffenderKind::Oversize { bytes: len })]
    );
    Ok(())
}

#[test]
fn tarball_over_the_judge_cap_is_refused_naming_its_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    // Exactly the cap in content, so the walk keeps it, but the header and the two end blocks put the tarball over.
    sparse(dir.path(), "data.bin", MAX_JUDGE_DATA_BYTES)?;
    let result = pack_judge_data(dir.path());
    let Err(error @ SnapshotError::JudgeTooLarge { .. }) = result else {
        return Err("expected JudgeTooLarge".into());
    };
    if let SnapshotError::JudgeTooLarge { bytes, max } = &error {
        assert_eq!(*max, MAX_JUDGE_DATA_BYTES);
        assert_eq!(*bytes, MAX_JUDGE_DATA_BYTES + 512 + 1024);
    }
    let text = error.to_string();
    assert!(text.contains("never truncated"), "{text:?}");
    assert!(text.contains(&MAX_JUDGE_DATA_BYTES.to_string()), "{text:?}");
    Ok(())
}

#[test]
fn content_over_the_judge_cap_is_refused() -> TestResult {
    let dir = judge_dir()?;
    let half = MAX_JUDGE_DATA_BYTES / 2 + 1;
    sparse(dir.path(), "a.bin", half)?;
    sparse(dir.path(), "b.bin", half)?;
    let Err(SnapshotError::TooLarge {
        bytes,
        max,
        largest,
    }) = pack_judge_data(dir.path())
    else {
        return Err("expected TooLarge".into());
    };
    assert_eq!(max, MAX_JUDGE_DATA_BYTES);
    assert!(bytes > MAX_JUDGE_DATA_BYTES, "{bytes}");
    let top: Vec<(&str, u64)> = largest
        .iter()
        .take(2)
        .map(|(path, len)| (path.as_str(), *len))
        .collect();
    assert_eq!(top, [("a.bin", half), ("b.bin", half)]);
    Ok(())
}

#[test]
fn too_many_judge_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    for index in 0..=MAX_JUDGE_FILES {
        fs::write(dir.path().join(format!("f{index}.txt")), b"a")?;
    }
    let result = pack_judge_data(dir.path());
    assert!(
        matches!(result, Err(SnapshotError::TooManyFiles { max }) if max == MAX_JUDGE_FILES),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn no_clean_check_and_no_judge_path_drop() -> TestResult {
    // Not a repository, and every name a manifest's judge argv or `[source] exclude` could name is kept: the
    // judge pack has no manifest, so it drops only what the hard denylist names.
    let dir = judge_dir()?;
    write_file(dir.path(), "judge", JUDGE, true)?;
    write_file(dir.path(), "judge-data/cases.txt", b"one\n", false)?;
    write_file(dir.path(), "target/out.txt", b"kept\n", false)?;
    let pack = pack_judge_data(dir.path())?;
    assert_eq!(
        packed(&pack),
        [
            "bin/judge",
            "expected.txt",
            "judge",
            "judge-data/cases.txt",
            "target/out.txt",
        ]
    );
    assert!(pack.report().denied().is_empty());
    Ok(())
}

/// A temporary directory holding the five-file project.
fn project() -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    for (rel, bytes, executable) in PROJECT {
        write_file(dir.path(), rel, bytes, executable)?;
    }
    Ok(dir)
}

/// Snapshots `dir` with the example manifest under `policy`, against the judge digests `judge`.
fn pack_source(
    dir: &Path,
    policy: CleanPolicy,
    judge: &BTreeSet<BlobId>,
) -> Result<Result<Snapshot, SnapshotError>, Box<dyn Error>> {
    let manifest = Manifest::parse_toml(CLAUDE_EXAMPLE)?;
    Ok(snapshot(dir, &manifest, policy, judge))
}

/// The judge digest set holding exactly the SHA-256 of `EXPECTED`.
fn expected_set() -> BTreeSet<BlobId> {
    BTreeSet::from([BlobId::of(EXPECTED)])
}

#[test]
fn source_file_equal_to_judge_data_is_refused() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), "src/fixture.txt", EXPECTED, false)?;
    let result = pack_source(dir.path(), CleanPolicy::AllowDirty, &expected_set())?;
    let Err(error @ SnapshotError::JudgeOverlap { .. }) = result else {
        return Err("expected JudgeOverlap".into());
    };
    if let SnapshotError::JudgeOverlap { paths, total } = &error {
        assert_eq!(*total, 1);
        let names: Vec<&str> = paths.iter().map(RelPath::as_str).collect();
        assert_eq!(names, ["src/fixture.txt"]);
    }
    let text = error.to_string();
    assert!(text.contains("src/fixture.txt"), "{text:?}");
    Ok(())
}

#[test]
fn every_overlapping_file_is_named_in_order() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), "tests/b.txt", EXPECTED, false)?;
    write_file(dir.path(), "data/a.txt", EXPECTED, false)?;
    let Err(SnapshotError::JudgeOverlap { paths, total }) =
        pack_source(dir.path(), CleanPolicy::AllowDirty, &expected_set())?
    else {
        return Err("expected JudgeOverlap".into());
    };
    assert_eq!(total, 2);
    let names: Vec<&str> = paths.iter().map(RelPath::as_str).collect();
    assert_eq!(names, ["data/a.txt", "tests/b.txt"]);
    Ok(())
}

#[test]
fn more_than_twenty_overlaps_are_counted() -> TestResult {
    let dir = project()?;
    for index in 0..25 {
        write_file(
            dir.path(),
            &format!("copies/c{index:02}.txt"),
            EXPECTED,
            false,
        )?;
    }
    let result = pack_source(dir.path(), CleanPolicy::AllowDirty, &expected_set())?;
    let Err(error @ SnapshotError::JudgeOverlap { .. }) = result else {
        return Err("expected JudgeOverlap".into());
    };
    if let SnapshotError::JudgeOverlap { paths, total } = &error {
        assert_eq!(*total, 25);
        assert_eq!(paths.len(), 20);
        assert_eq!(paths.first().map(RelPath::as_str), Some("copies/c00.txt"));
        assert_eq!(paths.last().map(RelPath::as_str), Some("copies/c19.txt"));
    }
    let text = error.to_string();
    assert!(text.contains("and 5 more"), "{text:?}");
    assert!(!text.contains("copies/c20.txt"), "{text:?}");
    Ok(())
}

#[test]
fn empty_files_are_exempt() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), "src/__init__.py", b"", false)?;
    write_file(dir.path(), "data/.gitkeep", b"", false)?;
    let judge = BTreeSet::from([BlobId::of(b""), BlobId::of(EXPECTED)]);
    let snap = pack_source(dir.path(), CleanPolicy::AllowDirty, &judge)??;
    assert_eq!(snap.report().files(), 7);
    Ok(())
}

#[test]
fn a_judge_set_that_matches_nothing_changes_nothing() -> TestResult {
    let dir = project()?;
    let plain = pack_source(dir.path(), CleanPolicy::AllowDirty, &BTreeSet::new())??;
    let checked = pack_source(dir.path(), CleanPolicy::AllowDirty, &expected_set())??;
    assert_eq!(plain.tarball(), checked.tarball());
    assert_eq!(plain.tree(), checked.tree());
    Ok(())
}

#[test]
fn overlap_is_checked_before_the_scan_and_the_clean_check() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), "src/fixture.txt", EXPECTED, false)?;
    let planted = format!("const KEY: &str = \"{}\";\n", aws_key());
    write_file(dir.path(), "src/key.rs", planted.as_bytes(), false)?;
    // Not a repository, so RequireClean would refuse as Dirty if it ran first; the scan would find the key.
    let result = pack_source(dir.path(), CleanPolicy::RequireClean, &expected_set())?;
    assert!(
        matches!(result, Err(SnapshotError::JudgeOverlap { total: 1, .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn pushed_judge_digests_refuse_their_copy_in_the_source() -> TestResult {
    let judge = judge_dir()?;
    let pushed = pack_judge_data(judge.path())?;
    let digests: BTreeSet<BlobId> = pushed.files().values().copied().collect();
    let dir = project()?;
    write_file(dir.path(), "bench/judge.sh", JUDGE, true)?;
    let Err(SnapshotError::JudgeOverlap { paths, total }) =
        pack_source(dir.path(), CleanPolicy::AllowDirty, &digests)?
    else {
        return Err("expected JudgeOverlap".into());
    };
    assert_eq!(total, 1);
    let names: Vec<&str> = paths.iter().map(RelPath::as_str).collect();
    assert_eq!(names, ["bench/judge.sh"]);
    Ok(())
}
