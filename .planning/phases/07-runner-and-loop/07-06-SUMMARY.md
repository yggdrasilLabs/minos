---
phase: 07-runner-and-loop
plan: 06
subsystem: minos-core (diff identity), minos-propose (proposal type)
status: complete
tags: [core, diff, experiment, propose, trybuild, D-12, WR-03, LOOP-04, LOOP-05]
requires:
  - phase: 07-01
    provides: "the Phase 7 core types this plan builds on"
  - phase: 07-03
    provides: "minos-core dev-dependency trybuild"
provides:
  - "minos_core::diff::cumulative: BaseTree (new, id, tree), CumulativeDiff (recorded, base, as_diff, into_diff, exp), cumulative, NotRecorded (exp)"
  - "re-exports minos_core::{BaseTree, CumulativeDiff}"
  - "compose(&BaseTree, Option<&CumulativeDiff>, &Diff, ..) -> Result<CumulativeDiff, ComposeError>; ComposeError::BaseMismatch"
  - "Experiment::candidate(CumulativeDiff)"
  - "ProposeInput.base: &BaseTree; Proposal::{cumulative, into_parts -> (CumulativeDiff, String)}; parse_output(.., base: &BaseTree, ..)"
  - "test targets crates/minos-core/tests/cumulative.rs and compile_fail.rs; ui cases cumulative_from_parent_tree, experiment_from_bare_diff"
affects: [07-10, 07-12, 07-13, 07-16]
tech-stack:
  added: []
  patterns:
    - "Provenance as a type: a diff's base is carried beside it, and the only constructors are the base-handle minting paths or a re-read checked against its recorded hash"
    - "diff_trees stays generic (plain Diff), so a parent-relative render is never labelled cumulative"
key-files:
  created:
    - crates/minos-core/src/diff/cumulative.rs
    - crates/minos-core/tests/cumulative.rs
    - crates/minos-core/tests/compile_fail.rs
    - crates/minos-core/tests/ui/fail/cumulative_from_parent_tree.rs
    - crates/minos-core/tests/ui/fail/cumulative_from_parent_tree.stderr
    - crates/minos-core/tests/ui/fail/experiment_from_bare_diff.rs
    - crates/minos-core/tests/ui/fail/experiment_from_bare_diff.stderr
  modified:
    - crates/minos-core/src/diff/compose.rs
    - crates/minos-core/src/diff/mod.rs
    - crates/minos-core/src/experiment.rs
    - crates/minos-core/src/lib.rs
    - crates/minos-core/tests/golden_wire.rs
    - crates/minos-core/tests/guest_wire.rs
    - crates/minos-core/tests/prop_apply.rs
    - crates/minos-guest/src/test_fixture.rs
    - crates/minos-backend-krun/src/input_disk.rs
    - crates/minos-backend-krun/tests/harness/mod.rs
    - crates/minos-propose/src/proposer.rs
    - crates/minos-propose/src/proposal.rs
    - crates/minos-propose/tests/parse.rs
    - crates/minos-propose/tests/sweep.rs
    - crates/minos-propose/tests/single_shot.rs
    - crates/minos-propose/tests/proposer.rs
    - crates/minos-propose/tests/claude_cli.rs
    - crates/minos-propose/tests/live.rs
decisions:
  - "CumulativeDiff has a pub(super) minted constructor, so only diff::cumulative and diff::compose can mint one; outside core the only paths are cumulative, compose, and recorded"
  - "NotRecorded names the recorded experiment ID the diff failed to match and has no public constructor"
  - "compose checks the parent's base before applying anything, so a foreign parent is BaseMismatch, never an Apply error"
  - "Experiment keeps its Option<Diff> field and diff() accessor; candidate takes base and ID from the CumulativeDiff, so no Experiment accessor changed"
  - "Propose test helpers wrap their existing base Tree in BaseTree::new(TreeId::of(b\"base\"), ..) inside the helper, so the many call sites stay as they were"
  - "CLAUDE.md is not edited here: plan 07-16 Task 3 is the one writer for the CumulativeDiff text"
metrics:
  duration: 10min
  completed: 2026-10-23
  tasks: 3
  files: 25
---

# Phase 7 Plan 06: CumulativeDiff Summary

A diff that is cumulative from the run's base is now its own type. `CumulativeDiff` can be minted only against the `BaseTree` handle, through `cumulative` or `compose`, or rebuilt from a diff re-read from its blob that hashes to the recorded `ExperimentId`. `Experiment::candidate` accepts nothing else, so a parent-relative diff or a bare `Diff::parse` result can no longer become an experiment ID (D-12, 03-REVIEW WR-03). No wire byte, golden, or ID changed.

## What was built

**Task 1 (RED 0cca11a, GREEN a903c5e):** New `crates/minos-core/src/diff/cumulative.rs`:
- `BaseTree { id, tree }`, with a manual Debug that prints the ID and the file count.
- `CumulativeDiff { base, diff }`, with private fields and no conversion from `Diff`.
- `cumulative(&BaseTree, &Tree, ..)`, a wrapper around `diff_trees`.
- `CumulativeDiff::recorded(base, diff, &exp) -> Result<_, NotRecorded>`.

`compose` now takes `&BaseTree` and `Option<&CumulativeDiff>` and returns `CumulativeDiff`. It refuses a parent minted against another base with the new `ComposeError::BaseMismatch`. `diff_trees` keeps its signature and its plain `Diff` result. `Experiment::candidate(CumulativeDiff)` takes the base and the ID from the diff. `tests/cumulative.rs` has 9 tests, one per behavior row plus a Debug check, and they pin that `cumulative` and `compose` give the same bytes the untyped functions gave. `tests/compile_fail.rs` runs two trybuild cases. Each `.stderr` came from the real compiler and was checked: both show `error[E0308]: mismatched types`, one `expected &BaseTree, found &Tree` and the other `expected CumulativeDiff, found Diff`.

**Task 2 (9f993d3):** Each fixture now builds `Experiment::candidate(CumulativeDiff::recorded(base, diff, &ExperimentId::of(&base, &diff))?)`. These fixtures are in the guest, the krun input-disk tests, the krun VM harness, and core's `golden_wire` and `guest_wire`. `prop_apply` composes against a `BaseTree` built by a `based(tree)` helper and passes the parent as the previous `CumulativeDiff`. Every property's byte and ID assertion is unchanged, and `same_tree_same_experiment_id` also checks that `CumulativeDiff::exp` equals the hashed IDs.

**Task 3 (1bbe9fb):** `ProposeInput.base` is `&BaseTree`, and `check_input` compares the paths of `base.tree()`. `Proposal` holds a `CumulativeDiff`. `diff()` still returns `&Diff`, `cumulative()` is new, and `into_parts()` returns `(CumulativeDiff, String)`. `finish` mints with `cumulative(base, ..)` and keeps the same `SameAsParent`, `BackToBase`, and `Compose` mapping. `parse_output` takes `&BaseTree`. The sweep still renders against the parent with `diff_trees`, which stays parent-relative and is never labelled cumulative. `sweep.rs` and `single_shot.rs` needed no source edit, because `input.base` now has the right type when they pass it. The new test `every_proposal_is_minted_cumulative_from_the_input_base` in `tests/proposer.rs` checks, for both the sweep and single-shot tiers, that each item's `cumulative().base()` equals the input base's ID.

## Verification

All gate commands exit 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 1464 passed, 0 failed, 23 ignored (VM and live tests)
- `cargo deny check`: advisories, bans, licenses, and sources ok
- `cargo doc --workspace --no-deps`: no warnings after cb9c75a
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

C-01 printed only `ok` lines. C-02 printed nothing. `git diff 5c95720 -- crates/minos-core/tests/golden_ids.rs crates/minos-core/tests/snapshots crates/minos-propose/tests/snapshots` is empty, so no golden or ID vector moved. `cargo build --workspace --all-targets` passed at the end of Task 3: the compiler checked that every `Experiment::candidate` call passes a `CumulativeDiff`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Redundant intra-doc link target**
- **Found during:** final gate (`cargo doc --workspace --no-deps`)
- **Issue:** the new `diff_trees` doc sentence wrote ``[`cumulative`](super::cumulative::cumulative)`` while `cumulative` was already imported, so rustdoc warned `redundant_explicit_links`
- **Fix:** shortened it to ``[`cumulative`]``
- **Files modified:** crates/minos-core/src/diff/compose.rs
- **Commit:** cb9c75a

**2. [Rule 3 - Blocking] Shadowed fixture function in input_disk tests**
- **Found during:** Task 2
- **Issue:** binding a local named `diff` shadowed the `diff()` fixture function, which the same test calls again
- **Fix:** renamed the local to `parsed`
- **Files modified:** crates/minos-backend-krun/src/input_disk.rs
- **Commit:** 9f993d3

Otherwise the plan ran as written. `sweep.rs` and `single_shot.rs` are listed in the plan's files but needed no edit (see Task 3).

## TDD Gate Compliance

Task 1 has `tdd="true"`. The RED commit `0cca11a` (`test(07-06)`, tests that failed to compile against the old API) comes before the GREEN commit `a903c5e` (`feat(07-06)`). No refactor commit was needed.

## Threat Flags

None. The plan adds no new endpoint, file access, or trust-boundary surface. T-07-06-01 to 04 are mitigated as planned: no conversion from `Diff`, `recorded` checks the hash, `compose` returns `BaseMismatch` for a foreign parent, and the goldens are unchanged.

## Self-Check: PASSED

- All 7 created files exist on disk.
- Commits 0cca11a, a903c5e, 9f993d3, 1bbe9fb, and cb9c75a are in `git log`.
