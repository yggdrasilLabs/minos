---
phase: 07-runner-and-loop
plan: 12
subsystem: minos-runner (pipeline)
status: complete
tags: [runner, pipeline, judge-integrity, metering, LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04]
requires:
  - phase: 07-01
    provides: "JobResult::new(request, outcome, usage, judge_usage, backend, logs), which refuses tokens"
  - phase: 07-04
    provides: "BlobSink, SinkError, store_judge_input (candidate output only), store_tail"
  - phase: 07-06
    provides: "CumulativeDiff::recorded"
  - phase: 07-09
    provides: "RunnerError, InfraCause, ConfigCause, host_usage, RUNNER_VCPUS, Seen, Verdict, candidate_verdict, judge_verdict, judged_outcome"
provides:
  - "minos_runner::pipeline::Runner<B: Backend, S: Blobs> with new, run, check, store, and backend; re-exported as minos_runner::Runner"
  - "ConfigCause::BlobUnsafe (a tampered blob, an unsafe store path, or a poisoned lock) and ConfigCause::JobResult (the result could not be built from host usage)"
  - "Test target crates/minos-runner/tests/pipeline.rs with an in-test FakeBackend"
affects: [07-13, 07-14, 07-16, 07-17]
tech-stack:
  added: []
  patterns:
    - "A private StoreSink adapter is the production BlobSink over Blobs::put"
    - "Observe in the blame table's order (end, then stream, then status) in one generic helper shared by both roles"
    - "check encodes and hashes the guest jobs without storing them, so a pre-run check writes nothing"
key-files:
  created:
    - crates/minos-runner/src/pipeline.rs
    - crates/minos-runner/tests/pipeline.rs
  modified:
    - crates/minos-runner/src/lib.rs
    - crates/minos-runner/src/error.rs
decisions:
  - "Store errors are split three ways: retryable I/O is Infra(Store), a missing or over-cap blob is Config(BlobMissing), and anything else (tampered, unsafe, poisoned) is Config(BlobUnsafe). BlobSourceError from a launch follows the same split, with Io as Infra(BlobIo)"
  - "verify_diff also requires BlobId::of(canonical bytes) == the request's diff blob, so the guest always receives the exact canonical bytes the experiment ID hashes"
  - "Runner::put refuses a store that names the runner's own guest job by another digest (Infra(DigestMismatch)), matching what the sink exits already check"
  - "check also requires the root image blob, which the plan's truths name and its action did not"
  - "A JobResult::new error is Config(JobResult): host usage never carries tokens, so it is a runner defect, never retried"
  - "Log tails are stored whenever the output stream was read, before the verdict, so a rejected candidate still has its build and run logs"
metrics:
  duration: "about 8 minutes (2026-10-24T13:11:17Z to 13:18:59Z)"
  completed: 2026-10-24
  tasks: 2
  files: 4
---

# Phase 7 Plan 12: The runner pipeline Summary

`Runner<B: Backend, S: Blobs>` runs one `JobRequest` end to end. It verifies the diff blob through `CumulativeDiff::recorded`, launches the Candidate VM, and maps what the host observed through the 07-09 table. Only if the candidate passed does it re-pack the `out/` files with `store_judge_input` and launch the Judge VM, with judge data attached only through `secrets`. It then scores gates first and returns one `JobResult` with host-measured usage, capped log tails, and backend provenance. Eighteen tests over a scripted backend prove each step.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | The candidate-then-judge pipeline | `cb12cef` | src/pipeline.rs, src/lib.rs, src/error.rs |
| 2 | Pipeline tests over a scripted fake backend | `dcb68ff` | tests/pipeline.rs |

## What was built

- **`Runner::new`** builds `Machine::new(RUNNER_VCPUS, memory_mib)` and takes the arch from the manifest. A memory value that does not fit is `Config(Machine)`.
- **`Runner::run`** follows the plan's eleven numbered steps, one comment each:
  - judge version check;
  - diff verify;
  - candidate guest job and input;
  - spec, seal, preflight, launch, and wait;
  - `host_usage`;
  - observe and store log tails;
  - `candidate_verdict`;
  - `store_judge_input` (`Repack` is `Rejected(OutputInvalid)`);
  - judge job, spec with `.secrets(judge_data)`, and launch;
  - judge observe, log tail, `judge_verdict`, then `judged_outcome` over `into_scorecard`;
  - `JobResult::new` with `backend_info(root)`.
- **`Runner::check`** does four things without launching:
  - refuses a judge mismatch;
  - requires the root, base, judge data, and diff blobs;
  - encodes and hashes both guest jobs;
  - preflights both plans, the judge's against the empty-ustar placeholder `candidate_out`.
- **Error mapping:**
  - `LaunchError::Blob` follows the blob-source split; every other launch error is `Infra(Launch)`.
  - A wait error is `Infra(Wait)`.
  - A spec error is `Config(Spec)`, and refusals are `Config(Preflight)`.
  - A guest job that does not encode is `Infra(GuestJob)`.
  - A non-exhaustive variant the runner does not know fails closed as infrastructure.
- **Module docs** cover the order, `secrets` as the only way judge data enters a plan, the judge seeing only the re-packed `out/`, host-measured usage, and this module as the only production place output leaves `Untrusted`.
- **Tests** (18, `tests/pipeline.rs`). Each behavior row has its own named test:
  - order and distinct keys;
  - a Candidate plan with `judge_data()` `None`, naming the base and diff;
  - a Judge plan with `judge_data()` set to the runner's data, whose `candidate_out` read back and parsed with the tar crate is exactly `out/result.json` and `out/timing.json`;
  - the baseline;
  - usage of `1500 * 2` and `1500 * 2048`, judge usage of `700 * 2` and `700 * 2048`, despite a forged `out/timing.json` and `.minos/usage.json`;
  - a build failure skips the judge;
  - `GateFailed` with the objective missing;
  - backend provenance;
  - log stages, with the run tail exactly the last `MAX_STAGE_LOG_BYTES`;
  - launch and wait errors;
  - an input-step failure with a placeholder role;
  - `ScratchFull`;
  - `DiffMismatch`;
  - `JudgeMismatch`;
  - the three `check` cases.

## Verification

- `cargo test -p minos-runner --test pipeline`: 18 passed.
- Full gate:
  - `cargo fmt --all --check`: ok.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ok.
  - `cargo test --workspace`: 1630 passed, 0 failed. The known `minos-propose` timing flake did not occur.
  - `cargo deny check`: advisories, bans, licenses, and sources ok.
  - `cargo doc --workspace --no-deps`: no warnings.
  - Guest musl clippy: ok.
  - C-01: 11 of 11 crates ok.
  - C-02 grep: empty.
  - LOOP-01 grep: empty.
- Acceptance greps:
  - one `pub struct Runner<B: Backend, S: Blobs>`;
  - three `pub fn` lines for `new`, `run`, and `check`;
  - 17 lines naming the exits, `secrets(`, `host_usage`, `judged_outcome`, and `backend_info`;
  - no `dyn ` in `pipeline.rs`;
  - two `judge_data()` assertion lines in the tests.

## Deviations from Plan

1. **[Rule 2 - correctness] Two new `ConfigCause` variants in `error.rs`, a file the plan does not list.** `BlobUnsafe` and `JobResult` were left uncommitted by the interrupted earlier attempt. They were kept, and `BlobUnsafe`'s doc was widened to cover a poisoned lock.
   - The plan routes every non-retryable store error to `BlobMissing`. A tampered blob or a symlinked store path is not "missing", and the user needs to know which.
   - `JobResult::new` returns a `Result`, which needs a cause.
   - Commit `cb12cef`.
2. **[Rule 2 - fail closed] The diff blob must be the canonical bytes.** Without this check, a request whose stored diff was not canonical but parsed to the right experiment would send different bytes to the guest than the ones `ExperimentId` hashes. Commit `cb12cef`.
3. **[Rule 2 - fail closed] `Runner::put` checks the returned digest** for the runner's own guest jobs, as the sink exits already do for output. Commit `cb12cef`.
4. **[Rule 2 - correctness] `check` requires the root image blob.** The plan's truths name it, and its action listed only the base, judge data, and diff. Commit `cb12cef`.
5. **Test accessor.** The test's `Launched` record exposes the plan's judge data through a `judge_data()` method, so the LOOP-02 assertions read like the `LaunchPlan` API. Commit `dcb68ff`.

## TDD Gate Compliance

Task 2 is marked `tdd="true"`, but the plan orders it after Task 1's implementation. So the tests were written against existing code, and the history has `feat(07-12)` `cb12cef` before `test(07-12)` `dcb68ff`. No RED run that fails was possible without reverting Task 1. All 18 tests passed on their first green run, after one compile fix to a test (the `GateFailed` field is `gate`) and one clippy fix (a type alias).

## Known Stubs

None.

## Threat Flags

None. The pipeline touches only the surface in the plan's threat register, and each mitigation is in place and tested:

- T-07-12-01: no judge data on the Candidate plan.
- T-07-12-02: the judge input is exactly `out/`.
- T-07-12-03: usage comes from host elapsed time only.
- T-07-12-04: `GateFailed`.
- T-07-12-05: `DiffMismatch` before any launch.
- T-07-12-06: `ScratchFull`, spec, preflight, and missing-blob errors are `Config`.
- T-07-12-07: the LOOP-01 grep is empty.

## Self-Check: PASSED

- FOUND: crates/minos-runner/src/pipeline.rs
- FOUND: crates/minos-runner/tests/pipeline.rs
- FOUND: commits cb12cef, dcb68ff
