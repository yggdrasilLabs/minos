---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 05
subsystem: store
tags: [minos-store, file-ledger, blobs, file-lock, fsync, torn-tail, proptest, tempfile]

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    plan: 02
    provides: Ledger and Blobs traits, Appended, LedgerRead, StoreError, CorruptKind, MemoryStore, contract macros
  - phase: 03-core-types-and-pure-logic
    provides: Line, Line::verify, Record, Event::key, BlobId::of, RunId, golden line_v1
provides:
  - FileStore (open, Ledger, Blobs) and STORE_DIR = ".minos"
  - MAX_LEDGER_LINE_BYTES (1 MiB) and MAX_LEDGER_BYTES (64 MiB), re-exported from minos_store::file
  - StoreError::LineTooLong for a record whose serialized line is over the cap
  - File arms for every ledger and blob contract case, plus a file run of the first-write-wins property
affects: [phase-7-loop, phase-8-cli, 04-VERIFICATION]

tech-stack:
  added: []
  patterns:
    - "Store files: symlink_metadata check, open, then the handle's dev and inode must match the check; creation through create_new (O_EXCL), which never follows a symlink"
    - "Store directories: ensure_dir creates with mode 0700, treats a concurrent AlreadyExists as success after re-checking, and reports creation so the parent is fsynced"
    - "File-backed proptests run 32 cases: every append ends in sync_data (F_FULLFSYNC on macOS)"

key-files:
  created:
    - crates/minos-store/src/file/mod.rs
    - crates/minos-store/src/file/ledger.rs
    - crates/minos-store/src/file/blobs.rs
    - crates/minos-store/tests/file_ledger.rs
    - crates/minos-store/tests/file_blobs.rs
  modified:
    - crates/minos-store/src/lib.rs
    - crates/minos-store/tests/contract.rs

key-decisions:
  - "04-05: a final newline-terminated line counts as a torn tail only when it is not valid JSON at all; valid JSON that is not a Line is BadJson, because a crash cannot produce it"
  - "04-05: unterminated tail bytes over MAX_LEDGER_LINE_BYTES are refused as LineTooLong, because the store never writes a line that long, so they cannot be a torn write of its own"
  - "04-05: a record whose serialized line is over the cap is refused with a new StoreError::LineTooLong { bytes, max }, not Corrupt; the ledger is not corrupt"
  - "04-05: every store file is opened only after a symlink_metadata check and an inode match on the opened handle, closing the check-then-open window the plan left open"

patterns-established:
  - "Raw ledger corruption in tests goes through OpenOptions, and each refusal is asserted for read and for append, with the file bytes unchanged after the refused append"

requirements-completed: [STORE-01, STORE-02, STORE-03, STORE-04]

coverage:
  - id: D1
    description: "FileStore keeps one JSONL ledger per run and a sharded blob tree under a 0700 root; open refuses a symlinked or non-directory root"
    requirement: "STORE-02"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs::open_creates_a_missing_root_with_mode_0700, open_refuses_symlinked_root, open_refuses_a_root_that_is_a_file, reading_an_unknown_run_creates_nothing"
        status: pass
    human_judgment: false
  - id: D2
    description: "Appends are serialized by an exclusive File::lock per run, and the key set is rebuilt from the file under the lock"
    requirement: "STORE-02"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs::concurrent_appends_are_serialized"
        status: pass
    human_judgment: false
  - id: D3
    description: "A torn tail, with or without a newline, reads as a count and is truncated by the next append, Duplicate included"
    requirement: "STORE-02"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs::torn_tail_without_newline_is_tolerated, torn_tail_with_newline_is_tolerated, a_duplicate_append_still_truncates_the_torn_tail"
        status: pass
    human_judgment: false
  - id: D4
    description: "BadJson, KeyMismatch, SeqGap, UnknownVersion (mid-file and final), and LineTooLong refuse read and append at the exact 1-based line"
    requirement: "STORE-02"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs (11 refusal tests)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The first golden line is byte-identical to core's line_v1 plus a newline: 445 bytes, SHA-256 c595be4a...1909"
    requirement: "STORE-02"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs::first_line_matches_core_golden_bytes"
        status: pass
    human_judgment: false
  - id: D6
    description: "Appends only ever extend the file, and a same-key append leaves it byte-identical"
    requirement: "STORE-03"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_ledger.rs::append_only_prefix; tests/contract.rs duplicate_append_is_a_no_op::file, conflicting_append_is_not_written::file"
        status: pass
    human_judgment: false
  - id: D7
    description: "Blobs are written temp-then-rename in the shard, rehashed on every get, size-checked before opening, healed by put, and refused through a symlink"
    requirement: "STORE-04"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/file_blobs.rs (8 tests)"
        status: pass
    human_judgment: false
  - id: D8
    description: "The file store passes every contract case the memory store passes, and the two agree on random append sequences"
    requirement: "STORE-01"
    verification:
      - kind: test
        ref: "crates/minos-store/tests/contract.rs (file arms); tests/file_ledger.rs::memory_and_file_agree"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-10-15
status: complete
---

# Phase 4 Plan 05: File ledger and blob store Summary

**`FileStore`: a per-run JSONL ledger appended under an exclusive `File::lock`, made durable with `sync_data` and directory fsyncs, tolerant of a torn tail and strict about everything else, plus a sharded SHA-256 blob tree written temp-then-rename and rehashed on every read. It passes the same contract as `MemoryStore`.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-10-15T14:19:36Z
- **Completed:** 2026-10-15T14:25:32Z
- **Tasks:** 3 (Tasks 1 and 3 each as a RED and a GREEN commit; Task 2 is one test commit)
- **Files modified:** 7 (5 created, 2 modified)

## Accomplishments

- **Layout (D-10):** `runs/<run hex>/ledger.jsonl`, `runs/<run hex>/ledger.lock`, and `blobs/sha256/<2 hex>/<62 hex>` under a root created with mode 0700. `STORE_DIR = ".minos"`.
- **Append (D-11, D-12):** takes an exclusive lock on `ledger.lock`, reads the ledger under a 64 MiB cap, and parses and verifies every line. A torn tail is cut with `set_len` and synced. A same-key append returns `Duplicate` or `Conflict` and writes nothing. Otherwise the append writes one line with its newline and calls `sync_data`. It also fsyncs every directory that gained an entry (run directory, `runs/`, root).
- **Read:** takes a shared lock. A run that was never appended reads as empty and creates nothing. Read never truncates.
- **Parsing (D-13):** a line over 1 MiB is `LineTooLong`. A version probe that sees any `v` other than `"1"` is `UnknownVersion`, wherever the line is. A line that is not a `Line` is `BadJson`. `Line::verify` failing is `KeyMismatch`. A `seq` that does not match the line's position is `SeqGap`. Every error carries the run, the 1-based line, and the kind, never bytes.
- **Path safety:** every store directory and file is checked with `symlink_metadata`. Files are opened and then the handle's dev and inode are matched against that check. New files are created with `create_new` (O_EXCL). A symlinked root, run directory, ledger, shard, or blob is `Unsafe`.
- **Blobs (STORE-04):** a put writes a `NamedTempFile` in the shard, then calls `sync_all`, `persist`, and a shard fsync. A get checks the size from metadata before opening, reads with `take(max + 1)`, and rehashes. A put always writes, so it heals a tampered blob.
- **Tests:** every contract case and the first-write-wins property now run on both stores. `file_ledger.rs` has 24 tests: golden bytes, three torn-tail cases, 11 refusal cases, 6 path cases, the 4-thread concurrency test, and the `append_only_prefix` and `memory_and_file_agree` properties. `file_blobs.rs` has 8. A mutation check showed that removing `set_len` fails all three torn-tail tests.

## Task Commits

1. **Task 1: FileStore layout and the locked, durable ledger**: `7eeba9d` (test, RED), `c12a648` (feat, GREEN)
2. **Task 2: Crash, corruption, golden-byte, concurrency, and differential tests**: `6c74b6e` (test)
3. **Task 3: The atomic, verified blob tree**: `bf98478` (test, RED), `cc730b8` (feat, GREEN)

## Files Created/Modified

- `crates/minos-store/src/file/mod.rs`: `FileStore`, `STORE_DIR`, path helpers, `ensure_dir`, `dir_exists`, `fsync_dir`, `open_existing`, `open_or_create`
- `crates/minos-store/src/file/ledger.rs`: the caps, the version probe, `parse`, `read_capped`, `impl Ledger for FileStore`
- `crates/minos-store/src/file/blobs.rs`: `impl Blobs for FileStore`
- `crates/minos-store/src/lib.rs`: `pub mod file`, the `FileStore` and `STORE_DIR` re-exports, `StoreError::LineTooLong`, and crate docs
- `crates/minos-store/tests/contract.rs`: the file arms, a `FileStore` Send + Sync check, and `append_sequence_keeps_first_write_on_file`
- `crates/minos-store/tests/file_ledger.rs`, `crates/minos-store/tests/file_blobs.rs`: new

## Decisions Made

- **A torn tail ending in a newline must be invalid JSON.** If it is valid JSON but not a `Line`, it is `BadJson`. This is stricter than the plan's "fails to parse as a `Line`". A crash can leave partial or zeroed bytes but not a well-formed foreign object, and the plan's test (an incomplete JSON object) behaves the same either way.
- **Unterminated bytes over the 1 MiB line cap are refused as `LineTooLong`, not treated as torn.** The store never writes a line that long, so the bytes cannot be a torn write of its own.
- **A record whose line would exceed the cap is refused with `StoreError::LineTooLong { bytes, max }`.** `Corrupt` would wrongly blame the ledger. `StoreError` is `#[non_exhaustive]`, and the new variant is not retryable; `only_io_is_retryable` covers it.
- **The caps are `pub` and re-exported from `minos_store::file`,** so tests and a future `minos host doctor` use the same numbers.
- **File-backed proptests run 32 cases**, because each append ends in `F_FULLFSYNC`. The whole store test suite runs in about 5 seconds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Check-then-open window on store files**
- **Found during:** Task 1
- **Issue:** The plan checks a file with `symlink_metadata` and then opens it by path. A swap between the two steps would be followed. The lock file was to be opened with `create(true)`, which follows a symlink and creates its target (T-04-05-06).
- **Fix:** `open_existing` matches the opened handle's dev and inode against the check. `open_or_create` creates through `create_new` (O_EXCL, never follows a symlink) and, if it loses a creation race, falls back to the checked open. The run directory, `runs/`, and the blob directories are checked the same way, so a symlinked directory is `Unsafe`.
- **Files modified:** crates/minos-store/src/file/mod.rs, ledger.rs, blobs.rs
- **Commits:** c12a648, cc730b8

**2. [Rule 1 - Bug] Over-cap line on append would have been reported as corruption**
- **Found during:** Task 1
- **Issue:** The plan says to refuse a serialized line over the cap but names no error. No existing variant fit: `Corrupt` blames the ledger, and `Encode` needs a `serde_json::Error`.
- **Fix:** Added `StoreError::LineTooLong { bytes, max }`, with docs and a retryability test entry.
- **Files modified:** crates/minos-store/src/lib.rs, crates/minos-store/src/file/ledger.rs
- **Commit:** c12a648

### Minor

- `blob_shard` and `blob_path` build the two path parts with `chars().take(2)` and `chars().skip(2)`, not `split_at_checked(2)`. Both avoid indexing, and the `chars` form cannot fail, so it needs no error variant for an impossible case.
- Added tests the plan did not list: a duplicate append still truncates a torn tail, a valid-JSON non-line tail, an edited record, a ledger not starting at seq 0, an over-cap unterminated tail, a root that is a file, a symlinked run directory, and a symlinked shard directory.

**Total deviations:** 2 auto-fixed (1 security hardening, 1 error-type bug). **Impact:** stricter refusals; no contract change for callers.

## Issues Encountered

None. The known flaky `minos-backend-krun` supervise test did not fail.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 838 passed, 0 failed, 25 ignored
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`; `deny.toml`, `Cargo.toml`, and `Cargo.lock` are unchanged
- `cargo doc --workspace --no-deps`: exit 0
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no workspace member was added, so it is unchanged from 04-01
- C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing
- `grep -rn 'truncate(true)' crates/minos-store/src` prints nothing

## TDD Gate Compliance

Tasks 1 and 3 each have a `test(04-05)` commit that failed to compile against the missing `FileStore` or its `Blobs` impl, followed by a `feat(04-05)` commit. Task 2 only tests Task 1's code, so it went green when written. Its RED evidence is the mutation check: with `set_len` removed, the three torn-tail tests fail.

## Next Phase Readiness

Phase 7's loop and Phase 8's CLI can open `FileStore::open(&project.join(STORE_DIR))`. The whole-run `driver.lock` (LOOP-06) is a separate file on top of the per-append `ledger.lock`.

## Self-Check: PASSED

- FOUND: crates/minos-store/src/file/mod.rs, ledger.rs, blobs.rs; crates/minos-store/tests/file_ledger.rs, file_blobs.rs
- FOUND: commits 7eeba9d, c12a648, 6c74b6e, bf98478, cc730b8
