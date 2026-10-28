---
phase: 08-cli-sample-project-and-docs
plan: 13
subsystem: sample-vm-evidence
status: complete
tags: [vm, krun, sample, logparse, rlimit-as, budgets, evidence, live-provider, checkpoint]
requires:
  - minos_cli::{env_build, judge_push, project::prepare, jobs::krun_jobs, check, run, report, adopt} (08-06 to 08-12)
  - examples/logparse and the improved-parser fixture (08-04, 08-05)
provides:
  - crates/minos-cli/tests/sample_vm.rs (ignored end-to-end and measurement VM tests on the sample)
  - .planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md (every section through the live provider rerun)
  - measured [budget] build and judge (1m each) in examples/logparse/minos.toml
  - the live provider suite's result on claude 2.1.312, recorded for 08-14 to cite
affects: [08-14 docs cite the sample run, the measurements, and the 2.1.312 live run; 08-15 restates the D-07 blocker]
tech-stack:
  added: []
  patterns:
    - "The VM e2e composes prepare then krun_jobs, the exact production path of check and run; only the provider is scripted"
    - "Measurement rows are printed data; only the default-configuration boots are asserted"
    - "Live model calls are run by the user and recorded verbatim with home as ~ (06-CONTEXT D-17)"
key-files:
  created:
    - crates/minos-cli/tests/sample_vm.rs
    - .planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md
  modified:
    - examples/logparse/minos.toml
    - crates/minos-core/src/limits.rs
    - CLAUDE.md
key-decisions:
  - "The sample's [budget] build and judge are 1m: the largest default-configuration VM times were 1358 ms (candidate) and 1434 ms (judge), so 1m is more than 41 times either"
  - "DEFAULT_MEMORY_MIB and DEFAULT_SCRATCH_MIB are unchanged: the sample's release build passed at 448 MiB and above and failed at 384 MiB, so the default 768 MiB allowance holds 320 MiB of headroom"
  - "D-07 met partially and accepted by the user for this milestone (08-13 checkpoint, 2026-10-28): the objective spread is 0 by construction, the default of 3 trials stays a recorded assumption, VM wall-time variance is recorded instead, and no timing probe is scheduled in v1"
  - "The live provider suite passes on claude 2.1.312 (3 passed, exit 0, run by the user), so 08-14 may cite the provider's live behavior on the installed CLI"
requirements-completed: []
metrics:
  started: 2026-10-28T19:12:46Z
  completed: 2026-10-28T20:19:57Z
  duration: "about 67 minutes of wall time, including the user's Task 3 checkpoint"
  tasks: "3 of 3"
---

# Phase 8 Plan 13: The Sample on Real VMs and the Phase 8 Measurements Summary

The shipped `logparse` sample ran end to end on this Mac's hypervisor. The run covered `env build` (Docker buildx and the production packer), `judge push judge`, two `check` runs (the second reused the first and launched no VM), `run --generations 2`, `report`, and `adopt`. `patch -p1` then produced the improved parser byte for byte, and every `Judged` recorded krun on arm64. The `RLIMIT_AS` bisection, the judge VM times, and the boot-to-boot variance are recorded and applied. At the Task 3 checkpoint the user reran the live `claude` provider suite on 2.1.312 (3 passed) and accepted the partial D-07 outcome for this milestone.

## Status

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | The ignored sample VM tests | done | ccec267 |
| 2 | Run the VM tests on this Mac, record them, and apply what they measured | done | e880672 |
| 3 | The user reruns the live `claude` provider suite on the installed CLI (and answers D-07) | done (checkpoint resolved by the user) | 8ba528b |

## What was built

- **`crates/minos-cli/tests/sample_vm.rs`** has two ignored tests and builds no image and boots no VM in the gate (`2 ignored`).
  - `sample_end_to_end_on_real_vms` copies the sample into `/private/tmp/msv*` and writes `prices.toml` (compute, `haiku`, `claude-haiku-5-5`). It then runs `env_build`, `judge_push`, `prepare`, and `krun_jobs`. After that it runs `check` twice, then `run` to generation 2 with a scripted `FakeProvider` (the fixture, then the fixture plus one comment line), then `report` and `adopt` with the report's hash. Last, it runs `/usr/bin/patch -p1` with a cleared environment on a fresh copy of `src/parse.rs`.
  - `sample_measurements_on_real_vms` runs the baseline at five allowances (`memory_mib = 512 + allowance`, `scratch_mib = 256`), then five boots at the default configuration. It prints stage log tails through `LogExcerpt` for any row that does not score.
- **08-EVIDENCE.md** has a header in the 05-EVIDENCE format, then these sections: Build and sign, Host checks, Sample end to end on real VMs (`test result: ok. 2 passed`), RLIMIT_AS bisection (D-02), Judge compile time (D-04), Boot-to-boot variance (D-07) ending with `Decision for the user:` and the user's answer, Sample end to end with the final budgets (`test result: ok. 1 passed`), and Live provider tests rerun (08-13) (`test result: ok. 3 passed`, exit 0).
- **What the measurements changed:**
  - In the sample's `minos.toml`, `build` and `judge` go from `5m` to `1m`, and comments now cite 08-EVIDENCE.md.
  - In `limits.rs`, both "Recorded assumption ... flagged for the user to confirm" sentences are replaced with the measured result. The `grep -c 'Recorded assumption'` count went from 2 to 0, because the default allowance passed.
  - In CLAUDE.md, the `RLIMIT_AS` open question moved to "Verified facts, local" (now as of 2026-10-28), and a sample end-to-end fact was added there. `sample_vm.rs` was added to Testing and to Commands (3 lines name `sample_vm`). The D-07 finding replaces the RLIMIT_AS item in Open questions as the remaining assumption about the default trial count, and now records the user's acceptance.
- **Task 3:** 08-EVIDENCE.md gained `## Live provider tests rerun (08-13)` with the date, both commands, the verbatim output (home as `~`), the pass count, a case table, and the marker "Run by the user at the 08-13 checkpoint". CLAUDE.md "Verified facts, local" gained a `claude` 2.1.312 bullet dated 2026-10-28.

## Checkpoint resolution (Task 3)

- **What the user ran**, by hand at the repository root in their own logged-in terminal on 2026-10-28: `~/.local/bin/claude --version; echo "exit: $?"` (output `2.1.312 (Claude Code)`, exit 0) and `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1; echo "exit: $?"`.
- **Result:** `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 18.78s`, then `exit: 0`.
  - Case (a), the production provider: 2 proposals, 0 dropped, resolved model `claude-haiku-5-5`, usage input 2, output 441, cache creation 1119, cache read 2976, and no canary fired.
  - Case (b), the golden argv with the fixture as the CLI's own working directory: no hook or MCP canary fired, the marker was not in the envelope, and there were 2 valid proposals with 0 dropped.
  - Case (c), the positive control: all three canaries fired, and the marker was in the envelope.
- **D-07 answer, verbatim:** "Accept for this milestone. D-07 is accepted as met partially for this milestone: the objective is deterministic with spread 0, the default of 3 trials stays a recorded assumption, VM wall-time variance was recorded instead, and no timing probe is scheduled in v1." This answer is recorded under `User answer (08-13 checkpoint, 2026-10-28):` in 08-EVIDENCE.md. The STATE.md Phase 8 blocker stays open; 08-15 restates it.
- No agent ran the live suite, set `MINOS_CLAUDE_LIVE`, or ran the user's `claude`.

## Measurements (copied from 08-EVIDENCE.md)

- **RLIMIT_AS (D-02):**
  - 384 MiB: `rejected:build_failed`. rustc stopped with SIGABRT after `failed to register TLS destructor: out of memory`.
  - 448, 512, 640, and 768 MiB: `scored`.
  - The smallest passing allowance is 448 MiB. The default 768 MiB is 320 MiB above it.
- **Judge VM (D-04):** 1259 to 1434 ms, against the previous `[budget] judge` of 300000 ms. The candidate VM took 1189 to 1358 ms.
- **Five boots (D-07):** every boot scored `accuracy` and `accuracy_held_out` 0.3020833333333333, so the spread is 0. The candidate VM took 1237, 1358, 1204, 1189, and 1203 ms, and the judge VM took 1369, 1434, 1264, 1259, and 1262 ms.

## Verification

- `cargo test -p minos-cli --test sample_vm` reports `2 ignored`. `grep -c '#\[ignore'` returns 2, and the `krun_jobs(`/`env_build(`/`judge_push(`/`adopt(` grep matches 4 lines.
- On this Mac, the ignored suite passed with `2 passed` (exit 0), and the final-budgets rerun passed with `1 passed` (exit 0). `host doctor` and `docker buildx version` both exited 0.
- `cargo test -p minos-cli --test sample_manifest` passed 4 tests after the budget change. `cargo test -p minos-core` passed 766 with 0 failed.
- Task 3's verify, `grep -n '## Live provider tests rerun (08-13)'` on 08-EVIDENCE.md, matches line 1457.
- After Task 3: `cargo fmt --all --check` is clean, `cargo clippy --workspace --all-targets -- -D warnings` is clean, and `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'` prints nothing.
- No `cargo clean` was run (08-15 owns the phase-end gate), and no requirement IDs were ticked (08-15 does that).

## Deviations from Plan

1. **[Rule 3 - Blocking] The copy helper also skips a top-level `minos.lock`.** The plan says to copy the sample "without `target` or `.minos`". A `minos.lock` left in the repository's sample by a manual `env build` would make the copy's `env build` (without `--update`) refuse a different image. The sample's `.gitignore` excludes that file, so the shipped sample never contains it. Commit: ccec267.
2. **The test prints log tails for rows that do not score.** These go through `LogExcerpt` (2048 bytes), so the 384 MiB failure records its cause in the evidence. This is additive and no assertion changed.
3. **A recorded timing gap, not investigated.** By the shell's clock, the first suite run took 19:16:20 to 19:42:19 UTC, but libtest reported 408.44 s. The final-budgets run agreed with libtest (20:06:03 to 20:09:29 UTC, 205.86 s). The evidence states both values and offers no explanation.
4. **CLAUDE.md's D-07 open question now records the answer.** Its owner line named "the user's answer at the 08-13 checkpoint". Following the CLAUDE.md rule to keep the file current, it now says the user accepted the partial outcome for the milestone on 2026-10-28, that no timing probe is scheduled in v1, and that 08-15 restates it. The item stays an open recorded assumption. Commit: 8ba528b.

No code defect was found. Neither host failure path applied, and no assertion, budget, or invariant was weakened.

## Known Stubs

None.

## Threat Flags

None. The test reads `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` only from an ignored test file, and no production code reads them. The planned mitigations hold:
- T-08-13-01: judge data reaches only the judge VM, through the production runner, and the snapshot excludes `judge/`.
- T-08-13-02: the evidence records verbatim output and exit codes.
- T-08-13-SC: the base image is pinned by its index digest.
- T-08-13-04: the live suite ran only by the user's hand on their own subscription. Its output is recorded with the home path as `~` and holds no account value or cost figure.

## Self-Check: PASSED

- FOUND: crates/minos-cli/tests/sample_vm.rs
- FOUND: .planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md
- FOUND: ccec267, e880672, 8ba528b
