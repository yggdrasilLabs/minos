---
status: resolved
phase: 01-workspace-and-gates
source: [01-01-SUMMARY.md, 01-02-SUMMARY.md, 01-03-SUMMARY.md, 01-04-SUMMARY.md]
started: 2026-10-09T23:11:56Z
updated: 2026-10-10T00:21:28Z
---

## Current Test

[testing complete]

## Tests

### 1. Five gate commands pass on a clean checkout
expected: On the committed tree (HEAD, no RUSTFLAGS or RUSTDOCFLAGS set), run in order: cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; cargo deny check; cargo doc --workspace --no-deps. All five exit 0 with no script in front of them. cargo deny prints "advisories ok, bans ok, licenses ok, sources ok". Cargo.lock and .cargo/config.toml are tracked by git.
result: pass

### 2. Inner allow of a forbid lint fails clippy
decision: "No unwrap, expect, or slice indexing anywhere, test code included. Do not adopt allow-unwrap-in-tests, allow-expect-in-tests, or allow-indexing-slicing-in-tests; tests return Result or use ? and pattern matching. Closes 01-04 carry-forward note 1. Test conventions match production code."
expected: Add an inner `#![allow(clippy::unwrap_used, reason = "probe")]` at the top of any crate's lib.rs together with a function that calls .unwrap(). cargo clippy --workspace --all-targets -- -D warnings exits non-zero with error[E0453] "allow(clippy::unwrap_used) incompatible with previous forbid". The same happens for #[expect(clippy::indexing_slicing, reason = "probe")] on indexing code. Revert the edit afterwards and clippy passes again.
result: skipped
reason: "User rule: no unwrap, anywhere, ever, under any circumstances, tests or otherwise. The probe requires writing an unwrap, so it is not run. Covered by recorded evidence G-01-CLOSED and R-01 in 01-GATE-EVIDENCE.md (exit 101, E0453), reproduced by the verifier in a scratch copy on 2026-10-09."

### 3. Crate without [lints] workspace = true is still held by the backstop
expected: Delete the [lints] workspace = true table from one member manifest (for example crates/minos-loop/Cargo.toml) and add an .unwrap() and a slice index x[0] to its lib.rs. cargo clippy --workspace --all-targets -- -D warnings still exits non-zero, and the errors say the lint was "requested on the command line with -F clippy::unwrap-used" (and -F clippy::indexing-slicing), proving .cargo/config.toml [build] rustflags caught it. Revert with git checkout afterwards.
result: skipped
reason: "Same user rule: the probe requires writing an unwrap and a slice index. Covered by recorded evidence G-02-CLOSED run 1 in 01-GATE-EVIDENCE.md (exit 101, four errors citing -F on the command line) and B-01, reproduced by the verifier in a scratch copy on 2026-10-09."

### 4. Broken intra-doc link fails cargo doc only
expected: Append a doc comment such as `/// Link to [\`DoesNotExist\`].` to an item in any crate. cargo clippy --workspace --all-targets -- -D warnings still exits 0, but cargo doc --workspace --no-deps exits non-zero with "error: unresolved link to \`DoesNotExist\`" citing rustdoc::broken-intra-doc-links. Revert afterwards and cargo doc passes again.
result: pass

### 5. cargo clean leaves no target directory
expected: After the gate commands have run, cargo clean exits 0 and reports files removed; afterwards `ls target` reports no such directory and git status shows no change to tracked files.
result: pass

### 6. Decide F-01: clap derive is blocked by the forbid lints
expected: Finding F-01 from plan 01-04: #[derive(clap::Parser)] (and clap's Args, Subcommand, ValueEnum derives) fail the clippy gate with three E0453 errors because clap_derive 4.6.7 emits #[allow(clippy::restriction)], which the forbid on unwrap_used, expect_used, and indexing_slicing rejects. The clap builder API passes; serde and thiserror derives pass. .claude/CLAUDE.md still lists clap "with derive". You pick (A) minos-cli uses the clap builder API and the stack doc is corrected, (B) revisit the forbid decision for these three lints, or (C) defer the choice to Phase 8 planning. Reply with your choice.
result: issue
reported: "a"
decision: "A: minos-cli uses the clap builder API; the .claude/CLAUDE.md stack entry is corrected and the unused clap derive feature is dropped"
severity: major

### 7. Workspace lint baseline (unsafe_code forbid; unwrap_used, expect_used, indexing_slicing, allow_attributes, allow_attributes_without_reason) inherited by all ten crates
expected: Workspace lint baseline inherited by all ten crates via [lints] workspace = true
result: pass
source: automated
coverage_id: 01-01/D1

### 8. minos-core clippy.toml bans clock reads and hash collections
expected: minos-core clippy.toml bans SystemTime::now, Instant::now, HashMap, HashSet
result: pass
source: automated
coverage_id: 01-01/D2

### 9. Ten crates with exact ARCHITECTURE.md dependency edges, no C/OpenSSL/libgit2 crate in either aarch64 target graph
expected: Ten crates with exact dependency edges; cargo tree C-crate grep is 0 on both aarch64 targets
result: pass
source: automated
coverage_id: 01-01/D3

### 10. Supply-chain policy: crates.io only, git needs rev + allow-git, build-script allowlist, executables denied, C/OpenSSL/libgit2 banned, licenses and advisories checked
expected: cargo deny check passes with advisories ok, bans ok, licenses ok, sources ok
result: pass
source: automated
coverage_id: 01-02/D1

### 11. Crate layering encoded as wrappers entries for all seven internal libraries
expected: Seven minos-* wrappers entries in deny.toml; cargo deny check passes the allowed graph
result: pass
source: automated
coverage_id: 01-02/D2

### 12. fmt, clippy -D warnings, test, and deny pass together
expected: Four gate commands each exit 0 on the 01-02 tree
result: pass
source: automated
coverage_id: 01-02/D3

### 13. Code-lint rules fire under cargo clippy -D warnings (unsafe, unwrap, expect, index, outer allow with and without reason, unfulfilled expect, clock and HashMap in core); expect with reason passes
expected: 01-GATE-EVIDENCE.md V-01..V-10 (nine exit 101, V-07 positive control exit 0)
result: pass
source: automated
coverage_id: 01-03/D1

### 14. Supply-chain rules fire under cargo deny (git spec, git source, unknown registry, unlisted build script, C crate through a feature)
expected: 01-GATE-EVIDENCE.md V-11..V-14, V-23
result: pass
source: automated
coverage_id: 01-03/D2

### 15. Layering bans fire for loop->sandbox/backend/runner and core->sandbox/backend (dev); normal core->sandbox is a Cargo cycle
expected: 01-GATE-EVIDENCE.md V-15..V-20
result: pass
source: automated
coverage_id: 01-03/D3

### 16. Four gate commands pass on the final 01-03 tree; cargo clean ran last
expected: 01-GATE-EVIDENCE.md Final gate and cargo clean sections
result: pass
source: automated
coverage_id: 01-03/D4

## Summary

total: 16
passed: 13
issues: 1
pending: 0
skipped: 2
blocked: 0

## Gaps

- truth: "minos-cli can parse arguments under the clippy gate, and the stack of record (.claude/CLAUDE.md) names an API that passes it (F-01: clap derive fails under forbid)"
  status: resolved
  reason: "User reported: a (minos-cli uses the clap builder API; correct the stack doc)"
  severity: major
  test: 6
  root_cause: "clap_derive 4.6.7 emits #[allow(clippy::restriction)] on every generated impl (Parser, Args, Subcommand, ValueEnum). The restriction group contains unwrap_used, expect_used, and indexing_slicing, which are forbid, so rustc rejects the allow with E0453 (01-GATE-EVIDENCE.md F-01, three errors). The clap builder API emits no such attribute and passes (F-02). The decision is locked: builder API. No lint level changes."
  artifacts:
    - path: ".claude/CLAUDE.md"
      issue: "Decisions table row 5 and the Core Technologies table list clap 4.6.7 with derive; must say builder API and record why derive is impossible under forbid"
    - path: "Cargo.toml"
      issue: "Lines 81-86: the PENDING USER DECISION comment and clap = { version = \"=4.6.7\", features = [\"derive\"] }; drop the feature so clap_derive leaves the graph and replace the comment with the decision"
    - path: "crates/minos-cli/Cargo.toml"
      issue: "Lines 25-28: PENDING USER DECISION comment above clap.workspace = true; replace with one line stating the builder API is the decided surface"
    - path: ".claude/CLAUDE.md"
      issue: "Lines 39, 62, 168 all say clap with derive; line 284 (MSRV row) is still correct"
    - path: "Cargo.lock"
      issue: "clap_derive and any dependencies reachable only through it are removed when the feature is dropped; the lock must be regenerated with the locked toolchain and committed"
    - path: "deny.toml"
      issue: "[bans.build] allow-build-scripts may list crates (proc-macro2, quote, syn family) that were only needed by clap_derive; keep entries still required by serde/thiserror derives, remove only orphans, and record each removal"
    - path: ".planning/STATE.md"
      issue: "Line 96 carries the F-01 note as open; mark it decided (A)"
    - path: ".planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md"
      issue: "F-01 is recorded as a finding for the user; add a short Decision note under it pointing at this UAT test 6"
  missing:
    - "Edit .claude/CLAUDE.md: clap 4.6.7 builder API only (no derive feature), with a one-line reason citing E0453 from clap_derive's #[allow(clippy::restriction)] under forbid"
    - "Edit Cargo.toml [workspace.dependencies] clap to version = \"=4.6.7\" with default features only (no derive); regenerate Cargo.lock with cargo update -w (or cargo generate-lockfile) on the pinned toolchain and confirm clap_derive is gone from cargo tree on both aarch64 targets"
    - "Re-run cargo deny check; if it passes, prune [bans.build] allow-build-scripts entries that no longer appear in cargo tree (record the before/after list); if it fails, add nothing new without recording why"
    - "Update STATE.md F-01 note and add the F-01 decision note in 01-GATE-EVIDENCE.md"
    - "Five gate commands pass on the final tree (fmt, clippy, test, deny, doc); cargo clean last; no unwrap, expect, or indexing is written anywhere, including probes"
  debug_session: ""
