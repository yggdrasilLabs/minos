//! The source tree a stage runs in: the base tarball unpacked and the canonical diff applied (GUEST-02).
//!
//! The guest is the only place source is unpacked (04-CONTEXT D-17). [`unpack_tar`] reads the tarball in memory in
//! raw mode and writes each entry itself; it never calls the tar crate's `unpack`, which this crate's `clippy.toml`
//! bans. [`apply_diff`] parses the diff against the job's writable set, checks that it is the diff the job's
//! experiment names, and applies it with minos-core's one strict applier: exact positions, no fuzz, no offset
//! search. A hunk that does not apply is [`ApplyOutcome::Failed`], a candidate fault the runner records as
//! `Rejected`; it is never repaired.

use std::fs;
use std::io;
use std::path::Path;

use minos_core::diff::DiffError;
use minos_core::guest::{GuestJob, MAX_JUDGE_DATA_BYTES};
use minos_core::limits::{MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_ENTRY_BYTES};
use minos_core::path::RelPath;

/// Bytes in a MiB.
const MIB: u64 = 1_048_576;

/// The caps one unpack runs under: entries, bytes in one file, and bytes over every file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnpackCaps {
    entries: u64,
    file_bytes: u64,
    total_bytes: u64,
}

impl UnpackCaps {
    /// The base tarball: the source snapshot's own caps, 10,000 files, 16 MiB per file, 64 MiB in total.
    pub const BASE: Self = Self {
        entries: 10_000,
        file_bytes: 16 * MIB,
        total_bytes: 64 * MIB,
    };

    /// The judge's private data: 10,000 files, and core's [`MAX_JUDGE_DATA_BYTES`] per file and in total, the cap
    /// 05-04 binds to half the guest reserve.
    pub const JUDGE: Self = Self {
        entries: 10_000,
        file_bytes: MAX_JUDGE_DATA_BYTES,
        total_bytes: MAX_JUDGE_DATA_BYTES,
    };

    /// The candidate's output, as the judge receives it: core's output entry, per-file, and content caps.
    pub const CANDIDATE_OUT: Self = Self {
        entries: MAX_OUTPUT_ENTRIES as u64,
        file_bytes: MAX_OUTPUT_ENTRY_BYTES,
        total_bytes: MAX_OUTPUT_CONTENT_BYTES,
    };

    /// Most entries.
    #[must_use]
    pub const fn entries(&self) -> u64 {
        self.entries
    }

    /// Most bytes in one file.
    #[must_use]
    pub const fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    /// Most bytes over every file.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

/// What an unpack wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnpackReport {
    files: u64,
    bytes: u64,
}

impl UnpackReport {
    /// Files written.
    #[must_use]
    pub const fn files(&self) -> u64 {
        self.files
    }

    /// Content bytes written.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Why a tarball could not be unpacked. The tarball came from the host, which checked its digest, so this is a
/// host bug: an infrastructure failure, never a candidate fault. Messages are fixed text and numbers.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum UnpackError {
    /// The tar crate refused a header: a bad checksum or a corrupt numeric field.
    #[error("a tar header is malformed")]
    Malformed,
    /// A header is not POSIX ustar.
    #[error("a tar header is not ustar")]
    NotUstar,
    /// An entry is not a regular file: a link, directory, device, FIFO, or extension header.
    #[error("a tar entry is not a regular file")]
    EntryType,
    /// An entry's name is not UTF-8 or not a `RelPath`.
    #[error("a tar entry name is not a valid relative path")]
    Path,
    /// An entry's mode is not 0644 or 0755.
    #[error("a tar entry has mode {mode:o}, not 644 or 755")]
    Mode {
        /// The mode.
        mode: u32,
    },
    /// More entries than the cap.
    #[error("the tarball has more entries than its cap")]
    TooManyEntries,
    /// One file is over the per-file cap, checked from its header before reading.
    #[error("a tar entry of {bytes} bytes is over the per-file cap")]
    FileTooLarge {
        /// The header's size.
        bytes: u64,
    },
    /// The files together are over the total cap.
    #[error("the tarball's files are over the total cap")]
    TotalTooLarge,
    /// An entry's data is shorter than its header says.
    #[error("a tar entry's data is truncated")]
    Truncated,
    /// An entry's path, or one of its parent directories, already exists as something it cannot be.
    #[error("a tar entry collides with an earlier entry")]
    Collision,
    /// Writing the tree failed.
    #[error("could not write the unpacked tree")]
    Io(#[source] io::Error),
}

/// Unpacks `tar` into the existing directory `dest` under `caps`: regular files only, each named by a `RelPath`,
/// created new with mode 0644 or 0755, parent directories created 0755. The first refused entry stops the unpack,
/// so nothing past it is written.
///
/// # Errors
///
/// An [`UnpackError`] for the first refused entry or failed write.
pub fn unpack_tar(tar: &[u8], dest: &Path, caps: &UnpackCaps) -> Result<UnpackReport, UnpackError> {
    let _ = (tar, dest, caps);
    Err(UnpackError::Malformed)
}

/// How applying the diff went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The diff applied and every touched file was written back.
    Applied,
    /// A hunk did not apply, so no file was changed and no stage runs: a candidate fault, recorded as `Rejected`.
    Failed {
        /// The file whose hunk did not apply.
        path: RelPath,
        /// 1-based number of the hunk; 0 means the file is missing from the base tree.
        hunk: u32,
    },
}

/// Why the diff could not even be tried. The host parsed the diff and wrote the job, so each of these is a host
/// bug or a broken tree: an infrastructure failure, never a candidate fault.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ApplySetupError {
    /// The job has no base tree: it is not a candidate job.
    #[error("only a candidate job applies a diff")]
    Role,
    /// The diff text does not parse against the job's writable set.
    #[error("the diff does not parse")]
    Parse(#[source] DiffError),
    /// The parsed diff is not the one the job's experiment names: `ExperimentId::of(base, diff)` differs.
    #[error("the diff is not the job's experiment")]
    Identity,
    /// A touched path is not a one-link regular file.
    #[error("a file the diff touches is not a regular file")]
    NotRegular,
    /// A touched file is over the base per-file cap.
    #[error("a file the diff touches is over the per-file cap")]
    TooLarge,
    /// The applier reported a reason other than `ApplyFailed`.
    #[error("the applier reported an unexpected reason")]
    Reason,
    /// Reading or writing a touched file failed.
    #[error("could not read or write a file the diff touches")]
    Io(#[source] io::Error),
}

/// Applies the canonical diff `diff_text` to the tree under `src`, as the candidate job `job` describes.
///
/// # Errors
///
/// An [`ApplySetupError`] when the diff cannot be tried at all; a hunk that does not apply is
/// `Ok(ApplyOutcome::Failed)`.
pub fn apply_diff(
    src: &Path,
    diff_text: &str,
    job: &GuestJob,
) -> Result<ApplyOutcome, ApplySetupError> {
    let _ = (src, diff_text, job);
    Err(ApplySetupError::Reason)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::os::unix::fs::PermissionsExt;

    use tar::{EntryType, Header};

    use super::*;
    use crate::test_fixture::{
        DIFF, PARSE_RS, PARSE_RS_AFTER, baseline_job_for, candidate_job_for, judge_job, tar_of,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// One tar entry with a raw name, any type, and any mode, appended after `before` and before `after.txt`.
    fn with_raw_entry(
        name: &[u8],
        kind: EntryType,
        mode: u32,
        data: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut before = Header::new_ustar();
        before.set_path("before.txt")?;
        before.set_size(1);
        before.set_mode(0o644);
        before.set_entry_type(EntryType::Regular);
        before.set_cksum();
        builder.append(&before, &b"b"[..])?;

        let mut header = Header::new_ustar();
        for (slot, byte) in header.as_old_mut().name.iter_mut().zip(name) {
            *slot = *byte;
        }
        header.set_size(u64::try_from(data.len())?);
        header.set_mode(mode);
        header.set_entry_type(kind);
        if matches!(kind, EntryType::Symlink | EntryType::Link) {
            header.set_link_name("before.txt")?;
        }
        header.set_cksum();
        builder.append(&header, data)?;

        let mut after = Header::new_ustar();
        after.set_path("after.txt")?;
        after.set_size(1);
        after.set_mode(0o644);
        after.set_entry_type(EntryType::Regular);
        after.set_cksum();
        builder.append(&after, &b"a"[..])?;
        Ok(builder.into_inner()?)
    }

    fn mode_of(path: &Path) -> Result<u32, Box<dyn Error>> {
        Ok(fs::symlink_metadata(path)?.permissions().mode() & 0o7777)
    }

    #[test]
    fn unpack_regular_files() -> TestResult {
        let tar = tar_of(&[
            ("bin/run", 0o755, b"#!/bin/sh\n"),
            ("src/a.rs", 0o644, b"fn a() {}\n"),
        ])?;
        let dir = tempfile::tempdir()?;
        let report = unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        assert_eq!(report.files(), 2);
        assert_eq!(report.bytes(), 20);
        assert_eq!(fs::read(dir.path().join("src/a.rs"))?, b"fn a() {}\n");
        assert_eq!(fs::read(dir.path().join("bin/run"))?, b"#!/bin/sh\n");
        assert_eq!(mode_of(&dir.path().join("src/a.rs"))?, 0o644);
        assert_eq!(mode_of(&dir.path().join("bin/run"))?, 0o755);
        assert_eq!(mode_of(&dir.path().join("src"))?, 0o755);
        assert_eq!(mode_of(&dir.path().join("bin"))?, 0o755);
        Ok(())
    }

    #[test]
    fn unpack_refusals() -> TestResult {
        let cases: [(&[u8], EntryType, u32, &[u8]); 8] = [
            (b"link", EntryType::Symlink, 0o644, b""),
            (b"hard", EntryType::Link, 0o644, b""),
            (b"a/../b", EntryType::Regular, 0o644, b"x"),
            (b"/etc/passwd", EntryType::Regular, 0o644, b"x"),
            (b"bad\\name", EntryType::Regular, 0o644, b"x"),
            (b"setuid", EntryType::Regular, 0o4755, b"x"),
            (b"private", EntryType::Regular, 0o600, b"x"),
            (b"dir/", EntryType::Directory, 0o755, b""),
        ];
        for (name, kind, mode, data) in cases {
            let tar = with_raw_entry(name, kind, mode, data)?;
            let dir = tempfile::tempdir()?;
            assert!(unpack_tar(&tar, dir.path(), &UnpackCaps::BASE).is_err());
            assert!(dir.path().join("before.txt").exists());
            assert!(!dir.path().join("after.txt").exists());
        }
        Ok(())
    }

    #[test]
    fn unpack_refusal_kinds() -> TestResult {
        let kind = |name: &[u8], kind, mode| -> Result<UnpackError, Box<dyn Error>> {
            let tar = with_raw_entry(name, kind, mode, b"")?;
            let dest = tempfile::tempdir()?;
            unpack_tar(&tar, dest.path(), &UnpackCaps::BASE)
                .err()
                .ok_or_else(|| "accepted".into())
        };
        assert!(matches!(kind(b"l", EntryType::Symlink, 0o644)?, UnpackError::EntryType));
        assert!(matches!(kind(b"l", EntryType::Link, 0o644)?, UnpackError::EntryType));
        assert!(matches!(kind(b"a/../b", EntryType::Regular, 0o644)?, UnpackError::Path));
        assert!(matches!(kind(b"/abs", EntryType::Regular, 0o644)?, UnpackError::Path));
        assert!(matches!(
            kind(b"s", EntryType::Regular, 0o4755)?,
            UnpackError::Mode { mode: 0o4755 }
        ));
        Ok(())
    }

    #[test]
    fn unpack_caps() -> TestResult {
        let caps = UnpackCaps {
            entries: 2,
            file_bytes: 4,
            total_bytes: 6,
        };

        let dir = tempfile::tempdir()?;
        let big = tar_of(&[("a", 0o644, b"12345")])?;
        assert!(matches!(
            unpack_tar(&big, dir.path(), &caps),
            Err(UnpackError::FileTooLarge { bytes: 5 })
        ));
        assert!(!dir.path().join("a").exists());

        let dir = tempfile::tempdir()?;
        let many = tar_of(&[("a", 0o644, b"1"), ("b", 0o644, b"2"), ("c", 0o644, b"3")])?;
        assert!(matches!(
            unpack_tar(&many, dir.path(), &caps),
            Err(UnpackError::TooManyEntries)
        ));
        assert!(!dir.path().join("c").exists());

        let dir = tempfile::tempdir()?;
        let total = tar_of(&[("a", 0o644, b"1234"), ("b", 0o644, b"1234")])?;
        assert!(matches!(
            unpack_tar(&total, dir.path(), &caps),
            Err(UnpackError::TotalTooLarge)
        ));
        assert!(dir.path().join("a").exists());
        assert!(!dir.path().join("b").exists());
        Ok(())
    }

    #[test]
    fn unpack_refuses_duplicates_and_collisions() -> TestResult {
        let dir = tempfile::tempdir()?;
        let duplicate = tar_of(&[("a", 0o644, b"1"), ("a", 0o644, b"2")])?;
        assert!(unpack_tar(&duplicate, dir.path(), &UnpackCaps::BASE).is_err());
        assert_eq!(fs::read(dir.path().join("a"))?, b"1");

        let dir = tempfile::tempdir()?;
        let collision = tar_of(&[("a", 0o644, b"1"), ("a/b", 0o644, b"2")])?;
        assert!(matches!(
            unpack_tar(&collision, dir.path(), &UnpackCaps::BASE),
            Err(UnpackError::Collision)
        ));
        Ok(())
    }

    #[test]
    fn unpack_caps_are_core_caps() {
        assert_eq!(UnpackCaps::JUDGE.file_bytes(), MAX_JUDGE_DATA_BYTES);
        assert_eq!(UnpackCaps::JUDGE.total_bytes(), MAX_JUDGE_DATA_BYTES);
        assert_eq!(UnpackCaps::CANDIDATE_OUT.file_bytes(), MAX_OUTPUT_ENTRY_BYTES);
        assert_eq!(UnpackCaps::CANDIDATE_OUT.total_bytes(), MAX_OUTPUT_CONTENT_BYTES);
        assert_eq!(UnpackCaps::BASE.entries(), 10_000);
        assert_eq!(UnpackCaps::BASE.file_bytes(), 16 * MIB);
        assert_eq!(UnpackCaps::BASE.total_bytes(), 64 * MIB);
    }

    #[test]
    fn apply_changes_file() -> TestResult {
        let tar = tar_of(&[("src/parse.rs", 0o644, PARSE_RS)])?;
        let job = candidate_job_for(&tar, DIFF)?;
        let dir = tempfile::tempdir()?;
        unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        assert_eq!(apply_diff(dir.path(), DIFF, &job)?, ApplyOutcome::Applied);
        let parse = dir.path().join("src/parse.rs");
        assert_eq!(fs::read(&parse)?, PARSE_RS_AFTER);
        assert_eq!(mode_of(&parse)?, 0o644);
        Ok(())
    }

    #[test]
    fn apply_mismatch() -> TestResult {
        let other: &[u8] = b"fn parse() {\n    other();\n}\n";
        let tar = tar_of(&[("src/parse.rs", 0o644, other)])?;
        let job = candidate_job_for(&tar, DIFF)?;
        let dir = tempfile::tempdir()?;
        unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        assert_eq!(
            apply_diff(dir.path(), DIFF, &job)?,
            ApplyOutcome::Failed {
                path: RelPath::try_from("src/parse.rs")?,
                hunk: 1,
            }
        );
        assert_eq!(fs::read(dir.path().join("src/parse.rs"))?, other);

        let tar = tar_of(&[("src/lib.rs", 0o644, PARSE_RS)])?;
        let job = candidate_job_for(&tar, DIFF)?;
        let dir = tempfile::tempdir()?;
        unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        assert_eq!(
            apply_diff(dir.path(), DIFF, &job)?,
            ApplyOutcome::Failed {
                path: RelPath::try_from("src/parse.rs")?,
                hunk: 0,
            }
        );
        assert!(!dir.path().join("src/parse.rs").exists());
        Ok(())
    }

    #[test]
    fn apply_identity_mismatch() -> TestResult {
        let tar = tar_of(&[("src/parse.rs", 0o644, PARSE_RS)])?;
        let other = DIFF.replace("new();", "newer();");
        let job = candidate_job_for(&tar, &other)?;
        let dir = tempfile::tempdir()?;
        unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        assert!(matches!(
            apply_diff(dir.path(), DIFF, &job),
            Err(ApplySetupError::Identity)
        ));
        assert!(matches!(
            apply_diff(dir.path(), "not a diff\n", &job),
            Err(ApplySetupError::Parse(_))
        ));
        assert!(matches!(
            apply_diff(dir.path(), DIFF, &baseline_job_for(&tar)?),
            Err(ApplySetupError::Identity)
        ));
        assert!(matches!(
            apply_diff(dir.path(), DIFF, &judge_job()?),
            Err(ApplySetupError::Role)
        ));
        assert_eq!(fs::read(dir.path().join("src/parse.rs"))?, PARSE_RS);
        Ok(())
    }

    #[test]
    fn apply_refuses_a_symlinked_target() -> TestResult {
        let tar = tar_of(&[("src/real.rs", 0o644, PARSE_RS)])?;
        let job = candidate_job_for(&tar, DIFF)?;
        let dir = tempfile::tempdir()?;
        unpack_tar(&tar, dir.path(), &UnpackCaps::BASE)?;
        std::os::unix::fs::symlink(dir.path().join("src/real.rs"), dir.path().join("src/parse.rs"))?;
        assert!(matches!(
            apply_diff(dir.path(), DIFF, &job),
            Err(ApplySetupError::NotRegular)
        ));
        assert_eq!(fs::read(dir.path().join("src/real.rs"))?, PARSE_RS);
        Ok(())
    }
}
