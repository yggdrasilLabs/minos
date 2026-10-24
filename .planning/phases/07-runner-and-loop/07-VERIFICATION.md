---
phase: 07-runner-and-loop
verified: 2026-10-24T00:00:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
gaps: []
deferred: []
human_verification: []
---

# Phase 7: Runner and Loop Verification Report

**Phase Goal:** Generations run end to end (route, propose, validate, candidate VM, judge VM, select, record), and an interrupted, over-budget, or failing run resumes or stops from the ledger with the same result as an uninterrupted one.
**Verified:** 2026-10-24
**Status:** passed
**Re-verification:** No, initial verification

## Method

SUMMARY.md claims were treated as unproven. I read the production source of `minos-loop` (`driver.rs`, `lib.rs`, `Cargo.toml`, `clippy.toml`), `minos-runner` (`pipeline.rs`, `usage.rs`, `map.rs`), `minos-cli` (`jobs.rs`, `judge_push.rs`), `minos-store` (the `RunLock` impls), and the resume, lock, caps, breaker, stops, and pins test modules. I re-ran the cheap greps myself, and I ran three test targets that cover the central claims (below). I did not run the ignored VM tests; the 07-14 record in 07-EVIDENCE.md stands as the VM evidence, because the build note says not to rerun it.

Commands I ran:

- `cargo deny check bans`: `bans ok` (the lane wrapper entries are enforced).
- `cargo test -p minos-loop`: exit 0. `blocks` 42, `driver` 38, `stage` 13 passed.
- `cargo test -p minos-runner -p minos-store`: `map` 53, `pipeline` 18, `usage` 7, `contract` 43, `file_blobs` 17, `file_ledger` 25, store unit tests 9, all passed.
- The C-02 grep (`#!?\[allow\(`), the LOOP-01 grep (`Untrusted` exits outside runner and sandbox), the lane grep on both `Cargo.toml` files, and a `TBD|FIXME|XXX|TODO|HACK|todo!|unimplemented!` scan: all printed nothing or the expected lines. No `.unwrap()` or `.expect(` in `minos-loop` or `minos-runner`.

## Goal Achievement

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Against the in-memory store, a fake provider, and a fake job runner, the loop drives route, propose, validate, run, fold, select, and record, records before acting, resumes to the same result from any interruption, refuses a second driver, and depends only on core, store, and propose | VERIFIED | `driver.rs` `propose` calls `route`, `Proposer::propose`, `admit`; `run_trial` calls the `JobRunner`; `select` calls core `conclude` then appends `Selected`. The driver keeps no position: `drive` refolds the ledger and performs exactly `next_step`. `resume.rs` crashes before every append (every N from 1 to the last) via `FaultStore`, resumes, and requires the fold to equal the uninterrupted fold for the sweep tier (exact equality, including a re-measured trial after a crash between `Spent` and `Judged`). `lock.rs` proves `RunLocked` on `MemoryStore` and on two `FileStore` handles, and that nothing is appended by the refused `start`. `minos-loop/Cargo.toml` names only `minos-core`, `minos-store`, `minos-propose`; `deny.toml` bans the other edges and `cargo deny check bans` passed. `clippy.toml` bans clock, env, `Command`, `File`, `HashMap`, and `ExperimentId::of`. I ran the loop tests: 93 passed. |
| 2 | The run stops with a recorded `StopRequested` when the spend or token cap would be exceeded (checked before each generation and model call), after K empty generations, or after K consecutive infrastructure failures | VERIFIED | `safe_point` runs the pure core `circuit_breaker` (generation safe point) and `spend_ok` (generation, model call, after trial) and appends `StopRequested` through `stop`; `retry` appends `StopRequested { InfraFailures }` at `max_infra_failures`. `caps.rs` (5 tests: spend cap crossed by one job, token cap, unpriced model, calibrate at the cap), `breaker.rs` (6 tests: two empty generations, three job infra failures record nothing, success resets the streak, provider infra with usage still trips the breaker, retried append does not rerun the job), and `stops.rs` (4 tests: old stop does not block resume, cap re-trips on resume, cap raised resumes) pass. `spend_ok` in core compares metered spend and `total_tokens` to the manifest caps. The "model-call cap" in the criterion is the token cap (`max_tokens`); the manifest has no separate call-count key. |
| 3 | On real VMs the runner runs the candidate VM then the judge VM; only the judge mounts judge data read-only; no shared process or disk; usage is host-measured; a failed gate yields `Rejected` without scoring objectives; every `Judged` records backend and architecture; a run is pinned to one backend class; only `minos-runner` turns `Untrusted` output into an `Outcome` | VERIFIED | `Runner::run` in `pipeline.rs`: candidate plan built from `InputDisk::candidate` (no judge-data part), judge plan built with `SandboxSpec<Judge>::secrets(judge_data)`, the only method that attaches it (compile-fail case proves a Candidate cannot). Judge input is `store_judge_input` of the candidate's `out/` files only, under a separate launch key, job directory, and input disk. Usage is `host_usage(Finished::elapsed, machine)`, where `elapsed` is the backend's own `Instant` (`backend.rs:234`); the runner's `clippy.toml` bans clocks, and a pipeline test forges timing in candidate output with no effect. `judged_outcome` calls core `check_scorecard`, which checks gates first and returns `GateFailed` before objectives are read. `result` stamps `Capabilities::backend_info(root)` onto every `JobResult`, and the driver's `pin_backend` refuses a result whose `(backend, arch)` differs from the run's recorded class (`pins.rs`, 3 tests). The LOOP-01 grep prints nothing: no crate outside `minos-runner` and `minos-sandbox` calls an `Untrusted` exit. The two new sink exits were approved by the user at the 07-04 checkpoint ("trait") before code landed. Real-VM record (07-EVIDENCE.md "Runner on real VMs (07-14)", not rerun here): 3 passed on the production `Runner<KrunBackend, FileStore>` after a passing `host doctor`; baseline `Scored`, a candidate's `out/` reached the judge and scored, a gate-failing judge gave `Rejected(GateFailed { gate: probe_ok })`; every row records Krun on Arm64 and `mem_mib_ms` equals `vcpu_ms / 2 * 2048`. I ran the runner tests: `map` 53, `pipeline` 18, `usage` 7 passed. |
| 4 | `minos judge push <dir>` stores judge data in the local blob store as a new judge version with its hash recorded; the data never appears in the source tarball or image build context | VERIFIED | `judge_push.rs` packs with `pack_judge_data` (walker, hard denylist, secret scan, 96 MiB cap), refuses a directory inside the project not covered by `[source] exclude` (`DirInSnapshot`), requires every path the judge command names, refuses different data under a recorded label without `--update`, stores the tarball with `put_reader` (blob ID must equal the tree digest), and writes a `[[judge]]` entry to `minos.lock` last. `.minos/` is on the hard denylist, so the stored blob is not packed. `snapshot` takes the lock's judge file digests and refuses with `JudgeOverlap`, and `env build` passes them, so judge data reaches neither the tarball nor the build context. `tests/judge_push.rs` has 12 tests (first push, same push, changed data refused, dir in snapshot refused, secret refused, over cap refused, and CLI wiring), recorded as 12 passed in the final gate. I read the source; I did not rerun the CLI test target (it links the VMM crates). |
| 5 | `cargo clean` has been run after verification passes (GATE-06) | VERIFIED, with a handoff | 07-EVIDENCE.md records `cargo clean` ("Removed 185174 files, 19.8GiB") after the final gate, and `target/` was absent when I started. That clean ran before this verification, and my test runs rebuilt about 1.0 GiB under `target/`, so the orchestrator must run `cargo clean` once more after this report to meet "after verification passes". |

**Score:** 5/5 truths verified (0 present but behavior-unverified).

Every behavior-dependent truth (resume equivalence, cancellation and cleanup ordering, record-before-act, stop semantics, retry streak) has a passing behavioral test that I ran or whose passing result is in the recorded gate, so none is left at PRESENT_BEHAVIOR_UNVERIFIED.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/minos-loop/src/driver.rs` (1166 lines) | Start, resume, calibrate, `run_to`, safe points, retry | VERIFIED | Substantive and wired: used by the stage function, `admit`, `record`, `history`; no stubs, no debt markers. |
| `crates/minos-loop/src/{stage,admit,record,inputs,job,observe,error,history}.rs` | Pure stage function, batch blob, recorder, seam | VERIFIED | `next_step`, `admit`, `spend_once`, `check_resume`, `JobRunner`, `Observer` all present and exercised by tests. |
| `crates/minos-runner/src/{pipeline,map,usage,error}.rs` | Runner, blame table, host usage | VERIFIED | Fully read. Blame is three-way (`Outcome`, `Infra`, `Config`) with closed cause enums. |
| `crates/minos-cli/src/jobs.rs` | `RunnerJobs` adapter, `judge_inputs` | VERIFIED | Maps `RunnerError` by `is_infra` only; wired to the loop's `JobRunner`. |
| `crates/minos-cli/src/judge_push.rs` and `main.rs` command | `minos judge push` | VERIFIED | Wired in `main.rs` (`judge push`, `--update`). |
| `crates/minos-store/src/file/run_lock.rs`, `memory.rs` | `RunLock` | VERIFIED | `driver.lock` separate from `ledger.lock`; contract tests cover both stores. |
| `crates/minos-{loop,runner}/clippy.toml`, `deny.toml` | Lane and ban enforcement | VERIFIED | Ban proofs recorded in 07-EVIDENCE.md; `cargo deny check bans` passes. |
| `crates/minos-cli/tests/runner_vm.rs` | Ignored VM test of the production runner | VERIFIED | 3 ignored plus 1 gate test; run result recorded. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| Driver | `JobRunner` seam | `run_job` / `check_job` | WIRED | Loop never names sandbox or runner types. |
| CLI `RunnerJobs` | `minos_runner::Runner<B, FileStore>` | `JobRunner` impl | WIRED | Only crate where loop and runner meet. |
| Runner | `SandboxSpec<Judge>::secrets` | `judge_plan` | WIRED | Only place judge data enters a plan. |
| Runner | `Untrusted` exits | `take_status`, `into_scorecard`, `store_judge_input`, `store_tail` | WIRED | Only production caller (grep proof). |
| Driver | core `circuit_breaker`, `spend_ok`, `conclude`, `route` | `safe_point`, `select`, `propose` | WIRED | Decisions are core's pure functions; the driver only chooses where. |
| Driver | `RunLock` | `lock()` in `start` and `resume` | WIRED | Taken before the first read; guard held for the session. |
| `judge push` | `minos.lock` and `env build` snapshot | `[[judge]]` entry, `judge_file_digests` | WIRED | Overlap refusal blocks judge data in the build context. |

### Data-Flow Trace (Level 4)

| Artifact | Data | Source | Produces Real Data | Status |
|----------|------|--------|--------------------|--------|
| `Judged` record | outcome, backend, logs, usage | `JobResult` from `Runner::run`: scorecard via `into_scorecard`, usage from `Finished::elapsed`, backend from `Capabilities::backend_info` | Yes (real-VM run shows scored cards, nonzero usage, Krun/Arm64, log blobs) | FLOWING |
| `Spent` records | candidate and judge usage | `JobResult::usage()` and `judge_usage()` | Yes | FLOWING |
| `Selected` | decision | core `conclude` over the folded ledger | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Loop driver flow, caps, breaker, stops, pins, resume, lock | `cargo test -p minos-loop` | 93 passed, 0 failed | PASS |
| Runner pipeline, blame map, host usage | `cargo test -p minos-runner` | 78 passed, 0 failed | PASS |
| Store contract incl. conflict and run lock on both stores | `cargo test -p minos-store` | 94 passed, 0 failed | PASS |
| Lane enforcement | `cargo deny check bans` | bans ok | PASS |
| C-02 no `#[allow(`; LOOP-01 exits only in runner/sandbox | greps | no output | PASS |
| Real VM run | not rerun (ignored; 07-14 record) | 3 passed recorded | SKIP, recorded evidence accepted |

### Probe Execution

SKIPPED: the phase declares no `scripts/*/tests/probe-*.sh` probes. The `minos-probe` guest binary is a test fixture used by the VM tests.

### Requirements Coverage

All 13 Phase 7 IDs in REQUIREMENTS.md (LOOP-01 to LOOP-09, JUDGE-01, JUDGE-03, JUDGE-04, CLI-04) are claimed by at least one plan's `requirements` field, and GATE-06 is claimed by 07-17. No orphaned requirement: JUDGE-02 is mapped to Phase 8 and is not claimed here.

| Requirement | Source Plans | Status | Evidence |
|-------------|--------------|--------|----------|
| LOOP-01 | 07-04, 07-09, 07-12, 07-14, 07-17 | SATISFIED | Runner pipeline; LOOP-01 grep empty; VM run |
| LOOP-02 | 07-04, 07-12, 07-14 | SATISFIED | `secrets` only on Judge; pipeline tests `candidate_plan_has_no_judge_data...`, `judge_plan_carries...` |
| LOOP-03 | 07-03, 07-09, 07-12, 07-14 | SATISFIED | `host_usage`; forged-timing test; runner clippy bans |
| LOOP-04 | 07-01, 07-02, 07-03, 07-06, 07-07, 07-10, 07-13 | SATISFIED | `driver.rs`, `stage.rs`; `flow::` tests |
| LOOP-05 | 07-01, 07-05, 07-06, 07-07, 07-10, 07-13, 07-16 | SATISFIED | `resume.rs` crash-at-every-append tests |
| LOOP-06 | 07-05, 07-16 | SATISFIED | `RunLock`; `lock.rs` |
| LOOP-07 | 07-02, 07-15 | SATISFIED | `caps.rs`, `stops.rs`; overshoot bounded at one job or call (D-15) |
| LOOP-08 | 07-15 | SATISFIED | `breaker.rs` |
| LOOP-09 | 07-03, 07-07, 07-13 | SATISFIED | Entire loop suite runs on `MemoryStore`, `FakeProvider`, `FakeJobs` |
| JUDGE-01 | 07-02, 07-08, 07-11 | SATISFIED | `judge_push.rs`, `pack_judge_data`, overlap refusal |
| JUDGE-03 | 07-09, 07-12, 07-14 | SATISFIED | `check_scorecard` gates first; pipeline test; VM `Rejected(GateFailed)` |
| JUDGE-04 | 07-01, 07-12, 07-14, 07-15 | SATISFIED | `backend_info` on every result; `pin_backend`; `pins.rs` (pins every run, not only speed objectives, which is stricter than the requirement) |
| CLI-04 | 07-11 | SATISFIED | `minos judge push` command and tests |
| GATE-06 | 07-17 | SATISFIED pending final clean | See criterion 5 |

### Anti-Patterns Found

None. No `TBD`, `FIXME`, `XXX`, `TODO`, `HACK`, `todo!`, or `unimplemented!` in the loop, runner, store, or CLI source; no `#[allow(`; no `unwrap` or `expect` in the two new crates.

### Notes (non-blocking, recorded decisions)

- **LOOP-05 has one documented, bounded difference for model-backed tiers (D-07, stated in the roadmap-level plan 07-16 and in the `resume.rs` module docs).** If a crash falls after a model call's `Spent` and before the generation's first `Proposed`, the resumed session calls the model again. Its proposals, judgments, and decisions match the uninterrupted run, and its spends are the uninterrupted run's plus exactly one extra `Spent { ModelCall }`. That is real extra spend and is inherent to a non-deterministic model call that has no recorded result yet; the sweep tier, which has no model, resumes with exact fold equality at every crash point. This is a design decision made before execution, not a gap.
- **Pre-existing flake, not Phase 7:** two `minos-propose` process tests are timing-sensitive (deferred-items.md); they passed in the final gate and in my environment this session did not run them.
- **07-VALIDATION.md** is complete (`status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`, every task row green).
- **Verified-by-record, not re-executed here:** the full-workspace gate (1673 passed, 0 failed), `cargo doc`, the musl guest clippy, fmt, and the real-VM run. I spot-ran the three crates above and the greps; none contradicted the record.

### Human Verification Required

None.

### Gaps Summary

No gaps. The phase goal is achieved: the generation loop, the production runner, judge push, the run lock, the caps and breaker, and resume-from-ledger all exist, are wired, and are exercised by passing tests, with a recorded passing run on real VMs.

### Handoff

I rebuilt about 1.0 GiB under `target/` while running the three test targets above. Run `cargo clean` after this report (GATE-06), and no cargo command after it.

---

_Verified: 2026-10-24_
_Verifier: Claude (gsd-verifier)_
