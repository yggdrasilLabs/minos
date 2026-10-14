---
phase: 03-core-types-and-pure-logic
plan: 06
subsystem: minos-core
status: complete
tags: [core, diff, applier, compose, experiment, diffy, proptest]
requires:
  - "03-01: RelPath, Limits, TreeId, ExperimentId::baseline"
  - "03-02: Diff, FileDiff, Hunk, HunkLine (text, has_newline), DiffError, Diff::parse, ExperimentId::of"
  - "03-03: Reason::ApplyFailed { path, hunk }"
provides:
  - "diff/apply.rs: Tree (new, insert, get, contains, iter, len, is_empty), apply, ApplyError (MissingFile, ContextMismatch, OutOfRange, reason)"
  - "diff/compose.rs: diff_trees, compose, ComposeError (Apply, NotText, PathSetChanged, NoChange, Invalid)"
  - "experiment.rs: Experiment (baseline, candidate, id, base, diff, is_baseline)"
affects: [03-08, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "An exact-position applier with an output that closes after a line without a newline, so no later line can be glued onto it"
    - "Cumulative diffs are re-derived from file contents (diffy::create_patch per changed file), rendered from diffy's hunk structure, then validated by Diff::parse"
key-files:
  created:
    - crates/minos-core/tests/prop_apply.rs
  modified:
    - crates/minos-core/src/diff/apply.rs
    - crates/minos-core/src/diff/compose.rs
    - crates/minos-core/src/experiment.rs
    - CLAUDE.md
decisions:
  - "Only a file's last line can lack a newline, so the applier returns OutOfRange when a hunk appends after a base line without a newline, or when a hunk's new side ends without a newline before the base ends. Diff::parse cannot see these cases because it does not know the base"
  - "A hunk whose old side runs past the end of the file is OutOfRange, not ContextMismatch: there is no base line to name"
  - "diff_trees writes each section from diffy's parsed Patch (hunks, ranges, lines), not from its Display. The Display output depends on formatter options (suppress_blank_empty, the original/modified header names). Diff::parse canonicalizes the text either way"
  - "Experiment::id() and base() return by value, because ExperimentId and TreeId are Copy. The ID is computed once, at construction"
  - "A ComposeError raised while applying the parent's cumulative diff is still a candidate fault, as the plan specifies. A parent diff that came from compose always applies to the same base"
metrics:
  duration: "9 min"
  completed: 2026-10-14
  tasks: 2
  files: 5
---

# Phase 3 Plan 06: Strict applier, cumulative composition, and Experiment Summary

This plan adds three pieces:

- **`apply(&Tree, &Diff)`:** applies a canonical diff only at the positions it states. Every context and deleted line must equal the base line byte for byte, including whether it has a newline. There is no offset search, no fuzz, and no create or delete branch.
- **`compose`:** re-derives each experiment's diff from the run's base tree. Equal candidate trees therefore give byte-identical diffs and equal `ExperimentId`s, and a proposal that restores the base is `NoChange`.
- **`Experiment`:** a baseline with no diff, or a candidate with a non-empty parsed `Diff`.

## What was built

- **Tree (diff/apply.rs).**
  - A `BTreeMap<RelPath, Vec<u8>>` behind a private field.
  - Methods: `new` (const), `Default`, `insert`, `get`, `contains`, `iter` (path order), `len`, and `is_empty`.
  - A hand-written Debug prints `Tree {path: bytes, ...}`, never content.
- **apply.** It clones the tree and splits each target file into lines that keep their `\n`. It then walks the hunks with a cursor:
  - **Start index:** `old_start - 1`, or `old_start` when `old_len == 0`. The start must lie between the cursor and the line count.
  - **Matching:** context and delete lines are compared through `HunkLine::text()` plus `has_newline()`. Context and insert lines are written to the output, and the rest of the file is copied after the last hunk.
  - **Output rule:** the output closes once it has written a line without a newline, and any later push is `OutOfRange`.
  - **Indexing:** none. The code uses `.get()`, iterators, and `strip_suffix`.
- **ApplyError.** A `#[non_exhaustive]` thiserror enum with three variants:
  - `MissingFile { path }`
  - `ContextMismatch { path, hunk, line }` (1-based hunk and base line)
  - `OutOfRange { path, hunk }`

  `reason()` maps each one to `Reason::ApplyFailed { path, hunk }`, with hunk 0 for a missing file. Display and Debug carry no content.
- **diff_trees (diff/compose.rs).**
  1. The path sets must match. Otherwise the result is `PathSetChanged`, naming the first path present in only one tree, found by a merge walk.
  2. Each file whose bytes differ, in path order, must be UTF-8 on both sides (`NotText`). It becomes one section: the three canonical header lines, then hunks rendered from `diffy::create_patch` with explicit lengths, signs, and no-newline markers.
  3. If no file changed, the result is `NoChange`.
  4. The text goes through `Diff::parse`. A failure, such as a non-writable path or a cap, is `Invalid`.
- **compose.** Applies the parent's cumulative diff (if any), then the proposal, then calls `diff_trees(base, candidate)`.
- **Experiment (experiment.rs).**
  - Private fields `id`, `base`, and `diff: Option<Diff>`.
  - `baseline(base)` uses `ExperimentId::baseline(&base)`, and `candidate(base, diff)` uses `ExperimentId::of(&base, &diff)`.
  - Accessors: `id`, `base`, `diff`, and `is_baseline`.
  - Debug shows the id, the base, and `has_diff`.
- **CLAUDE.md.** One sentence in the Diff paragraph records the applier rules (including the end-of-file rule) and composition.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok |
| `cargo test --workspace` | exit 0; prop_apply 35 passed (30 tests, 5 proptests) |
| `PROPTEST_CASES=20000 cargo test -p minos-core --test prop_apply` | 35 passed |
| `cargo deny check` | advisories, bans, licenses, and sources all ok. The only warning is the existing `unmatched wrapper` for minos-sandbox |
| `cargo doc --workspace --no-deps` | ok, no warnings |
| C-01 `[lints] workspace = true` | all 10 crates ok |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `grep -rn 'diffy::apply' crates/minos-core/src` | prints nothing |
| `grep -nE '\.remove\(' crates/minos-core/src/diff/apply.rs` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true |

## TDD Gate Compliance

Both tasks were committed RED, then GREEN. Each RED commit failed to compile because the API it tests did not exist yet.

- Task 1: RED `1ce5f7a` (test), then GREEN `c90d710` (feat)
- Task 2: RED `19c79ba` (test), then GREEN `f538d28` (feat)

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | 1ce5f7a | test(03-06): add failing applier tests for exact-position apply |
| 1 GREEN | c90d710 | feat(03-06): implement Tree and the strict exact-position applier |
| 2 RED | 19c79ba | test(03-06): add failing composition and Experiment tests |
| 2 GREEN | f538d28 | feat(03-06): cumulative composition and the Experiment type |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] End-of-file rule in the applier**
- **Found during:** Task 1
- **Issue:** The plan's algorithm would accept two cases that Diff::parse cannot catch, because it does not know the base:
  - A hunk that inserts after a base last line lacking its newline. The base `a` plus `@@ -1,0 +2,1 @@ +b` would give `ab\n`, which silently changes line 1.
  - A hunk whose new side ends without a newline while base lines remain. The output would glue the remaining lines onto that line.
- **Fix:** The output closes after a line without a newline, and any later line is `OutOfRange`. Tests: `rejects_an_insert_after_a_line_without_newline`, `rejects_a_new_end_of_file_before_the_base_ends`. A hunk that runs past the end is also `OutOfRange` (`rejects_a_hunk_that_runs_past_the_end`).
- **Commit:** c90d710

**2. [Rule 1 - Robustness] Sections rendered from diffy's structure, not its Display**
- **Found during:** Task 2
- **Issue:** The plan says to drop diffy's two header lines from its `Display` output. Those header lines appear only when diffy has file names, and the formatter's `suppress_blank_empty` default decides how blank context lines are written. Stripping two lines blindly could drop a hunk line.
- **Fix:** `write_section` writes the hunks from `Patch::hunks()`, `HunkRange::start/len`, and `diffy::Line`. `create_patch` and `Diff::parse` are still the key links. The test helper keeps the plan's Display-based recipe, and `compose_matches_sequential_apply` asserts that both routes produce identical canonical bytes.
- **Commit:** f538d28

**3. [Rule 3 - Blocking] Proptest reject budget**
- **Found during:** Task 1, in a run with `PROPTEST_CASES=20000`
- **Issue:** `context_change_is_rejected` hit proptest's global reject limit, because an empty base yields a diff that reads no base line. This was a budget abort, not a counterexample.
- **Fix:** That property draws its base from a `non_empty_text()` strategy with at least one line.
- **Commit:** c90d710

**4. [Rule 2 - Docs] CLAUDE.md kept current**
- One sentence in the Diff paragraph records the applier and composition rules, including the new end-of-file decision.
- **Commit:** f538d28

No dependency, lockfile, `clippy.toml`, or `deny.toml` change.

## Known Stubs

None.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-06-01 | Exact position, byte-equal lines including newline presence, no search | `context_change_is_rejected`, `never_searches_another_offset`, `rejects_a_newline_presence_mismatch` |
| T-03-06-02 | No create or delete branch; the path set is preserved | `apply_never_adds_or_removes_paths`; no `.remove(` in apply.rs |
| T-03-06-03 | Diffs re-derived by create_patch and re-validated | `same_tree_same_experiment_id` (four differently written proposals, one ID), `compose_matches_sequential_apply` |
| T-03-06-04 | `ComposeError::NoChange` | `reverting_the_parent_is_no_change`, `diff_trees_of_equal_trees_is_no_change` |
| T-03-06-05 | Hand-written Debug; errors carry only paths and numbers | `debug_omits_content`, `apply_errors_display_without_content` |
| T-03-06-06 | The applier is linear, with one cursor and no search; Diff::parse caps the input | apply.rs structure |

No new threat surface beyond the register.

## Self-Check: PASSED

- crates/minos-core/src/diff/apply.rs, crates/minos-core/src/diff/compose.rs, crates/minos-core/src/experiment.rs, and crates/minos-core/tests/prop_apply.rs exist and are committed.
- Commits 1ce5f7a, c90d710, 19c79ba, and f538d28 are in `git log`.
