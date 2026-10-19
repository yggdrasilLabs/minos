---
phase: 05-guest-krun-backend-and-environment-build
reviewed: 2026-10-19T21:47:32Z
depth: standard
files_reviewed: 87
files_reviewed_list:
  - crates/minos-backend-krun/Cargo.toml
  - crates/minos-backend-krun/src/backend.rs
  - crates/minos-backend-krun/src/chunks.rs
  - crates/minos-backend-krun/src/host.rs
  - crates/minos-backend-krun/src/input_disk.rs
  - crates/minos-backend-krun/src/jobdir.rs
  - crates/minos-backend-krun/src/launch_config.rs
  - crates/minos-backend-krun/src/lib.rs
  - crates/minos-backend-krun/src/minos-vmm.sb
  - crates/minos-backend-krun/src/rootfs.rs
  - crates/minos-backend-krun/src/seatbelt.rs
  - crates/minos-backend-krun/src/supervise.rs
  - crates/minos-backend-krun/tests/exit.rs
  - crates/minos-backend-krun/tests/harness/mod.rs
  - crates/minos-backend-krun/tests/vm.rs
  - crates/minos-cli/Cargo.toml
  - crates/minos-cli/src/blob_source.rs
  - crates/minos-cli/src/builder.rs
  - crates/minos-cli/src/env_build.rs
  - crates/minos-cli/src/host.rs
  - crates/minos-cli/src/lib.rs
  - crates/minos-cli/src/locate.rs
  - crates/minos-cli/src/main.rs
  - crates/minos-cli/tests/env_build.rs
  - crates/minos-cli/tests/host_cmd.rs
  - crates/minos-core/src/chunk.rs
  - crates/minos-core/src/guest.rs
  - crates/minos-core/src/id.rs
  - crates/minos-core/src/ledger.rs
  - crates/minos-core/src/lib.rs
  - crates/minos-core/src/limits.rs
  - crates/minos-core/src/lock.rs
  - crates/minos-core/src/manifest.rs
  - crates/minos-core/src/outcome.rs
  - crates/minos-core/tests/blob_hasher.rs
  - crates/minos-core/tests/chunk.rs
  - crates/minos-core/tests/guest_wire.rs
  - crates/minos-core/tests/lock.rs
  - crates/minos-core/tests/manifest.rs
  - crates/minos-guest/Cargo.toml
  - crates/minos-guest/clippy.toml
  - crates/minos-guest/src/bin/minos-probe.rs
  - crates/minos-guest/src/lib.rs
  - crates/minos-guest/src/linux.rs
  - crates/minos-guest/src/main.rs
  - crates/minos-guest/src/nofollow.rs
  - crates/minos-guest/src/pack.rs
  - crates/minos-guest/src/pattern.rs
  - crates/minos-guest/src/run.rs
  - crates/minos-guest/src/sink.rs
  - crates/minos-guest/src/source.rs
  - crates/minos-guest/src/stage.rs
  - crates/minos-guest/src/test_fixture.rs
  - crates/minos-guest/src/workspace.rs
  - crates/minos-sandbox/src/backend.rs
  - crates/minos-sandbox/src/blob_source.rs
  - crates/minos-sandbox/src/finished.rs
  - crates/minos-sandbox/src/lib.rs
  - crates/minos-sandbox/src/output.rs
  - crates/minos-sandbox/src/preflight.rs
  - crates/minos-sandbox/src/spec.rs
  - crates/minos-sandbox/src/untrusted.rs
  - crates/minos-sandbox/tests/backend.rs
  - crates/minos-sandbox/tests/blob_source.rs
  - crates/minos-sandbox/tests/output_tar.rs
  - crates/minos-sandbox/tests/preflight.rs
  - crates/minos-sandbox/tests/spec.rs
  - crates/minos-sandbox/tests/ui/fail/finished_twice.rs
  - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
  - crates/minos-sandbox/tests/untrusted.rs
  - crates/minos-source/src/walk.rs
  - crates/minos-store/src/file/blobs.rs
  - crates/minos-store/src/file/mod.rs
  - crates/minos-store/src/lib.rs
  - crates/minos-store/src/memory.rs
  - crates/minos-store/tests/contract.rs
  - crates/minos-store/tests/file_blobs.rs
  - crates/minos-vmm/Cargo.toml
  - crates/minos-vmm/clippy.toml
  - crates/minos-vmm/golden/config-v2.json
  - crates/minos-vmm/src/config.rs
  - crates/minos-vmm/src/launch.rs
  - crates/minos-vmm/src/log.rs
  - crates/minos-vmm/src/main.rs
  - crates/minos-vmm/src/preflight.rs
  - crates/minos-vmm/src/watchdog.rs
findings:
  critical: 2
  warning: 4
  info: 7
  total: 13
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-10-19T21:47:32Z
**Depth:** standard
**Files Reviewed:** 87
**Status:** issues_found

## Summary

Reviewed the Phase 5 krun backend (`minos-backend-krun`), the `minos-vmm` launcher, the guest (`minos-guest`), the Phase 5 additions to `minos-core` (`chunk`, `guest`, `lock`, `limits`, the `[environment]` memory fields), the sandbox types the backend consumes, the store's streamed put and verified path, and the `minos env build` / `minos host` CLI paths. No cargo command was run; every finding is from reading the code. The pre-existing files on the list (`walk.rs`, `id.rs`, `ledger.rs`, `manifest.rs`, store, sandbox) were reviewed against their Phase 5 diffs (`4d80241^..HEAD`) plus the full text of the functions those diffs touch.

The security invariants hold as far as I can prove them from the code: no role has a network method; the VM has no network device, no virtio-fs, and inet hijack is banned by `clippy.toml`; every input to the guest arrives as a read-only raw disk and every output leaves as an authenticated chunk stream that is parsed under caps into `Untrusted`; judge data reaches only a `SandboxSpec<Judge>` and `check_role` in the guest refuses it for a candidate; the per-launch token never enters a blob, log, config, or argv; `Finished` consumes `Cleared`; the forbidden lints are not silenced anywhere (`grep '#!?\[allow\('` is empty, no `unwrap`, `expect`, `unsafe`, `HashMap`, or `SystemTime::now` in production code); no shell string is built from manifest or model input; all order-sensitive iteration is over `BTreeMap`/`BTreeSet` or sorted `Vec`s.

Two findings are blockers. Both are wrong-blame paths: a legitimate candidate is recorded as the candidate's fault, which the append-only ledger then keeps forever, and the "errors separate blame" rule is a stated code rule. (CR-01) `RLIMIT_CPU` is set to the wall-clock budget plus one second per process, but CPU time sums across a process's threads, so on any VM with more than one vCPU a parallel `rustc`/`cargo` build is killed by `SIGXCPU` before PID 1's wall-clock deadline and recorded as `Signaled { signal: 24 }`; every VM test runs one vCPU, which is why nothing observed it. (CR-02) A source failure that kept the token sends a status hardcoded to `role: candidate`; on a Judge launch the host's `take_status` refuses it as `WrongRole`, a candidate fault that is never retried, although the cause is a host or disk fault.

The warnings are defense-in-depth gaps (the root filesystem is mounted without `nosuid`, so a base image's setuid binaries hand uid 64000 root inside the VM; `env build` keeps setid bits and never reports them), a configuration failure that is misreported as an infrastructure retry loop (scratch too small for the role's inputs), an unbounded blocking `wait()` in `Helper::drop` after a helper survives `SIGKILL`, and a VMM-side socket error that is blamed on the candidate.

## Critical Issues

### CR-01: `RLIMIT_CPU` fires before the wall-clock deadline on multi-vCPU VMs and blames the candidate

**File:** `crates/minos-guest/src/stage.rs:120` (the limit), `:23-24` (the claim), `:98` (the cap `MAX_CPU_SECONDS`); applied in `crates/minos-guest/src/linux.rs:277`
**Issue:** `StageLimits::for_stage` sets `cpu_seconds = budget_ms.div_ceil(1000) + 1` and the module docs say this "backs up PID 1's own wall-clock deadline without ever firing before it". That is only true for a single-threaded process on a single vCPU. `RLIMIT_CPU` is per process and counts the CPU time of every thread of that process (`check_process_timers` uses the thread group's cputime), so on a VM with `vcpus = N` a process that keeps N threads busy exhausts the limit after about `budget / N` seconds of wall time and dies with `SIGXCPU` (24). `cargo build --release`, the CLAUDE.md example's build command, runs `rustc` with codegen threads, and every `rustc` is a separate process with its own 121 s CPU budget against a 120 s wall budget. The stage is then recorded as `StageEnd::Signaled { signal: 24 }` and the runner records a candidate failure, never a timeout, although PID 1's own deadline (`linux.rs:444-469`) had not passed. The VM tests cannot see this: `tests/harness/mod.rs:500` and `:533` build every machine with `Machine::new(1, ...)`. The plan's `Machine` carries `vcpus` but `GuestJob` does not, so the guest cannot scale the limit today.
**Fix:** Scale the CPU limit by the vCPU count and raise the trampoline's cap to match. Reading the count in the guest avoids a wire change (the job file has a `v` tag; adding a required field would need version 2):
```rust
// stage.rs
pub fn for_stage(job: &GuestJob, stage: &StageSpec, vcpus: u64) -> Self {
    let wall_seconds = stage.budget_ms().div_ceil(1000).saturating_add(1);
    Self {
        // every thread of the process may run on its own vCPU for the whole budget
        cpu_seconds: wall_seconds.saturating_mul(vcpus.max(1)),
        ..
    }
}
const MAX_CPU_SECONDS: u64 = (MAX_STAGE_MS.div_ceil(1000) + 1) * MAX_VCPUS as u64;
```
and in `linux.rs` pass `std::thread::available_parallelism().map_or(1, |n| n.get() as u64)` (PID 1 runs on every vCPU, so this is the VM's count). Fix the doc comment at `stage.rs:23-24` and add a unit test with `vcpus = 8` against `MAX_CPU_SECONDS`. If a wire change is acceptable instead, add `vcpus` to `RawGuestJob` under `GUEST_JOB_VERSION = 2` and compare it in `input_disk::job_matches` (`crates/minos-backend-krun/src/input_disk.rs:165-181`), which today checks memory but not vCPUs.

### CR-02: A source failure on a Judge launch is reported with `role: candidate`, which the host rejects as a candidate fault

**File:** `crates/minos-guest/src/run.rs:232-241` (the hardcoded role); `crates/minos-sandbox/src/untrusted.rs:192-209` (`take_status` refuses the role before anything else); `crates/minos-guest/src/source.rs:239-253` (the role is dropped even when the job parsed)
**Issue:** When the input disk read fails after the header was read (bad job JSON, `check_role` mismatch, a section digest that does not match, a non-UTF-8 diff), `run_job` sends an authenticated status with `GuestRole::Candidate` and `ApplyStatus::Baseline` "because the role is unknown", and the comment says the host "consults neither field". The host does: `Untrusted::<GuestOutput<Judge>>::take_status` parses the status and returns `StatusFault::WrongRole` when `status.role() != Judge`, and `StatusFault::reason()` maps every variant to `Reason::OutputInvalid`, a candidate fault that is never retried. So a judge VM whose input disk was corrupted on the host (a host bug, a torn write in `/private/tmp`, a store fault) produces `Rejected(OutputInvalid)` for the candidate under judgment, and the append-only `Judged` line keeps that verdict. This is the exact case the "failed setup with token" path exists to report as infrastructure, and it contradicts both "errors separate blame" and the guest's own documentation of `SetupStatus::Failed`. `GuestStatus::new` (`core/guest.rs:713-715`) also forces `apply == NotApplicable` iff `role == Judge`, so the guest cannot simply send `role: judge` without also guessing `apply`. In the common sub-case the role *is* known: `decode` in `source.rs:267-287` fails after `GuestJob::parse_json` succeeded, but `SourceFailure` carries only the token.
**Fix:** Make an Input-failed status role-agnostic on the host side, and carry the role when the guest knows it:
```rust
// untrusted.rs, take_status: an input failure precedes the role, so it cannot be checked
let status = GuestStatus::parse_json(&bytes).map_err(StatusFault::Invalid)?;
let input_failed = matches!(status.setup(), SetupStatus::Failed { step: SetupStep::Input });
if !input_failed && status.role() != guest_role(R::KIND) {
    return Err(StatusFault::WrongRole);
}
```
Document on `StatusFault::WrongRole` and `SetupStatus::Failed` that an `Input` failure is checked for `setup` before `role` and that Phase 7 maps every `SetupStatus::Failed` to an infrastructure failure before it reads any other field. Then let `SourceFailure` carry `Option<GuestRole>` (set in `decode` once `parse_json` succeeded) so `run_job` sends the real role and the matching `ApplyStatus` (`Baseline` for a candidate, `NotApplicable` for a judge) whenever it can; add a `run_source_failure_with_token` case for a judge disk and a `take_status` test that a judge output carrying an Input-failed candidate-role status is accepted.

## Warnings

### WR-01: The root filesystem is mounted without `nosuid`, so setuid binaries from the base image give the stage uid root inside the VM

**File:** `crates/minos-guest/src/linux.rs:100-110` (only `/proc` and `/sys` get `NOSUID|NODEV`; `/` is left as the kernel mounted it); `crates/minos-backend-krun/src/rootfs.rs:465`, `:885-887`, `:971` (setid bits are counted and kept); `crates/minos-cli/src/main.rs:138-144` (`setid_files` is never printed)
**Issue:** The SquashFS root is booted `ro` (`crates/minos-vmm/src/launch.rs:31`), but `ro` is not `nosuid`. The packer keeps the setuid and setgid bits of every file in the user's image (`meta_of` masks with `0o7777`), and ordinary base images ship setuid-root binaries (`su`, `mount`, `umount`, `passwd`, `chsh`, `newgrp`, and `sudo` when installed). Every stage runs as uid 64000 so that the candidate "cannot see PID 1" and "runs under the D-05 limits", but a setuid binary runs with euid 0 and can `mknod` a new `/dev/vdb` and read the per-launch token (undoing `protect_input_disk`), read `/proc/1/environ` and PID 1's memory, raise its own rlimits, and reach every root-only kernel interface the threat model wants to keep away from the candidate. None of this crosses the VM boundary, and the only thing the token protects is the candidate's own output stream, so this is a defense-in-depth gap rather than a judge-integrity break; but the stated uid isolation does not hold for a stock Debian base image, and the operator is never told the image has setid files.
**Fix:** In `pid1()`, after the pseudo-filesystem mounts and before any stage, remount the root with the missing flags (a bind remount changes per-mount flags on a read-only SquashFS):
```rust
use rustix::mount::{MountFlags, mount_remount};
// / stays read-only; add nosuid and nodev so no file in the user's image can change uid
if let Err(errno) = mount_remount("/", MountFlags::BIND | MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV, "") {
    console_line(&format!("minos-guest: remount / nosuid: {errno}"));
}
```
Treat a failed remount as a setup failure (do not run a stage). Independently, print `setid_files`, `hardlinks_copied`, and `init_overridden` from `PackReport` in `env_build_command`, and consider clearing `SETID_BITS` in `meta_of` so the image cannot carry them at all. Add a VM test that execs a setuid-root binary from the test image and asserts euid stays 64000.

### WR-02: Scratch capacity is never checked against the role's inputs; a too-small `scratch_mib` becomes an infrastructure retry loop

**File:** `crates/minos-core/src/manifest.rs:1643-1668` (`memory_from` checks only the memory budget), `crates/minos-core/src/limits.rs:217` (`MIN_SCRATCH_MIB = 64`), `crates/minos-guest/src/workspace.rs:40-59` (`UnpackCaps`: base 64 MiB, judge data 96 MiB, candidate output 16 MiB), `crates/minos-guest/src/run.rs:292-302` and `:376-394` (ENOSPC on unpack is `SetupStatus::Failed { step: Unpack }`)
**Issue:** The manifest accepts `scratch_mib = 64`, but the guest unpacks the base tarball (up to 64 MiB of content plus the `out`, `home`, and `tmp` directories) into that tmpfs for a candidate, and up to 96 MiB of judge data plus up to 16 MiB of candidate output for a judge. When the tmpfs fills, `unpack_tar` returns `UnpackError::Io(ENOSPC)`, PID 1 sends `setup: failed {unpack}`, and the host treats a failed setup as an infrastructure failure that is retried. The retry can never succeed, so the run burns `max_infra_failures` launches (each a full VM boot) and stops with a breaker message that points nowhere near `scratch_mib`. The same inputs decide the outcome every time, which is the definition of a configuration error in this codebase (`spec.rs:42-51` makes exactly that argument for `DurationOverCeiling`).
**Fix:** Decide it before a VM boots. Either raise `MIN_SCRATCH_MIB` to cover the worst case and say why (`MAX_JUDGE_DATA_BYTES + MAX_OUTPUT_CONTENT_BYTES` rounded up, about 128 MiB, is the floor a judge needs), or have the Phase 7 runner and `minos check` refuse a manifest whose `scratch_mib` is below the role's input caps, mirroring the `SpecError` handling. In the guest, map `io::ErrorKind::StorageFull` from `unpack_tar` and `apply_diff` to a distinct `SetupStep` (or `UnpackError::ScratchFull`) so the host can report "scratch too small" instead of retrying. Add a manifest test for the floor.

### WR-03: `Helper::drop` blocks without bound on a helper that survived `SIGKILL`

**File:** `crates/minos-backend-krun/src/supervise.rs:424-436` (`Helper::drop` calls `child.wait()` under the lock), `:252-259` (`Reaper::drop` does the same), `:527` (`watch` returns `Survived` after `KILL_WAIT`)
**Issue:** `watch` gives up after `KILL_WAIT` (5 s) with `SuperviseError::Survived`, and `KrunRunning::wait` maps that to an `InfraError` and returns. But the `?` at `backend.rs:233` drops `helper`, and `Helper::drop` calls `reaper.child.wait()`, which blocks until the helper actually dies. A helper that did not die within 5 s of `SIGKILL` is stuck in the kernel (a Hypervisor.framework teardown or an uninterruptible I/O), and the runner thread now hangs for as long as that takes, while holding the `Reaper` mutex so the console reader cannot finish either. The whole point of `Survived` is to let the caller report and move on; the drop undoes it. The same applies to `Reaper::drop` on any early-return path.
**Fix:** Make the drop bounded: poll `try_wait` for at most `KILL_WAIT`, re-sending `kill` each poll, and give up (leaving the zombie for the parent's eventual exit and recording the pid) instead of calling the blocking `wait()`:
```rust
fn drop(&mut self) {
    let mut reaper = lock(&self.reaper);
    if reaper.exit.is_some() { return; }
    let until = Instant::now() + KILL_WAIT;
    loop {
        reaper.kill().ok();
        match reaper.child.try_wait() {
            Ok(Some(status)) => { reaper.exit = Some(status); return; }
            _ if Instant::now() >= until => return, // reported as Survived; do not hang the runner
            _ => thread::sleep(POLL_INTERVAL),
        }
    }
}
```
Apply the same shape to `Reaper::drop`.

### WR-04: A VMM-side socket error mid-chunk is recorded as a chunk-protocol fault of the candidate

**File:** `crates/minos-backend-krun/src/chunks.rs:371` (`Err(_) => return Ok(Ok(bytes))`), `:233-236` (the partial bytes go to `Reassembler::accept`), `crates/minos-core/src/chunk.rs:355-357` and `:380-382` (a short connection is `Short` or `Length`)
**Issue:** The receiver's peer on the Unix socket is the `minos-vmm` process, not the guest. A read error other than `WouldBlock`/`Interrupted` (`ECONNRESET` when the VMM drops a proxy, as the three pinned `msb_krun_devices` defects do under load, DIAG-01) ends the connection with whatever bytes arrived, and those bytes are fed to the reassembler as if the guest had sent a truncated chunk. The result is `ChunkFault::Short` or `Length`, `fault_flag` is raised, the supervisor kills the VM, and `classify` returns `Ended::ChunkProtocol`: a candidate fault that is never retried, for a defect in the VMM. The chunked protocol was adopted precisely to stay inside the VMM's limits, so this should be rare, but when it happens the blame is wrong and permanent.
**Fix:** Keep host-side socket errors apart from the guest's bytes. Return a third outcome from `read_connection` (for example `Err(io::Error)` for a socket error with partial data, distinct from the guest's `ChunkFault`), and have `Receiver::run` end the stream as a host failure (`io::Result::Err`, which `stop_and_join` already maps to `InfraError::Io`) rather than a `ChunkFault`:
```rust
Err(error) if error.kind() == ErrorKind::ConnectionReset => {
    return Err(io::Error::new(ErrorKind::ConnectionReset, "the VMM reset an output connection"));
}
```
Document in `chunks.rs` which errors are the VMM's and add a unit test that resets a connection after a partial write and asserts the receiver reports a host error, not a fault.

## Info

### IN-01: A helper that exits in the same poll tick as the host deadline is recorded `TimedOut`

**File:** `crates/minos-backend-krun/src/supervise.rs:510-525` (`watch` decides a kill, then calls `kill_group`), `:195-199` (`Reaper::kill` sets `host_killed` before learning the leader already exited)
**Issue:** `watch` checks `try_wait` and then, up to 10 ms later, the deadline; a helper that exited 0 with a complete stream in that window is still "killed" (`killed = Some(Deadline)`) and `classify` returns `Ended::TimedOut` because a host kill decides first. The window is one poll interval and sits 30 s past the helper's own deadline, so it is improbable, but the verdict is wrong when it happens.
**Fix:** Decide the kill only after a second `try_wait` under the same lock (`Reaper::kill` can return whether the leader was already a zombie via `exited_unreaped`, and `watch` can leave `killed` unset in that case).

### IN-02: `locate` looks beside the symlink, not beside the resolved binary

**File:** `crates/minos-cli/src/locate.rs:109-110`
**Issue:** `locate` canonicalizes the parent directory of `current_exe()` but not the executable itself. A `minos` reached through a symlink (`~/bin/minos -> …/target/debug/minos`) is looked up beside `~/bin`, where no helper lives, although the docs say "the canonical directory of the running `minos`".
**Fix:** `fs::canonicalize(exe)` first, then take its parent.

### IN-03: `minos.lock` is written with mode 0600

**File:** `crates/minos-cli/src/env_build.rs:421-427`
**Issue:** `tempfile::Builder::tempfile_in` creates the file 0600 and `persist` renames it over `minos.lock`, so a committed, shareable project file ends up unreadable by other local users and differs from every other file the user committed.
**Fix:** Set `Permissions::from_mode(0o644)` on the temporary file before `persist`, or use the user's umask explicitly.

### IN-04: `env build` does not report what the packer changed in the user's image

**File:** `crates/minos-cli/src/main.rs:138-144`
**Issue:** `PackReport` records `setid_files`, `hardlinks_copied`, `replaced_duplicates`, `skipped_global_headers`, and `init_overridden`, but the command prints only files, directories, symlinks, and the `/init` digest. A Dockerfile that shipped its own `/init` (replaced silently) or setuid binaries (see WR-01) is not visible to the operator.
**Fix:** Print each non-zero counter and `init_overridden` on its own line.

### IN-05: The entitlement probe displays the signature but never verifies it

**File:** `crates/minos-backend-krun/src/host.rs:450-460`
**Issue:** `codesign --display --entitlements - --xml` prints the entitlements of whatever signature blob is attached, valid or not. A helper whose code was modified after signing still passes the probe and is recorded with the `Rlimits` layer; the launch then fails at `hv_vm_create` (exit 70, an infrastructure failure) with no probe pointing at the cause.
**Fix:** Run `codesign --verify --strict <vmm>` first and refuse with `EntitlementMissing` (or a new refusal) when it fails.

### IN-06: Timing assertions in tests can flake on a loaded machine

**File:** `crates/minos-backend-krun/src/chunks.rs:578` (`< 1 s` with a 200 ms deadline and 50 ms polls), `crates/minos-backend-krun/src/supervise.rs:1116` (`< 5 s`), `crates/minos-backend-krun/tests/vm.rs:373-376` (a 16 MiB VM launch `< 15 s`)
**Issue:** These assert wall-clock bounds that depend on scheduler latency and, for `vm.rs`, on a full VM boot plus 4097 vsock connections. The first two run in the gate.
**Fix:** Loosen the gate-run bounds (for example `< 5 s` in `receive_trickle`, which still proves the deadline ended the read before the 1 s close) and keep the `vm.rs` bound as a printed measurement with a generous assertion.

### IN-07: `SourceFailure` drops the role after the job parsed

**File:** `crates/minos-guest/src/source.rs:130-136`, `:239-253`
**Issue:** `decode` parses the job before `check_role` and the section takes, so for every failure after `GuestJob::parse_json` the role is known, but `SourceFailure` carries only the token and `run_job` must guess (CR-02). Carrying `Option<GuestRole>` makes the status honest in all but the unparsable-job case.
**Fix:** Add `role: Option<GuestRole>` to `SourceFailure`, set it in `DiskJobSource::read_input` once `parse_json` succeeds, and use it in `run_job`.

---

_Reviewed: 2026-10-19T21:47:32Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
