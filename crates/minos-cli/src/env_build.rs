//! `minos env build`: the project's Dockerfile becomes a raw read-only `SquashFS` root image with the trusted
//! `minos-guest` as `/init`, stored in `.minos/blobs`, and `minos.lock` records it (KRUN-09, CLI-02; 05-CONTEXT
//! D-09 to D-12).
//!
//! [`env_build`] runs, in order:
//!
//! 1. the manifest, `<project>/minos.toml`, read under [`MAX_MANIFEST_BYTES`] and validated;
//! 2. the source snapshot (`minos_source::snapshot`), with the clean check unless `--allow-dirty`, the same
//!    semantics as `minos run` (CLI-11). Its tarball is the whole build context: judge paths are excluded, the hard
//!    denylist (`.git/`, `.minos/`, `.env*`, `minos.lock`, keys) applies, any non-empty file equal to pushed judge
//!    data refuses it (the existing `minos.lock`'s `[[judge]]` file digests, read first; 07-CONTEXT D-04), and the
//!    secret scan has run over exactly the packed bytes, so the builder never sees judge data or a secret
//!    (KRUN-09, JUDGE-01);
//! 3. the Dockerfile, taken from that tarball so its SHA-256 is of the bytes the builder was given, read under
//!    [`MAX_DOCKERFILE_BYTES`], with every `FROM` pinned by digest (`image::base_images`);
//! 4. the guest binary, read under [`GUEST_MAX_BYTES`] and checked to be a static `AArch64` ELF before anything
//!    is built, because a Mach-O guest packed as `/init` would never boot and the run would be blamed on the
//!    candidate (05-RESEARCH Pitfall 8);
//! 5. the builder's identity, then the build, with its stdout streamed straight into the trusted packer, which
//!    writes the guest last as `/init` so a Dockerfile cannot replace it; the builder's tar is hostile input and
//!    is never unpacked on the host;
//! 6. the packed image stored with `Blobs::put_reader`; its blob ID is the image digest (D-09);
//! 7. `minos.lock`: a first build writes it; a build whose image digest matches the locked one leaves the image
//!    entry alone; a different digest is refused, naming both digests, unless `--update` (D-10). The lock is
//!    written only after the blob is stored, through a temporary file in `.minos/` that is synced and renamed over
//!    `minos.lock`, so a reader sees the old lock or the new one, never part of one.
//!
//! The D-09 limit, stated plainly: the streaming put reads from an in-memory packed image. `pack_root_image`
//! returns the whole `SquashFS` image as bytes, bounded by `PackLimits`' 1 GiB tar cap, and `put_reader` streams
//! those bytes into the store with no second whole-slice copy. Streaming the packer's output straight into the
//! store, so the image is never whole in memory, is a follow-up (recorded in CLAUDE.md Open questions by plan
//! 05-18); the packer is unchanged in this phase.
//!
//! Reproducibility (D-12): the packed image reproduces, because the packer zeroes every mtime, uid, and gid and
//! orders nodes by path (02-SPIKE-EVIDENCE.md RI-04). The builder's tar is not claimed to reproduce, since not
//! every OCI builder honors `SOURCE_DATE_EPOCH`.
//!
//! Deferred to Phase 7: D-10's lineage sentence, how the lock's digest feeds the manifest's image reference and
//! hence `ManifestId`, so the next run after `--update` records a new lineage, is implemented when Phase 7 wires
//! `minos run` (recorded in 05-CONTEXT.md Deferred Ideas). Meanwhile each `Judged` event records the root image
//! in `BackendInfo`.
//!
//! The Dockerfile's `USER` is ignored: every image file is root-owned after packing, and the guest runs the build
//! and run commands as uid 64000 (05-CONTEXT D-08). Install toolchains into world-readable prefixes such as
//! `/usr/local`, and set `CARGO_HOME` and `RUSTUP_HOME` there, never under `/root` (05-RESEARCH Pitfall 9).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use minos_backend_krun::rootfs::{
    ElfError, PackError, PackLimits, PackReport, check_guest_elf, pack_root_image,
};
use minos_core::id::{BlobId, ImageDigest};
use minos_core::image::{ImageError, ImageRef, base_images};
use minos_core::ledger::BackendKind;
use minos_core::limits::MAX_MANIFEST_BYTES;
use minos_core::lock::{Lock, LockError, LockedImage};
use minos_core::manifest::{Arch, Manifest, ManifestError};
use minos_core::path::RelPath;
use minos_core::text::SafeText;
use minos_source::{CleanPolicy, SnapshotError, snapshot};
use minos_store::{Blobs, FileStore, STORE_DIR, StoreError};

use crate::builder::{BuilderError, RootfsBuilder};
use crate::locate::GUEST_MAX_BYTES;
use crate::lock_file::{self, LockFileError, write_lock};

/// The most bytes [`env_build`] stores as one root image. The packer's 1 GiB tar cap (`PackLimits::default`)
/// bounds the file content, and the compressed image is smaller than that content plus `SquashFS` metadata in
/// practice; the image is held in memory when it is stored (the D-09 limit in the [module docs](self)).
pub const MAX_ROOT_IMAGE_BYTES: u64 = 1_073_741_824;

/// The most bytes read from the Dockerfile.
pub const MAX_DOCKERFILE_BYTES: u64 = 1024 * 1024;

/// The manifest's file name in a project.
pub const MANIFEST_FILE: &str = "minos.toml";

/// The lock's file name in a project.
pub const LOCK_FILE: &str = "minos.lock";

/// The backend `env build` builds for.
const BACKEND: BackendKind = BackendKind::Krun;

/// The architecture `env build` builds for: the krun backend boots `AArch64` guests only.
const ARCH: Arch = Arch::Arm64;

/// The flags of one `env build`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnvBuildOptions {
    update: bool,
    allow_dirty: bool,
}

impl EnvBuildOptions {
    /// `update`: accept an image digest that differs from `minos.lock` (`--update`). `allow_dirty`: skip only the
    /// clean check (`--allow-dirty`).
    #[must_use]
    pub const fn new(update: bool, allow_dirty: bool) -> Self {
        Self {
            update,
            allow_dirty,
        }
    }
}

/// What one `env build` produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvBuildReport {
    image: ImageDigest,
    lock_written: bool,
    pack: PackReport,
    base_images: Vec<ImageRef>,
}

impl EnvBuildReport {
    /// The packed root image's digest, equal to its blob ID in `.minos/blobs` (D-09).
    #[must_use]
    pub const fn image(&self) -> &ImageDigest {
        &self.image
    }

    /// Whether `minos.lock` was written; `false` when it already recorded exactly this build.
    #[must_use]
    pub const fn lock_written(&self) -> bool {
        self.lock_written
    }

    /// What the packer did.
    #[must_use]
    pub const fn pack(&self) -> &PackReport {
        &self.pack
    }

    /// The Dockerfile's `FROM` pins, sorted, each once.
    #[must_use]
    pub fn base_images(&self) -> &[ImageRef] {
        &self.base_images
    }
}

/// Why `env build` stopped. Messages name paths through [`SafeText`] or [`RelPath`] and never print file bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum EnvBuildError {
    /// A file could not be read.
    #[error("could not read {what}")]
    Read {
        /// Which file.
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
    /// The manifest asks for an architecture the krun backend cannot boot.
    #[error(
        "minos env build builds arm64 root images only; set [environment] requires = {{ arch = \"arm64\" }}"
    )]
    Arch,
    /// The source snapshot, the build context, was refused.
    #[error("the build context (the source snapshot) is refused")]
    Snapshot(#[source] SnapshotError),
    /// The Dockerfile is not in the snapshot, so the builder could not see it.
    #[error(
        "the Dockerfile {path} is not in the build context; an ignore rule, [source] exclude, or the denylist dropped it"
    )]
    DockerfileMissing {
        /// The manifest's Dockerfile path.
        path: RelPath,
    },
    /// The Dockerfile is over [`MAX_DOCKERFILE_BYTES`] or not UTF-8.
    #[error("the Dockerfile {path} is over {MAX_DOCKERFILE_BYTES} bytes or is not UTF-8 text")]
    Dockerfile {
        /// The manifest's Dockerfile path.
        path: RelPath,
    },
    /// A `FROM` line is not pinned by digest, or the Dockerfile cannot be read for its `FROM` lines.
    #[error("the Dockerfile's base images are refused")]
    BaseImages(#[source] ImageError),
    /// The guest binary is not a static `AArch64` ELF.
    #[error("the guest binary {path} cannot be written in as /init")]
    Guest {
        /// The guest binary's path.
        path: SafeText,
        /// What is wrong with it.
        #[source]
        source: ElfError,
    },
    /// The builder failed.
    #[error("the container builder failed")]
    Builder(#[source] BuilderError),
    /// The builder's rootfs tar could not be packed.
    #[error("the builder's rootfs tar could not be packed into a root image")]
    Pack(#[source] PackError),
    /// The root image could not be stored.
    #[error("the root image could not be stored in .minos/blobs")]
    Store(#[source] StoreError),
    /// `minos.lock` is refused or could not be serialized.
    #[error("minos.lock is refused")]
    Lock(#[source] LockError),
    /// `minos.lock` could not be written.
    #[error("could not write minos.lock")]
    LockWrite(#[source] io::Error),
    /// The built image differs from the locked one, and `--update` was not passed (D-10). The new blob stays in
    /// the store, which is harmless: nothing names it.
    #[error(
        "the built root image sha256 {built} differs from minos.lock's sha256 {locked}; rerun with --update to accept the new image"
    )]
    LockMismatch {
        /// The digest `minos.lock` records.
        locked: ImageDigest,
        /// The digest of the image just built.
        built: ImageDigest,
    },
}

/// Builds the project at `project` into a root image with `guest` as `/init`, using `builder`, and records it in
/// `minos.lock`, in the order the [module docs](self) give.
///
/// # Errors
///
/// An [`EnvBuildError`] for the first step that fails. [`EnvBuildError::LockMismatch`] leaves `minos.lock`
/// unchanged.
pub fn env_build<B: RootfsBuilder>(
    project: &Path,
    guest: &Path,
    builder: &B,
    options: EnvBuildOptions,
) -> Result<EnvBuildReport, EnvBuildError> {
    // 1. The manifest.
    let manifest_text = read_text(
        &project.join(MANIFEST_FILE),
        MANIFEST_FILE,
        MAX_MANIFEST_BYTES,
    )?;
    let manifest = Manifest::parse_toml(&manifest_text).map_err(EnvBuildError::Manifest)?;
    if manifest.environment().arch() != ARCH {
        return Err(EnvBuildError::Arch);
    }

    // 2. The build context: the source snapshot, and nothing else. The lock's judge file digests are read first,
    // so no file equal to pushed judge data reaches the builder (D-04, JUDGE-01).
    let policy = if options.allow_dirty {
        CleanPolicy::AllowDirty
    } else {
        CleanPolicy::RequireClean
    };
    let lock_path = project.join(LOCK_FILE);
    let judge_files = load_lock(&lock_path)?
        .as_ref()
        .map(Lock::judge_file_digests)
        .unwrap_or_default();
    let snapshot =
        snapshot(project, &manifest, policy, &judge_files).map_err(EnvBuildError::Snapshot)?;

    // 3. The Dockerfile as the builder will see it, and its digest pins.
    let dockerfile = manifest.environment().dockerfile();
    let dockerfile_bytes = context_file(snapshot.tarball(), dockerfile)?;
    let dockerfile_text =
        std::str::from_utf8(&dockerfile_bytes).map_err(|_| EnvBuildError::Dockerfile {
            path: dockerfile.clone(),
        })?;
    let base = base_images(dockerfile_text).map_err(EnvBuildError::BaseImages)?;
    let dockerfile_sha256 = BlobId::of(&dockerfile_bytes);

    // 4. The guest, checked before anything is built.
    let guest_bytes = read_capped(guest, "the guest binary", GUEST_MAX_BYTES)?;
    check_guest_elf(&guest_bytes).map_err(|source| EnvBuildError::Guest {
        path: SafeText::inline(&guest.to_string_lossy()),
        source,
    })?;
    let guest_sha256 = BlobId::of(&guest_bytes);

    // 5. The build, streamed into the packer, which writes the guest last as /init.
    let identity = builder.identity().map_err(EnvBuildError::Builder)?;
    let mut build = builder
        .start(snapshot.into_tarball(), dockerfile)
        .map_err(EnvBuildError::Builder)?;
    let packed = pack_root_image(build.rootfs_mut(), &guest_bytes, &PackLimits::default());
    let finished = build.finish();
    let packed = match (packed, finished) {
        (Ok(packed), Ok(())) => packed,
        (Ok(_), Err(error)) => return Err(EnvBuildError::Builder(error)),
        // A builder that failed mid-stream leaves a truncated tar; its own error says why.
        (Err(PackError::MalformedTar { .. } | PackError::Io(_)), Err(error)) => {
            return Err(EnvBuildError::Builder(error));
        }
        (Err(error), _) => return Err(EnvBuildError::Pack(error)),
    };

    // 6. The image into the store. The reader is the in-memory packed image (the D-09 limit in the module docs).
    let store = FileStore::open(&project.join(STORE_DIR)).map_err(EnvBuildError::Store)?;
    let blob = store
        .put_reader(packed.bytes(), MAX_ROOT_IMAGE_BYTES)
        .map_err(EnvBuildError::Store)?;
    let image = ImageDigest::from_blob_id(&blob);

    // 7. The lock, only after the blob is stored. It is read again, so an entry written during the build (a
    // `judge push`) is kept.
    let entry = LockedImage::new(
        BACKEND,
        ARCH,
        image,
        dockerfile_sha256,
        base.clone(),
        guest_sha256,
        identity,
    );
    let lock = load_lock(&lock_path)?;
    let lock_written = match lock.as_ref().and_then(|lock| lock.image(BACKEND, ARCH)) {
        Some(locked) if *locked == entry => false,
        Some(locked) if *locked.root_image() != image && !options.update => {
            return Err(EnvBuildError::LockMismatch {
                locked: *locked.root_image(),
                built: image,
            });
        }
        // A first build, an accepted new image, or the same image from changed inputs (a Dockerfile comment,
        // a builder upgrade): the entry is rewritten, so a later run's lock check sees the inputs on disk.
        _ => {
            let text = lock
                .unwrap_or_else(Lock::empty)
                .with_image(entry)
                .to_toml()
                .map_err(EnvBuildError::Lock)?;
            write_lock(&project.join(STORE_DIR), &lock_path, text.as_bytes())
                .map_err(EnvBuildError::LockWrite)?;
            true
        }
    };

    Ok(EnvBuildReport {
        image,
        lock_written,
        pack: packed.report().clone(),
        base_images: base,
    })
}

/// The bytes of `path` in the snapshot `tarball`, which Minos built itself, so its entries are regular files with
/// valid paths.
fn context_file(tarball: &[u8], path: &RelPath) -> Result<Vec<u8>, EnvBuildError> {
    let missing = || EnvBuildError::DockerfileMissing { path: path.clone() };
    let unreadable = |source| EnvBuildError::Read {
        what: "the build context",
        source,
    };
    let mut archive = tar::Archive::new(tarball);
    for entry in archive.entries().map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        if entry.path_bytes().as_ref() != path.as_str().as_bytes() {
            continue;
        }
        if entry.size() > MAX_DOCKERFILE_BYTES {
            return Err(EnvBuildError::Dockerfile { path: path.clone() });
        }
        let mut bytes = Vec::new();
        entry
            .take(MAX_DOCKERFILE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        return Ok(bytes);
    }
    Err(missing())
}

/// Reads at most `max` bytes of `path`, refusing a longer file.
fn read_capped(path: &Path, what: &'static str, max: u64) -> Result<Vec<u8>, EnvBuildError> {
    let file = File::open(path).map_err(|source| EnvBuildError::Read { what, source })?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| EnvBuildError::Read { what, source })?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max) {
        return Err(EnvBuildError::TooLarge { what, max });
    }
    Ok(bytes)
}

/// Reads `path` as UTF-8 text of at most `max` bytes.
fn read_text(path: &Path, what: &'static str, max: u64) -> Result<String, EnvBuildError> {
    String::from_utf8(read_capped(path, what, max)?).map_err(|_| EnvBuildError::Read {
        what,
        source: io::Error::new(io::ErrorKind::InvalidData, "not UTF-8 text"),
    })
}

/// The lock at `path` through the shared reader, with its errors on `env build`'s own variants, so the messages
/// are those `env build` has always printed.
fn load_lock(path: &Path) -> Result<Option<Lock>, EnvBuildError> {
    lock_file::read_lock(path).map_err(|error| match error {
        LockFileError::Read { source } => EnvBuildError::Read {
            what: LOCK_FILE,
            source,
        },
        LockFileError::Lock(error) => EnvBuildError::Lock(error),
    })
}
