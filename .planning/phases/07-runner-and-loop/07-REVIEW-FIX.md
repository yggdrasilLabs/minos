---
phase: 07-runner-and-loop
fixed_at: 2026-10-24T15:16:22Z
review_path: .planning/phases/07-runner-and-loop/07-REVIEW.md
iteration: 1
findings_in_scope: 10
fixed: 9
skipped: 1
status: partial
---

# Phase 7: Code Review Fix Report

**Fixed at:** 2026-10-24T15:16:22Z
**Source review:** .planning/phases/07-runner-and-loop/07-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 10 (fix_scope `all`: CR-01, WR-01, WR-02, WR-03, IN-01 to IN-06)
- Fixed: 9
- Skipped: 1 (WR-01: a design decision; the gap is pinned by a test and recorded in CLAUDE.md, commit `9c44503`)

**Gate, run on the worktree after the last fix commit (every command's real result):**
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 88 suites `ok`, 0 failures (the ignored VM and live tests stay ignored).
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo doc --workspace --no-deps`: no warnings.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: clean.
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing.
- No wire format changed: every golden snapshot and SHA-256 pin passes unchanged; no ledger event, key, or field was touched.
- `cargo clean` was not run, as instructed.

Each fix is one commit on `main` (fast-forwarded from the fixer's worktree branch), in the order below. REVIEW-FIX.md itself is not committed.

## Fixed Issues

### CR-01: A candidate's output size can stop the run as a "configuration error" through the judge VM's `ScratchFull`

**Files modified:** `crates/minos-core/src/limits.rs`, `crates/minos-core/src/guest.rs`, `crates/minos-runner/src/pipeline.rs`, `crates/minos-runner/src/error.rs`, `crates/minos-runner/src/map.rs`, `crates/minos-runner/src/lib.rs`, `crates/minos-runner/tests/pipeline.rs`, `crates/minos-runner/tests/usage.rs`, `crates/minos-cli/src/jobs.rs`, `crates/minos-cli/tests/runner_vm.rs`, `CLAUDE.md`
**Commit:** `0a774f2`
**Status:** fixed: requires human verification (the sizing rule is a design choice; see below)
**Applied fix:** The review's first option: make the judge VM's scratch provably large enough for any candidate output before generation 0. `minos_core::limits` gains `TMPFS_PAGE_BYTES` (4096), `JUDGE_SCRATCH_SLACK_BYTES` (16 MiB), and `judge_scratch_needed_bytes(judge_bytes, judge_files)` = judge content + one tmpfs page per judge file + `MAX_OUTPUT_CONTENT_BYTES` (16 MiB, what the `CANDIDATE_OUT` section holds unpacked) + one page per `MAX_OUTPUT_ENTRIES` + slack, saturating. `minos-runner` gains `JudgeExtent { bytes, files }` (the lock's `LockedJudge::bytes` and `file_count`), a seventh parameter of `Runner::new`, which refuses `[environment] scratch_mib` below the need with the new `ConfigCause::JudgeScratch { scratch, needed }` before anything launches; `judge_inputs` in `minos-cli` returns the extent as a third tuple element. Const assertions pin that the smallest scratch (64 MiB) holds an empty judge set plus a full candidate output and that the default scratch (1024 MiB) holds the largest judge data (96 MiB in 10,000 files) plus a full output. Two pipeline tests cover a refused construction (64 MiB scratch, 40 MiB judge data) and an accepted one at both caps; the `ScratchFull` text now says "the guest's inputs filled" rather than "the build or run filled", because it is a setup failure. The map table, the error docs, `MIN_SCRATCH_MIB`'s doc, and CLAUDE.md's runner paragraph record the rule.
**For the human:** confirm the sizing constants (one 4 KiB page of tmpfs rounding per unpacked file, 16 MiB of slack for directories, `HOME`, `TMP`, and the scorecard). The runner cannot see per-file sizes, so the per-file term uses the lock's file count; a judge that writes more than the slack fails in its own stage as `Crashed { Judge }`, which is unchanged.

### WR-02: The post-call model-call safe point discards a paid call's proposals, so a cap or user stop during the call costs a second call on resume

**Files modified:** `crates/minos-loop/src/driver.rs`, `crates/minos-loop/tests/driver/caps.rs`, `CLAUDE.md`
**Commit:** `3a7bb30`
**Status:** fixed: requires human verification (an ordering change to D-14)
**Applied fix:** The second `safe_point(SafePoint::ModelCall)` moved from before `admit` to after the batch blob and the `Proposed` records land (and after `Selected` when nothing survived). A cap the call crossed, a `minos stop` appended during the call, or an unpriced resolved model now ends the session with the paid call's proposals recorded, and the resumed session runs those trials under the generation safe point instead of calling again. The D-14 table in the driver docs, `SafePoint::ModelCall`'s doc, the `run_to` bullet, the step comments, and CLAUDE.md's safe-point sentence say so. `token_cap_below_one_call...` became `token_cap_below_one_call_records_its_spend_and_proposals_then_stops_and_resumes_without_a_second_call`: it asserts `spent, proposed, stop_requested` as the last records, then resumes under a raised `max_tokens` and asserts the trial runs with exactly one model call ever made. `unpriced_resolved_model...` now asserts `spent, proposed` as the last records and no trial of generation 1. The one-call overshoot bound is unchanged, because admission records nothing a cap counts.
**For the human:** D-13's "right after the first call that names it" still holds for `UnpricedModel`, but the error now arrives after that call's proposals are recorded; confirm that reading of D-13 and D-14.

### WR-03: `recorded_manifest` turns a missing, tampered, or unparseable manifest blob into "the manifest changed; start a new run"

**Files modified:** `crates/minos-loop/src/driver.rs`, `crates/minos-loop/src/error.rs`, `crates/minos-loop/src/inputs.rs`, `crates/minos-loop/tests/blocks.rs`, `crates/minos-loop/tests/driver/flow.rs`, `CLAUDE.md`
**Commit:** `0bb5451`
**Applied fix:** `recorded_manifest` returns `Result<Manifest, DriverError>`; `BlobMissing`, `BlobTooLarge`, `BlobTampered`, non-UTF-8 bytes, and a `parse_toml` failure are the new `DriverError::ManifestRecordUnreadable { run, manifest }` ("...is missing, tampered, or no longer parses; the store needs a human"), and any other store error stays `DriverError::Store`. `check_resume` takes `recorded: &Manifest` (no `Option`), so the "could not be verified" arm is gone; `start` passes the current manifest. The `blocks.rs` call sites were updated, the `None` case test was removed as unrepresentable, and a driver test on a `FileStore` damages the manifest blob three ways (deleted, tampered, bytes that are not a manifest) and asserts the refusal names the run and the recorded manifest and records nothing. The inputs table, the `ManifestChanged` doc, and CLAUDE.md's resume sentence record it.

### IN-01: `MAX_MANIFEST_BYTES` is restated in the loop and can drift from the CLI's

**Files modified:** `crates/minos-core/src/limits.rs`, `crates/minos-cli/src/env_build.rs`, `crates/minos-cli/src/judge_push.rs`, `crates/minos-loop/src/driver.rs`
**Commit:** `1c3cd02`
**Applied fix:** `pub const MAX_MANIFEST_BYTES: u64 = 1_048_576` lives in `minos_core::limits` (manifest section); `env_build.rs` and `judge_push.rs` import it from core, and the loop's private restatement (and its now-unused `byte_cap` import) is gone; `recorded_manifest` reads under the shared constant.

### IN-02: The driver's infra-streak stop and `circuit_breaker`'s infra arm encode the same threshold twice

**Files modified:** `crates/minos-core/src/policy.rs`, `crates/minos-core/tests/breaker.rs`, `crates/minos-loop/src/driver.rs`
**Commit:** `18326ca`
**Applied fix:** The review's two options both changed a public signature or needed a ledger fold inside `retry`; the applied variant keeps `circuit_breaker(state, budget, infra_streak)` as is and adds `pub const fn infra_breaker(budget, infra_streak) -> Option<StopReason>` as the one owner of the threshold: `circuit_breaker` calls it for its infrastructure arm and the driver's `retry` consults it instead of comparing on its own. A core test asserts that on a run with no empty streak the breaker's verdict equals the arm's at every streak (0, 1, 2, 3, 4, `u32::MAX`).

### IN-03: `candidate_status` cannot check the apply status against the request

**Files modified:** `crates/minos-runner/src/map.rs`, `crates/minos-runner/src/pipeline.rs`, `crates/minos-runner/tests/map.rs`
**Commit:** `9661dfa`
**Applied fix:** `candidate_verdict(seen, has_diff)` and `candidate_status(status, has_diff)`: `(Applied, true)` and `(Baseline, false)` pass, `(Failed, true)` is `ApplyFailed`, and every other combination (`Baseline` for a diff, `Applied` or a failed apply for the baseline, `NotApplicable`) is `Rejected(OutputInvalid)`. The pipeline passes `request.diff().is_some()`. The map tests pass `true` for candidate fixtures and `false` for the baseline, a new test covers each mismatching direction, and the property test draws `has_diff` and checks the pass predicate against it. The map table gained the row.

### IN-04: `store_logs` silently skips a stage whose path constant stopped parsing

**Files modified:** `crates/minos-runner/src/pipeline.rs`, `crates/minos-runner/src/error.rs`
**Commit:** `f13d3da`
**Applied fix:** The `let Ok(path) = ... else { continue }` became `RelPath::from_str(path).map_err(|_| ConfigCause::LogPath)?`, with the new `ConfigCause::LogPath` ("a stage log path is not a valid relative path; a runner defect"). A `const fn` `RelPath` constructor was not added: `RelPath` validates, and the forbid lints rule out a `LazyLock` with `expect`; the loud configuration error stops the run instead of quietly storing no log.

### IN-05: `excerpt_stage` attributes judge-launch output faults to the candidate's run log

**Files modified:** `crates/minos-loop/src/history.rs`, `crates/minos-loop/tests/blocks.rs`
**Commit:** `c2377dc`
**Applied fix:** New `pub fn excerpt_log(judgment) -> Option<&BlobId>` picks the log from what the trial recorded: for `OutputInvalid`, `OutputTooLarge`, and `ChunkProtocol`, a recorded `Stage::Judge` log wins, because the runner stores a judge log only for a launch the candidate passed into, so its presence names the judge VM as the faulting one; otherwise the `excerpt_stage` mapping stands. `excerpt` uses it. `excerpt_stage` is unchanged (pure, from the outcome alone), and the ledger was not touched (the review's "carry the failing stage in `Judged`" alternative is a wire change). A blocks test covers each of the three reasons with and without a judge log, plus a gate failure and a build failure beside a judge log. Remaining limit, documented in the function: a judge-side fault whose stream was unreadable leaves no judge log and is indistinguishable from a candidate-side fault after passing stages, so the run log is shown as before.

### IN-06: `relative_to` treats the project directory itself as "inside the project" only via a failed `RelPath` parse

**Files modified:** `crates/minos-cli/src/judge_push.rs`, `crates/minos-cli/tests/judge_push.rs`
**Commit:** `f5294ce`
**Applied fix:** `check_placement` checks `rel.as_os_str().is_empty()` explicitly and returns the new `JudgePushError::DirIsProject { dir }` ("the judge data directory {dir} is the project itself; push a directory holding only the judge data, outside the project or under a [source] exclude entry"); the `dir_in_snapshot_refused` test asserts the new variant for `judge push <project>`.

## Skipped Issues

### WR-01: A second `minos stop` at a generation already stopped in an earlier session is invisible to the resumed driver

**File:** `crates/minos-loop/src/driver.rs:282-283`, `crates/minos-loop/src/driver.rs:624-630`, `crates/minos-core/src/ledger.rs:23`
**Reason:** skipped: design decision needed (every behavioural fix is either a wire-format change or Phase 8's `minos stop`). The behaviour is unchanged. What was done instead, in commit `9c44503`: a driver test, `stops::a_repeated_user_stop_at_a_generation_an_earlier_session_stopped_in_is_a_duplicate_the_resume_never_sees`, pins the gap (session A honours the first `(1, UserStop)`; session B resumes, the second `minos stop` at generation 1 is a store `Duplicate` that leaves no line, and B reaches generation 2), so Phase 8 must flip it consciously; and CLAUDE.md's existing open question ("How `minos stop` records a stop the driver has not seen...", owner Phase 8) now states the user-visible effect and the options.
**Options for the user:**
1. Key `StopRequested` per request (an `ordinal` like `Steered`'s, or the `seq`), so each `minos stop` is a new line. `StopRequested`'s `EventKey` derivation is frozen and golden-pinned, so this is a `"v":"2"` decision, not a fix the fixer may apply.
2. Keep the key and have Phase 8's `minos stop` refuse with a clear message when the `(generation, UserStop)` pair already exists and the run is locked, so the user learns the stop was not registered. No wire change; `minos stop` does not exist yet, so there is nothing to change in Phase 7.
3. Snapshotting the ledger `seq` at session start was considered and does not help on its own: the repeated stop never lands, so there is no new `seq` to see; it only works together with option 1.
**Original issue:** D-19 makes stops session-relative, but `StopRequested`'s key is `(generation, reason)`, so a repeated `StopRequested { g, UserStop }` is a store `Duplicate` that writes nothing; a session that resumed at generation `g` after an earlier session stopped there runs every remaining trial of `g` and only sees a new stop once the generation advances.

---

_Fixed: 2026-10-24T15:16:22Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
