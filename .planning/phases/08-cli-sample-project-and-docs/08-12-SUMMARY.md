---
phase: 08-cli-sample-project-and-docs
plan: 12
subsystem: cli-binary
status: complete
tags: [cli, clap, main, exit-codes, provider-preflight, krun]
requires:
  - minos_cli::project::{prepare, open_runs, prices_path, read_prices} and minos_cli::jobs::krun_jobs (08-06)
  - minos_cli::check::{check, CheckOptions, ProviderStatus} (08-07)
  - minos_cli::run::{run, RunOptions, RunEnded} (08-08)
  - minos_cli::{status, log, show} (08-09), minos_cli::{report, adopt} (08-10), minos_cli::{stop, init} (08-11)
  - minos_propose::{ClaudeCli::preflight, ParentEnv::from_vars, Proposer}
provides:
  - "the `minos` binary's full command tree: init, check, run, stop, status, log, show, report, adopt beside host, env, judge"
  - "exit codes: run 2 for a cap, breaker, or provider stop; check 1 only on a refusal; stop and status 1; any error 1"
  - crates/minos-cli/tests/cli_binary.rs (the binary's own wiring, no VM, no model call)
affects: [08-13 the VM end-to-end test drives this binary, 08-14 docs, 08-15 requirement marking]
tech-stack:
  added: []
  patterns:
    - "clap builder only; prefixes through HexPrefix::from_str and hashes through ReportHash::from_str as value parsers"
    - "one long-help const per command, checked recursively for estimate and exact"
    - "the clock, HOME, the provider, and the backend are chosen in main.rs only"
key-files:
  created:
    - crates/minos-cli/tests/cli_binary.rs
  modified:
    - crates/minos-cli/src/main.rs
    - CLAUDE.md
key-decisions:
  - "check and run run ClaudeCli::preflight before composing krun_jobs, so a refused run starts nothing and sweeps nothing; check reports the refusal as ProviderStatus::Refused and still prints the rest"
  - "status reads prices.toml after open_runs and, when HOME or the table is missing or bad, warns on stderr through SafeText and passes None, so progress still prints"
  - "run's exit 2 shares its number with clap's usage errors; CLAUDE.md states both"
requirements-completed: []
coverage:
  - deliverable: "every Phase 8 command parses with its decided flags and refuses bad arguments"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/src/main.rs tests::phase_8_commands_parse_with_their_flags, tests::phase_8_commands_refuse_bad_arguments"
  - deliverable: "no help text says estimate or exact"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/src/main.rs tests::no_help_text_says_estimate_or_exact"
  - deliverable: "the binary's wiring end to end without a VM: --help lists 12 commands, init scaffolds then refuses, status/stop/log/show/report/adopt refuse without .minos and create nothing, bad arguments exit 2, banned words checked on every stream"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/cli_binary.rs (8 tests)"
  - deliverable: "check and run against a real project with the production backend and provider"
    human_judgment: true
    verification:
      - kind: deferred
        ref: "08-13 (real VMs); the live provider path stays under MINOS_CLAUDE_LIVE"
metrics:
  duration: "about 15 minutes"
  completed: 2026-10-28
---

# Phase 8 Plan 12: The `minos` Binary's Command Tree and Dispatch Summary

The `minos` binary now runs all nine Phase 8 commands. They are built with clap's builder API and dispatched from `main.rs`. That file is the one place `ClaudeCli` meets the krun backend and the place that reads the clock and `HOME`. Exit codes follow D-09, D-13, and D-15. A binary-level test runs the built `minos` with a cleared environment. It boots no VM and calls no model.

## What was built

- **Command tree** (`crates/minos-cli/src/main.rs`):
  - `init --kind <tune|optimize|explore>` takes its value through `PossibleValuesParser` and then `InitKind::from_str`.
  - `check [--fresh] [--allow-dirty]`.
  - `run --generations <N>` takes a `u32` of at least 1 (`range(1..)`), with optional `[--run <prefix>] [--allow-dirty]`.
  - `stop [<run>]`.
  - `status [--run] [--full-ids]`.
  - `log [--run] [--generation N] [--exp] [--json] [--full-ids]`.
  - `show <exp> [--run] [--logs] [--full-ids]`.
  - `report <exp> [--run]`.
  - `adopt <exp> --reviewed <hash> [--run] [--out <path>]`.
  - Every run and experiment prefix goes through `HexPrefix::from_str`, and `--reviewed` goes through `ReportHash::from_str`.
  - Each command has a long-help `const`. The `status` help does not call the command read-only, because its probe creates an empty `driver.lock`.
- **Dispatch:** one function per command, each returning `anyhow::Result<ExitCode>`.
  - `check` and `run` each call `prepare` with the located guest, `$HOME/.config/minos/prices.toml`, and `CleanPolicy::AllowDirty` only under `--allow-dirty`.
  - When a `[[proposer]]` names a model, both run `ClaudeCli::preflight(&ParentEnv::from_vars(env::vars_os()))` once per command:
    - `check` turns a refusal into `ProviderStatus::Refused` and exits 1.
    - `run` returns the error, so nothing starts, and otherwise uses `Proposer::with_provider`.
  - Both then compose `krun_jobs(host_paths(..))`, which refuses a host whose probes failed and names `minos host doctor`.
  - `run` exits 0 on `Reached` or on a `UserStop`, and 2 on any other stop.
  - The read-only commands call `open_runs` first, so a project with no `.minos` is refused with "no runs recorded" and nothing is created.
  - `status` exits 1 when the ledger is unhealthy.
- **`tests/cli_binary.rs`:** 8 tests covering:
  - `--help` lists all 12 commands.
  - Every command's `--help` exits 0.
  - `init` writes the four files and the price table, then refuses a second time with `already exist`.
  - `init` keeps an existing price table.
  - `status` and `stop` refuse without runs.
  - `log`, `show`, `report`, and `adopt` refuse without runs and create no `.minos`.
  - Bad arguments exit with clap's usage status 2.
  - `assert_no_banned_words` runs over stdout and stderr of every invocation.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 (RED) | a27dfc4 | test(08-12): add failing parse and help-text tests for the Phase 8 commands |
| 1 (GREEN) | 42b82c3 | feat(08-12): add the nine Phase 8 commands to the minos command tree |
| 2 | 21c4e60 | feat(08-12): dispatch the Phase 8 commands with the production provider and backend |

## Verification

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p minos-cli`: all targets pass. The bin has 11 tests and `cli_binary` has 8.
- `cargo doc --workspace --no-deps`: no warnings.
- `cargo deny check`: advisories, bans, licenses, and sources all pass.
- `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing.
- Acceptance greps all match:
  - every `Command::new("<cmd>")` is present
  - `range(1..)`
  - `krun_jobs(`
  - `ClaudeCli::preflight`
  - `SystemTime::now`
  - `ExitCode::from(2)`
  - `fn assert_no_banned_words`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Keep the design of record current] CLAUDE.md's CLI section**
- **Found during:** Task 2
- **Issue:** CLAUDE.md was out of date in three places:
  - The workspace layout said `minos-cli` ships only `env build`, `judge push`, `host doctor`, and `host setup`.
  - The CLI block said `check` builds, which D-06 rejects.
  - The CLI block listed `stop <run>` with the run required.
- **Fix:** The layout line now names the Phase 8 commands. The `check` and `stop` lines are corrected. One paragraph now gives the exit codes, when the preflight runs, and that the binary alone reads the clock and `HOME`.
- **Files modified:** CLAUDE.md
- **Commit:** 21c4e60

**2. [Minor] `check` and `run` call `check(.., CheckOptions::new(fresh, started_at), ..)` with six arguments**
- **Issue:** The plan sketches eight arguments. 08-07 folded `started_at` and the driver options into `CheckOptions`, and its default driver options are already the production retry pause.
- **Fix:** No `with_driver` call was needed. `run` passes `DriverOptions::default()`, as the plan says.

**3. [Rule 2] `status` handles a missing `HOME` as a warning**
- **Issue:** The plan makes a missing `HOME` an error for the commands that read the price table. For `status`, the table is optional.
- **Fix:** A missing `HOME` and an unreadable table both produce the same `minos: warning:` line, and the command continues with `None`.

## Known Stubs

None.

## Threat Flags

None. The binary adds no new surface beyond the plan's threat model. T-08-12-01 to T-08-12-05 are mitigated as planned:
- The preflight refuses credential variables by name.
- The value parsers validate prefixes and hashes.
- Errors and warnings print through `SafeText`.
- `krun_jobs` refuses an unprobed host.
- A help-text word test guards the help.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/main.rs
- FOUND: crates/minos-cli/tests/cli_binary.rs
- FOUND: a27dfc4, 42b82c3, 21c4e60
