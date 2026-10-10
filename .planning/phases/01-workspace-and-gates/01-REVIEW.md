---
phase: 01-workspace-and-gates
reviewed: 2026-10-10T00:28:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - Cargo.toml
  - .cargo/config.toml
  - .claude/CLAUDE.md
  - crates/minos-cli/Cargo.toml
  - crates/minos-core/clippy.toml
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 1: Code Review Report

**Reviewed:** 2026-10-10T00:28:00Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Re-review after gap-closure plan 01-05 (commits `5762daf`, `9d83ca8`, `3be27f1`). It replaces the 28-file review at this path dated 2026-10-09T22:41Z, whose 14 findings `01-REVIEW-FIX.md` records as fixed; none of them is re-raised here, and I spot-checked that each fix is still in place (forbid levels at `Cargo.toml:45-47`, the rustflags backstop, `unmaintained = "all"` with the named `RUSTSEC-2025-0141` ignore, the tokio wrapper, the explicit member list, `overflow-checks`, the clippy.toml tables).

**What 01-05 did, verified independently.** `Cargo.toml:84` now reads `clap = { version = "=4.6.7" }` and `crates/minos-cli/Cargo.toml:26` keeps `clap.workspace = true`. `Cargo.lock` lost exactly `clap_derive 4.6.7` and `heck 0.5.0`; nothing was added and the lock still has zero git sources. `cargo tree -i` on the darwin graph shows `proc-macro2`, `quote`, `syn 3.0.6` still consumed by `serde_derive`, `thiserror-impl`, `async-trait`, and `rustversion` by `deku`, so no `[bans.build] allow-build-scripts` entry was orphaned, which matches commit `9d83ca8`. The stack-of-record claims introduced in `3be27f1` are accurate against the pinned registry sources: `clap-4.6.7/Cargo.toml` has `rust-version = "1.85"`, and `clap_derive-4.6.7` emits `#[allow(clippy::style, clippy::complexity, clippy::pedantic, clippy::restriction, ...)]` at `derives/into_app.rs:43,91`, `value_enum.rs:65`, `subcommand.rs:82,116`, and `args.rs:102,145`. Allowing a group that contains a lint forbidden by a non-group `-F` flag is a hard E0453 (the future-compat `forbidden_lint_groups` path applies only when the *forbid* was on a group), so "a derive cannot pass the gate" is correct as stated. The `[workspace.lints]` tables, the `.cargo/config.toml` backstop, `deny.toml`, and `rust-toolchain.toml` were untouched by 01-05 and remain coherent with each other: four lints forbid in both places, groups at priority -1, carve-outs with reasons, `publish = false` making `cargo_common_metadata` skip (that lint returns early for unpublished packages, so the comment at `Cargo.toml:23` is right).

**What is wrong.** The defects are in the stack-of-record file and its generator source, in the completeness of the core purity denylist, and in a process rule violated on disk. `.claude/CLAUDE.md` is generated from `.planning/research/STACK.md` between the `GSD:stack-start`/`stack-end` markers (lines 23-307), and the generator drops fenced code while keeping lines that begin with `#`, so the file an agent loads every session has five empty sections and twelve TOML/Rust comment lines rendered as H1 headings. The generator source's `## Installation` sketch, which 01-05 edited (its clap line) and declared authoritative, still says `unwrap_used = "deny"`, `members = ["crates/*", "xtask"]`, and `rust-version = "1.97"`, contradicting the real `Cargo.toml` on the one thing this phase exists to lock. The stack doc also still prescribes an `xtask` in two places after D-01/D-07 ruled one out. `crates/minos-core/clippy.toml`'s new entries can each be sidestepped by a sibling std API the list does not name (`OpenOptions` yields a `File` without ever writing the type, `vars()` beside `var()`, `stdout()` + `write!` beside `println!`). And `target/` (4090 files, written 20:17-20:23 local) is present at review time although the phase was marked complete at 20:16:44 and CLAUDE.md says `cargo clean` runs before that, no exceptions; `01-VERIFICATION.md` Truth 12 defers the clean to "the orchestrator", which has not happened.

No command run during this review built anything; `cargo tree --offline` and `git` only. `target/` was present before the first command and unchanged after.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: The generated stack block in `.claude/CLAUDE.md` has empty sections and code-comment lines rendered as H1 headings

**File:** `.claude/CLAUDE.md:102`, `115`, `156-159`, `182-188`, `218-226`, `286` (block spans `23-307`, source `research/STACK.md`)
**Issue:** The generator that fills `<!-- GSD:stack-start source:research/STACK.md -->` strips fenced code blocks but keeps any line in them that starts with `#`. The result, in the file that is loaded into every session as project instructions:

- Headings with no body: `### Why a separate launcher process is mandatory, whatever the binding` (102), `### Concrete launcher recipe (sketch; verify method names against the pinned source before coding)` (115), `## Installation` (218), `## Corrections to current planning assumptions` (286). An agent told to "verify method names against the pinned source" finds nothing to verify against.
- Rust attributes from a code sample promoted to H1: lines 156-159 are `#[derive(Deserialize)]`, `#[serde(deny_unknown_fields)]`, `#[derive(Deserialize)]`, `#[serde(try_from = "RawManifest")]` as four top-level headings under "Question 4".
- TOML comments promoted to H1: 182-188 (`# deny.toml`, `# Curated allowlist; with msb_krun[blk] the probe found exactly these build scripts:` and its two continuation lines) and 220-226 (`# Cargo.toml (workspace root), sketch`, `# dev`, `# .cargo/config.toml`, `# crates/minos-core/clippy.toml  (replaces, does not merge with, any root clippy.toml)`).

The 01-05 commit message for `3be27f1` treats STACK.md as the source and warns that edits made only to the generated copy are reverted, so this cannot be fixed by hand-editing `.claude/CLAUDE.md`.
**Fix:** Either make the generator keep fenced blocks intact (the sketches are the useful part of the Installation and recipe sections) or make it drop the whole fenced block including `#` lines and omit any heading whose section becomes empty. Then regenerate. If the block is meant to be prose only, delete the four empty-section headings and the Question 4 / Question 7 / Installation code remnants from STACK.md's prose and point to STACK.md for the sketches with one line.

### WR-02: The Installation sketch that 01-05 edited and declared authoritative contradicts the real `Cargo.toml` on the forbid levels, members, and `rust-version`

**File:** `.claude/CLAUDE.md:218-226` (empty here; source `.planning/research/STACK.md:360-436`)
**Issue:** Commit `3be27f1` changed STACK.md line 387 to `clap = { version = "=4.6.7" }` "because it is the generator source of the .claude/CLAUDE.md stack block". D-14 in 01-CONTEXT.md sends implementers to that same section ("The full v1 dependency table from `.planning/research/STACK.md` (Installation section) is declared in Phase 1"). The rest of the sketch was not brought into agreement with the files it sketches:

| STACK.md line | Sketch says | Repository says |
|---|---|---|
| 366 | `members = ["crates/*", "xtask"]` | `Cargo.toml:5-16` explicit ten-crate list; D-01/D-07: no `xtask` |
| 370 | `rust-version = "1.97"` | `Cargo.toml:21` `"1.97.1"`; `rust-toolchain.toml` `1.97.1` |
| 377-379 | `unwrap_used = "deny"`, `expect_used = "deny"`, `indexing_slicing = "deny"` | `Cargo.toml:45-47` all `forbid` (UAT test 1, D-12) |
| 418-420 | `.cargo/config.toml` is `[target.aarch64-unknown-linux-musl] linker = "rust-lld"` | `.cargo/config.toml:9-10` is the four-lint `[build] rustflags` backstop; no linker key (D-09 defers musl) |
| 424-434 | `clippy.toml` with two methods and two types | `crates/minos-core/clippy.toml` has 10 methods, 7 types, 3 macros |

The forbid row is the one that matters: the sketch now documents the pre-UAT lint levels as the recommended configuration in the file this phase designated as the source of truth for the stack. A reader reconciling the two will not know which one is stale, and the `deny` sketch is what G-01 showed can be bypassed by an inner `#![allow(..., reason)]`.
**Fix:** Update STACK.md 366, 370, 377-379, 418-420, 424-434 to the committed values (or replace the sketch with "see `Cargo.toml`, `.cargo/config.toml`, `crates/minos-core/clippy.toml`; they are the record"), then regenerate `.claude/CLAUDE.md` so WR-01's `## Installation` section either carries the corrected sketch or disappears. Add a one-line note at the top of the Installation section: "Sketch written 2026-10-09 before UAT; the committed files win on any difference."

### WR-03: The stack of record still prescribes an `xtask` in two places after D-01 and D-07 ruled one out

**File:** `.claude/CLAUDE.md:96`, `178` (source STACK.md 89, 176, 247)
**Issue:** Line 96: codesign of `minos-vmm` is "Called from an `xtask` (argv array), not from `build.rs`". Line 178: VM integration tests run via "`cargo xtask vm-test`". 01-CONTEXT.md D-01 says "No `ci` script and no `xtask` crate", D-07 says "no `xtask` member", and the same file at 105 records that "The user prefers no custom tooling, so Phase 5 must decide how `codesign` runs". The stack block is loaded as project instructions, so the next agent that reaches Phase 5 is told to build the thing the user has twice rejected. (`.planning/REQUIREMENTS.md:85` KRUN-08 has the same wording and is outside this review's file list; it should change in the same edit.)
**Fix:** Line 96: "Run after `cargo build` as a documented argv command; the mechanism is a Phase 5 decision (01-CONTEXT.md 105). D-01 rules out an `xtask`." Line 178: "`#[ignore]`d tests run with `cargo test -- --ignored` on a host with the signed `minos-vmm` and libkrunfw present." Make the edits in STACK.md and regenerate.

### WR-04: `crates/minos-core/clippy.toml` bans can each be sidestepped by a sibling std API the list does not name

**File:** `crates/minos-core/clippy.toml:5-30`
**Issue:** The WR-03 fix in the prior round added the paths the review listed, and each resolves (C-03b). But `clippy::disallowed_types` fires only where a type is *written*, and `disallowed_methods`/`disallowed_macros` fire only on the exact paths named, so the following compile in `minos-core` with nothing to fail, defeating the entry next to them:

- Filesystem: `std::fs::OpenOptions::new().read(true).open(p)?` returns a `File` that is never named, so the `std::fs::File` type ban (line 20) does not fire. `std::fs::read_dir`, `metadata`, `copy`, `rename`, `create_dir_all`, `remove_file`, `remove_dir_all` are not in lines 12-14.
- Environment: `std::env::vars()`, `vars_os()`, `current_dir()`, `temp_dir()` sit next to the banned `var`/`var_os`/`args` (lines 9-11).
- Output: `std::io::stdout()` / `stderr()` with `write!`/`writeln!` prints without `println!` (lines 27-28).
- Process and threads: `std::process::exit`, `std::process::abort`, `std::thread::spawn` (core is "sync, no I/O"; a thread is a nondeterminism source under durable replay).
- Network: `std::os::unix::net::UnixStream` / `UnixListener` beside the three `std::net` types (lines 22-24). This matters because the design's local transport *is* a Unix socket, so it is the most likely thing to be reached for from core by mistake.

The list is a denylist and will never be complete; the point is that each existing entry has an adjacent escape, so the comment "minos-core has no filesystem" at line 20 promises more than the lint delivers.
**Fix:**
```toml
disallowed-methods = [
  # ...existing entries...
  { path = "std::fs::read_dir",          reason = "minos-core has no filesystem" },
  { path = "std::fs::metadata",          reason = "minos-core has no filesystem" },
  { path = "std::fs::copy",              reason = "minos-core has no filesystem" },
  { path = "std::fs::rename",            reason = "minos-core has no filesystem" },
  { path = "std::fs::create_dir_all",    reason = "minos-core has no filesystem" },
  { path = "std::fs::remove_file",       reason = "minos-core has no filesystem" },
  { path = "std::fs::remove_dir_all",    reason = "minos-core has no filesystem" },
  { path = "std::env::vars",             reason = "minos-core reads no environment" },
  { path = "std::env::vars_os",          reason = "minos-core reads no environment" },
  { path = "std::env::current_dir",      reason = "minos-core reads no environment" },
  { path = "std::io::stdout",            reason = "minos-core has no I/O" },
  { path = "std::io::stderr",            reason = "minos-core has no I/O" },
  { path = "std::process::exit",         reason = "minos-core returns values, it does not exit" },
  { path = "std::process::abort",        reason = "minos-core returns values, it does not abort" },
  { path = "std::thread::spawn",         reason = "minos-core is single-threaded and deterministic" },
]
disallowed-types = [
  # ...existing entries...
  { path = "std::fs::OpenOptions",           reason = "minos-core has no filesystem" },
  { path = "std::os::unix::net::UnixStream",   reason = "minos-core has no network or IPC" },
  { path = "std::os::unix::net::UnixListener", reason = "minos-core has no network or IPC" },
]
```
Run `cargo clippy -p minos-core --all-targets -- -D warnings` on the untouched crate and confirm zero `unknown path` warnings (a mistyped path only warns, Pitfall 3), then probe one new method, one new type, and one new macro the way V-09b/V-10b did and record it. State in the header comment that the file is a denylist and that review, not the lint, is the guarantee.

### WR-05: `target/` is present after the phase was marked complete; the `cargo clean` rule has not been honored

**File:** `target/` (4090 files, mtimes 20:17-20:23 -0400); rule at `CLAUDE.md` Commands ("Run `cargo clean` at the end of every phase, after verification passes and before the phase is marked complete. No exceptions.")
**Issue:** The phase-complete commit `7214368` ("update STATE and ROADMAP after plan 01-05") is at 20:16:44. `target/debug` and `target/doc` were written at 20:17-20:23 by the round-2 verification, and `01-VERIFICATION.md` Truth 12 says "`target/` exists while I write this; per the orchestrator's instruction `cargo clean` is run by the orchestrator after this report". At 20:27, when this review ran, it had not been. The evidence files record `cargo clean` as the last step of 01-04, 01-05, and both review-fix passes, so every recorded run honored the rule and the one unrecorded run did not. The user memory `cargo-clean-end-of-phase` marks this as a standing rule.
**Fix:** Run `cargo clean` as the last cargo command of whatever closes this workflow and record the exit code and the absence of `target/` in `01-VERIFICATION.md` Truth 12 (replace the deferral with the result). For future phases, make the verifier itself run `cargo clean` last rather than deferring to an orchestrator step that has no evidence line.

## Info

### IN-01: `.claude/CLAUDE.md` still says the root `CLAUDE.md` uses nano-dollar precision; that was corrected in `6b4f6c7`

**File:** `.claude/CLAUDE.md:42`, `192` (source STACK.md)
**Issue:** Decision 8 reads "**CLAUDE.md's "nano-dollar precision" cannot represent published AWS rates**" and Question 8 ends "**Update CLAUDE.md to pico-dollars.**" The root `CLAUDE.md` Cost section already says "pico-dollar precision (10^-12 USD; published AWS rates have 10 fractional digits)" (fix IN-06 of the prior round). The stack block now tells the reader to make a change that has been made.
**Fix:** "CLAUDE.md's Cost section originally said nano-dollars (9 digits), which cannot represent published AWS rates; corrected to pico-dollars on 2026-10-09." in both places; regenerate.

### IN-02: Toolchain targets and the build-script allowlist in the stack block no longer match `rust-toolchain.toml` and `deny.toml`

**File:** `.claude/CLAUDE.md:53`, `184-188`
**Issue:** Line 53: "Add targets `aarch64-apple-darwin` and `aarch64-unknown-linux-musl`." `rust-toolchain.toml` installs no targets and its comment, with D-09 and the root `CLAUDE.md` Commands paragraph, defers musl to Phase 5. Lines 184-188 say the probe "found exactly these build scripts" and list 14 crates; `deny.toml` `allow-build-scripts` has 25 entries (adds `anyhow`, `crossbeam-deque`, `crossbeam-epoch`, `kvm-ioctls`, `num-traits`, `radium`, `rustix`, `rustversion`, `serde_json`, `target-tuple`, `zmij`). The second is labelled as a probe result so it is not false, but it is the only list of build scripts a reader of the stack block sees.
**Fix:** Line 53: "Targets: `aarch64-apple-darwin` now; `aarch64-unknown-linux-musl` added in Phase 5 (D-09)." Lines 184-188: "The probe on `msb_krun[blk]` alone found 14 build scripts; the full v1 graph has 25, listed in `deny.toml` `[bans.build]`."

### IN-03: `clippy.toml` header overstates what was proven

**File:** `crates/minos-core/clippy.toml:3`
**Issue:** "Every path below is a real std path, proven to fire once (01-GATE-EVIDENCE.md V-09, V-10, V-09b, V-10b)." Per `01-REVIEW-FIX.md` WR-03, V-09b/V-10b probed one entry per table (`std::env::var`, `std::fs::File`, `std::println`), and V-09/V-10 covered `SystemTime::now` and `HashMap`. So 5 of 27 entries were proven to fire; the other 22 were shown to *resolve* (C-03b, zero `unknown path` warnings), which is a weaker claim. The distinction matters because the file's own line 4 says a mistyped path only warns.
**Fix:** "Every path below resolves (C-03b: zero `unknown path` warnings on `cargo clippy -p minos-core`). One entry per table was proven to fire (V-09, V-10, V-09b, V-10b). Re-run both checks when you change an entry."

### IN-04: Doctests are outside every forbid lint and outside the C-02 grep

**File:** `Cargo.toml:45-47`, `.cargo/config.toml:10`, `CLAUDE.md` Commands (C-02)
**Issue:** `clippy::unwrap_used`, `expect_used`, and `indexing_slicing` are implemented by clippy-driver. Doctests are compiled by rustdoc with plain rustc, which accepts `-Fclippy::...` tool lints silently and never evaluates them, and `cargo clippy` does not lint doctest bodies at all. So an `.unwrap()` or `x[0]` inside a `///` example in any crate passes `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (runs it), and the C-02 grep (which looks for `allow(` only). The user rule is no unwrap anywhere, including tests and probes; `rules/doc-question-mark.md` already asks for `?` in examples.
**Fix:** Write doc examples as `# fn main() -> Result<(), Box<dyn std::error::Error>> {` ... `# Ok(()) }` with `?`. Add a second per-phase grep next to C-02 in `CLAUDE.md` Commands: `grep -rnE '\.(unwrap|expect)\(' crates --include='*.rs'` must print nothing (it covers doctests because they live in `.rs` files; add `#!?\[allow\(` is already C-02).

---

_Reviewed: 2026-10-10T00:28:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
