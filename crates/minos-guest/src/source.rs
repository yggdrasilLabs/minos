//! The source of one job: where PID 1 reads the job and its input sections (GUEST-02; 05-CONTEXT D-07).
//!
//! PID 1 reads its input through the [`JobSource`] trait, so the decoders, the applier, and the command runner
//! never name where the bytes came from. The Linux entry point opens the read-only input disk `/dev/vdb` and wraps
//! it in a [`DiskJobSource`]; the AWS milestone adds a lifecycle-hook source that implements the same trait over
//! the bytes it receives. Both decode through minos-core's byte-oriented codecs ([`read_input_disk`] and
//! [`GuestJob::parse_json`]), so the wire forms cannot drift between the backends.
//!
//! The host is the only writer of the input disk, but the guest validates it as hostile: core checks the header,
//! the table, every section's cap and digest, and [`InputSections::check_role`] checks that the sections are
//! exactly the job's role's, with BASE hashing to the job's base tree and DIFF to its diff blob. A failure here is a
//! host bug or a corrupted disk, an infrastructure failure, never a candidate fault.

use std::fmt;
use std::io::Read;

use minos_core::chunk::ChunkToken;
use minos_core::guest::{
    DiskError, GuestJob, GuestJobError, GuestRole, InputSections, SectionKind, read_input_disk,
};

/// One job as PID 1 runs it: the per-launch output token, the job, and the sections its role needs.
///
/// A candidate has a base tarball and, unless it is the baseline, a diff; a judge has its private data and the
/// candidate's output stream. Built only by a [`JobSource`], after every check. `Debug` shows section lengths
/// only, never their bytes, and the token stays redacted.
pub struct GuestInput {
    token: ChunkToken,
    job: GuestJob,
    base: Option<Vec<u8>>,
    diff: Option<String>,
    judge_data: Option<Vec<u8>>,
    candidate_out: Option<Vec<u8>>,
}

/// The parts of a [`GuestInput`], in field order: the token, the job, the base tarball, the diff, the judge data,
/// and the candidate output.
pub type GuestInputParts = (
    ChunkToken,
    GuestJob,
    Option<Vec<u8>>,
    Option<String>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);

impl GuestInput {
    /// The per-launch token that authenticates every output chunk. PID 1 never stores or logs it.
    #[must_use]
    pub const fn token(&self) -> &ChunkToken {
        &self.token
    }

    /// The job.
    #[must_use]
    pub const fn job(&self) -> &GuestJob {
        &self.job
    }

    /// The base tarball a candidate unpacks; `None` for a judge.
    #[must_use]
    pub fn base(&self) -> Option<&[u8]> {
        self.base.as_deref()
    }

    /// The canonical cumulative diff a candidate applies; `None` for the baseline and for a judge.
    #[must_use]
    pub fn diff(&self) -> Option<&str> {
        self.diff.as_deref()
    }

    /// The judge's private data; `None` for a candidate.
    #[must_use]
    pub fn judge_data(&self) -> Option<&[u8]> {
        self.judge_data.as_deref()
    }

    /// The candidate's output stream, which the judge scores; `None` for a candidate.
    #[must_use]
    pub fn candidate_out(&self) -> Option<&[u8]> {
        self.candidate_out.as_deref()
    }

    /// The parts, so PID 1 can drop each section's buffer once it is unpacked and return its memory to the guest
    /// reserve before the stages run.
    #[must_use]
    pub fn into_parts(self) -> GuestInputParts {
        (
            self.token,
            self.job,
            self.base,
            self.diff,
            self.judge_data,
            self.candidate_out,
        )
    }
}

impl fmt::Debug for GuestInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestInput")
            .field("token", &self.token)
            .field("job", &self.job)
            .field("base_len", &self.base.as_ref().map(Vec::len))
            .field("diff_len", &self.diff.as_ref().map(String::len))
            .field("judge_data_len", &self.judge_data.as_ref().map(Vec::len))
            .field(
                "candidate_out_len",
                &self.candidate_out.as_ref().map(Vec::len),
            )
            .finish()
    }
}

/// Where PID 1 reads its one job.
pub trait JobSource {
    /// Reads, decodes, and checks the job and its sections.
    ///
    /// # Errors
    ///
    /// A [`SourceFailure`] naming what failed, carrying the token when the source read it.
    fn read_input(&mut self) -> Result<GuestInput, SourceFailure>;
}

/// Why a [`JobSource`] could not produce a job.
///
/// When the header was read, the token is returned, so PID 1 can still send an authenticated `setup: failed
/// {input}` status the host trusts. A source failure is a host bug or a corrupt disk: an infrastructure failure,
/// never a candidate fault.
#[derive(Debug, thiserror::Error)]
#[error("the job source failed")]
pub struct SourceFailure {
    token: Option<ChunkToken>,
    #[source]
    error: SourceError,
}

impl SourceFailure {
    /// The per-launch token, if the source read it before failing.
    #[must_use]
    pub const fn token(&self) -> Option<&ChunkToken> {
        self.token.as_ref()
    }

    /// What failed.
    #[must_use]
    pub const fn error(&self) -> &SourceError {
        &self.error
    }

    /// The token and the error, so PID 1 can keep the token for its status after logging the error.
    #[must_use]
    pub fn into_parts(self) -> (Option<ChunkToken>, SourceError) {
        (self.token, self.error)
    }
}

/// What a [`JobSource`] found wrong with its input. Messages are fixed text and numbers, never input bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The input disk failed core's checks: its header, table, a section's cap or digest, or the role check.
    #[error("the input disk is not valid")]
    Disk(#[source] DiskError),
    /// The JOB section is not a valid guest job.
    #[error("the guest job is not valid")]
    Job(#[source] GuestJobError),
    /// The DIFF section is not UTF-8, so it cannot be the canonical diff.
    #[error("the diff section is not UTF-8")]
    DiffNotUtf8,
    /// A [`ReadJobSource`] was asked for its job a second time: the one job was already handed out.
    #[error("the job was already read")]
    AlreadyRead,
}

/// A [`JobSource`] over an input disk read from `reader`.
///
/// The Linux entry point opens `/dev/vdb`; tests use an in-memory reader. The reader is read once, front to back,
/// and never past the end of the last section.
#[derive(Debug)]
pub struct DiskJobSource<R: Read> {
    reader: R,
}

impl<R: Read> DiskJobSource<R> {
    /// A source that reads the input disk from `reader`.
    #[must_use]
    pub const fn new(reader: R) -> Self {
        Self { reader }
    }
}

/// A [`JobSource`] over a job that was already read: it hands out that one result once.
///
/// PID 1 reads the input disk before it calls [`run_job`](crate::run::run_job), because it needs the token for
/// the sink, the job's scratch cap for the tmpfs, and the disk closed so it can remove the device node before any
/// stage runs (05-RESEARCH Pitfall 1). This source then gives `run_job` exactly what that read produced, a
/// failure included, so a source failure that kept the token still reaches the host as an authenticated status.
/// A second call is [`SourceError::AlreadyRead`] with no token.
#[derive(Debug)]
pub struct ReadJobSource {
    read: Option<Result<GuestInput, SourceFailure>>,
}

impl ReadJobSource {
    /// A source that returns `read` on its first call.
    #[must_use]
    pub const fn new(read: Result<GuestInput, SourceFailure>) -> Self {
        Self { read: Some(read) }
    }
}

impl JobSource for ReadJobSource {
    fn read_input(&mut self) -> Result<GuestInput, SourceFailure> {
        self.read.take().unwrap_or(Err(SourceFailure {
            token: None,
            error: SourceError::AlreadyRead,
        }))
    }
}

/// The section of `kind`, or [`DiskError::MissingSection`]. The role check has already proved it present; this
/// keeps the take total without a panic path.
fn required(sections: &mut InputSections, kind: SectionKind) -> Result<Vec<u8>, SourceError> {
    sections
        .take(kind)
        .ok_or(SourceError::Disk(DiskError::MissingSection {
            kind: kind.code(),
        }))
}

impl<R: Read> JobSource for DiskJobSource<R> {
    fn read_input(&mut self) -> Result<GuestInput, SourceFailure> {
        let mut sections = read_input_disk(&mut self.reader).map_err(|error| SourceFailure {
            token: None,
            error: SourceError::Disk(error),
        })?;
        let token = sections.token().clone();
        let decoded = decode(&mut sections);
        match decoded {
            Ok((job, base, diff, judge_data, candidate_out)) => Ok(GuestInput {
                token,
                job,
                base,
                diff,
                judge_data,
                candidate_out,
            }),
            Err(error) => Err(SourceFailure {
                token: Some(token),
                error,
            }),
        }
    }
}

/// The job and the role's sections, decoded from a disk whose header and table core already checked.
type Decoded = (
    GuestJob,
    Option<Vec<u8>>,
    Option<String>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);

/// Parses the JOB section, checks the sections against the job's role, and takes the role's sections.
fn decode(sections: &mut InputSections) -> Result<Decoded, SourceError> {
    let job_bytes = required(sections, SectionKind::Job)?;
    let job = GuestJob::parse_json(&job_bytes).map_err(SourceError::Job)?;
    drop(job_bytes);
    sections.check_role(&job).map_err(SourceError::Disk)?;
    match job.role() {
        GuestRole::Candidate => {
            let base = required(sections, SectionKind::Base)?;
            let diff = sections
                .take(SectionKind::Diff)
                .map(|bytes| String::from_utf8(bytes).map_err(|_| SourceError::DiffNotUtf8))
                .transpose()?;
            Ok((job, Some(base), diff, None, None))
        }
        GuestRole::Judge => {
            let judge_data = required(sections, SectionKind::JudgeData)?;
            let candidate_out = required(sections, SectionKind::CandidateOut)?;
            Ok((job, None, None, Some(judge_data), Some(candidate_out)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Cursor;

    use minos_core::chunk::ChunkToken;
    use minos_core::guest::{DiskError, SectionKind, write_input_disk};
    use minos_core::id::BlobId;

    use super::{DiskJobSource, GuestInput, JobSource, ReadJobSource, SourceError, SourceFailure};
    use crate::test_fixture::{BASE, baseline_job, candidate_job, diff, judge_job};

    type TestResult = Result<(), Box<dyn Error>>;

    const TOKEN: [u8; 32] = [0x5a; 32];

    /// The disk of `sections` under the fixed token.
    fn disk(sections: &[(SectionKind, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut out = Vec::new();
        write_input_disk(&mut out, &ChunkToken::from_bytes(TOKEN), sections)?;
        Ok(out)
    }

    fn read(bytes: Vec<u8>) -> Result<GuestInput, SourceFailure> {
        DiskJobSource::new(Cursor::new(bytes)).read_input()
    }

    fn refused(bytes: Vec<u8>) -> Result<SourceFailure, Box<dyn Error>> {
        match read(bytes) {
            Ok(_) => Err("the disk was accepted".into()),
            Err(failure) => Ok(failure),
        }
    }

    fn has_fixed_token(failure: &SourceFailure) -> bool {
        failure.token().is_some_and(|token| token.ct_eq(&TOKEN))
    }

    #[test]
    fn disk_source_candidate() -> TestResult {
        let job = candidate_job()?;
        let json = job.to_json()?;
        let diff = diff()?;
        let bytes = disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, BASE),
            (SectionKind::Diff, diff.as_bytes()),
        ])?;

        let input = read(bytes)?;

        assert!(input.token().ct_eq(&TOKEN));
        assert_eq!(input.job(), &job);
        assert_eq!(input.base(), Some(BASE));
        assert_eq!(input.diff().map(str::as_bytes), Some(diff.as_bytes()));
        assert!(input.judge_data().is_none());
        assert!(input.candidate_out().is_none());
        let (token, parts_job, base, parts_diff, judge_data, candidate_out) = input.into_parts();
        assert!(token.ct_eq(&TOKEN));
        assert_eq!(parts_job, job);
        assert_eq!(base.as_deref(), Some(BASE));
        assert_eq!(
            parts_diff.as_deref().map(str::as_bytes),
            Some(diff.as_bytes())
        );
        assert!(judge_data.is_none() && candidate_out.is_none());
        Ok(())
    }

    #[test]
    fn disk_source_baseline_and_judge() -> TestResult {
        let baseline = baseline_job()?;
        let json = baseline.to_json()?;
        let input = read(disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, BASE),
        ])?)?;
        assert_eq!(input.job(), &baseline);
        assert_eq!(input.base(), Some(BASE));
        assert!(input.diff().is_none());

        let judge = judge_job()?;
        let json = judge.to_json()?;
        let input = read(disk(&[
            (SectionKind::Job, &json),
            (SectionKind::JudgeData, b"private judge data"),
            (SectionKind::CandidateOut, b"candidate output tar"),
        ])?)?;
        assert_eq!(input.job(), &judge);
        assert!(input.base().is_none());
        assert!(input.diff().is_none());
        assert_eq!(input.judge_data(), Some(b"private judge data".as_slice()));
        assert_eq!(
            input.candidate_out(),
            Some(b"candidate output tar".as_slice())
        );
        Ok(())
    }

    #[test]
    fn disk_source_failure_keeps_token() -> TestResult {
        let json = candidate_job()?.to_json()?;
        let diff = diff()?;
        let wrong_base = disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, b"not the job's base tree"),
            (SectionKind::Diff, diff.as_bytes()),
        ])?;
        let failure = refused(wrong_base)?;
        assert!(has_fixed_token(&failure));
        assert!(matches!(
            failure.error(),
            SourceError::Disk(DiskError::BaseMismatch)
        ));

        let mut bad_magic = disk(&[(SectionKind::Job, &json), (SectionKind::Base, BASE)])?;
        bad_magic
            .first_mut()
            .map(|byte| *byte ^= 0xff)
            .ok_or("empty disk")?;
        let failure = refused(bad_magic)?;
        assert!(failure.token().is_none());
        assert!(matches!(
            failure.error(),
            SourceError::Disk(DiskError::Magic)
        ));

        let not_a_job = disk(&[(SectionKind::Job, b"{}"), (SectionKind::Base, BASE)])?;
        let failure = refused(not_a_job)?;
        assert!(has_fixed_token(&failure));
        assert!(matches!(failure.error(), SourceError::Job(_)));

        let judge_json = judge_job()?.to_json()?;
        let judge_sees_base = disk(&[
            (SectionKind::Job, &judge_json),
            (SectionKind::Base, BASE),
            (SectionKind::JudgeData, b"data"),
            (SectionKind::CandidateOut, b"out"),
        ])?;
        let failure = refused(judge_sees_base)?;
        assert!(has_fixed_token(&failure));
        assert!(matches!(
            failure.error(),
            SourceError::Disk(DiskError::UnexpectedSection { kind: 2 })
        ));
        Ok(())
    }

    #[test]
    fn disk_source_diff_not_utf8() -> TestResult {
        let not_utf8: &[u8] = b"diff \xff\xfe";
        let mut value = serde_json::to_value(candidate_job()?)?;
        let object = value.as_object_mut().ok_or("job is not an object")?;
        object.insert(
            "diff".to_owned(),
            serde_json::to_value(BlobId::of(not_utf8))?,
        );
        let json = serde_json::to_vec(&value)?;
        let bytes = disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, BASE),
            (SectionKind::Diff, not_utf8),
        ])?;

        let failure = refused(bytes)?;

        assert!(has_fixed_token(&failure));
        assert!(matches!(failure.error(), SourceError::DiffNotUtf8));
        Ok(())
    }

    #[test]
    fn disk_source_debug_shows_no_section_bytes() -> TestResult {
        let json = baseline_job()?.to_json()?;
        let input = read(disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, BASE),
        ])?)?;
        let shown = format!("{input:?}");
        assert!(shown.contains("ChunkToken(..)"));
        assert!(shown.contains(&format!("base_len: Some({})", BASE.len())));
        // The Debug of the base bytes as a list would start with the bytes of "the".
        assert!(!shown.contains("[116, 104, 101"));
        Ok(())
    }

    #[test]
    fn read_source_hands_out_its_one_result() -> TestResult {
        let job = candidate_job()?;
        let json = job.to_json()?;
        let diff = diff()?;
        let bytes = disk(&[
            (SectionKind::Job, &json),
            (SectionKind::Base, BASE),
            (SectionKind::Diff, diff.as_bytes()),
        ])?;
        let mut source = ReadJobSource::new(read(bytes));

        let first = source.read_input()?;
        let second = source.read_input();

        assert!(first.token().ct_eq(&TOKEN));
        assert_eq!(first.job(), &job);
        let Err(failure) = second else {
            return Err("a second read returned a job".into());
        };
        assert!(failure.token().is_none());
        assert!(matches!(failure.error(), SourceError::AlreadyRead));
        Ok(())
    }

    #[test]
    fn read_source_passes_a_failure_through() -> TestResult {
        let bytes = disk(&[(SectionKind::Job, b"{not json")])?;
        let mut source = ReadJobSource::new(read(bytes));

        let Err(failure) = source.read_input() else {
            return Err("a bad job was accepted".into());
        };

        assert!(has_fixed_token(&failure));
        assert!(matches!(failure.error(), SourceError::Job(_)));
        Ok(())
    }
}
