//! Runs one minos-vmm helper: job files, the vsock output listener, a host deadline, and capped guest bytes.
//!
//! A launch is four steps, in this order (02-05-PLAN.md E-4):
//!
//! 1. [`short_job_dir`] makes a private directory under `/private/tmp`, and [`JobFiles::in_dir`] names the job's
//!    files in it (E-5). The directory is short and canonical so the socket path fits `sun_path` (RESEARCH
//!    Pitfall 5) and passes minos-vmm's canonical-path preflight.
//! 2. [`write_disks`] and [`write_config`] create the root, input, and scratch images and `vmm.json`. Every file is
//!    created new; nothing is overwritten.
//! 3. [`bind_output`] binds the vsock output listener. It must exist before the helper starts: the VMM connects to
//!    it only when the guest connects, and a missing listener resets the guest (Pitfall 6).
//! 4. [`run`] spawns the helper argv with an empty environment, stdin from `/dev/null`, stdout and stderr to files,
//!    and its own process group; collects each guest connection through a byte cap; and kills the helper at the
//!    host deadline.
//!
//! Every byte that comes back (vsock connections, the console log, the helper's stdout and stderr) is guest
//! influenced and hostile. It is read through a cap plus one byte, so overflow is detected rather than silently
//! truncated, and it is returned only as [`GuestBytes`], which has no `Debug`, `Display`, `Deref`, or `Clone`.
//! The host never opens a disk image the guest could have written: the scratch image is created sparse with
//! `set_len` and never read back.

use std::ffi::OsString;
use std::fs::{self, File, Permissions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::launch_config::LaunchConfig;

/// Magic bytes at the start of the guest's status frame (02-01 E-10).
pub const FRAME_MAGIC: [u8; 8] = *b"MINOSST1";
/// Version carried in the status-frame header.
pub const FRAME_VERSION: u32 = 1;
/// Length of the status-frame header: magic, a `u32` version, and a `u64` payload length, both big-endian.
pub const FRAME_HEADER_BYTES: usize = 20;
/// Largest status-frame payload the host accepts, whatever the caller asks for.
pub const MAX_FRAME_BYTES: u64 = 1_048_576;

/// Longest socket path the host binds: macOS `sun_path` is 104 bytes including the terminating NUL.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;
/// Canonical parent of every job directory (`/tmp` is a symlink to it on macOS).
pub const JOB_ROOT: &str = "/private/tmp";

/// Host deadline when [`Launch::host_deadline`] is not called.
pub const DEFAULT_HOST_DEADLINE: Duration = Duration::from_mins(2);
/// Per-connection byte cap when [`Launch::conn_caps`] is not called.
pub const DEFAULT_CONN_CAP: u64 = 1_048_576;
/// Most vsock connections collected when [`Launch::max_connections`] is not called.
pub const DEFAULT_MAX_CONNECTIONS: usize = 4;
/// Console byte cap when [`Launch::console_cap`] is not called.
pub const DEFAULT_CONSOLE_CAP: u64 = 1_048_576;
/// Most bytes kept from each of the helper's stdout and stderr files.
pub const VMM_STDIO_CAP: u64 = 65_536;
/// Most bytes read from `codesign --display` before it is treated as hostile output.
pub const CODESIGN_OUTPUT_CAP: u64 = 65_536;
/// The entitlement Hypervisor.framework requires of the process that creates a VM.
pub const HYPERVISOR_ENTITLEMENT: &str = "com.apple.security.hypervisor";

/// Root image file name (E-5).
pub const ROOT_FILE: &str = "root.img";
/// Job input image file name.
pub const INPUT_FILE: &str = "input.img";
/// Scratch image file name.
pub const SCRATCH_FILE: &str = "scratch.img";
/// vsock output socket file name.
pub const SOCKET_FILE: &str = "o.sock";
/// Console log file name.
pub const CONSOLE_FILE: &str = "console.log";
/// Launch config file name.
pub const CONFIG_FILE: &str = "vmm.json";
/// Helper stdout file name.
pub const VMM_STDOUT_FILE: &str = "vmm.stdout";
/// Helper stderr file name.
pub const VMM_STDERR_FILE: &str = "vmm.stderr";
/// Initramfs file name, used only by 02-06's negative-evidence run.
pub const INITRAMFS_FILE: &str = "initramfs.cpio";

const CODESIGN: &str = "/usr/bin/codesign";
const ENTITLED_XML: &[u8] = b"<key>com.apple.security.hypervisor</key><true/>";
const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Least time a connection gets to deliver bytes already queued, even after the host deadline has passed.
const MIN_READ_WINDOW: Duration = Duration::from_millis(50);
const READ_CHUNK_BYTES: usize = 65_536;

/// Why the supervisor could not prepare or run a launch. A guest that misbehaves is not an error: its bytes come
/// back capped in [`VmRun`], and a helper that outlives the deadline is killed and reported there.
#[derive(Debug, thiserror::Error)]
pub enum SuperviseError {
    /// A host filesystem, socket, or process operation failed.
    #[error("could not {action} {}", .path.display())]
    Io {
        /// What the supervisor was doing.
        action: &'static str,
        /// The file, socket, or program involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The socket path would not fit in `sun_path`.
    #[error("socket path {} is {len} bytes; macOS allows at most {MAX_SOCKET_PATH_BYTES}", .path.display())]
    SocketPathTooLong {
        /// The refused path.
        path: PathBuf,
        /// Its length in bytes.
        len: usize,
    },
    /// The launch argv has no program.
    #[error("the launch argv is empty")]
    EmptyArgv,
    /// The helper could not be started.
    #[error("could not spawn {}", .program.display())]
    Spawn {
        /// The program the argv named.
        program: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The launch config could not be serialized.
    #[error("could not serialize the launch config")]
    Json(#[source] serde_json::Error),
    /// `codesign` wrote more than [`CODESIGN_OUTPUT_CAP`] bytes.
    #[error("codesign output exceeds {CODESIGN_OUTPUT_CAP} bytes")]
    CodesignOutput,
    /// The host deadline is too far in the future for the monotonic clock.
    #[error("the host deadline overflows the clock")]
    DeadlineOverflow,
}

/// Why guest bytes are not a valid status frame. Messages never include guest text.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// Fewer than [`FRAME_HEADER_BYTES`] bytes arrived.
    #[error("status frame is shorter than its {FRAME_HEADER_BYTES}-byte header")]
    Short,
    /// The frame does not start with [`FRAME_MAGIC`].
    #[error("status frame magic is {found:02x?}, expected MINOSST1")]
    BadMagic {
        /// The first eight bytes.
        found: [u8; 8],
    },
    /// The header names a version other than [`FRAME_VERSION`].
    #[error("status frame version {0} is not supported")]
    Version(u32),
    /// The header declares more payload than allowed.
    #[error("status frame declares {declared} payload bytes; at most {max} are allowed")]
    TooLarge {
        /// Length from the header.
        declared: u64,
        /// The limit applied.
        max: u64,
    },
    /// The connection hit its byte cap, so trailing bytes were dropped.
    #[error("status frame connection exceeded its byte cap")]
    Overflowed,
    /// The payload is not exactly the declared length.
    #[error("status frame declares {declared} payload bytes but {present} are present")]
    LengthMismatch {
        /// Length from the header.
        declared: u64,
        /// Payload bytes after the header.
        present: u64,
    },
    /// The payload is not JSON.
    #[error("status frame payload is not valid JSON")]
    Json(#[source] serde_json::Error),
    /// The payload is JSON but not an object whose `v` is 1.
    #[error("status frame payload is not a JSON object with v = 1")]
    NotV1Object,
}

/// Makes a fresh job directory `/private/tmp/mnsXXXXXXXX` with mode 0700. Dropping the [`TempDir`] removes it.
///
/// # Errors
///
/// [`SuperviseError::Io`] if the directory cannot be created.
pub fn short_job_dir() -> Result<TempDir, SuperviseError> {
    todo!()
}

/// The fixed file names of one job (E-5), resolved under a canonical job directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFiles {
    dir: PathBuf,
    root: PathBuf,
    input: PathBuf,
    scratch: PathBuf,
    socket: PathBuf,
    console: PathBuf,
    config: PathBuf,
    vmm_stdout: PathBuf,
    vmm_stderr: PathBuf,
    initramfs: PathBuf,
}

impl JobFiles {
    /// Canonicalizes `dir` and names the job's files in it.
    ///
    /// # Errors
    ///
    /// [`SuperviseError::Io`] if `dir` cannot be canonicalized, and [`SuperviseError::SocketPathTooLong`] if the
    /// socket path would exceed [`MAX_SOCKET_PATH_BYTES`].
    pub fn in_dir(dir: &Path) -> Result<Self, SuperviseError> {
        todo!()
    }

    /// The canonical job directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The read-only root image.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The read-only job input image.
    #[must_use]
    pub fn input(&self) -> &Path {
        &self.input
    }

    /// The scratch image. The host creates it and never reads it.
    #[must_use]
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }

    /// The vsock output socket.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The console log, which must not exist before the launch.
    #[must_use]
    pub fn console(&self) -> &Path {
        &self.console
    }

    /// The launch config.
    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// The helper's stdout.
    #[must_use]
    pub fn vmm_stdout(&self) -> &Path {
        &self.vmm_stdout
    }

    /// The helper's stderr.
    #[must_use]
    pub fn vmm_stderr(&self) -> &Path {
        &self.vmm_stderr
    }

    /// The initramfs, used only by the negative-evidence run.
    #[must_use]
    pub fn initramfs(&self) -> &Path {
        &self.initramfs
    }
}

/// Creates the root and input images with the given bytes and a sparse scratch image of `scratch_cap_bytes`.
///
/// # Errors
///
/// [`SuperviseError::Io`] if any file already exists or cannot be written.
pub fn write_disks(
    files: &JobFiles,
    root_image: &[u8],
    input_image: &[u8],
    scratch_cap_bytes: u64,
) -> Result<(), SuperviseError> {
    todo!()
}

/// Binds the vsock output listener on the job's socket path and makes it non-blocking.
///
/// # Errors
///
/// [`SuperviseError::Io`] if the socket cannot be bound or configured.
pub fn bind_output(files: &JobFiles) -> Result<UnixListener, SuperviseError> {
    todo!()
}

/// Creates `vmm.json` holding `config` as compact JSON.
///
/// # Errors
///
/// [`SuperviseError::Json`] if the config cannot be serialized, and [`SuperviseError::Io`] if the file exists or
/// cannot be written.
pub fn write_config(files: &JobFiles, config: &LaunchConfig) -> Result<(), SuperviseError> {
    todo!()
}

/// The lowercase hex SHA-256 of the file at `path`, read in chunks.
///
/// # Errors
///
/// [`SuperviseError::Io`] if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, SuperviseError> {
    todo!()
}

/// Whether the binary at `vmm` is signed with `com.apple.security.hypervisor` set to true.
///
/// Runs `/usr/bin/codesign --display --entitlements - --xml <vmm>` with an empty environment and reads at most
/// [`CODESIGN_OUTPUT_CAP`] bytes of its stdout. An unsigned binary yields `false`.
///
/// # Errors
///
/// [`SuperviseError::Io`] if `vmm` does not exist or codesign cannot be waited on, [`SuperviseError::Spawn`] if
/// codesign cannot start, and [`SuperviseError::CodesignOutput`] if it writes more than the cap.
pub fn has_hypervisor_entitlement(vmm: &Path) -> Result<bool, SuperviseError> {
    todo!()
}

/// Whether codesign's XML entitlements set the hypervisor key to true.
fn entitled(xml: &[u8]) -> bool {
    todo!()
}

/// One helper launch: the argv, the job's files, the pre-bound listener, and the host-side limits.
pub struct Launch<'a> {
    argv: Vec<OsString>,
    files: &'a JobFiles,
    listener: UnixListener,
    host_deadline: Duration,
    conn_caps: Vec<u64>,
    max_connections: usize,
    console_cap: u64,
}

impl<'a> Launch<'a> {
    /// A launch of `argv` (program first) for the job in `files`, collecting output from `listener`.
    ///
    /// `argv` is the full command: `[vmm, config]` for a plain launch, or a `sandbox-exec` prefix (02-09). It is
    /// passed to the program as separate arguments, never through a shell.
    #[must_use]
    pub fn new<I, S>(argv: I, files: &'a JobFiles, listener: UnixListener) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            files,
            listener,
            host_deadline: DEFAULT_HOST_DEADLINE,
            conn_caps: vec![DEFAULT_CONN_CAP],
            max_connections: DEFAULT_MAX_CONNECTIONS,
            console_cap: DEFAULT_CONSOLE_CAP,
        }
    }

    /// Kills the helper if it is still running this long after it was spawned.
    #[must_use]
    pub const fn host_deadline(mut self, deadline: Duration) -> Self {
        self.host_deadline = deadline;
        self
    }

    /// Byte caps for the first, second, ... connection; the last value repeats. An empty list means the default.
    #[must_use]
    pub fn conn_caps(mut self, caps: Vec<u64>) -> Self {
        self.conn_caps = caps;
        self
    }

    /// Stops accepting connections after this many.
    #[must_use]
    pub const fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Most console bytes read back.
    #[must_use]
    pub const fn console_cap(mut self, cap: u64) -> Self {
        self.console_cap = cap;
        self
    }

    fn conn_cap(&self, index: usize) -> u64 {
        self.conn_caps
            .get(index)
            .or_else(|| self.conn_caps.last())
            .copied()
            .unwrap_or(DEFAULT_CONN_CAP)
    }
}

/// Runs one helper to completion or to the host deadline and returns what it left behind.
///
/// While the helper runs, the loop accepts a pending connection and reads it through its cap plus one byte (the
/// read is bounded by the host deadline), else checks whether the helper exited, else checks the deadline, else
/// sleeps 10 ms. At the deadline the helper is killed and reaped. After it exits, connections already pending are
/// accepted once. The console, stdout, and stderr files are then read through their caps.
///
/// # Errors
///
/// [`SuperviseError::EmptyArgv`], [`SuperviseError::Spawn`], [`SuperviseError::DeadlineOverflow`], or
/// [`SuperviseError::Io`] for a host-side failure. The helper is killed and reaped on every error path.
pub fn run(launch: Launch<'_>) -> Result<VmRun, SuperviseError> {
    todo!()
}

/// What one helper launch left behind. Everything guest-influenced is a [`GuestBytes`].
pub struct VmRun {
    status: ExitStatus,
    killed_at_deadline: bool,
    elapsed: Duration,
    pid: u32,
    connections: Vec<GuestBytes>,
    console: GuestBytes,
    vmm_stdout: GuestBytes,
    vmm_stderr: GuestBytes,
}

impl VmRun {
    /// The helper's exit status.
    #[must_use]
    pub const fn status(&self) -> ExitStatus {
        self.status
    }

    /// The helper's exit code; `None` if a signal ended it.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.status.code()
    }

    /// The signal that ended the helper; `None` if it exited.
    #[must_use]
    pub fn signal(&self) -> Option<i32> {
        self.status.signal()
    }

    /// Whether the supervisor killed the helper at the host deadline.
    #[must_use]
    pub const fn killed_at_deadline(&self) -> bool {
        self.killed_at_deadline
    }

    /// Time from spawn to exit.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// The helper's process id (and process group id).
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// The vsock connections, in the order they were accepted.
    #[must_use]
    pub fn connections(&self) -> &[GuestBytes] {
        &self.connections
    }

    /// The console log.
    #[must_use]
    pub const fn console(&self) -> &GuestBytes {
        &self.console
    }

    /// The helper's stdout.
    #[must_use]
    pub const fn vmm_stdout(&self) -> &GuestBytes {
        &self.vmm_stdout
    }

    /// The helper's stderr.
    #[must_use]
    pub const fn vmm_stderr(&self) -> &GuestBytes {
        &self.vmm_stderr
    }
}

/// Capped bytes that came from the guest or the helper running it. Hostile input.
///
/// No `Debug`, `Display`, `Deref`, or `Clone`: the bytes leave only through [`GuestBytes::status_frame`] (a
/// validating decoder), [`GuestBytes::contains`] and [`GuestBytes::matches_pattern`] (comparisons),
/// [`GuestBytes::sha256_hex`], and [`GuestBytes::excerpt`] (an allowlist sanitizer for display).
pub struct GuestBytes {
    bytes: Vec<u8>,
    overflowed: bool,
}

impl GuestBytes {
    /// Wraps up to `cap + 1` bytes read through a cap: more than `cap` means overflow, and only `cap` are kept.
    fn capped(mut bytes: Vec<u8>, cap: u64) -> Self {
        let keep = usize::try_from(cap).unwrap_or(usize::MAX);
        let overflowed = bytes.len() > keep;
        bytes.truncate(keep);
        Self { bytes, overflowed }
    }

    /// Number of bytes kept, never more than the cap.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether no bytes arrived.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Whether more bytes arrived than the cap allowed.
    #[must_use]
    pub const fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Lowercase hex SHA-256 of the kept bytes.
    #[must_use]
    pub fn sha256_hex(&self) -> String {
        todo!()
    }

    /// Whether `needle` occurs in the kept bytes. An empty needle always does.
    #[must_use]
    pub fn contains(&self, needle: &[u8]) -> bool {
        todo!()
    }

    /// Whether every kept byte at index `i` equals `pattern(i)`.
    #[must_use]
    pub fn matches_pattern(&self, pattern: impl Fn(u64) -> u8) -> bool {
        todo!()
    }

    /// Up to `max_chars` characters safe to print to a terminal.
    ///
    /// An allowlist: printable characters, newline, and tab are kept. C0 controls (including ESC and carriage
    /// return), DEL, C1 controls, and the bidi overrides U+202A to U+202E and U+2066 to U+2069 are dropped, and
    /// invalid UTF-8 becomes U+FFFD. The count applies after filtering.
    #[must_use]
    pub fn excerpt(&self, max_chars: usize) -> String {
        todo!()
    }

    /// Decodes a `MINOSST1` status frame whose payload is at most `max_bytes` (and never more than
    /// [`MAX_FRAME_BYTES`]) into a JSON object whose `v` is 1.
    ///
    /// # Errors
    ///
    /// [`FrameError`] for a short header, a wrong magic or version, a declared length over the limit, an
    /// overflowed connection, a payload that is not exactly the declared length, invalid JSON, or JSON that is
    /// not an object with `v` = 1.
    pub fn status_frame(&self, max_bytes: u64) -> Result<serde_json::Value, FrameError> {
        todo!()
    }
}

/// The excerpt allowlist: newline, tab, and every character that is neither a control nor a bidi override.
fn displayable(character: char) -> bool {
    matches!(character, '\n' | '\t')
        || !(character.is_control()
            || matches!(character, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
}

/// Kills and reaps the child on drop, so no early return leaves a helper running. Once the child has been
/// reaped, std's `kill` returns `Ok` without signalling, so a reused pid is never hit.
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

/// Accepts one pending connection, or returns `None` if none is pending.
fn accept_pending(
    listener: &UnixListener,
    socket: &Path,
) -> Result<Option<UnixStream>, SuperviseError> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(Some(stream)),
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(None),
            // A peer that reset before the accept is the guest's doing, not a host failure.
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::Interrupted | ErrorKind::ConnectionAborted
                ) => {}
            Err(source) => return Err(io_at("accept on", socket)(source)),
        }
    }
}

/// Reads one connection through `cap + 1` bytes, bounded in total by the later of `deadline` and a short window
/// from now. A peer error or a timeout ends the connection with the bytes already read; the stream is dropped
/// by the caller, so a guest still writing past the cap gets an error.
fn read_connection(
    stream: &UnixStream,
    cap: u64,
    deadline: Instant,
    socket: &Path,
) -> Result<GuestBytes, SuperviseError> {
    // On macOS an accepted socket inherits the listener's O_NONBLOCK.
    stream
        .set_nonblocking(false)
        .map_err(io_at("make blocking a connection on", socket))?;
    let now = Instant::now();
    let read_until = now
        .checked_add(MIN_READ_WINDOW)
        .map_or(deadline, |window| window.max(deadline));
    let mut reader = stream.take(cap.saturating_add(1));
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
    loop {
        let remaining = read_until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(io_at("set a read timeout on a connection on", socket))?;
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                if let Some(part) = chunk.get(..read) {
                    bytes.extend_from_slice(part);
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    Ok(GuestBytes::capped(bytes, cap))
}

/// Reads a helper-written file through `cap + 1` bytes; a file the helper never created reads as empty.
fn read_capped_file(path: &Path, cap: u64) -> Result<GuestBytes, SuperviseError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(GuestBytes::capped(Vec::new(), cap));
        }
        Err(source) => return Err(io_at("open", path)(source)),
    };
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_at("read", path))?;
    Ok(GuestBytes::capped(bytes, cap))
}

/// Creates `path` (it must not exist) and writes `bytes` to it.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), SuperviseError> {
    let mut file = File::create_new(path).map_err(io_at("create", path))?;
    file.write_all(bytes).map_err(io_at("write", path))
}

/// Builds the error-mapping closure for an I/O failure on `path`.
fn io_at(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> SuperviseError {
    let path = path.to_path_buf();
    move |source| SuperviseError::Io {
        action,
        path,
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::io::{self, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::{
        FrameError, GuestBytes, JobFiles, Launch, MAX_FRAME_BYTES, MAX_SOCKET_PATH_BYTES,
        SuperviseError, VmRun, bind_output, entitled, has_hypervisor_entitlement, run, sha256_file,
        short_job_dir, write_config, write_disks,
    };
    use crate::launch_config::{Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock};

    type TestResult = Result<(), Box<dyn Error>>;

    /// 02-05-PLAN.md Exact texts E-3 (02-01 E-10): the header of the golden frame, then its 35-byte payload.
    const E3_HEADER_HEX: &str = "4d494e4f53535431000000010000000000000023";
    const E3_PAYLOAD: &[u8] = br#"{"v":1,"steps":[],"job_error":null}"#;

    const GENEROUS: Duration = Duration::from_secs(10);

    fn hex(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        text.as_bytes()
            .chunks(2)
            .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
            .collect()
    }

    fn golden_frame() -> Result<Vec<u8>, Box<dyn Error>> {
        let mut frame = hex(E3_HEADER_HEX)?;
        frame.extend_from_slice(E3_PAYLOAD);
        Ok(frame)
    }

    fn frame(magic: &[u8; 8], version: u32, declared: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = magic.to_vec();
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&declared.to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn guest(bytes: &[u8]) -> GuestBytes {
        GuestBytes::capped(bytes.to_vec(), u64::MAX)
    }

    fn prepared() -> Result<(TempDir, JobFiles, UnixListener), Box<dyn Error>> {
        let dir = short_job_dir()?;
        let files = JobFiles::in_dir(dir.path())?;
        let listener = bind_output(&files)?;
        Ok((dir, files, listener))
    }

    fn write_then_close(socket: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut stream = UnixStream::connect(socket)?;
        stream.write_all(bytes)
    }

    fn first(vm: &VmRun) -> Result<&GuestBytes, Box<dyn Error>> {
        Ok(vm
            .connections()
            .first()
            .ok_or("no connection was collected")?)
    }

    #[test]
    fn supervise_short_job_dir_is_private_canonical_and_short() -> TestResult {
        let dir = short_job_dir()?;
        let path = dir.path();
        assert!(path.starts_with("/private/tmp"));
        assert_eq!(fs::canonicalize(path)?, path);
        assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o700);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("no name")?;
        assert!(name.starts_with("mns"));
        assert_eq!(name.len(), 11);
        let files = JobFiles::in_dir(path)?;
        assert_eq!(files.socket(), path.join("o.sock"));
        assert!(files.socket().as_os_str().len() <= MAX_SOCKET_PATH_BYTES);
        for (actual, name) in [
            (files.root(), "root.img"),
            (files.input(), "input.img"),
            (files.scratch(), "scratch.img"),
            (files.console(), "console.log"),
            (files.config(), "vmm.json"),
            (files.vmm_stdout(), "vmm.stdout"),
            (files.vmm_stderr(), "vmm.stderr"),
            (files.initramfs(), "initramfs.cpio"),
        ] {
            assert_eq!(actual, path.join(name));
        }
        Ok(())
    }

    #[test]
    fn supervise_job_files_refuse_a_socket_path_over_103_bytes() -> TestResult {
        let dir = short_job_dir()?;
        let long = dir.path().join("a".repeat(80));
        fs::create_dir(&long)?;
        assert!(matches!(
            JobFiles::in_dir(&long),
            Err(SuperviseError::SocketPathTooLong { len, .. }) if len > 103
        ));
        Ok(())
    }

    #[test]
    fn supervise_write_disks_creates_new_files_and_a_sparse_scratch() -> TestResult {
        let dir = short_job_dir()?;
        let files = JobFiles::in_dir(dir.path())?;
        write_disks(&files, b"root", b"input", 1_048_576)?;
        assert_eq!(fs::read(files.root())?, b"root");
        assert_eq!(fs::read(files.input())?, b"input");
        assert_eq!(fs::metadata(files.scratch())?.len(), 1_048_576);
        assert!(matches!(
            write_disks(&files, b"other", b"other", 1),
            Err(SuperviseError::Io { source, .. }) if source.kind() == io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read(files.root())?, b"root");
        Ok(())
    }

    #[test]
    fn supervise_write_config_writes_the_serialized_launch_config_once() -> TestResult {
        let dir = short_job_dir()?;
        let files = JobFiles::in_dir(dir.path())?;
        let config = LaunchConfig::new(
            Kernel::new("/k", "00"),
            Boot::RootDisk,
            Disks::new(files.root(), files.input(), files.scratch(), 1),
            Vsock::new(1024, files.socket()),
            Console::new(files.console(), 1),
            1,
            Machine::new(1, 256),
        );
        write_config(&files, &config)?;
        assert_eq!(fs::read_to_string(files.config())?, config.to_json()?);
        assert!(write_config(&files, &config).is_err());
        Ok(())
    }

    #[test]
    fn supervise_sha256_file_matches_the_abc_vector() -> TestResult {
        let dir = short_job_dir()?;
        let path = dir.path().join("abc");
        fs::write(&path, b"abc")?;
        assert_eq!(
            sha256_file(&path)?,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            guest(b"abc").sha256_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        Ok(())
    }

    #[test]
    fn supervise_entitlement_check_reads_codesign_xml() -> TestResult {
        let key = "<key>com.apple.security.hypervisor</key>";
        assert!(entitled(format!("<dict>{key}<true/></dict>").as_bytes()));
        assert!(!entitled(format!("<dict>{key}<false/></dict>").as_bytes()));
        assert!(!entitled(
            b"<dict><key>com.apple.security.hypervisor.extra</key><true/></dict>"
        ));
        assert!(!entitled(b""));
        // The test binary is linker-signed with no entitlements.
        assert!(!has_hypervisor_entitlement(&std::env::current_exe()?)?);
        assert!(has_hypervisor_entitlement(Path::new("/private/tmp/minos-no-such-vmm")).is_err());
        Ok(())
    }

    #[test]
    fn supervise_run_true_exits_zero_with_no_connections() -> TestResult {
        let (_dir, files, listener) = prepared()?;
        let vm = run(Launch::new(["/usr/bin/true"], &files, listener).host_deadline(GENEROUS))?;
        assert_eq!(vm.exit_code(), Some(0));
        assert_eq!(vm.signal(), None);
        assert!(vm.status().success());
        assert!(!vm.killed_at_deadline());
        assert!(vm.connections().is_empty());
        assert!(vm.console().is_empty());
        assert!(vm.vmm_stdout().is_empty());
        assert!(vm.vmm_stderr().is_empty());
        assert!(vm.pid() > 0);
        Ok(())
    }

    #[test]
    fn supervise_run_kills_the_helper_at_the_host_deadline() -> TestResult {
        let (_dir, files, listener) = prepared()?;
        let vm = run(Launch::new(["/bin/sleep", "5"], &files, listener)
            .host_deadline(Duration::from_millis(200)))?;
        assert!(vm.killed_at_deadline());
        assert_eq!(vm.exit_code(), None);
        assert_eq!(vm.signal(), Some(9));
        assert!(vm.elapsed() >= Duration::from_millis(200));
        assert!(vm.elapsed() < Duration::from_secs(2));
        Ok(())
    }

    #[test]
    fn supervise_run_clears_the_environment() -> TestResult {
        assert!(
            std::env::vars_os().next().is_some(),
            "the test process has an environment to leak"
        );
        let (_dir, files, listener) = prepared()?;
        let vm = run(Launch::new(["/usr/bin/env"], &files, listener).host_deadline(GENEROUS))?;
        assert_eq!(vm.exit_code(), Some(0));
        assert!(vm.vmm_stdout().is_empty());
        Ok(())
    }

    #[test]
    fn supervise_run_captures_stdout_and_gives_stdin_eof() -> TestResult {
        let (_dir, files, listener) = prepared()?;
        let vm =
            run(Launch::new(["/bin/echo", "hello"], &files, listener).host_deadline(GENEROUS))?;
        assert_eq!(vm.vmm_stdout().len(), 6);
        assert!(vm.vmm_stdout().contains(b"hello\n"));
        let (_dir, files, listener) = prepared()?;
        let vm = run(Launch::new(["/bin/cat"], &files, listener).host_deadline(GENEROUS))?;
        assert!(
            !vm.killed_at_deadline(),
            "cat waited on stdin instead of reading /dev/null"
        );
        assert_eq!(vm.exit_code(), Some(0));
        assert!(vm.vmm_stdout().is_empty());
        Ok(())
    }

    #[test]
    fn supervise_run_refuses_an_empty_argv() -> TestResult {
        let (_dir, files, listener) = prepared()?;
        let empty: [&str; 0] = [];
        assert!(matches!(
            run(Launch::new(empty, &files, listener)),
            Err(SuperviseError::EmptyArgv)
        ));
        Ok(())
    }

    #[test]
    fn supervise_run_flags_a_connection_over_its_cap() -> TestResult {
        let cap = 4096_usize;
        let payload = vec![0xA5_u8; cap + 10];
        let (_dir, files, listener) = prepared()?;
        let (vm, written) = thread::scope(|scope| {
            let writer = scope.spawn(|| write_then_close(files.socket(), &payload).is_ok());
            let vm = run(Launch::new(["/bin/sleep", "1"], &files, listener)
                .conn_caps(vec![u64::try_from(cap).unwrap_or(u64::MAX)])
                .host_deadline(GENEROUS));
            (vm, writer.join())
        });
        let vm = vm?;
        let _write_completed: bool = written.map_err(|_| "writer thread panicked")?;
        assert_eq!(vm.connections().len(), 1);
        let conn = first(&vm)?;
        assert_eq!(conn.len(), cap);
        assert!(conn.overflowed());
        assert!(conn.matches_pattern(|_| 0xA5));
        assert!(!vm.killed_at_deadline());
        Ok(())
    }

    #[test]
    fn supervise_run_keeps_a_connection_of_exactly_its_cap() -> TestResult {
        let cap = 4096_usize;
        let payload: Vec<u8> = (0..cap)
            .map(|index| u8::try_from(index % 251).unwrap_or(0))
            .collect();
        let (_dir, files, listener) = prepared()?;
        let (vm, written) = thread::scope(|scope| {
            let writer = scope.spawn(|| write_then_close(files.socket(), &payload));
            let vm = run(Launch::new(["/bin/sleep", "1"], &files, listener)
                .conn_caps(vec![u64::try_from(cap)?])
                .host_deadline(GENEROUS));
            Ok::<_, Box<dyn Error>>((vm, writer.join()))
        })?;
        let vm = vm?;
        written.map_err(|_| "writer thread panicked")??;
        let conn = first(&vm)?;
        assert_eq!(conn.len(), cap);
        assert!(!conn.overflowed());
        assert!(conn.matches_pattern(|index| u8::try_from(index % 251).unwrap_or(0)));
        assert!(!conn.matches_pattern(|_| 0));
        Ok(())
    }

    #[test]
    fn supervise_run_applies_caps_per_connection_and_stops_at_the_maximum() -> TestResult {
        let (_dir, files, listener) = prepared()?;
        let (vm, written) = thread::scope(|scope| {
            let writer = scope.spawn(|| -> io::Result<()> {
                for _ in 0..3 {
                    write_then_close(files.socket(), b"0123456789")?;
                }
                Ok(())
            });
            let vm = run(Launch::new(["/bin/sleep", "1"], &files, listener)
                .conn_caps(vec![4, 8])
                .max_connections(2)
                .host_deadline(GENEROUS));
            (vm, writer.join())
        });
        let vm = vm?;
        written.map_err(|_| "writer thread panicked")??;
        let lens: Vec<(usize, bool)> = vm
            .connections()
            .iter()
            .map(|conn| (conn.len(), conn.overflowed()))
            .collect();
        assert_eq!(lens, [(4, true), (8, true)]);
        Ok(())
    }

    #[test]
    fn supervise_status_frame_decodes_the_e3_golden_frame() -> TestResult {
        let value = guest(&golden_frame()?).status_frame(MAX_FRAME_BYTES)?;
        assert_eq!(value.get("v").and_then(serde_json::Value::as_u64), Some(1));
        assert_eq!(
            value
                .get("steps")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn supervise_status_frame_rejects_a_bad_header() -> TestResult {
        let golden = golden_frame()?;
        let short = golden.get(..19).ok_or("short golden")?;
        assert!(matches!(
            guest(short).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::Short)
        ));
        assert!(matches!(
            guest(&frame(b"MINOSSTX", 1, 35, E3_PAYLOAD)).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::BadMagic { found }) if found == *b"MINOSSTX"
        ));
        assert!(matches!(
            guest(&frame(b"MINOSST1", 2, 35, E3_PAYLOAD)).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::Version(2))
        ));
        Ok(())
    }

    #[test]
    fn supervise_status_frame_rejects_a_declared_length_over_the_limit() -> TestResult {
        assert!(matches!(
            guest(&frame(b"MINOSST1", 1, 1_048_577, b"")).status_frame(u64::MAX),
            Err(FrameError::TooLarge {
                declared: 1_048_577,
                max: 1_048_576
            })
        ));
        assert!(matches!(
            guest(&golden_frame()?).status_frame(34),
            Err(FrameError::TooLarge {
                declared: 35,
                max: 34
            })
        ));
        Ok(())
    }

    #[test]
    fn supervise_status_frame_rejects_a_length_mismatch_or_overflow() -> TestResult {
        let mut long = golden_frame()?;
        long.push(b' ');
        assert!(matches!(
            guest(&long).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::LengthMismatch {
                declared: 35,
                present: 36
            })
        ));
        let golden = golden_frame()?;
        let truncated = golden.get(..golden.len() - 1).ok_or("short golden")?;
        assert!(matches!(
            guest(truncated).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::LengthMismatch {
                declared: 35,
                present: 34
            })
        ));
        let overflowed = GuestBytes::capped(long, u64::try_from(golden.len())?);
        assert!(overflowed.overflowed());
        assert!(matches!(
            overflowed.status_frame(MAX_FRAME_BYTES),
            Err(FrameError::Overflowed)
        ));
        Ok(())
    }

    #[test]
    fn supervise_status_frame_rejects_invalid_json_and_non_v1_objects() {
        assert!(matches!(
            guest(&frame(b"MINOSST1", 1, 3, b"{x}")).status_frame(MAX_FRAME_BYTES),
            Err(FrameError::Json(_))
        ));
        for payload in [
            &b"[]"[..],
            br#"{"v":2}"#,
            br#"{"steps":[]}"#,
            b"1",
            br#"{"v":"1"}"#,
        ] {
            let declared = u64::try_from(payload.len()).unwrap_or(u64::MAX);
            assert!(matches!(
                guest(&frame(b"MINOSST1", 1, declared, payload)).status_frame(MAX_FRAME_BYTES),
                Err(FrameError::NotV1Object)
            ));
        }
    }

    #[test]
    fn supervise_excerpt_strips_controls_and_bidi_overrides() {
        let mut text = "a\u{1b}[31mb\tc\nd\re\u{7f}\u{85}f\u{202a}\u{202e}g\u{2066}\u{2069}h\u{0}i"
            .as_bytes()
            .to_vec();
        text.push(0xff);
        assert_eq!(guest(&text).excerpt(100), "a[31mb\tc\ndefghi\u{fffd}");
    }

    #[test]
    fn supervise_excerpt_truncates_after_filtering() {
        assert_eq!(guest(b"ab\x1bcdef").excerpt(3), "abc");
        assert_eq!(guest(b"abc").excerpt(0), "");
    }

    #[test]
    fn supervise_guest_bytes_comparisons() {
        let bytes = guest(b"MINOS-GUEST-BOOT-OK\n");
        assert!(bytes.contains(b"BOOT-OK"));
        assert!(bytes.contains(b""));
        assert!(!bytes.contains(b"JOB-ERROR"));
        assert!(!guest(b"ab").contains(b"abc"));
        let capped = GuestBytes::capped(b"abcd".to_vec(), 4);
        assert!(!capped.overflowed());
        assert_eq!(capped.len(), 4);
        let over = GuestBytes::capped(b"abcde".to_vec(), 4);
        assert!(over.overflowed());
        assert_eq!(over.len(), 4);
        assert!(!over.contains(b"e"));
    }
}
