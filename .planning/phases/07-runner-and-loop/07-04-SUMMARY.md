---
phase: 07-runner-and-loop
plan: 04
subsystem: minos-sandbox (Untrusted sink exits)
status: complete
tags: [sandbox, untrusted, judge-integrity, D-17, D-09, LOOP-01, LOOP-02]
requires:
  - phase: 07-02
    provides: "minos_core::limits::MAX_STAGE_LOG_BYTES = 262_144"
provides:
  - "minos_sandbox::BlobSink trait: type Error: std::error::Error + 'static; fn put(&self, &[u8]) -> Result<BlobId, Self::Error>"
  - "minos_sandbox::SinkError<E> { Repack, DigestMismatch, Store(E) } (#[non_exhaustive], Debug, thiserror)"
  - "Untrusted<GuestOutput<Candidate>>::store_judge_input<S: BlobSink>(self, &S) -> Result<BlobId, SinkError<S::Error>>"
  - "Untrusted<OutputFile<R>>::store_tail<S: BlobSink>(self, max: usize, &S) -> Result<BlobId, SinkError<S::Error>>"
  - "trybuild case tests/ui/fail/judge_store_judge_input.rs: judge output has no store_judge_input"
affects: [07-06, 07-07, 07-08, 07-09, 07-10, 07-11, 07-12, 07-17]
tech-stack:
  added: []
  patterns:
    - "Sink exit: hostile bytes leave Untrusted only into a content-addressed put, and the exit returns the ID only after checking it equals BlobId::of the bytes"
    - "Role-specific impl block (Untrusted<GuestOutput<Candidate>>) so the compiler, not a runtime check, refuses judge output"
    - "Canonical ustar repack with minos-source's hand-built header recipe, mode fixed at 0644"
key-files:
  created:
    - crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.rs
    - crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.stderr
  modified:
    - crates/minos-sandbox/src/untrusted.rs
    - crates/minos-sandbox/src/lib.rs
    - crates/minos-sandbox/tests/untrusted.rs
    - crates/minos-sandbox/tests/compile_fail.rs
    - CLAUDE.md
decisions:
  - "D-17 trait form: the user answered \"trait\" at the 07-04 checkpoint on 2026-10-22, so both exits take &S where S: BlobSink in place of a closure"
  - "BlobSink is not sealed: minos-sandbox does not link minos-store, so the runner's adapter over Blobs::put must implement it from outside; the doc comment says so"
  - "BlobSink::Error is bounded by std::error::Error + 'static so SinkError<E>'s Store(#[source] E) is a real error source"
  - "store_judge_input re-checks the repacked archive against MAX_CANDIDATE_OUT_BYTES before the put (fail closed), though the bound already holds by construction"
  - "No #[must_use] attribute on the two exits: they return Result, which is already #[must_use], and clippy's double_must_use refuses a second one under -D warnings"
metrics:
  duration: "about 6 minutes of execution after the checkpoint answer (2026-10-23T00:14:58Z to 00:20:15Z UTC)"
  completed: 2026-10-23
  tasks: 3
  files: 7
---

# Phase 7 Plan 04: Untrusted sink exits Summary

`Untrusted` gains exactly two capped sink exits, behind the `BlobSink` trait the user picked at the checkpoint. `store_judge_input` (candidate output only) re-packs the `out/` files as a canonical ustar archive. `store_tail` (any role) keeps the last `min(max, MAX_STAGE_LOG_BYTES)` raw bytes of a stage log. Each calls `put` once and returns only a `BlobId`, after checking it is the bytes' own SHA-256.

## Task 1 checkpoint answer (D-17)

- The orchestrator presented the plan's proposal verbatim on 2026-10-22 local time (2026-10-23 UTC). Nothing under `crates/` had changed at that point.
- The user first asked which form was more secure, the closure or the `BlobSink` trait. The orchestrator answered that neither is more secure against the guest and that the trait is slightly better for review.
- The user's answer, verbatim:

      trait

- This is the plan's "approved with BlobSink" option. `minos_sandbox::BlobSink`, with one method `put(&self, &[u8]) -> Result<BlobId, Self::Error>`, replaces the closure parameter of both exits. Everything else in the proposal is unchanged: the `SinkError<E>` variants, the `out/` selection, the canonical headers, the `MAX_STAGE_LOG_BYTES` clamp, the digest check, and the trybuild case.
- When this record was made: Task 1 was resolved before this executor started. A draft of this SUMMARY holding the answer was written after Task 2's code and tests existed in the working tree and before Task 2's implementation commit (`307c2fc`). It was not written before Task 2 began, so the plan's "recorded before Task 2 starts" criterion was met late. The answer itself was settled before any code was written.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | User approves the exit signatures (checkpoint) | resolved by the orchestrator, answer "trait" | none |
| 2 (RED) | Failing tests for the sink exits | `40157c2` | tests/untrusted.rs, tests/ui/fail/judge_store_judge_input.rs |
| 2 (GREEN) | Implement BlobSink, SinkError, store_judge_input, store_tail | `307c2fc` | src/untrusted.rs, src/lib.rs, tests/compile_fail.rs, tests/ui/fail/judge_store_judge_input.stderr |
| 3 | Record the approved exits in CLAUDE.md | `eca78d3` | CLAUDE.md |

## What was built

- **`BlobSink`** (`crates/minos-sandbox/src/untrusted.rs`, re-exported at the crate root) has one `put` method. Its doc comment says why it cannot be sealed: `minos-sandbox` does not link `minos-store`. It also names the runner's adapter over `Blobs::put` as the production implementor.
- **`SinkError<E>`** has three variants. `Repack` is a candidate fault, recorded as `Rejected(OutputInvalid)` and never retried. `DigestMismatch` is an infrastructure fault. `Store(#[source] E)` carries the store's own error. No variant holds a byte of output.
- **`store_judge_input`** sits in a new `impl Untrusted<GuestOutput<Candidate>>` block.
  - The filter `is_artifact` requires the `/` after `out`. So `outx/d` and a file named `out` are dropped, along with the status, the logs, `diffs/`, and both scorecard-shaped names.
  - Headers are built by hand: `new_ustar`, the path, the size, mode 0644, uid, gid, and mtime 0, typeflag Regular, then the checksum. User and group names stay empty.
  - `Builder::into_inner` appends the two zero end blocks.
  - The archive is checked against `MAX_CANDIDATE_OUT_BYTES` again before `put`.
- **`store_tail`** is in the existing `impl<R: Role> Untrusted<OutputFile<R>>` block. It picks the tail with `saturating_sub` and `split_at_checked`, with no indexing.
- **Shared `sink_put`** computes `BlobId::of(bytes)` before the put. It calls the sink once and refuses a different returned ID.
- **Tests** (11 new in `tests/untrusted.rs`):
  - The `out/`-only selection, path order, and per-entry header fields, read back with the tar crate. The archive must be exactly its entries plus two zero blocks.
  - The golden repack `BlobId` `2330b319...aa5f` (3072 bytes).
  - The empty archive: 1024 zero bytes, `BlobId` `5f70bf18...c6ef`.
  - A wrong digest and a store error for both exits, and exactly one put per call.
  - The tail cases: 100 bytes with max 10, a 5-byte log, `MAX_STAGE_LOG_BYTES + 7` bytes with `usize::MAX`, control and invalid UTF-8 bytes passed through unchanged, and judge-role output.
- **Golden values:** a Python script wrote each ustar header field by field to the tar crate's octal format and hashed the result. The Rust output matched on the first run.
- **trybuild:** `judge_store_judge_input.stderr` holds exactly `error[E0599]: no method named store_judge_input found for struct Untrusted<GuestOutput<minos_sandbox::Judge>>`, with the note that the method exists for `Candidate`. It was generated once with `TRYBUILD=overwrite`, reviewed, then re-run without the variable.

## Verification

- `cargo test -p minos-sandbox --test untrusted --test compile_fail`: 27 and 1 passed.
- Full gate:
  - `cargo fmt --all --check`: ok.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ok.
  - `cargo test --workspace`: 1453 passed, 0 failed.
  - `cargo deny check`: advisories, bans, licenses, and sources ok.
  - `cargo doc --workspace --no-deps`: no warnings.
  - Guest musl clippy: ok.
  - C-01: 11 of 11 crates ok.
  - C-02 grep: empty.
- LOOP-01 grep: `grep -rnE '\.(store_judge_input|store_tail)\(' crates/*/src --include='*.rs' | grep -vE '^crates/minos-sandbox/'` prints nothing.
- Acceptance greps:
  - One `pub fn store_judge_input`, one `pub fn store_tail`, and one `impl Untrusted<GuestOutput<Candidate>>`.
  - `MAX_STAGE_LOG_BYTES` appears in `untrusted.rs`.
  - CLAUDE.md has 3 `store_judge_input` lines, one dated `07-04 checkpoint` line, and `D-17` lines.

## Deviations from Plan

1. **[Rule 1 - lint conflict] No `#[must_use]` on the exits.** The plan asked for `#[must_use]` "where a result is returned". Both exits return `Result`, which is already `#[must_use]`, and clippy's `double_must_use` fails `-D warnings` on a second attribute. Callers still cannot drop the result silently. This matches the existing `into_scorecard` and `into_diff_texts`.
2. **[Rule 2 - doc accuracy] `tests/compile_fail.rs` module docs** now list the new invariant beside the existing ones. That file was not in the plan's `files_modified`.
3. **[Rule 2 - fail closed] Re-check against `MAX_CANDIDATE_OUT_BYTES`.** `store_judge_input` checks the archive size again before the put. The plan said the bound holds by construction, and it does; the check costs one comparison.
4. **TDD order.** The implementation was drafted before the tests. To keep the RED and GREEN gates honest, the draft was copied to the scratchpad and the two source files were restored from git with `git checkout -- <file>`. The tests were then confirmed failing (unresolved `BlobSink` and `SinkError` imports) and committed as `40157c2`. The draft was restored and committed as `307c2fc`.

## TDD Gate Compliance

RED `test(07-04)` `40157c2` precedes GREEN `feat(07-04)` `307c2fc`. No refactor commit was needed.

## Known Stubs

None.

## Threat Flags

None. The two exits are the surface the plan's threat register covers (T-07-04-01 to T-07-04-06), and each mitigation is in place:

- The checkpoint approved the signatures.
- Role-specific impl blocks and the trybuild proof keep `store_judge_input` off judge output.
- The archive holds only `out/` files.
- Headers are canonical, and the golden pin guards them.
- Both size caps apply.
- `LogExcerpt` is the documented only reader of a stored tail.
- The digest is checked on every put.

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.rs
- FOUND: crates/minos-sandbox/tests/ui/fail/judge_store_judge_input.stderr
- FOUND: commits 40157c2, 307c2fc, eca78d3
