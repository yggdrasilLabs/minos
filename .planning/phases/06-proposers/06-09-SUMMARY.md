---
phase: 06-proposers
plan: 09
subsystem: propose
tags: [claude-cli, live-tests, prop-02, prop-03, evidence, verified-facts]
status: complete

requires:
  - phase: 06-proposers
    plan: 08
    provides: crates/minos-propose/tests/live.rs (three ignored live cases gated by MINOS_CLAUDE_LIVE=1, the hostile fixture)
provides:
  - "06-EVIDENCE.md '## Live provider tests (06-09)': the user's hand run on claude 2.1.304, redacted verbatim output, canary table, what each case proves"
  - "CLAUDE.md Verified facts, local: the claude 2.1.304 bullet citing 06-EVIDENCE.md Live provider tests"
affects: [06-10 phase close]

tech-stack:
  added: []
  patterns:
    - "A live run is recorded with who ran it, the variables unset by name only (here none), the redaction rule applied, and a statement of what each case proves and no more"

key-files:
  created:
    - .planning/phases/06-proposers/06-09-SUMMARY.md
  modified:
    - .planning/phases/06-proposers/06-EVIDENCE.md
    - CLAUDE.md

key-decisions:
  - "Task 1 was resolved by the user's own hand run (2026-10-20); no agent started a live test, set MINOS_CLAUDE_LIVE, ran the user's claude, or ran env -u"
  - "The verbatim output is kept unaltered; because --nocapture splits each 'test <name> ... ok' across lines, the section lists the three results separately"
  - "Tokens: only case (a) prints usage (4475 tokens); (b) and (c) print none, so the three calls' total is stated as not in the output"
  - "The verified fact is scoped to claude 2.1.304 and the fixture; a later CLI version needs a rerun"

patterns-established:
  - "Verified facts, local cites 02-SPIKE-EVIDENCE.md or 06-EVIDENCE.md"

requirements-completed: [PROP-02, PROP-03]

duration: 10min
completed: 2026-10-20
---

# Phase 6 Plan 09: Live provider run recorded Summary

**The user's hand run of the live suite on `claude` 2.1.304 passed all three cases with nothing unset; it is recorded in 06-EVIDENCE.md with home paths redacted, and CLAUDE.md now lists 2.1.304 as a verified fact. Case (b) left every canary absent and case (c) fired all three, so PROP-03 is proven on 2.1.304 for this fixture.**

## Performance

- **Duration:** about 10 min (Task 2; Task 1 was the user's own run)
- **Completed:** 2026-10-20
- **Tasks:** 2 of 2
- **Files modified:** 2

## Accomplishments

- Task 1 (checkpoint:human-action) was resolved by the user. They ran `~/.local/bin/claude --version`, `readlink ~/.local/bin/claude`, and the live suite by hand in their own logged-in terminal. The CLI was 2.1.304 at `~/.local/share/claude/versions/2.1.304`. The output read `test result: ok. 3 passed; 0 failed`, then `exit: 0`. No variable was unset with `env -u`, and the preflight refused nothing.
- The orchestrator checked the output against Task 1's verification block, and I re-checked it here. All three tests are `ok`. Cases (a) and (b) left `canary-session-start`, `canary-user-prompt`, and `canary-mcp` absent, and (b) shows the marker as `false`. Case (c) fired all three canaries, and its marker shows `true`.
- 06-EVIDENCE.md now has `## Live provider tests (06-09)`. It records who ran the suite, the CLI version and link target, the exact commands, that nothing was unset, the exit code, and the verbatim output with the redaction rule applied (only the three home paths changed to `~`; the output held no account values). It also lists each result line, has a canary and marker table, and records the tokens: 4475 for (a), with no usage printed for (b) or (c). A final list states what each case proves.
- In CLAUDE.md, "Verified facts, local" is now dated 2026-10-20. Its intro cites 02-SPIKE-EVIDENCE.md or 06-EVIDENCE.md, and a new 2.1.304 bullet follows the kept 2.1.296 line. Every other section is unchanged byte for byte, checked with `cmp`.

## Task Commits

1. **Task 1: The user runs the live suite**: no commit. The user ran it by hand, and its output is the input to Task 2.
2. **Task 2: Record the live run and update the verified facts**: `95eaba0` (docs)

## Files Created/Modified

- `.planning/phases/06-proposers/06-EVIDENCE.md`: adds the live provider tests section
- `CLAUDE.md`: the Verified facts, local heading date, the intro, and the 2.1.304 bullet

## Decisions Made

- The verbatim output stays unaltered. Under `--nocapture` each test's printed lines sit between its name and its `ok`, so the literal `test <name> ... ok` strings never appear on one line. The section lists the three results separately and says why.
- The record keeps the cases' roles apart. Case (a) is the end-to-end production-provider check (PROP-02), not cwd-isolation evidence, because `ClaudeCli` always runs the CLI in its own fresh `minos-claude-*` directory. Cases (b) and (c) together carry PROP-03. The fact is scoped to 2.1.304 and this fixture.

## Deviations from Plan

None in substance; the plan ran as written. The one presentational addition is the list of result lines described under Decisions Made. The acceptance criterion's literal strings needed it, and the verbatim output was left as it was.

## Issues Encountered

- The `gsd-tools query state.record-metric` and `state.add-decision` positional forms refused their arguments. I used the flag forms (`--phase`, `--plan`, `--duration`, `--summary`) instead. Tooling only; no project impact.

## Verification

- `grep -n '^## Live provider tests (06-09)' .planning/phases/06-proposers/06-EVIDENCE.md` prints line 47, and `grep -n '06-EVIDENCE.md Live provider tests' CLAUDE.md` prints line 570.
- The section contains each of the three `test <name> ... ok` strings once. It holds no home path, email, organization, or account value, and names no variable value.
- `cargo test -p minos-propose --test live 2>&1 | grep -E '3 ignored'` prints `test result: ok. 1 passed; 0 failed; 3 ignored`. `MINOS_CLAUDE_LIVE` was not set, so no model call was made.
- Plan verification: the user ran the suite on their logged-in machine and unset nothing. The redacted output is in 06-EVIDENCE.md, and the CLAUDE.md bullet cites it.

## Next Phase Readiness

- Plan 06-10 closes the phase: the final gate, the requirements, the validation contract, and `cargo clean`.

## Self-Check: PASSED

- FOUND: .planning/phases/06-proposers/06-EVIDENCE.md (section at line 47)
- FOUND: CLAUDE.md (bullet at line 570)
- FOUND: commit 95eaba0
