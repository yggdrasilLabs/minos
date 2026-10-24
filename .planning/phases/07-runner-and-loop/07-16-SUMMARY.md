---
phase: 07-runner-and-loop
plan: 16
subsystem: minos-loop (driver tests), CLAUDE.md
status: complete
tags: [resume, crash-injection, run-lock, design-of-record]
requires:
  - "07-13 Driver::{start, resume, run_to}, FaultStore, FakeJobs"
  - "07-15 safe points, retries, session-relative stops"
  - "07-05 RunLock, StoreError::RunLocked, StoreError::Conflict"
provides:
  - "Exhaustive crash-point resume equivalence for the sweep and single_shot tiers (LOOP-05)"
  - "Second-driver refusal on MemoryStore and two FileStore handles (LOOP-06)"
  - "CLAUDE.md section ## minos-runner and minos-loop, plus Workspace, Diff, Ledger, minos-store, Environment, Cost, and Open questions updates"
affects: [07-17, phase 8]
tech-stack:
  added: []
  patterns:
    - "Crash injection through the store: fail append N with a non-retryable StoreError::Poisoned, then resume a fresh driver on the inner store"
    - "A re-measuring job runner for the resumed session's first job, so a crash between Spent and Judged proves the first measurement stands"
key-files:
  created:
    - crates/minos-loop/tests/driver/resume.rs
    - crates/minos-loop/tests/driver/lock.rs
  modified:
    - crates/minos-loop/tests/driver/main.rs
    - crates/minos-loop/tests/driver/support.rs
    - CLAUDE.md
decisions:
  - "The resumed session's job runner reports a different candidate usage for its first job whenever the crash point lies after a trial's first Spent and before its Judged, because a fully deterministic fake makes a re-appended Spent an equal duplicate and hides a missing spend_once check (shown by mutation)"
  - "Crash points run from append 1 to the last append (append 0 is RunStarted, whose failure leaves no run to resume), and each test checks that the five named crash points are among them by the records either side"
  - "The single_shot script repeats one reply, so a re-proposal after a crash answers exactly what the lost call did; generation 2 then admits only the rewrite the winner lacks"
  - "No driver change was needed: every crash point already resumed to the uninterrupted result"
metrics:
  duration: "about 9 minutes"
  completed: 2026-10-24
  tasks: 3
  files: 6
---

# Phase 7 Plan 16: Resume equivalence, the run lock end to end, and the design of record Summary

The test now fails every append of a sweep run and a single_shot run in turn, then resumes a fresh driver on what survived. Every crash point reaches the same fold as the uninterrupted run. The single_shot tier differs by one spend only: the repeated model call D-07 allows. A second driver on a held run is refused on both stores. CLAUDE.md now describes the runner and the loop as built.

## What was built

**Task 1 (`aa7d0c6`), `tests/driver/resume.rs`.** Shared pieces:
- `uninterrupted` counts the appends of a clean run. It asserts that each append landed as a new line, so append N is line N.
- `crash_then_resume` runs on `FaultStore<MemoryStore>`, which fails append N with `StoreError::Poisoned`. The session must end with exactly `DriverError::Store { Poisoned }`. A fresh `Driver::resume` on `store.inner()` then has to reach generation 2. Any other result, a `Conflict` included, fails and names N.
- `crash_points` labels each N by the records either side of it. It fails unless all five named points occur: baseline `Spent` to `Judged`, batch blob to first `Proposed`, `Proposed` to `Proposed`, `Spent { Candidate }` to `Spent { Judge }`, and last `Judged` to `Selected`.

The two tests:
- Sweep, target 2: `fold` of the resumed ledger `==` `fold` of the uninterrupted one at every N.
- single_shot on `FakeProvider`: decisions, every `Proposal`, and every `Judgment` must be equal. Every uninterrupted spend must be present and unchanged. The only extra allowed is one `Spent { ModelCall { call + 1 } }` with the fixed token usage, at the crash point between a call's `Spent` and its generation's first `Proposed`. The test asserts that this happens exactly twice, once per generation.

Where N falls after a trial's first `Spent` and before its `Judged`, the resumed session's first job reports a new candidate usage, and the fold must still equal the uninterrupted one (Pitfall 1). `support.rs` gained `FaultStore::inner`, `FaultStore::appends`, and `deterministic` (a result that depends only on the request).

**Task 2 (`7db47b4`), `tests/driver/lock.rs`.** Four tests:
- `MemoryStore`: a resume of a held run is `RunLocked { run }` and changes nothing; after `drop(holder)`, a resume reaches generation 2.
- Two `FileStore::open` handles over one temporary `.minos`: the same two checks.
- `Driver::start` with the same inputs and `started_at` while the run is held: refused and appends nothing, on both stores.
- The refusal's message names the run hex and says `wait` and `minos stop`.

**Task 3 (`dbf04ad`), CLAUDE.md:**
- Workspace: new lines for `minos-runner` and `minos-loop` as the plan worded them. The `minos-cli` lib now lists `RunnerJobs`.
- Diff: `CumulativeDiff` and `BaseTree`, the three ways to mint one, and `Experiment::candidate` taking it. `diff_trees` stays generic and parent-relative for the sweep (D-12).
- Ledger: the sketch shows `root_image`, `batch`, and `logs`. A new sentence says each is additive under `"v":"1"` and part of no key, and that `JobResult` carries the judge VM's usage separately (D-09, D-10, D-18).
- minos-store: `Err(StoreError::Conflict { seq })` replaces the old `Duplicate`/`Conflict` sentence (D-08). `RunLock` covers `driver.lock` with `File::try_lock`, the guard released on drop, and a separate file from `ledger.lock` (LOOP-06).
- Environment: the D-10 deferral is replaced. The root image is recorded at start, and `env build --update` means a new run.
- New section `## minos-runner and minos-loop`:
  - the runner's steps and its three blame classes, `RUNNER_VCPUS`, host usage, log tails at `MAX_STAGE_LOG_BYTES`, and `RunnerJobs`;
  - the loop's `next_step`, the absolute target, calibration as generation 0, `CANDIDATE_TRIALS`, record before act, the batch blob, the run lock, start and resume checks with `ManifestChanged`, prices, the safe points, session-relative stops, retries and `RETRY_PAUSE`, the backend pin, the observer, and the proof on fakes.
- Cost: the cap may be passed by at most one VM job or one model call (D-15). The undercount is at most one job per crash. Resolved model IDs must be priced, before any call and after the first call (D-13).
- Open questions: the Phase 8 question on a repeated stop key.

## Verification

Each gate command exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 1673 passed, 0 failed, 26 ignored. The driver target has 38 tests, 6 of them new. The known `minos-propose` timing flake did not occur.
- `cargo deny check`: advisories, bans, licenses, and sources ok.
- `cargo doc --workspace --no-deps`: no warnings.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed `ok` for all 11 crates, and the C-02 grep printed nothing.

Acceptance greps:
- `mod resume;` and `mod lock;` each appear once in `main.rs`.
- `Poisoned` and `fold(` appear in `resume.rs`, and `FileStore::open` appears on 4 lines of `lock.rs`.
- In CLAUDE.md:
  - `## minos-runner and minos-loop` appears once.
  - The old `Duplicate`/`Conflict` wording has 0 matches.
  - The three field signatures each appear.
  - `D-19` is on 3 lines.
  - `one VM job or one model call` appears once.
  - `driver.lock` appears with `RunLock` and `try_lock`.

Mutation checks, each reverted by copying the file back before any commit:
- `complete_batch` never completing a batch past generation 0 failed both resume tests, at the "between two Proposed of one batch" point.
- `spend_once` always appending passed at first, because the deterministic fake made the re-append an equal duplicate. That led to the re-measuring runner above. With it, the same mutation fails both tests at append 2 with "already holds a different spent at seq 1".

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] The crash test re-measures a re-run trial**
- **Found during:** Task 1
- **Issue:** The plan's fully deterministic `FakeJobs` (outcome and usage a pure function of the request) cannot detect a missing `spend_once` check. A re-run reports the same usage, so the re-append is an equal duplicate. Pitfall 1, a crash between a trial's `Spent` and its `Judged`, was therefore not actually exercised.
- **Fix:** When the crash point lies after a trial's first `Spent` and before its `Judged`, the resumed session's first job reports a different candidate usage. Exact fold equality is still required, so the first measurement must stand.
- **Files modified:** crates/minos-loop/tests/driver/resume.rs
- **Commit:** aa7d0c6

No driver fix was needed. Every crash point resumed to the uninterrupted result on the code as 07-13 and 07-15 left it.

## TDD Gate Compliance

Tasks 1 and 2 are `tdd="true"`, but they verify behavior that 07-13 and 07-15 already built, so both test commits passed when written, and there is no `feat` commit. The plan says to fix the driver only if a crash point fails, and none did. In place of a failing RED run, the mutation checks above show the tests fail when the behavior they guard is removed.

## Known Stubs

None.

## Threat Flags

None. No production code changed. The threat register's mitigations are the tests and the docs:
- T-07-16-01: exhaustive crash injection on both tiers.
- T-07-16-02: no `Conflict` at any crash point, and exact fold equality for the sweep.
- T-07-16-03: refusal on both stores.
- T-07-16-04: CLAUDE.md written from the summaries' names and constants, with the overshoot and undercount stated plainly.
