//! What a session of the driver reads from the CLI, and the checks it passes before anything is recorded
//! (07-CONTEXT D-10, D-13, D-19).
//!
//! The CLI builds a [`RunInputs`] from the files on disk: the manifest and its exact bytes, the base snapshot's
//! surface, the program file, `prices.toml`, and the root image and judge version `minos.lock` names. The driver
//! checks it once at start ([`check_start`]) and once at every resume ([`check_resume`]), before any VM or model
//! call, and each refusal names both values so the user can see what changed:
//!
//! | Refusal | Why | What the user does |
//! |---|---|---|
//! | [`DriverError::PricesMissing`] | `prices.toml` has no rate for a model a proposer names (D-13) | Add the rate; a run is never priced at zero |
//! | [`DriverError::BaseMismatch`] | The working tree's snapshot is not the run's base | Restore the tree, or start a new run |
//! | [`DriverError::RootImageChanged`] | `minos.lock` names another root image, or the run recorded none (D-10) | `env build --update` means a new run |
//! | [`DriverError::JudgeChanged`] | The baseline was judged only under another judge version | Judge data pushed again means a new run |
//! | [`DriverError::ManifestChanged`] | The experiment frame changed (D-19) | A frame edit means a new run |
//! | [`DriverError::ManifestRecordUnreadable`] | The recorded manifest blob is missing, tampered, or no longer parses (07-REVIEW WR-03) | Look at the store; it is never a manifest edit |
//!
//! # Manifest edits (D-19)
//!
//! A changed manifest does not by itself refuse a resume. The *experiment frame* is what an experiment means: the
//! writable and readable surface, the program path, the source excludes, the commands, the environment, and the
//! judge spec. When the recorded manifest and the current one share it, the resume continues and the driver
//! records `ManifestChanged` from the next generation, so a run stopped by a spend cap or the breaker resumes
//! once the user edits `[budget]`, `[policy]`, the proposers, or the params. A frame edit is refused, because a
//! candidate scored under one frame cannot be compared with one scored under another. This supersedes
//! 07-RESEARCH Assumption A6, which refused every manifest change.

use minos_core::diff::cumulative::BaseTree;
use minos_core::id::{ImageDigest, ManifestId};
use minos_core::manifest::{JudgeVersion, Manifest};
use minos_core::money::Prices;
use minos_core::state::RunState;

use crate::error::DriverError;

/// Everything a session reads from the CLI: a parameter bundle of borrowed, already validated values, not a
/// validated type itself. [`check_start`] and [`check_resume`] check it against the run before anything is
/// recorded.
#[derive(Debug, Clone, Copy)]
pub struct RunInputs<'a> {
    /// The validated manifest.
    pub manifest: &'a Manifest,
    /// The exact `minos.toml` bytes the manifest was parsed from. The driver stores them as a blob, whose ID has
    /// the manifest ID's digest, so a later resume can compare experiment frames (D-19).
    pub manifest_bytes: &'a [u8],
    /// The run's base surface, from the snapshot whose tarball hashes to `base.id()`.
    pub base: &'a BaseTree,
    /// The program file as packed in that snapshot.
    pub program: &'a [u8],
    /// The validated `prices.toml` (D-13).
    pub prices: &'a Prices,
    /// The root image digest `Lock::check` returned (D-10).
    pub root_image: ImageDigest,
    /// `JudgeVersion::of` the lock's judge data tree for the manifest's label and the manifest's judge spec.
    pub judge: JudgeVersion,
}

impl RunInputs<'_> {
    /// The manifest's ID: [`ManifestId::of`] its exact bytes.
    #[must_use]
    pub fn manifest_id(&self) -> ManifestId {
        ManifestId::of(self.manifest_bytes)
    }

    /// The size a single-shot prompt must carry, in bytes: the program file plus every base surface file,
    /// saturating. `SurfaceStats` is built from it, so the driver's routing and `minos check`'s route table see
    /// the same figure.
    #[must_use]
    pub fn surface_bytes(&self) -> u64 {
        let size = |len: usize| u64::try_from(len).unwrap_or(u64::MAX);
        self.base
            .tree()
            .iter()
            .fold(size(self.program.len()), |total, (_, bytes)| {
                total.saturating_add(size(bytes.len()))
            })
    }
}

/// How a resume relates to the manifest the run recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resume {
    /// The current manifest is the one in effect.
    Unchanged,
    /// The manifest changed outside the experiment frame. The driver records `ManifestChanged` with this
    /// `from_generation`, one past the last concluded generation (D-19).
    ManifestChanged {
        /// The first generation that uses the current manifest.
        from_generation: u32,
    },
}

/// Refuses a session whose inputs cannot price the run, before any VM or model call (D-13).
///
/// # Errors
///
/// [`DriverError::PricesMissing`] naming every model a proposer names that `prices.toml` does not price.
pub fn check_start(inputs: &RunInputs<'_>) -> Result<(), DriverError> {
    let models = inputs.prices.missing_models(inputs.manifest);
    if models.is_empty() {
        Ok(())
    } else {
        Err(DriverError::PricesMissing { models })
    }
}

/// Whether `recorded` and `current` share the experiment frame: the writable and readable surface, the program
/// path, the source excludes, the commands, the environment, and the judge spec (D-19).
///
/// Budgets, the policy, the proposers, and the params are outside the frame: they decide how the loop searches,
/// not what an experiment means.
#[must_use]
pub fn same_frame(recorded: &Manifest, current: &Manifest) -> bool {
    recorded.writable() == current.writable()
        && recorded.readable() == current.readable()
        && recorded.program() == current.program()
        && recorded.excludes() == current.excludes()
        && recorded.commands() == current.commands()
        && recorded.environment() == current.environment()
        && recorded.judge() == current.judge()
}

/// Checks a resumed session's inputs against the run's record, in order: prices, base, root image, judge
/// version, then the manifest. `recorded` is the manifest the run has in effect, parsed from its stored bytes;
/// the driver refuses the resume before this check when it cannot read them (07-REVIEW WR-03).
///
/// The judge check refuses only when the baseline has recorded trials and none of them is under `inputs.judge`,
/// so a run that has not judged anything yet can resume under any version. A recorded root image of `None` is
/// refused too: a run started without a lock cannot be resumed under a pinned image.
///
/// # Errors
///
/// [`DriverError::PricesMissing`] as [`check_start`]; [`DriverError::BaseMismatch`] when `inputs.base` is not the
/// run's base; [`DriverError::RootImageChanged`] when the recorded root image is absent or differs;
/// [`DriverError::JudgeChanged`] when the baseline was judged only under other versions; and
/// [`DriverError::ManifestChanged`] when the manifest differs and `recorded` does not share the experiment frame
/// with the current one.
pub fn check_resume(
    state: &RunState,
    recorded: &Manifest,
    inputs: &RunInputs<'_>,
) -> Result<Resume, DriverError> {
    check_start(inputs)?;

    let supplied = inputs.base.id();
    if *state.base() != supplied {
        return Err(DriverError::BaseMismatch {
            recorded: *state.base(),
            supplied,
        });
    }

    if state.root_image() != Some(&inputs.root_image) {
        return Err(DriverError::RootImageChanged {
            recorded: state.root_image().copied(),
            current: inputs.root_image,
        });
    }

    let judges = state.judge_versions(state.baseline());
    if let Some(recorded_judge) = judges.first()
        && !judges.contains(&inputs.judge)
    {
        return Err(DriverError::JudgeChanged {
            recorded: *recorded_judge,
            current: inputs.judge,
        });
    }

    let current = inputs.manifest_id();
    if *state.manifest() == current {
        return Ok(Resume::Unchanged);
    }
    if same_frame(recorded, inputs.manifest) {
        Ok(Resume::ManifestChanged {
            from_generation: state.last_concluded_generation().saturating_add(1),
        })
    } else {
        Err(DriverError::ManifestChanged {
            recorded: *state.manifest(),
            current,
        })
    }
}
