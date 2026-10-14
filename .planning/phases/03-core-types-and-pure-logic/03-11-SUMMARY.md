---
phase: 03-core-types-and-pure-logic
plan: 11
subsystem: minos-core
status: complete
tags: [core, policy, routing, circuit-breaker, spend, proptest, permutation-invariance]
requires:
  - "03-03: Metric (ONE, Ord), Scorecard (get, value, iter), Outcome::scorecard, Reason"
  - "03-04: Prices, Cost::of, Cost::checked_add, Cost::micro_dollars, ApproxCost, CostError, Usage::total_tokens"
  - "03-05: Manifest (objective, judge, budget, policy, proposers, params), Objective, Goal, MinImprovement::threshold_at, JudgeSpec, JudgeVersion, Budget, ProposerSpec::declared_eligibility, Tier::rank, Eligibility, IneligibleReason::SurfaceTooLarge"
  - "03-07: Decision, DecisionKind, StopReason, SpendSource"
  - "03-10: fold, RunState (proposals_in, trials, decision, decisions, last_concluded_generation, incumbent_before, baseline, spends, total_usage)"
provides:
  - "policy.rs: check_scorecard, lower_median, ReducedScore (metrics, get, trials), reduce_trials, improves, Selection (Improved, Kept), select_greedy, conclude, PolicyError (IncumbentUnscored, TooManyProposals, GenerationZero), NoiseFloor (median, min, max, trials, spread), noise_floor, NoiseVerdict (Ok, Warn, Refuse), noise_check, circuit_breaker, SpendCheck (Within, Stop), spend_ok"
  - "route.rs: SurfaceStats (new, bytes), ProposerSpec::eligible, RouteRow (index, tier, eligibility), route_table, Route (proposer, tier, stall, step), route, RouteError::NoEligibleTier"
  - "tests/prop_policy.rs, tests/route.rs, tests/breaker.rs"
affects: [03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Greedy best is the max of a total order over (oriented value, reversed ExperimentId) pairs, never first-seen, so duplicate IDs and any permutation give the same Selection"
    - "Multiply-and-compare improvement test: bound = incumbent +/- threshold_at(incumbent); a non-finite threshold or bound is no improvement"
    - "Proptest record-order permutation by sorting the ledger tail on 64 generated u64 keys, combined with per-experiment prop_shuffle of trial values"
key-files:
  created:
    - crates/minos-core/tests/prop_policy.rs
    - crates/minos-core/tests/prop_policy.proptest-regressions
    - crates/minos-core/tests/route.rs
    - crates/minos-core/tests/breaker.rs
  modified:
    - crates/minos-core/src/policy.rs
    - crates/minos-core/src/route.rs
decisions:
  - "conclude re-runs check_scorecard on every recorded Scored trial against the manifest's judge; a trial that fails it makes the experiment invalid, like an unscored trial (defense in depth for T-03-11-03)"
  - "check_scorecard also rejects a gate marked held-out with HeldOutMismatch, so a gate's flag must be false like an objective's"
  - "PolicyError gained GenerationZero: generation 0 is calibration, and the fold rejects any decision there"
  - "conclude checks the incumbent's score before the candidates, so an uncalibrated baseline is IncumbentUnscored even when no candidate is valid"
  - "noise_floor counts only Scored baseline trials that report the metric; noise_check returns Warn for a non-finite threshold and Refuse for an infinite spread"
  - "spend_ok compares rounded micro-dollars strictly above spend_cap_micro, checks spend before tokens, and treats a token-total overflow as over the cap"
  - "Route::step is the raw stall / stall_step; the chosen rung is min(step, top)"
metrics:
  duration: "8 min"
  completed: 2026-10-14
  tasks: 3
  files: 6
---

# Phase 3 Plan 11: Policy, Routing, Breaker, and Spend Summary

Pure, permutation-invariant loop decisions in minos-core: a gate-first scorecard check, lower-median trial reduction, multiply-and-compare greedy selection tie-broken on ExperimentId, a generation `conclude`, an advisory noise check, stall-step routing that resets on improvement with a reason for every ineligible tier, an exact-K circuit breaker, and a spend and token cap check priced from prices.toml rates.

## What was built

**policy.rs (CORE-10, D-11, D-12)**
- `check_scorecard(judge, card)` runs these checks in order: each gate (sorted) must be present, not marked held-out, and exactly `Metric::ONE`; each objective must be present and not held-out; each held-out pairing must be present and marked held-out; every card metric must be declared.
- `lower_median` sorts a copy and takes `(n - 1) / 2`. `reduce_trials` returns `None` for zero trials, any non-Scored trial, a metric missing from some trial, or a count that does not fit a `u32`. It never averages.
- `improves` follows the interfaces block exactly. Tests cover a zero incumbent (must be strictly better), a negative incumbent (uses its magnitude), absolute thresholds, and near-`f64::MAX` bounds that overflow to infinity (always false).
- `select_greedy` and `conclude` produce a `Decision` with `proposed` and `dropped` counts. `conclude` reads the incumbent through `incumbent_before(generation)`, which matches the 03-10 fold's `BadDecision` rule.
- `noise_floor` folds the baseline's trials. `noise_check` takes `&Objective`, returns a verdict only, and never changes the manifest.

**route.rs (CORE-11, D-13)**
- `ProposerSpec::eligible` runs `declared_eligibility` first. After that, a single_shot surface over `SINGLE_SHOT_SURFACE_MAX_BYTES` is `SurfaceTooLarge { bytes, limit }`.
- `route_table` lists rows in manifest order. `route` sorts the eligible rows by (`rank`, index) and counts stall as the number of decisions after the last Improved. It picks `min(stall / stall_step, top)` using only saturating or checked arithmetic.

**policy.rs (CORE-17, D-14, D-09)**
- `circuit_breaker(state, budget, infra_streak)` walks NoValidCandidates decisions down from the last generation. It stops at any other kind or a missing generation, trips at exactly `max_empty_generations`, and only then checks the infra streak.
- `spend_ok` sums `Cost::of` over every Spent record with checked adds. It returns `Stop(SpendCap)`, `Stop(TokenCap)`, or `Within { spent, tokens }`, and returns `Err(UnpricedModel)` for any model missing from prices.toml.

## Tasks and commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 459399b | test(03-11): add failing policy selection, conclude, and noise tests |
| 1 | GREEN | a0d6e1f | feat(03-11): implement gate check, trial reduction, greedy selection, conclude, and noise check |
| 2 | RED | 9cd321d | test(03-11): add failing eligibility and routing tests |
| 2 | GREEN | 9810448 | feat(03-11): implement eligibility with reasons and routing |
| 3 | RED | 691f54f | test(03-11): add failing circuit breaker and spend check tests |
| 3 | GREEN | 8d3bfdc | feat(03-11): implement circuit breaker and spend check |

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps` all pass.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` passes.
- C-01 printed no MISSING line. C-02 (`grep -rnE '#!?\[allow\(' crates --include='*.rs'`) printed nothing.
- The prohibition grep for `powf|powi|ln|log10|log2|exp|sqrt` in policy.rs and route.rs printed nothing. The `noise_check` signature takes `&Objective`.
- Test counts: prop_policy 40, route 10, breaker 12.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] conclude re-checks recorded scorecards**
- **Found during:** Task 1
- **Issue:** The plan's `conclude` accepted any Scored trial. A Scored record whose gate is not 1 would then reach selection if the runner ever skipped `check_scorecard` (T-03-11-03, a high-severity mitigate).
- **Fix:** `conclude` passes every trial through `check_scorecard` against the manifest's judge before reducing it. A trial that fails is treated as unscored. Test: `conclude_reports_no_valid_candidates` (e3).
- **Files modified:** crates/minos-core/src/policy.rs
- **Commit:** a0d6e1f

**2. [Rule 2 - Missing critical functionality] A gate marked held-out is HeldOutMismatch**
- **Found during:** Task 1
- **Issue:** The plan checked the held-out flag only for objectives and pairings, so a gate's flag went unchecked.
- **Fix:** Gates go through the same presence and flag check, then the exact-1 rule. Test: `check_scorecard_rejects_a_gate_marked_held_out`.
- **Commit:** a0d6e1f

**3. [Rule 2 - Missing critical functionality] PolicyError::GenerationZero**
- **Found during:** Task 1
- **Issue:** Concluding generation 0 would return a NoValidCandidates decision that the fold rejects as `BadDecision`.
- **Fix:** Added the variant (`PolicyError` is `#[non_exhaustive]`). Test: `conclude_refuses_generation_zero`.
- **Commit:** a0d6e1f

**4. [Rule 1 - Bug, test only] Permutation property's winner lookup**
- **Found during:** Task 1 GREEN
- **Issue:** When candidate IDs repeated, the property took the maximum value for the winner, which is wrong for minimize. Proptest wrote the failing seed to `prop_policy.proptest-regressions`, and it is kept as a duplicate-ID regression case.
- **Fix:** The property now takes the winner's best value in the goal's direction.
- **Commit:** a0d6e1f

## TDD Gate Compliance

All three tasks have a RED `test(...)` commit followed by a GREEN `feat(...)` commit. In each RED commit the test file failed to compile because the API did not exist yet, matching 03-10's convention. No REFACTOR commits were needed.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/policy.rs, crates/minos-core/src/route.rs, crates/minos-core/tests/prop_policy.rs, crates/minos-core/tests/route.rs, crates/minos-core/tests/breaker.rs
- FOUND commits: 459399b, a0d6e1f, 9cd321d, 9810448, 691f54f, 8d3bfdc
