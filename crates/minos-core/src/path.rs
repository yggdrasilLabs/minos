//! `RelPath`, the single validated relative path type.
//!
//! Paths in diffs, manifests, scorecards, and output trees come from hostile sources, so the grammar is a
//! conservative ASCII allowlist (03-RESEARCH Pattern 3): one or more components joined by single `/`, each made of
//! `A-Z a-z 0-9 . _ + -`, never `.` or `..`, and never starting with `-`. Absolute paths, empty components, a
//! trailing `/`, backslashes, NUL, and every non-ASCII byte are rejected, as are paths over
//! [`MAX_PATH_BYTES`], deeper than [`MAX_PATH_DEPTH`], or with a component over [`MAX_COMPONENT_BYTES`].
//!
//! Equality and order are bytewise on that ASCII text, so a case or homoglyph variant of a listed path never
//! equals it (03-CONTEXT D-01). [`RelPath::ascii_case_key`] supports the manifest's fail-closed case-fold check
//! for case-insensitive filesystems.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::limits::{MAX_COMPONENT_BYTES, MAX_PATH_BYTES, MAX_PATH_DEPTH};

/// Why a path was rejected. Variants carry indexes only, never the input text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// The path is empty.
    #[error("a relative path cannot be empty")]
    Empty,
    /// The path starts with `/`.
    #[error("a relative path cannot start with /")]
    Absolute,
    /// The path ends with `/`.
    #[error("a relative path cannot end with /")]
    TrailingSlash,
    /// The path contains `//`.
    #[error("a relative path cannot contain an empty component")]
    EmptyComponent,
    /// A component is `.`.
    #[error("a relative path cannot contain a . component")]
    DotComponent,
    /// A component is `..`.
    #[error("a relative path cannot contain a .. component")]
    ParentComponent,
    /// A component starts with `-`, so it could be read as a command-line flag.
    #[error("a path component cannot start with -")]
    LeadingDash,
    /// A byte is outside `A-Z a-z 0-9 . _ + - /`.
    #[error("the byte at index {index} is not allowed in a relative path")]
    ForbiddenByte {
        /// Zero-based byte index of the first offending byte.
        index: usize,
    },
    /// The path is longer than [`MAX_PATH_BYTES`].
    #[error("a relative path is at most {MAX_PATH_BYTES} bytes")]
    TooLong,
    /// The path has more than [`MAX_PATH_DEPTH`] components.
    #[error("a relative path has at most {MAX_PATH_DEPTH} components")]
    TooDeep,
    /// A component is longer than [`MAX_COMPONENT_BYTES`].
    #[error("a path component is at most {MAX_COMPONENT_BYTES} bytes")]
    ComponentTooLong,
}

/// A validated relative path. If a value exists, it matches the grammar in the [module docs](self).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

/// Whether `byte` may appear inside a component.
const fn is_component_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
}

/// The one validator behind every constructor. It walks the raw bytes: `std::path::Path::components` silently
/// drops `.` and empty segments, so it cannot be trusted to see them.
fn validate(text: &str) -> Result<(), PathError> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return Err(PathError::Empty);
    }
    if bytes.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    if bytes.starts_with(b"/") {
        return Err(PathError::Absolute);
    }
    if bytes.ends_with(b"/") {
        return Err(PathError::TrailingSlash);
    }
    if let Some(index) = bytes
        .iter()
        .position(|byte| *byte != b'/' && !is_component_byte(*byte))
    {
        return Err(PathError::ForbiddenByte { index });
    }
    for (depth, component) in bytes.split(|byte| *byte == b'/').enumerate() {
        if depth >= MAX_PATH_DEPTH {
            return Err(PathError::TooDeep);
        }
        match component {
            b"" => return Err(PathError::EmptyComponent),
            b"." => return Err(PathError::DotComponent),
            b".." => return Err(PathError::ParentComponent),
            [b'-', ..] => return Err(PathError::LeadingDash),
            _ if component.len() > MAX_COMPONENT_BYTES => return Err(PathError::ComponentTooLong),
            _ => {}
        }
    }
    Ok(())
}

impl RelPath {
    /// The path as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The components, in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The last component.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0
            .rsplit_once('/')
            .map_or(self.0.as_str(), |(_, name)| name)
    }

    /// The number of components.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.bytes().filter(|byte| *byte == b'/').count() + 1
    }

    /// Whether `dir`'s components are a proper prefix of this path's components: `judge` covers `judge/x` but not
    /// `judgement`, and not `judge` itself.
    #[must_use]
    pub fn is_within(&self, dir: &Self) -> bool {
        self.0
            .strip_prefix(dir.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
    }

    /// An ASCII-lowercase copy of the path, for detecting two paths that collide on a case-insensitive
    /// filesystem. Never use it for equality: paths compare bytewise.
    #[must_use]
    pub fn ascii_case_key(&self) -> String {
        self.0.to_ascii_lowercase()
    }
}

impl TryFrom<String> for RelPath {
    type Error = PathError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        validate(&text)?;
        Ok(Self(text))
    }
}

impl TryFrom<&str> for RelPath {
    type Error = PathError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        validate(text)?;
        Ok(Self(text.to_owned()))
    }
}

impl FromStr for RelPath {
    type Err = PathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

impl From<RelPath> for String {
    fn from(path: RelPath) -> Self {
        path.0
    }
}

/// Prints the path unchanged: it is printable ASCII by construction, so it needs no renderer.
impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
