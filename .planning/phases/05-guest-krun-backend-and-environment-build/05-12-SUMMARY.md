---
phase: 05-guest-krun-backend-and-environment-build
plan: 12
subsystem: minos-guest
tags: [minos-guest, pid1, trampoline, rlimits, tmpfs, vsock, kill-all, token-protection, probe, guest-01, guest-03, guest-05]
status: complete
requires:
  - "05-10: run_job, StageExecutor, Handover, StageCommand, StageRun, JobEnd, pack::LOG_CAP_BYTES"
  - "05-08: DiskJobSource, ChunkSink and Connector with the standard backoff, stage.rs (GUEST_UID/GID, STAGE_ENV, trampoline_argv, parse_trampoline, scratch paths)"
  - "05-09: root images carry /out and /scratch mount points"
  - "05-21: the spike tests that used job.rs and report.rs are already gone"
provides:
  - "linux.rs: pid1, trampoline, VsockConnector, LinuxHandover, LinuxExecutor, kill-all and reap, power-off"
  - "main.rs: dispatch to the trampoline (--minos-stage), PID 1, or a usage line"
  - "source::ReadJobSource (hands run_job an already-read input once) and SourceError::AlreadyRead"
  - "stage::scratch_mount_data(scratch_mib) -> \"size=<n>m,mode=0755\""
  - "SETUP_ERROR_MARKER and OUTPUT_SENT_MARKER console markers"
  - "minos-probe fixture binary: egress, env, write-outside, forge, hang, marker-write, marker-check, output-fill, alloc, spawn, fill-scratch, scorecard, exit"
affects:
  - "05-16: env build packs only minos-guest as /init; minos-probe is never packed into a user image"
  - "05-17, 05-19: the VM tests place minos-probe at /usr/local/bin/minos-probe in their own image and read its JSON; the krun supervisor must reassemble chunks on OUTPUT_VSOCK_PORT, because the guest no longer sends a MINOSST1 status frame"
tech-stack:
  added: []
  patterns:
    - "Read the input disk before run_job, then hand run_job the result through a one-shot source"
    - "Spawn every stage through /init --minos-stage as uid 64000; the trampoline sets rlimits itself, so no pre_exec"
    - "waitpid by the stage pid while it runs; kill(-1, SIGKILL) and reap to ECHILD afterwards"
    - "Drain a non-blocking pipe on a scoped thread with a stop flag and one final pass"
key-files:
  created:
    - crates/minos-guest/src/bin/minos-probe.rs
  modified:
    - crates/minos-guest/src/linux.rs
    - crates/minos-guest/src/main.rs
    - crates/minos-guest/src/lib.rs
    - crates/minos-guest/src/source.rs
    - crates/minos-guest/src/stage.rs
    - crates/minos-guest/src/pattern.rs
  deleted:
    - crates/minos-guest/src/job.rs
    - crates/minos-guest/src/report.rs
decisions:
  - "run_job reads through a JobSource, so PID 1 wraps its earlier read in ReadJobSource, which returns it once and then SourceError::AlreadyRead. A source failure that kept the token still reaches the host as an authenticated setup: failed {input}"
  - "The scratch tmpfs is mounted only when the job parsed. When the mount fails, run_job cannot create its directories on the read-only root, so it sends setup: failed {mount}"
  - "The /tmp and /out bind mounts get no explicit remount. A bind mount copies the tmpfs's per-mount NOSUID and NODEV flags"
  - "An unexpected waitpid error on the stage child is logged once. The stage is then treated as running until its deadline (TimedOut) and killed with everything else"
  - "The kill-and-reap after a stage repeats kill(-1) every 10 ms until wait reports ECHILD, for at most 5 s. If processes still remain, PID 1 writes a console line and goes on; pack.rs's O_NOFOLLOW and inode checks remain the defense"
  - "The drain thread reads a non-blocking pipe. After PID 1 sets the stop flag (once every stage process is gone), the thread makes one more pass, so a process killed with an undrained pipe or stuck in D state cannot hang PID 1 on join"
  - "GUEST-01, GUEST-03, and GUEST-05 are not marked complete. The Linux code compiles for musl but cannot run on this Mac. The requirements hold only once the VM tests pass (05-17, 05-19)"
metrics:
  duration: "about 12 minutes"
  completed: "2026-10-18"
  tasks: 3
  files: 9
---

# Phase 5 Plan 12: Guest PID 1, stage trampoline, and minos-probe Summary

This plan writes the guest's Linux half and removes the Phase 2 spike protocol. The Linux half is:

- **PID 1.** It reads `/dev/vdb` once and removes the node. It mounts a tmpfs scratch at the job's cap and runs `run_job`. Each stage runs through the `/init --minos-stage` trampoline as uid 64000, and after each stage PID 1 kills every other process with `kill(-1)` and reaps. Then it powers off.
- **minos-probe**, a candidate-side fixture for the VM tests.

## Tasks

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | PID 1, the trampoline, and the Linux trait implementations | 191a8cd | linux.rs, main.rs, lib.rs, source.rs, stage.rs |
| 2 | The minos-probe fixture binary | d6c7268 | bin/minos-probe.rs |
| 3 | Remove the spike protocol from the guest | 392b526 | job.rs and report.rs (deleted), lib.rs, pattern.rs |

## What was built

**PID 1 (`linux::pid1`, GUEST-01)**
- Before the job arrives it does boot work only:
  - prints `BOOT_MARKER`;
  - mounts `/proc` with `hidepid=invisible` and NOSUID, NODEV, NOEXEC;
  - mounts `/sys` read-only with the same flags.
- Then the per-run work:
  1. It opens `/dev/vdb` and reads it once through `DiskJobSource` over a `BufReader`, and the file is dropped in the same statement.
  2. It removes the node with `unlink`, falling back to `chmod` mode 0. Either failure is a console line.
  3. If the disk could not be opened, or the read failed before the token was known, it prints `MINOS-GUEST-SETUP-ERROR <reason>` and powers off.
  4. Otherwise it mounts a tmpfs at `/scratch` with `size=<scratch_mib>m,mode=0755` and NOSUID and NODEV.
  5. It calls `run_job` with:
     - `ReadJobSource` over the read result;
     - `ChunkSink::new(VsockConnector, token)`, which uses the standard backoff;
     - `LinuxExecutor` and `LinuxHandover`;
     - the root `/scratch`.
  6. It prints `MINOS-GUEST-OUTPUT-SENT <connections>`, or the sink error, or the setup-error marker when nothing was packed, followed by any setup error chain. Then it runs `sync` and powers off.
- If PID 1 panics, the kernel panics, and the host classifies the run as `Crashed` (P-PANIC). This is documented in the module docs.

**Trampoline (`linux::trampoline`; D-08, GUEST-03)**
- It exits 126 with one stderr line in each of these cases, and no stage command runs:
  - it is PID 1 or its effective uid is 0;
  - `parse_trampoline` refuses the argv;
  - any `setrlimit` fails. The limits are `RLIMIT_AS`, `FSIZE`, `NPROC`, `CPU`, `NOFILE`, and `CORE` (0), each with current equal to maximum, and the error line names the resource.
- After the limits are set it calls `Command::new(program).args(rest).exec()`. If that returns, it exits 127 with the error kind.
- There is no `pre_exec` anywhere: `grep -c pre_exec` prints 0.

**Trait implementations**
- **`VsockConnector`:** `VsockStream::connect_with_cid_port(VMADDR_CID_HOST, OUTPUT_VSOCK_PORT)`. Closing a connection is `shutdown(Write)`.
- **`LinuxHandover`:**
  - an iterative walk with `symlink_metadata` and `chownat(CWD, path, 64000, 64000, AT_SYMLINK_NOFOLLOW)`, descending only into real directories;
  - the first time it is called for `/scratch/tmp` and `/scratch/out`, it bind-mounts them on `/tmp` and `/out`.
- **`LinuxExecutor`:**
  - It creates the pipe with `io::pipe()` and makes the read end non-blocking.
  - It starts a scoped drain thread through `thread::Builder`, so a failure to spawn it is `SpawnFailed` rather than a panic.
  - It spawns `trampoline_argv("/init", limits, argv)` with:
    - `uid` and `gid` 64000 (std's `setgroups(0)` drops supplementary groups);
    - `env_clear` and then `STAGE_ENV`;
    - `current_dir` set to the stage's cwd and `process_group(0)`;
    - stdin null, with stdout and stderr both on the pipe.
  - It drops the `Command`, so PID 1 holds no write end.
  - It polls `waitpid(Some(child), NOHANG)` every 10 ms until the deadline. At the deadline the stage is `TimedOut`.
  - Then it always runs `kill_process_group(Pid::INIT, SIGKILL)`, which is `kill(-1)`, and reaps with `wait(NOHANG)` until `ECHILD`, for at most 5 s.
  - Finally it sets the stop flag and joins the drain. The drain keeps at most `LOG_CAP_BYTES + 1` bytes.
  - Wait statuses map to `Exited { code }` and `Signaled { signal }`; a spawn failure is `SpawnFailed`.

**minos-probe (fixture)**
- It is a separate binary in `src/bin`. On non-Linux, `main` explains itself and exits with failure.
- Every command named in the plan is implemented. Results are written with serde derive, so the JSON keys are fixed. Exit codes: 0 when the probe ran, 1 when it could not run or write its result, 2 for a usage error.
- `forge` sends a DATA chunk and an END chunk built by `ChunkStream` with an all-zero token, each over its own vsock connection.
- `spawn` starts children by re-running its own binary (`current_exe`) with `hang`, so the test image needs no `/bin/sleep`.
- `scorecard` takes no path argument, so a judge argv of `minos-probe scorecard` passes the manifest's judge-path rule.

**Spike protocol removed (Task 3)**
- `job.rs` and `report.rs` are deleted.
- These are removed from `lib.rs`: `JOB_ERROR_MARKER`, `FRAME_SENT_MARKER`, `PROTOCOL_VERSION`, `HEADER_LEN`, `MAX_JOB_BYTES`, `MAX_FRAME_BYTES`, and `test_hex`.
- The crate doc now describes the production flow and the probe.
- `pattern.rs` stays, because the probe uses it.
- `grep -rn MINOSST1 crates/minos-guest/src` prints nothing.

## Verification

- `cargo fmt --all --check`: ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: ok.
- `env -u INSTA_UPDATE cargo test --workspace`: 57 suites ok, no failures.
- `minos-guest` has 59 unit tests, which include 3 new ones: `read_source_hands_out_its_one_result`, `read_source_passes_a_failure_through`, and `scratch_mount_data_names_the_cap_and_mode`. The 20 tests of the deleted spike modules went with them.
- `cargo deny check`: advisories, bans, licenses, and sources ok.
- `cargo doc --workspace --no-deps`: no warnings.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: ok.
- `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`: `file` reports both `minos-guest` and `minos-probe` as "ELF 64-bit LSB executable, ARM aarch64, ... statically linked".
- C-01 prints no MISSING line, and the C-02 grep prints nothing.
- Every acceptance grep in the three tasks matches. The probe command-name grep counts 15 lines (at least 13 required).
- The Linux code cannot run on this Mac. It is checked by the musl clippy and build here; the real-VM tests arrive in 05-17 and 05-19.

## Requirements

None are marked complete. The plan lists GUEST-01, GUEST-03, and GUEST-05, and it delivers their Linux code:

- GUEST-01: the static PID 1 that reaps and creates nothing per-run before the job.
- GUEST-03: argv stages as uid 64000 under rlimits, writing only to scratch.
- GUEST-05: the environment cleared to `STAGE_ENV`.

Each requirement is verified only inside a VM. GUEST-03 is checked by 05-17's exit tests, and GUEST-05 by 05-19's `exit_env`, whose wording requires "a test that reads `env` and `/proc/cmdline` inside the VM". This follows the 05-10 precedent.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `ReadJobSource` and `SourceError::AlreadyRead`**
- **Found during:** Task 1
- **Issue:** `run_job` takes a `JobSource`, but PID 1 must read the disk first. It needs the token for the sink, the scratch cap for the tmpfs, and the disk closed so the node can be removed. `SourceFailure` has private fields, so the binary could not build a source that returns an earlier read.
- **Fix:** Added `source::ReadJobSource`. It hands out its stored result once, and a second call returns `SourceError::AlreadyRead` with no token. Two host tests cover it.
- **Files modified:** crates/minos-guest/src/source.rs
- **Commit:** 191a8cd

**2. [Rule 2 - Correctness] Host-testable tmpfs mount data**
- **Found during:** Task 1
- **Fix:** Added `stage::scratch_mount_data(scratch_mib)` with a unit test, so the tmpfs size and mode are tested on the host rather than only in a VM.
- **Files modified:** crates/minos-guest/src/stage.rs
- **Commit:** 191a8cd

**3. [Rule 3 - Blocking] The two console markers moved to Task 1**
- **Issue:** Task 1's PID 1 prints `SETUP_ERROR_MARKER` and `OUTPUT_SENT_MARKER`, but the plan added them in Task 3. Task 1 could not compile without them.
- **Fix:** Added both markers to `lib.rs` in Task 1. Task 3 only removes the spike items.
- **Commit:** 191a8cd

**4. [Rule 2 - Robustness] Non-blocking drain with a stop flag**
- **Issue:** With a blocking drain, a stage process that survived the 5 s reap bound (for example, one stuck in D state) would keep a write end open, and PID 1 would wait on `join` forever.
- **Fix:** The drain thread reads a non-blocking pipe and ends after PID 1 sets the stop flag and one more pass finds the pipe empty. The extra pass catches bytes written just before the stop flag was set.
- **Commit:** 191a8cd

## Deferred Issues

- `minos-backend-krun/src/supervise.rs` still decodes the `MINOSST1` status frame, and `input_disk.rs` still holds the spike job header. The guest no longer sends that frame. This is outside this plan's files. The plans that move the krun supervisor to chunk reassembly over `OUTPUT_VSOCK_PORT` (05-13 onward and 05-17) replace those modules.

## Known Stubs

None.

## Threat Flags

None. The plan's register covers every new surface:

- T-05-12-01: token protection by unlink, hidepid, and the stage uid.
- T-05-12-02: `kill(-1)` and reap before packing.
- T-05-12-03: uid and gid with groups dropped, and the trampoline's refusal of root.
- T-05-12-04: rlimits with exit 126 on failure, plus the deadline.
- T-05-12-05: `env_clear` plus `STAGE_ENV`.
- T-05-12-06: the read-only root and the tmpfs binds.
- T-05-12-07: the probe is documented as a fixture.

The probe's own vsock connect in `forge` is test-only, and it is the forgery the host must refuse.

## Self-Check: PASSED

- FOUND: crates/minos-guest/src/linux.rs, crates/minos-guest/src/main.rs, crates/minos-guest/src/bin/minos-probe.rs, crates/minos-guest/src/lib.rs
- GONE as intended: crates/minos-guest/src/job.rs, crates/minos-guest/src/report.rs
- FOUND commits: 191a8cd, d6c7268, 392b526
