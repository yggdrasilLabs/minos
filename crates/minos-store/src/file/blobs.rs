//! The file blob tree (STORE-04): one file per blob at `blobs/sha256/<2 hex>/<62 hex>`, shared by every run.
//!
//! A blob is addressed by the plain SHA-256 of its bytes ([`BlobId::of`]), the same digest S3 verifies, so the
//! path doubles as the checksum. A put writes a temporary file in the blob's shard directory, syncs it, renames it
//! over the final path, and fsyncs the shard directory: a reader sees the whole blob or none of it. A put always
//! writes, so storing the bytes again heals a tampered or replaced blob.
//!
//! A get refuses a symlink or any other file type, refuses a stored size over the caller's cap from the metadata
//! before opening the file, reads at most one byte more than the cap, and rehashes what it read, so a flipped byte
//! on disk is [`StoreError::BlobTampered`] and never reaches a caller.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};

use minos_core::id::BlobId;
use tempfile::NamedTempFile;

use super::{FileStore, dir_exists, ensure_dir, fsync_dir, io_error, len_u64, open_existing};
use crate::{Blobs, StoreError};

impl FileStore {
    /// Whether the directories that would hold blob `id` all exist; [`StoreError::Unsafe`] when one is a symlink
    /// or another file type.
    fn blob_dirs_exist(&self, id: &BlobId) -> Result<bool, StoreError> {
        Ok(dir_exists(&self.blobs_dir(), "blobs directory")?
            && dir_exists(&self.sha256_dir(), "blobs directory")?
            && dir_exists(&self.blob_shard(id), "blob shard directory")?)
    }
}

impl Blobs for FileStore {
    fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError> {
        let id = BlobId::of(bytes);
        let blobs_dir = self.blobs_dir();
        let sha256_dir = self.sha256_dir();
        let shard = self.blob_shard(&id);
        if ensure_dir(&blobs_dir, "blobs directory")? {
            fsync_dir(&self.root)?;
        }
        if ensure_dir(&sha256_dir, "blobs directory")? {
            fsync_dir(&blobs_dir)?;
        }
        if ensure_dir(&shard, "blob shard directory")? {
            fsync_dir(&sha256_dir)?;
        }

        let mut temp =
            NamedTempFile::new_in(&shard).map_err(io_error("creating a blob temporary file"))?;
        temp.write_all(bytes).map_err(io_error("writing a blob"))?;
        temp.as_file()
            .sync_all()
            .map_err(io_error("syncing a blob"))?;
        temp.persist(self.blob_path(&id))
            .map_err(|failed| StoreError::Io {
                op: "renaming a blob into place",
                source: failed.error,
            })?;
        fsync_dir(&shard)?;
        Ok(id)
    }

    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, StoreError> {
        if !self.blob_dirs_exist(id)? {
            return Err(StoreError::BlobMissing { id: *id });
        }
        let path = self.blob_path(id);
        let seen = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::BlobMissing { id: *id });
            }
            Err(error) => return Err(io_error("inspecting a blob")(error)),
        };
        if !seen.file_type().is_file() {
            return Err(StoreError::Unsafe { what: "blob file" });
        }
        if seen.len() > max {
            return Err(StoreError::BlobTooLarge {
                id: *id,
                size: seen.len(),
                max,
            });
        }

        let mut options = OpenOptions::new();
        options.read(true);
        let file = open_existing(&path, "blob file", &options)?
            .ok_or(StoreError::BlobMissing { id: *id })?;
        let mut bytes = Vec::new();
        file.take(max.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(io_error("reading a blob"))?;
        let size = len_u64(bytes.len());
        if size > max {
            return Err(StoreError::BlobTooLarge { id: *id, size, max });
        }
        if BlobId::of(&bytes) == *id {
            Ok(bytes)
        } else {
            Err(StoreError::BlobTampered { id: *id })
        }
    }

    fn contains(&self, id: &BlobId) -> Result<bool, StoreError> {
        if !self.blob_dirs_exist(id)? {
            return Ok(false);
        }
        match fs::symlink_metadata(self.blob_path(id)) {
            Ok(meta) => Ok(meta.file_type().is_file()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error("inspecting a blob")(error)),
        }
    }
}
