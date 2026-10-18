---
phase: 05-guest-krun-backend-and-environment-build
plan: 04
subsystem: minos-core
tags: [guest-job, guest-status, input-disk, token, golden, d-05, d-06, d-07, d-15, guest-02, guest-03, guest-04]
status: complete
requires:
  - "05-01: limits memory budget (GUEST_RESERVE_MIB, candidate_allowance_mib), output caps, Environment memory_mib/scratch_mib"
  - "05-02: ChunkToken (from_bytes, crate-private as_bytes, TOKEN_BYTES)"
provides:
  - "minos-core::guest constants: GUEST_JOB_VERSION, GUEST_STATUS_VERSION, MAX_GUEST_JOB_BYTES, MAX_STATUS_BYTES, OUTPUT_VSOCK_PORT, STATUS_PATH, DISK_MAGIC, DISK_VERSION, DISK_HEADER_BYTES, SECTION_ENTRY_BYTES, SECTOR_BYTES, MAX_BASE_TAR_BYTES, MAX_JUDGE_DATA_BYTES, MAX_CANDIDATE_OUT_BYTES, MAX_GUEST_INPUT_BYTES"
  - "GuestRole (stages), StageSpec, RawStageSpec, GuestJob (candidate, judge, parse_json, to_json, allowance_mib, getters incl. judge_version), RawGuestJob, GuestJobError"
  - "SetupStatus, SetupStep, ApplyStatus, StageEnd, OutputStatus, StageReport, GuestStatus (new, parse_json, to_json, getters), RawGuestStatus, GuestStatusError"
  - "SectionKind (cap, code, from_u32), write_input_disk, read_input_disk, InputSections (token, take, contains, check_role), DiskError"
  - "Crate-root re-exports GuestJob, GuestStatus"
affects:
  - "minos-guest (05-08, 05-10): JobSource reads the disk with read_input_disk, parses the JOB section with GuestJob::parse_json, calls check_role before taking sections, writes .minos/status.json with GuestStatus"
  - "minos-backend-krun: writes the disk with write_input_disk from blob bytes and replaces the v1 MINOSJB1 writer"
  - "minos-sandbox (05-06): InputDisk becomes the IDs of the disk's parts"
  - "Phase 7: maps SetupStatus::Failed to infrastructure failures and StageEnd to Outcomes; minos judge push refuses judge data over 96 MiB"
tech-stack:
  added: []
  patterns:
    - "One codec in minos-core for each host-guest wire form, linked by both writer and reader"
    - "Canonical binary layout: the reader recomputes every offset and refuses any other form"
    - "Every table entry validated before any section byte is read"
    - "Errors on hostile input carry numbers only (GuestStatusError::Json keeps line and column, not serde text)"
    - "Golden binary pin computed independently from the layout spec in Python, not from the code's own output"
key-files:
  created:
    - crates/minos-core/src/guest.rs
    - crates/minos-core/tests/guest_wire.rs
    - crates/minos-core/tests/snapshots/guest_wire__guest_job_candidate.snap
    - crates/minos-core/tests/snapshots/guest_wire__guest_job_judge.snap
    - crates/minos-core/tests/snapshots/guest_wire__guest_status_candidate.snap
    - crates/minos-core/tests/snapshots/guest_wire__guest_status_apply_failed.snap
    - crates/minos-core/tests/snapshots/guest_wire__guest_status_judge.snap
  modified:
    - crates/minos-core/src/lib.rs
    - crates/minos-source/src/walk.rs
decisions:
  - "GuestJob's judge-version getter is judge_version(), because the plan's judge(manifest, request) constructor takes the name judge"
  - "The input disk has one canonical layout: ascending kinds, each section on the first 512-byte boundary after the table or the previous section, zero padding. The reader refuses gaps, overlaps, misalignment, and non-zero padding as DiskError::Layout"
  - "GuestStatus refuses a candidate status with apply not_applicable as well as a judge status without it, and a packed output over MAX_OUTPUT_ENTRIES or MAX_OUTPUT_CONTENT_BYTES"
  - "GuestJob::to_json refuses JSON over MAX_GUEST_JOB_BYTES, so the writer never emits a job the reader refuses"
  - "check_role binds JUDGE_DATA and CANDIDATE_OUT by their table digests only, because the job names no ID for them"
metrics:
  duration: "about 12 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 10
---

# Phase 5 Plan 04: Guest wire contract Summary

`minos-core::guest` now holds the three wire forms between the backend and PID 1, and both sides link the same code for each:

- the guest job, as v1 JSON built from a validated manifest and request;
- the guest status at `.minos/status.json`, made only of enums, numbers, and `RelPath`s;
- the input disk v2: a `MINOSJB2` header carrying the 32-byte per-launch token, a table with a length and a SHA-256 for each section, and 512-byte-aligned sections.

Const assertions keep each role's in-memory input within half the guest reserve (128 MiB).

## What was built

**Task 1: GuestJob and GuestStatus (commits 968d8df RED, e2265fe GREEN)**
- `GuestJob::candidate` and `GuestJob::judge` cannot fail, because every value they take is already validated.
- `RawGuestJob`/`RawStageSpec` (`deny_unknown_fields`) run through `TryFrom`, which checks:
  - the version and the trial;
  - that the role's exact stage list is present (build then run, or judge);
  - for a candidate: a base, 1 to 64 writable paths, and a diff exactly when the experiment is not `ExperimentId::baseline(base)`;
  - for a judge: no base, no diff, and no writable set;
  - each argv against the manifest command rule;
  - each stage budget against the stage bounds;
  - memory and scratch against their ranges and `candidate_allowance_mib`.
- `parse_json` and `to_json` both enforce the 1 MiB cap. `allowance_mib` is computed once when the job is built.
- The status enums use `rename_all = "snake_case", deny_unknown_fields`. A test shows an unknown field inside a variant is refused.
- `GuestStatus::new` and the parser run the same checks: the stages are ordered, unique, and belong to the role; signals are 1 to 64; apply is `not_applicable` exactly for a judge; output stays within the output caps.
- `GuestStatusError` carries no input text. A test plants a hostile field name and confirms it does not appear in the message.
- Five golden snapshots, pinned by SHA-256 (`cfae3d32...922b3a4a`, computed with awk and shasum). They reuse the `golden_wire` fixture IDs.

**Task 2: input disk v2 (commits bf5fed0 RED, bf9aa28 GREEN)**
- `write_input_disk` sorts sections by kind and refuses an empty list, a duplicate kind, an over-cap section, or a missing JOB section, all before it writes a byte. It returns the total length, a whole number of sectors.
- `read_input_disk` reads the header, then the whole table, and checks every entry before it reads any section byte:
  - the kind is known and appears once;
  - kinds are in ascending order;
  - the length is within that kind's cap;
  - the offset is the canonical one.
- Next, it checks that the padding is zero and reads each section with an exact `try_reserve_exact`. It compares each section's SHA-256 with the table and stops at the end of the last section.
- `InputSections` has a manual `Debug` that shows kinds and lengths only; the token stays redacted.
- `check_role` covers the candidate, baseline, and judge rules: `BaseMismatch` from `TreeId::of`, `DiffMismatch` from `BlobId::of`, and `MissingSection` or `UnexpectedSection` by kind.
- The disk golden hash (`f59100e4...0ef83c`) was computed by a Python script that builds the disk from the layout spec, independently of the Rust writer.
- `minos-source`: `snapshot_tarball_fits_guest_base_cap` checks that 64 MiB + 10,002 KiB is at most 80 MiB.

**Task 3: guest reserve bound (commits 57d0bb1 RED, e53d4cb GREEN)**
- `MAX_GUEST_INPUT_BYTES = 134_217_728`. A const assertion pins it to `GUEST_RESERVE_MIB * 1 MiB / 2`.
- Const assertions also bound each role's caps:
  - candidate: job + base + diff = 82 MiB, at most 128 MiB;
  - judge: job + judge data + candidate output, about 114 MiB, at most 128 MiB.
- Raising `MAX_JUDGE_DATA_BYTES` to 120 MiB as a temporary edit made the build fail with "the judge role's input sections fit half the guest reserve". The edit was reverted.
- The `MAX_JUDGE_DATA_BYTES` doc records three things: judge push refuses larger data, 05-13 records the streaming-unpack open question, and the cap is checked from the table.

## Verification

- `cargo fmt --all --check`: passes
- `cargo clippy --workspace --all-targets -- -D warnings`: passes
- `env -u INSTA_UPDATE cargo test --workspace`: 60 suites, all ok, no failures. `guest_wire` has 41 tests.
- `cargo deny check`: advisories, bans, licenses, and sources all ok. The existing duplicate and unused-wrapper warnings are unrelated, and this plan changed no dependencies.
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: passes
- C-01: no MISSING lines. C-02: the grep prints nothing.
- Acceptance greps: every listed signature and constant prints one line, and there are 5 `guest_wire__` snapshots.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The getter `judge()` clashed with the constructor `judge(manifest, request)`**
- **Found during:** Task 1
- **Fix:** The judge-version getter is `judge_version()`. The constructor keeps the signature the plan requires.
- **Commit:** e2265fe

**2. [Rule 2 - Correctness] Extra GuestJob checks: trial cap, writable set per role, and the output cap in `to_json`**
- Added the `GuestJobError::Trial` and `GuestJobError::Writable` variants.
- `to_json` refuses JSON over 1 MiB. Argv entries made of characters that JSON escapes could otherwise push a job past what the reader accepts.
- **Commit:** e2265fe

**3. [Rule 2 - Correctness] Extra GuestStatus checks**
- A candidate status with apply `not_applicable` is refused.
- A packed output over the output caps is refused, with the new `GuestStatusError::Output`.
- **Commit:** e2265fe

**4. [Rule 2 - Correctness] The disk has one canonical form**
- Gaps between sections and non-zero padding are refused as `Layout`, as well as misaligned, overlapping, or out-of-order entries.
- `SectionKind::code` was added beside `from_u32`.
- **Commit:** bf9aa28

**5. [Test coverage] Extra disk tests**
- `disk_reads_exactly_to_the_last_section`, `disk_sections_debug_shows_no_bytes`, `disk_writer_refusals`, and `disk_constants`.
- **Commit:** bf5fed0

## Known Stubs

None.

## Requirements

GUEST-02, GUEST-03, and GUEST-04 are not marked complete. This plan supplies their core wire contract. The guest-side reading and applying (05-08, 05-10) and the end-to-end proof (05-17) finish them.

## TDD Gate Compliance

Each task has a `test(...)` RED commit followed by a `feat(...)` GREEN commit: 968d8df then e2265fe, bf5fed0 then bf9aa28, and 57d0bb1 then e53d4cb. Each RED run failed to compile because the API did not exist yet. The two RED pins were placeholders, replaced by hashes computed outside the code.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/guest.rs, crates/minos-core/tests/guest_wire.rs, the five guest_wire__*.snap files, crates/minos-source/src/walk.rs
- FOUND commits: 968d8df, e2265fe, bf5fed0, bf9aa28, 57d0bb1, e53d4cb
