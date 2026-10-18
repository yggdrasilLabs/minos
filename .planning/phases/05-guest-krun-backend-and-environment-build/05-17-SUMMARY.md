---
phase: 05-guest-krun-backend-and-environment-build
plan: 17
subsystem: minos-backend-krun
tags: [vm-tests, harness, krun, guest, pid1, rlimits, chunks, vsock-reaper, nofile, judge, guest-01, guest-02, guest-03, guest-04, krun-04, krun-10, d-05, d-14]
status: complete
requires:
  - "05-12: minos-guest PID 1, the trampoline, and minos-probe's commands and JSON keys"
  - "05-15: KrunBackend::with_job_root, launch, KrunRunning::wait, the supervisor and classify"
  - "05-09: rootfs::{pack_root_image, PackLimits, check_guest_elf} and the embedded Seatbelt profile"
  - "05-22: Finished::{ended, elapsed, console_excerpt, into_output}, Untrusted::take_status"
provides:
  - "crates/minos-backend-krun/tests/harness/mod.rs: SETUP, PROBE, PROBE_NAME, VALUE_PATH, env_path, MapBlobSource (put), Vm, vm, manifest, base_tar, diff_text, candidate, judge, exited, stage_end, drop_logs, probe_json, metric"
  - "crates/minos-backend-krun/tests/vm.rs: ten ignored functional VM tests, all passing on this Mac"
  - "minos-vmm NOFILE_LIMIT 8192 (was 1024): a whole chunked output stream fits one vsock reaper window"
affects:
  - "05-19: tests/exit.rs reuses harness/mod.rs with `mod harness;`; every harness item must be used by each test target that includes it, or -D warnings fails on dead code"
  - "05-20: records the final VM run in 05-EVIDENCE.md; GUEST-04 stays pending for 05-19's forged-token probe"
  - "Phase 7 runner: a 16 MiB output costs about 0.6 s of launch time, not 20 s, so the 30 s grace after the last stage is not consumed by output"
  - "Phase 8: RLIMIT_AS at the default allowance against a real toolchain build is still unmeasured (owner: the sample project)"
tech-stack:
  added: []
  patterns:
    - "A test BlobSource that shares its map through Arc<Mutex<..>>, so blobs are put after the backend took its clone"
    - "Probe results leave the VM only through Untrusted::take, into_log_excerpt, and serde_json; the console only through console_excerpt"
    - "Test root image packed by the production packer from an in-memory rootfs tar, so no Docker daemon is needed"
key-files:
  created:
    - crates/minos-backend-krun/tests/harness/mod.rs
    - crates/minos-backend-krun/tests/vm.rs
  modified:
    - crates/minos-vmm/src/main.rs
    - CLAUDE.md
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
decisions:
  - "The helper's RLIMIT_NOFILE is 8192: the pinned msb_krun_devices 0.1.41 vsock reaper holds every closed connection's host socket for 5 s, and a full output stream is about 4353 connections; at 1024 a 16 MiB output took 20.5 s with 300 guest connect retries, at 8192 it takes about 0.6 s with none"
  - "output_16_mib_is_reassembled asserts the launch takes under the 15 s W0-CHUNKS selection bound, so a regression of the descriptor limit fails the test"
  - "The test image adds /etc and /root to the rootfs so the write-outside probe meets a read-only root (EROFS) and a 0700 root-owned directory (EACCES), not just missing paths"
  - "Cargo.toml is unchanged: serde_json and tempfile are already normal dependencies of minos-backend-krun, which integration tests can use"
metrics:
  duration: "about 25 minutes"
  completed: 2026-10-18
  tasks: 2
  files: 5
---

# Phase 5 Plan 17: VM harness and functional VM tests Summary

**Ten ignored VM tests now boot the static musl `minos-guest` as PID 1 through the production `KrunBackend`, under the embedded deny-default Seatbelt profile, and all ten pass on this Mac. Each test boots from a root image in the `.minos/blobs` layout and gets a per-job input disk. The run also found and fixed a real defect: the helper's 1024 open-file limit made a 16 MiB output take 20.5 s. With 8192 it takes about 0.6 s.**

## What was built

### Task 1: the harness (`tests/harness/mod.rs`, commit 017fc11)

- **`vm()`**
  - Reads `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`. A missing or relative variable is an error carrying `SETUP`, never a skip.
  - Checks the guest and the sibling `minos-probe` with `check_guest_elf`.
  - Builds the root image:
    - An in-memory rootfs tar holds `etc/`, `root/` (0700), `usr/local/bin/`, and `usr/local/bin/minos-probe` (0755).
    - `pack_root_image(tar, guest, &PackLimits::default())` packs it.
    - The image is written to `<tmp>/proj/.minos/blobs/sha256/<2 hex>/<62 hex>` and wrapped as a canonical `VerifiedPath` (D-14 store-path branch).
  - Opens `JobRoot::open_at(<tmp>)` under a fresh `/private/tmp/mvt*` directory.
  - Builds `KrunBackend::with_job_root(HostPaths::new(..), MapBlobSource, root)`.
  - Refuses to go on if the probed capabilities show any host refusal, or lack Seatbelt or Rlimits.
- **`MapBlobSource`**
  - An `Arc<Mutex<BTreeMap<BlobId, Vec<u8>>>>` plus the image map.
  - A poisoned lock is an error, never an unwrap.
  - `get` refuses over the cap.
  - `root_image` returns the stored `VerifiedPath` or `RootImageMissing`.
- **`manifest(build, run, judge, memory_mib, scratch_mib, budgets_ms)`**
  - Builds the TOML text with writable `src/value.txt`, a gate on `probe_ok`, one objective, and one single-shot proposer.
  - Budgets must be whole seconds.
  - The judge argv is bare words (`minos-probe scorecard`).
- **`base_tar` and `diff_text`**
  - `base_tar` builds hand-made ustar headers (mtime, uid, and gid 0), sorted by path.
  - `diff_text` builds a one-hunk diff.
- **`candidate` and `judge`**
  - Each stores the blobs and builds the `JobRequest` (a baseline, or a candidate through `Diff::parse` and `Experiment::candidate`), then the `GuestJob` JSON, then the `InputDisk`.
  - Then `SandboxSpec::<R>::new(.., Machine::new(1, memory), Arch::Arm64)`, plus `.secrets(JudgeData)` for a judge.
  - Then `seal`, `preflight` against `backend.capabilities()`, `launch`, and `wait`.
  - Each prints the host-observed `Ended` and elapsed time. For any launch that did not end `Exited`, it also prints the sanitized console tail.
- **Readers**
  - `exited` requires `Ended::Exited`, then calls `into_output` and `take_status`.
  - `stage_end` and `metric` look up values.
  - `drop_logs` takes the three PID 1 logs.
  - `probe_json` goes through `take`, then `into_log_excerpt(4096)` (refused if truncated), then `serde_json`.

### Task 2: the tests (`tests/vm.rs`, commit 754ed85) and the descriptor fix (commit 15cabe4)

| Test | What it proves |
|---|---|
| `experiment_applies_diff_and_runs_as_candidate_uid` | Diff Applied; build and run Exited 0. uid, euid, gid, and egid are all 64000, there are no supplementary groups, and cwd is `/scratch/src`. Every `/proc/self/limits` row is at its expected value: address space 805306368 (768 MiB), file size 1073741824, processes 512, cpu 61 s, open files 4096, core 0. The run stage sees the applied file. |
| `apply_mismatch_is_rejected_and_runs_nothing` | `ApplyStatus::Failed { path: src/value.txt, hunk: 1 }`, both stages Skipped, no stage output, `Ended::Exited` |
| `writes_outside_scratch_fail` | Writes to `/`, `/etc`, and `/usr` fail with EROFS. `/root` fails with EACCES. `/scratch/judge` and `/scratch/candidate` fail with ENOENT. `/proc/1/environ` and `/dev/vdb` cannot be opened (ENOENT, from hidepid and the unlinked node). `/tmp` and `/out` succeed. |
| `over_budget_candidates_are_stopped_in_the_vm` | `alloc` 832 MiB got 764 MiB under a 768 MiB `RLIMIT_AS`. `spawn` 2000 started 511 before EAGAIN. `fill-scratch` at a 128 MiB cap wrote 134213632 bytes before ENOSPC. Every run ended Exited 0, `Ended::Exited`, with no InfraError. |
| `two_sequential_experiments_share_no_build_output` | Experiment A writes `/scratch/src/target/marker`, and its own run sees it. Experiment B, launched next on the same base, sees no marker. |
| `output_16_mib_is_reassembled` | `Packed { files: 2, content_bytes: 16000000 }`. The out files total 16000000 bytes. The launch takes 0.60 s, under the 15 s bound. |
| `output_over_cap_is_refused_by_the_guest` | 17000000 bytes gives `OutputStatus::TooLarge` and no `out/` entry |
| `judge_role_returns_a_scorecard` | Judge role, apply `NotApplicable`, judge Exited 0. `into_scorecard` returns one metric, `probe_ok` = 1. |
| `candidate_output_never_carries_a_scorecard` | A candidate's `/out/scorecard.json` stays `out/scorecard.json`, and there is no `.minos/scorecard.json`. The same VM has no `/scratch/judge` (ENOENT). |
| `stage_timeout_is_reported_by_the_guest` | With a 2 s run budget, `hang` gives run TimedOut and build Exited 0, with `Ended::Exited` from PID 1's deadline. The launch took 2.34 s. |

## Full suite output (phase-end gate)

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1`. It ran after building and signing `minos-vmm` (both entitlements listed) and building the musl guest, at HEAD 754ed85. Exit 0. `pgrep -fl minos-vmm` printed nothing afterwards. The two long `out/w.json` and `out/build.json` lines are shortened here with `...`; they appear in full in 05-EVIDENCE.md "05-17 VM suite".

```text
running 10 tests
test apply_mismatch_is_rejected_and_runs_nothing ... == candidate: ended Exited after 486.721917ms
ok
test candidate_output_never_carries_a_scorecard ... == candidate: ended Exited after 334.317375ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},...,{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
ok
test experiment_applies_diff_and_runs_as_candidate_uid ... == candidate: ended Exited after 338.918417ms
== out/build.json: {"cmdline":"... root=/dev/vda ...","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":[...,"Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
== out/run.json: {"exists":true,"path":"/scratch/src/src/value.txt"}
ok
test judge_role_returns_a_scorecard ... == judge: ended Exited after 329.305209ms
ok
test output_16_mib_is_reassembled ... == candidate: ended Exited after 602.41ms
== output status Packed { files: 2, content_bytes: 16000000 }
== 16 MiB output: 2 files, 16000000 content bytes, launch elapsed 602.41ms
ok
test output_over_cap_is_refused_by_the_guest ... == candidate: ended Exited after 353.996833ms
ok
test over_budget_candidates_are_stopped_in_the_vm ... == candidate: ended Exited after 468.704708ms
== out/a.json: {"allocated_mib":764,"error":"allocation refused","requested_mib":832}
== RLIMIT_AS allowance 768 MiB: the probe allocated 764 of 832 MiB
== candidate: ended Exited after 353.404041ms
== out/s.json: {"os_error":11,"requested":2000,"started":511}
== RLIMIT_NPROC 512: the probe started 511 of 2000 children
== candidate: ended Exited after 424.1565ms
== out/f.json: {"bytes_written":134213632,"os_error":28,"path":"/scratch/tmp/minos-probe-fill"}
== scratch 128 MiB: the probe wrote 134213632 bytes before a write failed
ok
test stage_timeout_is_reported_by_the_guest ... == candidate: ended Exited after 2.339398334s
== a 2 s run budget: the launch took 2.339398334s
ok
test two_sequential_experiments_share_no_build_output ... == candidate: ended Exited after 330.532417ms
== out/m.json: {"exists":true,"path":"/scratch/src/target/marker"}
== candidate: ended Exited after 340.470583ms
== out/m.json: {"exists":false,"path":"/scratch/src/target/marker"}
ok
test writes_outside_scratch_fail ... == candidate: ended Exited after 338.603583ms
== out/w.json: {... same shape as above ...}
ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.19s
```

**16 MiB output elapsed time (S03-ANSWER, production measurement):** 602.41 ms for the whole launch, from spawn to helper exit, with boot, build, run, pack, and 3909 chunk connections. An earlier run after the fix took 606.65 ms. Before the fix it took 20.528531416 s.

## Open risk restated: RLIMIT_AS against a real toolchain

The probes are small. `over_budget_candidates_are_stopped_in_the_vm` observed a 768 MiB allowance with the defaults (2048 MiB memory, 1024 MiB scratch, 256 MiB guest reserve): the probe got 764 MiB of an 832 MiB request. That shows the limit is enforced. It does not show that 768 MiB of address space is enough for a real build: Go, the JVM, V8, and sanitizers reserve large virtual ranges (05-RESEARCH Pitfall 10, Open Question 3). **Owner: Phase 8's sample project**, which measures one real build against the defaults. The risk is already in CLAUDE.md Open questions (05-01). The defaults can be tuned per manifest (D-06).

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: ok
- `env -u INSTA_UPDATE cargo test --workspace`: 63 result lines, 0 failed
- `cargo deny check`: advisories, bans, licenses, and sources ok
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: ok
- C-01: no MISSING line. C-02: prints nothing.
- Fast check (`--exact experiment_applies_diff_and_runs_as_candidate_uid`): 1 passed. Full suite: 10 passed, re-signed after `cargo test --workspace`.
- Acceptance greps:
  - `pub struct MapBlobSource`, `.minos/blobs/sha256`, and `minos-probe` are in the harness.
  - `grep -c '#\[ignore' tests/vm.rs` prints 10.
  - Both named test functions are present.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug, mechanism proven] The helper's RLIMIT_NOFILE stalled the chunked output**
- **Found during:** Task 2, the first full suite run. `output_16_mib_is_reassembled` passed, but the launch took 20.5 s, against about 0.59 s for the same stream in W0-CHUNKS.
- **Proof:** temporary instrumentation, reverted before any commit, showed:
  - The host receiver was idle 99% of the time. It spent 20.25 s waiting on the listener and 0.19 s reading.
  - The guest's connects took 20.07 s in total, but only 4 of 3909 took longer than 2 ms. Each of those 4 took about 5 s, made of about 75 reset-and-retry attempts (300 retries in all).
  - VM memory (2048, 1024, or 640 MiB) made no difference.
  - Pinned source `msb_krun_devices-0.1.41/src/virtio/vsock/reaper.rs` keeps every released connection's proxy, and its host socket, for `TIMEOUT = 5 s`. With 05-07's `NOFILE_LIMIT` of 1024, the helper ran out of descriptors every thousand or so connections.
  - Setting the limit to 8192 gave 0 retries and a 0.86 s launch. W0 had run before 05-07 added the limit.
- **Fix:** `NOFILE_LIMIT` is now 8192, documented with the mechanism. That is above one whole stream (about 4353 connections) in one reaper window, and it is still a bound. This is a Minos configuration change with no dependency fork or patch. Seatbelt, the token, and the chunk protocol are unchanged. CLAUDE.md's vsock bullet and the chunk-cost open question now carry the production numbers. The 16 MiB test now asserts the 15 s bound.
- **Files modified:** crates/minos-vmm/src/main.rs, CLAUDE.md, crates/minos-backend-krun/tests/vm.rs
- **Commit:** 15cabe4 (fix), 754ed85 (assertion)
- **Why it mattered:** the helper's deadline is the stage budgets plus 30 s. A candidate that used most of its run budget and wrote 12 MiB or more of output would have been killed at the deadline and recorded `TimedOut`.

**2. [Rule 2 - Test strength] The write-outside targets exist in the test image**
- The plan's image had only `usr/local/bin/minos-probe`, so `/etc` and `/root` writes would have failed only because the directories were missing.
- The test rootfs now also carries `etc/` (0755) and `root/` (0700). The probe then meets EROFS and EACCES, the real defenses.
- **Commit:** 017fc11

**3. [Rule 2 - Test strength] Positive controls**
- `two_sequential_experiments_share_no_build_output` also checks that experiment A's own run sees its marker, so B's "absent" result cannot pass because the marker was never written.
- `writes_outside_scratch_fail` asserts the exact read targets and the count of attempts.
- **Commit:** 754ed85

**4. [Plan wording] No Cargo.toml change**
- `serde_json` and `tempfile` are already normal dependencies of `minos-backend-krun`, and integration tests can use them, so the plan's conditional step did nothing.

**5. [Plan wording] `MapBlobSource::put` returns `TestResult<BlobId>`**
- A poisoned lock must be an error, so `put` returns a `Result` rather than a bare `BlobId`. `base_tar` likewise returns a `Result`, because a ustar header can refuse a path.

### Notes
- **The first deny-default boot under 05-09's write-only console rule worked.** The rule is `(allow file-write* (literal "/dev/fd/1"))`. There was no exit 66 and no missing console (05-EVIDENCE.md "05-17 first production boot").
- **What "the run check sees the changed file" proves.** `marker-check` reports only that the file exists. That the file holds the new content follows from `ApplyStatus::Applied`, because the strict applier writes the hunk's new side only after every context line matched. The probe has no read-content command, and none was added.
- **Shared harness and dead code.** `harness/mod.rs` is included by each test target that declares `mod harness;`. When 05-19's `exit.rs` includes it, every harness item must be used there too, or `-D warnings` fails on dead code (an allow attribute is not an option).

## Requirements

- **Marked complete:** GUEST-01, GUEST-02, GUEST-03, and KRUN-10. Each is verified in real VMs by this plan's tests, and no later plan lists them.
- **Already complete:** KRUN-04.
- **Left pending:** GUEST-04. 05-19 also lists it and owns the forged-token probe that finishes it.

## Known Stubs

None.

## Threat Flags

None. The harness adds no production surface. The descriptor limit change keeps a bound on the helper's descriptors (T-05-07-03 is still mitigated). A candidate that floods the output port is still stopped by the token and the receiver's first-fault close.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/tests/harness/mod.rs, crates/minos-backend-krun/tests/vm.rs, crates/minos-vmm/src/main.rs
- FOUND commits: 017fc11, 15cabe4, 754ed85
