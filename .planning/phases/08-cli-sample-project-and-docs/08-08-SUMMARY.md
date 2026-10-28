---
phase: 08-cli-sample-project-and-docs
plan: 08
subsystem: cli-run
status: complete
tags: [cli, run, resume, progress, observer, cost-line]
requires:
  - minos_cli::project::Prepared, minos_cli::address::{HexPrefix, resolve_run, fold_run}, minos_cli::render helpers (08-06)
  - minos_core::money::CostLine and render::cost_line (08-01, 08-06)
  - minos_loop::{Driver::start, Driver::resume, calibrate, run_to, into_observer, Observer, LoopEvent} (Phase 7)
provides:
  - minos_cli::run::{run, RunOptions, RunOutcome, RunEnded, RunError, ProgressObserver}
  - minos_cli::run::{trial_line, proposed_line, selected_line, retrying_line, stopped_line, cost_text}
  - test targets crates/minos-cli/tests/run.rs (8 tests) and crates/minos-cli/tests/progress_golden.rs (4 tests)
affects: [08-12 main.rs wiring of `minos run`, 08-13 the VM end-to-end test, 08-14 docs]
tech-stack:
  added: []
  patterns:
    - "One pure formatter per progress line kind, pinned byte for byte by a literal golden"
    - "The observer re-folds the ledger for percent, elapsed, and cost; the events carry no free text"
    - "run and the driver-owned observer share one writer through a RefCell handle borrowed for one write at a time"
key-files:
  created:
    - crates/minos-cli/tests/run.rs
    - crates/minos-cli/tests/progress_golden.rs
  modified:
    - crates/minos-cli/src/run.rs
    - CLAUDE.md
key-decisions:
  - "N is the absolute generation to reach for a new run and for --run alike (08-RESEARCH Open Question 1, decided); CLAUDE.md records it"
  - "The noise verdict prints as `g00 noise verdict: <word> (advisory; minos check explains)` and never refuses (Open Question 2, decided; D-07)"
  - "The observer and run share the output through a private Shared(&RefCell) writer, because the driver owns its observer for the session while run prints between calls; a failed borrow is an io::Error, never a panic"
  - "The post-calibration cost line is labelled with the last concluded generation, so a new run prints g00 and a resume prints the generation it resumes after"
  - "RunError::ResumeRefused and RunError::Driver box their DriverError (clippy result_large_err), and a Store variant was added for the run listing"
  - "A `kept` decision prints its incumbent, so the three selected forms are improved <exp>, kept <exp>, and no_valid_candidates"
requirements-completed: []
coverage:
  - deliverable: "The pinned progress format: trial, proposed, selected, retrying, stopped, and cost lines"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/progress_golden.rs (4 tests)"
        status: pass
  - deliverable: "run starts, resumes to an absolute target, refuses collisions and changed frames, honours a stop, prints percent and cost lines"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/run.rs (8 tests)"
        status: pass
metrics:
  duration: 8 min
  started: 2026-10-28T18:20:30Z
  completed: 2026-10-28
  tasks: 2
  files: 4
---

# Phase 8 Plan 08: `minos run` Summary

`run` starts a new run or resumes one by prefix, drives it to an absolute generation, and prints one sanitized, fixed-width line per recorded effect, with a delta percent of the incumbent, host-measured elapsed time, and a `CostLine` after every concluded generation.

## Accomplishments

- **Line formatters** (`run.rs`). Six pure `#[must_use]` functions lay out every progress line, with the column table in the module docs:
  - `trial_line` is `g{gen:02} {exp:<12} {tier:<11} {outcome:<26} {percent:>7} {elapsed:>7}`. For example, `g03 3f9a2c1d7e4b single_shot scored                       +6.2%   1m42s`.
  - `proposed_line`, `selected_line`, `retrying_line`, `stopped_line`, and `cost_text`. The last is `g03 ` followed by the `CostLine` text.
- **Golden** (`tests/progress_golden.rs`). One literal pins 15 lines:
  - trial lines: scored with a positive and a negative percent, a baseline trial, `rejected:gate_failed`, `crashed:build`, `timed_out:run`, the widest word `rejected:held_out_mismatch`, and a three-digit generation;
  - `proposed`, the three `selected` forms, `retrying`, `stopped`, and a cost line.

  Other tests check that no line ends in a space and that every line is `SafeText`-stable.
- **`ProgressObserver`** implements `Observer`. It refolds the ledger only for the events that need it:
  - a baseline trial gets tier `baseline` and its elapsed time;
  - a candidate trial gets `percent_of(delta, incumbent_before(gen) reduced ScoredOnly)` and `trial_elapsed_ms`;
  - a decision is followed by the cost line for the run so far, or `g<N> cost: unavailable (<why>)`.

  A fold failure prints `n/a` columns. The first write error is kept and returned by `finish`.
- **`run`** refuses `ZeroGenerations`.
  - **Without `--run`**, it names the run with `RunId::of`. An ID already in the store is refused as `RunExists`, naming `--run`. Otherwise it calls `Driver::start`.
  - **With `--run`**, it resolves the prefix and calls `Driver::resume`. `BaseMismatch`, `RootImageChanged`, `JudgeChanged`, and `ManifestChanged` become `ResumeRefused`, whose text includes the driver's message and `rerun without --run`.
  - **Then** it calibrates, printing the advisory noise verdict and the generation-0 cost line, and calls `run_to(N)`. A `Calibrated::Stopped` ends the session at generation 0.
  - **Last** it releases the session and prints `run <hex> reached generation <g>` or `run <hex> stopped at generation <g>: <reason>`.
- **CLAUDE.md.** The CLI table and the loop section's "The target is absolute" bullet now say that `--generations N` is absolute for both new and `--run` resumed runs.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 3838c57 | test(08-08): add the failing golden test for the run progress format |
| 1 | GREEN | 674823c | feat(08-08): define the run progress line formatters |
| 2 | RED | ce42ff6 | test(08-08): add failing tests for minos run |
| 2 | GREEN | 2fdffe5 | feat(08-08): start or resume a run to an absolute generation with progress lines |
| docs | | d234d4e | docs(08-08): record that minos run takes an absolute target for new and resumed runs |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] A shared writer instead of `&mut *out` for the observer**
- **Found during:** Task 2 design
- **Issue:** The driver owns its observer for the whole session. `run` must still print the noise verdict and the generation-0 cost line between `calibrate` and `run_to`, so the observer cannot hold `&mut *out`. Ending the session and resuming it to regain the writer would also reset the driver's session-relative stop snapshot, and a `minos stop` arriving in that gap would be ignored.
- **Fix:** A private `Shared<'_, T>(&RefCell<T>)` writer. It is borrowed with `try_borrow_mut` for one write at a time, and a failed borrow becomes an `io::Error`, so there is no panic path. The observer gets `Shared(&cell)` and `run` writes through the same cell. `ProgressObserver` stays generic over any `W: Write`.
- **Commit:** 2fdffe5

**2. [Rule 3 - Blocking] `DriverError` is boxed in `RunError`**
- **Found during:** Task 2 clippy (`result_large_err`)
- **Fix:** `ResumeRefused { run, source: Box<DriverError> }` and `Driver { run, source: Box<DriverError> }`. The text is unchanged.
- **Commit:** 2fdffe5

**3. [Rule 2 - Missing error path] `RunError::Store`**
- **Issue:** The `RunExists` check lists runs through `Ledger::runs`, which can fail. The planned enum had no variant for that.
- **Fix:** Added `Store(#[source] StoreError)`. The enum is `#[non_exhaustive]`.
- **Commit:** 2fdffe5

**4. [Rule 3 - Blocking] `run` consumes `RunOptions`**
- **Issue:** clippy `needless_pass_by_value` fired on the planned by-value signature.
- **Fix:** `run` destructures the options. The signature is unchanged at seven arguments, under `too_many_arguments`.
- **Commit:** 2fdffe5

**5. [Rule 1 - Bug] Test fixture**
- **Issue:** The test `prices.toml` had no `[models]` table, which the price parser requires.
- **Fix:** The fixture prices `haiku`, as the check fixture does.
- **Commit:** 2fdffe5

**6. [Rule 2 - Docs current] CLAUDE.md**
- CLAUDE.md now records that N is absolute for a resumed run too (Open Question 1, decided here). CLAUDE.md was not in `files_modified`.
- **Commit:** d234d4e

Three smaller points:
- The observer folds only for the baseline-trial, candidate-trial, and decision events, not on every event, because the `proposed`, `retrying`, and `stopped` lines read nothing from the ledger.
- The experiment column is padded to 12 (`{exp:<12}`), so an `n/a` from a failed fold keeps the width.
- Two tests go beyond the behavior list: zero generations together with an unknown prefix, and the advisory `refuse` verdict that still reaches the target.

**Total deviations:** 6 (1 bug, 2 missing pieces, 3 blocking). **Impact:** none on scope. The public names and the seven-argument `run` signature match the plan.

## TDD Gate Compliance

Each task has a `test(08-08)` RED commit before its `feat(08-08)` GREEN commit:

- **Task 1:** 3838c57, then 674823c. RED failed to compile on the missing formatters. The GREEN commit also carries rustfmt's re-wrapping of the golden test.
- **Task 2:** ce42ff6, then 2fdffe5. RED failed to compile on the missing `run` items.

No refactor commit was needed.

## Known Stubs

None in `run.rs`. `run` is not yet wired into `main.rs`; plan 08-12 does that with the real provider and `krun_jobs`. No requirement ID was ticked in REQUIREMENTS.md, because plan 08-15 marks requirements.

## Threat Flags

None. T-08-08-01 to T-08-08-05 are mitigated as planned:

- **T-08-08-01.** Lines carry only IDs, numbers, and closed-set names. Every line goes through `write_line`, and the golden asserts that each line is `SafeText`-stable.
- **T-08-08-02.** A new run with an existing ID is `RunExists` before `Driver::start`, and nothing is printed or launched (tested).
- **T-08-08-03.** `Driver::resume`'s checks stay in force, and a changed base is `ResumeRefused` naming `rerun without --run` (tested).
- **T-08-08-04.** Cost is printed only through `CostLine`, and the banned-word helper runs on every output.
- **T-08-08-05.** A mid-trial `UserStop` ends the run as `Stopped` with a `stopped user_stop` line (tested).

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli --test run --test progress_golden`: 8 and 4 passed. All of `cargo test -p minos-cli` passes.
- `cargo doc -p minos-cli --no-deps`: no warnings
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Acceptance greps: there are six formatter `pub fn` lines and the `Observer for ProgressObserver` impl. `Driver::resume`, `Driver::start`, and `run_to(` appear in code, and `rerun without --run` appears once. The golden test holds `g03 3f9a2c1d7e4b single_shot scored`.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/run.rs, crates/minos-cli/tests/run.rs, crates/minos-cli/tests/progress_golden.rs
- FOUND: 3838c57, 674823c, ce42ff6, 2fdffe5, d234d4e
