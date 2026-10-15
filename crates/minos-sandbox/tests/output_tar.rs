//! The hostile output tar reader (SBX-05, D-16): stream checks, end-of-archive checks, fault mapping, and the
//! `Finished` wrapper a backend returns.
//!
//! Every stream here is built header by header in memory with `tar::Header`; nothing shells out to a system `tar`.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::Isolation;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::outcome::Reason;
use minos_sandbox::output::MAX_OUTPUT_STREAM_BYTES;
use minos_sandbox::{
    Candidate, Capabilities, Cleared, Ended, Finished, GuestOutput, InputDisk, Judge, JudgeData,
    Machine, OutputCap, OutputFault, OutputInvalid, Role, SandboxSpec, Untrusted, preflight,
};
use tar::Header;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Cleared plans
// ---------------------------------------------------------------------------------------------------------------

/// A baseline job: build 60 s, run 120 s, judge 90 s.
fn job() -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(60_000, 120_000, 90_000)?;
    let judge = JudgeVersion::from_str(&"ab".repeat(32))?;
    Ok(JobRequest::baseline(TreeId::of(b"t"), judge, 0, budget)?)
}

/// An arm64 Hardware host with room for every test plan and no host refusals.
fn host() -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        BTreeSet::new(),
    ))
}

/// A spec of role `R` for the baseline job.
fn spec<R: Role>() -> Result<SandboxSpec<R>, Box<dyn Error>> {
    Ok(SandboxSpec::<R>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        InputDisk::new(BlobId::of(b"input disk")),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?)
}

/// A cleared Judge plan carrying judge data.
fn judge_cleared() -> Result<Cleared<Judge>, Box<dyn Error>> {
    let plan = spec::<Judge>()?
        .secrets(JudgeData::new(BlobId::of(b"judge data")))
        .seal();
    preflight(&host()?, plan).map_err(|refusals| format!("{refusals:?}").into())
}

/// A cleared Candidate plan.
fn candidate_cleared() -> Result<Cleared<Candidate>, Box<dyn Error>> {
    preflight(&host()?, spec::<Candidate>()?.seal())
        .map_err(|refusals| format!("{refusals:?}").into())
}

/// Reads `stream` as the output of an exited Judge launch.
fn read(stream: Vec<u8>) -> Result<Result<Untrusted<GuestOutput<Judge>>, OutputFault>, Box<dyn Error>> {
    Ok(Finished::new(&judge_cleared()?, Ended::Exited, stream).into_output())
}

/// The fault `stream` is refused with, or an error when it was accepted.
fn fault(stream: Vec<u8>) -> Result<OutputFault, Box<dyn Error>> {
    match read(stream)? {
        Ok(_) => Err("the stream was accepted".into()),
        Err(fault) => Ok(fault),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Block builder
// ---------------------------------------------------------------------------------------------------------------

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// Which header format a block uses.
#[derive(Clone, Copy, Debug)]
enum Magic {
    /// POSIX ustar: magic `ustar\0`, version `00`.
    Ustar,
    /// GNU: magic `ustar `, version ` \0`.
    Gnu,
    /// Pre-POSIX v7: no magic.
    Old,
}

/// A header of `magic` named by the raw bytes `name` (copied into the 100-byte name field, so hostile names that
/// `Header::set_path` refuses still land), with typeflag `typeflag`, a size field of `size`, mode 0644, owner 0,
/// mtime 0, and a valid checksum.
fn header(magic: Magic, name: &[u8], typeflag: u8, size: u64) -> Header {
    let mut header = match magic {
        Magic::Ustar => Header::new_ustar(),
        Magic::Gnu => Header::new_gnu(),
        Magic::Old => Header::new_old(),
    };
    for (slot, byte) in header.as_old_mut().name.iter_mut().zip(name) {
        *slot = *byte;
    }
    header.as_old_mut().linkflag = [typeflag];
    header.set_size(size);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    header
}

/// One header block followed by `data`, zero-padded to a whole number of blocks. `data` need not match the
/// header's size field.
fn entry(header: &Header, data: &[u8]) -> Vec<u8> {
    let mut out = header.as_bytes().to_vec();
    out.extend_from_slice(data);
    let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
    out.resize(out.len() + padding, 0);
    out
}

/// A ustar regular-file entry named `name` holding `data`.
fn regular(name: &str, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let size = u64::try_from(data.len())?;
    Ok(entry(&header(Magic::Ustar, name.as_bytes(), b'0', size), data))
}

/// The two zero blocks that end an archive.
fn end_blocks() -> Vec<u8> {
    vec![0; 2 * BLOCK]
}

/// The scorecard a valid judge writes.
const SCORECARD: &[u8] = br#"{"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false}]}"#;
/// The judge's log.
const JUDGE_LOG: &[u8] = b"judge ok\n";
/// A judge artifact.
const RESULT: &[u8] = b"{\"ok\":true}\n";

/// A valid judge output: the scorecard, the judge log, one artifact, and two end blocks.
fn valid_judge_output() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = regular(".minos/scorecard.json", SCORECARD)?;
    out.extend(regular(".minos/judge.log", JUDGE_LOG)?);
    out.extend(regular("out/result.json", RESULT)?);
    out.extend(end_blocks());
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn valid_stream_reads_with_counts() -> TestResult {
    let output = read(valid_judge_output()?)??;
    assert_eq!(output.entry_count(), 3);
    let total = SCORECARD.len() + JUDGE_LOG.len() + RESULT.len();
    assert_eq!(output.content_bytes(), u64::try_from(total)?);
    Ok(())
}

#[test]
fn stream_over_its_cap_is_refused_before_parsing() -> TestResult {
    // One byte over the cap and not block-aligned: the length check comes first.
    let len = usize::try_from(MAX_OUTPUT_STREAM_BYTES)?.saturating_add(1);
    assert_eq!(
        fault(vec![0; len])?,
        OutputFault::TooLarge {
            cap: OutputCap::Stream
        }
    );
    Ok(())
}

#[test]
fn unaligned_stream_is_refused() -> TestResult {
    assert_eq!(
        fault(vec![0; BLOCK + 1])?,
        OutputFault::Invalid {
            why: OutputInvalid::NotBlockAligned
        }
    );
    Ok(())
}

#[test]
fn empty_archive_reads_as_no_entries() -> TestResult {
    let output = read(end_blocks())??;
    assert_eq!(output.entry_count(), 0);
    assert_eq!(output.content_bytes(), 0);
    Ok(())
}

#[test]
fn empty_slice_has_no_end_marker() -> TestResult {
    assert_eq!(
        fault(Vec::new())?,
        OutputFault::Invalid {
            why: OutputInvalid::NoEndMarker
        }
    );
    Ok(())
}

#[test]
fn faults_map_to_candidate_reasons() {
    for cap in [
        OutputCap::Stream,
        OutputCap::Entries,
        OutputCap::Entry,
        OutputCap::Total,
        OutputCap::DiffCount,
        OutputCap::DiffBytes,
    ] {
        assert_eq!(
            OutputFault::TooLarge { cap }.reason(),
            Reason::OutputTooLarge
        );
    }
    for why in [
        OutputInvalid::NotBlockAligned,
        OutputInvalid::NotUstar,
        OutputInvalid::EntryType,
        OutputInvalid::Path,
        OutputInvalid::Truncated,
        OutputInvalid::Duplicate,
        OutputInvalid::NoEndMarker,
        OutputInvalid::TrailingData,
        OutputInvalid::Malformed,
        OutputInvalid::DiffNotText,
    ] {
        assert_eq!(OutputFault::Invalid { why }.reason(), Reason::OutputInvalid);
    }
}

#[test]
fn finished_reports_its_end_and_key() -> TestResult {
    let cleared = judge_cleared()?;
    let finished = Finished::new(&cleared, Ended::TimedOut, end_blocks());
    assert_eq!(finished.ended(), Ended::TimedOut);
    assert_eq!(finished.key(), cleared.plan().key());

    let candidate = candidate_cleared()?;
    let finished = Finished::new(&candidate, Ended::Exited, valid_judge_output()?);
    assert_eq!(finished.key(), candidate.plan().key());
    assert_eq!(finished.into_output()?.entry_count(), 3);
    Ok(())
}
