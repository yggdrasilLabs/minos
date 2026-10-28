---
phase: 08-cli-sample-project-and-docs
plan: 04
subsystem: sample-project
status: complete
tags: [sample, logparse, judge, held-out, rust, std-only]
requires: []
provides:
  - examples/logparse candidate crate (std only, own lints, committed Cargo.lock)
  - examples/logparse/judge crate logparse-judge (std only, own lints, its own Cargo.lock)
  - metrics rows_match (gate), accuracy (objective), accuracy_held_out (held-out)
  - crates/minos-cli/tests/fixtures/logparse_improved_parse.rs (full-file improved parser)
  - root workspace exclude = ["examples"]
affects: [08-05 sample manifest and Dockerfile, the Phase 8 VM test, DOCS-01]
tech-stack:
  added: []
  patterns:
    - "Sample crates carry a literal copy of [workspace.lints] because the VM build has no parent workspace"
    - "The judge takes its three paths as argv, so the manifest's judge argv names no project path"
    - "Visible and held-out halves mirror each other's messiness, so a general parser scores equally on both"
key-files:
  created:
    - examples/logparse/Cargo.toml
    - examples/logparse/Cargo.lock
    - examples/logparse/src/main.rs
    - examples/logparse/src/record.rs
    - examples/logparse/src/parse.rs
    - examples/logparse/data/input.log
    - examples/logparse/data/examples.tsv
    - examples/logparse/program.md
    - examples/logparse/judge/Cargo.toml
    - examples/logparse/judge/Cargo.lock
    - examples/logparse/judge/src/main.rs
    - examples/logparse/judge/answers.tsv
    - examples/logparse/judge/.gitignore
    - crates/minos-cli/tests/fixtures/logparse_improved_parse.rs
  modified:
    - Cargo.toml
decisions:
  - "The judge matches candidate rows to key rows by their line-number field, not by position; a line number written twice is ambiguous and scores no fields"
  - "The judge reads at most 16 MiB + 1 of candidate output (the candidate out/ cap); more, a read error, or non-UTF-8 counts as no output and scores 0, exit 0"
  - "The baseline keeps a level only when it is already canonical (DEBUG, INFO, WARN, ERROR) and only parses single-spaced format A, so it scores 29/96 on each half"
  - "The answer key was written by hand from the input lines, independently of the improved fixture, and the fixture's output matched it byte for byte"
metrics:
  duration: 8m
  completed: 2026-10-28
  tasks: 2
  files: 15
---

# Phase 8 Plan 04: The logparse Sample Crates Summary

Std-only `logparse` candidate (one writable `src/parse.rs`, a deliberately imperfect baseline) and a std-only `logparse-judge` that scores the candidate's output file against a private 48-row answer key. It reports a `rows_match` gate, visible `accuracy`, and held-out `accuracy_held_out`. On the host the baseline scores 0.302 on both accuracies, and the improved fixture scores 1 on both.

## What was built

- **Workspace exclude.** The root `Cargo.toml` now has `exclude = ["examples"]`, with a comment explaining why (08-RESEARCH E8). `cargo metadata` lists no `logparse` member. The workspace gate still passes (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo deny check`).
- **Candidate crate `examples/logparse`.** It has no `[dependencies]` and no `[workspace]`. Its `[lints.rust]`/`[lints.clippy]` tables copy the workspace levels literally, and `[profile.release] overflow-checks = true`. `cargo tree` prints the package alone.
  - `src/record.rs`: a `Record` with private fields. `new`, `Default`, and `to_tsv` render missing fields as `-`.
  - `src/main.rs`: takes exactly two argv paths (else usage and exit 2). It writes `<N>\t<fields>\n` for each input line, and an I/O error gives a fixed message and exit 1. No shell.
  - `src/parse.rs`: the baseline `pub fn parse_line(line: &str) -> Record`. It parses format A only when the tokens are single-spaced and the level is canonical.
- **Data.** `data/input.log` has 48 lines: lines 1-24 are visible and 25-48 are held out. Each half has 8 lines of each format (A, B, C, interleaved A/B/C). The held-out half mirrors the visible half's messiness category by category: extra spaces, leading or trailing spaces, lowercase or synonym levels (`warning`, `err`, `Err`, `ERR`, `Warning`, `WARNING`, `Debug`), single-digit syslog days, and lines without latency. `data/examples.tsv` has a `#` header and the 24 visible rows. No line matches a secret-scan pattern.
- **`program.md`.** States the full output spec with one example per format. It notes that the held-out lines are scored and that hard-coding visible answers is flagged in review, and gives the std-only, no-unwrap/expect/indexing/allow rules and the fixed `parse_line` signature.
- **Improved fixture** `crates/minos-cli/tests/fixtures/logparse_improved_parse.rs`. A complete replacement `src/parse.rs` that implements all three formats using slice patterns, `strip_prefix`/`strip_suffix`, `split_once`, and `windows(3)`, with no indexing. It has 4 unit tests. Cargo does not compile it as a test target, because `tests/fixtures/` has no `main.rs`.
- **Judge crate `examples/logparse/judge`** (package `logparse-judge`).
  - Takes `<answers.tsv> <candidate-output.tsv> <scorecard.json>`.
  - The key parser requires rows numbered 1..N with no gap, a split of `visible` or `held_out`, exactly 4 fields, both splits present, and at most 100,000 rows.
  - `rows_match` is 1 only when the row count equals the key count, the Nth row starts with `N`, and every row has exactly 5 tab-separated fields.
  - Accuracies are matched fields divided by 4 × rows, per split, and are 0 for an empty split, so a value is always finite.
  - Writes the scorecard JSON by hand in the order rows_match, accuracy, accuracy_held_out, with `held_out` true only for the last.
  - Exits 1 with a fixed message for an unreadable or malformed key or an unwritable scorecard, and 2 for usage.
  - Its `Cargo.lock` differs from the candidate's (`cmp` exits 1). Its `.gitignore` is `/target/`.
- **`answers.tsv`.** A `#` header, then 48 rows: 24 `visible` and 24 `held_out`. Rows 1-24 agree with `data/examples.tsv` field for field (checked with `diff`).

## Host scoring proof

Built with `--release --frozen --target-dir target/examples`, run from the repository root:

Baseline (`logparse data/input.log fields.tsv`, then `logparse-judge answers.tsv fields.tsv scorecard.json`, exit 0):

```json
{"v":1,"metrics":[{"name":"rows_match","value":1,"held_out":false},{"name":"accuracy","value":0.3020833333333333,"held_out":false},{"name":"accuracy_held_out","value":0.3020833333333333,"held_out":true}]}
```

Improved fixture (a copy of `examples/logparse` with `src/parse.rs` replaced by the fixture). `cargo clippy --all-targets --target-dir target/examples-improved -- -D warnings` was clean, then rebuilt `--release --frozen` and rescored (exit 0):

```json
{"v":1,"metrics":[{"name":"rows_match","value":1,"held_out":false},{"name":"accuracy","value":1,"held_out":false},{"name":"accuracy_held_out","value":1,"held_out":true}]}
```

Absent output file: the scorecard has all three metrics at 0 and the judge exits 0. Missing key: exit 1 with `logparse-judge: cannot read the answer key`. Two arguments: exit 2 with the usage line.

The baseline scores 29/96 on each half because the halves mirror each other's messiness. That makes the held-out comparison fair: a general parser scores equally on both halves, and only a diff that hard-codes visible answers diverges.

## Verification

- Candidate gate: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` (8 passed), and `cargo build --release --frozen`, all with `--manifest-path examples/logparse/Cargo.toml --target-dir target/examples`. All pass.
- Judge gate: the same four commands on `examples/logparse/judge/Cargo.toml` (12 tests passed). All pass.
- Fixture in the scratch copy: fmt, clippy, and test (9 passed) all pass.
- `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'` prints nothing.
- No file in the candidate tree has the same SHA-256 as a judge file (0 shared digests).
- Acceptance greps: `exclude = ["examples"]` appears once in `Cargo.toml`; `"name":"logparse"` count is 0; `cargo tree` prints 1 line for each crate; `input.log` has 48 lines; `examples.tsv` has 24 non-comment rows; `answers.tsv` has 48 rows, 24 `held_out` and 24 tab-delimited `visible`; `parse_line` appears in both parse files.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 91d8bf7 | feat(08-04): add the logparse sample candidate crate and its data |
| 2 (RED) | ac42c2d | test(08-04): add failing tests for the logparse judge |
| 2 (GREEN) | 09bfa06 | feat(08-04): implement the logparse judge scoring |

## Deviations from Plan

None in behaviour. Process notes:

- **TDD gate, Task 1.** The baseline parser *is* the deliverable, and its behaviour tests describe the baseline, so there was no meaningful failing state to commit. The tests and code went in one `feat` commit. Task 2 followed RED (ac42c2d: stubbed scoring, 10 of 12 tests failing) and then GREEN (09bfa06).
- **Extra judge tests beyond the listed behaviours:** a row with an extra field fails the gate, one wrong field costs 1/8 of a 2-row split, a key with a gap or an unknown split is malformed, and a missing output file still writes the zero scorecard through `run`.

## Notes for later plans

- 08-05's manifest can use the judge argv shape `cargo run --release --frozen --quiet --target-dir /tmp/judge-target -- answers.tsv /scratch/candidate/out/fields.tsv /out/scorecard.json`. `answers.tsv` is a bare word, so it names no project path, and the candidate's run command should write `/out/fields.tsv`.
- The project `.gitignore` that 08-05 writes must not be byte-identical to `judge/.gitignore` (`/target/\n`).
- CLAUDE.md's Workspace section does not yet mention `examples/` or the workspace exclude. The docs plans in this phase should add both alongside the sample's description.

## Known Stubs

None. The baseline parser is imperfect on purpose (D-02, D-03), and its module doc says so.

## Threat Flags

None. The judge reads two files and writes one, which is the surface the plan's threat model covers (T-08-04-01 to T-08-04-03).

## Self-Check: PASSED

- All 15 files in `files_modified` exist on disk.
- Commits 91d8bf7, ac42c2d, and 09bfa06 are in `git log`.
