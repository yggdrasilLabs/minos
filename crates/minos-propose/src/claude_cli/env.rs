//! The parent-environment snapshot and the six-variable child environment (06-CONTEXT D-03 extended; 06-RESEARCH
//! Pitfall 4, Pitfall 6).
//!
//! The crate never reads the process environment: edition 2024 makes `std::env::set_var` unsafe, so a check that
//! read it could not be tested, and a value read here could leak. The caller (the CLI, or a live test) builds a
//! [`ParentEnv`] from the real environment, and [`ParentEnv::from_vars`] keeps only the four values the child
//! inherits plus the *names* of variables that would carry a credential or redirect the CLI. Every other value is
//! dropped at once.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;

/// The only `PATH` the child sees: the system directories, never the user's shell `PATH` (SPIKE-07 contract).
pub const CHILD_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// The child's fixed `LANG`, as in the SPIKE-07 contract's environment (02-SPIKE-EVIDENCE.md C-02).
pub const CHILD_LANG: &str = "en_US.UTF-8";

/// Variables whose presence in the parent environment refuses a run, by name (D-03 as extended).
///
/// The list is the credential and redirect variables the installed CLI reads, found by a string scan of the
/// 2.1.304 bundle (06-RESEARCH Pitfall 4): an API key or token would take over from the subscription login, a base
/// URL or custom header would redirect traffic, and `CLAUDE_CONFIG_DIR` would move the login elsewhere. The child
/// never sees them, because its environment is cleared, but their presence makes the user's intent ambiguous, and
/// this provider is subscription-only.
pub const REFUSED_VARIABLES: [&str; 13] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
    "CLAUDE_CODE_API_KEY_FILE_DESCRIPTOR",
    "ANTHROPIC_FOUNDRY_API_KEY",
    "ANTHROPIC_AWS_API_KEY",
    "ANTHROPIC_IDENTITY_TOKEN",
    "ANTHROPIC_IDENTITY_TOKEN_FILE",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_CUSTOM_HEADERS",
    "CLAUDE_CONFIG_DIR",
];

/// A variable whose name starts with this prefix refuses a run (D-03): `CLAUDE_CODE_USE_BEDROCK`,
/// `CLAUDE_CODE_USE_VERTEX`, and the other provider switches.
///
/// The prefix also matches switches that select no provider (`CLAUDE_CODE_USE_POWERSHELL_TOOL`, for example).
/// That is accepted: the refusal names the variable, so the user can see exactly what to unset.
pub const REFUSED_PREFIX: &str = "CLAUDE_CODE_USE_";

/// The parent environment, reduced to what the provider may use.
///
/// It holds the values of `HOME`, `USER`, `LOGNAME`, and `TMPDIR` (none of which is a secret) and the names of
/// refused variables, never their values. `Debug` prints the refused names and which of the four variables are
/// present, never a value.
#[derive(Clone, PartialEq, Eq)]
pub struct ParentEnv {
    home: Option<OsString>,
    user: Option<OsString>,
    logname: Option<OsString>,
    tmpdir: Option<OsString>,
    refused: BTreeSet<String>,
}

/// Why no child environment could be built from the parent snapshot.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EnvError {
    /// A variable the child needs is absent or empty in the parent environment.
    #[error("{name} is not set in the environment")]
    Missing {
        /// The variable's name.
        name: &'static str,
    },
    /// A variable that must name an absolute path does not.
    #[error("{name} is not an absolute path")]
    NotAbsolute {
        /// The variable's name.
        name: &'static str,
    },
}

impl ParentEnv {
    /// The snapshot of `vars`, typically the caller's whole process environment from `vars_os`.
    ///
    /// The values of `HOME`, `USER`, `LOGNAME`, and `TMPDIR` are kept. A name in [`REFUSED_VARIABLES`] or starting
    /// with [`REFUSED_PREFIX`] is recorded, and its value dropped. Every other pair is dropped at once. Names are
    /// compared as bytes, so a name that is not UTF-8 still matches the prefix, and is recorded lossily.
    #[must_use]
    pub fn from_vars<I: IntoIterator<Item = (OsString, OsString)>>(vars: I) -> Self {
        let mut snapshot = Self {
            home: None,
            user: None,
            logname: None,
            tmpdir: None,
            refused: BTreeSet::new(),
        };
        for (name, value) in vars {
            match name.as_encoded_bytes() {
                b"HOME" => snapshot.home = Some(value),
                b"USER" => snapshot.user = Some(value),
                b"LOGNAME" => snapshot.logname = Some(value),
                b"TMPDIR" => snapshot.tmpdir = Some(value),
                bytes if is_refused(bytes) => {
                    snapshot.refused.insert(name.to_string_lossy().into_owned());
                }
                _ => {}
            }
        }
        snapshot
    }

    /// The names of refused variables present in the parent environment, in name order.
    #[must_use]
    pub const fn refused(&self) -> &BTreeSet<String> {
        &self.refused
    }

    /// The parent's `HOME`, if set.
    #[must_use]
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref().map(Path::new)
    }

    /// The parent's `TMPDIR`, if set.
    #[must_use]
    pub fn tmpdir(&self) -> Option<&Path> {
        self.tmpdir.as_deref().map(Path::new)
    }

    /// The child's whole environment: exactly `HOME`, `LANG`, `LOGNAME`, `PATH`, `TMPDIR`, and `USER`, in that
    /// order. `PATH` is [`CHILD_PATH`] and `LANG` is [`CHILD_LANG`]; the other four come from the snapshot.
    ///
    /// # Errors
    ///
    /// [`EnvError::Missing`] when one of the four is absent or empty, and [`EnvError::NotAbsolute`] when `HOME`
    /// or `TMPDIR` is not an absolute path.
    pub fn child_env(&self) -> Result<Vec<(OsString, OsString)>, EnvError> {
        let home = absolute("HOME", self.home.as_deref())?;
        let user = present("USER", self.user.as_deref())?;
        let logname = present("LOGNAME", self.logname.as_deref())?;
        let tmpdir = absolute("TMPDIR", self.tmpdir.as_deref())?;
        Ok(vec![
            (OsString::from("HOME"), home.to_owned()),
            (OsString::from("LANG"), OsString::from(CHILD_LANG)),
            (OsString::from("LOGNAME"), logname.to_owned()),
            (OsString::from("PATH"), OsString::from(CHILD_PATH)),
            (OsString::from("TMPDIR"), tmpdir.to_owned()),
            (OsString::from("USER"), user.to_owned()),
        ])
    }
}

impl fmt::Debug for ParentEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParentEnv")
            .field("home_set", &self.home.is_some())
            .field("user_set", &self.user.is_some())
            .field("logname_set", &self.logname.is_some())
            .field("tmpdir_set", &self.tmpdir.is_some())
            .field("refused", &self.refused)
            .finish()
    }
}

/// Whether a variable named `name` refuses a run.
fn is_refused(name: &[u8]) -> bool {
    name.starts_with(REFUSED_PREFIX.as_bytes())
        || REFUSED_VARIABLES
            .iter()
            .any(|refused| refused.as_bytes() == name)
}

/// `value` when it is set and not empty.
fn present<'a>(name: &'static str, value: Option<&'a OsStr>) -> Result<&'a OsStr, EnvError> {
    value
        .filter(|value| !value.is_empty())
        .ok_or(EnvError::Missing { name })
}

/// `value` when it is set and an absolute path.
fn absolute<'a>(name: &'static str, value: Option<&'a OsStr>) -> Result<&'a OsStr, EnvError> {
    let value = present(name, value)?;
    if Path::new(value).is_absolute() {
        Ok(value)
    } else {
        Err(EnvError::NotAbsolute { name })
    }
}
