---
phase: 05-guest-krun-backend-and-environment-build
plan: 08
subsystem: minos-guest
tags: [minos-guest, job-source, output-sink, chunk-sink, trampoline, rlimits, d-05, d-07, d-08, guest-02, guest-03, guest-04]
status: complete
requires:
  - "05-02: minos_core::chunk (ChunkToken, ChunkStream, Reassembler) and the 4144-byte connection cap"
  - "05-04: minos_core::guest (read_input_disk, InputSections::check_role, GuestJob, StageSpec, SectionKind, DiskError)"
  - "05-01: the D-05 memory budget (GuestJob::allowance_mib, scratch_mib) and MAX_MEMORY_MIB, MAX_SCRATCH_MIB, MAX_STAGE_MS"
provides:
  - "minos_guest::source: JobSource trait, DiskJobSource<R: Read>, GuestInput (token, job, base, diff, judge_data, candidate_out; into_parts -> GuestInputParts), SourceFailure (token: Option, error), SourceError { Disk, Job, DiffNotUtf8 }"
  - "minos_guest::sink: OutputSink trait, Connector trait (connect, close), Backoff (standard 1 ms doubling to 64 ms, 200 attempts; none), ChunkSink (new, with_backoff), SendReport (connections, retries, bytes), SinkError { TooLarge, Connect { chunk, attempts, kind }, Write { chunk, kind } }"
  - "minos_guest::stage: GUEST_UID and GUEST_GID 64000, SCRATCH_MOUNT, SRC_DIR, HOME_DIR, SCRATCH_TMP_DIR, SCRATCH_OUT_DIR, JUDGE_DIR, CANDIDATE_DIR, TMP_MOUNT, OUT_MOUNT, STAGE_ENV (PATH, HOME, TMPDIR, LANG), MAX_PROCESSES 512, MAX_OPEN_FILES 4096, StageLimits::for_stage, TRAMPOLINE_FLAG, trampoline_argv, parse_trampoline, TrampolineError { Layout, Number { position }, EmptyArgv }"
  - "crates/minos-guest/src/test_fixture.rs: cfg(test) candidate, baseline, and judge GuestJobs from the CLAUDE.md example manifest"
affects:
  - "05-10 and 05-12: PID 1 calls DiskJobSource::new(File /dev/vdb).read_input(); on SourceFailure with a token it sends an authenticated setup-failure status; it drops each GuestInput part after unpacking"
  - "05-12: the Linux Connector opens a vsock stream to the host CID on OUTPUT_VSOCK_PORT and its close does shutdown(Write); the trampoline entry calls parse_trampoline(&args_os), refuses PID 1 and uid 0, applies the five StageLimits plus RLIMIT_CORE 0 with current = maximum, and execs; PID 1 spawns trampoline_argv with Command::uid(GUEST_UID).gid(GUEST_GID), env_clear() plus STAGE_ENV"
  - "05-12: the spike modules (job, report, pattern) and markers in lib.rs remain until linux.rs is replaced"
tech-stack:
  added:
    - "proptest (workspace pin 1.11.0) as a minos-guest dev-dependency; Cargo.lock gains only the edge, no new package"
  patterns:
    - "A JobSource/OutputSink trait pair over generic Read and Connector, so only the Linux entry names /dev/vdb and vsock"
    - "A failure type that carries the already-read token, so input errors are still reported over the authenticated channel"
    - "A strict positional argv parsed with one slice pattern, no indexing, and a proptest round trip"
key-files:
  created:
    - crates/minos-guest/src/source.rs
    - crates/minos-guest/src/sink.rs
    - crates/minos-guest/src/stage.rs
    - crates/minos-guest/src/test_fixture.rs
    - crates/minos-guest/proptest-regressions/stage.txt
  modified:
    - crates/minos-guest/src/lib.rs
    - crates/minos-guest/Cargo.toml
    - Cargo.lock
decisions:
  - "ChunkSink refuses a tar over MAX_OUTPUT_STREAM_BYTES before the first connect (SinkError::TooLarge), because ChunkStream ends without an END past u32 chunks and the host refuses an over-cap stream anyway"
  - "A failed close (shutdown of the write side) is reported as SinkError::Write for that chunk, since the host may not have seen the chunk end"
  - "parse_trampoline refuses a limit of 0 and a leading zero as well as signs, spaces, and values over the cap, and refuses an empty program after the separator as EmptyArgv; TrampolineError::Number carries the argv index (2 to 6)"
  - "Trampoline caps: address space MAX_MEMORY_MIB MiB, file size MAX_SCRATCH_MIB MiB, processes 512, CPU MAX_STAGE_MS/1000 + 1 = 3601 s, open files 4096"
  - "GuestInput::into_parts returns a tuple alias (GuestInputParts) rather than a struct with pub fields, so the validated type gains no pub field"
metrics:
  duration: "about 10 minutes"
  completed: "2026-10-18"
  tasks: 2
  files: 8
---

# Phase 5 Plan 08: Guest JobSource, OutputSink, and stage trampoline Summary

The guest now reads its job through a `JobSource` trait and sends its output through an `OutputSink` trait (D-07). `DiskJobSource` decodes the v2 input disk with core's checked reader and runs `check_role`; when it fails after the header it still returns the per-launch token. `ChunkSink` sends the output tar as authenticated 4144-byte-max chunks, one connection each, retrying a refused or reset connect with bounded backoff. `stage.rs` fixes the uid and gid for every stage at 64000, sets a fixed four-key environment, derives the D-05 rlimits, and encodes them in an `/init --minos-stage` argv that only a strict parser accepts (D-08). All of it compiles and is tested on the host, and it passes the musl clippy gate.

## Tasks

| Task | Name | Commits | Files |
| ---- | ---- | ------- | ----- |
| 1 | JobSource and DiskJobSource; OutputSink and ChunkSink (D-07) | 858a9eb (RED), 59e205b (GREEN) | source.rs, sink.rs, lib.rs, test_fixture.rs |
| 2 | Stage limits and the /init trampoline argv (D-05, D-08) | 58ba10a (RED), 7a09531 (GREEN), 5526ca3 (proptest seed) | stage.rs, lib.rs, Cargo.toml, Cargo.lock, proptest-regressions/stage.txt |

## What was built

- **source.rs.** `DiskJobSource<R: Read>` calls `read_input_disk`. If that fails, the token is `None`. Otherwise it clones the token and then parses the JOB section, runs `check_role`, and takes the role's sections: BASE plus an optional UTF-8 DIFF for a candidate, `JUDGE_DATA` plus `CANDIDATE_OUT` for a judge. Every later failure returns `SourceFailure { token: Some(..) }`. `GuestInput`'s `Debug` shows section lengths only, and the token stays redacted.
- **sink.rs.** `ChunkSink` iterates `ChunkStream::new(&token, tar)`. For each frame it connects (retrying only `ConnectionRefused` and `ConnectionReset`, at most `Backoff::attempts` times), calls `write_all`, then `close`. It counts connections, retries, and bytes written. With `Backoff::none()` it never sleeps, so the tests run instantly. `Backoff::standard()` waits 1, 2, 4, ... ms up to 64 ms between attempts, at most 200 attempts per chunk.
- **stage.rs.** `StageLimits::for_stage(job, stage)`: the example job (memory 2048, scratch 1024, build 120000 ms) gives 768 MiB of address space, a 1024 MiB file size, 512 processes, 121 CPU seconds, and 4096 open files. `trampoline_argv` writes `[init, --minos-stage, as, fsize, nproc, cpu, nofile, --, argv...]`. `parse_trampoline` matches that layout with one slice pattern, and a proptest checks the round trip for any limits within their caps and any argv of 1 to 8 non-empty entries without NUL.

## Verification

- `cargo test -p minos-guest`: 44 unit tests pass (5 source, 8 sink, 9 stage, 22 existing).
- `cargo clippy -p minos-guest --all-targets -- -D warnings` and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: clean.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (64 suites ok), `cargo doc --workspace --no-deps`, `cargo deny check` (advisories, bans, licenses, sources ok; the existing unused-wrapper warning for minos-runner and minos-cli is unchanged): all pass.
- `grep -n 'cfg(target_os' crates/minos-guest/src/source.rs crates/minos-guest/src/sink.rs` prints nothing.
- C-01: no MISSING line. C-02: printed nothing.

## Requirements

GUEST-02, GUEST-03, and GUEST-04 are not marked complete. This plan delivers their host-compilable halves. The Linux entry in 05-12 still has to open `/dev/vdb`, apply the rlimits and exec in the trampoline, and connect over vsock before any of the three holds end to end.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] ChunkSink caps the tar before sending**
- **Found during:** Task 1
- **Issue:** `ChunkStream`'s doc says callers must cap the payload first. A payload too large for a `u32` seq ends with no END chunk, and the host refuses anything over `MAX_OUTPUT_STREAM_BYTES` anyway.
- **Fix:** Added `SinkError::TooLarge { bytes }`. It is returned before the first connect when the tar exceeds `MAX_OUTPUT_STREAM_BYTES`, and a test covers it.
- **Files modified:** crates/minos-guest/src/sink.rs
- **Commit:** 59e205b

**2. [Rule 3 - Blocking] Shared cfg(test) fixture module**
- **Found during:** Task 1
- **Issue:** The source and stage tests both need valid `GuestJob`s, which only a validated `Manifest` and `JobRequest` can build.
- **Fix:** Added `crates/minos-guest/src/test_fixture.rs` (`#[cfg(test)]`). It builds the CLAUDE.md example manifest through public constructors only, the same way `minos-core/tests/guest_wire.rs` does.
- **Files modified:** crates/minos-guest/src/test_fixture.rs, crates/minos-guest/src/lib.rs
- **Commit:** 858a9eb

Additional tests beyond the behavior block: an empty tar sends one END, a non-retryable connect error fails after 1 attempt, the backoff schedule, a `--` inside the command is kept, a limit at its cap is accepted, and section bytes never appear in `Debug`.

## TDD Gate Compliance

Each task has a `test(05-08)` RED commit, with tests that failed against stub bodies (12 source and sink tests failed, then 7 stage tests failed), followed by a `feat(05-08)` GREEN commit. No refactor commit was needed. The proptest seed recorded during the stage RED run is committed as `crates/minos-guest/proptest-regressions/stage.txt`, as CLAUDE.md Testing asks.

## Known Stubs

None. The traits have no Linux implementation yet. That is 05-12's scope by design, and the plan objective says so.

## Self-Check: PASSED

- FOUND: crates/minos-guest/src/source.rs, crates/minos-guest/src/sink.rs, crates/minos-guest/src/stage.rs, crates/minos-guest/src/test_fixture.rs, crates/minos-guest/proptest-regressions/stage.txt
- FOUND commits: 858a9eb, 59e205b, 58ba10a, 7a09531, 5526ca3
