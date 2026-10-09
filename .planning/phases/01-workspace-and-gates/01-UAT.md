---
status: testing
phase: 01-workspace-and-gates
source: [01-VERIFICATION.md]
started: 2026-10-09T21:44:23Z
updated: 2026-10-09T21:44:23Z
---

## Current Test

number: 1
name: Decide open decision 1: an inner `#![allow(clippy::..., reason = "...")]` passes the clippy gate
expected: |
  User picks (A) accept and document the gap, or (B) raise unwrap_used, expect_used, indexing_slicing to forbid (an edit to the CLAUDE.md Code rules). ROADMAP SC2 and GATE-05 say "no allow(clippy::...) appears anywhere"; today the gate enforces that only for outer attributes and for reasonless inner ones. Evidence: G-01 in 01-GATE-EVIDENCE.md, independently reproduced by the verifier.
awaiting: user response

## Tests

### 1. Decide open decision 1: an inner `#![allow(clippy::..., reason = "...")]` passes the clippy gate
expected: User picks (A) accept and document the gap, or (B) raise unwrap_used, expect_used, indexing_slicing to forbid (an edit to the CLAUDE.md Code rules). ROADMAP SC2 and GATE-05 say "no allow(clippy::...) appears anywhere"; today the gate enforces that only for outer attributes and for reasonless inner ones. Evidence: G-01.
result: [pending]

### 2. Decide open decision 2: a crate that omits `[lints] workspace = true` escapes every workspace lint
expected: User picks (A) a one-time check plus manifest review for every new crate, or (B) a .cargo/config.toml rustflags backstop (a policy file D-01 did not list). All 10 current manifests carry the table. Evidence: G-02.
result: [pending]

### 3. Decide open decision 3: `[workspace.lints.rustdoc] broken_intra_doc_links = deny` is enforced only by `cargo doc`
expected: User picks (A) add `cargo doc --workspace --no-deps` as a fifth gate command (changes the CLAUDE.md Commands section and ROADMAP SC1), or (B) document that the rustdoc lint table only takes effect when docs are built. Evidence: G-03 (clippy and test exit 0, cargo doc exits 101).
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
