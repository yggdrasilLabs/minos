---
phase: 07-runner-and-loop
plan: 07
subsystem: minos-loop (contracts and the pure stage function)
status: complete
tags: [loop, seam, errors, observer, resume, proptest, tdd]
requires:
  - "07-01: RunState::judged and the other fold accessors"
  - "07-03: the minos-loop crate stub and its clippy policy"
  - "07-05: StoreError::Conflict and RunLocked"
provides:
  - "minos_loop::job: JobRunner (run, check) and JobFailure (Infra, Config, is_infra)"
  - "minos_loop::error::DriverError with 22 variants, each documenting its blame"
  - "minos_loop::observe: Observer, LoopEvent (Calibrated, Proposed, Judged, Selected, Retrying, Stopped), NoopObserver"
  - "minos_loop::stage: Step (Calibrate, Propose, RunTrial, Conclude, Done), next_step, CANDIDATE_TRIALS = 1"
  - "crates/minos-loop/tests/stage.rs: 11 table tests and 2 proptests"
affects: [07-08, 07-09, 07-10, 07-11, 07-12, 07-13, 07-14, 07-16]
tech-stack:
  added: []
  patterns:
    - "The job seam is defined in the loop and implemented in the CLI, so minos-loop and minos-runner never depend on each other"
    - "Resume as a fold: next_step reads only RunState and fixed run inputs, so the driver keeps no position of its own"
    - "A synthetic driver in the property test performs only the steps next_step returns, and its ledgers feed the permutation property"
key-files:
  created:
    - crates/minos-loop/src/job.rs
    - crates/minos-loop/src/error.rs
    - crates/minos-loop/src/observe.rs
    - crates/minos-loop/src/stage.rs
    - crates/minos-loop/tests/stage.rs
    - crates/minos-loop/tests/stage.proptest-regressions
  modified:
    - crates/minos-loop/src/lib.rs
decisions:
  - "JobFailure carries &'static str in both variants (the plan's shape), not a ConfigCause enum as 07-RESEARCH Pattern 1 sketched; the CLI adapter maps RunnerError onto fixed text"
  - "Wrapped errors (Propose, Route, Policy, Fold, Request, Cost) are #[error(transparent)], matching ProposeError's style; Store keeps the run ID in its message and the StoreError as its source"
  - "BackendChanged prints the closed BackendKind and Arch enums with Debug, because neither has a Display impl"
  - "Step has no BaselineReady variant (the plan's five variants, not 07-RESEARCH Pattern 5's six); the unscored-baseline check (DriverError::BaselineUnscored) belongs to the driver once Calibrate stops being returned"
  - "LoopEvent derives Eq as well as the plan's Debug, Clone, PartialEq, since every field is Eq"
metrics:
  duration: "about 7 minutes"
  completed: 2026-10-22
  tasks: 2
  files: 7
---

# Phase 7 Plan 07: Loop contracts and the pure stage function Summary

`minos-loop` now holds the contracts every later loop plan builds on: the `JobRunner` seam with its blame split, `DriverError` naming every stop without an outcome, the `Observer` seam with text-free events, and `next_step`, a pure function of the ledger fold that makes resume a fold rather than a journal. The stage function has table tests and property tests.

## What was built

**Task 1 (db8449b):** Four modules.
- `job.rs`: `JobRunner { run(&JobRequest) -> Result<JobResult, JobFailure>, check(&JobRequest) -> Result<(), JobFailure> }` and `#[non_exhaustive] JobFailure { Infra { what }, Config { what } }` with `is_infra`. `Infra` is retried at the same `LaunchKey` and counted toward the breaker (D-16). `Config` stops the driver and records nothing.
- `error.rs`: `DriverError` with every variant the plan lists (`RunLocked`, `RunNotFound`, `Conflict`, `Store`, `ManifestChanged`, `BaseMismatch`, `RootImageChanged`, `JudgeChanged`, `PricesMissing`, `UnpricedModel`, `Cost`, `BackendChanged`, `BaselineUnscored`, `Job`, `Infra`, `Propose`, `Route`, `Policy`, `Fold`, `Request`, `Batch`, `DiffRecord`). Each variant's docs say whether it is a configuration error, corruption, or unrecorded infrastructure, and whether a human must act. Messages name IDs, numbers, and fixed text only.
- `observe.rs`: `Observer` (plus a blanket `&mut O` impl), `NoopObserver`, and `LoopEvent`. Every field is an ID, a number, a closed enum, or a validated `Outcome`, `Decision`, or `Metric`. No event holds a `String`.
- `lib.rs`: crate docs covering the lane, generics without trait objects, record before act, the fold as the only state, and validated output; it also re-exports the principal types.

**Task 2 (TDD; RED 9a7357e, GREEN c3fbed2):** `stage.rs` with `CANDIDATE_TRIALS = 1` (its reason cites 07-RESEARCH A4 and NEXT-05), `Step`, and `next_step(state, judge, baseline_trials, candidate_trials, target)`. The order is: the lowest unjudged baseline trial, then `Done` once `last_concluded_generation() >= target`, then `Propose` for generation `last_concluded + 1` when it has no proposals, then the first unjudged `(exp, trial)` in `ExperimentId` and trial order, then `Conclude`. `tests/stage.rs` has 11 table tests, one or more per behavior row, plus three extra rows: no baseline trials, a target below the last concluded generation, and `candidate_trials = 2` ordering. It also has two proptests:
- A synthetic driver appends only the record that completes each returned step: a `Judged` with a generated scored or crashed outcome, a generated batch of 0 to 3 `Proposed` (an empty batch appends its `Selected` directly), or a `Selected` that is `Improved` or `Kept`. The test checks that calibration precedes any proposal, that no step comes back after its record exists, and that `Done { generation: target }` arrives within a computed step bound.
- Any prefix of a driven ledger, with the records after `RunStarted` permuted, gives the same step.

## Verification

Full gate, each command exiting 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 1477 passed, 0 failed, 23 ignored
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok
- `cargo doc --workspace --no-deps`
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed `ok` for all 11 crates, and the C-02 grep printed nothing.

Acceptance greps:
- The `JobRunner` signature grep counts 2.
- The `DriverError` variant grep counts 7.
- `minos-loop/Cargo.toml` names only `minos-core`, `minos-store`, and `minos-propose`.
- `pub fn next_step` and `pub const CANDIDATE_TRIALS: u32 = 1` each appear once.
- `cargo test -p minos-loop --test stage` passes 13 tests.

RED showed 10 of 13 failing against a stub that always returned `Done`. A temporary mutation (`>=` to `>` in the target check, reverted before commit) failed 4 tests, both proptests among them, so the properties catch an off-by-one in termination.

## Deviations from Plan

None in behavior. These are shape choices within the plan's latitude, recorded under decisions: `JobFailure::Config` uses fixed text rather than a cause enum, wrapped errors are transparent, `LoopEvent` also derives `Eq`, and `BackendChanged` formats its enums with `Debug`.

`crates/minos-loop/tests/stage.proptest-regressions` was generated by the RED run and the mutation check. It is committed, as the plan asks, and both seeds pass against the real implementation.

## TDD Gate Compliance

RED `test(07-07)` 9a7357e precedes GREEN `feat(07-07)` c3fbed2. No refactor commit was needed.

## Known Stubs

None. Several `DriverError` variants (`Batch`, `DiffRecord`, `PricesMissing`, and others) have no producer yet. They are contracts the driver plans (07-08 to 07-13) raise, not stubs that block this plan's goal.

## Threat Flags

None. No new I/O, endpoint, or trust-boundary surface was added. The seam and the events carry validated values only (T-07-07-02), and the lane is unchanged (T-07-07-04).

## Self-Check: PASSED

- FOUND: crates/minos-loop/src/job.rs, error.rs, observe.rs, stage.rs, lib.rs; crates/minos-loop/tests/stage.rs; crates/minos-loop/tests/stage.proptest-regressions
- FOUND: commits db8449b, 9a7357e, c3fbed2
