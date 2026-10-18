//! The three capped exits from `Untrusted` output (SBX-04), and the judge-only scorecard path (D-14).
//!
//! Streams are built header by header in memory with `tar::Header`. The compile-fail proof that candidate output
//! has no `into_scorecard` lands with plan 04-08; here the exits are exercised for both roles.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;

use minos_core::guest::{
    ApplyStatus, GuestRole, GuestStatus, GuestStatusError, MAX_STATUS_BYTES, OutputStatus,
    SetupStatus, StageEnd, StageReport,
};
use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::{BackendKind, ConfinementLayer, Isolation};
use minos_core::limits::Limits;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::metric::{Scorecard, ScorecardError};
use minos_core::outcome::{Reason, Stage};
use minos_core::path::RelPath;
use minos_sandbox::{
    Candidate, Capabilities, Ended, Finished, GuestOutput, InputDisk, JUDGE_LOG_PATH, Judge,
    JudgeData, Machine, OutputCap, OutputFault, OutputInvalid, RUN_LOG_PATH, Role, RoleKind,
    SCORECARD_PATH, STATUS_PATH, SandboxSpec, ScorecardFault, StatusFault, Untrusted, preflight,
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

#[test]
fn status_path_is_core_s() {
    assert_eq!(STATUS_PATH, minos_core::guest::STATUS_PATH);
}
