---
phase: 06-proposers
plan: 10
subsystem: propose
tags: [phase-close, final-gate, validation, requirements, gate-06]
status: complete

requires:
  - phase: 06-proposers
    plan: 09
    provides: "06-EVIDENCE.md '## Live provider tests (06-09)': three passing live cases on claude 2.1.304, canaries absent in (b), all three fired in (c)"
provides:
  - "06-EVIDENCE.md '## Final gate': six gate commands, C-01, C-02 green on HEAD a114940; live target 1 passed, 3 ignored (no model call)"
  - "REQUIREMENTS.md: PROP-08 and PROP-09 notes naming what Phase 7's loop appends; footer for the Phase 6 final gate; GATE-06 left cross-cutting"
  - "06-VALIDATION.md filled: status complete, nyquist_compliant and wave_0_complete true, every row green, every --test target checked against the real files"
  - "cargo clean after the final gate (GATE-06), recorded in 06-EVIDENCE.md"
affects: [phase 7 runner and loop, /gsd-verify-work for phase 6]

tech-stack:
  added: []
  patterns:
    - "Phase close: final gate on the final HEAD, then requirements, then the validation contract, then cargo clean as the last cargo command"

key-files:
  created:
    - .planning/phases/06-proposers/06-10-SUMMARY.md
  modified:
    - .planning/phases/06-proposers/06-EVIDENCE.md
    - .planning/REQUIREMENTS.md
    - .planning/phases/06-proposers/06-VALIDATION.md

key-decisions:
  - "PROP-01 to PROP-10 were already checked and Complete before this plan; the checkboxes were left as found and only the PROP-08/PROP-09 notes and the footer changed"
  - "The ignored VM tests (--test vm, --test exit) were not rerun: Phase 6 changed only minos-core and minos-propose (git diff fa19cbd~1..HEAD -- crates)"
  - "cargo clean runs after the SUMMARY/STATE/ROADMAP commit; its record goes into 06-EVIDENCE.md and 06-VALIDATION.md in a git-only commit afterwards, with no cargo command after the clean"

patterns-established:
  - "The final gate lists every test binary with its verbatim result line, and names each ignored test group, so the live target's 3 ignored is visible"

requirements-completed: [PROP-01, PROP-02, PROP-03, PROP-04, PROP-05, PROP-06, PROP-07, PROP-08, PROP-09, PROP-10]

duration: 10min
completed: 2026-10-20
---

# Phase 6 Plan 10: Phase close (final gate, requirements, validation, cargo clean) Summary

**On HEAD `a114940`, all six gate commands, C-01, and C-02 pass: `cargo test --workspace` ran 1357 passed, 0 failed, 23 ignored over 77 result lines. The live target reports `1 passed; 3 ignored`, so the gate made no model call. Requirements PROP-01 to PROP-10 stand complete on the strength of this record and 06-09's live run. The validation contract is filled and checked against the real test targets, and `cargo clean` closes the phase.**

## Performance

- **Duration:** about 10 min
- **Started:** 2026-10-20T13:05Z (approx.)
- **Completed:** 2026-10-20
- **Tasks:** 2 of 2
- **Files modified:** 3 (plus this summary)

## Accomplishments

- Precondition checked: 06-EVIDENCE.md "## Live provider tests (06-09)" records the user's own run, with all three cases passing, every canary absent in (b), and all three firing in (c). No agent ran a live test, set `MINOS_CLAUDE_LIVE`, or ran the user's `claude` in this plan.
- Final gate on HEAD `a114940` with `TRYBUILD` and `MINOS_CLAUDE_LIVE` unset (rustc 1.97.1, macOS 27.0.1). Every command exited 0:
  - `cargo fmt --all --check`, with no output.
  - `cargo clippy --workspace --all-targets -- -D warnings`.
  - `cargo test --workspace`: 1357 passed, 0 failed, 23 ignored. The ignored tests are 17 krun VM tests, 2 CLI host tests, 1 in sandbox `output_tar`, and the 3 live cases.
  - `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`, with the same seven duplicate-crate warnings as Phase 5.
  - `cargo doc --workspace --no-deps`, with no warning.
  - `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`.
  - C-01 printed eleven `ok` lines, and C-02 printed nothing.
- The only non-`--test` command in the contract, `cargo test -p minos-propose --lib claude_cli::process`, selects ten real tests, and all pass.
- REQUIREMENTS.md: PROP-08 now notes that Phase 6 returns usage per call and per model for the loop's `Spent` event (LOOP-04). PROP-09 now notes the provider-stop mapping to `StopReason::ProviderLimit`/`ProviderAuth`, with the loop appending `StopRequested` (LOOP-08). The footer records the Phase 6 final gate and keeps the earlier update lines. GATE-06 stays unchecked because it is cross-cutting.
- 06-VALIDATION.md:
  - Every per-task row is green; the 06-10 Task 2 row turns green in the post-clean record commit.
  - Every File Exists cell is ✅.
  - The requirement-to-test table is confirmed against the final gate's counts. No target was renamed or moved.
  - The Wave 0 items are ticked with where each landed: the dependencies in 06-02, self-contained targets, the three propose snapshots in 06-02, 06-03, and 06-04, and the core golden line in 06-01.
  - The manual live item records that the user ran it on 2026-10-20 on `claude` 2.1.304.
  - The sign-off is ticked, and the frontmatter is `status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`.
- Every doc-grep command in the per-task map was rerun and passes, including `cargo test -p minos-propose --test live | grep '3 ignored'`.

## Task Commits

1. **Task 1: Final gate, requirements, and the validation contract**: `251abac` (docs)
2. **Task 2: cargo clean (GATE-06)**: runs after the SUMMARY/STATE/ROADMAP commit. Its record (the 06-EVIDENCE.md line and the 06-10 Task 2 row turned green) is committed right after it, as `docs(06-10): record cargo clean after the final gate (GATE-06)`. That commit is git-only, and no cargo command runs after the clean.

## Files Created/Modified

- `.planning/phases/06-proposers/06-EVIDENCE.md`: `## Final gate` (and, after the clean, the cargo clean line)
- `.planning/REQUIREMENTS.md`: PROP-08/PROP-09 notes, footer
- `.planning/phases/06-proposers/06-VALIDATION.md`: filled contract
- `.planning/phases/06-proposers/06-10-SUMMARY.md`: this file

## Decisions Made

- The PROP checkboxes were already `[x]` and Complete before this plan, set by earlier plans as noted in 06-05's summary. They were left as found. The plan's ordering guard was still met, because "Live provider tests (06-09)" and "Final gate" now both exist with passing runs.
- The VM suites were not rerun. Phase 6's crate changes are confined to `minos-core` (4 files) and `minos-propose` (38 files). The Final gate section records them as not run and points to the Phase 5 final gate.

## Deviations from Plan

### Ordering

**1. [Ordering] cargo clean before its record commit, after the SUMMARY/STATE/ROADMAP commit**
- **Issue:** The plan's Task 2 runs `cargo clean` and then writes its record. The executor instructions require `cargo clean` after the final SUMMARY/STATE/ROADMAP commit, with no cargo command after it.
- **Resolution:** SUMMARY, STATE, and ROADMAP are committed first. Then `cargo clean` runs, and then a git-only commit records it in 06-EVIDENCE.md and 06-VALIDATION.md. A clean can't be recorded honestly before it runs, and no cargo command follows it, so GATE-06's rule holds. Phase 5 (05-20) committed its clean record the same way.

### Notes

- `grep -n 'nyquist_compliant: true' 06-VALIDATION.md` prints two lines: the frontmatter and the sign-off checklist item. That item is template text, and 04- and 05-VALIDATION.md keep it too. The frontmatter line is the one the criterion means.
- PROP-08's note cites LOOP-04 as the plan specifies. No LOOP requirement names `Spent` by name; LOOP-04 is the loop's generation driver.
- Line 41 of 06-EVIDENCE.md, in a section an earlier plan wrote, still shows an unredacted home path in a `Checking minos-propose` line. It is out of scope for this plan and was left unchanged. The Final gate section holds no home path.

## Issues Encountered

None. Every gate passed on the first run, and no code changed.

## Known Stubs

None (documentation-only plan).

## Next Phase Readiness

Phase 6 is ready for `/gsd-verify-work`. Phase 7 (runner and loop) consumes `Proposer<P>`, the provider stops (`ProviderLimit`/`ProviderAuth`), and the per-call usage. The loop appends `Spent` and `StopRequested` from these.

## Self-Check: PASSED

- FOUND: .planning/phases/06-proposers/06-10-SUMMARY.md
- FOUND: .planning/phases/06-proposers/06-EVIDENCE.md `## Final gate` (line 139)
- FOUND: commit 251abac
- FOUND: 06-VALIDATION.md `nyquist_compliant: true`, `wave_0_complete: true`, `status: complete`
- FOUND: REQUIREMENTS.md 10 `[x] **PROP-` lines, `[ ] **GATE-06**` unchanged
