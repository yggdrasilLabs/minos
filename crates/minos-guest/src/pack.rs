//! The output tar PID 1 sends back (GUEST-04; 05-RESEARCH Pattern 6).
//!
//! **Layout.** Every output holds `.minos/status.json` and the log of each stage that ran (`.minos/build.log`,
//! `.minos/run.log`, or `.minos/judge.log`), each cut to [`LOG_CAP_BYTES`]. A candidate's output adds its `/out`
//! tree under `out/`. A judge's adds `.minos/scorecard.json`, read from `/out/scorecard.json`, and nothing else
//! from `/out`. [`build_output`] refuses a scorecard for a candidate status and any `out/` entry outside `out/`,
//! so a candidate's output can never carry `.minos/scorecard.json`: only the judge-role channel delivers a
//! scorecard, and the host's `into_scorecard` exists only on judge output.
//!
//! **The walk.** PID 1 packs after every process the candidate started has been killed and reaped (D-08), so no
//! candidate process can race [`walk_out`]. It still never follows a link, because the candidate wrote `/out`:
//! every entry is checked with `symlink_metadata`, a file must be a one-link regular file, it is opened with
//! `O_NOFOLLOW`, and the opened file's device and inode must match the checked ones. Anything else makes the whole
//! output [`OutputStatus::Invalid`]; over a cap it is [`OutputStatus::TooLarge`]. Either way no `out/` file is
//! packed, only PID 1's own files.
//!
//! **Headers** are built by hand as minos-source builds them: ustar, mtime, uid, and gid 0, empty owner names,
//! mode 0644, typeflag regular, entries in path order, and two zero end blocks.
//!
//! The layout constants restate minos-sandbox's (`crates/minos-sandbox/src/untrusted.rs`), which the guest cannot
//! link; the VM tests read them back through minos-sandbox's validating exits, so a drift fails there.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use minos_core::guest::{
    GuestRole, GuestStatus, GuestStatusError, MAX_STATUS_BYTES, OutputStatus, STATUS_PATH,
};
use minos_core::limits::{
    MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_ENTRY_BYTES, MAX_PATH_DEPTH,
    MAX_SCORECARD_BYTES,
};
use minos_core::outcome::Stage;
use minos_core::path::RelPath;
use tar::{Builder, EntryType, Header};

use crate::nofollow::{open_nofollow, same_file};

/// The build stage's log in the output tar.
pub const BUILD_LOG: &str = ".minos/build.log";

/// The run stage's log in the output tar.
pub const RUN_LOG: &str = ".minos/run.log";

/// The judge stage's log in the output tar.
pub const JUDGE_LOG: &str = ".minos/judge.log";

/// The judge's scorecard in the output tar; only a judge output has it.
pub const SCORECARD: &str = ".minos/scorecard.json";

/// The directory a candidate's `/out` tree is packed under.
pub const OUT_PREFIX: &str = "out";

/// Where the judge writes its scorecard, relative to `/out`.
pub const SCORECARD_FILE: &str = "scorecard.json";

/// Most bytes kept of one stage log. Three logs and the status stay far under the 16 MiB content cap.
pub const LOG_CAP_BYTES: usize = 262_144;

/// The line that ends a log cut to [`LOG_CAP_BYTES`].
pub const LOG_TRUNCATED_LINE: &[u8] = b"\n[minos-guest: log truncated]\n";

/// Entries PID 1 itself may add to an output: the status and two logs, or the status, one log, and a scorecard.
/// The `/out` walk keeps this many entries free under [`MAX_OUTPUT_ENTRIES`], so the whole tar stays within the
/// host reader's entry cap.
pub const PID1_ENTRIES: usize = 3;

/// Most `/out` files one output can carry.
pub const MAX_OUT_FILES: usize = MAX_OUTPUT_ENTRIES - PID1_ENTRIES;

/// Most directories the `/out` walk visits. Directories are not packed, but each costs a `read_dir`.
pub const MAX_OUT_DIRS: usize = MAX_OUTPUT_ENTRIES;

/// The output path of `stage`'s log.
#[must_use]
pub const fn log_path(stage: Stage) -> &'static str {
    match stage {
        Stage::Build => BUILD_LOG,
        Stage::Run => RUN_LOG,
        Stage::Judge => JUDGE_LOG,
    }
}

/// `log` as packed: unchanged up to [`LOG_CAP_BYTES`], otherwise its first bytes followed by
/// [`LOG_TRUNCATED_LINE`], exactly [`LOG_CAP_BYTES`] long.
#[must_use]
pub fn cap_log(log: &[u8]) -> Cow<'_, [u8]> {
    let _ = log;
    Cow::Borrowed(&[])
}

/// The content budget left for `/out` once PID 1's own files are counted: the content cap, less the largest status
/// and every log as it will be packed.
#[must_use]
pub fn out_budget(logs: &BTreeMap<Stage, Vec<u8>>) -> u64 {
    let _ = logs;
    0
}

/// Walks `out_dir` without following links and returns the output status and the files under `out/`, in path
/// order. `budget` caps their content bytes. On [`OutputStatus::Invalid`] or [`OutputStatus::TooLarge`] the list
/// is empty.
#[must_use]
pub fn walk_out(out_dir: &Path, budget: u64) -> (OutputStatus, Vec<(RelPath, Vec<u8>)>) {
    let _ = (out_dir, budget);
    (OutputStatus::NotPacked, Vec::new())
}

/// The judge's scorecard, `out_dir/scorecard.json`: a one-link regular file of at most [`MAX_SCORECARD_BYTES`],
/// opened without following a link. Anything else is `None`, which the host reports as a missing scorecard.
#[must_use]
pub fn read_scorecard(out_dir: &Path) -> Option<Vec<u8>> {
    let _ = out_dir;
    None
}

/// Why an output tar could not be built. Each is a PID 1 bug, never a candidate fault: the walk already refused
/// whatever the candidate put in `/out`.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// The status did not serialize.
    #[error("the status did not serialize")]
    Status(#[source] GuestStatusError),
    /// A log for a stage the status's role does not run.
    #[error("a log for a stage the role does not run")]
    Stage,
    /// `out/` files for a judge, or a scorecard for a candidate.
    #[error("the output does not match the status's role")]
    Role,
    /// An `out/` file outside `out/`.
    #[error("an out file is not under out/")]
    OutPath,
    /// A scorecard over [`MAX_SCORECARD_BYTES`].
    #[error("the scorecard is over its cap")]
    Scorecard,
    /// Two entries with one path.
    #[error("two output entries have the same path")]
    Duplicate,
    /// Over an output cap.
    #[error("the output is over a cap")]
    TooLarge,
    /// A path does not fit a ustar header.
    #[error("an output path does not fit a ustar header")]
    Header,
    /// Writing the tar to memory failed.
    #[error("could not write the output tar")]
    Io(#[source] io::Error),
}

/// The output tar of `status`, `logs`, `out`, and, for a judge, `scorecard`, in the [module docs](self) layout.
///
/// # Errors
///
/// A [`PackError`] for a combination the role does not allow or an output over a cap.
pub fn build_output(
    status: &GuestStatus,
    logs: &BTreeMap<Stage, Vec<u8>>,
    out: &[(RelPath, Vec<u8>)],
    scorecard: Option<&[u8]>,
) -> Result<Vec<u8>, PackError> {
    let _ = (status, logs, out, scorecard);
    Err(PackError::TooLarge)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Cursor;

    use minos_core::guest::{ApplyStatus, SetupStatus, StageEnd, StageReport};

    use super::*;

    type TestResult = Result<(), Box<dyn Error>>;

    fn write(root: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)
    }

    fn names(entries: &[(RelPath, Vec<u8>)]) -> Vec<&str> {
        entries.iter().map(|(path, _)| path.as_str()).collect()
    }

    fn candidate_status(output: OutputStatus) -> Result<GuestStatus, GuestStatusError> {
        GuestStatus::new(
            GuestRole::Candidate,
            SetupStatus::Ok,
            ApplyStatus::Applied,
            vec![
                StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
                StageReport::new(Stage::Run, StageEnd::Exited { code: 0 }),
            ],
            output,
        )
    }

    fn judge_status() -> Result<GuestStatus, GuestStatusError> {
        GuestStatus::new(
            GuestRole::Judge,
            SetupStatus::Ok,
            ApplyStatus::NotApplicable,
            vec![StageReport::new(Stage::Judge, StageEnd::Exited { code: 0 })],
            OutputStatus::Packed {
                files: 1,
                content_bytes: 2,
            },
        )
    }

    /// Every entry of `tar` as (name, bytes), checking each header is plain ustar with zeroed metadata.
    fn read_tar(tar: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Box<dyn Error>> {
        assert_eq!(tar.len() % 512, 0);
        let mut entries = Vec::new();
        let mut archive = tar::Archive::new(Cursor::new(tar));
        for entry in archive.entries()?.raw(true) {
            let mut entry = entry?;
            let header = entry.header();
            assert!(header.as_ustar().is_some());
            assert_eq!(header.entry_type(), EntryType::Regular);
            assert_eq!(header.mtime()?, 0);
            assert_eq!(header.uid()?, 0);
            assert_eq!(header.gid()?, 0);
            assert_eq!(header.mode()?, 0o644);
            assert_eq!(header.username_bytes(), Some(&b""[..]));
            assert_eq!(header.groupname_bytes(), Some(&b""[..]));
            let name = String::from_utf8(header.path_bytes().into_owned())?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            entries.push((name, bytes));
        }
        assert!(tar.ends_with(&[0; 1024]));
        Ok(entries)
    }

    fn tar_names(entries: &[(String, Vec<u8>)]) -> Vec<&str> {
        entries.iter().map(|(name, _)| name.as_str()).collect()
    }

    #[test]
    fn walk_out_regular_files() -> TestResult {
        let dir = tempfile::tempdir()?;
        write(dir.path(), "a.txt", b"alpha")?;
        write(dir.path(), "dir/b.bin", b"\x00\x01")?;
        let (status, entries) = walk_out(dir.path(), MAX_OUTPUT_CONTENT_BYTES);
        assert_eq!(
            status,
            OutputStatus::Packed {
                files: 2,
                content_bytes: 7
            }
        );
        assert_eq!(names(&entries), ["out/a.txt", "out/dir/b.bin"]);
        assert_eq!(
            entries
                .iter()
                .map(|(_, bytes)| bytes.as_slice())
                .collect::<Vec<_>>(),
            [&b"alpha"[..], &b"\x00\x01"[..]]
        );

        let empty = tempfile::tempdir()?;
        assert_eq!(
            walk_out(empty.path(), MAX_OUTPUT_CONTENT_BYTES).0,
            OutputStatus::Packed {
                files: 0,
                content_bytes: 0
            }
        );
        Ok(())
    }

    #[test]
    fn walk_out_refuses_links() -> TestResult {
        let secret = tempfile::tempdir()?;
        write(secret.path(), "judge-data", b"secret")?;

        let symlinked = tempfile::tempdir()?;
        write(symlinked.path(), "a.txt", b"a")?;
        fs::create_dir(symlinked.path().join("sub"))?;
        std::os::unix::fs::symlink(
            secret.path().join("judge-data"),
            symlinked.path().join("sub/link"),
        )?;

        let dir_link = tempfile::tempdir()?;
        std::os::unix::fs::symlink(secret.path(), dir_link.path().join("dir"))?;

        let hard = tempfile::tempdir()?;
        write(hard.path(), "a.txt", b"a")?;
        fs::hard_link(hard.path().join("a.txt"), hard.path().join("b.txt"))?;

        let bad_name = tempfile::tempdir()?;
        write(bad_name.path(), "bad\\name", b"x")?;

        let socket = tempfile::tempdir()?;
        let _listener = std::os::unix::net::UnixListener::bind(socket.path().join("s"))?;

        for dir in [&symlinked, &dir_link, &hard, &bad_name, &socket] {
            let (status, entries) = walk_out(dir.path(), MAX_OUTPUT_CONTENT_BYTES);
            assert_eq!(status, OutputStatus::Invalid);
            assert!(entries.is_empty());
        }

        let missing = secret.path().join("missing");
        assert_eq!(
            walk_out(&missing, MAX_OUTPUT_CONTENT_BYTES).0,
            OutputStatus::Invalid
        );
        let not_dir = secret.path().join("judge-data");
        assert_eq!(
            walk_out(&not_dir, MAX_OUTPUT_CONTENT_BYTES).0,
            OutputStatus::Invalid
        );
        Ok(())
    }

    #[test]
    fn walk_out_caps() -> TestResult {
        let many = tempfile::tempdir()?;
        for index in 0..=MAX_OUTPUT_ENTRIES {
            write(many.path(), &format!("f{index}"), b"")?;
        }
        let (status, entries) = walk_out(many.path(), MAX_OUTPUT_CONTENT_BYTES);
        assert_eq!(status, OutputStatus::TooLarge);
        assert!(entries.is_empty());

        let big = tempfile::tempdir()?;
        let size = usize::try_from(MAX_OUTPUT_ENTRY_BYTES)? + 1;
        write(big.path(), "big", &vec![0; size])?;
        let (status, entries) = walk_out(big.path(), MAX_OUTPUT_CONTENT_BYTES);
        assert_eq!(status, OutputStatus::TooLarge);
        assert!(entries.is_empty());

        let budget = tempfile::tempdir()?;
        write(budget.path(), "a", b"123456")?;
        write(budget.path(), "b", b"123456")?;
        assert_eq!(
            walk_out(budget.path(), 12).0,
            OutputStatus::Packed {
                files: 2,
                content_bytes: 12
            }
        );
        let (status, entries) = walk_out(budget.path(), 11);
        assert_eq!(status, OutputStatus::TooLarge);
        assert!(entries.is_empty());

        let deep = tempfile::tempdir()?;
        let path = vec!["d"; MAX_PATH_DEPTH].join("/");
        write(deep.path(), &format!("{path}/f"), b"x")?;
        assert_eq!(
            walk_out(deep.path(), MAX_OUTPUT_CONTENT_BYTES).0,
            OutputStatus::Invalid
        );
        Ok(())
    }

    #[test]
    fn build_output_candidate() -> TestResult {
        let dir = tempfile::tempdir()?;
        write(dir.path(), "a.txt", b"alpha")?;
        let (output, out) = walk_out(dir.path(), MAX_OUTPUT_CONTENT_BYTES);
        let status = candidate_status(output)?;
        let logs = BTreeMap::from([
            (Stage::Build, b"built\n".to_vec()),
            (Stage::Run, b"ran\n".to_vec()),
        ]);
        let tar = build_output(&status, &logs, &out, None)?;
        let entries = read_tar(&tar)?;
        assert_eq!(
            tar_names(&entries),
            [BUILD_LOG, RUN_LOG, STATUS_PATH, "out/a.txt"]
        );
        let status_bytes = entries
            .iter()
            .find(|(name, _)| name == STATUS_PATH)
            .map(|(_, bytes)| bytes.as_slice())
            .ok_or("no status")?;
        assert_eq!(GuestStatus::parse_json(status_bytes)?, status);
        assert!(
            entries
                .iter()
                .any(|(name, bytes)| name == BUILD_LOG && bytes == b"built\n")
        );
        Ok(())
    }

    #[test]
    fn build_output_judge() -> TestResult {
        let dir = tempfile::tempdir()?;
        write(dir.path(), SCORECARD_FILE, b"{}")?;
        write(dir.path(), "other.txt", b"ignored")?;
        let card = read_scorecard(dir.path()).ok_or("no scorecard")?;
        let status = judge_status()?;
        let logs = BTreeMap::from([(Stage::Judge, b"judged\n".to_vec())]);
        let entries = read_tar(&build_output(&status, &logs, &[], Some(&card))?)?;
        assert_eq!(tar_names(&entries), [JUDGE_LOG, SCORECARD, STATUS_PATH]);
        assert!(
            entries
                .iter()
                .any(|(name, bytes)| name == SCORECARD && bytes == b"{}")
        );

        let entries = read_tar(&build_output(&status, &logs, &[], None)?)?;
        assert_eq!(tar_names(&entries), [JUDGE_LOG, STATUS_PATH]);

        let (_, out) = walk_out(dir.path(), MAX_OUTPUT_CONTENT_BYTES);
        assert!(matches!(
            build_output(&status, &logs, &out, Some(&card)),
            Err(PackError::Role)
        ));
        let candidate_log = BTreeMap::from([(Stage::Build, Vec::new())]);
        assert!(matches!(
            build_output(&status, &candidate_log, &[], None),
            Err(PackError::Stage)
        ));
        assert!(matches!(
            build_output(&status, &logs, &[], Some(&[b' '; MAX_SCORECARD_BYTES + 1])),
            Err(PackError::Scorecard)
        ));
        Ok(())
    }

    #[test]
    fn read_scorecard_refusals() -> TestResult {
        let secret = tempfile::tempdir()?;
        write(secret.path(), "judge-data", b"secret")?;

        let link = tempfile::tempdir()?;
        std::os::unix::fs::symlink(
            secret.path().join("judge-data"),
            link.path().join(SCORECARD_FILE),
        )?;
        assert_eq!(read_scorecard(link.path()), None);

        let hard = tempfile::tempdir()?;
        write(hard.path(), "card", b"{}")?;
        fs::hard_link(hard.path().join("card"), hard.path().join(SCORECARD_FILE))?;
        assert_eq!(read_scorecard(hard.path()), None);

        let big = tempfile::tempdir()?;
        write(big.path(), SCORECARD_FILE, &[b' '; MAX_SCORECARD_BYTES + 1])?;
        assert_eq!(read_scorecard(big.path()), None);

        let exact = tempfile::tempdir()?;
        write(exact.path(), SCORECARD_FILE, &[b' '; MAX_SCORECARD_BYTES])?;
        assert_eq!(
            read_scorecard(exact.path()).map(|card| card.len()),
            Some(MAX_SCORECARD_BYTES)
        );

        let absent = tempfile::tempdir()?;
        assert_eq!(read_scorecard(absent.path()), None);
        Ok(())
    }

    #[test]
    fn candidate_never_carries_scorecard() -> TestResult {
        let dir = tempfile::tempdir()?;
        write(dir.path(), SCORECARD_FILE, b"{\"v\":1,\"metrics\":[]}")?;
        let (output, out) = walk_out(dir.path(), MAX_OUTPUT_CONTENT_BYTES);
        let status = candidate_status(output)?;
        let logs = BTreeMap::new();
        let entries = read_tar(&build_output(&status, &logs, &out, None)?)?;
        assert_eq!(tar_names(&entries), [STATUS_PATH, "out/scorecard.json"]);
        assert!(!entries.iter().any(|(name, _)| name == SCORECARD));

        assert!(matches!(
            build_output(&status, &logs, &out, Some(b"{}")),
            Err(PackError::Role)
        ));
        let forged = vec![(RelPath::try_from(SCORECARD)?, b"{}".to_vec())];
        assert!(matches!(
            build_output(&status, &logs, &forged, None),
            Err(PackError::OutPath)
        ));
        let bare = vec![(RelPath::try_from(OUT_PREFIX)?, b"{}".to_vec())];
        assert!(matches!(
            build_output(&status, &logs, &bare, None),
            Err(PackError::OutPath)
        ));
        Ok(())
    }

    #[test]
    fn logs_are_capped() -> TestResult {
        let log = vec![b'x'; 1_048_576];
        let capped = cap_log(&log);
        assert_eq!(capped.len(), LOG_CAP_BYTES);
        assert!(capped.ends_with(LOG_TRUNCATED_LINE));
        assert_eq!(cap_log(b"short").as_ref(), b"short");
        let exact = vec![b'y'; LOG_CAP_BYTES];
        assert_eq!(cap_log(&exact).as_ref(), exact.as_slice());

        let status = candidate_status(OutputStatus::Packed {
            files: 0,
            content_bytes: 0,
        })?;
        let logs = BTreeMap::from([(Stage::Build, log)]);
        let entries = read_tar(&build_output(&status, &logs, &[], None)?)?;
        let packed = entries
            .iter()
            .find(|(name, _)| name == BUILD_LOG)
            .map(|(_, bytes)| bytes.len())
            .ok_or("no build log")?;
        assert_eq!(packed, LOG_CAP_BYTES);

        let budget = MAX_OUTPUT_CONTENT_BYTES
            - u64::try_from(MAX_STATUS_BYTES)?
            - u64::try_from(LOG_CAP_BYTES)?;
        assert_eq!(out_budget(&logs), budget);
        Ok(())
    }
}
