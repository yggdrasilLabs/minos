---
phase: 1
slug: workspace-and-gates
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-09
updated: 2026-10-09
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | The four gate commands themselves: rustfmt 1.9.0, clippy 0.1.97, and `cargo test` on rustc 1.97.1, plus cargo-deny 0.20.2. The stub crates contain no tests; `cargo test --workspace` passes with 0 tests per crate. |
| **Config file** | `Cargo.toml` (`[workspace.lints]`), `crates/minos-core/clippy.toml`, `rust-toolchain.toml` (Plan 01-01, wave 1); `deny.toml` (Plan 01-02, wave 2) |
| **Quick run command** | Wave 1: `cargo clippy --workspace --all-targets -- -D warnings`. From wave 2 on: `cargo clippy --workspace --all-targets -- -D warnings && cargo deny check bans sources` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check` |
| **Estimated runtime** | About 15 seconds cold (13 s measured in research from a clean target directory), under 2 seconds warm |

---

## Sampling Rate

- **After every task commit:** Run the quick run command for the current wave (deny.toml does not exist until wave 2)
- **After every plan wave:** Run the full suite command (wave 1 omits `cargo deny check`), and confirm `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -ci 'unknown lint'` prints 0
- **Before `/gsd-verify-work`:** Full suite green on the final tree; every D-11 proof recorded verbatim in `01-GATE-EVIDENCE.md`; tree reverted to HEAD outside `.planning`; then `cargo clean` (GATE-06). If verification runs cargo again afterwards, it ends with `cargo clean`.
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | GATE-01, GATE-05 | T-1-01, T-1-02, T-1-05 | Workspace lints (unsafe forbid; unwrap, expect, indexing, allow attributes denied) and core clock and hash-map bans are active for minos-core on rustc 1.97.1 | build/lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ (created by the task) | ⬜ pending |
| 1-01-02 | 01 | 1 | GATE-01 | T-1-03 | Six library crates inherit the workspace lints and declare only their ARCHITECTURE.md edges | build/lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ (created by the task) | ⬜ pending |
| 1-01-03 | 01 | 1 | GATE-01, GATE-05 | T-1-03, T-1-04, T-1-SC | All ten crates inherit lints, no misspelled lint, exact dependency map, no C, OpenSSL, or libgit2 crate in either target graph, Cargo.lock committed | build/lint/test | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | ✅ (created by the task) | ⬜ pending |
| 1-02-01 | 02 | 2 | GATE-03 | T-1-07 to T-1-10, T-1-12, T-1-13 | Sources, licenses, advisories, C bans, exact pins, and the 25-crate build-script allowlist pass the real graph | supply-chain | `cargo deny check` | ✅ (created by the task) | ⬜ pending |
| 1-02-02 | 02 | 2 | GATE-02, GATE-04 | T-1-11 | Every internal crate's allowed direct dependents are encoded; all four gate commands pass together | full gate | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check` | ✅ (created by the task) | ⬜ pending |
| 1-03-01 | 03 | 3 | GATE-01, GATE-05 | T-1-15, T-1-17, T-1-18 | V-01 to V-10 each fail clippy, the expect positive control passes, gaps G-01 to G-03 are demonstrated and left alone, tree reverted | gate self-test | `cargo clippy --workspace --all-targets -- -D warnings` (per proof, then on the reverted tree) | ✅ (evidence file created by the task) | ⬜ pending |
| 1-03-02 | 03 | 3 | GATE-03, GATE-04 | T-1-15, T-1-16, T-1-SC | V-11 to V-23 each fail cargo-deny (or Cargo's cycle check for V-20), no `.cargo` left, lockfile equals HEAD | gate self-test | `cargo deny check sources` / `cargo deny check bans` (per proof), then `cargo deny check` | ✅ (evidence file appended) | ⬜ pending |
| 1-03-03 | 03 | 3 | GATE-02 | T-1-18, T-1-19 | Gate wording matches D-01, four commands green on the final tree, CLAUDE.md and rules/ unchanged, open decisions recorded, `cargo clean` last | full gate + docs | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check`, then `cargo clean` and `test ! -d target` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. The phase is the test infrastructure: the gate is cargo, clippy, and cargo-deny, all installed (rustc 1.97.1, clippy 0.1.97, cargo-deny 0.20.2). D-10 forbids fixture crates, so there is no Wave 0 test scaffold.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Each D-11 violation fails its gate command | GATE-01, GATE-03, GATE-04, GATE-05 | D-10: one-time proof, with no fixture crate or committed probe. The commands are automated, but the violation edits are applied by hand and reverted, so they are not re-runnable from the repo | Follow the proof protocol in 01-03-PLAN.md. Re-run the affected rows of 01-GATE-EVIDENCE.md whenever Cargo.toml lints, crates/minos-core/clippy.toml, or deny.toml change |
| The three gaps the four commands cannot catch (an inner allow with a reason, a crate without `[lints] workspace = true`, `broken_intra_doc_links` outside `cargo doc`) | GATE-01, GATE-05 | Each needs a user decision; the phase must not resolve them by changing CLAUDE.md lint levels or adding a command | Read "Open decisions for the user" in 01-03-SUMMARY.md and choose an option for each |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (none)
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
