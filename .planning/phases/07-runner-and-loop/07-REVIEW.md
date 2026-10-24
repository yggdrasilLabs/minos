---
phase: 07-runner-and-loop
reviewed: 2026-10-24T00:00:00Z
depth: standard
files_reviewed: 112
files_reviewed_list:
  - crates/minos-backend-krun/src/input_disk.rs
  - crates/minos-backend-krun/tests/harness/mod.rs
  - crates/minos-cli/src/env_build.rs
  - crates/minos-cli/src/jobs.rs
  - crates/minos-cli/src/judge_push.rs
  - crates/minos-cli/src/lib.rs
  - crates/minos-cli/src/locate.rs
  - crates/minos-cli/src/lock_file.rs
  - crates/minos-cli/src/main.rs
  - crates/minos-cli/tests/env_build.rs
  - crates/minos-cli/tests/judge_push.rs
  - crates/minos-cli/tests/runner_vm.rs
  - crates/minos-core/Cargo.toml
  - crates/minos-core/src/diff/compose.rs
  - crates/minos-core/src/diff/cumulative.rs
  - crates/minos-core/src/diff/mod.rs
  - crates/minos-core/src/experiment.rs
  - crates/minos-core/src/job.rs
  - crates/minos-core/src/ledger.rs
  - crates/minos-core/src/lib.rs
  - crates/minos-core/src/limits.rs
  - crates/minos-core/src/lock.rs
  - crates/minos-core/src/manifest.rs
  - crates/minos-core/src/money.rs
  - crates/minos-core/src/policy.rs
  - crates/minos-core/src/state.rs
  - crates/minos-core/tests/breaker.rs
  - crates/minos-core/tests/compile_fail.rs
  - crates/minos-core/tests/cumulative.rs
  - crates/minos-core/tests/golden_wire.rs
  - crates/minos-core/tests/guest_wire.rs
  - crates/minos-core/tests/lock.rs
  - crates/minos-core/tests/manifest.rs
  - crates/minos-core/tests/money.rs
  - crates/minos-core/tests/prop_apply.rs
  - crates/minos-core/tests/prop_fold.rs
  - crates/minos-core/tests/prop_keys.rs
  - crates/minos-core/tests/prop_policy.rs
  - crates/minos-core/tests/report.rs
  - crates/minos-core/tests/route.rs
  - crates/minos-core/tests/snapshots/golden_wire__job_result_judge_usage.snap
  - crates/minos-core/tests/snapshots/golden_wire__record_v1_judged_logs.snap
  - crates/minos-core/tests/snapshots/golden_wire__record_v1_proposed_batch.snap
  - crates/minos-core/tests/snapshots/golden_wire__record_v1_run_started_root_image.snap
  - crates/minos-core/tests/snapshots/lock__minos_lock_v1_judge.snap
  - crates/minos-core/tests/ui/fail/cumulative_from_parent_tree.rs
  - crates/minos-core/tests/ui/fail/cumulative_from_parent_tree.stderr
  - crates/minos-core/tests/ui/fail/experiment_from_bare_diff.rs
  - crates/minos-core/tests/ui/fail/experiment_from_bare_diff.stderr
  - crates/minos-guest/src/bin/minos-probe.rs
  - crates/minos-guest/src/test_fixture.rs
  - crates/minos-loop/Cargo.toml
  - crates/minos-loop/clippy.toml
  - crates/minos-loop/src/admit.rs
  - crates/minos-loop/src/driver.rs
  - crates/minos-loop/src/error.rs
  - crates/minos-loop/src/history.rs
  - crates/minos-loop/src/inputs.rs
  - crates/minos-loop/src/job.rs
  - crates/minos-loop/src/lib.rs
  - crates/minos-loop/src/observe.rs
  - crates/minos-loop/src/record.rs
  - crates/minos-loop/src/stage.rs
  - crates/minos-loop/tests/blocks.rs
  - crates/minos-loop/tests/driver/breaker.rs
  - crates/minos-loop/tests/driver/caps.rs
  - crates/minos-loop/tests/driver/flow.rs
  - crates/minos-loop/tests/driver/lock.rs
  - crates/minos-loop/tests/driver/main.rs
  - crates/minos-loop/tests/driver/pins.rs
  - crates/minos-loop/tests/driver/resume.rs
  - crates/minos-loop/tests/driver/stops.rs
  - crates/minos-loop/tests/driver/support.rs
  - crates/minos-loop/tests/snapshots/blocks__batch_v1.snap
  - crates/minos-loop/tests/stage.rs
  - crates/minos-propose/src/proposal.rs
  - crates/minos-propose/src/proposer.rs
  - crates/minos-propose/tests/claude_cli.rs
  - crates/minos-propose/tests/live.rs
  - crates/minos-propose/tests/parse.rs
  - crates/minos-propose/tests/proposer.rs
  - crates/minos-propose/tests/single_shot.rs
  - crates/minos-propose/tests/sweep.rs
  - crates/minos-runner/Cargo.toml
  - crates/minos-runner/clippy.toml
  - crates/minos-runner/src/error.rs
  - crates/minos-runner/src/lib.rs
  - crates/minos-runner/src/map.rs
  - crates/minos-runner/src/pipeline.rs
  - crates/minos-runner/src/usage.rs
  - crates/minos-runner/tests/map.rs
  - crates/minos-runner/tests/pipeline.rs
  - crates/minos-runner/tests/usage.rs
  - crates/minos-sandbox/src/lib.rs
  - crates/minos-sandbox/src/untrusted.rs
  - crates/minos-sandbox/tests/compile_fail.rs
  - crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.rs
  - crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.stderr
  - crates/minos-sandbox/tests/untrusted.rs
  - crates/minos-source/src/judge.rs
  - crates/minos-source/src/lib.rs
  - crates/minos-source/src/walk.rs
  - crates/minos-source/tests/judge.rs
  - crates/minos-source/tests/pack.rs
  - crates/minos-source/tests/refusals.rs
  - crates/minos-store/src/file/ledger.rs
  - crates/minos-store/src/file/mod.rs
  - crates/minos-store/src/file/run_lock.rs
  - crates/minos-store/src/lib.rs
  - crates/minos-store/src/memory.rs
  - crates/minos-store/tests/contract.rs
  - crates/minos-store/tests/file_ledger.rs
findings:
  critical: 1
  warning: 3
  info: 6
  total: 10
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-10-24
**Depth:** standard
**Files Reviewed:** 112 (50 source files read in full against `git diff fdd5999..HEAD`; tests, snapshots, and `.stderr` files read as context for them)
**Status:** issues_found

## Summary

Phase 7 adds the runner pipeline (`minos-runner`), the generation driver (`minos-loop`), the `CumulativeDiff` type, the `Untrusted` sink exits, the store's `Conflict` error and whole-run lock, judge-data packing with the snapshot overlap refusal, `minos judge push`, and the ledger's optional provenance fields.

What holds up under adversarial reading:

- **Hostile bytes stay contained.** The only exits from `Untrusted` in production are `take_status`, `into_scorecard`, and the two sink exits, and both sink exits verify the sink's returned digest and return only a `BlobId`. Every `DriverError`, `RunnerError`, `JobFailure`, and `LoopEvent` carries IDs, numbers, closed enums, or `&'static str`; the transparent inner errors (`ProposeError`, `ProviderError`, `PolicyError`, `FoldError`, `JobError`, `RouteError`) were checked variant by variant and none interpolates model or sandbox text. Rationales, notes, and log tails reach the prompt only through `SafeText` and `LogExcerpt`.
- **Spend is not double-recorded.** `spend_once` keys on `SpendId`; a model call with per-model usage records only the per-model rows; a torn append that actually landed replays as `Duplicate`.
- **Resume is a fold.** `next_step` reads only `RunState`; the batch blob makes a half-recorded propose step complete deterministically; `load_batch` cross-checks every recorded `Proposed` against it.
- **Lock and append races.** `driver.lock` is a separate `flock` from `ledger.lock`, `O_EXCL`-created with the store's symlink and inode checks; `Conflict` is now an error so `?` cannot skip it.
- The C-02 grep (`#!?\[allow\(`) prints nothing; no `unwrap`/`expect`/debug artifacts in the changed sources.

The defects found are in the blame map's edges and the stop semantics, not in the containment layer. One is rated critical because a candidate can halt the whole run through it with nothing recorded.

## Critical Issues

### CR-01: A candidate's output size can stop the run as a "configuration error" through the judge VM's `ScratchFull`

**File:** `crates/minos-runner/src/map.rs:140-155`, `crates/minos-runner/src/pipeline.rs:209-222`, `crates/minos-runner/src/pipeline.rs:111-133`
**Issue:** `setup_verdict` maps `SetupStatus::Failed { step: ScratchFull }` to `Verdict::Config(ConfigCause::ScratchFull)` for **both** roles, and `judge_verdict` applies it to the judge VM. The judge VM's setup (`crates/minos-guest/src/run.rs:377-390`) unpacks the judge data **and then the candidate's `candidate_out`** into the same scratch tmpfs, and an `ENOSPC` on either unpack is reported as `ScratchFull`. `candidate_out` is candidate-controlled up to `MAX_CANDIDATE_OUT_BYTES` (16 MiB), and nothing in `Manifest::try_from`, `Runner::new`, or `Runner::check` relates `[environment] scratch_mib` to the pushed judge data's size plus that cap. So on a project whose scratch is within 16 MiB of its judge data, a candidate that writes a large `out/` (a sloppy agent dumping a dataset, or an adversarial one on purpose) ends the run with `DriverError::Job { "the build or run filled [environment] scratch_mib; raise it" }`, **nothing recorded for the trial**, and the candidate never blamed. That is the blame mis-attribution the pipeline docs promise cannot happen ("a candidate fault is an `Outcome`, never an error"), and it hands an untrusted candidate a run-halting lever. The existing tests (`scratch_full_is_a_configuration_error`, `full_scratch_is_a_configuration_error_never_retried`) pin the current behavior and so do not catch it.
**Fix:** Make the judge VM's scratch provably large enough for any candidate output before generation 0, so a judge-side `ScratchFull` really is configuration. The CLI already has the judge data's content size (`LockedJudge::bytes`); thread it into the runner and refuse at construction and in `check`:

```rust
// pipeline.rs — Runner::new (or Runner::check), judge_bytes passed from the lock entry
let scratch = u64::from(environment.scratch_mib()) * 1024 * 1024;
let needed = judge_bytes
    .saturating_add(MAX_CANDIDATE_OUT_BYTES)
    .saturating_add(JUDGE_SCRATCH_HEADROOM);
if scratch < needed {
    return Err(ConfigCause::JudgeScratch { scratch, needed }.into());
}
```

Alternatively (less precise), split the guest's report so the `candidate_out` unpack failing on `ENOSPC` is `OutputStatus::TooLarge` for the judge role, which `judge_status` would then map to `Rejected(OutputTooLarge)`. Either way, add a pipeline test in which the judge VM reports `ScratchFull` after a candidate with a maximal `out/` and assert the run does not stop as configuration. Record the chosen rule in CLAUDE.md's "minos-runner and minos-loop" section.

## Warnings

### WR-01: A second `minos stop` at a generation already stopped in an earlier session is invisible to the resumed driver

**File:** `crates/minos-loop/src/driver.rs:282-283`, `crates/minos-loop/src/driver.rs:624-630`, `crates/minos-core/src/ledger.rs:23`
**Issue:** D-19 makes stops session-relative: `session_stops` is the set of `(generation, reason)` pairs in the ledger when the session starts, and `safe_point` honours only a pair outside it. But `StopRequested`'s `EventKey` is `u64(generation), u64(reason tag)`, so a repeated `StopRequested { generation: g, reason: UserStop }` is a store `Duplicate` that writes nothing. Sequence: the user runs `minos stop` during generation 3 (session A honours it); the user resumes (session B snapshots `(3, UserStop)`); the user runs `minos stop` again while session B is still in generation 3. The append is a no-op, the fold is unchanged, and session B runs every remaining trial of generation 3 and only sees a new stop once the generation advances. With `width` candidates and multi-minute stage budgets that is a long window in which the user's stop silently does nothing, and the ledger doc ("a resumed run can stop again later") leaves the gap unstated. Phase 8 will build `minos stop` on this seam.
**Fix:** Give user stops a key that is distinct per request, the way `Steered` is keyed by `(generation, ordinal)`, so each `minos stop` is a new line; or have the driver snapshot the ledger's `seq` at session start and honour any `StopRequested` line with a greater `seq` (which requires the repeated stop to actually land, so the key change is needed either way). If the frozen event set must not grow, make `minos stop` refuse with a clear message when the pair already exists and the run is currently locked, so the user knows the stop was not registered. Add a driver test: stop, resume at the same generation, stop again, assert the session ends at the next safe point.

### WR-02: The post-call model-call safe point discards a paid call's proposals, so a cap or user stop during the call costs a second call on resume

**File:** `crates/minos-loop/src/driver.rs:864-899`
**Issue:** After a successful model call the driver records its `Spent`, then runs `safe_point(SafePoint::ModelCall)` (step 6) **before** `admit` (step 7). If that safe point halts (a spend or token cap crossed by this very call, or a `minos stop` appended while the model was thinking), the validated proposals are dropped with no batch blob and no `Proposed`. On resume the step is `Propose` again under the next call index: another model call is paid for the same generation, and the run's proposals differ from what an uninterrupted session would have admitted. Admitting the batch first and letting the launch safe point stop the session would record the proposals (the ledger already permits `Proposed` without trials) and cost nothing extra: `spend_ok` would still see the same recorded spend at the launch's after-trial check, and the module's "overshoot by at most one model call" bound is unchanged. `token_cap_below_one_call_records_its_spend_then_stops_with_token_cap_and_calls_once` pins the current ordering.
**Fix:** Move the second `safe_point(SafePoint::ModelCall, generation)` to after `admit` and the `Proposed` appends (or make the `Launch` safe point check spend too, which the table at `driver.rs:41-46` currently excludes), so a stop that arrives during or right after a call never throws away what was paid for. Update the D-14 table and the cap test accordingly.

### WR-03: `recorded_manifest` turns a missing, tampered, or unparseable manifest blob into "the manifest changed; start a new run"

**File:** `crates/minos-loop/src/driver.rs:1082-1109`, `crates/minos-loop/src/inputs.rs:155-167`
**Issue:** `recorded_manifest` collapses `BlobMissing`, `BlobTooLarge`, `BlobTampered`, a non-UTF-8 blob, and a `Manifest::parse_toml` failure into `Ok(None)`. `check_resume` then reports any manifest-ID difference as `DriverError::ManifestChanged { recorded, current }` whose message tells the user "start a new run". A tampered or corrupted blob in `.minos/blobs` is thereby reported as a user edit, and the user is steered away from the store problem the project otherwise treats as needing a human (`DriverError::Store` docs). The `.ok()` on `parse_toml` also hides that a manifest the CLI once accepted no longer parses, which would be a wire-compatibility regression worth surfacing.
**Fix:** Return the store errors as `DriverError::Store` (as the surrounding code does for every other `StoreError`) and give the unreadable-blob case its own variant or message:

```rust
Err(StoreError::BlobMissing { .. } | StoreError::BlobTooLarge { .. } | StoreError::BlobTampered { .. }) => {
    return Err(DriverError::ManifestRecordUnreadable { run: *state.run(), manifest: *state.manifest() });
}
```

and map a `parse_toml` failure to the same variant. Keep `Ok(None)` only if there is a documented reason a run should resume with an unverifiable frame (there is not one in D-19).

## Info

### IN-01: `MAX_MANIFEST_BYTES` is restated in the loop and can drift from the CLI's

**File:** `crates/minos-loop/src/driver.rs:130-132`, `crates/minos-cli/src/env_build.rs` (`MAX_MANIFEST_BYTES`)
**Issue:** The loop cannot link the CLI, so it hard-codes `1024 * 1024` with a comment that it must equal the CLI's constant. Nothing enforces that; a CLI change would make a manifest the CLI accepted unreadable on resume (and, with WR-03, be reported as "the manifest changed").
**Fix:** Move the constant to `minos_core::limits` (both crates depend on core) and import it in both places.

### IN-02: The driver's infra-streak stop and `circuit_breaker`'s infra arm encode the same threshold twice

**File:** `crates/minos-loop/src/driver.rs:675-698`, `crates/minos-core/src/policy.rs:541-561`
**Issue:** `retry` stops the session when `streak >= max_infra_failures`, so by the time the generation safe point calls `circuit_breaker(state, budget, self.streak)` the streak is always below the limit and the breaker's `InfraFailures` arm is unreachable from the driver. Two places own one rule; if either changes (for example `>` versus `>=`), the ledger's `StopRequested { InfraFailures }` generation could differ between them.
**Fix:** Let one function own it: either drop the `infra_streak` parameter from `circuit_breaker` and document that the driver records the infra stop itself, or have `retry` consult `circuit_breaker` instead of comparing on its own.

### IN-03: `candidate_status` cannot check the apply status against the request

**File:** `crates/minos-runner/src/map.rs:199-209`
**Issue:** `ApplyStatus::Baseline | ApplyStatus::Applied` both pass, so a status that says `Baseline` for a candidate request that carried a diff (or `Applied` for the baseline) is accepted, because the pure map does not know whether the request had a diff. PID 1 is trusted to be consistent, so this is a defense-in-depth gap, not a current fault.
**Fix:** Pass `has_diff: bool` into `candidate_verdict`/`candidate_status` and reject the mismatching combination as `Rejected(OutputInvalid)`, with a map test for each direction.

### IN-04: `store_logs` silently skips a stage whose path constant stopped parsing

**File:** `crates/minos-runner/src/pipeline.rs:348-353`
**Issue:** `let Ok(path) = RelPath::from_str(path) else { continue; }` fails open: if `BUILD_LOG_PATH` or its siblings ever stop being valid `RelPath`s, the log is quietly not stored and only a pipeline test notices. The comment relies on that test.
**Fix:** Make the constants typed at compile time (a `const fn` constructor on `RelPath` for literals, or a `LazyLock<RelPath>`), or return `InfraCause::Unmapped` instead of `continue` so the gap is loud at run time too.

### IN-05: `excerpt_stage` attributes judge-launch output faults to the candidate's run log

**File:** `crates/minos-loop/src/history.rs:101-120`
**Issue:** `Reason::OutputInvalid`, `OutputTooLarge`, and `ChunkProtocol` map to `Stage::Run`, but the same reasons are also produced for the **judge** VM by `judge_verdict` (`Seen::Output`, `Seen::Status`, `output_verdict`). For those trials the prompt's excerpt shows the candidate's run log, which has nothing to do with why the trial was rejected, while the judge log is the one that would explain it.
**Fix:** Pick the excerpt from the logs actually recorded for the trial: prefer `Stage::Judge` when the judgment's `logs` holds it and no `Build`/`Run` failure explains the outcome, or carry the failing stage in the `Judged` record so the mapping needs no guess.

### IN-06: `relative_to` treats the project directory itself as "inside the project" only via a failed `RelPath` parse

**File:** `crates/minos-cli/src/judge_push.rs:346-383`
**Issue:** `judge push .` from the project root is refused only because `RelPath::try_from("")` fails and the `is_some_and` falls through to `DirInSnapshot`. The refusal is correct, but it rests on an incidental property of `RelPath` rather than an explicit check, and the error message ("no [source] exclude entry covers it") does not say what is wrong.
**Fix:** Check `rel.as_os_str().is_empty()` explicitly and return a dedicated message ("the judge data directory is the project itself").

---

_Reviewed: 2026-10-24_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
