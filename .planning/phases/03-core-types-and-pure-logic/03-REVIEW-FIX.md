---
phase: 03-core-types-and-pure-logic
fixed_at: 2026-10-14T16:15:40Z
review_path: .planning/phases/03-core-types-and-pure-logic/03-REVIEW.md
iteration: 1
findings_in_scope: 12
fixed: 11
skipped: 1
status: partial
---

# Phase 03: Code Review Fix Report

**Fixed at:** 2026-10-14T16:15:40Z
**Source review:** .planning/phases/03-core-types-and-pure-logic/03-REVIEW.md
**Iteration:** 1
**Scope:** all (Critical, Warning, and Info)

**Summary:**
- Findings in scope: 12
- Fixed: 11
- Skipped: 1 (WR-03, design change that ripples across the crate)

Each fix is one commit on `main`, made in an isolated worktree on `gsd-reviewfix/03-54683` and fast-forwarded into `main` at the end. Every commit message ends with the `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` line. `cargo test -p minos-core` and `cargo clippy -p minos-core --all-targets -- -D warnings` were run after each fix, and the full six-command gate at the end (results below, verbatim).

## Per-finding table

| ID | Severity | Status | Commit | What was done |
|---|---|---|---|---|
| CR-01 | Critical | fixed | `307dc7a` | `base_images` strips a UTF-8 BOM from the first line, trims leading whitespace by Unicode `White_Space`, splits words on Unicode whitespace, and refuses a line whose first word is not a Dockerfile instruction keyword with the new `ImageError::UnknownInstruction` (never a skip). A `FROM` behind a BOM or NBSP is now `Unpinned`; heredoc bodies are refused in v1 and the module docs say so. Tests: BOM-prefixed `FROM`, five Unicode-whitespace prefixes, and unknown keywords including a heredoc body. |
| WR-01 | Warning | fixed: requires human verification | `b2b50f1` | New `policy::TrialRule { AllScored, ScoredOnly }`. Candidates still need every trial scored; `conclude` reduces the incumbent, and the report reduces the baseline, over scored trials only and counts the skipped ones (`ReducedScore::skipped`, `NoiseFloor::skipped`, `SummaryRow::baseline_skipped`). The report prints "N of M trials scored" only when a trial was skipped, so the two pinned report snapshots are unchanged. The anomaly flag no longer goes quiet for a lineage whose baseline has one unscored trial. CLAUDE.md's `minos-core` policy sentence records the rule. Flagged for human verification because it is a policy-logic decision (the review's first option) rather than a syntax fix. |
| WR-02 | Warning | fixed | `d62fac0` | Keys left unchanged (content-derived, golden-pinned, and the store's same-key no-op depends on them). The fold now returns `FoldError::DuplicateProposal { exp }` for a second `Proposed` of one experiment with different fields and `FoldError::NoteConflict { generation, ordinal }` for two different notes at one ordinal, each naming the obligation. The ledger key table documents that the loop checks `RunState::proposal(&exp).is_none()` before appending and that `minos steer` reads `next_note_ordinal` and appends under the ledger lock. The Steered key change (adding `digest(note)`) was not applied: it would alter a golden key, the `prop_keys` spec, and the notes map structure. |
| WR-03 | Warning | skipped | — | See "Skipped Issues" below. |
| WR-04 | Warning | fixed | `0c3431e` | `Manifest::try_from` refuses a judge path in `surface.readable` (`ManifestError::JudgeReadable`) and requires every judge path to lie under a `[source] exclude` entry (`ManifestError::JudgeNotExcluded`, checked after the case-fold rule so the existing case-fold test still reports the collision first). The judge program therefore reaches neither the prompt nor the source tarball. The breaker test fixture gained the `[source] exclude` it was missing, and the absent-`[source]` test now uses a bare program name. Module docs (rules renumbered 3 to 10) and CLAUDE.md (`minos-core` and Judge sections) record the rule. |
| WR-05 | Warning | fixed | `57b1f61` | `is_invisible_format` now covers the full General Category `Cf` set of Unicode 16.0 (U+0600 to U+0605, U+06DD, U+070F, U+0890, U+0891, U+08E2, U+206A to U+206F, U+FFF9 to U+FFFB, U+110BD, U+110CD, U+13430 to U+1343F, U+1BCA0 to U+1BCA3, U+1D173 to U+1D17A) plus the Mongolian variation selectors U+180B to U+180F. The edge test is extended, and a new test iterates every `char` against a pinned range table so a Unicode addition is a deliberate edit. |
| IN-01 | Info | fixed (documentation) | `4bf9d79` | Neither escape option was applied: rendering is idempotent (pinned by `rendering_is_idempotent`), and any escape of a literal `<U+` would re-escape the rendering of a hidden character on a second pass, so the ambiguity is inherent. The choice is documented next to `push_visible`: a `SafeText` holds no hidden code point, a literal notation is only ever visible text the author wrote, and the HiddenCharacters flag fires on actual code points, never the notation. |
| IN-02 | Info | fixed | `680aa35` | `IdHasher::new` asserts the domain holds no NUL byte, with a `# Panics` section; a `should_panic` test pins the message. |
| IN-03 | Info | fixed | `174ce80` | `judge_paths` applies the same rule as `CommandArgv::program` to every entry: a bare word (no `/` or `\`, no leading `./`, not `.` or `..`) names no project path, so `python3` and the `json` of `--format=json` no longer enter `JudgeSpec::paths`. Path-like entries that are not a valid `RelPath` are still refused. Landed before WR-04 so bare words need no exclude entry. Module docs and CLAUDE.md updated. |
| IN-04 | Info | fixed | `7cc4f8e` | New `ManifestError::Reserved { field, path }`: `minos.toml` and `minos.lock` are refused in `surface.writable`, and `minos.lock` in `surface.readable`, compared ASCII case-insensitively. The manifest itself may stay readable. Module docs and CLAUDE.md updated. |
| IN-05 | Info | fixed (documentation + test) | `407e39d` | `ReportInput::new` documents that `manifest` is the one in effect at the target's proposal generation (`RunState::manifest_at`) and why. A test shows a report and its hash survive a `ManifestChanged` recorded for a later generation. The lookup-closure alternative was not taken; the caller owns manifest storage. |
| IN-06 | Info | fixed | `cd920e3` | `JobResult::new` (now fallible and no longer `const`) and `TryFrom<RawJobResult>` return the new `JobError::TokensInJob` when the usage's token total is non-zero. Tests cover each token field through the constructor and the wire form. |

## Fixed Issues

### CR-01: A leading BOM or non-ASCII whitespace hides a `FROM` line from the base-image pin check

**Files modified:** `crates/minos-core/src/image.rs`, `crates/minos-core/tests/manifest.rs`
**Commit:** `307dc7a`
**Applied fix:** BOM strip on the first line, `char::is_whitespace` trim, `split_whitespace`, and an `INSTRUCTIONS` keyword table; an unknown first word is `ImageError::UnknownInstruction`. One deviation from the review's test list: `\u{00A0}FROM\u{00A0}alpine:3.20` is `Unpinned`, not `UnknownInstruction`, because `split_whitespace` splits on NBSP; both are errors, so the check still fails closed.

### WR-01: One non-scored baseline calibration trial permanently blocks `conclude`

**Files modified:** `CLAUDE.md`, `crates/minos-core/src/policy.rs`, `crates/minos-core/src/report.rs`, `crates/minos-core/tests/prop_policy.rs`, `crates/minos-core/tests/report.rs`
**Commit:** `b2b50f1`
**Applied fix:** `reduce_trials(outcomes, rule)` with `TrialRule`; `conclude` uses `ScoredOnly` for the incumbent and `AllScored` for candidates; `report::score` uses `ScoredOnly` for the baseline. New tests: `reduce_trials_skips_and_counts_unscored_trials_under_scored_only`, `conclude_reduces_the_incumbent_over_its_scored_trials`, `anomalous_jump_still_fires_when_a_baseline_trial_crashed` (also checks the "3 of 4 trials scored" rendering).

### WR-02: `Proposed` and `Steered` keys omit fields the event carries

**Files modified:** `crates/minos-core/src/state.rs`, `crates/minos-core/src/ledger.rs`, `crates/minos-core/tests/prop_fold.rs`
**Commit:** `d62fac0`
**Applied fix:** Dedicated `FoldError::DuplicateProposal` and `FoldError::NoteConflict`, key table documentation of the loop's and `minos steer`'s obligations. Keys unchanged.

### WR-04: Judge argv paths need not be excluded from the source snapshot or kept out of `readable`

**Files modified:** `CLAUDE.md`, `crates/minos-core/src/manifest.rs`, `crates/minos-core/tests/breaker.rs`, `crates/minos-core/tests/manifest.rs`
**Commit:** `0c3431e`
**Applied fix:** Rule 3 extended to `readable` (`JudgeReadable`); new rule 6 requires every judge path under an exclude entry (`JudgeNotExcluded`). The CLAUDE.md example manifest (`command = ["./judge"]`, `exclude = ["judge/", ...]`) still validates because a `judge/` entry covers the file `judge`.

### WR-05: `is_invisible_format` misses several `Cf` code points

**Files modified:** `crates/minos-core/src/text.rs`, `crates/minos-core/tests/render.rs`
**Commit:** `57b1f61`
**Applied fix:** Table extended as above; `invisible_table_edges` extended; new `invisible_table_is_exactly_the_pinned_ranges`.

### IN-01: `SafeText`'s `<U+XXXX>` notation is ambiguous with the same literal ASCII text

**Files modified:** `crates/minos-core/src/text.rs`
**Commit:** `4bf9d79`
**Applied fix:** Documentation of the trade-off next to `push_visible` (see table).

### IN-02: `IdHasher::new` does not enforce its no-NUL requirement

**Files modified:** `crates/minos-core/src/id.rs`, `crates/minos-core/tests/golden_ids.rs`
**Commit:** `680aa35`
**Applied fix:** `assert!` with a `# Panics` section; `should_panic` test.

### IN-03: `judge_paths` treats bare words as project-relative judge paths

**Files modified:** `CLAUDE.md`, `crates/minos-core/src/manifest.rs`, `crates/minos-core/tests/manifest.rs`
**Commit:** `174ce80`
**Applied fix:** Bare-word rule in `judge_paths`; new test `bare_judge_words_are_not_judge_paths`.

### IN-04: `minos.toml` and `minos.lock` can be listed as writable

**Files modified:** `CLAUDE.md`, `crates/minos-core/src/manifest.rs`, `crates/minos-core/tests/manifest.rs`
**Commit:** `7cc4f8e`
**Applied fix:** `ManifestError::Reserved`; new test `rejects_reserved_surface_files`.

### IN-05: The report re-parses a diff against the current manifest's `writable`

**Files modified:** `crates/minos-core/src/report.rs`, `crates/minos-core/tests/report.rs`
**Commit:** `407e39d`
**Applied fix:** `ReportInput::new` documentation; new test `a_report_survives_a_later_manifest_change`.

### IN-06: `JobResult` accepts token usage for a VM run

**Files modified:** `crates/minos-core/src/job.rs`, `crates/minos-core/tests/golden_wire.rs`
**Commit:** `cd920e3`
**Applied fix:** `JobError::TokensInJob` checked in `JobResult::new` and `TryFrom<RawJobResult>`; new test `a_result_carries_no_tokens`.

## Skipped Issues

### WR-03: Nothing in the type system distinguishes a composed cumulative diff from a raw proposal

**File:** `crates/minos-core/src/experiment.rs:40-46`, `crates/minos-core/src/id.rs:289-291`, `crates/minos-core/src/diff/compose.rs:65-111`
**Reason:** skipped: design change that ripples too widely to apply atomically in a review-fix pass. A `CumulativeDiff` newtype returned by `compose` and `diff_trees` and required by `ExperimentId::of` and `Experiment::candidate` touches 21 call sites across 7 files (`experiment.rs`, `id.rs`, `compose.rs`, `job.rs` `verify_diff`, `report.rs` `analyze`, and the `golden_wire`, `prop_diff`, `prop_apply`, and `report` tests). The report's and the guest's re-verification (`Diff::parse` of the stored blob, then `ExperimentId::of`) would have to re-derive through `diff_trees(base, apply(base, parsed))`, which needs the base tree on both sides; the test fixtures that build experiments from `Diff::parse` would all need a `Tree`. A half-application (newtype on `compose` only, `ExperimentId::of` still accepting any `Diff`) would not close the gap the review names. The module docs already place the obligation on the caller; recommend doing this with Phase 5's loop, which is the only producer of candidate IDs, so the newtype and its one consumer land together.
**Original issue:** `Experiment::candidate(base, diff)` and `ExperimentId::of(base, diff)` accept any `Diff`; a model's parsed proposal passed directly gets a different `ExperimentId` from the composed form of the same change, and for a generation-2 candidate a non-cumulative diff hashed under the base tree does not apply to its base.

## Verification gate (run in the worktree after all fixes, verbatim)

```
=== cargo fmt --all --check
=== exit 0 (cargo fmt --all --check)
=== cargo clippy --workspace --all-targets -- -D warnings
    Checking minos-vmm v0.1.0 (/private/tmp/sv-03-reviewfix-Ww3FOj/crates/minos-vmm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.00s
=== exit 0 (cargo clippy --workspace --all-targets -- -D warnings)
=== cargo test --workspace
(41 `test result` lines: 729 passed, 0 failed, 25 ignored; the ignored tests are the VM spikes)
=== exit 0 (cargo test --workspace)
=== cargo deny check
advisories ok, bans ok, licenses ok, sources ok
=== exit 0 (cargo deny check)
=== cargo doc --workspace --no-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.16s
   Generated /private/tmp/sv-03-reviewfix-Ww3FOj/target/doc/minos_backend_krun/index.html and 9 other files
=== exit 0 (cargo doc --workspace --no-deps)
=== grep -rnE '#!?\[allow\(' crates --include='*.rs'
=== exit 1 (grep; 1 means no match, which is the pass condition)
```

`cargo deny check` also printed a pre-existing warning (`unmatched wrapper` for `minos-runner`, a crate not yet in the workspace, on the `minos-sandbox` bans entry at `deny.toml:67`); it predates these fixes and does not affect the exit code.

Per-fix checks: after each commit, `cargo test -p minos-core` (all passing) and `cargo clippy -p minos-core --all-targets -- -D warnings` (clean). `cargo doc -p minos-core --no-deps` was run for the documentation-only commits.

## Notes for the orchestrator

- `cargo clean` was not run: CLAUDE.md requires it at the end of every phase after verification passes; the orchestrator or the phase-close step should run it in the main repository after committing this report.
- Rule renumbering in `manifest.rs` module docs (3 to 10) is documentation only; no `ManifestError` variant was removed or renamed, so no persisted wire format changed. The ledger `Event` set, the scorecard, `JobResult`'s fields, and the two report snapshots are byte-identical to before.
- A stray `crates/minos-core/tests/breaker.proptest-regressions` generated while the WR-04 breaker fixture was briefly invalid was deleted before the commit; it recorded a fixture bug, not a regression.

---

_Fixed: 2026-10-14T16:15:40Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
