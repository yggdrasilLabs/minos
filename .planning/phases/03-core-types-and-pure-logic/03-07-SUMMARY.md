---
phase: 03-core-types-and-pure-logic
plan: 07
subsystem: minos-core
status: complete
tags: [core, ledger, event, wire-format, keys, serde, proptest, golden-vectors]
requires:
  - "03-01: digest_id!, IdHasher, RunId, ManifestId, TreeId, ExperimentId::baseline, BlobId, PromptHash, ReportHash, ImageDigest, ModelId"
  - "03-03: Outcome, Reason, Stage, Scorecard"
  - "03-04: Usage"
  - "03-05: Tier, Arch, JudgeVersion (and its golden vector)"
provides:
  - "ledger.rs: EventKey, SpendId (of), ReviewId (of), Origin, StopReason (Display, private tag), ReviewKind, ReviewAnswer, SpendSource (exp), DecisionKind, Decision (new, kind, proposed, dropped), BackendKind, Isolation, ConfinementLayer, EXPECTED_KRUN_LAYERS, BackendInfo (new and accessors), RawBackendInfo, BackendInfoError, Event (eleven variants, key), Record (V1, v1, event), Line (new, seq, key, record, verify), LineError::KeyMismatch"
  - "tests/prop_keys.rs: key distinctness against an independent key-field model, non-key invariance, wire shape and rejections, Line verify, six golden key vectors"
  - "CLAUDE.md: IDs, Manifest, Metrics, Outcome, Ledger, and Wire formats paragraphs; Projects optional keys, sweep-param limit, infra-streak note; Judge gate and scorecard bullet"
affects: [03-09, 03-10, 03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Event keys hash the run digest first, then an explicit per-kind tag, then the key fields, all through IdHasher, so keys are unique across runs and kinds"
    - "Key distinctness is tested against a test-side model of each event's key fields: equal fields give equal keys, distinct fields give distinct keys"
    - "A set-valued wire field (BackendInfo.layers) goes through a raw struct whose array must be strictly ascending, so each value has exactly one wire form"
key-files:
  created:
    - crates/minos-core/tests/prop_keys.rs
  modified:
    - crates/minos-core/src/ledger.rs
    - CLAUDE.md
decisions:
  - "BackendInfo deserializes through RawBackendInfo, and its layers array must be strictly ascending. A BTreeSet would otherwise drop a repeated layer without an error, and that value would re-serialize to different bytes"
  - "Record and Event are #[non_exhaustive], so a later V2 record or a new event is an additive change for downstream matches. Isolation and DecisionKind stay exhaustive, so the report and the breaker can match them in full"
  - "Decision derives Deserialize directly with deny_unknown_fields, because every kind and count combination is a valid record. 03-11 counts proposed and dropped separately, so the two counts have no ordering invariant"
  - "LineError::KeyMismatch is a unit variant with a fixed message. A line can come from a tampered file, so the error echoes nothing from it"
  - "A numeric version tag (\"v\":1 or \"v\":0) was probed and serde already rejects it. The test record_rejects_a_numeric_version_tag keeps it that way"
metrics:
  duration: "9 min"
  completed: 2026-10-14
  tasks: 2
  files: 3
---

# Phase 3 Plan 07: Ledger freeze Summary

This plan freezes the ledger wire format, so later plans can write golden files against it:

- An eleven-variant `Event`.
- `Record::V1`, written as `{"v":"1","<snake_case_variant>":{...}}`.
- The JSONL `Line { seq, key, record }` with `verify`.
- One documented, content-derived `EventKey` per event.

Six golden key vectors pin the key encoding. CLAUDE.md now records the frozen design.

## What was built

- **Keys (ledger.rs).**
  - `EventKey`, `SpendId`, and `ReviewId` are `digest_id!` types under `minos/event/v1`, `minos/spend/v1`, and `minos/review/v1`.
  - `Event::key(&self, run: &RunId) -> EventKey` uses one explicit match arm per kind, following the plan's frozen table.
  - Tags for `StopReason` and `ReviewKind` are private `const fn tag` match arms. No key depends on enum discriminants.
- **Module docs.** They hold:
  - The full key table, with the reason for each key field.
  - The `SpendId` and `ReviewId` encodings.
  - That generation 0 is baseline calibration.
  - The evolution rule.
  - That no ledger type holds a 128-bit integer or a dollar amount.
- **Payloads.**
  - `Origin::{Sweep, Model { model, prompt }}`.
  - `StopReason`: six variants, each with a fixed display string.
  - `ReviewKind` and `ReviewAnswer`.
  - `SpendSource::{ModelCall { call, model }, Candidate { exp, trial }, Judge { exp, trial }}`, with `exp()`.
  - `DecisionKind` and `Decision { kind, proposed, dropped }`.
  - `BackendKind::Krun`.
  - `Isolation::{WeakProcess, Hardware}`, ordered weakest first.
  - `ConfinementLayer::{Seatbelt, DedicatedUser, Rlimits}` and `EXPECTED_KRUN_LAYERS`.
  - `BackendInfo`.
  - Every struct, and every enum with struct variants, has `deny_unknown_fields`.
- **Record and Line.**
  - `Record` is `#[serde(tag = "v")]` with `#[serde(rename = "1")] V1(Event)`.
  - `Line::new` computes the key from the record and the run.
  - `Line::verify` recomputes the key and returns `LineError::KeyMismatch` when it differs.
- **Tests (prop_keys.rs, 32 tests).**
  - `key_table_covers_every_variant`: eleven events give eleven keys.
  - `distinct_key_fields_give_distinct_keys`: a proptest over up to 48 generated events with small seeds, so key fields often collide. It checks keys against a test-side `key_fields` model in both directions.
  - Non-key invariance for `Judged`, `Proposed`, `Spent`, and `RunStarted`.
  - Key separation: a manifest change A to B to A gives three keys; stop keys differ by generation and by reason; a review request and its answer differ; the same event under two runs differs.
  - Wire form: every variant round-trips under its tag, and the exact `RunStarted` wire form is checked.
  - Rejections: `"v":"2"`, a missing tag, a numeric tag, an unknown variant, extra fields at both levels, a missing field, and non-canonical backend layers.
  - `Line`: verify, the round trip, an unknown field, and a tampered record.
  - Fixed `StopReason` strings and snake_case enum spellings.
  - Golden vectors for `RunStarted`, `Judged` (trial 2), `StopRequested` (UserStop at generation 3), `SpendId` and the `Spent` key (ModelCall call 0, haiku, generation 1), and `ReviewId` (Params, `BlobId::of(b"review")`). Each was computed with `printf`, `xxd -r -p`, and `/usr/bin/shasum -a 256`; the recipe sits beside the literal.
- **CLAUDE.md.**
  - IDs: lists every digest type. Plain IDs are bare SHA-256 and composite IDs go through `IdHasher`; the sketch now uses `IdHasher`. Covers `baseline` and `RunId::of`.
  - Manifest: the rules 03-05 implemented, including the judge-path rule. Ends with "`Manifest::try_from` stays pure".
  - Metrics: `float_roundtrip`, the array wire form, and held-out metrics.
  - Outcome: `Crashed { stage }` and `TimedOut { stage }`, plus `Stage`.
  - Ledger: the frozen eleven-event set, the key and no-op rule, and the evolution rule.
  - Wire formats: insta snapshots plus a SHA-256 byte pin, and `.config/insta.yaml`.
  - Projects: the optional keys and defaults, the `[[param]]` shape, integer-only sweep params, and that a restart resets the infra streak.
  - Judge: a gate passes only at exactly 1, and the scorecard wire form.

## Task Commits

1. **Task 1 RED:** `2a9c0c8` test(03-07): add failing ledger key and wire-shape tests
2. **Task 1 GREEN:** `7090f02` feat(03-07): freeze the ledger Event set, keys, Record, and Line
3. **Task 2 vectors:** `fe504b7` test(03-07): pin ledger key golden vectors (also rewraps one ledger.rs doc paragraph)
4. **Task 2 docs:** `a2026ba` docs(03-07): record the frozen ledger and core decisions in CLAUDE.md

## Verification

| Check | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok |
| `cargo test --workspace` | ok (prop_keys: 32 passed) |
| `cargo deny check` | advisories, bans, licenses, sources ok |
| `cargo doc --workspace --no-deps` | ok, no warnings |
| C-01 lints table in every crate | no MISSING line |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `grep -n u128 crates/minos-core/src/ledger.rs` | prints nothing |
| `grep -rn 'serde_json::Value' crates/minos-core/src` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true, so no golden wire file precedes the freeze |
| Every event named on a `//!` line | yes |
| `grep -c 'shasum -a 256' tests/prop_keys.rs` | 8 |
| CLAUDE.md greps (Steered, StopRequested, Crashed { stage, float_roundtrip, base_images, exactly 1, the scorecard form, integer ranges, restart resets, stays pure; old Spent sketch gone) | all as required |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] Canonical wire form for BackendInfo.layers**
- **Found during:** Task 1
- **Issue:** A derived `BTreeSet` deserializer accepts `["seatbelt","seatbelt"]` or an unsorted array without complaint. The value then re-serializes to different bytes, which breaks the rule that deserialization is validation and makes golden bytes ambiguous.
- **Fix:** `BackendInfo` uses `#[serde(try_from = "RawBackendInfo", into = "RawBackendInfo")]`. The raw `layers` is a `Vec` that must be strictly ascending, or the result is `BackendInfoError::LayersNotCanonical`. The wire form is unchanged: a sorted array.
- **Files modified:** crates/minos-core/src/ledger.rs
- **Verification:** `backend_layers_must_be_canonical`
- **Committed in:** 7090f02

**2. [Rule 3 - Blocking] Prohibition grep on `u128`**
- **Found during:** Task 1 acceptance check
- **Issue:** The module doc said "`u128`" in its no-128-bit-integer sentence, so the prohibition grep printed a line.
- **Fix:** Reworded to "128-bit integer". No code change.
- **Committed in:** 7090f02, rewrapped in fe504b7

**Total deviations:** 1 Rule 2 (it only rejects more input) and 1 Rule 3 (wording). **Impact:** no field list, key, or invariant from the plan changed.

## TDD Gate Compliance

- Task 1 has a RED commit (`2a9c0c8`: prop_keys.rs fails to compile because the ledger types do not exist yet), then a GREEN commit (`7090f02`).
- Task 2's golden vectors pin code that Task 1 had already committed, so they passed as soon as they were written. A failing-first step was not possible for them. They were computed independently with the shell, which is the point of a golden vector.

## Issues Encountered

None.

## Known Stubs

None. `state.rs`, `policy.rs`, and the other skeleton modules belong to later plans (03-09 to 03-12), as the phase plans state.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-07-01 | `from_generation`, generation plus reason, and distinct request and answer tags in the keys; the distinctness proptest | ledger.rs `Event::key`; `distinct_key_fields_give_distinct_keys`, `manifest_a_to_b_to_a_gives_three_keys`, `stop_requests_are_keyed_by_generation_and_reason`, `a_review_request_and_its_answer_have_different_keys` |
| T-03-07-02 | `SpendId` over run, generation, and an indexed `SpendSource` | `spend_ids_separate_sources`, `spend_id_and_spent_key_golden_vectors` |
| T-03-07-03 | Closed `StopReason` with fixed strings; steering notes as `BlobId`; `Origin` holds `ModelId` and `PromptHash` | `stop_reason_display_strings_are_fixed` |
| T-03-07-04 | Domain `minos/event/v1`, run digest first, a tag per kind; golden vectors | `the_same_event_under_two_runs_gives_two_keys`, golden vector tests |
| T-03-07-05 | `deny_unknown_fields` everywhere, the `v` tag, the documented evolution rule | `record_rejects_*`, `line_round_trips_and_rejects_unknown_fields` |
| T-03-07-06 | No 128-bit field; `Spent` carries `Usage` only | the `u128` grep; the Spent field list |
| T-03-07-07 | `Line::verify` recomputes the key | `line_verifies_against_its_own_run_only`, `a_tampered_record_fails_verification` |

No new threat surface: the module does no I/O.

## Next Phase Readiness

- 03-09 can snapshot `Record` and `Line` against these exact shapes. Its kitchen-sink `BackendInfo` (Seatbelt and Rlimits) serializes as `["seatbelt","rlimits"]`.
- 03-10's fold can key records with `Event::key`, compare events with `PartialEq`, and store `StopReason`, `ReviewKind`, and `ReviewAnswer` in ordered sets (all derive `Ord`).
- 03-11 and 03-12 use `Decision`, `SpendSource`, `Origin`, `Isolation` (its minimum is the weakest level), and `EXPECTED_KRUN_LAYERS`.

## Self-Check: PASSED

- crates/minos-core/src/ledger.rs, crates/minos-core/tests/prop_keys.rs, and CLAUDE.md exist and contain the plan's artifacts.
- Commits 2a9c0c8, 7090f02, fe504b7, and a2026ba are in `git log`.
