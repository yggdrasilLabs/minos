---
phase: 01-workspace-and-gates
plan: 01
subsystem: infra
tags: [cargo, workspace, clippy, rustfmt, toolchain, lints, licenses]
status: complete

requires: []
provides:
  - "Ten-crate Cargo workspace (7 libraries, 3 binaries) with the ARCHITECTURE.md dependency edges"
  - "Full [workspace.lints] baseline inherited by every crate via [lints] workspace = true"
  - "minos-core clippy.toml banning SystemTime::now, Instant::now, HashMap, HashSet"
  - "31 exact-pinned [workspace.dependencies] (7 internal, 24 external) and a committed Cargo.lock (207 packages)"
  - "rust-toolchain.toml pinning 1.97.1; LICENSE-APACHE and LICENSE-MIT"
affects: [01-02 deny.toml, 01-03 gate proofs, 02 spikes, 03 minos-core, 05 krun backend]

tech-stack:
  added: [serde 1.0.229, serde_json 1.0.151, toml 1.1.8, thiserror 2.0.21, anyhow 1.0.104, clap 4.6.7, sha2 0.11.0, base16ct 1.0.0, diffy 0.5.2, tar 0.4.46, ignore 0.4.33, regex 1.13.1, rand_chacha 0.10.0, rand_core 0.10.1, rustix 1.1.5, vsock 0.5.4, msb_krun 0.1.41, backhand 0.25.5, tempfile 3.27.0, tracing 0.1.44, tracing-subscriber 0.3.23, proptest 1.11.0, insta 1.49.0, trybuild 1.0.122]
  patterns:
    - "Every member manifest: [package] fields .workspace = true, then [lints] workspace = true, then <dep>.workspace = true only"
    - "Lint groups at priority -1 so single-lint entries override them; carve-out reasons live in TOML comments"
    - "Internal path dependencies carry version = \"=0.1.0\" so cargo-deny's wildcard check passes without an exception"

key-files:
  created:
    - rust-toolchain.toml
    - rustfmt.toml
    - .gitignore
    - Cargo.toml
    - Cargo.lock
    - LICENSE-APACHE
    - LICENSE-MIT
    - crates/minos-core/clippy.toml
    - crates/*/Cargo.toml (10)
    - crates/<lib>/src/lib.rs (7)
    - crates/<bin>/src/main.rs (3)
  modified: []

key-decisions:
  - "Used RESEARCH.md Code Examples configuration verbatim; no deviation from it was needed"
  - "Kept missing_errors_doc, missing_panics_doc, must_use_candidate enabled (rules require them); carved out only too_many_lines, similar_names, struct_excessive_bools, multiple_crate_versions"
  - "rustix and vsock sit under a cfg(target_os = \"linux\") target table in minos-guest"

patterns-established:
  - "Stub crates are one manifest plus a one-line //! crate doc (bins add fn main() {})"
  - "No root clippy.toml, no .cargo/, no xtask, no scripts: policy lives only in Cargo.toml, crates/minos-core/clippy.toml, rust-toolchain.toml (and deny.toml in 01-02)"

requirements-completed: [GATE-01, GATE-05]

coverage:
  - id: D1
    description: "Workspace lint baseline (unsafe_code forbid; unwrap_used, expect_used, indexing_slicing, allow_attributes, allow_attributes_without_reason deny) inherited by all ten crates"
    requirement: GATE-01
    verification:
      - kind: other
        ref: "cargo clippy --workspace --all-targets -- -D warnings; per-manifest [lints] workspace = true check"
        status: pass
    human_judgment: false
  - id: D2
    description: "minos-core clippy.toml bans clock reads and hash collections"
    requirement: GATE-05
    verification:
      - kind: other
        ref: "grep of crates/minos-core/clippy.toml; clippy on minos-core reports no unreachable-path warning. Firing proofs are Plan 01-03 V-09/V-10"
        status: pass
    human_judgment: false
  - id: D3
    description: "Ten crates with exact ARCHITECTURE.md dependency edges, no C/OpenSSL/libgit2 crate in either aarch64 target graph"
    verification:
      - kind: other
        ref: "cargo metadata ten-line dependency map; cargo tree --target {aarch64-apple-darwin,aarch64-unknown-linux-musl} C-crate grep = 0"
        status: pass
    human_judgment: false

duration: 3min
completed: 2026-10-09
---

# Phase 1 Plan 01: Workspace Skeleton and Lint Baseline Summary

**Ten-crate Rust 1.97.1 workspace with a deny-by-default lint baseline inherited by every crate, minos-core clock and hash-map bans, 31 exact dependency pins, and a committed 207-package Cargo.lock; fmt, clippy -D warnings, and test all pass.**

## Performance

- **Duration:** about 3 min
- **Started:** 2026-10-09T21:23:58Z
- **Completed:** 2026-10-09T21:26:41Z
- **Tasks:** 3
- **Files created:** 28 tracked files

## Accomplishments

- Root `Cargo.toml`: resolver 3, edition 2024, rust-version 1.97.1, `Apache-2.0 OR MIT`, `publish = false`; seven clippy groups at priority -1; the locked CLAUDE.md lints plus `allow_attributes`, `allow_attributes_without_reason`, `undocumented_unsafe_blocks` at deny; four commented carve-outs; rustdoc `broken_intra_doc_links = "deny"`.
- Ten crates (minos-core, -store, -sandbox, -backend-krun, -propose, -runner, -loop libraries; minos-vmm, minos-guest, and minos-cli, whose binary is named `minos`), each with `[lints] workspace = true` and exactly the RESEARCH.md stub dependency edges.
- `crates/minos-core/clippy.toml` with `disallowed-methods` (SystemTime::now, Instant::now) and `disallowed-types` (HashMap, HashSet). No root clippy.toml exists.
- LICENSE-APACHE downloaded from apache.org, SHA-256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` (matches the plan). LICENSE-MIT with `Copyright (c) 2026 Sean McAuliffe`.

## Task Commits

1. **Task 1: Root workspace manifest, toolchain pin, minos-core stub** - `531f2bf` (chore)
2. **Task 2: Six library crate stubs** - `86f8279` (chore)
3. **Task 3: Binary stubs, license files, gate run** - `4780858` (chore)

## Measurements

- **Cargo.lock packages:** 207 (`grep -c '^\[\[package\]\]' Cargo.lock`). The lockfile is platform-independent, so it also lists x86_64/Windows-only crates (`cc`, `bzip2-sys`, `zstd-sys`, `winapi`) that are absent from both aarch64 target graphs (C-crate check below prints 0).
- **Cold** (fresh scratch `CARGO_TARGET_DIR`, registry already cached): fmt + clippy + test = **19 s**. The first-ever clippy run, including the crates.io download, took about 19 s across the three tasks.
- **Warm** (no changes): fmt + clippy + test = **1 s**.
- Unique package nodes in `cargo tree -e all`: 782 for aarch64-apple-darwin, 811 for aarch64-unknown-linux-musl.

## Gate Output (final tree, actual text)

```
$ cargo fmt --all --check
exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
    Checking minos-vmm v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-vmm)
    Checking minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
    Checking minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
    Checking minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
    Checking minos-guest v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-guest)
    Checking minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)
    Checking minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
    Checking minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
    Checking minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
exit 0
$ cargo test --workspace   (filtered to Compiling minos/Finished/Running/Doc-tests/test result lines)
   Compiling minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
   Compiling minos-vmm v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-vmm)
   Compiling minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
   Compiling minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
   Compiling minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
   Compiling minos-guest v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-guest)
   Compiling minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)
   Compiling minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
   Compiling minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
   Compiling minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-bda2b316f1f16fad)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos-0a774703e7599d0a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_core-cae5489e7b9c95c9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_guest-d76265ead7088bad)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_loop-e38611b5e3892c40)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_propose-537621b17e99ad0a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_runner-a4fda74d27c6ec2c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-f05f201c10f3f4b9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_store-0f568eafc6c6e157)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_vmm-ddae4c37ddaf60a6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_backend_krun
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_loop
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_propose
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_runner
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_sandbox
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_store
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0
```

Other acceptance output (actual):

```
unknown lint count in clippy output: 0
minos-backend-krun normal=backhand,minos-core,minos-sandbox,serde_json,tempfile,thiserror dev=
minos-cli normal=anyhow,clap,ignore,minos-backend-krun,minos-core,minos-loop,minos-propose,minos-runner,minos-sandbox,minos-store,regex,serde,sha2,tar,toml,tracing,tracing-subscriber dev=
minos-core normal=base16ct,diffy,rand_chacha,rand_core,serde,serde_json,sha2,thiserror,toml dev=insta,proptest
minos-guest normal=minos-core,rustix,tar,thiserror,vsock dev=
minos-loop normal=minos-core,minos-propose,minos-store,thiserror dev=
minos-propose normal=minos-core,serde,serde_json,thiserror dev=insta
minos-runner normal=minos-core,minos-sandbox,minos-store,thiserror dev=
minos-sandbox normal=minos-core,tar,thiserror dev=trybuild
minos-store normal=base16ct,minos-core,serde,serde_json,tempfile,thiserror dev=proptest
minos-vmm normal=msb_krun,serde,serde_json,thiserror dev=
[('rustix', 'cfg(target_os = "linux")'), ('vsock', 'cfg(target_os = "linux")')]
minos minos-guest minos-vmm
manifests with [lints] workspace = true: 10 of 10
C-crates in aarch64-apple-darwin + aarch64-unknown-linux-musl graphs: 0
workspace.dependencies entries: 31, exact "=" pins: 31
unused manifest key warnings: 0
```

## Decisions Made

- Followed RESEARCH.md Code Examples verbatim for rust-toolchain.toml, Cargo.toml, member manifests, stub sources, and core clippy.toml. No change to any CLAUDE.md lint level.
- The rustdoc comment in Cargo.toml points to "open decision 3 in 01-01-PLAN.md" instead of RESEARCH.md's "Open Question 3"; same meaning.

## Deviations from Plan

None. The plan was executed as written.

Observations for later plans (no action taken):

- **Cold timing method:** `cargo clean` is reserved for Plan 01-03, so the cold number was measured with a throwaway `CARGO_TARGET_DIR` in the session scratchpad. The repo's `target/` was not cleaned.
- **Task 2's version-line grep stops being valid after Task 3.** `grep -rhE '^[a-z0-9_-]+ *= *"' crates/*/Cargo.toml | grep -vE '^(name|description) '` would match minos-cli's `[[bin]]` `path = "src/main.rs"`. Task 3 does not repeat that check. If Plan 01-03 reuses it, exclude `path` as well.
- **tokio is in the aarch64-unknown-linux-musl graph**, only through `minos-vmm -> msb_krun -> msb_krun_devices -> msb-imago`. minos-vmm is a macOS host binary and is never built for Linux in v1. This does not conflict with the "no tokio in v1" stack decision, which is about our own crates. Plan 01-02's deny.toml graph will include it.

## Issues Encountered

None.

## Known Stubs

All ten crates are intentional empty stubs (one `//!` line; bins add `fn main() {}`). This is the plan's goal (D-06). Phase 3 fills minos-core, and later phases fill the rest.

## Open decisions for the user

The four gate commands cannot catch these three gaps on stable Rust 1.97.1. Each was verified in research (RESEARCH.md Open Questions 1 to 3). This plan does not resolve them: it does not change the CLAUDE.md lint levels and it does not add a gate command.

1. **An inner `#![allow(..., reason = "...")]` passes.** clippy's `allow_attributes` only lints outer `#[allow]`. `#![allow(clippy::unwrap_used, reason = "...")]` plus an `unwrap()` passes `cargo clippy --workspace --all-targets -- -D warnings`. A reasonless inner allow is still caught by `allow_attributes_without_reason`. Options: (A) accept and document the gap; (B) raise `unwrap_used`, `expect_used`, and `indexing_slicing` to `forbid`, which also makes `#[expect]` impossible for those three and is an edit to CLAUDE.md Code rules.
2. **A crate that omits `[lints] workspace = true` escapes every workspace lint.** Cargo rejects a partial override but not a missing table. Options: (A) the one-time check this phase runs, plus review of every new crate manifest; (B) a `.cargo/config.toml` `[build] rustflags` backstop (`-Funsafe_code`, `-Dclippy::unwrap_used`, `-Dclippy::expect_used`, `-Dclippy::indexing_slicing`), which duplicates the lint list, is silently overridden by a `RUSTFLAGS` environment variable, and is not one of D-01's policy files.
3. **`broken_intra_doc_links = "deny"` is enforced only by `cargo doc`.** `cargo clippy` and `cargo test` ignore `[workspace.lints.rustdoc]`; only `cargo doc --workspace --no-deps` fails on a broken link. Options: (A) add `cargo doc --workspace --no-deps` as a fifth command, which changes CLAUDE.md Commands and ROADMAP Phase 1 criterion 1; (B) document that the rustdoc table takes effect only when docs are built.

## Next Phase Readiness

- Plan 01-02 can add deny.toml against this exact graph. All 24 external workspace dependencies are used by at least one crate, so `[bans.workspace-dependencies] unused = "deny"` should pass.
- Plan 01-03 can run the violation proofs. Lint firing (V-01 to V-10) is not yet proven; this plan only shows the clean tree passes and that no lint name is unknown.

## Self-Check: PASSED

- 28 of 28 created files are tracked in git
- Commits 531f2bf, 86f8279, 4780858 found in `git log`
- `git status --porcelain` outside `.planning` is empty after the Task 3 commit

---
*Phase: 01-workspace-and-gates*
*Completed: 2026-10-09*
