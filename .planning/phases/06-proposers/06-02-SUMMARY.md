---
phase: 06-proposers
plan: 02
subsystem: propose
tags: [model-provider, json-schema, strict-parsing, drop-reasons, limits, clippy, golden, proptest]
status: complete

requires:
  - phase: 03-core
    provides: diff_trees, Tree, Diff, RelPath, Limits, ModelId, PromptHash, Usage, BATCH_MIN/BATCH_MAX
  - phase: 06-proposers
    plan: 01
    provides: StopReason::ProviderAuth (mapped from ProviderStop::Auth in 06-04)
provides:
  - "minos_propose::limits: 14 propose-only caps with units, reasons, and const assertions"
  - "minos_propose::schema: OutputSchema::for_batch (byte-stable, golden plus SHA-256 pin), SchemaError"
  - "minos_propose::provider: ModelProvider trait, ModelRequest, ModelResponse, Completion, EmptyCause, ModelOutput, ProviderStop, InfraCause (19 variants), ProviderError"
  - "minos_propose::prompt: Prompt value (prefix then suffix, PromptHash, MAX_PROMPT_BYTES cap), PromptError"
  - "minos_propose::proposal: Proposal, DropReason (16 variants), DropCounts, Parsed, parse_output, crate-private finish"
  - "06-EVIDENCE.md with the HashMap/HashSet clippy ban proof"
affects: [06-03 prompt assembly, 06-04 ClaudeCli classifier, 06-05 tiers, 06-06, 06-07, 07 loop]

tech-stack:
  added: []
  patterns:
    - "Hostile model JSON wrapped in a derive-less newtype whose only exit is a crate-private into_value consumed by one strict parser"
    - "Compile-time not-implemented check via ambiguous blanket impls (static_assertions trick) in an integration test"
    - "JSON Schema built from explicit serde_json::Map inserts in sorted key order, so the text is identical with or without preserve_order"
    - "One shared finish(base, parent, candidate) helper maps diff_trees errors to SameAsParent, BackToBase, Compose"

key-files:
  created:
    - crates/minos-propose/clippy.toml
    - crates/minos-propose/src/limits.rs
    - crates/minos-propose/src/schema.rs
    - crates/minos-propose/src/provider.rs
    - crates/minos-propose/src/prompt.rs
    - crates/minos-propose/src/proposal.rs
    - crates/minos-propose/tests/schema.rs
    - crates/minos-propose/tests/provider.rs
    - crates/minos-propose/tests/parse.rs
    - crates/minos-propose/tests/snapshots/schema__output_schema_batch_4.snap
    - .planning/phases/06-proposers/06-EVIDENCE.md
  modified:
    - crates/minos-propose/Cargo.toml
    - crates/minos-propose/src/lib.rs
    - Cargo.lock

key-decisions:
  - "Rationale caps are checked in bytes (MAX_RATIONALE_BYTES = 2000), stricter than the schema's character maxLength of the same number"
  - "Per-file rules run file by file in plan order (PathInvalid, NotWritable, DuplicatePath, NotInParent, ForbiddenByte, FileTooLarge); the first failing rule decides the item's one drop"
  - "An over-batch output counts every item as an OutputShape drop and parses none of them"
  - "ProviderStop and EmptyCause stay exhaustive (closed sets matched downstream); InfraCause, ProviderError, SchemaError, PromptError are #[non_exhaustive]"

patterns-established:
  - "Schema pin procedure: accept the snapshot with INSTA_UPDATE=always, review, hash the body with awk plus shasum (body plus one newline)"
  - "Over-cap diff tests use the hunk cap (many small separated edits) rather than the line cap, since diffy's Myers diff on a fully rewritten 40,000-line file does not finish in a debug build"

requirements-completed: [PROP-01, PROP-07]

coverage:
  - id: D1
    description: "Propose-only caps in limits.rs with reasons; const assertions on MAX_PROMPT_BYTES"
    requirement: PROP-07
    verification:
      - kind: build
        ref: "crates/minos-propose/src/limits.rs (const assertions compile)"
        status: pass
    human_judgment: false
  - id: D2
    description: "HashMap/HashSet banned by clippy.toml, proven to fire"
    requirement: PROP-07
    verification:
      - kind: other
        ref: ".planning/phases/06-proposers/06-EVIDENCE.md#clippy-bans-fire-06-02"
        status: pass
    human_judgment: false
  - id: D3
    description: "OutputSchema::for_batch golden snapshot, SHA-256 pin, structure, determinism, refusals"
    requirement: PROP-07
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/schema.rs#output_schema_golden"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/schema.rs#output_schema_bytes_pinned"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/schema.rs#output_schema_structure"
        status: pass
    human_judgment: false
  - id: D4
    description: "ModelProvider abstraction; ModelOutput has no Debug, Display, Clone, Deref (compile-time check)"
    requirement: PROP-01
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/provider.rs#model_output_has_no_debug_display_clone_or_deref"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/provider.rs#resolved_model_is_the_single_entry"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/provider.rs#provider_error_separates_stops_from_infrastructure"
        status: pass
    human_judgment: false
  - id: D5
    description: "Strict parser: one test per DropReason (except Sweep), round trip, proptest on arbitrary JSON"
    requirement: PROP-07
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/parse.rs#two_items_yield_two_cumulative_proposals"
        status: pass
      - kind: unit
        ref: "crates/minos-propose/tests/parse.rs#schema_example_payload_round_trips"
        status: pass
      - kind: property
        ref: "crates/minos-propose/tests/parse.rs#arbitrary_json_never_panics_and_is_accounted"
        status: pass
      - kind: property
        ref: "crates/minos-propose/tests/parse.rs#arbitrary_items_never_panic_and_are_accounted"
        status: pass
    human_judgment: false

duration: 14min
completed: 2026-10-19
---

# Phase 6 Plan 02: minos-propose contracts Summary

**The `ModelProvider` abstraction with blame-separated errors, a byte-pinned JSON output schema, the `Prompt` value, every propose-only cap, and one strict parser that turns hostile `{rationale, files}` model output into canonical cumulative-diff proposals, dropping and counting every invalid item by a closed `DropReason`.**

## Performance

- **Duration:** about 14 min
- **Started:** 2026-10-19T21:08Z (local -0400)
- **Completed:** 2026-10-19
- **Tasks:** 3
- **Files modified:** 14 (11 created)

## Accomplishments

- `minos-propose` gains only existing workspace pins (rustix, tempfile; dev: proptest, tempfile, sha2, base16ct), links only `minos-core` internally, and `cargo deny check` passes with no `deny.toml` change.
- `clippy.toml` bans `HashMap` and `HashSet`; both bans are proven to fire in 06-EVIDENCE.md.
- `OutputSchema::for_batch` gives compact, sorted-key JSON Schema text: `proposals` 1..=batch, items `{rationale, files}` with a path `enum` of the writable files and `additionalProperties` false at every object level; golden snapshot plus SHA-256 pin `bd6ccb83…fd83`.
- `ModelProvider::complete(&self, &ModelRequest) -> Result<ModelResponse, ProviderError>`; `ProviderError` is `Stop { ProviderStop, usage }` or `Infra { InfraCause, usage }`, with fixed text and numbers only.
- `ModelOutput` has no derives; a compile-time ambiguity check fails the build if it ever implements Debug, Display, Clone, or Deref. Its only exit, `pub(crate) into_value`, is consumed by `parse_output`.
- `parse_output` parses through three `deny_unknown_fields` raw types (no `flatten`), applies every rule in a fixed order, builds the candidate from the parent tree plus returned files, and derives the cumulative diff through `finish` and `diff_trees`. Survivors keep output order; within-batch duplicates are counted.

## Task Commits

1. **Task 1: Crate dependencies, clippy policy with proof, crate docs, and limits.rs** - `eb07cd9` (feat)
2. **Task 2: Output schema, provider abstraction, and the Prompt value** - `d27a593` (test, RED), `5ae2e57` (feat, GREEN)
3. **Task 3: Strict output parser with closed drop reasons** - `e2305cf` (test, RED), `e7fa995` (feat, GREEN)

## Files Created/Modified

- `crates/minos-propose/Cargo.toml`, `Cargo.lock` - the new dependency edges
- `crates/minos-propose/clippy.toml` - HashMap/HashSet ban
- `crates/minos-propose/src/lib.rs` - trust-boundary docs; `limits`, `prompt`, `proposal`, `provider`, `schema` modules
- `crates/minos-propose/src/limits.rs` - 14 caps with reasons and three const assertions
- `crates/minos-propose/src/schema.rs` - `OutputSchema`, `SchemaError`
- `crates/minos-propose/src/provider.rs` - the provider trait and its request, response, and error types
- `crates/minos-propose/src/prompt.rs` - `Prompt`, `PromptError` (assembly arrives in 06-03)
- `crates/minos-propose/src/proposal.rs` - `Proposal`, `DropReason`, `DropCounts`, `Parsed`, `parse_output`, `finish`
- `crates/minos-propose/tests/{schema,provider,parse}.rs` - 8, 11, and 27 tests
- `crates/minos-propose/tests/snapshots/schema__output_schema_batch_4.snap` - the reviewed schema golden
- `.planning/phases/06-proposers/06-EVIDENCE.md` - Phase 6 evidence, starting with the clippy ban proof

## Decisions Made

- The rationale cap is checked in bytes, which is stricter than the schema's `maxLength` (characters) of the same number.
- Per-file rules run file by file; the first failing rule over the whole item decides its one drop.
- An over-batch output drops every item as `OutputShape` and parses none.
- Schema keys are inserted in sorted order, so the text stays the same even if a later dependency turned on serde_json's `preserve_order` through feature unification (the pin would catch any change).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The Compose test uses the hunk cap, not a line-cap overflow**
- **Found during:** Task 3
- **Issue:** The first Compose test rewrote all 40,000 lines of a file to go over the 65,536-line diff cap. diffy's Myers diff on a full rewrite costs O(N*D), with N and D both around 80,000, so the debug-build test did not finish in 120 s.
- **Fix:** The test makes 300 small edits ten lines apart (300 hunks, over `MAX_HUNKS_PER_FILE` 256) and adds a control at 150 hunks that is accepted. The test still checks a diff over core's caps, `DropReason::Compose`, and it runs in milliseconds.
- **Files modified:** crates/minos-propose/tests/parse.rs
- **Commit:** e7fa995

**2. [Rule 1 - Bug] `?Sized` on the Clone blanket impl in the not-implemented check**
- **Found during:** Task 3 (workspace clippy)
- **Issue:** `Clone` requires `Sized`, so `impl<T: ?Sized + Clone>` fails clippy (`?Sized` ignored). It compiled under `cargo test` in the Task 2 commit.
- **Fix:** The Clone impl is now `impl<T: Clone>`. The check behaves the same.
- **Files modified:** crates/minos-propose/tests/provider.rs
- **Commit:** e7fa995

**3. [Note] Intermediate dead-code warning between the Task 2 and Task 3 commits**
- `ModelOutput::into_value` had no consumer until the parser landed, so `cargo clippy -D warnings` failed on commit `5ae2e57` alone. Task 2's own check (`cargo test`) passed. No `allow` was added. `e7fa995` clears it, and every gate is green from there.

Separately, the plan's step 2 put the clippy probe in `lib.rs` and named one type. The probe proved both `HashMap` and `HashSet`, so every `clippy.toml` entry is shown to fire.

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug), plus 1 note.
**Impact on plan:** No change in scope. Every invariant was kept: `ModelOutput` stays derive-less and every validated type has private fields.

## Issues Encountered

None beyond the deviations above.

## Known Stubs

- `crates/minos-propose/src/prompt.rs`: only the `Prompt` value exists. Deterministic assembly is planned for 06-03, as this plan specifies.

## User Setup Required

None.

## Next Phase Readiness

- 06-03 (prompt assembly) builds `Prompt::new(prefix, suffix)` and uses the caps in `limits.rs`.
- 06-04 maps `ProviderStop` to `StopReason::{ProviderLimit, ProviderAuth}` and classifies envelopes into `InfraCause`.
- 06-05's tiers call `parse_output` and `finish`, and use `DropReason::Sweep` for render failures.

## Verification

- `cargo test -p minos-propose`: schema 8, provider 11, parse 27, all passing
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo deny check`, `cargo doc -p minos-propose --no-deps` (no warnings): all pass
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing

## Self-Check: PASSED
