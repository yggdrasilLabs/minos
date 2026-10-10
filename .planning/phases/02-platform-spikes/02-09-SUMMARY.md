---
phase: 02-platform-spikes
plan: 09
subsystem: infra
tags: [rust, macos, seatbelt, sandbox-exec, libkrun, minos-vmm, spike, k6]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-05: supervise::run and Launch::new taking the full argv; 02-06: E-1 environment, E-4 signing with the K5 two-key entitlement set, root-disk boot; 02-08: the 8 KiB-per-connection vsock limit (the boot frame is 761 bytes)"
provides:
  - "crates/minos-vmm/minos-vmm.sb: a parameterized deny-default Seatbelt profile under which minos-vmm creates and runs a VM"
  - "Ignored tests boot_under_seatbelt, profile_denies_network, profile_denies_unlisted_file and the seatbelt_argv helper (spike_seatbelt.rs)"
  - "SPIKE-05 answer: yes; hv_vm_create needs no hypervisor-specific rule"
  - "The dedicated _minos user fallback recorded as a recipe only, with its launch-as-user open issue (D-10)"
affects: [02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "Seatbelt allowances come only from logged denials that stopped a run; each carries a ; comment naming the denial and the iteration"
    - "Ancestor stats for canonicalize use (path-ancestors (param ...)), so no version or job directory is hard-coded"
    - "A profile's denials are proven on other binaries (nc, cat) by swapping only the exec parameter, beside an unsandboxed control"

key-files:
  created:
    - crates/minos-vmm/minos-vmm.sb
    - crates/minos-backend-krun/tests/spike_seatbelt.rs
    - .planning/phases/02-platform-spikes/evidence/02-09.md
  modified: []

key-decisions:
  - "SPIKE-05: yes. minos-vmm boots under a (deny default) Seatbelt profile after three allowances beyond E-1: sysctl-read hw.pagesize_compat (Rust std's guard page), file-read-metadata of path-ancestors of KRUNFW, KRUNFW_LINK, CONFIG (preflight's canonicalize), and file-read-metadata of the SOCK literal (preflight's socket check)"
  - "hv_vm_create needed no hypervisor-specific rule: no iokit-open or mach-lookup allowance; kern.hv_vmm_present stays denied without effect"
  - "The profile's only network allowance is network-outbound to the literal vsock route socket; removing it denies the AF_UNIX connect and no frame arrives (S05-CTRL-SOCK)"
  - "Phase 5 can report Seatbelt as an active local confinement layer in BackendInfo (KRUN-07) when the helper is launched through /usr/bin/sandbox-exec with this profile; the dedicated _minos user is not active (recipe only), so every local run records reduced local hardening for that layer"

patterns-established:
  - "A confinement test checks the frame, not the helper's exit code: under a broken profile the helper still exits 0"

requirements-completed: [SPIKE-05]

duration: about 10 min
completed: 2026-10-10
---

# Phase 2 Plan 09: Seatbelt

**SPIKE-05 is yes: `minos-vmm` creates and runs a VM under a deny-default Seatbelt profile, with no hypervisor-specific rule. Four iterations were needed, three allowances were added from logged denials, and the only network allowance is the AF_UNIX connect to the vsock route socket. The same profile denies `nc` an outbound TCP connection and denies `cat` an unlisted file while a listed file stays readable. The dedicated `_minos` user is written down as a recipe and was not created (D-10).**

## Performance

- **Duration:** about 10 min
- **Started:** 2026-10-10T15:41:00Z (plan base `a63968f`)
- **Completed:** 2026-10-10T15:50Z
- **Tasks:** 2 of 2
- **Files modified:** 3 (`minos-vmm.sb`, `tests/spike_seatbelt.rs`, `evidence/02-09.md`)

## Accomplishments

- `minos-vmm.sb` starts from 02-09 E-1 and gets every path as a canonical `-D` parameter. No `/Users/` path, no general network rule, no default allow.
- `spike_seatbelt.rs` holds the three ignored tests and `seatbelt_argv`. All three pass on the final profile in one run (S05-BOOT).
- Every iteration is recorded with its full profile text, its test output, and its denial log (S05-ITER-1 to S05-ITER-4).
- Two controls back the claims. Without the socket rule the route connect is denied (S05-CTRL-SOCK). Unsandboxed, `nc` connects to `1.1.1.1:443` (S05-DENY-NET).
- The `_minos` recipe and its open issue are in S05-RECIPE. `dscl . -read /Users/_minos` returns `eDSRecordNotFound`.

## SPIKE-05 answer

Yes. With the final profile, `boot_under_seatbelt` exits 0. The console shows `MINOS-GUEST-BOOT-OK`, one valid v1 status frame (761 bytes) arrives, and the guest's connect to `1.1.1.1:443` fails with `ENETUNREACH`.

| # | Allowance | The denial or file it answers | Evidence |
|---|---|---|---|
| 1 | `process-exec` of `VMM` | `sandbox-exec` execs the helper, and only the helper | E-1; S05-BOOT |
| 2 | `file-read*` of `/` (literal), `/usr/lib`, `/System/Library`, `/private/var/db/dyld`, `/dev/null`, `/dev/urandom` | dyld, libSystem, the linked frameworks | E-1; S05-BOOT |
| 3 | `file-read*` of `VMM`, `CONFIG`, `KRUNFW`, `KRUNFW_LINK`, `ROOT_IMG`, `INPUT_IMG` | helper, config, libkrunfw (Cellar and symlink), read-only disks | E-1; S05-BOOT |
| 4 | `file-read*` and `file-write*` of `SCRATCH_IMG`, `CONSOLE_LOG` | scratch disk, console file | E-1; S05-BOOT |
| 5 | `network-outbound` to the literal `SOCK` path | the AF_UNIX connect of the vsock route | E-1; S05-CTRL-SOCK |
| 6 | `sysctl-read` of `hw.pagesize`, `hw.ncpu`, `hw.memsize`, `hw.optional.arm.*`, `kern.proc.pid.*` | E-1 starting set | E-1 |
| 7 | `sysctl-read` of `hw.pagesize_compat` | std aborted with "failed to allocate a guard page" | S05-ITER-1 |
| 8 | `file-read-metadata` of `path-ancestors` of `KRUNFW`, `KRUNFW_LINK`, `CONFIG` | `deny(1) file-read-metadata /opt`; preflight canonicalize | S05-ITER-2 |
| 9 | `file-read-metadata` of the literal `SOCK` path | preflight's is-a-socket check | S05-ITER-3 |

Rows 1 to 6 are the E-1 starting set and were not minimized one at a time. Only row 5 was tested by removing it. Rows 7 to 9 each carry a `;` comment in the profile. Denials that did not affect the run stay denied: OS-version and boot-args sysctls, `kern.hv_vmm_present`, `vfs.disk-space`, `/var` and `/etc` metadata, the Cryptex probes, and `mach-lookup` of `com.apple.logd`, the notification center, and opendirectoryd.

### Confinement layers Phase 5 can report (KRUN-07)

| Layer | Status | Phase 5 records |
|---|---|---|
| Hardware virtualization (Hypervisor.framework) | active | always |
| No network device, no TSI, no virtio-fs | active | always |
| Seatbelt deny-default profile on `minos-vmm` | can be active | Only when launched through `/usr/bin/sandbox-exec` with this profile. Record the profile SHA-256 and the `sandbox-exec` SHA-256. If either is missing, the layer is recorded as absent; never a silent fallback |
| Dedicated `_minos` user | not active (recipe only) | Absent, and recorded as reduced local hardening in provenance, until `minos host setup` solves launch-as-user (sudoers rule or LaunchDaemon) |
| seccomp, cgroups | not on macOS | Recorded as absent |

## Task Commits

1. **Task 1: Starting profile and the Seatbelt spike tests** - `3c72528` (test)
2. **Task 2: Iterate on logged denials, answer SPIKE-05, record the dedicated-user recipe** - `d26391b` (docs)

## Files Created/Modified

- `crates/minos-vmm/minos-vmm.sb` - parameterized deny-default profile (E-1 plus three commented allowances)
- `crates/minos-backend-krun/tests/spike_seatbelt.rs` - `boot_under_seatbelt`, `profile_denies_network`, `profile_denies_unlisted_file`, `seatbelt_argv`
- `.planning/phases/02-platform-spikes/evidence/02-09.md` - 09-00, S05-ITER-1..4, S05-CTRL-SOCK, S05-BOOT, S05-DENY-NET, S05-DENY-FILE, S05-RECIPE, SPIKE-05 answer

## Decisions Made

- The ancestor-metadata allowance uses Seatbelt's `path-ancestors` filter on parameters instead of literal ancestor paths. Literal paths would hard-code the libkrunfw version and the job directory. `system.sb` uses the same filter, and ITER-3 shows it accepts `(param ...)`.
- `KRUNFW_LINK` gets only its parent canonicalized. The parameter stays the symlink path, as E-1 intends.
- Outcome A applies, so `boot_under_seatbelt` keeps its name. No `seatbelt_blocks_vm_creation` test was needed.

## Deviations from Plan

None in scope. Two additions go beyond the plan's minimum:

- **S05-CTRL-SOCK (added evidence, no code):** one extra run with the socket connect rule temporarily removed from the profile, then restored. `cmp` against the saved copy matched. The run proves 02-RESEARCH.md Pitfall 4 on this host, and shows that the helper exits 0 when the profile breaks its output path.
- **`minos-vmm` re-signed before ITER-1:** the binary had been relinked (`flags=0x20002(adhoc,linker-signed)`, no entitlements). It was re-signed with the 02-06 E-4 argv and recorded in 09-00. That is the documented procedure, not a code change.

No shared code was edited. `git diff --name-only a63968f HEAD` lists only this plan's three files.

## Issues Encountered

- zsh's `log` builtin shadows `/usr/bin/log` (`(eval):log:1: too many arguments`), so every denial-log command names `/usr/bin/log` in full.
- `sandbox-exec` has no version flag. 09-00 records its SHA-256 and the "DEPRECATED" text from its man page. It printed no deprecation warning at run time.

## Carry-forward for Phase 5

- Launch the helper as `["/usr/bin/sandbox-exec", "-f", <profile>, "-D", "NAME=value", ..., <minos-vmm>, <vmm.json>]` with every value canonical. `Launch::new` already takes this argv unchanged.
- If preflight gains a new path check, the profile probably needs a matching metadata rule. Re-run `boot_under_seatbelt` after any preflight change.
- `host doctor` should run the S05-DENY-NET and S05-DENY-FILE equivalents before it claims the Seatbelt layer, because `sandbox-exec` is deprecated.
- The `_minos` launch-as-user design is open: it needs sudoers or a LaunchDaemon, job-directory ownership or ACLs, and access to the supervisor socket.

## User Setup Required

None.

## Next Phase Readiness

SPIKE-05 is closed. 02-11 Task 2 re-runs `spike_seatbelt.rs` from the final tree, and needs `minos-vmm` signed per 02-06 E-4 first. `cargo clean` was not run, as the plan requires (02-11 runs it).

## Self-Check: PASSED

- FOUND: crates/minos-vmm/minos-vmm.sb
- FOUND: crates/minos-backend-krun/tests/spike_seatbelt.rs
- FOUND: .planning/phases/02-platform-spikes/evidence/02-09.md
- FOUND: 3c72528
- FOUND: d26391b
- Gate: fmt, workspace clippy, workspace test, deny, doc, and guest musl clippy all exit 0. C-01 prints ten `ok`. C-02 and the unsafe grep print nothing. All three ignored seatbelt tests pass.
