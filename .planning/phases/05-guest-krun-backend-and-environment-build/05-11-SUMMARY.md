---
phase: 05-guest-krun-backend-and-environment-build
plan: 11
subsystem: minos-backend-krun
tags: [krun, host-probes, preflight, seatbelt, entitlements, libkrunfw, jobdir, sweep, krun-05, krun-06, krun-07, krun-08]
status: complete
requires:
  - "05-06: Capabilities::new(BackendKind, ...).with_layers(...).with_kernel(...) and the Refusal variants VmmMissing, GuestMissing, EntitlementMissing, KernelMissing, KernelHashMismatch, HypervisorUnsupported, SocketPathTooLong, SeatbeltMissing, SeatbeltNotEnforcing"
  - "05-09: seatbelt::{PROFILE, SANDBOX_EXEC, ProfileParams, argv} and rootfs::check_guest_elf"
  - "supervise.rs (kept by 05-15): sha256_file and MAX_SOCKET_PATH_BYTES only"
provides:
  - "host::{KRUNFW_PATH, KRUNFW_SHA256, KRUNFW_LINK, REQUIRED_ENTITLEMENTS, TOOL_PATH, TOOL_DEADLINE, CODESIGN_OUTPUT_CAP, CODESIGN, SYSCTL, NC, CAT}"
  - "host::HostPaths::new(vmm, guest, krunfw) with vmm(), guest(), krunfw()"
  - "host::probe_host(&HostPaths, job_root) -> HostReport { probes(), capabilities(), into_capabilities() }"
  - "host::Check (nine checks, name()), host::Probe { check(), paths(), outcome(), is_passed(), refusal() }, host::ProbeOutcome { Passed { detail: SafeText }, Refused { refusal, detail: SafeText } }"
  - "host::entitlements_present(xml) and host::signed_with_entitlements(vmm) -> io::Result<bool>"
  - "pub(crate) host::run_tool and ToolOutput: absolute-path argv, env cleared to TOOL_PATH, stdin null, capped stdout and stderr, own process group, killed at TOOL_DEADLINE"
  - "jobdir::{JOB_ROOT_PARENT, ROOT_PREFIX, JOB_PREFIX, JOB_RANDOM_CHARS, OWNER_LOCK_FILE, PID_FILE, CONFIG_FILE, INPUT_FILE, SOCKET_FILE, MAX_PID_FILE_BYTES}"
  - "jobdir::JobRoot::{open, open_at, path, create_job}, jobdir::JobDir::{path, config, input, socket, pid_file, owner_lock, write_pid}, jobdir::JobDirError"
  - "jobdir::sweep(&JobRoot, vmm) -> SweepReport { entries() } of SweepEntry::{Removed, Killed, Live, Unverified}"
affects:
  - "05-15: KrunBackend::new calls probe_host once and keeps into_capabilities(); its supervisor uses JobRoot::create_job, writes the helper pid with write_pid, and holds the JobDir for the job's life; it runs sweep at start"
  - "05-15: spawn the helper through seatbelt::argv with the canonical vmm path and <job dir>/vmm.json as its only argument, in its own process group, or the sweep's exact-argv match never fires"
  - "Phase 8 minos host doctor: prints HostReport::probes() (check name, paths, sanitized detail, refusal Display) and the SweepReport"
tech-stack:
  added: []
  patterns:
    - "Self-checking denial probes: a positive control (listed read under the profile, unsandboxed loopback connect) must succeed before a denial counts, and an inconclusive probe fails closed"
    - "Layers derived from probe outcomes by one pure function, tested exhaustively over all 64 outcome combinations"
    - "Prove before acting on pids: a free owner lock, a bounded pid file, and a ps row with ppid 1 and the exact argv, else Unverified and nothing is signalled"
key-files:
  created:
    - crates/minos-backend-krun/src/host.rs
    - crates/minos-backend-krun/src/jobdir.rs
  modified:
    - crates/minos-backend-krun/Cargo.toml
    - crates/minos-backend-krun/src/lib.rs
    - Cargo.lock
    - CLAUDE.md
decisions:
  - "The network denial probe runs only after the file probe's listed read under the same profile succeeds, so a sandbox-exec that failed to apply the profile (non-zero exit without running nc) can never read as a network denial"
  - "Probe is keyed by a Check enum with name() rather than a &'static str, and a Refused outcome carries a sanitized detail as well as the Refusal, so doctor can say why"
  - "A check counts as passed only when it ran and every probe of it passed; the entitlement check fails closed (EntitlementMissing) when the helper is missing"
  - "When sysctl cannot report the host's size, the hypervisor check refuses and the host maximum is the smallest machine (1 vCPU, MIN_MEMORY_MIB), a const checked at compile time"
  - "ps runs with -ww so a long helper path is never truncated before the exact-argv comparison"
  - "The sweep leaves a job directory that has no owner lock yet (a job being created) as Unverified rather than removing it"
  - "libkrunfw is loaded from the Cellar path after its hash check and not copied into a private directory (05-RESEARCH Open Question 6), recorded in CLAUDE.md as an assumption flagged for the user"
metrics:
  duration: 15min
  completed: 2026-10-18
  tasks: 3
  files: 6
---

# Phase 5 Plan 11: Host Probes and Job Directories Summary

**`probe_host` checks the helper's two entitlement keys, the pinned libkrunfw hash, `kern.hv_support`, the socket path length, `sandbox-exec`, two offline Seatbelt denial probes with controls, and a static aarch64 guest, then turns the results into `Capabilities` and explained `Refusal`s. The layers are Seatbelt and Rlimits, never DedicatedUser. A `jobdir` module adds 0700 job directories, each with an exclusive `owner.lock` and a `pid` file, and a sweep that kills a helper only once `ps` proves it is an orphan with this job's exact argv.**

## What was built

### Task 1: host probes (KRUN-06, KRUN-07, KRUN-08; D-01 to D-04)
- `probe_host(paths, job_root)` runs nine checks in order. Each failure is one `Refusal` whose `Display` carries the fix: helper present (`VmmMissing`), both entitlement keys verified with `codesign --display --entitlements - --xml` (`EntitlementMissing`), libkrunfw present and matching `59764391…e5217` (`KernelMissing` or `KernelHashMismatch`), `kern.hv_support` plus `hw.ncpu` and `hw.memsize` (`HypervisorUnsupported`), the longest socket path `<job_root>/minos-<uid>/mnsXXXXXXXX/o.sock` within 103 bytes (`SocketPathTooLong`), `/usr/bin/sandbox-exec` (`SeatbeltMissing`), both denial probes (`SeatbeltNotEnforcing`), and `check_guest_elf` on the guest (`GuestMissing`).
- Largest VM: `min(8, ncpu)` vCPUs and `min(16384, memsize_mib / 2)` MiB. On this Mac that is 8 vCPUs and 16384 MiB.
- Capabilities: `Krun`, `Arm64`, `Hardware`, `MAX_LAUNCH_MS`, the libkrunfw digest as the kernel (only when it matched), and the layers from `layers_for`.
- Denial probes run in a 0700 `mnp*` directory under the job root, which is removed afterwards. Under the profile with `VMM=/bin/cat`, `cat` must read the listed file back exactly and must fail on an unlisted file. Then an unsandboxed `nc -z -G 2 127.0.0.1 <port>` must reach a probe-owned listener, and the same `nc` under the profile (`VMM=/usr/bin/nc`) must exit non-zero while the listener accepts nothing.
- Every tool runs by absolute path with the environment cleared to `TOOL_PATH`, stdin `/dev/null`, capped output, its own process group, and a 5 s deadline. Production code reads no environment variable.

### Task 2: job directories and the sweep (KRUN-05)
- `JobRoot::open_at` creates `<parent>/minos-<uid>` with mode 0700. It refuses a root that is a symlink, a non-directory, another user's, or not mode 0700.
- `create_job` makes a 0700 `mns` + 8 alphanumerics directory, creates `owner.lock` with `create_new`, and holds an exclusive `File::lock` on it until the `JobDir` is dropped. On drop the directory is removed first and the lock released second.
- `sweep` looks only at `mns????????` directories, in name order:
  - If `try_lock` returns `WouldBlock`, the entry is Live.
  - If the lock is free, the sweep reads the pid file: at most 32 bytes of plain decimal, a pid above 1 that fits an i32.
  - It then runs `/bin/ps -ww -o ppid=,command= -p <pid>`. If ps shows no row, the directory is removed.
  - Only a row with ppid 1 and exactly `<vmm> <dir>/vmm.json` gets SIGKILL to its process group. The directory is removed once ps shows no row.
  - Anything else is Unverified and the directory is left: `EPERM` or any errno other than `ESRCH`, a failed ps, a mismatched row, or a bad pid file.

### Task 3: CLAUDE.md
- Seatbelt is mandatory, with the offline self-checking probes (D-03).
- The `_minos` user is deferred and recorded as missing, so every v1 run shows reduced hardening. `host setup` signs the helper and verifies it (D-01, D-02).
- The recorded layers and their conditions are written down.
- Fixed locations with the pinned SHA-256 (D-04). libkrunfw is not copied into a private directory (Open Question 6).
- The job-directory layout and the sweep's guarantees are written down.
- The libkrunfw-copy open question is removed, and the `_minos` start question is marked "Deferred (D-01):".

## Verification

- `cargo test -p minos-backend-krun --lib host`: 14 passed, 2 ignored. That count includes the crate's other modules, because the filter is a substring.
- `cargo test -p minos-backend-krun --lib jobdir`: 8 passed. It was run five times with no flake.
- `minos-vmm` was rebuilt and re-signed with the CLAUDE.md codesign argv, and the musl guest was built. Then `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --lib host -- --ignored --nocapture --test-threads=1` passed both ignored tests:
  - `probe_host_on_this_mac`: all nine checks passed, no refusals, layers {Seatbelt, Rlimits}, a kernel digest present, the socket path 41 of 103 bytes.
  - `seatbelt_denial_probes_hold`: the control connected, the sandboxed `nc` exited 1 and the listener accepted nothing, the listed file read back, and `cat` on the unlisted file got "Operation not permitted".
- A manual negative run with `MINOS_VMM=/bin/cat` and the macOS (Mach-O) `minos-guest` refused with `{EntitlementMissing, GuestMissing}`.
- Gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (no failures), `cargo deny check`, `cargo doc --workspace --no-deps` (no warnings), and the musl guest clippy all exit 0. C-01 prints no MISSING line. C-02 prints nothing.
- Acceptance greps all match:
  - `pub fn probe_host`, `pub fn entitlements_present`, the disable-library-validation key, and the pinned hash.
  - 0 `env::var` uses before `#[cfg(test)]`.
  - No supervise import beyond `sha256_file` and `MAX_SOCKET_PATH_BYTES`.
  - `CODESIGN_OUTPUT_CAP: u64 = 65_536`, `pub fn sweep`, `try_lock`, `JOB_ROOT_PARENT`, `Unverified` (8 lines).
  - In CLAUDE.md: Seatbelt is mandatory, owner.lock, not copied into a private directory, and Deferred (D-01).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] The network denial needs a positive control under the profile**
- **Found during:** Task 1
- **Issue:** If `sandbox-exec` failed to apply the profile, it would exit non-zero without running `nc`, and that would read as "network denied".
- **Fix:** The network probe runs only after the file probe's listed read under the same profile has succeeded. Otherwise it is inconclusive and refused as `SeatbeltNotEnforcing`.
- **Files modified:** crates/minos-backend-krun/src/host.rs
- **Commit:** 9c8ccf3

**2. [Rule 1 - Bug] `ps` would truncate a long command line**
- **Found during:** Task 2
- **Issue:** BSD `ps` cuts the command column to the terminal width (79 columns when there is no tty). A long helper path would then never match the exact argv.
- **Fix:** The argv is `/bin/ps -ww -o ppid=,command= -p <pid>`.
- **Files modified:** crates/minos-backend-krun/src/jobdir.rs
- **Commit:** 1335d42

**3. [Rule 2] Probe shape**
- `Probe.check` is a `Check` enum with `name()` instead of a `&'static str` (rules: no stringly typing).
- `ProbeOutcome::Refused` carries a sanitized `detail` next to the `Refusal`, so doctor can show why a check failed. `detail` is stored as `SafeText`, not as a `String` that has passed through it.
- **Commit:** 9c8ccf3

**4. Layout constants moved to jobdir**
- Task 2 moved the job layout constants (`ROOT_PREFIX`, `JOB_PREFIX`, `JOB_RANDOM_CHARS`, `SOCKET_FILE`) into `jobdir`.
- `host.rs`'s socket-length check now uses those constants, so the probe and the real layout cannot drift.
- `run_tool` is `pub(crate)` so the sweep runs `ps` through the same runner.
- **Commit:** c6fb62e (RED commit of Task 2, which touched host.rs)

**5. Extra test: `sweep_kills_an_orphaned_helper`**
- **What:** It orphans `/bin/cat <dir>/vmm.json` (blocked on a FIFO) into its own process group under launchd. It asserts the sweep reports Killed, removes the directory, and the process is gone. This proves the kill path, not only the guards.
- **Why the shape:** The shell is given the FIFO path as `$1`, never as shell text.
- **Also:** `sweep_guards_pid_reuse` also checks an exact-argv `cat` that is a child of the test (its ppid is not 1), as well as the `/bin/sleep` case.
- **Incident while writing it:** The first draft left the orphan's stdout on the shell's pipe and hung. The stray process, which the test itself had spawned, was killed, and its output is now redirected to `/dev/null`.
- **Commit:** 1335d42

## Requirements

KRUN-05, KRUN-06, KRUN-07, and KRUN-08 are not marked complete in REQUIREMENTS.md. This plan builds the host side of each, but later plans that also list them finish them:
- 05-14: `minos host doctor` and `host setup` (KRUN-06, KRUN-08).
- 05-15: the supervisor's group kill, and the layers flowing into `BackendInfo` (KRUN-05, KRUN-07).
- 05-19 and 05-20: the `kill -9` proof (KRUN-05).

## TDD Gate Compliance

- Task 1: RED `05ddabb` (`test(05-11)`: three tests failing against stubbed helpers), then GREEN `9c8ccf3` (`feat(05-11)`).
- Task 2: RED `c6fb62e` (`test(05-11)`: six tests failing against a stubbed sweep and parsers), then GREEN `1335d42` (`feat(05-11)`).
- No refactor commits were needed.

## Known Stubs

None. The `X` placeholders in `longest_socket_path` are deliberate: they stand for the eight random characters when measuring the longest socket path.

## Threat Flags

None. The new surface (host tool execution, a loopback listener bound only for the probe, and sweep signalling) is covered by T-05-11-01 to T-05-11-07.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/host.rs
- FOUND: crates/minos-backend-krun/src/jobdir.rs
- FOUND commits: 05ddabb, 9c8ccf3, c6fb62e, 1335d42, e697d28
