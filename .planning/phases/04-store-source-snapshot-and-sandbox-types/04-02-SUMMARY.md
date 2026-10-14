---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 02
subsystem: store
tags: [minos-store, ledger, blobs, contract-tests, proptest, thiserror]

requires:
  - phase: 03-core-types-and-pure-logic
    provides: Line, Record, Event::key, EventKey, BlobId::of, RunId::of
  - phase: 04-store-source-snapshot-and-sandbox-types
    plan: 01
    provides: minos-store dependency edges and its clippy.toml bans
provides:
  - Ledger and Blobs synchronous traits, the contract every store implements
  - Appended (New, Duplicate, Conflict), LedgerRead, CorruptKind, StoreError with is_retryable
  - MemoryStore, the in-memory Ledger and Blobs for every test above the store
  - tests/contract.rs with ledger_contract! and blob_contract! macros ready for the file-store arm
affects: [04-05, phase-7-loop, phase-8-cli]

tech-stack:
  added: []
  patterns:
    - "Contract tests: one generic fn per case over L: Ledger or B: Blobs, expanded by a macro into one #[test] per implementation; arms reach the case through super::, so there is no glob import"
    - "Appended is #[must_use]: a caller that drops a Conflict gets a compile error under -D warnings"

key-files:
  created:
    - crates/minos-store/src/memory.rs
    - crates/minos-store/tests/contract.rs
  modified:
    - crates/minos-store/src/lib.rs

key-decisions:
  - "04-02: LedgerRead::new stays pub(crate) as planned, so only minos-store can build a LedgerRead; a store implemented in another crate (the AWS milestone) would need a public constructor, to be decided then"
  - "04-02: Appended is not #[non_exhaustive]: its three outcomes are complete, and the loop should match them exhaustively"
  - "04-02: STORE-03 and STORE-04 stay unchecked; this plan delivers the contract and the memory half, and 04-05's file store completes both"

patterns-established:
  - "Store errors: Display names IDs, numbers, and fixed &'static str context only; never file bytes"

requirements-completed: [STORE-01]

coverage:
  - id: D1
    description: "Ledger and Blobs are sync traits with &self methods, documented as a contract with # Errors sections"
    requirement: "STORE-01"
    verification:
      - kind: other
        ref: "grep -c '# Errors' crates/minos-store/src/lib.rs (6); cargo doc --workspace --no-deps"
        status: pass
    human_judgment: false
  - id: D2
    description: "A same-key append writes nothing: Duplicate for an equal record, Conflict for a different one, both with the first line's seq"
    requirement: "STORE-03"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/contract.rs::duplicate_append_is_a_no_op, conflicting_append_is_not_written, append_sequence_keeps_first_write"
        status: pass
    human_judgment: false
  - id: D3
    description: "Blobs are content-addressed; get refuses a size over max before copying, rehashes, and reports a missing blob"
    requirement: "STORE-04"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/contract.rs::put_returns_the_content_address, get_over_max_is_refused, get_missing_is_refused, contains_tracks_put, put_is_idempotent"
        status: pass
    human_judgment: false
  - id: D4
    description: "StoreError: Io is the only retryable variant; Corrupt names run, 1-based line, and kind"
    verification:
      - kind: test
        ref: "crates/minos-store/src/lib.rs tests::only_io_is_retryable, corrupt_display_names_the_run_line_and_kind"
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-10-14
status: complete
---

# Phase 4 Plan 02: Store contract and MemoryStore Summary

**Synchronous `Ledger` and `Blobs` traits with a `#[must_use]` `Appended::{New, Duplicate, Conflict}` result, a `StoreError` that separates retryable I/O from corruption and blob faults, a `Mutex<BTreeMap>` `MemoryStore`, and one contract test file whose macros the file store joins in 04-05.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-10-14T20:14:18Z
- **Completed:** 2026-10-14T20:19:32Z
- **Tasks:** 2 (each as a RED and a GREEN commit)
- **Files modified:** 3 (2 created, 1 modified)

## Accomplishments

- `lib.rs` documents the store contract: lines are in `seq` order from 0 with no gap, a same-key append writes nothing, an append is durable before it returns, and a blob is addressed by its plain SHA-256 and rehashed on every read.
- `Appended::seq` returns the seq of the line that holds the key. `Conflict` covers 04-RESEARCH Pitfall 4: the first write wins, and the caller is told.
- `CorruptKind` (`UnknownVersion`, `BadJson`, `KeyMismatch`, `SeqGap`, `LineTooLong`) and `StoreError` (`Io`, `Corrupt`, `LedgerTooLarge`, `Encode`, `BlobMissing`, `BlobTooLarge`, `BlobTampered`, `Unsafe`, `Poisoned`) are both `#[non_exhaustive]`. `is_retryable` is true for `Io` only.
- `MemoryStore` maps a poisoned lock to `StoreError::Poisoned` and holds a guard only for the map operation. `get` checks the size against `max` before copying, then rehashes the copy.
- `tests/contract.rs` has 10 contract cases (5 ledger, 5 blob), a Send + Sync check, and the `append_sequence_keeps_first_write` property. The property checks every append result and the final read against a first-write-wins model over a 9-key space.

## Task Commits

1. **Task 1: The store contract in lib.rs**: `e384eb8` (test, RED), `78854a8` (feat, GREEN)
2. **Task 2: MemoryStore and the contract test file**: `62b4dbf` (test, RED), `8b69aeb` (feat, GREEN)

## Files Created/Modified

- `crates/minos-store/src/lib.rs`: traits, `Appended`, `LedgerRead`, `CorruptKind`, `StoreError`, the `memory` module and `MemoryStore` re-export, and unit tests
- `crates/minos-store/src/memory.rs`: `MemoryStore`
- `crates/minos-store/tests/contract.rs`: the contract macros, cases, and property

## Decisions Made

- `Appended` is not `#[non_exhaustive]`, because its three outcomes are complete. `CorruptKind` and `StoreError` are.
- `LedgerRead::new` is `pub(crate)`, as planned. A future store in another crate would need a public constructor; that is left to the AWS milestone.
- Lengths are converted with `u64::try_from(len).unwrap_or(u64::MAX)`, the existing workspace idiom. A length that does not fit counts as larger than any cap.

## Deviations from Plan

### Moved within the plan

**1. [Rule 3 - Blocking] `LedgerRead::new` and `pub mod memory;` landed in Task 2, not Task 1**
- **Found during:** Task 1
- **Issue:** A `pub(crate)` constructor with no caller trips `dead_code`, so Task 1's `cargo clippy -D warnings` check would fail. Declaring `pub mod memory;` in Task 1 needs `memory.rs`, which is Task 2's file.
- **Fix:** Task 1 shipped `LedgerRead` with its accessors only. `new`, the module declaration, and the `MemoryStore` re-export came in Task 2's GREEN commit, together with their first caller.
- **Files modified:** crates/minos-store/src/lib.rs
- **Commit:** 8b69aeb

**2. [Rule 1 - Bug] A contract case dropped an `Appended` value**
- **Found during:** Task 2 GREEN
- **Issue:** `unknown_run_reads_empty` ignored the result of `append`, and `#[must_use]` refused it under `-D warnings`.
- **Fix:** The case now asserts `Appended::New { seq: 0 }`.
- **Files modified:** crates/minos-store/tests/contract.rs
- **Commit:** 8b69aeb

### Requirement checkboxes

- Only STORE-01 is marked complete. STORE-03 and STORE-04 also apply to the file store (04-05 lists both), and STORE-04's wording is about the file blob store. Following 04-01's practice, REQUIREMENTS.md does not claim code that does not exist yet.

**Total deviations:** 2 auto-fixed (1 blocking order change, 1 test bug) and one bookkeeping choice. **Impact:** none on the delivered contract.

## Issues Encountered

None. The full `cargo test --workspace` passed on the first run (746 passed, 0 failed, 25 ignored). The known flaky `minos-backend-krun` test did not fail.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 746 passed, 0 failed, 25 ignored
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`. The duplicate-version and wrapper warnings were already there; `deny.toml` is unchanged.
- `cargo doc --workspace --no-deps`: exit 0
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing
- `test ! -e crates/minos-store/tests/common`: ok

## TDD Gate Compliance

Each task has a `test(04-02)` RED commit that failed to compile against the missing items, followed by a `feat(04-02)` GREEN commit. There was no refactor step.

## Next Phase Readiness

04-05 adds a `#[test] fn file()` arm to `ledger_contract!` and `blob_contract!`, and calls `keeps_first_write` with a `FileStore`. It uses the `CorruptKind`, `LedgerTooLarge`, `Encode`, `Unsafe`, and `Io` variants that only the file store produces.

## Self-Check: PASSED

- FOUND: crates/minos-store/src/lib.rs, crates/minos-store/src/memory.rs, crates/minos-store/tests/contract.rs
- FOUND: commits e384eb8, 78854a8, 62b4dbf, 8b69aeb
