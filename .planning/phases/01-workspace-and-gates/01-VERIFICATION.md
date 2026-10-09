---
phase: 01-workspace-and-gates
verified: 2026-10-09T22:30:00Z
status: human_needed
score: 8/8 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps: []
human_verification:
  - test: "Decide open decision 1: an inner #![allow(clippy::..., reason = \"...\")] passes the clippy gate"
    expected: "User picks (A) accept and document the gap, or (B) raise unwrap_used, expect_used, indexing_slicing to forbid (edit to CLAUDE.md Code rules). ROADMAP SC2 and GATE-05 say 'no allow(clippy::...) appears anywhere'; the gate enforces that only for outer attributes and for reasonless inner ones."
    why_human: "Changing a CLAUDE.md lint level is a design-of-record decision, not something a verifier or executor may do. Evidence G-01 (and my own reproduction) shows clippy exits 0."
  - test: "Decide open decision 2: a crate that omits [lints] workspace = true escapes every workspace lint"
    expected: "User picks (A) one-time check plus manifest review for every new crate, or (B) a .cargo/config.toml rustflags backstop. Today all 10 manifests carry the table (re-verified below)."
    why_human: "The fix choice adds a policy file (.cargo/config.toml) that D-01 did not list, or relies on review. Evidence G-02 shows clippy exits 0 for such a crate."
  - test: "Decide open decision 3: [workspace.lints.rustdoc] broken_intra_doc_links = deny is enforced only by cargo doc"
    expected: "User picks (A) add `cargo doc --workspace --no-deps` as a fifth gate command (changes CLAUDE.md Commands and ROADMAP SC1), or (B) document that the rustdoc table only takes effect when docs are built."
    why_human: "Adding a gate command changes the design of record. Evidence G-03: clippy and test exit 0, cargo doc exits 101."
---

# Phase 1: Workspace and Gates Verification Report

**Phase Goal:** Every line of code in the repo, from the first commit, is held to the workspace lints, the supply-chain policy, and the crate-layering rules by four plain cargo commands (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`) on rustc 1.97.1 with edition 2024, with all policy in Cargo-native configuration files and no script.
**Verified:** 2026-10-09
**Status:** human_needed (all must-haves verified; three documented gate gaps need a user decision; none was resolved or hidden)
**Re-verification:** No, initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria first, then PLAN must-haves)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC1: On the ten-crate skeleton, the four commands pass on rustc 1.97.1 / edition 2024 with no script, and Cargo.lock is committed | VERIFIED | I ran them on HEAD fb098a5: `cargo --version` = 1.97.1, `cargo fmt --all --check` exit 0, `cargo clippy --workspace --all-targets -- -D warnings` exit 0, `cargo test --workspace` exit 0 (17 `test result: ok`, 0 tests), `cargo deny check` exit 0 with `advisories ok, bans ok, licenses ok, sources ok`. `git ls-files Cargo.lock` lists it (207 packages). `rust-toolchain.toml` pins 1.97.1; `edition = "2024"`, `resolver = "3"`. Tree outside `.planning` matches HEAD. |
| 2 | SC2: Each code violation makes clippy fail: `unsafe`, `unwrap`, `expect`, slice index, `allow(clippy::...)` (only `#[expect]` passes), `SystemTime::now` and `HashMap` in minos-core | VERIFIED (with caveat, see truth 8 and Human Verification 1) | `01-GATE-EVIDENCE.md` V-01..V-10 have verbatim output and exit codes: V-01 exit 101 (`usage of an \`unsafe\` block`, `-F unsafe-code`; this is the backticked form the task notes expected); V-02/V-03 `unwrap_used`/`expect_used`; V-04 `indexing may panic`; V-05 outer allow without reason (two errors); V-06 outer allow with reason still fails (`#[allow] attribute found`); V-07 `#[expect(..., reason)]` exits 0 (positive control); V-08 unfulfilled expectation fails; V-09 `disallowed method std::time::SystemTime::now` with note `minos-core has no clock`; V-10 `disallowed type std::collections::HashMap`. Independent reproduction in a scratchpad copy (not the repo): V-09 reproduced (`use of a disallowed method`, exit 101). |
| 3 | SC3: Each supply-chain or layering violation makes `cargo deny check` fail: git dep without rev, unknown registry/git source, unlisted build script, core/loop to sandbox or backend; normal core->sandbox fails earlier as a Cargo cycle | VERIFIED | Evidence V-11 (`git-source-underspecified`, exit 8), V-12 (`source-not-allowed` git, exit 8), V-13 (`source-not-allowed` registry, exit 8), V-14 (`build-script-not-allowed` httparse, exit 2), V-15/16/17 (loop to sandbox/backend/runner, `explicitly banned`, exit 2), V-18/19 (core dev-edge to sandbox/backend, banned, exit 2), V-20 (normal edge: `cyclic package dependency`, deny exit 1 and clippy exit 101), V-21 anyhow, V-22 msb_krun, V-23 (`cc`, `zstd-sys` explicitly banned via backhand zstd feature, exit 2; the expected result, not the UNEXPECTED path). Independent reproduction of V-15 in a scratchpad copy: `error[banned]: crate 'minos-sandbox = 0.1.0' is explicitly banned`, `bans FAILED`. |
| 4 | SC4 / GATE-06: `cargo clean` has run after verification passes | VERIFIED | Plan 01-03 ran `cargo clean` (evidence: Removed 4871 files). A `target/` (981 MB) reappeared at 17:40 from a later cargo run, and my own gate re-runs also used it. I ran `cargo clean` as the last cargo action of this verification (Removed 3814 files, 1.1GiB); `test ! -d target` confirmed. No cargo command was run after it. |
| 5 | Plan 01-01: ten crates exist with exactly the ARCHITECTURE.md dependency edges; every member has `[lints] workspace = true`; only minos-core has a clippy.toml | VERIFIED | `cargo metadata --no-deps` normal/dev edges match the plan's ten-line map exactly (re-run by me). All 10 manifests pass the `[lints]`/`workspace = true` check (re-run). `find` shows exactly one clippy.toml (`crates/minos-core/clippy.toml`, bans `SystemTime::now`, `Instant::now`, `HashMap`, `HashSet`); no root clippy.toml. |
| 6 | Plan 01-01: lint baseline in `[workspace.lints]` matches CLAUDE.md and D-02/D-12; every `[workspace.dependencies]` entry is an exact `=` pin | VERIFIED | Cargo.toml: `unsafe_code = "forbid"`, `unwrap_used`/`expect_used`/`indexing_slicing = "deny"`, `allow_attributes` and `allow_attributes_without_reason = "deny"`, seven groups at priority -1, four carve-outs each with a comment reason. Workspace dependency block has 31 entries, 31 contain `"=` (exact pins, internal path deps included). No C/OpenSSL/libgit2 crate in either target graph (`cargo tree` grep count 0). LICENSE-APACHE sha256 matches the plan's `cfc7749b...`; LICENSE-MIT present. |
| 7 | Plan 01-02: deny.toml encodes sources (rev required, `allow-git = []`, unknown registry/git denied), a 25-crate build-script allowlist plus `executables = "deny"`, 9 C bans, anyhow->minos-cli, msb_krun->minos-vmm, and seven per-crate layering `wrappers` entries | VERIFIED | Read deny.toml in full. Layering wrappers match the plan literally and match the real dependency edges (checked against the metadata output above): core listed for 8 dependents, loop only minos-cli, runner only minos-cli, etc. `[bans.workspace-dependencies]` has duplicates/unused deny and `include-path-dependencies = true`. `multiple-versions = "warn"` produces 7 duplicate warnings (bitflags, getrandom, miniz_oxide, nix, rand_chacha, rand_core, syn), visible but not failing, as designed. |
| 8 | Plan 01-03: the three gaps the four commands cannot catch are recorded as open decisions, not resolved; CLAUDE.md and rules/ unchanged; REQUIREMENTS/ROADMAP reworded to match D-01 | VERIFIED (the gaps themselves are real, see Human Verification) | G-01 (inner allow with reason, exit 0), G-02 (crate without `[lints]`, exit 0), G-03 (rustdoc lint: clippy and test exit 0, `cargo doc` exit 101 `unresolved link`) are recorded verbatim, and the `## Open decisions for the user` section lists three numbered decisions. I reproduced G-01 independently in a scratchpad copy: clippy exits 0 with an inner `#![allow(clippy::unwrap_used, reason = "x")]` plus an `unwrap()`. `git diff --quiet HEAD -- CLAUDE.md rules` = 0 and `git log 7195911..HEAD -- CLAUDE.md rules` empty. REQUIREMENTS.md GATE-02/04/05 and ROADMAP Phase 1 text carry the new wording; zero `` `ci` `` or `A CI check` remain; the Phase 5 xtask wording (KRUN-08, ROADMAP line 153) was deliberately left alone per CONTEXT.md. |

**Score:** 8/8 truths verified (0 behavior-unverified, 0 overrides). Status is human_needed because the user has three pending decisions about gaps that weaken the literal wording of GATE-01/GATE-05, not because any must-have failed.

### Decision D-01 (Cargo-native, no scripts)

Confirmed. `find` (excluding target, .git, rules) finds no `*.sh`, Makefile, justfile, xtask, `.github`, `.cargo`, or `ci*`. `rules/` contains no `.sh` either. `.gitignore` is the single line `/target`. `crates/` has exactly the ten planned members and no xtask or AWS crate (D-07). All policy lives in `Cargo.toml`, `crates/minos-core/clippy.toml`, `deny.toml`, `rust-toolchain.toml`, `rustfmt.toml`.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `rust-toolchain.toml` | channel 1.97.1, rustfmt+clippy | VERIFIED | Content as planned, no targets (D-09) |
| `Cargo.toml` | workspace, lint baseline, 31 exact pins | VERIFIED | See truth 6 |
| `crates/minos-core/clippy.toml` | clock and hash-order bans | VERIFIED | Real std paths; proven live by V-09/V-10 |
| `Cargo.lock` | committed | VERIFIED | Tracked, 207 packages |
| `crates/minos-cli/Cargo.toml` | `minos` bin | VERIFIED | Binary set is `minos`, `minos-guest`, `minos-vmm` |
| `LICENSE-APACHE` / `LICENSE-MIT` | D-08 | VERIFIED | Hash matches; MIT text present |
| `deny.toml` | contains `required-git-spec = "rev"` | VERIFIED | Full policy, see truth 7 |
| `01-GATE-EVIDENCE.md` | verbatim proofs, `## Open decisions for the user` | VERIFIED | 23 `### V-`, 3 `### G-`, 3 `### C-`; 11 `Exit code: 0`, 24 non-zero exit-code lines (includes the extra clippy run in V-20 and the Deny re-check/Final gate zeros) |
| `.planning/REQUIREMENTS.md`, `ROADMAP.md` | D-01 wording | VERIFIED | Greps pass |

Stub-level note: the crate sources are one-line doc stubs by design (Phase 1 is a skeleton phase). That is the specified deliverable, not a stub defect; no `TBD/FIXME/XXX/TODO` markers exist in `crates/`, `Cargo.toml`, or `deny.toml`.

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/*/Cargo.toml` | `[workspace.lints]` | `[lints] workspace = true` | WIRED | 10 of 10 (C-01 plus my re-run) |
| `crates/minos-core/clippy.toml` | `cargo clippy` | nearest clippy.toml | WIRED | V-09/V-10 fire; I reproduced V-09 |
| member deps | `[workspace.dependencies]` | `.workspace = true` | WIRED | `[bans.workspace-dependencies]` duplicates/unused = deny and passes |
| `deny.toml` wrappers | crate dependency edges | one deny entry per internal lib | WIRED | Matches metadata; V-15..V-19 fire |
| `deny.toml [graph]` | Cargo.lock for both triples | `targets` | WIRED | deny passes on both; `cargo tree` clean for both |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Four gate commands pass on HEAD | fmt / clippy / test / deny | all exit 0 | PASS |
| `SystemTime::now` in core rejected | probe appended in a scratchpad copy, clippy | `use of a disallowed method`, exit 101 | PASS |
| loop -> sandbox edge rejected | edge added in a scratchpad copy, `cargo deny check bans` | `crate 'minos-sandbox = 0.1.0' is explicitly banned` | PASS |
| Gap G-01 real | inner allow + unwrap in a scratchpad copy | clippy `Finished`, exit 0 | CONFIRMED GAP |

All violation reproductions were made in a copy under the session scratchpad, not in the repository; the copy and its target dir were deleted. The repo tree is unchanged outside `.planning`.

### Probe Execution

Step 7c: SKIPPED. The phase declares no `probe-*.sh` scripts (D-01 forbids scripts); the proof mechanism is the evidence file plus my reproductions above.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GATE-01 | 01-01, 01-03 | rustc 1.97.1 / edition 2024, `unsafe_code = forbid`, unwrap/expect/indexing denied | SATISFIED, with the G-02 and G-03 caveats | Cargo.toml, V-01..V-04, truths 1, 2, 6 |
| GATE-02 | 01-02, 01-03 | Four plain cargo commands pass, Cargo.lock committed | SATISFIED | Truth 1 (run by me) |
| GATE-03 | 01-02, 01-03 | deny.toml: rev required, unknown registry/git denied, build scripts allowlisted | SATISFIED | deny.toml, V-11..V-14 |
| GATE-04 | 01-02, 01-03 | `cargo deny check` fails on core/loop -> sandbox or backend | SATISFIED | V-15..V-20 and reproduced V-15 |
| GATE-05 | 01-01, 01-03 | core bans `SystemTime::now`/`HashMap`; no `allow(clippy::...)` anywhere | SATISFIED for outer attributes and reasonless inner ones; PARTIAL for inner `#![allow(..., reason)]` (G-01) | V-05..V-10, C-02, G-01 |
| GATE-06 | cross-cutting | `cargo clean` after verification | SATISFIED | Ran `cargo clean` last; `target/` absent |

All five phase IDs (GATE-01..05) appear in at least one PLAN `requirements:` field (01-01: 01,05; 01-02: 02,03,04; 01-03: all five) and all five are mapped to Phase 1 in REQUIREMENTS.md. No orphaned requirements. REQUIREMENTS.md still shows GATE-06 unticked, which is correct until the orchestrator marks the phase complete.

### Anti-Patterns Found

None blocking. No debt markers, no placeholder logic beyond the intended doc-only stubs, no `#[allow]` anywhere in `crates` (C-02 re-checked: grep empty), no scripts.

Observations (info):

| Item | Severity | Impact |
|------|----------|--------|
| Plan acceptance greps expected `usage of an unsafe block`; clippy 0.1.97 prints ``usage of an `unsafe` block`` | Info | Evidence has the backticked form in V-01 and the gate did reject (exit 101); the plan grep was wrong, not the gate |
| `cargo deny check` prints 7 `multiple-versions` warnings | Info | By design (`warn`), clippy's `multiple_crate_versions` is carved out |
| A 981 MB `target/` existed at the start of verification despite 01-03's `cargo clean` | Info | A later cargo run recreated it; cleaned again by this verification |
| `01-VALIDATION.md` frontmatter still says `status: draft` and the per-task table shows pending | Info | Documentation bookkeeping only |

### Human Verification Required

#### 1. Inner `#![allow(..., reason)]` passes the clippy gate (open decision 1, G-01)

**Test:** Decide between (A) accept and document, or (B) raise `unwrap_used`, `expect_used`, `indexing_slicing` to `forbid` (edits CLAUDE.md Code rules, and makes `#[expect]` unusable for those three).
**Expected:** A recorded decision. Until then, GATE-05's "no allow(clippy::...) anywhere" and ROADMAP SC2's "only `#[expect]` passes" are enforced for outer attributes and reasonless inner attributes only.
**Why human:** Changing a locked CLAUDE.md lint level is the user's call.

#### 2. A crate without `[lints] workspace = true` escapes all lints (open decision 2, G-02)

**Test:** Decide between (A) manifest review plus the one-time C-01 check, or (B) a `.cargo/config.toml` rustflags backstop.
**Expected:** A recorded decision. Today all ten manifests are compliant.
**Why human:** Option B adds a policy file D-01 did not list and duplicates the lint list.

#### 3. rustdoc `broken_intra_doc_links = deny` is not enforced by the four commands (open decision 3, G-03)

**Test:** Decide between (A) add `cargo doc --workspace --no-deps` as a fifth command, or (B) document that the rustdoc lint takes effect only on doc builds.
**Expected:** A recorded decision. D-12 listed this lint in the baseline, but only `cargo doc` runs it.
**Why human:** Adding a gate command changes CLAUDE.md Commands and ROADMAP SC1.

### Gaps Summary

No must-have failed. The phase delivers what it set out to: ten-crate workspace, full lint baseline, 31 exact pins, a deny.toml whose sources, build-script, C-crate, and layering rules each have verbatim failing output (and two of which I reproduced myself), and the four gate commands green on HEAD. Decision D-01 (no scripts) is honored. The residual risk is the three gate holes the executor found and surfaced instead of papering over; they await the user's choice and do not block starting Phases 2 and 3, but decision 2 is cheapest to settle before new crates arrive in later phases.

GATE-06 note: this verification ran cargo commands (gate re-runs, `cargo metadata`, `cargo tree`, `cargo deny`, scratchpad reproductions) and then ran `cargo clean` as its final cargo action. `target/` does not exist. Nothing in the repository was modified by the verifier except this file.

---

_Verified: 2026-10-09_
_Verifier: Claude (gsd-verifier)_
