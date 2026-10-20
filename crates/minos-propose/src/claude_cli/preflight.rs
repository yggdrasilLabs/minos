//! The per-run preflight: proof that the user's own native install, at a proven version, logged in with the
//! user's subscription, is what every model call will run (06-CONTEXT D-01 to D-04 as amended and extended;
//! 06-RESEARCH Pattern 3, Finding 2, Finding 3, Pitfalls 1, 3, 4, and 6).
//!
//! [`preflight`] checks, in order, and refuses at the first failure:
//!
//! 1. No credential or redirect variable is set in the parent environment, by name; values are never seen
//!    (D-03 extended).
//! 2. The six-variable child environment can be built.
//! 3. The binary is the native install (D-01), by [`locate`].
//! 4. The work root, the canonical `TMPDIR`, lies outside any repository or project configuration, because
//!    Claude Code reads `CLAUDE.md`, `.mcp.json`, and `.claude/` up the directory tree.
//! 5. `claude --version` prints a version equal to the binary's file name and at least
//!    [`MIN_CLAUDE_VERSION`] (D-04).
//! 6. `claude auth status` reports a logged-in subscription (`claude.ai`) on Anthropic's first-party API with no
//!    API key source (D-02 amended). This is the one check that sees an `apiKeyHelper` or a provider switch set
//!    in `~/.claude/settings.json`, which the cleared environment cannot remove.
//!
//! Steps 5 and 6 run the located binary through the capped runner, each under the six-variable environment in a
//! fresh, empty, private directory under the work root that is removed afterwards. Neither makes a model call.
//! A [`Preflighted`] value exists only when every check passed, the way a `Cleared<R>` exists only after the
//! sandbox preflight.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use minos_core::text::SafeText;
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use crate::claude_cli::classify::Exit;
use crate::claude_cli::command::{
    AUTH_STATUS_ARGS, ClaudeVersion, MIN_CLAUDE_VERSION, VERSION_ARGS, parse_version_output,
};
use crate::claude_cli::env::{EnvError, ParentEnv};
use crate::claude_cli::locate::{LocateError, Located, locate};
use crate::claude_cli::process::{self, RunOutput, RunSpec};
use crate::limits::{MAX_PREFLIGHT_STDOUT_BYTES, MAX_STDERR_BYTES, PREFLIGHT_DEADLINE};
use crate::provider::InfraCause;

/// The `authMethod` of a subscription login.
const SUBSCRIPTION_METHOD: &str = "claude.ai";

/// The `apiProvider` of Anthropic's own API, as opposed to Bedrock, Vertex, or Foundry.
const FIRST_PARTY_PROVIDER: &str = "firstParty";

/// Files and directories whose presence in the work root or an ancestor refuses it: each makes Claude Code read
/// project configuration or treat the directory as a project. `.claude` is checked separately, because the
/// user's own `$HOME/.claude` is expected.
const PROJECT_MARKERS: [&str; 3] = [".git", "CLAUDE.md", ".mcp.json"];

/// The project configuration directory, refused anywhere but the canonical `HOME`.
const CLAUDE_DIR: &str = ".claude";

/// The prefix of each preflight run's and each model call's private directory under the work root.
pub(crate) const RUN_DIR_PREFIX: &str = "minos-claude-";

/// Most bytes of CLI output kept in a refusal, cut at a character boundary.
const DETAIL_BYTES: usize = 256;

/// A preflight query that runs the binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightStep {
    /// `claude --version`.
    Version,
    /// `claude auth status`.
    AuthStatus,
}

impl PreflightStep {
    /// The query's argv after the program.
    fn args(self) -> Vec<OsString> {
        match self {
            Self::Version => VERSION_ARGS.iter().map(OsString::from).collect(),
            Self::AuthStatus => AUTH_STATUS_ARGS.iter().map(OsString::from).collect(),
        }
    }
}

impl fmt::Display for PreflightStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Version => "claude --version",
            Self::AuthStatus => "claude auth status",
        })
    }
}

/// Why the preflight refused. Each message names the fix. No variant holds a credential value, and CLI output
/// appears only as a short [`SafeText`] excerpt.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreflightError {
    /// Credential or redirect variables are set in the parent environment (D-03 extended).
    #[error(
        "the environment sets {names}; the claude CLI provider uses only your subscription login, never an API \
         key, token, or redirect; unset them, or use the API-key provider once it exists",
        names = .names.join(", ")
    )]
    CredentialVariables {
        /// The variables' names, in name order. Never their values.
        names: Vec<String>,
    },
    /// The child environment could not be built from the parent environment.
    #[error(
        "the claude CLI provider cannot build its environment: {0}; set HOME, USER, LOGNAME, and TMPDIR, with \
         HOME and TMPDIR absolute paths"
    )]
    Environment(#[from] EnvError),
    /// The native install was not located (D-01).
    #[error("{0}; install Claude Code with its native installer, which writes ~/.local/bin/claude")]
    Locate(#[from] LocateError),
    /// The work root lies inside a repository or project configuration.
    #[error(
        "TMPDIR is inside a repository or project configuration: {path} holds {marker}, and Claude Code reads \
         project configuration up the directory tree; set TMPDIR to a directory outside any repository",
        path = .path.display()
    )]
    WorkRootInRepository {
        /// The directory that holds the marker: the work root or one of its ancestors.
        path: PathBuf,
        /// The marker found.
        marker: &'static str,
    },
    /// The work root could not be resolved, checked, or written.
    #[error(
        "the work directory {path} could not be resolved, checked, or written; set TMPDIR to an existing \
         directory you own, outside any repository",
        path = .path.display()
    )]
    WorkRootUnusable {
        /// The directory that failed.
        path: PathBuf,
    },
    /// A preflight query did not complete.
    #[error("{step} did not complete: {cause}")]
    Process {
        /// The query.
        step: PreflightStep,
        /// Why it did not complete.
        cause: InfraCause,
    },
    /// `claude --version` failed or printed something other than `X.Y.Z (Claude Code)`.
    #[error(
        "claude --version did not print a version (`{detail}`); reinstall Claude Code with its native installer"
    )]
    VersionUnreadable {
        /// An excerpt of what it printed.
        detail: SafeText,
    },
    /// The binary's file name and its `--version` disagree.
    #[error(
        "the claude binary named {file} reports version {reported}; reinstall Claude Code with its native \
         installer"
    )]
    VersionMismatch {
        /// The version the file name states.
        file: ClaudeVersion,
        /// The version `--version` printed.
        reported: ClaudeVersion,
    },
    /// The binary is older than the version the provider's contract was proven on (D-04).
    #[error(
        "claude {found} is older than {minimum}, the version the provider's contract was proven on; update \
         Claude Code with `claude update`"
    )]
    VersionTooOld {
        /// The binary's version.
        found: ClaudeVersion,
        /// [`MIN_CLAUDE_VERSION`].
        minimum: ClaudeVersion,
    },
    /// `claude auth status` reports no login.
    #[error("claude is not logged in; run `claude` and log in with your Claude subscription")]
    NotLoggedIn,
    /// `claude auth status` failed with an exit other than 0 or 1, or a signal.
    #[error(
        "claude auth status failed ({code}): `{detail}`; run `claude auth status` to see why",
        code = ExitText(*.code)
    )]
    AuthStatusFailed {
        /// The exit code, or `None` for a signal.
        code: Option<i32>,
        /// An excerpt of its stderr, or of its stdout when stderr was empty.
        detail: SafeText,
    },
    /// `claude auth status` printed something other than the expected JSON object.
    #[error(
        "claude auth status printed output that is not the expected JSON object; update Claude Code"
    )]
    AuthStatusUnreadable,
    /// The login is not a Claude subscription.
    #[error(
        "claude is logged in with `{method}`, not a Claude subscription (claude.ai); run `claude auth login` and \
         choose your subscription"
    )]
    NotSubscriptionLogin {
        /// The `authMethod` reported, or `(none)` when absent.
        method: SafeText,
    },
    /// The CLI uses a provider other than Anthropic's own API.
    #[error(
        "claude uses the API provider `{provider}`, not Anthropic's first-party API; remove the provider switch \
         (for example CLAUDE_CODE_USE_BEDROCK in the env table of ~/.claude/settings.json)"
    )]
    NotFirstParty {
        /// The `apiProvider` reported, or `(none)` when absent.
        provider: SafeText,
    },
    /// The CLI reports an API key source, which would take over from the subscription login.
    #[error(
        "claude reports an API key source; remove the apiKeyHelper or key configuration from \
         ~/.claude/settings.json so your subscription login is used"
    )]
    ApiKeySourcePresent,
}

impl PreflightError {
    /// Whether the refusal is about the login, which the user fixes by logging in again, as opposed to the
    /// install, the environment, or the host.
    #[must_use]
    pub const fn is_auth(&self) -> bool {
        matches!(
            self,
            Self::NotLoggedIn
                | Self::NotSubscriptionLogin { .. }
                | Self::NotFirstParty { .. }
                | Self::ApiKeySourcePresent
        )
    }
}

/// Renders an exit code, or a signal when there is none.
struct ExitText(Option<i32>);

impl fmt::Display for ExitText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(code) => write!(f, "exit code {code}"),
            None => f.write_str("killed by a signal"),
        }
    }
}

/// Proof that the preflight passed: the located binary, the child environment, `HOME`, and the work root every
/// call's private directory is created under.
///
/// Only [`preflight`] makes one. No `Clone` and no `Default`; `Debug` prints the version and the binary's path
/// only.
pub struct Preflighted {
    located: Located,
    env: Vec<(OsString, OsString)>,
    home: PathBuf,
    work_root: PathBuf,
}

impl Preflighted {
    /// The binary the checks ran.
    #[must_use]
    pub const fn located(&self) -> &Located {
        &self.located
    }

    /// The six-variable child environment the checks ran under.
    #[must_use]
    pub fn child_env(&self) -> &[(OsString, OsString)] {
        &self.env
    }

    /// The parent's `HOME`, for locating the binary again before each call.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The canonical `TMPDIR`, checked to lie outside any repository or project configuration.
    #[must_use]
    pub fn work_root(&self) -> &Path {
        &self.work_root
    }

    /// Adopts `located` as the binary every later call runs.
    ///
    /// Call it only after [`reverify`] passed for that same `located` under this value's environment and work
    /// root: that keeps the proof valid, because the version and login checks then hold for the new binary as
    /// they did for the old one. Crate-private, and the only way the located binary changes: there is no public
    /// setter.
    pub(crate) fn replace_located(&mut self, located: Located) {
        self.located = located;
    }
}

impl fmt::Debug for Preflighted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Preflighted")
            .field("version", &self.located.version())
            .field("path", &self.located.path())
            .finish_non_exhaustive()
    }
}

/// Runs every preflight check against `parent`, the caller's snapshot of its own environment.
///
/// # Errors
///
/// The first refusal, as a [`PreflightError`], in the order the module documentation lists.
pub fn preflight(parent: &ParentEnv) -> Result<Preflighted, PreflightError> {
    if !parent.refused().is_empty() {
        return Err(PreflightError::CredentialVariables {
            names: parent.refused().iter().cloned().collect(),
        });
    }
    let env = parent.child_env()?;
    let (Some(home), Some(tmpdir)) = (parent.home(), parent.tmpdir()) else {
        // `child_env` succeeded, so both are set; kept as a refusal rather than a panic.
        let name = if parent.home().is_none() {
            "HOME"
        } else {
            "TMPDIR"
        };
        return Err(PreflightError::Environment(EnvError::Missing { name }));
    };
    let located = locate(home)?;
    let work_root = work_root(tmpdir, home)?;
    reverify(&located, &env, &work_root)?;
    Ok(Preflighted {
        located,
        env,
        home: home.to_owned(),
        work_root,
    })
}

/// Runs the `--version` and `auth status` checks, steps 5 and 6, against `located`.
///
/// [`preflight`] calls it once; the provider calls it again when the native install's link has moved to another
/// binary since the preflight (D-01 and D-02 amended, run lifetime).
pub(crate) fn reverify(
    located: &Located,
    env: &[(OsString, OsString)],
    work_root: &Path,
) -> Result<(), PreflightError> {
    let version = run_step(PreflightStep::Version, located, env, work_root)?;
    check_version(located, &version)?;
    let auth = run_step(PreflightStep::AuthStatus, located, env, work_root)?;
    check_auth(&auth)
}

/// The canonical `tmpdir`, refused when it or any ancestor holds project configuration.
fn work_root(tmpdir: &Path, home: &Path) -> Result<PathBuf, PreflightError> {
    let unusable = |path: &Path| PreflightError::WorkRootUnusable {
        path: path.to_owned(),
    };
    let root = fs::canonicalize(tmpdir).map_err(|_| unusable(tmpdir))?;
    if !root.is_dir() {
        return Err(unusable(&root));
    }
    let home = fs::canonicalize(home).ok();
    for dir in root.ancestors() {
        for marker in PROJECT_MARKERS {
            if holds(dir, marker)? {
                return Err(PreflightError::WorkRootInRepository {
                    path: dir.to_owned(),
                    marker,
                });
            }
        }
        if home.as_deref() != Some(dir) && holds(dir, CLAUDE_DIR)? {
            return Err(PreflightError::WorkRootInRepository {
                path: dir.to_owned(),
                marker: CLAUDE_DIR,
            });
        }
    }
    Ok(root)
}

/// Whether `dir` holds an entry named `name` of any kind, checked without following it. An entry that cannot be
/// checked refuses the work root: an unknown answer is never read as absent.
fn holds(dir: &Path, name: &str) -> Result<bool, PreflightError> {
    match fs::symlink_metadata(dir.join(name)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(PreflightError::WorkRootUnusable {
            path: dir.to_owned(),
        }),
    }
}

/// Runs `step` with the located binary in a fresh private directory under `work_root`, removed afterwards.
///
/// A directory that cannot be removed makes the work root unusable, unless the query itself failed, which is the
/// more useful diagnosis and is reported instead; the removal failure never masks it (review IN-02).
fn run_step(
    step: PreflightStep,
    located: &Located,
    env: &[(OsString, OsString)],
    work_root: &Path,
) -> Result<RunOutput, PreflightError> {
    let unusable = || PreflightError::WorkRootUnusable {
        path: work_root.to_owned(),
    };
    // Mode 0700 is set explicitly, as for each model call: `tempfile`'s default is the umask-derived mode.
    let dir = tempfile::Builder::new()
        .prefix(RUN_DIR_PREFIX)
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(work_root)
        .map_err(|_| unusable())?;
    let args = step.args();
    let spec = RunSpec {
        program: located.path(),
        args: &args,
        env,
        cwd: dir.path(),
        stdin: &[],
        deadline: PREFLIGHT_DEADLINE,
        stdout_cap: MAX_PREFLIGHT_STDOUT_BYTES,
        stderr_cap: MAX_STDERR_BYTES,
    };
    let output = process::run(&spec).map_err(|error| PreflightError::Process {
        step,
        cause: error.cause(),
    });
    match (dir.close(), output) {
        (Ok(()), output) => output,
        (Err(_), Err(process)) => Err(process),
        (Err(_), Ok(_)) => Err(unusable()),
    }
}

/// Step 5: the version output parses, equals the file name's version, and is at least the minimum.
fn check_version(located: &Located, output: &RunOutput) -> Result<(), PreflightError> {
    if output.exit != Exit::Code(0) {
        return Err(PreflightError::VersionUnreadable {
            detail: failure_detail(output),
        });
    }
    let reported =
        parse_version_output(&output.stdout).ok_or_else(|| PreflightError::VersionUnreadable {
            detail: excerpt(&output.stdout),
        })?;
    if reported != located.version() {
        return Err(PreflightError::VersionMismatch {
            file: located.version(),
            reported,
        });
    }
    if reported < MIN_CLAUDE_VERSION {
        return Err(PreflightError::VersionTooOld {
            found: reported,
            minimum: MIN_CLAUDE_VERSION,
        });
    }
    Ok(())
}

/// The `auth status` fields the preflight reads, parsed leniently: every other field is accepted and ignored,
/// never bound, so the account's email address and organization never enter Minos (PROP-04). This is the second
/// documented exception to strict parsing, after the result envelope, and for the same reason: the CLI adds
/// fields often.
#[derive(Deserialize)]
struct AuthStatus {
    #[serde(rename = "loggedIn")]
    logged_in: bool,
    #[serde(rename = "authMethod", default)]
    auth_method: Option<String>,
    #[serde(rename = "apiProvider", default)]
    api_provider: Option<String>,
    /// Whether the key is present at all, whatever its value, `null` included: `auth status` prints it only when
    /// a key source exists (D-02 amended).
    #[serde(rename = "apiKeySource", default, deserialize_with = "present")]
    api_key_source: bool,
}

/// Consumes any value without binding it and reports that the field was present.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    IgnoredAny::deserialize(deserializer)?;
    Ok(true)
}

/// Step 6: a logged-in subscription on the first-party API with no API key source.
fn check_auth(output: &RunOutput) -> Result<(), PreflightError> {
    match output.exit {
        Exit::Code(0) => {}
        Exit::Code(1) => return Err(PreflightError::NotLoggedIn),
        Exit::Code(code) => {
            return Err(PreflightError::AuthStatusFailed {
                code: Some(code),
                detail: failure_detail(output),
            });
        }
        Exit::Signal(_) => {
            return Err(PreflightError::AuthStatusFailed {
                code: None,
                detail: failure_detail(output),
            });
        }
    }
    // A JSON array would also fill a struct, by position; only an object is the documented shape.
    let is_object = output
        .stdout
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'{');
    if !is_object {
        return Err(PreflightError::AuthStatusUnreadable);
    }
    let status: AuthStatus =
        serde_json::from_slice(&output.stdout).map_err(|_| PreflightError::AuthStatusUnreadable)?;
    if !status.logged_in {
        return Err(PreflightError::NotLoggedIn);
    }
    if status.auth_method.as_deref() != Some(SUBSCRIPTION_METHOD) {
        return Err(PreflightError::NotSubscriptionLogin {
            method: reported(status.auth_method.as_deref()),
        });
    }
    if status.api_provider.as_deref() != Some(FIRST_PARTY_PROVIDER) {
        return Err(PreflightError::NotFirstParty {
            provider: reported(status.api_provider.as_deref()),
        });
    }
    if status.api_key_source {
        return Err(PreflightError::ApiKeySourcePresent);
    }
    Ok(())
}

/// A reported field value for a refusal, or `(none)` when absent.
fn reported(value: Option<&str>) -> SafeText {
    excerpt(value.unwrap_or("(none)").as_bytes())
}

/// The detail of a failed query: its stderr tail, or its stdout when stderr was empty.
fn failure_detail(output: &RunOutput) -> SafeText {
    if output.stderr_tail.iter().all(u8::is_ascii_whitespace) {
        excerpt(&output.stdout)
    } else {
        excerpt(&output.stderr_tail)
    }
}

/// `bytes` as one sanitized line of at most [`DETAIL_BYTES`] bytes before sanitizing, cut at a character
/// boundary.
fn excerpt(bytes: &[u8]) -> SafeText {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let mut cut = text.len().min(DETAIL_BYTES);
    while !text.is_char_boundary(cut) {
        cut = cut.saturating_sub(1);
    }
    SafeText::inline(text.get(..cut).unwrap_or_default())
}
