---
phase: 01-workspace-and-gates
fixed_at: 2026-10-09T22:54:04Z
review_path: .planning/phases/01-workspace-and-gates/01-REVIEW.md
iteration: 1
fix_scope: critical_warning
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 1: Code Review Fix Report

**Fixed at:** 2026-10-09T22:54:04Z
**Source review:** .planning/phases/01-workspace-and-gates/01-REVIEW.md
**Iteration:** 1
**Scope:** critical_warning (CR-01, WR-01 through WR-06; the seven IN-* findings were out of scope and are untouched)

**Summary:**
- Findings in scope: 7
- Fixed: 7
- Skipped: 0

All fixes were made in an isolated worktree on a temp branch and fast-forwarded onto `main`. One commit per finding. `.planning/config.json` (pre-existing unrelated modification) was not staged. This report is not committed.

## Fixed Issues

### CR-01: `unmaintained = "workspace"` silently hides RUSTSEC-2025-0141 (`bincode`) in the VMM dependency path

**Files modified:** `deny.toml`, `.planning/research/STACK.md`, `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`
**Commit:** ef28e77
**Applied fix:** Restored cargo-deny's default `unmaintained = "all"` scope (stricter than before) and replaced the blanket narrowing with exactly one named, reasoned `ignore` entry for RUSTSEC-2025-0141 (`bincode 2.0.1` via `msb_krun =0.1.41`, reachable only inside `minos-vmm`). No other ignore or allow entry was added. Reworded the `[advisories]` comment so it no longer says "never an ignore" while carrying one; it now states that the only way past a hit is a named entry with a reviewed reason, never scope narrowing. Reproduced the failure first (`advisories FAILED` with the default scope on the pre-fix file), then recorded the advisory, the decision, the repro, and the passing run in 01-GATE-EVIDENCE.md and next to the `msb_krun` pin row and the deny.toml sketch in STACK.md.
**Verification:** `cargo deny check` exit code **0** -- `advisories ok, bans ok, licenses ok, sources ok`.

### WR-01: `CLAUDE.md` Workspace section contradicts the crate list and dependency direction that `deny.toml` enforces

**Files modified:** `CLAUDE.md`
**Commit:** fb6e855
**Applied fix:** Per instruction, `deny.toml` was treated as authoritative and CLAUDE.md was brought into agreement (not the reverse). The crate list now matches `crates/` (added `minos-loop`, `minos-vmm`; `minos-backend-lambda`, `minos-backend-chv`, `minos-orchestrator`, `minos-store-aws`, and the GPU daemon are listed as later milestones). The direction paragraph now restates the DAG as the `[bans]` wrapper entries encode it: runner depends on core/store/sandbox and is generic over `Backend` (never names a backend crate); loop depends on core/store/propose and never links sandbox or a backend; vmm depends on no internal crate; only the CLI picks a concrete backend. The `AnyBackend` sentence in the minos-sandbox section was replaced with ARCHITECTURE.md amendment A4 verbatim. `deny.toml:49` was left unchanged.

### WR-02: The `Cargo.toml:29` comment still claims a guarantee the gate does not provide for non-forbid lints

**Files modified:** `Cargo.toml`, `CLAUDE.md`, `.planning/phases/01-workspace-and-gates/01-CONTEXT.md`, `.planning/REQUIREMENTS.md`
**Commit:** fa5634c
**Applied fix:** Replaced the comment with the review's wording (outer allow denied, forbid lints unsuppressible, inner `#![allow(..., reason)]` on any other lint still passes clippy per R-03, the per-phase C-02 grep catches it). Made C-02 recurring: added a paragraph to CLAUDE.md Commands and a sentence to D-15 in 01-CONTEXT.md stating every phase verification runs `grep -rnE '#!?\[allow\(' crates --include='*.rs'` and it must print nothing. Qualified the GATE-05 wording in REQUIREMENTS.md. The grep uses the exact C-02 command already recorded in 01-GATE-EVIDENCE.md (the review's `^\s*` variant was dropped for portability; the simpler pattern matches a superset). `cargo metadata --no-deps` confirmed Cargo.toml still parses.

### WR-03: `minos-core`'s clippy bans cover only the clock and hash collections

**Files modified:** `crates/minos-core/clippy.toml`, `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`
**Commit:** af36563
**Applied fix:** Applied the review's three tables in full: `disallowed-methods` (+ `std::thread::sleep`, `std::env::{var,var_os,args}`, `std::fs::{read,read_to_string,write}`, `std::io::stdin`), `disallowed-types` (+ `std::fs::File`, `std::process::Command`, `std::net::{TcpStream,TcpListener,UdpSocket}`), and a new `disallowed-macros` table (`std::println`, `std::eprintln`, `std::dbg`). Header comment notes every path must be a real std path and re-proven on change.
**Verification:** `cargo clippy -p minos-core --all-targets -- -D warnings` on the untouched crate: exit 0, zero `unknown path`/`invalid path` warnings (recorded as C-03b). A throwaway probe appended to `lib.rs` (one use per table; reverted with `git checkout --`, not committed) produced `use of a disallowed method \`std::env::var\``, `use of a disallowed type \`std::fs::File\``, and `use of a disallowed macro \`std::println\`` (recorded as V-09b/V-10b).

### WR-04: The "no C" ban keys on compilation (`cc`) only

**Files modified:** `deny.toml`, `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`
**Commit:** faf5ede
**Applied fix:** Added the eight ban entries from the review (`pkg-config`, `vcpkg`, `cmake`, `bindgen`, `bzip2-sys`, `libz-sys`, `aws-lc-sys`, `ring`) with their reasons, plus a two-line comment explaining why `cc` alone is insufficient. `cargo tree -i` for both checked targets confirmed none of them is in either aarch64 graph before editing.
**Verification:** `cargo deny check` exit code 0, all four checks ok.

### WR-05: `tokio` is in `minos-vmm`'s `aarch64-apple-darwin` graph and nothing fences it

**Files modified:** `deny.toml`, `.claude/CLAUDE.md`, `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`
**Commit:** b72e874
**Applied fix:** Added `{ crate = "tokio", wrappers = ["msb-imago"], reason = ... }` next to the `msb_krun` entry. `cargo tree -i tokio --depth 1` on both targets showed `msb-imago v0.1.7` as the sole direct dependent, so the wrapper name matches the one existing edge. Corrected decision 10 in `.claude/CLAUDE.md` (both the decisions table and the "What NOT to Use" row) to "no Minos crate uses tokio; the pinned VMM links it internally and deny.toml fences that single edge".
**Verification:** `cargo deny check` exit code 0, all four checks ok; the existing `msb_krun` wrapper edge still passes.

### WR-06: The five gate commands never compile, lint, or test `minos-guest` for the target it runs on

**Files modified:** `CLAUDE.md`, `rust-toolchain.toml`
**Commit:** 6a67a2e
**Applied fix:** Added the sixth gate line to CLAUDE.md Commands, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings   # Phase 5 onward`, with a paragraph stating the gap (host gate builds the guest for darwin where its Linux-only deps do not exist), the Phase 5 closure (D-09 adds the musl target to `rust-toolchain.toml` then, plus the matching `cargo test` once a Linux runner exists), and the interim mitigation (keep host-compilable guest code outside `cfg(target_os = "linux")`). Added a pointer comment to `rust-toolchain.toml`. Per instruction, the musl target was **not** added to `rust-toolchain.toml` now.

## Skipped Issues

None.

## Full gate after all fixes (run in the worktree)

| Command | Exit |
|---|---|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 (0 unknown-lint / unknown-path warnings) |
| `cargo test --workspace` | 0 (17 test binaries, all `test result: ok`) |
| `cargo deny check` | 0 (`advisories ok, bans ok, licenses ok, sources ok`) |
| `cargo doc --workspace --no-deps` | 0 (0 warnings) |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `cargo clean` | run; no `target/` left in the worktree or the main checkout |

---

_Fixed: 2026-10-09T22:54:04Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
