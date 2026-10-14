---
phase: 03-core-types-and-pure-logic
plan: 09
subsystem: minos-core
status: complete
tags: [core, job, wire-format, golden-files, insta, byte-pin, serde, proptest]
requires:
  - "03-03: Outcome, Reason, Stage, Scorecard (from_entries)"
  - "03-04: Usage (tokens, compute)"
  - "03-05: Manifest::parse_toml, Budget (build_ms, run_ms, judge_ms), JudgeVersion::of, Tier, Arch"
  - "03-06: Experiment (baseline, candidate, id, base, diff), Diff::parse, Diff::as_bytes"
  - "03-07: frozen Event, Record::v1, Line, BackendInfo, Decision, SpendSource, ReviewId"
provides:
  - "job.rs: JOB_VERSION, JobBudget (new, from_budget, build_ms, run_ms, judge_ms), RawJobBudget, JobRequest (baseline, candidate, parse_json, verify_diff, exp, base, diff, is_baseline, judge, trial, budget), RawJobRequest, JobResult (new, parse_json, exp, judge, trial, outcome, usage, backend, logs), RawJobResult, JobError (Json, Version, Trial, Budget, DiffMismatch, BaselineAsCandidate)"
  - "tests/golden_wire.rs: 24 named insta snapshots, all_golden_bytes_pinned, round-trip and rejection tests, spent_carries_no_dollar_field, metric_bits_survive_record_round_trip"
  - "tests/snapshots/golden_wire__*.snap: the first golden wire files in the repository (24 files)"
affects: [03-10, 03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "One insta snapshot per golden line plus one SHA-256 literal over all the bodies joined with newlines, because insta trims trailing whitespace and normalizes line endings"
    - "The byte pin is computed outside the code under test: awk extracts each .snap body and shasum hashes the concatenation; the recipe sits in a comment above the literal"
    - "Rejection inputs are golden strings edited with str::replacen through an edit helper that fails if the target text is missing, so no rejection test is vacuous"
    - "Private budget fields drop the _ms suffix (clippy struct_field_names) and the raw struct renames them to build_ms, run_ms, judge_ms on the wire, following 03-04's TokenRates"
key-files:
  created:
    - crates/minos-core/tests/golden_wire.rs
    - crates/minos-core/tests/snapshots/ (24 golden_wire__*.snap files)
  modified:
    - crates/minos-core/src/job.rs
decisions:
  - "A trial index must be below MAX_TRIALS (0 to 31), because MAX_TRIALS is the most trials of one experiment and trials count from 0"
  - "JobRequest::candidate also refuses a diff blob that is not BlobId::of the experiment's canonical diff (DiffMismatch), so a request can never name another diff's blob"
  - "JobResult gets parse_json beside JobRequest's, and both go through the same TryFrom as serde"
  - "Job JSON uses an integer version (\"v\":1), like the scorecard and the guest job; ledger records keep the string tag (\"v\":\"1\") that the internally tagged Record needs"
  - "JobResult.logs is a JSON object keyed by stage name. Both sides of a job are trusted code, so serde_json keeping the last of two equal keys is not an attack surface here"
metrics:
  duration: "5 min"
  completed: 2026-10-14
  tasks: 2
  files: 26
---

# Phase 3 Plan 09: Job contract and golden wire files Summary

This plan adds the versioned `JobRequest`/`JobResult` contract between the loop and the runner. It also commits the repository's first golden wire files: 24 reviewed insta snapshots, one for every `Record::V1` event shape, every `Outcome` variant, `Line`, `Scorecard`, and the job contract, plus a SHA-256 pin over their exact bytes.

## What was built

- **job.rs (CORE-13).**
  - `JobRequest { exp, base, diff: Option<BlobId>, judge, trial, budget }` and `JobResult { exp, judge, trial, outcome, usage, backend, logs: BTreeMap<Stage, BlobId> }` have private fields. Both serialize through `Raw*` structs that refuse unknown fields and are read back through the same `TryFrom` checks as the constructors.
  - The checks are: version 1, a trial below `MAX_TRIALS`, each stage budget in `MIN_STAGE_MS..=MAX_STAGE_MS`, and `diff` is `None` exactly when `exp == ExperimentId::baseline(&base)`. A violation of the last check is `DiffMismatch`.
  - `JobRequest::candidate` refuses the baseline (`BaselineAsCandidate`) and any blob that does not address the canonical diff.
  - `verify_diff` recomputes `ExperimentId::of(base, diff)`. The module docs require the runner to call it before it builds the candidate VM.
  - Neither type has a `u128` or a dollar field.
- **golden_wire.rs (CORE-07, CORE-09, COST-01).**
  - Fixtures are built only from fixed bytes and public constructors. The manifest is the CLAUDE.md example, and the judge is `JudgeVersion::of(TreeId::of(b"judge"), ..)`.
  - The `BackendInfo` fixture has Hardware isolation with Seatbelt and Rlimits and no DedicatedUser, so the report's reduced-hardening path has data.
  - The metric values 1.0, 12.25, and 0.5 are exactly representable.
  - Three snapshot values match earlier golden vectors: the `RunId` (6a65b71b...), the baseline (6fd8be38...), and the `JudgeVersion` (3a2f24af...bc12).
  - Every snapshot was reviewed after the deliberate `INSTA_UPDATE=always` run. Each is one JSON line, with keys in declaration order, exact metric values, and `"v":"1"` on all 19 record files.
  - The plain `cargo test` then passed, and no `.snap.new` files were written.

## Tests

- **Job contract (19 tests).**
  - Constructors, the trial and budget caps, and `verify_diff`.
  - Request and result round trips through both serde and `parse_json`.
  - Rejection of: a candidate without a diff, a baseline with a diff, trial 32, out-of-range budgets (each field named), `"v":2`, and an unknown field at the top level or inside the budget.
  - Result rejection of: `"v":2`, trial 32, a `cost_usd` field, and an unknown log stage.
- **Golden files (12 tests).**
  - `golden_snapshots` checks all 24 snapshots. `all_golden_bytes_pinned` checks `8020cfde...1039`. `every_golden_line_is_one_json_line` checks that each golden is a single line.
  - Every event variant and every `Outcome` variant appears in a golden line.
  - Every golden line parses back to an equal value, and `Line::verify` passes on the golden line.
  - Records must carry the string `v` tag.
  - Records are rejected for: `"v":"2"` on every record, unknown variants (an event and an outcome), an extra field inside every variant (plus `cost_usd` in a usage and `network` in a backend), and a missing required field (`started_at`, `trial`, `vcpu_ms`, and a line's `seq`).
  - `spent_carries_no_dollar_field`.
  - The proptest `metric_bits_survive_record_round_trip` draws metric values from all finite bit patterns.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check` (exit 0), and `cargo doc --workspace --no-deps` all pass.
- C-01 prints no MISSING line. C-02 (`grep -rnE '#!?\[allow\(' crates --include='*.rs'`) prints nothing.
- `ls crates/minos-core/tests/snapshots/golden_wire__*.snap | wc -l` is 24, and the count of `.snap.new` files is 0. 19 record snapshots contain `"v":"1"`.
- The freeze-ancestor check exits 0: freeze commit 7090f02 is an ancestor of snapshot commit f40af94.
- `grep -n 'u128' crates/minos-core/src/job.rs` prints nothing. `grep -n 'update: no' .config/insta.yaml` prints a line.

## Commits

| Task | Gate | Commit | Message |
|---|---|---|---|
| 1 | RED | bb105c7 | test(03-09): add failing tests for the JobRequest and JobResult contract |
| 1 | GREEN | 71ffd22 | feat(03-09): implement the JobRequest and JobResult contract |
| 2 | RED | bfdf73e | test(03-09): add failing golden wire snapshots, byte pin, and rejection tests |
| 2 | GREEN | f40af94 | test(03-09): commit reviewed golden wire snapshots and the byte pin |

## TDD Gate Compliance

Both tasks have a RED commit that failed: Task 1 failed to compile, and in Task 2 the snapshots were missing and the pin was a placeholder. A GREEN commit follows each RED commit. Task 2's GREEN commit uses the `test` type, because it adds only snapshot files and the pin literal to test code; no library code changed in Task 2.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical check] `JobRequest::candidate` checks that the diff blob addresses the diff**
- **Found during:** Task 1
- **Issue:** As specified, `candidate` would accept any `diff_blob`, so a request could name a blob that is not the experiment's diff. That is the T-03-09-01 tampering case.
- **Fix:** `candidate` returns `DiffMismatch` unless `BlobId::of(diff.as_bytes()) == diff_blob`. Ledger `Proposed.diff` is documented as the canonical diff's blob, so the two agree.
- **Files modified:** crates/minos-core/src/job.rs
- **Commit:** 71ffd22

**2. [Rule 3 - Blocking] Budget field names and the non-const `JobBudget::new`**
- **Found during:** Task 1
- **Issue:** Clippy's `struct_field_names` fires on three fields that all end in `_ms`. Also, `JobBudget::new` cannot be a `const fn`, because `JobError` holds a `serde_json::Error` that has a destructor.
- **Fix:** The private fields are named `build`, `run`, and `judge`, and serde renames them to `build_ms`, `run_ms`, and `judge_ms` on the raw struct (the 03-04 `TokenRates` precedent). `new` is a plain `fn`. The wire form and the accessors match the plan.
- **Files modified:** crates/minos-core/src/job.rs
- **Commit:** 71ffd22

### Additions within the plan

- **`JobBudget::new`:** a fallible public constructor that the `try_from` path shares.
- **`JobResult::parse_json`:** mirrors `JobRequest::parse_json`.
- **`JobRequest::is_baseline`:** an accessor.

### Interpretation

- "trial at most MAX_TRIALS" is enforced as `trial < MAX_TRIALS`. `MAX_TRIALS` counts trials, and trial indices start at 0. The behavior "trial above MAX_TRIALS fails" holds, and trial 32 also fails.

## Known Stubs

None.

## Threat Flags

None. The plan's threat register covers every new surface: T-03-09-01 is mitigated by `verify_diff`, the parse-time `DiffMismatch`, and the blob check in `candidate`; T-03-09-02 to T-03-09-05 are covered by the snapshots, the byte pin, the rejection tests, `spent_carries_no_dollar_field`, and the metric-bits property.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/job.rs, crates/minos-core/tests/golden_wire.rs, 24 files under crates/minos-core/tests/snapshots/
- FOUND commits: bb105c7, 71ffd22, bfdf73e, f40af94
