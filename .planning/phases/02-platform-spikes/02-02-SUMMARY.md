---
phase: 02-platform-spikes
plan: 02
subsystem: propose
tags: [claude-cli, spike, structured-output, subscription-auth, env-allowlist, evidence]
status: complete

requires: []
provides:
  - SPIKE-07 answered from this plan's own calls: `claude -p` with `--tools ""` and `--json-schema` returns `structured_output` under subscription login, from an empty temp dir, with the six-variable env allowlist
  - SPIKE-07 contract table (evidence/02-02.md) for the Phase 6 ClaudeCli provider (PROP-02, PROP-04)
  - Research claims checked list for 02-10 / 02-11 (DOCS-05) to carry into CLAUDE.md
affects: [02-10, 02-11, 06-propose]

tech-stack:
  added: []
  patterns:
    - "Raw CLI output captured to scratch files; one documented sed -E redaction pass before anything is appended to evidence"
    - "Each live call from a fresh mktemp -d directory, env -i with a six-variable allowlist, canonical binary path"

key-files:
  created:
    - .planning/phases/02-platform-spikes/evidence/02-02.md
  modified: []

key-decisions:
  - "Canonical claude binary is realpath ~/.local/bin/claude (~/.local/share/claude/versions/2.1.296); the PATH claude is a cmux shim and is never executed"
  - "Under --max-turns 1 a schema call reports num_turns 2 and succeeds; an unsatisfiable schema ends in error_max_turns (exit 1), never error_max_structured_output_retries, under --max-turns 1 or 3"
  - "Token accounting must read top-level usage (equal to modelUsage); usage.iterations does not sum to the totals in a multi-turn call"
  - "--disable-slash-commands empties slash_commands and skills but agents stay listed; with --tools \"\" no Agent or Skill tool exists"

patterns-established:
  - "Evidence protocol for live CLI spikes: verbatim redacted JSON, read-offs listed under each block, contract table rows cite C-NN"

requirements-completed: [SPIKE-07]

duration: 7min
completed: 2026-10-10
---

# Phase 2 Plan 02: `claude -p` contract (SPIKE-07) Summary

**`claude -p --tools "" --json-schema` under subscription login, from an empty temp dir with a six-variable env, returns `structured_output` `{"n":7}` in `num_turns` 2; ten haiku calls pin the flag matrix, field names, and failure subtypes for the Phase 6 provider.**

## Performance

- **Duration:** about 7 min
- **Started:** 2026-10-10T12:55:51Z
- **Completed:** 2026-10-10T13:02:38Z
- **Tasks:** 2
- **Files modified:** 1
- **Model calls:** 10 of the 10 allowed, all `haiku` (C-03 to C-12). C-01, C-02, C-13, C-14 made no model call.

## Accomplishments

- C-01 to C-04: the canonical binary, auth under the allowlist, the base contract, and the `system`/`init` tools list, recorded verbatim with PII and cost estimates removed.
- C-05 to C-12: one call per flag in the CONTEXT discretion list, plus the unsatisfiable-schema case under `--max-turns 1` and `3`.
- C-13 (help text) and C-14 (no session directory for any work dir), no model calls.
- SPIKE-07 contract table and a research-claims check: three claims differ from 02-RESEARCH.md (usage has three extra fields; `--disable-slash-commands` leaves `agents`; the failing schema gives `error_max_turns`).

## SPIKE-07 contract (copied from evidence/02-02.md)

| Item | Observed | Evidence |
|---|---|---|
| Canonical binary path and version | `realpath ~/.local/bin/claude` gives `~/.local/share/claude/versions/2.1.296`, which prints `2.1.296 (Claude Code)`. The first `claude` on the inherited PATH is a cmux bash shim under `$TMPDIR/cmux-cli-shims/`; it is never executed. | C-01 |
| Env allowlist works with subscription auth | Yes. Under `env -i` with only `HOME`, `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `USER`, `LOGNAME`, `TMPDIR`, `LANG=en_US.UTF-8`: `loggedIn` `true`, `authMethod` `"claude.ai"`, `apiProvider` `"firstParty"`, `subscriptionType` `"max"`; `apiKeySource` `"none"` in `system`/`init`; all ten model calls returned a `result` with non-zero `usage.output_tokens`. | C-02, C-04, C-03 to C-12 |
| Work directory | A fresh `mktemp -d` directory under `$TMPDIR`, outside any repository, one per call; `system`/`init` `cwd` reports it. | C-03 to C-12 |
| Prompt delivery | On stdin (`printf '...' \| ...`) with no positional prompt. | C-03 |
| Base argv | `-p --model haiku --tools "" --output-format json --json-schema '<schema>' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`: exit 0, one JSON object on stdout, empty stderr. | C-03 |
| `structured_output` shape | Top-level key `structured_output` holds the schema object as JSON (`{"n":7}`); `result` holds the same object as a JSON string (`"{\"n\":7}"`). Both keys are absent from error results, which carry `errors: [string]` instead. | C-03, C-11, C-12 |
| `subtype` values seen | `"success"` (C-03 to C-10) and `"error_max_turns"` (C-11, C-12). Not seen: `error_max_structured_output_retries`, `error_during_execution`, `error_max_budget_usd`. | C-03 to C-12 |
| `is_error` and exit code | `success`: `is_error` `false`, exit 0. `error_max_turns`: `is_error` `true`, exit 1. Stderr was empty in every call. | C-03, C-11, C-12 |
| `num_turns` under `--max-turns 1`, with a schema | `2`, `subtype` `success`, `stop_reason` `"tool_use"` (structured output goes through the `StructuredOutput` tool). | C-03, C-04, C-05, C-07 to C-10 |
| `num_turns` under `--max-turns 1`, without a schema | `1`, `subtype` `success`, `stop_reason` `"end_turn"`, `result` `"ok"`, no `structured_output` key. | C-06 |
| `usage` field names | `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`, `output_tokens_details.thinking_tokens`, `server_tool_use.{web_search_requests, web_fetch_requests}`, `service_tier`, `cache_creation.{ephemeral_1h_input_tokens, ephemeral_5m_input_tokens}`, `inference_geo`, `iterations[].{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens, cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}, type}`, `speed`, `fallback_credit`. In a multi-turn call `iterations` held one entry that did not sum to the top-level totals; the top-level totals equal `modelUsage`. | C-03, C-12 |
| `modelUsage` field names | One key per resolved model id (`claude-haiku-5-5` for alias `haiku`), each with `inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `webSearchRequests`, `costUSD` (estimate, never recorded), `contextWindow`, `maxOutputTokens`, `thinkingTokens`, `canonicalModel`, `provider`, `costBasis`. | C-03 |
| Tools list under `--tools ""` | With `--json-schema`: `["StructuredOutput"]`. Without: `[]`. Agent, Skill, and every other built-in tool are absent in both. `mcp_servers` `[]`. | C-04, C-06 |
| Slash commands and skills under `--disable-slash-commands` | `slash_commands` `[]` and `skills` `[]` (55 and 19 without the flag); `terminal_slash_commands` absent. `agents` still lists 4 entries, but no Agent tool exists. Result unchanged. | C-04, C-05 |
| `--disallowedTools` | `--disallowedTools "mcp__*"` accepted alongside `--tools ""`; exit 0, result unchanged (same key paths as C-03). | C-07 |
| Permission flag | Both work with `-p`. `--permission-prompts none` leaves `permissionMode` `"default"`; `--permission-mode dontAsk` sets it to `"dontAsk"`. Results identical. No call made a permission request, so the behavioral difference between them is `open` (not exercised with tools off). | C-04, C-08, C-13 |
| `--setting-sources ""` | Accepted; auth works; result unchanged. | C-09 |
| `--restricted` | Accepted together with `--safe-mode` and `--strict-mcp-config`; auth works; result unchanged. | C-10 |
| `--safe-mode` | Accepted; three builtin plugins (`cc-plugin-agents-md`, `cc-plugin-telemetry`, `cc-plugin-plugin-authoring`) are still listed in `system`/`init`, as its help text says. | C-04, C-13 |
| `--strict-mcp-config` with an empty config | `mcp_servers` `[]`. | C-04 |
| `--max-turns` | Not in `--help`, but accepted and enforced (`errors: ["Reached maximum number of turns (1)"]`). At the cap `num_turns` reads one more than the flag value. | C-11, C-12, C-13 |
| `--no-session-persistence` | Accepted. No directory under `~/.claude/projects` names any of the ten work directories. Whether the CLI writes one without the flag is `open` (no control call; budget spent). | C-03, C-14 |
| Unsatisfiable-schema behavior | `--max-turns 1`: exit 1, `subtype` `error_max_turns`, `is_error` `true`, `num_turns` 2, `terminal_reason` `"max_turns"`, no `structured_output`. `--max-turns 3`: the same subtype and exit, `num_turns` 4. The turn cap ends the run before any structured-output retry limit shows. | C-11, C-12 |
| Model calls made by this plan | 10 (C-03 to C-12), all `haiku`. | C-03 to C-12 |

## Task Commits

1. **Task 1: Binary identity, auth under the allowlisted environment, and the base contract** - `789538c` (docs)
2. **Task 2: Flag matrix and the SPIKE-07 contract table** - `51c0e0f` (docs)

## Files Created/Modified

- `.planning/phases/02-platform-spikes/evidence/02-02.md` - C-01 to C-14 verbatim calls, SPIKE-07 contract table, research claims checked

## Decisions Made

See `key-decisions` in the frontmatter. No design-of-record file was edited (02-10 and 02-11 carry these into CLAUDE.md).

## Deviations from Plan

### Procedural adjustments

**1. C-08 used `--output-format stream-json --verbose` instead of `json`**
- **Found during:** Task 2
- **Issue:** the question asks whether the permission flag changes `permissionMode`, which only the `system`/`init` line reports; the `json` result has no such field.
- **Fix:** ran C-08 as stream-json and kept the `system`/`init` and `result` lines (noted in the section). The `result` line carries the same keys as the `json` output (C-03 against C-04). Same call count.

**2. Added C-13 (`--help` excerpt) and C-14 (session directory check), both with no model call**
- **Issue:** the permission-flag and `--max-turns` rows needed the help text as evidence, and `--no-session-persistence` had no observable effect in the call outputs.
- **Fix:** two no-model sections. Total `Command:` lines is 14, at the plan's cap.

**3. C-01 runs its four commands as one `Command:` line** (`cd W && { ...; }`) so the fragment stays within the 14-line cap and every `Command:` begins with `cd `.

**4. The redaction program also matches keys containing `email`, and it runs on every output**, so the `uuid` value in each `result` and `system`/`init` line is `"<redacted>"` too. The header of the fragment states this. No other value is altered.

---

**Total deviations:** 4 procedural, no code changes.
**Impact on plan:** none on scope; every acceptance criterion holds.

## Issues Encountered

- A read-off bullet under C-11 first stated `usage.iterations` had 2 entries; checking the raw file showed 1. Corrected to the observed value before the Task 2 commit.

## Known Stubs

None. This plan writes evidence only.

## Next Phase Readiness

- Phase 6 can build the `ClaudeCli` provider from the contract table: argv, env allowlist, stdin prompt, `structured_output` and `errors` shapes, exit codes, subtypes, and token fields.
- Open for later: the behavioral difference between `--permission-prompts none` and `--permission-mode dontAsk` (no permission request occurs with tools off), and whether the CLI writes a session without `--no-session-persistence`.
- 02-10 should carry the three `differs` research claims into CLAUDE.md, and note the CLI is now 2.1.296 (PROJECT.md and `.claude/CLAUDE.md` Question 9 cite 2.1.295).

## Verification

- Task 1 and Task 2 automated verify blocks: passed (both negative greps print nothing; 14 `Command:` lines).
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`, and the musl `minos-guest` clippy: all exit 0.
- Ten `[lints] workspace = true` crates; the `#[allow(` grep prints nothing; `git diff --name-only` lists only `.planning/` paths.
- `cargo clean` not run (02-11 does, GATE-06).

## Self-Check: PASSED

- FOUND: .planning/phases/02-platform-spikes/evidence/02-02.md
- FOUND: .planning/phases/02-platform-spikes/02-02-SUMMARY.md
- FOUND: 789538c, 51c0e0f
