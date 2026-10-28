---
phase: 08-cli-sample-project-and-docs
plan: 06
subsystem: cli-library
status: complete
tags: [cli, prepare, prefix-resolution, render, krun-composition]
requires:
  - minos_core::money::CostLine and minos_core::policy::run_cost (08-01)
  - minos_core::state::RunState::proposals (08-01)
  - minos_loop::RunInputs (Phase 7)
  - minos_store::Ledger::runs (08-02)
  - minos_source::{read_files, Snapshot::surface} (08-03)
provides:
  - minos_cli::project::{prepare, Prepared, PrepareError, open_runs, base_surface, prices_path, read_prices, PRICES_FILE, MAX_PRICES_BYTES}
  - minos_cli::jobs::{krun_jobs, ComposeError}
  - minos_cli::address::{HexPrefix, Resolved, resolve_run, resolve_exp, fold_run, AddressError}
  - minos_cli::render::{SHORT_HEX, IdStyle, id_text, outcome_word, reason_name, stop_reason_name, decision_word, percent_of, percent_text, elapsed_text, trial_elapsed_ms, objective_of, run_judge, working_generation, cost_line, utc_text, write_line}
  - the nine command modules check, run, status, log, show, report, adopt, stop, init (docs only)
affects: [08-07 check and run, 08-08 status and log, 08-09 show, 08-10 report and adopt, 08-11 stop and init, 08-12 main.rs wiring, 08-13 the VM end-to-end test]
tech-stack:
  added: []
  patterns:
    - "One preparation path for check and run; every user-fixable refusal names the next command"
    - "Prefix resolution by name only through Ledger::runs; only the latest-run choice reads ledgers"
    - "Every printed line through render::write_line (SafeText); dollars only through CostLine"
key-files:
  created:
    - crates/minos-cli/src/project.rs
    - crates/minos-cli/src/address.rs
    - crates/minos-cli/src/render.rs
    - crates/minos-cli/src/check.rs
    - crates/minos-cli/src/run.rs
    - crates/minos-cli/src/status.rs
    - crates/minos-cli/src/log.rs
    - crates/minos-cli/src/show.rs
    - crates/minos-cli/src/report.rs
    - crates/minos-cli/src/adopt.rs
    - crates/minos-cli/src/stop.rs
    - crates/minos-cli/src/init.rs
    - crates/minos-cli/tests/project.rs
    - crates/minos-cli/tests/address.rs
  modified:
    - crates/minos-cli/src/jobs.rs
    - crates/minos-cli/src/lib.rs
    - crates/minos-cli/src/main.rs
    - CLAUDE.md
key-decisions:
  - "PrepareError::Lock is #[error(transparent)] over LockMismatch, so the top-level message is the lock's own text naming `minos env build`, `minos env build --update`, or `minos judge push`, and anyhow's chain print does not repeat it"
  - "write_line uses SafeText::new, not inline: a diff line keeps its tab, and ESC, CR, and C1 controls are still dropped; one call writes one line plus one newline"
  - "reason_name and stop_reason_name map an unknown future variant of the non_exhaustive Reason and StopReason to `other` rather than failing"
  - "Prepared::judge_data, root_image, judge, and judge_extent return Copy values by value"
requirements-completed: [CLI-05, CLI-06, CLI-07, CLI-10]
coverage:
  - deliverable: "project::prepare builds the run inputs in order, and each refusal names the next command"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/project.rs (13 tests)"
        status: pass
  - deliverable: "open_runs creates nothing; base_surface rebuilds the base from its stored tarball"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/project.rs#open_runs_refuses_a_project_without_a_store_and_creates_nothing"
        status: pass
      - kind: test
        ref: "crates/minos-cli/tests/project.rs#base_surface_reads_the_prepared_base_back_from_the_store"
        status: pass
  - deliverable: "address prefix resolution and the latest-run choice"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/address.rs (7 tests)"
        status: pass
  - deliverable: "render helpers (percent, elapsed, UTC, outcome words, write_line, working_generation)"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/src/render.rs#tests (7 tests)"
        status: pass
  - deliverable: "jobs::krun_jobs composes the production runner and refuses a host whose probes failed"
    human_judgment: true
    rationale: "Compiled and clippy-clean in the gate; it needs a signed helper and real VMs, which plan 08-13 exercises"
  - deliverable: "env build --help states the fixed stage environment"
    human_judgment: false
    verification:
      - kind: command
        ref: "grep -n 'HOME=/scratch/home' crates/minos-cli/src/main.rs"
        status: pass
metrics:
  duration: 8 min
  started: 2026-10-28T17:59:43Z
  completed: 2026-10-28
  tasks: 2
  files: 18
---

# Phase 8 Plan 06: The Shared CLI Library Summary

`project::prepare` turns a project into the run inputs `check` and `run` share (manifest, lock check, snapshot surface, judge entry, blob presence, prices, stored base), and every refusal names the next command. `address` resolves runs and experiments by lowercase hex prefix. `render` holds every output helper, with `write_line` passing each line through `SafeText`. `jobs::krun_jobs` is the production krun composition, and the nine command modules are declared for the wave-3 plans.

## Accomplishments

- **`project.rs`.** `prepare(project, guest, prices, policy)` runs ten steps in order. It canonicalizes the project, reads the manifest under its cap with its exact bytes, and reads `minos.lock` (with none, `NoLock`: "run `minos env build`"). It takes the snapshot under the lock's judge digests, then reads the surface and program back from the snapshot (`Snapshot::surface`) and the Dockerfile from the tarball (`read_files`). It runs `Lock::check` against the Dockerfile and guest digests and `judge_inputs`. It opens the store, checks that the root image and judge data blobs are present (`RootImageMissing` names `minos env build`, `JudgeDataMissing` names `minos judge push <dir>`), reads `prices.toml` (`PricesMissing` names the path and `minos init`), and stores the base under its tree ID (`BaseNotStored` otherwise). The rest of the module:
  - `open_runs` returns `NoRuns` without creating `.minos`.
  - `base_surface` rebuilds a `BaseTree` from the stored base blob under `MAX_BASE_TAR_BYTES`.
  - `prices_path` gives `~/.config/minos/prices.toml`, and `read_prices` reads it under a 1 MiB cap.
- **`jobs::krun_jobs`.** Builds `KrunBackend` over `FileBlobSource`, then refuses with `ComposeError::HostRefused { count }` (naming `minos host doctor`) when any host probe failed. Otherwise it wraps `Runner::new` behind `RunnerJobs`. A runner refusal surfaces as `ComposeError::Runner { what }`, using the cause's fixed text.
- **`address.rs`.** `HexPrefix` accepts 1 to 64 characters of `[0-9a-f]`.
  - `resolve_run` with a prefix matches names from `Ledger::runs` and reads no ledger. An ambiguous prefix lists every full ID in sorted order.
  - With no prefix, `resolve_run` folds each run and keeps the maximum `(started_at, RunId)`. Runs that do not fold are listed in `Resolved::unreadable`, and when no run folds the result is `Unreadable`.
  - `fold_run` refuses an empty run. `resolve_exp` searches the baseline and every proposal.
- **`render.rs`.** Holds every shared output helper:
  - IDs: `SHORT_HEX` (12), `IdStyle`, `id_text`.
  - Closed-set words: `outcome_word`, `reason_name` (11 names), `stop_reason_name` (7), `decision_word`.
  - Numbers and times: `percent_of` (the report's `delta / |incumbent| * 100`), `percent_text`, `elapsed_text`, `trial_elapsed_ms` (candidate plus judge vCPU-ms divided by `RUNNER_VCPUS`), `utc_text` (integer civil-from-days).
  - Run state: `objective_of`, `run_judge`, `working_generation`.
  - Output: `cost_line` (through `CostLine`) and `write_line`.
- **Command modules.** `check`, `run`, `status`, `log`, `show`, `report`, `adopt`, `stop`, and `init` exist with module docs only. They are declared in `lib.rs`, which now has 19 `pub mod` lines and a module-doc bullet for each new module.
- **`env build --help`.** States the fixed stage environment (`PATH=...`, `HOME=/scratch/home`, `TMPDIR=/tmp`, `LANG=C`) and says to put toolchains on that `PATH`, replacing the CARGO_HOME and RUSTUP_HOME advice (G9).
- **CLAUDE.md.** The `minos-cli` workspace line names the new library modules.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 01c32dd | test(08-06): add failing tests for project preparation |
| 1 | GREEN | c41a8e3 | feat(08-06): prepare a project for a run and compose the krun job runner |
| 2 | RED | 8337e14 | test(08-06): add failing tests for run and experiment prefix resolution |
| 2 | GREEN | 05c150e | feat(08-06): resolve runs by prefix, share the output helpers, declare the command modules |
| docs | | b97f077 | docs(08-06): name the CLI library's shared modules in CLAUDE.md |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The `prepare` signature spans six lines**
- **Found during:** Task 1 acceptance check
- **Issue:** The acceptance grep expects the whole `prepare` signature on one line. That line is longer than rustfmt's 100-column width, so `cargo fmt --all --check`, a gate command, wraps it.
- **Fix:** The signature is exactly as planned (`project: &Path, guest: &Path, prices: &Path, policy: CleanPolicy) -> Result<Prepared, PrepareError>`). It is formatted across lines, which a multi-line read of `src/project.rs` lines 305-310 confirms. The gate takes precedence over the single-line grep.
- **Files modified:** crates/minos-cli/src/project.rs
- **Commit:** c41a8e3

**2. [Rule 1 - Bug] `JudgeData` is `Copy`**
- **Found during:** Task 1 clippy (`clone_on_copy`)
- **Fix:** `Prepared::judge_data()` returns `JudgeData` by value, and `krun_jobs` passes it without cloning.
- **Commit:** c41a8e3

**3. [Rule 3 - Blocking] `lib.rs` changed in Task 1**
- `pub mod project;` had to be declared for Task 1's tests to compile. Task 2 then reordered `lib.rs` and filled in its docs, as planned.

**4. [Rule 2 - Docs current] CLAUDE.md workspace line**
- CLAUDE.md lists what the `minos-cli` library holds, so it now names `krun_jobs`, `project`, `address`, `render`, and the command modules. CLAUDE.md was not in `files_modified`.
- **Commit:** b97f077

The tests add a few cases beyond the behavior list:
- A changed guest also names `--update`.
- `open_runs` opens an existing store.
- `prices_path` and `read_prices` round-trip.
- A non-folding run is listed in `unreadable`, and when it is the only run the result is `Unreadable`.
- `utc_text` is checked at 2100-03-01, which is not a leap year.
- `id_text` is checked in both styles.

**Total deviations:** 4 (1 bug, 2 blocking, 1 docs). **Impact:** none on scope.

## TDD Gate Compliance

Each task has a `test(08-06)` RED commit before its `feat(08-06)` GREEN commit: 01c32dd then c41a8e3, and 8337e14 then 05c150e. Both RED runs failed to compile because the module did not exist yet. The `render` unit tests are in-module, as the plan places them, so they arrived with the GREEN commit; the plan's RED target for Task 2 is `tests/address.rs`. No refactor commit was needed.

## Known Stubs

The nine command modules (`crates/minos-cli/src/{check,run,status,log,show,report,adopt,stop,init}.rs`) hold module docs only, by design. Plans 08-07 to 08-11 fill them, and no command is wired into `main.rs` yet. No requirement ID was ticked in REQUIREMENTS.md; per the orchestrator, plan 08-15 marks requirements.

## Threat Flags

None. T-08-06-01 to T-08-06-06 are mitigated as planned:
- Lock and blob checks run before any VM, and each is tested.
- `write_line` applies `SafeText`, tested with an ESC sequence.
- Prefixes are lowercase hex only, and absent or ambiguous prefixes are named errors.
- `resolve_run` and `open_runs` create nothing.
- `krun_jobs` refuses a host with failed probes.
- A missing `prices.toml` refuses the run, naming `minos init`.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli`: pass. lib has 24 tests, project 13, and address 7; every other target passes and the ignored ones were skipped.
- `cargo doc -p minos-cli --no-deps`: no warnings
- `cargo deny check`: advisories, bans, licenses, and sources ok
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Acceptance greps: every Task 2 grep passes, and every Task 1 grep passes except the single-line `prepare` signature (deviation 1).

## Next Phase Readiness

The wave-3 command plans (08-07 to 08-11) can each fill their own module without touching `lib.rs`. They prepare through `project::prepare` or open through `project::open_runs`, name runs with `address`, and print through `render`.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/project.rs, address.rs, render.rs, and the nine command modules
- FOUND: crates/minos-cli/tests/project.rs, crates/minos-cli/tests/address.rs
- FOUND: 01c32dd, c41a8e3, 8337e14, 05c150e, b97f077
