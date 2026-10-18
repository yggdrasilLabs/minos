//! [`BlobSource`] over the project's [`FileStore`] (05-CONTEXT D-13, D-14).
//!
//! `deny.toml` keeps `minos-store` out of every backend crate, so a backend is constructed with a
//! [`BlobSource`] and the CLI, the only crate where the store and the backend meet, supplies this one. Bytes come
//! through [`Blobs::get`], which checks the caller's cap before copying and rehashes what it read; a root image
//! comes back as the path [`FileStore::verified_path`] checked by `symlink_metadata` and inode, never a copy.

use std::io;
use std::path::Path;

use minos_core::id::{BlobId, ImageDigest};
use minos_sandbox::{BlobSource, BlobSourceError, VerifiedPath};
use minos_store::{Blobs, FileStore, STORE_DIR, StoreError};

/// The blobs and root images of one project's store at `<project>/.minos`.
#[derive(Debug, Clone)]
pub struct FileBlobSource {
    store: FileStore,
}

impl FileBlobSource {
    /// Opens the store at `<project>/`[`STORE_DIR`], creating it with mode 0700 when absent.
    ///
    /// # Errors
    ///
    /// As [`FileStore::open`]: [`StoreError::Unsafe`] when the store root is a symlink or not a directory, and
    /// [`StoreError::Io`] when it cannot be inspected or created.
    pub fn open(project: &Path) -> Result<Self, StoreError> {
        FileStore::open(&project.join(STORE_DIR)).map(|store| Self { store })
    }
}

impl BlobSource for FileBlobSource {
    fn get(&self, id: &BlobId, _max: u64) -> Result<Vec<u8>, BlobSourceError> {
        Err(BlobSourceError::Missing { id: *id })
    }

    fn root_image(&self, _digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
        Err(BlobSourceError::Unsafe { what: "stub" })
    }
}

/// The store failures a [`BlobSource`] caller cannot fix by asking again differently: a symlink or a wrong file
/// type is [`BlobSourceError::Unsafe`]; everything else, I/O included, is [`BlobSourceError::Io`] at `op`.
fn infrastructure(error: StoreError, op: &'static str) -> BlobSourceError {
    match error {
        StoreError::Unsafe { what } => BlobSourceError::Unsafe { what },
        StoreError::Io { op, source } => BlobSourceError::Io { op, source },
        other => BlobSourceError::Io {
            op,
            source: io::Error::other(other),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use minos_core::id::{BlobId, ImageDigest};
    use minos_sandbox::{BlobSource, BlobSourceError};
    use minos_store::{Blobs, FileStore, STORE_DIR};

    use super::FileBlobSource;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn file_blob_source_get() -> TestResult {
        let dir = tempfile::tempdir()?;
        let store = FileStore::open(&dir.path().join(STORE_DIR))?;
        let id = store.put(b"judge data")?;
        let source = FileBlobSource::open(dir.path())?;

        assert_eq!(source.get(&id, 10)?, b"judge data");
        assert!(matches!(
            source.get(&id, 9),
            Err(BlobSourceError::TooLarge { id: got, size: 10, max: 9 }) if got == id
        ));
        let absent = BlobId::of(b"never stored");
        assert!(matches!(
            source.get(&absent, 1024),
            Err(BlobSourceError::Missing { id: got }) if got == absent
        ));
        Ok(())
    }

    #[test]
    fn file_blob_source_root_image() -> TestResult {
        let dir = tempfile::tempdir()?;
        let store = FileStore::open(&dir.path().join(STORE_DIR))?;
        let id = store.put_reader(&b"a root image"[..], 1024)?;
        let source = FileBlobSource::open(dir.path())?;

        let path = source.root_image(&ImageDigest::from_blob_id(&id))?;
        assert_eq!(path.as_path(), store.verified_path(&id)?);

        let absent = ImageDigest::of(b"never built");
        assert!(matches!(
            source.root_image(&absent),
            Err(BlobSourceError::RootImageMissing { digest }) if digest == absent
        ));
        Ok(())
    }
}
