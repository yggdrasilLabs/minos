---
phase: 06-proposers
plan: 07
subsystem: propose
tags: [claude-cli, model-provider, typestate, preflight, re-resolution, subprocess, tdd, design-of-record]
status: complete

requires:
  - phase: 06-proposers
    plan: 02
    provides: ModelProvider, ModelRequest, ModelResponse, ProviderError, ProviderStop, InfraCause, CALL_DEADLINE, MAX_ENVELOPE_BYTES, MAX_STDERR_BYTES
  - phase: 06-proposers
    plan: 04
    provides: argv, classify, Exit, locate, Located, ParentEnv
  - phase: 06-proposers
    plan: 05
    provides: Proposer, ProposeInput, Proposals, FakeProvider, FakeReply
  - phase: 06-proposers
    plan: 06
    provides: preflight, Preflighted, PreflightError, crate-private reverify, crate-private process::run
provides:
  - "minos_propose::claude_cli::ClaudeCli: preflight(&ParentEnv) (only constructor), version(), impl ModelProvider, manual Debug"
  - "crate-private Preflighted::replace_located, used only after reverify passes"
  - "crate-root re-exports ClaudeCli, ParentEnv, PreflightError, Preflighted"
  - "tests/claude_cli.rs: fake claude -p branch (argv, env, cwd, listing, stdin, canned envelope, exit code) and the generic propose_with<P: ModelProvider> helper"
  - "CLAUDE.md provider section, Workspace line, Open questions; .claude/CLAUDE.md Phase 6 corrections block"
affects: [06-08 live tests, 06-09, 06-10 phase close, 07 minos-loop, 08 minos check and minos run]

tech-stack:
  added: []
  patterns:
    - "Typestate provider: ClaudeCli wraps Mutex<Preflighted>; only ClaudeCli::preflight builds one; no Clone, no Default"
    - "Per-call re-resolution: locate under the lock, reverify only on a changed target, adopt via replace_located, copy out and release the lock before the spawn"
    - "Call directory removal is checked: a failed close is InfraCause::WorkDir carrying the call's usage, unless a stop already ended the run"

key-files:
  created:
    - .planning/phases/06-proposers/06-07-SUMMARY.md
  modified:
    - crates/minos-propose/src/claude_cli/mod.rs
    - crates/minos-propose/src/claude_cli/preflight.rs
    - crates/minos-propose/src/provider.rs
    - crates/minos-propose/src/lib.rs
    - crates/minos-propose/tests/claude_cli.rs
    - CLAUDE.md
    - .claude/CLAUDE.md

key-decisions:
  - "ClaudeCli holds its Preflighted behind a Mutex so complete(&self) can adopt a re-verified binary; the lock is released before the model call, so only a re-verification serializes callers"
  - "A changed binary that fails reverify is not adopted: the provider keeps the preflighted binary, and the next call re-resolves and re-verifies again"
  - "The call directory is closed explicitly; a removal failure is reported as InfraCause::WorkDir with the call's usage instead of being dropped silently, and a provider stop from the same call still wins"
  - "Each call's directory reuses the preflight's minos-claude- prefix (RUN_DIR_PREFIX made pub(crate)), so every claude process runs under one naming rule"
  - "ClaudeCli's Debug uses try_lock and prints only the version and binary path, or 'in use' / 'poisoned'"

patterns-established:
  - "Provider-agnostic proof: one generic propose_with<P: ModelProvider> helper drives Proposer over ClaudeCli and FakeProvider with the same structured output, and the Proposals compare equal"
  - "Fake claude -p branch records argv one per line and compares it to argv(model, schema) as exact bytes, so an empty --tools value and a multi-line schema are both checked"

requirements-completed: [PROP-01, PROP-02, PROP-04, PROP-08, PROP-09]

coverage:
  - id: D1
    description: "ClaudeCli::complete returns Completion::Structured, the envelope's token usage, and resolved_model claude-haiku-5-5; Debug shows the version and never account data"
    requirement: PROP-08
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs a_call_returns_the_structured_output_its_usage_and_the_resolved_model"
        status: pass
    human_judgment: false
  - id: D2
    description: "The call runs argv(model, schema) exactly, with exactly six variables, the prompt bytes on stdin, in a fresh empty minos-claude-* directory directly under the canonical TMPDIR that is gone afterwards"
    requirement: PROP-02
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs a_call_runs_the_golden_argv_with_six_variables_and_the_prompt_on_stdin_in_a_fresh_directory"
        status: pass
    human_judgment: false
  - id: D3
    description: "A 429 envelope (success, is_error true, exit 1) is ProviderError::Stop { Limit, usage: Some(..) }"
    requirement: PROP-09
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs a_429_envelope_is_a_limit_stop_with_usage"
        status: pass
    human_judgment: false
  - id: D4
    description: "Re-resolution: a moved link re-runs --version and auth status once and succeeds; an unmoved link runs no preflight step; an auth refusal is a ProviderAuth stop with no -p run; an old version is Infra Preflight; a removed link is Infra BinaryMissing"
    requirement: PROP-04
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs a_moved_link_* and a_removed_link_is_binary_missing (4 tests)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The same Proposer/single-shot code over ClaudeCli and FakeProvider gives equal Proposals; through ClaudeCli the origin is Origin::Model { claude-haiku-5-5, PromptHash::of(stdin bytes) }"
    requirement: PROP-01
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs the_same_single_shot_code_gives_the_same_proposals_over_claude_cli_and_the_fake, through_claude_cli_the_origin_is_the_resolved_model_and_the_hash_of_the_bytes_sent"
        status: pass
    human_judgment: false
  - id: D6
    description: "CLAUDE.md and .claude/CLAUDE.md describe the provider as built (api_error_status, apiKeySource, minos-claude-, ProviderAuth, {rationale, files, Moot for v1 twice, --bare open question, Phase 6 corrections block before Conventions)"
    requirement: PROP-02
    verification:
      - kind: other
        ref: "Task 3 acceptance greps"
        status: pass
    human_judgment: true

duration: about 12 min
completed: 2026-10-20
---

# Phase 6 Plan 07: ClaudeCli provider call path Summary

**`ClaudeCli` is now a `ModelProvider` that exists only behind its preflight: each call re-resolves the native install's link (re-running `--version` and `auth status` only when the target moved), then runs the golden argv in a fresh 0700 `minos-claude-*` directory with exactly six variables, the prompt on stdin, a 15-minute group-kill deadline, and a 4 MiB stdout cap, and classifies the result; the same single-shot code gives identical proposals over `ClaudeCli` and `FakeProvider`, and CLAUDE.md records the contract as built.**

## Performance

- **Duration:** about 12 min
- **Started:** 2026-10-20T12:23:00Z
- **Completed:** 2026-10-20T12:35:00Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- `ClaudeCli` (`preflight`, `version`, `impl ModelProvider`) with no `Clone` or `Default` and a `Debug` that prints only the version and the binary's path.
- Per-call re-resolution: `locate` before every call; a changed target must pass `reverify` before `replace_located` adopts it; a login refusal is `ProviderStop::Auth`, any other refusal `InfraCause::Preflight`, a vanished binary `InfraCause::BinaryMissing`.
- The fake `claude` gained a `-p` branch, and 7 call-path tests plus 2 end-to-end tests run on the real spawn path (29 tests in `tests/claude_cli.rs`, all 06-06 tests still passing).
- Crate-root exports of `ClaudeCli`, `ParentEnv`, `PreflightError`, `Preflighted`.
- CLAUDE.md "The model stays outside the sandbox" now states the provider as built; the Workspace line, the two SPIKE-07 open questions (moot for v1), and three new open questions are updated; `.claude/CLAUDE.md` has the Phase 6 corrections block.

## Task Commits

1. **Task 1: ClaudeCli typestate and the ModelProvider call path**: `f52506f` (test, RED), `3f6a259` (feat, GREEN)
2. **Task 2: End-to-end single-shot over ClaudeCli and crate-root exports**: `2250bba` (test, RED), `65f36ba` (feat, GREEN)
3. **Task 3: Design of record**: `f163ecb` (docs)

## Files Created/Modified

- `crates/minos-propose/src/claude_cli/mod.rs`: `ClaudeCli`, its `ModelProvider` implementation, and module docs restating the contract.
- `crates/minos-propose/src/claude_cli/preflight.rs`: crate-private `Preflighted::replace_located`; `RUN_DIR_PREFIX` made `pub(crate)`.
- `crates/minos-propose/src/provider.rs`: `InfraCause::WorkDir` doc and message now say "created or removed".
- `crates/minos-propose/src/lib.rs`: crate-root re-exports.
- `crates/minos-propose/tests/claude_cli.rs`: the `-p` branch, `install`/`relink`/`recorded_bytes` fixture helpers, shared `six_variables`/`fresh_directory` checks, and the new tests.
- `CLAUDE.md`, `.claude/CLAUDE.md`: design-of-record updates.

## Decisions Made

See `key-decisions` in the frontmatter. The main one: the `Mutex<Preflighted>` is held only while locating and re-verifying, never across the model call.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Call directory removal failure is reported, not dropped**
- **Found during:** Task 1
- **Issue:** The plan said to drop the `TempDir`, which ignores a removal error, so a leftover directory under `$TMPDIR` would go unnoticed (an empty-catch pattern).
- **Fix:** `complete` calls `TempDir::close`; a failure becomes `InfraCause::WorkDir` carrying the call's usage, unless the call already ended in a provider stop, which stands. `InfraCause::WorkDir`'s doc and message now read "created or removed". `provider.rs` was not in the plan's file list.
- **Files modified:** `crates/minos-propose/src/claude_cli/mod.rs`, `crates/minos-propose/src/provider.rs`
- **Commit:** `3f6a259`

**2. [Rule 3 - Blocking] Clippy pedantic fixes in the test file**
- **Found during:** Tasks 1 and 2
- **Issue:** `needless_pass_by_value` on the `failed` helper and `redundant_closure_for_method_calls` on a `rationale` map.
- **Fix:** `failed` consumes its result through `.err().ok_or_else(..)`, and the map uses `Proposal::rationale`.
- **Files modified:** `crates/minos-propose/tests/claude_cli.rs`
- **Commits:** `3f6a259`, `65f36ba`

**3. [Note] The "copy whose auth status exits 1"**
- The fake reads its control files from `$HOME/fake`, so the test installs a plain copy at 2.1.305 and writes `auth.code` = 1 after the first preflight. The behavior is the same: the re-verification's `auth status` exits 1, and the `-p` branch never runs.

## Verification

- `cargo test -p minos-propose --test claude_cli`: 29 passed.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps`: all pass.
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing.
- Task 1 and Task 3 acceptance greps: pass. `grep -n 'derive(.*Clone' crates/minos-propose/src/claude_cli/mod.rs` prints nothing.
- No test ran the user's own `claude`, made a model call, or set a live-test gate variable; every test uses the fake in a temporary `HOME`.

## Threat Flags

None. The plan's threat register covers every surface added here. T-06-07-02 (re-resolution and reverify), T-06-07-03 (six variables, no credential read, no stderr in errors), and T-06-07-04 (`CALL_DEADLINE`, `MAX_ENVELOPE_BYTES`) are covered by tests against the fake. T-06-07-01's live canary proof belongs to 06-08.

## Known Stubs

None.

## Next Phase Readiness

06-08 can run the ignored live suite against `ClaudeCli::preflight` and `complete`. Phase 7's loop can hold a `Proposer<ClaudeCli>` and map `ProviderError::stop_reason` and `is_infra` to the ledger and the breaker.

## Self-Check: PASSED
