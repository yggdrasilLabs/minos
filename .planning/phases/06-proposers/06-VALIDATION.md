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
| 06-01-01 | 01 | 1 | PROP-09 | T-06-01-02 | `StopReason::ProviderAuth` has tag 7, fixed Display, distinct keys | unit + proptest | `cargo test -p minos-core --test prop_keys` | ✅ extend | ⬜ pending |
| 06-01-02 | 01 | 1 | PROP-09 | T-06-01-01 | One new golden line; no existing wire byte moves; pin recomputed | golden | `cargo test -p minos-core --test golden_wire` | ✅ extend | ⬜ pending |
| 06-02-01 | 02 | 1 | PROP-01 | T-06-02-04, T-06-02-SC | Only existing pins added; HashMap/HashSet ban proven to fire; caps const-asserted | lint + supply chain | `cargo clippy -p minos-propose --all-targets -- -D warnings && cargo deny check` | ❌ W0 | ⬜ pending |
| 06-02-02 | 02 | 1 | PROP-01, PROP-07 | T-06-02-02, T-06-02-04 | Schema golden + pin; `ModelOutput` has no Debug/Display/Clone/Deref (compile-time check) | golden + unit | `cargo test -p minos-propose --test schema --test provider` | ❌ W0 | ⬜ pending |
| 06-02-03 | 02 | 1 | PROP-07 | T-06-02-01, T-06-02-03, T-06-02-05 | Strict per-item parse; every drop rule counted; arbitrary JSON never panics | unit + proptest | `cargo test -p minos-propose --test parse && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-03-01 | 03 | 2 | PROP-06 | T-06-03-01, T-06-03-02 | History rows sanitized, capped, visible-objective deltas only | unit | `cargo test -p minos-propose --test history` | ❌ W0 | ⬜ pending |
| 06-03-02 | 03 | 2 | PROP-05, PROP-06 | T-06-03-01, T-06-03-03, T-06-03-04 | Golden + pin; shuffle invariance; stable prefix; unforgeable fences; typed refusals | golden + proptest | `cargo test -p minos-propose --test prompt` | ❌ W0 | ⬜ pending |
| 06-03-03 | 03 | 2 | PROP-06 | T-06-03-02 | Held-out values cannot change a prompt byte; names and values absent | proptest | `cargo test -p minos-propose --test held_out && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-04-01 | 04 | 2 | PROP-02, PROP-04 | T-06-04-01, T-06-04-02, T-06-04-05 | Argv golden + pin; exactly six child variables; credential names scanned, values dropped | golden + unit | `cargo test -p minos-propose --test argv_golden` | ❌ W0 | ⬜ pending |
| 06-04-02 | 04 | 2 | PROP-08, PROP-09 | T-06-04-03, T-06-04-04 | Pattern 7 table; usage from every variant; dollar fields never bound; stop mapping | unit (table) | `cargo test -p minos-propose --test classify` | ❌ W0 | ⬜ pending |
| 06-04-03 | 04 | 2 | PROP-04 | T-06-04-06 | Only the native install is located; every refusal names the paths checked | unit (temp layouts) | `cargo test -p minos-propose --test locate && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-05-01 | 05 | 3 | PROP-10 | T-06-05-05 | Sweep proposes from params with no provider; deterministic; cumulative from base | unit | `cargo test -p minos-propose --test sweep` | ❌ W0 | ⬜ pending |
| 06-05-02 | 05 | 3 | PROP-01, PROP-07, PROP-08, PROP-09 | T-06-05-01, T-06-05-02, T-06-05-03, T-06-05-04 | One call; strict parse; empty call is Ok; usage, origin, typed stops | unit (FakeProvider) | `cargo test -p minos-propose --test single_shot` | ❌ W0 | ⬜ pending |
| 06-05-03 | 05 | 3 | PROP-01, PROP-07, PROP-10 | T-06-05-02 | Generic dispatch; sweep makes no request; PROP-07 reworded (D-11) | unit + doc grep | `cargo test -p minos-propose --test proposer && cargo clippy -p minos-propose --all-targets -- -D warnings && grep -n 'PROP-07.*{rationale, files}' .planning/REQUIREMENTS.md` | ❌ W0 | ⬜ pending |
| 06-06-01 | 06 | 3 | PROP-02 | T-06-06-01, T-06-06-02 | Deadline + process-group kill; caps; exact env; cwd; stdin | unit (real processes) | `cargo test -p minos-propose --lib claude_cli::process` | ❌ W0 | ⬜ pending |
| 06-06-02 | 06 | 3 | PROP-02, PROP-04 | T-06-06-03, T-06-06-04, T-06-06-05, T-06-06-06 | Preflight refusals; auth status binds no PII; fresh empty dir per run | fake-binary integration | `cargo test -p minos-propose --test claude_cli && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-07-01 | 07 | 4 | PROP-02, PROP-04, PROP-09 | T-06-07-01, T-06-07-02, T-06-07-04 | Call path: re-resolve, re-preflight on change, golden argv, six vars, stdin, dir removed | fake-binary integration | `cargo test -p minos-propose --test claude_cli` | ❌ W0 | ⬜ pending |
| 06-07-02 | 07 | 4 | PROP-01, PROP-08 | T-06-07-03 | Same tier code over FakeProvider and ClaudeCli; origin prompt hash matches stdin | fake-binary integration | `cargo test -p minos-propose --test claude_cli && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-07-03 | 07 | 4 | PROP-02, PROP-04, PROP-09 | T-06-07-05 | Design of record matches the provider as built | doc grep | `grep -n 'api_error_status' CLAUDE.md && grep -n 'apiKeySource' CLAUDE.md && grep -n 'Phase 6 corrections to the stack block' .claude/CLAUDE.md && grep -n 'Proposer<P>' CLAUDE.md` | ✅ | ⬜ pending |
| 06-08-01 | 08 | 5 | PROP-02, PROP-03 | T-06-08-02, T-06-08-03 | Live suite ignored and gated; positive control present | compile + ignored | `cargo test -p minos-propose --test live 2>&1 \| grep -E '3 ignored' && cargo clippy -p minos-propose --all-targets -- -D warnings` | ❌ W0 | ⬜ pending |
| 06-08-02 | 08 | 5 | PROP-03 | — | CLAUDE.md documents the live suite and its gate variable | doc grep | `grep -n 'MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live' CLAUDE.md` | ✅ | ⬜ pending |
| 06-09-01 | 09 | 6 | PROP-02, PROP-03 | T-06-09-01, T-06-09-02 | Live run: no canary in (a)/(b), hook canary in (c); recorded and redacted | manual live | `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` | ❌ W0 | ⬜ pending |
| 06-09-02 | 09 | 6 | PROP-01 to PROP-10 | T-06-09-01, T-06-09-03, T-06-09-04 | Full gate, C-01, C-02 green on final code | full gate | full suite command above | ✅ | ⬜ pending |
| 06-09-03 | 09 | 6 | GATE-06 | — | cargo clean last | command | `test ! -d target/debug` | ✅ | ⬜ pending |

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
- [ ] Self-contained test targets in place of a shared `tests/support/` module (every item of a shared module must be used by each target that declares it): the fake-claude HOME layout lives in `tests/claude_cli.rs` (06-06, 06-07), the canned envelopes in `tests/classify.rs` (06-04)
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
