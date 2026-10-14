---
phase: 03-core-types-and-pure-logic
reviewed: 2026-10-14T15:50:23Z
depth: standard
files_reviewed: 42
files_reviewed_list:
  - Cargo.toml
  - .config/insta.yaml
  - crates/minos-core/src/lib.rs
  - crates/minos-core/src/limits.rs
  - crates/minos-core/src/id.rs
  - crates/minos-core/src/path.rs
  - crates/minos-core/src/text.rs
  - crates/minos-core/src/metric.rs
  - crates/minos-core/src/outcome.rs
  - crates/minos-core/src/usage.rs
  - crates/minos-core/src/money.rs
  - crates/minos-core/src/image.rs
  - crates/minos-core/src/manifest.rs
  - crates/minos-core/src/diff/mod.rs
  - crates/minos-core/src/diff/apply.rs
  - crates/minos-core/src/diff/compose.rs
  - crates/minos-core/src/experiment.rs
  - crates/minos-core/src/ledger.rs
  - crates/minos-core/src/state.rs
  - crates/minos-core/src/policy.rs
  - crates/minos-core/src/route.rs
  - crates/minos-core/src/sweep.rs
  - crates/minos-core/src/job.rs
  - crates/minos-core/src/report.rs
  - crates/minos-core/tests/breaker.rs
  - crates/minos-core/tests/golden_ids.rs
  - crates/minos-core/tests/golden_wire.rs
  - crates/minos-core/tests/manifest.rs
  - crates/minos-core/tests/money.rs
  - crates/minos-core/tests/prop_apply.rs
  - crates/minos-core/tests/prop_diff.rs
  - crates/minos-core/tests/prop_fold.rs
  - crates/minos-core/tests/prop_keys.rs
  - crates/minos-core/tests/prop_metric.rs
  - crates/minos-core/tests/prop_path.rs
  - crates/minos-core/tests/prop_policy.rs
  - crates/minos-core/tests/render.rs
  - crates/minos-core/tests/report.rs
  - crates/minos-core/tests/route.rs
  - crates/minos-core/tests/sweep.rs
  - crates/minos-core/tests/snapshots/report__clean_report.snap
  - crates/minos-core/tests/snapshots/report__weak_isolation_report.snap
findings:
  critical: 1
  warning: 5
  info: 6
  total: 12
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-10-14T15:50:23Z
**Depth:** standard
**Files Reviewed:** 42
**Status:** issues_found

## Summary

This review covered the whole of `minos-core` as landed in Phase 3: the ID and name types, `RelPath`, the `SafeText` renderer, `Metric`/`Scorecard`, `Usage` and the pico-dollar money types, the Dockerfile pin checker, the manifest validator, the three-stage diff pipeline (parse, strict apply, compose), the frozen ledger wire format and its content-derived keys, the fold, policy, routing, the seeded sweep, the job contract, and the report builder, plus the sixteen test files and two report snapshots. The workspace lint gate (`forbid` on `unwrap`, `expect`, indexing, `unsafe`; no `allow` anywhere) holds: the C-02 grep over `crates/` prints nothing, and no test uses `unwrap`, `expect`, direct indexing, `HashMap`, a clock, or a sleep. All 26 insta snapshots are committed and `.config/insta.yaml` pins `update: no`.

The invariants the phase set out to protect are, in the main, enforced where they claim to be. Every parsed type reaches its validated form through the same constructor serde uses (`try_from` on every raw struct, `deny_unknown_fields` on every raw struct and payload enum, and a probe confirmed that an extra field or a second variant key inside the internally tagged `Record` is refused). `Metric` cannot be NaN or infinite; no dollar figure is a float or enters the ledger; policy sorts trials and ranks candidates by `(value, ExperimentId)`; the fold keys every record and builds state from a `BTreeMap` so record order cannot leak into the result; the applier has no offset search and no create or delete branch; the report passes every sandbox- or model-originated string through `SafeText`. The error enums carry IDs, counts, and validated names only. The tests are unusually thorough (golden vectors with shell recipes, pinned snapshot digests, permutation proptests for `fold`, `select_greedy`, and `conclude`).

The review found one Critical defect and five Warnings. The Critical one is a bypass of the Dockerfile base-image pin check: a leading UTF-8 BOM (or any non-ASCII-space leading whitespace Docker strips) hides a `FROM` line from `base_images`, which then returns `Ok([])` for an unpinned image (reproduced with a scratch probe). The Warnings are robustness and design gaps rather than hostile-input bypasses: a single non-scored baseline calibration trial permanently blocks `conclude` while the report's noise floor quietly tolerates it; the content-derived keys for `Proposed` and `Steered` turn an ordinary re-proposal or a concurrent steer into `FoldError::KeyConflict` (reproduced); nothing in the type system distinguishes a composed cumulative diff from a raw proposal before it is hashed into an `ExperimentId`; the manifest does not require judge argv paths to be excluded from the candidate's source snapshot or kept out of `readable`; and the invisible-format table in `SafeText` is missing several `Cf` code points that then render invisibly in reports.

## Critical Issues

### CR-01: A leading BOM or non-ASCII whitespace hides a `FROM` line from the base-image pin check

**File:** `crates/minos-core/src/image.rs:237-247`, `crates/minos-core/src/image.rs:281-287`
**Issue:** `instructions` trims only `' '` and `'\t'` from a physical line, and `base_images` recognises an instruction only when `split_ascii_whitespace().next()` is case-insensitively `FROM`; any other first word is silently `continue`d. Docker's parser strips a UTF-8 BOM from the first line and trims leading whitespace with `unicode.IsSpace` (which includes U+000B, U+000C, U+0085, and U+00A0). So a Dockerfile that begins `\u{FEFF}FROM alpine:3.20` (the form Windows editors save by default), or a line such as `\u{00A0}FROM alpine:3.20`, is a valid unpinned `FROM` to Docker but an unrecognised word to this function, which returns `Ok([])`. Reproduced with a scratch crate: `base_images("\u{feff}FROM alpine:3.20\nRUN true\n")` returns `Ok([])`, while the same text without the BOM returns `Err(Unpinned { line: 1 })`. The function exists to make "images are digests, never tags" (CORE-03) fail closed, and `minos env build` and `minos check` are to call it; a single invisible byte sequence defeats the whole check without an error. The design treats shared projects as in scope ("experiments from other people run on the user's hardware"), so a project's Dockerfile is not fully trusted input.
**Fix:** Normalise the way Docker does and fail closed on anything else. Strip a leading BOM from the first line, trim with `char::is_whitespace` (which covers U+000B, U+000C, U+0085, U+00A0), split words with `split_whitespace`, and refuse a line whose keyword is not a known Dockerfile instruction instead of skipping it:
```rust
const INSTRUCTIONS: &[&str] = &[
    "FROM", "RUN", "CMD", "LABEL", "MAINTAINER", "EXPOSE", "ENV", "ADD", "COPY", "ENTRYPOINT",
    "VOLUME", "USER", "WORKDIR", "ARG", "ONBUILD", "STOPSIGNAL", "HEALTHCHECK", "SHELL",
];

// in `instructions`: strip the BOM once, trim like Docker
let raw = if index == 0 { raw.strip_prefix('\u{FEFF}').unwrap_or(raw) } else { raw };
let trimmed = raw.trim_start_matches(char::is_whitespace);

// in `base_images`: an unknown keyword is an error, never a skip
let mut words = text.split_whitespace();
let keyword = words.next().ok_or(ImageError::UnknownInstruction { line })?;
if !INSTRUCTIONS.iter().any(|known| keyword.eq_ignore_ascii_case(known)) {
    return Err(ImageError::UnknownInstruction { line });
}
if !keyword.eq_ignore_ascii_case("FROM") { continue; }
```
Add tests for a BOM-prefixed `FROM`, an NBSP-prefixed `FROM`, and an unknown keyword, each expecting an error.

## Warnings

### WR-01: One non-scored baseline calibration trial permanently blocks `conclude`, while the report's noise floor silently tolerates it

**File:** `crates/minos-core/src/policy.rs:154-196`, `crates/minos-core/src/policy.rs:300-302`, `crates/minos-core/src/policy.rs:375-392`, `crates/minos-core/src/report.rs:352-358`, `crates/minos-core/src/report.rs:666-683`
**Issue:** `reduce_cards` returns `None` when *any* trial is not `Outcome::Scored` (or, through `valid_score`, fails `check_scorecard`). `conclude` applies that rule to the incumbent, so a baseline with five calibration trials of which one is `TimedOut` or `Crashed` (a plausible outcome of a busy host during `minos check`) has no valid score under that judge version, every generation returns `PolicyError::IncumbentUnscored`, and the run can never proceed. There is no recovery short of pushing a new judge version or starting a new run, because `Judged` is keyed by `(exp, judge, trial)` and a re-run of the same trial index with a different outcome is a `KeyConflict` (or a silent store no-op). Meanwhile `noise_floor` uses `filter_map(scorecard)` and skips the unscored trials, so `summary_row` in the report still produces a baseline row, but `anomaly_flag` calls `score(state, judge, baseline)`, which is `None`, so `step(last)` is `None` and the AnomalousJump flag is silently never raised for a lineage whose baseline has one unscored trial. The two modules disagree about what a baseline with a failed trial means, and neither surfaces the disagreement.
**Fix:** Decide one rule and apply it in both places. The least surprising is: a candidate is invalid if any of its trials failed (a candidate that crashes once is unreliable), but the incumbent's and the baseline's reduced score is taken over its `Scored` trials, with the count of skipped trials reported. Concretely, give `reduce_cards` a mode, use the lenient mode in `valid_score` for the incumbent and in `report::score` when `exp == state.baseline()`, and have `summary_row`/`noise_floor` expose `skipped_trials` so the report can print "3 of 4 trials scored". If instead the strict rule is intended, make `noise_floor` strict too and have `minos check` refuse to start a run whose baseline has a non-scored trial, so the failure is reported once, up front, rather than on every generation.

### WR-02: `Proposed` and `Steered` keys omit fields the event carries, so an ordinary re-proposal or a concurrent steer is a `KeyConflict`

**File:** `crates/minos-core/src/ledger.rs:632`, `crates/minos-core/src/ledger.rs:646-653`, `crates/minos-core/src/state.rs:316-330`
**Issue:** `Event::Proposed`'s key is `digest(exp)` alone, but the event also carries `parent`, `generation`, `tier`, `diff`, `rationale`, and `origin`. Because `ExperimentId` is content-derived (base tree plus cumulative diff), the same experiment can legitimately be proposed again in a later generation from a different parent, by a different tier, with a different rationale; the sweep module says as much ("`sample` does not deduplicate: admission (Phase 7) drops a proposal already tried"). If admission ever lets one through, the fold's phase 1 sees the same key with a different event and returns `FoldError::KeyConflict`, which is documented as "a record was rewritten" and is an infrastructure fault that stops the run. Reproduced with a scratch crate: a valid ledger plus `Proposed { exp: E, generation: 1, rationale: r1 }` and `Proposed { exp: E, generation: 2, rationale: r2 }` folds to `Err(KeyConflict)`. The same holds for `Steered`, keyed by `(generation, ordinal)` but not the note blob: two `minos steer` invocations that both read `next_note_ordinal` before either appends produce a `KeyConflict` (reproduced), or, under STORE-03's "append with an existing key is a no-op", one note is silently dropped. Either way the ledger's own invariant ("the same key means the same record") is violated by normal use rather than by tampering, and `Decision::proposed` (a count the loop supplies) can disagree with `proposals_in(generation)`.
**Fix:** Either make the key cover what makes the record distinct, or make the fold treat the case as what it is. For `Proposed`, the cleanest option is to key by `digest(exp)` *and* `u64(generation)`, and have `RunState::validate` reject two proposals of one experiment (a re-proposal is then a `FoldError::DuplicateProposal { exp }` with a clear message, not a "rewritten record"). Alternatively keep the key and add a dedicated error:
```rust
Some(existing) if *existing != event && matches!(event, Event::Proposed { exp, .. }) => {
    return Err(FoldError::DuplicateProposal { exp: *exp });
}
```
For `Steered`, include `digest(note)` in the key so two distinct notes never collide, and let `RunState::notes` return them in `(ordinal, note)` order; the ordinal then orders, and the blob distinguishes. Document in `ledger.rs`'s key table that the loop must check `state.proposal(&exp).is_none()` before appending `Proposed`.

### WR-03: Nothing in the type system distinguishes a composed cumulative diff from a raw proposal before it becomes an `ExperimentId`

**File:** `crates/minos-core/src/experiment.rs:40-46`, `crates/minos-core/src/id.rs:289-291`, `crates/minos-core/src/diff/compose.rs:65-111`
**Issue:** `Experiment::candidate(base, diff)` and `ExperimentId::of(base, diff)` accept any `Diff`. The documentation says `diff` "must be cumulative from `base`: the output of `compose` or `diff_trees`", but a model's parsed proposal is the same type. If the loop (Phase 5) or a test passes the proposal directly for a generation-1 candidate, it gets a *different* `ExperimentId` from the composed form of the same change (diffy re-derives with three context lines, a model usually writes different context), so the "equal trees give equal IDs" property that justifies `compose` silently fails, dedupe at admission misses repeats, and `JobRequest::verify_diff` and `report::analyze` happily agree with the wrong ID because they recompute from the same bytes. For a generation-2 candidate the consequence is worse: a non-cumulative diff hashed under the base tree is an experiment whose diff does not apply to its base, and `minos adopt` would write an unusable patch. The module docs put the burden on the caller; the code rules elsewhere in this crate ("if a value exists, it is valid") put it on the type.
**Fix:** Return a distinct newtype from `compose` and `diff_trees` and require it where cumulativity matters:
```rust
/// A canonical diff cumulative from a run's base tree: only `compose` and `diff_trees` construct it.
pub struct CumulativeDiff(Diff);
impl CumulativeDiff {
    pub fn as_diff(&self) -> &Diff { &self.0 }
}
// compose.rs
pub fn diff_trees(..) -> Result<CumulativeDiff, ComposeError>
// experiment.rs
pub fn candidate(base: TreeId, diff: CumulativeDiff) -> Self
// id.rs
pub fn of(base: &TreeId, diff: &CumulativeDiff) -> Self
```
The report's re-parse (`report.rs:777-781`) would then re-derive through `diff_trees(apply(base, parsed))` or compare canonical bytes, which it already effectively does by hashing.

### WR-04: Judge argv paths need not be excluded from the source snapshot or kept out of `readable`, so the judge program can be read by the candidate and the proposer

**File:** `crates/minos-core/src/manifest.rs:1631-1655`, `crates/minos-core/src/manifest.rs:1792-1808`, `crates/minos-core/src/manifest.rs:1810-1824`
**Issue:** Rule 3 refuses a judge path in `writable`, and rule 4 refuses a surface path under a `[source] exclude` entry, but nothing requires the paths `judge_paths` extracts (for example `judge/score.py` from `["python3", "judge/score.py"]`, or `judge` from `["./judge"]`) to be covered by an exclude entry, and nothing refuses them in `readable`. A project that forgets `exclude = ["judge/"]` ships its judge program in the deterministic tarball, where the candidate sandbox sees it (full source tree, read-only), and a project that lists it in `readable` hands it to the proposer verbatim. The design of record states the judge-integrity invariant as "the code being scored cannot read, edit, or influence the thing that scores it"; this validator enforces only "edit". Note the example manifest in CLAUDE.md has `command = ["./judge"]` with `exclude = ["judge/"]` (the directory, not the file), so even the reference layout leaves the judge binary in the snapshot. Whether judge logic in the tree is acceptable is a design decision, but today it is neither enforced nor documented, and the manifest is the one place that can check it.
**Fix:** Decide and enforce in `Manifest::try_from`. If the judge program must stay out of the tree, add rule 3b: every path in `judge_paths` must be covered by some `SourceExclude` (`excludes.iter().any(|e| e.covers(path))`), returning a new `ManifestError::JudgeNotExcluded { path }`, and extend rule 3's `protected` check to `readable` as well as `writable`. If judge logic may be public, say so in the module docs and in CLAUDE.md's Judge section, and still refuse judge paths in `readable` so the proposer never receives the scorer's source in its prompt.

### WR-05: `is_invisible_format` misses several `Cf` code points, which then render invisibly in reports

**File:** `crates/minos-core/src/text.rs:34-56`
**Issue:** The renderer promises that "every other control character is dropped" and invisible format characters are shown as `<U+XXXX>`, and `report.rs:805` raises `HiddenCharacters` from the same table. The table omits a number of General Category `Cf` (format) characters that are invisible in most terminals and editors: U+180B to U+180D (Mongolian free variation selectors; U+180E is listed but its neighbours are not), U+FFF9 to U+FFFB (interlinear annotation anchor, separator, terminator), U+1D173 to U+1D17A (musical symbol format controls), U+0600 to U+0605, U+06DD, U+070F, U+08E2, U+110BD, U+110CD (Arabic, Syriac, Kaithi number and mark signs), U+13430 to U+1343F (Egyptian hieroglyph format controls), and U+1BCA0 to U+1BCA3 (shorthand format controls). A model that wants to hide text or split a token invisibly in a rationale, a diff line, or a literal can use any of these and the report shows nothing, and no flag fires. The renderer is the last line of defence for the review gate, so a gap here is a gap in the thing the user acknowledges.
**Fix:** Extend the table to the full `Cf` category as of the current Unicode version, and add a unit test that pins the exact set so a future Unicode addition is a deliberate edit:
```rust
| '\u{0600}'..='\u{0605}' | '\u{06DD}' | '\u{070F}' | '\u{08E2}'
| '\u{180B}'..='\u{180F}'
| '\u{FFF9}'..='\u{FFFB}'
| '\u{110BD}' | '\u{110CD}'
| '\u{13430}'..='\u{1343F}'
| '\u{1BCA0}'..='\u{1BCA3}'
| '\u{1D173}'..='\u{1D17A}'
```
`render.rs`'s `invisible_table_edges` should assert the new boundaries, and a property test can check that every `char` with `is_control() == false` that is in a hand-maintained `Cf` list is rendered visibly.

## Info

### IN-01: `SafeText`'s `<U+XXXX>` notation is ambiguous with the same literal ASCII text

**File:** `crates/minos-core/src/text.rs:66-76`, `crates/minos-core/src/text.rs:91-92`
**Issue:** A real U+202E renders as `<U+202E>`, and the eight ASCII bytes `<U+202E>` written by a model render unchanged as `<U+202E>` (reproduced). A reader of the report cannot tell a genuine hidden character from text that merely claims one. The `HiddenCharacters` flag disambiguates for diff lines, but not for rationales or literals.
**Fix:** Escape a literal `<U+` in input (for example render it as `<<U+`), or use a notation that cannot occur in printable input, such as a private marker, and document the choice next to `push_visible`.

### IN-02: `IdHasher::new` documents a no-NUL requirement on `domain` but does not enforce it for external callers

**File:** `crates/minos-core/src/id.rs:47-53`
**Issue:** The builder is `pub` because `minos-sandbox` will hash launch keys through it. A domain containing a NUL byte would make the domain-terminator ambiguous and let two domains collide. Only this crate's own constants are tested (`domains_contain_no_nul`).
**Fix:** `assert!(!domain.as_bytes().contains(&0), "an IdHasher domain contains no NUL")` in `new` (an `assert!` is allowed under the lint set), or take the domain as a `const`-checked newtype.

### IN-03: `judge_paths` treats bare words such as `python3` and `json` as project-relative judge paths

**File:** `crates/minos-core/src/manifest.rs:1641-1649`
**Issue:** Any argv entry that happens to be a valid `RelPath` (which includes a bare `python3`, or the `json` in `--format=json`) is inserted into `JudgeSpec::paths`, participates in the case-fold check, and is reported as a judge path. This is fail-closed, but `JudgeSpec::paths` is documented as "every project-relative path the judge command names", and a later phase that mounts or excludes those paths would act on names that are not files. It also differs from `CommandArgv::program`, which treats a bare first entry as "no path".
**Fix:** Apply the same rule as `program_path` to every entry: an entry with no `/` (after stripping `./`) names no project path. Keep the error for an entry that looks like a path but is not a valid `RelPath`.

### IN-04: `minos.toml` and `minos.lock` can be listed as writable

**File:** `crates/minos-core/src/manifest.rs:1767-1775`
**Issue:** The manifest is parsed from text and does not know its own file name, but the project layout fixes it as `minos.toml` (and `minos.lock` for images). A manifest with `writable = ["minos.toml"]` validates, and an adopted patch could then change the judge command, the budgets, or the excludes for the next run. The run itself is pinned to a `ManifestId`, so this is an adopt-time hazard, not a live one.
**Fix:** Refuse `minos.toml` and `minos.lock` in `writable` (and `readable` for the lockfile) with an `Overlap { path, with: "project.manifest" }`-style error, next to the `program` and Dockerfile checks in rule 3.

### IN-05: The report re-parses an experiment's diff against the current manifest's `writable`, not the one in effect when it was proposed

**File:** `crates/minos-core/src/report.rs:776-781`
**Issue:** `analyze` parses the diff blob with `input.manifest.writable()`. After a `ManifestChanged` that removes a path from `writable`, a report for an earlier, valid winner fails with `ReportError::InvalidDiff`, and so cannot be acknowledged or adopted. `RunState::manifest_at(generation)` exists but the report takes one `Manifest`.
**Fix:** Document that the caller must pass the manifest in effect at `proposal.generation()` (which `RunState::manifest_at` identifies), or have `ReportInput` carry a `&dyn Fn(ManifestId) -> &Manifest` lookup so the report chooses. Either way, add a test that a report survives a later `ManifestChanged`.

### IN-06: `JobResult` accepts token usage for a VM run

**File:** `crates/minos-core/src/job.rs:459-475`
**Issue:** A job is a VM run with no model call, so its `Usage` should carry compute time only; the constructor and `TryFrom<RawJobResult>` accept any `Usage`. Downstream, `Cost::of` with `model: None` returns `TokensWithoutModel` for such a spend and `spend_ok` stops the run. Catching it at the job boundary gives a clearer error and keeps the ledger free of impossible records.
**Fix:** In `TryFrom<RawJobResult>` (and `JobResult::new`), refuse a usage whose `total_tokens()` is non-zero with a new `JobError::TokensInJob`.

---

_Reviewed: 2026-10-14T15:50:23Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
