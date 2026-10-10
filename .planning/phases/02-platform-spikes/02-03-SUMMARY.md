---
phase: 02-platform-spikes
plan: 03
subsystem: infra
tags: [rust, msb_krun, libkrun, vmm, launcher, serde, clippy, entitlements, watchdog]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: sha2 and base16ct edges in minos-vmm, libkrunfw 5.6.2 canonical path and SHA-256, the static guest /init"
provides:
  - minos-vmm launch config v1 (E-1, E-1b, E-2) with RawVmmConfig (deny_unknown_fields) and VmmConfig (try_from)
  - preflight that refuses MSB_KRUN_/DYLD_ variables, non-canonical or wrong-type files, an existing console, scratch over cap, and a libkrunfw or initramfs hash mismatch
  - VM assembly for root_disk (D-18) and initramfs (negative evidence) boot modes
  - watchdog with exit codes 124, 125, 126 through Vm::exit_code and ExitHandle::trigger
  - the E-3 exit-code map (0, 64, 65, 66, 67, 70, 124, 125, 126) with one stderr line per failure
  - crates/minos-vmm/clippy.toml banning 13 msb_krun methods, each proven to fire
  - crates/minos-vmm/minos-vmm.entitlements with com.apple.security.hypervisor only
affects: [02-05, 02-06, 02-07, 02-08, 02-09, 02-10, 02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "parse() deserializes RawVmmConfig and then calls VmmConfig::try_from directly, so errors stay typed; #[serde(try_from)] on VmmConfig routes any other serde path through the same constructor"
    - "Path rules walk the raw bytes as well as Path::components, because components() silently drops `.`, empty, and trailing segments"
    - "Range checks narrow into the stored type in one helper: in_range::<T: TryFrom<u64>>(field, value, min, max)"
    - "Errors carry the source with #[source] and a Display that does not repeat it; main joins the chain with `: ` and skips a source whose text already ends the line (msb_krun::Error::Io repeats its source)"
    - "Clippy ban probes are a throwaway #[cfg(test)] #[test] fn, so the probe is not dead code and the output holds only the ban errors"

key-files:
  created:
    - crates/minos-vmm/src/config.rs
    - crates/minos-vmm/src/preflight.rs
    - crates/minos-vmm/src/launch.rs
    - crates/minos-vmm/src/watchdog.rs
    - crates/minos-vmm/clippy.toml
    - crates/minos-vmm/minos-vmm.entitlements
    - .planning/phases/02-platform-spikes/evidence/02-03.md
  modified:
    - crates/minos-vmm/src/main.rs

key-decisions:
  - "All 13 E-5 clippy bans fire with their definition paths (msb_krun::api::builder / msb_krun::api::builders); no entry needed the re-export form, and FsBuilder::custom fires as a bare path expression"
  - "The VMM's pre-takeover failures exit 64/65/66/67/70; a SHA-256 mismatch (67) is reported apart from other preflight failures (66)"
  - "Preflight requires the vsock socket path to be canonical as well as a socket (stricter than the plan's exists-and-is-socket)"

patterns-established:
  - "Launch config: one wire shape shared with minos-backend-krun by embedding the identical E-1 text in a test on each side"
  - "Watchdog trips are a closed enum (Trip) with code() and reason(); the stored exit code wins over the vCPU code"

# SPIKE-01 (VM boot), SPIKE-02 (no egress), and SPIKE-04 (signed helper starts a VM) are contributed to, not completed:
# no VM has booted through this launcher yet (02-06 onward), so REQUIREMENTS.md is left unchanged, as in 02-01.
requirements-completed: []

coverage:
  - id: D1
    description: "Launch config v1 parses E-1 and E-1b and rejects every E-2 rule, unknown keys at every level, and oversized input before parsing"
    requirement: SPIKE-04
    verification:
      - kind: unit
        ref: "cargo test -p minos-vmm config (36 tests)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Pre-takeover exit codes 64, 65, 65, 66 recorded from real runs without a VM"
    requirement: SPIKE-04
    verification:
      - kind: other
        ref: "evidence/02-03.md V-EXIT"
        status: pass
    human_judgment: false
  - id: D3
    description: "Thirteen dangerous msb_krun methods banned in clippy.toml, each proven to fire, probe reverted"
    requirement: SPIKE-02
    verification:
      - kind: other
        ref: "evidence/02-03.md C-03c (exit 0, 0 unresolved paths), D-PROBE (exit 101, 13 distinct paths; exit 0 after revert)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Preflight environment and file-type checks, and watchdog trip order and codes"
    requirement: SPIKE-04
    verification:
      - kind: unit
        ref: "cargo test -p minos-vmm preflight watchdog (8 tests)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Hypervisor-only entitlements plist"
    requirement: SPIKE-04
    verification:
      - kind: other
        ref: "plutil -lint crates/minos-vmm/minos-vmm.entitlements (OK); grep -c '<key>' prints 1"
        status: pass
    human_judgment: false
  - id: D6
    description: "The launcher boots a VM in root_disk mode, the watchdog trips 124/125/126 at runtime, the hash check returns 67, and on_exit prints the code"
    requirement: SPIKE-01
    verification: []
    human_judgment: true
    rationale: "Needs a signed minos-vmm, a packed root image, and a bound socket; the boot and exit spikes run in 02-06 and 02-07"

duration: 8min
completed: 2026-10-10
---

# Phase 2 Plan 03: minos-vmm launcher Summary

**`minos-vmm <config.json>` now parses a versioned, validated launch config, runs preflight on the environment and every named file (including the libkrunfw SHA-256), assembles a VM with three raw disks, one vsock route to a Unix socket, a file console, and no network or virtio-fs device, then starts a 50 ms watchdog and enters the VM. All 13 dangerous msb_krun methods are banned in clippy, and each ban is proven to fire.**

## Performance

- **Duration:** about 8 min
- **Started:** 2026-10-10T13:08:21Z
- **Completed:** 2026-10-10T13:16:42Z
- **Tasks:** 3
- **Files modified:** 8 (7 created, 1 modified)

## Accomplishments

- `config.rs`: `RawVmmConfig` plus seven raw object structs, all with `deny_unknown_fields` (8 in total, no `flatten`). `RawBoot` is an externally tagged `snake_case` enum, so both `"boot":"root_disk"` and `"boot":{"initramfs":{...}}` parse. `VmmConfig` has `#[serde(try_from = "RawVmmConfig")]`, private fields, and the 14 accessors. `parse` refuses more than 65,536 bytes before JSON. `ConfigError` names the dotted field for every rule.
- `preflight.rs` runs its checks in this order:
  - the environment (`MSB_KRUN_*`, `DYLD_*`);
  - `canonicalize(path) == path` plus a `symlink_metadata` type check for krunfw, root, input, scratch, and initramfs;
  - scratch length against its cap;
  - the socket (canonical, and a socket);
  - the console (must not exist, and its parent must be a canonical directory);
  - a buffered SHA-256 of libkrunfw and the initramfs, using `fill_buf`/`consume` so nothing is sliced.
- `launch.rs` builds the VM:
  - machine: `vcpus`, `memory_mib`, `balloon(false)`, `msb_metrics(false)`;
  - kernel: E-4 constants per boot mode;
  - disks: root (read-only), input (read-only), and scratch (read-write), each `path` then `read_only`, with no format call;
  - vsock: `unix_connect(port, socket)`;
  - console: `output(console)`;
  - `on_exit` prints `minos-vmm: exit <code>`;
  - `rng` is never called.
- `watchdog.rs` checks the deadline (124), the console cap (125), and the parent id (126), in that order, every 50 ms. On a trip it prints `minos-vmm: watchdog <reason>`, stores the code with `SeqCst`, and calls `trigger()`.
- `main.rs` maps each error to its E-3 exit code and prints exactly one stderr line. The `Infallible` result of `enter` is handled with an empty match (`Ok(never) => match never {}`), which raised no `unreachable_patterns` warning on rustc 1.97.1. stdin is never read.
- V-EXIT recorded real runs that exited 64 (no argument), 65 (missing file), 65 (unknown key `x`), and 66 (E-1: `disks.root` missing). An extra two-argument run also exited 64.
- C-03c: clippy exits 0 on the untouched crate, and no disallowed path is unresolved. D-PROBE: exit 101 with 13 distinct `use of a disallowed method` paths. After the revert, clippy exits 0 again and `git diff -- crates/minos-vmm/src` is empty.

### E-3 exit codes as implemented

| Code | Meaning | stderr | Source |
|---|---|---|---|
| 0 | the VMM exited on its own | `minos-vmm: exit 0` (on_exit) | msb_krun; confirmed in 02-07 |
| 64 | not exactly one argument | `minos-vmm: usage: minos-vmm <config.json>` | `LaunchError::Usage` |
| 65 | config unreadable, over 65,536 bytes, not JSON, unknown field, or an E-2 rule failed | `minos-vmm: error: reading launch config <path>: ...` or `minos-vmm: error: launch config <path>: ...` | `LaunchError::Read`, `LaunchError::Config` |
| 66 | preflight failure other than a hash mismatch | `minos-vmm: error: preflight: ...` | `LaunchError::Preflight(_)` |
| 67 | libkrunfw or initramfs SHA-256 mismatch | `minos-vmm: error: preflight: <field> <path>: SHA-256 is <actual>, the config expects <expected>` | `LaunchError::Preflight(PreflightError::Hash)` |
| 70 | `launch::build` failed, the watchdog thread failed to spawn, or `enter` returned `Err` | `minos-vmm: error: building the VM: ...`, `starting the watchdog: ...`, or `starting the VM: ...` | `LaunchError::{Build, Watchdog, Enter}` |
| 124 | `max_duration_ms` elapsed | `minos-vmm: watchdog deadline`, then `minos-vmm: exit 124` | `Trip::Deadline` |
| 125 | console file length passed `console.cap_bytes` | `minos-vmm: watchdog console-cap`, then `minos-vmm: exit 125` | `Trip::ConsoleCap` |
| 126 | `parent_id()` changed | `minos-vmm: watchdog parent-gone`, then `minos-vmm: exit 126` | `Trip::ParentGone` |

Clippy paths: every E-5 entry fired as written, using the definition path under `msb_krun::api::builder` or `msb_krun::api::builders`. **No entry needed the re-export form.** `FsBuilder::custom` fired when referenced as a bare path expression (`let custom = msb_krun::FsBuilder::custom;`), so the probe needed no call.

## Task Commits

1. **Task 1 (RED): failing launch config tests:** `c257b68` (test)
2. **Task 1 (GREEN): E-2 validation:** `575d72a` (feat)
3. **Task 2: preflight, assembly, watchdog, exit codes, plist, V-EXIT:** `eb2e854` (feat)
4. **Task 3: clippy bans, C-03c, D-PROBE:** `333c670` (chore)

## Files Created/Modified

- `crates/minos-vmm/src/config.rs`: launch config raw and validated types, E-2 rules, 36 unit tests embedding E-1 and E-1b.
- `crates/minos-vmm/src/preflight.rs`: `check`, `PreflightError` (including `Hash`), and 4 unit tests.
- `crates/minos-vmm/src/launch.rs`: `build` and the E-4 constants `ROOT_DISK_CMDLINE`, `ROOT_DISK_INIT`, and `INITRAMFS_CMDLINE`.
- `crates/minos-vmm/src/watchdog.rs`: `spawn`, `Limits`, `Trip` (124/125/126), and 4 unit tests.
- `crates/minos-vmm/src/main.rs`: exit-code constants, `LaunchError`, the E-3 mapping, the source-chain printer, and the panic-free stderr writer.
- `crates/minos-vmm/clippy.toml`: 13 `disallowed-methods` entries (E-5 verbatim).
- `crates/minos-vmm/minos-vmm.entitlements`: the hypervisor entitlement only (E-6 verbatim).
- `.planning/phases/02-platform-spikes/evidence/02-03.md`: V-EXIT, C-03c, and D-PROBE.

## Decisions Made

- `parse` deserializes `RawVmmConfig` and then calls `VmmConfig::try_from` itself, instead of `serde_json::from_slice::<VmmConfig>`. Going through serde's `try_from` would flatten each `ConfigError` into a serde custom-message string, and the tests could no longer match the field. The attribute stays on `VmmConfig`, and a test proves the serde path rejects the same input.
- Path validation checks both `Path::components()` and the raw bytes split on `/`. `components()` normalizes away `.`, `//`, and a trailing `/`, so on its own it cannot reject `/private/tmp/./root.img`.
- `watchdog::spawn` returns `io::Result<()>` and detaches the thread. It uses `thread::Builder`, so a spawn failure becomes exit 70 rather than a panic. The plan allowed "or equivalent".
- The socket path must be canonical, not only exist and be a socket. The 02-05 paths sit under `/private/tmp`, so this costs nothing and closes the symlink case for T-02-21.

## Deviations from Plan

### Auto-fixed Issues

None. The plan executed as written. These choices sat within the plan's latitude and are recorded above:
- `parse` calls `try_from` directly.
- The probe is written as a `#[test]` function.
- The socket must be canonical.
- An extra two-argument V-EXIT case was recorded.

## TDD Gate Compliance

The RED commit `c257b68` ran 36 tests: 30 failed and 6 passed. The 6 passes cover behavior that lives in the type skeleton, not in the stubbed validation:
- `deny_unknown_fields` at every level (3 tests);
- the unknown boot variant;
- the pre-parse size cap;
- `v = 2`, which the stub's placeholder error (`Version(raw.v)`) matched by coincidence.

GREEN commit `575d72a`: 36 of 36 pass. No refactor commit was needed.

## Issues Encountered

- Edition 2024 `impl Trait` already captures every lifetime in scope, so `+ '_` on `preflight::inspect` was a compile error and was removed.
- Clippy `doc_markdown` wants `msb_krun` and `SquashFS` in backticks, and `duration_suboptimal_units` wants `Duration::from_mins(1)`. Both were fixed in the code, with no allow.

## Known Stubs

None.

## User Setup Required

None. Signing (`codesign --force -s - --entitlements crates/minos-vmm/minos-vmm.entitlements ...`) and the first boot belong to 02-06.

## Next Phase Readiness

- 02-05 can serialize E-1 and spawn `minos-vmm <path>` with `env_clear()`, stdin `/dev/null`, and the socket bound before spawn. Preflight requires every path in canonical `/private/tmp/...` form, a console path that does not yet exist, and a scratch image no longer than its cap.
- 02-06 must ad-hoc sign `target/debug/minos-vmm` with the plist before the first boot. The V-EXIT codes need no signature.
- Not yet observed at runtime: 67 and 70, the watchdog codes, and the `on_exit` line. They are covered by unit tests and source reading only, until 02-06 and 02-07.

## Self-Check: PASSED

- FOUND: crates/minos-vmm/src/main.rs, config.rs, preflight.rs, launch.rs, watchdog.rs, clippy.toml, minos-vmm.entitlements, evidence/02-03.md
- FOUND: c257b68, 575d72a, eb2e854, 333c670
- Gate: fmt, workspace clippy, workspace test, deny, doc, and musl guest clippy all exit 0. C-01 prints 10 `ok` lines. The C-02 and unsafe greps print nothing. `git diff --name-only 80a220d -- Cargo.toml Cargo.lock deny.toml crates/minos-vmm/Cargo.toml` prints nothing.
