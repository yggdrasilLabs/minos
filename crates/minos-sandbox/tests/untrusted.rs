//! The three capped exits from `Untrusted` output (SBX-04), the judge-only scorecard path (D-14), and the two sink
//! exits, `store_judge_input` on candidate output and `store_tail` on any file (07-CONTEXT D-17).
//!
//! Streams are built header by header in memory with `tar::Header`. The compile-fail proof that candidate output
//! has no `into_scorecard` lands with plan 04-08; here the exits are exercised for both roles.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io::{Cursor, Read};
use std::str::FromStr;
use std::time::Duration;

use minos_core::guest::{
    ApplyStatus, GuestRole, GuestStatus, GuestStatusError, MAX_STATUS_BYTES, OutputStatus,
    SetupStatus, SetupStep, StageEnd, StageReport,
};
use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
use minos_core::limits::{Limits, MAX_STAGE_LOG_BYTES};
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::metric::{Scorecard, ScorecardError};
use minos_core::outcome::{Reason, Stage};
use minos_core::path::RelPath;
use minos_sandbox::{
    BUILD_LOG_PATH, BlobSink, Candidate, Capabilities, Ended, Finished, GuestOutput, InputDisk,
    JUDGE_LOG_PATH, Judge, JudgeData, Machine, OutputCap, OutputFault, OutputInvalid, RUN_LOG_PATH,
    Role, RoleKind, SCORECARD_PATH, STATUS_PATH, SandboxSpec, ScorecardFault, SinkError,
    StatusFault, Untrusted, preflight,
};
use tar::Header;

type TestResult = Result<(), Box<dyn Error>>;

/// The confinement layers a v1 krun host reports: Seatbelt and rlimits, never the deferred dedicated user (D-01).
fn seatbelt() -> BTreeSet<ConfinementLayer> {
    BTreeSet::from([ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits])
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// A baseline job: build 60 s, run 120 s, judge 90 s.
fn job() -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(60_000, 120_000, 90_000)?;
    let judge = JudgeVersion::from_str(&"ab".repeat(32))?;
    Ok(JobRequest::baseline(TreeId::of(b"t"), judge, 0, budget)?)
}

/// An arm64 Hardware host with room for every test plan and no host refusals.
fn host() -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        BackendKind::Krun,
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        BTreeSet::new(),
    )
    .with_layers(seatbelt()))
}

/// The input disk of the right shape for role `R`: the baseline's candidate parts, or a judge's parts.
fn input<R: Role>() -> InputDisk {
    if R::KIND == RoleKind::Judge {
        InputDisk::judge(BlobId::of(b"guest job"), BlobId::of(b"candidate output"))
    } else {
        InputDisk::candidate(BlobId::of(b"guest job"), TreeId::of(b"t"), None)
    }
}

/// A spec of role `R` for the baseline job.
fn spec<R: Role>() -> Result<SandboxSpec<R>, Box<dyn Error>> {
    Ok(SandboxSpec::<R>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        input::<R>(),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?)
}

/// A valid output tar holding `files` in the given order, then two end blocks.
fn tar(files: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut header = Header::new_ustar();
        header.set_path(name)?;
        header.set_size(u64::try_from(data.len())?);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        out.resize(out.len() + padding, 0);
    }
    out.resize(out.len() + 2 * BLOCK, 0);
    Ok(out)
}

/// `files` read as the output of an exited Judge launch.
fn judge_output(files: &[(&str, &[u8])]) -> Result<Untrusted<GuestOutput<Judge>>, Box<dyn Error>> {
    let plan = spec::<Judge>()?
        .secrets(JudgeData::new(BlobId::of(b"judge data")))
        .seal();
    let cleared = preflight(&host()?, plan).map_err(|refusals| format!("{refusals:?}"))?;
    Ok(Finished::new(
        cleared,
        Ended::Exited,
        tar(files)?,
        Duration::ZERO,
        Vec::new(),
    )
    .into_output()?)
}

/// `files` read as the output of an exited Candidate launch.
fn candidate_output(
    files: &[(&str, &[u8])],
) -> Result<Untrusted<GuestOutput<Candidate>>, Box<dyn Error>> {
    let cleared = preflight(&host()?, spec::<Candidate>()?.seal())
        .map_err(|refusals| format!("{refusals:?}"))?;
    Ok(Finished::new(
        cleared,
        Ended::Exited,
        tar(files)?,
        Duration::ZERO,
        Vec::new(),
    )
    .into_output()?)
}

/// A valid scorecard in the wire form.
const SCORECARD: &[u8] = br#"{"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false},{"name":"identical_output","value":1,"held_out":true}]}"#;

/// The diff-text fault of `output`, or an error when it was accepted.
fn diff_fault<R: Role>(output: Untrusted<GuestOutput<R>>) -> Result<OutputFault, Box<dyn Error>> {
    match output.into_diff_texts(&Limits::standard()) {
        Ok(texts) => Err(format!("{} texts accepted", texts.len()).into()),
        Err(fault) => Ok(fault),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// into_scorecard (judge output only)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn judge_scorecard_parses_like_core() -> TestResult {
    let limits = Limits::standard();
    let output = judge_output(&[(SCORECARD_PATH, SCORECARD), ("out/x", b"artifact")])?;
    assert_eq!(
        output.into_scorecard(&limits)?,
        Scorecard::parse_json(SCORECARD, &limits)?
    );
    Ok(())
}

#[test]
fn missing_scorecard_is_refused() -> TestResult {
    let output = judge_output(&[(JUDGE_LOG_PATH, b"no scorecard\n")])?;
    let Err(fault) = output.into_scorecard(&Limits::standard()) else {
        return Err("a scorecard was produced".into());
    };
    assert!(matches!(fault, ScorecardFault::Missing));
    assert_eq!(fault.reason(), Reason::ScorecardInvalid);
    Ok(())
}

#[test]
fn oversized_scorecard_is_refused_by_core() -> TestResult {
    let big = vec![b' '; 5000];
    let output = judge_output(&[(SCORECARD_PATH, &big)])?;
    let Err(fault) = output.into_scorecard(&Limits::standard()) else {
        return Err("a scorecard was produced".into());
    };
    assert!(matches!(
        fault,
        ScorecardFault::Invalid(ScorecardError::TooLarge { bytes: 5000 })
    ));
    assert_eq!(fault.reason(), Reason::ScorecardInvalid);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// take and into_log_excerpt
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn take_projects_one_file_once() -> TestResult {
    let mut output = judge_output(&[
        (SCORECARD_PATH, SCORECARD),
        (JUDGE_LOG_PATH, b"judge ok\n"),
        ("out/x", b"artifact"),
    ])?;
    let before = output.content_bytes();
    let log = RelPath::try_from(JUDGE_LOG_PATH)?;
    let file = output.take(&log).ok_or("the log was not taken")?;
    assert!(output.take(&log).is_none());
    assert_eq!(output.entry_count(), 2);
    assert_eq!(output.content_bytes(), before - 9);
    assert_eq!(file.into_log_excerpt(4096).text().as_str(), "judge ok\n");
    assert!(output.into_scorecard(&Limits::standard()).is_ok());
    Ok(())
}

#[test]
fn log_excerpt_is_capped_and_sanitized() -> TestResult {
    let mut log = vec![b'a'; 10_000 - 16];
    log.extend_from_slice("\x1b[31m\rX\u{202E}Y\n".as_bytes());
    log.resize(10_000, b'z');
    let mut output = judge_output(&[(JUDGE_LOG_PATH, &log)])?;
    let file = output
        .take(&RelPath::try_from(JUDGE_LOG_PATH)?)
        .ok_or("the log was not taken")?;
    let excerpt = file.into_log_excerpt(4096);
    let text = excerpt.text().as_str();
    assert!(text.len() <= 4096);
    assert!(excerpt.is_truncated());
    assert!(
        text.chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\t')
    );
    assert!(!text.contains('\u{202E}'));
    assert!(text.contains("<U+202E>"));
    assert!(text.contains("[31mX<U+202E>Y"));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// into_diff_texts
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn diff_texts_in_path_order_under_diffs_only() -> TestResult {
    let output = judge_output(&[
        ("diffs/1.diff", b"second\n"),
        ("diffs/0.diff", b"first\n"),
        ("diffsx/2.diff", b"not a diff dir\n"),
        ("out/diffs/3.diff", b"nested elsewhere\n"),
        (RUN_LOG_PATH, b"log\n"),
    ])?;
    assert_eq!(
        output.into_diff_texts(&Limits::standard())?,
        vec!["first\n".to_owned(), "second\n".to_owned()]
    );
    Ok(())
}

#[test]
fn seventeen_diffs_are_too_many() -> TestResult {
    let names: Vec<String> = (0..17)
        .map(|index| format!("diffs/{index:02}.diff"))
        .collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .map(|name| (name.as_str(), b"text\n".as_slice()))
        .collect();
    assert_eq!(
        diff_fault(judge_output(&files)?)?,
        OutputFault::TooLarge {
            cap: OutputCap::DiffCount
        }
    );
    Ok(())
}

#[test]
fn sixteen_diffs_are_accepted() -> TestResult {
    let names: Vec<String> = (0..16)
        .map(|index| format!("diffs/{index:02}.diff"))
        .collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .map(|name| (name.as_str(), b"text\n".as_slice()))
        .collect();
    assert_eq!(
        judge_output(&files)?
            .into_diff_texts(&Limits::standard())?
            .len(),
        16
    );
    Ok(())
}

#[test]
fn diff_over_max_bytes_is_too_large() -> TestResult {
    let limits = Limits::standard();
    let at_cap = vec![b'a'; limits.max_diff_bytes()];
    assert_eq!(
        judge_output(&[("diffs/0.diff", &at_cap)])?
            .into_diff_texts(&limits)?
            .len(),
        1
    );
    let over = vec![b'a'; limits.max_diff_bytes() + 1];
    assert_eq!(
        diff_fault(judge_output(&[("diffs/0.diff", &over)])?)?,
        OutputFault::TooLarge {
            cap: OutputCap::DiffBytes
        }
    );
    Ok(())
}

#[test]
fn diff_that_is_not_text_is_refused() -> TestResult {
    let not_text: [&[u8]; 3] = [b"a\0b\n", b"a\r\nb\n", b"a\xff\xfeb\n"];
    for bytes in not_text {
        assert_eq!(
            diff_fault(judge_output(&[("diffs/0.diff", bytes)])?)?,
            OutputFault::Invalid {
                why: OutputInvalid::DiffNotText
            }
        );
    }
    Ok(())
}

#[test]
fn candidate_output_has_diff_and_log_exits() -> TestResult {
    let mut output = candidate_output(&[
        ("diffs/0.diff", b"first\n"),
        (RUN_LOG_PATH, b"run \x1b[2Jok\n"),
    ])?;
    let log = output
        .take(&RelPath::try_from(RUN_LOG_PATH)?)
        .ok_or("the log was not taken")?;
    assert_eq!(log.into_log_excerpt(4096).text().as_str(), "run [2Jok\n");
    assert_eq!(
        output.into_diff_texts(&Limits::standard())?,
        vec!["first\n".to_owned()]
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// take_status
// ---------------------------------------------------------------------------------------------------------------

/// A valid candidate status: the baseline built and ran, and one file was packed.
fn candidate_status() -> Result<GuestStatus, Box<dyn Error>> {
    Ok(GuestStatus::new(
        GuestRole::Candidate,
        SetupStatus::Ok,
        ApplyStatus::Baseline,
        vec![
            StageReport::new(Stage::Build, StageEnd::Exited { code: 0 }),
            StageReport::new(Stage::Run, StageEnd::Exited { code: 0 }),
        ],
        OutputStatus::Packed {
            files: 1,
            content_bytes: 2,
        },
    )?)
}

/// A valid judge status: the judge ran, and two files were packed.
fn judge_status() -> Result<GuestStatus, Box<dyn Error>> {
    Ok(GuestStatus::new(
        GuestRole::Judge,
        SetupStatus::Ok,
        ApplyStatus::NotApplicable,
        vec![StageReport::new(Stage::Judge, StageEnd::Exited { code: 0 })],
        OutputStatus::Packed {
            files: 2,
            content_bytes: 64,
        },
    )?)
}

/// The status fault `result` holds, or an error when the status was accepted.
fn status_fault(result: Result<GuestStatus, StatusFault>) -> Result<StatusFault, Box<dyn Error>> {
    match result {
        Ok(status) => Err(format!("a {:?} status was accepted", status.role()).into()),
        Err(fault) => Ok(fault),
    }
}

#[test]
fn take_status_valid() -> TestResult {
    // Arrange
    let status = candidate_status()?;
    let json = status.to_json()?;
    let mut output = candidate_output(&[(STATUS_PATH, &json), ("out/r", b"{}")])?;

    // Act
    let taken = output.take_status()?;

    // Assert: the status is the one written, and the file is gone from the output.
    assert_eq!(taken, status);
    assert_eq!(output.entry_count(), 1);
    assert!(output.take(&RelPath::try_from(STATUS_PATH)?).is_none());
    assert_eq!(
        status_fault(output.take_status())?,
        StatusFault::Missing,
        "a second take_status found a status"
    );
    Ok(())
}

#[test]
fn take_status_valid_for_judge_output() -> TestResult {
    // Arrange
    let status = judge_status()?;
    let mut output = judge_output(&[
        (STATUS_PATH, &status.to_json()?),
        (SCORECARD_PATH, SCORECARD),
    ])?;

    // Act
    let taken = output.take_status()?;

    // Assert: the judge's other exits still work after the status is taken.
    assert_eq!(taken, status);
    assert_eq!(
        output.into_scorecard(&Limits::standard())?,
        Scorecard::parse_json(SCORECARD, &Limits::standard())?
    );
    Ok(())
}

#[test]
fn take_status_refusals() -> TestResult {
    // Missing.
    let mut output = candidate_output(&[("out/r", b"{}")])?;
    let missing = status_fault(output.take_status())?;
    assert_eq!(missing, StatusFault::Missing);
    assert_eq!(output.entry_count(), 1, "a missing status removed a file");

    // Not the status wire form.
    let mut output = candidate_output(&[(STATUS_PATH, b"{not json")])?;
    let invalid = status_fault(output.take_status())?;
    assert!(
        matches!(invalid, StatusFault::Invalid(GuestStatusError::Json { .. })),
        "bad JSON was not refused as invalid"
    );

    // Over the byte cap: a valid status padded past MAX_STATUS_BYTES is refused before parsing.
    let mut padded = candidate_status()?.to_json()?;
    padded.resize(MAX_STATUS_BYTES + 1, b' ');
    let mut output = candidate_output(&[(STATUS_PATH, &padded)])?;
    let too_large = status_fault(output.take_status())?;
    assert_eq!(too_large, StatusFault::Invalid(GuestStatusError::TooLarge));

    // A judge status in candidate output, and a candidate status in judge output.
    let mut output = candidate_output(&[(STATUS_PATH, &judge_status()?.to_json()?)])?;
    let wrong_role = status_fault(output.take_status())?;
    assert_eq!(wrong_role, StatusFault::WrongRole);
    let mut output = judge_output(&[(STATUS_PATH, &candidate_status()?.to_json()?)])?;
    assert_eq!(status_fault(output.take_status())?, StatusFault::WrongRole);

    // Every fault is the same ledger reason.
    for fault in [missing, invalid, too_large, wrong_role] {
        assert_eq!(fault.reason(), Reason::OutputInvalid);
    }
    Ok(())
}

/// A status whose setup failed at `step` under `role`, with the apply status core requires for that role.
fn setup_failed_status(role: GuestRole, step: SetupStep) -> Result<GuestStatus, Box<dyn Error>> {
    let apply = match role {
        GuestRole::Candidate => ApplyStatus::Baseline,
        GuestRole::Judge => ApplyStatus::NotApplicable,
    };
    Ok(GuestStatus::new(
        role,
        SetupStatus::Failed { step },
        apply,
        Vec::new(),
        OutputStatus::NotPacked,
    )?)
}

#[test]
fn take_status_input_failure_is_accepted_whatever_role_it_names() -> TestResult {
    // PID 1 could not parse its job on a judge launch, so it sent the placeholder candidate role (review CR-02).
    let placeholder = setup_failed_status(GuestRole::Candidate, SetupStep::Input)?;
    let mut output = judge_output(&[(STATUS_PATH, &placeholder.to_json()?)])?;
    let taken = output.take_status()?;
    assert_eq!(taken, placeholder);
    assert_eq!(
        taken.setup(),
        SetupStatus::Failed {
            step: SetupStep::Input
        },
        "the caller reads setup first and maps it to an infrastructure failure"
    );

    // The same input failure the other way round is accepted too.
    let judge_placeholder = setup_failed_status(GuestRole::Judge, SetupStep::Input)?;
    let mut output = candidate_output(&[(STATUS_PATH, &judge_placeholder.to_json()?)])?;
    assert_eq!(output.take_status()?, judge_placeholder);

    // A later setup step is past the role: the role is known by then, and a mismatch is still refused.
    for step in [
        SetupStep::Mount,
        SetupStep::Unpack,
        SetupStep::Apply,
        SetupStep::Output,
    ] {
        let later = setup_failed_status(GuestRole::Candidate, step)?;
        let mut output = judge_output(&[(STATUS_PATH, &later.to_json()?)])?;
        assert_eq!(
            status_fault(output.take_status())?,
            StatusFault::WrongRole,
            "{step:?}"
        );
    }
    Ok(())
}

#[test]
fn status_path_is_core_s() {
    assert_eq!(STATUS_PATH, minos_core::guest::STATUS_PATH);
}

// ---------------------------------------------------------------------------------------------------------------
// Sink exits: store_judge_input (candidate output only) and store_tail (any file)
// ---------------------------------------------------------------------------------------------------------------

/// The store error a failing [`Recorder`] returns.
#[derive(Debug, PartialEq, Eq)]
struct SinkFailed;

impl fmt::Display for SinkFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the test sink failed")
    }
}

impl Error for SinkFailed {}

/// What a [`Recorder`] answers to a put.
#[derive(Clone, Copy)]
enum Answer {
    /// The bytes' own digest, as an honest store does.
    Honest,
    /// Another blob's ID.
    WrongId,
    /// A store failure.
    Fail,
}

/// A sink that records every put and answers as told.
struct Recorder {
    answer: Answer,
    puts: RefCell<Vec<Vec<u8>>>,
}

impl Recorder {
    fn new(answer: Answer) -> Self {
        Self {
            answer,
            puts: RefCell::new(Vec::new()),
        }
    }

    /// The one put the exit made, or an error when it made none or several.
    fn only_put(self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut puts = self.puts.into_inner();
        match (puts.pop(), puts.is_empty()) {
            (Some(bytes), true) => Ok(bytes),
            (None, _) => Err("the sink was never called".into()),
            (Some(_), false) => Err(format!("the sink was called {} times", puts.len() + 1).into()),
        }
    }
}

impl BlobSink for Recorder {
    type Error = SinkFailed;

    fn put(&self, bytes: &[u8]) -> Result<BlobId, SinkFailed> {
        self.puts.borrow_mut().push(bytes.to_vec());
        match self.answer {
            Answer::Honest => Ok(BlobId::of(bytes)),
            Answer::WrongId => Ok(BlobId::of(b"some other blob")),
            Answer::Fail => Err(SinkFailed),
        }
    }
}

/// One entry of a re-packed archive as the tar crate reads it back: path, bytes, mode, uid, gid, mtime, user
/// name, group name, and whether it is a regular file.
type ReadEntry = (String, Vec<u8>, u32, u64, u64, u64, Vec<u8>, Vec<u8>, bool);

/// The entries of `archive`, read with the tar crate in memory (never unpacked).
fn read_back(archive: &[u8]) -> Result<Vec<ReadEntry>, Box<dyn Error>> {
    let mut entries = Vec::new();
    for entry in tar::Archive::new(Cursor::new(archive)).entries()? {
        let mut entry = entry?;
        let header = entry.header().clone();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        entries.push((
            header.path()?.to_string_lossy().into_owned(),
            bytes,
            header.mode()?,
            header.uid()?,
            header.gid()?,
            header.mtime()?,
            header.username_bytes().ok_or("no user name field")?.to_vec(),
            header.groupname_bytes().ok_or("no group name field")?.to_vec(),
            header.entry_type() == tar::EntryType::Regular,
        ));
    }
    Ok(entries)
}

/// The size `archive` must have: one header block and the padded data per entry, then the two end blocks.
fn archive_len(entries: &[ReadEntry]) -> usize {
    entries
        .iter()
        .map(|(_, bytes, ..)| BLOCK + bytes.len().div_ceil(BLOCK) * BLOCK)
        .sum::<usize>()
        + 2 * BLOCK
}

/// A candidate output holding a status, logs, a diff, a scorecard-shaped file, two artifacts, and names that only
/// look like `out/`.
fn mixed_candidate_output() -> Result<Untrusted<GuestOutput<Candidate>>, Box<dyn Error>> {
    candidate_output(&[
        (STATUS_PATH, b"{}"),
        (BUILD_LOG_PATH, b"build log"),
        (RUN_LOG_PATH, b"run log"),
        ("diffs/a.diff", b"--- a/x\n"),
        ("out/b.txt", b"hello, judge\n"),
        ("out/a/c.bin", b"\x00\x01\x02binary\n"),
        ("outx/d", b"not an artifact"),
        ("out", b"a file named out"),
        ("scorecard.json", SCORECARD),
        (SCORECARD_PATH, SCORECARD),
    ])
}

#[test]
fn judge_input_keeps_only_out_files_in_path_order_with_canonical_headers() -> TestResult {
    let sink = Recorder::new(Answer::Honest);
    let id = mixed_candidate_output()?.store_judge_input(&sink)?;
    let archive = sink.only_put()?;
    assert_eq!(id, BlobId::of(&archive));
    let entries = read_back(&archive)?;
    let empty = Vec::new();
    assert_eq!(
        entries,
        vec![
            (
                "out/a/c.bin".to_owned(),
                b"\x00\x01\x02binary\n".to_vec(),
                0o644,
                0,
                0,
                0,
                empty.clone(),
                empty.clone(),
                true,
            ),
            (
                "out/b.txt".to_owned(),
                b"hello, judge\n".to_vec(),
                0o644,
                0,
                0,
                0,
                empty.clone(),
                empty,
                true,
            ),
        ]
    );
    // Two zero end blocks and nothing after them.
    assert_eq!(archive.len(), archive_len(&entries));
    let (_, end) = archive
        .split_at_checked(archive.len() - 2 * BLOCK)
        .ok_or("archive shorter than its end blocks")?;
    assert!(end.iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn judge_input_repack_is_pinned() -> TestResult {
    // Golden: computed outside the code under test by a Python script that writes each ustar header field by
    // field (name; mode, uid, gid, size, and mtime as zero-padded octal ending in a NUL; typeflag `0`; magic
    // `ustar\0`, version `00`; zero user, group, device, and prefix fields; the checksum over the block with the
    // checksum field as eight spaces, written as seven octal digits and a NUL), pads each file to 512 bytes,
    // appends 1024 zero bytes, and hashes the 3072 bytes with SHA-256.
    let sink = Recorder::new(Answer::Honest);
    let id = mixed_candidate_output()?.store_judge_input(&sink)?;
    assert_eq!(
        id.to_string(),
        "2330b3195cf77d69a0c71abd7e955f90ce7964a7086632f22ff895c18c87aa5f"
    );
    assert_eq!(sink.only_put()?.len(), 3072);
    Ok(())
}

#[test]
fn judge_input_without_artifacts_is_the_empty_archive() -> TestResult {
    let sink = Recorder::new(Answer::Honest);
    let output = candidate_output(&[(STATUS_PATH, b"{}"), (RUN_LOG_PATH, b"log")])?;
    let id = output.store_judge_input(&sink)?;
    assert_eq!(sink.only_put()?, vec![0; 2 * BLOCK]);
    // Golden: the SHA-256 of 1024 zero bytes, computed with Python's hashlib.
    assert_eq!(
        id.to_string(),
        "5f70bf18a086007016e948b04aed3b82103a36bea41755b6cddfaf10ace3c6ef"
    );
    Ok(())
}

#[test]
fn judge_input_refuses_a_sink_that_names_another_digest() -> TestResult {
    let sink = Recorder::new(Answer::WrongId);
    let result = mixed_candidate_output()?.store_judge_input(&sink);
    assert!(matches!(result, Err(SinkError::DigestMismatch)));
    sink.only_put()?;
    Ok(())
}

#[test]
fn judge_input_passes_on_the_store_error() -> TestResult {
    let sink = Recorder::new(Answer::Fail);
    let result = mixed_candidate_output()?.store_judge_input(&sink);
    assert!(matches!(result, Err(SinkError::Store(SinkFailed))));
    sink.only_put()?;
    Ok(())
}

/// `bytes` as the run log of a candidate output, projected with `take`.
fn run_log(bytes: &[u8]) -> Result<Untrusted<minos_sandbox::OutputFile<Candidate>>, Box<dyn Error>> {
    candidate_output(&[(RUN_LOG_PATH, bytes)])?
        .take(&RelPath::try_from(RUN_LOG_PATH)?)
        .ok_or_else(|| "the log was not taken".into())
}

/// The bytes `store_tail(max, ..)` handed an honest sink for `log`, checked against the returned ID.
fn stored_tail<R: Role>(
    log: Untrusted<minos_sandbox::OutputFile<R>>,
    max: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let sink = Recorder::new(Answer::Honest);
    let id = log.store_tail(max, &sink)?;
    let bytes = sink.only_put()?;
    assert_eq!(id, BlobId::of(&bytes));
    Ok(bytes)
}

#[test]
fn tail_keeps_the_last_max_bytes() -> TestResult {
    let log: Vec<u8> = (0..100).collect();
    assert_eq!(stored_tail(run_log(&log)?, 10)?, (90..100).collect::<Vec<u8>>());
    Ok(())
}

#[test]
fn tail_of_a_short_log_is_the_whole_log() -> TestResult {
    assert_eq!(stored_tail(run_log(b"short")?, 10)?, b"short".to_vec());
    Ok(())
}

#[test]
fn tail_is_clamped_to_the_stage_log_cap() -> TestResult {
    let log: Vec<u8> = (0..MAX_STAGE_LOG_BYTES + 7)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    let tail = stored_tail(run_log(&log)?, usize::MAX)?;
    assert_eq!(tail.len(), MAX_STAGE_LOG_BYTES);
    assert_eq!(tail.as_slice(), log.get(7..).ok_or("short log")?);
    Ok(())
}

#[test]
fn tail_bytes_are_not_sanitized() -> TestResult {
    let log = b"ok\n\x1b[31mred\x1b[0m\x00\x07\r\xff\xfe\xe2\x80\xae";
    assert_eq!(stored_tail(run_log(log)?, 1024)?, log.to_vec());
    Ok(())
}

#[test]
fn tail_works_for_judge_output_too() -> TestResult {
    let log = judge_output(&[(JUDGE_LOG_PATH, b"judge log")])?
        .take(&RelPath::try_from(JUDGE_LOG_PATH)?)
        .ok_or("the log was not taken")?;
    assert_eq!(stored_tail(log, 5)?, b"e log".to_vec());
    Ok(())
}

#[test]
fn tail_refuses_a_wrong_digest_and_passes_on_a_store_error() -> TestResult {
    let sink = Recorder::new(Answer::WrongId);
    assert!(matches!(
        run_log(b"log")?.store_tail(10, &sink),
        Err(SinkError::DigestMismatch)
    ));
    sink.only_put()?;
    let sink = Recorder::new(Answer::Fail);
    assert!(matches!(
        run_log(b"log")?.store_tail(10, &sink),
        Err(SinkError::Store(SinkFailed))
    ));
    sink.only_put()?;
    Ok(())
}
