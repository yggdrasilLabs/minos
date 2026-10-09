---
phase: 01-workspace-and-gates
plan: 04
subsystem: infra
tags: [gate-closure, clippy, forbid, rustflags, cargo-doc, uat, evidence]
status: complete
gap_closure: true

requires:
  - phase: 01-03
    provides: "Gate evidence with G-01 to G-03 gap demonstrations and the three open decisions"
provides:
  - "unwrap_used, expect_used, indexing_slicing at forbid in Cargo.toml and CLAUDE.md Code rules (UAT test 1 closed)"
  - ".cargo/config.toml [build] rustflags forbid backstop for unsafe_code and the three clippy lints (UAT test 2 closed, with D-15)"
  - "cargo doc --workspace --no-deps as the fifth gate command in CLAUDE.md Commands (UAT test 3 closed)"
  - "01-GATE-EVIDENCE.md ## Gap closure (01-04): 15 subsections of verbatim closing proofs, the five-command final gate, and cargo clean"
affects: [01 phase verification, 03 minos-core (test code cannot #[expect] unwrap/expect/indexing), 05 krun backend and guest (target rustflags caveat), 08 CLI (clap derive blocked by forbid)]

tech-stack:
  added: []
  patterns:
    - "Forbid-level lints repeated as [build] rustflags; workspace [lints] stays the primary policy and both files change together"

key-files:
  created:
    - .cargo/config.toml
  modified:
    - Cargo.toml
    - CLAUDE.md
    - .planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md
    - .planning/phases/01-workspace-and-gates/01-CONTEXT.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/PROJECT.md

key-decisions:
  - "UAT test 1 (B): unwrap_used, expect_used, indexing_slicing raised to forbid; #[expect] can no longer silence them"
  - "UAT test 2 (A+B): .cargo/config.toml rustflags backstop at forbid plus D-15 per-manifest [lints] workspace = true check for every new crate"
  - "UAT test 3 (A): cargo doc --workspace --no-deps is the fifth gate command"
  - "F-01 (clap derive fails clippy under forbid) recorded and carried to the user; no lint level, manifest, or clap feature changed"

patterns-established:
  - "Any new crate carries the D-15 C-01 manifest check as an acceptance criterion"

requirements-completed: [GATE-01, GATE-02, GATE-05]

duration: 5min
completed: 2026-10-09
---

# Phase 1 Plan 04: Gate Gap Closure Summary

**Clippy unwrap/expect/indexing raised to forbid, a `.cargo/config.toml` forbid rustflags backstop, and `cargo doc --workspace --no-deps` as the fifth gate command, each proven firing with verbatim evidence; clap derive is now blocked by forbid (F-01, for the user).**

## Performance

- **Duration:** about 5 min
- **Started:** 2026-10-09T22:26:09Z
- **Completed:** 2026-10-09T22:31Z
- **Tasks:** 3
- **Files modified:** 9 (1 created, 8 modified)

## Accomplishments

- UAT tests 1, 2, and 3 closed. Each closing proof is recorded verbatim in `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` under `## Gap closure (01-04)`.
- The five gate commands pass on the final tree, every probe was reverted, and `cargo clean` ran last (removed 5800 files, 1.3GiB).
- CLAUDE.md changed only in the Code rules and Commands blocks (`6	3	CLAUDE.md` against c4b5bab). `rules/` is unchanged.

## Closing proofs (see `## Gap closure (01-04)` in 01-GATE-EVIDENCE.md)

All clippy commands are `cargo clippy --workspace --all-targets -- -D warnings` unless stated otherwise.

| ID | Command | Observed exit | Closes |
|----|---------|---------------|--------|
| G-01-CLOSED | clippy, inner `#![allow(clippy::unwrap_used, reason)]` plus `unwrap()` | 101 (E0453) | UAT test 1 |
| R-01 | clippy, `#[expect(clippy::indexing_slicing)]` (V-07 re-run) | 101 (E0453) | consequence of test 1 |
| R-02 | clippy, `#[expect(clippy::get_first)]` | 0 | positive control |
| R-03 | clippy, inner `#![allow(clippy::get_first, reason)]` | 0 | residual, recorded |
| F-01 | clippy, `#[derive(clap::Parser)]` in minos-cli | 101 (3 x E0453) | finding |
| F-02 | clippy, clap builder API | 0 | positive control |
| F-03 | clippy, serde and thiserror derives | 0 | positive control |
| G-02-CLOSED run 1 | clippy, minos-loop without `[lints]`, unsafe/unwrap/expect/index | 101 (4 errors, `-F` on the command line) | UAT test 2 |
| G-02-CLOSED run 2 | `RUSTFLAGS= cargo clippy ...` with the same edit | 0 | caveat demonstration |
| C-03-RERUN | clippy piped to `grep -ci 'unknown lint'` | output `0` | typo check |
| B-01 | `cargo build --workspace`, `cargo test --workspace`, `cargo deny check` | 0, 0, 0 | UAT test 2 (deps unaffected) |
| G-03-CLOSED run 1 | `cargo doc --workspace --no-deps`, untouched tree | 0 | UAT test 3 |
| G-03-CLOSED run 2 | `cargo doc --workspace --no-deps`, broken intra-doc link | 101 (`unresolved link`) | UAT test 3 |
| C-01-RERUN | per-manifest `[lints] workspace = true` loop | 10 `ok`, 0 `MISSING` | D-15 |
| Final gate | fmt, clippy, test, deny, doc | 0, 0, 0, 0, 0 | GATE-02 |
| cargo clean (01-04) | `cargo clean` | 0 | GATE-06 |

## Task Commits

1. **Task 1: forbid the three clippy lints, close G-01, record R-01 to R-03 and F-01 to F-03** - `4a0f49a` (fix)
2. **Task 2: rustflags backstop and cargo doc as the fifth gate command, close G-02 and G-03, B-01** - `3751a25` (fix)
3. **Task 3: planning doc amendments, decisions rewrite, five-command final gate, cargo clean** - `53140e8` (docs)

## Findings for the user

**F-01: `#[derive(clap::Parser)]` fails the gate under forbid.** clap_derive 4.6.7 emits `#[allow(clippy::restriction)]` on its generated impls, and that group contains the three forbidden lints. Verbatim from the F-01 run:

```text
error[E0453]: allow(clippy::restriction) incompatible with previous forbid
 --> crates/minos-cli/src/main.rs:4:10
  |
4 | #[derive(clap::Parser)]
  |          ^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::unwrap_used`)
  = note: this error originates in the derive macro `clap::Parser` (in Nightly builds, run with -Z macro-backtrace for more info)
```

The same error appears for `-F clippy::expect_used` and `-F clippy::indexing_slicing`. Per the D-12 amendment, clap's `Args`, `Subcommand`, and `ValueEnum` derives emit the same attribute. The clap builder API passes (F-02), and serde and thiserror derives pass (F-03). `[workspace.dependencies]` still enables clap's `derive` feature, and `.claude/CLAUDE.md` still lists clap with `derive`. Nothing was changed. Whether minos-cli uses the builder API or the forbid decision is revisited is your call.

**R-03: an inner allow with a reason still passes for lints that are not forbidden.** `#![allow(clippy::get_first, reason = "...")]` passed the gate (exit 0). GATE-05's "no allow anywhere" is therefore enforced mechanically only for outer allows, for reasonless inner allows, and for the three forbid lints. C-02's grep (`grep -rnE '#!?\[allow\(' crates`) is the only other check.

**Unexpected results:** none. Every item matched its expected exit status, so no heading carries `(UNEXPECTED RESULT)`.

## Carry-forward notes

1. Phase 3: `#[expect]` is no longer available for unwrap, expect, or indexing in test code. Tests either return `Result` or adopt `allow-unwrap-in-tests`, `allow-expect-in-tests`, and `allow-indexing-slicing-in-tests`. If adopted, those settings must go in both a root clippy.toml and crates/minos-core/clippy.toml, because core's file replaces any root file.
2. Phase 8, and any earlier minos-cli argument parsing: see F-01. Bring it to the user before writing a clap derive.
3. Any re-run of 01-03's V-13 (unknown registry) must add the mirror table to the existing `.cargo/config.toml` and restore it with `git checkout HEAD -- .cargo/config.toml`. Never delete `.cargo`.
4. Every plan that adds a crate carries the D-15 check.
5. Phase 5: `build.rustflags` also applies to `--target aarch64-unknown-linux-musl` builds of minos-guest. Any `target.<triple>.rustflags` key added for the guest would replace the backstop for that target (D-01 caveat).
6. 01-03's notes 2 (cargo-insta 1.48.0 against insta =1.49.0) and 3 (Phase 5 codesign mechanism) still stand.
7. GATE-06: if the phase verifier runs any cargo command after this plan, it must run `cargo clean` again as its last step.

## Decisions Made

All three decisions were locked by the user before execution (01-UAT.md tests 1 to 3). The executor made no design decisions. One evidence-format choice: the B-01 section states in one sentence that `cargo test` and `cargo deny check` show only their closing summary lines, matching 01-03's Final gate. The seven `warning[duplicate]` lines that cargo-deny prints (multiple-versions is `warn`) are not reproduced.

## Deviations from Plan

None. The plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. This plan adds no Rust code; every crate stays a Phase 1 stub by design.

## Threat Flags

None. `.cargo/config.toml` is the planned policy file (T-1-22, T-1-23), and no other new surface was introduced.

## Next Phase Readiness

- Phase 1 gate gaps are closed. Phases 2 and 3 can start from a gate that enforces forbid, the rustflags backstop, and rustdoc links.
- Blocking item for the user before any clap derive: F-01.

## Self-Check: PASSED

- FOUND: .cargo/config.toml (tracked; `git ls-files .cargo` prints it)
- FOUND: .planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md, with `## Gap closure (01-04)` holding 15 subsections, 15 `Exit code: 0` lines, and 5 non-zero lines
- FOUND: commits 4a0f49a, 3751a25, 53140e8
- `git status --porcelain --untracked-files=all -- crates Cargo.lock` prints nothing; no probe identifiers in crates/; `target/` absent after cargo clean
- The first 6757 lines of the evidence file match c4b5bab (sha256 840c1b56...)
