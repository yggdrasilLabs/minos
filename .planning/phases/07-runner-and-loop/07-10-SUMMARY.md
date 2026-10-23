---
phase: 07-runner-and-loop
plan: 10
subsystem: minos-loop (run inputs, recorder, batch blob and admission, history readers)
status: complete
tags: [loop, resume, batch, admission, history, LOOP-04, LOOP-05, D-08, D-09, D-10, D-12, D-13, D-18, D-19]
requires:
  - phase: 07-01
    provides: "Proposed.batch, Judged.logs, RunStarted.root_image; RunState::{has_spend, next_model_call, judged, root_image}, Judgment::logs, Proposal::batch"
  - phase: 07-02
    provides: "Prices::missing_models, JudgeSpec::trials, MAX_STAGE_LOG_BYTES"
  - phase: 07-05
    provides: "StoreError::Conflict, StoreError::is_retryable"
  - phase: 07-06
    provides: "BaseTree, CumulativeDiff::recorded, Experiment::candidate(CumulativeDiff), Proposal::into_parts"
  - phase: 07-07
    provides: "DriverError variants (Conflict, Store, ManifestChanged, BaseMismatch, RootImageChanged, JudgeChanged, PricesMissing, Batch, DiffRecord)"
provides:
  - "minos_loop::inputs: RunInputs { manifest, manifest_bytes, base, program, prices, root_image, judge } with manifest_id(); Resume { Unchanged, ManifestChanged { from_generation } }; check_start; check_resume; same_frame"
  - "minos_loop::record: append (Conflict -> DriverError::Conflict { run, seq, kind }), spend_once (appends only when the SpendId is absent), event_kind"
  - "minos_loop::admit: BATCH_VERSION, MAX_BATCH_BYTES, Batch (new, to_json, parse_json, accessors), BatchItem, RawBatch, RawBatchItem, BatchError, AdmitStep, Admission, admit, missing_proposed, load_batch"
  - "minos_loop::history: recorded_diff, parent_tree, excerpt_stage, history, notes"
  - "minos_core::state: RunState::judge_versions(exp) and RunState::notes_by_ordinal(generation)"
  - "Golden crates/minos-loop/tests/snapshots/blocks__batch_v1.snap with a SHA-256 pin"
affects: [07-13, 07-15, 07-16]
tech-stack:
  added: []
  patterns:
    - "Parameter bundles with pub fields (RunInputs, AdmitStep) for driver inputs, as ProposeInput does; validated types stay private-field"
    - "A raw wire struct with deny_unknown_fields and a v field behind try_from/into, error variants with no input text"
    - "Every ledger append through one recorder that names the event kind on a conflict"
key-files:
  created:
    - crates/minos-loop/src/inputs.rs
    - crates/minos-loop/src/record.rs
    - crates/minos-loop/src/admit.rs
    - crates/minos-loop/src/history.rs
    - crates/minos-loop/tests/blocks.rs
    - crates/minos-loop/tests/snapshots/blocks__batch_v1.snap
  modified:
    - crates/minos-loop/src/lib.rs
    - crates/minos-core/src/state.rs
    - crates/minos-core/tests/prop_fold.rs
decisions:
  - "admit takes an AdmitStep parameter bundle (generation, parent, tier, origin, dropped) instead of eight arguments, because clippy's too_many_arguments fires above seven under -D warnings"
  - "The experiment frame compared by same_frame is the writable and readable sets, the program path, the source excludes, the commands, the environment, and the whole JudgeSpec (so a min_improvement or [judge] trials edit is a frame change); budget, policy, proposers, and params are outside it (D-19)"
  - "check_resume names the recorded judge version as the lowest version the baseline was judged under, read through the new RunState::judge_versions accessor"
  - "A batch also refuses a tier that disagrees with its origin (BatchError::OriginTier), since a Proposed with that pair would fail the fold"
  - "BatchError::Json carries no serde_json error, so no input text from a tampered blob reaches a message"
  - "load_batch treats a generation whose Proposed records name different batches, or only some name one, as DriverError::Batch; an over-cap batch blob is Batch, other store errors are Store"
  - "recorded_diff treats an over-cap diff blob as DiffRecord, other store errors as Store"
  - "notes renders each note under its recorded ordinal through the new RunState::notes_by_ordinal accessor rather than its position"
metrics:
  duration: "about 20 minutes (2026-10-23T01:01:30Z to 01:21:30Z)"
  completed: 2026-10-23
  tasks: 3
  files: 9
---

# Phase 7 Plan 10: The loop's building blocks Summary

Run inputs with start and resume checks (prices, base, root image, judge version, and D-19's experiment frame), a recorder that turns `StoreError::Conflict` into a named `DriverError::Conflict` and never re-appends a recorded spend, a versioned and pinned batch blob with admission that dedupes against the run's history, and pure ledger readers for the parent tree, prompt history, and steer notes, all tested on `MemoryStore`.

## What was built

- **Task 1, inputs and recorder** (`bdbdfa6` test, `56e48b1` feat). `RunInputs` is the bundle the CLI builds. `check_start` refuses unpriced proposer models (D-13). `check_resume` refuses, each naming both values, a changed base, a recorded root image that is absent or different (D-10), and a baseline judged only under another judge version. A changed manifest is `Resume::ManifestChanged { from_generation: last_concluded + 1 }` when the recorded manifest shares the frame, and `DriverError::ManifestChanged` otherwise or when no recorded manifest is supplied (D-19, superseding 07-RESEARCH A6). `record::append` classifies conflicts with the event kind; `spend_once` attempts no append when the `SpendId` is recorded (Pitfall 1); `event_kind` names all eleven variants.
- **Task 2, batch blob and admission** (`e576c0f` test, `01db80b` feat). `Batch` is `{"v":1,"generation","parent","tier","origin","dropped","items":[{"exp","diff","rationale"}]}` through `RawBatch`/`RawBatchItem` with `deny_unknown_fields`. `admit` names each proposal through `Experiment::candidate(cumulative.clone()).id()`, drops and counts repeats of recorded or same-step experiments, stores nothing when nothing survives, and otherwise stores every diff and rationale and then the batch. `missing_proposed` and `load_batch` complete and validate a partial batch after a crash (D-18).
- **Task 3, history readers** (`a318d27` test, `9db318c` feat). `recorded_diff` re-reads a diff blob through `CumulativeDiff::recorded`; `parent_tree` is the base for the baseline, else the base with the incumbent's recorded diff applied. `history` builds one `HistoryEntry` per earlier proposal with excerpts read under `MAX_STAGE_LOG_BYTES` only through `LogExcerpt::from_bytes`. `notes` reads each note under `MAX_STEER_NOTE_BYTES` and builds `SteerNote`s in ordinal order.
- **Doc fix** (`f35bfac`): an unresolved intra-doc link in `admit.rs` failed `cargo doc`.

The batch golden was accepted once with `INSTA_UPDATE=always cargo test -p minos-loop --test blocks`. The snapshot body is the single line above with fixed IDs, and it contains `"v":1` and `"items":[`. The pin `a8b2fbde29490dddfece7b1cbd3245060a2a18ada08280d2355b7df2a8a25108` was computed with `awk 'n >= 2 { print } /^---$/ { n++ }' blocks__batch_v1.snap | tr -d '\n' | shasum -a 256`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `RunState` had no way to list the judge versions an experiment was judged under**
- **Found during:** Task 1
- **Issue:** `check_resume` must name the recorded judge version in `JudgeChanged`, but `RunState::trials` and `judged` need the version as input.
- **Fix:** Added `RunState::judge_versions(&ExperimentId) -> BTreeSet<JudgeVersion>` in core with a fold test. It is additive, and core stays pure.
- **Files modified:** crates/minos-core/src/state.rs, crates/minos-core/tests/prop_fold.rs
- **Commits:** bdbdfa6, 56e48b1

**2. [Rule 3 - Blocking] `admit`'s planned eight-argument signature fails `-D warnings`**
- **Found during:** Task 2
- **Issue:** clippy's `too_many_arguments` fires above seven arguments.
- **Fix:** `admit(store, state, step: AdmitStep<'_>, items)`, where `AdmitStep` bundles generation, parent, tier, origin, and dropped as documented pub fields, the same pattern as `ProposeInput`.
- **Files modified:** crates/minos-loop/src/admit.rs
- **Commit:** 01db80b

**3. [Rule 2 - Correctness] A batch whose tier disagrees with its origin was representable**
- **Found during:** Task 2
- **Issue:** Such a batch would produce `Proposed` events that the fold refuses (`OriginTierMismatch`).
- **Fix:** Added `BatchError::OriginTier`, checked in `Batch::new` and therefore also on parse. A test covers it.
- **Commit:** 01db80b

**4. [Rule 1 - Bug] Notes rendered by position would misnumber a ledger with gaps in its ordinals**
- **Found during:** Task 3
- **Issue:** `RunState::notes` returns blobs without ordinals, but `SteerNote::new` takes the recorded ordinal.
- **Fix:** Added `RunState::notes_by_ordinal(generation) -> Vec<(u32, BlobId)>`, extended the existing fold tests, and made `notes` render each note under its recorded ordinal.
- **Files modified:** crates/minos-core/src/state.rs, crates/minos-core/tests/prop_fold.rs
- **Commits:** a318d27, 9db318c

**5. [Rule 3 - Blocking] Unresolved intra-doc link**
- **Found during:** gate (`cargo doc --workspace --no-deps`)
- **Fix:** Linked `CumulativeDiff` by its full path in `admit.rs`.
- **Commit:** f35bfac

## TDD Gate Compliance

Each task has a `test(07-10)` commit followed by a `feat(07-10)` commit. Each RED commit is a compile failure, because the modules and accessors the tests import did not exist at that commit. The implementation was drafted before the tests were run, so no RED run was observed separately from the GREEN run.

## Verification

All gate commands exited 0 after the doc fix:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 84 result lines, all ok, including `blocks` with 42 tests and `prop_fold` with 53
- `cargo deny check`
- `cargo doc --workspace --no-deps`
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed only `ok` lines, and the C-02 grep printed nothing.

## Known Stubs

None.

## Notes for later plans

- 07-13 calls `admit(store, &state, AdmitStep { generation, parent, tier, origin: p.origin(), dropped: p.dropped().total() }, p.into_items())`.
- CLAUDE.md's workspace line for `minos-loop` does not yet list these modules; the phase-close documentation plan can add them.

## Self-Check: PASSED

- FOUND: crates/minos-loop/src/inputs.rs, record.rs, admit.rs, history.rs, tests/blocks.rs, tests/snapshots/blocks__batch_v1.snap
- FOUND commits: bdbdfa6, 56e48b1, e576c0f, 01db80b, a318d27, 9db318c, f35bfac
