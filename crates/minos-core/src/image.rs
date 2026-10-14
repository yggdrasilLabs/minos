//! `ImageRef` and the Dockerfile base-image pins: images are named by digest, never by tag.
//!
//! The v1 manifest names a Dockerfile rather than an image, so this module is where CORE-03's "images are
//! digests" holds (user decision 2026-10-14): [`base_images`] reads a Dockerfile's text and refuses any `FROM`
//! line whose image is not pinned by digest. `minos env build` and `minos check` call it (Phases 5 and 8);
//! `Manifest::try_from` stays pure and reads no Dockerfile.
//!
//! [`base_images`] follows Docker's own line rules where they decide what is an instruction: a line ending in
//! `\` (after trailing spaces and tabs) continues onto the next, comment and blank lines inside a continuation are
//! dropped, and keywords are ASCII case-insensitive. Where the rules would let a Dockerfile change how it is read,
//! it fails closed instead: an `escape` parser directive other than `\` is refused, and a `syntax` directive (a
//! frontend image the builder pulls) must itself be digest-pinned. Every comment line is checked for both
//! directives, not only the leading ones, which is stricter than Docker.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::ImageDigest;

/// Longest image name before the `@`, in bytes: the OCI distribution limit for a repository name.
const MAX_IMAGE_NAME_BYTES: usize = 255;
/// The only digest algorithm an [`ImageRef`] accepts.
const DIGEST_PREFIX: &str = "sha256:";

/// Why an image reference or a Dockerfile's base images were refused. Variants carry a line number at most,
/// never the text, because the Dockerfile is project content.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// The reference names a tag (`alpine:3.20`), with or without a digest beside it.
    #[error("an image is pinned by digest, never by tag: write name@sha256:<64 lowercase hex>")]
    Tag,
    /// The reference has no `@sha256:` digest.
    #[error("an image reference needs a @sha256:<64 lowercase hex> digest")]
    MissingDigest,
    /// The digest is not `sha256:` followed by exactly 64 lowercase hex characters.
    #[error("an image digest is sha256: followed by 64 lowercase hex characters")]
    BadDigest,
    /// The name is empty, too long, has an empty segment, or uses a byte outside `a-z 0-9 . _ - / :`.
    #[error("an image name is 1 to 255 bytes of a-z 0-9 . _ - / : with no empty segment")]
    BadName,
    /// A `FROM` image is a build variable, so the pin cannot be checked.
    #[error(
        "Dockerfile line {line}: a FROM image cannot be a build variable, because its pin cannot be checked"
    )]
    Variable {
        /// 1-based line where the instruction starts.
        line: usize,
    },
    /// A `FROM` image (or a `syntax` frontend) is not a valid digest-pinned reference.
    #[error(
        "Dockerfile line {line}: the image is not pinned by digest (name@sha256:<64 lowercase hex>)"
    )]
    Unpinned {
        /// 1-based line where the instruction or directive is.
        line: usize,
    },
    /// A `FROM` instruction names no image.
    #[error("Dockerfile line {line}: a FROM instruction names no image")]
    EmptyFrom {
        /// 1-based line where the instruction starts.
        line: usize,
    },
    /// A `FROM` instruction has a flag without `=value`, or words after the image other than `AS <name>`.
    #[error(
        "Dockerfile line {line}: a FROM instruction is FROM [--flag=value ...] <image> [AS <name>]"
    )]
    MalformedFrom {
        /// 1-based line where the instruction starts.
        line: usize,
    },
    /// An `escape` parser directive changes the escape character, which changes how lines are joined.
    #[error("Dockerfile line {line}: only the default \\ escape character is supported")]
    EscapeDirective {
        /// 1-based line of the directive.
        line: usize,
    },
}

/// A container image named by digest: `<name>@sha256:<64 lowercase hex>`.
///
/// The name is 1 to 255 bytes of `a-z 0-9 . _ - / :` with no empty `/`-separated segment. A `:` may appear only
/// in the first segment of a multi-segment name (a registry port, as in `localhost:5000/team/img`); a `:` in the
/// last segment is a tag and is refused, even beside a digest, because the pin must be the only identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ImageRef {
    name: String,
    digest: ImageDigest,
}

impl ImageRef {
    /// The name before the `@`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pinned digest.
    #[must_use]
    pub const fn digest(&self) -> &ImageDigest {
        &self.digest
    }
}

/// Checks an image name; a tag in the last segment is [`ImageError::Tag`].
fn check_name(name: &str) -> Result<(), ImageError> {
    if name.is_empty() || name.len() > MAX_IMAGE_NAME_BYTES {
        return Err(ImageError::BadName);
    }
    let allowed = |byte: u8| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'.' | b'_' | b'-' | b'/' | b':')
    };
    if !name.bytes().all(allowed) {
        return Err(ImageError::BadName);
    }
    let segments: Vec<&str> = name.split('/').collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(ImageError::BadName);
    }
    let count = segments.len();
    for (index, segment) in segments.iter().enumerate() {
        if !segment.contains(':') {
            continue;
        }
        if index + 1 == count {
            return Err(ImageError::Tag);
        }
        if index > 0 {
            return Err(ImageError::BadName);
        }
    }
    Ok(())
}

impl TryFrom<&str> for ImageRef {
    type Error = ImageError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        let Some((name, digest)) = text.split_once('@') else {
            check_name(text)?;
            return Err(ImageError::MissingDigest);
        };
        check_name(name)?;
        let hex = digest
            .strip_prefix(DIGEST_PREFIX)
            .ok_or(ImageError::BadDigest)?;
        let digest = ImageDigest::from_str(hex).map_err(|_| ImageError::BadDigest)?;
        Ok(Self {
            name: name.to_owned(),
            digest,
        })
    }
}

impl TryFrom<String> for ImageRef {
    type Error = ImageError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::try_from(text.as_str())
    }
}

impl FromStr for ImageRef {
    type Err = ImageError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

impl From<ImageRef> for String {
    fn from(image: ImageRef) -> Self {
        image.to_string()
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{DIGEST_PREFIX}{}", self.name, self.digest)
    }
}

/// One logical Dockerfile instruction: continuation lines joined, starting at `line` (1-based).
struct Instruction {
    line: usize,
    text: String,
}

/// Whether a line, with leading spaces and tabs removed, is a comment or blank.
fn is_comment_or_blank(trimmed: &str) -> bool {
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Reads a comment line as a `# key=value` parser directive, if it has that shape.
fn directive(comment: &str) -> Option<(&str, &str)> {
    let body = comment.strip_prefix('#')?;
    let (key, value) = body.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    Some((key, value.trim()))
}

/// Refuses a comment line that is an `escape` directive other than `\`, or an unpinned `syntax` directive.
fn check_directive(comment: &str, line: usize) -> Result<(), ImageError> {
    let Some((key, value)) = directive(comment) else {
        return Ok(());
    };
    if key.eq_ignore_ascii_case("escape") && value != "\\" {
        return Err(ImageError::EscapeDirective { line });
    }
    if key.eq_ignore_ascii_case("syntax") && ImageRef::from_str(value).is_err() {
        return Err(ImageError::Unpinned { line });
    }
    Ok(())
}

/// Splits a physical line into its body and whether it continues onto the next line.
fn continuation(raw: &str) -> (&str, bool) {
    let trimmed = raw.trim_end_matches([' ', '\t']);
    trimmed
        .strip_suffix('\\')
        .map_or((raw, false), |body| (body, true))
}

/// Joins a Dockerfile's physical lines into instructions, checking every comment line's directives.
fn instructions(dockerfile: &str) -> Result<Vec<Instruction>, ImageError> {
    let mut out = Vec::new();
    let mut current: Option<Instruction> = None;
    for (index, raw) in dockerfile.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim_start_matches([' ', '\t']);
        if trimmed.starts_with('#') {
            check_directive(trimmed, line)?;
        }
        if is_comment_or_blank(trimmed) {
            // Outside an instruction this is a comment or blank line; inside a continuation Docker drops it.
            continue;
        }
        let (body, continues) = continuation(raw);
        match current.as_mut() {
            Some(instruction) => instruction.text.push_str(body),
            None => {
                current = Some(Instruction {
                    line,
                    text: body.to_owned(),
                });
            }
        }
        if !continues {
            out.extend(current.take());
        }
    }
    out.extend(current.take());
    Ok(out)
}

/// The digest-pinned base images a Dockerfile's `FROM` lines name, sorted and without duplicates.
///
/// A `FROM` line may carry `--name=value` flags, then its image, then optionally `AS <name>`. The image must be a
/// digest-pinned [`ImageRef`], `scratch`, or the name of an earlier stage (compared ASCII case-insensitively, as
/// Docker does). `scratch` and stage names add nothing to the result.
///
/// # Errors
///
/// Returns [`ImageError::Variable`] for an image containing `$`, [`ImageError::Unpinned`] for an image that is
/// not a digest-pinned reference (a tag, no digest, or a malformed reference) or an unpinned `syntax` directive,
/// [`ImageError::EmptyFrom`] for a `FROM` with no image, [`ImageError::MalformedFrom`] for a flag without `=` or
/// unexpected trailing words, and [`ImageError::EscapeDirective`] for an `escape` directive other than `\`.
pub fn base_images(dockerfile: &str) -> Result<Vec<ImageRef>, ImageError> {
    let mut refs = BTreeSet::new();
    let mut stages: BTreeSet<String> = BTreeSet::new();
    for Instruction { line, text } in instructions(dockerfile)? {
        let mut words = text.split_ascii_whitespace();
        if !words
            .next()
            .is_some_and(|keyword| keyword.eq_ignore_ascii_case("FROM"))
        {
            continue;
        }
        let image = loop {
            match words.next() {
                None => return Err(ImageError::EmptyFrom { line }),
                Some(flag) if flag.starts_with("--") && flag.contains('=') => {}
                Some(flag) if flag.starts_with('-') => {
                    return Err(ImageError::MalformedFrom { line });
                }
                Some(image) => break image,
            }
        };
        if image.contains('$') {
            return Err(ImageError::Variable { line });
        }
        let alias = match (words.next(), words.next(), words.next()) {
            (None, _, _) => None,
            (Some(keyword), Some(alias), None) if keyword.eq_ignore_ascii_case("AS") => Some(alias),
            _ => return Err(ImageError::MalformedFrom { line }),
        };
        if image != "scratch" && !stages.contains(&image.to_ascii_lowercase()) {
            let pinned = ImageRef::from_str(image).map_err(|_| ImageError::Unpinned { line })?;
            refs.insert(pinned);
        }
        if let Some(alias) = alias {
            stages.insert(alias.to_ascii_lowercase());
        }
    }
    Ok(refs.into_iter().collect())
}
