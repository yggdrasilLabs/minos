//! `minos.lock`: what `minos env build` produced, per backend and architecture (CLI-02; 05-CONTEXT D-10, D-11).
//!
//! The lock records, for each (backend, arch) pair, the root image's digest, the Dockerfile's SHA-256, every `FROM`
//! base-image pin as [`crate::image::base_images`] extracts it, the SHA-256 of the `minos-guest` binary the packer
//! wrote in as `/init`, and the builder's name and version (D-11). Two rules use it:
//!
//! - A run refuses to start when the Dockerfile or the guest binary on disk no longer matches the lock (D-11).
//!   Phases 7 and 8 call [`Lock::check`] before a run, which reports a [`LockMismatch`] carrying both digests.
//! - `minos env build` refuses an image digest that differs from the lock unless `--update` is passed (D-10), and
//!   `--update` rewrites the entry with [`Lock::with_image`].
//!
//! Only the packed `SquashFS` image is claimed to reproduce; the builder's rootfs tar may differ between builds,
//! because not every OCI builder honors `SOURCE_DATE_EPOCH` (D-12). D-10's lineage sentence, the lock's digest
//! feeding the manifest's image reference and hence `ManifestId`, is deferred to Phase 7, which wires `minos run`;
//! 05-CONTEXT.md records the deferral.
//!
//! The wire form is TOML with `v = 1` and one `[[image]]` table per pair, each with a `[image.builder]` table. Every
//! raw table refuses unknown keys, the text is capped at [`MAX_LOCK_BYTES`] before it is parsed, and the validated
//! types deserialize through the same checks as [`Lock::parse_toml`]: a duplicate (backend, arch) pair and a
//! `base_images` list that is not strictly ascending are refused, so each lock has exactly one wire form. The
//! golden file `tests/snapshots/lock__minos_lock_v1.snap` and its SHA-256 pin guard it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{BlobId, ImageDigest};
use crate::image::ImageRef;
use crate::ledger::BackendKind;
use crate::manifest::Arch;

/// The lock wire version this code reads and writes.
pub const LOCK_VERSION: u32 = 1;

/// Most bytes in a `minos.lock`: longer text is refused before it is parsed.
pub const MAX_LOCK_BYTES: usize = 65_536;

/// Most bytes in a builder name or version.
pub const MAX_BUILDER_TEXT_BYTES: usize = 128;

/// The wire name of a backend, for messages.
const fn backend_name(backend: BackendKind) -> &'static str {
    match backend {
        BackendKind::Krun => "krun",
    }
}

/// The wire name of an architecture, for messages.
const fn arch_name(arch: Arch) -> &'static str {
    match arch {
        Arch::Arm64 => "arm64",
        Arch::X86_64 => "x86_64",
    }
}

/// Why a `minos.lock` was refused. Messages never echo the file's text.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// The text is over [`MAX_LOCK_BYTES`].
    #[error("minos.lock is over {MAX_LOCK_BYTES} bytes")]
    TooLarge,
    /// The text is not the lock's TOML form: a syntax error, a wrong type, a missing or unknown key, or a malformed
    /// digest or image reference.
    #[error("minos.lock is not valid lock TOML")]
    Toml(#[source] toml::de::Error),
    /// The `v` key is not [`LOCK_VERSION`].
    #[error("minos.lock version is not {LOCK_VERSION}")]
    Version,
    /// Two `[[image]]` tables name the same backend and architecture.
    #[error("minos.lock has two images for backend {} and arch {}", backend_name(*backend), arch_name(*arch))]
    DuplicateImage {
        /// The repeated backend.
        backend: BackendKind,
        /// The repeated architecture.
        arch: Arch,
    },
    /// A `base_images` list is repeated or out of order.
    #[error("minos.lock base_images must be listed once each, in ascending order")]
    BaseImagesNotCanonical,
    /// A builder name or version is empty, over [`MAX_BUILDER_TEXT_BYTES`], or has a byte outside ASCII `!` to `~`.
    #[error(
        "a minos.lock builder name or version is 1 to {MAX_BUILDER_TEXT_BYTES} bytes of ASCII ! to ~ (no spaces or control characters)"
    )]
    BuilderText,
    /// The lock could not be written as TOML.
    #[error("minos.lock could not be serialized")]
    Serialize(#[source] toml::ser::Error),
}

/// Why the files on disk no longer match `minos.lock`, so a run must not start (D-11).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockMismatch {
    /// The lock has no image for this backend and architecture.
    #[error(
        "minos.lock has no image for backend {} and arch {}: run `minos env build`",
        backend_name(*backend),
        arch_name(*arch)
    )]
    NoImage {
        /// The backend asked for.
        backend: BackendKind,
        /// The architecture asked for.
        arch: Arch,
    },
    /// The Dockerfile changed since the lock was written.
    #[error(
        "the Dockerfile changed since minos.lock was written (locked sha256 {locked}, found {found}): run `minos env build --update`"
    )]
    Dockerfile {
        /// The SHA-256 the lock records.
        locked: BlobId,
        /// The SHA-256 of the file on disk.
        found: BlobId,
    },
    /// The `minos-guest` binary changed since the lock was written.
    #[error(
        "the minos-guest binary changed since minos.lock was written (locked sha256 {locked}, found {found}): run `minos env build --update`"
    )]
    Guest {
        /// The SHA-256 the lock records.
        locked: BlobId,
        /// The SHA-256 of the binary on disk.
        found: BlobId,
    },
}

// ---------------------------------------------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------------------------------------------

/// A builder name or version: 1 to [`MAX_BUILDER_TEXT_BYTES`] bytes, every one ASCII graphic (`!` to `~`), so it
/// has no space, no control character, and nothing a terminal would interpret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BuilderText(String);

impl BuilderText {
    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BuilderText {
    type Error = LockError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let valid = !text.is_empty()
            && text.len() <= MAX_BUILDER_TEXT_BYTES
            && text.bytes().all(|byte| byte.is_ascii_graphic());
        if valid {
            Ok(Self(text))
        } else {
            Err(LockError::BuilderText)
        }
    }
}

impl From<BuilderText> for String {
    fn from(text: BuilderText) -> Self {
        text.0
    }
}

/// The OCI builder that produced the rootfs tar, such as `docker-buildx` `v0.35.0-desktop.2`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawBuilder", into = "RawBuilder")]
pub struct BuilderId {
    name: BuilderText,
    version: BuilderText,
}

impl BuilderId {
    /// A builder from its name and version.
    #[must_use]
    pub const fn new(name: BuilderText, version: BuilderText) -> Self {
        Self { name, version }
    }

    /// The builder's name.
    #[must_use]
    pub const fn name(&self) -> &BuilderText {
        &self.name
    }

    /// The builder's version.
    #[must_use]
    pub const fn version(&self) -> &BuilderText {
        &self.version
    }
}

/// [`BuilderId`] as written: `[image.builder]` with `name` and `version`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBuilder {
    name: String,
    version: String,
}

impl TryFrom<RawBuilder> for BuilderId {
    type Error = LockError;

    fn try_from(raw: RawBuilder) -> Result<Self, Self::Error> {
        Ok(Self::new(
            BuilderText::try_from(raw.name)?,
            BuilderText::try_from(raw.version)?,
        ))
    }
}

impl From<BuilderId> for RawBuilder {
    fn from(builder: BuilderId) -> Self {
        Self {
            name: builder.name.into(),
            version: builder.version.into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// LockedImage
// ---------------------------------------------------------------------------------------------------------------

/// What one `env build` produced for one backend and architecture.
///
/// `dockerfile_sha256` and `guest_sha256` are plain SHA-256 content digests, the same digest [`BlobId::of`]
/// computes over the file's bytes. `root_image` is the packed image's blob ID in `.minos/blobs` (D-09).
/// `base_images` is sorted and holds each pin once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawLockedImage", into = "RawLockedImage")]
pub struct LockedImage {
    backend: BackendKind,
    arch: Arch,
    root_image: ImageDigest,
    dockerfile_sha256: BlobId,
    base_images: Vec<ImageRef>,
    guest_sha256: BlobId,
    builder: BuilderId,
}

impl LockedImage {
    /// One lock entry. `base_images` is sorted and repeated pins are dropped, so the entry has one wire form.
    #[must_use]
    pub fn new(
        backend: BackendKind,
        arch: Arch,
        root_image: ImageDigest,
        dockerfile_sha256: BlobId,
        mut base_images: Vec<ImageRef>,
        guest_sha256: BlobId,
        builder: BuilderId,
    ) -> Self {
        base_images.sort();
        base_images.dedup();
        Self {
            backend,
            arch,
            root_image,
            dockerfile_sha256,
            base_images,
            guest_sha256,
            builder,
        }
    }

    /// The backend the image is for.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
    }

    /// The architecture the image is for.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        self.arch
    }

    /// The packed root image's digest, equal to its blob ID (D-09).
    #[must_use]
    pub const fn root_image(&self) -> &ImageDigest {
        &self.root_image
    }

    /// The SHA-256 of the Dockerfile the image was built from.
    #[must_use]
    pub const fn dockerfile_sha256(&self) -> &BlobId {
        &self.dockerfile_sha256
    }

    /// The Dockerfile's `FROM` pins, sorted, each once.
    #[must_use]
    pub fn base_images(&self) -> &[ImageRef] {
        &self.base_images
    }

    /// The SHA-256 of the `minos-guest` binary written in as `/init`.
    #[must_use]
    pub const fn guest_sha256(&self) -> &BlobId {
        &self.guest_sha256
    }

    /// The builder that produced the rootfs tar.
    #[must_use]
    pub const fn builder(&self) -> &BuilderId {
        &self.builder
    }
}

/// [`LockedImage`] as written: one `[[image]]` table.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLockedImage {
    backend: BackendKind,
    arch: Arch,
    root_image: ImageDigest,
    dockerfile_sha256: BlobId,
    base_images: Vec<ImageRef>,
    guest_sha256: BlobId,
    builder: RawBuilder,
}

impl TryFrom<RawLockedImage> for LockedImage {
    type Error = LockError;

    fn try_from(raw: RawLockedImage) -> Result<Self, Self::Error> {
        let canonical = raw
            .base_images
            .windows(2)
            .all(|pair| matches!(pair, [earlier, later] if earlier < later));
        if !canonical {
            return Err(LockError::BaseImagesNotCanonical);
        }
        Ok(Self {
            backend: raw.backend,
            arch: raw.arch,
            root_image: raw.root_image,
            dockerfile_sha256: raw.dockerfile_sha256,
            base_images: raw.base_images,
            guest_sha256: raw.guest_sha256,
            builder: BuilderId::try_from(raw.builder)?,
        })
    }
}

impl From<LockedImage> for RawLockedImage {
    fn from(image: LockedImage) -> Self {
        Self {
            backend: image.backend,
            arch: image.arch,
            root_image: image.root_image,
            dockerfile_sha256: image.dockerfile_sha256,
            base_images: image.base_images,
            guest_sha256: image.guest_sha256,
            builder: image.builder.into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Lock
// ---------------------------------------------------------------------------------------------------------------

/// The parsed `minos.lock`: at most one [`LockedImage`] per (backend, arch) pair, written in that order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawLock", into = "RawLock")]
pub struct Lock {
    images: BTreeMap<(BackendKind, Arch), LockedImage>,
}

impl Lock {
    /// A lock with no images, before the first `env build`.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            images: BTreeMap::new(),
        }
    }

    /// Parses a `minos.lock`.
    ///
    /// # Errors
    ///
    /// [`LockError::TooLarge`] for text over [`MAX_LOCK_BYTES`]; [`LockError::Toml`] for a syntax error, a missing
    /// or unknown key, or a malformed value; and [`LockError::Version`], [`LockError::DuplicateImage`],
    /// [`LockError::BaseImagesNotCanonical`], or [`LockError::BuilderText`] for a rule in the
    /// [module docs](self).
    pub fn parse_toml(text: &str) -> Result<Self, LockError> {
        if text.len() > MAX_LOCK_BYTES {
            return Err(LockError::TooLarge);
        }
        let raw: RawLock = toml::from_str(text).map_err(LockError::Toml)?;
        Self::try_from(raw)
    }

    /// The lock as TOML text, the one wire form.
    ///
    /// # Errors
    ///
    /// [`LockError::Serialize`] if the TOML writer fails.
    pub fn to_toml(&self) -> Result<String, LockError> {
        toml::to_string(&RawLock::from(self.clone())).map_err(LockError::Serialize)
    }

    /// The image for `backend` and `arch`, if the lock has one.
    #[must_use]
    pub fn image(&self, backend: BackendKind, arch: Arch) -> Option<&LockedImage> {
        self.images.get(&(backend, arch))
    }

    /// The lock with `image` as the entry for its backend and architecture, replacing any earlier entry for that
    /// pair and leaving the others (`env build --update`, D-10).
    #[must_use]
    pub fn with_image(mut self, image: LockedImage) -> Self {
        self.images.insert((image.backend, image.arch), image);
        self
    }

    /// The locked image for `backend` and `arch`, after checking that the Dockerfile and the guest binary on disk
    /// still have the SHA-256 digests the lock records (D-11).
    ///
    /// # Errors
    ///
    /// [`LockMismatch::NoImage`] when the lock has no entry for the pair, and [`LockMismatch::Dockerfile`] or
    /// [`LockMismatch::Guest`], carrying both digests, when a file changed.
    pub fn check(
        &self,
        backend: BackendKind,
        arch: Arch,
        dockerfile: &BlobId,
        guest: &BlobId,
    ) -> Result<&LockedImage, LockMismatch> {
        let image = self
            .image(backend, arch)
            .ok_or(LockMismatch::NoImage { backend, arch })?;
        if image.dockerfile_sha256 != *dockerfile {
            return Err(LockMismatch::Dockerfile {
                locked: image.dockerfile_sha256,
                found: *dockerfile,
            });
        }
        if image.guest_sha256 != *guest {
            return Err(LockMismatch::Guest {
                locked: image.guest_sha256,
                found: *guest,
            });
        }
        Ok(image)
    }
}

/// [`Lock`] as written: `v = 1`, then one `[[image]]` table per entry. An empty lock writes no `image` key.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLock {
    v: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image: Vec<RawLockedImage>,
}

impl TryFrom<RawLock> for Lock {
    type Error = LockError;

    fn try_from(raw: RawLock) -> Result<Self, Self::Error> {
        if raw.v != LOCK_VERSION {
            return Err(LockError::Version);
        }
        let mut images = BTreeMap::new();
        for raw_image in raw.image {
            let image = LockedImage::try_from(raw_image)?;
            let key = (image.backend, image.arch);
            if images.insert(key, image).is_some() {
                return Err(LockError::DuplicateImage {
                    backend: key.0,
                    arch: key.1,
                });
            }
        }
        Ok(Self { images })
    }
}

impl From<Lock> for RawLock {
    fn from(lock: Lock) -> Self {
        Self {
            v: LOCK_VERSION,
            image: lock
                .images
                .into_values()
                .map(RawLockedImage::from)
                .collect(),
        }
    }
}
