---
phase: 03-core-types-and-pure-logic
plan: 01
subsystem: minos-core
status: complete
tags: [core, ids, hashing, sha256, relpath, limits, serde, insta]
requires: []
provides:
  - "minos-core module skeleton (19 public modules, each with a //! contract doc)"
  - "limits.rs: every Phase 3 cap and default, plus Limits::standard"
  - "id.rs: IdHasher, digest_id! macro, plain and composite SHA-256 IDs, MetricName, ModelId"
  - "path.rs: RelPath and PathError"
  - "serde_json float_roundtrip workspace-wide; .config/insta.yaml update: no"
affects: [03-02, 03-03, 03-04, 03-05, 03-06, 03-07, 03-08, 03-09, 03-10, 03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "digest_id! macro: private [u8; 32], lowercase hex Display/serde, strict 64-char FromStr, module-private from_digest"
    - "IdHasher: NUL-terminated domain, fixed 32-byte digests, u64 big-endian integers, u64 length-prefixed bytes"
    - "name_type! macro (private): validated String names through one checker, serde try_from/into String"
    - "Byte-level path grammar walk; never std::path::Path::components"
    - "Golden vectors: shasum/xxd recipe in a comment above each hex literal, plain assert_eq!"
key-files:
  created:
    - .config/insta.yaml
    - crates/minos-core/src/limits.rs
    - crates/minos-core/src/id.rs
    - crates/minos-core/src/path.rs
    - crates/minos-core/src/{text,metric,outcome,usage,money,image,manifest,experiment,ledger,state,policy,route,sweep,job,report}.rs
    - crates/minos-core/src/diff/{mod,apply,compose}.rs
    - crates/minos-core/tests/golden_ids.rs
    - crates/minos-core/tests/prop_path.rs
  modified:
    - Cargo.toml
    - crates/minos-core/src/lib.rs
decisions:
  - "Limits private fields drop the max_ prefix (pedantic struct_field_names); the public accessors keep the planned max_* names"
  - "lib.rs separates pub mod lines with blank lines so rustfmt's reorder_modules keeps the planned module order"
  - "IdHasher::new takes the domain without the NUL and writes the NUL itself; the plan's domain constants carry no NUL and a unit test asserts it"
  - "Name checks run Empty, TooLong, BadStart, BadByte in that order; RelPath checks run Empty, TooLong, Absolute, TrailingSlash, ForbiddenByte, then per-component TooDeep, EmptyComponent, DotComponent, ParentComponent, LeadingDash, ComponentTooLong"
metrics:
  duration: "6 min"
  completed: 2026-10-14
  tasks: 3
  files: 26
---

# Phase 3 Plan 01: Core foundation, IDs, and RelPath Summary

SHA-256 ID layer for `minos-core` (one `IdHasher` for every composite ID, a `digest_id!` macro for strict lowercase hex newtypes, and golden vectors computed with shasum, including the D-04 baseline `5b161f65...6f86`), a byte-level ASCII `RelPath`, `limits.rs` with every Phase 3 cap, the 19-module skeleton, serde_json `float_roundtrip`, and an insta config that cannot auto-accept.

## What was built

- **Workspace switches (Wave 0).** `serde_json` is pinned with `features = ["float_roundtrip"]`, and `cargo tree` confirms the feature is resolved. Cargo.lock did not change. `.config/insta.yaml` sets `behavior.update: no`, so a plain `cargo test --workspace` never writes or accepts a snapshot.
- **Module skeleton.** `lib.rs` declares, in order: `limits, id, path, text, metric, outcome, usage, money, image, manifest, diff, experiment, ledger, state, policy, route, sweep, job, report`. `diff/mod.rs` declares `apply` and `compose`. Every skeleton file holds one `//!` paragraph stating its contract and the plan that fills it. No other plan needs to edit `lib.rs` until 03-13 adds the crate-root re-exports.
- **limits.rs.** Every constant named in the plan, with its unit and reason in its doc comment. This includes D-12 (`NOISE_REFUSE_MULTIPLE` 1.0, `NOISE_WARN_MULTIPLE` 2.0), D-14 (stall-step, empty-generation, and infra-failure min/max/default), and D-16 (`ANOMALY_MULTIPLE` 10.0). `Limits` has private fields, `const fn standard()`, `Default`, and seven `#[must_use] const fn` accessors, with no setter.
- **id.rs (CORE-01).**
  - `IdHasher` provides `new(domain)`, `digest`, `u64`, `bytes`, and `finish`.
  - The `digest_id!` macro is exported crate-wide with `pub(crate) use`, for JudgeVersion, EventKey, SpendId, and ReviewId in later plans. Expansions call the crate-visible `decode_hex_digest` and `write_hex_digest`.
  - `IdParseError::Malformed` carries no input.
  - Plain digests: `BlobId`, `TreeId` (with `to_blob_id`), `ManifestId`, `PricesId`, `ImageDigest`.
  - Composite IDs: `ExperimentId::baseline`, built through a private `hash(base, canonical)` helper that 03-02 reuses for `ExperimentId::of`. `RunId::of(&ManifestId, &TreeId, started_at)`, `PromptHash::of`, `ReportHash::of`.
  - Validated names: `MetricName` and `ModelId`, with `NameError` (`Empty`, `TooLong { max }`, `BadByte { index }`, `BadStart`).
- **path.rs (CORE-02, D-01).**
  - `RelPath` has one private validator behind `TryFrom<String>`, `TryFrom<&str>`, `FromStr`, and serde `try_from`.
  - It provides `as_str`, `components`, `file_name`, `depth`, `is_within` (respects component boundaries, so `judge` does not cover `judgement`), `ascii_case_key`, and `Display`.
  - `PathError` has 11 variants. Each carries an index or a limit, never the input text.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok (exit 0) |
| `cargo test --workspace` | exit 0; golden_ids 16 passed, prop_path 11 passed, id unit test 1 passed |
| `cargo deny check` | exit 0: advisories ok, bans ok, licenses ok, sources ok (one existing `unmatched wrapper` warning for minos-sandbox, not from this plan) |
| `cargo doc --workspace --no-deps` | ok |
| C-01 per-crate `[lints] workspace = true` | ok for all 10 crates, no MISSING |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true |
| `git diff --stat HEAD -- crates/minos-core/clippy.toml` | prints nothing |

Golden vectors, all computed with `/usr/bin/shasum -a 256` and `xxd`, with the recipe beside each literal:

- `BlobId` of empty input: `e3b0c442...b855`
- `BlobId` of `abc`: `ba7816bf...15ad`
- `ExperimentId::baseline(TreeId::of(b""))`: `5b161f655168a2b16645e03b77d61b0bfba4935d1be19c6293ec41a5998b6f86` (D-04)
- `ExperimentId::baseline(TreeId::of(b"t"))`: `6fd8be38...75cb`
- `RunId` of manifest `m`, tree `t`, and start time 1760000000: `6a65b71b...56cb`
- `PromptHash` of `prompt`: `c91644f8...cf9b`
- `ReportHash` of `report`: `b3c17682...8487`
- `IdHasher` builder vector: `014b1dd0...476e`

## TDD Gate Compliance

Tasks 2 and 3 followed RED then GREEN. Each RED commit failed to compile against an unimplemented API, and each GREEN commit made every test pass. No refactor commits were needed.

- Task 2: RED `fd702a4` (test), then GREEN `c0cf675` (feat)
- Task 3: RED `5df3074` (test), then GREEN `26155ac` (feat)

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 | 338d7f9 | feat(03-01): add core module skeleton, limits, and workspace switches |
| 2 RED | fd702a4 | test(03-01): add failing golden vectors for core IDs |
| 2 GREEN | c0cf675 | feat(03-01): implement IdHasher, digest IDs, and validated names |
| 3 RED | 5df3074 | test(03-01): add failing RelPath grammar and property tests |
| 3 GREEN | 26155ac | feat(03-01): implement RelPath with a byte-level ASCII grammar |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Limits private field names**
- **Found during:** Task 1
- **Issue:** Clippy's pedantic `struct_field_names` lint fails under `-D warnings` when every private field of `Limits` carries the `max_` prefix. No `allow` attribute is permitted (C-02).
- **Fix:** Renamed the private fields to `diff_bytes`, `diff_lines`, and so on. The public accessors keep the planned names (`max_diff_bytes()` and the rest), so the API is unchanged.
- **Files modified:** crates/minos-core/src/limits.rs
- **Commit:** 338d7f9

**2. [Rule 1 - Acceptance criterion] Test doc wording**
- **Found during:** Task 2
- **Issue:** The `golden_ids.rs` module doc said "never insta snapshots", which failed the `grep -c 'insta'` = 0 criterion.
- **Fix:** Reworded it to "never snapshot assertions".
- **Files modified:** crates/minos-core/tests/golden_ids.rs
- **Commit:** c0cf675

No other deviations. `clippy.toml` was not touched. No dependency or lockfile change was needed beyond the serde_json feature.

## Known Stubs

These stubs are intentional and required by the plan. Each skeleton module holds only its `//!` contract paragraph, and the named plan fills it:

| File | Filled by |
|---|---|
| text.rs | 03-04 |
| metric.rs | 03-03 |
| outcome.rs | 03-03 |
| usage.rs | 03-04 |
| money.rs | 03-04 |
| image.rs | 03-05 |
| manifest.rs | 03-05 |
| diff/mod.rs | 03-02 |
| diff/apply.rs | 03-06 |
| diff/compose.rs | 03-06 |
| experiment.rs | 03-06 |
| ledger.rs | 03-07 |
| state.rs | 03-10 |
| policy.rs | 03-11 |
| route.rs | 03-11 |
| sweep.rs | 03-08 |
| job.rs | 03-09 |
| report.rs | 03-12 |

They do not block this plan's goal, which is the skeleton itself.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-01-01 | Byte-level grammar, with every rejection unit-tested and property-tested | prop_path.rs |
| T-03-01-02 | Bytewise Eq and Ord. Case and Cyrillic-homoglyph tests. `ascii_case_key` | prop_path.rs |
| T-03-01-03 | One `IdHasher` (NUL domain, fixed digests, length prefixes), golden vectors computed outside the code | id.rs, golden_ids.rs |
| T-03-01-04 | Length and charset checked before decode. Uppercase, 63/65 chars, and non-hex rejected. JSON numbers rejected. No input in the error | id.rs, golden_ids.rs |
| T-03-01-05 | `RunId::of` takes `started_at` as an argument; the core clippy.toml clock ban is untouched | id.rs |
| T-03-01-06 | `float_roundtrip` enabled and resolved | Cargo.toml |
| T-03-01-07 | `.config/insta.yaml` sets `update: no` | .config/insta.yaml |

No new threat surface beyond the register.

## Self-Check: PASSED

- All 26 created or modified files exist.
- Commits 338d7f9, fd702a4, c0cf675, 5df3074, and 26155ac are in `git log`.
