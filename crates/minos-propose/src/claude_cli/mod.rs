//! The pure parts of the `ClaudeCli` provider: the personal default that runs the user's own `claude` binary in
//! print mode (CLAUDE.md "The model stays outside the sandbox"; 02-SPIKE-EVIDENCE.md SPIKE-07 contract).
//!
//! The provider's contract:
//!
//! - **The binary** is the user's native install only, `$HOME/.local/bin/claude` resolved into
//!   `$HOME/.local/share/claude/versions/`, never whatever `claude` is first on `PATH`, which may be a wrapper
//!   that injects settings (06-CONTEXT D-01; SPIKE-07 C-01).
//! - **The argv** is exactly the SPIKE-07 contract, built by [`command::argv`] as an array, never a shell string
//!   (D-06).
//! - **The environment** is cleared and then holds exactly six variables, built by
//!   [`env::ParentEnv::child_env`] from a snapshot of the parent environment the caller passes as a value.
//! - **The working directory** is a fresh, empty, private directory outside any repository, one per call.
//! - **The prompt** goes on stdin, whole: the stable prefix, then the suffix (D-06).
//! - **Authentication** is the user's own subscription login, and nothing else. Minos never reads, stores, or
//!   forwards a credential: the parent snapshot keeps only the *names* of credential and redirect variables, so a
//!   run can be refused naming one (D-03), and their values are dropped on construction.
//!
//! Every decision here is a pure function with a golden or table test; the process that runs the call is wired to
//! them separately, so nothing in this module spawns a process or reads the process environment.

pub mod classify;
pub mod command;
pub mod env;
