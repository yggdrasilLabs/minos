---
phase: 05-guest-krun-backend-and-environment-build
plan: 19
subsystem: minos-backend-krun
tags: [exit-tests, krun, guest, egress, env, cmdline, kill9, watchdog, sweep, forge, chunk-token, krun-03, guest-05, krun-05, guest-04]
status: complete
requires:
  - "05-17: tests/harness/mod.rs (vm, candidate, judge, exited, probe_json, ...), the functional VM tests, and the helper RLIMIT_NOFILE fix"
  - "05-15: KrunBackend::launch, KrunRunning::{helper_pid, job_dir}, Ended::ChunkProtocol"
  - "05-11: JobRoot::open_at, the sweep, SweepEntry"
  - "05-12: minos-probe egress, env, write-outside, forge, hang"
provides:
  - "crates/minos-backend-krun/tests/exit.rs: exit_egress, exit_env, exit_kill9, exit_forge (all ignored, 4 passed on this Mac)"
  - "harness::start_candidate: a candidate launch that returns the running VM without waiting"
  - "CLAUDE.md: the exact guest /proc/cmdline and the one KRUN_BOOT_START_NS recorded exception (user decision 2026-10-19), the exit tests in Testing, MINOS_EXIT_CANARY / MINOS_EXIT_HOST_IP / MINOS_EXIT_KILL9_CHILD in Commands, and the token-removal open question"
affects:
  - "05-20: reruns the exit suite in the final gate, records it in 05-EVIDENCE.md, and records GUEST-05's one stated exception in REQUIREMENTS.md"
  - "The phase that next bumps the msb_krun pin: owns removing KRUN_BOOT_START_NS (vendor the single msb_krun crate under [patch.crates-io]) and must re-run exit_env"
tech-stack:
  added: []
  patterns:
    - "A kill -9 test re-executes its own test binary as the victim (an env var selects the launcher half), kills only that child by its own handle, and reads the helper pid and job dir from one report line"
    - "Every harness item is used by every test target that declares mod harness; a non-waiting launch is factored so the waiting one calls it, keeping both targets free of dead code"
key-files:
  created:
    - crates/minos-backend-krun/tests/exit.rs
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-19-SUMMARY.md
  modified:
    - crates/minos-backend-krun/tests/harness/mod.rs
    - CLAUDE.md
decisions:
  - "Task 1 (2026-10-19), first answer: no-allowance. After the orchestrator proved the mechanism from the pinned source (msb_krun 0.1.41 build_kernel_cmdline writes KRUN_BOOT_START_NS unconditionally, no opt-out, no newer release), the user declined a fork and chose, on 2026-10-19, to allow KRUN_BOOT_START_NS=<ASCII digits> as a recorded exception; GUEST-05 is met with that one stated exception"
  - "The removal path the user would accept later: vendor only the top-level msb_krun crate (13 files, about 7,100 lines, Apache-2.0, no build script) under [patch.crates-io] as a path dependency with a one-line change; no git fork of the whole project"
  - "exit_forge adds a judge-role control: the same receiver accepts PID 1's genuine stream and scorecard, so the ChunkProtocol on the forged stream is not a receiver that rejects everything"
  - "exit_kill9 asserts the sweep entry is Removed (the helper was already gone, so the watchdog stopped it, not the sweep), not merely that the directory is absent"
metrics:
  duration: "about 25 minutes (Tasks 2 and 3, after the checkpoint)"
  completed: 2026-10-19
  tasks: 3
  files: 3
---

# Phase 5 Plan 19: the permanent exit tests Summary

The phase's exit criterion passes on this Mac against the production backend and guest. `exit_egress`, `exit_env`, `exit_kill9`, and `exit_forge` passed together: 4 passed, exit 0. Guest egress fails closed. The stage environment is exactly four guest values. `/proc/cmdline` matches token for token, with the one recorded `KRUN_BOOT_START_NS=<digits>` exception. After SIGKILL of the launching process, its helper exited on its own in about 280 ms, and the sweep removed the job directory. A guessed-token stream ends `ChunkProtocol`, and the candidate cannot open `/dev/vdb`.

## The Task 1 decision (checkpoint:decision, 2026-10-19)

Question: may `exit_env` allow exactly one host-derived token in the guest's `/proc/cmdline`, `KRUN_BOOT_START_NS` followed by ASCII digits only, and still count GUEST-05 as met?

1. **First answer, 2026-10-19: no-allowance.**
2. **Root cause.** The orchestrator then proved the mechanism from the pinned source. In msb_krun 0.1.41, `src/api/vm.rs` `build_kernel_cmdline` (lines 919 to 946) always writes `KRUN_BOOT_START_NS={boot_start_ns}` into the `krun_env` segment of `KernelCmdlineConfig`. `boot_start_ns` comes from `SystemTime::now()` at line 549.
   - No builder method, cargo feature, or launch option removes it.
   - The `MSB_KRUN_KERNEL_CMDLINE` environment variable only appends to the prolog, and the helper's environment is cleared anyway.
   - Nothing under `crates/` references the token.
   - 0.1.41 (released 2026-10-08) is the newest release on crates.io, so no pin bump removes it.
   - The crate's repository is https://github.com/zerocore-ai/libkrun.
3. **Final answer, 2026-10-19: "Allow it as a recorded exception".** With the root cause proven, the user declined a fork and chose this.
   - `exit_env` allows exactly one token: `KRUN_BOOT_START_NS=` followed by ASCII digits only.
   - GUEST-05 is met with that one stated exception.
   - CLAUDE.md records it as a deferred open question, not an endorsement. It names the removal path the user would accept later: vendor only the top-level `msb_krun` crate (13 files, about 7,100 lines, Apache-2.0, no build script) under `[patch.crates-io]` as a path dependency, with a one-line change that drops the token. The `msb_krun_*` sub-crates stay on crates.io.
   - The user explicitly does not want a git fork of the whole project.

## Tasks

### Task 2: the permanent exit tests (`tests/exit.rs`, commit f791e2b)

- **`exit_egress` (KRUN-03).**
  - Host canaries listen on 127.0.0.1:0 and [::1]:0, nonblocking. A LAN canary is added when `MINOS_EXIT_HOST_IP` is set; it was unset in this run, and the test says so.
  - The candidate's run stage is `minos-probe egress`. Every target failed to connect: host-port loopback v4 and v6, 1.1.1.1:443, 8.8.8.8:53, and [2606:4700:4700::1111]:443. IPv4 failed with errno 101 and IPv6 with 99.
  - Both canaries' accept returned WouldBlock. The interfaces are exactly `["lo"]`.
  - No virtio device id is 0x0001 (net) or 0x001a (fs). The six devices are 0x0004 rng, 0x0003 console, 0x0002 block twice, 0x0013 vsock, and 0x4d47. The last is msb_krun's own VM-generation-ID device (`msb_krun_devices` 0.1.41 `virtio/vmgenid`, `VIRTIO_ID_MSB_VMGENID`). It is neither net nor fs.
- **`exit_env` (GUEST-05).**
  - It requires `MINOS_EXIT_CANARY`, at least 16 characters. A missing canary is an error that names `$(uuidgen)`.
  - The build stage runs `minos-probe env` on a real experiment (a parsed diff applied).
  - `vars` is exactly `{PATH, HOME=/scratch/home, TMPDIR=/tmp, LANG=C}`, with PATH as the guest's.
  - `/proc/cmdline`, split on whitespace, equals the 18-token list. Exactly one token matches `KRUN_BOOT_START_NS=` plus digits, and it is the only `KRUN_` token.
  - The canary, the host HOME, and the host USER appear nowhere in the serialized `env.json`.
  - The allowance is documented in the test with the date of the user's answer.
- **`exit_kill9` (KRUN-05).**
  - The test re-executes its own binary with `MINOS_EXIT_KILL9_CHILD=1`. That child is the launcher: it starts a candidate whose run is `hang` under a 600 s run budget, prints `HELPER_PID=<pid> JOB_DIR=<dir>`, and sleeps.
  - The parent reads that line within 60 s, then sends SIGKILL to only that child, through its own `Child` handle. A drop guard kills and reaps that same handle if the test fails early. Nothing else on the host is signalled.
  - The parent polls `/bin/ps -o pid= -p <pid>` every 50 ms. The helper was gone after 278 ms in this run and 280 ms in the first, against a 5 s bound.
  - `/bin/ps -A -ww -o command=` shows no line naming the job directory.
  - A new `KrunBackend::with_job_root` over the same job root sweeps it. Its report is `[Removed { dir: <job dir> }]`, and the directory no longer exists.
  - The killed child's own `/private/tmp/mvt*` directory is removed afterwards. The path is first checked to be a direct `mvt*` child of `/private/tmp`.
- **`exit_forge` (GUEST-04).**
  - Run 1, `write-outside`: `/dev/vdb` and `/proc/1/environ` both fail to open, with errno 2.
  - Run 2, `forge`: the host saw `Ended::ChunkProtocol`.
  - Control: a judge launch on the same backend is accepted, and its scorecard carries `probe_ok` = 1.
- **Harness change.** `harness::start_candidate` returns the `KrunRunning<Candidate>` without waiting, and `harness::candidate` is now `start_candidate` plus `wait`. See Deviations.

### Task 3: CLAUDE.md (commit 71a905b)

- **Local backends, boot bullet.** It gives the exact `/proc/cmdline` token list and the one host-derived `KRUN_BOOT_START_NS=<host wall-clock ns>`, with the proven mechanism. It also notes that the kernel hands the token to PID 1's environment, which PID 1 clears before any stage. `exit_env` allows exactly that key with an all-digit value. This is recorded as a recorded exception by the user's decision at the 05-19 checkpoint on 2026-10-19, with the decision history, not as an assumption.
- **Testing.** A new bullet covers the four exit tests and `tests/vm.rs`, the shared harness, and the dead-code rule.
- **Commands.** `MINOS_EXIT_CANARY` (for example `$(uuidgen)`), the optional `MINOS_EXIT_HOST_IP`, and `MINOS_EXIT_KILL9_CHILD`, which the test sets for its launcher only.
- **Open questions.** Removing `KRUN_BOOT_START_NS`: the proven mechanism (no opt-out, no newer release) and the accepted removal path (vendor the single `msb_krun` crate under `[patch.crates-io]` with a one-line change; no fork of the whole project). Owner: the phase that next bumps the `msb_krun` pin, or earlier at the user's call.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | none | checkpoint:decision, answered by the user on 2026-10-19 (no files) |
| 2 | f791e2b | test(05-19): the permanent exit tests on the production backend and guest |
| 3 | 71a905b | docs(05-19): CLAUDE.md records the boot-time exception and the exit tests |

## Verification

Each gate command was run once, after both task commits:

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace`: exit 0; 64 result lines, 1139 passed, 0 failed, 19 ignored.
- `cargo deny check`: exit 0; `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo doc --workspace --no-deps`: exit 0.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0.
- C-01: eleven `ok crates/...` lines and no MISSING line. C-02 printed nothing (grep exit 1).
- Re-signed after `cargo test --workspace`. `codesign --display --entitlements -` lists `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`.
- Fast check, `--exact exit_egress`: 1 passed.
- Functional suite, re-run because the harness changed (`--test vm`): 10 passed, exit 0, in 15.54 s.
- No `/private/tmp/mvt*` directory was left after either exit run.

### The exit suite (phase-end gate), verbatim, exit 0

Command: `MINOS_EXIT_CANARY=$(uuidgen) MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test exit -- --ignored --nocapture --test-threads=1`

The console tail below is printed for the `ChunkProtocol` launch only, through `Finished::console_excerpt`.

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/exit.rs (target/debug/deps/exit-c99de225879ade1b)

running 4 tests
test exit_egress ... == LAN target: not run (MINOS_EXIT_HOST_IP is unset)
== candidate: ended Exited after 509.173208ms
== out/egress.json: {"attempts":[{"ok":false,"os_error":101,"target":"127.0.0.1:59382"},{"ok":false,"os_error":99,"target":"[::1]:59383"},{"ok":false,"os_error":101,"target":"1.1.1.1:443"},{"ok":false,"os_error":101,"target":"8.8.8.8:53"},{"ok":false,"os_error":99,"target":"[2606:4700:4700::1111]:443"}],"net_ifaces":["lo"],"virtio_devices":[{"device":"0x0004","name":"virtio0"},{"device":"0x0003","name":"virtio1"},{"device":"0x0002","name":"virtio2"},{"device":"0x0002","name":"virtio3"},{"device":"0x0013","name":"virtio4"},{"device":"0x4d47","name":"virtio5"}]}
== connect 127.0.0.1:59382: connected false, errno Some(101)
== connect [::1]:59383: connected false, errno Some(99)
== connect 1.1.1.1:443: connected false, errno Some(101)
== connect 8.8.8.8:53: connected false, errno Some(101)
== connect [2606:4700:4700::1111]:443: connected false, errno Some(99)
== 127.0.0.1 canary: no connection arrived
== [::1] canary: no connection arrived
== interfaces: ["lo"]
== virtio virtio0: 0x0004
== virtio virtio1: 0x0003
== virtio virtio2: 0x0002
== virtio virtio3: 0x0002
== virtio virtio4: 0x0013
== virtio virtio5: 0x4d47
ok
test exit_env ... == candidate: ended Exited after 336.744417ms
== out/env.json: {"cmdline":"reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792444887439886000  --","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":["Max cpu time              61                   61                   seconds   ","Max file size             1073741824           1073741824           bytes     ","Max core file size        0                    0                    bytes     ","Max processes             512                  512                  processes ","Max open files            4096                 4096                 files     ","Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
== the canary appears nowhere in env.json
== the host HOME appears nowhere in env.json
== the host USER appears nowhere in env.json
ok
test exit_forge ... == candidate: ended Exited after 337.26025ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
== open /proc/1/environ: opened false, errno Some(2)
== open /dev/vdb: opened false, errno Some(2)
== candidate: ended ChunkProtocol after 336.155375ms
== candidate: console tail
r interface
[    0.020621] rtc-pl031 a001000.rtc: registered as rtc0
[    0.020669] rtc-pl031 a001000.rtc: setting system clock to 2026-10-19T21:21:29 UTC (1792444889)
[    0.020752] i2c_dev: i2c /dev entries driver
[    0.020819] pps_ldisc: PPS line discipline registered
[    0.021044] NET: Registered PF_INET6 protocol family
[    0.021153] Segment Routing with IPv6
[    0.021194] In-situ OAM (IOAM) with IPv6
[    0.021244] NET: Registered PF_PACKET protocol family
[    0.021297] Bridge firewalling registered
[    0.021336] can: controller area network core
[    0.021388] NET: Registered PF_CAN protocol family
[    0.021436] can: raw protocol
[    0.021476] can: broadcast manager protocol
[    0.021515] can: netlink gateway - max_hops=1
[    0.021579] NET: Registered PF_KCM protocol family
[    0.021648] NET: Registered PF_VSOCK protocol family
[    0.021926] NET: Registered PF_TSI protocol family
[    0.021967] NET: Registered PF_TSI6 protocol family
[    0.022011] NET: Registered PF_TSIU protocol family
[    0.022297] registered taskstats version 1
[    0.022865] Btrfs loaded, zoned=yes, fsverity=no
[    0.022977] Key type encrypted registered
[    0.032689] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.032781] alg: No test for 842 (842-scomp)
[    0.032839] alg: No test for 842 (842-generic)
[    0.036850] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.038277] clk: Disabling unused clocks
[    0.038317] PM: genpd: Disabling unused power domains
[    0.038364] ALSA device list:
[    0.038404]   No soundcards found.
[    0.048801] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049015] devtmpfs: mounted
[    0.049128] Freeing unused kernel memory: 2496K
[    0.049183] Run /init as init process
[    0.049221]   with arguments:
[    0.049260]     /init
[    0.049293]   with environment:
[    0.049328]     HOME=/
[    0.049361]     TERM=linux
[    0.049394]     KRUN_BOOT_START_NS=1792444888945435000
MINOS-GUEST-BOOT-OK

== judge: ended Exited after 325.006416ms
ok
test exit_kill9 ... launcher: 
launcher: running 1 test
launcher: test exit_kill9 ... 
launcher: HELPER_PID=53674 JOB_DIR=/private/tmp/mvt5FcjEm/minos-501/mns0p3rXQnd
== launcher 53665 killed: signal: 9 (SIGKILL)
== the helper 53674 exited 278.406833ms after the launcher's SIGKILL
== sweep: [Removed { dir: "/private/tmp/mvt5FcjEm/minos-501/mns0p3rXQnd" }]
== removed /private/tmp/mvt5FcjEm
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.71s
```

Exit code: 0. The helper exited 278.406833 ms after the launcher's SIGKILL; the earlier run of the same suite gave 280.178875 ms.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] A non-waiting candidate launch in the harness**
- **Found during:** Task 2 (`exit_kill9`)
- **Issue:** `exit_kill9`'s launcher must print the helper pid and job directory while the VM runs. The harness's only candidate launch waited for the VM to end, and the backend and blob source are private to the harness.
- **Fix:** Added `harness::start_candidate`, which returns `KrunRunning<Candidate>`. `harness::candidate` now calls it and then waits, so both test targets use it and neither has dead code. Nothing in the launch path changed: the same spec, seal, preflight, and `KrunBackend::launch` run as before. `tests/vm.rs` was re-run: 10 passed.
- **Files modified:** crates/minos-backend-krun/tests/harness/mod.rs
- **Commit:** f791e2b

**2. [Rule 2 - Correctness] A positive control in `exit_forge`, and every harness item used**
- **Found during:** Task 2
- **Issue:** On its own, a `ChunkProtocol` end does not show that the receiver accepts a genuine stream. Separately, `exit.rs` had to use every harness item (`judge`, `metric`, `drop_logs`, `diff_text`, `stage_end`), or `-D warnings` fails on dead code; an allow attribute is not an option.
- **Fix:** `exit_forge` runs a judge launch as a control, asserting its scorecard carries `probe_ok` = 1. `exit_env` runs on a real experiment (`diff_text`). `exit_egress` drops the stage logs before reading the probe JSON.
- **Files modified:** crates/minos-backend-krun/tests/exit.rs
- **Commit:** f791e2b

**3. [Rule 2 - Correctness] `exit_kill9` checks how the sweep handled the job**
- **Issue:** The plan only required the job directory to be gone, which a sweep that killed a still-running helper would also satisfy.
- **Fix:** The test asserts the sweep entry is `SweepEntry::Removed`, meaning the helper was already gone and the watchdog stopped it, not the sweep. The `ps` command check uses `-ww`, so a long command line cannot be truncated past the job path.
- **Commit:** f791e2b

No security invariant was relaxed beyond the one exception the user recorded.

## Observations

- On the guest command line, several spaces separate `root=/dev/vda` from `KRUN_BOOT_START_NS=` (msb_krun's segment join). `exit_env` splits on whitespace, so the gap does not matter.
- The 0x4d47 virtio device is msb_krun's VM-generation-ID device. It moves data host to guest and is not a network or filesystem device. 02-SPIKE-EVIDENCE.md does not list it by id, so 05-20 may want to name it in 05-EVIDENCE.md.
- `/proc/1/environ` fails with ENOENT for the candidate, not EACCES, the same as in 05-17's `writes_outside_scratch_fail`.

## Known Stubs

None.

## Threat Flags

None. The tests add no production surface. `MINOS_EXIT_*` variables are read only by ignored tests.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/tests/exit.rs, crates/minos-backend-krun/tests/harness/mod.rs, CLAUDE.md
- FOUND: commits f791e2b, 71a905b
