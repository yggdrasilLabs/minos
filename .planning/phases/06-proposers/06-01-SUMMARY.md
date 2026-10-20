---
phase: 06-proposers
plan: 01
subsystem: core-ledger
tags: [ledger, wire-format, stop-reason, golden, proptest]
status: complete

requires:
  - phase: 03-core
    provides: frozen ledger Event set, StopReason, EventKey scheme, golden wire pin
provides:
  - "StopReason::ProviderAuth (serde provider_auth, key tag 7, Display: model provider authentication failed; log in again)"
  - "Golden wire line record_v1_stop_requested_provider_auth and a recomputed 25-line byte pin"
affects: [06-04 ClaudeCli classifier, 07 loop stop handling]

tech-stack:
  added: []
  patterns:
    - "New closed enum variant appended with an explicit tag arm; existing wire bytes untouched, pin recomputed outside the code under test"

key-files:
  created:
    - crates/minos-core/tests/snapshots/golden_wire__record_v1_stop_requested_provider_auth.snap
  modified:
    - crates/minos-core/src/ledger.rs
    - crates/minos-core/tests/prop_keys.rs
    - crates/minos-core/tests/golden_wire.rs

key-decisions:
  - "StopReason::ProviderAuth uses key tag 7 as an explicit match arm, distinct from ProviderLimit (6), so an auth failure is never counted as a rate limit (D-15)"

patterns-established:
  - "Golden pin procedure validated by first reproducing the old pin over the old line set, then hashing the new set"

requirements-completed: [PROP-09]

coverage:
  - id: D1
    description: "StopReason::ProviderAuth with tag 7, fixed Display text, serde name provider_auth, documented tag"
    requirement: PROP-09
    verification:
      - kind: unit
        ref: "crates/minos-core/tests/prop_keys.rs#provider_auth_stop_record_parses"
        status: pass
      - kind: unit
        ref: "crates/minos-core/tests/prop_keys.rs#provider_auth_key_differs_from_every_other_reason"
        status: pass
      - kind: unit
        ref: "crates/minos-core/tests/prop_keys.rs#stop_reason_display_strings_are_fixed"
        status: pass
    human_judgment: false
  - id: D2
    description: "Golden wire line for the new stop reason; all existing snapshots byte-unchanged; pin over 25 lines"
    requirement: PROP-09
    verification:
      - kind: unit
        ref: "crates/minos-core/tests/golden_wire.rs#golden_snapshots"
        status: pass
      - kind: unit
        ref: "crates/minos-core/tests/golden_wire.rs#all_golden_bytes_pinned"
        status: pass
    human_judgment: false

duration: 4min
completed: 2026-10-19
---

# Phase 6 Plan 01: StopReason::ProviderAuth Summary

**Closed ledger stop reason `provider_auth` (key tag 7, fixed Display text) so a mid-run login failure stops a run with its own recorded reason, distinct from a rate limit, with one new pinned golden wire line and no existing wire byte moved.**

## Performance

- **Duration:** about 4 min
- **Completed:** 2026-10-19
- **Tasks:** 2
- **Files modified:** 4 (1 created)

## Accomplishments

- `StopReason::ProviderAuth` added after `ProviderLimit` in `crates/minos-core/src/ledger.rs`: doc comment per D-15, `Self::ProviderAuth => 7` tag arm, Display `model provider authentication failed; log in again`, module docs list `ProviderAuth` 7. `#[non_exhaustive]`, derives, and serde attributes unchanged.
- `prop_keys.rs`: the independent-model property now draws all seven reasons (`0_u8..7`, explicit `5 => ProviderLimit`, `_ => ProviderAuth`); Display and snake_case tests extended; new `provider_auth_stop_record_parses` (parses `{"v":"1","stop_requested":{"generation":3,"reason":"provider_auth"}}` and round-trips it) and `provider_auth_key_differs_from_every_other_reason` (seven distinct keys at generation 3).
- `golden_wire.rs`: entry `record_v1_stop_requested_provider_auth` directly after `record_v1_stop_requested`, line count 25, pin procedure comment updated, pin `dedebd15e58765bb6106c6932023bab9d99c623f918cc5abcb58b3e4870a368f`.

## Task Commits

1. **Task 1 RED: failing tests** - `fa19cbd` (test)
2. **Task 1 GREEN: the variant** - `7bd82cd` (feat)
3. **Task 2: golden line and pin** - `1378ca5` (test)

## Verification

- `cargo test -p minos-core`: all targets pass (prop_keys 34, golden_wire 32).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all --check` and `cargo doc -p minos-core --no-deps`: clean.
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing.
- Snapshot acceptance with `INSTA_UPDATE=always` produced exactly one new untracked file and modified no existing snapshot. The new body is `{"v":"1","stop_requested":{"generation":3,"reason":"provider_auth"}}`.
- Pin check: the documented awk and shasum loop over the old 24 names reproduced the old pin `8020cfde...1039` first, so the procedure was confirmed before the 25-name digest replaced it.

## Decisions Made

None beyond the plan: tag 7 and the Display text are as D-15 and the plan specify.

## Deviations from Plan

None - plan executed exactly as written. One clarification: the plan says to add the entry "in `goldens()`"; the event list actually lives in `golden_records()`, which `goldens()` serializes first, so the entry went there and the order of `goldens` is as the plan intends.

## TDD Gate Compliance

RED `fa19cbd` (test, failed to compile on the missing variant) precedes GREEN `7bd82cd` (feat). No refactor needed.

## Issues Encountered

None.

## Next Phase Readiness

Plan 06-04's `ClaudeCli` classifier can map a 401/403 result or a mid-run auth refusal to `StopReason::ProviderAuth`.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/ledger.rs, crates/minos-core/tests/prop_keys.rs, crates/minos-core/tests/golden_wire.rs, crates/minos-core/tests/snapshots/golden_wire__record_v1_stop_requested_provider_auth.snap
- FOUND commits: fa19cbd, 7bd82cd, 1378ca5
