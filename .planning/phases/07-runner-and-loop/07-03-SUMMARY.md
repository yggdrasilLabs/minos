---
phase: 07-runner-and-loop
plan: 03
subsystem: workspace scaffolding (minos-core, minos-runner, minos-loop manifests and clippy policy)
status: complete
tags: [dependencies, clippy, layering, evidence, wave-0]
requires: []
provides:
  - "minos-core dev-dependency trybuild (for 07-06's CumulativeDiff compile-fail proof)"
  - "minos-runner dev-dependencies tar and proptest; regular deps still exactly minos-core, minos-store, minos-sandbox, thiserror"
  - "minos-loop dependencies serde and serde_json; dev-dependencies proptest, insta, tempfile"
  - "crates/minos-runner/clippy.toml: 8 bans (SystemTime::now, Instant::now, env::var, env::var_os, HashMap, HashSet, process::Command, fs::File)"
  - "crates/minos-loop/clippy.toml: the same 8 plus minos_core::id::ExperimentId::of (D-12); thread::sleep allowed for D-16"
  - ".planning/phases/07-runner-and-loop/07-EVIDENCE.md with ## Clippy bans fire (07-03)"
affects: [07-06, 07-07, 07-09, 07-10, 07-12, 07-16]
tech-stack:
  added: []
  patterns:
    - "Every phase dependency edge lands in one Wave 0 plan so same-wave plans stay file-disjoint on Cargo.lock"
    - "A disallowed-methods ban on a workspace crate's own method by full path; it also fires through the crate-root re-export"
key-files:
  created:
    - crates/minos-runner/clippy.toml
    - crates/minos-loop/clippy.toml
    - .planning/phases/07-runner-and-loop/07-EVIDENCE.md
  modified:
    - crates/minos-core/Cargo.toml
    - crates/minos-runner/Cargo.toml
    - crates/minos-loop/Cargo.toml
    - Cargo.lock
decisions:
  - "The HashMap and HashSet probes construct the type rather than take it as a parameter, because a parameter also trips the pedantic implicit_hasher lint; with constructions every probe error is a ban diagnostic"
  - "The ExperimentId::of ban was proven through both spellings, minos_core::id::ExperimentId::of and the crate-root re-export minos_core::ExperimentId::of, so a re-export cannot route around D-12"
  - "The loop's process::Command reason names ModelProvider and JobRunner as its only ways out, matching the runner's Backend::launch reason"
metrics:
  duration: "about 3 minutes"
  completed: 2026-10-22
  tasks: 2
  files: 7
---

# Phase 7 Plan 03: Wave 0 dependency edges and runner/loop clippy policy Summary

Every Phase 7 dependency edge added once from existing workspace pins (no new package in Cargo.lock, deny.toml untouched), and both new crates now carry a clippy policy that makes LOOP-03 (no clock in the runner) and D-12 (no hand-minted `ExperimentId` in the loop) compile-time facts, each ban proven to fire in 07-EVIDENCE.md.

## What was built

**Task 1 (764663d):** `minos-core` gained dev `trybuild`; `minos-runner` gained dev `tar` and `proptest`; `minos-loop` gained `serde`, `serde_json`, and dev `proptest`, `insta`, `tempfile`. `git diff Cargo.lock` shows only the dependency lists of the three crates changing, with zero `+name = ` lines. `cargo deny check` exits 0 (its seven `duplicate` warnings predate this plan). The internal-crate lanes are unchanged: runner on core, store, sandbox; loop on core, store, propose.

**Task 2 (4218064):** `crates/minos-runner/clippy.toml` (8 entries) and `crates/minos-loop/clippy.toml` (9 entries, adding `minos_core::id::ExperimentId::of`). The loop's header comment records that `std::thread::sleep` stays allowed for D-16's retry pause. Temporary probes in both `src/lib.rs` stubs produced exactly one error per entry (two for `ExperimentId::of`, one per spelling) and no other diagnostic; the stubs were restored byte-identical (SHA-256 checked before and after) and both crates' clippy reran green. `07-EVIDENCE.md` records the probes, the verbatim errors, the per-entry checklist, and the exit codes.

## Verification

Full gate, each exit 0: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` (1428 passed, 0 failed, 23 ignored); `cargo deny check`; `cargo doc --workspace --no-deps`; `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`. C-01 printed only `ok` lines for all 11 crates; the C-02 grep printed nothing.

Acceptance greps: `path = ` counts are 8 (runner) and 9 (loop); `std::thread::sleep` appears in the loop's clippy.toml only on a `#` comment line; the evidence section contains `disallowed_methods`, `disallowed_types`, and `ExperimentId::of`; `git diff --exit-code` on both `src/lib.rs` files exits 0.

## Deviations from Plan

None in the deliverables. One adjustment to the temporary probes (not committed code): the first run's HashMap and HashSet probes took the type as a `&` parameter, which also fired clippy's pedantic `implicit_hasher`; they were changed to constructions so the recorded run shows only ban diagnostics. Recorded in 07-EVIDENCE.md.

## Known Stubs

None added. `crates/minos-runner/src/lib.rs` and `crates/minos-loop/src/lib.rs` remain the one-line stubs later plans fill in.

## Self-Check: PASSED

- FOUND: crates/minos-runner/clippy.toml, crates/minos-loop/clippy.toml, .planning/phases/07-runner-and-loop/07-EVIDENCE.md
- FOUND: commits 764663d, 4218064
