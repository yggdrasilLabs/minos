//! One job, start to finish, for either role (GUEST-02, GUEST-03, GUEST-04; 05-CONTEXT D-07, D-08).
//!
//! [`run_job`] fixes the order of the work, and four traits supply everything that touches the outside world:
//!
//! 1. read the job through a [`JobSource`];
//! 2. create the per-run directories under the scratch root, which must not exist yet: per-run state starts empty;
//! 3. a candidate unpacks its base tree into `src` and applies its diff with core's strict applier; a judge
//!    unpacks its private data into `judge` and the candidate's output into `candidate`, both left root-owned so
//!    the judge can read them but not change them;
//! 4. drop every input buffer, so no stage runs while PID 1 still holds its inputs in the guest reserve (D-05);
//! 5. hand the writable directories to the stage uid through a [`Handover`];
//! 6. run each stage through a [`StageExecutor`] until one does not exit 0; the rest are skipped;
//! 7. pack the output tar and send it through an [`OutputSink`].
//!
//! A diff that does not apply is a candidate fault: every stage is skipped and the status says
//! `ApplyStatus::Failed`, which the runner records as `Rejected`. A step that fails for any other reason is a
//! setup failure, reported as `SetupStatus::Failed` with no stage run, which the host treats as an infrastructure
//! failure. A source failure that kept the token still sends that status, authenticated, so the host learns why;
//! without the token nothing can be sent.
//!
//! The Linux entry (plan 05-12) supplies the real parts: a source over `/dev/vdb`, a vsock sink, an executor that
//! runs each stage through the `/init --minos-stage` trampoline as uid 64000 under its deadline, then kills and
//! reaps everything the stage left, and a handover that changes owners recursively without following links. The
//! AWS milestone supplies a lifecycle-hook source and sink to the same function.

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use minos_core::guest::{
    ApplyStatus, GuestJob, GuestRole, GuestStatus, GuestStatusError, OutputStatus, SetupStatus,
    SetupStep, StageEnd, StageReport,
};
use minos_core::outcome::Stage;

use crate::pack::{PackError, build_output, out_budget, read_scorecard, walk_out};
use crate::sink::{OutputSink, SendReport, SinkError};
use crate::source::{JobSource, SourceError};
use crate::stage::StageLimits;
use crate::workspace::{
    ApplyOutcome, ApplySetupError, UnpackCaps, UnpackError, apply_diff, unpack_tar,
};

/// A candidate's source tree, under the scratch root: the build and run working directory.
pub const SRC: &str = "src";

/// Where every stage writes its output, under the scratch root; bind-mounted on `/out`.
pub const OUT: &str = "out";

/// The stage's `HOME`, under the scratch root.
pub const HOME: &str = "home";

/// The stage's temporary directory, under the scratch root; bind-mounted on `/tmp`.
pub const TMP: &str = "tmp";

/// A judge's private data, under the scratch root: the judge's working directory.
pub const JUDGE: &str = "judge";

/// The candidate's output as a judge receives it, under the scratch root.
pub const CANDIDATE: &str = "candidate";

/// One stage as the executor runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCommand {
    stage: Stage,
    argv: Vec<String>,
    cwd: PathBuf,
    limits: StageLimits,
    deadline: Duration,
}

impl StageCommand {
    /// Which stage.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// The command, as the manifest names it.
    #[must_use]
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    /// The working directory.
    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The rlimits the trampoline applies.
    #[must_use]
    pub const fn limits(&self) -> StageLimits {
        self.limits
    }

    /// PID 1's wall-clock deadline: the stage budget.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }
}

/// How one stage ended and what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageRun {
    end: StageEnd,
    log: Vec<u8>,
}

impl StageRun {
    /// A stage that ended as `end` after printing `log` (stdout and stderr together).
    #[must_use]
    pub const fn new(end: StageEnd, log: Vec<u8>) -> Self {
        Self { end, log }
    }

    /// How it ended.
    #[must_use]
    pub const fn end(&self) -> StageEnd {
        self.end
    }

    /// What it printed.
    #[must_use]
    pub fn log(&self) -> &[u8] {
        &self.log
    }
}

/// Runs one stage.
pub trait StageExecutor {
    /// Runs `command` to its end or its deadline and captures its combined output. Before returning it kills every
    /// process the stage left and reaps them all, so nothing the candidate started survives to race the packer
    /// (D-08). A stage stopped at its deadline is [`StageEnd::TimedOut`] for that stage (05-RESEARCH Pitfall 12).
    fn run_stage(&mut self, command: &StageCommand) -> StageRun;
}

/// Gives a writable directory to the stage uid.
pub trait Handover {
    /// Hands `dir` and everything under it to the stage uid and gid, never following a link.
    ///
    /// # Errors
    ///
    /// The ownership change's error.
    fn hand_over(&mut self, dir: &Path) -> io::Result<()>;
}

/// Why a job did not run cleanly. Messages are fixed text; the sources carry numbers and kinds only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// The job source failed.
    #[error("the job source failed")]
    Source(#[source] SourceError),
    /// The job lacks an input section its role needs, which the source should have refused.
    #[error("the job lacks an input its role needs")]
    MissingInput,
    /// A per-run directory could not be created, or already existed.
    #[error("could not create the per-run directories")]
    Directories(#[source] io::Error),
    /// An input could not be unpacked.
    #[error("could not unpack an input")]
    Unpack(#[source] UnpackError),
    /// The diff could not be tried.
    #[error("could not apply the diff")]
    Apply(#[source] ApplySetupError),
    /// A directory could not be handed to the stage uid.
    #[error("could not hand a directory to the stage uid")]
    Handover(#[source] io::Error),
    /// The status did not pass core's checks.
    #[error("the guest status is not valid")]
    Status(#[source] GuestStatusError),
    /// The output tar could not be built.
    #[error("could not pack the output")]
    Pack(#[source] PackError),
}

/// What [`run_job`] did.
#[derive(Debug)]
pub struct JobEnd {
    status: Option<GuestStatus>,
    sent: Option<Result<SendReport, SinkError>>,
    error: Option<JobError>,
}

impl JobEnd {
    /// The status that was packed, or `None` when nothing could be packed.
    #[must_use]
    pub const fn status(&self) -> Option<&GuestStatus> {
        self.status.as_ref()
    }

    /// The send's result, or `None` when nothing was sent: the source failed before the token was known, or no
    /// tar could be built.
    #[must_use]
    pub const fn sent(&self) -> Option<&Result<SendReport, SinkError>> {
        self.sent.as_ref()
    }

    /// The first setup or source error, if any. A candidate fault such as a diff that does not apply or a stage
    /// that fails is not an error: it is in the status.
    #[must_use]
    pub const fn error(&self) -> Option<&JobError> {
        self.error.as_ref()
    }
}

/// Runs one job, as the [module docs](self) describe, with the per-run directories under `root`.
pub fn run_job<S: JobSource, O: OutputSink, E: StageExecutor, H: Handover>(
    source: &mut S,
    sink: &mut O,
    executor: &mut E,
    handover: &mut H,
    root: &Path,
) -> JobEnd {
    let _ = (source, sink, executor, handover, root);
    JobEnd {
        status: None,
        sent: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::error::Error;
    use std::io::{Cursor, Read, Write};
    use std::rc::Rc;

    use minos_core::chunk::{ChunkToken, Reassembler};
    use minos_core::guest::{SectionKind, write_input_disk};
    use minos_core::limits::MAX_OUTPUT_STREAM_BYTES;
    use minos_core::path::RelPath;

    use super::*;
    use crate::pack::{BUILD_LOG, JUDGE_LOG, RUN_LOG, SCORECARD};
    use crate::sink::{Backoff, ChunkSink, Connector};
    use crate::source::DiskJobSource;
    use crate::stage::{HOME_DIR, SCRATCH_MOUNT, SCRATCH_OUT_DIR, SCRATCH_TMP_DIR, SRC_DIR};
    use crate::test_fixture::{
        DIFF, PARSE_RS, PARSE_RS_AFTER, baseline_job_for, candidate_job_for, judge_job, tar_of,
    };

    type TestResult = Result<(), Box<dyn Error>>;

    /// A parsed tar: each entry's name and bytes.
    type TarEntries = Vec<(String, Vec<u8>)>;

    const TOKEN: [u8; 32] = [0x7e; 32];

    fn token() -> ChunkToken {
        ChunkToken::from_bytes(TOKEN)
    }

    /// Keeps every closed connection's bytes where the test can read them after the sink is done.
    #[derive(Default, Clone)]
    struct Memory {
        closed: Rc<RefCell<Vec<Vec<u8>>>>,
    }

    impl Connector for Memory {
        type Stream = Vec<u8>;

        fn connect(&mut self) -> io::Result<Vec<u8>> {
            Ok(Vec::new())
        }

        fn close(&mut self, stream: Vec<u8>) -> io::Result<()> {
            self.closed.borrow_mut().push(stream);
            Ok(())
        }
    }

    /// Plants a symlink in `out` when it is handed over, as a candidate could before the walk.
    struct Plant {
        record: Record,
        out: PathBuf,
    }

    impl Handover for Plant {
        fn hand_over(&mut self, dir: &Path) -> io::Result<()> {
            if dir == self.out {
                std::os::unix::fs::symlink("/etc/hosts", dir.join("link"))?;
            }
            self.record.hand_over(dir)
        }
    }

    /// Runs every stage with a scripted end, recording each command, and writes files into `out` during a stage.
    struct Script {
        ends: BTreeMap<Stage, StageEnd>,
        writes: Vec<(Stage, &'static str, Vec<u8>)>,
        out: PathBuf,
        seen: Vec<StageCommand>,
        src_at_build: Option<Vec<u8>>,
    }

    impl Script {
        fn new(root: &Path) -> Self {
            Self {
                ends: BTreeMap::new(),
                writes: Vec::new(),
                out: root.join(OUT),
                seen: Vec::new(),
                src_at_build: None,
            }
        }
    }

    impl StageExecutor for Script {
        fn run_stage(&mut self, command: &StageCommand) -> StageRun {
            self.seen.push(command.clone());
            if command.stage() == Stage::Build {
                self.src_at_build = fs::read(command.cwd().join("src/parse.rs")).ok();
            }
            for (stage, name, bytes) in &self.writes {
                if *stage == command.stage() {
                    let mut file = fs::File::create(self.out.join(name)).ok();
                    if let Some(file) = file.as_mut() {
                        let _ = file.write_all(bytes);
                    }
                }
            }
            let end = self
                .ends
                .get(&command.stage())
                .copied()
                .unwrap_or(StageEnd::Exited { code: 0 });
            StageRun::new(end, format!("{} ran\n", command.stage()).into_bytes())
        }
    }

    /// Records every directory handed over.
    #[derive(Default)]
    struct Record {
        dirs: Vec<PathBuf>,
    }

    impl Handover for Record {
        fn hand_over(&mut self, dir: &Path) -> io::Result<()> {
            self.dirs.push(dir.to_path_buf());
            Ok(())
        }
    }

    /// The disk of `sections` under the fixed token.
    fn disk(sections: &[(SectionKind, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut out = Vec::new();
        write_input_disk(&mut out, &token(), sections)?;
        Ok(out)
    }

    /// A candidate disk: the job for `base_tar` and `diff`, the base, and the diff if any.
    fn candidate_disk(base_tar: &[u8], diff: Option<&str>) -> Result<Vec<u8>, Box<dyn Error>> {
        let job = match diff {
            Some(text) => candidate_job_for(base_tar, text)?,
            None => baseline_job_for(base_tar)?,
        };
        let json = job.to_json()?;
        let mut sections: Vec<(SectionKind, &[u8])> =
            vec![(SectionKind::Job, &json), (SectionKind::Base, base_tar)];
        if let Some(text) = diff {
            sections.push((SectionKind::Diff, text.as_bytes()));
        }
        disk(&sections)
    }

    /// What one run_job sent.
    struct Sent {
        end: JobEnd,
        connections: usize,
        tar: Option<Vec<u8>>,
    }

    fn run<H: Handover>(
        disk: Vec<u8>,
        executor: &mut Script,
        handover: &mut H,
        root: &Path,
    ) -> Result<Sent, Box<dyn Error>> {
        let memory = Memory::default();
        let mut source = DiskJobSource::new(Cursor::new(disk));
        let mut sink = ChunkSink::new(memory.clone(), token()).with_backoff(Backoff::none());
        let end = run_job(&mut source, &mut sink, executor, handover, root);
        let closed = memory.closed.take();
        let connections = closed.len();
        let tar = if closed.is_empty() {
            None
        } else {
            let mut receiver = Reassembler::new(token(), MAX_OUTPUT_STREAM_BYTES);
            for connection in &closed {
                receiver.accept(connection)?;
            }
            Some(receiver.finish()?)
        };
        Ok(Sent {
            end,
            connections,
            tar,
        })
    }

    fn read_tar(tar: &[u8]) -> Result<TarEntries, Box<dyn Error>> {
        let mut entries = Vec::new();
        let mut archive = tar::Archive::new(Cursor::new(tar));
        for entry in archive.entries()?.raw(true) {
            let mut entry = entry?;
            let name = String::from_utf8(entry.header().path_bytes().into_owned())?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            entries.push((name, bytes));
        }
        Ok(entries)
    }

    fn entry<'a>(entries: &'a TarEntries, name: &str) -> Option<&'a [u8]> {
        entries
            .iter()
            .find(|(entry, _)| entry == name)
            .map(|(_, bytes)| bytes.as_slice())
    }

    fn sent_status(sent: &Sent) -> Result<(GuestStatus, TarEntries), Box<dyn Error>> {
        let tar = sent.tar.as_deref().ok_or("nothing was sent")?;
        let entries = read_tar(tar)?;
        let status = GuestStatus::parse_json(
            entry(&entries, minos_core::guest::STATUS_PATH).ok_or("no status")?,
        )?;
        Ok((status, entries))
    }

    fn ends(status: &GuestStatus) -> Vec<(Stage, StageEnd)> {
        status
            .stages()
            .iter()
            .map(|report| (report.stage(), report.end()))
            .collect()
    }

    fn base_tar() -> Result<Vec<u8>, Box<dyn Error>> {
        tar_of(&[
            ("src/lib.rs", 0o644, b"pub mod parse;\n"),
            ("src/parse.rs", 0o644, PARSE_RS),
        ])
    }

    #[test]
    fn run_dirs_match_stage_paths() {
        let scratch = Path::new(SCRATCH_MOUNT);
        assert_eq!(scratch.join(SRC), Path::new(SRC_DIR));
        assert_eq!(scratch.join(OUT), Path::new(SCRATCH_OUT_DIR));
        assert_eq!(scratch.join(HOME), Path::new(HOME_DIR));
        assert_eq!(scratch.join(TMP), Path::new(SCRATCH_TMP_DIR));
        assert_eq!(scratch.join(JUDGE), Path::new(crate::stage::JUDGE_DIR));
        assert_eq!(
            scratch.join(CANDIDATE),
            Path::new(crate::stage::CANDIDATE_DIR)
        );
    }

    #[test]
    fn run_candidate_happy_path() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = base_tar()?;
        let job = candidate_job_for(&base, DIFF)?;
        let mut executor = Script::new(root.path());
        executor
            .writes
            .push((Stage::Run, "result.json", b"{\"ok\":1}".to_vec()));
        let mut handover = Record::default();
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut handover,
            root.path(),
        )?;

        assert!(sent.end.error().is_none());
        assert!(matches!(sent.end.sent(), Some(Ok(_))));
        assert_eq!(
            handover.dirs,
            [SRC, OUT, HOME, TMP].map(|dir| root.path().join(dir))
        );
        assert_eq!(executor.src_at_build.as_deref(), Some(PARSE_RS_AFTER));
        assert_eq!(executor.seen.len(), 2);
        for (command, spec) in executor.seen.iter().zip(job.stages()) {
            assert_eq!(command.stage(), spec.stage());
            assert_eq!(command.argv(), spec.argv());
            assert_eq!(command.cwd(), root.path().join(SRC));
            assert_eq!(command.limits(), StageLimits::for_stage(&job, spec));
            assert_eq!(command.deadline(), Duration::from_millis(spec.budget_ms()));
        }

        let (status, entries) = sent_status(&sent)?;
        assert_eq!(Some(&status), sent.end.status());
        assert_eq!(status.setup(), SetupStatus::Ok);
        assert_eq!(status.apply(), &ApplyStatus::Applied);
        assert_eq!(
            ends(&status),
            [
                (Stage::Build, StageEnd::Exited { code: 0 }),
                (Stage::Run, StageEnd::Exited { code: 0 }),
            ]
        );
        assert_eq!(
            status.output(),
            OutputStatus::Packed {
                files: 1,
                content_bytes: 8
            }
        );
        assert_eq!(entry(&entries, "out/result.json"), Some(&b"{\"ok\":1}"[..]));
        assert_eq!(entry(&entries, BUILD_LOG), Some(&b"build ran\n"[..]));
        assert_eq!(entry(&entries, RUN_LOG), Some(&b"run ran\n"[..]));
        assert_eq!(entry(&entries, SCORECARD), None);
        Ok(())
    }

    #[test]
    fn run_build_failure_skips_run() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = base_tar()?;
        let mut executor = Script::new(root.path());
        executor
            .ends
            .insert(Stage::Build, StageEnd::Exited { code: 2 });
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut Record::default(),
            root.path(),
        )?;
        let (status, entries) = sent_status(&sent)?;
        assert_eq!(
            ends(&status),
            [
                (Stage::Build, StageEnd::Exited { code: 2 }),
                (Stage::Run, StageEnd::Skipped),
            ]
        );
        assert_eq!(executor.seen.len(), 1);
        assert!(entry(&entries, BUILD_LOG).is_some());
        assert!(entry(&entries, RUN_LOG).is_none());
        Ok(())
    }

    #[test]
    fn run_apply_failure_skips_all() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = tar_of(&[("src/parse.rs", 0o644, b"fn parse() {\n    other();\n}\n")])?;
        let mut executor = Script::new(root.path());
        let mut handover = Record::default();
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut handover,
            root.path(),
        )?;
        assert!(sent.end.error().is_none());
        let (status, _) = sent_status(&sent)?;
        assert_eq!(status.setup(), SetupStatus::Ok);
        assert_eq!(
            status.apply(),
            &ApplyStatus::Failed {
                path: RelPath::try_from("src/parse.rs")?,
                hunk: 1,
            }
        );
        assert_eq!(
            ends(&status),
            [
                (Stage::Build, StageEnd::Skipped),
                (Stage::Run, StageEnd::Skipped)
            ]
        );
        assert!(executor.seen.is_empty());
        assert!(handover.dirs.is_empty());
        Ok(())
    }

    #[test]
    fn run_baseline() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = base_tar()?;
        let mut executor = Script::new(root.path());
        let sent = run(
            candidate_disk(&base, None)?,
            &mut executor,
            &mut Record::default(),
            root.path(),
        )?;
        let (status, _) = sent_status(&sent)?;
        assert_eq!(status.apply(), &ApplyStatus::Baseline);
        assert_eq!(
            ends(&status),
            [
                (Stage::Build, StageEnd::Exited { code: 0 }),
                (Stage::Run, StageEnd::Exited { code: 0 }),
            ]
        );
        assert_eq!(executor.src_at_build.as_deref(), Some(PARSE_RS));
        Ok(())
    }

    #[test]
    fn run_judge() -> TestResult {
        let root = tempfile::tempdir()?;
        let job = judge_job()?;
        let judge_data = tar_of(&[
            ("judge", 0o755, b"#!/bin/sh\n"),
            ("expected.txt", 0o644, b"42\n"),
        ])?;
        let candidate_out = tar_of(&[("out/result.json", 0o644, b"{}")])?;
        let json = job.to_json()?;
        let disk = disk(&[
            (SectionKind::Job, &json),
            (SectionKind::JudgeData, &judge_data),
            (SectionKind::CandidateOut, &candidate_out),
        ])?;
        let mut executor = Script::new(root.path());
        executor.writes.push((
            Stage::Judge,
            "scorecard.json",
            b"{\"v\":1,\"metrics\":[]}".to_vec(),
        ));
        let mut handover = Record::default();
        let sent = run(disk, &mut executor, &mut handover, root.path())?;

        assert!(sent.end.error().is_none());
        assert_eq!(
            handover.dirs,
            [OUT, HOME, TMP].map(|dir| root.path().join(dir))
        );
        assert_eq!(fs::read(root.path().join("judge/expected.txt"))?, b"42\n");
        assert_eq!(
            fs::read(root.path().join("candidate/out/result.json"))?,
            b"{}"
        );
        assert!(!root.path().join(SRC).exists());
        let [command] = executor.seen.as_slice() else {
            return Err("the judge stage did not run exactly once".into());
        };
        assert_eq!(command.stage(), Stage::Judge);
        assert_eq!(command.cwd(), root.path().join(JUDGE));

        let (status, entries) = sent_status(&sent)?;
        assert_eq!(status.role(), GuestRole::Judge);
        assert_eq!(status.apply(), &ApplyStatus::NotApplicable);
        assert_eq!(
            ends(&status),
            [(Stage::Judge, StageEnd::Exited { code: 0 })]
        );
        assert_eq!(
            entry(&entries, SCORECARD),
            Some(&b"{\"v\":1,\"metrics\":[]}"[..])
        );
        assert_eq!(entry(&entries, JUDGE_LOG), Some(&b"judge ran\n"[..]));
        assert!(!entries.iter().any(|(name, _)| name.starts_with("out/")));
        Ok(())
    }

    #[test]
    fn run_source_failure_with_token() -> TestResult {
        let root = tempfile::tempdir()?;
        let disk = disk(&[(SectionKind::Job, b"not a job")])?;
        let mut executor = Script::new(root.path());
        let sent = run(disk, &mut executor, &mut Record::default(), root.path())?;
        assert!(matches!(sent.end.error(), Some(JobError::Source(_))));
        let (status, _) = sent_status(&sent)?;
        assert_eq!(
            status.setup(),
            SetupStatus::Failed {
                step: SetupStep::Input
            }
        );
        assert!(status.stages().is_empty());
        assert_eq!(status.output(), OutputStatus::NotPacked);
        assert!(executor.seen.is_empty());
        Ok(())
    }

    #[test]
    fn run_source_failure_without_token() -> TestResult {
        let root = tempfile::tempdir()?;
        let mut executor = Script::new(root.path());
        let sent = run(
            b"garbage".to_vec(),
            &mut executor,
            &mut Record::default(),
            root.path(),
        )?;
        assert!(matches!(sent.end.error(), Some(JobError::Source(_))));
        assert!(sent.end.sent().is_none());
        assert!(sent.end.status().is_none());
        assert_eq!(sent.connections, 0);
        Ok(())
    }

    #[test]
    fn run_timeout_is_per_stage() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = base_tar()?;
        let mut executor = Script::new(root.path());
        executor.ends.insert(Stage::Build, StageEnd::TimedOut);
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut Record::default(),
            root.path(),
        )?;
        let (status, _) = sent_status(&sent)?;
        assert_eq!(
            ends(&status),
            [
                (Stage::Build, StageEnd::TimedOut),
                (Stage::Run, StageEnd::Skipped)
            ]
        );
        Ok(())
    }

    #[test]
    fn run_refuses_existing_state() -> TestResult {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join(SRC))?;
        let base = base_tar()?;
        let mut executor = Script::new(root.path());
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut Record::default(),
            root.path(),
        )?;
        assert!(matches!(sent.end.error(), Some(JobError::Directories(_))));
        let (status, _) = sent_status(&sent)?;
        assert_eq!(
            status.setup(),
            SetupStatus::Failed {
                step: SetupStep::Mount
            }
        );
        assert!(executor.seen.is_empty());
        Ok(())
    }

    #[test]
    fn run_invalid_out_packs_no_out_files() -> TestResult {
        let root = tempfile::tempdir()?;
        let base = base_tar()?;
        let mut executor = Script::new(root.path());
        executor.writes.push((Stage::Run, "a.txt", b"a".to_vec()));
        let mut plant = Plant {
            record: Record::default(),
            out: root.path().join(OUT),
        };
        let sent = run(
            candidate_disk(&base, Some(DIFF))?,
            &mut executor,
            &mut plant,
            root.path(),
        )?;
        assert!(sent.end.error().is_none());
        let (status, entries) = sent_status(&sent)?;
        assert_eq!(status.output(), OutputStatus::Invalid);
        assert!(!entries.iter().any(|(name, _)| name.starts_with("out/")));
        assert!(entry(&entries, RUN_LOG).is_some());
        Ok(())
    }
}
