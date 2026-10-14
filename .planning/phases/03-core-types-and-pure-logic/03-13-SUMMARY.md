---
phase: 03-core-types-and-pure-logic
plan: 13
subsystem: minos-core
status: complete
tags: [core, reexports, design-of-record, validation, evidence, gate, cargo-clean]
requires:
  - "03-01 to 03-12: every minos-core module, test file, and recorded deviation"
provides:
  - "lib.rs: explicit crate-root pub use re-exports of the principal types (17 pub use lines, no globs); every module stays public"
  - "CLAUDE.md: minos-core module list as built, Policy paragraph, sweep sentence, v1 Usage fields, v1 report flags"
  - "03-VALIDATION.md: per-requirement verification map (CORE-01 to CORE-17, COST-01, GATE-06), nyquist_compliant true"
  - "03-EVIDENCE.md: five deliberate violations with test runs and probe runs, and the final gate"
affects: [phase-04, phase-05, phase-06, phase-07, phase-08]
tech-stack:
  added: []
  patterns:
    - "Flat crate-root API through explicit pub use per module, with the modules kept public so existing paths resolve"
    - "Violation evidence by probe: swap a test's expected string for a wrong value, capture the assertion's left value (the real error Display), restore the file from git"
key-files:
  created:
    - .planning/phases/03-core-types-and-pure-logic/03-EVIDENCE.md
  modified:
    - crates/minos-core/src/lib.rs
    - CLAUDE.md
    - .planning/phases/03-core-types-and-pure-logic/03-VALIDATION.md
decisions:
  - "minos-core keeps every module public and adds explicit root re-exports; no name collided, so every planned name is at the root"
  - "GATE-06 stayed pending in 03-VALIDATION.md until cargo clean actually ran, and was flipped to green in the final docs commit"
  - "The guest musl clippy command (the sixth CLAUDE.md gate command) was run with the plan's ten checks, as step 8b"
metrics:
  duration: "6 min"
  completed: 2026-10-14
  tasks: 3
  files: 4
---

# Phase 3 Plan 13: Phase close (re-exports, design of record, validation, evidence, gate) Summary

This plan closes Phase 3:

- `minos-core` now has a flat crate-root API through explicit `pub use` re-exports.
- CLAUDE.md describes what the phase built, with `tool.rs` (TIER-01), `select_pareto` (POL-01), and the runtime-behavior flags (RPT-01) named as deferred.
- The validation map is complete and marked Nyquist-compliant.
- The five deliberate violations are recorded with each one's real rejection text.
- The full gate passed and was recorded. Only then did `cargo clean` run, as the last cargo action of the phase.

## What was built

- **lib.rs (Task 1).** After the `pub mod` lines there are 17 explicit `pub use` lines, grouped by module, each with a one-line comment:
  - **id:** IDs, `IdHasher`, `MetricName`, `ModelId`
  - **path, limits, text:** `RelPath`, `Limits`, `SafeText`, `LogExcerpt`
  - **metric, outcome, usage, money:** `Metric`, `Scorecard`, `Outcome`, `Reason`, `Stage`, `Usage`, `Rate`, `Prices`, `Cost`, `ApproxCost`
  - **image, manifest:** `ImageRef`, `Manifest`, `JudgeVersion`, `JudgeSpec`, `Objective`, `Goal`, `Tier`
  - **diff, experiment:** `Diff`, `diff::apply::Tree`, `Experiment`
  - **ledger:** `Event`, `Record`, `Line`, `EventKey`, `SpendId`, `StopReason`, `Decision`, `BackendInfo`
  - **state, job, report:** `RunState`, `fold`, `JobRequest`, `JobResult`, `Report`, `build_report`

  There are no glob re-exports and no names collided. The crate doc gains a paragraph that names the entry points: `Manifest::parse_toml`, `Diff::parse`, `fold`, `policy::conclude`, `route::route`, and `build_report`. Every module path that earlier plans and tests use still resolves.
- **CLAUDE.md (Task 2).** Only the five places the plan names changed:
  - **Module list:** the real modules, and that `tool.rs` arrives with TIER-01 while the `requires` table lives in manifest.rs.
  - **Policy:** the lower median per metric (D-11) and multiply-and-compare `min_improvement`. `conclude` re-checks recorded scorecards. `noise_check` (D-12), `route` (D-13), `circuit_breaker` (D-14), and `spend_ok` live in core. `select_pareto` is deferred with POL-01.
  - **Proposer ladder:** the ChaCha8 sweep with Minos's own range mapping and anchored-line rendering.
  - **Cost:** the six v1 `Usage` fields, string rates, and `ApproxCost` as the one dollar formatter.
  - **Report and adopt:** the v1 flags plus weak isolation in provenance. The runtime-behavior flags arrive with RPT-01.
- **03-VALIDATION.md (Task 2).**
  - One row per requirement, with real task IDs, waves, threat refs, test files, and commands. The CORE-03 row keeps its 2026-10-14 amendment.
  - Frontmatter: `status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`.
  - The Wave 0 and sign-off checklists are ticked.
  - A Deviations list records three choices: no `tests/common` module, inline fixtures, and proptest regression files kept beside their tests.
- **03-EVIDENCE.md (Tasks 2 and 3).** It has one section per deliberate violation:
  - `Diff::parse` rename: `line 2: forbidden header: rename`
  - `Manifest::try_from` overlap: `src/parse.rs is writable, so it cannot also be in surface.readable`
  - `Metric::new` NaN: `a metric must be a finite number`
  - `Rate::parse` with 13 fractional digits: `a rate has at most 12 fractional digits and is never rounded`
  - `fold` duplicate `RunStarted`: `the ledger has more than one RunStarted record`

  Each section shows the `--exact` test run (`test result: ok`) and a probe run. In the probe, the test's expected string was temporarily swapped for a wrong value, so the failing assertion printed the real error Display as `left:`. The test file was then restored with `git checkout -- <file>`, and no probe edit was committed. The file also has the Final gate section.

## Final gate (Task 3, GATE-06)

All of these ran at `ec1eba6`, in the plan's order, and passed:

| # | Check | Result |
|---|---|---|
| 1 | `cargo fmt --all --check` | exit 0, no output |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| 3 | `cargo test --workspace` | exit 0; 40 test binaries, 713 passed, 0 failed, 25 ignored by design |
| 4 | `cargo deny check` | exit 0; `advisories ok, bans ok, licenses ok, sources ok` (duplicate-version and existing unused-wrapper warnings only) |
| 5 | `cargo doc --workspace --no-deps` | exit 0, no warning |
| 6 | C-01 | 10 `ok` lines, no `MISSING` |
| 7 | C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | no output |
| 8 | `cargo check -p minos-core --target aarch64-unknown-linux-musl` | exit 0 |
| 8b | `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | exit 0 |
| 9 | `git log --oneline cf607f2..HEAD -- crates/minos-core/clippy.toml` | no output |
| 10 | `.snap.new` count and `git status --porcelain crates/minos-core` | `0`, clean |

The evidence was committed in `f2861b8`. Then, as the final cargo action of the phase, `cargo clean` ran:

```
$ cargo clean
     Removed 57513 files, 6.2GiB total
exit status: 0
```

Afterwards `test ! -d target || test -z "$(ls -A target)"` succeeded. No cargo command ran after `cargo clean`. The remaining steps were the 03-VALIDATION.md GATE-06 status flip, a wording fix in 03-EVIDENCE.md, this SUMMARY, and the GSD state updates. They used only python, git, and the node-based gsd-tools.

## Verifier note (GATE-06)

The phase's `cargo clean` ran after the full gate passed. The orchestrator's post-execution verification must not rebuild `target/` after this point. If the verifier runs any cargo command that rebuilds `target/`, it reruns `cargo clean` as its own last step, after its checks pass and before the phase is marked complete.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 | 7893236 | feat(03-13): re-export the principal minos-core types at the crate root |
| 2 | ec1eba6 | docs(03-13): reconcile CLAUDE.md, complete the validation map, record violation evidence |
| 3 | f2861b8 | docs(03-13): record the final Phase 3 gate in 03-EVIDENCE.md |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Evidence strength] Probe runs added to each violation section**
- **Found during:** Task 2
- **Issue:** A passing `--exact` run shows that the assertion holds, but it never prints the rejection text itself.
- **Fix:** For each test, the expected string was temporarily replaced, the failing run was captured to show the real Display as `left:`, and the file was restored from git. This follows the project rule for deliberate-violation evidence: introduce, capture verbatim, revert, and leave the tree clean.
- **Files modified:** 03-EVIDENCE.md only. The test files were restored, and `git status` confirmed it.
- **Commit:** ec1eba6

**2. [Rule 2 - Gate completeness] Guest musl clippy run as step 8b**
- **Found during:** Task 3
- **Issue:** The plan's ten checks omit `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, which CLAUDE.md and the project rules list as part of the full gate.
- **Fix:** It ran before `cargo clean`, exited 0, and is recorded.
- **Commit:** f2861b8

**3. [Ordering] GATE-06 marked green only after `cargo clean`**
- **Found during:** Task 2
- **Issue:** The plan says to mark statuses green in Task 2, but GATE-06 is not met until the clean runs.
- **Fix:** The row stayed pending in the Task 2 commit and was flipped in the final docs commit, after `cargo clean`.

**4. [Shell] Step 10 recorded under `/bin/sh`**
- **Issue:** Under zsh, the unmatched `*.snap.new` glob printed `no matches found` alongside the correct `0`.
- **Fix:** The step was rerun under `/bin/sh`, and the evidence notes the difference.

No code deviation. `clippy.toml`, `deny.toml`, the lints, and the tests are unchanged.

## Known Stubs

None. The minos-core modules are complete for Phase 3. The deferred items are named in CLAUDE.md: `tool.rs` (TIER-01), `select_pareto` (POL-01), and the runtime-behavior flags (RPT-01).

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-13-01 | Verbatim output and exit statuses for every check | 03-EVIDENCE.md Final gate |
| T-03-13-02 | The C-02 grep is empty, `clippy.toml` is unchanged since cf607f2, and no lint or ban was edited | Steps 7 and 9 |
| T-03-13-03 | CLAUDE.md matches the built modules, and the deferred items are named | CLAUDE.md minos-core, Proposer ladder, Cost, Report and adopt |
| T-03-13-04 | Core builds for `aarch64-unknown-linux-musl` | Step 8 |

## Self-Check: PASSED

- FOUND: crates/minos-core/src/lib.rs (17 `pub use` lines), CLAUDE.md, 03-VALIDATION.md (`nyquist_compliant: true`, 0 `TBD`), 03-EVIDENCE.md (6 `## ` sections, 5 `test result: ok`)
- FOUND commits: 7893236, ec1eba6, f2861b8
- `target/` is absent or empty after `cargo clean`
