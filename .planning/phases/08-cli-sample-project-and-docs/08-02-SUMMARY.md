---
phase: 08-cli-sample-project-and-docs
plan: 02
subsystem: store
status: complete
tags: [store, ledger, run-listing, cli-seams]
requires: []
provides:
  - minos_store::Ledger::runs (trait method)
  - MemoryStore::runs and FileStore::runs
affects: [08 check's D-05 diagnostic-run reuse, every run-taking command's D-10 hex prefix and latest-run default, the AWS milestone's DynamoDB ledger (same trait method)]
tech-stack:
  added: []
  patterns:
    - "A run is listed iff Ledger::read would return at least one line or an error: lock-only, missing, empty, and torn-only ledgers are not runs; a corrupt ledger is listed so its refusal reaches a human"
    - "Listing reads only the first line of each ledger (capped at MAX_LEDGER_LINE_BYTES + 1) through open_existing, takes no lock, and creates nothing"
key-files:
  created: []
  modified:
    - crates/minos-store/src/lib.rs
    - crates/minos-store/src/file/ledger.rs
    - crates/minos-store/src/memory.rs
    - crates/minos-store/tests/contract.rs
    - crates/minos-loop/tests/blocks.rs
    - crates/minos-loop/tests/driver/support.rs
    - CLAUDE.md
key-decisions:
  - "FileStore::runs lists a run only when its ledger's first line is newline-terminated or over the line cap, not merely when ledger.jsonl exists: an empty file or a short unterminated tail (a torn first append) reads as zero lines, so it is not a run; a ledger read would refuse (BadJson, LineTooLong) is still listed so status can report it"
  - "Listing takes no ledger lock, because taking one would create ledger.lock as a side effect; an append writes line and newline in one write_all and only bytes after the last newline are ever truncated, so a newline once seen stays"
  - "A symlinked ledger.jsonl is Unsafe { what: \"ledger file\" }, the same text read and append use, rather than the plan's \"ledger\""
requirements-completed: []
coverage:
  - deliverable: "Ledger::runs on the trait and both stores, under one contract"
    human_judgment: false
  - deliverable: "FileStore::runs refuses symlinks, non-directories, non-RunId names, a symlinked runs/, and a non-regular ledger; creates nothing"
    human_judgment: false
metrics:
  duration: 4min
  completed: 2026-10-28
  tasks: 1
  files: 7
---

# Phase 8 Plan 02: Ledger::runs Summary

`Ledger::runs` lists every run with at least one appended line as a `BTreeSet<RunId>` on both `MemoryStore` and `FileStore`; the file store creates nothing, takes no lock, and refuses any symlink or foreign entry under `runs/` as `StoreError::Unsafe`.

## What was built

- **The trait method.** `fn runs(&self) -> Result<BTreeSet<RunId>, StoreError>` on `Ledger` (crates/minos-store/src/lib.rs), documented with `# Errors` naming `Io`, `Unsafe`, and `Poisoned`. Because it is a trait method, the AWS milestone's DynamoDB ledger implements the same contract.
- **MemoryStore.** The key set of the ledger map, filtered to runs with at least one line, under the same mutex `read` takes; a poisoned lock is `Poisoned`.
- **FileStore.** A missing `runs/` (checked through `dir_exists`, which refuses a symlink) lists nothing and creates nothing. Every entry is checked with `symlink_metadata`: a symlink or non-directory, or a name that is not UTF-8 or not 64 lowercase hex, is `Unsafe { what: "run directory" }`. Each run's `ledger.jsonl` is opened read-only through `open_existing` (symlink and inode checks; a symlink or non-file is `Unsafe { what: "ledger file" }`), and only its first line is read, capped at `MAX_LEDGER_LINE_BYTES + 1`.
- **Forwarders.** The two test-only implementors in minos-loop (`Counting`, `FaultStore<S>`) forward `runs` to their inner store.
- **CLAUDE.md.** The minos-store section names `runs` and its listing rule.

## Tests

Contract cases for both stores: `runs_lists_appended_runs`, `runs_of_an_empty_store_is_empty`, `a_locked_run_is_not_listed`. File-only cases: a fresh store lists nothing and creates no `runs/`; listing a lock-only run creates nothing in its directory; an empty ledger and a torn-only ledger are not listed; a garbled ledger and an unterminated over-cap ledger are listed (and `read` refuses both); a symlink, a regular file, a `not-a-run` directory, an uppercase-hex directory under `runs/`, a symlinked `ledger.jsonl`, and a symlinked `runs/` are each `Unsafe` with the expected `what`.

Gates run: `cargo test -p minos-store` (59 contract tests pass), `cargo test -p minos-loop`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo doc -p minos-store --no-deps`, and the C-02 grep (prints nothing).

## Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 3d42ba9 | test(08-02): add failing contract tests for Ledger::runs |
| 1 | GREEN | 0aeeb91 | feat(08-02): add Ledger::runs to both stores |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] A run is listed by its first complete line, not by the ledger file's existence**
- **Found during:** Task 1
- **Issue:** The plan listed a run "only when its `ledger.jsonl` exists as a regular file". An append that creates the file and then fails (`LineTooLong`), or a crash mid-way through the first append, leaves a ledger that `read` gives as zero lines, so listing it would contradict the must-have truth "every run with at least one appended line" and hand the CLI a run with no `RunStarted`.
- **Fix:** `lists_its_run` reads the first line (capped) and lists the run when it is newline-terminated or over the line cap. Empty and torn-only ledgers are not listed; a ledger `read` would refuse is listed so its refusal reaches a human. Two tests pin both directions.
- **Files modified:** crates/minos-store/src/file/ledger.rs, crates/minos-store/tests/contract.rs
- **Commit:** 0aeeb91

**2. [Rule 3 - Blocking] Test plants use a `create_new` helper rather than `fs::write`**
- **Found during:** Task 1
- **Issue:** minos-store's clippy configuration disallows `std::fs::write` (STORE-04), so the planted-entry tests failed the lint gate.
- **Fix:** A `plant` helper that opens with `create_new(true)` and `write_all`, as the existing driver-lock symlink test does.
- **Files modified:** crates/minos-store/tests/contract.rs
- **Commit:** 0aeeb91

**3. [Rule 2 - Documentation] CLAUDE.md updated in the same change**
- CLAUDE.md's minos-store section names the `Ledger` methods; it now names `runs` and its listing rule, per "Keep this file current". CLAUDE.md was not in the plan's `files_modified`.

The `Unsafe` text for a symlinked ledger is `"ledger file"`, matching `read` and `append`, not the plan's `"ledger"`.

## TDD Gate Compliance

RED (`test(08-02)`, 3d42ba9: the contract file failed to compile because no `runs` method existed) precedes GREEN (`feat(08-02)`, 0aeeb91). No refactor commit was needed.

## Known Stubs

None.

## Threat Flags

None. The listing adds no new trust boundary beyond T-08-02-01 to T-08-02-03, each mitigated and tested.

## Self-Check: PASSED

- FOUND: crates/minos-store/src/lib.rs `fn runs(&self) -> Result<BTreeSet<RunId>, StoreError>`
- FOUND: commit 3d42ba9
- FOUND: commit 0aeeb91
