//! What a backend returns when a launch ends (refinement of 04-CONTEXT D-17; 04-RESEARCH Open Question 1).
//!
//! D-17 had a backend's `wait` return `Untrusted<GuestOutput<R>>`. The K7 classification (the module docs of
//! `minos_core::outcome`) also has host-observed results that are neither infrastructure errors nor guest data:
//! `Crashed` when no valid status frame arrived or the console cap fired, and `TimedOut` at the host deadline.
//! Putting those inside `Untrusted` would hide trusted facts behind hostile-data exits. So `wait` (plan 04-08)
//! returns a [`Finished`], which holds the trusted, host-observed [`Ended`] beside the still-hostile output
//! stream, and infrastructure failures stay on the `Err` path exactly as D-17 intends.
//!
//! A `Finished<R>` can only be built from a [`Cleared<R>`], so output of a role exists only for a cleared launch of
//! that role, and the output reader itself is crate-private.

use std::marker::PhantomData;

use crate::key::LaunchKey;
use crate::output::{OutputFault, read_output};
use crate::preflight::Cleared;
use crate::role::Role;
use crate::untrusted::{GuestOutput, Untrusted};

/// Why the host judged a launch to have crashed. Host-observed, so trusted.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashCause {
    /// The launcher exited 0 but no valid status frame arrived: the guest died, panicked, or never reported
    /// (K7: "exit 0 with no valid frame means `Crashed`").
    NoStatusFrame,
    /// The launcher exited 125 with its `watchdog console-cap` marker: the guest flooded the console (K7). Never
    /// retried.
    ConsoleCap,
}

/// How a launch ended, as the host observed it. Trusted: the guest cannot forge any of these.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The launcher exited 0 with exactly one valid status frame: the guest decided, and the frame says what
    /// happened (K7).
    Exited,
    /// The host classified the launch as crashed (K7), for the given cause.
    Crashed(CrashCause),
    /// The host deadline passed, or the launcher exited 124 with its `watchdog deadline` marker (K7).
    TimedOut,
}

/// The end of one launch of role `R`: the host-observed [`Ended`], the plan's [`LaunchKey`], and the hostile output
/// stream, still unread.
///
/// No derives. [`Finished::new`] is the only constructor and needs the launch's [`Cleared<R>`].
pub struct Finished<R: Role> {
    key: LaunchKey,
    ended: Ended,
    stream: Vec<u8>,
    role: PhantomData<fn() -> R>,
}

impl<R: Role> Finished<R> {
    /// The end of the cleared launch `cleared`: `ended` as the host observed it, and `stream`, the reassembled
    /// output tar, unread.
    ///
    /// Requiring the `Cleared` of role `R` means output of a role exists only for a cleared launch of that role.
    #[must_use]
    pub fn new(cleared: &Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self {
        Self {
            key: *cleared.plan().key(),
            ended,
            stream,
            role: PhantomData,
        }
    }

    /// How the launch ended, as the host observed it.
    #[must_use]
    pub const fn ended(&self) -> Ended {
        self.ended
    }

    /// The launch key of the plan that ran.
    #[must_use]
    pub const fn key(&self) -> &LaunchKey {
        &self.key
    }

    /// Reads the output stream in memory under the caps in [`crate::output`].
    ///
    /// # Errors
    ///
    /// An [`OutputFault`] when the stream breaks a cap or a rule: a candidate fault, never retried.
    pub fn into_output(self) -> Result<Untrusted<GuestOutput<R>>, OutputFault> {
        read_output::<R>(&self.stream)
    }
}
