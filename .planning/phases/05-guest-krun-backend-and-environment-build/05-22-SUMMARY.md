---
phase: 05-guest-krun-backend-and-environment-build
plan: 22
subsystem: minos-sandbox
tags: [sandbox, finished, untrusted, guest-status, console, chunk-protocol, guest-04, krun-01]
status: complete
requires:
  - "05-04: minos_core::guest::{GuestStatus, GuestRole, GuestStatusError, STATUS_PATH, MAX_STATUS_BYTES} and GuestStatus::parse_json"
  - "05-06: the reshaped sandbox test fixtures (role-shaped InputDisk, Capabilities with Seatbelt layers)"
provides:
  - "Finished::new(cleared, ended, stream, elapsed: Duration, console: Vec<u8>), still consuming the Cleared<R>"
  - "Finished::elapsed() -> Duration (host-measured, spawn to exit) and Finished::console_excerpt(max) -> LogExcerpt (the only exit for console bytes)"
  - "Ended::ChunkProtocol: a host-observed candidate fault, recorded as Rejected(Reason::ChunkProtocol), never retried; Finished::new discards the stream for it"
  - "Untrusted<GuestOutput<R>>::take_status(&mut self) -> Result<GuestStatus, StatusFault>"
  - "StatusFault { Missing, Invalid(GuestStatusError), WrongRole } with reason() -> Reason::OutputInvalid"
  - "STATUS_PATH and StatusFault re-exported at the minos-sandbox root"
affects:
  - "05-07 / 05-11 / 05-15: the krun backend's RunningVm::wait passes the measured elapsed time and the capped console into Finished::new, and returns Ended::ChunkProtocol when chunk reassembly fails"
  - "Phase 7 runner: maps Ended::ChunkProtocol to Rejected(ChunkProtocol), derives usage from elapsed (LOOP-03), and reads PID 1's status through take_status before any other exit"
tech-stack:
  added: []
  patterns:
    - "Hostile side data on a trusted envelope: Finished holds console bytes privately and exposes them only through a sanitizing accessor"
    - "Validating exit that removes its file: take_status takes the file out of the output whether or not it validates, so it cannot be read twice"
    - "Contract made true by construction: Finished::new drops the stream for Ended::ChunkProtocol rather than trusting the backend to pass an empty one"
key-files:
  created: []
  modified:
    - crates/minos-sandbox/src/finished.rs
    - crates/minos-sandbox/src/untrusted.rs
    - crates/minos-sandbox/src/lib.rs
    - crates/minos-sandbox/src/backend.rs
    - crates/minos-sandbox/tests/backend.rs
    - crates/minos-sandbox/tests/untrusted.rs
    - crates/minos-sandbox/tests/output_tar.rs
    - crates/minos-sandbox/tests/ui/fail/finished_twice.rs
    - crates/minos-sandbox/tests/ui/fail/finished_twice.stderr
    - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
decisions:
  - "StatusFault::Invalid carries the core GuestStatusError (line and column only, no hostile byte), and an over-cap status maps to Invalid(GuestStatusError::TooLarge), so the sandbox's own pre-parse cap and core's agree"
  - "Finished::new discards the stream when ended is Ended::ChunkProtocol, so 'no byte of a broken stream is kept' holds whatever a backend passes"
metrics:
  duration: 6min
  completed: 2026-10-18
  tasks: 2
  files: 10
---

# Phase 5 Plan 22: Finished elapsed and console, Ended::ChunkProtocol, and take_status Summary

`Finished` now carries the host-measured elapsed time and the captured console. The console has one exit, `console_excerpt`, which goes through `LogExcerpt::from_bytes`. `Ended::ChunkProtocol` covers a broken chunk stream, and its bytes are discarded by construction. The guest's `.minos/status.json` leaves `Untrusted` only through `take_status`, which caps the bytes, parses them strictly, and checks the role.

## What was built

**Task 1: Finished elapsed and console, and Ended::ChunkProtocol** (RED `b37bbaf`, GREEN `1d0e9e9`, fmt `22b8ff8`)
- `Finished::new(cleared, ended, stream, elapsed, console)` still consumes the `Cleared<R>`.
- `elapsed()` returns the host-measured `Duration`. `console_excerpt(max)` returns a `LogExcerpt` capped at 4096 bytes with control characters stripped. No other accessor exposes the console bytes.
- `Ended::ChunkProtocol` is documented as a host-observed candidate fault that is never retried. `CrashCause::ConsoleCap`'s doc was reworded to "the console passed its cap, as the host observed it".
- Every `Finished::new` call in the sandbox tests, the `judge_flow` pass case, and the `finished_twice` fail case now uses the new signature. `finished_twice.stderr` was regenerated with `TRYBUILD=overwrite` and still holds exactly one error: E0382, a use of the moved `ok: Cleared<Judge>`.
- New tests: `finished_reports_elapsed_and_sanitized_console`, `console_reaches_callers_only_through_the_excerpt` (BEL and OSC sequences through the fake backend), and `ended_chunk_protocol_exists`.

**Task 2: The take_status exit and its re-exports** (RED `5d348c2`, GREEN `83e79af`)
- `take_status` first removes the file at `STATUS_PATH`. It then refuses it over `MAX_STATUS_BYTES` before parsing, parses it with `GuestStatus::parse_json`, and refuses a role other than `R::KIND`, mapped through a const `RoleKind -> GuestRole` match.
- `StatusFault { Missing, Invalid(GuestStatusError), WrongRole }` is `#[non_exhaustive]`, and every variant's `reason()` is `Reason::OutputInvalid`.
- `minos_sandbox::{STATUS_PATH, StatusFault}` are re-exported. `STATUS_PATH` is core's own constant, so it cannot drift.
- New tests: `take_status_valid`, `take_status_valid_for_judge_output`, `take_status_refusals` (missing, bad JSON, padded past the cap, a judge status in candidate output, a candidate status in judge output), and `status_path_is_core_s`.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --workspace --no-deps` (0 warnings), and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` are all clean.
- `env -u TRYBUILD cargo test --workspace`: 1103 passed, 0 failed.
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok. The two "unmatched wrapper" warnings on `deny.toml` lines 65 and 67 were already there; `deny.toml` is unchanged since 04-01.
- C-01 printed no MISSING line. C-02 printed nothing.
- `grep -c '^error' crates/minos-sandbox/tests/ui/fail/finished_twice.stderr` is 1.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] The empty stream for ChunkProtocol is now enforced, not just documented**
- **Found during:** Task 1 review, after GREEN
- **Issue:** The plan's doc for `Ended::ChunkProtocol` says the stream is empty, but `Finished::new` kept whatever bytes the backend passed. A backend bug could have kept bytes of a broken stream, and a later `into_output` could read them.
- **Fix:** `Finished::new` replaces the stream with an empty `Vec` when `ended == Ended::ChunkProtocol`. The new test `chunk_protocol_keeps_no_stream_byte` shows that a fake backend passing a valid tar with `ChunkProtocol` still yields no output.
- **Files modified:** crates/minos-sandbox/src/finished.rs, crates/minos-sandbox/tests/backend.rs
- **Commit:** 3555872

**2. [Rule 3 - Doc consistency] Module and trait docs updated**
- The `lib.rs` flow steps 5 and 7, the `RunningVm::wait` doc in `backend.rs`, and the `untrusted.rs` layout doc now name the console excerpt, `ChunkProtocol`, and `take_status`. `backend.rs` was not in the plan's `files_modified` list; the change there is doc only.

## TDD Gate Compliance

Both tasks have a `test(05-22)` RED commit, followed by a `feat(05-22)` GREEN commit: b37bbaf then 1d0e9e9, and 5d348c2 then 83e79af. Before each GREEN commit, the RED tests failed to compile with E0061, E0599, and E0432 against the old API.

## Threat Mitigations

| Threat | Mitigation in code |
|---|---|
| T-05-22-01 forged or wrong-role status | `take_status`: the cap check comes before parsing; `GuestStatus::parse_json` uses `deny_unknown_fields` and the `new` checks; the role must equal `R::KIND` |
| T-05-22-02 console escapes reaching a terminal | Console bytes are private to `Finished`; the only exit is `console_excerpt` -> `LogExcerpt::from_bytes` |
| T-05-22-03 chunk break retried forever | `Ended::ChunkProtocol` is a host-observed candidate fault, and its stream is discarded |
| T-05-22-04 one clearance, two results | `Finished::new` consumes `Cleared<R>`; `finished_twice` still fails with exactly one E0382 |

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/src/finished.rs (ChunkProtocol, pub fn elapsed, pub fn console_excerpt, LogExcerpt::from_bytes)
- FOUND: crates/minos-sandbox/src/untrusted.rs (pub fn take_status, GuestStatus::parse_json)
- FOUND: crates/minos-sandbox/src/lib.rs (StatusFault, STATUS_PATH)
- FOUND commits: b37bbaf, 1d0e9e9, 22b8ff8, 5d348c2, 83e79af, 3555872
