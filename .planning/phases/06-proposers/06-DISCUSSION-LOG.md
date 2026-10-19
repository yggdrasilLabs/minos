# Phase 6: Proposers - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-19
**Phase:** 06-proposers
**Areas discussed:** Auth preflight and binary resolution, Prompt assembly and cache boundary, Proposal format from the model, Failure classification and tests

---

## Auth preflight and binary resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Native install layout only | Resolve `$HOME/.local/bin/claude` through its symlink, require the target under `$HOME/.local/share/claude/versions/`; anything else refused naming the path checked; no config file | ✓ |
| Native layout plus an explicit override | Same check, plus `--claude-bin <path>` or an env var naming another binary | |
| Any `claude` on PATH, resolved and recorded | First `claude` on PATH after resolving symlinks (the cmux shim case) | |

**User's choice:** Native install layout only

| Option | Description | Selected |
|--------|-------------|----------|
| Run `claude auth status` once per run | Before the first model call, same six-variable environment; require `loggedIn` true, `authMethod` `claude.ai`, `apiKeySource` `none` | ✓ |
| Env scan only | Refuse on `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_USE_*`; cannot see an `apiKeyHelper` | |
| Both, and re-check per call | Env scan plus `auth status` before every call | |

**User's choice:** Run `claude auth status` once per run

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse the run | Stop before any call, name the variable, point at the future API-key provider | ✓ |
| Warn and continue on the subscription | Print once and proceed with the variable stripped | |

**User's choice:** Refuse the run

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse below the verified version | `claude --version` at preflight; refuse below 2.1.296; newer allowed | ✓ |
| No version check | Trust whatever the native layout holds | |

**User's choice:** Refuse below the verified version

---

## Prompt assembly and cache boundary

| Option | Description | Selected |
|--------|-------------|----------|
| The parent's (incumbent's) surface files | Prefix is `program.md` plus the parent tree's surface files; stable per incumbent; sweep renders against the same tree | ✓ |
| The base tree's files, with the incumbent's cumulative diff in the suffix | Prefix never changes for the run; the model must mentally apply a diff | |

**User's choice:** The parent's surface files

| Option | Description | Selected |
|--------|-------------|----------|
| All on stdin, as verified | Prefix then suffix in one stdin payload; argv unchanged | ✓ |
| Prefix via `--system-prompt-file`, suffix on stdin | Replaces Claude Code's default system prompt; unverified; needs a spike | |

**User's choice:** All on stdin

| Option | Description | Selected |
|--------|-------------|----------|
| Rationale excerpt, outcome, and signed delta per visible objective | One row per experiment in the window plus every winner; 200-byte rationale; outcome; signed percent delta vs incumbent; gates as words; no absolute values | ✓ |
| Same, plus absolute visible-metric values | Also the incumbent's and each row's absolute values | |
| Outcome and delta only, no rationale text | Smallest table, no earlier model text re-enters the prompt | |

**User's choice:** Rationale excerpt, outcome, and signed delta

| Option | Description | Selected |
|--------|-------------|----------|
| Tail of the failed stage's log, capped, in a fenced untrusted block | Last `MAX_LOG_EXCERPT_BYTES` of the stage's `LogExcerpt`, delimited as untrusted, per-prompt total cap | ✓ |
| Outcome and stage only, no log text | No sandbox-derived text in the prompt at all | |

**User's choice:** Tail of the failed stage's log, capped, fenced

| Option | Description | Selected |
|--------|-------------|----------|
| Last in the suffix, labelled as the user's notes, verbatim through SafeText | After history and excerpts, heading says they are the user's and take priority, ordinal order | ✓ |
| First in the suffix, before the history | Notes frame how the model reads the history | |

**User's choice:** Last in the suffix

---

## Proposal format from the model

| Option | Description | Selected |
|--------|-------------|----------|
| Full contents of each changed writable file; Minos derives the diff | `{rationale, files: [{path, content}]}`, paths limited to writable; `diff_trees` derives the canonical diff against the parent tree; out-of-writable, missing, or unchanged dropped, never repaired; PROP-07 reworded | ✓ |
| Unified diff text, as PROP-07 says | `{rationale, diff}` straight through `Diff::parse`; every miscounted hunk is a dropped proposal | |
| Diff text, with a file-contents fallback | Both representations; doubles tokens and lets them disagree | |

**User's choice:** Full contents of each changed writable file

| Option | Description | Selected |
|--------|-------------|----------|
| 1 to batch items; fewer is fine, zero valid is an empty call | `minItems` 1, `maxItems` = batch; zero survivors counts toward `max_empty_generations`, never infra; dropped items counted | ✓ |
| Exactly batch items required | `minItems` = `maxItems` = batch | |

**User's choice:** 1 to batch items

---

## Failure classification and tests

**Notes:** The first draft of these questions framed the rate-limit response shape as unverified. The user asked why it was not simply looked up. The Claude Code docs document the result contract (`SDKResultMessage` with `api_error_status`, the usage-limit messages and their `-p` behavior, the 429 retry policy, and the auth messages), so the questions were re-asked on documented facts. Sources: code.claude.com/docs/en/headless, /agent-sdk/typescript, /agent-sdk/agent-loop, /errors.

| Option | Description | Selected |
|--------|-------------|----------|
| By documented subtype and `api_error_status`; unmatched is infra | 429 → `ProviderLimit`; 401/403 → auth; `error_max_turns` / `error_max_structured_output_retries` → zero proposals; else infra via the breaker streak; research verifies `api_error_status` on 2.1.296; message prefixes as fallback | ✓ |
| Switch to stream-json and classify from `rate_limit_event` and `api_retry` | Richer signals; multi-line parsing; golden argv changes | |
| Any non-success stops the run as `ProviderLimit` | Fail closed on every failure | |

**User's choice:** By documented subtype and `api_error_status`

| Option | Description | Selected |
|--------|-------------|----------|
| New closed variant `StopReason::ProviderAuth`, stop at once | Keeps existing lines parseable; never retried; `minos status` says log in again | ✓ |
| Reuse `ProviderLimit` | No ledger change; misleading reason | |
| Infra failure through the breaker | Retries against a dead login, then `InfraFailures` | |

**User's choice:** New closed variant `ProviderAuth`

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed constant in `limits.rs`, kill the process group, infra failure | On the order of 15 minutes, above the CLI's 10-minute request timeout plus retries; no manifest knob | ✓ |
| Manifest knob under `[budget]` | User-tunable deadline with a default | |

**User's choice:** Fixed constant in `limits.rs`

| Option | Description | Selected |
|--------|-------------|----------|
| Ignored tests, run by hand like the VM tests | `#[ignore]`d integration tests in `crates/minos-propose/tests/`, env-var gated; hostile fixture plants a `SessionStart` hook and an MCP server writing canaries; gate makes no model call | ✓ |
| Always-on tests that skip when not logged in | Skipped looks like passed; a logged-in developer pays per run | |

**User's choice:** Ignored tests, run by hand

---

## Claude's Discretion

- `ModelProvider` and `Proposer` type shapes (generic, no trait objects).
- History window N, table byte cap, per-prompt excerpt cap, per-file content cap, each in `limits.rs` with a reason.
- Instruction block wording and untrusted-block delimiters.
- Whether `Origin::Model { model }` records the resolved model ID or the manifest alias.
- Temp directory naming and cleanup; module layout within `minos-propose`.

## Deferred Ideas

- Stable prefix through `--system-prompt-file` (needs a spike; changes the golden argv).
- A manifest knob for the model-call deadline.
- Recording the `claude` CLI version in the ledger.
- `planned` and `agentic` tiers, Bedrock and API-key providers, `DesignJudge` and inbox: later milestones.
