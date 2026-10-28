---
phase: 08-cli-sample-project-and-docs
plan: 09
subsystem: cli-inspect
status: complete
tags: [cli, status, log, show, renderer, liveness, cost-line]
requires:
  - minos_cli::address::{HexPrefix, resolve_run, fold_run, resolve_exp, AddressError} (08-06)
  - minos_cli::project::{base_surface, PrepareError} (08-06)
  - minos_cli::render::{write_line, id_text, IdStyle, outcome_word, decision_word, stop_reason_name, percent_of, percent_text, elapsed_text, trial_elapsed_ms, objective_of, run_judge, working_generation, cost_line, utc_text} (08-06)
  - minos_loop::history::{recorded_manifest, recorded_manifest_at, recorded_diff} and minos_loop::record::event_kind (Phase 7, 08-01)
  - minos_store::{Ledger::read, RunLock::lock_run, StoreError::{Corrupt, LedgerTooLarge, RunLocked}}
provides:
  - minos_cli::status::{status, StatusOptions, StatusOutcome, StatusError}
  - minos_cli::log::{log, LogOptions, LogError}
  - minos_cli::show::{show, ShowOptions, ShowError}
  - serde_json as a minos-cli dependency (log --json)
  - test target crates/minos-cli/tests/inspect.rs (14 tests)
affects: [08-12 main.rs wiring of status, log, and show, 08-14 docs]
tech-stack:
  added: []
  patterns:
    - "A ledger the store refuses is reported by status as a health line that says it needs a human, not as an error"
    - "Liveness is a lock_run probe whose guard is dropped at once; RunLocked means a driver holds the run"
    - "Hostile blob text (rationale, stage log) reaches the terminal only through LogExcerpt, indented two spaces"
    - "log attributes generations from the lines themselves, so it needs no fold"
key-files:
  created:
    - crates/minos-cli/tests/inspect.rs
  modified:
    - crates/minos-cli/src/status.rs
    - crates/minos-cli/src/log.rs
    - crates/minos-cli/src/show.rs
    - crates/minos-cli/Cargo.toml
    - Cargo.lock
key-decisions:
  - "The ledger records no generation target, so status shows the concluded count and, while a driver holds the run, the generation in progress (working_generation); D-15's target line never applies in this milestone"
  - "status reports a Corrupt ledger (line and kind), a LedgerTooLarge one, and one that does not fold as `ledger: ...; the ledger needs a human` and returns StatusOutcome { healthy: false }, never an error"
  - "status's torn-tail line counts torn lines, not bytes, because LedgerRead::torn_tail is a line count (0 or 1)"
  - "log builds its experiment-to-generation map from the RunStarted and Proposed lines, so it prints a ledger that reads but does not fold; Spent uses its own recorded generation"
  - "show indents rationale and log lines by two spaces so no hostile line can pose as one of the command's own lines; the canonical diff prints unindented"
  - "StatusError::Record, ShowError::Record, and ShowError::Prepare box their large sources (clippy result_large_err)"
requirements-completed: []
coverage:
  - deliverable: "status: start, concluded count, liveness probe, incumbent against the baseline median and noise floor, CostLine or cost: unavailable, stops, corrupt ledger health"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/inspect.rs (4 status tests)"
        status: pass
  - deliverable: "log: one line per event in seq order, generation and experiment filters, --json byte equality with ledger.jsonl, --full-ids"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/inspect.rs (5 log tests)"
        status: pass
  - deliverable: "show: header, trials, lineage, rationale, diff, opt-in logs, hostile text rendered, ambiguous prefix refused"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/inspect.rs (5 show tests)"
        status: pass
metrics:
  duration: 11 min
  started: 2026-10-28T18:30:00Z
  completed: 2026-10-28
  tasks: 2
  files: 6
---

# Phase 8 Plan 09: `status`, `log`, and `show` Summary

`status` shows a run's fold, liveness, cost, stops, and ledger health. `log` prints one rendered line per event, or with `--json` the stored lines byte for byte. `show` prints one experiment's header, trials, lineage, rationale, and cumulative diff, with stage logs only behind `--logs`. Every line goes through the sanitizing renderer, and hostile blob text goes through `LogExcerpt` only.

## Accomplishments

- **`status`** (`status.rs`). It resolves the run and prints a warning for each unreadable run the latest-run choice skipped. It reads the ledger, then prints:
  - `run <id> started <UTC> (unix <secs>)` and `generations concluded: <n>`.
  - `driver: running (generation <g> in progress)` or `driver: not running`, from a `lock_run` probe whose guard is dropped at once.
  - `incumbent: <id> <metric> <value> (baseline median <m> over <n> trials, <percent>; noise floor spread <s>)`, or `incumbent: baseline (no generation concluded)`. The metric comes from `recorded_manifest`.
  - The `CostLine`, or `cost: unavailable (<why>)` when there is no price table or it cannot price a spend.
  - One `stop: g<gen> <reason>[ ordinal <o>]` per recorded stop, and a torn-tail line when one exists.

  A `Corrupt`, `LedgerTooLarge`, or non-folding ledger prints the run, the driver line, and `ledger: ...; the ledger needs a human`, and the outcome is unhealthy. The module docs say that `status` never appends and creates no run. Its probe opens `driver.lock` exactly as a driver does, so it may create that empty file. The docs also say the ledger records no target.
- **`log`** (`log.rs`). It prints `<seq> <kind> <generation or -> <fields>` per event. The module docs list the fields for every event kind.
  - `--generation N` keeps one generation. `judged` and `report_acknowledged` count under their experiment's generation, and review events have none.
  - `--exp <prefix>` keeps the proposed, judged, candidate and judge spent, report_acknowledged, and selected lines that name the experiment. An absent or ambiguous prefix is a named `AddressError`.
  - `--json` writes `serde_json::to_vec(line)` through `write_line`. With no filter the output equals `ledger.jsonl` (tested).
- **`show`** (`show.rs`). It prints:
  - `experiment <id>` (with ` (baseline)` for the baseline), `run <id>`, and for a proposal `generation <g>  tier <tier>  origin <sweep | model <id>>  parent <id>`.
  - One line per trial: `trial <i>: <word>[ (<Reason>)] <percent> <elapsed>`.
  - `lineage: a -> b -> c`.
  - For a proposal, the rationale (read under `MAX_RATIONALE_BYTES`, rendered through `LogExcerpt::from_bytes(.., MAX_LOG_EXCERPT_BYTES)`, with a `[truncated]` line when cut) and the full cumulative diff. The diff is re-read through `recorded_diff` against `recorded_manifest_at(generation)` and `base_surface`, and rendered through `SafeText`.
  - With `--logs` only, `log trial <i> <stage>:` and each log's `LogExcerpt` tail.
- **`serde_json`** is a `minos-cli` dependency. No new crate entered `Cargo.lock`, and `cargo deny check` passes.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 5425737 | test(08-09): add failing tests for status and log |
| 1 | GREEN | 478919d | feat(08-09): show a run's status and log through the renderer |
| 2 | RED | 0aa0396 | test(08-09): add failing tests for show |
| 2 | GREEN | ba7df50 | feat(08-09): show one experiment with its rationale, diff, and opt-in logs |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The torn-tail line counts lines, not bytes**
- **Found during:** Task 1
- **Issue:** The plan's text is `ledger: <n> torn bytes after the last line ...`. `LedgerRead::torn_tail` is a count of torn lines (0 or 1), and the store never reports a torn tail's bytes (04-CONTEXT D-13).
- **Fix:** The line reads `ledger: <n> torn line after the last complete line; the next append truncates it`.
- **Commit:** 478919d

**2. [Rule 2 - Missing error path] A ledger that reads but does not fold**
- **Issue:** The plan reports `Corrupt` and `LedgerTooLarge`. A ledger whose lines verify but do not fold also cannot be shown.
- **Fix:** `status` reports it as `ledger: does not fold (<FoldError>); the ledger needs a human` and returns unhealthy. `log` builds its generation map from the lines themselves, so it still prints such a ledger.
- **Commit:** 478919d

**3. [Rule 3 - Blocking] `StatusError::Record` and boxed sources**
- **Issue:** The incumbent's metric comes from `recorded_manifest`, which returns a `DriverError`, and the planned `StatusError` had no variant for it. `DriverError` and `PrepareError` are large enough to trip `clippy::result_large_err`.
- **Fix:** Added `StatusError::Record(Box<DriverError>)`. `ShowError::Record` and `ShowError::Prepare` box their sources too. The enums are `#[non_exhaustive]`.
- **Commit:** 478919d, ba7df50

**4. [Rule 3 - Blocking] `log` destructures `LogOptions`**
- **Issue:** clippy `needless_pass_by_value` fired on the by-value options.
- **Fix:** `log` destructures them. The signature is unchanged.
- **Commit:** 478919d

**5. [Rule 3 - Blocking] Runs are driven through `minos_cli::run::run`**
- **Issue:** The plan says to drive the fixture run through `Driver::start`, `calibrate`, and `run_to`.
- **Fix:** The tests call `run::run`, which composes exactly those calls from `Prepared`'s `RunInputs`. That avoids rebuilding the inputs in the test.
- **Commit:** 5425737

Two more points:
- The ambiguous-prefix test drives the sweep run to generation 8, so more than one experiment shares a first hex character. The generation-2 run's four experiments happen to have four distinct first characters. The fixture is deterministic, and the test fails with a clear message if no shared character exists.
- `show` indents rationale and log lines by two spaces, so a hostile line cannot pose as one of the command's own lines.

**Total deviations:** 5 (1 bug, 1 missing path, 3 blocking). **Impact:** none on scope. The public signatures match the plan's acceptance greps.

## TDD Gate Compliance

Each task has a `test(08-09)` RED commit before its `feat(08-09)` GREEN commit:

- **Task 1:** 5425737, then 478919d.
- **Task 2:** 0aa0396, then ba7df50.

Both RED runs failed to compile on the missing `status`/`log` and `show` items. The Task 2 GREEN commit also carries two clippy fixes to the test file: backticked tier names in docs, and an infallible `show_text`. No refactor commit was needed.

## Known Stubs

None. `status`, `log`, and `show` are not yet wired into `main.rs`. Plan 08-12 does that, passing `Some(prices)` to `status` when `prices.toml` reads and `None` otherwise. No requirement ID was ticked in REQUIREMENTS.md, because plan 08-15 marks requirements.

## Threat Flags

None. T-08-09-01 to T-08-09-06 are handled as planned:

- **T-08-09-01.** Rationales and logs pass only through `LogExcerpt`, and the diff and every line through `SafeText`. Tests check for ESC and U+202E.
- **T-08-09-02.** Logs print only with `--logs` (tested).
- **T-08-09-03.** Blobs are read under `MAX_RATIONALE_BYTES`, `MAX_STAGE_LOG_BYTES`, and the diff cap, and excerpts are capped at `MAX_LOG_EXCERPT_BYTES`.
- **T-08-09-04.** A corrupt ledger is unhealthy, with its line and kind (tested).
- **T-08-09-05.** Accepted. The probe holds the lock for an instant and may create `driver.lock`, and the docs say so.
- **T-08-09-06.** Cost prints only through `CostLine`. `assert_no_banned_words` runs over every captured output and error text: 19 call sites.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli --test inspect`: 14 passed. All of `cargo test -p minos-cli` passes.
- `cargo doc -p minos-cli --no-deps`: no warnings
- `cargo deny check`: advisories, bans, licenses, and sources ok
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Acceptance greps: every signature grep prints one line, `needs a human` prints 6 lines, and `lock_run(` prints 1 line. `serde_json.workspace = true` and `serde_json::to_vec` are present. `recorded_diff(` prints 1 line and `LogExcerpt::from_bytes` prints 4. `assert_no_banned_words(` has 19 call sites.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/status.rs, log.rs, show.rs, crates/minos-cli/tests/inspect.rs
- FOUND: 5425737, 478919d, 0aa0396, ba7df50
