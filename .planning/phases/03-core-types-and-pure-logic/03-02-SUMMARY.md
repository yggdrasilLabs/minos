---
phase: 03-core-types-and-pure-logic
plan: 02
subsystem: minos-core
status: complete
tags: [core, diff, diffy, canonical-form, experiment-id, sha256, proptest]
requires:
  - "03-01: RelPath, Limits, IdHasher, and the private ExperimentId::hash helper"
provides:
  - "diff/mod.rs: Diff, FileDiff, Hunk, HunkLine, DiffError, ForbiddenHeader, and Diff::parse, a seven-stage pipeline"
  - "The canonical diff form (Diff::as_str and as_bytes), re-parse-stable under the same caps"
  - "id.rs: ExperimentId::of(&TreeId, &Diff) over the canonical bytes, with a golden vector"
  - "CLAUDE.md: Diff paragraph rewritten for D-01 to D-04 and the canonical form"
affects: [03-06, 03-08, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Our line grammar runs before diffy, and line accounting runs after it, so nothing diffy skips can hide in the text"
    - "Stage functions are shared by parse and the round-trip re-read (read(), never Diff::parse), so there is no recursion"
    - "Hand-written Debug on every diff type prints paths, counts, and lengths, never content"
key-files:
  created:
    - crates/minos-core/tests/prop_diff.rs
  modified:
    - crates/minos-core/src/diff/mod.rs
    - crates/minos-core/src/id.rs
    - CLAUDE.md
decisions:
  - "HunkLine::text returns the line without its trailing newline, and has_newline reports whether it had one. The stored String keeps the newline, so the raw content is the pair"
  - "A no-newline marker closes its side for the rest of the section: no further line of that side and no further hunk may follow, because only a file's last line can lack a newline"
  - "Stage 7 re-runs stages 1 to 5 on the canonical text, not only 3 to 5. Stage 1 keeps every Diff re-parseable under the same caps: a canonical form over the byte cap is TooLarge. Stage 2 supplies the per-section shape that stage 5 needs"
  - "File and hunk caps are checked in the stage 2 grammar as sections and hunks arrive, so the work stays bounded. TooManyFiles and TooManyHunks carry the cap"
  - "Stage 4 checks where diffy disagrees with the grammar (binary, a mode, a non-Modify operation, a different section or hunk count) return Malformed, because no line number is available there"
metrics:
  duration: "9 min"
  completed: 2026-10-14
  tasks: 2
  files: 4
---

# Phase 3 Plan 02: Diff::parse and the canonical diff form Summary

`Diff::parse(text, writable, limits)` runs seven stages: byte caps, our own counted line grammar, diffy's gitdiff parse, per-file semantic checks, line accounting, canonical serialization, and a canonical re-read. A diff that touches a non-writable path (bytewise, D-01), creates, deletes, renames, copies, changes a mode, carries a symlink or gitlink mode, carries a binary patch (D-02), or changes nothing (D-03) cannot become a `Diff`. `ExperimentId::of` hashes only the canonical bytes, and its golden vector `b58d3c2e...f7e6` was computed with shasum.

## What was built

- **Stage 1 (bytes).** Checks, in order: `Empty`, `TooLarge`, `ForbiddenByte` (NUL or CR, with the 1-based line), `MissingFinalNewline`, `TooManyLines`.
- **Stage 2 (grammar).** Each section must be exactly:
  - `diff --git a/P b/P`;
  - an optional `index <7-64 lowercase hex>..<7-64 lowercase hex>[ 100644|100755]` line, where mode `120000` is `Symlink` and `160000` is `Gitlink`;
  - `--- a/P`, then `+++ b/P`.

  The hunk header counts decide how many body lines follow, so a deleted line `-- a/...` is read as body, not as a file header. Hunk numbers must be ASCII digits with no leading zero, fit in `u32`, and a non-empty side cannot start at 0. Function context after `@@ ` is accepted and dropped. Twelve extended-header prefixes map to `ForbiddenHeader`. Any other unexpected line is `UnexpectedLine`: prose, quoted paths, timestamps, trailing text, or a line beyond the header counts. A truncated hunk is `Malformed`.
- **Stages 3 and 4.** `PatchSet::parse(text, ParseOptions::gitdiff())` runs, and any error is `Malformed` with no diffy text. Its sections must match the grammar's sections in count, path, and hunk count, and each must be an in-place `Modify` of `a/P` to `b/P` with no modes and no binary. Then come the writable set (exact `RelPath` equality), `DuplicateFile`, `NoOpHunk` (old-side and new-side sequences compared including newline presence), and `InconsistentNewStart`. That last check uses 0-based positions: `start - 1`, or `start` for an empty side, plus the running inserted-minus-deleted delta.
- **Stage 5.** The text's line count must equal the sum, over sections, of 3, plus 1 if there is an `index` line, plus, for each hunk, its header, its lines, and one marker per line lacking its newline.
- **Stages 6 and 7.** The serializer writes sections sorted by path, the three canonical header lines, `@@ -a,b +c,d @@` with both lengths, body lines prefixed with ` `/`-`/`+`, and the marker after a line lacking its newline. A blank context line is written as `" \n"`. Then `read()` (stages 1 to 5) runs on the canonical text and must give back the same files, or the error is `CanonicalRoundTrip`. A sample canonical output with a blank context line and a no-newline marker passed both `git apply --check` and `patch -p1 --dry-run`.
- **Accessors.**
  - `Diff`: `parse`, `files`, `paths`, `as_str`, `as_bytes`, `len`, and `is_empty` (always false).
  - `FileDiff`: `path`, `hunks`.
  - `Hunk`: `old_start`, `old_len`, `new_start`, `new_len`, `lines`.
  - `HunkLine`: `text`, `has_newline`.
- **`ExperimentId::of`** calls the existing private `hash(base, diff.as_bytes())`. Its doc records that the diff is cumulative from the run's base tree (the D-04 amendment).

### DiffError listing (Pitfall 10 review)

`Empty`; `TooLarge { bytes: usize }`; `TooManyLines { lines: usize }`; `ForbiddenByte { line: usize, byte: u8 }`; `MissingFinalNewline`; `UnexpectedLine { line: usize }`; `ForbiddenHeader { line: usize, header: ForbiddenHeader }`; `DevNull { line: usize }`; `PathMismatch { line: usize }`; `InvalidPath { line: usize, error: PathError }`; `Malformed`; `NotWritable { path: RelPath }`; `DuplicateFile { path: RelPath }`; `TooManyFiles { max: usize }`; `NoHunks { path: RelPath }`; `TooManyHunks { path: RelPath, max: usize }`; `NoOpHunk { path: RelPath, hunk: usize }`; `InconsistentNewStart { path: RelPath, hunk: usize }`; `CanonicalRoundTrip`; `LineAccounting { expected: usize, found: usize }`.

No variant has a `String` or `&str` field. `awk '/^pub enum DiffError/,/^}/' ... | grep -E 'String|&str'` prints nothing. The test `errors_carry_no_input_text` asserts that an ESC-laden hostile line reaches neither Display nor Debug. The rename message asserted for plan 03-13 is `line 2: forbidden header: rename`.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok |
| `cargo test --workspace` | exit 0; prop_diff 67 passed (64 tests plus 3 proptests) |
| `PROPTEST_CASES=20000` on the 3 proptests | passed |
| `cargo deny check` | advisories, bans, licenses, and sources all ok (only the existing `unmatched wrapper` warning for minos-sandbox) |
| `cargo doc --workspace --no-deps` | ok, no warnings |
| C-01 `[lints] workspace = true` | no MISSING line |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true |
| `grep -rn 'diffy::apply' crates/minos-core/src` | prints nothing |

Golden vector: for `ExperimentId::of(TreeId::of(b""), parse(VALID))`, the canonical text is 136 bytes and the ID is `b58d3c2e7829fc7a79daf3e292980c236a225083c2e051d1210ba78f067ff7e6`. The shasum and xxd recipe sits in a comment beside the literal.

## TDD Gate Compliance

Both tasks followed RED then GREEN. Each RED commit failed to compile against the missing API.

- Task 1: RED `a610a13` (test), GREEN `2d0f218` (feat)
- Task 2: RED `33f7483` (test), GREEN `2493823` (feat)

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | a610a13 | test(03-02): add failing Diff::parse rejection and acceptance tests |
| 1 GREEN | 2d0f218 | feat(03-02): implement Diff::parse stages 1-5 with typed rejections |
| 2 RED | 33f7483 | test(03-02): add failing canonical form and ExperimentId::of tests |
| 2 GREEN | 2493823 | feat(03-02): canonical diff form, round-trip check, and ExperimentId::of |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Accessors moved from Task 1 to Task 2, and `is_empty` added**
- **Found during:** Task 1
- **Issue:** `as_str`, `as_bytes`, and `len` need the canonical string, which Task 2 builds. Adding them in Task 1 would have meant a stub, or a serializer that made Task 2's RED tests pass before any Task 2 code existed. Separately, Clippy's `len_without_is_empty` fires on a public `len`.
- **Fix:** The three accessors and the `canonical` field landed with the Task 2 serializer. `Diff::is_empty` was added; it is documented as always false (D-03, D-04).
- **Commit:** 2493823

**2. [Rule 3 - Blocking] Lint-driven rewrites**
- **Found during:** Tasks 1 and 2
- **Issue:** Pedantic `naive_bytecount` flagged byte-filter counting, and `format_push_string` flagged `push_str(&format!(..))`. No `allow` is permitted.
- **Fix:** Lines are counted with `str::matches('\n')`, and the serializer pushes string pieces and `u32::to_string`.
- **Commits:** 2d0f218, 2493823

**3. [Rule 2 - Correctness] Stricter grammar than listed**
- **Found during:** Task 1
- **Issue:** The plan's grammar would accept a no-newline marker followed by more lines of the same side, or by another hunk. Such a text cannot describe a real file, and appliers might disagree about it.
- **Fix:** A marker closes its side for the rest of the section. Three tests cover this: `rejects_old_side_line_after_its_no_newline_marker`, `rejects_hunk_after_end_of_file_marker`, and `rejects_misplaced_no_newline_marker`. Hunk numbers with leading zeros, numbers over ten digits, and a non-empty side starting at 0 are also `UnexpectedLine`.
- **Commit:** 2d0f218

**4. [Plan clarification] Where some rejections fire**
- **Hunk count mismatch.** diffy would report this as "hunk header does not match hunk", but the counted grammar catches it first. A truncated hunk is `Malformed` (`rejects_truncated_hunk`), and a surplus line is `UnexpectedLine` (`rejects_body_line_beyond_header_counts`). `Malformed` from diffy itself is shown by `rejects_hunks_out_of_order`.
- **Zero-hunk sections.** The grammar reports these as `NoHunks { path }`.
- **Stage 7.** It re-runs stages 1 to 5 rather than 3 to 5, for the reason in the decisions list.

No dependency, lockfile, `clippy.toml`, or `deny.toml` change.

## Known Stubs

None in this plan's code. `diff/apply.rs` and `diff/compose.rs` remain the 03-01 skeletons, which plan 03-06 fills.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-02-01 | Exact `RelPath` membership. The same `P` byte for byte on all three header lines | `rejects_case_variant_of_writable_path`, `rejects_path_not_in_writable`, `rejects_diff_git_path_disagreeing_with_file_headers` |
| T-03-02-02 | Stage 2 grammar, stage 4 section cross-check, stage 5 accounting, stage 7 re-read | prose, extra-line, zero-hunk, duplicate, quoted-path, and timestamp tests; `canonical_round_trip` |
| T-03-02-03 | Forbidden headers, index-line modes, `/dev/null`, and stage 4 binary, mode, and operation checks | one named test each |
| T-03-02-04 | Byte and line caps before parsing. File and hunk caps while the grammar runs | `rejects_text_over_*`, `rejects_too_many_*` |
| T-03-02-05 | `ExperimentId::of` hashes only canonical bytes | `experiment_id_of_golden_vector`, `texts_with_the_same_canonical_form_share_an_experiment_id` |
| T-03-02-06 | No text in errors. Hand-written Debug | `errors_carry_no_input_text`, `debug_output_omits_content` |
| T-03-02-07 | D-03 no-op rejection | `rejects_no_op_hunk`, `rejects_context_only_hunk`, `rejects_zero_hunk_section` |

No new threat surface beyond the register.

## Self-Check: PASSED

- crates/minos-core/src/diff/mod.rs, crates/minos-core/src/id.rs, crates/minos-core/tests/prop_diff.rs, and CLAUDE.md exist and are committed.
- Commits a610a13, 2d0f218, 33f7483, and 2493823 are in `git log`.
