---
phase: 01-workspace-and-gates
verified: 2026-10-09T23:30:00Z
status: passed
score: 12/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: human_needed
  previous_score: 8/8
  gaps_closed:
    - "UAT test 1 / G-01: inner #![allow(clippy::unwrap_used, reason)] passed the clippy gate (now E0453 under forbid)"
    - "UAT test 2 / G-02: a member without [lints] workspace = true escaped the workspace lints (now caught by the .cargo/config.toml rustflags backstop)"
    - "UAT test 3 / G-03: [workspace.lints.rustdoc] was enforced by no gate command (now enforced by the fifth command, cargo doc --workspace --no-deps)"
  gaps_remaining: []
  regressions: []
gaps: []
human_verification: []
---

# Phase 1: Workspace and Gates Verification Report

**Phase Goal:** Every line of code in the repo, from the first commit, is held to the workspace lints, the supply-chain policy, and the crate-layering rules by five plain cargo commands (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`), with all policy in Cargo-native configuration files and no script.
**Verified:** 2026-10-09, HEAD 8f035fa (working tree clean except `.planning/config.json`, which this phase did not touch)
**Status:** passed
**Re-verification:** Yes, after gap-closure plan 01-04 (the previous report was `human_needed` with three pending user decisions; all three were decided in 01-UAT.md and implemented)

## Goal Achievement

### Observable Truths

ROADMAP success criteria first, then the 01-04 must-haves that carry the UAT decisions. Plan 01-01 to 01-03 must-haves were re-checked as regressions (quick check) and are summarised at the end of the table.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC1: on the ten-crate skeleton, all five commands pass on rustc 1.97.1 / edition 2024 with no script, and `Cargo.lock` is committed | VERIFIED | I ran all five on HEAD (`cargo 1.97.1`; no `RUSTFLAGS`/`RUSTDOCFLAGS`/`CARGO_ENCODED_RUSTFLAGS` set): `cargo fmt --all --check` exit 0, `cargo clippy --workspace --all-targets -- -D warnings` exit 0, `cargo test --workspace` exit 0 (all 0-test suites ok), `cargo deny check` exit 0 (`advisories ok, bans ok, licenses ok, sources ok`), `cargo doc --workspace --no-deps` exit 0 (`Generated .../minos_backend_krun/index.html and 9 other files`). `git ls-files` lists `Cargo.lock` and `.cargo/config.toml`. No `*.sh`, Makefile, justfile, xtask, or `.github` exists outside `target/`, `.git/`, `rules/`. |
| 2 | SC2 / UAT 1: `unwrap_used`, `expect_used`, `indexing_slicing` are at `forbid` in `Cargo.toml` and in the CLAUDE.md Code rules; an inner `#![allow(clippy::unwrap_used, reason)]` plus `unwrap()` fails clippy | VERIFIED | `Cargo.toml` `[workspace.lints.clippy]` lines 107-109 equivalent: `unwrap_used = "forbid"`, `expect_used = "forbid"`, `indexing_slicing = "forbid"` (and `unsafe_code = "forbid"`). CLAUDE.md lines 106-112 show the same three at `forbid` plus the sentence that neither allow nor expect can silence them. `git diff c4b5bab HEAD -- CLAUDE.md` touches exactly two hunks (the three lint levels plus that one sentence, and the new `cargo doc` Commands line); `rules/` is unchanged. I reproduced the G-01 probe in a scratch copy of the tree (separate target dir, repo untouched): inner allow with reason plus `unwrap()` in `minos-store` gives `error[E0453]: allow(clippy::unwrap_used) incompatible with previous forbid`, exit 101. Evidence `G-01-CLOSED` records the same verbatim. |
| 3 | SC2: `#[expect]` of a forbidden lint now fails; `#[expect]` of other lints still passes; the residual inner-allow gap on non-forbidden lints is stated, not hidden | VERIFIED | My scratch probe: `#[expect(clippy::indexing_slicing, reason)]` on indexing code gives E0453, exit 101 (matches evidence R-01). Evidence R-02 (`#[expect(clippy::get_first, reason)]` exit 0) and R-03 (inner `#![allow(clippy::get_first, reason)]` exit 0) are recorded verbatim. ROADMAP SC2 and REQUIREMENTS GATE-05 state the residual explicitly ("an inner `#![allow(..., reason = ...)]` is not caught" for other lints), so the wording is honest. No `allow(`/`expect(`/`unsafe`/`unwrap`/TODO/FIXME/TBD/XXX in any `crates/**/*.rs` (16 lines of stub code in total). |
| 4 | UAT 2: `.cargo/config.toml` exists with the `[build] rustflags` forbid backstop, and a member that omits `[lints] workspace = true` still fails clippy | VERIFIED | `.cargo/config.toml` (tracked) contains `[build] rustflags = ["-Funsafe_code", "-Fclippy::unwrap_used", "-Fclippy::expect_used", "-Fclippy::indexing_slicing"]`, with comments naming the duplication, the `RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` / `target.*.rustflags` replacement caveat, and that dependencies are unaffected (`--cap-lints allow`). My scratch probe deleted `[lints]`/`workspace = true` from `minos-loop` and added `unwrap()` and `x[0]`: both fail with `requested on the command line with -F clippy::unwrap-used` / `-F clippy::indexing-slicing`, exit 101. The untouched tree passes all five commands, so no dependency fails under the flags (evidence B-01 also records `cargo build`, `test`, `deny` at exit 0). The `RUSTFLAGS=` weakness is demonstrated in evidence G-02-CLOSED (exit 0 with it set) and recorded as a caveat in D-01, not hidden. |
| 5 | UAT 2 (option A): every crate manifest inherits the workspace lints, and a standing acceptance criterion exists for new crates | VERIFIED | The C-01 loop over `crates/*/Cargo.toml` prints `ok` for all ten crates (I re-ran it) and no `MISSING`. D-15 in 01-CONTEXT.md adds the acceptance criterion with the exact command. |
| 6 | UAT 3: `cargo doc --workspace --no-deps` is the fifth gate command in CLAUDE.md Commands, ROADMAP Phase 1, REQUIREMENTS GATE-02, and 01-CONTEXT.md D-01, and it fails on a broken intra-doc link | VERIFIED | CLAUDE.md Commands line 414: `cargo doc --workspace --no-deps          # the only command that enforces [workspace.lints.rustdoc]`. ROADMAP Phase 1 goal and SC1 name all five commands; REQUIREMENTS GATE-02 lists all five; 01-CONTEXT.md D-01 lists five commands and five policy files and adds D-15. My scratch probe: appended `/// Link to [\`DoesNotExist\`].` gives `error: unresolved link to DoesNotExist ... -D rustdoc::broken-intra-doc-links`, exit 101, while `cargo clippy` on the same tree exits 0 (so the fifth command is genuinely needed). No `RUSTDOCFLAGS` is set and `.cargo/config.toml` has no `rustdocflags` key. `Cargo.toml` comment on the rustdoc table names the fifth command. |
| 7 | UAT: the `## Gap closure (01-04)` section of `01-GATE-EVIDENCE.md` records `G-01-CLOSED`, `G-02-CLOSED`, `G-03-CLOSED` with verbatim output | VERIFIED | Section at line 6779; headings `G-01-CLOSED` (6790, exit 101, E0453 text), `G-02-CLOSED` (6940, exit 101 with four errors for unsafe/unwrap/expect/index, plus a second run with `RUSTFLAGS=` empty at exit 0), `G-03-CLOSED` (7078, exit 0 on the clean tree, exit 101 with the unresolved-link error). Also `R-01..R-03`, `F-01..F-03`, `C-03-RERUN`, `B-01`, `C-01-RERUN`, a five-command "Final gate", "Invariant check (01-04)", and "cargo clean (01-04)". Every error text I reproduced matches the recorded text. Files are genuinely probed-and-reverted: `git status` shows no source/policy change. |
| 8 | SC3: each supply-chain or layering violation makes `cargo deny check` fail (regression check) | VERIFIED | `deny.toml` unchanged by 01-04: `required-git-spec = "rev"`, `allow-git = []`, `unknown-registry/unknown-git = "deny"`, `[bans.build] allow-build-scripts` list plus `executables = "deny"`, nine C/git crate bans, anyhow limited to `minos-cli`, `msb_krun` to `minos-vmm`, and seven per-crate layering `wrappers` entries (core, store, sandbox, backend-krun, propose, runner, loop). `cargo deny check` passes live. Evidence V-11..V-23 (verbatim failing outputs) are untouched. |
| 9 | SC2: `SystemTime::now` and `HashMap` in `minos-core` are rejected (regression check) | VERIFIED | `crates/minos-core/clippy.toml` has `disallowed-methods` for `SystemTime::now` and `Instant::now` and `disallowed-types` for `HashMap` and `HashSet`; only minos-core has a clippy.toml. Evidence V-09 and V-10 untouched. |
| 10 | Plan 01-01/01-02 structure: ten crates with the planned dependency edges, resolver 3, edition 2024, rust-version 1.97.1, exact `=` pins, toolchain pinned | VERIFIED | `crates/` has exactly the ten planned members and no xtask/AWS crate. `Cargo.toml` shows resolver 3, edition 2024, rust-version 1.97.1, every `[workspace.dependencies]` entry `=`-pinned (internal paths carry `version = "=0.1.0"`). `rust-toolchain.toml` pins 1.97.1 with rustfmt and clippy. `cargo deny` `[bans.workspace-dependencies]` (`duplicates`, `unused`) passes. |
| 11 | CLAUDE.md and `rules/` are not weakened: only the user-decided edits were made | VERIFIED | The CLAUDE.md diff above is the whole change since the plan baseline c4b5bab, and both edits are the user's UAT decisions B (test 1) and A (test 3). `git diff --stat c4b5bab HEAD -- rules` is empty. No lint was lowered; three were raised. |
| 12 | GATE-06: `cargo clean` ran last and no `target/` remains | VERIFIED | I ran `cargo clean` as the final cargo command of this verification ("Removed 4103 files, 1.1GiB total") and `ls target` reports no such directory. |

**Score:** 12/12 truths verified (0 behavior-unverified, 0 overrides). None of the truths is a runtime state-transition or cleanup invariant; they are all configuration-enforcement claims, and each was proven by a live failing-then-reverted probe, not by presence alone.

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `Cargo.toml` | `unwrap_used`, `expect_used`, `indexing_slicing` at forbid; `unsafe_code` forbid; rustdoc table naming the fifth command | VERIFIED | Read in full. Carve-outs (`too_many_lines`, `similar_names`, `struct_excessive_bools`, `multiple_crate_versions`) each carry a rules/-cited reason comment. |
| `.cargo/config.toml` | `[build] rustflags` forbid backstop with caveats | VERIFIED | Tracked in git, 8 lines, contains `-Fclippy::indexing_slicing`. |
| `CLAUDE.md` | Code rules at forbid + one sentence; Commands with fifth command | VERIFIED | Exactly two diff hunks, see truth 2 and 6. |
| `deny.toml` | Supply-chain and layering policy | VERIFIED | See truth 8. |
| `crates/minos-core/clippy.toml` | Clock and hash-order bans | VERIFIED | See truth 9. |
| `rust-toolchain.toml`, `rustfmt.toml`, `LICENSE-*`, `Cargo.lock` | Toolchain pin, fmt config, licenses, lockfile | VERIFIED | Present; `Cargo.lock` tracked. |
| `01-GATE-EVIDENCE.md` (Gap closure 01-04) | Verbatim closing proofs | VERIFIED | See truth 7. |
| `01-CONTEXT.md` D-01 (amended), D-15 (new) | Five files, five commands, caveats; new-crate criterion | VERIFIED | Lines 21 and 43. |
| `REQUIREMENTS.md` GATE-01/02/05, `ROADMAP.md` Phase 1 | Five commands, forbid, backstop | VERIFIED | GATE-01 to GATE-05 are `[x]` and `Complete` in the traceability table; GATE-06 stays `[ ]`/Pending by design (cross-cutting). |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `Cargo.toml [workspace.lints.clippy]` | `cargo clippy ... -D warnings` | forbid turns allow/expect into E0453 | WIRED | Reproduced (P1, P4). |
| `.cargo/config.toml [build] rustflags` | clippy on a manifest lacking `[lints]` | build.rustflags reach every workspace crate | WIRED | Reproduced (P2). |
| `CLAUDE.md Commands` / `Cargo.toml [workspace.lints.rustdoc]` | `cargo doc --workspace --no-deps` | only rustdoc applies the rustdoc table | WIRED | Reproduced (P3, with clippy passing on the same tree). |
| `crates/*/Cargo.toml [lints]` | `Cargo.toml [workspace.lints]` | `workspace = true` in all ten members | WIRED | C-01 loop, ten `ok`. |
| `deny.toml` wrappers | crate dependency edges | per-crate `wrappers` lists | WIRED | `cargo deny check` passes; violations recorded V-15..V-20. |

### Data-Flow Trace (Level 4)

Not applicable: this phase produces configuration and ten stub crates that render no dynamic data.

### Behavioral Spot-Checks

Run in a scratch copy of the tree under `$scratchpad` with its own `CARGO_TARGET_DIR`; the repository was never edited. The scratch copy was deleted afterwards.

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Inner allow of a forbidden lint is rejected | `cargo clippy --workspace --all-targets -- -D warnings` with `#![allow(clippy::unwrap_used, reason)]` + `unwrap()` | E0453 "overruled by previous forbid", exit 101 | PASS |
| `#[expect]` of a forbidden lint is rejected | same, `#[expect(clippy::indexing_slicing, reason)]` | E0453, exit 101 | PASS |
| Crate without `[lints]` is still held | same, `minos-loop` manifest stripped, `unwrap()` and `x[0]` added | two clippy errors citing `-F`, exit 101 | PASS |
| Broken doc link fails the fifth command only | `cargo doc --workspace --no-deps` / clippy | doc exit 101 "unresolved link"; clippy exit 0 | PASS |
| Clean-tree gate | five commands on HEAD | all exit 0 | PASS |

### Probe Execution

SKIPPED: the phase declares no probe scripts, and no `scripts/*/tests/probe-*.sh` exists.

### Requirements Coverage

Requirement IDs declared in PLAN frontmatter: 01-01 `[GATE-01, GATE-05]`, 01-02 `[GATE-02, GATE-03, GATE-04]`, 01-03 `[GATE-01..GATE-05]`, 01-04 `[GATE-01, GATE-02, GATE-05]`. REQUIREMENTS.md maps GATE-01..GATE-05 to Phase 1 and GATE-06 to all phases. Every ID is accounted for; no orphans.

| Requirement | Source Plan | Description (current wording) | Status | Evidence |
| ----------- | ----------- | ----------------------------- | ------ | -------- |
| GATE-01 | 01-01, 01-03, 01-04 | rustc 1.97.1 / edition 2024; `unsafe_code`, `unwrap_used`, `expect_used`, `indexing_slicing` all forbid, repeated as a `.cargo/config.toml` backstop | SATISFIED | Truths 2, 4, 5, 10 |
| GATE-02 | 01-02, 01-03, 01-04 | five plain cargo commands pass, no script, `Cargo.lock` committed | SATISFIED | Truth 1 and 6 |
| GATE-03 | 01-02, 01-03 | `deny.toml` requires `rev`, denies unknown registries/git, allowlists build scripts | SATISFIED | Truth 8 |
| GATE-04 | 01-02, 01-03 | `cargo deny check` fails on a core/loop dependency on sandbox or backends | SATISFIED | Truth 8 (V-15..V-20) |
| GATE-05 | 01-01, 01-03, 01-04 | core `disallowed-methods`/`disallowed-types`; no `allow(clippy::...)` anywhere, and not even `#[expect]` for the three forbid lints | SATISFIED | Truths 2, 3, 9 |
| GATE-06 | cross-cutting | `cargo clean` after verification | SATISFIED for this phase | Truth 12; the REQUIREMENTS checkbox stays open because it applies to every phase |

### Anti-Patterns Found

None blocking. Scan of `crates/**/*.rs` for `allow(`, `expect(`, `unsafe`, `unwrap`, `TODO`, `FIXME`, `XXX`, `TBD` returned nothing; the stubs are 16 lines in total, and stubs are the intended Phase 1 deliverable.

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `01-GATE-EVIDENCE.md` older sections ("Final gate", "Invariant check", "Decisions") | various | say "four gate commands" / "No gate command was added" | Info | These are the pre-closure before-state records; the post-closure state is in "Gap closure (01-04)", which says so explicitly. No correction needed. |
| `.planning/ROADMAP.md` | 79 | 01-02 plan line says "all four gate commands green" | Info | Historically accurate for plan 01-02 (the fifth command did not exist yet). |
| `deny.toml` | 1 | "`cargo deny check` is the fourth gate command" | Info | Still correct in the five-command ordering. |

### Findings carried forward (not Phase 1 gaps)

These are recorded so they are not lost; neither blocks Phase 1, and both were already surfaced by plan 01-04 and STATE.md.

1. **F-01 (Phase 8 / any earlier minos-cli argument parsing):** `#[derive(clap::Parser)]` fails the clippy gate under `forbid`, because clap_derive 4.6.7 emits `#[allow(clippy::restriction)]`, which E0453 rejects. The clap builder API passes (F-02) and serde/thiserror derives pass (F-03). This is a direct consequence of UAT decision B and conflicts with the `.claude/CLAUDE.md` stack choice "clap 4.6.7 with `derive`". The user must choose between the builder API and revisiting forbid before any clap derive is written. `[workspace.dependencies]` still enables clap's `derive` feature and `cargo deny` `unused` passes. STATE.md line 96 holds the note.
2. **Residual, accepted by the user and stated in ROADMAP SC2 / GATE-05:** an inner `#![allow(..., reason = ...)]` on a lint that is not one of the forbid lints is not caught by the gate (evidence R-03). Only the four forbid lints are airtight.
3. **Backstop weakness, accepted and documented (D-01):** a `RUSTFLAGS` environment variable (or `CARGO_ENCODED_RUSTFLAGS`, or a `target.*.rustflags` key) silently replaces the `.cargo/config.toml` backstop. The primary control remains the `[lints] workspace = true` table in every manifest, guarded by the D-15 acceptance criterion. Phase 5 will want to re-check this when it adds the `aarch64-unknown-linux-musl` target.

### Human Verification Required

None. The three decisions that made the previous report `human_needed` were taken by the user (01-UAT.md tests 1 to 3) and implemented and proven by plan 01-04.

### Gaps Summary

No gaps. All three UAT gaps are closed with reproducible evidence: the three clippy lints are at forbid in `Cargo.toml` and CLAUDE.md, the `.cargo/config.toml` backstop exists and fires on a manifest that omits `[lints]`, and `cargo doc --workspace --no-deps` is the documented, working fifth gate command in CLAUDE.md, ROADMAP, REQUIREMENTS GATE-02, and 01-CONTEXT.md D-01 (with D-15 added). All five gate commands pass on HEAD 8f035fa, and `cargo clean` ran last (no `target/` remains).

---

_Verified: 2026-10-09_
_Verifier: Claude (gsd-verifier)_
