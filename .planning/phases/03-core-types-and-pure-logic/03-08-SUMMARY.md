---
phase: 03-core-types-and-pure-logic
plan: 08
subsystem: minos-core
status: complete
tags: [core, sweep, chacha8, rand_core, rejection-sampling, diff, golden-vectors, proptest]
requires:
  - "03-01: IdHasher, RunId, Limits"
  - "03-02: Diff, Diff::parse, DiffError::NotWritable"
  - "03-05: SweepParam, ParamDomain, IntRange (points, value_at, min, max, step), ChoiceList (len, get, as_slice), ParamName, Manifest::parse_toml"
  - "03-06: Tree, apply, diff_trees, ComposeError"
provides:
  - "sweep.rs: seed, private below, ParamValue (Int, Choice, render_text), Sample (get, iter, describe), sample, render, SweepError (NoParams, Domain, Unassigned, Undeclared, AnchorMissing, AnchorAmbiguous, AnchorConflict, FileMissing, NotText, NoChange, Compose)"
  - "tests/sweep.rs: chacha8_known_answer (TC1, 8 rounds), seed_golden_vector, sweep_golden_samples, identical_inputs_identical_batches, every_value_is_in_its_domain, render tests"
affects: [03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "The only thing taken from the RNG crate is the raw ChaCha8 stream through Rng::next_u64; range mapping is our own rejection sampler, so samples are value-stable across rand versions"
    - "Three independent pins under the sweep: the cipher stream (published TC1 vector), the seed (shasum), and the samples (reproduced by a separate Python ChaCha8)"
    - "Sweep anchors are matched against the base text, never against an earlier param's edit, so render does not depend on edit order"
key-files:
  created:
    - crates/minos-core/tests/sweep.rs
  modified:
    - crates/minos-core/src/sweep.rs
decisions:
  - "render refuses a sample whose names are not exactly the params (Undeclared, Unassigned) and a value outside its param's domain (Domain), so a sample drawn from other params cannot edit a file"
  - "Anchors match base lines only: an earlier param's new value cannot make a later anchor match a line the base does not have"
  - "A file with carriage returns renders but fails diff validation (Compose(Invalid)), because Diff::parse rejects \\r anywhere; render does not special-case CRLF"
metrics:
  duration: "5 min"
  completed: 2026-10-14
  tasks: 2
  files: 2
---

# Phase 3 Plan 08: Seeded sweep sampler and renderer Summary

The sweep tier's proposer is a ChaCha8 stream seeded by `IdHasher("minos/sweep/v1")` over the run and generation, mapped to param indices by our own unbiased rejection sampler. A sample is rendered into a canonical cumulative `Diff` by editing each param's anchored line and passing the result through `diff_trees`.

## What was built

- **`seed(run, generation)`** hashes in the domain `minos/sweep/v1`: the run digest, then the generation widened to `u64`. That 32-byte result seeds one `ChaCha8Rng` per generation.
- **`below(rng, n)`** is private. It returns `None` for `n == 0`. Otherwise it rejects draws below `n.wrapping_neg() % n` and returns `x % n`. It uses only `next_u64`, with no `rand` distribution or range helper.
- **`sample(run, generation, params, batch)`** returns `NoParams` when there are no params. Otherwise it fills each batch item by drawing every param in declaration order. Integer params map an index through `IntRange::value_at`, and choice params through `ChoiceList::get`. Duplicates are kept; admission drops them later.
- **`ParamValue`** has two variants, `Int(i64)` and `Choice(String)`, and `render_text` prints either one. **`Sample`** keeps a private `BTreeMap` and offers `get`, `iter`, and `describe`. `describe` produces `name=value, ...` in name order, which becomes the rationale text in Phase 6.
- **`render(base, params, sample, writable, limits)`** takes three steps:
  - It checks that the sample's names are exactly the declared params and that every value is in its param's domain.
  - For each param in declaration order, the file must be in the base and must be UTF-8. Exactly one base line must start with the anchor, and no earlier param may have claimed that line. Everything after the anchor, up to the line's newline, becomes the value's text. The newline, or its absence on a last line, is kept.
  - The edited tree goes through `diff_trees`. `ComposeError::NoChange` maps to `SweepError::NoChange`, and every other compose error becomes `SweepError::Compose`.
- **`SweepError`** is `#[non_exhaustive]` and derives `Clone` and `PartialEq`. Every variant means "drop the sample", never an infrastructure error. Variants carry validated names and paths, never file content.

## Verification

- `chacha8_known_answer`: a fresh `ChaCha8Rng::from_seed([0; 32])` returns the published TC1 stream for the 8-round, 256-bit-key case (draft-strombergson-chacha-test-vectors-01). The four `next_u64` values are `0xd6405f892fef003e ...`, and the eight `next_u32` values are `0x2fef003e, 0xd6405f89, ...`. It passed on the first run, so the literals were never touched.
- `seed_golden_vector`: the seed for the `golden_ids.rs` run at generation 1 equals `5ccbb1c0...70cf`, computed with `printf | xxd | shasum`, independently of the code.
- `sweep_golden_samples`: the result is `["a=60, b=z", "a=20, b=x", "a=90, b=x", "a=70, b=z"]`. These values were recorded from the first run and then reproduced by a separate Python ChaCha8 and rejection sampler written from the specification. That script also reproduced TC1.
- Properties: `identical_inputs_identical_batches`, `every_value_is_in_its_domain`, and the unit tests `below_is_under_n` and `below_is_under_n_near_the_top`.
- Render: the replaced line is checked by `apply`. Other tests cover a kept missing final newline, a two-file cumulative diff, deterministic bytes, sampled values landing on their lines, and every error variant. A non-writable file gives `Compose(Invalid(NotWritable))`, and a CRLF file gives `Compose(Invalid)`.
- Gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps` all pass. The C-01 check prints no MISSING line and the C-02 grep prints nothing. The prohibition greps for `RngCore`, `gen_range|random_range|Uniform|distributions`, and `f32|f64` in `sweep.rs` print nothing.

## TDD Gate Compliance

- Task 1: RED `6e24b9a` (test) then GREEN `320c72e` (feat).
- Task 2: RED `1ecfd43` (test) then GREEN `17f7273` (feat).
- No refactor commits were needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing validation] render checks that the sample matches the params**
- **Found during:** Task 2
- **Issue:** The plan's `render` signature takes `params` and `sample` separately, and the plan's error list had nothing for a sample drawn from different params. Such a sample could miss a declared param, assign an undeclared name, or carry a value outside the param's domain. For example, an `Int` value could go into a choice param, or a value could be off the step grid.
- **Fix:** Added `SweepError::Unassigned { name }` and `SweepError::Undeclared { name }`. `render` now rejects an out-of-domain value with the existing `SweepError::Domain { name }`, whose message was broadened to "a value outside its domain". Added two tests.
- **Files modified:** crates/minos-core/src/sweep.rs, crates/minos-core/tests/sweep.rs
- **Commit:** 17f7273

**2. [Rule 2 - Correctness] Anchors match the base, not earlier edits**
- **Found during:** Task 2
- **Issue:** If anchors were matched against the edited text, an earlier param's new value could make a later anchor match a line that does not exist in the base, and the result would depend on edit order.
- **Fix:** `FileEdit` keeps the base lines and records replacements by index. Added the test `render_matches_anchors_against_the_base_not_earlier_edits`.
- **Commit:** 17f7273

Additions beyond the plan's test list: `seed_golden_vector` (an independent shasum pin of the seed), a `next_u32` check inside `chacha8_known_answer` (it separates word order from stream identity), and a Python cross-check of the golden samples.

## Deferred Issues

None. `cargo deny check` prints `unused-wrapper` warnings for the `minos-core` ban wrappers, because those crates do not depend on core yet. The warnings predate this plan and do not fail the gate.

## Known Stubs

None.

## Self-Check: PASSED

Files: sweep.rs, tests/sweep.rs, and this SUMMARY exist. Commits 6e24b9a, 320c72e, 1ecfd43, 17f7273 exist.
