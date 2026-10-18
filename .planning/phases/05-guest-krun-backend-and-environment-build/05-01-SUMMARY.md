---
phase: 05-guest-krun-backend-and-environment-build
plan: 01
subsystem: minos-core
tags: [limits, manifest, memory-budget, blob-hasher, d-05, d-06, d-09]
status: complete
requires: []
provides:
  - minos-core::limits VM machine bounds (MAX_VCPUS, MIN_MEMORY_MIB, MAX_MEMORY_MIB)
  - minos-core::limits memory budget (GUEST_RESERVE_MIB, MIN_CANDIDATE_ALLOWANCE_MIB, scratch bounds, defaults, candidate_allowance_mib)
  - minos-core::limits shared output caps (MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRY_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_STREAM_BYTES)
  - "[environment] memory_mib / scratch_mib with Environment accessors and ManifestError::MemoryBudget"
  - BlobHasher, ImageDigest::from_blob_id, ImageDigest::to_blob_id
affects:
  - minos-sandbox (05-06 switches spec.rs and output.rs to the core constants)
  - minos-guest (05-08 sets RLIMIT_AS to candidate_allowance_mib)
  - minos-store (streaming put names blobs with BlobHasher)
tech-stack:
  added: []
  patterns:
    - "Cross-crate bounds live in minos-core::limits, the one crate every crate may link"
    - "Static leaf-limit text pinned to the constants by a test so the two cannot drift"
key-files:
  created:
    - crates/minos-core/tests/blob_hasher.rs
  modified:
    - crates/minos-core/src/limits.rs
    - crates/minos-core/src/manifest.rs
    - crates/minos-core/src/id.rs
    - crates/minos-core/tests/manifest.rs
    - CLAUDE.md
decisions:
  - "VM machine bounds and the D-05 memory budget live in minos-core::limits; minos-sandbox will use them (05-06), because core depends on no internal crate"
  - "Environment stores the candidate allowance computed once at parse time; callers read it, never recompute it"
  - "Defaults memory_mib 2048 and scratch_mib 1024 are a recorded assumption (05-RESEARCH A6) for the user to confirm; Phase 8's sample project owns measuring RLIMIT_AS at the default 768 MiB allowance"
metrics:
  duration: "about 7 minutes"
  completed: 2026-10-18
  tasks: 2
  files: 6
---

# Phase 5 Plan 01: Core machine bounds, memory budget, and BlobHasher Summary

The VM machine bounds, the D-05 memory budget (256 MiB guest reserve + tmpfs scratch cap + at least 256 MiB candidate allowance), and the guest output caps now live in `minos-core::limits`. `Manifest::try_from` parses `[environment] memory_mib` (default 2048) and `scratch_mib` (default 1024), checks each against its range, and refuses an over-budget manifest with `ManifestError::MemoryBudget`. A streaming `BlobHasher` names a blob exactly as `BlobId::of` does for the whole slice.

## What was built

**Task 1: limits and manifest (commits 4d80241 RED, 47f7656 GREEN)**
- `limits.rs`: `MAX_VCPUS` 8, `MIN_MEMORY_MIB` 128, `MAX_MEMORY_MIB` 16384, `GUEST_RESERVE_MIB` 256, `MIN_CANDIDATE_ALLOWANCE_MIB` 256, `MIN_SCRATCH_MIB` 64, `MAX_SCRATCH_MIB` 15872 (derived), `DEFAULT_MEMORY_MIB` 2048, `DEFAULT_SCRATCH_MIB` 1024. `const fn candidate_allowance_mib` uses checked subtraction and returns `None` below the minimum allowance. Const assertions check that the defaults fit the budget and that the scratch bounds are ordered. The four output caps are copied from `minos-sandbox/src/output.rs` with their values and docs unchanged. The module header now says that bounds shared across crates live here.
- `manifest.rs`: `RawEnvironment` gains `memory_mib` and `scratch_mib` as `Option<u16>` and still uses `deny_unknown_fields`. `Environment` gains private `memory_mib`, `scratch_mib`, and `candidate_allowance_mib` fields, each with a `const fn` accessor. A new `memory_from` step runs in leaf validation: it applies the defaults, checks the ranges through `bounded` (field paths `environment.memory_mib` and `environment.scratch_mib`), and then applies the budget rule. The `MemoryBudget` message prints `GUEST_RESERVE_MIB` and `MIN_CANDIDATE_ALLOWANCE_MIB` from the constants. The doc comment on `Environment` records D-06 and D-08: the guest ignores the Dockerfile `USER`.
- Tests: seven `manifest_environment_memory_*` tests and `limits_budget_holds_for_defaults`.
- `CLAUDE.md`: the new keys and their bounds under "Optional keys and their defaults", the budget rule among the Manifest cross-field checks, and an Open-questions bullet on `RLIMIT_AS` at the default allowance (owner: Phase 8's sample project).

**Task 2: BlobHasher (commits c322637 RED, 6919946 GREEN)**
- `id.rs`: `pub struct BlobHasher(Sha256)`, deriving only `Default`, with `update(&mut self, &[u8])` and `finish(self) -> BlobId`. Also `ImageDigest::from_blob_id` and `ImageDigest::to_blob_id`, both `const fn`, matching `TreeId::to_blob_id`.
- `tests/blob_hasher.rs`: a proptest on split invariance (up to 64 KiB, up to 16 arbitrary cut points), the empty-input case, the FIPS 180-2 `abc` vector fed in three pieces, and the `ImageDigest` round trip.
- `CLAUDE.md`: the streaming-put open question now says the core half exists and the store's streaming put is still to come.

## Verification

- `cargo fmt --all --check`: passes
- `cargo clippy --workspace --all-targets -- -D warnings`: passes
- `cargo test --workspace`: all suites pass (57 `test result: ok` lines, no failures)
- `cargo deny check`: advisories, bans, licenses, and sources all ok. The existing "unmatched wrapper" warning is unrelated to this plan, which changed no dependencies.
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: passes
- C-01: no MISSING lines. C-02: the grep prints nothing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] A test ties the static leaf-limit text to the constants**
- **Found during:** Task 1
- **Issue:** The `InvalidField` limit text is `&'static str` (`"128 to 16384 MiB"`, `"64 to 15872 MiB"`), following the existing pattern, so it could drift from the constants without anyone noticing.
- **Fix:** Added `manifest_environment_memory_limits_name_the_bounds`, which builds the expected text from the constants with `format!`.
- **Files modified:** crates/minos-core/tests/manifest.rs
- **Commit:** 47f7656

**2. [Rule 2 - Coverage] Two extra tests: a value outside u16 and the scratch-over-memory underflow**
- `manifest_environment_memory_out_of_u16_is_a_type_error` (70000 is a toml error), and `candidate_allowance_mib(128, 15872)` returns `None` without wrapping.
- **Commit:** 4d80241

**3. [Doc accuracy] The `limits.rs` module header said "Later plans do not edit this file"**
- That contradicted this plan, so the header now says single-module caps stay in their own module and bounds shared across crates live in `limits.rs`.
- **Commit:** 47f7656

## Known Stubs

None.

## TDD Gate Compliance

Each task has a `test(...)` RED commit followed by a `feat(...)` GREEN commit: 4d80241 then 47f7656, and c322637 then 6919946. In both RED runs the tests failed to compile because the API did not exist yet.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/limits.rs, crates/minos-core/src/manifest.rs, crates/minos-core/src/id.rs, crates/minos-core/tests/manifest.rs, crates/minos-core/tests/blob_hasher.rs, CLAUDE.md
- FOUND commits: 4d80241, 47f7656, c322637, 6919946
