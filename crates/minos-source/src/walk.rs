//! The walk: which files of a project enter the snapshot, and which refuse it (04-CONTEXT D-01, D-03, D-04,
//! D-05, D-07).
//!
//! Exactly four exclusion sources apply, as a union (D-04):
//!
//! 1. `.gitignore` files inside the project, read without a repository (`require_git(false)`).
//! 2. `.minosignore` files, read by the same matcher as a custom ignore file name.
//! 3. The manifest's `[source] exclude` entries, applied in the entry filter.
//! 4. The hard denylist ([`crate::denylist`]) and every path the judge command names, applied in the entry filter
//!    before the excludes, so a drop is reported with its rule.
//!
//! `standard_filters(false)` is called first, which turns off hidden-file skipping, parent directories' ignore
//! files, `.ignore` files, and git's global and `info/exclude` sources, so nothing outside these four applies. The
//! ignore matcher checks a custom ignore file before `.gitignore` and the first match wins, so a `!` line in a
//! `.minosignore` would re-include a gitignored file and break the union; such a line refuses the pack. An ignore
//! file the matcher cannot fully read or parse also refuses it, because the exclusions would then not be the ones
//! the user wrote.
//!
//! The entry filter prunes a directory before descent, so the contents of `.git/` or `.minos/` are never read.
//! Every other entry must be a regular file with one link, a name inside the [`RelPath`] grammar, a path that fits
//! a ustar header, and a size under the cap. Every offender is collected before the walk fails, so one error lists
//! them all. A file is opened only after `lstat`, and read only if the opened file is the same regular file
//! (device, inode, and length), so a file swapped for a link mid-walk is refused, never followed.
//!
//! The judge-data pack (`crate::judge`, 07-CONTEXT D-03) walks with the same code under [`WalkRules::judge`]: the
//! same ignore files, hard denylist, offender rules, and identity check, with no manifest excludes, no judge-path
//! drop, and the judge caps.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};

use ignore::{DirEntry, Walk, WalkBuilder};
use minos_core::guest::MAX_JUDGE_DATA_BYTES;
use minos_core::limits::MAX_JUDGE_FILES;
use minos_core::manifest::{Manifest, SourceExclude};
use minos_core::path::RelPath;
use minos_core::text::SafeText;

use crate::denylist::{DenyRule, deny_component};
use crate::{Denied, Offender, OffenderKind, SnapshotError};

/// The most regular files a snapshot holds. Real source trees for an experiment are far smaller; the cap bounds
/// the walk, the scan, and the tarball (D-05).
pub(crate) const MAX_SNAPSHOT_FILES: u64 = 10_000;

/// The largest file a snapshot holds (16 MiB). Source files and small fixtures fit; datasets are data blobs, not
/// source (D-05). Checked against `lstat` before the file is read.
pub(crate) const MAX_SNAPSHOT_FILE_BYTES: u64 = 16_777_216;

/// The most content a snapshot holds (64 MiB). The files map and the tarball are both in memory, and Phase 5's
/// input disk must grow past its 1 MiB job cap to carry the tarball (D-05).
pub(crate) const MAX_SNAPSHOT_BYTES: u64 = 67_108_864;

/// The largest `.minosignore` read for the negation check (64 KiB). An ignore file is a short list of patterns.
pub(crate) const MAX_IGNORE_FILE_BYTES: u64 = 65_536;

/// The most offenders, largest files, or secret hits an error lists; the rest are counted.
pub(crate) const SHOWN_OFFENDERS: usize = 20;

/// The custom ignore file name.
const MINOSIGNORE: &str = ".minosignore";

/// The UTF-8 byte order mark, which the ignore matcher strips from the first line of an ignore file.
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// The caps one walk enforces. Each is checked from `lstat` before a file is read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Caps {
    /// The most regular files, offenders included.
    pub(crate) files: u64,
    /// The largest single file.
    pub(crate) file_bytes: u64,
    /// The most content, summed over every packable file.
    pub(crate) total_bytes: u64,
}

/// What one walk drops and what it caps: the source snapshot's rules or the judge data's.
pub(crate) struct WalkRules {
    /// The case-folded paths the judge command names; each is dropped and reported with
    /// [`DenyRule::JudgePath`].
    judge_keys: Vec<String>,
    /// The manifest's `[source] exclude` entries, dropped silently.
    excludes: Vec<SourceExclude>,
    /// The caps.
    caps: Caps,
}

impl WalkRules {
    /// The source snapshot's rules: the judge paths and `[source] exclude` entries of `manifest`, and the
    /// `MAX_SNAPSHOT_*` caps (04-CONTEXT D-04, D-05).
    pub(crate) fn source(manifest: &Manifest) -> Self {
        Self {
            judge_keys: manifest
                .judge()
                .paths()
                .iter()
                .map(RelPath::ascii_case_key)
                .collect(),
            excludes: manifest.excludes().to_vec(),
            caps: Caps {
                files: MAX_SNAPSHOT_FILES,
                file_bytes: MAX_SNAPSHOT_FILE_BYTES,
                total_bytes: MAX_SNAPSHOT_BYTES,
            },
        }
    }

    /// The judge data's rules (07-CONTEXT D-03). No judge-path drop, because it would drop the judge program from
    /// its own data; no excludes, because a judge directory has no manifest; a file-count cap of
    /// [`MAX_JUDGE_FILES`]; and a per-file and total cap of [`MAX_JUDGE_DATA_BYTES`], the judge section of the
    /// input disk, which PID 1 reads whole (the pack checks the tarball length against the same cap).
    pub(crate) const fn judge() -> Self {
        Self {
            judge_keys: Vec::new(),
            excludes: Vec::new(),
            caps: Caps {
                files: MAX_JUDGE_FILES,
                file_bytes: MAX_JUDGE_DATA_BYTES,
                total_bytes: MAX_JUDGE_DATA_BYTES,
            },
        }
    }
}

/// One file to be packed: its exact bytes and whether any execute bit was set.
pub(crate) struct FileEntry {
    pub(crate) bytes: Vec<u8>,
    pub(crate) executable: bool,
}

/// Everything the walk keeps: the files to pack in path order, the denylist drops sorted by path, and the total
/// content length.
pub(crate) struct Collected {
    pub(crate) files: BTreeMap<RelPath, FileEntry>,
    pub(crate) denied: Vec<Denied>,
    pub(crate) content_bytes: u64,
}

/// Walks the tree at `root` under `rules` and the ignore files and hard denylist, and reads every file to be
/// packed.
///
/// # Errors
///
/// [`SnapshotError::Walk`] for a walk or ignore-file error, [`SnapshotError::Io`] for a failed `lstat`, open, or
/// read, [`SnapshotError::MinosignoreNegation`], [`SnapshotError::IgnoreFileTooLarge`],
/// [`SnapshotError::TooManyFiles`], [`SnapshotError::Offenders`] listing every offender, and
/// [`SnapshotError::TooLarge`] naming the largest files.
pub(crate) fn collect(root: &Path, rules: &WalkRules) -> Result<Collected, SnapshotError> {
    let (sender, receiver) = mpsc::channel();
    let lost = Arc::new(AtomicBool::new(false));
    let drops = Drops {
        sender,
        lost: Arc::clone(&lost),
    };
    let walk = walker(root, rules, drops);
    let mut state = State::new(rules.caps);
    for item in walk {
        let entry = item.map_err(|error| walk_error(&error))?;
        if let Some(error) = entry.error() {
            return Err(walk_error(error));
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| SnapshotError::Walk {
                message: SafeText::inline("the walk returned an entry outside the project root"),
            })?;
        let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
        if entry.depth() == 0 && !is_dir {
            return Err(SnapshotError::Walk {
                message: SafeText::inline("the project root is not a directory"),
            });
        }
        if is_dir {
            // Directories add nothing to a files-only tarball; each one's .minosignore is checked here, whether
            // or not the file itself is packed, because the matcher reads it either way.
            state.check_minosignore(entry.path(), rel)?;
            continue;
        }
        state.visit(entry.path(), rel)?;
    }
    // The walk, and with it the filter's sender, is gone, so try_iter sees every drop.
    if lost.load(Ordering::SeqCst) {
        return Err(SnapshotError::Walk {
            message: SafeText::inline("a denylist drop could not be recorded"),
        });
    }
    let mut denied: Vec<Denied> = receiver.try_iter().collect();
    denied.sort();
    state.finish(denied)
}

/// The walker: `standard_filters(false)` first, then exactly the sources D-04 names.
fn walker(root: &Path, rules: &WalkRules, denied: Drops) -> Walk {
    let mut walk = WalkBuilder::new(root);
    walk.standard_filters(false)
        .git_ignore(true)
        .require_git(false)
        .add_custom_ignore_filename(".minosignore");
    walk.hidden(false)
        .follow_links(false)
        .sort_by_file_path(Ord::cmp);
    let base = root.to_path_buf();
    let judge = rules.judge_keys.clone();
    let excludes = rules.excludes.clone();
    walk.filter_entry(move |entry| keep(&base, &judge, &excludes, &denied, entry));
    walk.build()
}

/// Whether the walk keeps `entry` (and descends into it, for a directory). A denylist name or a judge path is
/// dropped and reported; a `[source] exclude` entry is dropped silently, because the user wrote it.
fn keep(
    base: &Path,
    judge: &[String],
    excludes: &[SourceExclude],
    denied: &Drops,
    entry: &DirEntry,
) -> bool {
    let Ok(rel) = entry.path().strip_prefix(base) else {
        // The main loop refuses an entry outside the root.
        return true;
    };
    if let Some(rule) = deny_component(entry.file_name().as_bytes()) {
        denied.report(rel, rule);
        return false;
    }
    // A name outside the grammar is neither a judge path nor excluded; the main loop lists it as an offender.
    let Some(path) = rel.to_str().and_then(|text| RelPath::try_from(text).ok()) else {
        return true;
    };
    let key = path.ascii_case_key();
    if judge.iter().any(|judge| covers(judge, &key)) {
        denied.report(rel, DenyRule::JudgePath);
        return false;
    }
    !excludes.iter().any(|exclude| exclude.covers(&path))
}

/// Where the entry filter reports denylist drops. The filter must be `Send + Sync + 'static`, so it sends them
/// over a channel; a failed send sets `lost`, and `collect` then refuses the pack rather than drop a file silently.
struct Drops {
    sender: Sender<Denied>,
    lost: Arc<AtomicBool>,
}

impl Drops {
    /// Reports that the entry at `rel` was dropped by `rule`.
    fn report(&self, rel: &Path, rule: DenyRule) {
        let drop = Denied {
            path: shown(rel),
            rule,
        };
        if self.sender.send(drop).is_err() {
            self.lost.store(true, Ordering::SeqCst);
        }
    }
}

/// Whether the case-folded path `key` equals the case-folded judge path `judge` or lies under it.
fn covers(judge: &str, key: &str) -> bool {
    key.strip_prefix(judge)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// A walk or ignore-file error, shown through [`SafeText`] because it can carry a hostile path.
fn walk_error(error: &ignore::Error) -> SnapshotError {
    SnapshotError::Walk {
        message: SafeText::inline(&error.to_string()),
    }
}

/// The relative path as shown to the user.
fn shown(rel: &Path) -> SafeText {
    SafeText::inline(&rel.to_string_lossy())
}

/// An I/O error on the entry at `rel`.
fn io_error(op: &'static str, rel: &Path) -> impl FnOnce(io::Error) -> SnapshotError {
    let path = shown(rel);
    move |source| SnapshotError::Io { op, path, source }
}

/// The walk's running state.
struct State {
    /// The caps this walk enforces.
    caps: Caps,
    files: BTreeMap<RelPath, FileEntry>,
    /// The first [`SHOWN_OFFENDERS`] offenders in path order.
    offenders: BTreeSet<Offender>,
    /// Every offender found, shown or not.
    offender_count: u64,
    /// Regular files seen, offenders included.
    regular: u64,
    /// The summed length of every packable file, read or not.
    total: u64,
    /// Every packable file's length, so an over-total error can name the largest.
    sizes: Vec<(RelPath, u64)>,
    /// The summed length of the files read.
    content_bytes: u64,
}

impl State {
    /// An empty state under `caps`.
    const fn new(caps: Caps) -> Self {
        Self {
            caps,
            files: BTreeMap::new(),
            offenders: BTreeSet::new(),
            offender_count: 0,
            regular: 0,
            total: 0,
            sizes: Vec::new(),
            content_bytes: 0,
        }
    }

    /// Records one offender, keeping only the first [`SHOWN_OFFENDERS`] in path order.
    fn offend(&mut self, path: SafeText, kind: OffenderKind) {
        self.offender_count = self.offender_count.saturating_add(1);
        self.offenders.insert(Offender { path, kind });
        if self.offenders.len() > SHOWN_OFFENDERS {
            self.offenders.pop_last();
        }
    }

    /// Checks the `.minosignore` of the directory at `dir` (relative path `rel`), if it has one: a regular file
    /// within the size cap with no line starting with `!`.
    fn check_minosignore(&mut self, dir: &Path, rel: &Path) -> Result<(), SnapshotError> {
        let file = dir.join(MINOSIGNORE);
        let file_rel = rel.join(MINOSIGNORE);
        let meta = match fs::symlink_metadata(&file) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error("inspect", &file_rel)(error)),
        };
        // The matcher would follow a link or fail on a directory; neither is a .minosignore Minos can check.
        if meta.file_type().is_symlink() {
            self.offend(shown(&file_rel), OffenderKind::Symlink);
            return Ok(());
        }
        if !meta.is_file() {
            self.offend(shown(&file_rel), OffenderKind::Special);
            return Ok(());
        }
        let mut bytes = Vec::new();
        File::open(&file)
            .map_err(io_error("open", &file_rel))?
            .take(MAX_IGNORE_FILE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(io_error("read", &file_rel))?;
        if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_IGNORE_FILE_BYTES) {
            return Err(SnapshotError::IgnoreFileTooLarge {
                file: shown(&file_rel),
                max: MAX_IGNORE_FILE_BYTES,
            });
        }
        for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
            let line = if index == 0 {
                line.strip_prefix(UTF8_BOM).unwrap_or(line)
            } else {
                line
            };
            if line.first() == Some(&b'!') {
                return Err(SnapshotError::MinosignoreNegation {
                    file: shown(&file_rel),
                    line: u64::try_from(index).map_or(u64::MAX, |index| index.saturating_add(1)),
                });
            }
        }
        Ok(())
    }

    /// Classifies one non-directory entry and, if it is packable, reads it.
    fn visit(&mut self, abs: &Path, rel: &Path) -> Result<(), SnapshotError> {
        let meta = fs::symlink_metadata(abs).map_err(io_error("inspect", rel))?;
        if !meta.is_file() && rel.file_name().is_some_and(|name| name == MINOSIGNORE) {
            // Already listed by the directory's .minosignore check.
            return Ok(());
        }
        if meta.file_type().is_symlink() {
            self.offend(shown(rel), OffenderKind::Symlink);
            return Ok(());
        }
        if !meta.is_file() {
            self.offend(shown(rel), OffenderKind::Special);
            return Ok(());
        }
        self.regular = self.regular.saturating_add(1);
        if self.regular > self.caps.files {
            return Err(SnapshotError::TooManyFiles {
                max: self.caps.files,
            });
        }
        let Some(path) = rel.to_str().and_then(|text| RelPath::try_from(text).ok()) else {
            self.offend(shown(rel), OffenderKind::NotInGrammar);
            return Ok(());
        };
        if meta.nlink() > 1 {
            self.offend(shown(rel), OffenderKind::Hardlink);
            return Ok(());
        }
        if tar::Header::new_ustar().set_path(path.as_str()).is_err() {
            self.offend(shown(rel), OffenderKind::UstarPath);
            return Ok(());
        }
        let len = meta.len();
        if len > self.caps.file_bytes {
            self.offend(shown(rel), OffenderKind::Oversize { bytes: len });
            return Ok(());
        }
        self.total = self.total.saturating_add(len);
        self.sizes.push((path.clone(), len));
        if self.total > self.caps.total_bytes {
            // The pack fails; keep recording sizes so the error can name the largest files, but read no more.
            return Ok(());
        }
        match read_same(abs, rel, &meta)? {
            Some(file) => {
                self.content_bytes = self.content_bytes.saturating_add(len);
                self.files.insert(path, file);
            }
            None => self.offend(shown(rel), OffenderKind::Raced),
        }
        Ok(())
    }

    /// The collected files, or the error that lists every offender or the largest files.
    fn finish(self, denied: Vec<Denied>) -> Result<Collected, SnapshotError> {
        if !self.offenders.is_empty() {
            let shown: Vec<Offender> = self.offenders.into_iter().collect();
            let listed = u64::try_from(shown.len()).unwrap_or(u64::MAX);
            return Err(SnapshotError::Offenders {
                more: self.offender_count.saturating_sub(listed),
                shown,
            });
        }
        if self.total > self.caps.total_bytes {
            let mut largest = self.sizes;
            largest.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            largest.truncate(SHOWN_OFFENDERS);
            return Err(SnapshotError::TooLarge {
                bytes: self.total,
                max: self.caps.total_bytes,
                largest,
            });
        }
        Ok(Collected {
            files: self.files,
            denied,
            content_bytes: self.content_bytes,
        })
    }
}

/// Opens the file at `abs` and reads it, or returns `None` if it is no longer the regular file `lstat` saw (a
/// different device or inode, another type, or another length) or its length changed during the read.
fn read_same(abs: &Path, rel: &Path, seen: &Metadata) -> Result<Option<FileEntry>, SnapshotError> {
    let file = File::open(abs).map_err(io_error("open", rel))?;
    let opened = file.metadata().map_err(io_error("inspect", rel))?;
    let len = seen.len();
    if !opened.is_file()
        || opened.dev() != seen.dev()
        || opened.ino() != seen.ino()
        || opened.len() != len
    {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    file.take(len.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error("read", rel))?;
    if u64::try_from(bytes.len()).ok() != Some(len) {
        return Ok(None);
    }
    Ok(Some(FileEntry {
        bytes,
        executable: opened.permissions().mode() & 0o111 != 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::{MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_FILES};

    /// The largest tarball a snapshot can pack fits the guest's base section: every byte of content, plus one
    /// 512-byte header and at most 511 padding bytes for each file (rounded up to 1 KiB), plus the two end blocks.
    #[test]
    fn snapshot_tarball_fits_guest_base_cap() {
        let overhead = (MAX_SNAPSHOT_FILES + 2) * 1024;
        assert!(MAX_SNAPSHOT_BYTES + overhead <= minos_core::guest::MAX_BASE_TAR_BYTES);
    }
}
