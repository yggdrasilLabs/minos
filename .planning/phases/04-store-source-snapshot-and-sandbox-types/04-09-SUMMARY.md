---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 09
subsystem: docs
tags: [claude-md, validation, evidence, gate, cargo-clean, GATE-06]

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    provides: "04-01 to 04-08: minos-store (MemoryStore, FileStore), minos-source (snapshot, denylist, scan, clean check), minos-sandbox (roles, specs, preflight, Untrusted, output reader, Backend), their tests and SUMMARYs"
provides:
  - "CLAUDE.md ## minos-store: the shipped store contract, layout, locking, durability, torn-tail repair, refusals, Duplicate and Conflict, and verified blobs"
  - "CLAUDE.md Open questions: id_* denylist breadth and extra entries, backend blob access, streaming blob put"
  - "04-VALIDATION.md: 26-row per-task map, nyquist_compliant, deviations and known limitations"
  - "04-EVIDENCE.md: real output of six deliberate violations and the ## Final gate, ending in cargo clean"
affects: [phase-5-krun-backend, phase-7-loop, phase-8-cli, 04-VERIFICATION]

tech-stack:
  added: []
  patterns:
    - "Evidence probe: run the test with --exact, add a temporary println! of the error Display or counts, run with --nocapture, paste, git checkout the file; git status --porcelain crates must print nothing"
    - "Secret evidence is checked for any 4-byte window of the planted value before pasting; only path, line, and pattern name appear"

key-files:
  created:
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-09-SUMMARY.md
  modified:
    - CLAUDE.md
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-VALIDATION.md
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md

key-decisions:
  - "04-09: GATE-06 is not checked off in REQUIREMENTS.md; it is cross-cutting (the last criterion of every phase), and the phase verifier reruns cargo clean as its own last command"
  - "04-09: the known limitations from 04-02 to 04-08 (process-group kill in the git check, FIFO swap hang, crate-private LedgerRead::new, two unadded compile-fail cases, unused-wrapper warnings, ustar prefix in path_bytes, id_* breadth) are listed in 04-VALIDATION.md; only the three plan-named items went into CLAUDE.md Open questions"

patterns-established:
  - "Phase close: design-of-record section, validation map, deliberate-violation evidence, then the ordered gate and cargo clean last"

requirements-completed: []

coverage:
  - id: D1
    description: "CLAUDE.md ## minos-store states the shipped store, placed before ## minos-sandbox, naming driver.lock and Conflict"
    verification:
      - kind: other
        ref: "grep -n '^## minos-store' CLAUDE.md (line 211, before ## minos-sandbox at 221); grep -n 'driver.lock' CLAUDE.md; grep -n 'Conflict' CLAUDE.md"
        status: pass
    human_judgment: false
  - id: D2
    description: "CLAUDE.md Open questions list the id_* breadth, backend blob access, and streaming put"
    verification:
      - kind: other
        ref: "grep -n 'id_map.rs' CLAUDE.md; grep -n 'streaming put' CLAUDE.md; grep -n 'id_ed25519' CLAUDE.md"
        status: pass
    human_judgment: false
  - id: D3
    description: "04-VALIDATION.md maps all 26 tasks, no template placeholder left, nyquist_compliant and wave_0_complete true"
    verification:
      - kind: other
        ref: "grep -c '^| 4-0' 04-VALIDATION.md (26); placeholder grep (0)"
        status: pass
    human_judgment: false
  - id: D4
    description: "04-EVIDENCE.md shows the real output of the torn line, unknown v, tampered blob, planted secret (location only), offenders, and the 12 compile-fail first error lines; no probe committed"
    verification:
      - kind: other
        ref: "04-EVIDENCE.md six 04-09 sections; test -z \"$(git status --porcelain crates)\""
        status: pass
    human_judgment: false
  - id: D5
    description: "Full gate green, then cargo clean as the last cargo command (GATE-06, executor side)"
    requirement: "GATE-06"
    verification:
      - kind: other
        ref: "04-EVIDENCE.md ## Final gate; test ! -d target/debug"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-10-15
status: complete
---

# Phase 4 Plan 09: Phase close (store design of record, validation map, evidence, final gate) Summary

**CLAUDE.md now describes the shipped file store (per-run JSONL ledger under an exclusive `File::lock`, `sync_data` per append, torn-tail repair, hard refusals, `Duplicate`/`Conflict`, rehashed temp-then-rename blobs) and lists three open questions. 04-VALIDATION.md maps all 26 Phase 4 tasks. 04-EVIDENCE.md shows the real output of six deliberate violations. The full gate passed, and `cargo clean` ran last.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-10-15T15:02:29Z
- **Completed:** 2026-10-15T15:08:40Z
- **Tasks:** 3
- **Files modified:** 3 (plus this summary)

## Accomplishments

- **CLAUDE.md `## minos-store`** (line 211, before `## minos-sandbox`) covers:
  - the synchronous `&self` traits, the memory and file implementations, and the one contract test file
  - the `.minos/` layout: 0700, symlinks refused, the root on the denylist
  - the exclusive per-append lock that rebuilds the key set, and `sync_data` with directory fsyncs
  - shared-lock reads, `Duplicate` and `Conflict`, and torn-tail tolerance with `set_len` repair
  - every hard refusal: `BadJson`, `KeyMismatch`, `SeqGap`, `LineTooLong`, `UnknownVersion`, and `LedgerTooLarge`
  - verified temp-then-rename blobs, and `driver.lock` as a separate Phase 7 file
- **CLAUDE.md Open questions** has three new bullets:
  - the `id_*` breadth (it also drops `src/id_map.rs`) and the extra candidates `.netrc`, `.npmrc`, `.pypirc`, `*.key`, `*.p12` (the user decides)
  - how a backend receives blob bytes (Phase 5)
  - a streaming put that needs a core hashing constructor (Phase 5)
- **04-VALIDATION.md** fills the template in 03-VALIDATION.md's format:
  - test infrastructure and a 26-row per-task map; each row has its requirement, threat IDs, secure behavior, and the plan's exact verify command
  - Wave 0 items, a Deviations list copied from the eight SUMMARYs, and a known-limitations list
  - sign-off ticked; `status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`
- **04-EVIDENCE.md** gains a method note and six sections. Each section has the `--exact` `test result: ok` line, the probe output, and its restore:
  - a torn tail: read 3 lines with 1 torn, then 4 lines and 0 torn after the next append; the file went 752 bytes to 860
  - an unknown `v`: the Display names the run and line 2
  - a tampered blob: `ba7816bf...15ad does not hash to its id`
  - a planted secret: `src/lib.rs:4: matches aws-access-key-id`, with no 4-byte window of the key present
  - six offenders listed in one error
  - the trybuild run and the first `error` line of all 12 `.stderr` files
- **`## Final gate`:** fmt, clippy, tests (911 passed, 0 failed, 26 ignored), deny, doc, musl clippy, C-01 (11 ok), C-02 (empty), and the unchanged clippy bans all exit 0. Then `cargo clean` removed 36320 files (5.0 GiB).

## Task Commits

1. **Task 1: CLAUDE.md store section and open questions; complete 04-VALIDATION.md**: `2ef4582` (docs)
2. **Task 2: Deliberate-violation evidence**: `11f7ed3` (docs)
3. **Task 3: Final gate, then cargo clean (GATE-06)**: `5c1f677` (docs)

**Plan metadata:** recorded in the final `docs(04-09): complete ...` commit

## Files Created/Modified

- `CLAUDE.md`: new `## minos-store` section; three Open questions bullets
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-VALIDATION.md`: completed validation contract
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md`: six violation sections and `## Final gate`

## Command log (Task 3, in order)

```
cargo fmt --all --check                                                              exit 0
cargo clippy --workspace --all-targets -- -D warnings                                exit 0
cargo test --workspace   (TRYBUILD unset)                                            exit 0  911 passed, 0 failed, 26 ignored
cargo deny check                                                                     exit 0  advisories ok, bans ok, licenses ok, sources ok
cargo doc --workspace --no-deps                                                      exit 0
cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings   exit 0
C-01                                                                                 11 ok, 0 MISSING
C-02  grep -rnE '#!?\[allow\(' crates --include='*.rs'                               no output
git log --oneline 56467aa..HEAD -- crates/minos-{store,sandbox,source}/clippy.toml   no output
git diff --quiet -- crates/minos-core/clippy.toml                                    exit 0
cargo clean                                                                          Removed 36320 files, 5.0GiB total
```

No cargo command ran after `cargo clean`. Only git commits and the gsd-tools state updates followed.

## Decisions Made

- GATE-06 is not checked off in REQUIREMENTS.md. It is cross-cutting (STATE.md roadmap decision), and Phase 3 left it pending in the same way.
- The extra known limitations go in 04-VALIDATION.md, not CLAUDE.md. The prompt asked for these limitations to be carried forward, but the plan asks for only three CLAUDE.md bullets and no other edit to CLAUDE.md. The three bullets are the questions that need a decision. The rest are implementation follow-ups.

## Deviations from Plan

- **Evidence wording:** the secret section first said the output held no "`AKIA` prefix". That literal is itself part of the key's shape, so it was reworded to "no access-key prefix" before the commit. Neither file holds any fragment of the vector.
- **Unknown-version probe shape:** that test calls a shared helper (`assert_refused`) that hides the error. The probe therefore wrote the same bytes into a fresh store inside the test and printed the read and append errors. The file was restored afterwards.
- **Table escaping:** commands that contain `|` are written as `\|` in the 04-VALIDATION.md table, so the markdown table renders. A note above the table says so.

**Total deviations:** 3, all documentation-shape choices. **Impact:** none on code. `crates/` is untouched by this plan.

## Issues Encountered

None. The known flaky `minos-backend-krun` supervise test passed.

## Verifier handoff

This `cargo clean` ends the executor's work. GATE-06, however, requires `cargo clean` **after verification passes**, and post-execution verification reruns the gate commands, which rebuild `target/`. So the phase verifier must run `cargo clean` as its own last command. It runs unconditionally, after its verification passes and before the phase is marked complete. The verifier records that command and its output in its verification report. No cargo command may run after the verifier's `cargo clean`.

## Next Phase Readiness

Phase 4 is complete pending verification. Phase 5 inherits the open questions on blob delivery to a backend and on streaming puts. The user decides the denylist breadth.

## Self-Check: PASSED

- FOUND: CLAUDE.md (`## minos-store` at line 211), 04-VALIDATION.md (26 rows, `nyquist_compliant: true`), 04-EVIDENCE.md (`## Final gate`, `cargo clean` in the last 5 lines)
- FOUND: commits 2ef4582, 11f7ed3, 5c1f677
- `git status --porcelain crates` prints nothing; `test ! -d target/debug` holds
