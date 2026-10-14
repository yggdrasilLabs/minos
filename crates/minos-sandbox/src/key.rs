//! The launch key: the idempotency identity of one sandbox launch (SBX-06).
//!
//! A launch key hashes the experiment, the role, the judge version, and the trial, through the one composite-ID
//! builder, [`IdHasher`]. It matches the key fields of a `Judged` event (experiment, judge version, trial) plus
//! the role, so a rejudge under a new judge version is a new launch and never reuses a cached result. A backend
//! derives its idempotency token from it.

use std::fmt;

use minos_core::id::{ExperimentId, IdHasher};
use minos_core::manifest::JudgeVersion;

use crate::role::Role;

/// Domain of [`LaunchKey`].
const LAUNCH_DOMAIN: &str = "minos/launch/v1";

/// The identity of one sandbox launch: experiment, role, judge version, and trial.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaunchKey([u8; 32]);

impl LaunchKey {
    /// The key of trial `trial` of `exp` in role `R` under judge version `judge`.
    ///
    /// The hash is `IdHasher::new("minos/launch/v1")`, then the experiment digest, the role tag as a `u64`, the
    /// judge version digest, and the trial as a `u64`.
    #[must_use]
    pub fn of<R: Role>(exp: &ExperimentId, judge: &JudgeVersion, trial: u32) -> Self {
        Self(
            IdHasher::new(LAUNCH_DOMAIN)
                .digest(exp.as_bytes())
                .u64(R::KIND.tag())
                .digest(judge.as_bytes())
                .u64(u64::from(trial))
                .finish(),
        )
    }

    /// The 32 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The digest as 64 lowercase hex characters.
    #[must_use]
    pub fn to_hex(&self) -> String {
        base16ct::lower::encode_string(&self.0)
    }
}

impl fmt::Display for LaunchKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0_u8; 64];
        let text = base16ct::lower::encode_str(&self.0, &mut buf).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}

impl fmt::Debug for LaunchKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LaunchKey(")?;
        fmt::Display::fmt(self, f)?;
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::LAUNCH_DOMAIN;

    #[test]
    fn launch_domain_has_no_nul() {
        assert!(!LAUNCH_DOMAIN.as_bytes().contains(&0));
    }
}
