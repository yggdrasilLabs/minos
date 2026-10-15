---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 04
subsystem: source
tags: [minos-source, denylist, secret-scan, regex, git, clean-check, CLI-11]

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    provides: "04-01 minos-source crate skeleton, its dependency edges (regex, tempfile), and its clippy.toml bans"
  - phase: 03-core-types-and-pure-logic
    provides: "minos-core RelPath"
provides:
  - "DenyRule, DENYLIST, deny_component: the fixed D-07 hard denylist, ASCII case-insensitive"
  - "Scanner, SecretHit, ScanError, PATTERN_NAMES: nine anchored linear-time patterns, location-only hits"
  - "CleanPolicy, Dirty, check_clean: read-only git status plus ls-files subset check"
  - "CLAUDE.md Source paragraph and Code rules record the read-only git carve-out"
affects: [04-07, 04-09, phase-8-run]

tech-stack:
  added: []
  patterns:
    - "Secret-shaped test vectors are joined at run time; a self-scan test proves the crate's own sources hold none"
    - "Subprocess with env_clear plus an exact allowlist, scoped-thread capped pipe readers, try_wait deadline, and a Drop reaper"
    - "A private run_capped(program, root, args, deadline, cap) lets the timeout and cap paths be unit-tested with /bin/sleep and /usr/bin/yes"

key-files:
  created:
    - crates/minos-source/src/denylist.rs
    - crates/minos-source/src/scan.rs
    - crates/minos-source/src/clean.rs
    - crates/minos-source/tests/denylist.rs
    - crates/minos-source/tests/scan.rs
    - crates/minos-source/tests/clean.rs
  modified:
    - crates/minos-source/src/lib.rs
    - CLAUDE.md

key-decisions:
  - "04-04: DENYLIST is a public [DenyRule; 9]; each rule's name pattern lives on a private DenyRule::name_pattern, because a public const cannot expose the private NamePattern type"
  - "04-04: every git failure, including a failed read of git's output, maps to a Dirty variant; Dirty::GitFailed { code: None } covers a signal, a wait error, or an unreadable pipe"
  - "04-04: CLI-11 stays unchecked; the guards exist but the packer that composes them is 04-07"

patterns-established:
  - "Location-only reporting: SecretHit holds path, line, and pattern name, never matched bytes; Display is <path>:<line>: matches <pattern>"

requirements-completed: []

coverage:
  - id: D1
    description: "Hard denylist: nine name rules with reasons, ASCII case-insensitive, look-alikes not denied"
    requirement: "CLI-11"
    verification:
      - kind: test
        ref: "crates/minos-source/tests/denylist.rs"
        status: pass
    human_judgment: false
  - id: D2
    description: "Secret scan: hit and near miss per pattern, binary bytes, sorted location-only hits, own-source self-scan"
    requirement: "CLI-11"
    verification:
      - kind: test
        ref: "crates/minos-source/tests/scan.rs#own_sources_have_no_secrets"
        status: pass
      - kind: test
        ref: "crates/minos-source/src/scan.rs#pattern_names_list_every_pattern_in_order"
        status: pass
    human_judgment: false
  - id: D3
    description: "Clean check: clean, untracked, modified, staged, non-repository, info/exclude-only, subdirectory project, timeout, cap, exit code, env allowlist"
    requirement: "CLI-11"
    verification:
      - kind: test
        ref: "crates/minos-source/tests/clean.rs"
        status: pass
      - kind: test
        ref: "crates/minos-source/src/clean.rs#tests"
        status: pass
    human_judgment: false
  - id: D4
    description: "CLAUDE.md records the read-only git carve-out and its reason"
    verification:
      - kind: other
        ref: "grep -n 'The clean check is the one place Minos runs git' CLAUDE.md"
        status: pass
    human_judgment: false

duration: 5min
completed: 2026-10-15
status: complete
---

# Phase 4 Plan 04: Hard denylist, secret scan, and read-only git clean check Summary

**Three standalone snapshot guards in `minos-source`: the hard denylist (a fixed table of nine name rules with reasons, ASCII case-insensitive), a secret scanner over nine anchored `regex::bytes` patterns that reports path, pattern, and line but never the bytes, and a git clean check. The clean check runs git with a cleared environment and treats packed-but-untracked files and every git failure as dirty.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-10-15T14:10:12Z
- **Completed:** 2026-10-15T14:14:41Z
- **Tasks:** 3
- **Files modified:** 8 (6 created, 2 modified)

## Accomplishments

- `denylist.rs`: `DenyRule` (`GitDir`, `MinosDir`, `ClaudeDir`, `AwsDir`, `SshDir`, `EnvFile`, `PemFile`, `SshKey`, `MinosLock`, `JudgePath`), each with `pattern()` and `reason()`. `DENYLIST` holds the nine name rules. `deny_component(&[u8])` matches each component as an exact name, a prefix, or a suffix with `eq_ignore_ascii_case`, using only `get` and `checked_sub`, never indexing. `JudgePath` is left to the walk in 04-07.
- `scan.rs`: a `Scanner` makes one `RegexSet` pass, then runs `find_iter` for each matched pattern and counts lines incrementally. `SecretHit { path, line, pattern }` derives `Ord` in that field order, so hits sort by path, then line, then pattern. `scan(&self, path, bytes)` takes no exemption parameter, so no pattern can be switched off.
- `clean.rs`: `check_clean(root, packed)` runs `STATUS_ARGS`. Any output means `Changes`. It then runs `TRACKED_ARGS`, and any packed path missing from the tracked set means `Untracked { count, shown }` (at most 20 shown, sorted). The environment is exactly `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `LC_ALL=C`, `GIT_CONFIG_NOSYSTEM=1`, and `GIT_CONFIG_GLOBAL=/dev/null`. Each run has a 30 s deadline with kill and reap, 16 MiB stdout and 64 KiB stderr caps read on scoped threads, and a `Drop` reaper. stderr is drained and never shown.
- CLAUDE.md: the Source paragraph records the carve-out, its environment, the ls-files subset check, and its reason. The Code rules "No git in the trusted path" bullet names the exception.

## Task Commits

1. **Task 1: The hard denylist and the secret scanner**: `92eaf57` (test, RED), `7d01ccd` (feat, GREEN)
2. **Task 2: The read-only git clean check**: `59c44f0` (test, RED), `d92ec19` (feat, GREEN)
3. **Task 3: CLAUDE.md records the read-only git carve-out**: `cf5e627` (docs)

## Files Created/Modified

- `crates/minos-source/src/denylist.rs`: `DenyRule`, private `NamePattern`, `DENYLIST`, `deny_component`
- `crates/minos-source/src/scan.rs`: `PATTERNS`, `PATTERN_NAMES`, `Scanner`, `SecretHit`, `ScanError`, plus a unit test that ties the names to the patterns
- `crates/minos-source/src/clean.rs`: git constants and argv, `CleanPolicy`, `Dirty`, `check_clean`, private `run_git`, `run_capped`, `wait_until`, and `Reaper`, with 6 unit tests
- `crates/minos-source/src/lib.rs`: `pub mod clean, denylist, scan` and the root re-exports
- `crates/minos-source/tests/denylist.rs` (5 tests), `tests/scan.rs` (9 tests), `tests/clean.rs` (10 tests)
- `CLAUDE.md`: Source paragraph carve-out; one sentence in Code rules

## Decisions Made

- `DENYLIST` is `[DenyRule; 9]`. The plan described an array of `(DenyRule, NamePattern)` pairs, but a `pub const` cannot carry the private `NamePattern` type. Each rule's pattern therefore lives in a private `const fn name_pattern`, which `deny_component` reads. The match arms still hold every `Exact`, `Prefix`, and `Suffix`.
- `D-07`'s `id_*` is implemented exactly as locked. It also matches names like `id_map.rs` (RESEARCH Pitfall 6, Open Question 4). 04-07 reports every denylist drop, so nothing vanishes silently, but the user should confirm this pattern.
- A failed join or read of git's stdout maps to `Dirty::GitFailed { code: None }`, so every failure is dirty and none is an unclassified error.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] DENYLIST shape**
- **Found during:** Task 1
- **Issue:** A public const array of `(DenyRule, NamePattern)` would leak a private type into the public API, which does not compile.
- **Fix:** `DENYLIST: [DenyRule; 9]` plus a private `DenyRule::name_pattern`. Behavior and the table's contents are unchanged.
- **Files modified:** crates/minos-source/src/denylist.rs
- **Commit:** 7d01ccd

**2. [Rule 2 - Missing critical test coverage] Timeout, cap, exit-code, and environment paths tested**
- **Found during:** Task 2
- **Issue:** The plan's tests reach the timeout (T-04-04-05) and the output cap only through a real git run, which cannot be made slow or large in a test.
- **Fix:** `run_git` delegates to a private `run_capped(program, root, args, deadline, stdout_cap)`. Unit tests drive it with `/bin/sleep` (killed after 100 ms, so `TimedOut`), `/usr/bin/yes` (`OutputTooLarge`, finished in well under 5 s because the closed pipe stops it), `/usr/bin/false` (`GitFailed { code: Some(1) }`), and `/usr/bin/env`, which shows that the child sees exactly the four allowlisted variables. None of them uses a shell.
- **Files modified:** crates/minos-source/src/clean.rs
- **Commit:** d92ec19

**Other small choices:** `STATUS_ARGS` and `TRACKED_ARGS` are fixed-size `[&str; N]` arrays. The git constants are `pub`, so the docs can link them. `SecretHit` stores its fields in the order path, line, pattern, so the derived `Ord` gives the required sort.

**Total deviations:** 2 auto-fixed (1 blocking, 1 added coverage). **Impact:** none on the plan's interfaces or invariants.

## Issues Encountered

- Two clippy pedantic findings in Task 1 were fixed in the GREEN commit: `explicit_iter_loop`, and `naive_bytecount`, which is now a `fold`. A third, `case_sensitive_file_extension_comparisons`, appeared in the self-scan test and was fixed by switching to `Path::extension`.
- The known flaky `minos-backend-krun` supervise test did not fail. The full workspace run passed: 794 passed, 0 failed, 25 ignored.

## Known Limitations

- The deadline kills the `git` process itself, not a process group (`minos-source` has no `rustix` edge, and `Cargo.toml` is frozen for this phase). `ls-files --recurse-submodules` starts child `git` processes for submodules. If one of them outlives a killed parent, it keeps the stdout pipe open until it finishes on its own, so the deadline could overrun by that much. It cannot hang forever on a finite tree, and the result is still `TimedOut`, so the tree is dirty. If this matters, a later plan can add process-group kill.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 794 passed, 0 failed, 25 ignored
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`
- `cargo doc --workspace --no-deps`: exit 0, no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no member added, so unchanged from 04-01
- C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing
- Every acceptance grep in the plan prints its expected line or count. `sh -c|"/bin/sh"|"bash"` in clean.rs counts 0.

## Next Phase Readiness

04-07 can compose `deny_component`, `Scanner::scan`, and `check_clean` under `CleanPolicy` into `snapshot`, the only producer of a tarball. It still needs to apply `DenyRule::JudgePath` to the paths the manifest's judge command names.

## Self-Check: PASSED

- FOUND: crates/minos-source/src/denylist.rs, src/scan.rs, src/clean.rs, tests/denylist.rs, tests/scan.rs, tests/clean.rs
- FOUND: commits 92eaf57, 7d01ccd, 59c44f0, d92ec19, cf5e627
