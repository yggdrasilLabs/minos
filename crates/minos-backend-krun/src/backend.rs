//! `KrunBackend`, the libkrun [`Backend`] (SBX-06; 05-CONTEXT D-13 to D-15).
//!
//! A launch, in order, and who owns each byte:
//!
//! 1. [`fetch_parts`] fetches every blob the plan names through the backend's [`BlobSource`], under its section
//!    cap, and checks the guest job against the plan (D-13). The runner passes IDs only.
//! 2. The root image is the store's verified, canonical path, named under a read-only raw disk with no copy per
//!    job (D-14; 05-EVIDENCE.md Mechanism selections, `root image: store-path`).
//! 3. A fresh 0700 job directory under the user's job root takes its owner lock, and the input disk is written
//!    into it with a fresh output token from `/dev/urandom` (D-15).
//! 4. `vmm.json` (launch config v2) is written new, the vsock listener is bound, and the chunk receiver starts on
//!    it, so the guest's first connection is never refused.
//! 5. The helper is spawned through `/usr/bin/sandbox-exec -p` with the embedded profile over the job's canonical
//!    paths, as `<vmm> <job dir>/vmm.json`, the exact argv the sweep matches; its pid goes into the job directory.
//! 6. [`KrunRunning::wait`] watches the helper until it exits, killing its process group at the host deadline
//!    (the plan's hard maximum plus [`HOST_DEADLINE_GRACE`]), at the console cap, or on a chunk fault, then
//!    classifies the ending with [`classify`] and returns a [`Finished`]. The job directory, with its input disk,
//!    is deleted when the running value is consumed or dropped.
//!
//! Every byte from the guest stays hostile: the reassembled stream goes into the `Finished` unread, and the
//! console bytes leave it only as a sanitized excerpt.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::Instant;

use minos_core::guest::OUTPUT_VSOCK_PORT;
use minos_core::ledger::ConfinementLayer;
use minos_core::limits::MAX_OUTPUT_STREAM_BYTES;
use minos_sandbox::{
    Backend, BlobSource, Capabilities, Cleared, Finished, InfraError, LaunchError, Role, RunningVm,
};

use crate::chunks::{CONNECTION_DEADLINE, ChunkReceiver};
use crate::host::{HostPaths, KRUNFW_LINK, KRUNFW_SHA256, probe_host};
use crate::input_disk::{fetch_parts, new_token, write_input_disk};
use crate::jobdir::{JobDir, JobDirError, JobRoot, SweepReport, sweep};
use crate::launch_config::{Disks, Kernel, LaunchConfig, Machine, Vsock};
use crate::seatbelt::{self, ProfileParams};
use crate::supervise::{
    Classification, HOST_DEADLINE_GRACE, Helper, Observed, SuperviseError, classify, watch,
};

/// The console cap: 1 MiB. Boot lines and PID 1's own lines are a few KiB; the cap stops a guest flooding the
/// console, and the supervisor kills the VM at one byte past it.
pub const CONSOLE_CAP_BYTES: u64 = 1_048_576;

/// The libkrun backend: the host paths, a [`BlobSource`], the user's job root, and the capabilities probed once.
pub struct KrunBackend<B: BlobSource> {
    paths: HostPaths,
    blobs: B,
    root: JobRoot,
    caps: Capabilities,
    sweep: SweepReport,
    console_cap: u64,
}

impl<B: BlobSource> KrunBackend<B> {
    /// Opens the job root under `/private/tmp`, sweeps it for helpers a dead supervisor left behind, and probes the
    /// host once ([`probe_host`]); the capabilities are kept for every launch.
    ///
    /// # Errors
    ///
    /// [`LaunchError::Io`] or [`LaunchError::Backend`] when the job root cannot be opened or is unsafe.
    pub fn new(paths: HostPaths, blobs: B) -> Result<Self, LaunchError> {
        let root = JobRoot::open().map_err(job_error("open the job root"))?;
        Self::with_job_root(paths, blobs, root)
    }

    /// As [`KrunBackend::new`], over the job root `root`: the VM and exit tests keep off the shared
    /// `/private/tmp/minos-<uid>` root with this. The host probes run in the directory that holds `root`.
    ///
    /// # Errors
    ///
    /// None today; the signature matches [`KrunBackend::new`] so a probe failure can become one.
    pub fn with_job_root(paths: HostPaths, blobs: B, root: JobRoot) -> Result<Self, LaunchError> {
        let sweep = sweep(&root, paths.vmm());
        let parent = root.path().parent().unwrap_or_else(|| root.path());
        let caps = probe_host(&paths, parent).into_capabilities();
        Ok(Self {
            paths,
            blobs,
            root,
            caps,
            sweep,
            console_cap: CONSOLE_CAP_BYTES,
        })
    }

    /// What the start-up sweep did with each job directory it found.
    #[must_use]
    pub const fn sweep_report(&self) -> &SweepReport {
        &self.sweep
    }

    /// The host paths this backend launches with.
    #[must_use]
    pub const fn paths(&self) -> &HostPaths {
        &self.paths
    }

    /// Refuses a launch on a host whose own probes failed, whatever capabilities the plan was cleared against.
    fn check_host(&self) -> Result<(), LaunchError> {
        if !self.caps.host_refusals().is_empty() {
            return Err(LaunchError::Backend {
                what: "the host failed its launch checks; run minos host doctor",
            });
        }
        if !self.caps.layers().contains(&ConfinementLayer::Seatbelt) {
            return Err(LaunchError::Backend {
                what: "the Seatbelt layer is not enforcing; run minos host doctor",
            });
        }
        Ok(())
    }
}

impl<B: BlobSource> Backend for KrunBackend<B> {
    type Running<R: Role> = KrunRunning<R>;

    fn capabilities(&self) -> Capabilities {
        self.caps.clone()
    }

    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError> {
        self.check_host()?;
        let plan = ok.plan();
        let parts = fetch_parts(&self.blobs, plan)?;
        let root_image = self.blobs.root_image(plan.root())?;
        let dir = self
            .root
            .create_job()
            .map_err(job_error("create the job directory"))?;
        let token = new_token().map_err(io_error("draw the output token"))?;
        let input = write_input_disk(&dir, &token, &parts)?;
        drop(parts);
        let config = LaunchConfig::new(
            Kernel::new(self.paths.krunfw(), KRUNFW_SHA256),
            Disks::new(root_image.as_path(), &input),
            Vsock::new(OUTPUT_VSOCK_PORT, dir.socket()),
            plan.max_duration_ms(),
            Machine::new(
                u32::from(plan.machine().vcpus()),
                u32::from(plan.machine().memory_mib()),
            ),
        );
        write_config(&dir, &config)?;
        let listener =
            UnixListener::bind(dir.socket()).map_err(io_error("bind the output socket"))?;
        let receiver = ChunkReceiver::spawn(
            listener,
            token,
            MAX_OUTPUT_STREAM_BYTES,
            CONNECTION_DEADLINE,
        )
        .map_err(io_error("start the chunk receiver"))?;
        let vmm = self.paths.vmm();
        let config_path = dir.config();
        let params = ProfileParams::new(
            vmm,
            &config_path,
            self.paths.krunfw(),
            Path::new(KRUNFW_LINK),
            root_image.as_path(),
            &input,
            &dir.socket(),
        )
        .map_err(|_| LaunchError::Backend {
            what: "a Seatbelt parameter is not an absolute canonical path",
        })?;
        let argv = seatbelt::argv(&params, vmm, &[config_path.as_os_str()]);
        let lifetime = plan
            .max_duration()
            .checked_add(HOST_DEADLINE_GRACE)
            .ok_or(DEADLINE_OVERFLOW)?;
        let started = Instant::now();
        let deadline = started.checked_add(lifetime).ok_or(DEADLINE_OVERFLOW)?;
        let helper = Helper::spawn(&argv, self.console_cap).map_err(spawn_error)?;
        dir.write_pid(helper.pid())
            .map_err(job_error("record the helper's pid"))?;
        Ok(KrunRunning {
            helper,
            receiver,
            dir,
            cleared: ok,
            started,
            deadline,
        })
    }
}

/// One running krun VM: the helper, its chunk receiver, and its job directory.
///
/// Fields drop in declaration order, so dropping the value kills the helper's process group first, then stops
/// the receiver, then deletes the job directory with its input disk.
pub struct KrunRunning<R: Role> {
    helper: Helper,
    receiver: ChunkReceiver,
    dir: JobDir,
    cleared: Cleared<R>,
    started: Instant,
    deadline: Instant,
}

impl<R: Role> KrunRunning<R> {
    /// The helper's pid, which is also its process group id. For diagnostics and the exit tests.
    #[must_use]
    pub const fn helper_pid(&self) -> u32 {
        self.helper.pid()
    }

    /// The job directory. For diagnostics and the exit tests.
    #[must_use]
    pub fn job_dir(&self) -> &Path {
        self.dir.path()
    }
}

impl<R: Role> RunningVm<R> for KrunRunning<R> {
    fn wait(self) -> Result<Finished<R>, InfraError> {
        let Self {
            helper,
            receiver,
            dir,
            cleared,
            started,
            deadline,
        } = self;
        let killed_for =
            watch(&helper, deadline, || receiver.faulted()).map_err(supervise_error)?;
        let elapsed = started.elapsed();
        let end = helper.finish().map_err(supervise_error)?;
        let chunks = receiver
            .stop_and_join()
            .map_err(|source| InfraError::Io {
                op: "receive the output stream",
                source,
            })?
            .into_outcome();
        let (exit, console, stderr_lines) = end.into_parts();
        let result = match classify(Observed::new(exit, stderr_lines, killed_for, chunks)) {
            Classification::Infra(what) => Err(InfraError::Backend { what }),
            Classification::Ended(ended, stream) => {
                Ok(Finished::new(cleared, ended, stream, elapsed, console))
            }
        };
        drop(dir);
        result
    }
}

/// The refusal for a deadline the monotonic clock cannot hold.
const DEADLINE_OVERFLOW: LaunchError = LaunchError::Backend {
    what: "the host deadline overflows the clock",
};

/// Writes `config` as the job's new `vmm.json`, mode 0600. `create_new` refuses an existing file or symlink.
fn write_config(dir: &JobDir, config: &LaunchConfig) -> Result<(), LaunchError> {
    let json = config.to_json().map_err(|_| LaunchError::Backend {
        what: "the launch config could not be serialized",
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir.config())
        .map_err(io_error("create the launch config"))?;
    file.write_all(json.as_bytes())
        .map_err(io_error("write the launch config"))
}

/// Maps a host I/O failure during a launch.
fn io_error(op: &'static str) -> impl FnOnce(io::Error) -> LaunchError {
    move |source| LaunchError::Io { op, source }
}

/// Maps a job directory failure during a launch: an I/O error stays one; an unsafe job root is the backend's.
fn job_error(op: &'static str) -> impl FnOnce(JobDirError) -> LaunchError {
    move |error| match error {
        JobDirError::Io { source, .. } => LaunchError::Io { op, source },
        _ => LaunchError::Backend {
            what: "the job root is unsafe; run minos host doctor",
        },
    }
}

/// Maps a failure to start the helper.
fn spawn_error(error: SuperviseError) -> LaunchError {
    match error {
        SuperviseError::Spawn { source, .. } => LaunchError::Io {
            op: "spawn the helper",
            source,
        },
        SuperviseError::Host { action, source } | SuperviseError::Io { action, source, .. } => {
            LaunchError::Io { op: action, source }
        }
        _ => LaunchError::Backend {
            what: "the helper argv was refused",
        },
    }
}

/// Maps a failure to supervise a running helper.
fn supervise_error(error: SuperviseError) -> InfraError {
    match error {
        SuperviseError::Host { action, source } | SuperviseError::Io { action, source, .. } => {
            InfraError::Io { op: action, source }
        }
        SuperviseError::Survived => InfraError::Backend {
            what: "the helper survived SIGKILL",
        },
        SuperviseError::PipeOpen => InfraError::Backend {
            what: "a helper pipe stayed open after the helper exited",
        },
        _ => InfraError::Backend {
            what: "the supervisor failed",
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::error::Error;
    use std::fs;
    use std::path::Path;

    use minos_core::guest::GuestJob;
    use minos_core::id::{BlobId, ImageDigest, TreeId};
    use minos_core::job::{JobBudget, JobRequest};
    use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
    use minos_core::manifest::{Arch, JudgeVersion, Manifest};
    use minos_sandbox::spec::MAX_LAUNCH_MS;
    use minos_sandbox::{
        Backend, BlobSource, BlobSourceError, Candidate, Capabilities, InputDisk, LaunchError,
        Machine, SandboxSpec, VerifiedPath, preflight,
    };

    use super::{CONSOLE_CAP_BYTES, KrunBackend};
    use crate::host::HostPaths;
    use crate::jobdir::{JOB_PREFIX, JobRoot, sweep};

    type TestResult = Result<(), Box<dyn Error>>;

    /// The `minos.toml` example in CLAUDE.md, section "Projects".
    const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

    /// The base tarball's bytes. The backend never parses them, so any bytes serve.
    const BASE: &[u8] = b"base tarball bytes";

    /// A [`BlobSource`] over an in-memory map that keeps the trait's contract, with no root image.
    #[derive(Default)]
    struct MapBlobSource(BTreeMap<BlobId, Vec<u8>>);

    impl MapBlobSource {
        fn put(&mut self, bytes: &[u8]) {
            self.0.insert(BlobId::of(bytes), bytes.to_vec());
        }
    }

    impl BlobSource for MapBlobSource {
        fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, BlobSourceError> {
            let bytes = self.0.get(id).ok_or(BlobSourceError::Missing { id: *id })?;
            let size =
                u64::try_from(bytes.len()).map_err(|_| BlobSourceError::Unsafe { what: "size" })?;
            if size > max {
                return Err(BlobSourceError::TooLarge { id: *id, size, max });
            }
            Ok(bytes.clone())
        }

        fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
            Err(BlobSourceError::RootImageMissing { digest: *digest })
        }
    }

    fn manifest() -> Result<Manifest, Box<dyn Error>> {
        Ok(Manifest::parse_toml(MANIFEST)?)
    }

    /// The baseline request of trial `trial` over [`BASE`].
    fn request(trial: u32) -> Result<JobRequest, Box<dyn Error>> {
        let manifest = manifest()?;
        Ok(JobRequest::baseline(
            TreeId::of(BASE),
            JudgeVersion::of(&TreeId::of(b"judge"), manifest.judge()),
            trial,
            JobBudget::from_budget(manifest.budget()),
        )?)
    }

    fn job_bytes(trial: u32) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(GuestJob::candidate(&manifest()?, &request(trial)?).to_json()?)
    }

    /// A backend over `blobs` and the job root `root`, with capabilities that clear the test plan and no probes
    /// run: the fields are set directly, so no helper or host tool is involved.
    fn backend(
        blobs: MapBlobSource,
        root: JobRoot,
    ) -> Result<KrunBackend<MapBlobSource>, Box<dyn Error>> {
        let caps = Capabilities::new(
            BackendKind::Krun,
            Arch::Arm64,
            Machine::new(8, 16_384)?,
            MAX_LAUNCH_MS,
            Isolation::Hardware,
            BTreeSet::new(),
        )
        .with_layers(BTreeSet::from([
            ConfinementLayer::Seatbelt,
            ConfinementLayer::Rlimits,
        ]));
        // The root is a fresh empty directory, so the sweep finds nothing and signals nothing.
        let sweep = sweep(&root, Path::new("/nonexistent/minos-vmm"));
        Ok(KrunBackend {
            paths: HostPaths::new(
                "/nonexistent/minos-vmm",
                "/nonexistent/minos-guest",
                "/nonexistent/libkrunfw",
            ),
            blobs,
            root,
            caps,
            sweep,
            console_cap: CONSOLE_CAP_BYTES,
        })
    }

    /// Launches the baseline plan of trial 1 whose job blob is `plan_job`, over `blobs`, and returns the error
    /// and the job directories left in the root.
    fn launch_error(
        blobs: MapBlobSource,
        plan_job: &[u8],
    ) -> Result<(LaunchError, usize), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        let root_path = root.path().to_path_buf();
        let backend = backend(blobs, root)?;
        let request = request(1)?;
        let input = InputDisk::candidate(BlobId::of(plan_job), request.base(), request.diff());
        let plan = SandboxSpec::<Candidate>::new(
            &request,
            ImageDigest::of(b"root image"),
            input,
            Machine::new(2, manifest()?.environment().memory_mib())?,
            Arch::Arm64,
        )?
        .seal();
        let cleared =
            preflight(&backend.capabilities(), plan).map_err(|refusals| format!("{refusals:?}"))?;
        let Err(error) = backend.launch(cleared) else {
            return Err("the launch succeeded".into());
        };
        let jobs = fs::read_dir(&root_path)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(JOB_PREFIX))
            .count();
        Ok((error, jobs))
    }

    #[test]
    fn launch_refuses_a_missing_job_blob_before_any_job_directory() -> TestResult {
        let mut blobs = MapBlobSource::default();
        blobs.put(BASE);
        let (error, jobs) = launch_error(blobs, &job_bytes(1)?)?;
        assert!(
            matches!(error, LaunchError::Blob(BlobSourceError::Missing { .. })),
            "{error:?}"
        );
        assert_eq!(jobs, 0);
        Ok(())
    }

    #[test]
    fn launch_refuses_a_job_that_does_not_match_the_plan() -> TestResult {
        // The job blob is a valid guest job for trial 2, named by a plan for trial 1.
        let job = job_bytes(2)?;
        let mut blobs = MapBlobSource::default();
        blobs.put(&job);
        blobs.put(BASE);
        let (error, jobs) = launch_error(blobs, &job)?;
        assert!(
            matches!(
                error,
                LaunchError::Backend {
                    what: "the guest job does not match the plan"
                }
            ),
            "{error:?}"
        );
        assert_eq!(jobs, 0);
        Ok(())
    }

    #[test]
    fn launch_refuses_a_missing_root_image_before_any_job_directory() -> TestResult {
        let job = job_bytes(1)?;
        let mut blobs = MapBlobSource::default();
        blobs.put(&job);
        blobs.put(BASE);
        let (error, jobs) = launch_error(blobs, &job)?;
        assert!(
            matches!(
                error,
                LaunchError::Blob(BlobSourceError::RootImageMissing { .. })
            ),
            "{error:?}"
        );
        assert_eq!(jobs, 0);
        Ok(())
    }

    #[test]
    fn launch_refuses_a_host_that_failed_its_checks() -> TestResult {
        let parent = tempfile::tempdir()?;
        let mut backend = backend(MapBlobSource::default(), JobRoot::open_at(parent.path())?)?;
        backend.caps = backend
            .caps
            .clone()
            .with_layers(BTreeSet::from([ConfinementLayer::Rlimits]));
        assert!(matches!(
            backend.check_host(),
            Err(LaunchError::Backend { .. })
        ));
        Ok(())
    }
}
