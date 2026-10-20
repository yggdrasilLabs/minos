---
phase: 06-proposers
plan: 05
subsystem: propose
tags: [sweep, single-shot, proposer, fake-provider, generic-provider, drops, usage, origin, tdd]
status: complete

requires:
  - phase: 06-proposers
    plan: 02
    provides: ModelProvider, ModelRequest, ModelResponse, Completion, EmptyCause, ProviderError, ModelOutput, OutputSchema, parse_output, finish, Proposal, DropReason, DropCounts
  - phase: 06-proposers
    plan: 03
    provides: assemble, PromptInputs, Prompt::hash, History, SteerNote, PromptError
provides:
  - "minos_propose::proposer: ProposeInput (manifest, run, generation, base, parent, program, history, notes), Proposals (tier, items, dropped, empty, usage, models, origin, into_items), ProposeError (Generation0, TreeMismatch, TierMismatch, NoModel, NoProvider, TierNotInThisRelease, Sweep, Prompt, Schema, Provider; stop_reason, is_infra, usage), Proposer<P> (with_provider, without_provider, provider, propose)"
  - "minos_propose::sweep::propose(spec, input): no provider parameter, Origin::Sweep"
  - "minos_propose::single_shot::propose<P: ModelProvider>(provider, spec, input): exactly one complete call"
  - "minos_propose::fake: FakeProvider (new, requests), FakeReply (Structured, Empty, Error), RecordedRequest (model, prompt, schema)"
  - "Crate-root re-exports of the principal types"
  - "REQUIREMENTS.md PROP-07 and ROADMAP.md Phase 6 criterion 4 reworded to {rationale, files} (D-11)"
affects: [06-06 ClaudeCli spawn, 06-07 ClaudeCli through single_shot, 07 loop (calls Proposer::propose once per generation)]

tech-stack:
  added: []
  patterns:
    - "Tier entry points check tier, generation >= 1, and identical base/parent path sets before any work"
    - "Sweep renders against the parent, applies the step, and goes through the same finish helper as model proposals"
    - "Origin model: the single resolved model from per-model usage, else the spec's requested model"
    - "Test double replays raw JSON through ModelOutput::from_value, so every test runs the strict parser"

key-files:
  created:
    - crates/minos-propose/src/proposer.rs
    - crates/minos-propose/src/sweep.rs
    - crates/minos-propose/src/fake.rs
    - crates/minos-propose/src/single_shot.rs
    - crates/minos-propose/tests/sweep.rs
    - crates/minos-propose/tests/single_shot.rs
    - crates/minos-propose/tests/proposer.rs
  modified:
    - crates/minos-propose/src/lib.rs
    - crates/minos-propose/src/prompt.rs
    - crates/minos-propose/src/prompt/history.rs
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md

key-decisions:
  - "Origin::Model records the resolved model when the response names exactly one per-model entry, else the manifest's requested model"
  - "A sweep rationale is Sample::describe cut at a char boundary to MAX_RATIONALE_BYTES; Minos's own text, so cutting is not repair"
  - "Proposer<P> without a provider proposes only sweeps, so a sweep-only project needs no model login"

requirements-completed: [PROP-01, PROP-07, PROP-08, PROP-09, PROP-10]

duration: 6min
completed: 2026-10-20
---

# Phase 6 Plan 05: Sweep and Single-Shot Tiers Summary

**Sweep tier over core's seeded sampler with no provider, single-shot tier generic over `ModelProvider` with one call and strict parsing, a raw-JSON `FakeProvider`, and the `Proposer<P>` dispatcher the Phase 7 loop calls once per generation**

## Performance

- **Duration:** about 6 min
- **Started:** 2026-10-20T12:05:50Z
- **Completed:** 2026-10-20T12:11:30Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- `sweep::propose` takes no provider, so it cannot call a model. It draws `batch` samples from `minos_core::sweep::sample(run, generation, ..)`, renders each against the parent, applies the step, and derives a diff cumulative from the base through the shared `finish` helper. Drops: `SameAsParent`, `BackToBase`, `Sweep`, `Compose`, `DuplicateInBatch`. The same run and generation give byte-identical diffs.
- `single_shot::propose<P: ModelProvider>` assembles the prompt, builds the batch schema, and calls `complete` once. Output reaches proposals only through `parse_output`. Zero survivors is `Ok` with every drop counted, and an empty completion sets `empty()`. Usage and per-model usage are returned. The origin is `Origin::Model { model, prompt }`, where `prompt` is the hash of the exact prompt sent.
- `ProposeError` keeps configuration errors (never retried) apart from `Provider(..)`. `stop_reason()` maps a provider stop to `ProviderLimit` or `ProviderAuth`. `is_infra()` is true for a provider infrastructure failure. `usage()` keeps the spend of a failed call.
- `FakeProvider` replays raw JSON, empty completions, and typed errors, and records every request in call order. An exhausted script or a poisoned lock is `InfraCause::Fake`, never a panic.
- `Proposer<P>` matches every tier with no trait object. Sweep goes to `sweep::propose`, single-shot to `single_shot::propose` (or `NoProvider` when there is no provider), and planned and agentic give `TierNotInThisRelease`.
- D-11: PROP-07 in REQUIREMENTS.md and Phase 6 criterion 4 in ROADMAP.md now say `{rationale, files}` candidates, and both keep a note with the superseded wording.

## Task Commits

1. **Task 1: ProposeInput, Proposals, ProposeError, and the sweep tier**: `9d497ed` (test, RED), `495b4a0` (feat, GREEN)
2. **Task 2: FakeProvider and the single-shot tier**: `eb8d230` (test, RED), `28f1dbd` (feat, GREEN)
3. **Task 3: Proposer dispatch, crate-root exports, PROP-07 rewording**: `d1f1dc2` (test, RED), `b565139` (feat, GREEN)

## Files Created/Modified

- `crates/minos-propose/src/proposer.rs`: `ProposeInput`, `Proposals`, `ProposeError`, crate-private `check_input`, `Proposer<P>`
- `crates/minos-propose/src/sweep.rs`: the sweep tier
- `crates/minos-propose/src/single_shot.rs`: the single-shot tier
- `crates/minos-propose/src/fake.rs`: `FakeProvider`, `FakeReply`, `RecordedRequest` (its `Debug` prints lengths only)
- `crates/minos-propose/src/lib.rs`: the new modules and crate-root re-exports
- `crates/minos-propose/src/prompt.rs`, `src/prompt/history.rs`: `cut` widened from `pub(super)` to `pub(crate)` and re-exported inside the crate for the sweep rationale
- `crates/minos-propose/tests/{sweep,single_shot,proposer}.rs`: 10, 8, and 4 behavior tests
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`: the D-11 rewording

## Decisions Made

- The origin's model is the response's single resolved model (for example `claude-haiku-5-5`). When there are zero or several per-model entries, it is the spec's requested model, as documented on `ModelResponse`.
- `check_input` checks the tier before the input, so a wrong spec reports `TierMismatch` even when the generation is 0.
- `Proposals` has a crate-private constructor, which is how the T-06-05-03 mitigation holds. Code outside the crate cannot build a result with unvalidated proposals.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Shared the prompt's char-boundary `cut` with the sweep tier**
- **Found during:** Task 1
- **Issue:** The plan asks the sweep to cut `Sample::describe()` at a char boundary. The existing helper was `pub(super)` inside the private `prompt::history` module.
- **Fix:** Widened it to `pub(crate)` and added a crate-private re-export, `pub(crate) use history::cut;`, in `prompt.rs` instead of writing a second copy.
- **Files modified:** `crates/minos-propose/src/prompt.rs`, `crates/minos-propose/src/prompt/history.rs`
- **Commit:** `495b4a0`

### Notes

- PROP-07's checkbox in REQUIREMENTS.md was already `[x]` (traceability "Complete") before this plan ran. The plan says to leave it unchecked for 06-09. I changed only the wording and left the checkbox as I found it.
- The ROADMAP superseded-wording note sits just before the `(PROP-07, PROP-08, PROP-09, PROP-10)` group, where the plan put it.

## Issues Encountered

None. Two clippy findings in the new tests (a redundant closure and a complex return type) were fixed before the GREEN commits.

## Verification

- `cargo test -p minos-propose`: every target passes (sweep 10, single_shot 8, proposer 4, plus the earlier targets)
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo fmt --all --check`, `cargo doc -p minos-propose --no-deps`, `cargo deny check`: clean
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'` and `grep -rn 'dyn ModelProvider' crates/minos-propose`: both print nothing
- `grep -c '\.complete(' crates/minos-propose/src/single_shot.rs`: 1

## TDD Gate Compliance

Each task has a `test(06-05)` RED commit followed by a `feat(06-05)` GREEN commit. RED failed to compile because the modules did not exist yet. No REFACTOR commits were needed.

## Next Phase Readiness

- 06-06 and 06-07 can pass `ClaudeCli` to `single_shot::propose` or `Proposer::with_provider` with no change to any caller.
- The Phase 7 loop calls `Proposer::propose(spec, &ProposeInput { .. })` and records `Proposed` per item, `Decision::dropped` from `dropped().total()`, `Spent` from `models()` or `usage()`, and `StopRequested` from `ProposeError::stop_reason()`.

## Self-Check: PASSED

- Found every created file: proposer.rs, sweep.rs, fake.rs, single_shot.rs, tests/sweep.rs, tests/single_shot.rs, tests/proposer.rs
- Found all six commits: 9d497ed, 495b4a0, eb8d230, 28f1dbd, d1f1dc2, b565139
