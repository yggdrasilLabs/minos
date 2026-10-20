---
phase: 06-proposers
plan: 03
subsystem: propose
tags: [prompt, history, determinism, fences, held-out, golden, proptest, prompt-injection]
status: complete

requires:
  - phase: 03-core
    provides: reduce_trials, TrialRule, ReducedScore, Outcome, Reason, Stage, SafeText, LogExcerpt, IdHasher, Tree, Manifest, JudgeSpec
  - phase: 06-proposers
    plan: 02
    provides: Prompt value, PromptError, limits (HISTORY_GENERATIONS, HISTORY_TABLE_MAX_BYTES, EXCERPTS_MAX_BYTES, STEER_NOTES_MAX_BYTES, MAX_PROMPT_BYTES)
provides:
  - "minos_propose::prompt::HistoryEntry (new, generation, exp, is_accepted, rationale)"
  - "minos_propose::prompt::History (empty, new, table, excerpts, row_count, omitted_rows): rendered text only, no scorecard"
  - "minos_propose::prompt::SteerNote (new, ordinal, text), PromptInputs (public borrowed fields), assemble"
  - "PromptError::{MissingFile, NotText, SurfaceTooLarge}"
  - "Fence domain minos/prompt-fence/v1; prompt format heading '# Minos proposal request (format 1)'"
  - "Golden prompt snapshot pinned at c75a29cef272e83311ed570a5b1768e727ea937347f3cf7d0fcf93a5ec195ca8"
affects: [06-05 single-shot tier, 06-06, 07 loop (builds HistoryEntry values and chooses excerpts)]

tech-stack:
  added: []
  patterns:
    - "History view holds rendered strings only; the only metric names read are JudgeSpec::gates and Objective::metric"
    - "Content-derived fences: <<<BEGIN KIND label token>>> ... <<<END KIND token>>>, token = first 8 bytes of IdHasher(content)"
    - "Mutation check of a non-interference property: render the forbidden value, watch the property fail, restore"

key-files:
  created:
    - crates/minos-propose/src/prompt/history.rs
    - crates/minos-propose/tests/history.rs
    - crates/minos-propose/tests/prompt.rs
    - crates/minos-propose/tests/held_out.rs
    - crates/minos-propose/tests/held_out.proptest-regressions
    - crates/minos-propose/tests/snapshots/prompt__golden_prompt.snap
  modified:
    - crates/minos-propose/src/prompt.rs

key-decisions:
  - "An entry with no trials renders 'not judged' (the plan names no text for it); every other unscored row follows the plan's phrases"
  - "Excerpts come from unscored rows still in the table after the table cap, newest first, stopping at the first excerpt that does not fit"
  - "The output contract adds one line: a block ends only at the END line whose token matches its BEGIN line, so a forged END for another token is visibly not the end"
  - "The golden pin hashes the prompt bytes themselves: the prompt ends with exactly one newline, so the awk-printed snapshot body equals it"
  - "PromptError drops Copy, because MissingFile and NotText carry a RelPath"

patterns-established:
  - "Prompt sections are separated by one blank line; the suffix starts with a newline, so prefix().ends_with('>>>\\n')"
  - "Readable FILE blocks are labelled by path only; writable FILE blocks add 'final-newline: yes|no'"

requirements-completed: [PROP-05, PROP-06]

coverage:
  - id: D1
    description: "History rows: gates as pass/fail, signed percent deltas on visible objectives, fixed phrases, sanitized rationales, window, order, caps"
    requirement: PROP-06
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/history.rs (18 tests)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Golden prompt snapshot and SHA-256 pin"
    requirement: PROP-05
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/prompt.rs#golden_prompt"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/prompt.rs#golden_prompt_bytes_pinned"
        status: pass
    human_judgment: false
  - id: D3
    description: "Shuffled history and notes give identical bytes and PromptHash; prefix stable across history, notes, generation"
    requirement: PROP-05
    verification:
      - kind: property
        ref: "crates/minos-propose/tests/prompt.rs#shuffled_history_and_notes_give_the_same_prompt"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/prompt.rs#prefix_is_stable_across_history_notes_and_generation"
        status: pass
    human_judgment: false
  - id: D4
    description: "Hostile excerpt with a guessed END marker and a forged notes heading stays inside its fence"
    requirement: PROP-06
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/prompt.rs#hostile_excerpt_stays_inside_its_fence"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/prompt.rs#every_block_closes_with_its_own_token"
        status: pass
    human_judgment: false
  - id: D5
    description: "Held-out values and names never reach a prompt"
    requirement: PROP-06
    verification:
      - kind: property
        ref: "crates/minos-propose/tests/held_out.rs#held_out_values_never_change_the_prompt"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/held_out.rs#held_out_names_and_values_are_absent"
        status: pass
    human_judgment: false

duration: 14min
completed: 2026-10-19
---

# Phase 6 Plan 03: Deterministic prompt assembly Summary

**`assemble` builds the single-shot prompt as a pinned, pure function of its inputs: a byte-stable prefix (instructions, visible objectives and gates, the output contract, `program.md`, and the parent tree's surface files verbatim) and a suffix of history rows, untrusted failure excerpts, and the user's notes last. Every block is fenced with a token derived from its content, and no held-out metric's name or value can reach the text.**

## Performance

- **Duration:** about 14 min
- **Started:** 2026-10-19T21:23 (local -0400)
- **Completed:** 2026-10-19T21:37 (local -0400)
- **Tasks:** 3
- **Files modified:** 7 (6 created)

## Accomplishments

- `History::new` sorts entries by `(generation, exp)`, keeps the last `HISTORY_GENERATIONS` generations plus every winner, reduces candidates under `TrialRule::AllScored` and the incumbent under `TrialRule::ScoredOnly` (as `select_greedy` does), and renders one line per row. Scored rows show each gate as `pass` (exactly `Metric::ONE`) or `fail`, and each visible objective as `{:+.2}%` against the incumbent, or `n/a`. Unscored rows show the first non-scored trial in trial order, with fixed phrases. All four scorecard-shape reasons render `the judge's scorecard was invalid`, with no metric name.
- `History` holds only `table`, `excerpts` (label and sanitized text), and row counts. It never holds a `Scorecard`, an `Outcome`, or a metric value. `history.rs` never calls `Objective::held_out` or `JudgeSpec::held_out_metrics`.
- Caps: the table stays under `HISTORY_TABLE_MAX_BYTES`, dropping the oldest non-winner first, then the oldest winner, and ends with `(<n> earlier rows omitted)`. Excerpts stay under `EXCERPTS_MAX_BYTES`, newest whole excerpts first. Notes stay under `STEER_NOTES_MAX_BYTES`, keeping the highest ordinals, and end with `(<n> earlier notes omitted)`.
- Fences: `<<<BEGIN KIND label token>>>` and `<<<END KIND token>>>`, where the token is 16 hex characters of `IdHasher::new("minos/prompt-fence/v1").bytes(content)`. The golden fixture's crash log contains a guessed END line and a forged notes heading. Both sit wholly inside their own block, and the real END marker appears exactly once, after them.
- Refusals: a surface file missing from the parent is `MissingFile`, total program plus surface over `SINGLE_SHOT_SURFACE_MAX_BYTES` is `SurfaceTooLarge`, and a non-UTF-8 program or surface file is `NotText`. A lossy render never happens.
- Golden snapshot reviewed. Pin `c75a29ce…5ca8` was computed with `awk | shasum` from the snapshot body, and the test's own hash of the prompt bytes matched it.

## Task Commits

1. **Task 1: History view** - `4bdcc8e` (test, RED), `1391f8c` (feat, GREEN)
2. **Task 2: assemble(), fences, golden and shuffle tests** - `5327ff8` (test, RED), `07c9212` (feat, GREEN), `7077970` (docs: fence domain named once, so the acceptance grep prints one line)
3. **Task 3: Held-out non-interference** - `3588eb2` (test)

## Files Created/Modified

- `crates/minos-propose/src/prompt.rs` - module docs, `SteerNote`, `PromptInputs`, `assemble`, fences, three new `PromptError` variants
- `crates/minos-propose/src/prompt/history.rs` - `HistoryEntry`, `History`, row rendering, caps
- `crates/minos-propose/tests/history.rs` - 18 tests
- `crates/minos-propose/tests/prompt.rs` - 13 tests, including the golden snapshot, the pin, and the shuffle property
- `crates/minos-propose/tests/held_out.rs` - the non-interference property and the absence test
- `crates/minos-propose/tests/held_out.proptest-regressions` - the shrunk all-zero seed from the mutation check
- `crates/minos-propose/tests/snapshots/prompt__golden_prompt.snap` - the reviewed golden prompt

## Decisions Made

- An entry with no trials renders `not judged`. The plan names no text for that case, and calling it a scorecard fault would be wrong.
- Excerpts are drawn from rows still in the table after the table cap, so every excerpt label matches a visible row.
- The output contract says a block ends only at the END line whose token matches its BEGIN line, so the model can tell a forged END line apart.
- `PromptError` is no longer `Copy`, because two variants carry a `RelPath`. No caller relied on `Copy`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Golden pin hashes the prompt itself, not the prompt plus a newline**
- **Found during:** Task 2
- **Issue:** The schema test's pin hashes text plus one newline, because the schema has no final newline. The prompt already ends with exactly one newline, so the awk-printed snapshot body equals the prompt bytes, and hashing text plus a newline would never match.
- **Fix:** `golden_prompt_bytes_pinned` asserts the prompt ends with `>>>\n` and hashes `prompt.as_bytes()`. Its comment records the derivation.
- **Files modified:** crates/minos-propose/tests/prompt.rs
- **Commit:** 07c9212

**2. [Rule 3 - Blocking] Fence domain appeared twice in prompt.rs**
- **Found during:** overall verification
- **Issue:** The acceptance grep for `minos/prompt-fence/v1` must print one line, but the module docs also quoted the domain.
- **Fix:** The docs now say "the prompt-fence domain", and only the constant holds the string.
- **Files modified:** crates/minos-propose/src/prompt.rs
- **Commit:** 7077970

### Notes

- Task 3 is test-only. Its tests passed on the first run because Tasks 1 and 2 implement the invariant. To check that the tests can fail, a temporary edit rendered held-out values in each scored row: both tests failed, and the file was restored with `git checkout -- <file>`. The all-zero seed proptest shrank to during that run is committed.
- One extra test beyond the plan's rows: `entry_with_no_trials_is_not_judged`.

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking).
**Impact on plan:** No change in scope. Every invariant was kept. Held-out values are unreachable by construction, excerpts stay fenced and sanitized, and no `allow` was added.

## Issues Encountered

None.

## Known Stubs

None. Nothing yet builds `HistoryEntry` values from the ledger, and choosing which log tail becomes an entry's excerpt belongs to Phase 7, as 06-CONTEXT Deferred Ideas says.

## Threat Flags

None. No new surface beyond the plan's threat model. T-06-03-01 through T-06-03-05 are mitigated as planned.

## User Setup Required

None.

## Next Phase Readiness

- 06-05's single-shot tier calls `assemble(&PromptInputs { .. })` and passes the `Prompt` to the provider. `Prompt::hash` goes into `Origin::Model`.
- Phase 7 builds `HistoryEntry` values from `Proposed`, `Judged`, and `Selected` events, and `SteerNote` values from `Steered` blobs.

## Verification

- `cargo test -p minos-propose`: history 18, prompt 13, held_out 2, parse 27, provider 11, schema 8, all passing
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo deny check`, `cargo doc --workspace --no-deps` (no warnings): all pass
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing

## Self-Check: PASSED
