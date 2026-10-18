//! The `BlobSource` contract over an in-file map, the `VerifiedPath` checks, and `LaunchError::Blob` (D-13, D-14).
//!
//! `MapBlobSource` is the shape a test backend is constructed with: blob bytes by ID and verified root image
//! paths by digest. The CLI's implementation over `FileStore` lands with the CLI; this file pins the contract a
//! backend relies on.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use minos_core::id::{BlobId, ImageDigest};
use minos_sandbox::{BlobSource, BlobSourceError, LaunchError, VerifiedPath};

type TestResult = Result<(), Box<dyn Error>>;

/// Blob bytes by ID and root image paths by digest, in memory.
struct MapBlobSource {
    blobs: BTreeMap<BlobId, Vec<u8>>,
    images: BTreeMap<ImageDigest, VerifiedPath>,
}

impl MapBlobSource {
    /// A source holding `blobs`, each stored under its own ID, and no root image.
    fn of(blobs: &[&[u8]]) -> Self {
        Self {
            blobs: blobs
                .iter()
                .map(|bytes| (BlobId::of(bytes), bytes.to_vec()))
                .collect(),
            images: BTreeMap::new(),
        }
    }
}

impl BlobSource for MapBlobSource {
    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, BlobSourceError> {
        let bytes = self
            .blobs
            .get(id)
            .ok_or(BlobSourceError::Missing { id: *id })?;
        let size = u64::try_from(bytes.len()).map_err(|_| BlobSourceError::TooLarge {
            id: *id,
            size: u64::MAX,
            max,
        })?;
        if size > max {
            return Err(BlobSourceError::TooLarge { id: *id, size, max });
        }
        if BlobId::of(bytes) != *id {
            return Err(BlobSourceError::Tampered { id: *id });
        }
        Ok(bytes.clone())
    }

    fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
        self.images
            .get(digest)
            .cloned()
            .ok_or(BlobSourceError::RootImageMissing { digest: *digest })
    }
}

#[test]
fn map_blob_source_get() -> TestResult {
    let source = MapBlobSource::of(&[b"guest job", b"base tarball"]);
    let job = BlobId::of(b"guest job");
    assert_eq!(source.get(&job, 9)?, b"guest job".to_vec());

    match source.get(&job, 8) {
        Err(BlobSourceError::TooLarge { id, size, max }) => {
            assert_eq!((id, size, max), (job, 9, 8));
        }
        other => return Err(format!("expected TooLarge, got {other:?}").into()),
    }

    let absent = BlobId::of(b"absent");
    match source.get(&absent, 1024) {
        Err(BlobSourceError::Missing { id }) => assert_eq!(id, absent),
        other => return Err(format!("expected Missing, got {other:?}").into()),
    }

    // Bytes stored under another blob's ID never reach a caller.
    let mut tampered = MapBlobSource::of(&[]);
    tampered.blobs.insert(job, b"guest jog".to_vec());
    match tampered.get(&job, 1024) {
        Err(BlobSourceError::Tampered { id }) => assert_eq!(id, job),
        other => return Err(format!("expected Tampered, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn map_blob_source_root_image() -> TestResult {
    let digest = ImageDigest::of(b"root image");
    let path = VerifiedPath::new(PathBuf::from("/store/blobs/sha256/ab/cdef"))?;
    let mut source = MapBlobSource::of(&[]);
    source.images.insert(digest, path.clone());
    assert_eq!(source.root_image(&digest)?, path);

    let other = ImageDigest::of(b"another image");
    match source.root_image(&other) {
        Err(error @ BlobSourceError::RootImageMissing { .. }) => {
            assert!(error.to_string().contains("minos env build"), "{error}");
        }
        other => return Err(format!("expected RootImageMissing, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn verified_path_refuses() -> TestResult {
    let refused = [
        String::new(),
        "relative/root.img".to_owned(),
        "/".to_owned(),
        "/store/./root.img".to_owned(),
        "/store/../root.img".to_owned(),
        "/store/..".to_owned(),
        "/store//root.img".to_owned(),
        "/store/root.img/".to_owned(),
        "/store/root\0.img".to_owned(),
        format!("/{}", "a".repeat(1024)),
    ];
    for path in refused {
        match VerifiedPath::new(PathBuf::from(&path)) {
            Err(BlobSourceError::Unsafe { what }) => assert_eq!(what, "root image path"),
            other => return Err(format!("{path:?} gave {other:?}").into()),
        }
    }

    let accepted = "/Users/someone/project/.minos/blobs/sha256/ab/cdef";
    let path = VerifiedPath::new(PathBuf::from(accepted))?;
    assert_eq!(path.as_path(), Path::new(accepted));

    // Exactly the launcher's 1024-byte limit is accepted.
    let longest = format!("/{}", "a".repeat(1023));
    assert_eq!(longest.len(), 1024);
    let path = VerifiedPath::new(PathBuf::from(&longest))?;
    assert_eq!(path.as_path(), Path::new(&longest));
    Ok(())
}

#[test]
fn launch_error_from_blob_source() {
    let id = BlobId::of(b"input");
    let launch = LaunchError::from(BlobSourceError::Missing { id });
    assert!(matches!(
        &launch,
        LaunchError::Blob(BlobSourceError::Missing { id: missing }) if *missing == id
    ));
    assert_eq!(
        launch.to_string(),
        "a blob the plan names could not be read"
    );
    let source = Error::source(&launch).map(ToString::to_string);
    assert_eq!(source, Some(BlobSourceError::Missing { id }.to_string()));
}
