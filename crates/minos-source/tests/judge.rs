//! The judge-data pack (07-CONTEXT D-03): the source snapshot's hygiene under the judge caps, with no manifest
//! excludes, no judge-path drop, and no clean check.
//!
//! No complete secret-shaped value appears in this file. The AWS-shaped key is joined at run time from pieces that
//! do not match on their own (04-RESEARCH Pitfall 5), so the self-scan in tests/scan.rs still passes.

use std::error::Error;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::id::{BlobId, TreeId};
use minos_core::limits::MAX_JUDGE_FILES;
use minos_core::path::RelPath;
use minos_source::{DenyRule, JudgePack, OffenderKind, SnapshotError, pack_judge_data};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The expected output the judge compares against.
const EXPECTED: &[u8] = b"42\n";

/// The judge program.
const JUDGE: &[u8] = b"#!/bin/sh\necho judged\n";

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
