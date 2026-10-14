---
phase: 3
slug: core-types-and-pure-logic
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-10-10
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

Filled in by the planner from the requirement-to-test map below (03-RESEARCH.md, Validation Architecture). Every task's `<automated>` verify names one of these commands.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | CORE-01 | — | Each ID hash equals a hex literal; composite IDs differ when any field changes; baseline vector (D-04) | golden + proptest | `cargo test -p minos-core --test golden_ids` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-02 | — | `RelPath` rejects `..`, `.`, absolute, empty, `//`, trailing `/`, `\`, NUL, over-length; serde round-trip | proptest + unit | `cargo test -p minos-core --test prop_path` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-03 | — | Each manifest rule rejects its violation (unknown field, overlap including a judge argument path, budget range, policy/objective mismatch, no eligible tier, D-14 bounds); image tags are rejected by `ImageRef` and `image::base_images` on Dockerfile `FROM` lines, not by `Manifest::try_from` (amended 2026-10-14) | unit with TOML fixtures | `cargo test -p minos-core --test manifest` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-04 | — | Every diffy leniency is a rejection; create/delete/no-op rejected (D-02, D-03); `parse(serialize(d)) == d` | unit + proptest | `cargo test -p minos-core --test prop_diff` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-05 | — | `apply(base, d) == modified`; any context change fails with `ContextMismatch` | proptest + unit | `cargo test -p minos-core --test prop_apply` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-06 | — | NaN and infinities rejected; `-0.0` becomes `0.0`; JSON round-trip bit-exact | proptest | `cargo test -p minos-core --test prop_metric` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-07 | — | Every `Outcome` variant serializes and parses; no error type carries an outcome | unit + golden | `cargo test -p minos-core --test golden_wire` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-08 | — | Every event's `EventKey` documented and distinct across a generated ledger | proptest + `cargo doc` | `cargo test -p minos-core --test prop_keys` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-09 | — | Golden `Record::V1` line per variant (insta + SHA-256 literal); fold permutation-invariant; corrupt ledgers return `FoldError` | golden + proptest | `cargo test -p minos-core --test golden_wire --test prop_fold` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-10 | — | `select_greedy` permutation-invariant over experiments and trials; honors `min_improvement`; ties on `ExperimentId` | proptest | `cargo test -p minos-core --test prop_policy` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-11 | — | Route climbs one tier per `stall_step`, clamps, resets after improvement; reason per ineligible tier | unit + proptest | `cargo test -p minos-core --test route` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-12 | — | Fixed seed gives literal samples; `below(n)` in range; identical batches for identical inputs | golden + proptest | `cargo test -p minos-core --test sweep` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-13 | — | `JobRequest`/`JobResult` golden JSON; unknown fields rejected | golden + unit | `cargo test -p minos-core --test golden_wire` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-14 | — | `Rate::parse` rejects negative, exponent, 13 fractional digits, overflow; exact AWS rate products; round-half-up micro conversion | unit + proptest | `cargo test -p minos-core --test money` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-15 | — | Escape corpus renders inert; bidi and zero-width shown as `<U+XXXX>`; `\n`/`\t` kept | golden | `cargo test -p minos-core --test render` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-16 | — | Same inputs give identical report bytes and hash under shuffled ledger order; each of the five flags fires on its fixture only; weak-isolation path golden (D-18) | golden + proptest | `cargo test -p minos-core --test report` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | CORE-17 | — | Breaker stops after exactly `max_empty_generations` empty generations and `max_infra_failures` infra failures | unit + proptest | `cargo test -p minos-core --test breaker` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | COST-01 | — | `Spent` has no dollar field (golden); a numeric rate in `prices.toml` is a parse error with a location | golden + unit | `cargo test -p minos-core --test money` | ❌ W0 | ⬜ pending |
| TBD | last | last | GATE-06 | — | `cargo clean` after verification passes | manual | `cargo clean` | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Deliberate-violation evidence (the Phase 1 and 2 standard): for at least `Diff::parse` (a rename), `Manifest::try_from` (an overlap), `Metric::new` (NaN), `Rate::parse` (13 fractional digits), and the fold (a duplicate `RunStarted`), verification records the actual failing output.

---

## Wave 0 Requirements

- [ ] `Cargo.toml` — `serde_json` gains `features = ["float_roundtrip"]` before any metric round-trip test or golden file
- [ ] `.config/insta.yaml` — `behavior: update: no`, so the plain `cargo test --workspace` gate never writes snapshots
- [ ] `crates/minos-core/tests/fixtures/` — TOML manifests, diff texts, escape corpus, loaded by `include_str!`
- [ ] `crates/minos-core/tests/common/mod.rs` — `TestResult`, `sha256_hex`, builders that go through public constructors only (no `unwrap`, `expect`, or indexing)
- [ ] `crates/minos-core/proptest-regressions/` — created by proptest on first failure; commit it when it appears
- [ ] No framework install needed

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo clean` after verification | GATE-06 | A build-tree action, not a test | Run `cargo clean` after the full gate passes and before the phase is marked complete |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
