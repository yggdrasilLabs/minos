---
phase: 03-core-types-and-pure-logic
plan: 12
subsystem: minos-core
status: complete
tags: [core, report, review-gate, flags, safetext, insta, golden, proptest, determinism]
requires:
  - "03-01: ReportHash::of, IdHasher, limits (ANOMALY_MULTIPLE, NOISE_WARN_MULTIPLE, MIN_NOISE_TRIALS, ENCODED_RUN_MIN, LITERAL_MIN_BYTES, RATIONALE_EXCERPT_BYTES), RelPath::file_name, .config/insta.yaml update: no"
  - "03-02: Diff::parse, FileDiff, Hunk (old_start, old_len, new_start), HunkLine"
  - "03-04: SafeText (new, inline), text::is_invisible_format"
  - "03-05: Manifest (judge, writable, surface), Objective (metric, goal, min_improvement, held_out), Goal::oriented, JudgeSpec (objectives, gates, held_out_metrics), JudgeVersion"
  - "03-06: diff::apply::Tree"
  - "03-07: Origin, BackendInfo, BackendKind, Isolation, ConfinementLayer, EXPECTED_KRUN_LAYERS"
  - "03-10: fold, RunState (proposal, lineage, trials, baseline, base, run), Proposal"
  - "03-11: reduce_trials, ReducedScore, noise_floor, NoiseFloor, improves"
provides:
  - "report.rs: DEPENDENCY_FILE_NAMES, Flag (AnomalousJump, HeldOutDivergence, LiteralMatch, NewDependency, EncodedBlob, HiddenCharacters, WeakIsolation), MetricRole (Objective, Gate, HeldOut), SummaryRow (accessors), ReportInput (new), ReportError (NotInLedger, Baseline, MissingBlob, NotText, InvalidDiff, Unscored, Format), flags, summary, Report (text, hash, flags, summary), build_report, private format_metric"
  - "tests/report.rs: fires-and-quiet fixtures for every flag, summary row tests, rendering and refusal tests, report_is_order_insensitive, golden_report_bytes_pinned"
  - "tests/snapshots/report__clean_report.snap and report__weak_isolation_report.snap"
affects: [03-13, phase-08-cli-report-adopt]
tech-stack:
  added: []
  patterns:
    - "One private analyze() computes lineage, re-parsed diff, rows, and flags once; flags(), summary(), and build_report() all reuse it"
    - "Flags collected into a BTreeSet<Flag> (derived Ord), so their order never depends on input order"
    - "Rendering through a put(out, format_args!) helper that maps fmt::Error to ReportError::Format, so no write result is discarded"
    - "Golden text snapshot plus one SHA-256 pin over both bodies, computed with awk + shasum outside the code under test"
key-files:
  created:
    - crates/minos-core/tests/report.rs
    - crates/minos-core/tests/snapshots/report__clean_report.snap
    - crates/minos-core/tests/snapshots/report__weak_isolation_report.snap
  modified:
    - crates/minos-core/src/report.rs
decisions:
  - "build_report re-parses the target's diff blob against the manifest's writable set and refuses unless ExperimentId::of(base, diff) equals the target, so the report can only show the diff the experiment ID covers"
  - "Every fetched blob is checked against BlobId::of its bytes; a mismatch is MissingBlob, so a tampered blob store cannot change what the report shows"
  - "ReportError::InvalidDiff carries the experiment ID ({ exp }), so the CLI can name it"
  - "clears_noise compares the delta's magnitude with NOISE_WARN_MULTIPLE times the baseline spread (a change outside the noise, in either direction); clears_threshold is the direction-aware improves() test"
  - "A spread of zero counts as no measured noise floor: the anomaly flag then falls back to the largest earlier positive step in the lineage, and stays quiet when there is none"
  - "Reduced hardening lists every EXPECTED_KRUN_LAYERS member missing from at least one lineage trial; only weak isolation raises a flag"
  - "Rationale excerpts are cut to RATIONALE_EXCERPT_BYTES at a char boundary before sanitizing and marked [truncated]; the diff section is unindented so it reads as a patch"
metrics:
  duration: 8min
  completed: 2026-10-14
  tasks: 2
  files: 4
---

# Phase 3 Plan 12: build_report and the review flags Summary

`build_report` renders a deterministic, SafeText-sanitized review artifact. It has the D-17 summary rows, seven flags (the five v1 flags plus hidden characters and weak isolation), provenance with the weakest lineage isolation and missing confinement layers, the lineage, and the target's re-verified cumulative diff. `ReportHash::of` covers exactly the rendered bytes, and two golden snapshots carry a SHA-256 byte pin.

## What was built

- **Inputs (`ReportInput`).** The folded `RunState`, the manifest, the `JudgeVersion`, the target `ExperimentId`, a `BTreeMap<BlobId, Vec<u8>>` of blobs, and the base surface `Tree`. Judge data is not a field, so it cannot be an input.
- **Checks before rendering.**
  - The baseline is `Baseline`, and an unknown experiment is `NotInLedger`.
  - Every blob must hash to its ID, or the report fails with `MissingBlob`. Diff and rationale blobs must be UTF-8, or it fails with `NotText`.
  - The diff must parse and hash to the target, or it fails with `InvalidDiff`.
  - The target and baseline must have complete scores, or it fails with `Unscored`.
- **Summary rows (D-17).** There is one row for each objective, gate, and held-out metric, in that order and sorted within each group. Each row has:
  - the baseline median, min, max, and trial count, from `noise_floor`;
  - the winner's median and trial count, from `reduce_trials`;
  - the absolute delta, and the percent delta, which is absent at a zero baseline;
  - `clears_threshold`, from `improves`, on objective rows only;
  - `clears_noise`, which is absent with fewer than `MIN_NOISE_TRIALS` trials or a zero spread.
- **Flags.** Each is a pure function, and they come back sorted:
  - `AnomalousJump`: the step over the parent exceeds `ANOMALY_MULTIPLE` times the spread, or that multiple of the largest earlier lineage step when there is no spread.
  - `HeldOutDivergence`: read from the summary rows.
  - `LiteralMatch`: quoted strings and digit runs of at least `LITERAL_MIN_BYTES` that also appear in a readable or writable surface file, outside the hunk's own old-side lines. There is one flag per (path, literal).
  - `NewDependency`: the diff touches a file named in `DEPENDENCY_FILE_NAMES`, a list of 31 names.
  - `EncodedBlob`: an added run of at least `ENCODED_RUN_MIN` characters from `[A-Za-z0-9+/=_-]`.
  - `HiddenCharacters`: an added line contains a character that `is_invisible_format` matches.
  - `WeakIsolation`: the minimum `Isolation` over every lineage trial is `WeakProcess`.
- **Rendering.**
  - Sections come in a fixed order: header, summary, flags (or `none`), provenance, lineage, diff. Lines end in LF only.
  - Flags, model IDs, and rationales go through `SafeText::inline`. Diff lines go through `SafeText::new`.
  - Provenance always shows the isolation level, the missing layers (`reduced hardening`), the judge, the backends, the root image and kernel digests, the models, and the prompt hashes.
  - Numbers go through `format_metric`: `{}` for values in [1e-4, 1e15), `{:e}` otherwise, and `0` for zero.

## Tasks

| Task | Name | Commits | Files |
| ---- | ---- | ------- | ----- |
| 1 | Flags and summary rows | c8064e9 (test), ec0a1bd (feat) | src/report.rs, tests/report.rs |
| 2 | build_report rendering, hash, determinism, golden reports | 4f2fad0 (test), a734e08 (feat) | src/report.rs, tests/report.rs, two .snap files |

## Verification

- `cargo test -p minos-core --test report`: 45 passed. The `report.rs` unit tests (`format_metric`) also pass.
- Full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (0 failures), `cargo deny check` (exit 0), and `cargo doc --workspace --no-deps` (no warnings).
- C-01 printed no MISSING line. C-02 (`grep -rnE '#!?\[allow\(' crates --include='*.rs'`) printed nothing.
- Prohibitions: no `serde_json` in report.rs, no `judge_data`/`JudgeSecrets`/`secret` in report.rs, and no `.snap.new` files.
- Snapshots were written once with `INSTA_UPDATE=always`, reviewed, and pinned (`b63aba37...a4c`, from awk plus `shasum -a 256` over the two bodies). After that, the plain test gate passed and wrote nothing.

## TDD Gate Compliance

Both tasks have a RED `test(03-12)` commit (a compile failure on the missing API) followed by a GREEN `feat(03-12)` commit. No refactor commit was needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] The target's diff must hash to the target**
- **Found during:** Task 1
- **Issue:** The fold does not check that a `Proposed` diff blob hashes to its `ExperimentId`, so without a check the report could show a diff other than the one the hash covers. That would undermine T-03-12-02.
- **Fix:** `analyze` recomputes `ExperimentId::of(state.base(), &diff)` and refuses a mismatch with `InvalidDiff { exp }`. Fixtures build real diffs and IDs.
- **Files modified:** crates/minos-core/src/report.rs
- **Commit:** ec0a1bd

**2. [Rule 2 - Missing critical functionality] Blobs are checked against their IDs**
- **Found during:** Task 1
- **Issue:** The report is the review gate, so a blob map with a wrong entry must not change what it shows.
- **Fix:** Every lookup checks `BlobId::of(bytes) == id`, and a mismatch is `MissingBlob`. The test `a_blob_that_does_not_match_its_id_is_refused` covers it.
- **Files modified:** crates/minos-core/src/report.rs, crates/minos-core/tests/report.rs
- **Commit:** ec0a1bd, a734e08

**3. [Interface detail] `ReportError::InvalidDiff` carries `{ exp }`**
- The plan listed `InvalidDiff` without fields. It now names the experiment, like the other variants that carry IDs.

## Known Stubs

None. The literal `n/a` and `none` strings in the rendered text are intentional display values for absent data.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/report.rs, crates/minos-core/tests/report.rs, crates/minos-core/tests/snapshots/report__clean_report.snap, crates/minos-core/tests/snapshots/report__weak_isolation_report.snap
- FOUND commits: c8064e9, ec0a1bd, 4f2fad0, a734e08
