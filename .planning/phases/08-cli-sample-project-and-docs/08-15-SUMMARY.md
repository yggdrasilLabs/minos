---
phase: 08-cli-sample-project-and-docs
plan: 15
subsystem: phase close (final gate, requirements, validation contract, cargo clean)
status: complete
tags: [phase-close, final-gate, validation, requirements, gate-06, milestone-v1]

requires:
  - phase: 08-cli-sample-project-and-docs
    plan: 13
    provides: "08-EVIDENCE.md: the sample end to end on real VMs (2 passed, then 1 passed with the final budgets), the D-07 decision with the user's answer, and the user's live provider rerun on claude 2.1.312 (3 passed)"
  - phase: 08-cli-sample-project-and-docs
    plan: 14
    provides: "SECURITY.md, README.md, and the docs_words overclaim test"
provides:
  - "08-EVIDENCE.md '## Final gate': the six gate commands, the six sample gate commands, C-01, C-02 over crates and examples, the LOOP-01 check, the test-variable check over crates/*/src, and the validation-target check, all passing at HEAD b93a1a2; cargo test 1839 passed, 0 failed, 28 ignored over 104 result lines; sample_vm 2 ignored, runner_vm 1 passed and 3 ignored (no VM launched, no image built)"
  - "REQUIREMENTS.md: CLI-01, CLI-05 to CLI-10, JUDGE-02, COST-02, DOCS-01 to DOCS-04 checked and Complete; the JUDGE-02 D-07 note and a CLI-05 D-06 note; a Phase 8 footer; GATE-06 left cross-cutting"
  - "STATE.md: the Phase 8 baseline repeat count blocker restated (met partially, still open, the user's answer), never closed"
  - "08-VALIDATION.md filled: status complete, nyquist_compliant and wave_0_complete true, 29 task rows green"
  - "The host probe VM test moved from host.rs's unit tests into tests/vm.rs, so no src file names a test variable"
  - "cargo clean after the final gate (GATE-06): 168360 files, 18.1 GiB removed; no cargo command after it"
affects: [/gsd-verify-work for phase 8, /gsd-complete-milestone for v1]

tech-stack:
  added: []
  patterns:
    - "Phase close: first pass, fix in the owning code, full rerun on the final HEAD; then requirements, the blocker restatement, and the validation contract; then cargo clean as the last cargo command"

key-files:
  created:
    - .planning/phases/08-cli-sample-project-and-docs/08-15-SUMMARY.md
  modified:
    - .planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md
    - .planning/phases/08-cli-sample-project-and-docs/08-VALIDATION.md
    - .planning/REQUIREMENTS.md
    - .planning/STATE.md
    - crates/minos-backend-krun/src/host.rs
    - crates/minos-backend-krun/tests/vm.rs

key-decisions:
  - "The test-variable check's four hits were the ignored unit test probe_host_on_this_mac in host.rs's cfg(test) module. Test code, not production, but the check covers all of crates/*/src, so the test moved to tests/vm.rs (assertions unchanged, variables read through harness::env_path) and the whole sequence was rerun. The check was not narrowed."
  - "requirements-completed lists the 13 Phase 8 requirement IDs and omits GATE-06, which is cross-cutting and stays unchecked (roadmap decision), as 07-17 did."
  - "CLI-05 says check builds; D-06 decided check builds nothing and names minos env build. CLI-05 is checked with a note saying so, beside the JUDGE-02 note the plan specified."
  - "The STATE.md blocker line keeps every earlier word and gains the D-07 outcome, 'Still open until a project with a timing objective...', and the user's answer copied from 08-EVIDENCE.md."
  - "The plan's validation-target loop reports MISSING for every name, because ls fails when either glob is absent (07-17's defect). It was run as an either-or test with compgen -G: 20 ok, none missing."
  - "The VM suite and the live provider suite were not rerun; 08-13's records are the evidence."

patterns-established:
  - "A test-variable check covers every src file of every crate, so an ignored test that reads a MINOS_* variable lives under tests/, never in a cfg(test) module under src/"

requirements-completed: [CLI-01, CLI-05, CLI-06, CLI-07, CLI-08, CLI-09, CLI-10, JUDGE-02, COST-02, DOCS-01, DOCS-02, DOCS-03, DOCS-04]

coverage:
  - id: D1
    description: "Every gate, sample gate, and grep check passes on the final code and is recorded in 08-EVIDENCE.md '## Final gate'"
    verification:
      - kind: other
        ref: "cargo fmt/clippy/test/deny/doc, guest musl clippy, the six sample gate commands, C-01, C-02, LOOP-01, test-variable grep at b93a1a2"
        status: pass
    human_judgment: false
  - id: D2
    description: "The 13 Phase 8 requirements marked Complete on 08-13's VM run, the live rerun, and the final gate; GATE-06 left cross-cutting"
    requirement: "CLI-01"
    verification:
      - kind: other
        ref: "grep -c '\\[x\\] \\*\\*CLI-0[156789]\\|\\[x\\] \\*\\*CLI-10' .planning/REQUIREMENTS.md prints 7; DOCS 4; JUDGE-02 and COST-02 2; GATE-06 unchecked 1"
        status: pass
    human_judgment: false
  - id: D3
    description: "The D-07 blocker restated as met partially and still open, with the user's answer"
    verification:
      - kind: other
        ref: "grep of the [Phase 8] line in STATE.md for 'met partially', 'Still open until a project with a timing objective', and 'User answer at the 08-13 checkpoint:'"
        status: pass
    human_judgment: false
  - id: D4
    description: "08-VALIDATION.md filled and Nyquist-compliant"
    verification:
      - kind: other
        ref: "grep nyquist_compliant: true and wave_0_complete: true (one line each); compgen -G check of every --test name (20 ok)"
        status: pass
    human_judgment: false
  - id: D5
    description: "cargo clean after the final gate (GATE-06)"
    requirement: "GATE-06"
    verification:
      - kind: other
        ref: "test ! -d target/debug && test ! -d target/examples"
        status: pass
    human_judgment: false
  - id: D6
    description: "The README quick start with the real binary (08-14's human check)"
    verification: []
    human_judgment: true
    rationale: "It spends one real model call on the user's subscription and checks that the README reads correctly to a person; no agent runs it"

duration: 10min
completed: 2026-10-28
metrics:
  started: 2026-10-28T20:36:25Z
  completed: 2026-10-28T20:46:30Z
  tasks: 2
  files: 6
---

# Phase 8 Plan 15: Phase close (final gate, requirements, validation, cargo clean) Summary

**Every Phase 8 gate passes at HEAD `b93a1a2`. That covers the six workspace gate commands, the six sample gate commands, C-01, C-02 over `crates` and `examples`, the LOOP-01 check, and the test-variable check over every crate's `src/`. `cargo test --workspace` ran 1839 passed, 0 failed, and 28 ignored over 104 result lines. `sample_vm` reported `2 ignored` and `runner_vm` reported `1 passed; 3 ignored`, so the gate launched no VM and built no image. The 13 Phase 8 requirements are Complete. The D-07 blocker is restated as met partially and still open. 08-VALIDATION.md is Nyquist-compliant. `cargo clean` closed the phase, the last phase of milestone v1.**

## Performance

- **Duration:** about 10 min
- **Started:** 2026-10-28T20:36:25Z
- **Completed:** 2026-10-28T20:46:30Z
- **Tasks:** 2
- **Files modified:** 6

## Preconditions (Task 1 step 0)

08-EVIDENCE.md already recorded everything Task 1 needed before it started:

- `## Sample end to end on real VMs (08-13)` with `2 passed`
- `## Sample end to end with the final budgets (08-13)` with `1 passed`
- The D-07 `Decision for the user:` paragraph with its `User answer (08-13 checkpoint, 2026-10-28): Accept for this milestone...` line
- `## Live provider tests rerun (08-13)` with all three cases passing on `claude` 2.1.312

## Accomplishments

- **First pass at `facf474`, one fix, full rerun at `b93a1a2`.**
  - On the first pass every step passed except the test-variable check. It printed four lines from `crates/minos-backend-krun/src/host.rs`: the ignored unit test `probe_host_on_this_mac`, which reads `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`.
  - The test moved into `tests/vm.rs` with its assertions unchanged, and it ran once on this Mac with all nine probes passing (`1 passed`).
  - The whole sequence was then rerun from `cargo fmt`. 08-EVIDENCE.md records the first pass, the fix, the moved test's run, and the rerun.
- **REQUIREMENTS.md.** These 13 are checked and set to Complete in the traceability table: CLI-01, CLI-05, CLI-06, CLI-07, CLI-08, CLI-09, CLI-10, JUDGE-02, COST-02, DOCS-01, DOCS-02, DOCS-03, and DOCS-04.
  - JUDGE-02 carries the planned note: the noise floor is 0 by construction, and the default of 3 trials stays a recorded assumption.
  - CLI-05 carries a D-06 note: `check` builds no image and instead names `minos env build` or `minos judge push`.
  - GATE-06 stays unchecked as cross-cutting. The footer is dated today, and the earlier-update lines are kept.
- **STATE.md.** The `[Phase 8]` baseline repeat count line keeps all of its earlier text. It gains the following:
  - the D-07 outcome (met partially, spread 0 by construction, default of 3 unchanged, VM wall time recorded instead)
  - "Still open until a project with a timing objective measures objective variance across boots"
  - the user's answer from the 08-13 checkpoint

  No other STATE.md line changed in Task 1.
- **08-VALIDATION.md.**
  - 29 task rows (08-01 to 08-15), each with the automated command as executed and all green.
  - A requirement map that names only targets that exist.
  - Wave 0 items ticked with where each one landed.
  - Three manual-only items: the `sample_vm` run and the live rerun, both done, and 08-14's README walkthrough, which is left for UAT.
  - Runtimes measured in the final gate, `status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`, and the sign-off ticked.
- **`cargo clean`** removed `target/` (168360 files, 18.1 GiB), including `target/examples` and `target/examples-improved`. No cargo command ran after it.

## Task Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 (fix found by the gate) | b93a1a2 | fix(08-15): move the host probe VM test out of src so no src file names the test variables |
| 1 | 1cee0a4 | docs(08-15): record the Phase 8 final gate, mark the requirements, and fill the validation contract |
| 2 | 542464b | chore(08-15): run cargo clean after the final gate (GATE-06) |

## Files Created/Modified

- `crates/minos-backend-krun/src/host.rs`: `probe_host_on_this_mac` and the three imports only it used are removed
- `crates/minos-backend-krun/tests/vm.rs`: `probe_host_on_this_mac`, which reads the variables through `harness::env_path`
- `.planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md`: `## Final gate` and `## cargo clean (08-15 Task 2, GATE-06)`
- `.planning/phases/08-cli-sample-project-and-docs/08-VALIDATION.md`: the filled contract
- `.planning/REQUIREMENTS.md`: the 13 checkboxes and traceability rows, two notes, and the footer
- `.planning/STATE.md`: the restated `[Phase 8]` blocker (Task 1), then position and session through gsd-tools

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The test-variable check found the host probe test in `src/`**
- **Found during:** Task 1, first pass at `facf474`
- **Issue:** `grep -rn 'MINOS_VMM\|...' crates/*/src` printed four lines from `host.rs` (05-11's ignored `probe_host_on_this_mac`, inside `#[cfg(test)] mod tests`). This is test code, but the plan's check covers all of `crates/*/src` and must print nothing. The hit is not a doc comment, so rewording could not fix it.
- **Fix:** Moved the test into `crates/minos-backend-krun/tests/vm.rs` with the same assertions. It reads the three variables through the shared harness's `env_path`, so `MINOS_KRUNFW` is now required for it, as it already is for every other test in that target. The krun library now reports `1 ignored` and the `vm` target `12 ignored`. The test passed on this Mac after the build and the signing. The check itself was not narrowed.
- **Files modified:** `crates/minos-backend-krun/src/host.rs`, `crates/minos-backend-krun/tests/vm.rs`
- **Commit:** b93a1a2

**2. [Rule 3 - Blocking] The plan's validation-target loop cannot pass**
- **Found during:** Task 1 verify
- **Issue:** `ls crates/*/tests/$t.rs crates/*/tests/$t/main.rs` exits non-zero whenever either path is absent, and no target has both forms. Under bash it printed `MISSING` for all 20 names. This is the same defect 07-17 recorded.
- **Fix:** The check ran as an either-or test with `compgen -G` on each form: 20 `ok` lines, no `MISSING`, exit 0. It is recorded as step 17 of the final gate.
- **Commit:** 1cee0a4

**3. [Rule 2 - Accuracy] A CLI-05 note**
- **Issue:** CLI-05's wording says `check` "builds". 08-CONTEXT D-06 decided that `check` builds nothing: a missing or stale lock, image, or judge push is a refusal that names the next command.
- **Fix:** CLI-05 is checked with a one-line note saying so, beside the JUDGE-02 note the plan specified, so the checkbox does not overclaim.
- **Commit:** 1cee0a4

**Total deviations:** 3 auto-fixed (2 blocking, 1 accuracy). **Impact:** one ignored test changed file, with no behavior change. No assertion, lint, check, or invariant was weakened.

## Verification

- Task 1 acceptance:
  - `## Final gate` appears once.
  - The CLI count is 7, DOCS is 4, JUDGE-02 and COST-02 make 2, and the unchecked GATE-06 is 1.
  - `nyquist_compliant: true` and `wave_0_complete: true` print one line each.
  - The either-or validation-target check prints no `MISSING`.
  - The `[Phase 8]` STATE line contains all three required phrases.
- Task 2 acceptance: `test ! -d target/debug && test ! -d target/examples` exits 0, and the last lines of 08-EVIDENCE.md record `cargo clean: exit 0, run after the final gate (GATE-06)`.
- C-02 over `crates` and `examples`, the LOOP-01 check, and the test-variable check print nothing on the final code.

## Known Stubs

None.

## Threat Flags

None. T-08-15-01 to 05 are mitigated as planned:
- Requirements were checked only after 08-13's records existed and `## Final gate` was written.
- The one failure was fixed in the owning code, and the whole sequence was rerun.
- The LOOP-01 and `MINOS_*` greps are recorded empty, and so is C-02 over `crates` and `examples`.
- The D-07 blocker is restated, not deleted.

## Issues Encountered

None beyond the deviations.

## User Setup Required

None.

## Next Phase Readiness

Phase 8 is complete, and it is the last phase of milestone v1. Next are `/gsd-verify-work 8` and `/gsd-complete-milestone`. Two items carry forward:
- UAT still holds 08-14's README walkthrough with the real binary, which needs one real model call by the user.
- The D-07 blocker stays open until a project with a timing objective measures objective variance across boots.

The target directory is clean, so the next VM run needs the build, the signing, and the musl guest build first.

## Self-Check: PASSED

- FOUND: .planning/phases/08-cli-sample-project-and-docs/08-15-SUMMARY.md
- FOUND: crates/minos-backend-krun/tests/vm.rs (`probe_host_on_this_mac`)
- FOUND: b93a1a2, 1cee0a4, 542464b in `git log`
