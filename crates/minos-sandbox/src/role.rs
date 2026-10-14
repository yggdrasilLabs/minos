//! Sandbox roles as types.
//!
//! A role decides what a sandbox may see. Each role is an uninhabited marker type, and [`Role`] is sealed: its
//! supertrait lives in a private module, so only this crate can implement it. No backend crate can add a role that
//! slips past the rule that only a [`Judge`] sandbox receives judge data, and no role anywhere has a way to attach a
//! network device.
//!
//! v1 has two roles, [`Candidate`] and [`Judge`]. The agent role (the tool loop's VM, TIER-01) is deferred to a
//! later milestone; its tag, 3, is reserved in [`RoleKind::tag`].

/// The private supertrait that seals [`Role`].
mod sealed {
    /// Implemented only in this module's parent crate, so no other crate can implement [`super::Role`].
    pub trait Sealed {}
}

/// A sandbox role. Sealed: implemented only for [`Candidate`] and [`Judge`].
pub trait Role: sealed::Sealed + 'static {
    /// The runtime tag of this role, hashed into the launch key and reported by a launch plan.
    const KIND: RoleKind;
}

/// The candidate role: runs the built experiment.
///
/// A candidate sandbox sees the full source tree with the experiment's diff applied, plus data blobs, read-only.
/// It never sees judge data and has no network device.
#[derive(Debug)]
pub enum Candidate {}

/// The judge role: runs the project's judge command.
///
/// A judge sandbox sees the candidate's artifacts plus the judge's private data, read-only. It has no network
/// device.
#[derive(Debug)]
pub enum Judge {}

impl sealed::Sealed for Candidate {}
impl sealed::Sealed for Judge {}

impl Role for Candidate {
    const KIND: RoleKind = RoleKind::Candidate;
}

impl Role for Judge {
    const KIND: RoleKind = RoleKind::Judge;
}

/// The runtime name of a [`Role`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoleKind {
    /// [`Candidate`].
    Candidate,
    /// [`Judge`].
    Judge,
}

impl RoleKind {
    /// The fixed integer hashed into the launch key: `Candidate` is 1 and `Judge` is 2. Tag 3 is reserved for the
    /// agent role (TIER-01). Written as explicit match arms, never a discriminant cast, so reordering the variants
    /// cannot change a key.
    #[must_use]
    pub const fn tag(self) -> u64 {
        match self {
            Self::Candidate => 1,
            Self::Judge => 2,
        }
    }
}
