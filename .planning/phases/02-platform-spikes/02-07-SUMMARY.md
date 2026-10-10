---
phase: 02-platform-spikes
plan: 07
subsystem: infra
tags: [rust, libkrun, msb_krun, libkrunfw, minos-vmm, watchdog, exit-codes, spike, k7]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: minos-guest jobs (report_boot, hang, console_flood, poweroff, return); 02-03: minos-vmm exit codes (E-3) and watchdog; 02-05: supervise::run, GuestBytes, job-directory helpers; 02-06: E-1 environment, E-4 signing, root-disk boot"
provides:
  - "Ignored K7 scenario tests k7_01_poweroff to k7_10_injected_cmdline_env, the k7_07b_parent_gone_after_boot diagnostic, and the orphan_launcher entry point (spike_exits.rs)"
  - "The K7 exit map, recorded from real runs (evidence/02-07.md K7-00 to K7-11, K7 table)"
  - "A classification rule Phase 5 can implement: the helper's exit status never carries the guest's result; only a valid vsock frame does"
  - "DEFECT-01 fixed in minos-vmm: parent PID captured at main entry, PID 1 parent counts as gone"
affects: [02-10, 02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "Orphaning a helper with no shell: the test binary re-executes itself (current_exe) with an argv array into an ignored entry point that spawns the helper and returns"
    - "A process-identity check that must survive the startup window reads its reference value as the first statement of main"

key-files:
  created:
    - crates/minos-backend-krun/tests/spike_exits.rs
    - .planning/phases/02-platform-spikes/evidence/02-07.md
  modified:
    - crates/minos-vmm/src/main.rs
    - crates/minos-vmm/src/watchdog.rs

key-decisions:
  - "K7: guest power-off, guest init return, and a host SIGTERM all exit the helper with 0; the exit code alone cannot tell a clean run from a crashed guest, only the vsock status frame can (overturns .planning/research/ARCHITECTURE.md Pattern 5 'Exit code' bullet; 02-10 records it under D-09)"
  - "K7-02 research correction: libkrunfw 5.6.2 patches do_exit so a returning PID 1 triggers orderly_reboot, not the 'Attempted to kill init' panic; PSCI SYSTEM_RESET is a Shutdown exit in msb_krun, so the status is still 0. The test now asserts the orderly-reboot line"
  - "DEFECT-01 (orchestrator-approved Rule 4 deviation): minos-vmm reads parent_id() as the first statement of main and passes it to watchdog::spawn; the parent-gone check also trips when the current parent is PID 1. Exit codes, stderr lines, and the launch config wire format unchanged"
  - "Phase 5 classification: 64/65/66/67/70 and unexpected signals are infrastructure failures; host deadline or 124 is TimedOut; 126 is an infrastructure failure (parent gone); 125 is Crashed (console cap); exit 0 with exactly one valid v1 frame is guest-decided; exit 0 without a valid frame is Crashed"

patterns-established:
  - "Every K7 scenario is an ignored integration test, so the exit map is reproduced by re-running spike_exits"

requirements-completed: [SPIKE-01]

duration: 15min
completed: 2026-10-10
---

# Phase 2 Plan 07: K7 exit map

**All ten K7 scenarios now run as ignored tests against the signed helper, and every row is recorded from a real run. A guest power-off, a guest init return, and a host SIGTERM all exit 0, so Phase 5 must classify a run from the vsock status frame and never from the helper's exit code. The watchdog codes (124, 125, 126), SIGKILL, and the preflight refusals (66, 67) each show up with their own status and stderr marker. One shared-code defect turned up: an orphaned helper was never stopped (DEFECT-01). It is fixed in `minos-vmm`, and its scenario now passes without any change to the test.**

## Performance

- **Duration:** about 15 min (two executor sessions with an orchestrator decision between them)
- **Started:** 2026-10-10T14:12:26Z
- **Completed:** 2026-10-10T14:28:00Z
- **Tasks:** 2 of 2
- **Files modified:** 4 (spike_exits.rs, evidence/02-07.md, minos-vmm main.rs and watchdog.rs)

## Accomplishments

- `spike_exits.rs` holds 12 ignored tests: `k7_01` to `k7_10`, the diagnostic `k7_07b_parent_gone_after_boot`, and the `orphan_launcher` entry point (E-1). No shell is used: the orphaning intermediate is the test binary itself, run with an argv array.
- The full suite passes on the final tree: 12 passed, then `pgrep -x minos-vmm` exits 1 (K7-11).
- K7-04 measured the console-cap overshoot. In its own run the console file reached 311098 bytes against the 262144-byte cap (48954 bytes over, 18.7 %). In the suite run it reached 292144 bytes (30000 over). The overshoot depends on how much the guest writes between two 50 ms watchdog polls.
- DEFECT-01 is fixed, with unit tests, and T-02-44 is closed.

## K7 table

| Scenario | Mechanism (source) | Observed status | stderr markers | Frame | Evidence |
|---|---|---|---|---|---|
| Guest power-off | `reboot(POWER_OFF)` -> PSCI `SYSTEM_OFF` -> `VcpuExit::Shutdown` | exit 0 | `minos-vmm: exit 0` | present (1, v1) | K7-01 |
| Guest init returns | libkrunfw patch 0001: PID 1 exit calls `orderly_reboot()`; patch 0002 forces `kernel_restart` -> PSCI `SYSTEM_RESET` -> `Shutdown` | exit 0 | `minos-vmm: exit 0` | present (sent before the return) | K7-02 |
| Launcher deadline | watchdog stores 124, triggers `ExitHandle` | exit 124 | `minos-vmm: watchdog deadline`, `minos-vmm: exit 124` | absent | K7-03 |
| Console cap | watchdog sees the console file over `cap_bytes`, stores 125 | exit 125; 311098 bytes for a 262144 cap (overshoot 48954; 30000 in the suite run) | `minos-vmm: watchdog console-cap`, `minos-vmm: exit 125` | absent | K7-04 |
| SIGTERM to the helper | msb_krun's handler writes `exit_evt`, graceful stop | exit 0 | `minos-vmm: exit 0` | absent | K7-05 |
| SIGKILL to the helper | the process dies along with the in-process HVF VM | signal 9; `ps -p` exits 1 | none | absent | K7-06 |
| Helper orphaned before the watchdog starts | before the fix, the watchdog recorded PID 1 as the parent and never tripped (DEFECT-01). After `8bd6881`, `main` records the parent first and a PID 1 parent counts as gone | before the fix: still running at 15 s. After: exit 126 in 505 ms (285 ms in the suite run) | `minos-vmm: watchdog parent-gone`, `minos-vmm: exit 126` | absent | K7-07, K7-11, DEFECT-01 |
| Helper orphaned after boot | watchdog sees `parent_id()` change, stores 126 | exit 126 in 59 ms | `minos-vmm: watchdog parent-gone`, `minos-vmm: exit 126` | absent | K7-07b |
| Kernel hash mismatch | preflight `PreflightError::Hash` | exit 67 | `minos-vmm: error: preflight: kernel.krunfw ...: SHA-256 is ..., the config expects ...` | absent; no console file | K7-08 |
| Kernel missing | preflight `canonicalize` fails | exit 66 | `minos-vmm: error: preflight: kernel.krunfw ...: No such file or directory (os error 2)` | absent; no console file | K7-09 |
| Injected `MSB_KRUN_KERNEL_CMDLINE` | preflight `check_environment` refuses `MSB_KRUN_*` before msb_krun reads it | exit 66 | `... environment variable MSB_KRUN_KERNEL_CMDLINE is set; ...` | absent; no console file | K7-10 |
| Wrong argument count | `config_path` | exit 64 | `minos-vmm: usage: ...` | n/a | 02-03 V-EXIT |
| Config unreadable or invalid | `read_config` / `config::parse` | exit 65 | `minos-vmm: error: ... launch config ...` | n/a | 02-03 V-EXIT |
| Unentitled helper | `hv_vm_create` refused before takeover | exit 70 | `minos-vmm: error: starting the VM: ...` | n/a | 02-06 S04-UNSIGNED |

## Classification rule for Phase 5

The helper's exit status never carries the guest's result. Phase 5 checks these in order, using only what the host observed:

1. **Infrastructure failure (retried):** exit 64, 65, 66, 67, or 70 with a `minos-vmm: error:` or `usage:` line; a spawn failure; a signal the host did not send; any code not in the table.
2. **`TimedOut`:** the host deadline fired, or exit 124 with `minos-vmm: watchdog deadline`.
3. **Infrastructure failure (parent gone):** exit 126 with `minos-vmm: watchdog parent-gone`.
4. **`Crashed` (console cap):** exit 125 with `minos-vmm: watchdog console-cap`. The guest caused it, so it is never retried.
5. **Guest decided:** exit 0 and exactly one connection that decodes as a valid v1 frame through `GuestBytes::status_frame`.
6. **`Crashed`:** exit 0, or a status the host caused on purpose, with no valid frame.

Rows 1 to 4 need both the exit code and the stderr marker. Only `minos-vmm` writes those markers. The guest can reach only the console file.

This overturns `.planning/research/ARCHITECTURE.md` Pattern 5, "Exit code" bullet. Under D-09 the research file stays as written, and 02-10 records the correction.

## Rows that differed from the prediction

- **K7-02 (research correction):** E-2 and 02-RESEARCH.md Pattern 3 predicted the "Attempted to kill init" panic. libkrunfw 5.6.2 patches `do_exit` so that a returning PID 1 calls `orderly_reboot()`, which then forces `kernel_restart`. msb_krun treats PSCI `SYSTEM_RESET` as `Shutdown`. The status is still 0, as predicted, but no panic happens and `panic=-1` plays no part. The assertion now requires `Failed to start orderly reboot: forcing the issue` and the absence of `Attempted to kill init` (commit `1ce9ac9`).
- **K7-07:** this row failed twice before the fix (DEFECT-01, below) and passed unchanged after it.

## Task Commits

1. **Task 1: K7 scenario tests, including an orphaned-helper scenario with no shell:** `8e52134` (test)
2. **Task 2: Run the K7 map and record the classification rule:** `1ce9ac9` (test, K7-02 assertion and K7-07b), `dee006c` (docs, K7 runs, DEFECT-01, table, rule), `8bd6881` (fix, DEFECT-01), `8c5d816` (docs, K7-07 rerun, full suite, DEFECT-01 outcome)

## Files Created/Modified

- `crates/minos-backend-krun/tests/spike_exits.rs`: the K7 scenarios, K7-07b, and `orphan_launcher`
- `.planning/phases/02-platform-spikes/evidence/02-07.md`: K7-00 to K7-11, K7-07b, DEFECT-01, the K7 table, and the classification rule
- `crates/minos-vmm/src/main.rs`: records the parent PID as the first statement of `main` and passes it through `run`
- `crates/minos-vmm/src/watchdog.rs`: `spawn` takes the expected parent; `parent_gone(expected, current)` also trips on PID 1; two new unit tests

## Decisions Made

See `key-decisions` in the frontmatter.

## Deviations from Plan

### Orchestrator-approved Rule 4 deviation

**1. [Rule 4 - Shared-code defect, fix approved by the orchestrator] DEFECT-01: a helper orphaned before its watchdog starts is never stopped**
- **Found during:** Task 2, K7-07 (two failing runs, both recorded verbatim).
- **Issue:** `watchdog::spawn` read `parent_id()` only after `read_config`, `preflight::check` (which hashes libkrunfw), and `launch::build` had finished. A parent that exited during that window left the helper reparented to launchd, so the watchdog recorded PID 1 and never tripped. The helper then ran until `max_duration_ms`, which can be up to one hour.
- **Fix:** `main` reads `std::os::unix::process::parent_id()` as its first statement and passes it to `watchdog::spawn`. The parent-gone check also trips when the current parent is PID 1, because launchd never legitimately starts `minos-vmm`, and that closes the window between `fork` and the first instruction. Two unit tests cover the change, written with no unwrap, expect, or indexing. Unchanged: exit codes 64/65/66/67/70/124/125/126, the stderr lines, the launch config wire format, `clippy.toml`, and the entitlements.
- **Scope:** this edits files outside the plan's `files_modified` (the prohibition on shared code). The orchestrator authorised the change after the wave-4 checkpoint.
- **Verification:** the helper was rebuilt and re-signed with 02-06 E-4, and `codesign -d --entitlements -` shows both keys. `k7_07_parent_gone` passes unchanged: exit 126 in 505 ms, and in 285 ms during the suite run. All 12 spike_exits tests pass, and `pgrep` exits 1.
- **Files modified:** `crates/minos-vmm/src/main.rs`, `crates/minos-vmm/src/watchdog.rs`
- **Commit:** `8bd6881`

### In-plan adjustments (plan step 3)

**2. K7-02 assertion changed to the observed behaviour.** This is the research correction described above. The plan's step 3 allows it. Commit `1ce9ac9`.

**3. K7-07b diagnostic added.** `k7_07b_parent_gone_after_boot` orphans the helper only after `MINOS-GUEST-BOOT-OK`. It showed that the trip and code 126 work once the parent has been recorded, which isolated DEFECT-01 to the startup window. It stays as a regression test. Commit `1ce9ac9`.

## Issues Encountered

- `cargo build` and `cargo test --workspace` relink `minos-vmm` and drop its signature. The helper was re-signed after the fix build and again after the gate run. It is signed as the tree stands now.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (119 passed, 0 failed, 18 ignored), `cargo deny check`, `cargo doc --workspace --no-deps`, and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: all exit 0.
- C-01 prints ten `ok` lines. The C-02 allow grep and the unsafe grep print nothing. The shell grep on `spike_exits.rs` prints 0.
- VM spike rerun: `cargo test -p minos-backend-krun --test spike_exits -- --ignored --nocapture --test-threads=1` passes (12 passed), then `pgrep -x minos-vmm` exits 1.
- `cargo clean` was not run. 02-11 runs it (GATE-06).

## Carry-forward for 02-10

- Record the K7 table and the classification rule in CLAUDE.md "Local backends" and in the design of the Phase 5 `Outcome` mapping. The helper's exit code is never the guest's result.
- Record the correction to `.planning/research/ARCHITECTURE.md` Pattern 5, "Exit code" bullet (D-09). It holds only for libkrun's `init.krun` plus virtio-fs path.
- Record the K7-02 mechanism. libkrunfw's patches turn a PID 1 exit into an orderly reboot, so there is no panic and the status is still 0.
- Update CLAUDE.md's launcher description. `minos-vmm` captures its parent PID at `main` entry, and a PID 1 parent counts as gone (DEFECT-01).
- Size Phase 5's console disk budget as the cap plus at least one 50 ms poll of console throughput (overshoot 30000 to 48954 bytes on a 262144 cap, Pitfall 8), or replace the file console with a capped writer.
- REQUIREMENTS.md: this plan marks SPIKE-01 `[x]` because K7, which D-05 folds into SPIKE-01, is now answered. 02-10's E-3a replacement line and its verify grep both use `- [ ] **SPIKE-01**`, which conflicts with its own rule "leave every checkbox as they are". 02-10 should keep `[x]` when it rewords the line.

## Threat Flags

None. The fix only tightens the watchdog and adds no new surface. T-02-43 through T-02-48 are all mitigated by the recorded runs. T-02-44 is closed by the DEFECT-01 fix (K7-07, K7-11).

## Next Phase Readiness

K7 is answered and the boot half came from 02-06, so SPIKE-01 is complete. 02-10 can record the exit map and the classification rule.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/tests/spike_exits.rs
- FOUND: .planning/phases/02-platform-spikes/evidence/02-07.md
- FOUND: commits 8e52134, 1ce9ac9, dee006c, 8bd6881, 8c5d816
