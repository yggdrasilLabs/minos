---
phase: 01-workspace-and-gates
fixed_at: 2026-10-09T23:02:11Z
review_path: .planning/phases/01-workspace-and-gates/01-REVIEW.md
iteration: 1
fix_scope: all
findings_in_scope: 14
fixed: 14
skipped: 0
status: all_fixed
---

# Phase 1: Code Review Fix Report

**Fixed at:** 2026-10-09T23:02:11Z
**Source review:** .planning/phases/01-workspace-and-gates/01-REVIEW.md
**Iteration:** 1
**Scope:** all (CR-01, WR-01 through WR-06, IN-01 through IN-07)

**Summary:**
- Findings in scope: 14
- Fixed: 14
- Skipped: 0

This report is cumulative over two passes against the same REVIEW.md. The first pass (scope `critical_warning`) fixed CR-01 and WR-01 through WR-06 in commits `ef28e77`..`6a67a2e`; those entries are carried forward unchanged below. The second pass (scope `all`) fixed IN-01 through IN-07 in commits `3b2b6ee`..`5677461`. Both passes ran in an isolated worktree on a temp branch that was fast-forwarded onto `main`, one commit per finding, each ending with the `Co-Authored-By` trailer. `.planning/config.json` (pre-existing unrelated modification) was never staged. This report is not committed.

No security invariant was weakened and no escape hatch was added in either pass. Every change in the second pass is a comment, a doc line, a `.gitignore` pattern, an explicit member list, or a stricter profile setting.

## Fixed Issues

### CR-01: `unmaintained = "workspace"` silently hides RUSTSEC-2025-0141 (`bincode`) in the VMM dependency path

**Files modified:** `deny.toml`, `.planning/research/STACK.md`, `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md`
**Commit:** ef28e77
**Applied fix:** Restored cargo-deny's default `unmaintained = "all"` scope (stricter than before) and replaced the blanket narrowing with exactly one named, reasoned `ignore` entry for RUSTSEC-2025-0141 (`bincode 2.0.1` via `msb_krun =0.1.41`, reachable only inside `minos-vmm`). No other ignore or allow entry was added. Reworded the `[advisories]` comment so it no longer says "never an ignore" while carrying one; it now states that the only way past a hit is a named entry with a reviewed reason, never scope narrowing. Reproduced the failure first (`advisories FAILED` with the default scope on the pre-fix file), then recorded the advisory, the decision, the repro, and the passing run in 01-GATE-EVIDENCE.md and next to the `msb_krun` pin row and the deny.toml sketch in STACK.md.
**Verification:** `cargo deny check` exit code 0 -- `advisories ok, bans ok, licenses ok, sources ok`.

### WR-01: `CLAUDE.md` Workspace section contradicts the crate list and dependency direction that `deny.toml` enforces

**Files modified:** `CLAUDE.md`
**Commit:** fb6e855
**Applied fix:** `deny.toml` was treated as authoritative and CLAUDE.md was brought into agreement (not the reverse). The crate list now matches `crates/` (added `minos-loop`, `minos-vmm`; `minos-backend-lambda`, `minos-backend-chv`, `minos-orchestrator`, `minos-store-aws`, and the GPU daemon are listed as later milestones). The direction paragraph now restates the DAG as the `[bans]` wrapper entries encode it: runner depends on core/store/sandbox and is generic over `Backend` (never names a backend crate); loop depends on core/store/propose and never links sandbox or a backend; vmm depends on no internal crate; only the CLI picks a concrete backend. The `AnyBackend` sentence in the minos-sandbox section was replaced with ARCHITECTURE.md amendment A4 verbatim. `deny.toml:49` was left unchanged.

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
**Applied fix:** Added the sixth gate line to CLAUDE.md Commands, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings   # Phase 5 onward`, with a paragraph stating the gap (host gate builds the guest for darwin where its Linux-only deps do not exist), the Phase 5 closure (D-09 adds the musl target to `rust-toolchain.toml` then, plus the matching `cargo test` once a Linux runner exists), and the interim mitigation (keep host-compilable guest code outside `cfg(target_os = "linux")`). Added a pointer comment to `rust-toolchain.toml`. The musl target was **not** added to `rust-toolchain.toml` now.

### IN-01: `.gitignore` does not exclude insta's pending-snapshot files

**Files modified:** `.gitignore`
**Commit:** 3b2b6ee
**Applied fix:** Appended `*.snap.new` and `*.pending-snap` with a two-line comment (accept with cargo-insta, never commit; `proptest-regressions/` stays committed). `proptest-regressions/` was not ignored.
**Verification:** `git check-ignore -v` matches `crates/minos-core/src/snapshots/x.snap.new` (rule `.gitignore:4`) and `crates/x.pending-snap` (rule `.gitignore:5`); `git check-ignore` on `proptest-regressions/foo.txt` and `crates/minos-core/proptest-regressions/a.txt` prints nothing (exit 1).

### IN-02: No `[profile.release]` hardening for binaries that parse hostile input

**Files modified:** `Cargo.toml`
**Commit:** fa0468c
**Applied fix:** Added `[profile.release]` with `overflow-checks = true` directly after `[workspace.package]`, with a comment giving the reason (hostile input in every binary; wraps must panic without relying on each call site) and noting that `panic = "abort"` for `minos-guest` belongs to the phase that gives it a body. `panic = "abort"` was **not** added.
**Verification:** `cargo metadata --no-deps` parses; `tomllib` reads `profile.release = {'overflow-checks': True}`.

### IN-03: `members = ["crates/*"]` plus opt-in `[lints] workspace = true` still lets a new directory escape the non-backstop lints

**Files modified:** `Cargo.toml`
**Commit:** d44839d
**Applied fix:** Replaced the glob with an explicit list of the ten existing crates (`minos-core`, `minos-store`, `minos-sandbox`, `minos-backend-krun`, `minos-propose`, `minos-runner`, `minos-loop`, `minos-vmm`, `minos-guest`, `minos-cli`) under a two-line comment stating that every new member needs `[lints] workspace = true`, a `deny.toml` layering entry, and (if core-like) its own `clippy.toml`.
**Verification:** `cargo metadata --no-deps --format-version 1` piped through node: `workspace_members.length == 10` and the sorted package-name list is identical before and after the edit (`MATCH: all ten crates are members`). A shell loop over `crates/*/` found no directory absent from the list.

### IN-04: `clap`'s `derive` feature is declared and linked, but D-12/F-01 establish that `#[derive(clap::Parser)]` cannot pass the gate

**Files modified:** `Cargo.toml`, `crates/minos-cli/Cargo.toml`
**Commit:** 7ea8970
**Applied fix:** Took the review's second option, as instructed: the `derive` feature was **not** dropped (that is a user decision flagged in the 01-04 summary). Added a `PENDING USER DECISION` comment above the `clap` entry in the root `[workspace.dependencies]` (F-01: clap_derive 4.6.7 emits `#[allow(clippy::restriction)]`, E0453 under the three forbid lints; F-02: builder API passes; the two resolutions and what each requires; do not write a clap derive before the decision) and a shorter one above `clap.workspace = true` in `crates/minos-cli/Cargo.toml` pointing back to the root entry. `.claude/CLAUDE.md` decision 5 was left unchanged since the decision is still pending.
**Verification:** `cargo metadata --no-deps` parses; `tomllib` confirms the `clap` entries themselves are byte-identical (`{'version': '=4.6.7', 'features': ['derive']}` and `{'workspace': True}`).

### IN-05: `deny.toml:51` reason text misattributes the `JobRunner` trait

**Files modified:** `deny.toml` (the entry is now at line 70 after the WR-04/WR-05 insertions)
**Commit:** fd7f8b6
**Applied fix:** Changed the `minos-runner` layering reason to the review's wording: `"loop defines JobRunner; the CLI adapts minos-runner to it, so neither depends on the other"`. The `wrappers` list is unchanged.
**Verification:** `tomllib` parses `deny.toml`; `cargo deny check` exit code 0 with the edited file.

### IN-06: `CLAUDE.md:370` still says nano-dollar precision; the stack research corrected it to pico-dollars

**Files modified:** `CLAUDE.md` (the line is now 380 after the WR-01/WR-02/WR-06 insertions)
**Commit:** 6b4f6c7
**Applied fix:** Changed "nano-dollar precision" in the Cost section to "pico-dollar precision (10^-12 USD; published AWS rates have 10 fractional digits)".
**Verification:** `grep -n "nano-dollar" CLAUDE.md` prints nothing; the pico-dollar line is present at 380.

### IN-07: `.cargo/config.toml` rustflags also apply to any `cargo install` run with the repo as the current directory

**Files modified:** `.cargo/config.toml`
**Commit:** 5677461
**Applied fix:** Added two comment lines to the caveat block, before `[build]`: `cargo install` from inside this directory also gets these flags (config discovery is cwd-based, and cap-lints does not apply to the installed package itself); run tool installs from elsewhere.
**Verification:** `tomllib` parses the file and `build.rustflags` is unchanged (`-Funsafe_code`, `-Fclippy::unwrap_used`, `-Fclippy::expect_used`, `-Fclippy::indexing_slicing`).

## Skipped Issues

None.

## Full gate after all fixes (second pass, run in the worktree at `5677461`)

| Command | Exit |
|---|---|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 (0 warnings) |
| `cargo test --workspace` | 0 (17 test binaries, all `test result: ok`) |
| `cargo deny check` | 0 (`advisories ok, bans ok, licenses ok, sources ok`; the 7 `warning[duplicate]` lines are the pre-existing `multiple-versions = "warn"` duplicates named in the `Cargo.toml` `multiple_crate_versions` comment) |
| `cargo doc --workspace --no-deps` | 0 (0 warnings) |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing (exit 1) |
| `cargo clean` | exit 0 (removed 3697 files, 881.9 MiB); no `target/` in the worktree or the main checkout afterward |

The first-pass gate (recorded at `6a67a2e`) had the same result: all five commands exit 0, C-02 prints nothing, `cargo clean` run.

## Commit summary

| Finding | Commit | Files |
|---|---|---|
| CR-01 | ef28e77 | deny.toml, STACK.md, 01-GATE-EVIDENCE.md |
| WR-01 | fb6e855 | CLAUDE.md |
| WR-02 | fa5634c | Cargo.toml, CLAUDE.md, 01-CONTEXT.md, REQUIREMENTS.md |
| WR-03 | af36563 | crates/minos-core/clippy.toml, 01-GATE-EVIDENCE.md |
| WR-04 | faf5ede | deny.toml, 01-GATE-EVIDENCE.md |
| WR-05 | b72e874 | deny.toml, .claude/CLAUDE.md, 01-GATE-EVIDENCE.md |
| WR-06 | 6a67a2e | CLAUDE.md, rust-toolchain.toml |
| IN-01 | 3b2b6ee | .gitignore |
| IN-02 | fa0468c | Cargo.toml |
| IN-03 | d44839d | Cargo.toml |
| IN-04 | 7ea8970 | Cargo.toml, crates/minos-cli/Cargo.toml |
| IN-05 | fd7f8b6 | deny.toml |
| IN-06 | 6b4f6c7 | CLAUDE.md |
| IN-07 | 5677461 | .cargo/config.toml |

---

_Fixed: 2026-10-09T23:02:11Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
