---
phase: 06-proposers
plan: 04
subsystem: propose
tags: [claude-cli, argv, environment, credentials, classifier, usage, locator, golden, tdd]
status: complete

requires:
  - phase: 06-proposers
    plan: 01
    provides: StopReason::{ProviderLimit, ProviderAuth}
  - phase: 06-proposers
    plan: 02
    provides: ModelResponse, Completion, EmptyCause, ProviderStop, ProviderError, InfraCause, ModelOutput, OutputSchema
provides:
  - "minos_propose::claude_cli::env: ParentEnv (from_vars, refused, home, tmpdir, child_env), EnvError, REFUSED_VARIABLES (13), REFUSED_PREFIX, CHILD_PATH, CHILD_LANG"
  - "minos_propose::claude_cli::command: ClaudeVersion (new, parse, parse_version_output), MIN_CLAUDE_VERSION 2.1.296, VERSION_ARGS, AUTH_STATUS_ARGS, EMPTY_MCP_CONFIG, argv (18 elements)"
  - "minos_propose::claude_cli::classify: Exit (Code, Signal), classify (Pattern 7 table)"
  - "minos_propose::claude_cli::locate: Located (path, version), LocateFailure (10 variants), LocateError (failure, link, versions, target), locate, LINK, VERSIONS_DIR"
  - "ProviderStop::stop_reason, ProviderError::stop_reason"
  - "Golden argv snapshot pinned at 9c421eb32cba7a0126e2dc0926125722fac117edd6da36fcae336682cb8464f6"
affects: [06-06 ClaudeCli preflight and spawn, 06-07, 06-08 live tests, 08 minos check]

tech-stack:
  added: []
  patterns:
    - "Parent environment passed as a value; values other than HOME, USER, LOGNAME, TMPDIR dropped at construction, refused names kept"
    - "Lenient CLI envelope declaring only the fields the table reads; no dollar field declared in any type"
    - "Locator inspects by symlink_metadata before following, then compares canonical parent with the canonical versions directory"

key-files:
  created:
    - crates/minos-propose/src/claude_cli/mod.rs
    - crates/minos-propose/src/claude_cli/env.rs
    - crates/minos-propose/src/claude_cli/command.rs
    - crates/minos-propose/src/claude_cli/classify.rs
    - crates/minos-propose/src/claude_cli/locate.rs
    - crates/minos-propose/tests/argv_golden.rs
    - crates/minos-propose/tests/classify.rs
    - crates/minos-propose/tests/locate.rs
    - crates/minos-propose/tests/snapshots/argv_golden__claude_argv.snap
  modified:
    - crates/minos-propose/src/lib.rs
    - crates/minos-propose/src/provider.rs

key-decisions:
  - "locate checks in a fixed order (link kind, versions dir, link resolution, direct child, file, owner, write bits, execute bit, version name) and refuses at the first failure"
  - "A link whose symlink_metadata fails for any reason other than NotFound is refused as NotALink, never followed"
  - "A dangling link names its own read_link text as the target, the only target there is; every later refusal names the canonical target"
  - "The versions directory must canonicalize to a directory, else VersionsDirMissing"
  - "LINK and VERSIONS_DIR are public consts, so the preflight and doctor output can name the layout without repeating the strings"

patterns-established:
  - "Locator refusals carry the raw HOME-joined link and versions paths, exactly as checked, plus the target when the check got that far"

requirements-completed: [PROP-02, PROP-04, PROP-08, PROP-09]

coverage:
  - id: D1
    description: "Golden 18-element argv, six-variable child environment, refused-name scan, version parsing"
    requirement: PROP-02
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/argv_golden.rs (14 tests, including claude_argv and claude_argv_bytes_pinned)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Credential and redirect variable names refused; values never kept or printed"
    requirement: PROP-04
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/argv_golden.rs (parent snapshot rows)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Pattern 7 result classification with usage, no dollar field, stop-reason mapping"
    requirement: PROP-09
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/classify.rs (39 tests)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Token usage returned with every parsed classification; string-typed dollar fields classify like numeric twins"
    requirement: PROP-08
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/classify.rs"
        status: pass
    human_judgment: false
  - id: D5
    description: "Native-install locator: only the versions-directory symlink layout, owned, not writable, executable, version-named"
    requirement: PROP-02
    verification:
      - kind: unit
        ref: "crates/minos-propose/tests/locate.rs (14 tests)"
        status: pass
    human_judgment: false

duration: about 10h 20min wall clock across two executors (interrupted); about 15 min of active work
completed: 2026-10-20
---

# Phase 6 Plan 04: ClaudeCli pure parts Summary

**The `claude` provider's decisions are pure and tested without spawning anything: the 18-element SPIKE-07 argv under a golden snapshot and SHA-256 pin, a six-variable child environment built from a parent snapshot that keeps credential variable names but never their values, a Pattern 7 result classifier that returns token usage and declares no dollar field, and a locator that accepts only the user's native install under `~/.local/share/claude/versions/`.**

## Performance

- **Duration:** Tasks 1 and 2 plus Task 3 RED by the first executor (2026-10-19T21:41 to 2026-10-20T01:37 local -0400); Task 3 GREEN onward by this executor (2026-10-20 about 07:55 to 08:10 local -0400)
- **Tasks:** 3
- **Files modified:** 11 (9 created)

## Accomplishments

- `command::argv(model, schema)` returns the SPIKE-07 contract as 18 `OsString` elements, with `--model` and the model, and `--json-schema` and the schema, each separate elements and the `--tools` value the empty string. The snapshot `argv_golden__claude_argv.snap` is pinned at `9c421eb3…64f6`.
- `ClaudeVersion::parse` accepts exactly three dot-separated runs of 1 to 9 digits with one text form; `parse_version_output` accepts exactly `X.Y.Z (Claude Code)` with at most one trailing newline; `MIN_CLAUDE_VERSION` is 2.1.296 (D-04).
- `ParentEnv::from_vars` keeps only HOME, USER, LOGNAME, and TMPDIR values and the names of the 13 refused variables plus any `CLAUDE_CODE_USE_*` (D-03 extended). Its Debug prints names and presence only. `child_env` returns HOME, LANG, LOGNAME, PATH, TMPDIR, USER in that order, with PATH `/usr/bin:/bin:/usr/sbin:/sbin` and LANG `en_US.UTF-8`. Nothing in the crate reads the process environment.
- `classify(stdout, exit)` follows the amended D-13 table. A signal is Infra. A non-envelope is `EnvelopeInvalid` with no usage. `success` with `is_error` false needs exit 0, structured output, and usage. `success` with `is_error` true checks 429 for Limit, then 401 and 403 for Auth, then the limit and auth message prefixes (with U+2019 normalized), then `stop_reason` `refusal`, else `ApiError`. Max turns and structured-output retries are empty calls. `error_during_execution` checks the prefixes on `errors`. Budget and unknown subtypes are Infra. Usage rides on every classification whose envelope parsed. `ProviderStop::stop_reason` maps Limit to `ProviderLimit` and Auth to `ProviderAuth` (D-15).
- `locate(home)` accepts `$HOME/.local/bin/claude` only as a symlink, possibly through a chain, whose canonical target is a direct child of the canonical `$HOME/.local/share/claude/versions`. The target must be a regular file owned by `getuid`, with no group or world write bit and at least one execute bit, named `X.Y.Z`. Each refusal states the failure, the link, the versions directory, the target when there is one, and that only the native install runs, with no configuration file and no override (D-01). A moved link yields a different `Located`, which is what the D-01 amendment's per-call re-resolution compares.

## Task Commits

1. **Task 1: Parent snapshot, child environment, golden argv, version parsing**: `e21778c` (test, RED), `394ceab` (feat, GREEN)
2. **Task 2: Result classifier, usage extraction, stop-reason mapping**: `c012523` (test, RED), `502d6db` (feat, GREEN)
3. **Task 3: Native-install locator**: `b9430f9` (test, RED), `213a068` (feat, GREEN)

## Files Created/Modified

- `crates/minos-propose/src/claude_cli/mod.rs`: the module and the provider contract in its docs
- `crates/minos-propose/src/claude_cli/env.rs`: `ParentEnv`, `EnvError`, the refused set, the child PATH and LANG
- `crates/minos-propose/src/claude_cli/command.rs`: `ClaudeVersion`, the minimum, the preflight args, `argv`
- `crates/minos-propose/src/claude_cli/classify.rs`: `Exit`, the lenient envelope, `classify`
- `crates/minos-propose/src/claude_cli/locate.rs`: `Located`, `LocateFailure`, `LocateError`, `locate`
- `crates/minos-propose/src/provider.rs`: `ProviderStop::stop_reason`, `ProviderError::stop_reason`
- `crates/minos-propose/src/lib.rs`: `pub mod claude_cli`
- `crates/minos-propose/tests/argv_golden.rs`, `tests/classify.rs`, `tests/locate.rs`, `tests/snapshots/argv_golden__claude_argv.snap`

## Decisions Made

See `key-decisions` in the frontmatter. All are within the plan's Task 3 specification; the two public consts `LINK` and `VERSIONS_DIR` are an addition to the plan's artifact list.

## Deviations from Plan

### Execution interruption

**1. [Process] Executor terminated mid-plan and continued by a second executor**
- **Found during:** Task 3
- **Issue:** The first executor completed Tasks 1 and 2 (RED and GREEN) and Task 3 RED (`b9430f9`), then was terminated by a transient API timeout before writing `src/claude_cli/locate.rs`.
- **Fix:** A continuation executor verified the five existing commits and that the Task 1 golden snapshot and its pin were in place (they were, and the pin test passes), then implemented Task 3 GREEN against the committed tests unchanged and completed the verification, summary, and state updates.
- **Commit:** `213a068`

### Auto-fixed Issues

**1. [Rule 1 - Bug] clippy `match_same_arms` on the link-kind check**
- **Found during:** Task 3 GREEN, workspace clippy gate
- **Issue:** Separate arms for "not a symlink" and "other I/O error" returned the same refusal.
- **Fix:** Merged into one `Ok(_) | Err(_)` arm with a comment that an unknown kind is refused and never followed.
- **Files modified:** `crates/minos-propose/src/claude_cli/locate.rs`
- **Commit:** `213a068` (fixed before the commit)

## Verification

- `cargo test -p minos-propose`: all targets pass (argv_golden 14, classify 39, locate 14, held_out 2, history 18, parse 27, prompt 13, provider 11, schema 8)
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --workspace --no-deps`, `cargo deny check`: pass
- `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing
- `grep -rn 'env::var' crates/minos-propose/src`: prints nothing
- No process was spawned and the user's `claude` was not run

## Threat Flags

None. The locator adds filesystem inspection of the user's own `HOME`, which T-06-04-06 covers.

## Known Stubs

None.

## Next Phase Readiness

06-06 can wire `locate`, `ParentEnv`, `argv`, `child_env`, the version and auth-status preflight args, and `classify` to a real process; the per-call re-resolution compares `Located` values.

## Self-Check: PASSED

- FOUND: crates/minos-propose/src/claude_cli/locate.rs, env.rs, command.rs, classify.rs, mod.rs
- FOUND: crates/minos-propose/tests/locate.rs, classify.rs, argv_golden.rs, snapshots/argv_golden__claude_argv.snap
- FOUND commits: e21778c, 394ceab, c012523, 502d6db, b9430f9, 213a068
