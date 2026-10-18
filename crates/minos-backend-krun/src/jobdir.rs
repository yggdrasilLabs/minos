//! Job directories, the owner lock that marks a job live, the helper's pid file, and the sweep that cleans up
//! after a supervisor that died (KRUN-05; 05-RESEARCH Pattern 8).
//!
//! Every job gets a fresh 0700 directory `mnsXXXXXXXX` under the private job root `/private/tmp/minos-<uid>/`,
//! holding the helper's launch config, the job's input disk, the vsock route socket, `owner.lock`, and `pid`. The
//! supervisor holds an exclusive lock on `owner.lock` for the job's whole life, so a lock that can be taken means
//! the supervisor is gone, and `pid` names the helper it launched (also the helper's process group, since the
//! helper leads its own group). The job root is never shared with a guest: the guest gets disk images, never a
//! host directory.
//!
//! The parent-death watchdog in `minos-vmm` is the primary mechanism that stops a VM whose supervisor died: the
//! helper exits 126 within about 300 ms of losing its parent (02-SPIKE-EVIDENCE.md K7-07, K7-11). [`sweep`] is
//! the backstop, run at backend start and in `minos host doctor`. It also deletes the leftover input disks, which
//! can hold judge data. It touches only directories this module creates (the `mns` name, an owner lock it can
//! take), and it signals a process only after `ps` shows that the pid is an orphan (ppid 1) running exactly this
//! job's helper argv, so a reused pid is never killed. A failure it cannot explain, `EPERM` from a group signal
//! included, leaves the directory in place and is reported, never treated as "gone" (05-RESEARCH Pitfall 11).

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, File, OpenOptions, Permissions, TryLockError};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process_group};
use tempfile::TempDir;

use crate::host::run_tool;

/// The directory the job root lives in. Canonical (`/private/tmp`, never `/tmp`), because Seatbelt compares its
/// rules with resolved paths, and short, so every socket path fits a Unix socket address.
pub const JOB_ROOT_PARENT: &str = "/private/tmp";

/// The job root's name is this prefix followed by the user's uid.
pub const ROOT_PREFIX: &str = "minos-";

/// Every job directory's name is this prefix followed by [`JOB_RANDOM_CHARS`] random ASCII letters and digits.
pub const JOB_PREFIX: &str = "mns";

/// How many random characters follow [`JOB_PREFIX`].
pub const JOB_RANDOM_CHARS: usize = 8;

/// The file whose exclusive lock the supervisor holds for the job's life.
pub const OWNER_LOCK_FILE: &str = "owner.lock";

/// The file holding the helper's pid, as decimal digits and a newline.
pub const PID_FILE: &str = "pid";

/// The helper's launch config.
pub const CONFIG_FILE: &str = "vmm.json";

/// The job's read-only input disk.
pub const INPUT_FILE: &str = "input.img";

/// The vsock route socket.
pub const SOCKET_FILE: &str = "o.sock";

/// The largest pid file the sweep reads.
pub const MAX_PID_FILE_BYTES: u64 = 32;

/// `ps`, by absolute path.
const PS: &str = "/bin/ps";

/// The most stdout bytes read from one `ps` call.
const PS_OUTPUT_CAP: u64 = 16_384;

/// How long the sweep waits for a killed helper to leave the process table.
const GONE_WINDOW: Duration = Duration::from_secs(2);

/// Pause between `ps` calls while waiting for a killed helper.
const GONE_POLL: Duration = Duration::from_millis(20);

/// `O_NOFOLLOW` in the form `OpenOptionsExt::custom_flags` takes. The flag is a small positive number, so the
/// reinterpretation is exact, and the assert keeps it so.
const O_NOFOLLOW: i32 = {
    let bits = rustix::fs::OFlags::NOFOLLOW.bits();
    assert!(bits <= i32::MAX.unsigned_abs());
    i32::from_ne_bytes(bits.to_ne_bytes())
};

/// Why the job root or a job directory could not be made.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JobDirError {
    /// A filesystem operation failed.
    #[error("could not {action} {}", .path.display())]
    Io {
        /// What was being done.
        action: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The job root is a symlink, which could point anywhere. Remove it.
    #[error("the job root {} is a symlink; remove it", .path.display())]
    Symlink {
        /// The job root.
        path: PathBuf,
    },
    /// The job root exists and is not a directory. Remove it.
    #[error("the job root {} is not a directory; remove it", .path.display())]
    NotDirectory {
        /// The job root.
        path: PathBuf,
    },
    /// The job root belongs to another user. Remove it as that user or as root.
    #[error("the job root {} is owned by uid {owner}, not {uid}; remove it", .path.display())]
    Owner {
        /// The job root.
        path: PathBuf,
        /// Its owner.
        owner: u32,
        /// This process's uid.
        uid: u32,
    },
    /// The job root's mode is not 0700, so another user could reach the jobs in it. Remove it.
    #[error("the job root {} has mode {mode:04o}, not 0700; remove it", .path.display())]
    Mode {
        /// The job root.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
}

/// The private directory every job of this user lives in: `<parent>/minos-<uid>`, mode 0700, owned by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRoot {
    dir: PathBuf,
}

impl JobRoot {
    /// Opens the job root under [`JOB_ROOT_PARENT`], creating it when absent.
    ///
    /// # Errors
    ///
    /// As [`JobRoot::open_at`].
    pub fn open() -> Result<Self, JobDirError> {
        Self::open_at(Path::new(JOB_ROOT_PARENT))
    }

    /// Opens the job root `<parent>/minos-<uid>`, creating it with mode 0700 when absent. `parent` is
    /// canonicalized first, so every job path is the resolved path Seatbelt rules compare against.
    ///
    /// # Errors
    ///
    /// [`JobDirError::Io`] when `parent` cannot be resolved or the root cannot be created or inspected;
    /// [`JobDirError::Symlink`], [`JobDirError::NotDirectory`], [`JobDirError::Owner`], or [`JobDirError::Mode`]
    /// when an existing root is a symlink, not a directory, another user's, or not mode 0700.
    pub fn open_at(parent: &Path) -> Result<Self, JobDirError> {
        let parent = fs::canonicalize(parent).map_err(io_at("resolve", parent))?;
        let uid = rustix::process::getuid().as_raw();
        let dir = parent.join(format!("{ROOT_PREFIX}{uid}"));
        match DirBuilder::new().mode(0o700).create(&dir) {
            // The umask can only clear bits; set the mode exactly on the directory just created.
            Ok(()) => fs::set_permissions(&dir, Permissions::from_mode(0o700))
                .map_err(io_at("set the mode of", &dir))?,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_at("create", &dir)(error)),
        }
        let meta = fs::symlink_metadata(&dir).map_err(io_at("inspect", &dir))?;
        if meta.file_type().is_symlink() {
            return Err(JobDirError::Symlink { path: dir });
        }
        if !meta.is_dir() {
            return Err(JobDirError::NotDirectory { path: dir });
        }
        if meta.uid() != uid {
            return Err(JobDirError::Owner {
                path: dir,
                owner: meta.uid(),
                uid,
            });
        }
        let mode = meta.mode() & 0o7777;
        if mode != 0o700 {
            return Err(JobDirError::Mode { path: dir, mode });
        }
        Ok(Self { dir })
    }

    /// The job root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// Makes a fresh 0700 job directory `mnsXXXXXXXX` and takes the exclusive lock on its new `owner.lock`, held
    /// until the [`JobDir`] is dropped.
    ///
    /// # Errors
    ///
    /// [`JobDirError::Io`] when the directory or the lock file cannot be created, or the lock cannot be taken.
    pub fn create_job(&self) -> Result<JobDir, JobDirError> {
        let dir = tempfile::Builder::new()
            .prefix(JOB_PREFIX)
            .rand_bytes(JOB_RANDOM_CHARS)
            .permissions(Permissions::from_mode(0o700))
            .tempdir_in(&self.dir)
            .map_err(io_at("create a job directory in", &self.dir))?;
        let lock_path = dir.path().join(OWNER_LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(&lock_path)
            .map_err(io_at("create", &lock_path))?;
        lock.lock().map_err(io_at("lock", &lock_path))?;
        Ok(JobDir { dir, _owner: lock })
    }
}

/// One job's directory, live while this value exists. Dropping it removes the directory and then releases the
/// owner lock, so the sweep never sees a removable directory of a live job.
#[derive(Debug)]
pub struct JobDir {
    // Fields drop in declaration order: the directory is removed while the lock is still held.
    dir: TempDir,
    /// The exclusive lock on `owner.lock`, held for the job's life and never read.
    _owner: File,
}

impl JobDir {
    /// The job directory, canonical.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The helper's launch config, `vmm.json`.
    #[must_use]
    pub fn config(&self) -> PathBuf {
        self.dir.path().join(CONFIG_FILE)
    }

    /// The job's input disk, `input.img`.
    #[must_use]
    pub fn input(&self) -> PathBuf {
        self.dir.path().join(INPUT_FILE)
    }

    /// The vsock route socket, `o.sock`.
    #[must_use]
    pub fn socket(&self) -> PathBuf {
        self.dir.path().join(SOCKET_FILE)
    }

    /// The helper's pid file, `pid`.
    #[must_use]
    pub fn pid_file(&self) -> PathBuf {
        self.dir.path().join(PID_FILE)
    }

    /// The owner lock, `owner.lock`.
    #[must_use]
    pub fn owner_lock(&self) -> PathBuf {
        self.dir.path().join(OWNER_LOCK_FILE)
    }

    /// Records the helper's pid as `<pid>\n` in a new file with mode 0600. A job records one helper, so a second
    /// call fails.
    ///
    /// # Errors
    ///
    /// [`JobDirError::Io`] when the pid file exists already or cannot be written.
    pub fn write_pid(&self, pid: u32) -> Result<(), JobDirError> {
        let path = self.pid_file();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(&path)
            .map_err(io_at("create", &path))?;
        file.write_all(format!("{pid}\n").as_bytes())
            .map_err(io_at("write", &path))
    }
}

/// What the sweep did with one job directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepEntry {
    /// The supervisor and the helper were gone; the directory was removed.
    Removed {
        /// The job directory.
        dir: PathBuf,
    },
    /// The supervisor was gone and its orphaned helper was still running; the helper's process group was killed
    /// and the directory removed.
    Killed {
        /// The job directory.
        dir: PathBuf,
        /// The helper's pid and process group.
        pid: u32,
    },
    /// A supervisor holds the owner lock; the directory was left.
    Live {
        /// The job directory.
        dir: PathBuf,
    },
    /// The sweep could not prove the directory is safe to clean; it was left, and nothing was signalled.
    Unverified {
        /// The job directory.
        dir: PathBuf,
        /// What could not be proved.
        reason: &'static str,
    },
}

/// Every job directory the sweep looked at, in name order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepReport {
    entries: Vec<SweepEntry>,
}

impl SweepReport {
    /// What the sweep did, one entry per job directory, in name order.
    #[must_use]
    pub fn entries(&self) -> &[SweepEntry] {
        &self.entries
    }
}

/// Cleans up job directories whose supervisor died, killing an orphaned helper first.
///
/// For each `mnsXXXXXXXX` directory in `root` (in name order; nothing else in the root is touched): when its
/// owner lock is held the job is [`SweepEntry::Live`]. When the lock can be taken the supervisor is gone, so the
/// pid file is read (at most [`MAX_PID_FILE_BYTES`] bytes, plain decimal, above 1). With no pid file the helper
/// was never started and the directory is removed. Otherwise `/bin/ps -ww -o ppid=,command= -p <pid>` decides:
/// no row means the helper is gone and the directory is removed; a row with ppid 1 and exactly
/// `<vmm> <dir>/vmm.json` as its command is the orphaned helper, so its process group gets SIGKILL and the
/// directory is removed once `ps` shows no row. Anything else (a row that is not this job's orphaned helper, a
/// signal that fails with `EPERM` or any errno but `ESRCH`, a `ps` that fails, a bad pid file) is
/// [`SweepEntry::Unverified`] and leaves the directory. `vmm` is the helper path the backend launches, resolved
/// before comparing.
#[must_use]
pub fn sweep(root: &JobRoot, vmm: &Path) -> SweepReport {
    let _ = (root, vmm);
    SweepReport { entries: Vec::new() }
}

/// Whether `name` is `mns` followed by exactly [`JOB_RANDOM_CHARS`] ASCII letters and digits.
fn is_job_name(name: &OsStr) -> bool {
    let _ = name;
    false
}

/// Sweeps one job directory.
fn sweep_one(dir: &Path, vmm: &Path) -> SweepEntry {
    let unverified = |reason| SweepEntry::Unverified {
        dir: dir.to_path_buf(),
        reason,
    };
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return unverified("it is not a directory"),
        Err(_) => return unverified("it could not be inspected"),
    }
    // A directory without a lock file is either being created right now or was not made by this module.
    let Ok(lock) = open_no_follow(&dir.join(OWNER_LOCK_FILE)) else {
        return unverified("it has no owner lock that could be opened");
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return SweepEntry::Live {
                dir: dir.to_path_buf(),
            };
        }
        Err(TryLockError::Error(_)) => return unverified("its owner lock could not be probed"),
    }
    // The lock was free, so the supervisor is gone. The lock is held until this function returns.
    let pid = match read_pid(&dir.join(PID_FILE)) {
        Ok(None) => return remove(dir, None),
        Ok(Some(pid)) => pid,
        Err(reason) => return unverified(reason),
    };
    let row = match ps_row(pid) {
        Ok(None) => return remove(dir, None),
        Ok(Some(row)) => row,
        Err(reason) => return unverified(reason),
    };
    let mut expected = vmm.as_os_str().as_bytes().to_vec();
    expected.push(b' ');
    expected.extend_from_slice(dir.join(CONFIG_FILE).as_os_str().as_bytes());
    if row != (1, expected) {
        return unverified("its pid names a process that is not this job's orphaned helper");
    }
    let Some(group) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return unverified("its pid is out of range");
    };
    match kill_process_group(group, Signal::KILL) {
        Ok(()) | Err(Errno::SRCH) => {}
        Err(_) => return unverified("the helper's process group could not be signalled"),
    }
    if !gone_within(pid, GONE_WINDOW) {
        return unverified("the helper still shows in ps after SIGKILL");
    }
    remove(dir, Some(pid))
}

/// Removes `dir`, reporting [`SweepEntry::Killed`] when a helper was killed first and [`SweepEntry::Removed`]
/// otherwise.
fn remove(dir: &Path, killed: Option<u32>) -> SweepEntry {
    if fs::remove_dir_all(dir).is_err() {
        return SweepEntry::Unverified {
            dir: dir.to_path_buf(),
            reason: "it could not be removed",
        };
    }
    let dir = dir.to_path_buf();
    match killed {
        Some(pid) => SweepEntry::Killed { dir, pid },
        None => SweepEntry::Removed { dir },
    }
}

/// Opens `path` read-only without following a symlink.
fn open_no_follow(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

/// The pid in a job's pid file: `Ok(None)` when there is no pid file, `Ok(Some(pid))` for plain decimal digits
/// and an optional newline naming a pid above 1 that fits an `i32`, and an error naming what is wrong otherwise.
fn read_pid(path: &Path) -> Result<Option<u32>, &'static str> {
    let file = match open_no_follow(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("its pid file could not be opened"),
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err("its pid file is not a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_PID_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "its pid file could not be read")?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_PID_FILE_BYTES) {
        return Err("its pid file is over 32 bytes");
    }
    parse_pid(&bytes)
}

/// Plain decimal digits and an optional newline, naming a pid above 1 that fits an `i32`.
fn parse_pid(bytes: &[u8]) -> Result<Option<u32>, &'static str> {
    let _ = bytes;
    Ok(None)
}

/// The `ps` row of `pid` as `(ppid, command)`, or `None` when `ps` shows no such process.
fn ps_row(pid: u32) -> Result<Option<(u32, Vec<u8>)>, &'static str> {
    let pid = pid.to_string();
    let args = ["-ww", "-o", "ppid=,command=", "-p", pid.as_str()];
    let output = run_tool(Path::new(PS), &args, PS_OUTPUT_CAP).map_err(|_| "ps could not run")?;
    if !output.status.success() {
        // ps exits 1 with no row when the pid names no process; any other failure proves nothing.
        return if output.status.code() == Some(1) && output.stdout.is_empty() {
            Ok(None)
        } else {
            Err("ps failed")
        };
    }
    parse_ps_row(&output.stdout)
        .map(Some)
        .ok_or("ps printed something other than one row")
}

/// One `ps -o ppid=,command=` row: optional leading spaces, the ppid digits, spaces, then the command, with an
/// optional final newline and no other line.
fn parse_ps_row(stdout: &[u8]) -> Option<(u32, Vec<u8>)> {
    let _ = stdout;
    None
}

/// Whether `ps` shows no row for `pid` within `window`. A failed `ps` is not proof, so it counts as still there.
fn gone_within(pid: u32, window: Duration) -> bool {
    let start = Instant::now();
    loop {
        match ps_row(pid) {
            Ok(None) => return true,
            Ok(Some(_)) | Err(_) if start.elapsed() < window => thread::sleep(GONE_POLL),
            Ok(Some(_)) | Err(_) => return false,
        }
    }
}

/// Maps an `io::Error` to [`JobDirError::Io`] for `action` on `path`.
fn io_at(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> JobDirError {
    let path = path.to_path_buf();
    move |source| JobDirError::Io {
        action,
        path,
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs::{self, File, Permissions, TryLockError};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};

    use super::{
        JOB_PREFIX, JOB_RANDOM_CHARS, JobDirError, JobRoot, OWNER_LOCK_FILE, PID_FILE, ROOT_PREFIX,
        SweepEntry, is_job_name, parse_pid, parse_ps_row, sweep,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// The job root's name for this user.
    fn root_name() -> String {
        format!("{ROOT_PREFIX}{}", rustix::process::getuid().as_raw())
    }

    /// A job directory whose supervisor is gone: the layout `create_job` makes, with nobody holding the lock,
    /// and `pid` holding `pid_bytes` when given.
    fn dead_job(
        root: &JobRoot,
        name: &str,
        pid_bytes: Option<&[u8]>,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let dir = root.path().join(name);
        fs::create_dir(&dir)?;
        fs::set_permissions(&dir, Permissions::from_mode(0o700))?;
        File::create(dir.join(OWNER_LOCK_FILE))?;
        if let Some(bytes) = pid_bytes {
            fs::write(dir.join(PID_FILE), bytes)?;
        }
        Ok(dir)
    }

    /// Kills and reaps the child on drop, so a failed assertion leaves no process behind.
    struct Reaped(Child);

    impl Drop for Reaped {
        fn drop(&mut self) {
            self.0.kill().ok();
            self.0.wait().ok();
        }
    }

    #[test]
    fn job_root_is_private() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        assert_eq!(
            root.path(),
            fs::canonicalize(parent.path())?.join(root_name())
        );
        assert_eq!(
            fs::symlink_metadata(root.path())?.permissions().mode() & 0o7777,
            0o700
        );
        assert_eq!(JobRoot::open_at(parent.path())?, root);

        let linked = tempfile::tempdir()?;
        let target = tempfile::tempdir()?;
        symlink(target.path(), linked.path().join(root_name()))?;
        assert!(matches!(
            JobRoot::open_at(linked.path()),
            Err(JobDirError::Symlink { .. })
        ));

        let open = tempfile::tempdir()?;
        let wide = open.path().join(root_name());
        fs::create_dir(&wide)?;
        fs::set_permissions(&wide, Permissions::from_mode(0o755))?;
        assert!(matches!(
            JobRoot::open_at(open.path()),
            Err(JobDirError::Mode { mode: 0o755, .. })
        ));

        let filed = tempfile::tempdir()?;
        fs::write(filed.path().join(root_name()), b"not a directory")?;
        assert!(matches!(
            JobRoot::open_at(filed.path()),
            Err(JobDirError::NotDirectory { .. })
        ));
        Ok(())
    }

    #[test]
    fn job_dir_lifecycle() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        let job = root.create_job()?;
        let dir = job.path().to_path_buf();
        assert_eq!(dir.parent(), Some(root.path()));
        let name = dir.file_name().ok_or("a job directory has a name")?;
        assert!(is_job_name(name));
        assert_eq!(name.len(), JOB_PREFIX.len() + JOB_RANDOM_CHARS);
        assert_eq!(
            fs::symlink_metadata(&dir)?.permissions().mode() & 0o7777,
            0o700
        );
        for (path, file) in [
            (job.config(), "vmm.json"),
            (job.input(), "input.img"),
            (job.socket(), "o.sock"),
            (job.pid_file(), "pid"),
            (job.owner_lock(), "owner.lock"),
        ] {
            assert_eq!(path, dir.join(file));
        }

        let other = File::open(job.owner_lock())?;
        assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));

        job.write_pid(4242)?;
        assert_eq!(fs::read(job.pid_file())?, b"4242\n");
        assert_eq!(
            fs::metadata(job.pid_file())?.permissions().mode() & 0o777,
            0o600
        );
        assert!(job.write_pid(4243).is_err());
        assert_eq!(fs::read(job.pid_file())?, b"4242\n");

        drop(job);
        assert!(!dir.exists());
        Ok(())
    }

    #[test]
    fn sweep_removes_dead_dirs() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        let dead = dead_job(&root, "mnsDEAD0001", None)?;
        // Names this module never makes are not touched.
        let foreign = root.path().join("mnsSHORT");
        fs::create_dir(&foreign)?;
        let unrelated = root.path().join("keep");
        fs::create_dir(&unrelated)?;

        let report = sweep(&root, Path::new("/nonexistent/minos-vmm"));
        assert_eq!(
            report.entries(),
            [SweepEntry::Removed { dir: dead.clone() }]
        );
        assert!(!dead.exists());
        assert!(foreign.exists());
        assert!(unrelated.exists());
        Ok(())
    }

    #[test]
    fn sweep_skips_live_dirs() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        let job = root.create_job()?;
        job.write_pid(std::process::id())?;
        let report = sweep(&root, Path::new("/nonexistent/minos-vmm"));
        assert_eq!(
            report.entries(),
            [SweepEntry::Live {
                dir: job.path().to_path_buf()
            }]
        );
        assert!(job.path().exists());
        Ok(())
    }

    #[test]
    fn sweep_guards_pid_reuse() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;

        // A process whose command differs from the helper's argv.
        let mut sleeper = Reaped(
            Command::new("/bin/sleep")
                .arg("30")
                .stdin(Stdio::null())
                .spawn()?,
        );
        let pid = sleeper.0.id();
        let dir = dead_job(&root, "mnsREUSED01", Some(format!("{pid}\n").as_bytes()))?;
        let report = sweep(&root, Path::new("/bin/sleep"));
        assert!(
            matches!(report.entries(), [SweepEntry::Unverified { dir: seen, .. }] if *seen == dir),
            "{report:?}"
        );
        assert!(dir.exists());
        assert!(
            sleeper.0.try_wait()?.is_none(),
            "the unrelated process is alive"
        );
        sleeper.0.kill()?;
        sleeper.0.wait()?;

        // A process with exactly the helper's argv (`cat <dir>/vmm.json`, blocked opening a FIFO) whose parent
        // is this test, not launchd: ppid alone stops the kill.
        let exact = dead_job(&root, "mnsEXACT001", None)?;
        let fifo = exact.join("vmm.json");
        let made = Command::new("/usr/bin/mkfifo").arg(&fifo).status()?;
        assert!(made.success());
        let mut cat = Reaped(
            Command::new("/bin/cat")
                .arg(&fifo)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()?,
        );
        fs::write(exact.join(PID_FILE), format!("{}\n", cat.0.id()))?;
        let report = sweep(&root, Path::new("/bin/cat"));
        // The sleeper is gone and reaped, so its directory is removed now; the exact-argv one is left.
        assert!(
            matches!(
                report.entries(),
                [SweepEntry::Unverified { dir: first, .. }, SweepEntry::Removed { dir: second }]
                    if *first == exact && *second == dir
            ),
            "{report:?}"
        );
        assert!(exact.exists());
        assert!(!dir.exists());
        assert!(
            cat.0.try_wait()?.is_none(),
            "a process whose ppid is not 1 is never killed"
        );
        cat.0.kill()?;
        cat.0.wait()?;

        // Once the process is gone and reaped, ps shows no row and the directory is removed.
        let report = sweep(&root, Path::new("/bin/cat"));
        assert_eq!(
            report.entries(),
            [SweepEntry::Removed { dir: exact.clone() }]
        );
        Ok(())
    }

    #[test]
    fn pid_file_bounds() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        let long = vec![b'1'; 33];
        let cases: [(&str, &[u8]); 5] = [
            ("mnsLONG0001", &long),
            ("mnsWORD0001", b"abc\n"),
            ("mnsZERO0001", b"0\n"),
            ("mnsSIGN0001", b"-5\n"),
            ("mnsINIT0001", b"1\n"),
        ];
        let mut dirs = Vec::new();
        for (name, bytes) in cases {
            dirs.push(dead_job(&root, name, Some(bytes))?);
        }
        let report = sweep(&root, Path::new("/nonexistent/minos-vmm"));
        assert_eq!(report.entries().len(), cases.len());
        for entry in report.entries() {
            assert!(matches!(entry, SweepEntry::Unverified { .. }), "{entry:?}");
        }
        for dir in dirs {
            assert!(dir.exists());
        }
        Ok(())
    }

    #[test]
    fn pid_and_ps_parsing() {
        assert_eq!(parse_pid(b"4242\n"), Ok(Some(4242)));
        assert_eq!(parse_pid(b"4242"), Ok(Some(4242)));
        assert!(parse_pid(b"").is_err());
        assert!(parse_pid(b"42 \n").is_err());
        assert!(parse_pid(b"4294967296").is_err());
        assert!(parse_pid(b"2147483648").is_err());

        assert_eq!(
            parse_ps_row(
                b"    1 /opt/minos/minos-vmm /private/tmp/minos-501/mnsAAAAAAAA/vmm.json\n"
            ),
            Some((
                1,
                b"/opt/minos/minos-vmm /private/tmp/minos-501/mnsAAAAAAAA/vmm.json".to_vec()
            ))
        );
        assert_eq!(
            parse_ps_row(b"512 /bin/sleep 30"),
            Some((512, b"/bin/sleep 30".to_vec()))
        );
        assert_eq!(parse_ps_row(b""), None);
        assert_eq!(parse_ps_row(b"    1\n"), None);
        assert_eq!(parse_ps_row(b"1 a\n1 b\n"), None);
        assert_eq!(parse_ps_row(b"x /bin/sleep\n"), None);
        assert_eq!(parse_ps_row(b"1/bin/sleep\n"), None);

        assert!(is_job_name("mnsAbC12345".as_ref()));
        assert!(!is_job_name("mnsAbC1234".as_ref()));
        assert!(!is_job_name("mnsAbC123456".as_ref()));
        assert!(!is_job_name("mnsAbC1234-".as_ref()));
        assert!(!is_job_name("mnpAbC12345".as_ref()));
    }
}
