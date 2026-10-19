---
phase: 05-guest-krun-backend-and-environment-build
fixed_at: 2026-10-19T22:11:21Z
review_path: .planning/phases/05-guest-krun-backend-and-environment-build/05-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-10-19T22:11:21Z
**Source review:** .planning/phases/05-guest-krun-backend-and-environment-build/05-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6 (fix scope `critical_warning`: CR-01, CR-02, WR-01, WR-02, WR-03, WR-04)
- Fixed: 6
- Skipped: 0
- Out of scope (Info, not attempted): IN-01 to IN-07; see "Out of scope" below

Six commits on top of `aca0761`, one per finding, 16 files changed, 667 insertions, 108 deletions. Every fix was made inside the review-fix worktree on branch `gsd-reviewfix/05-86699` and fast-forwarded onto `main` by the cleanup tail. No security invariant was weakened: no fix opens a network path, shares a host directory, trusts guest-written data, or lets a candidate reach judge data or the token. Each fix moves a wrong-blame case to the correct side of the "errors separate blame" boundary or closes a defense-in-depth gap.

## Fixed Issues

### CR-01: `RLIMIT_CPU` fires before the wall-clock deadline on multi-vCPU VMs and blames the candidate

**Files modified:** `crates/minos-guest/src/stage.rs`, `crates/minos-guest/src/run.rs`, `crates/minos-guest/src/linux.rs`
**Commit:** cca5417
**Applied fix:** `StageLimits::for_stage(job, stage, vcpus: u8)` multiplies the stage budget in whole seconds plus one by the vCPU count, clamped to `1..=MAX_VCPUS`, so a process that keeps every vCPU busy for the whole budget is still within its CPU limit when PID 1's wall-clock deadline fires. `MAX_CPU_SECONDS` (the trampoline's cap) is now `(MAX_STAGE_MS.div_ceil(1000) + 1) * MAX_VCPUS`, so the trampoline accepts every limit `for_stage` can produce. `run_job` takes `vcpus` and threads it through `Parts`; PID 1 passes `std::thread::available_parallelism()` (saturated to `u8`, 1 when the kernel cannot report it). The module doc now states why the limit scales. New unit test `stage_cpu_limit_scales_with_vcpus_within_the_cap` covers 8 vCPUs against the cap, 0 (treated as 1), and `u8::MAX` (clamped); `run_candidate_happy_path` runs with 2 vCPUs and asserts the count reaches the stage command's limits.

**Decision (stated as instructed):** I chose reading the count in PID 1 over adding `vcpus` to `GuestJob`. It fully fixes the finding: PID 1 runs on every vCPU of the VM, so `available_parallelism` is exactly the count the kernel schedules the stage on, and it is the count `RLIMIT_CPU` accumulates against. It needs no `GUEST_JOB_VERSION` bump and no golden change, which the wire-format rule reserves for a required new field. The one thing a wire field would add, `input_disk::job_matches` comparing the plan's `Machine::vcpus` against the job, is not needed for correctness of the limit, because the guest measures the real machine rather than trusting a declared one.

**Verified by:** guest unit tests (63 passed), `vm.rs` `experiment_applies_diff_and_runs_as_candidate_uid` (asserts `Max cpu time == build_seconds` on the one-vCPU test machine; passed), `exit.rs` `exit_env` (reports `Max cpu time 61 61` for a 60 s budget on one vCPU).

### CR-02: A source failure on a Judge launch is reported with `role: candidate`, which the host rejects as a candidate fault

**Files modified:** `crates/minos-core/src/guest.rs` (doc only), `crates/minos-guest/src/source.rs`, `crates/minos-guest/src/run.rs`, `crates/minos-sandbox/src/untrusted.rs`, `crates/minos-sandbox/tests/untrusted.rs`
**Commit:** fd11226
**Applied fix:** Both halves the review asked for. Guest: `SourceFailure` carries `role: Option<GuestRole>`; `DiskJobSource::read_input` now parses the job first (`parse_job`) and sets the role for every failure after that (`check_role`, a section digest, a non-UTF-8 diff), leaving it `None` only when the job did not parse. `run_job` sends the real role with the apply status core requires for it (`NotApplicable` for a judge, `Baseline` for a candidate or an unknown role). Host: `Untrusted::take_status` reads `setup` before `role`; for `SetupStatus::Failed { step: SetupStep::Input }` the role check is skipped, because that status may carry the placeholder role. Every later setup step is past the role and still refuses a mismatch with `StatusFault::WrongRole`. Docs on `StatusFault::WrongRole`, `take_status`, and `SetupStep::Input` say that Phase 7 maps every failed setup to an infrastructure failure before it reads any other field. This also resolves IN-07, which is the same gap.

**Invariant check:** the exemption applies only to a status that is already an infrastructure failure by its `setup` field; the status is still authenticated by the token, capped, and parsed strictly. A candidate cannot reach this path: only PID 1 can send an accepted chunk, and a forged status is a `ChunkProtocol` fault before parsing. A compromised judge guest could at most cause bounded retries (`max_infra_failures`), which it could already do by sending any failed setup.

**Tests added:** `run_source_failure_with_token_on_a_judge_disk_names_the_judge` (guest; a judge job whose disk carries a BASE section reports `role: judge`, `apply: not_applicable`, `setup: failed {input}`, no directory made), role assertions in `disk_source_failure_keeps_token` and `disk_source_diff_not_utf8`, and `take_status_input_failure_is_accepted_whatever_role_it_names` (sandbox; both role directions accepted for `Input`, `WrongRole` still raised for `Mount`, `Unpack`, `Apply`, `Output`).

### WR-01: The root filesystem is mounted without `nosuid`, so setuid binaries from the base image give the stage uid root inside the VM

**Files modified:** `crates/minos-guest/src/linux.rs`, `crates/minos-guest/src/bin/minos-probe.rs`, `crates/minos-cli/src/main.rs`, `crates/minos-backend-krun/tests/harness/mod.rs`, `crates/minos-backend-krun/tests/vm.rs`
**Commit:** 8afa3fd
**Applied fix:** PID 1 calls `harden_root()` after the `/proc` and `/sys` mounts and before any per-run work: `mount_remount("/", BIND | RDONLY | NOSUID | NODEV, "")`, a bind remount that changes only the per-mount flags of the read-only SquashFS (`/dev` is the kernel's own devtmpfs mount and is untouched). A failed remount prints a `SETUP_ERROR_MARKER` line and PID 1 then mounts no scratch, so `run_job` cannot create its directories on the read-only root and sends an authenticated `setup: failed {mount}` status with no stage run, which the host does not blame on the candidate (the same mechanism the existing scratch-mount failure path uses; "treat a failed remount as a setup failure" as the review asked). `minos env build` now prints `setid files kept: N (inert: the guest mounts the root nosuid)`, `hardlinks copied: N`, and whether `/init` replaced the image's own. The VM test image gains a second copy of the probe at `/usr/local/bin/minos-probe-setuid`, root-owned, mode 04755, and the `env` probe reports its own `exe_mode` (`st_mode & 0o7777` of `/proc/self/exe`).

**Conservative choice:** the packer still keeps setid bits (`meta_of`); clearing `SETID_BITS` would change the content and digest of every image built from a base with setuid files and is a user decision on image fidelity. With the root `nosuid` the bits are inert, and the operator now sees the count.

**Test added, as the task required for a nosuid remount:** `vm.rs` `setuid_root_binary_in_the_image_does_not_raise_the_stage_uid` execs the setuid-root copy as the build stage and asserts `exe_mode == 0o4755` (the bit really survived packing, so the exec was of a setuid-root file) and `uid == euid == gid == egid == 64000` with no supplementary groups. Observed in the run: `"exe_mode":2541` (= 0o4755), `"euid":64000`, `"uid":64000`. The existing `exit_forge` test (which opens `/dev/vdb` as the stage) still passes.

### WR-02: Scratch capacity is never checked against the role's inputs; a too-small `scratch_mib` becomes an infrastructure retry loop

**Files modified:** `crates/minos-core/src/guest.rs`, `crates/minos-core/src/limits.rs` (doc only), `crates/minos-core/tests/guest_wire.rs`, `crates/minos-guest/src/run.rs`, `crates/minos-guest/src/workspace.rs`
**Commit:** 062d4ae
**Applied fix:** The guest now tells the host that the scratch filled. `workspace.rs` maps `io::ErrorKind::StorageFull` (`ENOSPC`) on every write of `unpack_tar` and `create_parents` to the new `UnpackError::ScratchFull`, and on `apply_diff`'s write-back to the new `ApplySetupError::ScratchFull` (helpers `write_error` and `apply_write_error`). `run.rs` reports either as the new `SetupStep::ScratchFull` (`unpack_step`, `apply_step`), whose doc says it is a configuration error the same inputs reproduce on every launch, that Phase 7 must report it against `[environment] scratch_mib`, and that it must not be retried. The `MIN_SCRATCH_MIB` doc records that the floor is below the role's largest inputs and names the step the guest reports instead.

**Decisions (stated as instructed):**
- `MIN_SCRATCH_MIB` stays at 64. The design of record (CLAUDE.md: "scratch 64 to 15872 MiB") names that bound, a 64 MiB scratch is valid for a small project, and raising it is a user decision. The review offered this as one of two options; the guest-side distinct step is the other and is applied.
- `GUEST_STATUS_VERSION` stays 1. `SetupStep` gains a variant, serialized as `"scratch_full"`; under the project's wire rule an added enum variant keeps every existing status parseable (the five existing goldens and their SHA-256 pins are unchanged and pass). The wire form of the new variant is asserted inline in `guest_status_scratch_full_step_is_on_the_wire`, not as a new insta snapshot, so the `update: no` gate never had to accept one. A `v: 2` would be needed only for a required new field, a rename, or a type change.
- No manifest refusal: `Manifest::try_from` reads no file, so it cannot know a project's actual input sizes; the Phase 7 runner and `minos check` are where a pre-boot check belongs, and the new step gives them the signal.

**Tests added:** `a_full_scratch_is_scratch_full_and_other_writes_are_io` (workspace; `StorageFull` maps to `ScratchFull`, other kinds stay `Io` with their kind), `a_full_scratch_is_its_own_setup_step` (run; both mapping functions), `guest_status_scratch_full_step_is_on_the_wire` (core). `ENOSPC` itself cannot be produced on the macOS gate without a size-capped filesystem; the mapping is unit-tested at the one place the kind is inspected.

### WR-03: `Helper::drop` blocks without bound on a helper that survived `SIGKILL`

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** 3695324
**Applied fix:** New `Reaper::kill_and_reap(&mut self, wait)` re-sends the kill and polls `poll_exit` every `POLL_INTERVAL` for at most `wait`, then gives up. It goes through `poll_exit` rather than `try_wait` directly, so the existing "kill whatever is left in the group while the leader is still an unreaped zombie, then reap" ordering is preserved. `Helper::drop` and `Reaper::drop` both call it with `KILL_WAIT`; neither calls the blocking `child.wait()` any more. Docs on `Reaper`, both drops, and the new method explain the bound: `watch`/`finish` have already reported a surviving helper as `Survived`, and the drop must not hang the runner (while holding the lock the console reader needs) on a helper stuck in the kernel.

**Test added:** `drop_is_bounded_and_reaps_a_killed_helper` (a leader with a backgrounded grandchild; the drop returns well inside `KILL_WAIT` and the whole group is gone). A helper that genuinely survives `SIGKILL` cannot be manufactured in a test; the existing `drop_kills_group` and group-kill tests still pass.

### WR-04: A VMM-side socket error mid-chunk is recorded as a chunk-protocol fault of the candidate

**Files modified:** `crates/minos-backend-krun/src/chunks.rs`
**Commit:** b539a3e
**Applied fix:** `read_connection` now returns the outer `io::Error` for a read error other than `WouldBlock` or `Interrupted` (wrapped as "the VMM broke an output connection: ..." with the original kind), instead of `Ok(Ok(partial_bytes))`. The partial bytes are no longer fed to the reassembler as a truncated chunk of the guest's, so no `ChunkFault::Short`/`Length`, no fault flag, no `Ended::ChunkProtocol`. `Receiver::accept_pending` propagates it with `?`, the receiver thread ends with `Err`, and `ChunkReceiver::stop_and_join` returns it, which `KrunRunning::wait` already maps to `InfraError::Io { op: "receive the output stream" }`. The module docs gain a "Which errors are the host's" paragraph: the peer on the Unix socket is `minos-vmm`, so end-of-file carries the guest's bytes (short or empty stays the guest's fault) but a read error is the VMM's. `set_nonblocking` moved to the accept site and `read_connection` is generic over `S: AsFd` with `&S: Read`, so a test can inject the error. I applied this to every read error kind, not only `ConnectionReset` as the review's snippet showed, because every such error on that socket comes from the VMM process; the review's prose asked for exactly that separation.

**Behavioral note:** after the host error the receiver's listener closes with its thread, so PID 1's next connect fails, its sink gives up after its backoff, and the guest powers off on its own; the supervisor does not wait for the deadline. Stopping the VM early on a receiver host error (a fourth `HostKill` reason mapped to `Infra` in `classify`) would be a further refinement; it is not needed for correct blame and was left out to keep the fix scoped.

**Test added:** `a_vmm_side_read_error_is_a_host_error_not_a_fault`: a stream that serves ten bytes then fails with `ConnectionReset`, `BrokenPipe`, or `Other` yields `Err` of that kind (before the fix it yielded `Ok(Ok(ten bytes))`, which the reassembler refused as `Short`); the same ten bytes ended by a clean end-of-file are still `Ok(Ok(bytes))`, the guest's short chunk. A real `ECONNRESET` cannot be forced on a macOS Unix socket in a unit test, which is why the function is generic.

## Skipped Issues

None of the in-scope findings were skipped.

## Out of scope (Info findings, not attempted)

Listed as skipped only because `fix_scope` is `critical_warning`:

- IN-01 (a helper exiting in the same poll tick as the deadline is `TimedOut`): not attempted.
- IN-02 (`locate` looks beside the symlink): not attempted.
- IN-03 (`minos.lock` written 0600): not attempted.
- IN-04 (`env build` does not report packer changes): partly covered by WR-01's print of `setid_files`, `hardlinks_copied`, and `init_overridden`; `replaced_duplicates` and `skipped_global_headers` are still not printed.
- IN-05 (entitlement probe never verifies the signature): not attempted.
- IN-06 (timing assertions can flake): not attempted; the gate-run bounds are unchanged.
- IN-07 (`SourceFailure` drops the role): resolved by CR-02's fix, which carries `Option<GuestRole>` exactly as IN-07 asked.

## Verification record

All commands were run from the worktree root (`/tmp/sv-05-reviewfix-8DNBlo`, a clean checkout of `main` at `aca0761` plus the six fix commits; the target directory was built from scratch). `cargo clean` was not run, per instructions.

1. `cargo fmt --all --check`: no diff (fmt: OK).
2. `cargo clippy --workspace --all-targets -- -D warnings`: `Finished dev profile`, no warnings, no errors.
3. `cargo test --workspace`: 46 test binaries reported, **1147 passed, 0 failed, 5 ignored** (the five ignored are the VM suites, run in step 8 and 9). No `FAILED` or `panicked` line.
4. `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
5. `cargo doc --workspace --no-deps`: `Finished dev profile`, no rustdoc warnings; generated `target/doc/minos_backend_krun/index.html and 12 other files`.
6. `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: `Finished dev profile`, no warnings (also run per fix after each guest or probe change).
7. C-02: `grep -rnE '#!?\[allow\(' crates --include='*.rs'` printed nothing (exit 1, no matches).
8. VM build and sign: `cargo build -p minos-vmm` (Finished), `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm` ("replacing existing signature"), `codesign --display --entitlements - target/debug/minos-vmm` lists `com.apple.security.hypervisor`, `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl` (Finished; `minos-guest` 1,504,864 bytes and `minos-probe` 981,288 bytes beside it). Then with `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib` set: `cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1`: **11 passed, 0 failed** in 19.71 s (the 10 existing tests plus the new `setuid_root_binary_in_the_image_does_not_raise_the_stage_uid`; the task's expectation of 10 predates the added test). Every launch ended `Exited`; the setuid probe reported `exe_mode` 2541 (0o4755) with uid and euid 64000.
9. Helper re-signed (same argv), then with `MINOS_EXIT_CANARY=$(uuidgen)` added: `cargo test -p minos-backend-krun --test exit -- --ignored --nocapture --test-threads=1`: **4 passed, 0 failed** in 6.82 s (`exit_egress`, `exit_env`, `exit_forge`, `exit_kill9`). `exit_env` showed the stage limits `Max cpu time 61 61` (60 s budget plus one, on one vCPU), `Max address space 805306368`, `Max file size 1073741824`, `Max processes 512`, `Max open files 4096`, `Max core file size 0`, and the four-variable stage environment.

Per-fix verification before each commit: re-read of every edited region, `cargo fmt --all`, the touched crates' unit or integration tests, and `cargo clippy -D warnings` for the touched crates on the host target and (for guest or probe changes) on `aarch64-unknown-linux-musl`. One clippy error surfaced during WR-04 (a `UnixStream` import left unused by the generic `read_connection`) and was fixed before the commit.

Untracked files under `.planning/research/.cache/` were neither staged nor committed.

---

_Fixed: 2026-10-19T22:11:21Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
