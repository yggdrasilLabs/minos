---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 01
subsystem: infra
tags: [cargo, cargo-deny, clippy, layering, minos-source, tar, ignore]

requires:
  - phase: 03-core-types-and-pure-logic
    provides: minos-core types (RelPath, TreeId, IdHasher) that minos-source and minos-sandbox build on
provides:
  - minos-source library crate skeleton (crate doc only), linked only by minos-cli
  - every Phase 4 dependency edge in Cargo.lock (no new registry package)
  - deny.toml wrapper entry that refuses any minos-source dependent other than minos-cli
  - crate clippy.toml files for minos-store, minos-sandbox, and minos-source, every entry proven to fire
  - 04-EVIDENCE.md with the layering refusal and the clippy ban captures
affects: [04-02, 04-03, 04-04, 04-05, 04-06, 04-07, 04-08, phase-5-env-build, phase-8-run]

tech-stack:
  added: []
  patterns:
    - "Crate clippy.toml bans named by public re-export path (tar::Builder, tar::Entry, ignore::WalkBuilder); each proven by a throwaway #[test] probe"
    - "A layering violation is proven with the cargo-deny JSON diagnostic, because the text graph abbreviates dependents as (*)"

key-files:
  created:
    - crates/minos-source/Cargo.toml
    - crates/minos-source/src/lib.rs
    - crates/minos-source/clippy.toml
    - crates/minos-store/clippy.toml
    - crates/minos-sandbox/clippy.toml
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/deferred-items.md
  modified:
    - Cargo.toml
    - Cargo.lock
    - deny.toml
    - crates/minos-cli/Cargo.toml
    - crates/minos-sandbox/Cargo.toml
    - CLAUDE.md

key-decisions:
  - "04-01: the packer, denylist, secret scan, and git clean check live in a new library crate minos-source that only minos-cli links (not minos-store, which minos-loop links; not a minos-cli module, which cannot be integration-tested)"
  - "04-01: clippy disallowed paths for tar and ignore use the public crate-root re-exports (tar::Archive, tar::Entry, tar::Builder, tar::Header, ignore::WalkBuilder); all fire, so no definition-path form is needed"
  - "04-01: requirement checkboxes CLI-11, SBX-05, and STORE-02 stay open; this plan only lays groundwork, and 04-05, 04-06, and 04-07 deliver them"

patterns-established:
  - "Crate clippy.toml: core's header comment plus the HashMap and HashSet disallowed-types verbatim, then crate-specific bans whose reasons name the requirement or decision"

requirements-completed: []

coverage:
  - id: D1
    description: "minos-source crate exists with workspace lints, links minos-core only, and is linked only by minos-cli"
    requirement: "CLI-11"
    verification:
      - kind: other
        ref: "cargo build -p minos-source && cargo deny check"
        status: pass
      - kind: other
        ref: "04-EVIDENCE.md#Layering: cargo deny check bans refuses a minos-loop -> minos-source edge"
        status: pass
    human_judgment: false
  - id: D2
    description: "Three crate clippy.toml files; every entry fires exactly once on a probe"
    requirement: "SBX-05"
    verification:
      - kind: other
        ref: "04-EVIDENCE.md#Clippy bans fire (04-01): store 4/4, sandbox 7/7, source 13/13"
        status: pass
      - kind: other
        ref: "cargo clippy -p minos-store -p minos-sandbox -p minos-source --all-targets -- -D warnings"
        status: pass
    human_judgment: false
  - id: D3
    description: "CLAUDE.md Workspace section and Build order name minos-source and its dependency rule"
    verification:
      - kind: other
        ref: "grep -n 'source. depends on .core. only' CLAUDE.md"
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-10-14
status: complete
---

# Phase 4 Plan 01: minos-source crate, dependency edges, and proven clippy bans Summary

**New `minos-source` library crate that only `minos-cli` may link, enforced by a `deny.toml` wrapper and shown by a captured refusal; every Phase 4 dependency edge in `Cargo.lock`; and 24 crate clippy bans (tar unpack, truncating writes, nondeterministic tar builders, `max_filesize`, env reads, hash collections), each shown to fire once.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-10-14T18:15:53Z
- **Completed:** 2026-10-14T18:21:00Z
- **Tasks:** 3
- **Files modified:** 13 (7 created, 6 modified)

## Accomplishments

- `crates/minos-source` is a workspace member with `[lints] workspace = true`, depending on minos-core, tar, ignore, regex, and thiserror (dev: tempfile, proptest). Its `lib.rs` holds only the crate doc. `ignore` and `regex` moved from minos-cli to it.
- `minos-sandbox` gained `base16ct` and dev `proptest`. `Cargo.lock` has no new registry package, and `[bans.build]` is unchanged.
- `deny.toml` lets minos-source link minos-core and lets only minos-cli link minos-source. A deliberate minos-loop -> minos-source edge was refused (`error[banned]`, exit 2), and the JSON diagnostic names `minos-loop` as the parent.
- Clippy bans, each one firing exactly once on a probe:
  - minos-store: `File::create`, `fs::write`, `HashMap`, `HashSet`
  - minos-sandbox: `Archive::unpack`, `Entry::unpack`, `Entry::unpack_in`, `File`, `OpenOptions`, `HashMap`, `HashSet`
  - minos-source: six tar `Builder` conveniences, `Header::set_metadata` and `set_metadata_in_mode`, `WalkBuilder::max_filesize`, `env::var`, `env::var_os`, `HashMap`, `HashSet`
- CLAUDE.md now lists minos-source in the crate tree, the dependency bullets, and Build order step 3.

## Task Commits

1. **Task 1: Create minos-source and wire every Phase 4 dependency edge**: `1e209d0` (feat)
2. **Task 2: Three crate clippy.toml files, each entry proven to fire once**: `56467aa` (chore)
3. **Task 3: CLAUDE.md Workspace section names minos-source**: `32246e2` (docs)

## Files Created/Modified

- `crates/minos-source/Cargo.toml`: manifest for the new library crate
- `crates/minos-source/src/lib.rs`: crate-level doc only; `snapshot` arrives in 04-07
- `crates/minos-source/clippy.toml`: bans for determinism, silent skips, and env reads
- `crates/minos-store/clippy.toml`: no truncating writes
- `crates/minos-sandbox/clippy.toml`: no unpack and no file handles
- `Cargo.toml`, `Cargo.lock`: member and workspace dependency key, plus the new edges
- `deny.toml`: minos-source wrapper entry; minos-source added to minos-core's wrappers
- `crates/minos-cli/Cargo.toml`: links minos-source; ignore and regex removed
- `crates/minos-sandbox/Cargo.toml`: base16ct and dev proptest
- `CLAUDE.md`: Workspace tree, dependency bullets, Build order step 3
- `04-EVIDENCE.md`: layering refusal and clippy captures
- `deferred-items.md`: a flaky backend-krun test (out of scope)

## Decisions Made

- The public re-export paths work as clippy disallowed paths for tar and ignore, even though their defining modules are private. No definition-path form was needed.
- The layering evidence includes the `cargo deny --format json` diagnostic and `cargo tree -i`, because the text output abbreviates the dependents as `(*)` and does not name minos-loop.

## Deviations from Plan

### Requirement checkboxes not marked

- **Found during:** state updates
- **Issue:** The plan lists `requirements: [CLI-11, SBX-05, STORE-02]`, but this plan only adds the crate, the edges, and the bans. The packer (04-07), the output reader (04-06), and the append-only ledger (04-05) deliver those requirements, and those plans list them too.
- **Action:** `requirements-completed` is empty, and `requirements.mark-complete` was not run, so REQUIREMENTS.md does not claim code that does not exist yet.

### Evidence capture repeated

- **Found during:** Task 1
- **Issue:** The first evidence assembly ran the JSON `cargo deny` query after the forbidden edge had been reverted, so it captured nothing.
- **Fix:** The edge was re-applied, all three outputs were captured to scratch files, and the edge was reverted again. `git diff --quiet -- crates/minos-loop/Cargo.toml` and `cargo deny check` were green afterwards.

**Total deviations:** 2 (one bookkeeping choice, one redo of an evidence capture). **Impact:** none on code.

## Issues Encountered

- The first `cargo test --workspace` run failed in `minos-backend-krun` `supervise::tests::supervise_run_kills_the_whole_process_group_at_the_host_deadline` with `probing the process group: Operation not permitted (os error 1)`. This plan does not touch that crate. A full rerun passed (729 passed, 0 failed, 25 ignored). The single test failed 1 time in 8 repeated runs and passes outside the command sandbox. It is logged in `deferred-items.md` with a likely but unproven mechanism (macOS EPERM when signalling a group whose last member is a zombie) and was not fixed here.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 729 passed, 0 failed, 25 ignored on the rerun (see Issues)
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`
- `cargo doc --workspace --no-deps`: exit 0
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: 11 members ok, none MISSING
- C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing
- No `zz_clippy_probe.rs` remains under `crates/`

## Next Phase Readiness

The six implementation plans in waves 2 and 3 can run in parallel without touching `Cargo.toml`, `Cargo.lock`, or `deny.toml`. Ready for 04-02.

## Self-Check: PASSED

- FOUND: crates/minos-source/Cargo.toml, crates/minos-source/src/lib.rs, crates/minos-source/clippy.toml, crates/minos-store/clippy.toml, crates/minos-sandbox/clippy.toml, 04-EVIDENCE.md, deferred-items.md
- FOUND: commits 1e209d0, 56467aa, 32246e2
