---
phase: 05-guest-krun-backend-and-environment-build
plan: 20
subsystem: phase-close
tags: [final-gate, gate-06, krun-03, krun-05, guest-05, cli-03, evidence, validation, cargo-clean]
status: complete
requires:
  - "05-18: env build against Docker buildx, the decided D-12 wording"
  - "05-19: tests/exit.rs (exit_egress, exit_env, exit_kill9, exit_forge) and the KRUN_BOOT_START_NS recorded exception"
provides:
  - "05-EVIDENCE.md ## Final gate: the six gate commands, C-01, C-02, build and sign, host doctor, the exit_env smoke, the vm and exit suites, and the Docker test, verbatim, on one final build"
  - "REQUIREMENTS.md: GUEST-01 to GUEST-05, KRUN-01 to KRUN-10, CLI-02, CLI-03 complete; GUEST-05's recorded exception stated and dated"
  - "05-VALIDATION.md: 58-task verification map, Wave 0 landed, nyquist_compliant and wave_0_complete true"
  - "CLAUDE.md: minos-core module list with chunk.rs, lock.rs, guest.rs; Workspace descriptions; Open questions reconciled"
affects:
  - "Phase 6 onward: starts from a clean target directory and a design of record that matches the shipped Phase 5 code"
tech-stack:
  added: []
  patterns:
    - "A nocapture VM run interleaves libtest's ok with test output, so the final gate also records a captured run whose result lines read `test <name> ... ok`"
key-files:
  created:
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-20-SUMMARY.md
  modified:
    - CLAUDE.md
    - .planning/REQUIREMENTS.md
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-VALIDATION.md
decisions:
  - "The chunked output protocol's one-VM cost is answered (W0-CHUNKS, 05-17), so its Open questions bullet folds into the K8 concurrency bullet (deferred with NEXT-04) rather than standing alone without an owner"
  - "GUEST-05 is checked with its allowance on its own line: KRUN_BOOT_START_NS=<digits> is a recorded exception by the user's decision at the 05-19 checkpoint, 2026-10-19"
  - "GATE-06 stays unchecked (cross-cutting); SPIKE-03 stays as the user worded it"
metrics:
  duration: "about 8 minutes"
  completed: 2026-10-19
  tasks: 3
  files: 4
---

# Phase 5 Plan 20: close the phase Summary

The exit criterion passed on this Mac on one final build (HEAD `4cc176a`), and its verbatim output is in 05-EVIDENCE.md `## Final gate`:

- All six gate commands passed, along with C-01 and C-02.
- `minos host doctor` exited 0 with `result: every check passed`.
- The `exit_env` smoke passed first. Then the `vm` suite passed (10 tests), the `exit` suite passed (4 tests: `exit_egress`, `exit_env`, `exit_forge`, `exit_kill9`), and the Docker test passed.
- The requirements, the validation map, and CLAUDE.md now match the shipped code.
- `cargo clean` ran last.

## Tasks

### Task 1: reconcile CLAUDE.md (commit 4cc176a)

- **minos-core module list:** adds `chunk.rs`, `lock.rs`, and `guest.rs`, with one sentence each:
  - `chunk.rs`: the authenticated chunk codec.
  - `guest.rs`: the guest job, the guest status, and the `MINOSJB2` input disk codec.
  - `lock.rs`: `minos.lock` and `Lock::check`.
- **Workspace:**
  - `minos-guest` reads its job through `JobSource` and sends output through `OutputSink` (D-07). The `minos-probe` fixture is test-only.
  - The `minos-backend-krun` line names its parts: `KrunBackend`, the host probes, job directories and the sweep, the supervisor, the chunk receiver, the embedded Seatbelt profile, the input disk writer, and the packer.
  - `minos-cli` ships `env build`, `host doctor`, and `host setup`.
- **Open questions:**
  - The answered Phase 5 bullets were already gone: console cap, virtio-rng, the libkrunfw copy, backend blob bytes, and the streaming put.
  - The chunked protocol's one-VM cost is answered, so its bullet folds into K8 (concurrency, deferred with NEXT-04).
  - These stay, each with its owner:
    - the `_minos` user (D-01);
    - Seatbelt narrowing;
    - the `id_*` denylist (the user's call);
    - `RLIMIT_AS` at the defaults (Phase 8);
    - streaming unpack for judge data over 96 MiB (Phase 7);
    - packer-to-writer streaming (a follow-up);
    - removing `KRUN_BOOT_START_NS` (the next `msb_krun` pin bump).
- **Local backends and Environments and source:** I read both sections for contradictions and found none.
  - The only `scratch disk` mention says launch config v2 has none.
  - `initramfs` appears only as negative evidence.
  - No `spike_` name remains (count 0).
  - The D-12 wording from 05-18 is kept.
  - The boot bullet states the GUEST-05 allowance as the user's 2026-10-19 decision.

### Task 2: final gate, evidence, requirements, validation (commit 579793a)

- **Order of the run:**
  1. The Docker daemon check: 29.6.1, exit 0.
  2. The gate commands: fmt; clippy; `cargo test --workspace` (63 result lines, 1139 passed, 0 failed, 19 ignored); deny; doc; musl clippy.
  3. C-01: eleven `ok` lines. C-02: printed nothing, grep exit 1.
  4. The builds, then `--no-run` of the VM test binaries, then signing. Both entitlement keys were displayed, and they were re-checked before each VM run.
  5. `host doctor`: exit 0.
  6. The `exit_env` smoke, then the `vm` suite, then the `exit` suite. The exit suite ran twice: once with nocapture and once captured, so each result line is literal.
  7. The Docker test: 1 passed, both digests `41cfea1b…9276b`.
- **What the exit tests showed:**
  - `exit_kill9`: the helper exited 279.6 ms after SIGKILL.
  - `MINOS_EXIT_HOST_IP` was unset, so the LAN target did not run, and the evidence says so.
  - The virtio device `0x4d47` is now named in the evidence. It is msb_krun's VM-generation-ID device, `msb_krun_devices` 0.1.41 `src/virtio/vmgenid/mod.rs` line 20, `VIRTIO_ID_MSB_VMGENID`. It is not a network or filesystem device.
- **REQUIREMENTS.md:**
  - KRUN-02, KRUN-06, KRUN-08, and CLI-03 are now checked and their traceability rows say Complete. The other 13 were checked by earlier plans, so 17 are checked in total.
  - GUEST-05 has its allowance on its own line: the date 2026-10-19 and the phrase "recorded exception", matching CLAUDE.md.
  - GATE-06 is unchecked. SPIKE-03 is unchanged; W0-CHUNKS and `vm::output_16_mib_is_reassembled` are new evidence for it.
- **05-VALIDATION.md:**
  - 58 rows, one per task of plans 05-01 to 05-22. Each row gives the requirement, the threat refs from that plan's register, the plan's `<done>` text as the secure behavior, and the plan's exact `<automated>` command.
  - Wave 0 is marked complete, with where each item landed and what superseded it.
  - The Docker env build is the manual-only item, run in 05-18 and in the final gate.
  - `status: complete`, `nyquist_compliant: true`, `wave_0_complete: true`, and the sign-off is ticked.

### Task 3: cargo clean (commit bd23596)

`cargo clean` exited 0: `Removed 165992 files, 17.3GiB total`. `target/debug` is gone. This was the last cargo command. 05-EVIDENCE.md ends with `cargo clean: exit 0, run after the final gate (GATE-06)`, and the 05-20-T3 row is green.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 4cc176a | docs(05-20): reconcile CLAUDE.md with the shipped Phase 5 code |
| 2 | 579793a | docs(05-20): final gate evidence, Phase 5 requirements, and the validation map |
| 3 | bd23596 | docs(05-20): record cargo clean after the final gate (GATE-06) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Evidence] Extra exit-suite run with libtest capturing output**
- **Found during:** Task 2
- **Issue:** The acceptance check looks for the literal `test exit_egress ... ok`. Under `--nocapture`, libtest prints `ok` on its own line after the test's output.
- **Fix:** Ran the same suite once more on the same build without `--nocapture`, and recorded both runs verbatim. Both passed 4 of 4.
- **Commit:** 579793a

**2. [Rule 2 - Evidence] Docker test included in the final gate**
- **Found during:** Task 2
- **Issue:** The orchestrator asked for a Docker re-check before the final gate. The plan's command list leaves out the ignored Docker test, which is the manual-only item for KRUN-09 and CLI-02.
- **Fix:** Checked the daemon and ran `docker_env_build_reproduces` on the final build. Recorded verbatim; 1 passed.
- **Commit:** 579793a

No code changed. No assertion was weakened.

## Observations

- 05-19 reported "64 result lines" for `cargo test --workspace`. That count includes one test named `results_round_trip`, whose output line also begins with `test result`. The correct count is 63; the passed, failed, and ignored totals are the same.
- `.claude/CLAUDE.md`, the GSD-generated stack block, still names the Seatbelt profile as `crates/minos-vmm/minos-vmm.sb`. The shipped path is `crates/minos-backend-krun/src/minos-vmm.sb`, which CLAUDE.md states correctly. That generated block was outside this plan's files.
- The untracked `.planning/research/.cache/` files were left unstaged.

## Known Stubs

None.

## Threat Flags

None. No production surface changed.

## Self-Check: PASSED

- FOUND: 05-EVIDENCE.md `## Final gate` (line 1428), containing `test exit_egress ... ok`, `test exit_env ... ok`, `test exit_kill9 ... ok`
- FOUND: 17 checked GUEST, KRUN, and CLI-02/03 lines; GATE-06 unchecked; `KRUN_BOOT_START_NS` on the GUEST-05 line
- FOUND: `nyquist_compliant: true`, `wave_0_complete: true` in 05-VALIDATION.md
- FOUND: commits 4cc176a, 579793a, bd23596
- `test ! -d target/debug`: true
