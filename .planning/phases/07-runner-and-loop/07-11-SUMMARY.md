---
phase: 07-runner-and-loop
plan: 11
subsystem: minos-cli (judge push, shared lock helpers)
status: complete
tags: [cli, judge, lock, JUDGE-01, CLI-04, D-01, D-02, D-03, D-04, D-05, D-20]
requires:
  - phase: 07-02
    provides: "LockedJudge::new, Lock::{judge, with_judge, judge_file_digests}, MAX_LOCK_BYTES, JudgeSpec::{label, paths, trials}"
  - phase: 07-08
    provides: "minos_source::pack_judge_data -> JudgePack {tarball, tree, files, report}, SnapshotError::{JudgeOverlap, JudgeTooLarge}"
provides:
  - "minos_cli::lock_file: read_lock, write_lock, LockFileError { Read, Lock }"
  - "minos_cli::judge_push: judge_push, JudgePushOptions::new, JudgePushReport (label, tree, file_count, bytes, judge_version, lock_written, denied), JudgePushError (Read, TooLarge, Manifest, Dir, DirInSnapshot, Pack, MissingJudgePath, Store, StoredDigest, Version, Lock, LockFile, LockWrite, LabelMismatch)"
  - "CLI subcommand `minos judge push <dir> [--update]` with handler judge_push_command"
  - "Test target crates/minos-cli/tests/judge_push.rs (12 tests)"
  - "CLAUDE.md: [[judge]] lock table and 4 MiB cap, what committing the lock exposes, the overlap refusal with its zero-length exemption, [judge] trials, judge push in Judge, CLI, Workspace, and Open questions"
affects: [07-12, 07-14, 07-15]
tech-stack:
  added: []
  patterns:
    - "One shared lock reader and atomic writer for every command that touches minos.lock"
    - "The lock decision runs before the blob is stored and again before the lock is written, so a refusal stores nothing and a concurrent entry is kept"
    - "Path containment compared component by component ASCII case-insensitively, failing closed on a case-insensitive filesystem"
key-files:
  created:
    - crates/minos-cli/src/lock_file.rs
    - crates/minos-cli/src/judge_push.rs
    - crates/minos-cli/tests/judge_push.rs
  modified:
    - crates/minos-cli/src/env_build.rs
    - crates/minos-cli/src/lib.rs
    - crates/minos-cli/src/main.rs
    - CLAUDE.md
decisions:
  - "judge push decides the lock outcome before storing the tarball, so a refused push (label mismatch, missing judge path, misplaced directory, pack refusal) leaves .minos/blobs untouched; an unchanged push still stores the blob, so a fresh clone with a committed lock gets its data back"
  - "A judge directory inside the project is accepted only under a [source] exclude entry; the project root itself and a non-RelPath relative path are refused (fail closed)"
  - "env build keeps its own error messages: a private load_lock maps LockFileError onto EnvBuildError::Read and EnvBuildError::Lock"
metrics:
  duration: 6min
  completed: 2026-10-23
  tasks: 3
  files: 7
---

# Phase 7 Plan 11: minos judge push Summary

`minos judge push <dir> [--update]` packs judge data with the source snapshot's hygiene under the 96 MiB cap. It refuses a directory the snapshot would pick up and a tree missing a judge argv path, stores the tarball as the tree's blob, and records a `[[judge]]` entry under the manifest's label through a lock helper shared with `env build`. Different data under a recorded label is refused naming both digests unless `--update` is passed.

## What was built

- **`lock_file.rs`**: `read_lock` (symlink-metadata probe, reads one byte over `MAX_LOCK_BYTES` so an over-cap file is `LockError::TooLarge`) and `write_lock` (temporary file in `.minos/`, `sync_all`, `persist`, directory fsync), moved unchanged in behavior out of `env_build.rs`. `env_build` calls them through a private `load_lock` that keeps its existing error variants and messages; all 11 `env_build` tests pass unchanged.
- **`judge_push.rs`**: the numbered-step pipeline: manifest; canonical placement check (`relative_to` compares components ASCII case-insensitively, `SourceExclude::covers` decides); `pack_judge_data`; judge argv paths present; lock decision; `put_reader` under `MAX_JUDGE_DATA_BYTES` with the blob ID checked against `TreeId::to_blob_id`; lock re-read, re-decided, and written blob-first-lock-last. The report carries `JudgeVersion::of(&tree, manifest.judge())`. The `LabelMismatch` message names both digests and ends with the `--update` hint.
- **`main.rs`**: a `judge` group with `push` (positional `dir`, `--update`), builder API only, plus a long help on hygiene, the cap, and what the lock exposes. The handler prints label and digest, counts, judge version, `written`/`unchanged`, and every denylist drop through `SafeText`. There is a new unit test for argument parsing.
- **CLAUDE.md**: all seven edits the plan lists. The Source paragraph's deferral sentence is replaced by the D-04/D-20 overlap refusal.

## Verification

- `cargo test -p minos-cli --test judge_push --test env_build`: 12 and 11 passed (1 ignored Docker test).
- `cargo test -p minos-cli --bin minos`: 8 passed. `minos judge push --help` lists `<dir>` and `--update`.
- An end-to-end binary run on a scratch project printed `written`, then `unchanged`, then a label-mismatch refusal (exit 1), and the `.env` drop was listed every time.
- Full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (85 result lines, 0 failures), `cargo deny check` (advisories, bans, licenses, sources ok), `cargo doc --workspace --no-deps`, and the guest musl clippy all exit 0. C-01 printed only `ok` lines, and C-02 printed nothing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] The lock decision runs before the blob is stored**
- **Found during:** Task 1
- **Issue:** The plan reads the lock at step 7, after `put_reader`. A refused label mismatch would then leave the private judge data in the store, named by nothing.
- **Fix:** Decide before storing, then re-read and decide again before writing (as `env build` re-reads). A refused push stores nothing, and the test asserts the refused tree's blob is absent.
- **Files modified:** crates/minos-cli/src/judge_push.rs, crates/minos-cli/tests/judge_push.rs
- **Commit:** 22f5dd6

**2. [Rule 2 - Correctness] Two error variants beyond the plan's list**
- **Found during:** Task 1
- **Issue:** The manifest read needs a cap refusal, and the stored-digest check is clearer when it names both IDs.
- **Fix:** Added `JudgePushError::TooLarge { what, max }`. `StoredDigest` carries `{ tree, stored }`.
- **Commit:** 22f5dd6

**3. [Naming] The env_build wrapper is `load_lock`**
- **Issue:** The acceptance grep requires that no `fn read_lock` or `fn write_lock` remain in env_build.rs.
- **Fix:** The thin error-mapping wrapper is named `load_lock`.
- **Commit:** 22f5dd6

## TDD Gate Compliance

- RED: `8ec703e` test(07-11), failing to compile because `minos_cli::judge_push` did not exist.
- GREEN: `22f5dd6` feat(07-11). The `cli_wiring` test turned green with Task 2 (`5fb9124`). No refactor commit was needed.

## Threat Flags

None. The new surface (a user-chosen directory read into the store, its paths and digests written to the lock) is the plan's threat model, and T-07-11-01 to T-07-11-06 are each mitigated as listed.

## Known Stubs

None.

## Commits

- 8ec703e test(07-11): add failing tests for minos judge push
- 22f5dd6 feat(07-11): implement judge push and share the lock helpers
- 5fb9124 feat(07-11): add the minos judge push subcommand
- f163838 docs(07-11): record judge data, the lock table, trials, and the overlap refusal

## Self-Check: PASSED

All three created files exist; commits 8ec703e, 22f5dd6, 5fb9124, and f163838 are in the log.
