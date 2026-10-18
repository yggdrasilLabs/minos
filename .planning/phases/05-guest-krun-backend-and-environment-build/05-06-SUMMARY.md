---
phase: 05-guest-krun-backend-and-environment-build
plan: 06
subsystem: minos-sandbox
tags: [sandbox, input-disk, capabilities, preflight, seatbelt, blob-source, d-01, d-02, d-03, d-06, d-13, d-14, d-15, krun-06, krun-07]
status: complete
requires:
  - "05-01: minos-core::limits machine bounds (MAX_VCPUS, MIN_MEMORY_MIB, MAX_MEMORY_MIB) and output caps"
  - "05-04: the guest wire contract (the guest job blob an input disk names)"
provides:
  - "minos_sandbox::spec re-exports the machine bounds from minos-core::limits; minos_sandbox::output re-exports the four output caps"
  - "InputDisk::candidate(job, base, diff) and InputDisk::judge(job, candidate_out), with job, base, diff, candidate_out, role, blobs accessors; InputDisk::new and InputDisk::blob removed"
  - "SpecError::InputRole { role } and SpecError::InputMismatch, checked before the duration"
  - "Capabilities::new(backend, arch, max, max_duration_ms, isolation, host_refusals) plus with_layers, with_kernel, backend, layers, kernel, backend_info(root_image) -> BackendInfo"
  - "Refusal::{SeatbeltMissing, SeatbeltNotEnforcing, VmmMissing, GuestMissing}; EntitlementMissing names both entitlement keys"
  - "preflight refuses every krun plan whose host layers lack Seatbelt; JudgeDataIsInput when judge data is any input part"
  - "BlobSource trait (get with a cap, root_image -> VerifiedPath), BlobSourceError, VerifiedPath, MAX_VERIFIED_PATH_BYTES"
  - "LaunchError::Blob(#[from] BlobSourceError)"
affects:
  - "05-22: Finished elapsed and console, Ended::ChunkProtocol, take_status; rewrites the same sandbox test fixtures, which now build role-shaped inputs and krun hosts with Seatbelt"
  - "05-07 / 05-11 / 05-15: KrunBackend probes once into Capabilities with layers and kernel, is constructed with a BlobSource, and writes the per-job input disk from InputDisk parts"
  - "05-14: minos host doctor prints the new Refusal messages, each ending in a fix"
  - "minos-cli: implements BlobSource over FileStore (verified_path -> VerifiedPath::new)"
tech-stack:
  added: []
  patterns:
    - "Role-shaped value with a private parts enum: one public type, two const constructors, Option accessors per role, and a spec-time check that the shape matches the type-level role"
    - "Capabilities as the carrier of host provenance: the backend's probed layers and kernel digest flow to BackendInfo through the two-method Backend trait"
    - "Pure path checks on as_encoded_bytes (absolute, no empty, '.', or '..' segment, no NUL, length cap) because Path::components silently normalizes '/./' and '//'"
key-files:
  created:
    - crates/minos-sandbox/src/blob_source.rs
    - crates/minos-sandbox/tests/blob_source.rs
  modified:
    - crates/minos-sandbox/src/lib.rs
    - crates/minos-sandbox/src/backend.rs
    - crates/minos-sandbox/src/spec.rs
    - crates/minos-sandbox/src/preflight.rs
    - crates/minos-sandbox/src/output.rs
    - crates/minos-sandbox/tests/spec.rs
    - crates/minos-sandbox/tests/preflight.rs
    - crates/minos-sandbox/tests/backend.rs
    - crates/minos-sandbox/tests/untrusted.rs
    - crates/minos-sandbox/tests/output_tar.rs
    - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
decisions:
  - "The Seatbelt check is an if on BackendKind::Krun rather than a match with a wildcard arm: clippy's collapsible_match and single_match fire on the match form, and the if keeps a later backend free to bring its own mandatory layers"
  - "VerifiedPath checks the path bytes rather than Path::components, because components drops '.' and empty segments mid-path, so '/a/./b' and '/a//b' would otherwise pass"
  - "KRUN-06 and KRUN-07 are not marked complete: this plan lays the sandbox seams; doctor (05-14) and the recorded layers on a real run (05-11, 05-15) finish them"
metrics:
  duration: "about 8 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 13
---

# Phase 5 Plan 06: Sandbox seams for the krun backend Summary

The sandbox now takes its machine bounds and output caps from core. The input disk is the IDs of its D-15 parts, shaped per role and checked against the job. Capabilities carry the confinement layers and kernel digest and build the run's `BackendInfo`. Preflight refuses every krun plan that lacks Seatbelt. A `BlobSource` trait lets a backend fetch plan bytes and a verified root image path, and the `Backend` trait still has two methods.

## What was built

**Task 1 (D-06, D-15).**
- `spec.rs` re-exports `MAX_VCPUS`, `MIN_MEMORY_MIB`, and `MAX_MEMORY_MIB` from `minos-core::limits`. The WR-02 comment now says the bounds are defined in core and restated only in `minos-vmm`'s config, pinned by the paired tests. `output.rs` re-exports the four output caps the same way.
- `InputDisk` holds a private `job: BlobId` and a private `Parts` enum. A candidate's parts are `{ base: TreeId, diff: Option<BlobId> }` and a judge's are `{ candidate_out: BlobId }`.
- `blobs()` returns every blob the input names as a `BTreeSet`, with the base tree included through `TreeId::to_blob_id`.
- `SandboxSpec::new` refuses an input shaped for the other role (`InputRole`), and a candidate input whose base or diff differs from the job's (`InputMismatch`). Both checks run before the duration check.
- Preflight's `JudgeDataIsInput` now fires when the judge data equals any part of the input.

**Task 2 (KRUN-06, KRUN-07, D-01, D-02, D-03).**
- `Capabilities::new` takes the `BackendKind` first.
- `with_layers` and `with_kernel` are `#[must_use]` builders. The accessors are `backend()`, `layers()`, and `kernel()`, and `backend_info(root_image)` builds the `BackendInfo` a run records.
- The type doc covers what each layer means:
  - `Rlimits` is listed only when both the helper and guest halves are applied.
  - `DedicatedUser` is never listed in v1, so it always shows as reduced hardening.
- Preflight refuses every krun plan whose layers lack Seatbelt (`SeatbeltMissing`). A missing dedicated user does not refuse a plan.
- New refusals: `SeatbeltMissing`, `SeatbeltNotEnforcing`, `VmmMissing`, and `GuestMissing`. `EntitlementMissing` now names `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation` and ends with "run minos host setup". Every message ends with "; " followed by a fix, and a test checks the exact fix text.

**Task 3 (D-13, D-14).**
- New module `blob_source.rs` defines the `BlobSource` trait: `get(&BlobId, max)` and `root_image(&ImageDigest) -> VerifiedPath`.
- `BlobSourceError` has six variants: `Missing`, `TooLarge`, `Tampered`, `RootImageMissing` (its message says "run minos env build"), `Unsafe`, and `Io`.
- `VerifiedPath::new` makes only pure checks. The path must be absolute and at most 1024 bytes (the launcher's `MAX_PATH_BYTES`). It may not contain a NUL, be the root alone, or have an empty, `.`, or `..` component.
- `LaunchError::Blob(#[from] BlobSourceError)` reads "a blob the plan names could not be read", and its source is the `BlobSourceError`.
- `lib.rs` re-exports the three new types and names `BlobSource` in step 4 of the crate flow doc. The `backend.rs` module doc records Open Question 7 as resolved by D-13.
- `deny.toml` is unchanged and `cargo deny check` passes.

## Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 37e24b9 | test(05-06): add failing tests for core bounds and the D-15 input disk parts |
| 1 | GREEN | 6f1f22f | feat(05-06): core bounds and the D-15 input disk as part IDs |
| 2 | RED | 8f1a164 | test(05-06): add failing tests for confinement layers and Seatbelt-mandatory preflight |
| 2 | GREEN | 8552753 | feat(05-06): confinement layers and kernel in Capabilities, Seatbelt-mandatory preflight |
| 3 | RED | 4200725 | test(05-06): add failing tests for BlobSource, VerifiedPath, and LaunchError::Blob |
| 3 | GREEN | 234fc38 | feat(05-06): BlobSource, VerifiedPath, and LaunchError::Blob (D-13, D-14) |

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --workspace --no-deps`, and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` all pass.
- `cargo test --workspace` with `TRYBUILD` unset passes. That includes the trybuild suite: every compile-fail case still fails with its committed stderr, and `judge_flow` still passes.
- `cargo deny check` passes. Its one warning (an unmatched `minos-backend-krun` wrapper for `minos-sandbox`) was there before this plan.
- C-01 printed no MISSING line, and C-02 printed nothing.
- `tests/blob_source.rs` runs 4 tests, `tests/preflight.rs` runs 9, and `tests/spec.rs` runs 16.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated the InputDisk construction in backend, untrusted, and output_tar tests during Task 1**
- **Found during:** Task 1
- **Issue:** Task 1 removed `InputDisk::new`. The other sandbox test files build their inputs with it, so `cargo clippy --all-targets` (Task 1's verify command) would not compile until Task 2.
- **Fix:** Each of those files got a role-shaped `input::<R>()` helper in Task 1's RED commit. Task 2 then changed only their `Capabilities::new` calls.
- **Files modified:** crates/minos-sandbox/tests/backend.rs, tests/untrusted.rs, tests/output_tar.rs
- **Commit:** 37e24b9

**2. [Rule 1 - Formatting gate] The re-export's item order differs from the acceptance grep**
- **Found during:** Task 1
- **Issue:** The acceptance grep expects `pub use minos_core::limits::{MAX_VCPUS, MIN_MEMORY_MIB, MAX_MEMORY_MIB};`. rustfmt (style edition 2024) sorts the list, and the `cargo fmt --all --check` gate requires the sorted form.
- **Fix:** The line is now `pub use minos_core::limits::{MAX_MEMORY_MIB, MAX_VCPUS, MIN_MEMORY_MIB};`. It re-exports the same three items; only the literal grep differs.
- **Commit:** 6f1f22f

**3. [Rule 1 - Lint] The Seatbelt check is an `if`, not a `match` with a wildcard arm**
- **Found during:** Task 2
- **Issue:** The plan asked for a match on the non-exhaustive `BackendKind` with a wildcard arm. Under `-D warnings`, clippy's `collapsible_match` and `single_match` reject that form.
- **Fix:** `if host.backend == BackendKind::Krun && !host.layers.contains(&ConfinementLayer::Seatbelt)`. It behaves the same: a non-krun backend is unaffected.
- **Commit:** 8552753

## Known Stubs

None. `Capabilities::new` starts with no layers and no kernel by design. A krun host with no layers clears no plan, so a backend that forgets `with_layers` fails closed.

## Threat Flags

None. Every surface this plan touched is in the plan's threat register: T-05-06-01 (Seatbelt-mandatory preflight), T-05-06-02 (role-shaped inputs and `JudgeDataIsInput` over every part), T-05-06-05 (layers in `Capabilities` and `BackendInfo`), and T-05-06-06 (`VerifiedPath`).

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/src/blob_source.rs
- FOUND: crates/minos-sandbox/tests/blob_source.rs
- FOUND: 37e24b9, 6f1f22f, 8f1a164, 8552753, 4200725, 234fc38
