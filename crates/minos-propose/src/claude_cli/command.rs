//! The `claude` argv and version parsing (06-CONTEXT D-04, D-06; 02-SPIKE-EVIDENCE.md SPIKE-07 contract).

use std::ffi::OsString;
use std::fmt;

use minos_core::id::ModelId;

use crate::schema::OutputSchema;

/// A `claude` version: three numbers, `major.minor.patch`, ordered numerically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaudeVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

/// The oldest `claude` the provider runs: 2.1.296, the version the SPIKE-07 contract was proven on
/// (02-SPIKE-EVIDENCE.md C-01 to C-14). Newer versions are allowed (D-04).
pub const MIN_CLAUDE_VERSION: ClaudeVersion = ClaudeVersion::new(2, 1, 296);

/// The preflight's version query, after the program. It prints `X.Y.Z (Claude Code)` and makes no model call.
pub const VERSION_ARGS: [&str; 1] = ["--version"];

/// The preflight's login query, after the program. It prints a JSON object and makes no model call.
pub const AUTH_STATUS_ARGS: [&str; 2] = ["auth", "status"];

/// The `--mcp-config` value: no MCP servers, so with `--strict-mcp-config` none is ever started.
pub const EMPTY_MCP_CONFIG: &str = r#"{"mcpServers":{}}"#;

/// The most digits one version number may have, so every number fits a `u32`.
const MAX_VERSION_DIGITS: usize = 9;

/// The `--version` output after the version.
const VERSION_SUFFIX: &str = " (Claude Code)";

impl ClaudeVersion {
    /// The version `major.minor.patch`.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parses `text` as exactly three dot-separated numbers of 1 to 9 ASCII digits, with no sign, no leading zero
    /// on a number of more than one digit, and nothing before or after, so each version has one text form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let major = number(parts.next()?)?;
        let minor = number(parts.next()?)?;
        let patch = number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self::new(major, minor, patch))
    }
}

impl fmt::Display for ClaudeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parses the stdout of `claude --version`: exactly `X.Y.Z (Claude Code)`, with at most one trailing newline.
#[must_use]
pub fn parse_version_output(stdout: &[u8]) -> Option<ClaudeVersion> {
    let line = stdout.strip_suffix(b"\n").unwrap_or(stdout);
    let line = std::str::from_utf8(line).ok()?;
    ClaudeVersion::parse(line.strip_suffix(VERSION_SUFFIX)?)
}

/// The argv of one model call, after the program: exactly the SPIKE-07 contract, 18 elements.
///
/// `-p --model <model> --tools "" --output-format json --json-schema <schema> --no-session-persistence
/// --strict-mcp-config --mcp-config {"mcpServers":{}} --safe-mode --permission-prompts none --max-turns 1`
///
/// The model and the schema are each one element, and no element is ever built from a shell string. A
/// [`ModelId`] starts with an ASCII letter or digit, so it can never be read as a flag (06-RESEARCH Pitfall 9).
#[must_use]
pub fn argv(model: &ModelId, schema: &OutputSchema) -> Vec<OsString> {
    [
        "-p",
        "--model",
        model.as_str(),
        "--tools",
        "",
        "--output-format",
        "json",
        "--json-schema",
        schema.as_str(),
        "--no-session-persistence",
        "--strict-mcp-config",
        "--mcp-config",
        EMPTY_MCP_CONFIG,
        "--safe-mode",
        "--permission-prompts",
        "none",
        "--max-turns",
        "1",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

/// One version number: 1 to [`MAX_VERSION_DIGITS`] ASCII digits, with no leading zero unless it is `0`.
fn number(text: &str) -> Option<u32> {
    let digits = text.len();
    if digits == 0 || digits > MAX_VERSION_DIGITS || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}
