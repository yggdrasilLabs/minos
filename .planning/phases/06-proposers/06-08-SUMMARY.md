---
phase: 06-proposers
plan: 08
subsystem: propose
tags: [claude-cli, live-tests, prop-03, hostile-fixture, positive-control, design-of-record]
status: complete

requires:
  - phase: 06-proposers
    plan: 07
    provides: ClaudeCli (preflight, version, ModelProvider), crate-root ParentEnv, Proposer, argv, classify, locate
provides:
  - "crates/minos-propose/tests/live.rs: three ignored live cases gated by MINOS_CLAUDE_LIVE=1, the hostile fixture, and one model-free fixture check"
  - "CLAUDE.md Testing bullet and Commands entry for the live suite"
affects: [06-09 live-run checkpoint, 06-10 phase close]

tech-stack:
  added: []
  patterns:
    - "Live gate: every live test first calls live_gate(), which fails with the run command unless MINOS_CLAUDE_LIVE is exactly 1"
    - "Hostile fixture in a canonical TempDir: shell-form command hooks (single-quoted absolute canary paths, a quote in the root refused) and an argv-form .mcp.json stdio server"
    - "Negative canary checks wait CANARY_SETTLE (2 s) after the CLI exits, so a late hook or server cannot race the assertion"

key-files:
  created:
    - crates/minos-propose/tests/live.rs
    - .planning/phases/06-proposers/06-08-SUMMARY.md
  modified:
    - CLAUDE.md

key-decisions:
  - "Hook entries use the documented shell form (command string, no args) so the positive control fires on any CLI version that runs hooks; the exec form (args) is documented but its first version is not, and the installed CLI could not be queried without running it"
  - "The marker check scans the whole stdout envelope ASCII case-insensitively in (b) and (c), and every rationale in (a), which is stricter than checking parsed rationales only"
  - "The control's argv filter fails unless exactly four elements were removed, so a changed golden argv cannot leave an isolation flag in the control"
  - "Direct runs (b, c) discard the CLI's stderr, cap stdout at MAX_ENVELOPE_BYTES, and kill the process group at CALL_DEADLINE, mirroring the production runner's bounds"

patterns-established:
  - "A model-free, non-ignored test in a live test file checks the fixture's shape so a hand-run live session is never spent on a broken fixture"

requirements-completed: [PROP-02, PROP-03]

coverage:
  - id: D1
    description: "Three ignored live tests exist, each failing closed unless MINOS_CLAUDE_LIVE is 1; production code never reads the variable"
    requirement: PROP-03
    verification:
      - kind: integration
        ref: "cargo test -p minos-propose --test live: 1 passed, 3 ignored; grep -rn MINOS_CLAUDE_LIVE crates/minos-propose/src prints nothing"
        status: pass
    human_judgment: false
  - id: D2
    description: "The fixture's hooks, MCP server, and CLAUDE.md are well formed and name the canary paths exactly; the control argv drops exactly the three isolation flags and the --mcp-config value"
    requirement: PROP-03
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/live.rs the_fixture_and_the_control_argv_are_well_formed"
        status: pass
    human_judgment: false
  - id: D3
    description: "Case (a) proves the production provider end to end, cases (b) and (c) carry the PROP-03 proof on the user's real install"
    requirement: PROP-02
    verification:
      - kind: manual
        ref: "06-09 blocking checkpoint: MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1"
        status: pending
    human_judgment: true
  - id: D4
    description: "CLAUDE.md Testing and Commands describe the live suite, its gate variable, and its run command"
    requirement: PROP-03
    verification:
      - kind: other
        ref: "Task 2 acceptance greps (3 MINOS_CLAUDE_LIVE lines, the run command, positive control)"
        status: pass
    human_judgment: false

duration: about 15 min
completed: 2026-10-20
---

# Phase 6 Plan 08: Live provider suite Summary

**`crates/minos-propose/tests/live.rs` holds the ignored PROP-03 suite, gated by `MINOS_CLAUDE_LIVE=1`: (a) `Proposer<ClaudeCli>` end to end from a hostile working directory, (b) the golden argv with a hostile project as the CLI's own working directory, asserting no hook, MCP, or CLAUDE.md effect, and (c) a positive control without the isolation flags that must fire a hook canary; CLAUDE.md records how to run it.**

## Performance

- **Duration:** about 15 min
- **Started:** 2026-10-20T12:28:00Z
- **Completed:** 2026-10-20T12:44:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- The hostile fixture: `.claude/settings.json` with `SessionStart` and `UserPromptSubmit` command hooks (`/usr/bin/touch '<root>/canary-session-start'`, `canary-user-prompt`), a `.mcp.json` stdio server `minos-canary` (`/usr/bin/touch`, args `[<root>/canary-mcp]`), and a `CLAUDE.md` demanding `MINOSCANARYWORD` in every rationale, in the shapes the hooks and MCP references document.
- Case (a) `live_production_provider_ignores_hostile_project`: preflight and the single-shot path from inside the fixture, with the caller's directory restored before any assertion; asserts at least one proposal, every diff touching only `src/k.txt`, total tokens above zero, no canary, no marker; prints the version, counts, usage, origin model (through `SafeText::inline`), and prompt hash.
- Case (b) `live_golden_argv_in_hostile_cwd`: the canonical binary from `locate`, `argv(haiku, OutputSchema::for_batch(2, writable))`, the six-variable environment, the fixture as cwd; asserts exit 0, `Completion::Structured`, no canary, no marker anywhere in the envelope.
- Case (c) `live_positive_control_fixture_is_live`: the same call with the three isolation flags removed; asserts a hook canary exists and prints which of the three fired and whether the marker appeared.
- CLAUDE.md Testing has the provider-tests bullet; Commands has the run command and the sentence that `MINOS_CLAUDE_LIVE` is read only by ignored tests.

## Task Commits

1. **Task 1: Ignored live suite with the hostile fixture and positive control**: `5a6e45c` (test)
2. **Task 2: CLAUDE.md Testing and Commands entries**: `39a7c5d` (docs)

## Files Created/Modified

- `crates/minos-propose/tests/live.rs`: the live gate, the fixture, the bounded direct runner, the three ignored cases, and one model-free fixture check.
- `CLAUDE.md`: the Testing bullet, the Commands line, and one prose sentence; every other section unchanged.

## Decisions Made

See `key-decisions` in the frontmatter. The main one: the hooks use the documented shell form with single-quoted absolute paths (a root containing a quote is refused), because the exec form's first supported version is not documented and the installed CLI could not be checked without running it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] A model-free fixture check in the gate**
- **Found during:** Task 1
- **Issue:** The three cases are ignored, so nothing in the gate proved the fixture's manifest parses, its JSON files name the canary paths, or the control's argv filter removes exactly the isolation flags. A broken fixture would only show up during the user's live run at 06-09, and a broken filter could leave an isolation flag in the control.
- **Fix:** Added `the_fixture_and_the_control_argv_are_well_formed`, which builds the fixture, parses the manifest, reads back the hook commands and the MCP args, checks the marker sits only in `CLAUDE.md`, and checks the control argv. It runs no process, reads no environment variable, and makes no model call. `cargo test -p minos-propose --test live` therefore reports `1 passed; 3 ignored`; the plan's `3 ignored` check still holds.
- **Files modified:** `crates/minos-propose/tests/live.rs`
- **Commit:** `5a6e45c`

**2. [Rule 2 - Missing critical functionality] Bounded direct runs and a settle delay**
- **Found during:** Task 1
- **Issue:** The plan's direct runs (b, c) had no deadline, and an immediate canary check could miss a hook or server that wrote its file just after the CLI exited.
- **Fix:** The direct runner puts the CLI in its own process group, discards stderr, caps stdout at `MAX_ENVELOPE_BYTES`, and kills the group at `CALL_DEADLINE`. Every case waits 2 s (`CANARY_SETTLE`) before checking the canaries.
- **Files modified:** `crates/minos-propose/tests/live.rs`
- **Commit:** `5a6e45c`

## Verification

- `cargo test -p minos-propose --test live`: 1 passed, 3 ignored. No live case ran.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`: all pass.
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing.
- Task 1 greps: 3 `#[ignore = ` lines; `MINOS_CLAUDE_LIVE` in the test file and absent from `src`; each canary name present.
- Task 2 greps: the run command line, 3 `MINOS_CLAUDE_LIVE` lines, the `positive control` bullet.
- No live test was run with `--ignored`, `MINOS_CLAUDE_LIVE` was never set, and the user's `claude` was never run.

## Known Stubs

None.

## Threat Flags

None. T-06-08-01 (the control's hooks only touch canaries in the test's own temporary directory; a quoted root is refused), T-06-08-02 (all three cases ignored and gated), T-06-08-03 (case (c) asserts a hook canary fires; the docs say (b) and (c) carry PROP-03), and T-06-08-04 (model strings through `SafeText::inline`, stderr discarded, no `auth status` field printed) are covered.

## Next Phase Readiness

06-09 can hand the run to the user at its blocking checkpoint with `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1`. If (c) fires no canary, that is a finding to report, not a reason to change (a) or (b). Case (c) runs without the isolation flags, so the user's own user-level hooks and MCP servers may start during it.

## Self-Check: PASSED
