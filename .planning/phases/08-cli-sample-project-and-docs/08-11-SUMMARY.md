---
phase: 08-cli-sample-project-and-docs
plan: 11
subsystem: cli-commands
status: complete
tags: [cli, stop, init, scaffold, prices]
requires:
  - minos_cli::address::{HexPrefix, resolve_run, fold_run} and minos_cli::render::{working_generation, id_text, write_line} (08-06)
  - minos_core::state::RunState::next_user_stop_ordinal (07 WR-01 quick task)
  - minos_store::{RunLock::lock_run, Ledger::append -> Appended}
  - minos_core::{manifest::Manifest::parse_toml, image::base_images, money::Prices::parse_toml}
provides:
  - minos_cli::stop::{stop, StopOutcome, StopError}
  - minos_cli::init::{init, InitKind, UnknownKind, InitOptions, InitReport, PricesFile, InitError, manifest_template, program_template, DOCKERFILE_TEMPLATE, GITIGNORE_TEMPLATE, PRICES_TEMPLATE}
affects: [08-12 main.rs wiring of `minos stop` and `minos init`]
tech-stack:
  added: []
  patterns:
    - "lock_run probe whose guard drops at once; the stop goes through the ledger's append lock only"
    - "symlink_metadata pre-check of every target, then create_new for each file; nothing is ever overwritten"
key-files:
  created:
    - crates/minos-cli/tests/stop.rs
    - crates/minos-cli/tests/init.rs
  modified:
    - crates/minos-cli/src/stop.rs
    - crates/minos-cli/src/init.rs
    - CLAUDE.md
key-decisions:
  - "The scaffold Dockerfile names its base by digest alone (`FROM rust@sha256:8e8c...`), with the tag in a comment, matching examples/logparse/minos.Dockerfile, because env build refuses a tag beside a digest (carry-forward from 08-05)"
  - "The scaffold .gitignore also ignores /judge/target/ (08-RESEARCH Pitfall 4), and never minos.lock, which a user project commits"
  - "InitKind::from_str fails with its own UnknownKind error rather than an InitError variant"
  - "A scaffold write that fails removes the files this call created; a price table that fails after the scaffold leaves the scaffold in place"
requirements-completed: []
coverage:
  - deliverable: "stop refuses with no driver, appends fresh ordinals while held, reports a raced duplicate, and a live driver honours it"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/stop.rs (6 tests)"
        status: pass
  - deliverable: "init scaffolds four valid files per kind, never overwrites, and writes a fail-closed private price table"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/init.rs (8 tests)"
        status: pass
metrics:
  duration: 9 min
  started: 2026-10-28T18:50:43Z
  completed: 2026-10-28
  tasks: 2
  files: 5
---

# Phase 8 Plan 11: `minos stop` and `minos init` Summary

`stop` records a `UserStop` under the next free ordinal, and only when a driver can still honour it. `init` scaffolds a project of any kind and a price table that fails closed, and it never overwrites a file.

## Accomplishments

- **`stop.rs`.** `stop<S: Ledger + RunLock, W: Write>(store, run, out)` runs these steps:
  - It resolves the run by prefix against `Ledger::runs`, or takes the latest. Only a listed run is ever probed, because `lock_run` creates a run directory (Pitfall 8).
  - It probes `driver.lock`. When the lock can be taken, the guard is dropped and the result is `StopError::NoDriver` ("no driver holds run <id>; nothing to stop"), with nothing appended. When the lock is held, `RunLocked` lets the command go on.
  - It appends `StopRequested { working_generation, UserStop, Some(next_user_stop_ordinal) }` through the ledger's own append lock. `New` is `Recorded`, `Duplicate` is `AlreadyRecorded`, and any store error (`Conflict` included) is `StopError::Store`. If no ordinal is free, the result is `OrdinalsExhausted`.
  - It prints three lines through `write_line`: the recorded or already-recorded line, then that the driver stops at its next safe point and never kills a VM, then that the stop has no effect if the run ends first (Pitfall 7).
- **`init.rs`.** `init(project, prices, InitOptions)`:
  - It checks `minos.toml`, `program.md`, `minos.Dockerfile`, and `.gitignore` with `symlink_metadata`. If any of them is present (a file, a directory, or a symlink), it returns `Exists` listing every one, and writes nothing.
  - Otherwise it creates each file with `create_new`, `write_all`, and `sync_all`. If a write fails, it removes the files this call created.
  - It writes the price table only when nothing is at its path, creating the parents with mode 0700 and the file with mode 0600 through `create_new`.
- **Manifest templates.** Each has a comment on every table:
  - `optimize` generalizes the sample.
  - `tune` adds a `batch_size` integer param (8 to 256, step 8), with a `sweep` proposer of batch 4 and then a `single_shot` `haiku` proposer of batch 2.
  - `explore` uses the optimize layout with a `single_shot` proposer of batch 4.
  - Each says the surface files must exist before `minos check`, and that `judge/` is pushed with `minos judge push judge`.
- **`PRICES_TEMPLATE`.** The compute rates are the MicroVM list rates. `[models]` is empty, with a commented example of the four token rates. A manifest template's `haiku` therefore shows up in `missing_models`, so a model run is refused until the user prices it.
- **CLAUDE.md.** The WR-01 open-question bullet now records that the `minos stop` command is built.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 78e1f83 | test(08-11): add failing tests for minos stop |
| 1 | GREEN | 01f3f58 | feat(08-11): stop a running driver at its next safe point |
| 2 | RED | 4f45bbc | test(08-11): add failing tests for minos init |
| 2 | GREEN | 0e73cad | feat(08-11): scaffold a project and a fail-closed price table |
| docs | | 245aae6 | docs(08-11): record the minos stop command in CLAUDE.md's open questions |

## Deviations from Plan

### Auto-fixed Issues

**1. [Carry-forward from 08-05] The Dockerfile is digest-only**
- **Issue:** The plan's `DOCKERFILE_TEMPLATE` used `FROM rust:1.97.1-slim-trixie@sha256:...`. `minos env build` refuses a tag beside a digest.
- **Fix:** The template matches `examples/logparse/minos.Dockerfile`: `FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c`, with the tag in a comment line. `tests/init.rs` asserts that `base_images` accepts it with exactly one base image. No acceptance grep assumed the tagged form.
- **Commit:** 0e73cad

**2. [Rule 2 - Hygiene] The `.gitignore` also lists `/judge/target/`**
- Building the judge locally would otherwise leave an untracked directory, and the clean check would then fail (08-RESEARCH Pitfall 4).

**3. [Rule 2 - Correctness] Partial-write cleanup and `UnknownKind`**
- A failed scaffold write removes the files the call created, so a retry is not refused by its own partial output.
- `InitKind::from_str` has its own `UnknownKind` error, built through `SafeText`.

**4. [Rule 2 - Docs current] CLAUDE.md**
- The open-question bullet that named `minos stop` as remaining work is updated. CLAUDE.md was not in `files_modified`.
- **Commit:** 245aae6

The tests add cases beyond the behavior list:
- `stop` with an absent prefix creates no run, and `stop` on a store with no runs returns `NoRuns`.
- `init` refuses a dangling symlink at a scaffold path and never writes through it.
- Kind parsing is tested.

**Total deviations:** 4 (1 carry-forward, 3 Rule 2). **Impact:** none on scope.

## TDD Gate Compliance

Each task has its RED `test(08-11)` commit before its GREEN `feat(08-11)` commit. Both RED runs failed to compile because the items did not exist yet. No refactor commit was needed.

## Known Stubs

The templates hold `<...>` placeholders for the user to fill in, by design: the program text, and the knob description in `tune`. They are scaffold content, not stubs in code. Neither command is wired into `main.rs` yet; plan 08-12 does that.

## Threat Flags

None. T-08-11-01 to T-08-11-06 and T-08-11-08 are mitigated and tested. T-08-11-07 is accepted, and `stop` prints the caveat.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli`: pass. `stop` has 6 tests, `init` has 8, and the lib has 25; every other target passes.
- `cargo doc -p minos-cli --no-deps`: no warnings
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Every acceptance grep for both tasks passes. `create_new(true)` appears twice.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/stop.rs, crates/minos-cli/src/init.rs, crates/minos-cli/tests/stop.rs, crates/minos-cli/tests/init.rs
- FOUND: 78e1f83, 01f3f58, 4f45bbc, 0e73cad, 245aae6
