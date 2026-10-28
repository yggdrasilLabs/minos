---
phase: 08-cli-sample-project-and-docs
plan: 07
subsystem: cli-check
status: complete
tags: [cli, check, calibration, noise-floor, routes, advisory]
requires:
  - minos_cli::project::Prepared, minos_cli::address::fold_run, minos_cli::render::{write_line, id_text, outcome_word, stop_reason_name, cost_line} (08-06)
  - minos_core::limits::{batch_output_tokens, near_output_cap, CLI_DEFAULT_OUTPUT_TOKENS} and RunInputs::surface_bytes (08-01)
  - minos_loop::{Driver::start, Driver::calibrate, NoopObserver} (Phase 7)
provides:
  - minos_cli::check::{check, CheckOptions, CheckOutcome, CheckError, ProviderStatus}
  - test target crates/minos-cli/tests/check.rs (13 tests)
affects: [08-12 main.rs wiring of `minos check`, 08-13 the VM end-to-end test, 08-14 the D-07 noise measurement]
tech-stack:
  added: []
  patterns:
    - "check is layout over core functions: route_table, judge_scratch_needed_bytes, noise_floor, noise_check, baseline_ready"
    - "One report path from the run's fold, for a reused run and a new one alike"
    - "Advisories print and never refuse; only a provider refusal or a stopped calibration sets refused"
key-files:
  created:
    - crates/minos-cli/tests/check.rs
  modified:
    - crates/minos-cli/src/check.rs
key-decisions:
  - "check takes six arguments: started_at and the DriverOptions live in CheckOptions (new(fresh, started_at), with_driver), because the planned eight-argument signature fails clippy::too_many_arguments under -D warnings"
  - "A new run's RunId is checked against Ledger::runs before Driver::start, because start rejoins an existing run; a hit is CheckError::RunExists"
  - "A calibration that ends as Calibrated::Stopped prints `calibration stopped: <reason>` and sets refused; the noise verdict never does"
  - "The memory line reads Environment::candidate_allowance_mib (computed once at manifest parse, which its doc says callers read rather than recompute) instead of calling limits::candidate_allowance_mib again"
  - "A judge scratch smaller than the requirement adds `: raise [environment] scratch_mib` to the line; the runner's own check still refuses the run at Driver::start"
requirements-completed: []
coverage:
  - deliverable: "Static report: project, base with denylist drops, root image, judge, memory budget, judge scratch, routes, D-08 advisory, provider status"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/check.rs (7 static-report tests)"
        status: pass
  - deliverable: "Calibration reuse or a fresh run, RunExists, per-trial values, noise floor, min_improvement threshold, advisory verdict, cost line"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/check.rs (6 calibration tests)"
        status: pass
metrics:
  duration: 11 min
  started: 2026-10-28T18:08:40Z
  completed: 2026-10-28
  tasks: 2
  files: 2
---

# Phase 8 Plan 07: `minos check` Summary

`check` reports a prepared project's facts, budgets, route table, the single-shot output advisory, and the provider status. It then calibrates the baseline, either on a reused run whose frame matches (no VM launched) or on a fresh diagnostic run through `Driver::start` and `calibrate`. Last it prints each trial, the noise floor, `min_improvement` with its threshold at the median, the advisory verdict with advice, and the cost through `CostLine`.

## Accomplishments

- **Static report** (`report_facts`). Every line goes through `render::write_line`:
  - `project:` with the 12-hex manifest ID, then `base:` with tree, file count, and packed bytes.
  - One `dropped by the denylist: <path> (<pattern>)` line per drop.
  - `root image:`, then `judge:` with version, label, and the lock's file and byte extent.
  - The memory line, `memory: M MiB = guest reserve 256 MiB + scratch S MiB + candidate allowance A MiB`.
  - `judge scratch: needs N MiB of S MiB`, with N from `judge_scratch_needed_bytes` rounded up to whole MiB.
  - `routes:` and one row per proposer, `  <i> <tier>[ <model>]: eligible | ineligible: <reason>`.
  - The D-08 advisory for each eligible `single_shot` proposer whose `batch_output_tokens(writable bytes, batch)` satisfies `near_output_cap`.
  - The provider line for `NotNeeded`, `Ready`, or `Refused`. Only `Refused` sets `refused`.
- **Calibration** (`reusable_run`, `report_calibration`).
  - **Reuse.** Without `--fresh`, `check` reuses the latest `(started_at, RunId)` run that meets all of these: its `manifest_at(0)`, base, and root image equal the prepared ones; every trial `0..[judge] trials` is judged under the prepared judge version; and `baseline_ready` holds.
  - **New run.** Otherwise `check` computes `RunId::of`. It refuses an ID already in the store (`RunExists`) and prints `baseline: run <hex>, N trials (...)`. Then it starts and calibrates the run. A `Calibrated::Stopped` result prints `calibration stopped: <reason>` and sets `refused`.
  - **Report.** Both paths then fold the run and print the trial lines, `noise floor:` (or `not measured`), `min_improvement:`, `noise verdict: ok|warn|refuse`, an `advice:` line for warn and refuse, and the `approx. cost:` line.
- **Module docs.** They list all seven steps with their decision IDs.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 167d5fa | test(08-07): add failing tests for check's static report |
| 1 | GREEN | e0e557a | feat(08-07): print check's project facts, budgets, routes, advisory, and provider status |
| 2 | RED | dee94d5 | test(08-07): add failing tests for check's calibration and reuse |
| 2 | GREEN | eafa652 | feat(08-07): calibrate or reuse the baseline and print the noise floor beside min_improvement |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `check` takes six arguments, not eight**
- **Found during:** Task 1 design
- **Issue:** The planned signature, `(prepared, proposer, jobs, provider, options, started_at, driver, out)`, has eight arguments. `clippy::too_many_arguments` (complexity group, threshold 7) fails it under `-D warnings`, and no `allow` is permitted.
- **Fix:** `started_at` and `DriverOptions` moved into `CheckOptions`. `CheckOptions::new(fresh, started_at)` defaults to the production `RETRY_PAUSE`, and `with_driver(DriverOptions)` overrides it (tests pass `Duration::ZERO`). The generic header `pub fn check<P: ModelProvider, J: JobRunner, W: Write>` is unchanged, and so is its acceptance grep.
- **Files modified:** crates/minos-cli/src/check.rs
- **Commit:** e0e557a

**2. [Rule 1 - Bug] Test fixtures were dropped before use**
- **Found during:** Task 2 GREEN
- **Issue:** `Fixture::new()?.prepare()?` dropped the `TempDir` at the end of the statement, so the project directory was gone before `Driver::start` created `runs/`. The run failed with `creating a store directory: NotFound`. The Task 1 tests had passed only because they wrote nothing afterwards.
- **Fix:** Every test binds `let fixture = ...;` and keeps it alive while the prepared store is used.
- **Files modified:** crates/minos-cli/tests/check.rs
- **Commit:** eafa652

**3. [Rule 3 - Blocking] Task 2 fakes left out of the Task 1 commit**
- **Found during:** Task 1 GREEN
- **Issue:** The gate-failing script and the request counter were unused in Task 1, and the unused-code warnings would have failed clippy under `-D warnings`.
- **Fix:** Task 2's RED commit added them back.
- **Commit:** dee94d5

**Total deviations:** 3 (1 bug, 2 blocking). **Impact:** none on scope. One public signature is shaped differently, and the acceptance greps hold.

## TDD Gate Compliance

Each task has a `test(08-07)` RED commit before its `feat(08-07)` GREEN commit:

- **Task 1:** 167d5fa, then e0e557a. RED failed to compile on the missing `check` items.
- **Task 2:** dee94d5, then eafa652. RED compiled, and 6 tests failed on the missing calibration.

No refactor commit was needed.

## Known Stubs

- `CheckOutcome::run` is an `Option`, but every successful `check` now returns `Some`. The `Option` stays, as the plan specified.
- `check` is not yet wired into `main.rs`. Plan 08-12 does that, including the real provider preflight that produces the `ProviderStatus`.

## Threat Flags

None. T-08-07-01 to T-08-07-05 are mitigated as planned:

- **T-08-07-01.** Reuse requires an equal frame, all trials judged, and `baseline_ready`. A changed base starts a new run (tested).
- **T-08-07-02.** A same-second start is `RunExists` (tested).
- **T-08-07-03.** Every line goes through `write_line`.
- **T-08-07-04.** Cost is printed only through `cost_line`/`CostLine`, and the banned-word helper runs in every test.
- **T-08-07-05.** Neither the advisory nor the verdict sets `refused` (tested).

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli --test check`: 13 passed. All of `cargo test -p minos-cli` passes.
- `cargo doc -p minos-cli --no-deps`: no warnings
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Every acceptance grep for both tasks prints its expected lines.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/check.rs, crates/minos-cli/tests/check.rs
- FOUND: 167d5fa, e0e557a, dee94d5, eafa652
