---
phase: 06-proposers
verified: 2026-10-20T00:00:00Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 6: Proposers Verification Report

**Phase Goal:** The loop can get candidate diffs from a seeded sweep with no model, or from one text-only call to the user's own `claude` install that runs no tools, loads no project configuration, and handles no credentials, with model output treated as hostile.
**Verified:** 2026-10-20
**Status:** passed
**Re-verification:** No, initial verification

Method: source, tests, snapshots, and recorded gate output were read. No cargo command was run (`target/` is absent by design), and the live suite and the user's `claude` were not touched. Compiled results are taken from 06-EVIDENCE.md "Final gate" (HEAD `a114940`); `git diff a114940 HEAD -- crates Cargo.toml Cargo.lock deny.toml` is empty, so no code changed after the gate.

## Goal Achievement

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Argv matches golden; provider runs user's `claude` from a fresh empty dir outside any repo, six-variable env, tools off, strict empty MCP, no session persistence, prompt on stdin; hostile fixture proves nothing runs | VERIFIED | `command.rs::argv` builds exactly the 18-element SPIKE-07 argv and matches `tests/snapshots/argv_golden__claude_argv.snap` (plus a SHA pin in `argv_golden.rs`). `process.rs::run` uses `env_clear().envs(..)`, `current_dir(fresh dir)`, piped stdin, own process group, deadline, capped stdout. `ClaudeCli::complete` creates a `minos-claude-*` tempdir under a work root that `preflight::work_root` refuses if it or an ancestor holds `.git`, `CLAUDE.md`, `.mcp.json`, or a non-HOME `.claude`. `env.rs::child_env` yields exactly HOME, LANG, LOGNAME, PATH, TMPDIR, USER. Hostile-fixture proof (PROP-03) is the live suite, run by the user on claude 2.1.304 (06-EVIDENCE.md "Live provider tests (06-09)"): case (b) golden argv with the fixture as CLI cwd, canaries session-start, user-prompt, and mcp all absent; positive control (c) without the three isolation flags, all three FIRED, so (b) is not vacuous. The summary honestly states (a) is not cwd-isolation evidence. |
| 2 | Preflight confirms subscription login and refuses when an API key variable would take over; no credential read/stored/forwarded; a fake provider plugs into `ModelProvider` with no caller change | VERIFIED | `preflight.rs`: refuses on any of 13 named variables or the `CLAUDE_CODE_USE_` prefix (names only, values dropped in `ParentEnv::from_vars`), then `auth status` must show `claude.ai`, `firstParty`, and no `apiKeySource`. `grep` finds no `std::env` or `env::var` reads in `src/`. `auth status` struct binds only the four fields, so email and org are never bound. `ModelProvider` is a one-method trait; `Proposer<P: ModelProvider>` is generic (no trait objects); `tests/claude_cli.rs::the_same_single_shot_code_gives_the_same_proposals_over_claude_cli_and_the_fake` runs one helper over both. |
| 3 | Prompts byte-identical for shuffled inputs (golden), stable prefix then bounded history table and sanitized failure excerpts labelled untrusted; held-out values never appear in any prompt | VERIFIED | `prompt.rs::assemble`: prefix (instructions, objectives, program.md, surface files) then suffix (history, `UNTRUSTED SANDBOX OUTPUT` fenced blocks, notes), content-derived fence tokens. `tests/prompt.rs`: golden snapshot plus byte pin, a proptest on shuffled history and notes, a prefix-stability test, and a fence-forgery test. `history.rs` renders only deltas, never absolute values, reads only gates and each objective's own metric, and uses a fixed phrase for any rejection that could name a held-out metric. `tests/held_out.rs`: a proptest varies every held-out value and asserts byte-identical prompt bytes and hash; a direct test asserts the name and value are absent. |
| 4 | `single_shot` returns several candidates parsed into closed `deny_unknown_fields` types, invalid dropped not repaired; `sweep` proposes with no model call; token usage returned, CLI dollar figure ignored; limit/usage-cap surfaces as an infrastructure failure that stops the run, never an `Outcome` | VERIFIED (loop-side append is Phase 7 by design) | `proposal.rs`: `RawOutput`/`RawProposal`/`RawFile` are `deny_unknown_fields`; each item is dropped by a closed `DropReason`, with the canonical diff derived by `diff_trees`, nothing repaired. `sweep.rs::propose` takes no provider parameter, so it cannot call a model. `classify.rs` binds only token counts; `total_cost_usd` and `costUSD` are never declared (grep confirms only a doc comment). `Proposals::usage()` and `models()` are returned for `Spent`. Limit (429 or message prefix) maps to `ProviderStop::Limit` -> `StopReason::ProviderLimit`, auth to `ProviderAuth`; both are `ProviderError`/`ProposeError`, never an `Outcome`. `StopReason::ProviderAuth` (tag 7) is added in core with a golden `record_v1_stop_requested_provider_auth` and key-distinctness tests. Appending `StopRequested`/`Spent` is Phase 7 (LOOP-04, LOOP-08), as REQUIREMENTS.md PROP-08/09 state explicitly. |
| 5 | `cargo clean` run after verification passes (GATE-06) | VERIFIED | 06-EVIDENCE.md records `cargo clean` ("Removed 61913 files, 7.2GiB") as the last cargo command after the final gate; `/Users/seanmcauliffe/Repos/minos/target` does not exist. |

**Score:** 5/5 truths verified (0 behavior-unverified)

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/minos-propose/src/{provider,schema,proposal,prompt,prompt/history,proposer,single_shot,sweep,fake,limits}.rs` | VERIFIED | Substantive, no stubs; all re-exported from `lib.rs` and used by `Proposer`. |
| `crates/minos-propose/src/claude_cli/{command,env,locate,preflight,process,classify,mod}.rs` | VERIFIED | Wired: `ClaudeCli::complete` -> `prepare` (re-locate, `reverify` on a moved link) -> `argv` -> `process::run` -> `classify`. |
| `crates/minos-propose/clippy.toml` | VERIFIED | Bans HashMap/HashSet; fire-proof in 06-EVIDENCE.md. |
| `crates/minos-core/src/ledger.rs` `StopReason::ProviderAuth` | VERIFIED | Explicit tag 7, Display string, golden and key tests in `golden_wire.rs` and `prop_keys.rs`. |
| Tests: `argv_golden, classify, claude_cli, held_out, history, live, locate, parse, prompt, proposer, provider, schema, single_shot, sweep` | VERIFIED | All present; `provider.rs` has a compile-time proof that `ModelOutput` has no Debug, Display, Clone, or Deref. |

### Key Link Verification

| From | To | Via | Status |
|------|----|-----|--------|
| `Proposer::propose` | `single_shot::propose` / `sweep::propose` | tier dispatch | WIRED (planned/agentic return `TierNotInThisRelease`) |
| `single_shot::propose` | `ModelProvider::complete` -> `parse_output` | generic call, hostile `ModelOutput` opened only by the strict parser | WIRED |
| `ClaudeCli::complete` | `classify` -> `ProviderError`/`ModelResponse` | `process::run(..).and_then(classify)` | WIRED |
| `ProviderStop` | `StopReason` | `stop_reason()` via `ProposeError::stop_reason` | WIRED |
| `ClaudeCli::preflight` | `Preflighted` proof | only constructor of `ClaudeCli` | WIRED |

### Data-Flow Trace (Level 4)

Not applicable to UI. Data flow checked: `Prompt` bytes go to stdin; envelope `structured_output` -> `ModelOutput` -> `parse_output` -> `Proposal`; usage from the envelope -> `ModelResponse` -> `Proposals`. All real, no hardcoded empties.

### Behavioral Spot-Checks

SKIPPED by constraint (no build or test run permitted). Relied on the recorded final gate: `cargo test --workspace` 1357 passed, 0 failed, 23 ignored (the live target reports 3 ignored, so the gate made no model call); fmt, clippy `-D warnings`, `cargo deny check`, `cargo doc`, musl guest clippy all exit 0. The C-02 grep (`#!?\[allow\(`) was re-run here and printed nothing; no `TBD`/`FIXME`/`XXX` in `minos-propose` or the touched core ledger.

### Probe Execution

No phase-declared probes (`probe-*.sh`); not applicable.

### Requirements Coverage

| Requirement | Source Plan(s) | Status | Evidence |
|-------------|----------------|--------|----------|
| PROP-01 | 06-02, 06-05, 06-07, 06-10 | SATISFIED | `ModelProvider` trait + generic `Proposer`; same code over `FakeProvider` and `ClaudeCli` |
| PROP-02 | 06-04, 06-06, 06-07, 06-08, 06-09, 06-10 | SATISFIED | argv golden, env, fresh dir, stdin, runner; live case (a) |
| PROP-03 | 06-08, 06-09, 06-10 | SATISFIED | live cases (b) absent and (c) fired, on 2.1.304 |
| PROP-04 | 06-04, 06-06, 06-07, 06-10 | SATISFIED | preflight, refused-variable names only, no env reads |
| PROP-05 | 06-03, 06-10 | SATISFIED | golden + shuffle proptest |
| PROP-06 | 06-03, 06-10 | SATISFIED | bounded history, untrusted fences, held-out proptest |
| PROP-07 | 06-02, 06-05, 06-10 | SATISFIED | strict parser; wording superseded to `{rationale, files}` per D-10/D-11, recorded in REQUIREMENTS.md |
| PROP-08 | 06-04, 06-05, 06-07, 06-10 | SATISFIED (Phase 6 half) | usage returned per call and per model; no dollar type; `Spent` append is LOOP-04 |
| PROP-09 | 06-01, 06-04, 06-05, 06-07, 06-10 | SATISFIED (Phase 6 half) | classifier -> `ProviderStop` -> `StopReason`; `StopRequested` append is LOOP-08 |
| PROP-10 | 06-05, 06-10 | SATISFIED | `sweep::propose` takes no provider |

No orphaned requirements: all ten PROP IDs in REQUIREMENTS.md appear in plan frontmatter. GATE-06 satisfied (SC5).

### Anti-Patterns Found

None blocking. No unreferenced debt markers, no `allow` attributes, no stub returns found in the reviewed sources.

### Human Verification Required

None outstanding. The live PROP-03 proof was produced by the user's own run and is recorded.

### Notes (non-blocking)

- PROP-03's proof is tied to `claude` 2.1.304 and the fixture. A later CLI release (for example one making `--bare` the default) needs a rerun of `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored`; CLAUDE.md records this.
- Compiled gate results were not re-executed in this verification (forbidden); they rest on the recorded run at an unchanged code state.
- Phase 7 must append `Spent` and `StopRequested` from `Proposals::usage/models` and `ProposeError::stop_reason`; the Phase 6 contract for that is in place.

## Gaps Summary

No gaps. All five success criteria and all ten requirement IDs are accounted for in the codebase.

---

_Verified: 2026-10-20_
_Verifier: Claude (gsd-verifier)_
