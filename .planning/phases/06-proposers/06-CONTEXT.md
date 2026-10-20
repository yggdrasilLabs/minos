# Phase 6: Proposers - Context

**Gathered:** 2026-10-19
**Status:** Ready for planning

<domain>
## Phase Boundary

`minos-propose` gives the loop candidate proposals from two tiers, and nothing else. The sweep tier wraps core's seeded sampler and renderer (`minos_core::sweep`) as a proposer with no model call (PROP-10). The single-shot tier assembles one deterministic prompt, calls a `ModelProvider`, and turns the model's hostile structured output into validated proposals, returning the call's token usage for a `Spent` event (PROP-01, PROP-05 to PROP-09). The one v1 provider, `ClaudeCli`, execs the user's own `claude` binary in print mode under the SPIKE-07 contract, with a preflight that refuses anything but the subscription login (PROP-02 to PROP-04). A fake provider plugs into the same abstraction for every test above the provider.

It delivers:

- The `ModelProvider` abstraction with request and response types (prompt bytes, the JSON schema, the model ID, token usage), generic like `Backend`, never a trait object.
- `ClaudeCli`: binary resolution, the per-run preflight (`auth status`, `--version`, parent-environment key scan), the argv golden, the fresh empty cwd, the allowlisted environment, the prompt on stdin, the per-call deadline, the result classifier.
- Deterministic prompt assembly: stable prefix, dynamic suffix, golden and shuffled-input tests, the held-out exclusion test.
- `single_shot`: the output schema, strict parsing into closed types, diff derivation, dropping of invalid items with counts.
- `sweep`: the tier wrapper over core's sampler, against the parent tree.
- The ignored live tests (argv contract, hostile project fixture) and the gate tests against the fake provider.
- One core addition: `StopReason::ProviderAuth` (D-15), and the REQUIREMENTS.md wording change for PROP-07 (D-11).

Out of scope: the loop that calls the proposers and records `Proposed` and `Spent` (Phase 7), the runner, the CLI commands beyond what tests need (Phase 8), the `planned` and `agentic` tiers (deferred with TIER-01), the Bedrock and API-key providers (later implementations of the same abstraction), the `DesignJudge` phase and the inbox (POL-01).

</domain>

<decisions>
## Implementation Decisions

### Binary resolution and the per-run preflight (PROP-02, PROP-04)
- **D-01:** The provider finds `claude` by the **native install layout only**: it resolves `$HOME/.local/bin/claude` through its symlink and requires the target to lie under `$HOME/.local/share/claude/versions/`. A PATH lookup, a Homebrew or npm install, a wrapper, or a missing link is refused with a message naming every path checked. No config file and no override flag, as with `minos-vmm` (05-CONTEXT D-04). This is the SPIKE-07 C-01 finding made a rule: the first `claude` on PATH was a cmux shim.
- **D-02:** The preflight runs **`claude auth status` once per run**, before the first model call, with the resolved binary and the same six-variable environment as a model call, and requires `loggedIn` true, `authMethod` `"claude.ai"`, and `apiKeySource` `"none"` (the output shape is in 02-SPIKE-EVIDENCE.md C-02). Any other answer, a non-zero exit, or unparseable output refuses the run. This is the only check that sees an `apiKeyHelper` or a console login configured in `~/.claude/settings.json`. It is not repeated per call.
- **D-03:** If `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, or any `CLAUDE_CODE_USE_*` variable is set in the **parent** environment when the run starts, the run is **refused** before any call, naming the variable. `env_clear` would drop it anyway, but the user's intent is ambiguous and the CLI provider is subscription-only (PROJECT.md terms note); the message points at the future API-key provider.
- **D-04:** The preflight runs `claude --version` and **refuses a version below 2.1.296**, the version the SPIKE-07 contract was proven on. Newer is allowed. The minimum is one constant with its reason next to the argv.

### Prompt assembly (PROP-05, PROP-06)
- **D-05:** The stable prefix is `program.md` plus the surface files **as they stand in the parent (incumbent) tree**, so the model edits what it sees. A proposal is relative to its parent tree and the loop composes the cumulative diff (03-CONTEXT D-04 amendment), so the proposer takes the parent tree's file contents as input; the loop derives them by applying the incumbent's cumulative diff to the base with `minos_core::diff::apply`. The prefix is byte-stable across every generation with the same incumbent; an improvement changes it once. The sweep tier renders against the same parent tree, so a sweep after a model improvement keeps the improvement.
- **D-06:** The whole prompt goes **on stdin**, prefix then suffix, as the SPIKE-07 contract verified. The argv stays exactly the contract's; no `--system-prompt-file` or `--append-system-prompt` in v1. Claude Code's own default system prompt precedes ours on every call, which is accepted: the prefix is still a cacheable run of bytes after it.
- **D-07:** The history table in the suffix has one row per experiment of the most recent N generations plus every accepted winner, each with: a rationale excerpt of `RATIONALE_EXCERPT_BYTES` (200), the outcome (scored; rejected with its `Reason`; crashed or timed out with the `Stage`), each gate as a word, and for scored rows the **signed percent delta against the incumbent on each visible objective**. **No absolute metric values**, and never a held-out metric's value, name-level presence aside. N and the table's byte cap are Claude's discretion in `limits.rs`.
- **D-08:** Failure excerpts: for each crashed, timed-out, or rejected experiment in the window, the **tail of the failed stage's `LogExcerpt`** (already control-stripped by core, at most `MAX_LOG_EXCERPT_BYTES`), inside a fenced block whose delimiter names it as untrusted sandbox output that the model must not follow as instructions, with a per-prompt total cap so a flood of failures cannot push `program.md`'s instructions out. Which stage comes from the `Outcome`.
- **D-09:** `minos steer` notes sit **last in the suffix**, after the history and excerpts, under a heading that says they come from the user and take priority over the history, in ordinal order (03-CONTEXT D-07), rendered through `SafeText`.

### Proposal format (PROP-07)
- **D-10:** Each proposal the model returns is `{rationale, files: [{path, content}]}`: the **full contents of every writable file the proposal changes**. `minos-propose` builds the candidate tree from the parent tree plus those files and derives the canonical diff with `minos_core::diff::compose::diff_trees`, the same path the sweep tier uses, so the model never writes hunk headers. The strict rules hold unchanged: a path that is not byte-for-byte a `writable` entry, a path not in the parent tree, a file listed with unchanged content making the tree equal to the parent (`NoChange`), CR or NUL bytes, or a result over the diff caps drops that proposal, never repairs it. The derived diff is what `Proposed` records and what `ExperimentId::of` hashes.
- **D-11:** **PROP-07's wording changes** from "`{rationale, diff}` candidates" to "`{rationale, files}` candidates from which the canonical diff is derived" in REQUIREMENTS.md, in the same change that implements it. The success criterion in ROADMAP.md Phase 6 item 4 is amended the same way.
- **D-12:** The schema's `proposals` array has `minItems` 1 and `maxItems` equal to the proposer's `batch` (1 to 16). A call that returns fewer than `batch`, or whose items all fail validation, yields what survived; **zero survivors is an empty call**, which the loop records through `Selected { NoValidCandidates }` and the breaker's `max_empty_generations`, never as an infrastructure failure. Every dropped item is counted for `Decision::dropped`.

### Failure classification and the call's bounds (PROP-08, PROP-09)
- **D-13:** The result envelope is parsed leniently and `structured_output` strictly (`.claude/CLAUDE.md` Question 4). A non-success result is classified by the **documented `subtype` and `api_error_status`** fields of the result message (code.claude.com/docs/en/agent-sdk/typescript, `SDKResultMessage`): `error_during_execution` with `api_error_status` 429 is a provider limit, the subscription's session, weekly, or model-family limit or a throttle the CLI gave up retrying, and maps to `StopReason::ProviderLimit`; `error_during_execution` with 401 or 403 is an auth failure (D-15); `error_max_turns` or `error_max_structured_output_retries` is **zero valid proposals for this call** (D-12), never infra; everything else, including a spawn failure, a deadline (D-16), an envelope that does not parse, a non-zero exit with no classifiable result, or any other status, is an **infrastructure failure** counted by the loop's consecutive-failure streak for `circuit_breaker` and `max_infra_failures`. The documented message prefixes (`You've hit your session limit`, `You've hit your weekly limit`, `Login expired`, `OAuth token revoked`) are a fallback match on the `errors` strings when `api_error_status` is absent. Research verifies with one real call on 2.1.296 that `api_error_status` is present on this CLI version; the provider stays on `--output-format json`, not stream-json.
- **D-14:** Only token counts leave the envelope: `usage.input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` into `Usage::tokens`. `total_cost_usd` and `modelUsage.*.costUSD` are never read into any type (COST-01, PROP-08).
- **D-15:** An auth failure after a passed preflight is recorded as a **new closed variant `StopReason::ProviderAuth`**, stopping the run at once, never retried and never counted as a rate limit. Adding a variant keeps every existing ledger line parseable (03-CONTEXT ledger rule); the golden file for `StopRequested` gains one case, and `minos status` can tell the user to log in again before resuming.
- **D-16:** The subprocess has one **fixed per-call deadline, a constant in `limits.rs`** on the order of 15 minutes, chosen above the CLI's 10-minute per-request timeout plus its retry backoff, with its reason beside it. On expiry the provider kills the child's **process group**, as the VM supervisor does, and reports an infrastructure failure. No manifest knob in v1.

### Tests (PROP-02, PROP-03, PROP-05, PROP-06)
- **D-17:** Tests that make a real model call are **`#[ignore]`d integration tests** in `crates/minos-propose/tests/`, run by hand on a logged-in machine with `-- --ignored`, gated by an environment variable that only the ignored tests read (as `MINOS_VMM` and friends are, 05-CONTEXT D-04). They are: the argv-contract run (one structured call through the production provider) and the PROP-03 hostile fixture, a project directory with a `.claude/settings.json` declaring a `SessionStart` hook and a `.mcp.json` server, each of which writes a canary file when started; the test runs a proposal with that directory as the project and asserts no canary exists and a valid structured output arrived. The gate's `cargo test --workspace` makes no model call and needs no login: the argv golden, the prompt goldens and shuffled-input tests, the held-out exclusion test, the schema round trip, and the classifier's table all run against hand-built envelopes and the fake provider.

### Research amendments (2026-10-19, from 06-RESEARCH.md; intent unchanged, wording corrected; flagged for the user at the phase checkpoint)
- **D-02 (amended):** `claude auth status` prints no `apiKeySource: "none"`; the key is absent unless a key source exists, and `"none"` appears only in stream-json `system/init`. The preflight therefore requires `loggedIn` true, `authMethod` `"claude.ai"`, `apiProvider` `"firstParty"`, and **no `apiKeySource` key**, deserialized leniently into a struct that binds only those fields, with a non-zero exit (1 = not logged in), unparseable output, or a deadline as a refusal (06-RESEARCH Finding 2, Pitfall 3; verified live on 2.1.304).
- **D-01 and D-02 (amended, run lifetime):** the auto-updater keeps about four versions under `~/.local/share/claude/versions/`, deletes older ones, and moves the `~/.local/bin/claude` link (2.1.296 is already gone; 2.1.304 is installed). The provider re-resolves the link before every call and re-runs the `--version` and `auth status` preflight only when the resolved target differs from the one preflighted; "once per run" holds while the binary is unchanged (06-RESEARCH Finding 3, Pitfall 1).
- **D-13 (amended):** in both the documented `SDKResultMessage` type and the installed bundle, `api_error_status` exists **only on `subtype: "success"`**. An API error that ends the turn (a 429 usage or rate limit, a 401/403 auth failure) arrives as `subtype` `success`, `is_error` true, `api_error_status` the HTTP status or null, and the message in `result`, with no `errors` array; `error_during_execution` is the thrown-exception path with `errors[]` and never `api_error_status`. The classifier keys on `is_error` plus `api_error_status` on the success variant, then falls back to the documented message prefixes in `result` (success) or `errors` (error variants); `error_max_turns` and `error_max_structured_output_retries` remain zero valid proposals (D-12); `stop_reason` `"refusal"` is also an empty call; everything else is an infrastructure failure. The full table is 06-RESEARCH Pattern 7. `usage` is present on every result variant and is returned with every classification. The provider stays on `--output-format json`.
- **D-03 (extended, A3):** the refused parent-environment set is the credential and redirect variables the installed CLI reads, not only the three named: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_USE_*`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_OAUTH_REFRESH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR`, `CLAUDE_CODE_API_KEY_FILE_DESCRIPTOR`, `ANTHROPIC_FOUNDRY_API_KEY`, `ANTHROPIC_AWS_API_KEY`, `ANTHROPIC_IDENTITY_TOKEN`, `ANTHROPIC_IDENTITY_TOKEN_FILE`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_CUSTOM_HEADERS`, and `CLAUDE_CONFIG_DIR`. Only names are scanned; values are never bound, stored, or logged; the message names the variable. The parent environment is passed to the preflight as a value, because edition 2024 makes `set_var` unsafe and tests cannot change the process environment (06-RESEARCH Pitfall 4, Pitfall 6).
- **D-17 (extended):** the PROP-03 live suite has three ignored cases: (a) the production provider against the hostile fixture project, canaries absent; (b) the golden argv run with the fixture as cwd, canaries absent, proving `--safe-mode`, `--strict-mcp-config`, and the empty `--mcp-config` suffice as defense in depth; (c) a positive control without those flags, canaries present, proving the fixture is live on the installed version. The gate tests run the real spawn path against a fake `claude` shell script in a temporary `HOME` (06-RESEARCH Open Question 5, Pitfall 6).
- **D-10 (noted):** the CLI's default output cap is on the order of 32,000 tokens, so `batch` full-file proposals over large writable files end as an infrastructure failure every generation; the instruction block asks for changed files only, and a propose-side budget check is Claude's discretion (06-RESEARCH Pitfall 5).

### Claude's Discretion
- The `ModelProvider` trait's exact request and response types, and the `Proposer` interface the loop will call (inputs: manifest, program, parent-tree surface files, a read-only history view, steer notes, generation; outputs: proposals with rationale and derived diff, dropped count, usage, prompt hash). No trait objects; the loop is generic over the provider as the runner is over `Backend`.
- The history window N, the history table byte cap, the per-prompt excerpt cap, and the per-file content cap for returned files, each a constant in `limits.rs` with a reason, within the existing diff caps and `SINGLE_SHOT_SURFACE_MAX_BYTES`.
- The wording of the fixed instruction block in the prefix (the writable list, the output contract, the objective names and goals, the gates) and the delimiters of the untrusted blocks.
- Whether the resolved model ID from `modelUsage` (for example `claude-haiku-5-5` for the alias `haiku`) or the manifest's alias is what `Origin::Model { model }` records; either must be a valid `ModelId`, and the choice is stated in CLAUDE.md.
- The per-run temp directory's name and cleanup, under `$TMPDIR`, outside any repository, one fresh directory per call.
- Module layout within `minos-propose` (ARCHITECTURE.md sketches `prompt`, `provider`, `claude_cli`, `single_shot`, `schema`; add `sweep`).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record
- `CLAUDE.md` — Threat model (model output is hostile input); Architecture, "The model stays outside the sandbox" (the `claude -p` contract, the canonical binary rule, the terms note); Code rules; `minos-core` (Diff, `ExperimentId`, Ledger `Origin`, `SpendSource`, `StopReason`); Projects, Proposer ladder (sweep and single_shot in v1, prompt assembly deterministic, invalid proposals dropped); Cost (tokens only); Verified facts local, the `claude` 2.1.296 line.
- `.claude/CLAUDE.md` — Decisions at a glance 9 (`claude -p` with `env_clear` plus allowlist) and 10 (sync std, no tokio); Question 4 (lenient envelope, strict `structured_output`, `deny_unknown_fields`, no `flatten`); Question 9 (flags, result fields, corrections from the spike, binary identity); Question 6 (test tools); What NOT to Use (`--bare` with subscription auth, `total_cost_usd`).

### Phase 2 evidence this phase builds on
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` — Plan 02-02 (C-01 binary identity; C-02 `auth status` output under the six-variable environment; C-03 base argv and result shape; C-04 tools list and `apiKeySource`; C-11 and C-12 unsatisfiable schema ends in `error_max_turns`; C-14 session persistence) and the SPIKE-07 contract table.

### Phase 3 decisions this phase must honor
- `.planning/phases/03-core-types-and-pure-logic/03-CONTEXT.md` — D-03 (no-op diff invalid), D-04 and its amendment (cumulative diff, compose, `diff_trees`), D-05 (`StopReason` is closed; `ProviderLimit`), D-07 (steer notes in ordinal order), D-10 (`Proposed` with `Origin`, `Spent` with `SpendSource::ModelCall`), the renderer and sweep bullets under Claude's Discretion.
- `crates/minos-core/src/sweep.rs` — `seed`, `sample`, `render`; the tier wrapper calls these and nothing else.
- `crates/minos-core/src/diff/compose.rs` — `diff_trees`, which D-10 reuses; `crates/minos-core/src/diff/apply.rs` — `Tree` and the applier the loop uses to produce the parent tree.
- `crates/minos-core/src/ledger.rs` — `Origin`, `SpendSource`, `StopReason` (D-15 adds a variant), `Decision::dropped`.
- `crates/minos-core/src/route.rs` and `manifest.rs` — `ProposerSpec`, `Tier`, `eligible`, `SurfaceStats`, `BATCH_MIN`/`BATCH_MAX`.
- `crates/minos-core/src/usage.rs` — `Usage::tokens`; `crates/minos-core/src/text.rs` — `SafeText`, `LogExcerpt`; `crates/minos-core/src/id.rs` — `PromptHash::of`, `ModelId`; `crates/minos-core/src/limits.rs` — `RATIONALE_EXCERPT_BYTES`, `MAX_LOG_EXCERPT_BYTES`, `SINGLE_SHOT_SURFACE_MAX_BYTES`, `MAX_STEER_NOTE_BYTES`, and the rule that later caps live beside the code that enforces them.

### Claude Code documentation (the result and error contract, read 2026-10-19)
- https://code.claude.com/docs/en/agent-sdk/typescript — `SDKResultMessage` (subtypes, `is_error`, `errors`, `api_error_status`, `terminal_reason`), `SDKRateLimitEvent` (stream-json only), the `SDKAssistantMessageError` categories.
- https://code.claude.com/docs/en/agent-sdk/agent-loop — "Handle the result": which subtype carries `result`, the error subtypes, non-zero exit on an error result.
- https://code.claude.com/docs/en/errors — the usage-limit messages and their `-p` behavior ("ends the run and reports an execution error"; json output reports the message in the result), 429 retry policy (up to 10 retries, `CLAUDE_CODE_MAX_RETRIES`), the authentication messages and the `authentication_failed` code.
- https://code.claude.com/docs/en/headless — `-p` loads project hooks and `.mcp.json` without `--bare` (why D-17's fixture exists), the 10 MB stdin cap, `system/api_retry` categories, SIGTERM exit 143.

### Requirements and research (dated inputs; evidence wins over them)
- `.planning/REQUIREMENTS.md` — PROP-01 to PROP-10 (PROP-07 reworded by D-11).
- `.planning/ROADMAP.md` — Phase 6 goal and success criteria (item 4 reworded by D-11).
- `.planning/research/ARCHITECTURE.md` — the `minos-propose` module sketch and `ModelRequest` shape; Pattern 9 flag list (superseded by the SPIKE-07 contract where they differ).
- `.planning/research/PITFALLS.md` — Pitfall 9 (print mode loads project config), 11 (adaptive overfitting; held-out never in a prompt), 13 (prompt injection; sandbox text only as `LogExcerpt`), 17 (deterministic prompt assembly), the credit-exhaustion entry (stop, never tight-retry), the terms entry.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `minos_core::sweep::{seed, sample, render}`: the whole sweep tier's logic; the wrapper only supplies the run, generation, params, batch, and the parent tree.
- `minos_core::diff::compose::diff_trees` and `diff::apply::Tree`: D-10's diff derivation from returned file contents, and the parent-tree construction.
- `minos_core::text::{SafeText, LogExcerpt}`: every string from a model or a sandbox that enters a prompt or a message.
- `minos_core::usage::Usage::tokens`, `id::PromptHash::of`, `id::ModelId`, `ledger::{Origin, SpendSource, StopReason}`: the types the loop will record; nothing new is needed except D-15's variant.
- `minos_core::limits`: `RATIONALE_EXCERPT_BYTES`, `MAX_LOG_EXCERPT_BYTES`, `SINGLE_SHOT_SURFACE_MAX_BYTES`, `MAX_STEER_NOTE_BYTES`, `BATCH_MIN`/`BATCH_MAX`, `MAX_MODEL_ID_BYTES`.
- `crates/minos-propose/` is a stub with `minos-core`, `serde`, `serde_json`, and `thiserror` declared and `insta` as a dev-dependency; `deny.toml` already confines it to `minos-loop` and `minos-cli`.

### Established Patterns
- `std::process::Command` with `env_clear()` plus an allowlist, argv arrays, stdin for data, process-group kill on a deadline: `minos-source`'s git clean check (04-CONTEXT D-02) and `minos-backend-krun`'s supervisor are the models.
- Raw struct with `deny_unknown_fields` and `try_from` into a private-field type; the envelope is the one lenient exception (`.claude/CLAUDE.md` Question 4).
- Golden wire files as insta snapshots paired with a SHA-256 pin, `update: no`; golden hashes as plain `assert_eq!`.
- Ignored VM-host tests reading environment variables that production code never reads (05-CONTEXT D-04).
- Errors separate blame: `thiserror` enums distinguishing a candidate or model fault (dropped, or zero proposals) from an infrastructure failure (retried) from a provider stop (`ProviderLimit`, `ProviderAuth`).

### Integration Points
- Phase 7's loop calls the proposer per generation, supplies the parent tree and history view from the folded `RunState`, records `Proposed` (diff and rationale as blobs, `Origin::Model { model, prompt }`), `Spent { SpendSource::ModelCall }`, and `StopRequested` on `ProviderLimit` or `ProviderAuth`, and counts empty calls toward the breaker.
- Phase 8's `minos check` prints the route table and runs the provider preflight (D-01 to D-04) with its refusals explained.
- `minos-core::ledger` gains `StopReason::ProviderAuth` (D-15) with its golden case.

</code_context>

<specifics>
## Specific Ideas

- The CLI provider is subscription-only and personal by the terms Anthropic publishes: a preflight that refuses an API key in the environment (D-03) is the enforcement, and the README (Phase 8) points shared or hosted use at the API-key or Bedrock provider.
- The documented result contract is the classifier's source of truth: `api_error_status` 429 and 401/403 on `error_during_execution`. Message-prefix matching is only a fallback. One real call during research confirms the field on 2.1.296.
- A proposal is "the files as they should be", not "a patch": the model does what models do well, and Minos does the diffing, exactly as the sweep tier already does.

</specifics>

<deferred>
## Deferred Ideas

- Delivering the stable prefix through `--system-prompt-file` to replace Claude Code's default system prompt (fewer tokens, no coding-agent framing). Unverified with `-p`, `--json-schema`, and `--safe-mode` together; a later phase may spike it and change the golden argv.
- A manifest knob for the model-call deadline (D-16 fixes a constant).
- Recording the `claude` CLI version in the ledger (no field exists; `Origin::Model` carries model and prompt hash only). Consider with the AWS milestone's provider provenance.
- Failure excerpts after a restart: `Judged` carries no log blob IDs, so Phase 7 must decide whether to persist per-stage log `BlobId`s (an `Option` field keeps `"v":"1"`) or show excerpts only for the live session; Phase 6 defines the history input with an optional excerpt per row (06-RESEARCH Open Question 4).
- The `planned` and `agentic` tiers (TIER-01), the Bedrock and API-key providers, the `DesignJudge` phase and inbox (POL-01): later milestones, unchanged.

</deferred>

---

*Phase: 06-proposers*
*Context gathered: 2026-10-19*
