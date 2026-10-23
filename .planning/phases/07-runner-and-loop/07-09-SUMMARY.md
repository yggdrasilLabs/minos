---
phase: 07-runner-and-loop
plan: 09
subsystem: minos-runner (errors, host usage, blame map)
status: complete
tags: [runner, blame, outcome, usage, LOOP-01, LOOP-03, JUDGE-03]
requires:
  - phase: 07-03
    provides: "minos-runner crate, its dependency lanes, and its clippy bans (no clock, no env, no HashMap, no Command, no File)"
  - phase: 07-04
    provides: "the approved Untrusted sink exits (D-17), so runner code may land"
provides:
  - "minos_runner::error: RunnerError { Infra { cause }, Config { cause } } with is_infra() and what(); InfraCause { Launch, Wait, Store, BlobIo, GuestSetup { step }, DigestMismatch, GuestJob, Unmapped }; ConfigCause { Spec { error }, Preflight { refusals }, ScratchFull, DiffMismatch, DiffParse, JudgeMismatch, Machine, BlobMissing }; From<InfraCause> and From<ConfigCause> for RunnerError"
  - "minos_runner::usage: RUNNER_VCPUS = 2 (const-asserted within MAX_VCPUS), host_usage(elapsed, &Machine) -> Usage, saturating"
  - "minos_runner::map: Seen { NotExited, Output, Status, Guest }, Verdict { Pass, Done, Infra, Config }, candidate_verdict, judge_verdict, judged_outcome"
affects: [07-12, 07-14, 07-16]
tech-stack:
  added: []
  patterns:
    - "Blame table as a pure function over already-validated observations, one test per row, mirroring supervise::classify"
    - "Fixed-text error causes: a const what() is the single source of the text, and Display writes it"
key-files:
  created:
    - crates/minos-runner/src/error.rs
    - crates/minos-runner/src/usage.rs
    - crates/minos-runner/src/map.rs
    - crates/minos-runner/tests/map.rs
    - crates/minos-runner/tests/usage.rs
  modified:
    - crates/minos-runner/src/lib.rs
decisions:
  - "An Ended the map has no row for (Exited handed in as NotExited, or a future backend variant) is Infra(InfraCause::Unmapped): fails closed, retried and counted toward the breaker, never blamed on the candidate"
  - "A judge that exited 0 passes only with Packed output; TooLarge is Rejected(OutputTooLarge) and Invalid or NotPacked is Rejected(OutputInvalid), because PID 1 then packed only its own files and the scorecard is absent"
  - "A status of the other role with setup Ok is Rejected(OutputInvalid) in either verdict function, matching StatusFault::WrongRole's reason"
  - "InfraCause and ConfigCause text lives in a const what(); Display writes it, and RunnerError's Display names the blame with the cause as its error source"
  - "Verdict and Seen are not #[non_exhaustive], so a new verdict breaks every caller's match"
metrics:
  duration: "about 9 minutes (2026-10-23T00:51:50Z to 01:00:44Z)"
  completed: 2026-10-23
  tasks: 2
  files: 6
---

# Phase 7 Plan 09: Runner errors, host usage, and the blame map Summary

The runner's pure parts are in place: a `RunnerError` that splits infrastructure (retried) from configuration (stops the run); `host_usage`, which computes saturating vCPU-ms and MiB-ms from the backend's host-measured elapsed time only; and `map.rs`, the whole observation-to-outcome table. That table reads setup before role (CR-02), treats `ScratchFull` as configuration (WR-02), and sends judge scorecards through `check_scorecard` gates first (JUDGE-03). Every row has its own test.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Runner errors with blame and host-measured usage | `988928e` | src/lib.rs, src/error.rs, src/usage.rs, tests/usage.rs |
| 2 (RED) | Failing tests for the blame table and gate short-circuit | `def6cbb` | tests/map.rs |
| 2 (GREEN) | The observation-to-outcome map | `22b7898` | src/map.rs, src/lib.rs, src/error.rs |

## What was built

- **`error.rs`.** `RunnerError::{Infra, Config}` with `is_infra()` and `what()`. Each cause is a closed `#[non_exhaustive]` enum, and its fixed text comes from a `const fn what()`. The only payloads are a `SetupStep`, a `SpecError` (exposed as the error source), and a `BTreeSet<Refusal>`. No variant carries a guest or model byte.
- **`usage.rs`.** `RUNNER_VCPUS = 2`, with the A5 reason and a const assertion against `MAX_VCPUS`. `host_usage` computes `u64::try_from(as_millis()).unwrap_or(u64::MAX)`, then `saturating_mul` by vCPUs and by MiB. A saturated value is over every cap.
- **`map.rs`.** The module docs hold the full table. `candidate_verdict` checks setup, role, apply, build, run, then output. `judge_verdict` checks setup, role, the judge stage, then output. Stage ends are looked up with iterators, never by index. `judged_outcome` maps a `ScorecardFault` to its reason; any other scorecard goes through `check_scorecard`.
- **Tests.** `tests/map.rs` has 52 row tests and one proptest (2048 cases). The generator covers every `Ended`, a range of output and status faults, and every status `GuestStatus::new` accepts. Each case checks three things:
  - neither verdict function panics;
  - `Pass` occurs exactly when every pass condition holds;
  - a failed setup is never an `Outcome`.

  `tests/usage.rs` has 7 tests: the 1500 ms case, truncation, saturation, and the error blame split and its source chain.

## Verification

- `cargo test -p minos-runner --test map`: 53 passed. `--test usage`: 7 passed.
- Full gate, each exit 0:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`: 1556 passed, 0 failed, 23 ignored
  - `cargo deny check`: advisories, bans, licenses, and sources ok
  - `cargo doc --workspace --no-deps`: no warnings
  - musl guest clippy
- C-01 printed `ok` for all 11 crates. The C-02 grep printed nothing.
- Acceptance greps:
  - three `pub enum` lines in `error.rs`, and one `pub const fn is_infra`
  - `RUNNER_VCPUS: u8 = 2` and `pub fn host_usage` present, with two `saturating_mul`
  - the three `pub fn` lines in `map.rs`, and `check_scorecard` present
  - 53 `#[test]` in `tests/map.rs`, and `GateFailed` / `gate_failure_short_circuits` present

## Deviations from Plan

1. **[Rule 2 - fail closed] Added `InfraCause::Unmapped`.** `Ended` is `#[non_exhaustive]` in another crate, so the map needs a wildcard arm. `Seen::NotExited(Ended::Exited)` can also be built. Both cases now go to `Infra(Unmapped)`: retried and counted toward the breaker, never blamed on the candidate. Commit `988928e`.
2. **[Rule 2 - correctness] The judge's `Pass` also needs `Packed` output.** The plan's behavior row says "judge stage `Exited { 0 }` → `Pass`". When the output is not packed, PID 1 packed only its own files, so the scorecard is missing. The judge's output status now maps the same way as the candidate's: `TooLarge` is `OutputTooLarge`, and `Invalid` or `NotPacked` is `OutputInvalid`. Without this, both would read as `ScorecardInvalid`, which is less exact. Commit `22b7898`.
3. **[Rule 2 - tests] Added `tests/usage.rs`.** It was not in the plan's file list. It pins `host_usage`, including saturation, and the error blame split. Commit `988928e`.
4. **`pub mod map` landed with Task 2, not Task 1.** Task 1 declared only `error` and `usage`, because an empty `map` module would have broken the intra-doc links. The links were restored in the GREEN commit.
5. **Small additions.** `From<InfraCause>` and `From<ConfigCause>` for `RunnerError`, for 07-12's `?` plumbing. `Verdict` and `Seen` also derive `Eq`.

## TDD Gate Compliance

RED `test(07-09)` `def6cbb` failed on the unresolved `minos_runner::map` import. It precedes GREEN `feat(07-09)` `22b7898`. No refactor commit was needed.

## Known Stubs

None.

## Threat Flags

None. All five threats in the plan's register are mitigated:

- **T-01:** `host_usage` reads only `elapsed` and `Machine`.
- **T-02:** one test per table row, with setup read first.
- **T-03:** `judged_outcome` goes through `check_scorecard`, and the gate short-circuit test passes.
- **T-04:** the proptest covers every accepted status, and the code has no indexing.
- **T-05:** errors are fixed-text closed enums.

## Self-Check: PASSED

- FOUND: crates/minos-runner/src/error.rs, src/usage.rs, src/map.rs, tests/map.rs, tests/usage.rs
- FOUND: commits 988928e, def6cbb, 22b7898
