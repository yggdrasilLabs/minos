---
phase: 02-platform-spikes
plan: 06
subsystem: infra
tags: [rust, libkrun, msb_krun, libkrunfw, hypervisor-framework, codesign, squashfs, boot, spike]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: minos-guest (report_boot, scratch_roundtrip, run, MINOSST1 frame); 02-03: minos-vmm launcher, E-4 kernel arguments, entitlements plist; 02-04: digest-pinned Dockerfile to SquashFS producer; 02-05: supervise::run, GuestBytes, job-directory helpers"
provides:
  - "Ignored VM spikes: unsigned_helper_is_refused, boot_root_disk, boot_initramfs_attempt (spike_boot.rs) and boot_from_dockerfile (spike_rootimg.rs)"
  - "Proof that msb_krun boots stock libkrunfw from a raw read-only SquashFS root on /dev/vda with /init = minos-guest, no initramfs and no virtio-fs (SPIKE-01 boot half, D-18)"
  - "SPIKE-04 answered: linker-signed helper refused with exit 70; ad-hoc signed helper with the hardened runtime and two entitlement keys boots"
  - "SPIKE-06 answered: the Docker-produced image boots and /bin/cat from the image returns minos-spike-06"
  - "SPIKE-08 not triggered; 02-07, 02-08, 02-09 may start"
  - "The exact root-disk /proc/cmdline, asserted token by token (EXPECTED_CMDLINE)"
affects: [02-07, 02-08, 02-09, 02-10, 02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "Every VM test checks has_hypervisor_entitlement first and fails with the E-1/E-4 setup text; it never skips"
    - "A rebuild of minos-vmm drops its signature; re-sign with the E-4 argv after every build before any VM test"
    - "No-network checks are made two ways: the guest interface list is exactly lo, and the virtio device list has no 0x0001 (net) and no 0x001a (fs)"

key-files:
  created:
    - crates/minos-backend-krun/tests/spike_boot.rs
    - .planning/phases/02-platform-spikes/evidence/02-06.md
  modified:
    - crates/minos-backend-krun/tests/spike_rootimg.rs
    - crates/minos-vmm/minos-vmm.entitlements
    - crates/minos-vmm/src/launch.rs

key-decisions:
  - "K5: the hardened runtime refuses Homebrew libkrunfw (different Team IDs), so minos-vmm.entitlements carries com.apple.security.cs.disable-library-validation beside com.apple.security.hypervisor; minos-vmm's preflight SHA-256 check of libkrunfw stands in for the dropped validation"
  - "User decision (option 2, 2026-10-10): minos-vmm appends dummy.numdummies=0 to its root-disk kernel arguments so the built-in dummy driver creates no dummy0, and boot_root_disk also asserts no virtio-net (0x0001) device; the E-2 net_ifaces assertion (empty or exactly lo) stays as written"
  - "The guest kernel is 6.12.109 (osrelease), not 6.12.91: the libkrunfw v5.6.2 Makefile is right and its committed config header is stale"
  - "SPIKE-01 is not marked complete: K7 is folded into SPIKE-01 (D-05) and runs in 02-07, and 02-10 rewords SPIKE-01 with its checkbox untouched; SPIKE-08 is closed by 02-11"

patterns-established:
  - "Root-disk boot: KernelBuilder::cmdline(ROOT_DISK_CMDLINE).init_path(\"/init root=/dev/vda\") so the last root= is /dev/vda and the last rootfstype= is squashfs"

requirements-completed: [SPIKE-04, SPIKE-06]

duration: 24min
completed: 2026-10-10
---

# Phase 2 Plan 06: Signing, root-disk boot, Docker-image boot, SPIKE-08

**msb_krun, called only from `forbid(unsafe_code)` crates, boots stock Homebrew libkrunfw (kernel 6.12.109) from a raw read-only SquashFS root on `/dev/vda` whose `/init` is the static musl guest. There is no initramfs and no virtio-fs. The guest prints its marker, sends a v1 frame, and powers off with exit 0. The command line is asserted token by token, the guest has only `lo`, and the virtio list has no net or fs device. An unentitled helper is refused with exit 70. The Docker-produced image also boots and runs a command from the image. SPIKE-08 was not triggered.**

## Performance

- **Duration:** about 24 min (two sessions with a user decision between them)
- **Started:** 2026-10-10T13:47:53Z
- **Completed:** 2026-10-10T14:11:14Z
- **Tasks:** 3 of 3 (Task 3 completed without a question because SPIKE-08 was not triggered)
- **Files modified:** 6

## Accomplishments

- **SPIKE-04:** the linker-signed copy (`flags=0x20002(adhoc,linker-signed)`, no entitlements) is refused before the VM starts. The helper exits 70 with `minos-vmm: error: starting the VM: build error: start: build_microvm: Internal(Vm(VmSetup(VmCreate)))`. That text comes from msb_krun_hvf, which discards the `hv_return_t`, so the evidence shows that `hv_vm_create` failed but not the code it returned. The same bytes, signed ad hoc with `--options runtime` and the entitlements plist, boot.
- **K5:** the hypervisor entitlement alone was not enough under the hardened runtime. The `dlopen` of `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib` failed with "mapping process and mapped file (non-platform) have different Team IDs". E-5 was applied, and the final set is `com.apple.security.hypervisor` plus `com.apple.security.cs.disable-library-validation` (`grep -c '<key>'` prints 2).
- **SPIKE-01, boot half (D-18):** `boot_root_disk` passes every E-2 assertion: exit 0, not killed, all three markers, one connection with a v1 frame, the exact command line, the last `root=` is `/dev/vda`, the last `rootfstype=` is `squashfs`, `init=/init` is present, no `MSB_` or `DYLD_` env key, no `/Users/` value, `net_ifaces == ["lo"]`, no `0x0001` or `0x001a` device, and `scratch_roundtrip.ok`.
- **S01-INITRAMFS (negative evidence):** as predicted, the kernel never ran `/init`. It panicked with `VFS: Unable to mount root fs on "/dev/root"`, beside the H-02 config line `# CONFIG_BLK_DEV_INITRD is not set`. The helper still exited 0 (`panic=-1`, Pitfall 1).
- **SPIKE-06 (D-15):** the image built from the digest-pinned Dockerfile (4,456,448 bytes, SHA-256 `d1e93ef0...a0a4ae`, identical to 02-04 RI-03) booted as the root disk. `/bin/cat /etc/minos-spike` returned status 0 and stdout `minos-spike-06\n`. It was rerun after the option-2 change with the same result.

## Results the plan asked for

**K5 entitlement set:** `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`. The second key exists only because the hardened runtime refuses a Homebrew dylib signed under a different Team ID.

**Kernel version:** `osrelease` is `6.12.109`. CLAUDE.md and `.claude/CLAUDE.md` still say 6.12.91.

**Final `EXPECTED_CMDLINE`** (normalized `/proc/cmdline`, root-disk mode):

```text
reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda KRUN_BOOT_START_NS=<ns> --
```

This is E-3 plus one token that Minos adds, `dummy.numdummies=0`, placed between `ignore_loglevel` and `root=/dev/root`. Every other token matched the prediction from the pinned msb_krun source on the first boot.

**Virtio device list (guest `/sys/bus/virtio/devices`):** `virtio0:0x0004` (rng, the default), `virtio1:0x0003` (console), `virtio2:0x0002`, `virtio3:0x0002`, `virtio4:0x0002` (block: vda root, vdb input, vdc scratch), `virtio5:0x0013` (vsock), and `virtio6:0x4d47` (msb_krun vmgenid). There is no `0x0001` (net) and no `0x001a` (fs).

**SPIKE-08:** not triggered (`### S08: SPIKE-08 not triggered` in evidence/02-06.md).

## Task Commits

1. **Task 1: Boot spike tests**: `0c79791` (test)
2. **Task 2: Run SPIKE-04, K5, initramfs attempt, root-disk boot, cmdline, Dockerfile boot**:
   - `7dd8120` (fix): disable-library-validation added to the minos-vmm entitlements, forced by K5
   - `048d00b` (docs): evidence for every step
   - `4863655` (fix): `dummy.numdummies=0` in the root-disk kernel arguments (user decision, option 2)
   - `727e952` (test): `EXPECTED_CMDLINE` token and the no-virtio-net assertion
   - `41f4d47` (docs): the decision and the passing reruns recorded under S01-NETIFACES
3. **Task 3: SPIKE-08 stop-and-ask**: not presented. S08 was not triggered, so the task completed without a commit, as the plan's trigger rule requires.

## Decisions Made

- **The dummy0 interface (S01-NETIFACES).** The first passing boot reported `net_ifaces: ["dummy0", "lo"]`. libkrunfw builds the dummy driver in (`CONFIG_DUMMY=y`), and the driver creates one interface by default. No VMM device was attached. Three options went to the user: option-1, disable it in the launcher; option-2, option-1 plus a direct no-virtio-net assertion; option-3, accept `dummy0` by name. The user chose **"option 2"**. The reasoning: a dead sink interface is harmless but cheap to remove, and the direct virtio check closes the gap left by using the interface list as a proxy for "no network device".
- **K5:** the second entitlement key, recorded with its forcing error.

## For 02-10 (carry into CLAUDE.md and `.claude/CLAUDE.md`)

- The 02-03 fixed kernel argument list (`ROOT_DISK_CMDLINE`) and the E-3 command line each gained one Minos-originated token by user decision: `dummy.numdummies=0`. The root-disk arguments are now `rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0`, and `INITRAMFS_CMDLINE` is unchanged.
- The kernel osrelease is **6.12.109**, not 6.12.91.
- `minos-vmm.entitlements` has a **second key**, `com.apple.security.cs.disable-library-validation`. The reason: the hardened runtime refuses the Homebrew libkrunfw dylib because its signature has a different Team ID (K5). minos-vmm's preflight SHA-256 check of libkrunfw is the integrity check that remains.

## Deviations from Plan

### Changes to another plan's files

**1. [K5, plan step 4] `crates/minos-vmm/minos-vmm.entitlements` (02-03's file) replaced with E-5**
- **Found during:** Task 2, S01-ROOTDISK run 1
- **Issue:** the hardened runtime refused the libkrunfw `dlopen` (different Team IDs)
- **Fix:** E-5 plist, re-signed, re-run
- **Commit:** 7dd8120

**2. [User decision, Rule 4] `crates/minos-vmm/src/launch.rs` (02-03's file): `ROOT_DISK_CMDLINE` gains `dummy.numdummies=0`**
- **Found during:** Task 2, S01-ROOTDISK run 2 (the E-2 `net_ifaces` assertion failed on the kernel's `dummy0`)
- **Issue:** E-2 required `net_ifaces` to be empty or exactly `lo`. The stock kernel creates `dummy0`. Changing either E-2 or the 02-03 E-4 arguments would depart from locked text, so the executor stopped and asked.
- **Fix:** option 2 as chosen. The launcher token, the matching `EXPECTED_CMDLINE` token, and a new assertion that no device is `0x0001`. The E-2 assertion was not relaxed.
- **Files modified:** `crates/minos-vmm/src/launch.rs`, `crates/minos-backend-krun/tests/spike_boot.rs`
- **Commits:** 4863655, 727e952

No other deviations. No Cargo.toml, Cargo.lock, or deny.toml change; no new dependency; no unsafe code; no allow attribute; no initramfs or virtio-fs on the decided path; no other VMM, krunkit, or `-sys` crate.

## Issues Encountered

- The unsigned refusal text does not name Hypervisor.framework, because msb_krun_hvf maps any non-success `hv_vm_create` to `VmCreate`. The only difference from the binary that boots is the signature, which ties the refusal to the missing entitlement. This is recorded plainly in S04-UNSIGNED.

## Verification

- Gate (CLAUDE.md Commands): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check` (advisories, bans, licenses, sources ok), `cargo doc --workspace --no-deps`, and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`. All exit 0.
- C-01: ten `ok` lines. C-02 `#!?\[allow\(` grep: prints nothing. Unsafe grep: prints nothing.
- VM rerun with E-1's variables, a fresh unsigned copy, and a re-signed helper: `cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1` passes 3 of 3. `boot_from_dockerfile` passes.
- `cargo clean` was not run (02-11 does that, GATE-06). Note that `cargo test --workspace` may relink `target/debug/minos-vmm`, so re-sign it with E-4 before the next VM run.

## Next Phase Readiness

- 02-07 (K7 exit map), 02-08 (egress and vsock), and 02-09 (Seatbelt) can start: the D-18 boot works.
- 02-10's precondition holds: evidence/02-06.md contains `### S08: SPIKE-08 not triggered`.
