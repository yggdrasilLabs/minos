---
phase: 07-runner-and-loop
plan: 15
subsystem: minos-loop (the generation driver)
status: complete
tags: [loop, driver, safe-points, spend-cap, circuit-breaker, retries, backend-pin, LOOP-07, LOOP-08, JUDGE-04, D-11, D-13, D-14, D-15, D-16, D-19]
requires:
  - phase: 07-13
    provides: "Driver (start, resume, calibrate, run_to), DriverOptions::retry_pause, Calibrated::Stopped, RunEnd, flow tests and the driver test target"
  - phase: 07-01
    provides: "RunState::{stops, backend_classes}, circuit_breaker, spend_ok"
provides:
  - "minos_loop::driver: private SafePoint, safe_point, retry, retry_append, run_job, pin_backend, and the Halt/Flow internal control type; Driver session fields session_stops and streak"
  - "crates/minos-loop/tests/driver/{caps,breaker,stops,pins}.rs (18 tests); support.rs gains PRICES_DEAR, with_budget, backend_on, scored_on, reply_from, options, sweep_proposer, single_shot_proposer, calls, kinds, stops"
affects: [07-16, 07-17]
tech-stack:
  added: []
  patterns:
    - "Internal steps return Flow<T> = Result<T, Halt>, so `?` carries a recorded or honoured stop as readily as an error up to run_to or calibrate"
    - "Every check calls the core function that owns the decision (circuit_breaker, spend_ok, backend_classes); the driver only chooses the safe point"
    - "An append is retried on its own inside retry_append while the caller holds the job or call result"
key-files:
  created:
    - crates/minos-loop/tests/driver/caps.rs
    - crates/minos-loop/tests/driver/breaker.rs
    - crates/minos-loop/tests/driver/stops.rs
    - crates/minos-loop/tests/driver/pins.rs
  modified:
    - crates/minos-loop/src/driver.rs
    - crates/minos-loop/tests/driver/main.rs
    - crates/minos-loop/tests/driver/support.rs
    - crates/minos-loop/tests/driver/flow.rs
decisions:
  - "An append that lands on its first try does not reset the infra streak; only one that lands after failing does. Otherwise a provider infra error that reports usage would reset the streak when its spend is recorded and never reach max_infra_failures"
  - "LoopEvent::Retrying is emitted only when a retry follows: the failure that reaches max_infra_failures emits Stopped instead, matching the event's doc ('the step will be retried after the pause')"
  - "run_to returns Reached before the generation safe point when next_step is Done, so a run that reached its target is not stopped after the fact"
  - "Generation 0's generation safe point runs once per calibrate call, before its first pending baseline launch, so a resumed run whose cap is still exceeded launches nothing more"
  - "An honoured stop (one added during the session) ends the session as RunEnd::Stopped under the stop's own generation and records nothing"
  - "safe_point and retry return Flow<()> (a private Halt carrying the stop) rather than the plan's Result<Option<RunEnd>, DriverError>; the plan's sketch shows intent"
metrics:
  duration: "about 15 minutes (2026-10-24T13:46Z to 14:01Z)"
  completed: 2026-10-24
  tasks: 2
  files: 8
---

# Phase 7 Plan 15: Safe points, caps, the breaker, retries, and the backend pin Summary

The driver now ends a session when it must and keeps going when it should. It checks new stops, `circuit_breaker`, and `spend_ok` at the generation, model-call, launch, and after-trial safe points. Stops are session-relative. Infrastructure failures build a streak and are retried after the pause. A resolved model that `prices.toml` does not price stops the run after its first paid call, and a trial from another backend class is refused before anything is recorded. All of it is proven on fakes.

## What was built

**Task 1 (dc0529e), `driver.rs`:**
- **Session state (D-19, D-16).** `session_stops: BTreeSet<(u32, StopReason)>` is set from `state.stops()` at the end of `start` and `resume`. `streak: u32` counts consecutive infrastructure failures. The driver keeps no other new state.
- **`SafePoint { Generation, ModelCall, Launch, AfterTrial }` and `safe_point`.** It refolds first. A stop outside `session_stops` ends the session and records nothing. At `Generation` only, a `circuit_breaker` verdict appends `StopRequested` and ends the session. At every point except `Launch`, `spend_ok` runs: `Stop(reason)` appends and ends, `CostError::UnpricedModel` becomes `DriverError::UnpricedModel`, and any other cost error becomes `DriverError::Cost`. Each session end emits exactly one `LoopEvent::Stopped`.
- **Where the safe points run:**
  - Generation: before every `run_to` step other than `Done`, at last concluded + 1, and once before generation 0's first pending baseline launch in a `calibrate` call.
  - ModelCall: before each propose call, and again after any call that recorded spend (success or error with usage), before admission.
  - Launch: before every `JobRunner::run`.
  - AfterTrial: after each trial's `Spent` and `Judged` land.
- **Retries (D-16).** `retry` increments the streak. At `max_infra_failures` it appends `StopRequested { InfraFailures }` and ends the session. If that append itself fails on a retryable error, the result is `DriverError::Infra` naming the original failure. Below the limit it emits `Retrying { streak, max }`, sleeps `options.retry_pause`, and the caller re-derives the same step. It is used for `JobFailure::Infra`, provider errors with `is_infra()`, and retryable store errors on appends. `retry_append` retries only the append (`record`, and `spend` over `spend_once`) while the caller keeps its result. A successful job or propose call resets the streak, and so does an append that lands after failing. `RETRY_PAUSE` now documents its reason.
- **Backend pin (D-11, JUDGE-04).** `pin_backend` runs right after `jobs.run` succeeds, before any append. It refuses a result whose `(backend, arch)` differs from any recorded class with `BackendChanged { recorded, found }`. An empty set accepts the result.
- **Module docs** now cover the safe-point table, session-relative stops, metered-only caps with their one-step overshoot (D-15), the retry streak and pause, the backend pin, and the rule that the driver never kills a running VM.

**Task 2 (1e28be4), driver test target (32 tests: flow's 14 plus 18 new):**

| Behavior | Test |
|---|---|
| $1 cap, $1.50 trial: one job, its records, `StopRequested { 0, SpendCap }`; observer sees `Calibrated`, `Stopped` | `caps::spend_cap_below_one_job_records_the_first_baseline_trial_then_stops_with_spend_cap` |
| `calibrate` at the cap is `Calibrated::Stopped { SpendCap }` | `caps::calibrate_at_the_spend_cap_returns_calibrated_stopped` |
| $5 cap: baseline $4.50, the first candidate trial crosses; four jobs, then the stop at generation 1 | `caps::spend_cap_crossed_by_a_candidate_trial_overshoots_by_exactly_that_one_job` |
| `max_tokens = 1000`, a 1,200-token call: `Spent`, then `StopRequested { 1, TokenCap }`, no `Proposed`, one call | `caps::token_cap_below_one_call_records_its_spend_then_stops_with_token_cap_and_calls_once` |
| Reply resolved to `claude-haiku-4-5`: `UnpricedModel` naming it after exactly one call's `Spent` | `caps::unpriced_resolved_model_stops_with_unpriced_model_after_one_call_spend` |
| `max_empty_generations = 2`: generations 1 and 2 empty, `StopRequested { 3, NoValidCandidates }`, nothing proposed in 3 | `breaker::two_empty_generations_trip_the_breaker_with_no_valid_candidates` |
| Three job infra failures: three requests for trial 0, ledger `run_started, stop_requested`, `Retrying` 1 and 2, one `Stopped` | `breaker::three_job_infra_failures_record_nothing_and_stop_with_infra_failures` |
| Fail, fail, succeed, fail: streak 1, 2, 1, no stop, calibration completes | `breaker::a_success_resets_the_infra_streak_so_a_later_single_failure_does_not_stop` |
| Three provider infra errors: three calls, no spend, `InfraFailures` | `breaker::provider_infra_errors_count_toward_the_streak_and_stop_with_infra_failures` |
| Three provider infra errors with usage: spends at calls 0, 1, 2, still `InfraFailures` after three calls | `breaker::provider_infra_errors_with_usage_record_each_spend_and_still_trip_the_breaker` |
| `Io` on append 3: one job per trial, nine records, one `Retrying` | `breaker::a_retryable_append_failure_retries_the_append_without_rerunning_the_job` |
| An old `UserStop` does not block a resume to its target | `stops::an_old_user_stop_does_not_block_a_resume_from_reaching_its_target` |
| `minos stop` during trial 1: trial 1 recorded after the stop, two jobs, `Stopped { 0, UserStop }` once and last | `stops::a_user_stop_during_a_launch_ends_the_session_after_that_trial_is_recorded` |
| A `SpendCap` run resumes under a $100 cap, records `ManifestChanged { 1 }`, and reaches its target | `stops::a_spend_cap_stop_resumes_and_proceeds_once_the_cap_is_raised` |
| An unchanged cap re-trips before any launch, and the ledger is byte-for-byte unchanged | `stops::a_spend_cap_stop_re_trips_on_resume_when_the_cap_is_unchanged` |
| Baseline trial 1 on x86_64: `BackendChanged` with both classes, only trial 0 recorded | `pins::a_baseline_trial_on_another_backend_class_is_refused_and_records_nothing` |
| Candidates on x86_64: `BackendChanged`, proposals recorded, no candidate spend or judgment | `pins::a_candidate_trial_on_another_backend_class_is_refused_and_records_nothing` |
| All krun on arm64: every trial judged, one backend class | `pins::a_run_whose_every_trial_is_krun_on_arm64_records_them_all` |

Five temporary mutations of `driver.rs` were each reverted with `git checkout -- crates/minos-loop/src/driver.rs` before commit, and each one failed tests:
- Ignoring `session_stops` failed 2 stops tests.
- Removing the pin failed both pin-refusal tests.
- Resetting the streak on every append failed the provider-infra-with-usage test.
- Dropping the post-call check failed the token-cap and unpriced-model tests.
- Dropping the after-trial safe point failed 4 cap and stop tests.

## Verification

Each command below exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 1667 passed, 0 failed, 26 ignored, across 90 result lines; `--test driver` passed 32. No flaky failures this run.
- `cargo deny check`: advisories, bans, licenses, and sources all ok
- `cargo doc --workspace --no-deps`
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed `ok` for all 11 crates. The C-02 grep printed nothing, and no `#[expect(` appears under `crates/`.

Acceptance greps:
- `circuit_breaker(|spend_ok(|backend_classes()|session_stops|retry_pause` matches 13 lines in `driver.rs`.
- `StopReason::InfraFailures` appears once and `enum SafePoint` once.
- All four `mod` lines are in `main.rs`.
- Match counts: `SpendCap|TokenCap` 8 in caps.rs, `NoValidCandidates|InfraFailures` 12 in breaker.rs, `UserStop` 6 in stops.rs, `BackendChanged` 3 in pins.rs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Resetting the streak on every successful append would disable the breaker for provider failures that report usage**
- **Found during:** Task 1
- **Issue:** The plan resets the streak after every successful append. A provider infra error with usage records its spend, which is a successful append, before the retry is counted. So the streak would go 0 to 1 on every attempt and never reach `max_infra_failures`, retrying without bound.
- **Fix:** `retry_append` resets the streak only when an append lands after failing. A successful job or propose call still resets it. The `provider_infra_errors_with_usage...` test pins this down, and a mutation run shows it fails under the plan's rule.
- **Files modified:** crates/minos-loop/src/driver.rs
- **Commit:** dc0529e

**2. [Rule 2 - Correctness] Generation 0 gets a generation safe point**
- **Found during:** Task 1
- **Issue:** As written, the plan puts a generation safe point only before `run_to` steps, and calibration runs first. A resumed run still over its cap would launch one more baseline job on every resume before the after-trial check caught it, which breaks the one-job overshoot bound (D-15).
- **Fix:** `calibrate` passes `SafePoint::Generation` at generation 0 before its first pending launch. `stops::a_spend_cap_stop_re_trips_on_resume_when_the_cap_is_unchanged` asserts that no job runs.
- **Commit:** dc0529e

**3. [Rule 3 - Blocking] clippy `needless_pass_by_value` on the append wrapper**
- **Fix:** `record` moves the event into a `move` closure that clones it once per attempt.
- **Commit:** dc0529e

Test-support refactor: flow.rs's private `options`, `sweep_proposer`, `single_shot_proposer`, `calls`, and `kinds` moved to support.rs so the new modules share them. Flow's tests did not change.

## TDD Gate Compliance

Task 2 is marked `tdd="true"`, but the plan orders the driver's behavior first (Task 1, `feat` dc0529e) and the tests second (`test` 1e28be4), so no failing RED run came before GREEN. The five mutation runs above stand in for that evidence.

## Known Stubs

None.

## Threat Flags

None. The plan adds no endpoint, file access, or trust-boundary surface. Stops arrive only through the ledger and caps only through the validated manifest, and job and provider failures arrive as the existing typed failures. Mitigations T-07-15-01 through T-07-15-07 are each implemented and tested as listed above.

## Self-Check: PASSED

- FOUND: crates/minos-loop/src/driver.rs, crates/minos-loop/tests/driver/{caps,breaker,stops,pins}.rs
- FOUND commits: dc0529e, 1e28be4
