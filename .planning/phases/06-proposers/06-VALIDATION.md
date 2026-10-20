---
phase: 6
slug: proposers
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-10-19
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Derived from 06-RESEARCH.md "Validation Architecture".

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (toolchain 1.97.1), `insta` 1.49.0 (`.config/insta.yaml` `update: no`), `proptest` 1.11.0 |
| **Config file** | `.config/insta.yaml`; snapshots in `crates/minos-propose/tests/snapshots/` (Wave 0 creates) |
| **Quick run command** | `cargo test -p minos-propose` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps`, plus the C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` (must print nothing) |
| **Live (manual, logged-in machine)** | `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` |
| **Estimated runtime** | quick ~30 s; full gate ~5 min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p minos-propose` (and `cargo test -p minos-core` for the ledger task)
- **After every plan wave:** Run the full suite command
- **Before `/gsd-verify-work`:** Full suite must be green, the C-02 grep empty, and the ignored live tests run once by hand with results recorded; then `cargo clean` (GATE-06)
- **Max feedback latency:** 300 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by the planner from PLAN.md tasks) | | | PROP-01 to PROP-10 | | | | | | ⬜ pending |

Requirement to test mapping (from research):

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| PROP-01 | Single-shot runs unchanged over `FakeProvider` and `ClaudeCli` (generic); fake returns raw JSON only | unit/integration | `cargo test -p minos-propose --test single_shot` | ❌ W0 |
| PROP-02 | Argv equals golden (insta + SHA-256 pin); env exactly six keys; cwd fresh, empty, 0700, removed after; prompt on stdin byte-exact; deadline kills the process group and reports Infra | golden + fake-binary integration | `cargo test -p minos-propose --test argv_golden --test fake_claude` | ❌ W0 |
| PROP-03 | Hostile hooks, `.mcp.json`, and CLAUDE.md never run; positive control fires | ignored live | `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored` | ❌ W0 |
| PROP-04 | Preflight refuses parent credential vars, wrong layout, PATH shim, version < 2.1.296, version/file-name mismatch, `authMethod` ≠ `claude.ai`, `apiProvider` ≠ `firstParty`, `apiKeySource` present, exit 1, bad JSON | fake-binary integration + unit | `cargo test -p minos-propose --test preflight` | ❌ W0 |
| PROP-05 | Prompt golden (insta + pin); shuffled history, excerpts, notes give identical bytes; prefix stable across suffix changes | golden + proptest | `cargo test -p minos-propose --test prompt_golden --test prop_prompt` | ❌ W0 |
| PROP-06 | Held-out non-interference (byte-identical prompts under changed held-out values); caps hold; untrusted fences cannot be closed by content | proptest + unit | `cargo test -p minos-propose --test held_out` | ❌ W0 |
| PROP-07 | Schema golden + round trip; each drop rule; zero survivors = empty call; drop counts | unit + proptest | `cargo test -p minos-propose --test single_shot` | ❌ W0 |
| PROP-08 | Usage extracted from every variant; no type binds a cost field | unit (classifier table) | `cargo test -p minos-propose --test classify` | ❌ W0 |
| PROP-09 | 429 → `ProviderLimit`; 401/403 → `ProviderAuth`; prefix fallbacks; max-turns → Empty; else Infra; `StopReason::ProviderAuth` golden line | unit + golden | `cargo test -p minos-propose --test classify && cargo test -p minos-core --test golden_wire --test prop_keys` | ❌ W0 / ✅ extend |
| PROP-10 | Sweep proposals from params with no provider; same `(run, generation)` gives the same diffs; cumulative from base after a non-base parent | unit | `cargo test -p minos-propose --test sweep` | ❌ W0 |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/minos-propose/Cargo.toml` — add `rustix` and `tempfile` (deps); `proptest` and `tempfile` (dev-deps) as workspace pins require
- [ ] `crates/minos-propose/tests/support/` (shared module, every item used by each target that declares it) — fake-claude layout builder in a temp `HOME`, canned envelopes for each classifier row
- [ ] `crates/minos-propose/tests/snapshots/` — argv, schema, prompt goldens (accepted deliberately with `INSTA_UPDATE=always`, then pinned by SHA-256)
- [ ] `crates/minos-core/tests/snapshots/golden_wire__record_v1_stop_requested_provider_auth.snap` plus its pin

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real `claude -p` structured call through the production provider; hostile fixture canaries absent; positive control canaries present | PROP-02, PROP-03 | Needs the user's subscription login and a live model call; the gate makes no model call | On a logged-in machine: `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1`; record the result in 06-EVIDENCE.md |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
