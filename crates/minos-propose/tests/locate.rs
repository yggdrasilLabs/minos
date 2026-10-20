//! The native-install locator (06-CONTEXT D-01 and its amendment; 06-RESEARCH Pattern 3 step 1, Pitfall 1): only
//! `$HOME/.local/bin/claude` as a symlink naming a direct child of `$HOME/.local/share/claude/versions/` is
//! accepted, and the two directories that control what the link names must be the user's own and not group- or
//! world-writable (review WR-05).
//!
//! Each layout is built in a fresh temporary `HOME`. Accepted targets are compared after `canonicalize`, because
//! macOS temp paths resolve through `/private`; a refused link text is reported as written. A directory owned by
//! another user cannot be built without privileges, so `DirWrongOwner` is covered by construction only: it uses
//! the same `uid` comparison as the target's `WrongOwner` check.

use std::error::Error;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use minos_propose::claude_cli::command::ClaudeVersion;
use minos_propose::claude_cli::locate::{LocateDir, LocateError, LocateFailure, locate};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

struct Home {
    dir: TempDir,
}

impl Home {
    fn new() -> Result<Self, Box<dyn Error>> {
        let dir = TempDir::new()?;
        fs::create_dir_all(dir.path().join(".local/bin"))?;
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn link(&self) -> PathBuf {
        self.path().join(".local/bin/claude")
    }

    fn versions(&self) -> PathBuf {
        self.path().join(".local/share/claude/versions")
    }

    /// Writes an executable-looking file at `relative` under `HOME` with `mode`.
    fn file(&self, relative: &str, mode: u32) -> Result<PathBuf, Box<dyn Error>> {
        let path = self.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, b"#!/bin/sh\n")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
        Ok(path)
    }

    fn link_to(&self, target: &Path) -> TestResult {
        if fs::symlink_metadata(self.link()).is_ok() {
            fs::remove_file(self.link())?;
        }
        symlink(target, self.link())?;
        Ok(())
    }
}

fn refused(home: &Home) -> Result<LocateError, Box<dyn Error>> {
    match locate(home.path()) {
        Ok(found) => Err(format!("expected a refusal, got {found:?}").into()),
        Err(error) => Ok(error),
    }
}

/// The refusal names the link and the versions directory, and the target when `target` is given.
fn assert_names_paths(home: &Home, error: &LocateError, target: Option<&Path>) {
    let message = error.to_string();
    let link = home.link();
    let versions = home.versions();
    assert!(message.contains(&link.display().to_string()), "{message}");
    assert!(
        message.contains(&versions.display().to_string()),
        "{message}"
    );
    assert!(message.contains("native install"), "{message}");
    assert_eq!(error.link(), link.as_path());
    assert_eq!(error.versions(), versions.as_path());
    if let Some(target) = target {
        assert!(message.contains(&target.display().to_string()), "{message}");
        assert_eq!(error.target(), Some(target));
    }
}

#[test]
fn the_native_layout_is_located() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    home.link_to(&target)?;
    let found = locate(home.path())?;
    assert_eq!(found.path(), fs::canonicalize(&target)?.as_path());
    assert_eq!(found.version(), ClaudeVersion::new(2, 1, 304));
    Ok(())
}

#[test]
fn a_missing_link_is_refused() -> TestResult {
    let home = Home::new()?;
    home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::LinkMissing);
    assert_names_paths(&home, &error, None);
    assert_eq!(error.target(), None);
    Ok(())
}

#[test]
fn a_regular_file_at_the_link_is_refused() -> TestResult {
    // The PATH-shim case (02-SPIKE-EVIDENCE.md C-01): a wrapper script in place of the link.
    let home = Home::new()?;
    home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    home.file(".local/bin/claude", 0o755)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::NotALink);
    assert_names_paths(&home, &error, None);
    Ok(())
}

#[test]
fn a_link_outside_versions_is_refused() -> TestResult {
    let home = Home::new()?;
    home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    let target = home.file("other/2.1.304", 0o755)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::OutsideVersions);
    assert_names_paths(&home, &error, Some(&target));
    Ok(())
}

#[test]
fn a_link_into_a_versions_subdirectory_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/sub/2.1.304", 0o755)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::OutsideVersions);
    assert_names_paths(&home, &error, Some(&target));
    Ok(())
}

#[test]
fn a_relative_link_into_versions_is_accepted() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    home.link_to(Path::new("../share/claude/versions/2.1.304"))?;
    let found = locate(home.path())?;
    assert_eq!(found.path(), fs::canonicalize(&target)?.as_path());
    assert_eq!(found.version(), ClaudeVersion::new(2, 1, 304));
    Ok(())
}

#[test]
fn a_relative_link_of_any_other_shape_is_refused() -> TestResult {
    for text in [
        "2.1.304",
        "../share/claude/versions/sub/2.1.304",
        "../../.local/share/claude/versions/2.1.304",
        "../share/claude/versions/..",
        "../share/claude/versions/.",
    ] {
        let home = Home::new()?;
        home.file(".local/share/claude/versions/2.1.304", 0o755)?;
        home.link_to(Path::new(text))?;
        let error = refused(&home)?;
        assert_eq!(error.failure(), LocateFailure::OutsideVersions, "{text}");
        assert_eq!(error.target(), Some(Path::new(text)), "{text}");
    }
    Ok(())
}

#[test]
fn a_link_naming_the_versions_directory_itself_is_refused() -> TestResult {
    let home = Home::new()?;
    home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    for text in [home.versions(), home.versions().join(".")] {
        home.link_to(&text)?;
        let error = refused(&home)?;
        assert_eq!(
            error.failure(),
            LocateFailure::OutsideVersions,
            "{}",
            text.display()
        );
    }
    Ok(())
}

#[test]
fn a_dangling_link_is_refused() -> TestResult {
    let home = Home::new()?;
    fs::create_dir_all(home.versions())?;
    let target = home.versions().join("2.1.304");
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::Dangling);
    assert_names_paths(&home, &error, Some(&target));
    Ok(())
}

#[test]
fn a_group_writable_target_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o775)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::Writable);
    assert_names_paths(&home, &error, Some(&fs::canonicalize(&target)?));
    Ok(())
}

#[test]
fn a_world_writable_target_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o757)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::Writable);
    Ok(())
}

#[test]
fn a_non_executable_target_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o644)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::NotExecutable);
    assert_names_paths(&home, &error, Some(&fs::canonicalize(&target)?));
    Ok(())
}

#[test]
fn a_target_that_is_not_a_version_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/latest", 0o755)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::NotAVersion);
    assert_names_paths(&home, &error, Some(&fs::canonicalize(&target)?));
    Ok(())
}

#[test]
fn a_directory_target_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.versions().join("2.1.304");
    fs::create_dir_all(&target)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::NotAFile);
    Ok(())
}

#[test]
fn a_missing_versions_directory_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file("other/2.1.304", 0o755)?;
    home.link_to(&target)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::VersionsDirMissing);
    assert_names_paths(&home, &error, None);
    Ok(())
}

/// A hop through another link is refused even when it resolves into the versions directory: the hop sits in a
/// directory the locator never checked, so following it would let whoever can write there choose the binary.
/// Before WR-05 this layout was accepted.
#[test]
fn a_link_through_an_intermediate_hop_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    let hop = home.path().join("hop");
    symlink(&target, &hop)?;
    home.link_to(&hop)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::OutsideVersions);
    assert_names_paths(&home, &error, Some(&hop));
    Ok(())
}

#[test]
fn a_symlink_inside_the_versions_directory_is_refused() -> TestResult {
    let home = Home::new()?;
    let real = home.file("elsewhere/2.1.304", 0o755)?;
    fs::create_dir_all(home.versions())?;
    let inside = home.versions().join("2.1.304");
    symlink(&real, &inside)?;
    home.link_to(&inside)?;
    let error = refused(&home)?;
    assert_eq!(error.failure(), LocateFailure::NotAFile);
    assert_names_paths(
        &home,
        &error,
        Some(&fs::canonicalize(home.versions())?.join("2.1.304")),
    );
    Ok(())
}

#[test]
fn a_moved_link_is_followed() -> TestResult {
    let home = Home::new()?;
    let first = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    let second = home.file(".local/share/claude/versions/2.1.305", 0o755)?;
    home.link_to(&first)?;
    let before = locate(home.path())?;
    assert_eq!(before.path(), fs::canonicalize(&first)?.as_path());
    assert_eq!(before.version(), ClaudeVersion::new(2, 1, 304));

    home.link_to(&second)?;
    let after = locate(home.path())?;
    assert_eq!(after.path(), fs::canonicalize(&second)?.as_path());
    assert_eq!(after.version(), ClaudeVersion::new(2, 1, 305));
    assert_ne!(before, after);
    Ok(())
}

#[test]
fn a_group_writable_versions_directory_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    home.link_to(&target)?;
    fs::set_permissions(home.versions(), fs::Permissions::from_mode(0o775))?;
    let error = refused(&home)?;
    assert_eq!(
        error.failure(),
        LocateFailure::DirWritable {
            dir: LocateDir::Versions
        }
    );
    assert_names_paths(&home, &error, None);
    assert_eq!(error.target(), None);
    let message = error.to_string();
    assert!(
        message.contains("the versions directory is group- or world-writable"),
        "{message}"
    );
    Ok(())
}

#[test]
fn a_world_writable_link_directory_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    home.link_to(&target)?;
    fs::set_permissions(
        home.path().join(".local/bin"),
        fs::Permissions::from_mode(0o777),
    )?;
    let error = refused(&home)?;
    assert_eq!(
        error.failure(),
        LocateFailure::DirWritable {
            dir: LocateDir::LinkDir
        }
    );
    assert_names_paths(&home, &error, None);
    let message = error.to_string();
    assert!(
        message.contains("the link's directory is group- or world-writable"),
        "{message}"
    );
    Ok(())
}

#[test]
fn a_link_directory_that_is_itself_a_symlink_is_refused() -> TestResult {
    let home = Home::new()?;
    let target = home.file(".local/share/claude/versions/2.1.304", 0o755)?;
    let real_bin = home.path().join("realbin");
    fs::create_dir(&real_bin)?;
    symlink(&target, real_bin.join("claude"))?;
    fs::remove_dir(home.path().join(".local/bin"))?;
    symlink(&real_bin, home.path().join(".local/bin"))?;
    let error = refused(&home)?;
    assert_eq!(
        error.failure(),
        LocateFailure::DirNotADirectory {
            dir: LocateDir::LinkDir
        }
    );
    Ok(())
}

#[test]
fn directory_checks_run_before_the_link_is_read() -> TestResult {
    // A dangling link in a writable link directory: the directory refusal comes first.
    let home = Home::new()?;
    fs::create_dir_all(home.versions())?;
    home.link_to(&home.versions().join("2.1.304"))?;
    fs::set_permissions(
        home.path().join(".local/bin"),
        fs::Permissions::from_mode(0o777),
    )?;
    let error = refused(&home)?;
    assert_eq!(
        error.failure(),
        LocateFailure::DirWritable {
            dir: LocateDir::LinkDir
        }
    );
    Ok(())
}

#[test]
fn failures_display_fixed_text() {
    for failure in [
        LocateFailure::LinkMissing,
        LocateFailure::NotALink,
        LocateFailure::VersionsDirMissing,
        LocateFailure::DirNotADirectory {
            dir: LocateDir::LinkDir,
        },
        LocateFailure::DirWrongOwner {
            dir: LocateDir::Versions,
        },
        LocateFailure::DirWritable {
            dir: LocateDir::LinkDir,
        },
        LocateFailure::Dangling,
        LocateFailure::OutsideVersions,
        LocateFailure::NotAFile,
        LocateFailure::WrongOwner,
        LocateFailure::Writable,
        LocateFailure::NotExecutable,
        LocateFailure::NotAVersion,
    ] {
        assert!(!failure.to_string().is_empty(), "{failure:?}");
    }
}
