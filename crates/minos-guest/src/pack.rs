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
    if log.len() <= LOG_CAP_BYTES {
        return Cow::Borrowed(log);
    }
    let keep = LOG_CAP_BYTES - LOG_TRUNCATED_LINE.len();
    let mut capped = Vec::with_capacity(LOG_CAP_BYTES);
    capped.extend(log.iter().take(keep));
    capped.extend_from_slice(LOG_TRUNCATED_LINE);
    Cow::Owned(capped)
}

/// The content budget left for `/out` once PID 1's own files are counted: the content cap, less the largest status
/// and every log as it will be packed.
#[must_use]
pub fn out_budget(logs: &BTreeMap<Stage, Vec<u8>>) -> u64 {
    let pid1 = logs
        .values()
        .map(|log| log.len().min(LOG_CAP_BYTES))
        .fold(MAX_STATUS_BYTES, usize::saturating_add);
    MAX_OUTPUT_CONTENT_BYTES.saturating_sub(u64::try_from(pid1).unwrap_or(u64::MAX))
}

/// Walks `out_dir` without following links and returns the output status and the files under `out/`, in path
/// order. `budget` caps their content bytes. On [`OutputStatus::Invalid`] or [`OutputStatus::TooLarge`] the list
/// is empty.
#[must_use]
pub fn walk_out(out_dir: &Path, budget: u64) -> (OutputStatus, Vec<(RelPath, Vec<u8>)>) {
    let mut walk = Walk {
        files: Vec::new(),
        content: 0,
        dirs: 0,
        budget,
    };
    let walked = match fs::symlink_metadata(out_dir) {
        Ok(meta) if meta.is_dir() => walk.dir(out_dir, OUT_PREFIX, 1),
        _ => Err(Refused::Invalid),
    };
    let files = u32::try_from(walk.files.len());
    match (walked, files) {
        (Ok(()), Ok(files)) => {
            walk.files.sort_by(|left, right| left.0.cmp(&right.0));
            (
                OutputStatus::Packed {
                    files,
                    content_bytes: walk.content,
                },
                walk.files,
            )
        }
        (Err(Refused::Invalid), _) => (OutputStatus::Invalid, Vec::new()),
        (Err(Refused::TooLarge), _) | (Ok(()), Err(_)) => (OutputStatus::TooLarge, Vec::new()),
    }
}

/// Why the walk stopped.
enum Refused {
    /// Something other than a one-link regular file or a directory, or a name that is not a valid output path.
    Invalid,
    /// Over a cap.
    TooLarge,
}

/// The state of one `/out` walk.
struct Walk {
    files: Vec<(RelPath, Vec<u8>)>,
    content: u64,
    dirs: usize,
    budget: u64,
}

impl Walk {
    /// Walks the directory `dir`, whose output path is `rel` and whose depth in components is `depth`.
    fn dir(&mut self, dir: &Path, rel: &str, depth: usize) -> Result<(), Refused> {
        self.dirs = self.dirs.saturating_add(1);
        if self.dirs > MAX_OUT_DIRS {
            return Err(Refused::TooLarge);
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(dir).map_err(|_| Refused::Invalid)? {
            let entry = entry.map_err(|_| Refused::Invalid)?;
            if names.len() >= MAX_OUT_FILES.saturating_add(MAX_OUT_DIRS) {
                return Err(Refused::TooLarge);
            }
            names.push(entry.file_name());
        }
        names.sort();
        for name in names {
            let text = name.to_str().ok_or(Refused::Invalid)?;
            let child_rel = format!("{rel}/{text}");
            let child = dir.join(&name);
            let meta = fs::symlink_metadata(&child).map_err(|_| Refused::Invalid)?;
            if meta.is_dir() {
                if depth >= MAX_PATH_DEPTH {
                    return Err(Refused::Invalid);
                }
                self.dir(&child, &child_rel, depth.saturating_add(1))?;
            } else if meta.is_file() {
                self.file(&child, &child_rel, &meta)?;
            } else {
                return Err(Refused::Invalid);
            }
        }
        Ok(())
    }

    /// Reads the regular file `path`, whose output path is `rel`, checked as `meta`.
    fn file(&mut self, path: &Path, rel: &str, meta: &fs::Metadata) -> Result<(), Refused> {
        let name = RelPath::try_from(rel).map_err(|_| Refused::Invalid)?;
        if meta.nlink() != 1 || !fits_ustar(&name) {
            return Err(Refused::Invalid);
        }
        if self.files.len() >= MAX_OUT_FILES || meta.len() > MAX_OUTPUT_ENTRY_BYTES {
            return Err(Refused::TooLarge);
        }
        let file = open_nofollow(path).map_err(|_| Refused::Invalid)?;
        let opened = file.metadata().map_err(|_| Refused::Invalid)?;
        if !same_file(meta, &file).map_err(|_| Refused::Invalid)?
            || !opened.is_file()
            || opened.nlink() != 1
        {
            return Err(Refused::Invalid);
        }
        let mut bytes = Vec::new();
        file.take(MAX_OUTPUT_ENTRY_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| Refused::Invalid)?;
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if size > MAX_OUTPUT_ENTRY_BYTES {
            return Err(Refused::TooLarge);
        }
        self.content = self
            .content
            .checked_add(size)
            .filter(|sum| *sum <= self.budget)
            .ok_or(Refused::TooLarge)?;
        self.files.push((name, bytes));
        Ok(())
    }
}

/// Whether `path` fits a ustar header's prefix and name fields.
fn fits_ustar(path: &RelPath) -> bool {
    Header::new_ustar().set_path(path.as_str()).is_ok()
}

/// The judge's scorecard, `out_dir/scorecard.json`: a one-link regular file of at most [`MAX_SCORECARD_BYTES`],
/// opened without following a link. Anything else is `None`, which the host reports as a missing scorecard.
#[must_use]
pub fn read_scorecard(out_dir: &Path) -> Option<Vec<u8>> {
    let path = out_dir.join(SCORECARD_FILE);
    let cap = u64::try_from(MAX_SCORECARD_BYTES).ok()?;
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.nlink() != 1 || meta.len() > cap {
        return None;
    }
    let file = open_nofollow(&path).ok()?;
    let opened = file.metadata().ok()?;
    if !same_file(&meta, &file).ok()? || !opened.is_file() || opened.nlink() != 1 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_SCORECARD_BYTES).then_some(bytes)
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
    let role = status.role();
    let status_json = status.to_json().map_err(PackError::Status)?;
    let out_dir = RelPath::try_from(OUT_PREFIX).map_err(|_| PackError::OutPath)?;

    let mut entries: BTreeMap<RelPath, Cow<'_, [u8]>> = BTreeMap::new();
    add(
        &mut entries,
        fixed_path(STATUS_PATH)?,
        Cow::Owned(status_json),
    )?;
    for (stage, log) in logs {
        if !role.stages().contains(stage) {
            return Err(PackError::Stage);
        }
        add(&mut entries, fixed_path(log_path(*stage))?, cap_log(log))?;
    }
    match role {
        GuestRole::Judge => {
            if !out.is_empty() {
                return Err(PackError::Role);
            }
            if let Some(card) = scorecard {
                if card.len() > MAX_SCORECARD_BYTES {
                    return Err(PackError::Scorecard);
                }
                add(&mut entries, fixed_path(SCORECARD)?, Cow::Borrowed(card))?;
            }
        }
        GuestRole::Candidate => {
            if scorecard.is_some() {
                return Err(PackError::Role);
            }
            for (path, bytes) in out {
                if !path.is_within(&out_dir) {
                    return Err(PackError::OutPath);
                }
                add(&mut entries, path.clone(), Cow::Borrowed(bytes.as_slice()))?;
            }
        }
    }

    if entries.len() > MAX_OUTPUT_ENTRIES {
        return Err(PackError::TooLarge);
    }
    let mut total: u64 = 0;
    let mut builder = Builder::new(Vec::new());
    for (path, bytes) in &entries {
        let size = u64::try_from(bytes.len()).map_err(|_| PackError::TooLarge)?;
        if size > MAX_OUTPUT_ENTRY_BYTES {
            return Err(PackError::TooLarge);
        }
        total = total
            .checked_add(size)
            .filter(|sum| *sum <= MAX_OUTPUT_CONTENT_BYTES)
            .ok_or(PackError::TooLarge)?;
        builder
            .append(&header(path, size)?, bytes.as_ref())
            .map_err(PackError::Io)?;
    }
    builder.into_inner().map_err(PackError::Io)
}

/// Adds one entry, refusing a second entry with the same path.
fn add<'a>(
    entries: &mut BTreeMap<RelPath, Cow<'a, [u8]>>,
    path: RelPath,
    bytes: Cow<'a, [u8]>,
) -> Result<(), PackError> {
    if entries.insert(path, bytes).is_some() {
        return Err(PackError::Duplicate);
    }
    Ok(())
}

/// One of PID 1's own output paths as a `RelPath`.
fn fixed_path(path: &str) -> Result<RelPath, PackError> {
    RelPath::try_from(path).map_err(|_| PackError::Header)
}

/// The header of one entry, built with exactly these calls in this order, as minos-source builds its own.
fn header(path: &RelPath, size: u64) -> Result<Header, PackError> {
    let mut header = Header::new_ustar();
    header
        .set_path(path.as_str())
        .map_err(|_| PackError::Header)?;
    header.set_size(size);
    header.set_mode(0o644);
    // Set explicitly: they write seven `0` digits and a NUL, where new_ustar leaves zero bytes.
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    Ok(header)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Cursor;

    use minos_core::guest::{ApplyStatus, SetupStatus, StageEnd, StageReport};

    use super::*;

    type TestResult = Result<(), Box<dyn Error>>;

    /// A parsed tar: each entry's name and bytes.
    type TarEntries = Vec<(String, Vec<u8>)>;

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
    fn read_tar(tar: &[u8]) -> Result<TarEntries, Box<dyn Error>> {
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
