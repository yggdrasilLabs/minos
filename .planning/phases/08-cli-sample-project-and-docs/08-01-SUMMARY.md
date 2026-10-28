---
phase: 08-cli-sample-project-and-docs
plan: 01
subsystem: core-and-loop-seams
status: complete
tags: [cost, cli-seams, limits, policy, state, loop]
requires: []
provides:
  - minos_core::money::CostLine
  - minos_core::limits::{CLI_DEFAULT_OUTPUT_TOKENS, BYTES_PER_TOKEN_CONSERVATIVE, batch_output_tokens, near_output_cap}
  - minos_core::policy::run_cost
  - minos_core::state::RunState::proposals
  - minos_loop::history::{recorded_manifest, recorded_manifest_at}
  - minos_loop::inputs::RunInputs::surface_bytes
affects: [08-02 onward: every CLI command that prints cost, check's D-08 advisory, status, show, report, adopt prefix resolution]
tech-stack:
  added: []
  patterns:
    - "CostLine delegates the dollar figure to ApproxCost's Display, keeping one dollar formatter"
    - "Exact saturating integer ceiling via w = q*d + r decomposition (no u128 truncating cast)"
    - "One shared read-back for the recorded manifest, used by the driver and the CLI"
key-files:
  created:
    - crates/minos-core/tests/cli_seams.rs
  modified:
    - crates/minos-core/src/money.rs
    - crates/minos-core/src/limits.rs
    - crates/minos-core/src/policy.rs
    - crates/minos-core/src/state.rs
    - crates/minos-core/tests/money.rs
    - crates/minos-loop/src/history.rs
    - crates/minos-loop/src/driver.rs
    - crates/minos-loop/src/inputs.rs
key-decisions:
  - "batch_output_tokens is exact whenever the true ceiling fits u64 and saturates otherwise: computed as q*batch + ceil(r*batch/d) with q, r the quotient and remainder of writable_bytes by BYTES_PER_TOKEN_CONSERVATIVE, so no u128-to-u64 truncating cast (clippy cast_possible_truncation) is needed"
  - "run_cost returns Cost (unrounded); callers take .approx() or .micro_dollars(), and CostLine takes the Usage separately (RunState::total_usage)"
requirements-completed: [COST-02, CLI-05, CLI-06]
coverage:
  - deliverable: "CostLine prints approx. cost with input-side and output token counts through ApproxCost"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-core/tests/money.rs#cost_line_sums_input_side_tokens_and_rounds_through_approx_cost"
        status: pass
      - kind: test
        ref: "crates/minos-core/tests/money.rs#cost_line_shows_zero_cost_and_zero_tokens"
        status: pass
  - deliverable: "D-08 constants and integer advisory arithmetic"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-core/tests/cli_seams.rs#batch_output_tokens_saturates"
        status: pass
      - kind: test
        ref: "crates/minos-core/tests/cli_seams.rs#near_output_cap_fires_at_eighty_percent"
        status: pass
  - deliverable: "run_cost is the one spend sum; spend_ok uses it"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-core/tests/cli_seams.rs#run_cost_sums_every_spend_as_spend_ok_does"
        status: pass
      - kind: test
        ref: "crates/minos-core/tests/cli_seams.rs#run_cost_of_an_unpriced_model_is_an_error"
        status: pass
      - kind: test
        ref: "crates/minos-core/tests/breaker.rs (existing spend_ok tests, unchanged)"
        status: pass
  - deliverable: "RunState::proposals in ExperimentId order"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-core/tests/cli_seams.rs#proposals_lists_every_generation_in_experiment_id_order"
        status: pass
  - deliverable: "recorded_manifest, recorded_manifest_at, and RunInputs::surface_bytes are public and the driver uses them"
    human_judgment: false
    verification:
      - kind: command
        ref: "cargo test -p minos-loop (95 tests, none changed)"
        status: pass
metrics:
  duration: 6 min
  started: 2026-10-28T17:20:43Z
  completed: 2026-10-28
  tasks: 2
  files: 9
---

# Phase 8 Plan 01: Core and Loop Seams for the CLI Summary

The cost line with token counts (`CostLine` over `ApproxCost`), one shared run-cost sum (`run_cost`), the D-08 output-cap advisory arithmetic in integers, an all-proposals iterator, the advisory-only `noise_check` rustdoc, and the loop's manifest read-back and surface size made public for the CLI.

## Accomplishments

- `minos_core::money::CostLine`: `new(ApproxCost, &Usage)`, getters, and a `Display` that writes the `ApproxCost` through its own `Display` and then ` (in N tokens, out M tokens)`. "in" saturates over input, cache creation, and cache read tokens. `ApproxCost` is still the only type that writes `$`; its doc now points at `CostLine` for COST-02.
- `minos_core::limits`: `CLI_DEFAULT_OUTPUT_TOKENS = 32_000` (source cited: code.claude.com/docs/en/env-vars, 06-RESEARCH Pitfall 5, 08-RESEARCH A2), `BYTES_PER_TOKEN_CONSERVATIVE = 2` (no official figure; deliberately low; advisory only; 08-RESEARCH A1, D-08), `batch_output_tokens` (exact ceiling, saturating) and `near_output_cap` (80 % threshold, saturating multiply).
- `minos_core::policy::run_cost(state, prices) -> Result<Cost, CostError>`: the summing loop extracted from `spend_ok`, which now calls it. An unpriced model is still an error, never free.
- `noise_check` rustdoc: the verdict is advisory; `minos check` prints it with advice and `minos run` prints it at start, and neither refuses (08-CONTEXT D-07). No code path changed.
- `RunState::proposals()`: every proposal in `ExperimentId` order; the baseline is not a proposal.
- `minos_loop::history::{recorded_manifest, recorded_manifest_at}`: public wrappers over one private `manifest_blob` read-back with the 07-REVIEW WR-03 error mapping unchanged. The driver's private copy is deleted and the driver calls `history::recorded_manifest`.
- `RunInputs::surface_bytes()`: replaces the driver's private `surface_bytes`; the driver calls `self.inputs.surface_bytes()`.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 1cb16ae | test(08-01): add failing tests for CostLine and the D-08 arithmetic |
| 1 | GREEN | 681d987 | feat(08-01): add CostLine and the D-08 output advisory arithmetic |
| 2 | RED | c391ec2 | test(08-01): add failing tests for run_cost and RunState::proposals |
| 2 | GREEN | e3b901b | feat(08-01): add run_cost, RunState::proposals, and the shared loop read-backs |

## Files Created/Modified

- `crates/minos-core/src/money.rs`: `CostLine`; `ApproxCost` doc sentence updated
- `crates/minos-core/src/limits.rs`: the single-shot output advisory section
- `crates/minos-core/src/policy.rs`: `run_cost`; `spend_ok` delegates; `noise_check` rustdoc
- `crates/minos-core/src/state.rs`: `RunState::proposals`
- `crates/minos-core/tests/money.rs`: two `CostLine` display tests
- `crates/minos-core/tests/cli_seams.rs` (new): D-08 arithmetic, `run_cost`, and `proposals` tests
- `crates/minos-loop/src/history.rs`: `recorded_manifest`, `recorded_manifest_at`, private `manifest_blob`, module doc section
- `crates/minos-loop/src/driver.rs`: private copies removed; uses the shared helpers; unused imports dropped
- `crates/minos-loop/src/inputs.rs`: `RunInputs::surface_bytes`

## Decisions Made

- `batch_output_tokens` computes `q * batch + ceil(r * batch / d)` where `writable_bytes = q * d + r`. The second term is below `d * 2^32` and cannot overflow, and the first saturates, so the result is exact whenever it fits and `u64::MAX` otherwise. A `u128` product would have needed a `u128 as u64` cast, which `clippy::cast_possible_truncation` refuses under `-D warnings`.
- `run_cost` returns the unrounded `Cost`, so callers round once (`approx()` or `micro_dollars()`), as `spend_ok` does.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Saturation semantics of `batch_output_tokens`**
- **Found during:** Task 1 GREEN
- **Issue:** The first version saturated the byte product before dividing, so `batch_output_tokens(u64::MAX, 16)` returned `2^63` instead of the `u64::MAX` the behavior list requires (the true value, `u64::MAX * 8`, does not fit).
- **Fix:** Use the exact quotient and remainder decomposition above, which saturates only the final value.
- **Files modified:** `crates/minos-core/src/limits.rs`
- **Commit:** 681d987

**2. [Rule 3 - Blocking] Test fixture shape under clippy**
- **Found during:** Task 1 GREEN
- **Issue:** A test returning `TestResult` with no fallible step tripped `clippy::unnecessary_wraps`.
- **Fix:** That test returns `()`. `TestResult` came back in Task 2 where tests are fallible.
- **Files modified:** `crates/minos-core/tests/cli_seams.rs`
- **Commit:** 681d987

I also added a few assertions beyond the behavior list: `batch_output_tokens(10, 4) == 20`, `batch_output_tokens(u64::MAX, 1) == 2^63`, `near_output_cap(0)` and `near_output_cap(32_000)`, `run_cost` of an empty run is `Cost::ZERO`, and the constants are pinned.

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking). **Impact:** none on the plan's scope; every pre-existing core and loop test passes unchanged.

## TDD Gate Compliance

Both tasks have a `test(08-01)` RED commit followed by a `feat(08-01)` GREEN commit (1cb16ae then 681d987; c391ec2 then e3b901b). Both RED runs failed to compile on the missing items. Neither task needed a refactor commit.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: pass
- `cargo test -p minos-core -p minos-loop`: pass, 0 failures
- `cargo doc -p minos-core -p minos-loop --no-deps`: no warnings
- C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing
- `git diff --stat -- crates/minos-core/tests/prop_policy.rs crates/minos-loop/tests`: no change
- All acceptance-criteria greps for both tasks print the expected lines

## Known Stubs

None.

## Issues Encountered

None.

## Next Phase Readiness

Ready for 08-02. CLI commands can print `CostLine` from `run_cost(state, prices)?.approx()` and `state.total_usage()`, run the D-08 advisory with `batch_output_tokens(writable_bytes, batch)` and `near_output_cap`, resolve experiment prefixes over `RunState::proposals()`, read a run's manifest with `recorded_manifest_at`, and size the single-shot surface with `RunInputs::surface_bytes()`.

## Self-Check: PASSED

- FOUND: crates/minos-core/tests/cli_seams.rs
- FOUND: 1cb16ae, 681d987, c391ec2, e3b901b
