//! The per-job input disk, written by the backend from blob bytes (05-CONTEXT D-13, D-15).
//!
//! The runner passes IDs only: a [`LaunchPlan`] names the guest job blob and the parts its role needs. The backend
//! fetches each part through its [`BlobSource`] under the part's section cap with [`fetch_parts`], checks that the
//! guest job matches the plan before anything is written, and writes the disk into the job's 0700 directory with
//! [`write_input_disk`], through core's one codec ([`minos_core::guest::write_input_disk`]). The disk is never
//! stored as a blob, it is deleted with the job directory, and the host never unpacks the tarball or applies the
//! diff (04-CONTEXT D-17): the bytes go onto the disk exactly as fetched, and only the guest reads them.
//!
//! The per-launch output token comes from [`new_token`] and goes nowhere but the disk header and the host's chunk
//! receiver: never into a blob, a log, the launch config, or an argv (05-RESEARCH Pattern 2, Pitfall 1).

use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::PathBuf;

use minos_core::chunk::{ChunkToken, TOKEN_BYTES};
use minos_core::guest::{self, DiskError, GuestJob, GuestRole, SectionKind};
use minos_core::id::BlobId;
use minos_core::outcome::Stage;
use minos_sandbox::{
    BlobSource, BlobSourceError, LaunchError, LaunchKey, LaunchPlan, Role, RoleKind,
};

use crate::jobdir::JobDir;

/// Where the per-launch token comes from: the kernel's random source.
const URANDOM: &str = "/dev/urandom";

/// The refusal for a guest job that disagrees with the plan naming it: a runner bug, so an infrastructure failure.
const MISMATCH: LaunchError = LaunchError::Backend {
    what: "the guest job does not match the plan",
};

/// A fresh per-launch output token: 32 bytes read from `/dev/urandom`.
///
/// `std::random` is unstable on the pinned toolchain, and a seeded generator would not be a secret. The source is
/// checked to be a character device, so a planted regular file cannot supply a known token.
///
/// # Errors
///
/// An [`io::Error`] when `/dev/urandom` cannot be opened or read, or is not a character device.
pub fn new_token() -> io::Result<ChunkToken> {
    let mut file = File::open(URANDOM)?;
    if !file.metadata()?.file_type().is_char_device() {
        return Err(io::Error::other("/dev/urandom is not a character device"));
    }
    let mut bytes = [0_u8; TOKEN_BYTES];
    file.read_exact(&mut bytes)?;
    Ok(ChunkToken::from_bytes(bytes))
}

/// The bytes of every part a plan names, fetched under caps and checked against the plan.
///
/// Built only by [`fetch_parts`], so a value always holds exactly its role's parts: the job, the base, and (unless
/// the baseline) the diff for a candidate; the job, the judge data, and the candidate's output for a judge.
/// `Debug` shows the role and the part lengths only, never their bytes: the judge data is the judge's secret.
pub struct FetchedParts {
    job: GuestJob,
    job_bytes: Vec<u8>,
    base: Option<Vec<u8>>,
    diff: Option<Vec<u8>>,
    judge_data: Option<Vec<u8>>,
    candidate_out: Option<Vec<u8>>,
}

impl FetchedParts {
    /// The parsed guest job.
    #[must_use]
    pub const fn job(&self) -> &GuestJob {
        &self.job
    }

    /// The guest job blob's bytes, written to the disk exactly as fetched.
    #[must_use]
    pub fn job_bytes(&self) -> &[u8] {
        &self.job_bytes
    }

    /// The base tarball, for a candidate.
    #[must_use]
    pub fn base(&self) -> Option<&[u8]> {
        self.base.as_deref()
    }

    /// The canonical cumulative diff, for a candidate that is not the baseline.
    #[must_use]
    pub fn diff(&self) -> Option<&[u8]> {
        self.diff.as_deref()
    }

    /// The judge's private data, for a judge.
    #[must_use]
    pub fn judge_data(&self) -> Option<&[u8]> {
        self.judge_data.as_deref()
    }

    /// The candidate's output stream, for a judge.
    #[must_use]
    pub fn candidate_out(&self) -> Option<&[u8]> {
        self.candidate_out.as_deref()
    }

    /// The disk sections, by kind.
    fn sections(&self) -> Vec<(SectionKind, &[u8])> {
        let mut sections = vec![(SectionKind::Job, self.job_bytes.as_slice())];
        let optional = [
            (SectionKind::Base, self.base()),
            (SectionKind::Diff, self.diff()),
            (SectionKind::JudgeData, self.judge_data()),
            (SectionKind::CandidateOut, self.candidate_out()),
        ];
        sections.extend(
            optional
                .into_iter()
                .filter_map(|(kind, bytes)| bytes.map(|bytes| (kind, bytes))),
        );
        sections
    }
}

impl fmt::Debug for FetchedParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchedParts")
            .field("role", &self.job.role())
            .field("job_bytes", &self.job_bytes.len())
            .field("base", &self.base.as_ref().map(Vec::len))
            .field("diff", &self.diff.as_ref().map(Vec::len))
            .field("judge_data", &self.judge_data.as_ref().map(Vec::len))
            .field("candidate_out", &self.candidate_out.as_ref().map(Vec::len))
            .finish()
    }
}

/// The bytes of blob `id`, at most `kind`'s section cap of them.
///
/// The cap and the hash are re-checked here even though the [`BlobSource`] contract promises both, so a source
/// that breaks it fails closed rather than putting unchecked bytes on a disk.
fn fetch<B: BlobSource>(blobs: &B, id: BlobId, kind: SectionKind) -> Result<Vec<u8>, LaunchError> {
    let max = kind.cap();
    let bytes = blobs.get(&id, max)?;
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > max {
        return Err(BlobSourceError::TooLarge { id, size, max }.into());
    }
    if BlobId::of(&bytes) != id {
        return Err(BlobSourceError::Tampered { id }.into());
    }
    Ok(bytes)
}

/// The guest role of a sandbox role, or `None` for a role the guest does not run.
const fn guest_role(kind: RoleKind) -> Option<GuestRole> {
    match kind {
        RoleKind::Candidate => Some(GuestRole::Candidate),
        RoleKind::Judge => Some(GuestRole::Judge),
        _ => None,
    }
}

/// Whether `job` is the job `plan` describes: its role, its launch key (recomputed from the job's experiment,
/// judge version, and trial), its base and diff, every stage budget, and its memory.
fn job_matches<R: Role>(job: &GuestJob, plan: &LaunchPlan<R>) -> bool {
    let input = plan.input();
    let budget = plan.budget();
    guest_role(R::KIND) == Some(job.role())
        && LaunchKey::of::<R>(&job.exp(), &job.judge_version(), job.trial()) == *plan.key()
        && job.base() == input.base()
        && job.diff() == input.diff()
        && job.stages().iter().all(|stage| {
            let planned = match stage.stage() {
                Stage::Build => budget.build_ms(),
                Stage::Run => budget.run_ms(),
                Stage::Judge => budget.judge_ms(),
            };
            stage.budget_ms() == planned
        })
        && job.memory_mib() == plan.machine().memory_mib()
}

/// Fetches every part `plan` names through `blobs`, each under its section cap, after checking that the guest job
/// matches the plan.
///
/// The guest job blob is fetched and parsed first, and compared with the plan: the role, the [`LaunchKey`]
/// recomputed from the job's experiment, judge version, and trial, the base and diff, each stage budget, and the
/// memory. Only then are the base and diff (a candidate) or the judge data and the candidate output (a judge)
/// fetched. Nothing is written.
///
/// # Errors
///
/// [`LaunchError::Blob`] when a part cannot be read, is over its cap, or does not hash to its ID;
/// [`LaunchError::Backend`] when the job blob is not a valid guest job, when the job does not match the plan, or
/// when a judge plan has no judge data. Each is an infrastructure failure: the candidate has not run.
pub fn fetch_parts<B: BlobSource, R: Role>(
    blobs: &B,
    plan: &LaunchPlan<R>,
) -> Result<FetchedParts, LaunchError> {
    let input = plan.input();
    let job_bytes = fetch(blobs, input.job(), SectionKind::Job)?;
    let job = GuestJob::parse_json(&job_bytes).map_err(|_| LaunchError::Backend {
        what: "the guest job blob is not a valid guest job",
    })?;
    if !job_matches(&job, plan) {
        return Err(MISMATCH);
    }
    let mut parts = FetchedParts {
        job,
        job_bytes,
        base: None,
        diff: None,
        judge_data: None,
        candidate_out: None,
    };
    match parts.job.role() {
        GuestRole::Candidate => {
            let base = input.base().ok_or(MISMATCH)?;
            parts.base = Some(fetch(blobs, base.to_blob_id(), SectionKind::Base)?);
            parts.diff = input
                .diff()
                .map(|diff| fetch(blobs, diff, SectionKind::Diff))
                .transpose()?;
        }
        GuestRole::Judge => {
            let data = plan.judge_data().ok_or(LaunchError::Backend {
                what: "the judge plan has no judge data",
            })?;
            let out = input.candidate_out().ok_or(MISMATCH)?;
            parts.judge_data = Some(fetch(blobs, *data.blob(), SectionKind::JudgeData)?);
            parts.candidate_out = Some(fetch(blobs, out, SectionKind::CandidateOut)?);
        }
    }
    Ok(parts)
}

/// Maps a disk writer failure: an I/O error stays one, and anything else is a host bug, since the parts were
/// fetched under the very caps the writer checks.
fn disk_error(error: DiskError) -> LaunchError {
    match error {
        DiskError::Io(source) => LaunchError::Io {
            op: "write the input disk",
            source,
        },
        _ => LaunchError::Backend {
            what: "the input disk writer refused the fetched parts",
        },
    }
}

/// Writes the input disk of `parts` under `token` into the job directory `dir`, and returns its path,
/// [`JobDir::input`].
///
/// The disk is written to a new mode 0600 temporary file in the 0700 job directory, flushed and synced, and then
/// renamed to `input.img` without replacing anything: an existing `input.img` fails the write and is left as it
/// was, and the temporary file is removed. The disk goes away with the job directory.
///
/// # Errors
///
/// [`LaunchError::Io`] when the temporary file cannot be created, written, synced, or renamed into place
/// (including when `input.img` already exists); [`LaunchError::Backend`] if core's writer refuses the parts.
pub fn write_input_disk(
    dir: &JobDir,
    token: &ChunkToken,
    parts: &FetchedParts,
) -> Result<PathBuf, LaunchError> {
    let io_error = |op: &'static str| move |source: io::Error| LaunchError::Io { op, source };
    let mut temp = tempfile::Builder::new()
        .prefix(".input-")
        .permissions(std::fs::Permissions::from_mode(0o600))
        .tempfile_in(dir.path())
        .map_err(io_error("create the input disk"))?;
    {
        let mut out = BufWriter::new(temp.as_file_mut());
        guest::write_input_disk(&mut out, token, &parts.sections()).map_err(disk_error)?;
        out.flush().map_err(io_error("write the input disk"))?;
    }
    temp.as_file()
        .sync_all()
        .map_err(io_error("sync the input disk"))?;
    let target = dir.input();
    temp.persist_noclobber(&target)
        .map_err(|error| io_error("rename the input disk into place")(error.error))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::error::Error;
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;

    use minos_core::chunk::{CHUNK_HEADER_BYTES, ChunkStream, ChunkToken, TOKEN_BYTES};
    use minos_core::diff::Diff;
    use minos_core::experiment::Experiment;
    use minos_core::guest::{GuestJob, MAX_GUEST_JOB_BYTES, SectionKind, read_input_disk};
    use minos_core::id::{BlobId, ImageDigest, TreeId};
    use minos_core::job::{JobBudget, JobRequest};
    use minos_core::limits::Limits;
    use minos_core::manifest::{Arch, JudgeVersion, Manifest};
    use minos_sandbox::{
        BlobSource, BlobSourceError, Candidate, InputDisk, Judge, JudgeData, LaunchError,
        LaunchPlan, Machine, SandboxSpec, VerifiedPath,
    };
    use serde_json::Value;

    use super::{FetchedParts, fetch_parts, new_token, write_input_disk};
    use crate::jobdir::{INPUT_FILE, JobRoot, OWNER_LOCK_FILE};

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

    /// A single-hunk modify of the manifest's one writable file.
    const DIFF: &str = "\
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,3 +1,3 @@
 fn parse() {
-    old();
+    new();
 }
";

    /// The base tarball's bytes. The backend never parses them, so any bytes serve.
    const BASE: &[u8] = b"base tarball bytes";
    const JUDGE_DATA: &[u8] = b"judge private data";
    const CANDIDATE_OUT: &[u8] = b"candidate output stream";
    const TOKEN: [u8; TOKEN_BYTES] = [0x5a; TOKEN_BYTES];

    /// A [`BlobSource`] over an in-memory map that keeps the trait's contract: a cap refused before copying, and
    /// only bytes stored under their own ID.
    #[derive(Default)]
    struct MapBlobSource(BTreeMap<BlobId, Vec<u8>>);

    impl MapBlobSource {
        fn put(&mut self, bytes: &[u8]) -> BlobId {
            let id = BlobId::of(bytes);
            self.0.insert(id, bytes.to_vec());
            id
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

    /// A [`BlobSource`] that breaks the contract: it returns `bytes` for every ID, whatever the cap.
    struct LyingSource(Vec<u8>);

    impl BlobSource for LyingSource {
        fn get(&self, _id: &BlobId, _max: u64) -> Result<Vec<u8>, BlobSourceError> {
            Ok(self.0.clone())
        }

        fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError> {
            Err(BlobSourceError::RootImageMissing { digest: *digest })
        }
    }

    fn manifest() -> Result<Manifest, Box<dyn Error>> {
        Ok(Manifest::parse_toml(MANIFEST)?)
    }

    fn judge_version() -> Result<JudgeVersion, Box<dyn Error>> {
        Ok(JudgeVersion::of(&TreeId::of(b"judge"), manifest()?.judge()))
    }

    fn diff() -> Result<Diff, Box<dyn Error>> {
        Ok(Diff::parse(
            DIFF,
            manifest()?.writable(),
            &Limits::standard(),
        )?)
    }

    /// The request for trial `trial` of the one-diff candidate of [`BASE`].
    fn request(trial: u32) -> Result<JobRequest, Box<dyn Error>> {
        let experiment = Experiment::candidate(TreeId::of(BASE), diff()?);
        Ok(JobRequest::candidate(
            &experiment,
            BlobId::of(diff()?.as_bytes()),
            judge_version()?,
            trial,
            JobBudget::from_budget(manifest()?.budget()),
        )?)
    }

    fn baseline_request() -> Result<JobRequest, Box<dyn Error>> {
        Ok(JobRequest::baseline(
            TreeId::of(BASE),
            judge_version()?,
            0,
            JobBudget::from_budget(manifest()?.budget()),
        )?)
    }

    fn machine(memory_mib: u16) -> Result<Machine, Box<dyn Error>> {
        Ok(Machine::new(2, memory_mib)?)
    }

    fn default_memory() -> Result<u16, Box<dyn Error>> {
        Ok(manifest()?.environment().memory_mib())
    }

    /// A Candidate plan for `request` whose job blob is `job_bytes`, on a machine of `memory_mib`.
    fn candidate_plan(
        request: &JobRequest,
        job_bytes: &[u8],
        memory_mib: u16,
    ) -> Result<LaunchPlan<Candidate>, Box<dyn Error>> {
        let input = InputDisk::candidate(BlobId::of(job_bytes), request.base(), request.diff());
        Ok(SandboxSpec::<Candidate>::new(
            request,
            ImageDigest::of(b"root image"),
            input,
            machine(memory_mib)?,
            Arch::Arm64,
        )?
        .seal())
    }

    /// A Judge plan for `request` whose job blob is `job_bytes`, with judge data when `with_data`.
    fn judge_plan(
        request: &JobRequest,
        job_bytes: &[u8],
        with_data: bool,
    ) -> Result<LaunchPlan<Judge>, Box<dyn Error>> {
        let input = InputDisk::judge(BlobId::of(job_bytes), BlobId::of(CANDIDATE_OUT));
        let spec = SandboxSpec::<Judge>::new(
            request,
            ImageDigest::of(b"root image"),
            input,
            machine(default_memory()?)?,
            Arch::Arm64,
        )?;
        let spec = if with_data {
            spec.secrets(JudgeData::new(BlobId::of(JUDGE_DATA)))
        } else {
            spec
        };
        Ok(spec.seal())
    }

    /// A source holding the base, the diff, the judge data, the candidate output, and `job_bytes`.
    fn source(job_bytes: &[u8]) -> Result<MapBlobSource, Box<dyn Error>> {
        let mut blobs = MapBlobSource::default();
        blobs.put(job_bytes);
        blobs.put(BASE);
        blobs.put(diff()?.as_bytes());
        blobs.put(JUDGE_DATA);
        blobs.put(CANDIDATE_OUT);
        Ok(blobs)
    }

    fn candidate_job_bytes(trial: u32) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(GuestJob::candidate(&manifest()?, &request(trial)?).to_json()?)
    }

    /// The candidate job JSON of trial 1 with `edit` applied to it as a JSON value.
    fn edited_job(edit: impl FnOnce(&mut Value) -> Option<()>) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut value: Value = serde_json::from_slice(&candidate_job_bytes(1)?)?;
        edit(&mut value).ok_or("the edit found no field")?;
        let bytes = serde_json::to_vec(&value)?;
        // The edited job is itself valid, so only the plan comparison can refuse it.
        GuestJob::parse_json(&bytes)?;
        Ok(bytes)
    }

    fn is_mismatch(result: &Result<FetchedParts, LaunchError>) -> bool {
        matches!(
            result,
            Err(LaunchError::Backend {
                what: "the guest job does not match the plan"
            })
        )
    }

    #[test]
    fn fetch_parts_candidate() -> TestResult {
        let job_bytes = candidate_job_bytes(1)?;
        let blobs = source(&job_bytes)?;
        let plan = candidate_plan(&request(1)?, &job_bytes, default_memory()?)?;
        let parts = fetch_parts(&blobs, &plan)?;
        assert_eq!(parts.job(), &GuestJob::parse_json(&job_bytes)?);
        assert_eq!(parts.job_bytes(), job_bytes.as_slice());
        assert_eq!(parts.base(), Some(BASE));
        assert_eq!(parts.diff(), Some(diff()?.as_bytes()));
        assert_eq!(parts.judge_data(), None);
        assert_eq!(parts.candidate_out(), None);
        Ok(())
    }

    #[test]
    fn fetch_parts_baseline_has_no_diff() -> TestResult {
        let job_bytes = GuestJob::candidate(&manifest()?, &baseline_request()?).to_json()?;
        let blobs = source(&job_bytes)?;
        let plan = candidate_plan(&baseline_request()?, &job_bytes, default_memory()?)?;
        let parts = fetch_parts(&blobs, &plan)?;
        assert_eq!(parts.base(), Some(BASE));
        assert_eq!(parts.diff(), None);
        Ok(())
    }

    #[test]
    fn fetch_parts_judge() -> TestResult {
        let job_bytes = GuestJob::judge(&manifest()?, &request(1)?).to_json()?;
        let blobs = source(&job_bytes)?;
        let plan = judge_plan(&request(1)?, &job_bytes, true)?;
        let parts = fetch_parts(&blobs, &plan)?;
        assert_eq!(parts.job_bytes(), job_bytes.as_slice());
        assert_eq!(parts.base(), None);
        assert_eq!(parts.diff(), None);
        assert_eq!(parts.judge_data(), Some(JUDGE_DATA));
        assert_eq!(parts.candidate_out(), Some(CANDIDATE_OUT));
        Ok(())
    }

    #[test]
    fn fetch_parts_refuses_mismatch() -> TestResult {
        let memory = default_memory()?;
        // Role: a judge job named by a Candidate plan.
        let judge_job = GuestJob::judge(&manifest()?, &request(1)?).to_json()?;
        let plan = candidate_plan(&request(1)?, &judge_job, memory)?;
        assert!(is_mismatch(&fetch_parts(&source(&judge_job)?, &plan)));
        // Launch key: the job of trial 1 named by the plan of trial 2.
        let job = candidate_job_bytes(1)?;
        let plan = candidate_plan(&request(2)?, &job, memory)?;
        assert!(is_mismatch(&fetch_parts(&source(&job)?, &plan)));
        // Base and diff: a job edited to name another base or another diff, under the right key.
        let other_base = edited_job(|v| {
            *v.get_mut("base")? = Value::String(TreeId::of(b"other").to_hex());
            Some(())
        })?;
        let plan = candidate_plan(&request(1)?, &other_base, memory)?;
        assert!(is_mismatch(&fetch_parts(&source(&other_base)?, &plan)));
        let other_diff = edited_job(|v| {
            *v.get_mut("diff")? = Value::String(BlobId::of(b"other").to_hex());
            Some(())
        })?;
        let plan = candidate_plan(&request(1)?, &other_diff, memory)?;
        assert!(is_mismatch(&fetch_parts(&source(&other_diff)?, &plan)));
        // A stage budget: the run stage one second longer than the plan's.
        let other_budget = edited_job(|v| {
            let run = v.get_mut("stages")?.as_array_mut()?.get_mut(1)?;
            let budget = run.get("budget_ms")?.as_u64()?.checked_add(1000)?;
            *run.get_mut("budget_ms")? = Value::from(budget);
            Some(())
        })?;
        let plan = candidate_plan(&request(1)?, &other_budget, memory)?;
        assert!(is_mismatch(&fetch_parts(&source(&other_budget)?, &plan)));
        // Memory: the plan's machine is larger than the job's memory.
        let plan = candidate_plan(&request(1)?, &job, memory.checked_mul(2).ok_or("memory")?)?;
        assert!(is_mismatch(&fetch_parts(&source(&job)?, &plan)));
        // A Judge plan with no judge data attached.
        let plan = judge_plan(&request(1)?, &judge_job, false)?;
        assert!(matches!(
            fetch_parts(&source(&judge_job)?, &plan),
            Err(LaunchError::Backend { .. })
        ));
        Ok(())
    }

    #[test]
    fn fetch_parts_refuses_over_cap_and_unparsable_jobs() -> TestResult {
        let over = vec![b' '; MAX_GUEST_JOB_BYTES.checked_add(1).ok_or("cap")?];
        let plan = candidate_plan(&request(1)?, &over, default_memory()?)?;
        assert!(matches!(
            fetch_parts(&source(&over)?, &plan),
            Err(LaunchError::Blob(BlobSourceError::TooLarge { .. }))
        ));
        let junk = b"{\"v\":1}".to_vec();
        let plan = candidate_plan(&request(1)?, &junk, default_memory()?)?;
        assert!(matches!(
            fetch_parts(&source(&junk)?, &plan),
            Err(LaunchError::Backend { .. })
        ));
        let job = candidate_job_bytes(1)?;
        let plan = candidate_plan(&request(1)?, &job, default_memory()?)?;
        assert!(matches!(
            fetch_parts(&MapBlobSource::default(), &plan),
            Err(LaunchError::Blob(BlobSourceError::Missing { .. }))
        ));
        Ok(())
    }

    #[test]
    fn fetch_parts_refuses_a_source_that_breaks_its_contract() -> TestResult {
        // Bytes that do not hash to the ID asked for are refused as tampered.
        let job = candidate_job_bytes(1)?;
        let plan = candidate_plan(&request(1)?, &job, default_memory()?)?;
        assert!(matches!(
            fetch_parts(&LyingSource(b"not the job".to_vec()), &plan),
            Err(LaunchError::Blob(BlobSourceError::Tampered { .. }))
        ));
        // Bytes over the cap are refused even when the source ignored it.
        let over = vec![b' '; MAX_GUEST_JOB_BYTES.checked_add(1).ok_or("cap")?];
        let plan = candidate_plan(&request(1)?, &over, default_memory()?)?;
        assert!(matches!(
            fetch_parts(&LyingSource(over), &plan),
            Err(LaunchError::Blob(BlobSourceError::TooLarge { .. }))
        ));
        Ok(())
    }

    #[test]
    fn write_disk_round_trips() -> TestResult {
        let parent = tempfile::tempdir()?;
        let root = JobRoot::open_at(parent.path())?;
        for (job_bytes, judge) in [
            (candidate_job_bytes(1)?, false),
            (GuestJob::judge(&manifest()?, &request(1)?).to_json()?, true),
        ] {
            let blobs = source(&job_bytes)?;
            let parts = if judge {
                fetch_parts(&blobs, &judge_plan(&request(1)?, &job_bytes, true)?)?
            } else {
                fetch_parts(
                    &blobs,
                    &candidate_plan(&request(1)?, &job_bytes, default_memory()?)?,
                )?
            };
            let dir = root.create_job()?;
            let path = write_input_disk(&dir, &ChunkToken::from_bytes(TOKEN), &parts)?;
            assert_eq!(path, dir.input());
            let mode = fs::symlink_metadata(&path)?.permissions().mode() & 0o7777;
            assert_eq!(mode, 0o600);
            let mut sections = read_input_disk(&mut File::open(&path)?)?;
            assert!(sections.token().ct_eq(&TOKEN));
            sections.check_role(parts.job())?;
            assert_eq!(
                sections.take(SectionKind::Job).as_deref(),
                Some(job_bytes.as_slice())
            );
            assert_eq!(sections.take(SectionKind::Base).as_deref(), parts.base());
            assert_eq!(sections.take(SectionKind::Diff).as_deref(), parts.diff());
            assert_eq!(
                sections.take(SectionKind::JudgeData).as_deref(),
                parts.judge_data()
            );
            assert_eq!(
                sections.take(SectionKind::CandidateOut).as_deref(),
                parts.candidate_out()
            );
        }
        Ok(())
    }

    #[test]
    fn write_disk_never_overwrites() -> TestResult {
        let parent = tempfile::tempdir()?;
        let dir = JobRoot::open_at(parent.path())?.create_job()?;
        let job_bytes = candidate_job_bytes(1)?;
        let parts = fetch_parts(
            &source(&job_bytes)?,
            &candidate_plan(&request(1)?, &job_bytes, default_memory()?)?,
        )?;
        let path = write_input_disk(&dir, &ChunkToken::from_bytes(TOKEN), &parts)?;
        let first = fs::read(&path)?;
        let again = write_input_disk(&dir, &ChunkToken::from_bytes([0x11; TOKEN_BYTES]), &parts);
        assert!(matches!(again, Err(LaunchError::Io { .. })));
        assert_eq!(fs::read(&path)?, first);
        // The refused write left no temporary file behind.
        let mut names = fs::read_dir(dir.path())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()?;
        names.sort();
        assert_eq!(names, [INPUT_FILE, OWNER_LOCK_FILE]);
        Ok(())
    }

    #[test]
    fn new_token_is_random() -> TestResult {
        let first = new_token()?;
        let second = new_token()?;
        // The token bytes are private to core; a chunk built under `first` carries them in its header.
        let chunk = ChunkStream::new(&first, b"x").next().ok_or("no chunk")?;
        let sent = chunk.get(8..CHUNK_HEADER_BYTES - 8).ok_or("short chunk")?;
        assert_eq!(sent.len(), TOKEN_BYTES);
        assert!(first.ct_eq(sent));
        assert!(!second.ct_eq(sent));
        assert!(!sent.iter().all(|byte| *byte == 0));
        Ok(())
    }
}
