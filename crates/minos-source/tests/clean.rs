//! The read-only git clean check against temporary repositories: clean, every kind of dirty, outside a
//! repository, the packed-but-untracked gap, and a project that is a subdirectory of its repository.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use minos_core::path::RelPath;
use minos_source::{Dirty, check_clean};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// Runs the system git in `dir` with a cleared environment and a fixed test identity.
fn git(dir: &Path, args: &[&str]) -> TestResult {
    let status = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=minos-test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {args:?} failed: {status}").into())
    }
}

/// Writes `content` to `dir/rel`, creating parent directories.
fn write(dir: &Path, rel: &str, content: &str) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

/// A repository with `files` written and committed.
fn committed_repo(files: &[&str]) -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    git(dir.path(), &["init", "-q"])?;
    for file in files {
        write(dir.path(), file, "content\n")?;
    }
    git(dir.path(), &["add", "--all"])?;
    git(dir.path(), &["commit", "-q", "-m", "init"])?;
    Ok(dir)
}

fn paths(texts: &[&str]) -> Result<BTreeSet<RelPath>, Box<dyn Error>> {
    texts
        .iter()
        .map(|text| Ok(RelPath::try_from(*text)?))
        .collect()
}

#[test]
fn a_committed_tree_is_clean() -> TestResult {
    let files = ["src/lib.rs", "minos.toml", "program.md"];
    let repo = committed_repo(&files)?;
    check_clean(repo.path(), &paths(&files)?)?;
    Ok(())
}

#[test]
fn an_untracked_file_is_dirty() -> TestResult {
    let repo = committed_repo(&["src/lib.rs"])?;
    write(repo.path(), "src/new.rs", "new\n")?;
    let result = check_clean(repo.path(), &paths(&["src/lib.rs"])?);
    assert!(matches!(result, Err(Dirty::Changes)), "{result:?}");
    Ok(())
}

#[test]
fn a_modified_tracked_file_is_dirty() -> TestResult {
    let repo = committed_repo(&["src/lib.rs"])?;
    write(repo.path(), "src/lib.rs", "changed\n")?;
    let result = check_clean(repo.path(), &paths(&["src/lib.rs"])?);
    assert!(matches!(result, Err(Dirty::Changes)), "{result:?}");
    Ok(())
}

#[test]
fn a_staged_uncommitted_change_is_dirty() -> TestResult {
    let repo = committed_repo(&["src/lib.rs"])?;
    write(repo.path(), "src/lib.rs", "staged\n")?;
    git(repo.path(), &["add", "src/lib.rs"])?;
    let result = check_clean(repo.path(), &paths(&["src/lib.rs"])?);
    assert!(matches!(result, Err(Dirty::Changes)), "{result:?}");
    Ok(())
}

#[test]
fn a_directory_outside_any_repository_is_dirty() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "src/lib.rs", "content\n")?;
    let result = check_clean(dir.path(), &paths(&["src/lib.rs"])?);
    assert!(
        matches!(result, Err(Dirty::GitFailed { code: Some(code) }) if code != 0),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn a_packed_file_git_ignores_through_info_exclude_is_untracked() -> TestResult {
    let repo = committed_repo(&["src/lib.rs"])?;
    write(repo.path(), "notes.log", "local only\n")?;
    write(repo.path(), ".git/info/exclude", "notes.log\n")?;
    let result = check_clean(repo.path(), &paths(&["src/lib.rs", "notes.log"])?);
    match result {
        Err(Dirty::Untracked { count, shown }) => {
            assert_eq!(count, 1);
            assert_eq!(shown, [RelPath::try_from("notes.log")?]);
        }
        other => return Err(format!("expected Untracked, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn untracked_paths_are_counted_in_full_and_shown_sorted_up_to_twenty() -> TestResult {
    let repo = committed_repo(&["src/lib.rs"])?;
    write(repo.path(), ".git/info/exclude", "gen/\n")?;
    let mut packed = vec!["src/lib.rs".to_owned()];
    for index in 0..25 {
        let name = format!("gen/f{index:02}.txt");
        write(repo.path(), &name, "x\n")?;
        packed.push(name);
    }
    let packed: Vec<&str> = packed.iter().map(String::as_str).collect();
    match check_clean(repo.path(), &paths(&packed)?) {
        Err(Dirty::Untracked { count, shown }) => {
            assert_eq!(count, 25);
            assert_eq!(shown.len(), 20);
            assert_eq!(shown.first(), Some(&RelPath::try_from("gen/f00.txt")?));
            assert_eq!(shown.last(), Some(&RelPath::try_from("gen/f19.txt")?));
        }
        other => return Err(format!("expected Untracked, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn a_project_subdirectory_is_judged_on_its_own_files() -> TestResult {
    let repo = committed_repo(&["project/src/lib.rs", "project/minos.toml", "other/a.txt"])?;
    write(repo.path(), "other/a.txt", "dirty sibling\n")?;
    write(repo.path(), "other/untracked.txt", "dirty sibling\n")?;
    check_clean(
        &repo.path().join("project"),
        &paths(&["src/lib.rs", "minos.toml"])?,
    )?;
    Ok(())
}

#[test]
fn dirt_inside_a_project_subdirectory_is_still_dirty() -> TestResult {
    let repo = committed_repo(&["project/src/lib.rs"])?;
    write(repo.path(), "project/src/lib.rs", "changed\n")?;
    let result = check_clean(&repo.path().join("project"), &paths(&["src/lib.rs"])?);
    assert!(matches!(result, Err(Dirty::Changes)), "{result:?}");
    Ok(())
}

#[test]
fn dirty_messages_tell_the_user_what_to_do() {
    for dirty in [
        Dirty::GitUnavailable,
        Dirty::GitFailed { code: Some(128) },
        Dirty::GitFailed { code: None },
        Dirty::TimedOut,
        Dirty::OutputTooLarge,
        Dirty::Changes,
    ] {
        assert!(dirty.to_string().contains("--allow-dirty"), "{dirty}");
    }
}
