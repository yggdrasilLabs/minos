//! Opening files without following a symlink (05-RESEARCH Pattern 6, Security Domain "Symlink in /out").
//!
//! PID 1 runs as root and reads files the candidate could have written: `/out` and the source tree. A path is
//! never trusted to name what it named a moment ago, so every such open passes `O_NOFOLLOW`, which makes the open
//! fail when the last component is a symlink, and `O_NONBLOCK`, so a FIFO cannot block PID 1 on the open. The
//! caller then compares the opened file's device and inode with the `symlink_metadata` it checked first
//! ([`same_file`]), so a path swapped between the check and the open is refused as well. The candidate's processes
//! are all killed before PID 1 packs (D-08), so nothing races the walk; these checks hold even if that failed.

use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use rustix::fs::OFlags;

/// `O_NOFOLLOW | O_NONBLOCK` in the form `OpenOptionsExt::custom_flags` takes. Both flags are small positive
/// numbers, so the reinterpretation is exact, and the assert keeps it so.
pub const NOFOLLOW_FLAGS: i32 = {
    let bits = OFlags::NOFOLLOW.union(OFlags::NONBLOCK).bits();
    assert!(bits <= i32::MAX.unsigned_abs());
    i32::from_ne_bytes(bits.to_ne_bytes())
};

/// Opens `path` read-only, refusing a symlink as its last component.
///
/// # Errors
///
/// The open's error: on Linux a symlink fails with `ELOOP`, on macOS with `ELOOP` too.
pub fn open_nofollow(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW_FLAGS)
        .open(path)
}

/// Opens `path` to rewrite it in place: write-only, truncated, never created, refusing a symlink as its last
/// component. The file keeps its mode.
///
/// # Errors
///
/// The open's error, including `NotFound` when the file does not exist.
pub fn open_nofollow_rewrite(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .custom_flags(NOFOLLOW_FLAGS)
        .open(path)
}

/// Whether `file` is the same file `meta` describes: equal device and inode.
///
/// # Errors
///
/// The error of reading `file`'s metadata.
pub fn same_file(meta: &Metadata, file: &File) -> io::Result<bool> {
    let opened = file.metadata()?;
    Ok(opened.dev() == meta.dev() && opened.ino() == meta.ino())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Read;

    use super::*;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn nofollow_refuses_symlink() -> TestResult {
        let dir = tempfile::tempdir()?;
        let target = dir.path().join("target");
        std::fs::write(&target, b"secret")?;
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link)?;

        assert!(open_nofollow(&link).is_err());
        assert!(open_nofollow_rewrite(&link).is_err());
        assert_eq!(std::fs::read(&target)?, b"secret");

        let mut text = String::new();
        open_nofollow(&target)?.read_to_string(&mut text)?;
        assert_eq!(text, "secret");
        Ok(())
    }

    #[test]
    fn nofollow_same_file_compares_inode() -> TestResult {
        let dir = tempfile::tempdir()?;
        let one = dir.path().join("one");
        let two = dir.path().join("two");
        std::fs::write(&one, b"1")?;
        std::fs::write(&two, b"2")?;
        let meta = std::fs::symlink_metadata(&one)?;
        assert!(same_file(&meta, &open_nofollow(&one)?)?);
        assert!(!same_file(&meta, &open_nofollow(&two)?)?);
        Ok(())
    }

    #[test]
    fn nofollow_rewrite_never_creates() -> TestResult {
        let dir = tempfile::tempdir()?;
        let missing = dir.path().join("missing");
        let error = open_nofollow_rewrite(&missing).err().ok_or("opened a missing file")?;
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!missing.exists());
        Ok(())
    }
}
