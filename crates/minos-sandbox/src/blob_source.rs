//! How a backend obtains the bytes and paths a launch plan names (05-CONTEXT D-13, D-14).
//!
//! A [`LaunchPlan`](crate::LaunchPlan) carries IDs only: the guest job blob, the base tree and the diff blob for a
//! candidate, the candidate's output blob and the judge data for a judge, and the root image digest. A
//! [`BlobSource`] is how a backend turns those IDs into bytes under a cap, and the root image digest into a
//! verified path the VMM opens read-only. The CLI implements it over `FileStore`, tests over a map, and the Lambda
//! backend will implement it over S3.
//!
//! `deny.toml` layering is unchanged: `minos-store` stays out of backend crates and out of this one, because the
//! trait lives here and the CLI, the only place concrete types meet, supplies the implementation.
//! [`Backend::launch`](crate::Backend::launch) keeps its signature, because a backend is constructed with its
//! source.

use std::path::{Path, PathBuf};

use minos_core::id::{BlobId, ImageDigest};

/// Most bytes in a [`VerifiedPath`]: the launcher's own `MAX_PATH_BYTES` in `crates/minos-vmm/src/config.rs`,
/// which refuses a longer disk path in its config.
pub const MAX_VERIFIED_PATH_BYTES: usize = 1024;

/// A source of the blobs a launch plan names, and of verified root image paths.
///
/// Implemented by the CLI over `FileStore` and by tests over a map; the Lambda backend will implement it over S3.
pub trait BlobSource {
    /// The bytes of `id`, at most `max` of them.
    ///
    /// An implementation refuses a blob over `max` before copying it, and returns only bytes that hash to `id`.
    ///
    /// # Errors
    ///
    /// [`BlobSourceError::Missing`] when no blob `id` is stored, [`BlobSourceError::TooLarge`] when it is over
    /// `max`, [`BlobSourceError::Tampered`] when the stored bytes do not hash to `id`, and
    /// [`BlobSourceError::Unsafe`] or [`BlobSourceError::Io`] when the store could not be read safely.
    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, BlobSourceError>;

    /// A canonical, symlink-free path to the root image blob `digest`, never a copy.
    ///
    /// The store checked the path by `symlink_metadata` and inode before returning it, and `env build` verified
    /// the image's hash when it wrote the blob (05-CONTEXT D-14). The backend names the path under a read-only raw
    /// disk, so no per-launch copy or rehash of a multi-hundred-MB image is needed.
    ///
    /// # Errors
    ///
    /// [`BlobSourceError::RootImageMissing`] when no root image `digest` is stored, which means `minos env build`
    /// has not run for this environment, and [`BlobSourceError::Unsafe`] or [`BlobSourceError::Io`] when the
    /// store could not check the path.
    fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError>;
}

/// Why a [`BlobSource`] could not supply a blob or a root image path.
///
/// Every variant is an infrastructure or configuration failure, never an `Outcome`: the candidate has not run.
/// Payloads are IDs, numbers, fixed names, and the I/O error, never blob bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum BlobSourceError {
    /// No blob with this ID is stored.
    #[error("blob {id} is not stored")]
    Missing {
        /// The blob asked for.
        id: BlobId,
    },
    /// The blob is larger than the caller's cap, so it was not copied.
    #[error("blob {id} is {size} bytes, over the {max}-byte cap")]
    TooLarge {
        /// The blob asked for.
        id: BlobId,
        /// Its stored size, in bytes.
        size: u64,
        /// The caller's cap, in bytes.
        max: u64,
    },
    /// The stored bytes do not hash to the blob's ID; storing the blob again heals it.
    #[error("blob {id} does not match its hash")]
    Tampered {
        /// The blob asked for.
        id: BlobId,
    },
    /// No root image with this digest is stored.
    #[error("root image {digest} is not stored; run minos env build")]
    RootImageMissing {
        /// The root image asked for.
        digest: ImageDigest,
    },
    /// A path failed a safety check: a symlink, a changed inode, or a path a launch must not name.
    #[error("the {what} is unsafe")]
    Unsafe {
        /// What failed the check, as a fixed name.
        what: &'static str,
    },
    /// A store I/O operation failed.
    #[error("reading a blob failed at {op}")]
    Io {
        /// The operation that failed, as a fixed name.
        op: &'static str,
        /// The I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// A root image path a launch may name: absolute, with only normal components after the root, at most
/// [`MAX_VERIFIED_PATH_BYTES`] bytes, and no NUL.
///
/// [`VerifiedPath::new`] makes only these pure checks, because this crate opens no file. The filesystem checks are
/// the store's (05-CONTEXT D-14): it builds a `VerifiedPath` only after its `symlink_metadata` and inode check of
/// the blob, and `minos-vmm`'s preflight re-checks that the path is canonical at launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPath(PathBuf);

impl VerifiedPath {
    /// Checks `path` and wraps it.
    ///
    /// # Errors
    ///
    /// [`BlobSourceError::Unsafe`] with `what` `"root image path"` when the path is relative or empty, is the root
    /// alone, has a `.` or `..` component, an empty component (`//` or a trailing `/`), or a NUL byte, or is longer
    /// than [`MAX_VERIFIED_PATH_BYTES`].
    pub fn new(path: PathBuf) -> Result<Self, BlobSourceError> {
        let unsafe_path = || BlobSourceError::Unsafe {
            what: "root image path",
        };
        let bytes = path.as_os_str().as_encoded_bytes();
        if bytes.len() > MAX_VERIFIED_PATH_BYTES || bytes.contains(&0) {
            return Err(unsafe_path());
        }
        let rest = bytes.strip_prefix(b"/").ok_or_else(unsafe_path)?;
        let normal = rest
            .split(|byte| *byte == b'/')
            .all(|component| !matches!(component, b"" | b"." | b".."));
        if !normal {
            return Err(unsafe_path());
        }
        Ok(Self(path))
    }

    /// The path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
