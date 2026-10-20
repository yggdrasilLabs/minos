//! The native-install locator (06-CONTEXT D-01 and its amendment; 06-RESEARCH Pattern 3 step 1, Pitfall 1).
//!
//! The provider runs only the user's own native install of `claude`: `$HOME/.local/bin/claude`, a symlink that
//! names, directly, a child of `$HOME/.local/share/claude/versions/`. A PATH lookup, a Homebrew or npm install, or
//! a wrapper is refused, because the first `claude` on `PATH` may be a shim that injects settings
//! (02-SPIKE-EVIDENCE.md C-01: it was a cmux shim). There is no configuration file and no override, as for
//! `minos-vmm` (05-CONTEXT D-04).
//!
//! The auto-updater moves the link between versions and deletes old ones, so the provider calls [`locate`] again
//! before every call and compares the result with the one it preflighted (D-01 amendment).
//!
//! Replacing the binary needs write permission on a directory, not on the file: unlinking and creating, or
//! renaming over, the target needs it on the versions directory, and repointing the link needs it on the link's
//! directory. So the locator checks those two directories as it checks the target, and it never follows the link:
//! it reads the link's text and requires that text to name a direct child of the versions directory, so no
//! intermediate hop, which could sit in a directory the locator never checked, is ever followed (review WR-05).
//!
//! [`locate`] checks, in order, and refuses at the first failure, naming every path it checked:
//!
//! 1. The link exists and is a symlink, by `symlink_metadata`, so a regular file in its place (a wrapper script)
//!    is refused before anything is followed.
//! 2. The versions directory canonicalizes to a directory.
//! 3. The link's directory, `$HOME/.local/bin`, is a directory and not a symlink, and it and the canonical
//!    versions directory are owned by the current user with no group or world write bit.
//! 4. The link's text, read with `read_link` and never followed, names a direct child of the versions directory:
//!    either `<versions>/<name>`, with the versions directory written as `$HOME` gives it or in its canonical
//!    form, or the relative `../share/claude/versions/<name>`. Anything else, a path elsewhere, a subdirectory, or
//!    a hop through another link, is refused.
//! 5. The target, `<canonical versions>/<name>`, exists (else the link dangles) and, by `symlink_metadata` so a
//!    link inside the versions directory is refused too, is a regular file owned by the current user, with no
//!    group or world write bit and at least one execute bit.
//! 6. The target's file name is a version, `X.Y.Z`.

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::claude_cli::command::ClaudeVersion;

/// The link the native installer writes, relative to `HOME`.
pub const LINK: &str = ".local/bin/claude";

/// The directory the native installer keeps its versions in, relative to `HOME`.
pub const VERSIONS_DIR: &str = ".local/share/claude/versions";

/// The versions directory as a link text relative to the link's directory.
const VERSIONS_FROM_LINK_DIR: &str = "../share/claude/versions";

/// The group and world write bits: a target with either could be replaced by another user, and a directory with
/// either lets another user replace or repoint what it holds.
const GROUP_OR_WORLD_WRITE: u32 = 0o022;

/// The execute bits for owner, group, and world.
const ANY_EXECUTE: u32 = 0o111;

/// The located native install: the canonical binary and the version its file name states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    path: PathBuf,
    version: ClaudeVersion,
}

impl Located {
    /// The canonical path of the binary, a direct child of the versions directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The version the binary's file name states. The preflight's `--version` query checks the binary agrees.
    #[must_use]
    pub const fn version(&self) -> ClaudeVersion {
        self.version
    }
}

/// A directory the locator checks because write permission on it controls which binary the link names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocateDir {
    /// The link's directory, `$HOME/.local/bin`: whoever can write it can repoint the link.
    LinkDir,
    /// The canonical versions directory: whoever can write it can replace the target.
    Versions,
}

impl fmt::Display for LocateDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LinkDir => "the link's directory",
            Self::Versions => "the versions directory",
        })
    }
}

/// Why [`locate`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocateFailure {
    /// Nothing exists at the link path.
    LinkMissing,
    /// Something other than a symlink is at the link path, such as a wrapper script.
    NotALink,
    /// The versions directory does not exist or is not a directory.
    VersionsDirMissing,
    /// A checked directory is not a directory, or is a symlink to one.
    DirNotADirectory {
        /// Which directory.
        dir: LocateDir,
    },
    /// A checked directory is owned by another user.
    DirWrongOwner {
        /// Which directory.
        dir: LocateDir,
    },
    /// A checked directory is group- or world-writable.
    DirWritable {
        /// Which directory.
        dir: LocateDir,
    },
    /// The link's text names nothing that exists.
    Dangling,
    /// The link's text does not name a direct child of the versions directory.
    OutsideVersions,
    /// The target is not a regular file.
    NotAFile,
    /// The target is owned by another user.
    WrongOwner,
    /// The target is group- or world-writable.
    Writable,
    /// The target has no execute bit.
    NotExecutable,
    /// The target's file name is not a version `X.Y.Z`.
    NotAVersion,
}

impl fmt::Display for LocateFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LinkMissing => f.write_str("the link does not exist"),
            Self::NotALink => f.write_str("the link path is not a symlink"),
            Self::VersionsDirMissing => f.write_str("the versions directory does not exist"),
            Self::DirNotADirectory { dir } => write!(f, "{dir} is not a directory"),
            Self::DirWrongOwner { dir } => write!(f, "{dir} is not owned by the current user"),
            Self::DirWritable { dir } => write!(f, "{dir} is group- or world-writable"),
            Self::Dangling => f.write_str("the link does not resolve to a file"),
            Self::OutsideVersions => {
                f.write_str("the link does not name a direct child of the versions directory")
            }
            Self::NotAFile => f.write_str("the link target is not a regular file"),
            Self::WrongOwner => f.write_str("the link target is not owned by the current user"),
            Self::Writable => f.write_str("the link target is group- or world-writable"),
            Self::NotExecutable => f.write_str("the link target is not executable"),
            Self::NotAVersion => f.write_str("the link target's file name is not a version"),
        }
    }
}

/// A refusal from [`locate`]: what failed, and every path checked.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "claude was not located: {failure}; checked the link {link}, the versions directory {versions}{target}; \
     Minos runs only the native install, so a PATH lookup, a Homebrew or npm install, or a wrapper is refused, \
     and there is no configuration file and no override",
    link = .link.display(),
    versions = .versions.display(),
    target = TargetText(.target.as_deref()),
)]
pub struct LocateError {
    failure: LocateFailure,
    link: PathBuf,
    versions: PathBuf,
    target: Option<PathBuf>,
}

impl LocateError {
    /// What failed.
    #[must_use]
    pub const fn failure(&self) -> LocateFailure {
        self.failure
    }

    /// The link path checked, `$HOME/.local/bin/claude`.
    #[must_use]
    pub fn link(&self) -> &Path {
        &self.link
    }

    /// The versions directory checked, `$HOME/.local/share/claude/versions`.
    #[must_use]
    pub fn versions(&self) -> &Path {
        &self.versions
    }

    /// The link's target, when it got that far: the link's own text when that text was refused or names nothing,
    /// else the canonical target it names.
    #[must_use]
    pub fn target(&self) -> Option<&Path> {
        self.target.as_deref()
    }
}

/// Renders the optional target clause of [`LocateError`]'s message.
struct TargetText<'a>(Option<&'a Path>);

impl fmt::Display for TargetText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(target) => write!(f, ", and the link target {}", target.display()),
            None => Ok(()),
        }
    }
}

/// Locates the native install under `home`, the user's `HOME` as the caller read it.
///
/// Reads the filesystem only: no environment variable, no configuration file, and no process is run. The link is
/// read, never followed.
///
/// # Errors
///
/// [`LocateError`] naming the [`LocateFailure`] and every path checked, at the first check that fails, in the
/// order the module documentation gives.
pub fn locate(home: &Path) -> Result<Located, LocateError> {
    let link = home.join(LINK);
    let versions = home.join(VERSIONS_DIR);
    let refuse = |failure: LocateFailure, target: Option<PathBuf>| LocateError {
        failure,
        link: link.clone(),
        versions: versions.clone(),
        target,
    };

    match fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(refuse(LocateFailure::LinkMissing, None));
        }
        // Something other than a symlink, or any other error, which leaves the link's kind unknown: refused as not
        // a symlink, and never followed.
        Ok(_) | Err(_) => return Err(refuse(LocateFailure::NotALink, None)),
    }

    let canonical_versions = match fs::canonicalize(&versions) {
        Ok(path) if fs::metadata(&path).is_ok_and(|meta| meta.is_dir()) => path,
        _ => return Err(refuse(LocateFailure::VersionsDirMissing, None)),
    };

    let link_dir = link.parent().unwrap_or(home);
    if let Err(failure) = check_dir(link_dir, LocateDir::LinkDir) {
        return Err(refuse(failure, None));
    }
    if let Err(failure) = check_dir(&canonical_versions, LocateDir::Versions) {
        return Err(refuse(failure, None));
    }

    let Ok(text) = fs::read_link(&link) else {
        return Err(refuse(LocateFailure::NotALink, None));
    };
    let Some(name) = direct_child_name(&text, &versions, &canonical_versions) else {
        return Err(refuse(LocateFailure::OutsideVersions, Some(text)));
    };
    let target = canonical_versions.join(name);

    // Never followed: a symlink inside the versions directory is not a regular file.
    let meta = match fs::symlink_metadata(&target) {
        Ok(meta) if meta.file_type().is_file() => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(refuse(LocateFailure::Dangling, Some(text)));
        }
        _ => return Err(refuse(LocateFailure::NotAFile, Some(target))),
    };
    if meta.uid() != rustix::process::getuid().as_raw() {
        return Err(refuse(LocateFailure::WrongOwner, Some(target)));
    }
    let mode = meta.mode();
    if mode & GROUP_OR_WORLD_WRITE != 0 {
        return Err(refuse(LocateFailure::Writable, Some(target)));
    }
    if mode & ANY_EXECUTE == 0 {
        return Err(refuse(LocateFailure::NotExecutable, Some(target)));
    }

    let Some(version) = name.to_str().and_then(ClaudeVersion::parse) else {
        return Err(refuse(LocateFailure::NotAVersion, Some(target)));
    };

    Ok(Located {
        path: target,
        version,
    })
}

/// Step 3 for one directory: by `symlink_metadata`, so a symlink in its place is refused rather than followed, it
/// is a directory owned by the current user with no group or world write bit.
fn check_dir(path: &Path, dir: LocateDir) -> Result<(), LocateFailure> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => meta,
        _ => return Err(LocateFailure::DirNotADirectory { dir }),
    };
    if meta.uid() != rustix::process::getuid().as_raw() {
        return Err(LocateFailure::DirWrongOwner { dir });
    }
    if meta.mode() & GROUP_OR_WORLD_WRITE != 0 {
        return Err(LocateFailure::DirWritable { dir });
    }
    Ok(())
}

/// Step 4: the file name `text` names when it is a direct child of the versions directory, written absolutely as
/// `versions` or `canonical_versions`, or relatively as [`VERSIONS_FROM_LINK_DIR`] from the link's directory.
///
/// Purely lexical: nothing is followed. A relative text with any other shape, `.` or `..` in the name, or a bare
/// name is refused, as is a text whose parent is any other path.
fn direct_child_name<'a>(
    text: &'a Path,
    versions: &Path,
    canonical_versions: &Path,
) -> Option<&'a std::ffi::OsStr> {
    let parent = text.parent()?;
    let direct = if text.is_absolute() {
        parent == versions || parent == canonical_versions
    } else {
        parent == Path::new(VERSIONS_FROM_LINK_DIR)
    };
    if !direct {
        return None;
    }
    // `Path::file_name` is `None` for a text ending in `..`, and `Path` drops a trailing `.`, so `<versions>/.`
    // has the versions directory's parent as its parent and was refused above; the last component is still
    // required to be a normal one, so nothing but a plain name passes.
    let name = text.file_name()?;
    let normal = text
        .components()
        .next_back()
        .is_some_and(|last| matches!(last, std::path::Component::Normal(_)));
    normal.then_some(name)
}
