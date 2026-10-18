//! The file blob tree on disk (STORE-04): the sharded path, no leftover temporary file, a flipped byte refused on
//! read, the size cap checked before reading, a put that heals a tampered blob, and symlink refusals.
//!
//! Tampering goes through `std::fs::OpenOptions` and a seek (the crate's clippy bans on `File::create` and
//! `fs::write` apply to tests too); symlinks come from `std::os::unix::fs::symlink`.

use std::error::Error;
use std::fs::{self, OpenOptions, Permissions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use minos_core::id::BlobId;
use minos_store::{Blobs, FileStore, StoreError};

type TestResult = Result<(), Box<dyn Error>>;

/// The SHA-256 of `abc`: `printf 'abc' | shasum -a 256`.
const ABC_HEX: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// `<root>/blobs/sha256/ba`, the shard directory of `abc`.
fn abc_shard(root: &Path) -> PathBuf {
    root.join("blobs").join("sha256").join("ba")
}

/// `<root>/blobs/sha256/ba/7816bf…`, the file of `abc`.
fn abc_path(root: &Path) -> PathBuf {
    abc_shard(root).join("7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
}

/// Overwrites the first byte of the file at `path` in place.
fn flip_first_byte(path: &Path) -> TestResult {
    let mut file = OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"X")?;
    file.sync_all()?;
    Ok(())
}

/// The permission bits of `path`.
fn mode(path: &Path) -> std::io::Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

#[test]
fn put_writes_the_sharded_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    let id = store.put(b"abc")?;

    assert_eq!(id.to_hex(), ABC_HEX);
    assert_eq!(fs::read(abc_path(dir.path()))?, b"abc");
    for path in [
        dir.path().join("blobs"),
        dir.path().join("blobs").join("sha256"),
        abc_shard(dir.path()),
    ] {
        assert_eq!(mode(&path)?, 0o700);
    }
    Ok(())
}

#[test]
fn no_temp_file_remains() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    store.put(b"abc")?;
    store.put(b"abc")?;

    let names: Vec<_> = fs::read_dir(abc_shard(dir.path()))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(
        names,
        ["7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"]
    );
    Ok(())
}

#[test]
fn tampered_blob_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    flip_first_byte(&abc_path(dir.path()))?;

    let result = store.get(&id, u64::MAX);

    assert!(
        matches!(result, Err(StoreError::BlobTampered { id: refused }) if refused == id),
        "a flipped byte must be BlobTampered"
    );
    Ok(())
}

#[test]
fn over_max_is_refused_before_reading() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    // An unreadable file proves the cap is checked from the metadata, before the file is opened.
    let path = abc_path(dir.path());
    fs::set_permissions(&path, Permissions::from_mode(0o000))?;

    let result = store.get(&id, 2);

    fs::set_permissions(&path, Permissions::from_mode(0o600))?;
    assert!(
        matches!(
            result,
            Err(StoreError::BlobTooLarge { id: refused, size: 3, max: 2 }) if refused == id
        ),
        "a 3-byte blob under a 2-byte cap must be BlobTooLarge without opening it"
    );
    Ok(())
}

#[test]
fn put_heals_a_tampered_blob() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    flip_first_byte(&abc_path(dir.path()))?;

    assert_eq!(store.put(b"abc")?, id);

    assert_eq!(store.get(&id, 3)?, b"abc");
    assert_eq!(fs::read(abc_path(dir.path()))?, b"abc");
    Ok(())
}

#[test]
fn symlinked_blob_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    let elsewhere = dir.path().join("elsewhere");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&elsewhere)?
        .write_all(b"abc")?;
    let path = abc_path(dir.path());
    fs::remove_file(&path)?;
    symlink(&elsewhere, &path)?;

    let result = store.get(&id, u64::MAX);

    assert!(
        matches!(result, Err(StoreError::Unsafe { what: "blob file" })),
        "a symlinked blob must be Unsafe even when its target has the right bytes"
    );
    assert!(!store.contains(&id)?);
    Ok(())
}

#[test]
fn symlinked_shard_directory_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere)?;
    fs::create_dir_all(dir.path().join("blobs").join("sha256"))?;
    symlink(&elsewhere, abc_shard(dir.path()))?;

    let put = store.put(b"abc");
    let get = store.get(&BlobId::of(b"abc"), u64::MAX);

    assert!(
        matches!(
            put,
            Err(StoreError::Unsafe {
                what: "blob shard directory"
            })
        ),
        "put must refuse a symlinked shard directory"
    );
    assert!(
        matches!(
            get,
            Err(StoreError::Unsafe {
                what: "blob shard directory"
            })
        ),
        "get must refuse a symlinked shard directory"
    );
    assert_eq!(fs::read_dir(&elsewhere)?.count(), 0);
    Ok(())
}

#[test]
fn missing_blob_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let absent = BlobId::of(b"never stored");

    let before_any_put = store.get(&absent, u64::MAX);
    store.put(b"abc")?;
    let after_a_put = store.get(&absent, u64::MAX);

    for result in [before_any_put, after_a_put] {
        assert!(
            matches!(result, Err(StoreError::BlobMissing { id }) if id == absent),
            "an absent blob must be BlobMissing"
        );
    }
    assert!(!store.contains(&absent)?);
    Ok(())
}

/// `<root>/blobs/staging`, where streamed puts keep their temporary files.
fn staging(root: &Path) -> PathBuf {
    root.join("blobs").join("staging")
}

/// The names in the directory at `path`.
fn names(path: &Path) -> Result<Vec<std::ffi::OsString>, Box<dyn Error>> {
    Ok(fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?)
}

#[test]
fn put_reader_writes_the_sharded_path_and_leaves_staging_empty() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    let id = store.put_reader(b"abc".as_slice(), 3)?;

    assert_eq!(id.to_hex(), ABC_HEX);
    assert_eq!(fs::read(abc_path(dir.path()))?, b"abc");
    assert_eq!(mode(&staging(dir.path()))?, 0o700);
    assert!(names(&staging(dir.path()))?.is_empty());
    assert_eq!(
        names(&abc_shard(dir.path()))?,
        ["7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"]
    );
    Ok(())
}

#[test]
fn put_reader_over_max_leaves_no_staging_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;

    let result = store.put_reader(b"abcd".as_slice(), 3);

    assert!(
        matches!(result, Err(StoreError::PutTooLarge { max: 3 })),
        "a 4-byte stream under a 3-byte cap must be PutTooLarge"
    );
    assert!(names(&staging(dir.path()))?.is_empty());
    assert!(!store.contains(&BlobId::of(b"abcd"))?);
    Ok(())
}

#[test]
fn put_reader_heals_a_tampered_blob() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    flip_first_byte(&abc_path(dir.path()))?;

    assert_eq!(store.put_reader(b"abc".as_slice(), 3)?, id);

    assert_eq!(store.get(&id, 3)?, b"abc");
    Ok(())
}

#[test]
fn put_reader_refuses_a_symlinked_staging_directory() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir(&elsewhere)?;
    fs::create_dir_all(dir.path().join("blobs"))?;
    symlink(&elsewhere, staging(dir.path()))?;

    let result = store.put_reader(b"abc".as_slice(), 3);

    assert!(
        matches!(
            result,
            Err(StoreError::Unsafe {
                what: "blob staging directory"
            })
        ),
        "put_reader must refuse a symlinked staging directory"
    );
    assert!(names(&elsewhere)?.is_empty());
    Ok(())
}

#[test]
fn verified_path_is_canonical() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;

    let path = store.verified_path(&id)?;

    assert!(path.is_absolute());
    assert_eq!(path, fs::canonicalize(&path)?);
    assert_eq!(path, fs::canonicalize(abc_path(dir.path()))?);
    assert_eq!(BlobId::of(&fs::read(&path)?), id);
    Ok(())
}

#[test]
fn verified_path_under_symlinked_ancestor() -> TestResult {
    let dir = tempfile::tempdir()?;
    let real = dir.path().join("real");
    fs::create_dir(&real)?;
    let link = dir.path().join("link");
    symlink(&real, &link)?;
    let store = FileStore::open(&link.join("store"))?;
    let id = store.put(b"abc")?;

    let path = store.verified_path(&id)?;

    let resolved = fs::canonicalize(&real)?;
    assert!(
        path.starts_with(&resolved),
        "the path must resolve through the symlinked ancestor"
    );
    assert!(!path.starts_with(&link));
    assert_eq!(path, resolved.join("store").join(abc_path(Path::new(""))));
    Ok(())
}

#[test]
fn verified_path_missing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let absent = BlobId::of(b"never stored");

    let before_any_put = store.verified_path(&absent);
    store.put(b"abc")?;
    let after_a_put = store.verified_path(&absent);

    for result in [before_any_put, after_a_put] {
        assert!(
            matches!(result, Err(StoreError::BlobMissing { id }) if id == absent),
            "an absent blob must be BlobMissing"
        );
    }
    Ok(())
}

#[test]
fn verified_path_symlinked_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    let elsewhere = dir.path().join("elsewhere");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&elsewhere)?
        .write_all(b"abc")?;
    let path = abc_path(dir.path());
    fs::remove_file(&path)?;
    symlink(&elsewhere, &path)?;

    let result = store.verified_path(&id);

    assert!(
        matches!(result, Err(StoreError::Unsafe { what: "blob file" })),
        "a symlinked blob file must be Unsafe even when its target has the right bytes"
    );
    Ok(())
}

#[test]
fn verified_path_symlinked_shard() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FileStore::open(dir.path())?;
    let id = store.put(b"abc")?;
    let shard = abc_shard(dir.path());
    let moved = dir.path().join("moved-shard");
    fs::rename(&shard, &moved)?;
    symlink(&moved, &shard)?;

    let result = store.verified_path(&id);

    assert!(
        matches!(
            result,
            Err(StoreError::Unsafe {
                what: "blob shard directory"
            })
        ),
        "a symlinked shard directory must be Unsafe even when it holds the right blob"
    );
    Ok(())
}
