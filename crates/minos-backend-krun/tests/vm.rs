//! Functional VM tests (plan 05-17; GUEST-01 to GUEST-04, KRUN-04, KRUN-10; 05-CONTEXT D-05, D-14).
//!
//! Each test boots the static musl `minos-guest` as PID 1 from a root image in the `.minos/blobs` layout, with a
//! per-job read-only input disk, under the embedded Seatbelt profile, through [`harness::vm`]'s production
//! `KrunBackend` and the sandbox types end to end. `minos-probe` plays the candidate (or the judge) and reports
//! what it could and could not do as JSON, which the tests read only through the sanitizing exits.
//!
//! Every test is ignored: it needs a signed `MINOS_VMM`, `MINOS_GUEST` with `minos-probe` beside it, and
//! `MINOS_KRUNFW` (see [`harness::SETUP`]). Run:
//! `cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1`.

mod harness;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use minos_backend_krun::host::{HostPaths, probe_host};
use minos_core::guest::{ApplyStatus, GuestRole, OutputStatus, SetupStatus, StageEnd};
use minos_core::ledger::ConfinementLayer;
use minos_core::limits::Limits;
use minos_core::manifest::Manifest;
use minos_core::metric::Metric;
use minos_core::outcome::Stage;
use minos_core::path::RelPath;
use minos_sandbox::{Ended, SCORECARD_PATH};
use serde_json::Value;

use harness::{
    PROBE, PROBE_NAME, PROBE_SETUID, PROBE_SETUID_MODE, TestResult, VALUE_PATH, base_tar,
    candidate, diff_text, drop_logs, env_path, exited, judge, manifest, metric, probe_json,
    stage_end, vm,
};

/// Bytes in a MiB.
const MIB: u64 = 1_048_576;
/// The candidate uid and gid (`minos_guest::stage::GUEST_UID`).
const GUEST_ID: u64 = 64_000;
/// `RLIMIT_NPROC` of every stage (`minos_guest::stage::MAX_PROCESSES`).
const MAX_PROCESSES: u64 = 512;
/// `RLIMIT_NOFILE` of every stage (`minos_guest::stage::MAX_OPEN_FILES`).
const MAX_OPEN_FILES: u64 = 4096;
/// `ENOENT`.
const ENOENT: i64 = 2;
/// The default VM memory and scratch cap (`minos_core::limits`).
const MEMORY_MIB: u16 = 2048;
/// The default scratch cap.
const SCRATCH_MIB: u16 = 1024;
/// Build, run, and judge budgets: one minute each.
const BUDGETS: (u64, u64, u64) = (60_000, 60_000, 60_000);
/// The judge argv: bare words, so the manifest's judge-path rule holds.
const JUDGE: [&str; 2] = [PROBE_NAME, "scorecard"];
/// A stage that does nothing and succeeds.
const NOTHING: [&str; 3] = [PROBE, "exit", "0"];

/// The base tree every candidate starts from: the writable file and a build directory.
fn base() -> TestResult<Vec<u8>> {
    base_tar(&[
        (VALUE_PATH, b"old\n", 0o644),
        ("target/keep.txt", b"", 0o644),
    ])
}

/// A manifest with the default memory and budgets.
fn simple(build: &[&str], run: &[&str]) -> TestResult<Manifest> {
    manifest(build, run, &JUDGE, MEMORY_MIB, SCRATCH_MIB, BUDGETS)
}

/// The field `key` of a probe result.
fn field<'a>(value: &'a Value, key: &str) -> TestResult<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| format!("the probe result has no {key}").into())
}

/// The unsigned integer field `key`.
fn number(value: &Value, key: &str) -> TestResult<u64> {
    field(value, key)?
        .as_u64()
        .ok_or_else(|| format!("{key} is not an unsigned integer").into())
}

/// The boolean field `key`.
fn flag(value: &Value, key: &str) -> TestResult<bool> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| format!("{key} is not a boolean").into())
}

/// The array field `key`.
fn list<'a>(value: &'a Value, key: &str) -> TestResult<&'a [Value]> {
    Ok(field(value, key)?
        .as_array()
        .ok_or_else(|| format!("{key} is not an array"))?
        .as_slice())
}

/// One probe attempt: its target, whether it worked, and its OS error.
fn attempts(value: &Value, key: &str) -> TestResult<Vec<(String, bool, Option<i64>)>> {
    list(value, key)?
        .iter()
        .map(|attempt| {
            let target = field(attempt, "target")?
                .as_str()
                .ok_or("target is not a string")?
                .to_owned();
            Ok((
                target,
                flag(attempt, "ok")?,
                field(attempt, "os_error")?.as_i64(),
            ))
        })
        .collect()
}

/// The soft and hard values of the `/proc/self/limits` row `row` the `env` probe reported.
fn limit(env: &Value, row: &str) -> TestResult<(String, String)> {
    for line in list(env, "limits")? {
        let line = line.as_str().ok_or("a limits row is not a string")?;
        if let Some(rest) = line.strip_prefix(row) {
            let mut words = rest.split_whitespace();
            let soft = words.next().ok_or("a limits row has no soft value")?;
            let hard = words.next().ok_or("a limits row has no hard value")?;
            return Ok((soft.to_owned(), hard.to_owned()));
        }
    }
    Err(format!("the env probe reported no {row} row").into())
}

/// Asserts both values of limit `row` are `expected`.
fn assert_limit(env: &Value, row: &str, expected: u64) -> TestResult {
    let (soft, hard) = limit(env, row)?;
    assert_eq!(
        (soft.as_str(), hard.as_str()),
        (expected.to_string().as_str(), expected.to_string().as_str()),
        "{row}"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn experiment_applies_diff_and_runs_as_candidate_uid() -> TestResult {
    let vm = vm()?;
    let manifest = simple(
        &[PROBE, "env", "/out/build.json"],
        &[
            PROBE,
            "marker-check",
            "/scratch/src/src/value.txt",
            "/out/run.json",
        ],
    )?;
    let diff = diff_text(VALUE_PATH, "old", "new");
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, Some(&diff))?)?;
    assert_eq!(status.role(), GuestRole::Candidate);
    assert_eq!(status.setup(), SetupStatus::Ok);
    assert_eq!(status.apply(), &ApplyStatus::Applied);
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );

    let env = probe_json(&mut output, "out/build.json")?;
    for key in ["uid", "euid", "gid", "egid"] {
        assert_eq!(number(&env, key)?, GUEST_ID, "{key}");
    }
    assert!(list(&env, "groups")?.is_empty(), "supplementary groups");
    assert_eq!(field(&env, "cwd")?.as_str(), Some("/scratch/src"));
    let environment = manifest.environment();
    let build_seconds = manifest.budget().build_ms().div_ceil(1000) + 1;
    assert_limit(
        &env,
        "Max address space",
        u64::from(environment.candidate_allowance_mib()) * MIB,
    )?;
    assert_limit(
        &env,
        "Max file size",
        u64::from(environment.scratch_mib()) * MIB,
    )?;
    assert_limit(&env, "Max processes", MAX_PROCESSES)?;
    assert_limit(&env, "Max cpu time", build_seconds)?;
    assert_limit(&env, "Max open files", MAX_OPEN_FILES)?;
    assert_limit(&env, "Max core file size", 0)?;

    let run = probe_json(&mut output, "out/run.json")?;
    assert!(flag(&run, "exists")?, "the run stage sees the applied file");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn setuid_root_binary_in_the_image_does_not_raise_the_stage_uid() -> TestResult {
    // The test image ships the probe a second time, root-owned and mode 04755, as a stock base image ships `su`
    // or `mount`. The packer keeps the bit; the root is mounted nosuid, so the bit is inert (review WR-01).
    let vm = vm()?;
    let manifest = simple(&[PROBE_SETUID, "env", "/out/build.json"], &NOTHING)?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(status.setup(), SetupStatus::Ok);
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );

    let env = probe_json(&mut output, "out/build.json")?;
    // The bit survived packing, so the exec really was of a setuid-root file.
    assert_eq!(
        number(&env, "exe_mode")?,
        u64::from(PROBE_SETUID_MODE),
        "the setuid bit of {PROBE_SETUID} in the image"
    );
    // And it changed nothing: every id is still the stage's.
    for key in ["uid", "euid", "gid", "egid"] {
        assert_eq!(number(&env, key)?, GUEST_ID, "{key}");
    }
    assert!(list(&env, "groups")?.is_empty(), "supplementary groups");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn apply_mismatch_is_rejected_and_runs_nothing() -> TestResult {
    let vm = vm()?;
    let manifest = simple(&[PROBE, "env", "/out/build.json"], &NOTHING)?;
    // The context line `other` is not in the base, whose one line is `old`.
    let diff = diff_text(VALUE_PATH, "other", "new");
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, Some(&diff))?)?;
    assert_eq!(
        status.apply(),
        &ApplyStatus::Failed {
            path: RelPath::try_from(VALUE_PATH)?,
            hunk: 1,
        }
    );
    assert_eq!(stage_end(&status, Stage::Build)?, StageEnd::Skipped);
    assert_eq!(stage_end(&status, Stage::Run)?, StageEnd::Skipped);
    assert!(
        output.take(&RelPath::try_from("out/build.json")?).is_none(),
        "no stage ran"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn writes_outside_scratch_fail() -> TestResult {
    let vm = vm()?;
    let manifest = simple(&NOTHING, &[PROBE, "write-outside", "/out/w.json"])?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    let result = probe_json(&mut output, "out/w.json")?;
    let outside = attempts(&result, "outside")?;
    assert_eq!(outside.len(), 6);
    for (target, ok, os_error) in &outside {
        assert!(!ok, "{target} was writable");
        assert!(os_error.is_some(), "{target} failed with no OS error");
    }
    let reads = attempts(&result, "reads")?;
    assert_eq!(
        reads
            .iter()
            .map(|(target, ..)| target.as_str())
            .collect::<Vec<_>>(),
        ["/proc/1/environ", "/dev/vdb"]
    );
    for (target, ok, os_error) in &reads {
        assert!(!ok, "{target} could be opened");
        assert!(os_error.is_some(), "{target} failed with no OS error");
    }
    let inside = attempts(&result, "inside")?;
    assert_eq!(inside.len(), 2);
    for (target, ok, _) in &inside {
        assert!(ok, "{target} was not writable");
    }
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn over_budget_candidates_are_stopped_in_the_vm() -> TestResult {
    let vm = vm()?;
    let probe_manifest = simple(&NOTHING, &NOTHING)?;
    let allowance = u64::from(probe_manifest.environment().candidate_allowance_mib());
    let request = allowance + 64;
    let request_text = request.to_string();

    // Memory: RLIMIT_AS at the allowance refuses an allocation past it.
    let manifest = simple(&NOTHING, &[PROBE, "alloc", "/out/a.json", &request_text])?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    let alloc = probe_json(&mut output, "out/a.json")?;
    let allocated = number(&alloc, "allocated_mib")?;
    println!(
        "== RLIMIT_AS allowance {allowance} MiB: the probe allocated {allocated} of {request} MiB"
    );
    assert_eq!(number(&alloc, "requested_mib")?, request);
    assert!(
        allocated < request,
        "allocated {allocated} of {request} MiB"
    );
    assert!(
        allocated < allowance,
        "allocated {allocated} MiB under a {allowance} MiB allowance"
    );

    // Processes: RLIMIT_NPROC at 512 for the candidate uid.
    let manifest = simple(&NOTHING, &[PROBE, "spawn", "/out/s.json", "2000"])?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    let spawn = probe_json(&mut output, "out/s.json")?;
    let started = number(&spawn, "started")?;
    println!("== RLIMIT_NPROC {MAX_PROCESSES}: the probe started {started} of 2000 children");
    assert!(started < MAX_PROCESSES + 1, "started {started} children");
    assert!(field(&spawn, "os_error")?.as_i64().is_some());

    // Scratch: the tmpfs cap stops a write at or below it.
    let scratch_mib: u16 = 128;
    let manifest = harness::manifest(
        &NOTHING,
        &[PROBE, "fill-scratch", "/out/f.json"],
        &JUDGE,
        MEMORY_MIB,
        scratch_mib,
        BUDGETS,
    )?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    let fill = probe_json(&mut output, "out/f.json")?;
    let written = number(&fill, "bytes_written")?;
    println!("== scratch {scratch_mib} MiB: the probe wrote {written} bytes before a write failed");
    assert!(written <= u64::from(scratch_mib) * MIB);
    assert!(field(&fill, "os_error")?.as_i64().is_some());
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn two_sequential_experiments_share_no_build_output() -> TestResult {
    let vm = vm()?;
    let marker = "/scratch/src/target/marker";
    let first = simple(
        &[PROBE, "marker-write", marker],
        &[PROBE, "marker-check", marker, "/out/m.json"],
    )?;
    let (mut output, status) = exited(candidate(&vm, &first, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );
    assert!(
        flag(&probe_json(&mut output, "out/m.json")?, "exists")?,
        "the first experiment's own run sees its build output"
    );

    let second = simple(&[PROBE, "marker-check", marker, "/out/m.json"], &NOTHING)?;
    let (mut output, status) = exited(candidate(&vm, &second, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );
    assert!(
        !flag(&probe_json(&mut output, "out/m.json")?, "exists")?,
        "the second experiment sees the first one's build output"
    );
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn output_16_mib_is_reassembled() -> TestResult {
    let vm = vm()?;
    let manifest = simple(&NOTHING, &[PROBE, "output-fill", "/out", "16000000"])?;
    let finished = candidate(&vm, &manifest, &base()?, None)?;
    let elapsed = finished.elapsed();
    let (mut output, status) = exited(finished)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    println!("== output status {:?}", status.output());
    assert!(matches!(status.output(), OutputStatus::Packed { .. }));
    drop_logs(&mut output)?;
    println!(
        "== 16 MiB output: {} files, {} content bytes, launch elapsed {elapsed:?}",
        output.entry_count(),
        output.content_bytes()
    );
    // W0-CHUNKS's selection bound for the whole 16 MiB stream: a helper whose open-file limit is below one vsock
    // reaper window of connections stalls about 5 s per thousand connections and breaks it (05-17 evidence).
    assert!(
        elapsed < Duration::from_secs(15),
        "a 16 MiB output took {elapsed:?}"
    );
    assert_eq!(output.content_bytes(), 16_000_000);
    assert_eq!(output.entry_count(), 2);
    for name in ["out/fill-0000", "out/fill-0001"] {
        assert!(output.take(&RelPath::try_from(name)?).is_some(), "{name}");
    }
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn output_over_cap_is_refused_by_the_guest() -> TestResult {
    let vm = vm()?;
    let manifest = simple(&NOTHING, &[PROBE, "output-fill", "/out", "17000000"])?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    assert_eq!(status.output(), OutputStatus::TooLarge);
    drop_logs(&mut output)?;
    assert_eq!(output.entry_count(), 0, "no out/ entry");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn judge_role_returns_a_scorecard() -> TestResult {
    let vm = vm()?;
    let manifest = simple(&NOTHING, &NOTHING)?;
    let judge_data = base_tar(&[("expected.txt", b"1\n", 0o644)])?;
    let candidate_out = base_tar(&[("out/result.json", b"{}\n", 0o644)])?;
    let (output, status) = exited(judge(&vm, &manifest, &judge_data, &candidate_out)?)?;
    assert_eq!(status.role(), GuestRole::Judge);
    assert_eq!(status.apply(), &ApplyStatus::NotApplicable);
    assert_eq!(
        stage_end(&status, Stage::Judge)?,
        StageEnd::Exited { code: 0 }
    );
    let scorecard = output.into_scorecard(&Limits::standard())?;
    assert_eq!(metric(&scorecard, "probe_ok")?, Metric::ONE);
    assert_eq!(scorecard.len(), 1);
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn candidate_output_never_carries_a_scorecard() -> TestResult {
    let vm = vm()?;
    let manifest = simple(
        &[PROBE, "write-outside", "/out/w.json"],
        &[PROBE, "scorecard"],
    )?;
    let (mut output, status) = exited(candidate(&vm, &manifest, &base()?, None)?)?;
    assert_eq!(
        stage_end(&status, Stage::Run)?,
        StageEnd::Exited { code: 0 }
    );
    assert!(
        output.take(&RelPath::try_from(SCORECARD_PATH)?).is_none(),
        "a candidate's output carries {SCORECARD_PATH}"
    );
    assert!(
        output
            .take(&RelPath::try_from("out/scorecard.json")?)
            .is_some(),
        "the candidate's /out/scorecard.json stays an ordinary artifact"
    );
    let result = probe_json(&mut output, "out/w.json")?;
    for dir in ["/scratch/judge", "/scratch/candidate"] {
        let target = format!("{dir}/minos-probe-write");
        let attempt = attempts(&result, "outside")?
            .into_iter()
            .find(|(name, ..)| *name == target)
            .ok_or_else(|| format!("no attempt at {target}"))?;
        assert_eq!(
            (attempt.1, attempt.2),
            (false, Some(ENOENT)),
            "{dir} exists in a candidate VM"
        );
    }
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST (with minos-probe beside it), MINOS_KRUNFW; see harness::SETUP"]
fn stage_timeout_is_reported_by_the_guest() -> TestResult {
    let vm = vm()?;
    let manifest = harness::manifest(
        &NOTHING,
        &[PROBE, "hang"],
        &JUDGE,
        MEMORY_MIB,
        SCRATCH_MIB,
        (60_000, 2_000, 60_000),
    )?;
    let finished = candidate(&vm, &manifest, &base()?, None)?;
    assert_eq!(
        finished.ended(),
        Ended::Exited,
        "PID 1's deadline, not the host's"
    );
    let elapsed = finished.elapsed();
    let (_, status) = exited(finished)?;
    assert_eq!(
        stage_end(&status, Stage::Build)?,
        StageEnd::Exited { code: 0 }
    );
    assert_eq!(stage_end(&status, Stage::Run)?, StageEnd::TimedOut);
    println!("== a 2 s run budget: the launch took {elapsed:?}");
    Ok(())
}

/// Runs every host probe on this Mac (moved here from `host.rs`'s unit tests by 08-15, so no file under `src/`
/// names the test variables). Build first: `cargo build -p minos-vmm`, sign it with the CLAUDE.md codesign argv,
/// and `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`.
#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW; see harness::SETUP"]
fn probe_host_on_this_mac() -> TestResult {
    let paths = HostPaths::new(
        env_path("MINOS_VMM")?,
        env_path("MINOS_GUEST")?,
        env_path("MINOS_KRUNFW")?,
    );
    let host = probe_host(&paths, Path::new("/private/tmp"));
    for probe in host.probes() {
        println!(
            "{:<32} {:?} {:?}",
            probe.check().name(),
            probe.paths(),
            probe.outcome()
        );
    }
    let caps = host.capabilities();
    assert!(
        caps.host_refusals().is_empty(),
        "refusals: {:?}",
        caps.host_refusals()
    );
    assert_eq!(
        caps.layers(),
        &BTreeSet::from([ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits])
    );
    assert!(caps.kernel().is_some());
    assert_eq!(host.probes().len(), 9);
    Ok(())
}
