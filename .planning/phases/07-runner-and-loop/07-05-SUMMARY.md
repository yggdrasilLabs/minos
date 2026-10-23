---
phase: 07-runner-and-loop
plan: 05
subsystem: minos-store (ledger append semantics, whole-run lock)
status: complete
tags: [store, ledger, lock, contract, D-08, LOOP-06]
requires:
  - phase: 07-01
    provides: "the Phase 7 ledger fields the store round-trips"
provides:
  - "Appended { New, Duplicate } only, still #[must_use]"
  - "StoreError::Conflict { seq } (non-retryable, nothing written, first write wins)"
  - "StoreError::RunLocked { run } (non-retryable)"
  - "RunLock trait (type Guard<'a>, lock_run) re-exported at the crate root"
  - "MemoryStore run lock over Mutex<BTreeSet<RunId>>; MemoryRunGuard (re-exported at the crate root)"
  - "FileStore run lock on runs/<run hex>/driver.lock; FileRunGuard (minos_store::FileRunGuard and minos_store::file::FileRunGuard)"
  - "run_lock_contract! cases second_lock_is_refused, lock_after_drop_succeeds, runs_lock_independently, locked_run_still_appends"
affects: [07-13, 07-15, 07-16]
tech-stack:
  added: []
  patterns:
    - "A same-key conflict is an Err, so a caller cannot carry on past it with ? as if it succeeded"
    - "A GAT guard (type Guard<'a>) lets the memory guard borrow the store while the file guard owns a File"
    - "flock per open file description: two FileStore handles in one process refuse each other, so the file case is testable in-process"
key-files:
  created:
    - crates/minos-store/src/file/run_lock.rs
  modified:
    - crates/minos-store/src/lib.rs
    - crates/minos-store/src/memory.rs
    - crates/minos-store/src/file/mod.rs
    - crates/minos-store/src/file/ledger.rs
    - crates/minos-store/tests/contract.rs
    - crates/minos-store/tests/file_ledger.rs
decisions:
  - "FileRunGuard is re-exported as minos_store::file::FileRunGuard and at the crate root, with run_lock a private module like blobs and ledger, so the module tree's style is kept"
  - "lock_run fsyncs a runs or run directory it created, matching the store's rule that a new directory entry is synced; the lock file itself holds no data and is not synced"
  - "MemoryRunGuard's Drop ignores a poisoned mutex rather than recovering it, because every later lock_run on that store already fails closed as Poisoned"
  - "CLAUDE.md is not edited here: plan 07-16 Task 3 is the one sequential writer for the D-08 and run-lock text"
metrics:
  duration: "about 9 minutes"
  completed: 2026-10-22
  tasks: 2
  files: 7
---

# Phase 7 Plan 05: Conflict as a store error and the whole-run lock Summary

A same-key append with a different record is now `Err(StoreError::Conflict { seq })` in both stores (D-08), so a caller cannot carry on past it with `?` as if it had succeeded. The store also gains a whole-run exclusive lock, the `RunLock` trait (LOOP-06). `MemoryStore` implements it over a `BTreeSet`, and `FileStore` implements it with `flock` on its own `driver.lock`, so a locked run can still be appended to.

## What was built

**Task 1 (D-08).** `Appended` keeps `New` and `Duplicate` and stays `#[must_use]`. `StoreError::Conflict { seq }` reads "a different record already holds this key at seq {seq}; the first write wins and the ledger needs a human", and `is_retryable` is false for it. Both stores return it from the key branch before any write. In the file store that branch still runs under the exclusive `ledger.lock`. The module docs and `Ledger::append`'s `# Errors` now name the error. Test changes:
- `conflicting_append_is_not_written` asserts the error and that it is not retryable, and checks that the ledger is unchanged before and after.
- `keeps_first_write` expects the error at the first seq.
- `append_only_prefix` matches on the `Result`.
- `memory_and_file_agree` compares a `New`/`Duplicate`/`Conflict`/`Other` projection, and a file append that projects to `Other` fails the case.

**Task 2 (LOOP-06).** `RunLock { type Guard<'a>: Sized where Self: 'a; fn lock_run(&self, &RunId) -> Result<Self::Guard<'_>, StoreError> }` sits beside `Ledger` and `Blobs`. A held run is `StoreError::RunLocked { run }`, which reads "run {run} is being driven by another minos process; wait for it to finish or stop it with minos stop" and is not retryable.
- **`MemoryStore`:** gains `run_locks: Mutex<BTreeSet<RunId>>`. `MemoryRunGuard` removes its run on drop and never panics on a poisoned mutex.
- **`FileStore`:** creates the runs and run directories the way `append` does. It opens `runs/<run hex>/driver.lock` through `open_or_create` (the symlink and dev/inode checks; `O_EXCL`, mode 0600, never truncated), then calls `File::try_lock`. `WouldBlock` becomes `RunLocked` and any other lock error becomes `Io`. It never touches `ledger.lock` (Pitfall 11).
- **Contract:** `run_lock_contract!` runs four cases against both stores. Three file-only tests cover mode 0600 (with the run directory still 0700), a planted symlink refused as `Unsafe { what: "driver lock file" }`, and two `FileStore` handles refusing each other while still appending. A `memory.rs` unit test poisons the mutex and checks for `Poisoned` with no panic in the guard's drop.

## Threat mitigations

| Threat | Mitigation as built |
|---|---|
| T-07-05-01 | `Err(StoreError::Conflict { seq })`, nothing written, non-retryable, in both stores; contract and property tests |
| T-07-05-02 | `RunLock` on both stores; `flock` is released by the kernel when the descriptor closes |
| T-07-05-03 | `driver.lock` goes through `open_or_create`; `a_symlink_at_driver_lock_is_refused` |
| T-07-05-04 | Separate `driver.lock`; `locked_run_still_appends` (both stores) and the two-handle test append while the run is held |
| T-07-05-05 | `MemoryRunGuard::drop` uses `if let Ok`; `a_poisoned_run_lock_fails_closed_and_the_guard_drops_quietly` |

## Verification

Each gate command below exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: every result ok, none FAILED. For minos-store: lib 9, contract 43, file_blobs 17, file_ledger 25.
- `cargo deny check`: advisories, bans, licenses, and sources ok.
- `cargo doc --workspace --no-deps`: no warnings.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed only `ok` lines, and C-02 printed nothing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing test] Poisoned-mutex coverage for the memory run lock**
- **Found during:** Task 2
- **Issue:** One must-have says a poisoned mutex is `Poisoned`, never a panic, including in the guard's drop. The plan's contract cases cannot poison a private mutex.
- **Fix:** Added a `#[cfg(test)]` unit test in `memory.rs` that poisons `run_locks` from a scoped thread.
- **Files modified:** crates/minos-store/src/memory.rs
- **Commit:** ef27b7c

**2. [Rule 3 - Blocking] `std::fs::write` is banned in minos-store**
- **Found during:** Task 2
- **Issue:** The store's `clippy.toml` bans `fs::write`, which the symlink test first used for its target file.
- **Fix:** Created the target with `OpenOptions::create_new`.
- **Commit:** ef27b7c

**Acceptance-grep note:** The plan asks for `grep -n 'driver.lock' crates/minos-store/src/file/mod.rs` to print one line. In a regex, `.` matches any character, so the plan's own function name `driver_lock_path` and the layout docs also match, and the grep prints four lines. The literal `driver.lock` appears in exactly two places: the constant and the layout table.

**Path note:** The plan's artifact list names `minos_store::file::run_lock::FileRunGuard`. `run_lock` is private, like `blobs` and `ledger`, and the guard is reachable as `minos_store::file::FileRunGuard` and `minos_store::FileRunGuard`.

## TDD Gate Compliance

RED `test(07-05)` c4d7b3c, then GREEN `feat(07-05)` 5a05e9a (Task 1). RED `test(07-05)` 81e9e2f, then GREEN `feat(07-05)` ef27b7c (Task 2). Each RED commit failed to compile: no `StoreError::Conflict` for Task 1, and no `minos_store::RunLock` for Task 2.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | c4d7b3c | test(07-05): add failing tests for a conflicting append as a store error |
| 1 GREEN | 5a05e9a | feat(07-05): make a conflicting append a non-retryable store error (D-08) |
| 2 RED | 81e9e2f | test(07-05): add failing run-lock contract tests for both stores |
| 2 GREEN | ef27b7c | feat(07-05): add the whole-run RunLock to both stores (LOOP-06) |

## Known Stubs

None.

## Self-Check: PASSED

- crates/minos-store/src/file/run_lock.rs exists.
- Commits c4d7b3c, 5a05e9a, 81e9e2f, and ef27b7c are all in `git log`.
