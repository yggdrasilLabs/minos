---
phase: 08-cli-sample-project-and-docs
plan: 10
subsystem: cli-review-gate
status: complete
tags: [cli, report, adopt, review-gate, patch, golden, cost-wording]
requires:
  - minos_cli::address::{HexPrefix, resolve_run, fold_run, resolve_exp, AddressError} (08-06)
  - minos_cli::project::{base_surface, PrepareError} (08-06)
  - minos_cli::render::{write_line, id_text, IdStyle, run_judge} (08-06)
  - minos_loop::history::{recorded_manifest_at, recorded_diff} (Phase 7, 08-01)
  - minos_core::report::{build_report, ReportInput, Report, ReportError} (Phase 3)
provides:
  - minos_cli::report::{build, report, BuiltReport, ReportCmdError}
  - minos_cli::adopt::{adopt, AdoptOptions, AdoptOutcome, AdoptError}
  - the patch file at <project>/minos-<12 hex>.patch, or --out, created with create_new
  - test targets crates/minos-cli/tests/report_adopt.rs (9 tests) and crates/minos-cli/tests/patch_tools.rs (3 tests)
affects: [08-12 main.rs wiring of report and adopt, 08-13 the VM end-to-end test, 08-14 docs]
tech-stack:
  added: []
  patterns:
    - "adopt rebuilds the report with the same build function report prints, so the hash compared is the hash of what a user would read now"
    - "Order in adopt: hash compare, then ReportAcknowledged append, then create_new; each refusal stops before the next step"
    - "The patch file keeps the canonical bytes the experiment ID hashes; only the on-screen copy passes SafeText"
    - "Patch golden compares bytes, never exit codes alone, because git apply from a subdirectory exits 0 doing nothing"
key-files:
  created:
    - crates/minos-cli/tests/report_adopt.rs
    - crates/minos-cli/tests/patch_tools.rs
  modified:
    - crates/minos-cli/src/report.rs
    - crates/minos-cli/src/adopt.rs
key-decisions:
  - "ReportCmdError::Record and ::Prepare box DriverError and PrepareError (clippy result_large_err); AdoptError::Report is transparent over ReportCmdError"
  - "A patch write or sync that fails removes the file create_new made, so a partial patch never stays behind; the write error is the one reported"
  - "adopt prints the bare file name in the apply commands when the patch is in the project, and the full path for --out"
  - "AdoptOutcome's bool getter is newly_acknowledged(), as the plan names it"
requirements-completed: []
coverage:
  - deliverable: "report prints Report::text, the hash, and the adopt command, and appends nothing; the baseline is refused"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/report_adopt.rs#report_prints_the_text_the_hash_and_the_adopt_command_and_appends_nothing"
        status: pass
      - kind: test
        ref: "crates/minos-cli/tests/report_adopt.rs#report_refuses_the_baseline"
        status: pass
  - deliverable: "adopt refuses any hash but the current one (including a stale report after a new trial), appending and writing nothing"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/report_adopt.rs#adopt_refuses_a_hash_that_is_not_the_reports_and_records_and_writes_nothing"
        status: pass
      - kind: test
        ref: "crates/minos-cli/tests/report_adopt.rs#a_new_trial_makes_the_old_hash_stale"
        status: pass
  - deliverable: "adopt records ReportAcknowledged, writes the canonical cumulative diff with create_new, refuses an existing file or symlink, and honours --out"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/report_adopt.rs (5 adopt write tests)"
        status: pass
  - deliverable: "the patch applies byte for byte under git apply and patch -p1 outside a repository, at a root, and in a subdirectory with --directory; plain git apply in a subdirectory is a silent no-op"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-cli/tests/patch_tools.rs (3 tests)"
        status: pass
metrics:
  duration: 8 min
  started: 2026-10-28T18:42:06Z
  completed: 2026-10-28
  tasks: 2
  files: 4
---

# Phase 8 Plan 10: `report` and `adopt` Summary

`report` rebuilds the review report from the ledger and blobs alone through core's `build_report` and prints it with its hash and the exact `minos adopt` command. `adopt` rebuilds the report again, accepts only that current hash, appends `ReportAcknowledged`, and only then creates the canonical patch file with `create_new`. It never applies the patch. A golden test proves that both `git apply` and `patch -p1` apply the patch byte for byte, and pins the silent subdirectory no-op.

## Accomplishments

- **`report.rs`.** `build(store, run, exp)` resolves the run and the experiment by prefix and refuses the baseline. It then gathers the report's inputs:
  - the manifest recorded for the experiment's generation (`recorded_manifest_at`);
  - the run's one baseline judge version (`run_judge`), refusing the run with `NoJudge` otherwise;
  - the base surface, read back from the stored tarball (`base_surface`);
  - the recorded cumulative diff (`recorded_diff`);
  - a blob map of the target's diff blob (read under the standard diff cap) and every lineage rationale (read under `MAX_RATIONALE_BYTES`), keyed by `BlobId::of`.

  It calls `build_report` on these and returns a `BuiltReport` with the run, the experiment, the report, and the diff. `report(...)` writes every line of `Report::text()`, an empty line, `report hash: <64 hex>`, and `to adopt this experiment after review: minos adopt <12 hex> --run <12 hex> --reviewed <64 hex>`, each through `write_line`. It appends nothing.
- **`adopt.rs`.** `adopt(store, project, run, exp, options, out)` runs these steps in order, and each refusal stops it:
  1. It calls `report::build`.
  2. It compares hashes. A mismatch is `HashMismatch`, with no append and no write.
  3. It appends `ReportAcknowledged { exp, report }`. A `Duplicate` prints `(already recorded)`, and any store error stops it before any file exists.
  4. It writes the patch with `OpenOptions::new().write(true).create_new(true)`. `AlreadyExists` is `Exists`, which also covers a symlink, and the link is never followed. It writes the canonical bytes and calls `sync_all`.
  5. It prints the path, the patch through `SafeText`, `patch -p1 -i <file>`, `git apply --directory="$(git rev-parse --show-prefix)" <file>`, and the note that plain `git apply` from a subdirectory skips every file and that Minos never applies a patch.
- **`tests/report_adopt.rs`** has 9 tests: report output, nothing appended, the baseline refused, a wrong hash, adopting with the right hash (the file bytes equal `recorded_diff`), a second adopt, `--out` onto an existing file, `--out` onto a new path, a symlink at the patch path, and a stale hash after a new `Judged` trial. `assert_no_banned_words` runs over `Report::text()`, every captured output, and every provoked error's Display text.
- **`tests/patch_tools.rs`** has 3 tests over a `single_shot` run with seven newline cases, using `/usr/bin/git` and `/usr/bin/patch` as argv arrays with a cleared environment and `GIT_CEILING_DIRECTORIES`:
  - outside a repository;
  - at a repository root;
  - in `<repo>/sub/`, where plain `git apply` leaves the base bytes and `--directory=sub/` and `patch -p1` yield the proposed bytes.

  Every setting compares all eight files byte for byte and checks that no `.orig` or `.rej` file was left. The fixture also asserts that the patch has a section for each case and a `\ No newline at end of file` marker.

## Task Commits

| Task | Gate | Commit | Message |
|------|------|--------|---------|
| 1 | RED | 9a4c35c | test(08-10): add failing tests for report and adopt |
| 1 | GREEN | 279b9c9 | feat(08-10): print the hashed report and adopt only its current hash |
| 2 | golden | 9202d0d | test(08-10): pin adopt's patch under git apply and patch -p1 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Boxed error sources**
- **Issue:** `DriverError` and `PrepareError` are large, which trips clippy `result_large_err`.
- **Fix:** `ReportCmdError::Record(Box<DriverError>)` and `ReportCmdError::Prepare(Box<PrepareError>)`. This is the same approach 08-09 used.
- **Commit:** 279b9c9

**2. [Rule 2 - Missing critical path] A failed patch write leaves no partial file**
- **Issue:** The plan does not say what happens when `write_all` or `sync_all` fails after `create_new` has made the file. A half-written patch could later be applied as if it were whole.
- **Fix:** On that failure, `adopt` removes the file it created and returns `AdoptError::Write`.
- **Commit:** 279b9c9

**3. [Rule 3 - Acceptance grep] Module doc wording**
- **Issue:** The acceptance grep `git rev-parse --show-prefix` must print one line, and the module docs also quoted the command.
- **Fix:** The docs describe the `--directory=` form in words, so only the printed line holds the command.
- **Commit:** 279b9c9

Two additions beyond the behavior list: a symlink at the default patch path is refused and its target is never created (T-08-10-03), and the patch-tools fixture asserts the patch's sections and its no-newline marker.

**Total deviations:** 3 (1 missing path, 2 blocking). **Impact:** none on scope. Every public name and signature matches the plan.

## TDD Gate Compliance

Task 1 has a `test(08-10)` RED commit (9a4c35c), whose tests failed to compile on the missing `report` and `adopt` items, before its `feat(08-10)` GREEN commit (279b9c9). Task 2 is a golden test over Task 1's code, so it passed when written. It changes no production code, so it has no GREEN commit, and its single `test(08-10)` commit (9202d0d) is the whole task. No refactor commit was needed.

## Known Stubs

None. `report` and `adopt` are not yet wired into `main.rs`; plan 08-12 does that. No requirement ID was ticked in REQUIREMENTS.md, because plan 08-15 marks requirements.

## Threat Flags

None. T-08-10-01 to T-08-10-07 are mitigated as planned:
- **01, 02.** The hash comparison runs before any append or write, and the write happens only after the append. The stale-report and wrong-hash cases are tested.
- **03.** `create_new` covers both the default path and `--out`. An existing file and a symlink are both tested.
- **04.** `adopt` execs nothing and writes one file.
- **05.** Every printed line goes through `write_line`.
- **06.** The golden test pins the silent subdirectory no-op.
- **07.** Banned-word checks run over the report text, every output, and every error.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: pass
- `cargo test -p minos-cli --test report_adopt --test patch_tools`: 9 and 3 passed. All of `cargo test -p minos-cli` passes.
- `cargo doc -p minos-cli --no-deps`: no warnings
- C-02 `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing
- Acceptance greps all pass:
  - one line each for `pub fn build<S: Ledger + Blobs>`, `build_report(`, `create_new(true)`, `git rev-parse --show-prefix`, and `fn assert_no_banned_words`;
  - two lines for `ReportAcknowledged`;
  - two lines for `assert_no_banned_words(.*text()`;
  - lines for `"/usr/bin/patch"`, `"/usr/bin/git"`, and `--directory=`.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/report.rs, adopt.rs, tests/report_adopt.rs, tests/patch_tools.rs
- FOUND: 9a4c35c, 279b9c9, 9202d0d
