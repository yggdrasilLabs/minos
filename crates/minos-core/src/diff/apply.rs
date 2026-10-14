//! `Tree` and the strict exact-position applier: a hunk applies only at its stated line with exact context.
//!
//! One applier serves the guest (Phase 5), `minos adopt` (Phase 8), and the report, so the bytes a candidate runs,
//! the bytes the user adopts, and the bytes the report shows cannot disagree (CORE-05).
//!
//! The rules (03-CONTEXT D-02, 03-RESEARCH Pattern 5):
//!
//! - A hunk with old-side length above zero starts at the 0-based base line `old_start - 1`. A hunk with old-side
//!   length zero inserts after line `old_start`, so at index `old_start`; `old_start` 0 inserts at the top.
//! - Every context and deleted line must equal the next base line byte for byte, including whether it ends with a
//!   newline. There is no offset search and no fuzz: a hunk that does not match at its stated line fails.
//! - Only a file's last line can lack a newline, so a hunk may not append after a line that lacks one, and a hunk
//!   whose new side ends without a newline must also end the base.
//! - The applier modifies files in place. It has no branch that creates or deletes a file, so the output tree has
//!   exactly the input tree's paths.
//!
//! A failure is an [`ApplyError`], which is a candidate fault ([`Reason::ApplyFailed`]), never an infrastructure
//! error and never retried. The work is linear in the file and diff sizes, and both are already capped by
//! [`Diff::parse`](super::Diff::parse).

use std::collections::BTreeMap;
use std::fmt;

use super::{Diff, Hunk, HunkLine};
use crate::outcome::Reason;
use crate::path::RelPath;

/// An in-memory surface tree: file contents by path, in path order.
///
/// The guest builds it from the base tarball (Phase 5) and the report builds it from surface blobs. Contents are
/// bytes, so a file need not be UTF-8 to be carried; only the lines a diff touches are compared. Debug prints paths
/// and byte lengths only, never content.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Tree {
    files: BTreeMap<RelPath, Vec<u8>>,
}

impl Tree {
    /// An empty tree.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    /// Sets the contents of `path`, returning the previous contents if it was present.
    pub fn insert(&mut self, path: RelPath, bytes: Vec<u8>) -> Option<Vec<u8>> {
        self.files.insert(path, bytes)
    }

    /// The contents of `path`, if present.
    #[must_use]
    pub fn get(&self, path: &RelPath) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Whether `path` is present.
    #[must_use]
    pub fn contains(&self, path: &RelPath) -> bool {
        self.files.contains_key(path)
    }

    /// Paths and contents, in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&RelPath, &[u8])> {
        self.files
            .iter()
            .map(|(path, bytes)| (path, bytes.as_slice()))
    }

    /// The number of files.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the tree has no files.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tree ")?;
        f.debug_map()
            .entries(
                self.files
                    .iter()
                    .map(|(path, bytes)| (path.as_str(), bytes.len())),
            )
            .finish()
    }
}

/// Why a diff did not apply to a tree. A candidate fault: the experiment is rejected, never retried.
///
/// Every field is a validated path or a number; no variant carries file content or diff text.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    /// The diff modifies a file the tree does not have.
    #[error("{path} is not in the base tree")]
    MissingFile {
        /// The missing file.
        path: RelPath,
    },
    /// A context or deleted line differs from the base line at the hunk's stated position.
    #[error("hunk {hunk} of {path} does not match base line {line}")]
    ContextMismatch {
        /// The file.
        path: RelPath,
        /// 1-based hunk number within the file.
        hunk: u32,
        /// 1-based base line that differs.
        line: u64,
    },
    /// The hunk starts before the previous hunk ended or past the end of the file, runs past the end of the file,
    /// appends after a last line that lacks its newline, or ends the file before the base ends.
    #[error("hunk {hunk} of {path} is out of range")]
    OutOfRange {
        /// The file.
        path: RelPath,
        /// 1-based hunk number within the file.
        hunk: u32,
    },
}

impl ApplyError {
    /// The rejection reason recorded in the ledger: [`Reason::ApplyFailed`], with hunk 0 for a missing file.
    #[must_use]
    pub fn reason(&self) -> Reason {
        match self {
            Self::MissingFile { path } => Reason::ApplyFailed {
                path: path.clone(),
                hunk: 0,
            },
            Self::ContextMismatch { path, hunk, .. } | Self::OutOfRange { path, hunk } => {
                Reason::ApplyFailed {
                    path: path.clone(),
                    hunk: *hunk,
                }
            }
        }
    }
}

/// Applies `diff` to `tree` at exact positions and returns the new tree; `tree` is unchanged.
///
/// See the [module docs](self) for the rules.
///
/// # Errors
///
/// Returns [`ApplyError::MissingFile`] if the diff modifies a file the tree lacks,
/// [`ApplyError::ContextMismatch`] if a context or deleted line differs from the base, and
/// [`ApplyError::OutOfRange`] if a hunk's position does not fit the base.
pub fn apply(tree: &Tree, diff: &Diff) -> Result<Tree, ApplyError> {
    let mut out = tree.clone();
    for file in diff.files() {
        let path = file.path();
        let base = tree
            .get(path)
            .ok_or_else(|| ApplyError::MissingFile { path: path.clone() })?;
        let bytes = apply_file(base, file.hunks()).map_err(|failure| failure.into_error(path))?;
        out.insert(path.clone(), bytes);
    }
    Ok(out)
}

/// A failure inside one file, before the path is attached.
enum Failure {
    Mismatch { hunk: u32, line: u64 },
    OutOfRange { hunk: u32 },
}

impl Failure {
    fn into_error(self, path: &RelPath) -> ApplyError {
        match self {
            Self::Mismatch { hunk, line } => ApplyError::ContextMismatch {
                path: path.clone(),
                hunk,
                line,
            },
            Self::OutOfRange { hunk } => ApplyError::OutOfRange {
                path: path.clone(),
                hunk,
            },
        }
    }
}

/// The output of one file. Once a line without a newline is written, nothing may follow it.
struct Output {
    bytes: Vec<u8>,
    closed: bool,
}

impl Output {
    /// Appends one line, given as content without its newline plus whether it has one.
    fn push(&mut self, content: &[u8], newline: bool, hunk: u32) -> Result<(), Failure> {
        if self.closed {
            return Err(Failure::OutOfRange { hunk });
        }
        self.bytes.extend_from_slice(content);
        if newline {
            self.bytes.push(b'\n');
        } else {
            self.closed = true;
        }
        Ok(())
    }

    /// Appends one base line, which keeps its newline if it has one.
    fn push_base(&mut self, line: &[u8], hunk: u32) -> Result<(), Failure> {
        let (content, newline) = split_newline(line);
        self.push(content, newline, hunk)
    }
}

/// A line's content without its trailing newline, and whether it had one.
fn split_newline(line: &[u8]) -> (&[u8], bool) {
    line.strip_suffix(b"\n")
        .map_or((line, false), |content| (content, true))
}

/// Whether a base line equals a hunk line, byte for byte and in newline presence.
fn same_line(base: &[u8], line: &HunkLine) -> bool {
    let (content, newline) = split_newline(base);
    content == line.text().as_bytes() && newline == line.has_newline()
}

/// A 1-based number for a 0-based index; saturates, although every count is capped far below `u32::MAX`.
fn one_based_u32(index: usize) -> u32 {
    u32::try_from(index).map_or(u32::MAX, |number| number.saturating_add(1))
}

fn one_based_u64(index: usize) -> u64 {
    u64::try_from(index).map_or(u64::MAX, |number| number.saturating_add(1))
}

/// The 0-based base index where a hunk's old side begins.
fn start_index(hunk: &Hunk) -> Option<usize> {
    let start = usize::try_from(hunk.old_start()).ok()?;
    if hunk.old_len() > 0 {
        start.checked_sub(1)
    } else {
        Some(start)
    }
}

/// Applies the hunks of one file to its base bytes.
fn apply_file(base: &[u8], hunks: &[Hunk]) -> Result<Vec<u8>, Failure> {
    let lines: Vec<&[u8]> = base.split_inclusive(|byte| *byte == b'\n').collect();
    let mut out = Output {
        bytes: Vec::with_capacity(base.len()),
        closed: false,
    };
    let mut cursor = 0_usize;
    let mut number = 0_u32;
    for (position, hunk) in hunks.iter().enumerate() {
        number = one_based_u32(position);
        let start = start_index(hunk)
            .filter(|start| *start >= cursor && *start <= lines.len())
            .ok_or(Failure::OutOfRange { hunk: number })?;
        for line in lines.get(cursor..start).into_iter().flatten() {
            out.push_base(line, number)?;
        }
        cursor = start;
        for line in hunk.lines() {
            match line {
                HunkLine::Context(_) | HunkLine::Delete(_) => {
                    let base_line = lines
                        .get(cursor)
                        .ok_or(Failure::OutOfRange { hunk: number })?;
                    if !same_line(base_line, line) {
                        return Err(Failure::Mismatch {
                            hunk: number,
                            line: one_based_u64(cursor),
                        });
                    }
                    cursor += 1;
                    if matches!(line, HunkLine::Context(_)) {
                        out.push(line.text().as_bytes(), line.has_newline(), number)?;
                    }
                }
                HunkLine::Insert(_) => {
                    out.push(line.text().as_bytes(), line.has_newline(), number)?;
                }
            }
        }
    }
    for line in lines.get(cursor..).into_iter().flatten() {
        out.push_base(line, number)?;
    }
    Ok(out.bytes)
}
