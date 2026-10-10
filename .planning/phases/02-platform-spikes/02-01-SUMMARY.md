---
phase: 02-platform-spikes
plan: 01
subsystem: infra
tags: [rust, musl, rust-lld, libkrunfw, guest, pid1, vsock, serde, wire-format]
status: complete

requires:
  - phase: 01-workspace-and-gates
    provides: ten-crate workspace, forbid lints, [build] rustflags backstop, deny.toml layering, pinned workspace dependencies
provides:
  - aarch64-unknown-linux-musl target in rust-toolchain.toml, linked with rust-lld (linker key only)
  - B-MUSL proof that the four -F backstop flags reach minos_guest on the musl target
  - every wave-2 dependency edge already in Cargo.lock (package count unchanged at 205)
  - minos-backend-krun module layout (rootfs, initramfs, input_disk, launch_config, supervise)
  - libkrunfw 5.6.2 installed from libkrun/krun and hashed; D-18 kernel config line recorded
  - guest library: MINOSJB1 job decode (E-9), MINOSST1 status frame and report JSON (E-10, E-11), i % 251 pattern
  - static musl /init implementing every E-9 step
affects: [02-03, 02-04, 02-05, 02-06, 02-07, 02-08, 02-09, 02-10, 02-11]

tech-stack:
  added: []
  patterns:
    - "Raw struct with deny_unknown_fields, validated type with try_from and private fields, one InvalidField error naming the field and limit"
    - "Host-compilable guest library plus a single cfg-gated linux.rs; main.rs is the only file that names target_os"
    - "Console writes never panic: writeln + flush with the error dropped, no println!/eprintln!"
    - "Evidence captured to scratch files, home path redacted with sed, appended verbatim"

key-files:
  created:
    - crates/minos-guest/src/lib.rs
    - crates/minos-guest/src/job.rs
    - crates/minos-guest/src/report.rs
    - crates/minos-guest/src/pattern.rs
    - crates/minos-guest/src/linux.rs
    - crates/minos-backend-krun/src/rootfs.rs
    - crates/minos-backend-krun/src/initramfs.rs
    - crates/minos-backend-krun/src/input_disk.rs
    - crates/minos-backend-krun/src/launch_config.rs
    - crates/minos-backend-krun/src/supervise.rs
    - .planning/phases/02-platform-spikes/evidence/02-01.md
  modified:
    - rust-toolchain.toml
    - .cargo/config.toml
    - Cargo.toml
    - Cargo.lock
    - CLAUDE.md
    - crates/minos-guest/Cargo.toml
    - crates/minos-guest/src/main.rs
    - crates/minos-vmm/Cargo.toml
    - crates/minos-backend-krun/Cargo.toml
    - crates/minos-backend-krun/src/lib.rs

key-decisions:
  - "Cargo rejects panic in a [profile.*.package.*] override (P-PANIC); minos-guest keeps unwinding, and a PID 1 panic surfaces as no status frame (Crashed)"
  - "The guest reaps orphans with rustix::process::wait(NOHANG), not waitpid(None, ..), which in rustix 1.x waits only on the caller's process group"
  - "Job validation errors are structured: JobError::Invalid(InvalidField { field, limit }) with dotted field names such as steps.connect.targets"
  - "libkrunfw canonical path /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib, SHA-256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217"

patterns-established:
  - "Golden wire bytes are plain assert_eq! against hex literals, never insta"
  - "Every guest step failure is a field in its result; run() returns early only for the job's return finish"

# SPIKE-01 (VM boot) and SPIKE-04 (signed helper starts a VM) are contributed to, not completed: the boot and
# codesign spikes run in later plans, so REQUIREMENTS.md is left unchanged.
requirements-completed: []

coverage:
  - id: D1
    description: "musl target in the pinned toolchain with a linker-only target table, and the forbid backstop proven on that target"
    requirement: SPIKE-01
    verification:
      - kind: other
        ref: "evidence/02-01.md TC-01, B-MUSL (cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -v -- -D warnings)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Static aarch64 musl guest binary"
    requirement: SPIKE-01
    verification:
      - kind: other
        ref: "evidence/02-01.md L-MUSL, G-GUEST (file reports statically linked ARM aarch64)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Guest job decoder, status-frame encoder, and byte pattern with golden bytes"
    requirement: SPIKE-04
    verification:
      - kind: unit
        ref: "cargo test -p minos-guest (18 tests in job, report, pattern)"
        status: pass
    human_judgment: false
  - id: D4
    description: "libkrunfw installed from libkrun/krun and hashed; D-18 kernel config recorded; Docker and claude state recorded read-only"
    requirement: SPIKE-01
    verification:
      - kind: other
        ref: "evidence/02-01.md H-01, H-02, H-03"
        status: pass
    human_judgment: false
  - id: D5
    description: "Lock gains edges only, so wave-2 plans touch no shared file"
    requirement: SPIKE-04
    verification:
      - kind: other
        ref: "evidence/02-01.md LK-01 (205 before, 205 after; cargo deny check exit 0)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The PID 1 path (linux.rs) behaves correctly inside a VM"
    requirement: SPIKE-01
    verification: []
    human_judgment: true
    rationale: "linux.rs compiles and lints only for the musl target; no Linux runner exists, so its first execution is the VM boot spikes (02-06 to 02-09)"

duration: 12min
completed: 2026-10-10
---

# Phase 2 Plan 01: Foundation and Guest Summary

**musl guest target linked by rust-lld with the forbid backstop proven by `-v` inspection, every wave-2 lock edge added, libkrunfw 5.6.2 installed and hashed, and a static `/init` that speaks the MINOSJB1 job and MINOSST1 status-frame protocols defined here with golden bytes.**

## Performance

- **Duration:** about 12 min
- **Started:** 2026-10-10T12:41:20Z
- **Completed:** 2026-10-10T12:53Z
- **Tasks:** 4
- **Files modified:** 21 (11 created, 10 modified)

## Accomplishments

- `rust-toolchain.toml` lists `aarch64-unknown-linux-musl` with the D-03 reason. `.cargo/config.toml` gives it `linker = "rust-lld"` and nothing else. B-MUSL shows both `--crate-name minos_guest` clippy-driver lines carry `-Funsafe_code -Fclippy::unwrap_used -Fclippy::expect_used -Fclippy::indexing_slicing`. No forbidden construct was written.
- P-PANIC: Cargo **rejected** the override with `` `panic` may not be specified in a `package` profile `` (exit 101). The `[profile.release]` comment now says so (E-3).
- LK-01: `Cargo.lock` gained 8 dependency lines and no packages. `grep -c '^name = '` was 205 before and after. `cargo deny check` passes.
- `minos-backend-krun` declares its five modules. Each module file holds one `//!` line for 02-04 and 02-05 to fill.
- CLAUDE.md Commands: the musl clippy line now says Phase 2 onward, the musl build line is new, and the D-03 paragraph replaces the old Phase 5 paragraph.
- H-01: libkrunfw 5.6.2 was poured from a bottle (`arm64_tahoe`). Nothing was built from source.
  - Canonical path: `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`
  - SHA-256: `59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217`
  - krunkit, krunvm, and libkrun are not installed.
- H-02: the v5.6.2 config has `# CONFIG_BLK_DEV_INITRD is not set` at line 210, which confirms D-18. It also sets `CONFIG_SQUASHFS_ZLIB=y`, `CONFIG_DEVTMPFS_MOUNT=y`, `CONFIG_VIRTIO_BLK=y`, `CONFIG_VIRTIO_VSOCKETS=y`, and `CONFIG_TSI=y`.
- H-03: Docker 29.6.1 and buildx v0.35.0-desktop.2 were already running. The canonical `claude` is `~/.local/share/claude/versions/2.1.296`. Nothing was started.
- Guest library: `read_job` makes three fixed reads for the header and caps the length before reading any payload. It reads through `Read::take`, deserializes into `RawJob` with `deny_unknown_fields`, then validates with `Job::try_from`. `encode_frame` produces the exact E-10 golden bytes. `pattern_byte` is `i % 251`. 18 unit tests pass on macOS.
- Guest `/init`: `linux::run` prints `MINOS-GUEST-BOOT-OK` first and implements report_boot, connect, vsock_connect, vsock_stream, scratch_roundtrip, run, console_flood, and hang. It sends one frame to CID 2, then powers off or returns as the job says. The binary is a statically linked aarch64 ELF of 1,022,016 bytes.
  - SHA-256: `10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89` (built from commit 9e9058f's tree)

## Task Commits

1. **Task 1: Toolchain target, linker, backstop proof, manifests, lock, module layout, CLAUDE.md**: `1d3f2bf` (chore)
2. **Task 2: Host prerequisites H-01 to H-03**: `f50581a` (docs)
3. **Task 3: Guest library (TDD)**: RED `77e935a` (test), GREEN `47c291f` (feat). No refactor commit.
4. **Task 4: Guest PID 1 path and static musl build**: `9e9058f` (feat)

## Files Created/Modified

- `rust-toolchain.toml`: musl target with the D-03 reason (E-1)
- `.cargo/config.toml`: `[target.aarch64-unknown-linux-musl] linker = "rust-lld"` (E-2)
- `Cargo.toml`: the P-PANIC comment (E-3)
- `Cargo.lock`: 8 new dependency edges, no new packages
- `CLAUDE.md`: Commands section (E-4a, E-4b, E-4c)
- `crates/minos-guest/Cargo.toml`, `crates/minos-vmm/Cargo.toml`, `crates/minos-backend-krun/Cargo.toml`: E-5, E-6, E-7
- `crates/minos-backend-krun/src/{lib,rootfs,initramfs,input_disk,launch_config,supervise}.rs`: module layout (E-8)
- `crates/minos-guest/src/lib.rs`: markers, protocol constants, test hex helper
- `crates/minos-guest/src/job.rs`: `JOB_MAGIC`, limits, `read_job`, `RawJob`, `Job`, `RawStep`, `Step`, the eight step types, `Finish`, `JobError`, `InvalidField`
- `crates/minos-guest/src/report.rs`: `FRAME_MAGIC`, `Report`, `StepResult`, the result structs, `encode_frame`, `FrameError`
- `crates/minos-guest/src/pattern.rs`: `pattern_byte`, `fill_pattern`
- `crates/minos-guest/src/main.rs`: gates `mod linux`; on non-Linux it prints a message and returns failure
- `crates/minos-guest/src/linux.rs`: the PID 1 path
- `.planning/phases/02-platform-spikes/evidence/02-01.md`: TC-01, B-MUSL, L-MUSL, P-PANIC, LK-01, H-01, H-02, H-03, G-GUEST

## Decisions Made

- Cargo rejects the per-package panic override, so the guest keeps unwinding (E-3 applied).
- Validation errors carry a dotted field name and a limit in words, for example `steps.run.argv` with "led by an absolute path". This is one `InvalidField` type reached through `JobError::Invalid`.
- `RawJob` holds `Vec<RawStep>`, so `Job::try_from` validates steps through `Step::try_from`. `Step` also derives `Deserialize` through `try_from = "RawStep"`, so both paths share one constructor.
- `BootResult::new` sorts env, interfaces, and virtio devices itself, so the caller's order never reaches the wire. The `marker` field is fixed to `BOOT_MARKER` and cannot be set by the caller.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Backticks around `SquashFS` in the rootfs.rs module doc**
- **Found during:** Task 1 (workspace clippy)
- **Issue:** The exact E-8 line fails `clippy::doc_markdown` (pedantic, `-D warnings`).
- **Fix:** `SquashFS` became `` `SquashFS` ``. The rest of the line is unchanged.
- **Files modified:** crates/minos-backend-krun/src/rootfs.rs
- **Commit:** 1d3f2bf

**2. [Rule 1 - Bug] Orphan reaping uses `wait(NOHANG)`, not `waitpid(None, NOHANG)`**
- **Found during:** Task 4 (reading rustix 1.1.5 `src/process/wait.rs`)
- **Issue:** In rustix 1.x, `waitpid(None, ..)` waits only for children in the caller's process group (pid 0). An orphan that called `setsid` or `setpgid` would never be reaped by PID 1.
- **Fix:** `rustix::process::wait(WaitOptions::NOHANG)` (any child), looped until `Ok(None)` or an error. `NOHANG` still appears as the acceptance grep expects.
- **Files modified:** crates/minos-guest/src/linux.rs
- **Commit:** 9e9058f

**3. [Rule 1 - Bug] Exact written counts and no index overflow**
- **Found during:** Task 3 and Task 4
- **Issue:** `write_all` cannot report how many bytes were written before a mid-chunk error, which the vsock_stream and console_flood results need. Separately, `zip(offset..)` panics under `overflow-checks` if it steps past `u64::MAX`.
- **Fix:** A `write_counted` loop around `write` that retries on `Interrupted` and treats `Ok(0)` as `WriteZero`. `fill_pattern` and the flood loop zip with `offset..=u64::MAX`, which ends instead of overflowing. Scratch writes and the status frame still use `write_all`.
- **Files modified:** crates/minos-guest/src/pattern.rs, crates/minos-guest/src/linux.rs
- **Commits:** 47c291f, 9e9058f

**4. [Rule 1 - Bug] `run` drains each pipe past the cap**
- **Found during:** Task 4
- **Issue:** Reading only `take(cap + 1)` would leave a chatty child blocked on a full pipe, so `wait` would never return.
- **Fix:** After the capped read, the rest of the pipe is drained into `io::sink()` and discarded. Memory stays bounded.
- **Files modified:** crates/minos-guest/src/linux.rs
- **Commit:** 9e9058f

### Other notes

- **One acceptance check contradicts the plan's own text:** `grep -c 'Phase 2 onward (02-CONTEXT.md D-03)' CLAUDE.md` prints 2, not 1. E-4a (the clippy line comment) and E-4c (the paragraph, "runs from Phase 2 onward (02-CONTEXT.md D-03)") both contain the string. Both exact texts were applied verbatim; `grep -c 'Phase 5 onward' CLAUDE.md` prints 0.
- **H-01:** `brew install` and `brew info` ran with `HOMEBREW_NO_AUTO_UPDATE=1`, because `brew tap` had already auto-updated. The evidence states this and records the commands as run.
- **H-03:** `command -v claude` resolves to a cmux CLI shim under `/var/folders/...`, not `~/.local/bin/claude`. 02-07 should invoke the canonical path, or record which one it ran.
- **H-02 for K3:** the kernel has `CONFIG_TSI=y`, so the guest kernel supports TSI. Air-gapping depends entirely on the VMM leaving the hijack off (msb_krun `inet_hijack` defaults to false). The egress spike should treat this as a live risk.

## TDD Gate Compliance

The RED commit `77e935a` (`test(02-01)`) had all 18 tests failing against stub bodies. The GREEN commit `47c291f` (`feat(02-01)`) passes all 18. No refactor commit was needed.

## Known Stubs

- `crates/minos-backend-krun/src/{rootfs,initramfs,input_disk,launch_config,supervise}.rs`: one `//!` line each, as E-8 intends. 02-04 fills rootfs and initramfs; 02-05 fills input_disk, launch_config, and supervise.

## Issues Encountered

None beyond the deviations above.

## Requirements Status

The plan frontmatter lists SPIKE-01 and SPIKE-04. This plan supplies prerequisites for both: the static `/init`, libkrunfw, and the lock edges `minos-vmm` needs. It satisfies neither. Nothing has booted yet, and no signed helper has started a VM. `requirements.mark-complete` was run and then reverted, so REQUIREMENTS.md still shows both as pending. The plans that run the boot (02-06) and the signing spike should mark them complete.

## User Setup Required

None. The host changes were the D-10 pre-approved `brew tap libkrun/krun`, `brew trust libkrun/krun`, and `brew install libkrunfw`.

## Next Phase Readiness

- 02-03, 02-04, and 02-05 can run in parallel worktrees without touching `Cargo.lock`.
- 02-05 can embed the canonical E-9 job JSON and decode MINOSST1 frames against the same golden bytes.
- 02-04 has the guest binary path and SHA-256 to inject as `/init`.
- `cargo clean` was not run, per the plan; 02-11 runs it at the end of the phase.

## Self-Check: PASSED

- All 11 created files exist on disk.
- Commits 1d3f2bf, f50581a, 77e935a, 47c291f, and 9e9058f are present in `git log`.
- Gate: `cargo fmt --all --check`, workspace clippy, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`, and musl clippy all exit 0. C-01 prints ten `ok` lines; the C-02 and unsafe greps print nothing.
