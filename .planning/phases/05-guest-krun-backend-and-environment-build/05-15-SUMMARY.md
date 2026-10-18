---
phase: 05-guest-krun-backend-and-environment-build
plan: 15
subsystem: minos-backend-krun
tags: [krun, supervisor, classify, k7, backend, seatbelt, process-group, console-pipe, eperm, krun-01, krun-04, krun-05, krun-07, d-13, d-14, d-15]
status: complete
requires:
  - "05-06: BlobSource, LaunchError::Blob, Capabilities::backend_info"
  - "05-07: minos-vmm v2 exit codes 64-67/70/71/124/126, the whole-line markers, and stderr on a pipe (RLIMIT_FSIZE 0)"
  - "05-09: seatbelt::{ProfileParams, argv}"
  - "05-11: host::{HostPaths, probe_host, KRUNFW_LINK, KRUNFW_SHA256}, jobdir::{JobRoot, JobDir, sweep, SweepReport}"
  - "05-13: input_disk::{fetch_parts, new_token, write_input_disk}, chunks::{ChunkReceiver, ReceiveOutcome, CONNECTION_DEADLINE}"
  - "05-21: launch_config v2"
  - "05-22: Finished::new(cleared, ended, stream, elapsed, console), Ended::ChunkProtocol, CrashCause"
provides:
  - "supervise::{Helper (spawn, pid, console_over, kill_group, try_wait, finish), HelperEnd (exit, console, stderr_lines, into_parts), HelperExit, HostKill, Observed::new, Classification, classify, watch}"
  - "supervise::{DEADLINE_MARKER, PARENT_GONE_MARKER, HOST_DEADLINE_GRACE, VMM_STDIO_CAP (now 131072), KILL_WAIT, PIPE_DRAIN_WINDOW, MAX_SOCKET_PATH_BYTES, sha256_file, SuperviseError}"
  - "backend::{CONSOLE_CAP_BYTES, KrunBackend (new, with_job_root, sweep_report, paths), KrunRunning (helper_pid, job_dir)}, impl Backend for KrunBackend<B>, impl RunningVm<R> for KrunRunning<R>"
  - "lib.rs re-exports KrunBackend and KrunRunning"
affects:
  - "05-17, 05-19: VM tests launch through KrunBackend::with_job_root and inspect KrunRunning::helper_pid and job_dir"
  - "Phase 7 runner: Runner<KrunBackend<FileBlobSource>, FileStore>; maps a host Ended::TimedOut to Stage::Run (Candidate) or Stage::Judge (Judge); InfraError::Backend is retried"
  - "Phase 8 CLI: KrunBackend::new(paths, blobs); sweep_report() for minos host doctor"
tech-stack:
  added: []
  patterns:
    - "Exit observed with waitid(WNOWAIT), the group swept while the zombie leader still holds the pgid, then reaped: no member outlives the helper and no signal reaches a reused pid"
    - "One shared Reaper behind a Mutex, so the console reader can kill the group at cap + 1 without racing a reap"
    - "Pure classifier over an Observed record; host kills decide first, markers count only as whole lines with their exit code"
key-files:
  created:
    - crates/minos-backend-krun/src/backend.rs
  modified:
    - crates/minos-backend-krun/src/supervise.rs
    - crates/minos-backend-krun/src/lib.rs
    - crates/minos-core/src/outcome.rs
    - crates/minos-sandbox/src/finished.rs
    - CLAUDE.md
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/deferred-items.md
decisions:
  - "The EPERM flake's mechanism is proven: on macOS, kill(-pgid, 0) on a group whose only members are unreaped zombies returns EPERM (5 of 5), and ESRCH after the reap; EPERM is accepted only with that proof"
  - "VMM_STDIO_CAP rises from 65536 to 131072, twice minos-vmm's 64 KiB log budget, so the watchdog marker printed after a full log is never cut off; stderr past the cap is drained, never left to block the helper"
  - "Stderr control characters are replaced with U+FFFD rather than stripped, so stripping can never turn a line into a marker"
  - "A chunk fault wins over exit 0 and over exit 124 with its marker (both mean the guest ran); a host kill wins over everything"
  - "launch refuses a host whose own probes failed or that lacks Seatbelt, whatever capabilities the plan was cleared against (defence in depth for T-05-15-04)"
  - "Parts and the root image path are fetched before the job directory is created, so a blob failure leaves nothing on disk"
  - "The chunk receiver starts before the helper is spawned, so no failure after the spawn can leave a VM without its receiver"
  - "Root image branch: store-path (05-EVIDENCE.md Mechanism selections); no hard link and no copy"
metrics:
  duration: 18min
  completed: 2026-10-18
  tasks: 3
  files: 8
---

# Phase 5 Plan 15: Production supervisor and KrunBackend Summary

**`KrunBackend<B: BlobSource>` is the real `Backend`. It sweeps and probes the host once at construction. A launch fetches the plan's blobs under caps, names the store's verified root image read-only, writes `input.img` and `vmm.json` into a fresh job directory, starts the chunk receiver, and spawns the helper through `sandbox-exec -p`. The supervisor puts the console and stderr on capped pipes and kills the whole process group at the deadline, at the console cap, or on a chunk fault. A pure, exhaustively tested `classify` turns what the host observed into an `Ended` or an infrastructure failure.**

## What was built

### Task 1: the production supervisor and the pure classifier (KRUN-01, KRUN-05)

The flake investigation came first. The Phase 2 test `supervise_run_kills_the_whole_process_group_at_the_host_deadline` passed 20 of 20 runs before any change, with no `EPERM`. After the rewrite, `EPERM` showed up 3 times in 100 runs, always in `drop_kills_group`, where the group's only member was the test's own unreaped zombie.

A throwaway diagnostic proved the mechanism, 5 of 5 rounds, recorded in 05-EVIDENCE.md:
- `kill(-pgid, 0)` returned `EPERM` while the killed child was unreaped.
- `ps` listed that child as the group's only member, in state `Z`.
- The probe returned `ESRCH` once the child was reaped.

Three changes followed from the proof:
- `Helper::drop` now reaps right after the kill.
- `Reaper::kill` accepts `EPERM` only when `waitid(WNOWAIT)` shows the helper has exited.
- The tests' probe waits on `EPERM` only while `ps` shows no live member, and still ends only on `ESRCH`.

After the fix, 200 runs of the supervise tests and 50 runs of the whole crate's unit tests passed with no failure.

**Spawning.** `Helper::spawn(&argv, console_cap)`:
- refuses an empty argv or a relative program;
- spawns with `env_clear()`, stdin `Stdio::null()`, stdout and stderr from `std::io::pipe()`, and `process_group(0)`;
- drops the `Command` so the readers get end-of-file.

**The two pipe readers.**
- The console reader reads through `cap + 1` bytes. When it has them, it raises `console_over` and kills the group under the shared `Reaper` lock.
- The stderr reader keeps `VMM_STDIO_CAP + 1` bytes and drains the rest.

**`watch`.** It polls every 10 ms. It kills the group once, for the first of:
- the console cap (reported even when the reader's own kill ends the helper first);
- `faulted()`;
- the deadline.

It returns `Survived` if the helper is still alive 5 s after the kill.

**Reaping.** `try_wait` observes the exit with `waitid(WNOWAIT)`, kills whatever is left in the group, and only then reaps.

**`finish`.** It joins both readers within 2 s, or returns `PipeOpen`. Stderr is split into newline-terminated lines.

**`classify`.** Host kills decide first. After that, exit 0 decides by the stream. Exit 124 needs the exact `DEADLINE_MARKER` line. Everything else is infrastructure:
- 124 without its marker;
- 126 with or without its marker;
- 64 to 71;
- signals the host did not send;
- a host kill with no recorded reason;
- any other code, including 125.

**Removed.** The whole spike API and every helper only it used (the plan's list). A grep confirms none remain.

**Tests.** 27 supervise tests: the classify table, whole-line markers, and nine fake-helper tests:
- `yes` at a 65536 cap: exactly 65537 bytes kept, `ConsoleCap`, group gone;
- `sleep` killed at a 300 ms deadline;
- `sh` with a backgrounded sleep: deadline kill, every member gone;
- a member left behind at exit is killed;
- chunk-fault kill;
- drop kill;
- `env` sees an empty environment;
- `cat` sees EOF on stdin;
- a real stderr pipe carrying the marker.

### Task 2: KrunBackend implements Backend (D-13, D-14, D-15; KRUN-04, KRUN-07)

**Construction.**
- `KrunBackend::new(paths, blobs)` opens `JobRoot::open()` and calls `with_job_root`.
- `with_job_root` runs `sweep(&root, vmm)` and `probe_host(&paths, <root's parent>)` once and keeps `into_capabilities()`.
- `capabilities()` returns a clone of the stored capabilities.

**`launch(ok)`, in order:**
1. `check_host`: refuse on any host refusal or a missing Seatbelt layer.
2. `fetch_parts`.
3. `root_image(plan.root())`, the store-path branch.
4. `create_job`.
5. `new_token`.
6. `write_input_disk`.
7. `vmm.json`, create_new with mode 0600. It holds the libkrunfw path and `KRUNFW_SHA256`, the root and input disks, `OUTPUT_VSOCK_PORT` and the job socket, `max_duration_ms`, and the plan's machine.
8. `UnixListener::bind`.
9. `ChunkReceiver::spawn`.
10. `ProfileParams` from the canonical paths.
11. `seatbelt::argv(&params, vmm, &[config])`.
12. `Helper::spawn`.
13. `write_pid`.

The deadline is spawn time + `max_duration` + 30 s.

**`KrunRunning::wait`.**
- It runs `watch` against `receiver.faulted()`, measures the elapsed time, calls `finish`, calls `stop_and_join`, and classifies.
- `Infra(what)` becomes `InfraError::Backend`. `Ended(e, stream)` becomes `Finished::new(cleared, e, stream, elapsed, console)`.
- The job directory is dropped last, which deletes it with its input disk.
- The struct's field order kills the helper first, then stops the receiver, then deletes the directory, so a plain drop behaves the same.

**Tests.** Four unit tests with an in-file map `BlobSource`:
- a missing job blob gives `LaunchError::Blob(Missing)`;
- a job blob for another trial gives `LaunchError::Backend` "does not match the plan";
- a missing root image gives `LaunchError::Blob(RootImageMissing)`;
- a host without Seatbelt is refused.

Each launch failure leaves no `mns` directory, and none spawns a helper.

The `lib.rs` crate doc is rewritten around the launch flow. It no longer mentions initramfs.

### Task 3: documentation

- **`minos-core` `outcome.rs`:** the K7 table now has the console cap seen on the host's side of the pipe, `Rejected(ChunkProtocol)`, exit 0 with no END, and whole-line markers paired with their exit code.
- **`CrashCause::ConsoleCap`:** names the pipe mechanism.
- **CLAUDE.md, minos-sandbox section:**
  - `KrunBackend::new(paths, blobs)` takes its `BlobSource` (D-13), and the `deny.toml` layering is unchanged;
  - the bullet on how a backend receives blob bytes is removed from Open questions.
- **CLAUDE.md, Local backends:**
  - the classification as `supervise::classify` ships it, and the Pitfall 12 stage mapping;
  - the 128 KiB stderr cap;
  - the waitid sweep;
  - the zombie `EPERM` finding;
  - `wait` deleting the job directory (D-15).

## Verification

- `cargo fmt --all --check`: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `env -u INSTA_UPDATE cargo test --workspace`: exit 0, 60 result lines, all ok
- `cargo deny check`: advisories, bans, licenses, and sources ok. The duplicate-version warnings were already there.
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no MISSING line. C-02: prints nothing.
- Every acceptance grep for the three tasks matches. `MINOSST1` and the removed names count 0. The supervise filter runs 27 tests.
- No VM was launched. Full launches are 05-17's and 05-19's, as the plan says.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The console reader's own kill raced `watch`**
- **Found during:** Task 1
- **Issue:** The helper could die from the reader's kill before `watch` saw `console_over`, so the run was not reported as `ConsoleCap`.
- **Fix:** `watch` reports `ConsoleCap` when the flag is up at exit. The flag is raised before the kill.
- **Commit:** e5a3945

**2. [Rule 1 - Bug, proven] EPERM on a zombie-only group**
- **Found during:** Task 1
- **Issue:** See Task 1 above, and 05-EVIDENCE.md "05-15 EPERM on a zombie-only process group". `EPERM` is accepted only with that proof.
- **Commit:** e5a3945

**3. [Rule 2 - Correctness] Stderr cap and draining**
- **Found during:** Task 1
- **Issue:** At 65536 bytes, the cap equals the helper's log budget alone, so a guest that filled the log could push the 124 marker past the cap and turn a timeout into a retry.
- **Fix:** The cap is now 131072 bytes, and stderr past it is drained, so the helper never blocks.
- **Commit:** e5a3945

**4. [Rule 2 - Security] Replace stderr control characters, don't strip them**
- **Found during:** Task 1
- **Issue:** Stripping a control character could turn a line such as `...dead\0line` into a marker.
- **Fix:** Control characters become U+FFFD. An unterminated tail is not a line.
- **Commit:** e5a3945

**5. [Rule 2 - Correctness] Members left behind at exit**
- **Found during:** Task 1
- **Issue:** A group member that outlived the helper would hold the pipes and stay running.
- **Fix:** `try_wait` sweeps the group under `waitid(WNOWAIT)` before the reap.
- **Test:** `a_member_left_behind_at_exit_is_killed`
- **Commit:** e5a3945

**6. [Signature] `Helper::spawn` takes `&[OsString]`**
- **Issue:** Taking a `Vec` failed clippy's needless-pass-by-value lint.
- **Commit:** e5a3945

**7. [Structure] The poll loop is `supervise::watch`**
- **Why:** The fake-helper tests and `KrunRunning::wait` share one loop.
- **Commit:** e5a3945

**8. [Rule 2 - Defence in depth] `launch` re-checks the backend's own probes**
- **Fix:** `launch` refuses when the backend's probes recorded a host refusal or the Seatbelt layer is missing (T-05-15-04).
- **Commit:** 41a0292

**9. [Order] Fetch first, receiver before helper**
- **What:** Parts and the root image are fetched before the job directory is created, and the receiver starts before the helper.
- **Why:** A blob failure leaves nothing on disk, and no VM ever runs without its receiver.
- **Commit:** 41a0292

**10. [Doc] `KILL_WAIT` and `PIPE_DRAIN_WINDOW` are public**
- **Why:** Public docs link to them.
- **Commit:** 41a0292

### Not changed here
- The sweep in `jobdir` still reports `EPERM` as Unverified. That is correct for an orphan whose state it cannot prove.

## Requirements

- **Marked complete:** KRUN-07. The layers come from the probes into `Capabilities`, and `backend_info` builds the `BackendInfo`. `launch` refuses without Seatbelt rather than falling back. No later plan in this phase lists KRUN-07.
- **Already complete:** KRUN-01 and KRUN-04.
- **Left pending:** KRUN-05. The `kill -9` proof is 05-19's and 05-20's.

## TDD Gate Compliance

Task 1: RED 55379c7 (`test(05-15)`). Stubbed `classify`, `watch`, and `whole_lines` failed 18 of 27 tests. GREEN e5a3945 (`feat(05-15)`) followed. Task 2's tests went in with its implementation in 41a0292.

## Known Stubs

None.

## Threat Flags

None. The new surface is covered by T-05-15-01 to T-05-15-07: helper spawn, signalling, pipes, and the job directory.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/backend.rs, crates/minos-backend-krun/src/supervise.rs, crates/minos-backend-krun/src/lib.rs
- FOUND commits: 55379c7, e5a3945, 41a0292, 1b8c5b4
