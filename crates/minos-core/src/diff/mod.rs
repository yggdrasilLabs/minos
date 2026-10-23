//! The canonical `Diff` and its parse pipeline.
//!
//! [`Diff::parse`] is the only constructor of [`Diff`]. Diff text is model output and hostile, so diffy's parser is
//! one stage of several (03-RESEARCH Pattern 4):
//!
//! 1. Byte caps before any parsing: non-empty, at most [`Limits::max_diff_bytes`], no NUL and no CR byte (the
//!    canonical form is LF only, so a CRLF surface file cannot be edited in v1), a final newline, and at most
//!    [`Limits::max_diff_lines`] lines.
//! 2. Our own line grammar. Every section is exactly `diff --git a/P b/P`, an optional `index` line, `--- a/P`, and
//!    `+++ b/P` with the same `P` byte for byte, then one or more hunks. The hunk header counts decide how many body
//!    lines follow, so a deleted line that reads `--- a/...` is never mistaken for a file header. Every git extended
//!    header (rename, copy, mode, similarity, binary) is a [`DiffError::ForbiddenHeader`], and every other line the
//!    grammar does not expect (prose, quoted paths, timestamps, trailing text) is a [`DiffError::UnexpectedLine`].
//! 3. `diffy`'s git-diff parser.
//! 4. Per-file semantic checks on what diffy read: an in-place modify of the section's path, every path in the
//!    writable set byte for byte (03-CONTEXT D-01), each path once, no hunk that changes nothing (D-03), and new-side
//!    starts that follow from the old-side starts.
//! 5. Line accounting: diffy's reading must account for every input line, so nothing it skipped can hide in the
//!    text.
//! 6. The canonical form (see [`Diff::as_str`]): sections sorted by path, explicit hunk lengths, no `index` line,
//!    no function context, and no timestamps. Everything a model wrote outside the lines being changed is dropped,
//!    so the bytes [`ExperimentId::of`](crate::id::ExperimentId::of) hashes are ours.
//! 7. A round-trip check: stages 1 to 5 run again on the canonical text and must give back the same files.
//!
//! A `/dev/null` side (create or delete), a rename, a copy, a mode change, a symlink or gitlink mode, and a binary
//! patch are all rejected (D-02: a diff may only modify an existing writable file), so an experiment that edits the
//! judge, or leaves a hole where the judge expects a file, cannot be represented.

use std::collections::BTreeSet;
use std::fmt;

use diffy::patch_set::{FileOperation, ParseOptions, PatchSet};

use crate::limits::Limits;
use crate::path::{PathError, RelPath};

pub mod apply;

pub mod compose;

pub mod cumulative;

/// The exact marker that strips the newline from the body line before it.
const NO_NEWLINE_MARKER: &str = "\\ No newline at end of file";

/// Shortest abbreviated object name accepted on an `index` line (git's default abbreviation).
const MIN_INDEX_HASH: usize = 7;
/// Longest object name accepted on an `index` line (a full SHA-256 object name).
const MAX_INDEX_HASH: usize = 64;
/// Longest decimal number accepted in a hunk header: `u32::MAX` has ten digits.
const MAX_HUNK_NUMBER_DIGITS: usize = 10;

/// Git extended headers, in the order they are checked. Each one describes a change other than an in-place modify.
const FORBIDDEN_PREFIXES: [(&str, ForbiddenHeader); 12] = [
    ("rename from", ForbiddenHeader::Rename),
    ("rename to", ForbiddenHeader::Rename),
    ("copy from", ForbiddenHeader::Copy),
    ("copy to", ForbiddenHeader::Copy),
    ("old mode", ForbiddenHeader::OldMode),
    ("new mode", ForbiddenHeader::NewMode),
    ("new file mode", ForbiddenHeader::NewFileMode),
    ("deleted file mode", ForbiddenHeader::DeletedFileMode),
    ("similarity index", ForbiddenHeader::Similarity),
    ("dissimilarity index", ForbiddenHeader::Dissimilarity),
    ("Binary files", ForbiddenHeader::Binary),
    ("GIT binary patch", ForbiddenHeader::GitBinaryPatch),
];

/// A git header line that describes something other than an in-place modify of a regular file.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForbiddenHeader {
    /// `rename from` or `rename to`.
    Rename,
    /// `copy from` or `copy to`.
    Copy,
    /// `old mode`.
    OldMode,
    /// `new mode`.
    NewMode,
    /// `new file mode`: a create.
    NewFileMode,
    /// `deleted file mode`: a delete.
    DeletedFileMode,
    /// `similarity index`, which accompanies a rename or copy.
    Similarity,
    /// `dissimilarity index`, which accompanies a rewrite.
    Dissimilarity,
    /// `Binary files ... differ`.
    Binary,
    /// `GIT binary patch`.
    GitBinaryPatch,
    /// An `index` line with mode `120000`: a symlink.
    Symlink,
    /// An `index` line with mode `160000`: a submodule.
    Gitlink,
}

impl fmt::Display for ForbiddenHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rename => "rename",
            Self::Copy => "copy",
            Self::OldMode => "old mode",
            Self::NewMode => "new mode",
            Self::NewFileMode => "new file mode",
            Self::DeletedFileMode => "deleted file mode",
            Self::Similarity => "similarity index",
            Self::Dissimilarity => "dissimilarity index",
            Self::Binary => "binary files",
            Self::GitBinaryPatch => "GIT binary patch",
            Self::Symlink => "symlink mode 120000",
            Self::Gitlink => "gitlink mode 160000",
        })
    }
}

/// Why a diff was rejected.
///
/// Every variant is a candidate fault: the proposal is dropped, never retried. No variant carries input text. They
/// carry 1-based line numbers, counts, bytes, and validated [`RelPath`] values only, so hostile diff text cannot
/// reach a terminal or a log through an error (03-RESEARCH Pitfall 10).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiffError {
    /// The text is empty.
    #[error("the diff is empty")]
    Empty,
    /// The text, or its canonical form, is over [`Limits::max_diff_bytes`], or a number in it does not fit.
    #[error("the diff is {bytes} bytes, over the cap")]
    TooLarge {
        /// Size of the text, in bytes.
        bytes: usize,
    },
    /// The text has more than [`Limits::max_diff_lines`] lines.
    #[error("the diff has {lines} lines, over the cap")]
    TooManyLines {
        /// Number of lines in the text.
        lines: usize,
    },
    /// The text contains a NUL or a CR byte. The canonical form is LF only.
    #[error("line {line}: byte {byte:#04x} is not allowed in a diff")]
    ForbiddenByte {
        /// 1-based line of the first offending byte.
        line: usize,
        /// The offending byte.
        byte: u8,
    },
    /// The text does not end with a newline.
    #[error("the diff does not end with a newline")]
    MissingFinalNewline,
    /// A line the grammar does not expect at that position: prose, a quoted path, a timestamp, trailing text, a
    /// malformed hunk header, or a body line the hunk header does not count.
    #[error("line {line}: unexpected line")]
    UnexpectedLine {
        /// 1-based line number.
        line: usize,
    },
    /// A git extended header for a change other than an in-place modify.
    #[error("line {line}: forbidden header: {header}")]
    ForbiddenHeader {
        /// 1-based line number.
        line: usize,
        /// Which header.
        header: ForbiddenHeader,
    },
    /// A `--- /dev/null` (create) or `+++ /dev/null` (delete) side. A diff may only modify an existing file (D-02).
    #[error(
        "line {line}: a /dev/null side creates or deletes a file; a diff may only modify an existing file"
    )]
    DevNull {
        /// 1-based line number.
        line: usize,
    },
    /// A header line names a different path from the section's `diff --git a/P b/P` line, or that line names two
    /// different paths.
    #[error("line {line}: the path does not match the section's path")]
    PathMismatch {
        /// 1-based line number.
        line: usize,
    },
    /// A section's path is not a valid [`RelPath`].
    #[error("line {line}: invalid path: {error}")]
    InvalidPath {
        /// 1-based line number.
        line: usize,
        /// Why the path was rejected.
        error: PathError,
    },
    /// The text is not a complete diff (for example a truncated hunk), diffy rejected it, or diffy's reading of it
    /// disagrees with the line grammar.
    #[error("the diff is malformed")]
    Malformed,
    /// A touched path is not byte-equal to any writable entry (D-01).
    #[error("{path} is not writable")]
    NotWritable {
        /// The touched path.
        path: RelPath,
    },
    /// The same path has more than one section.
    #[error("{path} has more than one section")]
    DuplicateFile {
        /// The repeated path.
        path: RelPath,
    },
    /// The diff has more than [`Limits::max_diff_files`] sections.
    #[error("the diff touches more than {max} files")]
    TooManyFiles {
        /// The cap.
        max: usize,
    },
    /// A section has no hunks, so it changes nothing (D-03).
    #[error("{path} has a section with no hunks")]
    NoHunks {
        /// The section's path.
        path: RelPath,
    },
    /// A section has more than [`Limits::max_hunks_per_file`] hunks.
    #[error("{path} has more than {max} hunks")]
    TooManyHunks {
        /// The section's path.
        path: RelPath,
        /// The cap.
        max: usize,
    },
    /// A hunk's old-side lines equal its new-side lines, including newline presence, so it changes nothing (D-03).
    #[error("{path} hunk {hunk} changes nothing")]
    NoOpHunk {
        /// The section's path.
        path: RelPath,
        /// 1-based hunk number within the section.
        hunk: usize,
    },
    /// A hunk's new-side start is not its old-side start plus the lines inserted minus the lines deleted by the
    /// hunks before it, so two appliers could disagree about where it goes.
    #[error("{path} hunk {hunk}: the new-side start does not follow from the old-side start")]
    InconsistentNewStart {
        /// The section's path.
        path: RelPath,
        /// 1-based hunk number within the section.
        hunk: usize,
    },
    /// The canonical form did not read back to the same files. This is a defect in the serializer, never a
    /// property of the input; the diff is still rejected, never accepted.
    #[error("the canonical form did not re-parse to the same diff")]
    CanonicalRoundTrip,
    /// diffy's reading of the text does not account for every line, so it skipped something.
    #[error("the parsed diff accounts for {expected} lines but the text has {found}")]
    LineAccounting {
        /// Lines the parsed sections account for.
        expected: usize,
        /// Lines in the text.
        found: usize,
    },
}

/// One line of a hunk body.
///
/// The string holds the line's content including its trailing `\n`, unless the line lacked its newline (a
/// `\ No newline at end of file` marker followed it in the diff). Debug prints the kind and length only.
#[derive(Clone, PartialEq, Eq)]
pub enum HunkLine {
    /// A line present on both sides.
    Context(String),
    /// A line only on the old side.
    Delete(String),
    /// A line only on the new side.
    Insert(String),
}

impl HunkLine {
    /// The line's content, including its trailing newline when it has one.
    fn raw(&self) -> &str {
        match self {
            Self::Context(text) | Self::Delete(text) | Self::Insert(text) => text,
        }
    }

    /// The line's content without its trailing newline.
    #[must_use]
    pub fn text(&self) -> &str {
        let raw = self.raw();
        raw.strip_suffix('\n').unwrap_or(raw)
    }

    /// Whether the line ends with a newline. Only the last line of a file can lack one.
    #[must_use]
    pub fn has_newline(&self) -> bool {
        self.raw().ends_with('\n')
    }

    /// Whether the line is on the old side (context or delete).
    const fn is_old_side(&self) -> bool {
        matches!(self, Self::Context(_) | Self::Delete(_))
    }

    /// Whether the line is on the new side (context or insert).
    const fn is_new_side(&self) -> bool {
        matches!(self, Self::Context(_) | Self::Insert(_))
    }
}

impl fmt::Debug for HunkLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Context(_) => "Context",
            Self::Delete(_) => "Delete",
            Self::Insert(_) => "Insert",
        };
        f.debug_struct(kind)
            .field("bytes", &self.raw().len())
            .finish()
    }
}

/// One hunk: the old-side and new-side ranges from its header and its body lines.
///
/// A start is the 1-based line number written in the header. For a side of length zero it is the line after which
/// the change sits (`-4,0` inserts after line 4). Debug prints the ranges and the line count only.
#[derive(Clone, PartialEq, Eq)]
pub struct Hunk {
    old_start: u32,
    old_len: u32,
    new_start: u32,
    new_len: u32,
    lines: Vec<HunkLine>,
}

impl Hunk {
    /// The old-side start.
    #[must_use]
    pub const fn old_start(&self) -> u32 {
        self.old_start
    }

    /// The number of old-side lines (context plus delete).
    #[must_use]
    pub const fn old_len(&self) -> u32 {
        self.old_len
    }

    /// The new-side start.
    #[must_use]
    pub const fn new_start(&self) -> u32 {
        self.new_start
    }

    /// The number of new-side lines (context plus insert).
    #[must_use]
    pub const fn new_len(&self) -> u32 {
        self.new_len
    }

    /// The body lines, in order.
    #[must_use]
    pub fn lines(&self) -> &[HunkLine] {
        &self.lines
    }

    /// Bytes of line content in the body.
    fn content_bytes(&self) -> usize {
        self.lines.iter().map(|line| line.raw().len()).sum()
    }
}

impl fmt::Debug for Hunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hunk")
            .field("old_start", &self.old_start)
            .field("old_len", &self.old_len)
            .field("new_start", &self.new_start)
            .field("new_len", &self.new_len)
            .field("lines", &self.lines.len())
            .finish()
    }
}

/// The hunks that modify one writable file. Debug prints the path, the hunk count, and the content length only.
#[derive(Clone, PartialEq, Eq)]
pub struct FileDiff {
    path: RelPath,
    hunks: Vec<Hunk>,
}

impl FileDiff {
    /// The modified file.
    #[must_use]
    pub const fn path(&self) -> &RelPath {
        &self.path
    }

    /// The hunks, in file order. Never empty.
    #[must_use]
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }
}

impl fmt::Debug for FileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDiff")
            .field("path", &self.path.as_str())
            .field("hunks", &self.hunks.len())
            .field(
                "bytes",
                &self.hunks.iter().map(Hunk::content_bytes).sum::<usize>(),
            )
            .finish()
    }
}

/// A validated diff: in-place modifies of writable files only, each changing something.
///
/// If a value exists, it passed every stage of [`Diff::parse`]. Debug prints paths, hunk counts, and lengths only,
/// never content.
#[derive(Clone, PartialEq, Eq)]
pub struct Diff {
    files: Vec<FileDiff>,
    canonical: String,
}

impl fmt::Debug for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diff")
            .field("files", &self.files)
            .field("bytes", &self.canonical.len())
            .finish()
    }
}

impl Diff {
    /// Parses and validates hostile diff text against the writable set.
    ///
    /// `writable` comes from `Manifest::writable()`; the diff never needs the rest of the manifest.
    ///
    /// # Errors
    ///
    /// Returns a [`DiffError`] for any text that is not a well-formed, in-place modify of writable files that
    /// changes something, or that exceeds a cap in `limits`. See the [module docs](self) for the stages.
    pub fn parse(
        text: &str,
        writable: &BTreeSet<RelPath>,
        limits: &Limits,
    ) -> Result<Self, DiffError> {
        let mut files = read(text, writable, limits)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let canonical = serialize(&files);
        // Stage 7: the canonical bytes must read back to the same files under the same caps, so any consumer that
        // re-parses them (the report, a resumed run) sees exactly this diff. A canonical form over the byte cap is
        // `TooLarge`; any other failure is a defect in the serializer, still rejected.
        let again = read(&canonical, writable, limits).map_err(|error| match error {
            DiffError::TooLarge { .. } => error,
            _ => DiffError::CanonicalRoundTrip,
        })?;
        if again != files {
            return Err(DiffError::CanonicalRoundTrip);
        }
        Ok(Self { files, canonical })
    }

    /// The canonical text: sections sorted by path, each `diff --git a/P b/P`, `--- a/P`, `+++ b/P`, then hunks
    /// as `@@ -a,b +c,d @@` with both lengths and no function context, LF only, and `\ No newline at end of file`
    /// after a line that lacks its newline. No `index` line and no timestamps. Accepted by `git apply` and
    /// `patch -p1`, and `Diff::parse` of it gives this diff back.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    /// The canonical text as bytes: what [`ExperimentId::of`](crate::id::ExperimentId::of) hashes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.canonical.as_bytes()
    }

    /// Length of the canonical text, in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.canonical.len()
    }

    /// Always false: a parsed diff changes at least one line (03-CONTEXT D-03, D-04). The baseline, which has no
    /// diff, is `ExperimentId::baseline`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.canonical.is_empty()
    }

    /// The modified files, sorted by path.
    #[must_use]
    pub fn files(&self) -> &[FileDiff] {
        &self.files
    }

    /// The modified paths, sorted.
    pub fn paths(&self) -> impl Iterator<Item = &RelPath> {
        self.files.iter().map(FileDiff::path)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Stage 1: bytes and caps
// ---------------------------------------------------------------------------------------------------------------

/// Checks the byte caps and forbidden bytes, and returns the line count.
fn check_bytes(text: &str, limits: &Limits) -> Result<usize, DiffError> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return Err(DiffError::Empty);
    }
    if bytes.len() > limits.max_diff_bytes() {
        return Err(DiffError::TooLarge { bytes: bytes.len() });
    }
    if let Some((index, byte)) = bytes
        .iter()
        .copied()
        .enumerate()
        .find(|(_, byte)| matches!(byte, b'\0' | b'\r'))
    {
        // NUL and CR are ASCII, so `index` is a char boundary.
        let line = text
            .get(..index)
            .map_or(0, |before| before.matches('\n').count())
            + 1;
        return Err(DiffError::ForbiddenByte { line, byte });
    }
    if !bytes.ends_with(b"\n") {
        return Err(DiffError::MissingFinalNewline);
    }
    let lines = text.matches('\n').count();
    if lines > limits.max_diff_lines() {
        return Err(DiffError::TooManyLines { lines });
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------------------------------------------
// Stage 2: the line grammar
// ---------------------------------------------------------------------------------------------------------------

/// What the grammar learned about one section, for the cross-checks in stages 4 and 5.
struct Shape {
    path: RelPath,
    has_index: bool,
    hunks: usize,
}

/// Which kind of body line came last, for the no-newline marker.
#[derive(Clone, Copy)]
enum Kind {
    Context,
    Delete,
    Insert,
}

/// What the grammar expects next.
enum Expect {
    /// The first line of the text.
    Start,
    /// After `diff --git`: an `index` line or `--- a/P`.
    IndexOrOld,
    /// After `index`: `--- a/P`.
    Old,
    /// After `--- a/P`: `+++ b/P`.
    New,
    /// After `+++ b/P`: the first hunk header.
    FirstHunk,
    /// Inside a hunk, with the body lines its header still counts.
    Body {
        old_left: u32,
        new_left: u32,
        last: Option<Kind>,
    },
}

/// Per-section grammar state.
struct Section {
    shape: Shape,
    /// Whether the old side may still have lines: false after its last line lacked a newline.
    old_open: bool,
    /// Whether the new side may still have lines.
    new_open: bool,
}

/// The grammar's running state over the whole text.
struct Grammar<'a> {
    limits: &'a Limits,
    expect: Expect,
    done: Vec<Shape>,
    current: Option<Section>,
}

/// Maps a line the grammar did not expect to a forbidden header when it is one, or to an unexpected line.
fn unexpected(line: usize, body: &str) -> DiffError {
    FORBIDDEN_PREFIXES
        .iter()
        .find(|(prefix, _)| body.starts_with(prefix))
        .map_or(DiffError::UnexpectedLine { line }, |(_, header)| {
            DiffError::ForbiddenHeader {
                line,
                header: *header,
            }
        })
}

/// A hunk-header number: ASCII digits, no sign, no leading zero, at most ten digits, within `u32`.
fn hunk_number(text: &str) -> Option<u32> {
    let well_formed = !text.is_empty()
        && text.len() <= MAX_HUNK_NUMBER_DIGITS
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if well_formed { text.parse().ok() } else { None }
}

/// One side of a hunk header, `start[,len]`. An omitted length means 1. A non-empty side cannot start at 0.
fn hunk_range(text: &str) -> Option<(u32, u32)> {
    let (start, len) = match text.split_once(',') {
        Some((start, len)) => (hunk_number(start)?, hunk_number(len)?),
        None => (hunk_number(text)?, 1),
    };
    if len > 0 && start == 0 {
        None
    } else {
        Some((start, len))
    }
}

/// `@@ -a[,b] +c[,d] @@`, optionally followed by a space and function context, which is dropped.
fn hunk_header(body: &str) -> Option<((u32, u32), (u32, u32))> {
    let rest = body.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, tail) = rest.split_once(" @@")?;
    if !(tail.is_empty() || tail.starts_with(' ')) {
        return None;
    }
    Some((hunk_range(old)?, hunk_range(new)?))
}

/// An abbreviated or full object name on an `index` line: 7 to 64 lowercase hex characters.
fn index_hash(text: &str) -> bool {
    (MIN_INDEX_HASH..=MAX_INDEX_HASH).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// `index <hash>..<hash>[ <mode>]`, where the mode is a regular or executable file.
fn check_index(line: usize, rest: &str) -> Result<(), DiffError> {
    let (hashes, mode) = match rest.split_once(' ') {
        Some((hashes, mode)) => (hashes, Some(mode)),
        None => (rest, None),
    };
    let hashes_ok = hashes
        .split_once("..")
        .is_some_and(|(old, new)| index_hash(old) && index_hash(new));
    if !hashes_ok {
        return Err(DiffError::UnexpectedLine { line });
    }
    match mode {
        None | Some("100644" | "100755") => Ok(()),
        Some("120000") => Err(DiffError::ForbiddenHeader {
            line,
            header: ForbiddenHeader::Symlink,
        }),
        Some("160000") => Err(DiffError::ForbiddenHeader {
            line,
            header: ForbiddenHeader::Gitlink,
        }),
        Some(_) => Err(DiffError::UnexpectedLine { line }),
    }
}

/// A `--- a/P` or `+++ b/P` line. `marker` is `---` or `+++` and `prefix` is `a/` or `b/`.
fn check_file_header(
    line: usize,
    body: &str,
    marker: &str,
    prefix: &str,
    path: &RelPath,
) -> Result<(), DiffError> {
    let Some(rest) = body
        .strip_prefix(marker)
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return Err(unexpected(line, body));
    };
    if rest == "/dev/null" {
        return Err(DiffError::DevNull { line });
    }
    let Some(named) = rest.strip_prefix(prefix) else {
        return Err(DiffError::UnexpectedLine { line });
    };
    if named == path.as_str() {
        return Ok(());
    }
    // The section's path followed by a tab or space is a timestamp or trailing text, not a different path.
    let trailing = named
        .strip_prefix(path.as_str())
        .is_some_and(|tail| tail.starts_with(['\t', ' ']));
    if trailing || named.contains(['\t', '"']) {
        Err(DiffError::UnexpectedLine { line })
    } else {
        Err(DiffError::PathMismatch { line })
    }
}

impl<'a> Grammar<'a> {
    const fn new(limits: &'a Limits) -> Self {
        Self {
            limits,
            expect: Expect::Start,
            done: Vec::new(),
            current: None,
        }
    }

    /// The section in progress. Every state after `Start` has one.
    fn section(&mut self) -> Result<&mut Section, DiffError> {
        self.current.as_mut().ok_or(DiffError::Malformed)
    }

    fn feed(&mut self, line: usize, body: &str) -> Result<(), DiffError> {
        match self.expect {
            Expect::Start => self.start_section(line, body),
            Expect::IndexOrOld => {
                if let Some(rest) = body.strip_prefix("index ") {
                    check_index(line, rest)?;
                    self.section()?.shape.has_index = true;
                    self.expect = Expect::Old;
                    Ok(())
                } else {
                    self.old_header(line, body)
                }
            }
            Expect::Old => self.old_header(line, body),
            Expect::New => {
                let section = self.section()?;
                check_file_header(line, body, "+++", "b/", &section.shape.path)?;
                self.expect = Expect::FirstHunk;
                Ok(())
            }
            Expect::FirstHunk => {
                if body.starts_with("diff --git ") {
                    let path = self.section()?.shape.path.clone();
                    return Err(DiffError::NoHunks { path });
                }
                self.start_hunk(line, body)
            }
            Expect::Body {
                old_left,
                new_left,
                last,
            } => self.body(line, body, old_left, new_left, last),
        }
    }

    fn old_header(&mut self, line: usize, body: &str) -> Result<(), DiffError> {
        let section = self.section()?;
        check_file_header(line, body, "---", "a/", &section.shape.path)?;
        self.expect = Expect::New;
        Ok(())
    }

    /// `diff --git a/P b/P`, with the same `P` on both sides.
    fn start_section(&mut self, line: usize, body: &str) -> Result<(), DiffError> {
        let Some(rest) = body.strip_prefix("diff --git a/") else {
            return Err(unexpected(line, body));
        };
        let Some((named, _)) = rest.split_once(' ') else {
            return Err(DiffError::UnexpectedLine { line });
        };
        let path =
            RelPath::try_from(named).map_err(|error| DiffError::InvalidPath { line, error })?;
        let other_side = format!(" b/{named}");
        if rest.strip_prefix(named) != Some(other_side.as_str()) {
            return Err(DiffError::PathMismatch { line });
        }
        if let Some(section) = self.current.take() {
            self.done.push(section.shape);
        }
        if self.done.len() >= self.limits.max_diff_files() {
            return Err(DiffError::TooManyFiles {
                max: self.limits.max_diff_files(),
            });
        }
        self.current = Some(Section {
            shape: Shape {
                path,
                has_index: false,
                hunks: 0,
            },
            old_open: true,
            new_open: true,
        });
        self.expect = Expect::IndexOrOld;
        Ok(())
    }

    fn start_hunk(&mut self, line: usize, body: &str) -> Result<(), DiffError> {
        let Some(((_, old_len), (_, new_len))) = hunk_header(body) else {
            return Err(unexpected(line, body));
        };
        let max = self.limits.max_hunks_per_file();
        let section = self.section()?;
        // A side whose last line lacked its newline has reached the end of the file: no hunk can follow.
        if !(section.old_open && section.new_open) {
            return Err(DiffError::UnexpectedLine { line });
        }
        section.shape.hunks += 1;
        if section.shape.hunks > max {
            return Err(DiffError::TooManyHunks {
                path: section.shape.path.clone(),
                max,
            });
        }
        self.expect = Expect::Body {
            old_left: old_len,
            new_left: new_len,
            last: None,
        };
        Ok(())
    }

    fn body(
        &mut self,
        line: usize,
        body: &str,
        old_left: u32,
        new_left: u32,
        last: Option<Kind>,
    ) -> Result<(), DiffError> {
        if body == NO_NEWLINE_MARKER {
            let Some(kind) = last else {
                return Err(DiffError::UnexpectedLine { line });
            };
            let section = self.section()?;
            match kind {
                Kind::Context => {
                    section.old_open = false;
                    section.new_open = false;
                }
                Kind::Delete => section.old_open = false,
                Kind::Insert => section.new_open = false,
            }
            self.expect = Expect::Body {
                old_left,
                new_left,
                last: None,
            };
            return Ok(());
        }
        if old_left == 0 && new_left == 0 {
            return if body.starts_with("diff --git ") {
                self.start_section(line, body)
            } else {
                self.start_hunk(line, body)
            };
        }
        let section = self.section()?;
        let (old_open, new_open) = (section.old_open, section.new_open);
        let next = match body.as_bytes().first() {
            None | Some(b' ') if old_left > 0 && new_left > 0 && old_open && new_open => {
                (old_left - 1, new_left - 1, Kind::Context)
            }
            Some(b'-') if old_left > 0 && old_open => (old_left - 1, new_left, Kind::Delete),
            Some(b'+') if new_left > 0 && new_open => (old_left, new_left - 1, Kind::Insert),
            Some(b' ' | b'-' | b'+') | None => return Err(DiffError::UnexpectedLine { line }),
            Some(_) => return Err(unexpected(line, body)),
        };
        let (old_left, new_left, kind) = next;
        self.expect = Expect::Body {
            old_left,
            new_left,
            last: Some(kind),
        };
        Ok(())
    }

    /// Ends the text: the last section must have at least one complete hunk.
    fn finish(mut self) -> Result<Vec<Shape>, DiffError> {
        match self.expect {
            Expect::Body {
                old_left: 0,
                new_left: 0,
                ..
            } => {}
            Expect::FirstHunk => {
                let path = self.section()?.shape.path.clone();
                return Err(DiffError::NoHunks { path });
            }
            _ => return Err(DiffError::Malformed),
        }
        if let Some(section) = self.current.take() {
            self.done.push(section.shape);
        }
        Ok(self.done)
    }
}

/// Runs the line grammar over the whole text and returns one shape per section, in input order.
fn check_grammar(text: &str, limits: &Limits) -> Result<Vec<Shape>, DiffError> {
    let mut grammar = Grammar::new(limits);
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        let body = raw.strip_suffix('\n').unwrap_or(raw);
        grammar.feed(index + 1, body)?;
    }
    grammar.finish()
}

// ---------------------------------------------------------------------------------------------------------------
// Stages 3 and 4: diffy, then per-file semantic checks
// ---------------------------------------------------------------------------------------------------------------

/// A side's 0-based position: the index of its first line, or for an empty side the index it is inserted at.
fn position(start: u32, len: u32) -> Result<i64, DiffError> {
    let start = i64::from(start);
    if len == 0 {
        Ok(start)
    } else if start >= 1 {
        Ok(start - 1)
    } else {
        Err(DiffError::Malformed)
    }
}

/// Converts one diffy hunk and checks it changes something and starts where the earlier hunks put it.
fn read_hunk(
    hunk: &diffy::Hunk<'_, str>,
    path: &RelPath,
    number: usize,
    delta: &mut i64,
    bytes: usize,
) -> Result<Hunk, DiffError> {
    let too_large = |_| DiffError::TooLarge { bytes };
    let old_start = u32::try_from(hunk.old_range().start()).map_err(too_large)?;
    let old_len = u32::try_from(hunk.old_range().len()).map_err(too_large)?;
    let new_start = u32::try_from(hunk.new_range().start()).map_err(too_large)?;
    let new_len = u32::try_from(hunk.new_range().len()).map_err(too_large)?;
    let lines: Vec<HunkLine> = hunk
        .lines()
        .iter()
        .map(|line| match line {
            diffy::Line::Context(text) => HunkLine::Context((*text).to_owned()),
            diffy::Line::Delete(text) => HunkLine::Delete((*text).to_owned()),
            diffy::Line::Insert(text) => HunkLine::Insert((*text).to_owned()),
        })
        .collect();
    let old_side = lines
        .iter()
        .filter(|line| line.is_old_side())
        .map(HunkLine::raw);
    let new_side = lines
        .iter()
        .filter(|line| line.is_new_side())
        .map(HunkLine::raw);
    if old_side.eq(new_side) {
        return Err(DiffError::NoOpHunk {
            path: path.clone(),
            hunk: number,
        });
    }
    if position(new_start, new_len)? != position(old_start, old_len)? + *delta {
        return Err(DiffError::InconsistentNewStart {
            path: path.clone(),
            hunk: number,
        });
    }
    *delta += i64::from(new_len) - i64::from(old_len);
    Ok(Hunk {
        old_start,
        old_len,
        new_start,
        new_len,
        lines,
    })
}

/// Parses with diffy in git-diff mode, requires its sections to match the grammar's, and checks each section.
fn read_sections(
    text: &str,
    shapes: &[Shape],
    writable: &BTreeSet<RelPath>,
) -> Result<Vec<FileDiff>, DiffError> {
    let patches = PatchSet::parse(text, ParseOptions::gitdiff())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DiffError::Malformed)?;
    if patches.len() != shapes.len() {
        return Err(DiffError::Malformed);
    }
    let mut seen = BTreeSet::new();
    let mut files = Vec::with_capacity(shapes.len());
    for (patch, shape) in patches.iter().zip(shapes) {
        let path = &shape.path;
        let in_place = matches!(
            patch.operation(),
            FileOperation::Modify { original, modified }
                if original.strip_prefix("a/") == Some(path.as_str())
                    && modified.strip_prefix("b/") == Some(path.as_str())
        );
        if patch.patch().is_binary()
            || patch.old_mode().is_some()
            || patch.new_mode().is_some()
            || !in_place
        {
            return Err(DiffError::Malformed);
        }
        if !writable.contains(path) {
            return Err(DiffError::NotWritable { path: path.clone() });
        }
        if !seen.insert(path) {
            return Err(DiffError::DuplicateFile { path: path.clone() });
        }
        let raw_hunks = patch.patch().as_text().ok_or(DiffError::Malformed)?.hunks();
        if raw_hunks.is_empty() {
            return Err(DiffError::NoHunks { path: path.clone() });
        }
        if raw_hunks.len() != shape.hunks {
            return Err(DiffError::Malformed);
        }
        let mut delta = 0;
        let hunks = raw_hunks
            .iter()
            .enumerate()
            .map(|(index, hunk)| read_hunk(hunk, path, index + 1, &mut delta, text.len()))
            .collect::<Result<Vec<_>, _>>()?;
        files.push(FileDiff {
            path: path.clone(),
            hunks,
        });
    }
    Ok(files)
}

/// Stages 1 to 5 over one text: the files in input order.
fn read(
    text: &str,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<Vec<FileDiff>, DiffError> {
    let found = check_bytes(text, limits)?;
    let shapes = check_grammar(text, limits)?;
    let files = read_sections(text, &shapes, writable)?;
    check_accounting(&shapes, &files, found)?;
    Ok(files)
}

// ---------------------------------------------------------------------------------------------------------------
// Stage 5: line accounting
// ---------------------------------------------------------------------------------------------------------------

/// Requires the parsed sections to account for every line of the text: per section one `diff --git` line, an
/// optional `index` line, two file headers, and per hunk its header, its body lines, and one marker for each line
/// lacking its newline.
fn check_accounting(shapes: &[Shape], files: &[FileDiff], found: usize) -> Result<(), DiffError> {
    let expected: usize = shapes
        .iter()
        .zip(files)
        .map(|(shape, file)| {
            let hunks: usize = file
                .hunks
                .iter()
                .map(|hunk| {
                    let markers = hunk.lines.iter().filter(|line| !line.has_newline()).count();
                    1 + hunk.lines.len() + markers
                })
                .sum();
            3 + usize::from(shape.has_index) + hunks
        })
        .sum();
    if expected == found {
        Ok(())
    } else {
        Err(DiffError::LineAccounting { expected, found })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Stage 6: the canonical form
// ---------------------------------------------------------------------------------------------------------------

/// Writes `files` (already sorted by path) in the canonical form described at [`Diff::as_str`].
fn serialize(files: &[FileDiff]) -> String {
    let mut out = String::new();
    for file in files {
        let path = file.path.as_str();
        for piece in [
            "diff --git a/",
            path,
            " b/",
            path,
            "\n--- a/",
            path,
            "\n+++ b/",
            path,
            "\n",
        ] {
            out.push_str(piece);
        }
        for hunk in &file.hunks {
            for (part, number) in [
                ("@@ -", hunk.old_start),
                (",", hunk.old_len),
                (" +", hunk.new_start),
                (",", hunk.new_len),
            ] {
                out.push_str(part);
                out.push_str(&number.to_string());
            }
            out.push_str(" @@\n");
            for line in &hunk.lines {
                out.push(match line {
                    HunkLine::Context(_) => ' ',
                    HunkLine::Delete(_) => '-',
                    HunkLine::Insert(_) => '+',
                });
                out.push_str(line.raw());
                if !line.has_newline() {
                    out.push('\n');
                    out.push_str(NO_NEWLINE_MARKER);
                    out.push('\n');
                }
            }
        }
    }
    out
}
