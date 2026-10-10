---
phase: 01-workspace-and-gates
plan: 05
subsystem: infra
tags: [gate-closure, clap, forbid, cargo-lock, cargo-deny, uat, evidence]
status: complete
gap_closure: true

requires:
  - phase: 01-04
    provides: "F-01 to F-03 findings (clap derive is E0453 under forbid; builder API, serde, and thiserror derives pass) and the open user decision"
provides:
  - "clap pinned at =4.6.7 with default features only; clap_derive and heck removed from Cargo.lock (UAT test 6 closed, decision A)"
  - "Stack of record (.claude/CLAUDE.md) and its generator source (.planning/research/STACK.md) name the clap builder API only, with the E0453 reason"
  - "D-12 amended and STATE.md F-01 marked decided"
  - "01-GATE-EVIDENCE.md ## Decision on F-01 (01-05): F-01-CLOSED, P-01, B-02, invariant check, five-command final gate, cargo clean"
affects: [01 phase verification, 08 CLI (minos-cli argument parsing uses clap::Command and clap::Arg)]

tech-stack:
  added: []
  patterns:
    - "Stack-of-record corrections go into .planning/research/STACK.md as well as the generated .claude/CLAUDE.md block"

key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/minos-cli/Cargo.toml
    - .claude/CLAUDE.md
    - .planning/research/STACK.md
    - .planning/STATE.md
    - .planning/phases/01-workspace-and-gates/01-CONTEXT.md
    - .planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md

key-decisions:
  - "UAT test 6 (A): minos-cli uses the clap builder API; clap's derive feature is off; re-enabling it is a user decision, never a lint change"
  - "P-01: no build-script allowlist entry was orphaned by dropping clap derive; deny.toml unchanged"

patterns-established:
  - "Lock changes for a feature drop use cargo update --workspace --offline and must be removal-only (numstat 0 additions)"

requirements-completed: [GATE-01, GATE-02, GATE-03]

coverage:
  - id: D1
    description: "clap derive feature dropped; Cargo.lock loses exactly clap_derive and heck; clap_derive absent from both checked target graphs"
    requirement: "GATE-01"
    verification:
      - kind: other
        ref: "cargo update --workspace --offline; git diff --numstat d2cf8cf HEAD -- Cargo.lock (0 19); cargo tree absence greps for aarch64-apple-darwin and aarch64-unknown-linux-musl"
        status: pass
    human_judgment: false
  - id: D2
    description: "Build-script allowlist re-checked against both target graphs; cargo deny check passes; workspace builds"
    requirement: "GATE-03"
    verification:
      - kind: other
        ref: "cargo deny check; presence-table loop (25 entries, none at darwin=0 musl=0); cargo build --workspace"
        status: pass
    human_judgment: false
  - id: D3
    description: "Five gate commands exit 0 on the final tree, C-02 grep empty, cargo clean run last"
    requirement: "GATE-02"
    verification:
      - kind: other
        ref: "cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; cargo deny check; cargo doc --workspace --no-deps; grep -rnE '#!?\\[allow\\(' crates --include='*.rs'; cargo clean"
        status: pass
    human_judgment: false
  - id: D4
    description: "Stack of record and generator source name the clap builder API only; D-12 and STATE.md record decision A"
    verification:
      - kind: other
        ref: "grep -cF 'builder API only' (3 in each file); diff of mirrored lines empty; MSRV row unchanged"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-10-09
---

# Phase 1 Plan 05: F-01 Decision (clap builder API) Summary

**UAT test 6 is closed with decision A. clap is pinned at =4.6.7 with default features, and Cargo.lock lost exactly clap_derive and heck, with nothing added. The stack of record and its STACK.md generator source now name the clap builder API only, giving the E0453-under-forbid reason. All five gate commands are green, and `cargo clean` ran last.**

## Performance

- **Duration:** about 6 min
- **Started:** 2026-10-10T00:09:59Z
- **Completed:** 2026-10-10T00:16Z
- **Tasks:** 3
- **Files modified:** 8

## Evidence

All verbatim records are in `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`, section `## Decision on F-01 (01-05)`. The first 7322 lines of that file are byte-identical to d2cf8cf (sha256 `5d9b1d08...dd08e`).

| Item | Result |
|------|--------|
| F-01-CLOSED | `cargo update --workspace --offline` exited 0 and printed `Locking 0 packages`, `Removing clap_derive v4.6.7`, and `Removing heck v0.5.0`. The Cargo.lock numstat is `0	19`. Both absence greps (darwin, musl) printed nothing. `comm -13` (packages that are new after the change) is empty on both targets. |
| P-01 | `cargo deny check` exited 0 with `advisories ok, bans ok, licenses ok, sources ok`. All 25 allowlist entries are still in at least one graph (kvm-ioctls is musl-only, msb_krun_hvf is darwin-only). Result: `Removed: none`, and deny.toml is unchanged. |
| B-02 | `cargo build --workspace` exited 0. Its only non-progress line is `Finished`. |
| Invariant check | Under crates/, only `crates/minos-cli/Cargo.toml` differs. rules, .cargo, rust-toolchain.toml, rustfmt.toml, clippy.toml, and the root CLAUDE.md are unchanged (`0`). The numstat is as planned. No lint line changed in Cargo.toml. |
| Final gate | fmt, clippy, test, deny, and doc each `Exit code: 0`. The C-02 grep printed `(no output)`. |
| cargo clean | `Removed 4090 files, 1.1GiB total`, and `target/` is gone. No cargo command ran after it. |

**Before/after `cargo tree -i`:** Only clap_derive and its exclusive dependency heck left the graph, on both targets. The proc-macro toolchain survives through other dependents:
- proc-macro2, quote, and syn 3.0.6 survive through serde_derive and thiserror-impl, with async-trait also using syn 3.0.6.
- proc-macro2 and quote are also reached through syn 2.0.119, darling, deku_derive, and tracing-attributes, plus zerocopy-derive on musl.
- unicode-ident survives through proc-macro2 and both syn versions.

## Accomplishments

- Dropped clap's unused `derive` feature. The two PENDING USER DECISION comments became decided notes, and the lock change removes lines only.
- Showed that the cargo-deny build-script allowlist is still exact for the reduced graph, and that the workspace builds on it.
- Corrected the clap lines in `.claude/CLAUDE.md` (39, 62, 168) and in their generator source `.planning/research/STACK.md` (28, 53, 237, and the sketch line 387), so a regeneration will not bring back the derive recommendation.
- Recorded decision A in D-12 (01-CONTEXT.md) and in STATE.md Blockers/Concerns.

## Task Commits

1. **Task 1: Drop the clap derive feature, regenerate Cargo.lock, F-01-CLOSED**: `5762daf` (fix)
2. **Task 2: P-01 allowlist re-check and B-02 build**: `9d83ca8` (docs; no orphan, so deny.toml is unchanged)
3. **Task 3: Stack of record, decision record, final gate, cargo clean**: `3be27f1` (docs)

## Files Created/Modified

- `Cargo.toml`: the clap entry now has no `features` key, under a three-line decided note.
- `Cargo.lock`: clap_derive v4.6.7 and heck v0.5.0 removed. Nothing was added or moved.
- `crates/minos-cli/Cargo.toml`: a one-line decided note above `clap.workspace = true`.
- `.claude/CLAUDE.md`: three clap lines name the builder API only. The MSRV row is unchanged.
- `.planning/research/STACK.md`: the same three lines, plus the Installation sketch's clap entry.
- `.planning/STATE.md`: line 96 now reads F-01 decided.
- `.planning/phases/01-workspace-and-gates/01-CONTEXT.md`: D-12 gains the decision sentence, and a second Amended footer line was added.
- `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`: the appended decision section.

## Decisions Made

- I applied the user's decision A from UAT test 6 as the plan specified. I made no other decisions, because P-01 found no orphaned allowlist entry.

## Deviations from Plan

None. The plan was executed exactly as written. Two notes:
- STATE.md already carried the orchestrator's pre-dispatch position update, which has the same line count, so line 96 was still the F-01 note. Task 3 committed that update together with the E-8 line.
- All scratch files went in the session scratchpad, which is outside the repository.

## Findings for the user

None. No item was recorded with `(UNEXPECTED RESULT)`.

## Issues Encountered

None.

## User Setup Required

None. No external service configuration is required.

## Carry-forward notes

1. Phase 8: minos-cli argument parsing uses the clap builder API (`clap::Command`, `clap::Arg`, `ArgMatches`). A clap derive cannot pass the gate. Re-enabling clap's `derive` feature is a user decision, never a lint change.
2. Phase 8: `rules/proj-lib-main-split.md` uses a clap derive example to illustrate its lib/main split. The rule's substance (a thin `main.rs`, with logic in `lib.rs`) still applies, and the builder API meets it. The example's derive is not a mandate, and `rules/` is not edited.
3. If GSD regenerates the `.claude/CLAUDE.md` stack block, it now reproduces the corrected clap lines from `.planning/research/STACK.md`.
4. GATE-06: if the phase verifier runs any cargo command after this plan, it must run `cargo clean` again as its last step.

## Next Phase Readiness

UAT test 6 is closed, and Phase 1 has no open gap. The phase is complete and ready for phase verification.

## Self-Check: PASSED

- Every modified file in key-files exists on disk.
- Commits 5762daf, 9d83ca8, and 3be27f1 are in `git log`.
- Every acceptance criterion for Tasks 1 to 3 was run and passed. The Task 1 and Task 2 cargo-based checks ran before `cargo clean`. Task 3's checks use no cargo.

---
*Phase: 01-workspace-and-gates*
*Completed: 2026-10-09*
