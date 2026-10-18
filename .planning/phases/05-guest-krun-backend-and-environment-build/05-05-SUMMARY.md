---
phase: 05-guest-krun-backend-and-environment-build
plan: 05
subsystem: minos-store
tags: [blobs, streaming-put, verified-path, d-09, d-14, krun-09, store-04]
status: complete
requires:
  - "05-01: minos-core::id::BlobHasher (incremental plain SHA-256 equal to BlobId::of)"
provides:
  - "Blobs::put_reader<R: Read>(&self, reader, max) on FileStore and MemoryStore, same BlobId as put"
  - "StoreError::PutTooLarge { max } (not retryable)"
  - "FileStore::verified_path(&BlobId) -> canonical absolute PathBuf after symlink and inode checks, no rehash"
  - "blobs/staging layout entry for streamed temporary files"
affects:
  - "minos env build (05-18 or wherever it lands): streams the packed root image into FileStore with put_reader"
  - "minos-backend-krun / minos-cli launch config: names the root image by FileStore::verified_path under a read_only raw disk; Seatbelt ROOT_IMG param takes that canonical path"
tech-stack:
  added: []
  patterns:
    - "Streamed puts stage outside the shard (blobs/staging) because the shard is known only after the last byte; rename into the shard, fsync both directories"
    - "take(max + 1) plus a running count refuses over-cap streams before the over-cap chunk is written"
    - "Path handed to an external reader is bound to the opened inode: checked path, opened handle, and canonical path must agree on device and inode"
key-files:
  created: []
  modified:
    - crates/minos-store/src/lib.rs
    - crates/minos-store/src/file/mod.rs
    - crates/minos-store/src/file/blobs.rs
    - crates/minos-store/src/memory.rs
    - crates/minos-store/tests/contract.rs
    - crates/minos-store/tests/file_blobs.rs
    - CLAUDE.md
decisions:
  - "verified_path re-checks the store root with symlink_metadata as well as the blob directories, since the path leaves the store for the VM launcher"
  - "KRUN-09 is not marked complete here: this plan supplies only the store half; env build and minos.lock land later and 05-20 marks KRUN-09 after the final gate"
metrics:
  duration: "about 6 minutes"
  completed: 2026-10-18
  tasks: 2
  files: 7
---

# Phase 5 Plan 05: Streaming blob put and verified blob path Summary

`Blobs::put_reader` streams a reader into either store under a byte cap, hashing with core's `BlobHasher` so the ID equals `put` of the same bytes, and `FileStore::verified_path` hands out a canonical, inode-checked path to a blob so a root image can boot read-only without a copy per job.

## What was built

**Task 1 (D-09).** `Blobs` gained `fn put_reader<R: std::io::Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError>` and `StoreError` gained `PutTooLarge { max }`.
- FileStore: makes sure `blobs/` and `blobs/staging` exist, fsyncing each parent that gained an entry. It creates a `NamedTempFile` in staging and copies `reader.take(max + 1)` through a 64 KiB buffer, updating a `BlobHasher` and a running count per chunk. Once the count passes `max` it returns `PutTooLarge` before writing that chunk. When the stream ends it finishes the hash, makes sure `sha256/` and the shard exist, syncs and renames into the shard, then fsyncs the shard and staging directories. If the reader fails it returns `Io`. In both failure cases the temporary file deletes itself on drop.
- The shared directory and persist code in `put` was moved into private helpers (`ensure_blobs_dir`, `ensure_shard`, `persist_blob`) that `put` and `put_reader` both call. `put`'s behaviour is unchanged.
- MemoryStore: reads `take(max + 1)` into a `Vec`, refuses anything over `max`, then stores it as `put` does.
- The `Blobs` trait doc now says a whole or streamed put names a blob the same way, and that the trait is used only as a bound, never as a trait object.

**Task 2 (D-14).** `FileStore::verified_path(&self, id) -> Result<PathBuf, StoreError>`:
- It re-checks the root, checks the blob directories with `symlink_metadata`, and opens the blob file through `open_existing`, which matches device and inode against the checked path.
- It then canonicalizes the path, and the canonical path must be a regular file with the opened handle's device and inode.
- It does not rehash: `env build` checked the hash when it wrote the blob (D-14).
- CLAUDE.md's minos-store section now describes `put_reader`, `blobs/staging`, `PutTooLarge`, and `verified_path`. The open question about a streaming put for large blobs is removed.

## Tests

- contract.rs: `put_reader_equals_put` (200,000 bytes, spanning several 64 KiB buffers), `put_reader_exact_max`, `put_reader_over_max`, and `put_reader_reader_error`. Each runs against both MemoryStore and FileStore, 8 tests in all.
- file_blobs.rs:
  - `put_reader` cases: the sharded path is written and staging is left empty; an over-cap put leaves no staging file; `put_reader` heals a tampered blob; a symlinked staging directory is refused.
  - `verified_path` cases: the path is canonical; a symlinked ancestor resolves to the real path; a missing blob gives `BlobMissing`; a symlinked file or a symlinked shard gives `Unsafe`. That is 5 tests.
- lib.rs unit tests: `PutTooLarge` was added to the not-retryable list, plus a test of its display text.

## Verification

All six gate commands pass:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (60 suites ok)
- `cargo deny check` (advisories, bans, licenses, and sources ok)
- `cargo doc --workspace --no-deps`
- the musl clippy for minos-guest

C-01 printed 0 MISSING lines and C-02 printed nothing. `cargo test -p minos-store put_reader` ran 12 tests and `--test file_blobs verified_path` ran 5.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Security] verified_path re-checks the store root**
- **Found during:** Task 2
- **Issue:** The plan's steps check the blob directories and the file, but not the root. `FileStore::open` checks the root only once, and `symlink_metadata` of `root/blobs` follows a root that was swapped for a symlink after open.
- **Fix:** `verified_path` runs `dir_exists(&self.root, "store root")` first. A missing root gives `BlobMissing`; a symlinked root gives `Unsafe`.
- **Files modified:** crates/minos-store/src/file/blobs.rs
- **Commit:** 87e4dce

**2. [Rule 2 - Correctness] A reader that breaks the `Read` contract**
- **Found during:** Task 1
- **Issue:** A reader that reports more bytes than the buffer holds would make the chunk slice fail.
- **Fix:** The chunk is taken with `buffer.get(..read)`, and an out-of-range count becomes `StoreError::Io` with `InvalidData`, so there is no indexing and no panic.
- **Files modified:** crates/minos-store/src/file/blobs.rs
- **Commit:** 0d137c0

Otherwise the plan was executed as written. KRUN-09 is listed in the plan's frontmatter but was intentionally left Pending (see decisions).

## TDD Gate Compliance

- Task 1: RED 016ccae (test), GREEN 0d137c0 (feat).
- Task 2: RED 97a4e22 (test), GREEN 87e4dce (feat).
- No refactor commits were needed.

## Threat Flags

None. The new surfaces are the staging directory and a path handed to the launcher. Both are covered by T-05-05-01 to T-05-05-04: symlink and inode checks, temp-then-rename with fsync, a cap with drop cleanup, and the accepted no-rehash.

## Known Stubs

None.

## Self-Check: PASSED

- crates/minos-store/src/file/blobs.rs contains `pub fn verified_path` and `BlobHasher`: FOUND
- crates/minos-store/src/lib.rs contains `fn put_reader<R: std::io::Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError>;`: FOUND
- Commits 016ccae, 0d137c0, 97a4e22, 87e4dce: FOUND
