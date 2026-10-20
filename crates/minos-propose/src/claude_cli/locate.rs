//! The native-install locator (06-CONTEXT D-01 and its amendment; 06-RESEARCH Pattern 3 step 1, Pitfall 1).
//!
//! The provider runs only the user's own native install of `claude`: `$HOME/.local/bin/claude`, a symlink whose
//! canonical target is a direct child of `$HOME/.local/share/claude/versions/`. A PATH lookup, a Homebrew or npm
//! install, or a wrapper is refused, because the first `claude` on `PATH` may be a shim that injects settings
//! (02-SPIKE-EVIDENCE.md C-01: it was a cmux shim). There is no configuration file and no override, as for
//! `minos-vmm` (05-CONTEXT D-04).
//!
//! The auto-updater moves the link between versions and deletes old ones, so the provider calls [`locate`] again
//! before every call and compares the result with the one it preflighted (D-01 amendment).
//!
//! [`locate`] checks, in order, and refuses at the first failure, naming every path it checked:
//!
//! 1. The link exists and is a symlink, by `symlink_metadata`, so a regular file in its place (a wrapper script)
//!    is refused before anything is followed.
//! 2. The versions directory canonicalizes to a directory.
//! 3. The link canonicalizes; a link that resolves to nothing is dangling.
//! 4. The canonical target's parent is the canonical versions directory, so a link out of it, or into a
//!    subdirectory of it, is refused.
//! 5. The target is a regular file owned by the current user, with no group or world write bit and at least one
//!    execute bit.
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

/// The group and world write bits: a target with either could be replaced by another user.
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

/// Why [`locate`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocateFailure {
    /// Nothing exists at the link path.
    LinkMissing,
    /// Something other than a symlink is at the link path, such as a wrapper script.
    NotALink,
    /// The versions directory does not exist or is not a directory.
    VersionsDirMissing,
    /// The link resolves to nothing.
    Dangling,
    /// The link's canonical target is not a direct child of the versions directory.
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
        f.write_str(match self {
            Self::LinkMissing => "the link does not exist",
            Self::NotALink => "the link path is not a symlink",
            Self::VersionsDirMissing => "the versions directory does not exist",
            Self::Dangling => "the link does not resolve to a file",
            Self::OutsideVersions => {
                "the link does not resolve to a direct child of the versions directory"
            }
            Self::NotAFile => "the link target is not a regular file",
            Self::WrongOwner => "the link target is not owned by the current user",
            Self::Writable => "the link target is group- or world-writable",
            Self::NotExecutable => "the link target is not executable",
            Self::NotAVersion => "the link target's file name is not a version",
        })
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

    /// The link's target, when it got that far: the canonical target, or the link's own text when it dangles.
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
/// Reads the filesystem only: no environment variable, no configuration file, and no process is run.
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

    let Ok(target) = fs::canonicalize(&link) else {
        // A link that resolves to nothing, or to a loop: named by its own text, the only target there is.
        return Err(refuse(LocateFailure::Dangling, fs::read_link(&link).ok()));
    };

    if target.parent() != Some(canonical_versions.as_path()) {
        return Err(refuse(LocateFailure::OutsideVersions, Some(target)));
    }

    // The target is canonical, so `symlink_metadata` describes the file itself.
    let meta = match fs::symlink_metadata(&target) {
        Ok(meta) if meta.file_type().is_file() => meta,
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

    let Some(version) = target
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(ClaudeVersion::parse)
    else {
        return Err(refuse(LocateFailure::NotAVersion, Some(target)));
    };

    Ok(Located {
        path: target,
        version,
    })
}
