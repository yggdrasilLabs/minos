---
phase: 01-workspace-and-gates
plan: 03
subsystem: infra
tags: [gate-proofs, clippy, cargo-deny, layering, supply-chain, evidence]
status: complete

requires:
  - phase: 01-01
    provides: "Workspace lint baseline, minos-core clippy.toml, ten crates, committed Cargo.lock"
  - phase: 01-02
    provides: "deny.toml sources, build-script allowlist, C-crate bans, layering wrappers"
provides:
  - "01-GATE-EVIDENCE.md: verbatim command, exit code, and output for V-01..V-23, G-01..G-03, C-01..C-03, the deny re-check, the final four-command gate, and cargo clean"
  - "REQUIREMENTS GATE-02/04/05 and ROADMAP Phase 1 reworded to D-01 (four plain cargo commands, no script)"
  - "Three open decisions recorded for the user (inner allow with reason, missing [lints] table, rustdoc lint only under cargo doc)"
affects: [01 phase verification, 03 minos-core (test unwrap policy, cargo-insta version), 05 krun backend (codesign mechanism)]

tech-stack:
  added: []
  patterns:
    - "Violation proof protocol: clean check, single edit, single gate command with output captured outside the repo, verbatim evidence, revert from HEAD, clean check again"

key-files:
  created:
    - .planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md
  modified:
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md

key-decisions:
  - "Every D-11 violation fired its gate; V-23 (zstd feature on backhand) fired as expected, so no Findings-for-the-user section was needed"
  - "The three gaps (G-01..G-03) were demonstrated and left open for the user; no lint level or gate command changed"
  - "Evidence lives in 01-GATE-EVIDENCE.md because gsd-verifier owns 01-VERIFICATION.md"

patterns-established:
  - "Gate evidence records only what happened; expectations stay in the plan"

requirements-completed: [GATE-01, GATE-02, GATE-03, GATE-04, GATE-05]

coverage:
  - id: D1
    description: "Code-lint rules fire under cargo clippy -D warnings (unsafe, unwrap, expect, index, outer allow with and without reason, unfulfilled expect, clock and HashMap in core); expect with reason passes"
    requirement: GATE-01
    verification:
      - kind: other
        ref: "01-GATE-EVIDENCE.md V-01..V-10 (nine exit 101, V-07 positive control exit 0)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Supply-chain rules fire under cargo deny (git spec, git source, unknown registry, unlisted build script, C crate through a feature)"
    requirement: GATE-03
    verification:
      - kind: other
        ref: "01-GATE-EVIDENCE.md V-11..V-14, V-23"
        status: pass
    human_judgment: false
  - id: D3
    description: "Layering bans fire for loop->sandbox/backend/runner and core->sandbox/backend (dev); normal core->sandbox is a Cargo cycle"
    requirement: GATE-04
    verification:
      - kind: other
        ref: "01-GATE-EVIDENCE.md V-15..V-20"
        status: pass
    human_judgment: false
  - id: D4
    description: "Four gate commands pass on the final tree; cargo clean ran last"
    requirement: GATE-02
    verification:
      - kind: other
        ref: "01-GATE-EVIDENCE.md Final gate and cargo clean sections"
        status: pass
    human_judgment: false

duration: 4min
completed: 2026-10-09
---

# Phase 1 Plan 03: Gate Proofs and D-01 Wording Summary

**All 23 deliberate violations were rejected by their gate command and recorded with verbatim output. The three known gaps were reproduced and left open for the user. REQUIREMENTS and ROADMAP now describe the gate as four plain cargo commands, the final tree passes all four, and `cargo clean` ran last.**

## Performance

- **Duration:** about 4 min
- **Started:** 2026-10-09T21:33:58Z
- **Completed:** 2026-10-09T21:38:30Z
- **Tasks:** 3
- **Files:** 1 created, 2 modified (all under `.planning/`; nothing under `crates/`, `Cargo.lock`, or `.cargo` was committed)

## Evidence

The D-10 evidence is in `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`. Observed exit codes:

| ID | Gate command | Exit code |
|----|--------------|-----------|
| V-01 unsafe block | `cargo clippy --workspace --all-targets -- -D warnings` | 101 |
| V-02 unwrap | same | 101 |
| V-03 expect | same | 101 |
| V-04 slice index | same | 101 |
| V-05 outer allow, no reason | same | 101 |
| V-06 outer allow, with reason | same | 101 |
| V-07 positive control, expect with reason | same | 0 |
| V-08 unfulfilled expectation | same | 101 |
| V-09 SystemTime::now in core | same | 101 |
| V-10 HashMap in core | same | 101 |
| G-01 inner allow with reason (gap) | same | 0 |
| G-02 crate without `[lints]` (gap) | same | 0 |
| G-03 broken intra-doc link (gap) | clippy / `cargo test --workspace` / `cargo doc --workspace --no-deps` | 0 / 0 / 101 |
| V-11 git dependency without rev | `cargo deny check sources` | 8 |
| V-12 git source not allowed | `cargo deny check sources` | 8 |
| V-13 unknown registry | `cargo deny check sources` | 8 |
| V-14 unlisted build script (httparse) | `cargo deny check bans` | 2 |
| V-15 loop -> sandbox | `cargo deny check bans` | 2 |
| V-16 loop -> backend-krun | `cargo deny check bans` | 2 |
| V-17 loop -> runner | `cargo deny check bans` | 2 |
| V-18 core -> sandbox (dev) | `cargo deny check bans` | 2 |
| V-19 core -> backend-krun (dev) | `cargo deny check bans` | 2 |
| V-20 core -> sandbox (normal) | `cargo deny check` / clippy | 1 / 101 (Cargo dependency cycle) |
| V-21 anyhow in minos-store | `cargo deny check bans` | 2 |
| V-22 msb_krun in minos-backend-krun | `cargo deny check bans` | 2 |
| V-23 backhand `zstd` feature | `cargo deny check bans` | 2 (zstd-sys and cc explicitly banned, plus unlisted build scripts) |
| Deny re-check (reverted tree) | `cargo deny check` | 0 |
| Final gate | fmt / clippy / test / deny | 0 / 0 / 0 / 0 |
| cargo clean | `cargo clean` | 0 (`Removed 4871 files, 1.1GiB total`) |

One-time checks: C-01 printed ten `ok crates/...` lines and no `MISSING` line. C-02 printed nothing. C-03 printed `0`.

After every proof the tree outside `.planning/` matched HEAD, `Cargo.lock` was byte-identical to HEAD, and no `.cargo` directory existed. `grep -rn 'gate_probe\|GATE_PROBE' crates` prints nothing.

## Task Commits

1. **Task 1: code-lint proofs, gap demonstrations, one-time checks** - `0f8bdd7` (docs)
2. **Task 2: supply-chain and layering proofs** - `6fece0d` (docs)
3. **Task 3: D-01 wording, final gate, open decisions, cargo clean** - `b140c49` (docs)

## Decisions Made

- V-07 passed on its first run, so the probe needed no adjustment.
- V-12 used cfg-if rev `14432a28e180ec717933714aed7828fc7ed2fd29`, which `git ls-remote` confirmed is still HEAD.
- V-13's `.cargo/` was confirmed absent before the proof and deleted by its repository path afterwards. The rsproxy.cn mirror's cache entry under the user's cargo home, which was already there from research, was left in place.

## Deviations from Plan

### Auto-fixed Issues

None to code. Three things about how the evidence was recorded:

**1. Acceptance string mismatch for V-01 (not a gate gap)**
- **Found during:** Task 1 acceptance check
- **Issue:** The plan's acceptance list expects `grep -F 'usage of an unsafe block'`. clippy 0.1.97 actually prints ``error: usage of an `unsafe` block``, with backticks, so that literal grep returns 0. The gate did reject the violation (exit 101, with the `-F unsafe-code` forbid note and an extra `undocumented_unsafe_blocks` error).
- **Fix:** None. The evidence stays verbatim and was not edited to match the plan's string. The verifier should match ``usage of an `unsafe` block``.

**2. Invariant check moved out of the Final gate section**
- **Found during:** Task 3
- **Issue:** I first recorded the CLAUDE.md and rules/ check as a `###` subsection with an `Exit code: 0` line inside `## Final gate`. That would have given the section five `Exit code: 0` lines, not four.
- **Fix:** It is now its own `## Invariant check` section with the status in prose. `## Final gate` has exactly four `Exit code: 0` lines.

**3. Overall exit-code counts grew after Task 2**
- Task 2's criterion of 6 `Exit code: 0` lines held when Task 2 was committed. After Task 3 the file has 10: 5 from Task 1, 1 from the deny re-check, and 4 from the final gate. `cargo clean` adds an 11th, in its own section. The count of `Exit code: [1-9]` lines is 24, as expected.

### Notes

- V-23 was run once outside the helper so its result could decide the heading. That same run's output is what the evidence records; nothing was re-run.
- C-02's output block is empty because `grep` matched nothing.
- `cargo doc` output for G-03 keeps its `Documenting ...` lines, because they are not among the progress prefixes the protocol strips.

## Findings for the user

None. V-23 behaved as expected: `zstd-sys` (and `cc`) were reported as explicitly banned, so the C-crate ban fires through the backhand `zstd` feature.

## Open decisions for the user

The four gate commands cannot catch these three gaps on stable Rust 1.97.1. Each was verified in research (RESEARCH.md Open Questions 1 to 3) and is demonstrated again as G-01 to G-03 in 01-GATE-EVIDENCE.md. This phase does not resolve them: it does not change the CLAUDE.md lint levels and it does not add a gate command.

1. **An inner `#![allow(..., reason = "...")]` passes.** clippy's `allow_attributes` only lints outer `#[allow]`. `#![allow(clippy::unwrap_used, reason = "...")]` plus an `unwrap()` passes `cargo clippy --workspace --all-targets -- -D warnings`. A reasonless inner allow is still caught by `allow_attributes_without_reason`. Options: (A) accept and document the gap; (B) raise `unwrap_used`, `expect_used`, and `indexing_slicing` to `forbid`, which also makes `#[expect]` impossible for those three and is an edit to CLAUDE.md Code rules.
2. **A crate that omits `[lints] workspace = true` escapes every workspace lint.** Cargo rejects a partial override but not a missing table. Options: (A) the one-time check this phase runs, plus review of every new crate manifest; (B) a `.cargo/config.toml` `[build] rustflags` backstop (`-Funsafe_code`, `-Dclippy::unwrap_used`, `-Dclippy::expect_used`, `-Dclippy::indexing_slicing`), which duplicates the lint list, is silently overridden by a `RUSTFLAGS` environment variable, and is not one of D-01's policy files.
3. **`broken_intra_doc_links = "deny"` is enforced only by `cargo doc`.** `cargo clippy` and `cargo test` ignore `[workspace.lints.rustdoc]`; only `cargo doc --workspace --no-deps` fails on a broken link. Options: (A) add `cargo doc --workspace --no-deps` as a fifth command, which changes CLAUDE.md Commands and ROADMAP Phase 1 criterion 1; (B) document that the rustdoc table takes effect only when docs are built.

## Carry-forward notes

1. **Phase 3:** `#[cfg(test)]` code that unwraps will fail clippy. If `allow-unwrap-in-tests` and its siblings are adopted, they must go in both a root clippy.toml and crates/minos-core/clippy.toml, because core's file replaces any root file (RESEARCH.md Open Question 5).
2. **Phase 3:** the installed cargo-insta is 1.48.0, while `insta` is pinned at =1.49.0. Upgrade cargo-insta before the first golden file.
3. **Phase 5:** with no xtask, the codesign mechanism for minos-vmm is still open. ROADMAP Phase 5 criterion 2 and REQUIREMENTS KRUN-08 still name an xtask (CONTEXT.md Deferred Ideas).
4. **GATE-06:** if the phase verifier runs any cargo command after this plan, it must run `cargo clean` again as its last step before the phase is marked complete.

## Issues Encountered

- A `sed -i` edit to my scratchpad helper failed on quoting and left the helper unchanged. I added a separate summary function instead. The repository was not affected.

## Next Phase Readiness

Phase 1 is ready for verification. GATE-01 to GATE-05 are each shown firing (or, for GATE-02, passing on the final tree). GATE-06 `cargo clean` ran last, and `target/` is absent.

## Self-Check: PASSED

- `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` exists and is tracked
- Commits 0f8bdd7, 6fece0d, b140c49 found in `git log`
- The working tree outside `.planning/` is clean, there is no `.cargo`, `Cargo.lock` matches HEAD, and `target/` is absent
