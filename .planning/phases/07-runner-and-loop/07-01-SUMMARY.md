---
phase: 07-runner-and-loop
plan: 01
subsystem: minos-core (ledger, job, state, policy)
status: complete
tags: [ledger, wire-format, fold, policy, golden]
requires: []
provides:
  - "Event::RunStarted.root_image, Event::Proposed.batch, Event::Judged.logs (optional, omitted when None, in no key)"
  - "JobResult judge_usage: JobResult::new(request, outcome, usage, judge_usage, backend, logs), JobResult::judge_usage()"
  - "RunState::root_image, backend_classes, has_spend, next_model_call, judged, stops; Judgment::logs; Proposal::batch"
  - "policy::baseline_ready"
  - "four phase-7 golden snapshots with their own SHA-256 pin"
affects: [07-07, 07-10, 07-12, 07-13, 07-15, 07-16]
tech-stack:
  added: []
  patterns:
    - "Additive Option fields under \"v\":\"1\" with #[serde(default, skip_serializing_if = \"Option::is_none\")]"
    - "A second golden list with its own pin, so the existing pin never moves"
key-files:
  created:
    - crates/minos-core/tests/snapshots/golden_wire__record_v1_judged_logs.snap
    - crates/minos-core/tests/snapshots/golden_wire__record_v1_run_started_root_image.snap
    - crates/minos-core/tests/snapshots/golden_wire__record_v1_proposed_batch.snap
    - crates/minos-core/tests/snapshots/golden_wire__job_result_judge_usage.snap
  modified:
    - crates/minos-core/src/ledger.rs
    - crates/minos-core/src/job.rs
    - crates/minos-core/src/state.rs
    - crates/minos-core/src/policy.rs
    - crates/minos-core/tests/golden_wire.rs
    - crates/minos-core/tests/prop_fold.rs
    - crates/minos-core/tests/prop_keys.rs
    - crates/minos-core/tests/prop_policy.rs
    - crates/minos-core/tests/report.rs
    - crates/minos-core/tests/route.rs
    - crates/minos-core/tests/breaker.rs
    - crates/minos-store/tests/file_ledger.rs
decisions:
  - "baseline_ready also requires the objective metric in the reduced score, mirroring conclude's incumbent check exactly, so baseline_ready is false if and only if conclude for generation 1 returns IncumbentUnscored (check_scorecard already requires every objective, so this equals valid_score(..).is_some())"
  - "Phase-7 golden pin is 2da862bd29fbb884f745cc962d8bb9bec57f94b53f0d08b213d6e3b003e4c476, computed with awk and shasum over the four snapshot bodies; the 25-case pin dedebd15... is unchanged"
metrics:
  duration: "about 9 minutes"
  completed: 2026-10-22
  tasks: 3
  files: 16
---

# Phase 7 Plan 01: Ledger provenance fields, RunState accessors, and baseline_ready Summary

The ledger gains three optional provenance fields (`RunStarted.root_image`, `Proposed.batch`, `Judged.logs`). `JobResult` gains a judge usage that is checked for tokens. The fold carries all of them, and six `RunState` accessors give the loop the reads it needs: `root_image`, `backend_classes`, `has_spend`, `next_model_call`, `judged`, and `stops`. `policy::baseline_ready` stops an unscored baseline before any model call. Every existing wire line, golden snapshot, and the 25-case SHA-256 pin are unchanged, byte for byte.

## What was built

**Task 1 (wire format).** Each new field is `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, so lines written before Phase 7 still parse, and a `None` writes nothing. That covers `Judged.logs: Option<BTreeMap<Stage, BlobId>>` (D-09), `RunStarted.root_image: Option<ImageDigest>` (D-10), `Proposed.batch: Option<BlobId>` (D-18), and `RawJobResult.judge_usage`. `Event::key` is untouched, and the key table in the module docs now says the three fields are in no key. `JobResult::new` takes `judge_usage: Option<Usage>` after `usage` and runs `check_usage` on it. `TryFrom<RawJobResult>` runs the same check. There are four new golden cases (`record_v1_judged_logs`, `record_v1_run_started_root_image`, `record_v1_proposed_batch`, `job_result_judge_usage`) in `phase7_goldens()`, pinned by `phase7_golden_bytes_pinned`. Behavior tests cover:
- old lines read the new fields as `None`
- present fields are written under the right names
- unknown keys are still refused, and so are a `Stage` map key outside the enum, a sibling of `logs`, and a non-hex log ID
- the new fields change no key
- judge usage that carries tokens is refused

**Task 2 (fold and accessors).** `Judgment`, `Proposal`, and `RunState` store the new values, and the fold copies them from their events.
- `root_image()` serves the D-10 check.
- `backend_classes()` returns the `(BackendKind, Arch)` set (D-11).
- `has_spend()` serves Pitfall 1.
- `next_model_call(g)` is one past the highest `ModelCall` index in `g`, or 0 when there is none, saturating at `u32::MAX`.
- `judged(exp, judge, trial)` tells whether that trial has a recorded `Judged`.
- `stops()` returns every stop in key order, for D-19.
- `Judgment::logs()` and `Proposal::batch()` return the recorded values.

`stop_requested()` now documents that a resumed driver uses `stops()`. The prop_fold generators now mix in `logs`, batch blobs, root images, and up to three stops. The permutation property checks every new accessor over any record order. Each behavior row has its own unit test.

**Task 3 (`baseline_ready`).** A pure wrapper over the private `valid_score` with `TrialRule::ScoredOnly` that also requires the objective metric. It has unit tests for:
- no trial
- every trial failed
- no scorecard passes the judge
- one scored trial among failures
- only another judge scored

A proptest over generated baseline outcomes, judge choices, and an optional candidate shows it equals the model, and shows that whenever it is false, `conclude(.., 1, ..)` returns `IncumbentUnscored { baseline }`.

## Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 9d4f4ee | test(07-01): add failing golden cases for the phase 7 provenance fields |
| 1 | GREEN | de59855 | feat(07-01): add optional provenance fields to Judged, RunStarted, Proposed, and JobResult |
| 2 | RED | fadd212 | test(07-01): add failing tests for the RunState accessors the loop reads |
| 2 | GREEN | d7b2418 | feat(07-01): fold the provenance fields and add the RunState accessors the loop reads |
| 3 | RED | 817d7e8 | test(07-01): add failing tests for baseline_ready |
| 3 | GREEN | e843e31 | feat(07-01): add baseline_ready, the predicate generation 0 ends with |

## Verification

Each gate command exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (77 test binaries, none failed)
- `cargo deny check` (advisories, bans, licenses, and sources all ok)
- `cargo doc --workspace --no-deps` (no warnings)
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed only `ok` lines, and the C-02 grep printed nothing.

Acceptance checks:
- `git diff --exit-code` on the four existing snapshots the plan names exits 0.
- The `dedebd15...` literal appears once.
- `"logs":{` appears once in the new judged snapshot.
- `ledger.rs` has 3 `skip_serializing_if = "Option::is_none"` lines.
- `pub const fn judge_usage` exists.
- The six accessor greps and the `logs` and `batch` greps each print their lines.
- `baseline_ready` appears 11 times in `prop_policy.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Literal-site updates moved into Task 1's GREEN commit**
- **Found during:** Task 1
- **Issue:** The plan updates the struct literals in prop_fold, prop_keys, prop_policy, report, route, breaker, and file_ledger in Task 2. Until then, those test targets do not compile, and the `state.rs` fold patterns break the library build.
- **Fix:** Task 1's GREEN commit sets the new fields to `None` at all 29 initializer sites and in the prop_fold tamper pattern, and adds `..` to the three `state.rs` patterns. Task 2 replaced the `..` with the real bindings. No test assertion changed. Every commit now builds the workspace.
- **Files modified:** the seven test files above, plus `crates/minos-core/src/state.rs`
- **Commit:** de59855

**2. [Rule 2 - Correctness] `baseline_ready` also checks the objective metric**
- **Found during:** Task 3
- **Issue:** The plan says the function returns whether `valid_score(..)` is `Some`. `conclude` also requires the objective metric in the reduced score.
- **Fix:** Added `.and_then(|score| score.get(objective.metric()))`, so the predicate mirrors `conclude`'s incumbent check exactly. It gives the same result as the plan's form, because `check_scorecard` already requires every objective. The proptest asserts it in both directions.
- **Files modified:** `crates/minos-core/src/policy.rs`
- **Commit:** e843e31

### Notes

- Task 1 was written before its tests. RED was then shown honestly: the implementation was set aside, the original `ledger.rs` and `job.rs` were restored, the 18 compile errors were confirmed, the tests were committed, and the implementation was put back. No stash or reset was used.
- CLAUDE.md's ledger sketch does not yet show the three new fields. Plan 07-16 Task 3 updates it, so this plan leaves CLAUDE.md alone.
- No proptest-regressions file was written in this run.

## Known Stubs

None.

## Threat Flags

None. No new network, auth, file-access, or trust-boundary surface was added. Every mitigation in the plan's threat register is implemented and tested:
- T-07-01-01: both pins, and the existing snapshots unchanged
- T-07-01-02: unknown keys and bad `Stage` keys refused
- T-07-01-03: keys unchanged
- T-07-01-04: logs are blob IDs only
- T-07-01-05: `check_usage` runs on both usages
- T-07-01-06: `baseline_ready` and its agreement proptest

## Self-Check: PASSED

- FOUND: the four new snapshot files and every modified source file
- FOUND commits: 9d4f4ee, de59855, fadd212, d7b2418, 817d7e8, e843e31
