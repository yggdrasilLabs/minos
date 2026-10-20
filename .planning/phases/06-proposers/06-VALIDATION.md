---
phase: 6
slug: proposers
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-19
completed: 2026-10-20
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
- **Before `/gsd-verify-work`:** Full suite must be green, the C-02 grep empty, and the ignored live tests run once by the user's hand (06-09's checkpoint) with results recorded; then `cargo clean` (GATE-06, 06-10)
- **Max feedback latency:** 300 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | PROP-09 | T-06-01-02 | `StopReason::ProviderAuth` has tag 7, fixed Display, distinct keys | unit + proptest | `cargo test -p minos-core --test prop_keys` | ✅ | ✅ green |
| 06-01-02 | 01 | 1 | PROP-09 | T-06-01-01 | One new golden line; no existing wire byte moves; pin recomputed | golden | `cargo test -p minos-core --test golden_wire` | ✅ | ✅ green |
| 06-02-01 | 02 | 1 | PROP-01 | T-06-02-04, T-06-02-SC | Only existing pins added; HashMap/HashSet ban proven to fire; caps const-asserted | lint + supply chain | `cargo clippy -p minos-propose --all-targets -- -D warnings && cargo deny check` | ✅ | ✅ green |
| 06-02-02 | 02 | 1 | PROP-01, PROP-07 | T-06-02-02, T-06-02-04 | Schema golden + pin; `ModelOutput` has no Debug/Display/Clone/Deref (compile-time check) | golden + unit | `cargo test -p minos-propose --test schema --test provider` | ✅ | ✅ green |
| 06-02-03 | 02 | 1 | PROP-07 | T-06-02-01, T-06-02-03, T-06-02-05 | Strict per-item parse; every drop rule counted; arbitrary JSON never panics | unit + proptest | `cargo test -p minos-propose --test parse && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-03-01 | 03 | 2 | PROP-06 | T-06-03-01, T-06-03-02 | History rows sanitized, capped, visible-objective deltas only | unit | `cargo test -p minos-propose --test history` | ✅ | ✅ green |
| 06-03-02 | 03 | 2 | PROP-05, PROP-06 | T-06-03-01, T-06-03-03, T-06-03-04 | Golden + pin; shuffle invariance; stable prefix; unforgeable fences; typed refusals | golden + proptest | `cargo test -p minos-propose --test prompt` | ✅ | ✅ green |
| 06-03-03 | 03 | 2 | PROP-06 | T-06-03-02 | Held-out values cannot change a prompt byte; names and values absent | proptest | `cargo test -p minos-propose --test held_out && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-04-01 | 04 | 2 | PROP-02, PROP-04 | T-06-04-01, T-06-04-02, T-06-04-05 | Argv golden + pin; exactly six child variables; credential names scanned, values dropped | golden + unit | `cargo test -p minos-propose --test argv_golden` | ✅ | ✅ green |
| 06-04-02 | 04 | 2 | PROP-08, PROP-09 | T-06-04-03, T-06-04-04 | Pattern 7 table; usage from every variant; dollar fields never bound; stop mapping | unit (table) | `cargo test -p minos-propose --test classify` | ✅ | ✅ green |
| 06-04-03 | 04 | 2 | PROP-04 | T-06-04-06 | Only the native install is located; every refusal names the paths checked | unit (temp layouts) | `cargo test -p minos-propose --test locate && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-05-01 | 05 | 3 | PROP-10 | T-06-05-05 | Sweep proposes from params with no provider; deterministic; cumulative from base | unit | `cargo test -p minos-propose --test sweep` | ✅ | ✅ green |
| 06-05-02 | 05 | 3 | PROP-01, PROP-07, PROP-08, PROP-09 | T-06-05-01, T-06-05-02, T-06-05-03, T-06-05-04 | One call; strict parse; empty call is Ok; usage, origin, typed stops | unit (FakeProvider) | `cargo test -p minos-propose --test single_shot` | ✅ | ✅ green |
| 06-05-03 | 05 | 3 | PROP-01, PROP-07, PROP-10 | T-06-05-02 | Generic dispatch; sweep makes no request; PROP-07 reworded (D-11) | unit + doc grep | `cargo test -p minos-propose --test proposer && cargo clippy -p minos-propose --all-targets -- -D warnings && grep -n 'PROP-07.*{rationale, files}' .planning/REQUIREMENTS.md` | ✅ | ✅ green |
| 06-06-01 | 06 | 3 | PROP-02 | T-06-06-01, T-06-06-02 | Deadline + process-group kill; caps; exact env; cwd; stdin | unit (real processes) | `cargo test -p minos-propose --lib claude_cli::process` | ✅ | ✅ green |
| 06-06-02 | 06 | 3 | PROP-02, PROP-04 | T-06-06-03, T-06-06-04, T-06-06-05, T-06-06-06 | Preflight refusals; auth status binds no PII; fresh empty dir per run | fake-binary integration | `cargo test -p minos-propose --test claude_cli && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-07-01 | 07 | 4 | PROP-02, PROP-04, PROP-09 | T-06-07-01, T-06-07-02, T-06-07-04 | Call path: re-resolve, re-preflight on change, golden argv, six vars, stdin, dir removed | fake-binary integration | `cargo test -p minos-propose --test claude_cli` | ✅ | ✅ green |
| 06-07-02 | 07 | 4 | PROP-01, PROP-08 | T-06-07-03 | Same tier code over FakeProvider and ClaudeCli; origin prompt hash matches stdin | fake-binary integration | `cargo test -p minos-propose --test claude_cli && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-07-03 | 07 | 4 | PROP-02, PROP-04, PROP-09 | T-06-07-05 | Design of record matches the provider as built | doc grep | `grep -n 'api_error_status' CLAUDE.md && grep -n 'apiKeySource' CLAUDE.md && grep -n 'Phase 6 corrections to the stack block' .claude/CLAUDE.md && grep -n 'Proposer<P>' CLAUDE.md` | ✅ | ✅ green |
| 06-08-01 | 08 | 5 | PROP-02, PROP-03 | T-06-08-02, T-06-08-03 | Live suite ignored and gated; positive control present | compile + ignored | `cargo test -p minos-propose --test live 2>&1 \| grep -E '3 ignored' && cargo clippy -p minos-propose --all-targets -- -D warnings` | ✅ | ✅ green |
| 06-08-02 | 08 | 5 | PROP-03 | — | CLAUDE.md documents the live suite and its gate variable | doc grep | `grep -n 'MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live' CLAUDE.md` | ✅ | ✅ green |
| 06-09-01 | 09 | 6 | PROP-02, PROP-03 | T-06-09-01, T-06-09-02 | Blocking checkpoint: the user runs the live suite on their logged-in machine, or approves one unchanged agent run; no agent unsets a refused variable; (a) end to end, no canary in (b), hook canary in (c) | manual live (checkpoint:human-action) | `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` (run by the user) | ✅ | ✅ green |
| 06-09-02 | 09 | 6 | PROP-02, PROP-03 | T-06-09-02, T-06-09-03 | Live run recorded and redacted, naming who ran it and any variable unset; verified facts cite it | doc grep | `grep -n '^## Live provider tests (06-09)' .planning/phases/06-proposers/06-EVIDENCE.md && grep -n '06-EVIDENCE.md Live provider tests' CLAUDE.md` | ✅ | ✅ green |
| 06-10-01 | 10 | 7 | PROP-01 to PROP-10 | T-06-10-01, T-06-10-02, T-06-10-03, T-06-10-04 | Full gate, C-01, C-02 green on final code; every `--test` target named here exists | full gate | full suite command above | ✅ | ✅ green |
| 06-10-02 | 10 | 7 | GATE-06 | — | cargo clean last | command | `test ! -d target/debug` | ✅ | ⬜ pending |

Requirement to test mapping (from research, rewritten 2026-10-19 to the test targets the plans create; checked by 06-10 on 2026-10-20 against the SUMMARYs and the final gate's `cargo test --workspace` output: no target was renamed or moved, every `--test` target exists, and the counts at the final gate are provider 11, schema 8, parse 27, history 18, prompt 13, held_out 2, argv_golden 14, classify 39, locate 14, sweep 10, single_shot 8, proposer 4, claude_cli 29, live 1 passed and 3 ignored, the `claude_cli::process` unit tests 10, and minos-core golden_wire 32 and prop_keys 34; see 06-EVIDENCE.md "Final gate"):

| Req ID | Behavior | Test Targets (plan) | Test Type | Automated Command | File Exists |
|--------|----------|---------------------|-----------|-------------------|-------------|
| PROP-01 | `ModelProvider` is one synchronous method and `ModelOutput` has no Debug, Display, Clone, or Deref; single-shot runs unchanged over `FakeProvider` and `ClaudeCli` (generic, never a trait object); the fake returns raw JSON only; `Proposer<P>` dispatches by tier | `provider` (06-02), `single_shot` and `proposer` (06-05), `claude_cli` (06-07) | unit + fake-binary integration | `cargo test -p minos-propose --test provider --test single_shot --test proposer --test claude_cli` | ✅ |
| PROP-02 | Argv equals golden (insta + SHA-256 pin); child env exactly six keys; cwd a fresh, empty `minos-claude-*` directory under the canonical TMPDIR, removed after; prompt on stdin byte-exact; deadline kills the process group and reports Infra; live: the production provider end to end, case (a) | `argv_golden` (06-04), `claude_cli::process` unit tests (06-06), `claude_cli` (06-06, 06-07); live case (a) (06-08, run at 06-09) | golden + real-process unit + fake-binary integration | `cargo test -p minos-propose --test argv_golden --test claude_cli && cargo test -p minos-propose --lib claude_cli::process` | ✅ |
| PROP-03 | With the hostile fixture as the CLI's working directory, its hooks, `.mcp.json` server, and CLAUDE.md never run under the golden argv, case (b); the positive control without the isolation flags fires a hook canary, case (c); the gate makes no model call | `live` (06-08; run by the user at 06-09's checkpoint) | ignored live; gate shows `3 ignored` | gate: `cargo test -p minos-propose --test live` (`3 ignored`); manual: `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` | ✅ |
| PROP-04 | Preflight refuses parent credential and redirect variable names (values never bound), no link or a PATH shim or a target outside `versions/`, version < 2.1.296, `--version` and file-name mismatch, `auth status` exit 1, `authMethod` ≠ `claude.ai`, `apiProvider` ≠ `firstParty`, an `apiKeySource` key, unparseable output, and a work root inside a repository; re-runs the preflight when the link moves | `argv_golden` (env scan, version parsing; 06-04), `locate` (06-04), `claude_cli` (preflight, re-preflight; 06-06, 06-07) | unit + temp layouts + fake-binary integration | `cargo test -p minos-propose --test argv_golden --test locate --test claude_cli` | ✅ |
| PROP-05 | Prompt golden (insta + pin); shuffled history entries or steer notes give identical bytes and an equal `PromptHash`; the prefix is stable across suffix changes | `prompt` (06-03) | golden + proptest | `cargo test -p minos-propose --test prompt` | ✅ |
| PROP-06 | Held-out non-interference (byte-identical prompts under changed held-out values, names absent); history rows sanitized, capped, with visible-objective deltas only; caps hold; untrusted fences cannot be closed by content | `held_out`, `history`, `prompt` (06-03) | proptest + unit | `cargo test -p minos-propose --test held_out --test history --test prompt` | ✅ |
| PROP-07 | Schema golden + pin; schema round trip; each drop rule counted; arbitrary JSON never panics; zero survivors is an empty call with drop counts | `schema`, `parse` (06-02), `single_shot` (06-05) | golden + unit + proptest | `cargo test -p minos-propose --test schema --test parse --test single_shot` | ✅ |
| PROP-08 | Usage extracted from every classifier variant; no type binds a cost field; usage returned with proposals, empty calls, and stops | `classify` (06-04), `single_shot` (06-05) | unit (classifier table) + unit (FakeProvider) | `cargo test -p minos-propose --test classify --test single_shot` | ✅ |
| PROP-09 | `is_error` with `api_error_status` 429 → `ProviderLimit`; 401/403 → `ProviderAuth`; prefix fallbacks; max-turns, structured-output retries, and refusal → Empty; else Infra; typed stops through the tier and `ClaudeCli`; `StopReason::ProviderAuth` golden line | `classify` (06-04), `single_shot` (06-05), `claude_cli` (06-07); minos-core `golden_wire`, `prop_keys` (06-01) | unit + fake-binary integration + golden | `cargo test -p minos-propose --test classify --test single_shot --test claude_cli && cargo test -p minos-core --test golden_wire --test prop_keys` | ✅ |
| PROP-10 | Sweep proposals from params with no provider and no request; same `(run, generation)` gives the same diffs; cumulative from base after a non-base parent | `sweep`, `proposer` (06-05) | unit | `cargo test -p minos-propose --test sweep --test proposer` | ✅ |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `crates/minos-propose/Cargo.toml` — add `rustix` and `tempfile` (deps); `proptest` and `tempfile` (dev-deps) as workspace pins require. *Landed in 06-02 (existing workspace pins only; `cargo deny check` green).*
- [x] Self-contained test targets in place of a shared `tests/support/` module (every item of a shared module must be used by each target that declares it): the fake-claude HOME layout lives in `tests/claude_cli.rs` (06-06, 06-07), the canned envelopes in `tests/classify.rs` (06-04). *Landed as written; no `tests/support/` module exists.*
- [x] `crates/minos-propose/tests/snapshots/` — argv, schema, prompt goldens (accepted deliberately with `INSTA_UPDATE=always`, then pinned by SHA-256). *Landed: `schema__output_schema_batch_4.snap` (06-02), `prompt__golden_prompt.snap` (06-03), `argv_golden__claude_argv.snap` (06-04).*
- [x] `crates/minos-core/tests/snapshots/golden_wire__record_v1_stop_requested_provider_auth.snap` plus its pin. *Landed in 06-01 (one new golden line, no existing wire byte moved, pin recomputed over 25 names).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real `claude -p` structured call through the production provider, case (a); hostile fixture canaries absent with the fixture as the CLI's working directory, case (b); positive control canaries present, case (c) | PROP-02 (a), PROP-03 (b, c) | Needs the user's subscription login and three live model calls on it, and case (c) runs the fixture's hooks under the user's account (D-17: run by hand on a logged-in machine); the gate makes no model call | At 06-09's blocking checkpoint the user runs `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` on their logged-in machine, or explicitly approves one unchanged agent run; any `env -u` is the user's decision and is recorded by variable name only; 06-09 records the redacted result in 06-EVIDENCE.md. **Run 2026-10-20 by the user, by hand, in their own logged-in terminal, on `claude` 2.1.304; no variable unset; `3 passed`, exit 0; (a) end to end with 2 proposals, (b) all three canaries absent, (c) all three canaries fired and the marker appeared (06-EVIDENCE.md "Live provider tests (06-09)"). No agent ran it.** |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 300s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-20 (06-10: final gate green on HEAD `a114940`; live suite run by the user at 06-09)
