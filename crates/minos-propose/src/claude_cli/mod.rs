//! The `ClaudeCli` provider: the personal default that runs the user's own `claude` binary in print mode
//! (CLAUDE.md "The model stays outside the sandbox"; 02-SPIKE-EVIDENCE.md SPIKE-07 contract).
//!
//! The provider's contract:
//!
//! - **The binary** is the user's native install only, `$HOME/.local/bin/claude` resolved into
//!   `$HOME/.local/share/claude/versions/`, never whatever `claude` is first on `PATH`, which may be a wrapper
//!   that injects settings (06-CONTEXT D-01; SPIKE-07 C-01). It is resolved again before every call, because the
//!   auto-updater moves the link, and a changed target must pass `--version` and `auth status` again before it
//!   is used (D-01, D-02 amended; 06-RESEARCH Pitfall 1).
//! - **The argv** is exactly the SPIKE-07 contract, built by [`command::argv`] as an array, never a shell string
//!   (D-06).
//! - **The environment** is cleared and then holds exactly six variables, built by
//!   [`env::ParentEnv::child_env`] from a snapshot of the parent environment the caller passes as a value.
//! - **The working directory** is a fresh, empty, private (0700) `minos-claude-*` directory under the work root,
//!   one per call, created exclusively and removed when the call ends. A directory that cannot be removed never
//!   changes the call's result, because the tokens were spent and the output was classified either way: its path
//!   is recorded, [`ClaudeCli::leaked_dirs`] names it, and the next call retries the removal first (review IN-04).
//! - **The prompt** goes on stdin, whole: the stable prefix, then the suffix (D-06).
//! - **The bounds** are [`CALL_DEADLINE`], after which the whole process group is killed and the call is an
//!   infrastructure failure, and [`MAX_ENVELOPE_BYTES`] of stdout, over which the group is killed and the call is
//!   an empty one, [`EmptyCause::OutputTooLarge`], because the reply's size is the model's choice (D-12, D-16).
//!   A descendant that left the process group survives the kill, so each pipe is abandoned
//!   [`DRAIN_GRACE`](crate::limits::DRAIN_GRACE) after it, and a call returns within the deadline plus that
//!   grace. Stderr is drained and never surfaces.
//! - **The result** is classified by [`classify::classify`] alone, and never retried here: the loop decides.
//! - **Authentication** is the user's own subscription login, and nothing else. Minos never reads, stores, or
//!   forwards a credential: it checks the login only through `claude auth status`, whose account fields are never
//!   bound, and the parent snapshot keeps only the *names* of credential and redirect variables, so a run can be
//!   refused naming one (D-03), and their values are dropped on construction.
//!
//! Every decision is a pure function with a golden or table test: the argv, the child environment, the version
//! parsing, the result classification, and the locator's rules. Nothing here reads the process environment.
//!
//! Three parts touch the host. [`locate::locate`] inspects the native install's link and versions directory. The
//! crate-private `process` runner starts every `claude` subprocess, bounded by a deadline, output caps, and a
//! process-group kill (D-16). [`preflight::preflight`] runs `--version` and `auth status` through it, minting the
//! [`preflight::Preflighted`] proof that [`ClaudeCli::preflight`] wraps (D-01 to D-04), and [`ClaudeCli`] runs
//! each model call through it.

pub mod classify;
pub mod command;
pub mod env;
pub mod locate;
pub mod preflight;
mod process;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError, TryLockError};

use minos_core::usage::Usage;

use self::classify::classify;
use self::command::{ClaudeVersion, argv};
use self::env::ParentEnv;
use self::locate::locate;
use self::preflight::{PreflightError, Preflighted, RUN_DIR_PREFIX, reverify};
use self::process::{RunError, RunSpec};
use crate::limits::{CALL_DEADLINE, MAX_ENVELOPE_BYTES, MAX_STDERR_BYTES};
use crate::provider::{
    Completion, EmptyCause, InfraCause, ModelProvider, ModelRequest, ModelResponse, ProviderError,
    ProviderStop,
};

/// The `claude` CLI provider.
///
/// Only [`ClaudeCli::preflight`] makes one, so holding one proves the per-run checks passed: no credential or
/// redirect variable set, the native install located, the work root outside any repository, a proven version, and
/// a subscription login (06-CONTEXT D-01 to D-04). No `Clone` and no `Default`; `Debug` prints the preflighted
/// version, the binary's path, and the number of leaked call directories only.
///
/// The preflighted state sits behind a lock because a call may adopt a new binary after the auto-updater moved
/// the link; the lock is released before the model call starts. The leaked directories sit behind their own
/// lock, taken only briefly, and a poisoned one is recovered, because the set is only ever replaced whole.
pub struct ClaudeCli {
    state: Mutex<Preflighted>,
    /// Call directories that could not be removed when their call ended, each retried before the next call.
    leaked: Mutex<BTreeSet<PathBuf>>,
}

/// What one call runs, copied out of the state so the lock is not held across the model call.
struct CallSetup {
    program: PathBuf,
    env: Vec<(OsString, OsString)>,
    work_root: PathBuf,
}

impl ClaudeCli {
    /// Runs the per-run preflight against `parent`, the caller's snapshot of its own environment, and returns a
    /// provider when every check passed: the D-01 to D-04 checks of [`preflight::preflight`].
    ///
    /// # Errors
    ///
    /// The first refusal, as a [`PreflightError`]; [`PreflightError::is_auth`] tells a login problem from an
    /// install, environment, or host problem.
    pub fn preflight(parent: &ParentEnv) -> Result<Self, PreflightError> {
        preflight::preflight(parent).map(|state| Self {
            state: Mutex::new(state),
            leaked: Mutex::new(BTreeSet::new()),
        })
    }

    /// The call directories that could not be removed when their call ended and have not been removed since, in
    /// path order. Each is retried before the next call; what remains is for the user or `minos host doctor` to
    /// report. Empty on a provider that has leaked nothing.
    #[must_use]
    pub fn leaked_dirs(&self) -> Vec<PathBuf> {
        self.leaked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// Records `path`, a call directory whose removal failed.
    fn record_leak(&self, path: PathBuf) {
        self.leaked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path);
    }

    /// Retries the removal of every recorded leaked directory, forgetting each that is removed or already gone
    /// and keeping the rest. Only directories this provider created are ever named here.
    fn remove_leaked(&self) {
        let mut leaked = self.leaked.lock().unwrap_or_else(PoisonError::into_inner);
        leaked.retain(|path| match fs::remove_dir_all(path) {
            Ok(()) => false,
            Err(error) => error.kind() != ErrorKind::NotFound,
        });
    }

    /// The version of the binary calls currently run: the preflighted one, or the one adopted after the link
    /// moved. `None` only when the state's lock is poisoned.
    #[must_use]
    pub fn version(&self) -> Option<ClaudeVersion> {
        self.state
            .lock()
            .ok()
            .map(|state| state.located().version())
    }

    /// Resolves the binary again and, when its target changed, verifies the new one before adopting it; then
    /// copies out what the call runs.
    fn prepare(&self) -> Result<CallSetup, ProviderError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| infra(InfraCause::Preflight, None))?;
        let located = locate(state.home()).map_err(|_| infra(InfraCause::BinaryMissing, None))?;
        if &located != state.located() {
            reverify(&located, state.child_env(), state.work_root()).map_err(|error| {
                if error.is_auth() {
                    ProviderError::Stop {
                        stop: ProviderStop::Auth,
                        usage: None,
                    }
                } else {
                    infra(InfraCause::Preflight, None)
                }
            })?;
            state.replace_located(located);
        }
        Ok(CallSetup {
            program: state.located().path().to_owned(),
            env: state.child_env().to_vec(),
            work_root: state.work_root().to_owned(),
        })
    }
}

impl ModelProvider for ClaudeCli {
    /// Makes one model call: resolves the binary again (re-verifying a changed target), then runs the golden argv
    /// in a fresh private directory with the six-variable environment and the prompt on stdin, within
    /// [`CALL_DEADLINE`] and [`MAX_ENVELOPE_BYTES`], and classifies what it printed. Never retried here.
    ///
    /// A reply over [`MAX_ENVELOPE_BYTES`] is an empty call with [`EmptyCause::OutputTooLarge`], zero usage, and
    /// no per-model usage, because the envelope was never read.
    ///
    /// Before the call, the removal of every directory an earlier call leaked is retried. After it, the call's own
    /// directory is removed; when that fails, the result stands as classified and the directory is recorded for
    /// [`ClaudeCli::leaked_dirs`] (review IN-04).
    ///
    /// # Errors
    ///
    /// [`ProviderError::Stop`] with [`ProviderStop::Auth`] when a changed binary's login check refuses, and the
    /// classifier's stops; [`ProviderError::Infra`] with [`InfraCause::BinaryMissing`] when the binary cannot be
    /// located, [`InfraCause::Preflight`] when a changed binary fails any other check (or the state's lock is
    /// poisoned), [`InfraCause::WorkDir`] when the call's directory cannot be created, the runner's cause when the
    /// process fails (including [`InfraCause::Deadline`] after the group kill), and the classifier's causes.
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError> {
        self.remove_leaked();
        let call = self.prepare()?;
        // Mode 0700 is set explicitly: `tempfile` otherwise creates the directory with the umask-derived default,
        // which on a shared `TMPDIR` would let another user list the CLI's working directory.
        let dir = tempfile::Builder::new()
            .prefix(RUN_DIR_PREFIX)
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(&call.work_root)
            .map_err(|_| infra(InfraCause::WorkDir, None))?;
        let args = argv(request.model(), request.schema());
        let spec = RunSpec {
            program: &call.program,
            args: &args,
            env: &call.env,
            cwd: dir.path(),
            stdin: request.prompt().as_bytes(),
            deadline: CALL_DEADLINE,
            stdout_cap: MAX_ENVELOPE_BYTES,
            stderr_cap: MAX_STDERR_BYTES,
        };
        let classified = match process::run(&spec) {
            Ok(output) => classify(&output.stdout, output.exit),
            // The reply the model chose to write did not fit the cap: an empty call, never infrastructure (D-12).
            // The envelope was never read, so no usage is known.
            Err(RunError::StdoutTooLarge) => Ok(ModelResponse::new(
                Completion::Empty(EmptyCause::OutputTooLarge),
                Usage::default(),
                BTreeMap::new(),
            )),
            Err(error) => Err(infra(error.cause(), None)),
        };
        // A directory that cannot be removed is recorded, never ignored, and never changes the result: the tokens
        // were spent and the reply was classified, so retrying the call as an infrastructure failure would only
        // spend them again (review IN-04).
        let path = dir.path().to_owned();
        if dir.close().is_err() {
            self.record_leak(path);
        }
        classified
    }
}

const fn infra(cause: InfraCause, usage: Option<Usage>) -> ProviderError {
    ProviderError::Infra { cause, usage }
}

impl fmt::Debug for ClaudeCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = f.debug_struct("ClaudeCli");
        match self.state.try_lock() {
            Ok(state) => out
                .field("version", &state.located().version())
                .field("path", &state.located().path()),
            Err(TryLockError::WouldBlock) => out.field("state", &"in use"),
            Err(TryLockError::Poisoned(_)) => out.field("state", &"poisoned"),
        };
        out.field("leaked", &self.leaked_dirs().len());
        out.finish_non_exhaustive()
    }
}
