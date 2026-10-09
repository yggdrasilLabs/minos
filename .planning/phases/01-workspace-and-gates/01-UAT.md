---
status: resolved
phase: 01-workspace-and-gates
source: [01-VERIFICATION.md]
started: 2026-10-09T21:44:23Z
updated: 2026-10-09T22:43:43Z
---

## Current Test

[testing complete]

## Tests

### 1. Decide open decision 1: an inner `#![allow(clippy::..., reason = "...")]` passes the clippy gate
expected: User picks (A) accept and document the gap, or (B) raise unwrap_used, expect_used, indexing_slicing to forbid (an edit to the CLAUDE.md Code rules). ROADMAP SC2 and GATE-05 say "no allow(clippy::...) appears anywhere"; today the gate enforces that only for outer attributes and for reasonless inner ones. Evidence: G-01.
result: issue
reported: "B"
decision: "B: raise unwrap_used, expect_used, indexing_slicing to forbid"
severity: major

### 2. Decide open decision 2: a crate that omits `[lints] workspace = true` escapes every workspace lint
expected: User picks (A) a one-time check plus manifest review for every new crate, or (B) a .cargo/config.toml rustflags backstop (a policy file D-01 did not list). All 10 current manifests carry the table. Evidence: G-02.
result: issue
reported: "both I think, no?"
decision: "A+B: keep the per-manifest [lints] check and new-crate manifest review, and add a .cargo/config.toml rustflags backstop for the four core invariants"
severity: major

### 3. Decide open decision 3: `[workspace.lints.rustdoc] broken_intra_doc_links = deny` is enforced only by `cargo doc`
expected: User picks (A) add `cargo doc --workspace --no-deps` as a fifth gate command (changes the CLAUDE.md Commands section and ROADMAP SC1), or (B) document that the rustdoc lint table only takes effect when docs are built. Evidence: G-03 (clippy and test exit 0, cargo doc exits 101).
result: issue
reported: "a"
decision: "A: add cargo doc --workspace --no-deps as a fifth gate command"
severity: major

## Summary

total: 3
passed: 0
issues: 3
pending: 0
skipped: 0
blocked: 0

## Gaps

- truth: "An inner #![allow(clippy::unwrap_used, reason = ...)] cannot pass cargo clippy --workspace --all-targets -- -D warnings (GATE-05: no allow(clippy::...) appears anywhere)"
  status: resolved
  reason: "User reported: B (raise unwrap_used, expect_used, indexing_slicing to forbid)"
  severity: major
  test: 1
  root_cause: "clippy::allow_attributes lints only outer #[allow]; an inner #![allow(..., reason)] on a deny-level lint is accepted. Evidence G-01 in 01-GATE-EVIDENCE.md. A forbid-level lint cannot be overridden by allow at any position (rustc error E0453), which closes the gap."
  artifacts:
    - path: "Cargo.toml"
      issue: "[workspace.lints.clippy] sets unwrap_used, expect_used, indexing_slicing to deny; must be forbid"
    - path: "CLAUDE.md"
      issue: "Code rules block shows the three lints at deny; design of record must say forbid"
    - path: ".planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md"
      issue: "G-01 and Open decision 1 record the gap as open; needs a closing proof"
  missing:
    - "Raise unwrap_used, expect_used, indexing_slicing to \"forbid\" in Cargo.toml [workspace.lints.clippy]"
    - "Edit the CLAUDE.md Code rules TOML block to the same three forbid lines and note that #[expect] is no longer possible for these three lints"
    - "Re-run the G-01 probe (inner #![allow(clippy::unwrap_used, reason = ...)] plus an unwrap()) and record a non-zero clippy exit as the closing proof; re-run V-07 to confirm #[expect(clippy::unwrap_used, reason)] now fails and update SC2 wording accordingly"
    - "Four gate commands pass on the final tree; cargo clean last"
  debug_session: ""

- truth: "A workspace member that omits [lints] workspace = true still cannot compile unsafe code, unwrap, expect, or index a slice under the gate (GATE-01: every crate is held to the workspace lints)"
  status: resolved
  reason: "User reported: both I think, no? (decision A+B)"
  severity: major
  test: 2
  root_cause: "Stable Cargo 1.97.1 has no lint that rejects a member missing [lints] workspace = true; it rejects only a partial override. Evidence G-02 in 01-GATE-EVIDENCE.md. A [build] rustflags backstop in .cargo/config.toml reaches every workspace crate regardless of its manifest, while registry and git dependencies are unaffected because Cargo passes them --cap-lints allow (RESEARCH.md Open Question 2, verified)."
  artifacts:
    - path: ".cargo/config.toml"
      issue: "does not exist; D-01 did not list it as a policy file"
    - path: ".planning/phases/01-workspace-and-gates/01-CONTEXT.md"
      issue: "D-01 lists the Cargo-native policy files; must add .cargo/config.toml with its purpose and its known weakness"
    - path: ".planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md"
      issue: "G-02 and Open decision 2 record the gap as open; needs a closing proof"
  missing:
    - "Create .cargo/config.toml with [build] rustflags = [\"-Funsafe_code\", \"-Fclippy::unwrap_used\", \"-Fclippy::expect_used\", \"-Fclippy::indexing_slicing\"] (forbid, matching decision 1), with a comment that it is a backstop for a member missing [lints] workspace = true and that a RUSTFLAGS environment variable silently replaces it"
    - "Amend D-01 in 01-CONTEXT.md to list .cargo/config.toml as the fifth Cargo-native policy file and record the duplication and RUSTFLAGS caveats"
    - "Keep option A: the per-manifest [lints] workspace = true check stays an acceptance criterion for every plan that adds a crate (record this in CONTEXT.md or CLAUDE.md How to work)"
    - "Re-run the G-02 probe (delete [lints] from one member, add an unwrap()) and record a non-zero clippy exit as the closing proof; confirm cargo build, cargo test, and cargo deny check still exit 0 for the untouched tree and that no dependency fails under the rustflags"
    - "Four gate commands pass on the final tree; cargo clean last"
  debug_session: ""

- truth: "A broken intra-doc link fails the gate (D-12: [workspace.lints.rustdoc] broken_intra_doc_links = deny is enforced)"
  status: resolved
  reason: "User reported: a (add cargo doc --workspace --no-deps as a fifth gate command)"
  severity: major
  test: 3
  root_cause: "rustdoc lints in [workspace.lints.rustdoc] are applied only by rustdoc; cargo clippy and cargo test never invoke it on the crate docs, so only cargo doc --workspace --no-deps fails on a broken link. Evidence G-03 in 01-GATE-EVIDENCE.md (clippy and test exit 0, cargo doc exit 101)."
  artifacts:
    - path: "CLAUDE.md"
      issue: "Commands section lists four gate commands; must list cargo doc --workspace --no-deps as the fifth"
    - path: ".planning/ROADMAP.md"
      issue: "Phase 1 success criterion 1 says four plain cargo commands; must say five and name the fifth"
    - path: ".planning/REQUIREMENTS.md"
      issue: "GATE-02 wording names the gate commands; must include cargo doc"
    - path: ".planning/phases/01-workspace-and-gates/01-CONTEXT.md"
      issue: "D-01 says the four commands enforce policy; must say five"
    - path: ".planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md"
      issue: "G-03 and Open decision 3 record the gap as open; needs a closing proof"
  missing:
    - "Add `cargo doc --workspace --no-deps` to the CLAUDE.md Commands block as the fifth gate command, with a one-line note that it is what enforces [workspace.lints.rustdoc]"
    - "Update ROADMAP.md Phase 1 criterion 1, REQUIREMENTS.md GATE-02, and D-01 in 01-CONTEXT.md from four commands to five"
    - "Confirm RUSTDOCFLAGS is not needed: cargo doc --workspace --no-deps exits 0 on the clean tree and exits non-zero on the G-03 probe; record both as the closing proof"
    - "Five gate commands pass on the final tree; cargo clean last"
  debug_session: ""
