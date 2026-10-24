---
phase: 07-runner-and-loop
plan: 13
subsystem: minos-loop (the generation driver)
status: complete
tags: [loop, driver, resume, calibration, LOOP-04, LOOP-05, LOOP-09, D-05, D-06, D-07, D-08, D-10, D-13, D-18, D-19]
requires:
  - phase: 07-05
    provides: "RunLock::lock_run, StoreError::{RunLocked, Conflict}"
  - phase: 07-06
    provides: "BaseTree, CumulativeDiff, Experiment::candidate"
  - phase: 07-07
    provides: "JobRunner, JobFailure, DriverError, Observer, LoopEvent, Step, next_step, CANDIDATE_TRIALS"
  - phase: 07-10
    provides: "RunInputs, check_start, check_resume, record::{append, spend_once}, admit, load_batch, missing_proposed, history readers"
provides:
  - "minos_loop::driver: Driver (start, resume, run_id, options, observer, into_observer, calibrate, run_to), DriverOptions, RETRY_PAUSE, Calibration, Calibrated, RunEnd; re-exported at the crate root"
  - "crates/minos-loop/tests/driver/: main.rs, support.rs (FakeJobs, FaultStore, Recorder, fixtures), flow.rs (14 tests)"
affects: [07-15, 07-16, 07-17]
tech-stack:
  added: []
  patterns:
    - "The driver re-folds the ledger before every step and performs exactly next_step's step; its only session memory is the set of generations whose batch it completed"
    - "A partly recorded batch is completed first and next_step asked again, so a resumed session runs trials in the order an uninterrupted one would"
    - "One test target per scenario family with a shared support module, so every support item is used inside one crate"
key-files:
  created:
    - crates/minos-loop/src/driver.rs
    - crates/minos-loop/tests/driver/main.rs
    - crates/minos-loop/tests/driver/support.rs
    - crates/minos-loop/tests/driver/flow.rs
  modified:
    - crates/minos-loop/src/lib.rs
decisions:
  - "resume runs the baseline's JobRunner::check before appending ManifestChanged, not after, so it records nothing before every check has passed (the plan listed the check last)"
  - "The driver's private constructor was inlined into start and resume as struct literals, because an eight-argument fn needs #[expect(clippy::too_many_arguments)] and the codebase carries no #[expect]"
  - "SurfaceStats for routing counts the program file plus every base surface file, as SurfaceStats's own docs define it, rather than the base tree alone"
  - "A recorded manifest blob that is missing, over the 1 MiB cap, tampered, or unparsable reads as None (check_resume then refuses any manifest change); any other store error is DriverError::Store"
  - "Judged records take the experiment, judge, and trial from the request the driver sent, so a result is always attributed to the trial that was asked for"
  - "LoopEvent::Proposed is emitted only when a batch was recorded; an empty propose step emits Selected only"
metrics:
  duration: "about 15 minutes (2026-10-24T13:20Z to 13:35Z)"
  completed: 2026-10-24
  tasks: 2
  files: 5
---

# Phase 7 Plan 13: The generation driver Summary

`Driver` starts or resumes a run under its lock, calibrates generation 0 into a noise floor and verdict, and drives generations to an absolute target by performing exactly `next_step`'s step over a re-folded ledger, recording every spend before its batch or `Judged`. It is proven end to end on `MemoryStore`, `FakeProvider`, and an in-test `FakeJobs` for both the sweep and single_shot tiers.

## What was built

**Task 1 (77158e9), `driver.rs`:**
- `RETRY_PAUSE` (5 s, with its reason), `DriverOptions { retry_pause }` with `Default`, `Calibration { trials, floor, verdict }`, `Calibrated { Ready, Stopped }`, and `RunEnd { Reached, Stopped }`.
- `Driver<'a, S, P, J, O>` holds the store, the run lock's guard for the whole session, the proposer, the job runner, the observer, the inputs, the options, the run ID, and the set of generations whose batch it completed. Nothing else is kept between steps.
- `start` runs these checks, in order, before recording anything: `check_start` (prices), `JobRunner::check` on baseline trial 0, and `lock_run`. It then stores the manifest bytes as a blob, appends `RunStarted` with the lock's root image, and runs `check_resume` against the ledger.
- `resume`, in order: lock, read, fold (an empty ledger is `RunNotFound`), read the recorded manifest back from its blob, `check_resume`, then `JobRunner::check`. On `Resume::ManifestChanged` it stores the new bytes and appends `ManifestChanged`.
- `calibrate` loops on `next_step(.., target 0)` and records each baseline trial as `Spent { Candidate }`, then `Spent { Judge }` when the judge ran (each only if absent), then `Judged { logs }`. When every trial is recorded it checks `baseline_ready` and returns `noise_floor` and `noise_check` for the objective. It is idempotent.
- `run_to(target)` calibrates first and then performs each step:
  - **Propose:** the parent tree, then the route, history, notes, and `next_model_call`. On success it records `spend_once` per resolved model, or once under the spec's model, then `admit`; an empty admission concludes at once, and otherwise every `missing_proposed` is appended. On failure it records the call's spend if there was usage. A provider stop then appends `StopRequested` and returns `Stopped`, infrastructure is `DriverError::Infra`, and anything else is `Propose`.
  - **RunTrial:** builds the request from the recorded diff, runs it through `JobRunner::run`, records it, and emits `Judged` with the objective delta against the incumbent.
  - **Conclude:** `conclude` with the batch's drop count, then `Selected`.
  - A partly recorded batch is completed before any `RunTrial` or `Conclude`, and the step is asked again.

**Task 2 (bfe76a5), the `driver` test target:** `support.rs` has `FakeJobs<F>` (a closure answers each trial, the preflight result is fixed at construction, requests are recorded, a poisoned lock fails closed as `JobFailure::Infra`, and `Debug` prints only a count), `FaultStore<S>` (one scripted `StoreError` on the Nth append; its guard is the inner store's), `Recorder`, and the fixtures. `flow.rs` has 14 tests, one or more per behavior row:

| Behavior | Test |
|---|---|
| Sweep to target 2: `RunStarted` with the root image, three `Spent, Spent, Judged` baseline triples, one batch blob for generation 1, `Improved` winner, generation 2's parent is the winner, record-before-act order checked for every `Judged` and `Selected` | `sweep_run_to_two_records_calibration_then_two_generations_in_order` |
| single_shot: one call, `Spent { ModelCall { 0, haiku } }` at a lower seq than the first `Proposed`, both `Origin::Model` | `single_shot_records_the_call_spend_before_its_proposals` |
| A repeated experiment is not recorded again and counts in `Selected { 2 }.dropped` | `single_shot_repeat_of_an_earlier_experiment_is_dropped_and_counted` |
| An empty reply concludes as `NoValidCandidates`, with its spend first | `empty_provider_reply_concludes_with_no_valid_candidates` |
| An unscored baseline gives `BaselineUnscored { 3 }` with no model call | `unscored_baseline_stops_before_any_model_call` |
| A floor over three trials; a second `calibrate` appends nothing | `calibrate_returns_a_floor_over_three_trials_and_appends_nothing_again` |
| `PricesMissing` and a refused `check` each write no ledger line | `start_with_an_unpriced_model_records_nothing`, `start_refused_by_the_job_check_records_nothing` |
| `RootImageChanged` names both digests | `resume_with_another_root_image_is_refused_naming_both` |
| A `spend_usd` edit records `ManifestChanged { from_generation: 2 }` and continues to generation 2; a run-command edit is `ManifestChanged` and appends nothing | `resume_after_a_budget_edit_records_manifest_changed_and_continues`, `resume_after_a_run_command_edit_is_refused_and_records_nothing` |
| `ProviderLimit` records the spend, then `StopRequested`, and returns `Stopped`; no `Selected` | `provider_limit_records_its_spend_then_the_stop_and_ends_the_session` |
| A conflict on append 3 gives `Conflict { run, 7, "judged" }`, and the ledger holds only the three earlier lines | `conflicting_append_stops_the_driver_and_records_nothing_more` |
| The observer sees `Calibrated` ×3, `Proposed`, the `Judged` events in record order with delta 1.0, then `Selected` | `observer_sees_calibration_then_proposed_judged_and_selected_in_record_order` |

Two temporary mutations, each reverted before commit, showed the tests catch regressions. Zeroing the batch's drop count at conclusion failed the repeat test. Recording the candidate spend under the wrong source failed 9 tests.

## Verification

Each command below exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 1644 passed, 0 failed, 23 ignored, across 89 result lines; `--test driver` passed 14
- `cargo deny check`: advisories, bans, licenses, and sources all ok
- `cargo doc --workspace --no-deps`
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed `ok` for all 11 crates. The C-02 grep printed nothing, and no `#[expect(` appears under `crates/`.

Acceptance greps:
- The four `pub fn start|resume|calibrate|run_to` lines are present.
- The `next_step|lock_run|spend_once|missing_proposed|baseline_ready|conclude(` grep matches 20 lines.
- `RETRY_PAUSE` is defined once.
- `minos-loop/Cargo.toml` names only `minos-core`, `minos-store`, and `minos-propose`.
- `main.rs` has both `mod` lines.
- `impl<F> JobRunner for FakeJobs<F>` appears once.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] `resume` preflights before recording `ManifestChanged`**
- **Found during:** Task 1
- **Issue:** The plan's order (append `ManifestChanged`, then `jobs.check`) would record an event before every check had passed. That contradicts the plan's own must-have that neither constructor records anything first.
- **Fix:** `check_resume`, then `JobRunner::check`, then the manifest blob and `ManifestChanged`.
- **Files modified:** crates/minos-loop/src/driver.rs
- **Commit:** 77158e9

**2. [Rule 3 - Blocking] An eight-argument private constructor**
- **Found during:** Task 1
- **Issue:** clippy's `too_many_arguments` fires above seven arguments, and silencing it would add the codebase's first `#[expect]`.
- **Fix:** The struct literal is built inline in `start` and `resume`.
- **Commit:** 77158e9

**3. [Rule 1 - Bug] Test fixture: `prices.toml` requires a `[models]` table**
- **Found during:** Task 2
- **Fix:** The unpriced fixture now prices `sonnet` only (`PRICES_WITHOUT_HAIKU`).
- **Commit:** bfe76a5

## TDD Gate Compliance

Task 2 is marked `tdd="true"`, but the driver it tests was built and committed in Task 1 (`feat` 77158e9) before the tests (`test` bfe76a5), as the plan orders its tasks. So no failing RED run preceded GREEN. The two mutation runs above stand in for that evidence.

## Known Stubs

None. `Calibrated::Stopped` and `DriverOptions::retry_pause` have no producer or reader yet. Both are the interface 07-15's safe points and infrastructure retries build on, and the plan defines them for that purpose.

## Threat Flags

None. The driver adds no endpoint, file access, or trust-boundary surface. It reads the store only through the `Ledger`, `Blobs`, and `RunLock` traits. Model and job output reach it only as validated `Proposals` and `JobResult` values.

## Self-Check: PASSED

- FOUND: crates/minos-loop/src/driver.rs, crates/minos-loop/tests/driver/main.rs, support.rs, flow.rs
- FOUND commits: 77158e9, bfe76a5
