//! Preparing a project for a run, and opening its store for the commands that only read (08-CONTEXT D-06;
//! 08-RESEARCH Pattern 2, Pitfall 8, G7).
//!
//! `minos check` and `minos run` share one preparation path, [`prepare`], which runs, in order:
//!
//! 1. the project directory, canonicalized;
//! 2. the manifest, `<project>/minos.toml`, read under [`MAX_MANIFEST_BYTES`] and validated, keeping its exact
//!    bytes for the run's `ManifestId`;
//! 3. `minos.lock`; with none, the user must run `minos env build` first (D-06);
//! 4. the source snapshot with the lock's judge file digests, so no packed file equals pushed judge data
//!    (07-CONTEXT D-04), and the clean check unless `--allow-dirty` (CLI-11);
//! 5. the base surface and the program bytes, read back from the snapshot's own tarball, so a file edited after the
//!    pack never reaches the prompt or the base (08-RESEARCH G3);
//! 6. the Dockerfile from that tarball and the guest binary from disk, hashed for [`Lock::check`], which refuses a
//!    lock with no image (`minos env build`) or a changed Dockerfile or guest (`minos env build --update`; 05-CONTEXT
//!    D-11);
//! 7. the judge entry for the manifest's `[judge] version` label; with none, `minos judge push <dir>` (07-CONTEXT
//!    D-02);
//! 8. the store at `<project>/.minos`, which must hold the root image and the judge data the lock names, so a
//!    deleted blob is caught here and not inside a VM launch;
//! 9. `prices.toml`, required so a run is never priced at zero (07-CONTEXT D-13); with none, `minos init` writes
//!    a template (08-RESEARCH G7);
//! 10. the base tarball stored under its tree ID, which is where the runner fetches it.
//!
//! Every refusal names the next command to run, and carries paths through [`SafeText`] or [`RelPath`], never file
//! bytes. There is no implicit image build and no implicit push: those stay explicit, auditable commands (D-06).
//!
//! The read-only commands (`status`, `log`, `show`, `report`, `adopt`, `stop`) never prepare: [`open_runs`] opens an
//! existing store and creates nothing, and [`base_surface`] rebuilds a run's base surface from its stored base
//! tarball alone, so they read nothing from the working tree.
//!
//! [`Lock::check`]: minos_core::lock::Lock::check

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use minos_core::diff::cumulative::BaseTree;
use minos_core::guest::MAX_BASE_TAR_BYTES;
use minos_core::id::{BlobId, ImageDigest, ManifestId, TreeId};
use minos_core::ledger::BackendKind;
use minos_core::limits::MAX_MANIFEST_BYTES;
use minos_core::lock::LockMismatch;
use minos_core::manifest::{JudgeVersion, Manifest, ManifestError};
use minos_core::money::{Prices, PricesError};
use minos_core::path::RelPath;
use minos_core::state::RunState;
use minos_core::text::SafeText;
use minos_loop::RunInputs;
use minos_runner::JudgeExtent;
use minos_sandbox::JudgeData;
use minos_source::{CleanPolicy, PackReport, ReadError, SnapshotError, read_files, snapshot};
use minos_store::{Blobs, FileStore, STORE_DIR, StoreError};

use crate::env_build::{LOCK_FILE, MANIFEST_FILE};
use crate::jobs::judge_inputs;
use crate::locate::GUEST_MAX_BYTES;
use crate::lock_file::{LockFileError, read_lock};

/// The price table's file name, under `~/.config/minos` (CLAUDE.md "Cost").
pub const PRICES_FILE: &str = "prices.toml";

/// The most bytes read from `prices.toml`.
pub const MAX_PRICES_BYTES: u64 = 1_048_576;

/// The price table's path for the home directory `home`: `<home>/.config/minos/prices.toml`.
#[must_use]
pub fn prices_path(home: &Path) -> PathBuf {
    home.join(".config").join("minos").join(PRICES_FILE)
}

/// The price table at `path`, read under [`MAX_PRICES_BYTES`] and parsed.
///
/// # Errors
///
/// [`PrepareError::PricesMissing`] naming `path` when there is no file there, [`PrepareError::Read`] or
/// [`PrepareError::TooLarge`] when it cannot be read or is over the cap, and [`PrepareError::Prices`] when it does
/// not parse.
pub fn read_prices(path: &Path) -> Result<Prices, PrepareError> {
    let bytes = match read_capped(path, PRICES_FILE, MAX_PRICES_BYTES) {
        Err(PrepareError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            return Err(PrepareError::PricesMissing {
                path: SafeText::inline(&path.to_string_lossy()),
            });
        }
        other => other?,
    };
    let text = utf8(bytes, PRICES_FILE)?;
    Prices::parse_toml(&text).map_err(PrepareError::Prices)
}

/// Everything `check` and `run` build a session from, prepared in the order the [module docs](self) give.
///
/// The fields are private and there is no setter: a `Prepared` exists only when every step passed.
#[derive(Debug)]
pub struct Prepared {
    project: PathBuf,
    store: FileStore,
    manifest: Manifest,
    manifest_bytes: Vec<u8>,
    root_image: ImageDigest,
    judge_data: JudgeData,
    judge: JudgeVersion,
    judge_extent: JudgeExtent,
    prices: Prices,
    base: BaseTree,
    program: Vec<u8>,
    pack: PackReport,
}

impl Prepared {
    /// The canonical project directory.
    #[must_use]
    pub fn project(&self) -> &Path {
        &self.project
    }

    /// The project's store at `<project>/.minos`, which holds the base tarball, the root image, and the judge data.
    #[must_use]
    pub const fn store(&self) -> &FileStore {
        &self.store
    }

    /// The validated manifest.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The exact `minos.toml` bytes the manifest was parsed from.
    #[must_use]
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// The manifest's ID: [`ManifestId::of`] its exact bytes.
    #[must_use]
    pub fn manifest_id(&self) -> ManifestId {
        ManifestId::of(&self.manifest_bytes)
    }

    /// The root image digest `Lock::check` returned.
    #[must_use]
    pub const fn root_image(&self) -> ImageDigest {
        self.root_image
    }

    /// The judge data the lock records under the manifest's label.
    #[must_use]
    pub const fn judge_data(&self) -> JudgeData {
        self.judge_data
    }

    /// The judge version: the judge data tree and the manifest's judge spec.
    #[must_use]
    pub const fn judge(&self) -> JudgeVersion {
        self.judge
    }

    /// The judge data's recorded bytes and file count, which the runner checks `scratch_mib` against.
    #[must_use]
    pub const fn judge_extent(&self) -> JudgeExtent {
        self.judge_extent
    }

    /// The validated price table.
    #[must_use]
    pub const fn prices(&self) -> &Prices {
        &self.prices
    }

    /// The run's base surface, from the snapshot whose tarball is stored under `base().id()`.
    #[must_use]
    pub const fn base(&self) -> &BaseTree {
        &self.base
    }

    /// The program file as packed in the snapshot.
    #[must_use]
    pub fn program(&self) -> &[u8] {
        &self.program
    }

    /// What the snapshot packed and what the denylist dropped.
    #[must_use]
    pub const fn pack(&self) -> &PackReport {
        &self.pack
    }

    /// The inputs a driver session reads, borrowed from this preparation.
    #[must_use]
    pub const fn run_inputs(&self) -> RunInputs<'_> {
        RunInputs {
            manifest: &self.manifest,
            manifest_bytes: self.manifest_bytes.as_slice(),
            base: &self.base,
            program: self.program.as_slice(),
            prices: &self.prices,
            root_image: self.root_image,
            judge: self.judge,
        }
    }
}

/// Why a project could not be prepared or its store opened. Each refusal the user can fix names the command to
/// run next; messages carry paths through [`SafeText`] or [`RelPath`] and digests only, never file bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    /// A file or directory could not be read.
    #[error("could not read {what}")]
    Read {
        /// Which file or directory.
        what: &'static str,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A file is over its cap.
    #[error("{what} is over {max} bytes")]
    TooLarge {
        /// Which file.
        what: &'static str,
        /// The cap.
        max: u64,
    },
    /// `minos.toml` is not a valid manifest.
    #[error("minos.toml is refused")]
    Manifest(#[source] ManifestError),
    /// `minos.lock` could not be read or parsed.
    #[error("minos.lock could not be read")]
    LockFile(#[source] LockFileError),
    /// There is no `minos.lock`, so no root image has been built.
    #[error("minos.lock not found: run `minos env build`")]
    NoLock,
    /// The source snapshot was refused.
    #[error("the source snapshot is refused")]
    Snapshot(#[source] SnapshotError),
    /// The surface or program could not be read back from the snapshot.
    #[error("the source snapshot does not read back")]
    Surface(#[source] ReadError),
    /// The Dockerfile the manifest names is not in the source snapshot.
    #[error(
        "the Dockerfile {path} is not in the source snapshot; check [source] exclude and the ignore files, then run `minos env build`"
    )]
    DockerfileMissing {
        /// The Dockerfile's project path.
        path: RelPath,
    },
    /// The lock does not match the files on disk, or has no judge data for the manifest's label. The message
    /// names `minos env build`, `minos env build --update`, or `minos judge push`.
    #[error(transparent)]
    Lock(LockMismatch),
    /// The store could not be opened, read, or written.
    #[error("the store in .minos could not be used")]
    Store(#[source] StoreError),
    /// The root image the lock records is not in the store.
    #[error(
        "the root image sha256 {image} that minos.lock records is not in .minos/blobs: run `minos env build`"
    )]
    RootImageMissing {
        /// The root image digest the lock records.
        image: ImageDigest,
    },
    /// The judge data the lock records for the manifest's label is not in the store.
    #[error(
        "the judge data minos.lock records for label {label} is not in .minos/blobs: run `minos judge push <dir>`"
    )]
    JudgeDataMissing {
        /// The manifest's `[judge] version` label.
        label: u32,
    },
    /// There is no `prices.toml`, so the run cannot be priced (07-CONTEXT D-13).
    #[error(
        "{path} not found: run `minos init` to write a template, then price every model your proposers name"
    )]
    PricesMissing {
        /// The path looked at.
        path: SafeText,
    },
    /// `prices.toml` does not parse.
    #[error("prices.toml is refused")]
    Prices(#[source] PricesError),
    /// The stored base tarball's blob ID is not the snapshot's tree: an infrastructure fault.
    #[error("the stored base blob sha256 {stored} is not the snapshot tree sha256 {tree}")]
    BaseNotStored {
        /// The snapshot's tree.
        tree: TreeId,
        /// The blob ID the store returned.
        stored: BlobId,
    },
    /// The project has no store, so no run was ever recorded in it.
    #[error("no runs recorded in this project (.minos does not exist)")]
    NoRuns,
}

/// Prepares the project at `project` for a run, with the guest binary at `guest` and the price table at `prices`,
/// in the order the [module docs](self) give. `policy` is [`CleanPolicy::AllowDirty`] only for `--allow-dirty`.
///
/// # Errors
///
/// A [`PrepareError`] for the first step that fails. Nothing is written before step 10, which stores only the base
/// tarball under its own content address.
pub fn prepare(
    project: &Path,
    guest: &Path,
    prices: &Path,
    policy: CleanPolicy,
) -> Result<Prepared, PrepareError> {
    // 1. The project directory.
    let project = fs::canonicalize(project).map_err(|source| PrepareError::Read {
        what: "the project directory",
        source,
    })?;

    // 2. The manifest, with its exact bytes.
    let manifest_bytes = read_capped(
        &project.join(MANIFEST_FILE),
        MANIFEST_FILE,
        MAX_MANIFEST_BYTES,
    )?;
    let manifest_text = utf8(manifest_bytes, MANIFEST_FILE)?;
    let manifest = Manifest::parse_toml(&manifest_text).map_err(PrepareError::Manifest)?;
    let manifest_bytes = manifest_text.into_bytes();

    // 3. The lock.
    let lock = read_lock(&project.join(LOCK_FILE))
        .map_err(PrepareError::LockFile)?
        .ok_or(PrepareError::NoLock)?;

    // 4. The snapshot, under the lock's judge file digests.
    let snap = snapshot(&project, &manifest, policy, &lock.judge_file_digests())
        .map_err(PrepareError::Snapshot)?;

    // 5. The surface and program, from the packed bytes.
    let (surface, program) = snap.surface(&manifest).map_err(PrepareError::Surface)?;

    // 6. The Dockerfile as packed and the guest on disk, against the lock's image entry.
    let dockerfile = manifest.environment().dockerfile();
    let wanted = BTreeSet::from([dockerfile.clone()]);
    let files = read_files(snap.tarball(), &wanted).map_err(|error| match error {
        ReadError::Missing { .. } => PrepareError::DockerfileMissing {
            path: dockerfile.clone(),
        },
        other => PrepareError::Surface(other),
    })?;
    let dockerfile_sha256 =
        files
            .get(dockerfile)
            .map(BlobId::of)
            .ok_or_else(|| PrepareError::DockerfileMissing {
                path: dockerfile.clone(),
            })?;
    let guest_sha256 = BlobId::of(&read_capped(guest, "the guest binary", GUEST_MAX_BYTES)?);
    let image = lock
        .check(
            BackendKind::Krun,
            manifest.environment().arch(),
            &dockerfile_sha256,
            &guest_sha256,
        )
        .map_err(PrepareError::Lock)?;
    let root_image = *image.root_image();

    // 7. The judge entry for the manifest's label.
    let (judge_data, judge, judge_extent) =
        judge_inputs(&lock, &manifest).map_err(PrepareError::Lock)?;

    // 8. The store, holding the root image and the judge data.
    let store = FileStore::open(&project.join(STORE_DIR)).map_err(PrepareError::Store)?;
    if !store
        .contains(&root_image.to_blob_id())
        .map_err(PrepareError::Store)?
    {
        return Err(PrepareError::RootImageMissing { image: root_image });
    }
    if !store
        .contains(judge_data.blob())
        .map_err(PrepareError::Store)?
    {
        return Err(PrepareError::JudgeDataMissing {
            label: manifest.judge().label(),
        });
    }

    // 9. The price table.
    let prices = read_prices(prices)?;

    // 10. The base tarball, stored under its tree.
    let tree = snap.tree();
    let stored = store.put(snap.tarball()).map_err(PrepareError::Store)?;
    if stored != tree.to_blob_id() {
        return Err(PrepareError::BaseNotStored { tree, stored });
    }

    Ok(Prepared {
        project,
        store,
        manifest,
        manifest_bytes,
        root_image,
        judge_data,
        judge,
        judge_extent,
        prices,
        base: BaseTree::new(tree, surface),
        program,
        pack: snap.report().clone(),
    })
}

/// The store of the project at `project`, for a command that only reads runs. It creates nothing: a project with
/// no `.minos` has recorded no run (08-RESEARCH Pitfall 8).
///
/// # Errors
///
/// [`PrepareError::NoRuns`] when `<project>/.minos` does not exist, [`PrepareError::Read`] when it cannot be
/// inspected, and [`PrepareError::Store`] when [`FileStore::open`] refuses it (a symlink or not a directory).
pub fn open_runs(project: &Path) -> Result<FileStore, PrepareError> {
    let root = project.join(STORE_DIR);
    match fs::symlink_metadata(&root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(PrepareError::NoRuns),
        Err(source) => Err(PrepareError::Read {
            what: "the store directory",
            source,
        }),
        Ok(_) => FileStore::open(&root).map_err(PrepareError::Store),
    }
}

/// The base surface of the run `state` folds, rebuilt from its stored base tarball alone: the manifest's writable
/// and readable files, read back under [`MAX_BASE_TAR_BYTES`]. `show`, `report`, and `adopt` use it, so they read
/// nothing from the working tree.
///
/// # Errors
///
/// [`PrepareError::Store`] when the base blob is missing, over the cap, or tampered, and [`PrepareError::Surface`]
/// when it does not read back or lacks a surface path.
pub fn base_surface<S: Blobs>(
    store: &S,
    state: &RunState,
    manifest: &Manifest,
) -> Result<BaseTree, PrepareError> {
    let tarball = store
        .get(&state.base().to_blob_id(), MAX_BASE_TAR_BYTES)
        .map_err(PrepareError::Store)?;
    let paths: BTreeSet<RelPath> = manifest
        .writable()
        .iter()
        .chain(manifest.readable())
        .cloned()
        .collect();
    let tree = read_files(&tarball, &paths).map_err(PrepareError::Surface)?;
    Ok(BaseTree::new(*state.base(), tree))
}

/// Reads at most `max` bytes from `path`, refusing a longer file.
fn read_capped(path: &Path, what: &'static str, max: u64) -> Result<Vec<u8>, PrepareError> {
    let file = File::open(path).map_err(|source| PrepareError::Read { what, source })?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| PrepareError::Read { what, source })?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max) {
        return Err(PrepareError::TooLarge { what, max });
    }
    Ok(bytes)
}

/// `bytes` as UTF-8 text.
fn utf8(bytes: Vec<u8>, what: &'static str) -> Result<String, PrepareError> {
    String::from_utf8(bytes).map_err(|_| PrepareError::Read {
        what,
        source: io::Error::new(io::ErrorKind::InvalidData, "not UTF-8 text"),
    })
}
