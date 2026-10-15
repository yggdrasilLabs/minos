//! The secret scan (04-CONTEXT D-06, D-08, D-09; CLI-11).
//!
//! The scan covers exactly the bytes that will be packed, binary files included, because a PEM header can hide in
//! a blob. A hit blocks the pack, and there is no override: no manifest allowlist, no CLI flag, and no way to
//! switch a pattern off. `--allow-dirty` does not touch it. A hit names the path, the pattern, and the 1-based
//! line, never the matched bytes, so a report or a terminal never shows the secret it found.
//!
//! The patterns are a fixed set of anchored, high-signal shapes compiled by the `regex` crate, which matches in
//! linear time, so a hostile file cannot cause catastrophic backtracking. Each is compiled with `(?-u)` so `\b`
//! is the ASCII word boundary over raw bytes. There is no entropy heuristic: the repository's own golden SHA-256
//! vectors would trip it.

use std::fmt;

use minos_core::path::RelPath;
use regex::bytes::{Regex, RegexSet};

/// Every pattern, as (name, regex). Order is [`PATTERN_NAMES`] order.
const PATTERNS: [(&str, &str); 9] = [
    (
        "pem-private-key",
        r"(?-u)-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----",
    ),
    (
        "aws-access-key-id",
        r"(?-u)\b(?:A3T[A-Z0-9]|AKIA|ASIA|ABIA|ACCA)[A-Z2-7]{16}\b",
    ),
    ("github-token", r"(?-u)\bgh[pousr]_[A-Za-z0-9]{36}\b"),
    (
        "github-fine-grained-pat",
        r"(?-u)\bgithub_pat_[A-Za-z0-9_]{82}\b",
    ),
    ("anthropic-api-key", r"(?-u)\bsk-ant-[A-Za-z0-9_-]{20,}"),
    (
        "openai-api-key",
        r"(?-u)\bsk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}T3BlbkFJ[A-Za-z0-9_-]{20,}",
    ),
    ("slack-token", r"(?-u)\bxox[abposr]-[0-9A-Za-z-]{10,}"),
    (
        "jwt",
        r"(?-u)\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    ),
    ("google-api-key", r"(?-u)\bAIza[0-9A-Za-z_-]{35}\b"),
];

/// The name of every pattern, as a [`SecretHit`] reports it.
pub const PATTERN_NAMES: [&str; 9] = [
    "pem-private-key",
    "aws-access-key-id",
    "github-token",
    "github-fine-grained-pat",
    "anthropic-api-key",
    "openai-api-key",
    "slack-token",
    "jwt",
    "google-api-key",
];

/// The compiled patterns: one set for a single pass over the bytes, then each matched pattern alone for lines.
#[derive(Clone, Debug)]
pub struct Scanner {
    set: RegexSet,
    each: Vec<(&'static str, Regex)>,
}

/// A pattern failed to compile. The patterns are fixed, so this is a bug in Minos, never in the project.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ScanError {
    /// A pattern did not compile.
    #[error("a secret-scan pattern failed to compile")]
    Compile(#[source] regex::Error),
}

impl Scanner {
    /// Compiles every pattern.
    ///
    /// # Errors
    ///
    /// [`ScanError::Compile`] if a pattern does not compile, which the tests rule out.
    pub fn new() -> Result<Self, ScanError> {
        let set = RegexSet::new(PATTERNS.iter().map(|(_, pattern)| pattern))
            .map_err(ScanError::Compile)?;
        let each = PATTERNS
            .iter()
            .map(|(name, pattern)| Ok((*name, Regex::new(pattern).map_err(ScanError::Compile)?)))
            .collect::<Result<Vec<_>, ScanError>>()?;
        Ok(Self { set, each })
    }

    /// Every hit in `bytes`, the content of the file at `path`, sorted by path, then line, then pattern.
    #[must_use]
    pub fn scan(&self, path: &RelPath, bytes: &[u8]) -> Vec<SecretHit> {
        let mut hits = Vec::new();
        for index in &self.set.matches(bytes) {
            let Some((name, regex)) = self.each.get(index) else {
                continue;
            };
            // find_iter yields matches in order, so the newline count only ever moves forward.
            let mut counted_to = 0;
            let mut line: u64 = 1;
            for found in regex.find_iter(bytes) {
                line = bytes
                    .get(counted_to..found.start())
                    .unwrap_or_default()
                    .iter()
                    .fold(line, |at, byte| {
                        if *byte == b'\n' {
                            at.saturating_add(1)
                        } else {
                            at
                        }
                    });
                counted_to = found.start();
                hits.push(SecretHit {
                    path: path.clone(),
                    line,
                    pattern: name,
                });
            }
        }
        hits.sort();
        hits
    }
}

/// Where a pattern matched. It never holds the matched bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecretHit {
    path: RelPath,
    line: u64,
    pattern: &'static str,
}

impl SecretHit {
    /// The file the pattern matched in.
    #[must_use]
    pub fn path(&self) -> &RelPath {
        &self.path
    }

    /// The name of the pattern that matched, one of [`PATTERN_NAMES`].
    #[must_use]
    pub fn pattern(&self) -> &'static str {
        self.pattern
    }

    /// The 1-based line the match starts on.
    #[must_use]
    pub fn line(&self) -> u64 {
        self.line
    }
}

/// `<path>:<line>: matches <pattern>`.
impl fmt::Display for SecretHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: matches {}", self.path, self.line, self.pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::{PATTERN_NAMES, PATTERNS};

    #[test]
    fn pattern_names_list_every_pattern_in_order() {
        let names: Vec<&str> = PATTERNS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, PATTERN_NAMES);
    }
}
