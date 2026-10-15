//! The deterministic source snapshot of a Minos project.
//!
//! This crate walks a project tree under four exclusion sources (the project's `.gitignore` files, its
//! `.minosignore` files, the manifest's `[source] exclude` entries, and a hard denylist), refuses anything that is
//! not a regular file whose path fits the core path grammar, scans the exact bytes it will pack for secrets,
//! checks that the tree is committed, and writes a deterministic ustar tarball (sorted paths, zeroed metadata,
//! regular files only) whose SHA-256 is the snapshot's `TreeId`.
//!
//! [`snapshot`] is the only function that produces a tarball, and it always runs the denylist, the caps, and the
//! secret scan; [`CleanPolicy::AllowDirty`] skips only the git check. Only `minos-cli` links this crate: `deny.toml`
//! refuses any other dependent, so the filesystem walker and the read-only `git` exec of the clean check never
//! enter the loop's or the runner's graph.
//!
//! The guards are public and tested on their own: [`denylist`], [`scan`], and [`clean`]. None of them produces a
//! tarball, so exposing them opens no path around the scan.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::path::Path;

use minos_core::id::TreeId;
use minos_core::manifest::Manifest;
use minos_core::path::RelPath;
use minos_core::text::SafeText;

pub mod clean;
pub mod denylist;
mod pack;
pub mod scan;
mod walk;

pub use clean::{CleanPolicy, Dirty, check_clean};
pub use denylist::{DenyRule, deny_component};
pub use scan::{ScanError, Scanner, SecretHit};

/// One entry the hard denylist dropped from the snapshot. Every drop is reported, so nothing the user can see in
/// the tree vanishes silently (04-CONTEXT D-07).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Denied {
    path: SafeText,
    rule: DenyRule,
}

impl Denied {
    /// The dropped file or directory, relative to the project root. A directory's contents are not listed: the
    /// walk never descends into it.
    #[must_use]
    pub const fn path(&self) -> &SafeText {
        &self.path
    }

    /// The rule that dropped it.
    #[must_use]
    pub const fn rule(&self) -> DenyRule {
        self.rule
    }
}

/// `<path>: denied (<pattern>)`.
impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: denied ({})", self.path, self.rule.pattern())
    }
}

/// Why an entry cannot be packed (04-CONTEXT D-01, D-03). The pack fails; nothing is skipped or followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum OffenderKind {
    /// The name is not UTF-8 or falls outside the [`RelPath`] grammar.
    NotInGrammar,
    /// A symbolic link.
    Symlink,
    /// A regular file with more than one hard link.
    Hardlink,
    /// A FIFO, socket, or device: neither a regular file nor a directory.
    Special,
    /// The path does not fit a ustar header: 255 bytes split at a `/`, final component at most 100.
    UstarPath,
    /// The file is over the 16 MiB per-file cap.
    Oversize {
        /// The file's length.
        bytes: u64,
    },
    /// The file changed between `lstat` and the read: another device, inode, type, or length.
    Raced,
}

impl fmt::Display for OffenderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInGrammar => {
                f.write_str("the name is outside the path grammar ([A-Za-z0-9._+-], no leading -)")
            }
            Self::Symlink => f.write_str("a symbolic link; links are never followed or packed"),
            Self::Hardlink => f.write_str("a file with more than one hard link"),
            Self::Special => f.write_str("not a regular file (a FIFO, socket, or device)"),
            Self::UstarPath => {
                f.write_str("the path does not fit a ustar header (255 bytes, final component 100)")
            }
            Self::Oversize { bytes } => write!(f, "{bytes} bytes, over the 16 MiB per-file cap"),
            Self::Raced => f.write_str("the file changed while it was being read"),
        }
    }
}

/// One entry that refuses the pack.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Offender {
    path: SafeText,
    kind: OffenderKind,
}

impl Offender {
    /// The entry, relative to the project root, sanitized: a hostile name cannot carry a terminal escape.
    #[must_use]
    pub const fn path(&self) -> &SafeText {
        &self.path
    }

    /// Why it cannot be packed.
    #[must_use]
    pub const fn kind(&self) -> OffenderKind {
        self.kind
    }
}

/// `<path>: <why>`.
impl fmt::Display for Offender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.kind)
    }
}

/// What a snapshot holds and what the denylist dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackReport {
    files: u64,
    content_bytes: u64,
    denied: Vec<Denied>,
}

impl PackReport {
    /// How many files the tarball holds.
    #[must_use]
    pub const fn files(&self) -> u64 {
        self.files
    }

    /// The summed length of the packed files, headers and padding excluded.
    #[must_use]
    pub const fn content_bytes(&self) -> u64 {
        self.content_bytes
    }

    /// Every denylist drop, sorted by path.
    #[must_use]
    pub fn denied(&self) -> &[Denied] {
        &self.denied
    }
}

/// A packed project: the deterministic tarball, its [`TreeId`], and the pack report.
///
/// The caller stores the tarball with `Blobs::put`, whose ID equals [`TreeId::to_blob_id`] of [`Snapshot::tree`].
#[derive(Clone, Debug)]
pub struct Snapshot {
    tarball: Vec<u8>,
    tree: TreeId,
    report: PackReport,
}

impl Snapshot {
    /// The tarball bytes.
    #[must_use]
    pub fn tarball(&self) -> &[u8] {
        &self.tarball
    }

    /// The tarball bytes, consuming the snapshot.
    #[must_use]
    pub fn into_tarball(self) -> Vec<u8> {
        self.tarball
    }

    /// The tree's ID: the plain SHA-256 of the tarball.
    #[must_use]
    pub const fn tree(&self) -> TreeId {
        self.tree
    }

    /// What was packed and what the denylist dropped.
    #[must_use]
    pub const fn report(&self) -> &PackReport {
        &self.report
    }
}

/// Why a project could not be packed. Every message names paths only through [`SafeText`] or [`RelPath`] and never
/// prints file bytes.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SnapshotError {
    /// The walk, or an ignore file it reads, failed.
    #[error("walking the project failed: {message}")]
    Walk {
        /// What failed, sanitized.
        message: SafeText,
    },
    /// A filesystem operation on one entry failed.
    #[error("could not {op} {path}")]
    Io {
        /// The operation: inspect, open, read, pack, or finish.
        op: &'static str,
        /// The entry, relative to the project root.
        path: SafeText,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// Entries that cannot be packed: links, special files, names outside the grammar, paths that do not fit
    /// ustar, oversize files, and files that changed mid-read.
    #[error("the project holds entries that cannot be packed:{}", listing(.shown, *.more))]
    Offenders {
        /// The first offenders in path order, at most 20.
        shown: Vec<Offender>,
        /// How many more there are.
        more: u64,
    },
    /// More regular files than the cap.
    #[error(
        "the project holds more than {max} files; exclude some with .minosignore or [source] exclude"
    )]
    TooManyFiles {
        /// The cap.
        max: u64,
    },
    /// The files together are over the content cap.
    #[error(
        "the project's files total {bytes} bytes, over the {max}-byte cap; the largest:{}",
        sizes(.largest)
    )]
    TooLarge {
        /// The summed length of every packable file.
        bytes: u64,
        /// The cap.
        max: u64,
        /// The largest files, largest first, at most 20.
        largest: Vec<(RelPath, u64)>,
    },
    /// A `.minosignore` line starts with `!`. A negation there would re-include a file a `.gitignore` excludes,
    /// because the custom ignore file is checked first.
    #[error(
        "{file}:{line}: a .minosignore line may not start with `!`, because it would re-include a file .gitignore excludes"
    )]
    MinosignoreNegation {
        /// The `.minosignore` file, relative to the project root.
        file: SafeText,
        /// The 1-based line.
        line: u64,
    },
    /// A `.minosignore` file is over the cap.
    #[error("{file} is over {max} bytes")]
    IgnoreFileTooLarge {
        /// The `.minosignore` file, relative to the project root.
        file: SafeText,
        /// The cap.
        max: u64,
    },
    /// Surface or program paths the snapshot does not hold, typically dropped by an ignore rule or the denylist.
    #[error("the snapshot is missing [surface] or program paths:{}", listing(.paths, 0))]
    SurfaceMissing {
        /// The missing paths, sorted.
        paths: Vec<RelPath>,
    },
    /// The secret scanner did not compile.
    #[error("the secret scanner failed to start")]
    Scanner(#[source] ScanError),
    /// The secret scan found matches in the bytes to be packed. There is no override.
    #[error(
        "the secret scan found {total} match(es); remove them or exclude the files:{}",
        listing(.shown, total.saturating_sub(u64::try_from(.shown.len()).unwrap_or(u64::MAX)))
    )]
    Secrets {
        /// The first hits in path, line, and pattern order, at most 20.
        shown: Vec<SecretHit>,
        /// How many hits there are.
        total: u64,
    },
    /// The tree is not committed, and the policy requires it.
    #[error("the working tree is not clean")]
    Dirty(#[source] Dirty),
}

/// One item per line, then the count of the rest.
fn listing<T: fmt::Display>(items: &[T], more: u64) -> String {
    let mut text = String::new();
    for item in items {
        write!(text, "\n  {item}").ok();
    }
    if more > 0 {
        write!(text, "\n  and {more} more").ok();
    }
    text
}

/// One `<path>: <bytes> bytes` per line.
fn sizes(largest: &[(RelPath, u64)]) -> String {
    let mut text = String::new();
    for (path, bytes) in largest {
        write!(text, "\n  {path}: {bytes} bytes").ok();
    }
    text
}

/// Packs the project at `root` into a deterministic, secret-scanned snapshot.
///
/// In order: the walk under the four exclusion sources, which refuses every offender and every over-cap tree;
/// the check that every `[surface]` path and the program file are in the snapshot; the secret scan over exactly
/// the bytes to be packed; the clean check unless `policy` is [`CleanPolicy::AllowDirty`]; and the tarball, whose
/// SHA-256 is the [`TreeId`]. `AllowDirty` skips only the clean check.
///
/// Packed files are not compared with pushed judge data: judge data is stored as one tarball blob, so per-file
/// digests must first be recorded by `minos judge push` (Phase 7, JUDGE-01).
///
/// # Errors
///
/// - [`SnapshotError::Walk`], [`SnapshotError::Io`]: the walk or a filesystem operation failed.
/// - [`SnapshotError::Offenders`]: entries that are not packable regular files, all listed.
/// - [`SnapshotError::TooManyFiles`], [`SnapshotError::TooLarge`]: the tree is over a cap.
/// - [`SnapshotError::MinosignoreNegation`], [`SnapshotError::IgnoreFileTooLarge`]: a `.minosignore` is refused.
/// - [`SnapshotError::SurfaceMissing`]: a surface or program path is not in the snapshot.
/// - [`SnapshotError::Scanner`], [`SnapshotError::Secrets`]: the scanner failed, or found a secret.
/// - [`SnapshotError::Dirty`]: the tree is not committed and `policy` is [`CleanPolicy::RequireClean`].
pub fn snapshot(
    root: &Path,
    manifest: &Manifest,
    policy: CleanPolicy,
) -> Result<Snapshot, SnapshotError> {
    let collected = walk::collect(root, manifest)?;

    let missing: BTreeSet<&RelPath> = manifest
        .writable()
        .iter()
        .chain(manifest.readable())
        .chain([manifest.program()])
        .filter(|path| !collected.files.contains_key(*path))
        .collect();
    if !missing.is_empty() {
        return Err(SnapshotError::SurfaceMissing {
            paths: missing.into_iter().cloned().collect(),
        });
    }

    let scanner = Scanner::new().map_err(SnapshotError::Scanner)?;
    let mut shown = Vec::new();
    let mut total: u64 = 0;
    for (path, file) in &collected.files {
        for hit in scanner.scan(path, &file.bytes) {
            total = total.saturating_add(1);
            if shown.len() < walk::SHOWN_OFFENDERS {
                shown.push(hit);
            }
        }
    }
    if total > 0 {
        return Err(SnapshotError::Secrets { shown, total });
    }

    match policy {
        CleanPolicy::RequireClean => {
            let packed: BTreeSet<RelPath> = collected.files.keys().cloned().collect();
            check_clean(root, &packed).map_err(SnapshotError::Dirty)?;
        }
        CleanPolicy::AllowDirty => {}
    }

    let tarball = pack::tarball(&collected.files)?;
    let tree = TreeId::of(&tarball);
    Ok(Snapshot {
        tarball,
        tree,
        report: PackReport {
            files: u64::try_from(collected.files.len()).unwrap_or(u64::MAX),
            content_bytes: collected.content_bytes,
            denied: collected.denied,
        },
    })
}
