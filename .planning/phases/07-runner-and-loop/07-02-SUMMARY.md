---
phase: 07-runner-and-loop
plan: 02
subsystem: minos-core (limits, manifest, lock, money)
status: complete
tags: [manifest, lock, wire-format, golden, money, caps]
requires: []
provides:
  - "limits::{TRIALS_MIN, TRIALS_DEFAULT, MAX_JUDGE_FILES, MAX_STAGE_LOG_BYTES}"
  - "RawJudge.trials and JudgeSpec::trials() (default 3, range 1 to 32, not part of JudgeVersion)"
  - "lock::{JudgeFile, RawJudgeFile, LockedJudge, RawLockedJudge, JUDGE_FILE_PATH_MAX_BYTES, JUDGE_FILE_ENTRY_MAX_BYTES}"
  - "Lock::{judge, with_judge, check_judge, judge_file_digests}; RawLock.judge"
  - "LockError::{DuplicateJudge, JudgeFilesNotCanonical, JudgeFileCount, JudgeFilePath}; LockMismatch::NoJudge"
  - "MAX_LOCK_BYTES = 4_194_304, const-asserted against MAX_JUDGE_FILES entries"
  - "Prices::missing_models(&Manifest)"
  - "golden snapshot lock__minos_lock_v1_judge.snap with its own SHA-256 pin"
affects: [07-04, 07-08, 07-10, 07-11, 07-13, 07-15]
tech-stack:
  added: []
  patterns:
    - "Optional TOML array of tables under v = 1 with #[serde(default, skip_serializing_if = \"Vec::is_empty\")], so older locks keep their bytes"
    - "A second lock golden with its own pin, so the existing pin never moves"
key-files:
  created:
    - crates/minos-core/tests/snapshots/lock__minos_lock_v1_judge.snap
  modified:
    - crates/minos-core/src/limits.rs
    - crates/minos-core/src/manifest.rs
    - crates/minos-core/src/money.rs
    - crates/minos-core/src/lock.rs
    - crates/minos-core/tests/manifest.rs
    - crates/minos-core/tests/money.rs
    - crates/minos-core/tests/lock.rs
decisions:
  - "A [[judge.file]] path over 255 bytes (the ustar limit) is refused with LockError::JudgeFilePath, so the const assertion's worst-case entry size (JUDGE_FILE_ENTRY_MAX_BYTES = 400) is a true bound"
  - "MAX_LOCK_BYTES fits one label at MAX_JUDGE_FILES worst-case entries; every label's file list shares that cap"
  - "An empty judge file list is allowed in the lock (wire form `file = []`); whether an empty judge directory may be pushed is the judge pack's decision (07-08)"
  - "The judge lock golden pin is bc21f64fca6dc7cc085c5ae74e5d620f0a55d20cdca5eb4335ca79d5ec103b28, computed with awk and shasum over the snapshot body; the v1 lock pin a495cdba... is unchanged"
metrics:
  duration: "about 10 minutes"
  completed: 2026-10-22
  tasks: 2
  files: 8
---

# Phase 7 Plan 02: Judge trials, the judge lock table, price completeness, and shared caps Summary

`[judge] trials` is now an optional manifest key: it defaults to 3, must lie within 1 to 32, and is left out of `JudgeVersion`. `minos.lock` gains one `[[judge]]` table per label, each with a canonical per-file digest list. Four lookups read it: `judge`, `with_judge`, `check_judge`, and `judge_file_digests`. `Prices::missing_models` names every model a proposer uses that `prices.toml` has no price for. Core now owns the shared stage-log tail cap. The existing lock golden, its pin, and the `JudgeVersion` golden vector are unchanged, byte for byte.

## What was built

**Task 1 (caps, trials, prices).**
- `limits.rs` gains `TRIALS_MIN = 1` and `TRIALS_DEFAULT = 3`, with a const assertion that they are ordered under `MAX_TRIALS`. It also gains `MAX_JUDGE_FILES = 10_000` and `MAX_STAGE_LOG_BYTES = 262_144`, which equals the guest's `LOG_CAP_BYTES`. Each constant documents its unit and reason.
- `RawJudge` gains `trials: Option<u32>`, and `JudgeSpec` gains a private `trials` with the `pub const fn trials()` accessor.
- `Manifest::try_from` reads trials through the same `bounded` helper the budget bounds use. An out-of-range value is `InvalidField` naming `judge.trials`.
- `JudgeVersion::of` is untouched; its doc now says that `trials` is excluded.
- `Prices::missing_models(&Manifest) -> BTreeSet<ModelId>` collects every `ProposerSpec::model()` the table does not price. Its doc says that compute rates are already required by `parse_toml`, and that the loop checks resolved IDs again after each call (Pitfall 7).

**Task 2 (judge lock table).**
- `JudgeFile` holds a path and a SHA-256, ordered by path first.
- `LockedJudge` holds the label, `tree: TreeId`, `files`, `bytes`, `minos: BuilderText`, and `file: Vec<JudgeFile>`. Both have raw forms with `deny_unknown_fields`, and `LockedJudge` deserializes through `try_from`.
- `LockedJudge::new` takes a `BTreeMap<RelPath, BlobId>`, so the count and the order come from the map.
- `TryFrom<RawLockedJudge>` refuses the following:
  - a list that is not strictly ascending by path
  - a count that differs from the list's length
  - a list over `MAX_JUDGE_FILES`
  - a path over 255 bytes
- `RawLock.judge` is omitted when empty, so a lock written before this plan parses with no judge entries and writes the same bytes. A duplicate label is `DuplicateJudge`.
- `check_judge(label)` returns `LockMismatch::NoJudge { label }`, which tells the user to run `minos judge push`.
- `judge_file_digests()` is the union over all labels.
- `MAX_LOCK_BYTES` rises to 4 MiB. The const assertion is `MAX_JUDGE_FILES * JUDGE_FILE_ENTRY_MAX_BYTES as u64 <= MAX_LOCK_BYTES as u64`, which casts usize to u64, so it trips no truncation lint.
- The module docs state that the lock holds paths and digests only and exposes judge file paths. They also say that a low-entropy file's digest is guessable, and that a fresh clone must push the data again.

## Verification

Tests follow the TDD order. For each task, the RED state was confirmed by temporarily restoring the source files: the new tests failed to compile against the old API. The test commit came before the feat commit.

- `cargo test -p minos-core --test manifest --test money --test golden_ids`: 108, 48, and 17 passed.
- `cargo test -p minos-core --test lock`: 30 passed. This includes `lock_fits_one_label_at_the_judge_file_cap`, which serializes 10,000 entries with 255-byte paths beside the golden image, asserts the text fits `MAX_LOCK_BYTES`, and parses it back.
- The new golden hex values were checked with `shasum`: `judge data 1`, `hello`, and `#!/bin/sh\n`. The pin was computed with `awk ... | shasum -a 256` over the accepted snapshot body.
- `git diff --exit-code` against the pre-plan commit for `lock__minos_lock_v1.snap` and `tests/golden_ids.rs` reported no change.

The full gate passed:
- `cargo fmt --all --check` ok
- `cargo clippy --workspace --all-targets -- -D warnings` ok
- `cargo test --workspace`: 1428 passed, 0 failed
- `cargo deny check`: advisories, bans, licenses, and sources ok
- `cargo doc --workspace --no-deps`: 0 warnings
- the musl `minos-guest` clippy ok
- C-01: every crate prints `ok`
- C-02: the grep printed nothing

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] Added `LockError::JudgeFilePath` and `JUDGE_FILE_PATH_MAX_BYTES = 255`**
- **Found during:** Task 2
- **Issue:** `RelPath` allows paths up to 1024 bytes, so the const assertion's 400-byte worst-case entry, which the plan sizes on a 255-byte ustar path, would not have been a true bound for a hand-edited lock.
- **Fix:** `LockedJudge::new` and `TryFrom<RawLockedJudge>` refuse any path over 255 bytes, the limit of the packer that produces the tarball. A test exercises both paths.
- **Files modified:** crates/minos-core/src/lock.rs, crates/minos-core/tests/lock.rs
- **Commit:** d685980

**2. [Minor] `LockedJudge` derives its stored `files` count from the list**
- The plan lists `files: u64` as a field. It is kept, but it is always set from the list length, both in `new` and after the equality check in `try_from`, so the count and the list can never disagree.

## Known Stubs

None.

## Notes for later plans

- Every label's per-file list shares `MAX_LOCK_BYTES`. A project that keeps many large labels pushed could outgrow 4 MiB, and the lock is then refused as `TooLarge`. One label at the cap fits with about 190 KiB to spare.
- The CLAUDE.md wording for the judge table (the D-02 rewording) is owned by 07-11, as the threat model says. This plan changed only the module docs.

## Self-Check: PASSED

- FOUND: crates/minos-core/tests/snapshots/lock__minos_lock_v1_judge.snap
- FOUND: edd94c1, 62d8a23, aa59e50, d685980
