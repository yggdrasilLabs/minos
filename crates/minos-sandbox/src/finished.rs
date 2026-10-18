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
//! that role, and the output reader itself is crate-private. It is built by consuming that `Cleared<R>`, so one
//! clearance mints at most one `Finished` (review WR-03).
//!
//! Beside the stream, a `Finished` carries two more things the backend observed: the elapsed time it measured
//! from spawn to exit, which is trusted, and the console bytes it captured under its cap, which the guest
//! influenced and are therefore hostile. The console leaves only through [`Finished::console_excerpt`], a
//! truncated [`LogExcerpt`] with control characters stripped, so console bytes never reach a terminal or the
//! ledger raw (KRUN-01).

use std::marker::PhantomData;
use std::time::Duration;

use minos_core::text::LogExcerpt;

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
    /// The console passed its cap, as the host observed it: the guest flooded the console (K7). Never retried.
    ///
    /// The console is the helper's stdout pipe (05-EVIDENCE.md Mechanism selections, `console: pipe`): the krun
    /// supervisor reads it through cap + 1 bytes, and on reading the extra byte it kills the helper's whole process
    /// group, so the cap is exact and no console byte touches host disk. The Phase 2 file console's exit 125 with
    /// a `console-cap` marker no longer exists.
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
    /// The output stream broke the chunk protocol: a wrong token, a gap, a duplicate, a bad END, or an over-cap
    /// chunk (GUEST-04). Host-observed, so trusted. A candidate fault the runner records as
    /// `Rejected(Reason::ChunkProtocol)`, never retried. The stream is empty: no byte of a broken stream is kept.
    ChunkProtocol,
}

/// The end of one launch of role `R`: the host-observed [`Ended`] and elapsed time, the plan's [`LaunchKey`], the
/// hostile output stream, still unread, and the hostile console bytes.
///
/// No derives. [`Finished::new`] is the only constructor and consumes the launch's [`Cleared<R>`].
pub struct Finished<R: Role> {
    key: LaunchKey,
    ended: Ended,
    stream: Vec<u8>,
    elapsed: Duration,
    console: Vec<u8>,
    role: PhantomData<fn() -> R>,
}

impl<R: Role> Finished<R> {
    /// The end of the cleared launch `cleared`: `ended` as the host observed it; `stream`, the reassembled output
    /// tar, unread; `elapsed`, the time the host measured from spawn to exit; and `console`, the console bytes the
    /// backend captured under its cap.
    ///
    /// Requiring the `Cleared` of role `R` means output of a role exists only for a cleared launch of that role.
    /// It consumes the launch's clearance, so one cleared launch yields at most one `Finished` (review WR-03).
    ///
    /// For [`Ended::ChunkProtocol`] the stream is discarded here, whatever the backend passed, so no byte of a
    /// broken stream is ever kept.
    #[must_use]
    pub fn new(
        cleared: Cleared<R>,
        ended: Ended,
        stream: Vec<u8>,
        elapsed: Duration,
        console: Vec<u8>,
    ) -> Self {
        let stream = if ended == Ended::ChunkProtocol {
            Vec::new()
        } else {
            stream
        };
        Self {
            key: cleared.into_key(),
            ended,
            stream,
            elapsed,
            console,
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

    /// The time the host measured from spawn to exit. Trusted: the guest reports nothing about itself. The runner
    /// derives usage from it and the plan's machine (LOOP-03).
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// The sanitized tail of the captured console, through [`LogExcerpt::from_bytes`]: at most `max` bytes,
    /// clamped to 4096, with control characters dropped and invisible format characters shown as `<U+XXXX>`.
    ///
    /// The only exit for console bytes, which the guest influenced, so they never reach a terminal or the ledger
    /// raw (KRUN-01).
    #[must_use]
    pub fn console_excerpt(&self, max: usize) -> LogExcerpt {
        LogExcerpt::from_bytes(&self.console, max)
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
