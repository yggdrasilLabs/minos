---
phase: 3
slug: core-types-and-pure-logic
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-10
completed: 2026-10-14
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Built-in `cargo test` + `proptest` 1.11.0 + `insta` 1.49.0 |
| **Config file** | `.config/insta.yaml` (Wave 0 adds it, `behavior: update: no`); `crates/minos-core/clippy.toml` (exists) |
| **Quick run command** | `cargo test -p minos-core` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps`, then the C-01 and C-02 checks (`grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing) |
| **Estimated runtime** | ~60 seconds (quick); several minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p minos-core` plus `cargo clippy -p minos-core --all-targets -- -D warnings`
- **After every plan wave:** Run the full five-command gate plus the C-01 and C-02 checks
- **Before `/gsd-verify-work`:** Full suite must be green, then `cargo clean` (GATE-06)
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

Filled in from the plans 03-01 to 03-13 and their SUMMARY files. Task IDs are `<plan>-T<task>`. Every command runs from the repository root; the full gate in 03-EVIDENCE.md ran all of them together through `cargo test --workspace`.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-T2, 03-02-T2, 03-05-T2, 03-07-T2 | 03-01, 03-02, 03-05, 03-07 | 1, 2, 2, 3 | CORE-01 | T-03-01-03, T-03-01-04, T-03-02-05, T-03-07-04 | Each ID hash equals a hex literal computed with shasum outside the code; composite IDs differ when any field changes; baseline vector `5b161f65...6f86` (D-04); `ExperimentId::of`, `JudgeVersion`, and event-key vectors pinned | golden + proptest | `cargo test -p minos-core --test golden_ids` (also `--test prop_diff`, `--test manifest`, `--test prop_keys` for the later vectors) | ✅ `tests/golden_ids.rs` | ✅ green |
| 03-01-T3 | 03-01 | 1 | CORE-02 | T-03-01-01, T-03-01-02 | `RelPath` rejects `..`, `.`, absolute, empty, `//`, trailing `/`, `\`, NUL, over-length; bytewise order; serde round-trip | proptest + unit | `cargo test -p minos-core --test prop_path` | ✅ `tests/prop_path.rs` | ✅ green |
| 03-05-T1, 03-05-T2 | 03-05 | 2 | CORE-03 | T-03-05-01 to T-03-05-05, T-03-05-07 | Each manifest rule rejects its violation (unknown field in every table, overlap including a judge argument path, budget range, policy/objective mismatch, no eligible tier, D-14 bounds); image tags are rejected by `ImageRef` and `image::base_images` on Dockerfile `FROM` lines, not by `Manifest::try_from` (amended 2026-10-14) | unit with inline TOML fixtures | `cargo test -p minos-core --test manifest` | ✅ `tests/manifest.rs` | ✅ green |
| 03-02-T1, 03-02-T2 | 03-02 | 2 | CORE-04 | T-03-02-01 to T-03-02-07 | Every diffy leniency is a rejection; rename, copy, mode, binary, create/delete, and no-op rejected (D-02, D-03); `parse(serialize(d)) == d` | unit + proptest | `cargo test -p minos-core --test prop_diff` | ✅ `tests/prop_diff.rs` | ✅ green |
| 03-06-T1, 03-06-T2 | 03-06 | 3 | CORE-05 | T-03-06-01 to T-03-06-06 | `apply(base, d) == modified`; any context change fails with `ContextMismatch`; no offset search; end-of-file rule is `OutOfRange`; equal trees compose to equal IDs | proptest + unit | `cargo test -p minos-core --test prop_apply` | ✅ `tests/prop_apply.rs` | ✅ green |
| 03-03-T1 | 03-03 | 2 | CORE-06 | T-03-03-01 to T-03-03-04 | NaN and infinities rejected; `-0.0` becomes `0.0`; JSON round-trip bit-exact; duplicate scorecard names rejected | proptest | `cargo test -p minos-core --test prop_metric` | ✅ `tests/prop_metric.rs` | ✅ green |
| 03-03-T2, 03-09-T2 | 03-03, 03-09 | 2, 4 | CORE-07 | T-03-03-05, T-03-03-06, T-03-09-02 | Every `Outcome` variant serializes and parses; no error type carries an outcome | unit + golden | `cargo test -p minos-core --test golden_wire` (round trips also in `--test prop_metric`) | ✅ `tests/golden_wire.rs`, `tests/prop_metric.rs` | ✅ green |
| 03-07-T1, 03-07-T2 | 03-07 | 3 | CORE-08 | T-03-07-01 to T-03-07-07 | Every event's `EventKey` documented and distinct across a generated ledger; six golden key vectors | proptest + `cargo doc` | `cargo test -p minos-core --test prop_keys` | ✅ `tests/prop_keys.rs` | ✅ green |
| 03-09-T2, 03-10-T1, 03-10-T2 | 03-09, 03-10 | 4 | CORE-09 | T-03-09-02, T-03-09-03, T-03-10-01 to T-03-10-06 | Golden `Record::V1` line per variant (insta + SHA-256 literal); fold permutation-invariant and idempotent under duplicate replay; corrupt ledgers return `FoldError` | golden + proptest | `cargo test -p minos-core --test golden_wire --test prop_fold` | ✅ `tests/golden_wire.rs`, `tests/prop_fold.rs` | ✅ green |
| 03-11-T1 | 03-11 | 5 | CORE-10 | T-03-11-01 to T-03-11-03, T-03-11-06 | `select_greedy` permutation-invariant over experiments and trials; lower median per metric; honors `min_improvement` by multiply-and-compare; ties on `ExperimentId` | proptest | `cargo test -p minos-core --test prop_policy` | ✅ `tests/prop_policy.rs` | ✅ green |
| 03-05-T1, 03-11-T2 | 03-05, 03-11 | 2, 5 | CORE-11 | T-03-11-01 | Route climbs one tier per `stall_step`, clamps, resets after improvement; reason per ineligible tier | unit + proptest | `cargo test -p minos-core --test route` | ✅ `tests/route.rs` | ✅ green |
| 03-08-T1, 03-08-T2 | 03-08 | 4 | CORE-12 | T-03-08-01 to T-03-08-05 | ChaCha8 TC1 known answer; fixed seed gives literal samples; `below(n)` in range; identical batches for identical inputs; render edits only writable anchored lines | golden + proptest | `cargo test -p minos-core --test sweep` | ✅ `tests/sweep.rs` | ✅ green |
| 03-09-T1, 03-09-T2 | 03-09 | 4 | CORE-13 | T-03-09-01, T-03-09-04 | `JobRequest`/`JobResult` golden JSON; unknown fields rejected; diff blob must address the canonical diff | golden + unit | `cargo test -p minos-core --test golden_wire` | ✅ `tests/golden_wire.rs` | ✅ green |
| 03-04-T1, 03-04-T2 | 03-04 | 2 | CORE-14 | T-03-04-03 to T-03-04-05 | `Rate::parse` rejects negative, exponent, 13 fractional digits, overflow; exact AWS rate products; round-half-up micro conversion | unit + proptest | `cargo test -p minos-core --test money` | ✅ `tests/money.rs` | ✅ green |
| 03-04-T3 | 03-04 | 2 | CORE-15 | T-03-04-01, T-03-04-02, T-03-04-07 | Escape corpus renders inert; bidi and zero-width shown as `<U+XXXX>`; `\n`/`\t` kept | golden | `cargo test -p minos-core --test render` | ✅ `tests/render.rs` | ✅ green |
| 03-12-T1, 03-12-T2 | 03-12 | 6 | CORE-16 | T-03-12-02 to T-03-12-07 | Same inputs give identical report bytes and hash under shuffled ledger order; each flag (the five v1 flags plus hidden characters and weak isolation) fires on its fixture only; weak-isolation path golden (D-18) | golden + proptest | `cargo test -p minos-core --test report` | ✅ `tests/report.rs` | ✅ green |
| 03-11-T3 | 03-11 | 5 | CORE-17 | T-03-11-04, T-03-11-05 | Breaker stops after exactly `max_empty_generations` empty generations and `max_infra_failures` infra failures | unit + proptest | `cargo test -p minos-core --test breaker` | ✅ `tests/breaker.rs` | ✅ green |
| 03-04-T1, 03-04-T2, 03-07-T1, 03-09-T2 | 03-04, 03-07, 03-09 | 2, 3, 4 | COST-01 | T-03-04-06, T-03-07-06, T-03-09-05 | `Spent` has no dollar field (golden); a numeric rate in `prices.toml` is a parse error with a location | golden + unit | `cargo test -p minos-core --test money` and `cargo test -p minos-core --test golden_wire spent_carries_no_dollar_field` | ✅ `tests/money.rs`, `tests/golden_wire.rs` | ✅ green |
| 03-13-T3 | 03-13 | 7 | GATE-06 | T-03-13-01, T-03-13-02 | `cargo clean` after the full gate passes, recorded in 03-EVIDENCE.md and 03-13-SUMMARY.md | manual | `cargo clean` | n/a | ⬜ pending (flipped to green in the 03-13 docs commit, after `cargo clean` runs) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Deliberate-violation evidence (the Phase 1 and 2 standard): for `Diff::parse` (a rename), `Manifest::try_from` (an overlap), `Metric::new` (NaN), `Rate::parse` (13 fractional digits), and the fold (a duplicate `RunStarted`), 03-EVIDENCE.md records the exact rejection each test asserts and the test run.

---

## Wave 0 Requirements

- [x] `Cargo.toml` — `serde_json` gains `features = ["float_roundtrip"]` before any metric round-trip test or golden file (03-01, 338d7f9)
- [x] `.config/insta.yaml` — `behavior: update: no`, so the plain `cargo test --workspace` gate never writes snapshots (03-01, 338d7f9)
- [x] Fixtures — inline string constants in each test file instead of `crates/minos-core/tests/fixtures/` (see Deviations)
- [x] Test helpers — each test file defines its own `TestResult` and builders through public constructors only, instead of `crates/minos-core/tests/common/mod.rs` (see Deviations)
- [x] proptest regressions — proptest writes them beside the test file (`tests/prop_fold.proptest-regressions`, `tests/prop_policy.proptest-regressions`), and both are committed
- [x] No framework install needed

### Deviations

- No `tests/common` module. A helper module shared across integration test binaries trips `dead_code` under `-D warnings` in every binary that does not use every helper, and no allow attribute may silence it (C-02), so each test file defines its own `TestResult` and helpers.
- Fixtures are inline string constants (TOML manifests, diff texts, the escape corpus) rather than files in a `tests/fixtures/` directory loaded by `include_str!`.
- proptest regression files live beside their test files, not in a `proptest-regressions/` directory, because that is where proptest writes them for integration tests.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo clean` after verification | GATE-06 | A build-tree action, not a test | Run `cargo clean` after the full gate passes and before the phase is marked complete |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-14 (03-13, after the full gate in 03-EVIDENCE.md)
