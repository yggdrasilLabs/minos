---
phase: 06-proposers
plan: 06
subsystem: propose
tags: [claude-cli, subprocess, deadline, process-group, preflight, auth-status, credentials, tdd]
status: complete

requires:
  - phase: 06-proposers
    plan: 02
    provides: limits (PREFLIGHT_DEADLINE, MAX_PREFLIGHT_STDOUT_BYTES, MAX_STDERR_BYTES, POLL_INTERVAL), InfraCause
  - phase: 06-proposers
    plan: 04
    provides: ParentEnv, EnvError, ClaudeVersion, MIN_CLAUDE_VERSION, VERSION_ARGS, AUTH_STATUS_ARGS, Exit, locate, Located, LocateError
provides:
  - "crate-private claude_cli::process: RunSpec, RunOutput (exit, stdout, stderr_tail), RunError (Spawn, Wait, Io, Deadline, StdoutTooLarge; cause), run"
  - "minos_propose::claude_cli::preflight: preflight, Preflighted (located, child_env, home, work_root), PreflightError (15 variants; is_auth), PreflightStep (Version, AuthStatus), crate-private reverify"
  - "tests/claude_cli.rs: fake-claude native-layout HOME fixture and hand-built ParentEnv helper"
affects: [06-07 ClaudeCli provider call path and per-call re-resolution, 06-08 live tests, 08 minos check]

tech-stack:
  added: []
  patterns:
    - "One capped runner for every provider subprocess: waitid(WNOWAIT) polling, group SIGKILL plus reap before any reader join"
    - "Preflighted minted only by preflight, the Cleared<R> analog"
    - "Lenient auth-status struct binding four fields; apiKeySource presence via deserialize_with into IgnoredAny"

key-files:
  created:
    - crates/minos-propose/src/claude_cli/process.rs
    - crates/minos-propose/src/claude_cli/preflight.rs
    - crates/minos-propose/tests/claude_cli.rs
  modified:
    - crates/minos-propose/src/claude_cli/mod.rs

key-decisions:
  - "RunError gained an Io variant (stdout read failure, mapped to InfraCause::Io) so a read error is never silently swallowed"
  - "Version checks run in the order exit 0, parse, file-name match, minimum; a non-zero --version exit is VersionUnreadable with the stderr tail (or stdout) as detail"
  - "auth status stdout must start with '{' before parsing, so a JSON array cannot fill the struct by position"
  - "A work-root ancestor whose marker check fails with an error other than NotFound refuses as WorkRootUnusable: an unknown answer is never read as absent"
  - "Fake-claude environment comparison drops PWD, SHLVL, and _, which /bin/sh itself exports to the programs the script runs; the runner's own env test compares exactly"

patterns-established:
  - "Preflight refusals carry SafeText excerpts of CLI output cut to 256 bytes at a char boundary, never raw bytes"

requirements-completed: [PROP-02, PROP-04]

coverage:
  - id: D1
    description: "Capped runner: stdin, exact env, cwd, exit code, signal, deadline group kill, straggler, stdout cap, stderr tail, spawn failure"
    requirement: PROP-02
    verification:
      - kind: unit
        ref: "crates/minos-propose/src/claude_cli/process.rs tests (10 tests)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Preflight refusals: credential names, environment, locate, work root, version, auth status; Preflighted on a clean subscription login with no PII bound"
    requirement: PROP-04
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs (20 tests)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Each query runs once with exactly the six variables in a fresh empty minos-claude-* directory under the canonical TMPDIR, removed afterwards"
    requirement: PROP-02
    verification:
      - kind: integration
        ref: "crates/minos-propose/tests/claude_cli.rs each_query_runs_once_in_a_fresh_empty_directory_with_six_variables"
        status: pass
    human_judgment: false

duration: about 9 min
completed: 2026-10-20
---

# Phase 6 Plan 06: Capped runner and claude preflight Summary

**Every `claude` subprocess now runs through one capped runner (argv array, cleared environment, own process group, stdin from a thread, capped stdout, capped stderr tail, `waitid(WNOWAIT)` deadline polling, and a group SIGKILL plus reap before any reader join), and a run can obtain a `Preflighted` proof only on the user's own native install at 2.1.296 or newer, logged in with the subscription on the first-party API with no API key source and no credential variable in the parent environment.**

## Performance

- **Duration:** about 9 min (2026-10-20T12:12Z to 12:21Z)
- **Tasks:** 2
- **Files modified:** 4 (3 created)

## Accomplishments

- `process::run` proven on real processes: 100 KiB through `/bin/cat`, exactly `A=1` and `B=2` from `/usr/bin/env`, the canonical cwd from `/bin/pwd -P`, `Exit::Code(3)`, `Exit::Signal(9)`, `Deadline` within 5 s of a 300 ms deadline with the background `sleep` gone (`ESRCH`), a `sleep 30 &` straggler not hanging a run that exits 0, `/usr/bin/yes` ending as `StdoutTooLarge`, 1 MB of stderr kept as a tail of at most the cap with stdout `ok`, and a missing program as `Spawn`.
- `preflight(&ParentEnv)` checks in order: refused variable names (sorted, never values), the six-variable child environment, `locate`, the work root (canonical TMPDIR, refused when it or an ancestor holds `.git`, `CLAUDE.md`, `.mcp.json`, or a `.claude` that is not the canonical HOME), then `--version` and `auth status`, each through the runner with `PREFLIGHT_DEADLINE` and the preflight caps in a fresh `minos-claude-*` tempdir under the work root.
- `auth status` binds only `loggedIn`, `authMethod`, `apiProvider`, and whether `apiKeySource` is present (string or null); exit 1 is `NotLoggedIn`, other exits or a signal `AuthStatusFailed`. `is_auth` marks the four login refusals.
- `Preflighted` has no Clone or Default; its Debug prints the version and path only, and a test proves the email and organization strings never appear.

## Task Commits

1. **Task 1: Capped subprocess runner**: `81c59e6` (test, RED), `d646b3d` (feat, GREEN)
2. **Task 2: Preflight**: `c2df383` (test, RED), `182ca52` (feat, GREEN)

## Files Created/Modified

- `crates/minos-propose/src/claude_cli/process.rs`: `RunSpec`, `RunOutput`, `RunError`, `run`, and its unit tests
- `crates/minos-propose/src/claude_cli/preflight.rs`: `preflight`, `reverify`, `Preflighted`, `PreflightError`, `PreflightStep`
- `crates/minos-propose/src/claude_cli/mod.rs`: `pub mod preflight; mod process;` and the module docs on what touches the host
- `crates/minos-propose/tests/claude_cli.rs`: the fake `claude` fixture and 20 preflight tests

## Decisions Made

See `key-decisions` in the frontmatter.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] `RunError::Io` for a stdout read failure**
- **Found during:** Task 1
- **Issue:** The plan's four unit variants had no way to report a failed stdout read; returning partial bytes would have been a silent swallow.
- **Fix:** Added `RunError::Io`, mapped by `cause()` to the existing `InfraCause::Io`. A stderr read failure still only ends the tail, because stderr is a diagnostic.
- **Files modified:** `crates/minos-propose/src/claude_cli/process.rs`
- **Commit:** `d646b3d`

**2. [Rule 2 - Missing critical functionality] Additional preflight test rows**
- **Found during:** Task 2
- **Issue:** The behavior list did not cover a failing `--version` exit, `loggedIn` false with exit 0, a non-0/1 `auth status` exit, the other project markers, the user's own `$HOME/.claude`, a JSON array, or a missing TMPDIR.
- **Fix:** Added a test for each, so every `PreflightError` variant the fake can reach is exercised.
- **Files modified:** `crates/minos-propose/tests/claude_cli.rs`
- **Commit:** `c2df383`

## Verification

- `cargo test -p minos-propose`: all targets pass (lib 10, claude_cli 20, and every earlier target); `cargo test --workspace` passes
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --workspace --no-deps`, `cargo deny check`: pass
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing
- `grep -rn 'env::var' crates/minos-propose/src`: prints nothing
- `"claude.ai"` and `"firstParty"` each appear on one line of preflight.rs; `email|orgId|orgName` appear only in a doc comment
- Only the fake `claude` and system binaries ran; the user's `claude` was not run and no model call was made

## Threat Flags

None. The runner and preflight are the surfaces T-06-06-01 to T-06-06-06 cover.

## Known Stubs

None.

## Next Phase Readiness

06-07 can build the `ClaudeCli` provider from a `Preflighted`: `process::run` with `command::argv`, `CALL_DEADLINE`, and `MAX_ENVELOPE_BYTES`, `classify` on the output, and `locate` before each call with `reverify` when the located binary differs.
