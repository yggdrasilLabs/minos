//! The file blob tree (STORE-04): one file per blob at `blobs/sha256/<2 hex>/<62 hex>`, shared by every run.
//!
//! A blob is addressed by the plain SHA-256 of its bytes ([`BlobId::of`]), the same digest S3 verifies, so the
//! path doubles as the checksum. A put writes a temporary file in the blob's shard directory, syncs it, renames it
//! over the final path, and fsyncs the shard directory: a reader sees the whole blob or none of it. A put always
//! writes, so storing the bytes again heals a tampered or replaced blob.
//!
//! A streamed put ([`Blobs::put_reader`]; 05-CONTEXT D-09) cannot know the blob's shard until its last byte, so
//! it writes its temporary file in `blobs/staging`, copying the reader through a 64 KiB buffer and feeding each
//! buffer to a [`BlobHasher`] as it goes. More than the caller's cap is refused with [`StoreError::PutTooLarge`]
//! and a reader error with [`StoreError::Io`]; either way the temporary file deletes itself on drop and nothing
//! is stored. Otherwise the file is synced, renamed into its shard (a rename within one filesystem), and the
//! shard and staging directories are fsynced, so a reader sees the whole blob or none of it.
//!
//! A get refuses a symlink or any other file type, refuses a stored size over the caller's cap from the metadata
//! before opening the file, reads at most one byte more than the cap, and rehashes what it read, so a flipped byte
//! on disk is [`StoreError::BlobTampered`] and never reaches a caller.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};

use minos_core::id::{BlobHasher, BlobId};
use tempfile::NamedTempFile;

use super::{FileStore, dir_exists, ensure_dir, fsync_dir, io_error, len_u64, open_existing};
use crate::{Blobs, StoreError};

/// The copy buffer of a streamed put.
const STREAM_BUFFER_BYTES: usize = 64 * 1024;

impl FileStore {
    /// Makes sure `blobs/` exists, fsyncing the root when it was created here.
    fn ensure_blobs_dir(&self) -> Result<(), StoreError> {
        if ensure_dir(&self.blobs_dir(), "blobs directory")? {
            fsync_dir(&self.root)?;
        }
        Ok(())
    }

    /// Makes sure `blobs/sha256/` and blob `id`'s shard exist, fsyncing each parent that gained an entry. The
    /// caller has made sure `blobs/` exists.
    fn ensure_shard(&self, id: &BlobId) -> Result<(), StoreError> {
        let sha256_dir = self.sha256_dir();
        if ensure_dir(&sha256_dir, "blobs directory")? {
            fsync_dir(&self.blobs_dir())?;
        }
        if ensure_dir(&self.blob_shard(id), "blob shard directory")? {
            fsync_dir(&sha256_dir)?;
        }
        Ok(())
    }

    /// Syncs `temp` and renames it over blob `id`'s path, then fsyncs the shard. The caller has made sure the
    /// shard exists.
    fn persist_blob(&self, temp: NamedTempFile, id: &BlobId) -> Result<(), StoreError> {
        temp.as_file()
            .sync_all()
            .map_err(io_error("syncing a blob"))?;
        temp.persist(self.blob_path(id))
            .map_err(|failed| StoreError::Io {
                op: "renaming a blob into place",
                source: failed.error,
            })?;
        fsync_dir(&self.blob_shard(id))
    }

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
        self.ensure_blobs_dir()?;
        self.ensure_shard(&id)?;

        let mut temp = NamedTempFile::new_in(self.blob_shard(&id))
            .map_err(io_error("creating a blob temporary file"))?;
        temp.write_all(bytes).map_err(io_error("writing a blob"))?;
        self.persist_blob(temp, &id)?;
        Ok(id)
    }

    fn put_reader<R: Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError> {
        self.ensure_blobs_dir()?;
        let staging = self.staging_dir();
        if ensure_dir(&staging, "blob staging directory")? {
            fsync_dir(&self.blobs_dir())?;
        }

        let mut temp =
            NamedTempFile::new_in(&staging).map_err(io_error("creating a blob temporary file"))?;
        let mut limited = reader.take(max.saturating_add(1));
        let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
        let mut hasher = BlobHasher::default();
        let mut received: u64 = 0;
        loop {
            let read = match limited.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(io_error("reading a streamed blob")(error)),
            };
            // A reader that reports more bytes than the buffer holds breaks the `Read` contract.
            let chunk = buffer.get(..read).ok_or_else(|| {
                io_error("reading a streamed blob")(io::Error::from(io::ErrorKind::InvalidData))
            })?;
            received = received.saturating_add(len_u64(read));
            if received > max {
                return Err(StoreError::PutTooLarge { max });
            }
            hasher.update(chunk);
            temp.write_all(chunk)
                .map_err(io_error("writing a streamed blob"))?;
        }

        let id = hasher.finish();
        self.ensure_shard(&id)?;
        self.persist_blob(temp, &id)?;
        fsync_dir(&staging)?;
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
