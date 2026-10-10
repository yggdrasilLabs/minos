---
phase: 02-platform-spikes
plan: 05
subsystem: infra
tags: [rust, libkrun, minos-vmm, supervisor, vsock, unix-socket, serde, golden-bytes, untrusted-input]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: MINOSJB1 job decoder and MINOSST1 frame encoder in minos-guest (E-9, E-10); 02-03: minos-vmm launch config parser (E-1, E-1b), preflight rules, exit codes"
provides:
  - launch_config::LaunchConfig, which serializes compact to exactly E-1 and E-1 with E-1b (the bytes minos-vmm parses)
  - input_disk::input_disk_image, which writes MINOSJB1, version, u64 length, job JSON, and zero padding to 512-byte sectors
  - supervise::run, which launches one helper argv with env_clear, null stdin, file-backed stdio, its own process group, per-connection caps, and a host deadline
  - supervise::GuestBytes, the capped wrapper for guest-influenced bytes (no derives; exits only through validating, comparing, hashing, and sanitizing methods)
  - supervise job-directory helpers (short_job_dir, JobFiles, write_disks, bind_output, write_config, sha256_file, has_hypervisor_entitlement)
affects: [02-06, 02-07, 02-08, 02-09, 02-10, 02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "Cross-crate wire shapes are bound by identical golden texts in both crates' tests, never by a dependency edge (minos-backend-krun links neither minos-vmm nor minos-guest)"
    - "Guest bytes are read through take(cap + 1); more than cap sets an overflow flag and only cap bytes are kept"
    - "Connection reads are non-blocking and polled against one total deadline, not SO_RCVTIMEO (per-read and renewable by a trickling peer; EINVAL on macOS once the peer has closed)"
    - "A Reaper(Child) guard kills and reaps the helper on every early return; std's kill after a reap is a no-op, so a reused pid is never signalled"
    - "Excerpts are an allowlist over utf8_chunks: printable plus newline and tab; C0, DEL, C1, and bidi overrides dropped; invalid UTF-8 becomes U+FFFD; lazy, so a 1 GiB buffer is not copied"

key-files:
  created: []
  modified:
    - crates/minos-backend-krun/src/launch_config.rs
    - crates/minos-backend-krun/src/input_disk.rs
    - crates/minos-backend-krun/src/supervise.rs

key-decisions:
  - "Connection reads poll a non-blocking stream against a total read deadline (the later of the host deadline and now + 50 ms) instead of SO_RCVTIMEO: macOS returns EINVAL from setsockopt once the peer has closed, and a per-read timeout lets a trickling guest stretch the deadline"
  - "has_hypervisor_entitlement matches the exact compact XML `<key>com.apple.security.hypervisor</key><true/>`, so a false value or a longer key with the same prefix does not count"
  - "status_frame rejects an overflowed connection (FrameError::Overflowed) even if the kept bytes happen to form a valid frame, because trailing bytes existed"
  - "SPIKE-01 and SPIKE-03 are contributed to, not completed: no VM boots in this plan, so REQUIREMENTS.md is left unchanged, as in 02-01 and 02-03"

patterns-established:
  - "Every VM spike: short_job_dir -> JobFiles::in_dir -> write_disks -> write_config -> bind_output -> run(Launch::new([vmm, config], &files, listener)...)"

requirements-completed: []

duration: 10min
completed: 2026-10-10
---

# Phase 2 Plan 05: Host supervisor, launch-config serializer, and job input disk writer

**This plan writes the host side of one minos-vmm launch. A `LaunchConfig` serializes byte-for-byte to the E-1/E-1b texts that minos-vmm parses. A job disk writer produces the guest's MINOSJB1 layout. A supervisor spawns the helper with an empty environment and null stdin, collects vsock output through per-connection caps with overflow detection, kills at a host deadline, and returns guest bytes only as a capped `GuestBytes`.**

## Performance

- **Duration:** about 10 min
- **Started:** 2026-10-10T13:36:29Z
- **Completed:** 2026-10-10T13:45:50Z
- **Tasks:** 2 of 2
- **Files modified:** 3

## Accomplishments

- `LaunchConfig::to_json` produces exactly E-1 (root_disk) and E-1 with E-1b substituted (initramfs). The E-1 and E-1b constants in `launch_config.rs` are byte-identical to those in `crates/minos-vmm/src/config.rs`; the plan's `diff <(grep -o ...)` prints nothing for either.
- `input_disk_image(b"{}")` is 512 bytes: the E-2 golden header `4d494e4f534a4231 00000001 0000000000000002 7b7d`, then zeros. The canonical job (E-6) declares its own length. Exactly 1048576 bytes are accepted and 1048577 are rejected.
- `supervise::run` was unit-tested on macOS with real processes and no VM:
  - `/usr/bin/env` prints nothing, which shows `env_clear` took effect.
  - `/bin/cat` exits at once, which shows stdin is `/dev/null`.
  - `/bin/sleep 5` with a 200 ms deadline is killed with SIGKILL in under 2 s.
  - A connection of exactly the cap is kept whole. One over the cap is flagged overflowed and keeps exactly the cap.
  - Per-connection caps and the maximum connection count apply in the order connections are accepted.
- `GuestBytes::status_frame` decodes the E-3 golden frame. It rejects:
  - a short header, a wrong magic, and version 2;
  - a declared length over the limit;
  - a length mismatch in either direction;
  - an overflowed connection;
  - invalid JSON, and JSON that is not an object with `v` = 1.
- The supervisor has 22 unit tests. The `supervise` filter finishes in about 1 s, stable over 5 consecutive runs.

## Public API (as implemented; 02-06 to 02-09 build on this)

### `minos_backend_krun::launch_config`

- `pub const LAUNCH_CONFIG_VERSION: u32 = 1`
- `LaunchConfig::new(kernel: Kernel, boot: Boot, disks: Disks, vsock: Vsock, console: Console, max_duration_ms: u64, machine: Machine) -> Self`
- `LaunchConfig::to_json(&self) -> Result<String, serde_json::Error>` (compact; errors on a non-UTF-8 path)
- `Kernel::new(krunfw: impl Into<PathBuf>, sha256_hex: impl Into<String>)`
- `Boot::RootDisk`, `Boot::initramfs(path: impl Into<PathBuf>, sha256_hex: impl Into<String>)` (variant `Boot::Initramfs(Initramfs)`, with private fields)
- `Disks::new(root, input, scratch: impl Into<PathBuf>, scratch_cap_bytes: u64)`
- `Vsock::new(port: u32, socket: impl Into<PathBuf>)`
- `Console::new(path: impl Into<PathBuf>, cap_bytes: u64)`
- `Machine::new(vcpus: u32, memory_mib: u32)` (`const`)
- All types derive `Debug, Clone, PartialEq, Eq, Serialize`. Their fields are private and declared in E-1 key order. minos-vmm does all validation.

### `minos_backend_krun::input_disk`

- `JOB_MAGIC: [u8; 8] = *b"MINOSJB1"`, `PROTOCOL_VERSION: u32 = 1`, `MAX_JOB_BYTES: u64 = 1_048_576`, `SECTOR_BYTES: usize = 512`
- `input_disk_image(job_json: &[u8]) -> Result<Vec<u8>, InputDiskError>`
- `InputDiskError::TooLarge { size: usize }`

### `minos_backend_krun::supervise`

- Constants:
  - Frame: `FRAME_MAGIC` (`MINOSST1`), `FRAME_VERSION` 1, `FRAME_HEADER_BYTES` 20, `MAX_FRAME_BYTES` 1048576.
  - Paths: `MAX_SOCKET_PATH_BYTES` 103, `JOB_ROOT` `/private/tmp`.
  - Launch defaults: `DEFAULT_HOST_DEADLINE` 2 min, `DEFAULT_CONN_CAP` 1048576, `DEFAULT_MAX_CONNECTIONS` 4, `DEFAULT_CONSOLE_CAP` 1048576.
  - Caps and entitlement: `VMM_STDIO_CAP` 65536, `CODESIGN_OUTPUT_CAP` 65536, `HYPERVISOR_ENTITLEMENT`.
  - File names (E-5): `ROOT_FILE`, `INPUT_FILE`, `SCRATCH_FILE`, `SOCKET_FILE`, `CONSOLE_FILE`, `CONFIG_FILE`, `VMM_STDOUT_FILE`, `VMM_STDERR_FILE`, `INITRAMFS_FILE`.
- `short_job_dir() -> Result<tempfile::TempDir, SuperviseError>`: `/private/tmp/mnsXXXXXXXX` (8 random characters), mode 0700. Dropping it removes the directory.
- `JobFiles::in_dir(dir: &Path) -> Result<JobFiles, SuperviseError>` canonicalizes `dir` and refuses a socket path over 103 bytes. Accessors: `dir()`, `root()`, `input()`, `scratch()`, `socket()`, `console()`, `config()`, `vmm_stdout()`, `vmm_stderr()`, `initramfs()`, all `-> &Path`. Derives `Debug, Clone, PartialEq, Eq`.
- `write_disks(files: &JobFiles, root_image: &[u8], input_image: &[u8], scratch_cap_bytes: u64) -> Result<(), SuperviseError>`: `create_new` for every file. The scratch image is `set_len(scratch_cap_bytes)` (sparse) and is never read.
- `bind_output(files: &JobFiles) -> Result<UnixListener, SuperviseError>` binds a non-blocking listener. Call it before `run`.
- `write_config(files: &JobFiles, config: &LaunchConfig) -> Result<(), SuperviseError>` creates `vmm.json` with `create_new`.
- `sha256_file(path: &Path) -> Result<String, SuperviseError>` returns lowercase hex.
- `has_hypervisor_entitlement(vmm: &Path) -> Result<bool, SuperviseError>` returns `Err(Io)` if `vmm` does not exist and `Ok(false)` for an unsigned or unentitled binary.
- `Launch::new(argv: impl IntoIterator<Item = impl Into<OsString>>, files: &JobFiles, listener: UnixListener)`. Builders, all `#[must_use]`: `.host_deadline(Duration)`, `.conn_caps(Vec<u64>)` (the last value repeats; an empty list means the default), `.max_connections(usize)`, `.console_cap(u64)`. `argv` accepts `&str`, `&Path`, `&OsStr`, `PathBuf`, or `String`.
- `run(launch: Launch<'_>) -> Result<VmRun, SuperviseError>`
- `VmRun` accessors:
  - `status() -> ExitStatus`, `exit_code() -> Option<i32>`, `signal() -> Option<i32>`;
  - `killed_at_deadline() -> bool`, `elapsed() -> Duration` (spawn to exit), `pid() -> u32`;
  - `connections() -> &[GuestBytes]` (in accept order), `console()`, `vmm_stdout()`, `vmm_stderr()`, each `-> &GuestBytes`.
- `GuestBytes` has no derives and no `Debug`, `Display`, `Deref`, or `Clone`. Methods:
  - `len()`, `is_empty()`, `overflowed()`;
  - `sha256_hex() -> String`;
  - `contains(&[u8]) -> bool` (an empty needle is always true);
  - `matches_pattern(impl Fn(u64) -> u8) -> bool`;
  - `excerpt(max_chars: usize) -> String`;
  - `status_frame(max_bytes: u64) -> Result<serde_json::Value, FrameError>` (the limit is `min(max_bytes, MAX_FRAME_BYTES)`).
- `FrameError` variants: `Short`, `BadMagic { found: [u8; 8] }`, `Version(u32)`, `TooLarge { declared, max }`, `Overflowed`, `LengthMismatch { declared, present }`, `Json(serde_json::Error)`, `NotV1Object`. No message includes guest text; `found` is printed as hex.
- `SuperviseError` variants: `Io { action, path, source }`, `SocketPathTooLong { path, len }`, `EmptyArgv`, `Spawn { program, source }`, `Json(serde_json::Error)`, `CodesignOutput`, `DeadlineOverflow`.

### Run-loop contract (E-4)

While the helper runs, each pass of the loop takes the first applicable step:
1. If a connection is pending and fewer than `max_connections` have been taken, accept it and read it.
2. Otherwise, if the helper has exited, leave the loop.
3. Otherwise, if the deadline has passed, `kill` then `wait`, set `killed_at_deadline`, and leave the loop.
4. Otherwise, sleep 10 ms.

A connection read is bounded in total by the later of the host deadline and now + 50 ms. After the helper exits, connections already pending are accepted once. The console file is then read through `console_cap + 1` bytes, and the stdout and stderr files through 65537 bytes each. A file the helper never created reads as empty.

## Task Commits

1. **Task 1: Launch config serializer and job input disk writer**
   - `67a8823` (test, RED: 8 failing)
   - `ac41d47` (feat, GREEN: 8 passing)
2. **Task 2: Supervisor**
   - `88097cc` (test, RED: 22 failing on `todo!()` bodies)
   - `a5b59ca` (feat, GREEN: 22 passing)

## Files Created/Modified

- `crates/minos-backend-krun/src/launch_config.rs`: the `LaunchConfig` family, `to_json`, and 3 tests (E-1, E-1 with E-1b, a non-UTF-8 path).
- `crates/minos-backend-krun/src/input_disk.rs`: the constants, `input_disk_image`, `InputDiskError`, and 5 tests (E-2, E-6, the sector boundary, the largest job, one byte over).
- `crates/minos-backend-krun/src/supervise.rs`: the job-file helpers, `Launch`, `run`, `VmRun`, `GuestBytes`, `FrameError`, `SuperviseError`, and 22 tests.

## Decisions Made

- **Reading connections without `SO_RCVTIMEO`.** Connection reads poll a non-blocking stream against a total read deadline. This was found during GREEN: `set_read_timeout` failed with EINVAL on a connection whose peer had already written and closed. A per-read timeout is also the wrong bound, because a trickling guest can renew it on every byte.
- **Exact entitlement match.** The check matches the compact XML that `codesign --xml` actually prints. This was confirmed against `/usr/libexec/rosetta/oahd`, whose output is a single line of `<key>...</key><true/>` pairs.
- **Overflow wins over a valid prefix.** An overflowed connection is never decoded as a frame.
- **Requirements left open.** SPIKE-01 and SPIKE-03 are left unmarked because no VM boots here.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Connection reads switched from SO_RCVTIMEO to non-blocking polling**
- **Found during:** Task 2 GREEN.
- **Issue:** The plan's read loop sets a read timeout on each accepted stream. On macOS, `setsockopt(SO_RCVTIMEO)` returns EINVAL (os error 22) once the peer has closed. That turned a guest that wrote and closed quickly into a host `SuperviseError::Io`. A per-read timeout also cannot bound the total time.
- **Fix:** Read the stream non-blocking through `take(cap + 1)`. On `WouldBlock`, sleep 1 ms. Stop at EOF, the cap, a peer error, or the total read deadline.
- **Files modified:** `crates/minos-backend-krun/src/supervise.rs`
- **Commit:** `a5b59ca`

### Additions within the plan's latitude

- `SuperviseError::EmptyArgv` and `SuperviseError::DeadlineOverflow`. `run` must refuse an empty argv, and `Instant + Duration` must not panic.
- `FrameError::Overflowed`, which closes the valid-prefix case described above.
- File-name, cap, and default constants, plus `JobFiles::dir()`.
- `Launch` is generic over argv item types (`Into<OsString>`), so spikes can pass `&Path` values directly.
- One test the plan's behavior list did not name: `/bin/echo` stdout capture. It is a positive control, so the empty `/usr/bin/env` output cannot pass vacuously.

## TDD Gate Compliance

- **Task 1:** RED `67a8823` had 8 tests and 8 failed against stubbed `to_json` and `input_disk_image`. GREEN `ac41d47` passed 8 of 8.
- **Task 2:** RED `88097cc` had 22 tests and 22 failed (the bodies were `todo!()`, so every test panicked). GREEN `a5b59ca` passed 22 of 22.
- No refactor commits were needed.

## Issues Encountered

- clippy `trivially_copy_pass_by_ref` flagged the test helper `frame(magic: &[u8; 8], ..)`. The fix passes the array by value; no lint was allowed.

## Verification

From the repository root, every command exited 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (advisories, bans, licenses, and sources all ok)
- `cargo doc --workspace --no-deps`
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

Greps:
- C-01 prints 10 `ok` lines. C-02 and the unsafe grep print nothing.
- `env_clear` 2, `process_group` 1, `Stdio::null` 3. A `Debug`/`Display` impl for `GuestBytes`: 0. The E-3 golden hex: 1. `hdiutil` or `"mount"`: none.
- `git diff` shows no change to `Cargo.toml`, `Cargo.lock`, `deny.toml`, or the crate manifest.

`cargo clean` was not run, per the plan (02-11 runs it).

## Known Stubs

None.

## Threat Flags

None. The only surfaces are the ones in the plan's threat register:
- T-02-29 to T-02-35 are mitigated and unit-tested.
- The helper spawn uses `env_clear` and an argv array.
- The listener lives in a 0700 directory.
- No network endpoint is added. No host directory is shared. No guest-written disk is opened.

## Next Phase Readiness

- 02-06 to 02-09 can launch a VM in one library call. Call `has_hypervisor_entitlement` first and fail with the documented sign argv when it returns false; the debug `minos-vmm` is linker-signed without the entitlement until it is signed with the plist.
- Not yet exercised against a real VM: vsock throughput through the 1 ms poll (K4) and the console cap overshoot (Pitfall 8). 02-07 and 02-08 record both.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/launch_config.rs, input_disk.rs, supervise.rs
- FOUND: 67a8823, ac41d47, 88097cc, a5b59ca
- Stub scan (`TODO|FIXME|placeholder|coming soon|todo!`) over the three files prints nothing.
