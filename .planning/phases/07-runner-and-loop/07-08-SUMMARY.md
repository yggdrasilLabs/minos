---
phase: 07-runner-and-loop
plan: 08
subsystem: minos-source (judge-data pack and the judge-overlap refusal), minos-cli env build
status: complete
tags: [source, judge, hygiene, snapshot, env-build, security, tdd]
requires:
  - "07-02: limits::MAX_JUDGE_FILES, the [[judge]] lock table, Lock::with_judge, Lock::judge_file_digests"
provides:
  - "minos_source::judge: pack_judge_data(root) and JudgePack (tarball, into_tarball, tree, files, report), re-exported at the crate root"
  - "minos_source::snapshot(root, manifest, policy, judge_files: &BTreeSet<BlobId>)"
  - "SnapshotError::JudgeOverlap { paths, total } and SnapshotError::JudgeTooLarge { bytes, max }"
  - "Private walk::{WalkRules, Caps} with WalkRules::source(manifest) and WalkRules::judge()"
  - "minos env build passes the lock's judge file digests to its snapshot"
affects: [07-11, 07-12, 07-14]
tech-stack:
  added: []
  patterns:
    - "One walker, two rule sets: the snapshot and the judge pack share the ignore files, denylist, offender rules, identity check, scan, and packer, and differ only in WalkRules"
    - "A check that refuses lists the first 20 offenders in path order and counts the rest, as every other snapshot refusal does"
key-files:
  created:
    - crates/minos-source/src/judge.rs
    - crates/minos-source/tests/judge.rs
    - .planning/phases/07-runner-and-loop/deferred-items.md
  modified:
    - crates/minos-source/src/lib.rs
    - crates/minos-source/src/walk.rs
    - crates/minos-source/tests/pack.rs
    - crates/minos-source/tests/refusals.rs
    - crates/minos-cli/src/env_build.rs
    - crates/minos-cli/tests/env_build.rs
decisions:
  - "The judge pack's walk caps are MAX_JUDGE_FILES files and MAX_JUDGE_DATA_BYTES per file and in total, and the tarball itself is checked against MAX_JUDGE_DATA_BYTES after packing (JudgeTooLarge), because the tarball, headers included, is what the input disk's judge section carries"
  - "Content over the judge cap is the walk's existing TooLarge (naming the largest files), and a single file over it is an Oversize offender read from lstat before any read; JudgeTooLarge covers the remaining case where content fits but headers and end blocks do not"
  - "OffenderKind::Oversize keeps its shape (refusals.rs had to stay untouched in Task 1); its Display now names both caps instead of claiming 16 MiB, and TooManyFiles and TooLarge say 'the tree' rather than 'the project', since the judge pack reuses them"
  - "check_judge_overlap runs after the surface check and before the secret scan and the clean check; an empty judge set short-circuits, so existing projects without a [[judge]] entry hash no extra bytes"
  - "env build reads minos.lock before the snapshot for the digest set and still re-reads it at step 7, so an entry written by a concurrent judge push during the build is not dropped; an unparsable lock now fails before the build instead of after it"
  - "CLAUDE.md's Source paragraph note ('waits for Phase 7') is left for 07-11 Task 3, which owns the CLAUDE.md judge-data edits"
metrics:
  duration: "about 15 minutes"
  completed: 2026-10-22
  tasks: 2
  files: 9
---

# Phase 7 Plan 08: Judge-data pack and the judge-overlap refusal Summary

`pack_judge_data` packs a judge directory through the same walker, hard denylist, offender rules, secret scan, and ustar packer as the source snapshot, under the 96 MiB and 10,000-file judge caps. Every source snapshot, including the `env build` context, now refuses a non-empty file whose SHA-256 equals pushed judge data, naming every path. Zero-length files are the only exemption.

## What was built

**Task 1, `pack_judge_data` (commit 7c9480e).** `walk.rs` now runs over a private `WalkRules { judge_keys, excludes, caps }`. `WalkRules::source(manifest)` reproduces the snapshot's old behavior exactly: it drops judge paths, applies `[source] exclude`, and enforces the `MAX_SNAPSHOT_*` caps. `WalkRules::judge()` has no judge keys and no excludes, with caps of `MAX_JUDGE_FILES` files and `MAX_JUDGE_DATA_BYTES` per file and in total. `State` reads its caps from the rules. Everything else is unchanged for both walks: the `.gitignore` and `.minosignore` handling, the negation refusal, the denylist, the offender rules, and the read-same identity check.

The new `judge.rs` adds `JudgePack` (the tarball, its `TreeId`, a per-file `RelPath -> BlobId` map, and the `PackReport`) and `pack_judge_data`. It walks, runs the shared `scan_files` helper (the snapshot's scan loop, moved out of `snapshot`), packs, and refuses a tarball over `MAX_JUDGE_DATA_BYTES` with `JudgeTooLarge`. It does no clean check.

The module docs list what is shared, what differs, and why.

**Task 2, the overlap refusal (commits 76af457 and 5867708).** `snapshot` takes `judge_files: &BTreeSet<BlobId>`. After the surface check it refuses with `JudgeOverlap { paths, total }` when any non-empty packed file's `BlobId::of` is in the set. The error lists 20 paths in order and adds "and N more" for the rest. This check runs before the secret scan and the clean check. The pack and refusal tests pass an empty set and still pack byte-identically.

`env_build` reads `minos.lock` before step 2 and passes `Lock::judge_file_digests` (or an empty set when there is no lock). Judge data therefore cannot reach the image build context either.

## Tests

- `crates/minos-source/tests/judge.rs` has 18 tests:
  - **Determinism and contents:** packing is deterministic, the `TreeId` and per-file digests are correct, and the 0755 and 0644 modes survive.
  - **Drops:** `.env` and `id_rsa` drops are reported.
  - **Offenders:** a symlink, a FIFO, and hardlinks are refused, all listed in one error.
  - **Secrets:** a secret is refused naming file, pattern, and line, never the bytes.
  - **Ignore files:** `.gitignore` and `.minosignore` inside the judge directory apply, and a `!` negation is refused.
  - **Caps:**
    - a sparse file of 96 MiB + 1 is an `Oversize` offender;
    - a file of exactly 96 MiB gives `JudgeTooLarge` with bytes = cap + 1536;
    - two half-cap sparse files give `TooLarge`;
    - 10,001 files give `TooManyFiles`.
  - **No manifest rules:** no clean check and no judge-path drop.
  - **Overlap (7 tests):**
    - one offending path is named, and two are named in sorted order;
    - 25 overlaps are listed as 20 plus "and 5 more";
    - empty files are exempt;
    - a set that matches nothing produces identical bytes;
    - the overlap check comes before the scan and the clean check;
    - the digests from a real `pack_judge_data` refuse their copy in a project.
- `crates/minos-cli/tests/env_build.rs` has a new test, `judge_overlap_blocks_build`. A lock whose `[[judge]]` entry lists `src/lib.rs`'s bytes refuses with `EnvBuildError::Snapshot(SnapshotError::JudgeOverlap)`, the builder never starts, and the lock is unchanged. I proved this test fails when env build passes an empty set, then restored the wiring.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: ok
- `cargo test --workspace`: ok, all `test result` lines with 0 failed, on two of three runs; see Deferred Issues for the one flaky run
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok
- `cargo doc --workspace --no-deps`: no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: ok
- C-01: every crate prints `ok`
- C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing
- `crates/minos-source/tests/pack.rs` and `tests/refusals.rs` were unmodified after Task 1 (`git diff --stat` showed only `lib.rs` and `walk.rs`) and passed then. Task 2 changed only their `snapshot` call to pass an empty set.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Oversize and cap messages hard-coded the source caps**
- **Found during:** Task 1
- **Issue:** `OffenderKind::Oversize` displayed "over the 16 MiB per-file cap", and `TooManyFiles` suggested `[source] exclude`. Both would be wrong for a judge pack, whose per-file cap is 96 MiB and which has no manifest.
- **Fix:** Oversize now names both caps. `TooManyFiles` and `TooLarge` say "the tree" and mention `[source] exclude` only for a project. The variant shapes are unchanged, so `refusals.rs` stayed untouched.
- **Files modified:** `crates/minos-source/src/lib.rs`
- **Commit:** 7c9480e

**2. [Acceptance criterion] One `judge_files: &BTreeSet<BlobId>` line**
- **Found during:** Task 2 acceptance check
- **Issue:** The private overlap helper reused the parameter name, so the acceptance grep printed two lines instead of one.
- **Fix:** Renamed the helper's parameter to `judge`.
- **Commit:** 5867708

### TDD note

Both tasks are marked `tdd="true"` in a `type: execute` plan. I wrote each task's tests in the same change as its implementation and committed them together as one `feat` commit per task, not as separate RED and GREEN commits. For the env build wiring, I showed the RED state by hand: with an empty set, `judge_overlap_blocks_build` fails.

## Deferred Issues

- `minos-propose`'s `claude_cli::process::tests::stdout_over_its_cap_kills_the_run` and `the_deadline_kills_the_whole_process_group` failed once in three `cargo test --workspace` runs. They passed in the other two runs and in three isolated runs. This plan does not touch that crate. The flake is logged in `.planning/phases/07-runner-and-loop/deferred-items.md`.

## Known Stubs

None.

## Threat Flags

None. The plan's threat register covers the new surface:
- **T-07-08-01:** the overlap refusal, with env build wired.
- **T-07-08-02:** the shared scan.
- **T-07-08-03:** the shared offender rules and identity check.
- **T-07-08-04:** the judge caps, checked on metadata and on the tarball.
- **T-07-08-05:** the unchanged source tests with an empty set.

## Self-Check: PASSED

- FOUND: crates/minos-source/src/judge.rs
- FOUND: crates/minos-source/tests/judge.rs
- FOUND: .planning/phases/07-runner-and-loop/deferred-items.md
- FOUND: 7c9480e, 76af457, 5867708
