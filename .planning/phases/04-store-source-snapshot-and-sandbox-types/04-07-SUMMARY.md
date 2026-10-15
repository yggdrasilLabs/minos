---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 07
subsystem: source
tags: [minos-source, snapshot, tar, ustar, ignore, secret-scan, determinism, CLI-11]

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    provides: "04-04 deny_component and DenyRule, Scanner, CleanPolicy and check_clean; 04-01 crate edges and clippy bans"
  - phase: 03-core-types-and-pure-logic
    provides: "minos-core Manifest (excludes, judge paths, surface, program), RelPath, SafeText, TreeId, BlobId"
provides:
  - "snapshot(root, manifest, policy): the only producer of a source tarball"
  - "Snapshot (tarball, into_tarball, tree, report), PackReport (files, content_bytes, denied), Denied, Offender, OffenderKind, SnapshotError"
  - "walk.rs (crate-private): four-source walker, offender collection, fixed caps, .minosignore negation refusal"
  - "pack.rs (crate-private): hand-built ustar headers with zeroed metadata"
  - "CLAUDE.md Source paragraph records the snapshot rules"
affects: [04-09, phase-5-runner, phase-7-judge-push, phase-8-run]

tech-stack:
  added: []
  patterns:
    - "Offenders kept in a BTreeSet capped at 20 by pop_last plus a full count, so a hostile tree cannot grow the error unboundedly"
    - "Entry-filter drops reported over an mpsc channel; a failed send sets a flag and the pack fails closed"
    - "lstat, then open, then fstat with dev/ino/len equality before any read; a mismatch is the Raced offender"

key-files:
  created:
    - crates/minos-source/src/walk.rs
    - crates/minos-source/src/pack.rs
    - crates/minos-source/tests/pack.rs
    - crates/minos-source/tests/refusals.rs
  modified:
    - crates/minos-source/src/lib.rs
    - CLAUDE.md

key-decisions:
  - "04-07: an ignore file the walker cannot read or parse (ignore::DirEntry::error) fails the pack, because the exclusions would otherwise not be the ones the user wrote"
  - "04-07: the .minosignore negation check strips a UTF-8 BOM from line 1 first, because the ignore matcher does, so a BOM cannot hide a negation"
  - "04-07: a .minosignore that is a symlink or not a regular file is an offender, checked per directory, because the matcher would follow or fail on it"
  - "04-07: the offender list takes priority over the total-size error; reads stop once the total passes 64 MiB while sizes keep being recorded"

patterns-established:
  - "Golden tarball hash pinned as a plain assert_eq! literal computed outside the code, with the byte recipe in a comment, preceded by per-file content digests"

requirements-completed: [CLI-11]

coverage:
  - id: D1
    description: "Deterministic snapshot: same tree twice and two creation orders give identical bytes; golden five-file tree packs to 6656 bytes with the externally computed TreeId; zeroed headers; system tar lists it; TreeId equals the BlobId"
    requirement: "CLI-11"
    verification:
      - kind: integration
        ref: "crates/minos-source/tests/pack.rs#golden_project_tree"
        status: pass
      - kind: integration
        ref: "crates/minos-source/tests/pack.rs#headers_are_zeroed"
        status: pass
      - kind: integration
        ref: "crates/minos-source/tests/pack.rs#system_tar_lists_the_tarball"
        status: pass
    human_judgment: false
  - id: D2
    description: "Exactly four exclusion sources; parent .gitignore and .ignore have no effect; every denylist and judge-path drop reported with its rule"
    requirement: "CLI-11"
    verification:
      - kind: integration
        ref: "crates/minos-source/tests/pack.rs#exclusions"
        status: pass
      - kind: integration
        ref: "crates/minos-source/tests/pack.rs#denylist_drops_are_reported"
        status: pass
    human_judgment: false
  - id: D3
    description: "Every refusal: offenders listed together, sanitized hostile name, per-file/file-count/total caps, .minosignore negation, missing surface, planted secret (also under AllowDirty), clean check"
    requirement: "CLI-11"
    verification:
      - kind: integration
        ref: "crates/minos-source/tests/refusals.rs"
        status: pass
    human_judgment: false
  - id: D4
    description: "CLAUDE.md Source paragraph records the exclusion union, negation refusal, denylist, offenders, caps, surface rule, scan scope, header recipe, and the Phase 7 judge-digest deferral"
    verification:
      - kind: other
        ref: "grep -n 'Four exclusion sources apply as a union' CLAUDE.md"
        status: pass
    human_judgment: false

duration: 7min
completed: 2026-10-15
status: complete
---

# Phase 4 Plan 07: Deterministic, secret-scanned source snapshot Summary

**`snapshot(root, manifest, policy)` is now the only way to produce a source tarball. It walks the project under exactly four exclusion sources, reports every denylist and judge-path drop, and refuses links, special files, out-of-grammar names, paths that do not fit ustar, raced files, and over-cap trees in one listing error. It scans exactly the bytes it will pack, runs the git clean check unless `AllowDirty` is passed, and writes hand-built ustar headers whose SHA-256 matches the externally computed golden TreeId `bb2319a3...`.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-10-15T14:42:57Z
- **Completed:** 2026-10-15T14:49:34Z
- **Tasks:** 3
- **Files modified:** 6 (4 created, 2 modified)

## Accomplishments

- `walk.rs` builds the walker with `standard_filters(false)` first, then `git_ignore(true)`, `require_git(false)`, `add_custom_ignore_filename(".minosignore")`, `hidden(false)`, `follow_links(false)`, and `sort_by_file_path`. Its `filter_entry` checks rules in this order:
  - `deny_component` on the entry name: reported, then pruned.
  - A judge path, compared by ASCII case key at component boundaries: reported as `JudgePath`, then pruned.
  - A `[source] exclude` entry: pruned silently.
- In the main loop, every directory's `.minosignore` is checked for size (64 KiB) and for `!` lines. Each non-directory entry is classified: `Symlink`, `Special`, `NotInGrammar`, `Hardlink`, `UstarPath`, `Oversize`, or `Raced`. The file-count cap (10,000) fails at once. Reads stop once the content total passes 64 MiB, but sizes keep being recorded so the error can name the largest files.
- `pack.rs` writes headers with exactly the calls the plan lists, in that order, and appends them with `Builder::append`. The golden five-file tree packs to 6656 bytes and its TreeId is `bb2319a34a6a8a85974b4e7ce637428492c8f9b33f8b189747fca11462d9c0f0`. That value was computed outside the code, and the packer matched it on the first run.
- `lib.rs` adds `snapshot`, which runs these steps in order: walk, surface/program check, secret scan (first 20 hits plus a total), `check_clean` unless `AllowDirty`, the tarball, and `TreeId::of`. It also adds the public types `Snapshot`, `PackReport`, `Denied`, `Offender`, `OffenderKind`, and `SnapshotError`. Every `Display` shows paths only through `SafeText` or `RelPath`.
- CLAUDE.md's Source paragraph now states the snapshot rules. The 04-04 git carve-out is unchanged.

## Task Commits

1. **Task 1: Walk, pack, and snapshot**: `9fed5fd` (test, RED), `bd79b68` (feat, GREEN)
2. **Task 2: Every refusal**: `39d36bf` (test)
3. **Task 3: CLAUDE.md Source paragraph**: `f538821` (docs)

## Files Created/Modified

- `crates/minos-source/src/walk.rs`: the cap constants, `FileEntry`, `Collected`, `collect`, the walker and entry filter, `Drops`, `State`, and `read_same`
- `crates/minos-source/src/pack.rs`: `tarball` and `header`
- `crates/minos-source/src/lib.rs`: `snapshot` and the public types; private `mod walk; mod pack;`
- `crates/minos-source/tests/pack.rs`: 8 tests (determinism, golden, headers, system tar, blob address, exclusions, denylist report)
- `crates/minos-source/tests/refusals.rs`: 12 tests (every refusal, plus a nested BOM negation case)
- `CLAUDE.md`: the Source paragraph

## Decisions Made

- **Unreadable ignore file fails the pack.** If the walker cannot read or parse an ignore file (`DirEntry::error()`), the pack fails as `SnapshotError::Walk`. Otherwise a broken `.gitignore` would silently pack files the user meant to exclude.
- **BOM before a negation.** The negation check strips a UTF-8 BOM from line 1 before testing for `!`. ignore 0.4.33 strips the BOM before parsing, so `\xEF\xBB\xBF!keep.log` is a negation. A test covers this.
- **Non-regular `.minosignore`.** A `.minosignore` that is a symlink or not a regular file is listed once, as an offender, by the directory check. The main loop skips that entry so it is not listed twice.
- **Failed drop report fails closed.** If a denylist drop cannot be sent from the entry filter, a flag is set and the pack fails. The drop is never silently lost (rules/anti-empty-catch).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Ignore-file errors and the BOM-prefixed negation**
- **Found during:** Task 1. Reading the ignore 0.4.33 source showed that `DirEntry` carries partial errors from parsing ignore files, and that `GitignoreBuilder::add` strips a BOM from line 1.
- **Issue:** Without a fix, a broken ignore file would be skipped silently, and a negation behind a BOM would pass the `!` check.
- **Fix:** Treat `entry.error()` as `SnapshotError::Walk`, and strip the BOM from line 1 before the `!` check.
- **Files modified:** crates/minos-source/src/walk.rs
- **Verification:** `refusals.rs#nested_minosignore_negation_after_a_bom_is_refused`
- **Committed in:** bd79b68 (fix), 39d36bf (test)

**2. [Acceptance grep] `snapshot` signature wraps under rustfmt**
- **Issue:** The one-line grep for `pub fn snapshot(root: &Path, manifest: &Manifest, policy: CleanPolicy) -> Result<Snapshot, SnapshotError>` cannot match. The signature is longer than rustfmt's 100 columns, and `cargo fmt --all --check` is a gate.
- **Fix:** None needed. The signature is exactly the planned one, split over five lines (lib.rs lines 335-339).

**3. [TDD] Task 2 RED could not fail**
- **Issue:** Task 2 adds tests only, and Task 1 had already built the behavior they test, so the tests passed when first run. The plan says not to change src in Task 2 unless a test exposes a bug, and none did.
- **Note:** Task 1 has the full RED (`9fed5fd`, the tests did not compile without `snapshot`) and GREEN (`bd79b68`) sequence.

**Total deviations:** 3 (1 auto-fix under Rule 2, 1 grep that cannot match a formatted signature, 1 TDD note). **Impact:** stricter failure behavior only. No interface changes.

## Issues Encountered

- Two `redundant_closure_for_method_calls` clippy findings, one in walk.rs and one in refusals.rs, were fixed before their commits.
- The known flaky `minos-backend-krun` supervise test did not fail. The full workspace run passed: 904 passed, 0 failed, 26 ignored.

## Known Limitations

- If a regular file is swapped for a FIFO between `lstat` and `open`, the read-only `open` blocks until a writer appears. That would hang the pack, but no data could leak through it. std offers no `O_NONBLOCK`/`O_NOFOLLOW` without `libc`, and `Cargo.toml` is frozen this phase. It needs a concurrent attacker writing to the user's own working tree, which no candidate can do. A later plan with a `rustix` edge could open with `O_NOFOLLOW | O_NONBLOCK`.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 904 passed, 0 failed, 26 ignored
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok
- `cargo doc --workspace --no-deps`: exit 0, no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: 11 members ok, none MISSING. C-02: prints nothing.
- `grep -rn 'pub fn' crates/minos-source/src/pack.rs crates/minos-source/src/walk.rs`: prints nothing
- `cargo test -p minos-source --test scan`: 9 passed (the self-scan still finds no secret-shaped literal)

## Next Phase Readiness

`minos-cli` can call `snapshot` and store the tarball with `Blobs::put`, whose ID equals `snapshot.tree().to_blob_id()`. Comparing packed files with pushed judge data by digest waits for Phase 7 (JUDGE-01), when per-file digests are recorded.

## Self-Check: PASSED

- FOUND: crates/minos-source/src/walk.rs, src/pack.rs, src/lib.rs, tests/pack.rs, tests/refusals.rs
- FOUND: commits 9fed5fd, bd79b68, 39d36bf, f538821
