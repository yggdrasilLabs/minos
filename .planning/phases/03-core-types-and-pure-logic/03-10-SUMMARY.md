---
phase: 03-core-types-and-pure-logic
plan: 10
subsystem: minos-core
status: complete
tags: [core, ledger, fold, run-state, proptest, permutation-invariance]
requires:
  - "03-01: RunId::of, ExperimentId::baseline, digest_id! IDs (Copy, Ord)"
  - "03-03: Outcome, Reason, Stage"
  - "03-04: Usage::checked_add"
  - "03-05: Tier, JudgeVersion"
  - "03-07: Event (eleven variants), Event::key, Record::V1, Decision, DecisionKind, SpendId, SpendSource::exp, ReviewId, StopReason, BackendInfo, Origin"
provides:
  - "state.rs: fold, RunState (run, manifest, manifest_at, base, baseline, started_at, proposal, proposals_in, trials, decision, decisions, last_concluded_generation, incumbent_before, incumbent, notes, next_note_ordinal, stop_requested, spends, total_usage, reviews, is_acknowledged, lineage), Proposal, Judgment, Spend, Review, FoldError"
  - "tests/prop_fold.rs: accessor tests, one test per FoldError variant, generated-ledger properties (permutation invariance, duplicate replay, tamper detection)"
affects: [03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Two-phase fold: key every record into a BTreeMap<EventKey, &Event> (skip equal, refuse different), then build and validate from the key-ordered map, so input order cannot reach the result"
    - "Cross-references are validated against the complete state after insertion, never against what has been seen so far"
    - "Proptest ledger generator that keeps parents earlier, tiers and origins consistent, and Kept decisions equal to the tracked incumbent"
key-files:
  created: []
  modified:
    - crates/minos-core/src/state.rs
    - crates/minos-core/tests/prop_fold.rs
    - crates/minos-core/tests/prop_fold.proptest-regressions
decisions:
  - "A second RunStarted equal to the first is an exact duplicate and is skipped, so replaying a whole ledger is idempotent; a different one is DuplicateRunStarted"
  - "FoldError gained ManifestConflict { generation }: two ManifestChanged with the same from_generation have different keys (the key includes the manifest), so key dedup cannot catch them, and a generation-0 change must equal the RunStarted manifest"
  - "BadParent also covers a Proposed whose exp is the baseline, which has no parent"
  - "BadDecision also covers a Selected at generation 0 and a Kept whose incumbent is not incumbent_before(generation), matching how 03-11 conclude records Kept"
  - "next_note_ordinal returns Option<u32>: None once ordinal u32::MAX is recorded, so a note can never be silently keyed onto an existing one (the store treats a repeated key as a no-op)"
  - "Spends and reviews are exposed through small Spend and Review structs with accessors instead of tuples"
metrics:
  duration: "7 min"
  completed: 2026-10-14
  tasks: 2
  files: 3
---

# Phase 3 Plan 10: Ledger fold Summary

`fold` turns ledger records into a validated `RunState`. It rechecks `RunStarted`, keys every record, skips exact duplicates, and refuses a rewritten record. It checks every cross-reference against the complete state. Property tests show the result is the same under any order of records after `RunStarted` and under any duplicate replay (CORE-09).

## What was built

- **`fold` (state.rs).** It runs in two phases:
  - The first record must be `RunStarted`. Its `run` and `baseline` are recomputed with `RunId::of` and `ExperimentId::baseline`.
  - Phase 1 inserts every record into a `BTreeMap<EventKey, &Event>` under `event.key(&computed_run)`. An equal event is skipped. A different event under an existing key is `KeyConflict`, or `DuplicateRunStarted` when both are `RunStarted`.
  - Phase 2 iterates the key-ordered map to build the state. Answers are attached to their requests after all requests are in. Then `validate` checks proposals, judgments, decisions, spends, and acknowledgements against the full state.
  - The function doc says why this shape makes the result independent of record order.
- **`RunState`.** Private fields, and only `BTreeMap` and `BTreeSet`:
  - `manifests` by `from_generation`, plus the initial manifest.
  - `proposals`.
  - `judgments` by (experiment, judge), then by trial.
  - `decisions`, `spends`, and `total_usage`, a checked sum.
  - `notes` by (generation, ordinal), `stops`, `reviews`, and `acknowledged`.
  - It has every accessor the plan lists. Iterator accessors return entries in key order.
  - `incumbent_before(g)` is the latest `Improved` winner below `g`, else the baseline.
  - `lineage` walks parents from the experiment back to the baseline, bounded by the proposal count.
- **`Proposal`, `Judgment`, `Spend`, and `Review`.** Read-only values with accessors. Only the fold builds them.
- **`FoldError`.** `#[non_exhaustive]`, `Copy`, and IDs only. Variants: `Empty`, `FirstNotRunStarted`, `DuplicateRunStarted`, `RunIdMismatch { recorded, computed }`, `BaselineMismatch { recorded, computed }`, `KeyConflict { key }`, `UnknownExperiment { exp }`, `BadParent { exp, parent }`, `ProposedAtGenerationZero { exp }`, `OriginTierMismatch { exp }`, `BadDecision { generation }`, `AnswerWithoutRequest { request }`, `ManifestConflict { generation }`, and `UsageOverflow`. No variant has an `Outcome` payload. The type doc says ledger corruption is an infrastructure fault.
- **Tests (prop_fold.rs, 38 tests).**
  - Accessors over a valid two-generation ledger with every event kind. It covers manifests by generation, proposals and trials in key order, decisions, incumbents, notes in ordinal order, the earliest stop, total usage, reviews, acknowledgements, and lineage.
  - One test per corruption case, using `matches!`. The `DuplicateRunStarted` message is pinned exactly: "the ledger has more than one RunStarted record".
  - Properties over generated ledgers: 1 to 4 generations, 1 to 4 proposals each, 1 to 3 trials each, six outcome kinds, spends, notes, decisions, a manifest change, a review, a stop, and an acknowledgement:
    - `generated_ledgers_fold`
    - `fold_is_permutation_invariant`, using `prop_shuffle` over the records after `RunStarted`
    - `duplicate_replay_is_idempotent`, for a masked subset, for every record, and for a shuffled replay
    - `tampered_record_conflicts`

## Task Commits

1. **Task 1 RED:** `4d00d67` test(03-10): add failing fold accessor and corruption tests
2. **Task 1 GREEN:** `b7a57c2` feat(03-10): fold ledger records into RunState with validation
3. **Task 2:** `03c78d2` test(03-10): prove the fold order-insensitive, idempotent, and tamper-evident
4. **Refactor:** `e0c6540` refactor(03-10): key records under the recomputed run ID by name

## Verification

| Check | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok |
| `cargo test --workspace` | ok (prop_fold: 38 passed) |
| `cargo deny check` | advisories, bans, licenses, sources ok (the existing unmatched-wrapper warning for minos-sandbox is unchanged) |
| `cargo doc --workspace --no-deps` | ok, no warnings |
| C-01 lints table in every crate | no MISSING line |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `grep -nE 'Err\(Outcome' state.rs` | prints nothing |
| No `HashMap`/`HashSet` in state.rs | true; core clippy.toml bans them anyway |
| Acceptance greps (`pub fn fold`, `pub struct RunState`, `KeyConflict`, `DuplicateRunStarted`, `RunIdMismatch`, `fn incumbent_before`, `fn lineage`, `fn next_note_ordinal`, `RunId::of`, `.key(&`, the three property names, `prop_shuffle`) | each prints a line |
| `git status --porcelain crates/minos-core` after the commits | empty, so the regressions file is committed |

**Mutation check.** I temporarily changed phase 2 to iterate in input order and to check each `Judged` experiment as it arrived. `fold_is_permutation_invariant` and `duplicate_replay_is_idempotent` both failed. I restored state.rs from a scratch copy, and `git diff` was empty afterwards. The shrunk seeds from that run are in `prop_fold.proptest-regressions`, committed. They pass against the real fold and are replayed first on every run.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] `FoldError::ManifestConflict`**
- **Found during:** Task 1
- **Issue:** The `ManifestChanged` key includes the manifest. Two different manifests from the same generation therefore get two keys and pass key dedup. A generation-0 change could also contradict `RunStarted`. Either way the manifest schedule would be ambiguous.
- **Fix:** Added `ManifestConflict { generation }`, detected while the schedule is built. The plan said "exactly the variants in the behavior block"; this adds one variant and changes no other.
- **Files modified:** crates/minos-core/src/state.rs
- **Tests:** `two_manifests_for_one_generation_are_refused`, `a_generation_zero_manifest_must_match_run_started`
- **Committed in:** b7a57c2

**2. [Rule 2 - Correctness] Stricter `BadParent` and `BadDecision`**
- **Found during:** Task 1
- **Issue:** The behavior block did not cover three cases: a `Proposed` that claims the baseline ID, a `Selected` at generation 0 (generation 0 is calibration only), and a `Kept` naming an experiment that is not the incumbent.
- **Fix:** `BadParent` now covers the baseline claim. `BadDecision` now covers generation 0 and a `Kept` whose incumbent is not `incumbent_before(generation)`, which is the value 03-11's `conclude` records.
- **Tests:** `proposing_the_baseline_is_refused`, `a_decision_at_generation_zero_is_refused`, `a_kept_incumbent_that_is_not_the_incumbent_is_refused`
- **Committed in:** b7a57c2

**3. [Rule 2 - Correctness] `next_note_ordinal` returns `Option<u32>`**
- **Found during:** Task 1
- **Issue:** The plan wrote `-> u32`. Once ordinal `u32::MAX` is recorded there is no "one past". A saturating value would reuse the existing key, and the store treats a repeated key as a no-op (STORE-03), so the note would be dropped silently.
- **Fix:** Returns `None` when the ordinal space is used up. No later plan in this phase calls it, so no interface breaks.
- **Test:** `the_last_note_ordinal_has_no_successor`
- **Committed in:** b7a57c2

**4. [Rule 3 - Blocking] proptest `prop_assert!` with braces**
- **Found during:** Task 2
- **Issue:** `prop_assert!(matches!(.., KeyConflict { .. }))` failed to compile, because proptest turns the condition into a format string.
- **Fix:** Bind the bool first, then use `prop_assert!(flag, "message")`.
- **Committed in:** 03c78d2

**5. [Key-link wording] `event.key(&computed_run)`**
- The plan's key-link grep `\.key\(&` did not match `event.key(run)`, where `run` was already a reference. The fold now names the recomputed ID and borrows it at the call site. Behavior is unchanged.
- **Committed in:** e0c6540

**Total deviations:** three Rule 2 changes, which only make the fold refuse more corrupt ledgers, plus one Rule 3 fix and one naming change. **Impact:** every interface and must-have truth in the plan holds; the new behavior is listed above.

## TDD Gate Compliance

- Task 1: RED `4d00d67` (prop_fold.rs fails to compile, because `fold`, `RunState`, and `FoldError` do not exist yet), then GREEN `b7a57c2`.
- Task 2's properties test the fold Task 1 had already committed, so they passed as soon as they compiled. A failing-first step was not possible. Instead, the mutation check above showed that two of the properties fail against an input-order-dependent fold.

## Issues Encountered

None.

## Known Stubs

None. `policy.rs`, `route.rs`, `report.rs`, and the other skeletons belong to 03-11 to 03-13.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-10-01 | Every record keyed; a different event under an existing key is `KeyConflict` | `fold` phase 1; `the_same_key_with_different_content_conflicts`, `tampered_record_conflicts` |
| T-03-10-02 | Two-phase fold over the key-ordered map | `fold_is_permutation_invariant`, `duplicate_replay_is_idempotent`, and the mutation check |
| T-03-10-03 | `RunId` and baseline recomputed from `RunStarted` | `a_wrong_run_id_is_refused`, `a_wrong_baseline_is_refused` |
| T-03-10-04 | Cross-references checked after insertion, with typed variants | `validate`; the `UnknownExperiment`, `BadParent`, `BadDecision`, `AnswerWithoutRequest`, `ManifestConflict` tests |
| T-03-10-05 | Checked usage sum | `usage_overflow_is_refused` |
| T-03-10-06 | `FoldError` is an infrastructure fault with no `Outcome` payload | `FoldError` doc; the `Err(Outcome` grep prints nothing |

No new threat surface: the module does no I/O.

## Next Phase Readiness

- 03-11 can read `proposals_in`, `trials`, `incumbent_before`, `decision`, `decisions`, `last_concluded_generation`, `baseline`, `spends` (`Spend::{generation, source, usage}`), and `total_usage`. Its `Kept { incumbent: state.incumbent_before(g) }` is exactly what the fold accepts.
- 03-12 can read `proposal`, `lineage` (baseline first), `trials`, `baseline`, `base`, and `decisions`.
- 03-13 can re-export `RunState` and `fold` from the crate root.
- CORE-09 is complete: 03-09 covered the golden `Record::V1` half, and this plan covers the fold half.

## Self-Check: PASSED

- crates/minos-core/src/state.rs, crates/minos-core/tests/prop_fold.rs, and crates/minos-core/tests/prop_fold.proptest-regressions exist.
- Commits 4d00d67, b7a57c2, 03c78d2, and e0c6540 are in `git log`.
