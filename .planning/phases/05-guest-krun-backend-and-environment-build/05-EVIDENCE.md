# Phase 5 evidence

Wave 0 VM proofs for plan 05-03 (05-RESEARCH Pattern 1). Each mechanism below had no Phase 2 evidence; each was
run on this Mac under the real deny-default Seatbelt profile (`crates/minos-vmm/minos-vmm.sb`) before any
production code depends on it. Outputs are verbatim test output; the only edit is the home directory written as `~`.
Guest-derived text was escaped by the tests themselves (control characters as `\u{..}`, so the console's carriage
returns appear as `\u{d}`).

- Date: 2026-10-18
- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `sw_vers -productVersion`: 27.0.1 (Phase 2 ran 27.0)
- HEAD when the runs were made: 02042a7 (test(05-03): ignored Wave 0 VM proofs for chunks, console pipe, store root)
- libkrunfw: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib (5.6.2, `libkrun/krun`)
- Environment for every VM run: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`

## Build and sign

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

Command: `cargo build -p minos-vmm`

Exit code: 0

Command: `cargo test -p minos-backend-krun --no-run` (built before signing, so the runs below never relink the helper)

Exit code: 0

Command: `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`

Exit code: 0

Command: `codesign --display --entitlements - target/debug/minos-vmm`

Exit code: 0

```text
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

## W0-ROOT

`store_root_boots_under_seatbelt` (05-CONTEXT D-14): the root image is written to its content-addressed store path,
`<job>/proj/.minos/blobs/sha256/<2 hex>/<62 hex>` with 0700 directories, and passed as `disks.root` and as the
`ROOT_IMG` profile parameter. With the profile's ROOT_IMG `path-ancestors` metadata rule the helper exits 0, the guest
boots (`MINOS-GUEST-BOOT-OK`), and one connection carries a valid status frame. This invocation ran both store-root
tests; the second test's output is the first W0-ROOT-DENY run.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test wave0_store_root -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/wave0_store_root.rs (target/debug/deps/wave0_store_root-26f74b11f22f1340)

running 2 tests
test store_root_boots_under_seatbelt ... == store_root_boots_under_seatbelt: root image at /private/tmp/mnsyV8FkAbQ/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
== store_root_boots_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsyV8FkAbQ/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsyV8FkAbQ/console.log
  -D
  INPUT_IMG=/private/tmp/mnsyV8FkAbQ/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsyV8FkAbQ/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
  -D
  SCRATCH_IMG=/private/tmp/mnsyV8FkAbQ/scratch.img
  -D
  SOCK=/private/tmp/mnsyV8FkAbQ/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsyV8FkAbQ/vmm.json
== store_root_boots_under_seatbelt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 532.19825ms, connections 1
-- console (3595 bytes, overflowed false):
[    0.018710] loop: module loaded
[    0.018819] virtio_blk virtio2: 1/0/0 default/read/poll queues
[    0.018946] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)
[    0.019381] virtio_blk virtio3: 1/0/0 default/read/poll queues
[    0.019519] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)
[    0.019739] Dev vdb: unable to read RDB block 1
[    0.019797]  vdb: unable to read partition table
[    0.019847] vdb: partition table beyond EOD, truncated
[    0.019927] virtio_blk virtio4: 1/0/0 default/read/poll queues
[    0.020043] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)
[    0.020324] zram: Added device: zram0
[    0.020386] Loading iSCSI transport class v2.0-870.
[    0.020487] tun: Universal TUN/TAP device driver, 1.6
[    0.020556] vcan: Virtual CAN interface driver
[    0.020604] CAN device driver interface
[    0.020706] rtc-pl031 a001000.rtc: registered as rtc0
[    0.020756] rtc-pl031 a001000.rtc: setting system clock to 2026-10-18T19:31:31 UTC (1792351891)
[    0.020837] i2c_dev: i2c /dev entries driver
[    0.020900] pps_ldisc: PPS line discipline registered
[    0.021030] NET: Registered PF_INET6 protocol family
[    0.021142] Segment Routing with IPv6
[    0.021186] In-situ OAM (IOAM) with IPv6
[    0.021240] NET: Registered PF_PACKET protocol family
[    0.021293] Bridge firewalling registered
[    0.021343] can: controller area network core
[    0.021396] NET: Registered PF_CAN protocol family
[    0.021445] can: raw protocol
[    0.021489] can: broadcast manager protocol
[    0.021541] can: netlink gateway - max_hops=1
[    0.021613] NET: Registered PF_KCM protocol family
[    0.021689] NET: Registered PF_VSOCK protocol family
[    0.021998] NET: Registered PF_TSI protocol family
[    0.022048] NET: Registered PF_TSI6 protocol family
[    0.022096] NET: Registered PF_TSIU protocol family
[    0.022383] registered taskstats version 1
[    0.022961] Btrfs loaded, zoned=yes, fsverity=no
[    0.023074] Key type encrypted registered
[    0.036716] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036804] alg: No test for 842 (842-scomp)
[    0.036869] alg: No test for 842 (842-generic)
[    0.040846] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048132] clk: Disabling unused clocks
[    0.048184] PM: genpd: Disabling unused power domains
[    0.048233] ALSA device list:
[    0.048282]   No soundcards found.
[    0.049031] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049169] devtmpfs: mounted
[    0.049271] Freeing unused kernel memory: 2496K
[    0.049327] Run /init as init process
[    0.049374]   with arguments:
[    0.049415]     /init
[    0.049456]   with environment:
[    0.049498]     HOME=/
[    0.049536]     TERM=linux
[    0.049577]     KRUN_BOOT_START_NS=1792351891334142000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792351891334142000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1792351891334142000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
MINOS-GUEST-POWEROFF
[    0.069427] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792351891334142000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1792351891334142000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    }
  ],
  "v": 1
}
== end store_root_boots_under_seatbelt
ok
test store_root_without_ancestor_rule_is_denied ... == store_root_without_ancestor_rule_is_denied: profile without the rule at /private/tmp/mnsQXg87mmS/no-root-ancestors.sb
== store_root_without_ancestor_rule_is_denied: root image at /private/tmp/mnswyyAZZ3W/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
== store_root_without_ancestor_rule_is_denied argv:
  /usr/bin/sandbox-exec
  -f
  /private/tmp/mnsQXg87mmS/no-root-ancestors.sb
  -D
  CONFIG=/private/tmp/mnswyyAZZ3W/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnswyyAZZ3W/console.log
  -D
  INPUT_IMG=/private/tmp/mnswyyAZZ3W/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnswyyAZZ3W/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
  -D
  SCRATCH_IMG=/private/tmp/mnswyyAZZ3W/scratch.img
  -D
  SOCK=/private/tmp/mnswyyAZZ3W/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnswyyAZZ3W/vmm.json
== store_root_without_ancestor_rule_is_denied: exit_code Some(66), signal None, killed_at_deadline false, elapsed 15.268125ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (194 bytes):
minos-vmm: error: preflight: disks.root /private/tmp/mnswyyAZZ3W/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc: Operation not permitted (os error 1)

-- minos-vmm stdout (0 bytes):

== end store_root_without_ancestor_rule_is_denied
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.47s

```

## W0-ROOT-DENY

`store_root_without_ancestor_rule_is_denied`: the same layout under the profile with the ROOT_IMG `path-ancestors`
line and its comment removed (written to a temporary profile, passed with `-f`). The helper exits 66 before the VM
starts, and its stderr reads
`minos-vmm: error: preflight: disks.root <store path>: Operation not permitted (os error 1)`: preflight's
`canonicalize` cannot stat the ancestors of the store path (05-RESEARCH Pitfall 2). The first run is in W0-ROOT above
(exit_code Some(66), the same stderr line); this is a rerun of that one test.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test wave0_store_root store_root_without_ancestor_rule_is_denied -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/wave0_store_root.rs (target/debug/deps/wave0_store_root-26f74b11f22f1340)

running 1 test
test store_root_without_ancestor_rule_is_denied ... == store_root_without_ancestor_rule_is_denied: profile without the rule at /private/tmp/mns2NuRAJrN/no-root-ancestors.sb
== store_root_without_ancestor_rule_is_denied: root image at /private/tmp/mnsy6HpFBH2/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
== store_root_without_ancestor_rule_is_denied argv:
  /usr/bin/sandbox-exec
  -f
  /private/tmp/mns2NuRAJrN/no-root-ancestors.sb
  -D
  CONFIG=/private/tmp/mnsy6HpFBH2/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsy6HpFBH2/console.log
  -D
  INPUT_IMG=/private/tmp/mnsy6HpFBH2/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsy6HpFBH2/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc
  -D
  SCRATCH_IMG=/private/tmp/mnsy6HpFBH2/scratch.img
  -D
  SOCK=/private/tmp/mnsy6HpFBH2/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsy6HpFBH2/vmm.json
== store_root_without_ancestor_rule_is_denied: exit_code Some(66), signal None, killed_at_deadline false, elapsed 11.747333ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (194 bytes):
minos-vmm: error: preflight: disks.root /private/tmp/mnsy6HpFBH2/proj/.minos/blobs/sha256/d6/666db8273a77a22d8222c90bdaa1473cb4991ee1734e3a038ab344ebb5f8fc: Operation not permitted (os error 1)

-- minos-vmm stdout (0 bytes):

== end store_root_without_ancestor_rule_is_denied
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.46s
```

The unified log was queried for the kernel's Seatbelt report, as Phase 2 did (02-SPIKE-EVIDENCE.md S05-ITER). On
27.0.1 the query returned no `Sandbox: minos-vmm ... deny(1)` line for either run, only the query's own entries; the
denial is recorded here from the helper's stderr and exit code, which are the plan's criterion.

Command: `/usr/bin/log show --last 1m --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-18 15:31:38.276 Df log[1289:6cdde] [com.apple.log:] log run noninteractively, parent: 1288 (zsh), args: '/usr/bin/log' 'show' '--last' '5m' '--style' 'compact' '--predicate' 'eventMessage CONTAINS[c] "deny" AND eventMessage CONTAINS "minos-vmm"'
2026-10-18 15:31:59.874 Df log[1496:6d007] [com.apple.log:] log run noninteractively, parent: 1472 (zsh), args: '/usr/bin/log' 'show' '--last' '1m' '--style' 'compact' '--predicate' 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'
```

Result: the rule is needed and sufficient. It allows `file-read-metadata` on the ancestors of the one root image path
only: no file contents, no directory listing.

## W0-CONSOLE

`console_pipe_carries_boot_output` and `console_pipe_cap_is_exact` (KRUN-01, 05-RESEARCH Pattern 7). The launch config
names the console `/dev/fd/1` (`minos-vmm`'s `STDOUT_CONSOLE`), `CONSOLE_LOG` is the same literal, and the helper is
spawned directly with an empty environment, stdin `/dev/null`, stdout and stderr piped, and its own process group.
The profile needed no new rule: its existing `file-read* file-write*` literal on `CONSOLE_LOG` covers `/dev/fd/1`.

- A3 (the console works on a pipe under the deny-default profile): holds. The boot run exits 0, the stdout pipe
  carries 3634 console bytes including `MINOS-GUEST-BOOT-OK`, and one connection carries a valid status frame.
- A4 (the helper creates no regular file for the console): holds. After both runs the job directory holds exactly the
  five entries the test created (`input.img`, `o.sock`, `root.img`, `scratch.img`, `vmm.json`); no `console.log` or
  other file appeared.
- The cap is exact: with a 262144-byte cap and a 4 MiB `console_flood`, the reader stopped at exactly 262145 bytes
  (cap + 1), the test killed the helper's process group, and the helper ended by signal 9, the test's SIGKILL. The
  watchdog's console-cap trip is inert for the stdout literal, so the 30000 to 48954-byte overshoot of the file
  console (K7-04) does not apply.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test wave0_console_pipe -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/wave0_console_pipe.rs (target/debug/deps/wave0_console_pipe-2471d18c8f7842bf)

running 2 tests
test console_pipe_cap_is_exact ... == console_pipe_cap_is_exact argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsjhYvLnXi/vmm.json
  -D
  CONSOLE_LOG=/dev/fd/1
  -D
  INPUT_IMG=/private/tmp/mnsjhYvLnXi/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsjhYvLnXi/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsjhYvLnXi/scratch.img
  -D
  SOCK=/private/tmp/mnsjhYvLnXi/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsjhYvLnXi/vmm.json
== console_pipe_cap_is_exact: status ExitStatus(unix_wait_status(9)), exit code None, signal Some(9), killed_by_test true, elapsed 467.09375ms, connections 0
-- stdout read error None, stderr read error None
-- job directory entries: {"input.img", "o.sock", "root.img", "scratch.img", "vmm.json"}
-- console on the stdout pipe (262145 bytes), first 400 characters:
[    0.018527] random: crng init done\u{d}
[    0.018674] loop: module loaded\u{d}
[    0.018753] virtio_blk virtio2: 1/0/0 default/read/poll queues\u{d}
[    0.018900] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)\u{d}
[    0.019231] virtio_blk virtio3: 1/0/0 default/read/poll queues\u{d}
[    0.019343] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)\u{d
-- minos-vmm stderr (0 bytes):

== end console_pipe_cap_is_exact (467.09375ms)
ok
test console_pipe_carries_boot_output ... == console_pipe_carries_boot_output argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnspH7OcGlM/vmm.json
  -D
  CONSOLE_LOG=/dev/fd/1
  -D
  INPUT_IMG=/private/tmp/mnspH7OcGlM/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnspH7OcGlM/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnspH7OcGlM/scratch.img
  -D
  SOCK=/private/tmp/mnspH7OcGlM/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnspH7OcGlM/vmm.json
== console_pipe_carries_boot_output: status ExitStatus(unix_wait_status(0)), exit code Some(0), signal None, killed_by_test false, elapsed 308.323958ms, connections 1
-- stdout read error None, stderr read error None
-- job directory entries: {"input.img", "o.sock", "root.img", "scratch.img", "vmm.json"}
-- console on the stdout pipe (3634 bytes):
[    0.018510] random: crng init done\u{d}
[    0.018632] loop: module loaded\u{d}
[    0.018705] virtio_blk virtio2: 1/0/0 default/read/poll queues\u{d}
[    0.018824] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)\u{d}
[    0.019149] virtio_blk virtio3: 1/0/0 default/read/poll queues\u{d}
[    0.019280] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)\u{d}
[    0.019464] Dev vdb: unable to read RDB block 1\u{d}
[    0.019515]  vdb: unable to read partition table\u{d}
[    0.019568] vdb: partition table beyond EOD, truncated\u{d}
[    0.019640] virtio_blk virtio4: 1/0/0 default/read/poll queues\u{d}
[    0.019750] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)\u{d}
[    0.020007] zram: Added device: zram0\u{d}
[    0.020068] Loading iSCSI transport class v2.0-870.\u{d}
[    0.020156] tun: Universal TUN/TAP device driver, 1.6\u{d}
[    0.020215] vcan: Virtual CAN interface driver\u{d}
[    0.020257] CAN device driver interface\u{d}
[    0.020349] rtc-pl031 a001000.rtc: registered as rtc0\u{d}
[    0.020397] rtc-pl031 a001000.rtc: setting system clock to 2026-10-18T19:32:11 UTC (1792351931)\u{d}
[    0.020478] i2c_dev: i2c /dev entries driver\u{d}
[    0.020540] pps_ldisc: PPS line discipline registered\u{d}
[    0.020659] NET: Registered PF_INET6 protocol family\u{d}
[    0.020765] Segment Routing with IPv6\u{d}
[    0.020805] In-situ OAM (IOAM) with IPv6\u{d}
[    0.020853] NET: Registered PF_PACKET protocol family\u{d}
[    0.020901] Bridge firewalling registered\u{d}
[    0.020940] can: controller area network core\u{d}
[    0.020991] NET: Registered PF_CAN protocol family\u{d}
[    0.021035] can: raw protocol\u{d}
[    0.021071] can: broadcast manager protocol\u{d}
[    0.021110] can: netlink gateway - max_hops=1\u{d}
[    0.021183] NET: Registered PF_KCM protocol family\u{d}
[    0.021256] NET: Registered PF_VSOCK protocol family\u{d}
[    0.021536] NET: Registered PF_TSI protocol family\u{d}
[    0.021587] NET: Registered PF_TSI6 protocol family\u{d}
[    0.021631] NET: Registered PF_TSIU protocol family\u{d}
[    0.021913] registered taskstats version 1\u{d}
[    0.022492] Btrfs loaded, zoned=yes, fsverity=no\u{d}
[    0.022610] Key type encrypted registered\u{d}
[    0.036197] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)\u{d}
[    0.036275] alg: No test for 842 (842-scomp)\u{d}
[    0.036341] alg: No test for 842 (842-generic)\u{d}
[    0.038819] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher\u{d}
[    0.048094] clk: Disabling unused clocks\u{d}
[    0.048134] PM: genpd: Disabling unused power domains\u{d}
[    0.048195] ALSA device list:\u{d}
[    0.048234]   No soundcards found.\u{d}
[    0.048579] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.\u{d}
[    0.048707] devtmpfs: mounted\u{d}
[    0.048818] Freeing unused kernel memory: 2496K\u{d}
[    0.048871] Run /init as init process\u{d}
[    0.048910]   with arguments:\u{d}
[    0.048953]     /init\u{d}
[    0.048987]   with environment:\u{d}
[    0.049025]     HOME=/\u{d}
[    0.049059]     TERM=linux\u{d}
[    0.049090]     KRUN_BOOT_START_NS=1792351930982225000\u{d}
MINOS-GUEST-BOOT-OK\u{d}
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792351930982225000  --\u{d}
minos-guest: osrelease 6.12.109\u{d}
minos-guest: args /init\u{d}
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1792351930982225000 TERM=linux\u{d}
minos-guest: net_ifaces lo\u{d}
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47\u{d}
MINOS-GUEST-FRAME-SENT 635 bytes\u{d}
MINOS-GUEST-POWEROFF\u{d}
[    0.072065] reboot: Power down\u{d}

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- connection 0 (635 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792351930982225000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1792351930982225000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    }
  ],
  "v": 1
}
== end console_pipe_carries_boot_output (308.323958ms)
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.83s

```

## W0-CHUNKS

`chunked_16_mib_reassembles_and_is_measured` and `chunked_1_mib_reassembles` (GUEST-04, S03-ANSWER). The guest's
spike-only `chunk_stream` step sends the `i % 251` pattern with `minos_core::chunk::ChunkStream`, one fresh vsock
connection per chunk (at most 4144 bytes, under the 8192-byte single-connection limit of DIAG-01). The host waits on
the listener with `rustix::event::poll` (100 ms), reads each connection to EOF through `MAX_CHUNK_BYTES + 1` with a
5 s deadline, and feeds `MINOSCK1` connections to `minos_core::chunk::Reassembler` (max_total 17 MiB). Machine:
1 vCPU, 512 MiB; helper max duration 120 s. Both streams reassembled to exactly the pattern bytes with no fault, no
unrouted connection, and one valid status frame.

S03-ANSWER asked for the cost of one connection per 4 KiB chunk. Measured on this Mac:

| Payload bytes | Wall time, first chunk accept to END | Connections (host = guest) | Guest connect retries | Guest elapsed | Helper total run time |
|---|---|---|---|---|---|
| 16777216 | 246.860041 ms | 4097 | 0 | 258 ms | 588.207917 ms |
| 16777216 (repeat) | 247.31475 ms | 4097 | 0 | 258 ms | 588.677042 ms |
| 1048576 | 15.9885 ms | 257 | 0 | 16 ms | 350.070916 ms |

16 MiB in about 247 ms is about 65 MiB/s of payload, roughly ten times the about 6 MiB/s one connection measured in
DIAG-01's instrumented runs, and far inside the 15 s selection bound. The guest's `bytes_sent` (16973916 for 16 MiB)
counts wire bytes, headers included: 4096 DATA chunks of 4144 bytes plus one 92-byte END.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test wave0_chunks -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/wave0_chunks.rs (target/debug/deps/wave0_chunks-0661f10b4ae2b916)

running 2 tests
test chunked_16_mib_reassembles_and_is_measured ... == chunked_16_mib_reassembles_and_is_measured: job {"v":1,"vsock_port":1024,"steps":[{"chunk_stream":{"bytes":16777216,"token":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"}}],"finish":"poweroff"}
== chunked_16_mib_reassembles_and_is_measured argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsgiUZHIYb/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsgiUZHIYb/console.log
  -D
  INPUT_IMG=/private/tmp/mnsgiUZHIYb/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsgiUZHIYb/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsgiUZHIYb/scratch.img
  -D
  SOCK=/private/tmp/mnsgiUZHIYb/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsgiUZHIYb/vmm.json
== chunked_16_mib_reassembles_and_is_measured: status ExitStatus(unix_wait_status(0)), exit code Some(0), signal None, killed_at_deadline false, helper elapsed 588.207917ms
-- console (3141 bytes):
[    0.018700] random: crng init done\u{d}
[    0.018893] loop: module loaded\u{d}
[    0.018970] virtio_blk virtio2: 1/0/0 default/read/poll queues\u{d}
[    0.019106] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)\u{d}
[    0.019465] virtio_blk virtio3: 1/0/0 default/read/poll queues\u{d}
[    0.019582] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)\u{d}
[    0.019763] Dev vdb: unable to read RDB block 1\u{d}
[    0.019837]  vdb: unable to read partition table\u{d}
[    0.019889] vdb: partition table beyond EOD, truncated\u{d}
[    0.019966] virtio_blk virtio4: 1/0/0 default/read/poll queues\u{d}
[    0.020080] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)\u{d}
[    0.020335] zram: Added device: zram0\u{d}
[    0.020392] Loading iSCSI transport class v2.0-870.\u{d}
[    0.020480] tun: Universal TUN/TAP device driver, 1.6\u{d}
[    0.020542] vcan: Virtual CAN interface driver\u{d}
[    0.020590] CAN device driver interface\u{d}
[    0.020678] rtc-pl031 a001000.rtc: registered as rtc0\u{d}
[    0.020727] rtc-pl031 a001000.rtc: setting system clock to 2026-10-18T19:32:19 UTC (1792351939)\u{d}
[    0.020807] i2c_dev: i2c /dev entries driver\u{d}
[    0.020872] pps_ldisc: PPS line discipline registered\u{d}
[    0.020994] NET: Registered PF_INET6 protocol family\u{d}
[    0.021103] Segment Routing with IPv6\u{d}
[    0.021151] In-situ OAM (IOAM) with IPv6\u{d}
[    0.021205] NET: Registered PF_PACKET protocol family\u{d}
[    0.021257] Bridge firewalling registered\u{d}
[    0.021307] can: controller area network core\u{d}
[    0.021359] NET: Registered PF_CAN protocol family\u{d}
[    0.021406] can: raw protocol\u{d}
[    0.021450] can: broadcast manager protocol\u{d}
[    0.021490] can: netlink gateway - max_hops=1\u{d}
[    0.021567] NET: Registered PF_KCM protocol family\u{d}
[    0.021646] NET: Registered PF_VSOCK protocol family\u{d}
[    0.021946] NET: Registered PF_TSI protocol family\u{d}
[    0.021997] NET: Registered PF_TSI6 protocol family\u{d}
[    0.022045] NET: Registered PF_TSIU protocol family\u{d}
[    0.022330] registered taskstats version 1\u{d}
[    0.022898] Btrfs loaded, zoned=yes, fsverity=no\u{d}
[    0.023003] Key type encrypted registered\u{d}
[    0.036566] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)\u{d}
[    0.036698] alg: No test for 842 (842-scomp)\u{d}
[    0.036764] alg: No test for 842 (842-generic)\u{d}
[    0.040723] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher\u{d}
[    0.048202] clk: Disabling unused clocks\u{d}
[    0.048253] PM: genpd: Disabling unused power domains\u{d}
[    0.048321] ALSA device list:\u{d}
[    0.048368]   No soundcards found.\u{d}
[    0.048773] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.\u{d}
[    0.049118] devtmpfs: mounted\u{d}
[    0.049236] Freeing unused kernel memory: 2496K\u{d}
[    0.049293] Run /init as init process\u{d}
[    0.049337]   with arguments:\u{d}
[    0.049382]     /init\u{d}
[    0.049424]   with environment:\u{d}
[    0.049463]     HOME=/\u{d}
[    0.049504]     TERM=linux\u{d}
[    0.049542]     KRUN_BOOT_START_NS=1792351938990679000\u{d}
MINOS-GUEST-BOOT-OK\u{d}
minos-guest: chunk_stream 4097 connections, 0 retries, 258 ms\u{d}
MINOS-GUEST-FRAME-SENT 164 bytes\u{d}
MINOS-GUEST-POWEROFF\u{d}
[    0.330453] reboot: Power down\u{d}

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- host: chunk connections 4097, frames 1, other connections 0, first fault None, complete true
-- frame 0 (164 bytes):
{
  "job_error": null,
  "steps": [
    {
      "chunk_stream": {
        "bytes_sent": 16973916,
        "connect_retries": 0,
        "connections": 4097,
        "elapsed_ms": 258,
        "error": null
      }
    }
  ],
  "v": 1
}
== chunked_16_mib_reassembles_and_is_measured measurement: payload 16777216 bytes, wall time first chunk accept to END 246.860041ms (246 ms), host chunk connections 4097, guest connections 4097, guest connect_retries 0, guest elapsed_ms 258, guest bytes_sent 16973916, guest error null, helper total run time 588.207917ms (588 ms), reassembled exactly true
== chunks selection input: 16 MiB wall time 246.860041ms, bound 15s, within bound true
ok
test chunked_1_mib_reassembles ... == chunked_1_mib_reassembles: job {"v":1,"vsock_port":1024,"steps":[{"chunk_stream":{"bytes":1048576,"token":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"}}],"finish":"poweroff"}
== chunked_1_mib_reassembles argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsbSTAL2vz/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsbSTAL2vz/console.log
  -D
  INPUT_IMG=/private/tmp/mnsbSTAL2vz/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsbSTAL2vz/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsbSTAL2vz/scratch.img
  -D
  SOCK=/private/tmp/mnsbSTAL2vz/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsbSTAL2vz/vmm.json
== chunked_1_mib_reassembles: status ExitStatus(unix_wait_status(0)), exit code Some(0), signal None, killed_at_deadline false, helper elapsed 350.070916ms
-- console (3139 bytes):
[    0.018536] random: crng init done\u{d}
[    0.018695] loop: module loaded\u{d}
[    0.018766] virtio_blk virtio2: 1/0/0 default/read/poll queues\u{d}
[    0.018892] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)\u{d}
[    0.019215] virtio_blk virtio3: 1/0/0 default/read/poll queues\u{d}
[    0.019317] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)\u{d}
[    0.019488] Dev vdb: unable to read RDB block 1\u{d}
[    0.019555]  vdb: unable to read partition table\u{d}
[    0.019606] vdb: partition table beyond EOD, truncated\u{d}
[    0.019684] virtio_blk virtio4: 1/0/0 default/read/poll queues\u{d}
[    0.019799] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)\u{d}
[    0.020047] zram: Added device: zram0\u{d}
[    0.020111] Loading iSCSI transport class v2.0-870.\u{d}
[    0.020212] tun: Universal TUN/TAP device driver, 1.6\u{d}
[    0.020275] vcan: Virtual CAN interface driver\u{d}
[    0.020323] CAN device driver interface\u{d}
[    0.020427] rtc-pl031 a001000.rtc: registered as rtc0\u{d}
[    0.020477] rtc-pl031 a001000.rtc: setting system clock to 2026-10-18T19:32:20 UTC (1792351940)\u{d}
[    0.020555] i2c_dev: i2c /dev entries driver\u{d}
[    0.020619] pps_ldisc: PPS line discipline registered\u{d}
[    0.020750] NET: Registered PF_INET6 protocol family\u{d}
[    0.020863] Segment Routing with IPv6\u{d}
[    0.020910] In-situ OAM (IOAM) with IPv6\u{d}
[    0.020966] NET: Registered PF_PACKET protocol family\u{d}
[    0.021017] Bridge firewalling registered\u{d}
[    0.021065] can: controller area network core\u{d}
[    0.021117] NET: Registered PF_CAN protocol family\u{d}
[    0.021164] can: raw protocol\u{d}
[    0.021208] can: broadcast manager protocol\u{d}
[    0.021252] can: netlink gateway - max_hops=1\u{d}
[    0.021327] NET: Registered PF_KCM protocol family\u{d}
[    0.021405] NET: Registered PF_VSOCK protocol family\u{d}
[    0.021705] NET: Registered PF_TSI protocol family\u{d}
[    0.021752] NET: Registered PF_TSI6 protocol family\u{d}
[    0.021800] NET: Registered PF_TSIU protocol family\u{d}
[    0.022097] registered taskstats version 1\u{d}
[    0.022682] Btrfs loaded, zoned=yes, fsverity=no\u{d}
[    0.022792] Key type encrypted registered\u{d}
[    0.036317] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)\u{d}
[    0.036403] alg: No test for 842 (842-scomp)\u{d}
[    0.036465] alg: No test for 842 (842-generic)\u{d}
[    0.038783] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher\u{d}
[    0.048036] clk: Disabling unused clocks\u{d}
[    0.048087] PM: genpd: Disabling unused power domains\u{d}
[    0.048144] ALSA device list:\u{d}
[    0.048189]   No soundcards found.\u{d}
[    0.048598] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.\u{d}
[    0.048751] devtmpfs: mounted\u{d}
[    0.048863] Freeing unused kernel memory: 2496K\u{d}
[    0.048919] Run /init as init process\u{d}
[    0.048966]   with arguments:\u{d}
[    0.049011]     /init\u{d}
[    0.049051]   with environment:\u{d}
[    0.049094]     HOME=/\u{d}
[    0.049133]     TERM=linux\u{d}
[    0.049173]     KRUN_BOOT_START_NS=1792351940399974000\u{d}
MINOS-GUEST-BOOT-OK\u{d}
minos-guest: chunk_stream 257 connections, 0 retries, 16 ms\u{d}
MINOS-GUEST-FRAME-SENT 161 bytes\u{d}
MINOS-GUEST-POWEROFF\u{d}
[    0.088610] reboot: Power down\u{d}

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- host: chunk connections 257, frames 1, other connections 0, first fault None, complete true
-- frame 0 (161 bytes):
{
  "job_error": null,
  "steps": [
    {
      "chunk_stream": {
        "bytes_sent": 1060956,
        "connect_retries": 0,
        "connections": 257,
        "elapsed_ms": 16,
        "error": null
      }
    }
  ],
  "v": 1
}
== chunked_1_mib_reassembles measurement: payload 1048576 bytes, wall time first chunk accept to END 15.9885ms (15 ms), host chunk connections 257, guest connections 257, guest connect_retries 0, guest elapsed_ms 16, guest bytes_sent 1060956, guest error null, helper total run time 350.070916ms (350 ms), reassembled exactly true
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.32s

```

Repeat of the 16 MiB run:

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test wave0_chunks chunked_16_mib -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/wave0_chunks.rs (target/debug/deps/wave0_chunks-0661f10b4ae2b916)

running 1 test
test chunked_16_mib_reassembles_and_is_measured ... == chunked_16_mib_reassembles_and_is_measured: job {"v":1,"vsock_port":1024,"steps":[{"chunk_stream":{"bytes":16777216,"token":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"}}],"finish":"poweroff"}
== chunked_16_mib_reassembles_and_is_measured argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnshUU2mdi3/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnshUU2mdi3/console.log
  -D
  INPUT_IMG=/private/tmp/mnshUU2mdi3/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnshUU2mdi3/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnshUU2mdi3/scratch.img
  -D
  SOCK=/private/tmp/mnshUU2mdi3/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnshUU2mdi3/vmm.json
== chunked_16_mib_reassembles_and_is_measured: status ExitStatus(unix_wait_status(0)), exit code Some(0), signal None, killed_at_deadline false, helper elapsed 588.677042ms
-- console (3141 bytes):
[    0.018563] random: crng init done\u{d}
[    0.018717] loop: module loaded\u{d}
[    0.018793] virtio_blk virtio2: 1/0/0 default/read/poll queues\u{d}
[    0.018922] virtio_blk virtio2: [vda] 912 512-byte logical blocks (467 kB/456 KiB)\u{d}
[    0.019272] virtio_blk virtio3: 1/0/0 default/read/poll queues\u{d}
[    0.019390] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)\u{d}
[    0.019611] Dev vdb: unable to read RDB block 1\u{d}
[    0.019664]  vdb: unable to read partition table\u{d}
[    0.019713] vdb: partition table beyond EOD, truncated\u{d}
[    0.019796] virtio_blk virtio4: 1/0/0 default/read/poll queues\u{d}
[    0.019910] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)\u{d}
[    0.020176] zram: Added device: zram0\u{d}
[    0.020236] Loading iSCSI transport class v2.0-870.\u{d}
[    0.020334] tun: Universal TUN/TAP device driver, 1.6\u{d}
[    0.020398] vcan: Virtual CAN interface driver\u{d}
[    0.020445] CAN device driver interface\u{d}
[    0.020549] rtc-pl031 a001000.rtc: registered as rtc0\u{d}
[    0.020598] rtc-pl031 a001000.rtc: setting system clock to 2026-10-18T19:32:29 UTC (1792351949)\u{d}
[    0.020680] i2c_dev: i2c /dev entries driver\u{d}
[    0.020741] pps_ldisc: PPS line discipline registered\u{d}
[    0.020860] NET: Registered PF_INET6 protocol family\u{d}
[    0.020972] Segment Routing with IPv6\u{d}
[    0.021015] In-situ OAM (IOAM) with IPv6\u{d}
[    0.021070] NET: Registered PF_PACKET protocol family\u{d}
[    0.021122] Bridge firewalling registered\u{d}
[    0.021169] can: controller area network core\u{d}
[    0.021221] NET: Registered PF_CAN protocol family\u{d}
[    0.021273] can: raw protocol\u{d}
[    0.021320] can: broadcast manager protocol\u{d}
[    0.021363] can: netlink gateway - max_hops=1\u{d}
[    0.021428] NET: Registered PF_KCM protocol family\u{d}
[    0.021503] NET: Registered PF_VSOCK protocol family\u{d}
[    0.021812] NET: Registered PF_TSI protocol family\u{d}
[    0.021873] NET: Registered PF_TSI6 protocol family\u{d}
[    0.021919] NET: Registered PF_TSIU protocol family\u{d}
[    0.022205] registered taskstats version 1\u{d}
[    0.022771] Btrfs loaded, zoned=yes, fsverity=no\u{d}
[    0.022889] Key type encrypted registered\u{d}
[    0.036618] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)\u{d}
[    0.036704] alg: No test for 842 (842-scomp)\u{d}
[    0.036776] alg: No test for 842 (842-generic)\u{d}
[    0.040739] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher\u{d}
[    0.048148] clk: Disabling unused clocks\u{d}
[    0.048200] PM: genpd: Disabling unused power domains\u{d}
[    0.048254] ALSA device list:\u{d}
[    0.048299]   No soundcards found.\u{d}
[    0.048669] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.\u{d}
[    0.048792] devtmpfs: mounted\u{d}
[    0.048909] Freeing unused kernel memory: 2496K\u{d}
[    0.048967] Run /init as init process\u{d}
[    0.049013]   with arguments:\u{d}
[    0.049058]     /init\u{d}
[    0.049098]   with environment:\u{d}
[    0.049144]     HOME=/\u{d}
[    0.049184]     TERM=linux\u{d}
[    0.049225]     KRUN_BOOT_START_NS=1792351948985233000\u{d}
MINOS-GUEST-BOOT-OK\u{d}
minos-guest: chunk_stream 4097 connections, 0 retries, 258 ms\u{d}
MINOS-GUEST-FRAME-SENT 164 bytes\u{d}
MINOS-GUEST-POWEROFF\u{d}
[    0.331129] reboot: Power down\u{d}

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- host: chunk connections 4097, frames 1, other connections 0, first fault None, complete true
-- frame 0 (164 bytes):
{
  "job_error": null,
  "steps": [
    {
      "chunk_stream": {
        "bytes_sent": 16973916,
        "connect_retries": 0,
        "connections": 4097,
        "elapsed_ms": 258,
        "error": null
      }
    }
  ],
  "v": 1
}
== chunked_16_mib_reassembles_and_is_measured measurement: payload 16777216 bytes, wall time first chunk accept to END 247.31475ms (247 ms), host chunk connections 4097, guest connections 4097, guest connect_retries 0, guest elapsed_ms 258, guest bytes_sent 16973916, guest error null, helper total run time 588.677042ms (588 ms), reassembled exactly true
== chunks selection input: 16 MiB wall time 247.31475ms, bound 15s, within bound true
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 1.41s

```

## Mechanism selections

Plans 05-07, 05-09, 05-15, and 05-21 branch on these three lines.

- console: pipe
- root image: store-path
- chunks: proceed

console: both W0-CONSOLE tests pass, so the console is the helper's stdout pipe read by the supervisor through
cap + 1 (the file console with the 50 ms watchdog and a cap + 65536 host budget, K7-04, is not needed). root image:
the rule run boots and the no-rule run exits 66 with the denial (W0-ROOT, W0-ROOT-DENY), so the backend boots the
verified store path directly and keeps the ROOT_IMG ancestor-metadata rule (no hardlink). chunks: 16 MiB reassembled
exactly with no fault in about 247 ms, under the 15 s bound, so the protocol, caps, and dependencies stay as they are.

## 05-07 clippy bans fire

Requirement: KRUN-02 (05-RESEARCH Open Question 5), T-05-07-02

Edit: four entries added to `crates/minos-vmm/clippy.toml`: `msb_krun::api::builders::ConsoleBuilder::custom`,
`ConsoleBuilder::custom_with_options`, `VsockBuilder::custom`, and `VsockBuilder::custom_dgram`, each written with the
definition path, as D-PROBE did. To prove each fires, a throwaway `#[cfg(test)] mod disallowed_probe` with one
`#[test] fn every_new_banned_method_fires()` was appended to `crates/minos-vmm/src/launch.rs`. It binds each banned
method as a path expression through the `msb_krun::` root re-exports, with no call (the form D-PROBE showed fires for
`FsBuilder::custom`), and passes the four to `std::hint::black_box`: `let console_custom =
msb_krun::ConsoleBuilder::custom;`, `let console_custom_with_options =
msb_krun::ConsoleBuilder::custom_with_options;`, `let vsock_custom = msb_krun::VsockBuilder::custom;`, and `let
vsock_custom_dgram = msb_krun::VsockBuilder::custom_dgram;`. The probe uses no unwrap, expect, indexing, unsafe, or
allow. `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14).

Command: `cargo clippy -p minos-vmm --all-targets -- -D warnings > "$S/probe0507.txt" 2>&1; echo "exit: $?"; grep '^error' "$S/probe0507.txt" | sort | uniq -c`

Exit code: 101

```text
exit: 101
   1 error: could not compile `minos-vmm` (bin "minos-vmm" test) due to 4 previous errors
   1 error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::custom_with_options`
   1 error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::custom`
   1 error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::custom_dgram`
   1 error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::custom`
```

Four distinct paths, one error each, and no other error. Each error in full, with the reason text from `clippy.toml`:

Command: `grep -A6 '^error: use of a disallowed' "$S/probe0507.txt"`

Exit code: 0

```text
error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::custom`
   --> crates/minos-vmm/src/launch.rs:135:30
    |
135 |         let console_custom = msb_krun::ConsoleBuilder::custom;
    |                              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: a custom console backend hands host code a guest-facing port
--
error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::custom_with_options`
   --> crates/minos-vmm/src/launch.rs:136:43
    |
136 |         let console_custom_with_options = msb_krun::ConsoleBuilder::custom_with_options;
    |                                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: a custom console backend hands host code a guest-facing port
--
error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::custom`
   --> crates/minos-vmm/src/launch.rs:137:28
    |
137 |         let vsock_custom = msb_krun::VsockBuilder::custom;
    |                            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: only the unix_connect route may carry guest bytes
--
error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::custom_dgram`
   --> crates/minos-vmm/src/launch.rs:138:34
    |
138 |         let vsock_custom_dgram = msb_krun::VsockBuilder::custom_dgram;
    |                                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: only the unix_connect route may carry guest bytes
```

Revert: `git checkout -- crates/minos-vmm/src/launch.rs`, then `git diff --quiet -- crates/minos-vmm/src/launch.rs`
(exit 0).

Command: `cargo clippy -p minos-vmm --all-targets -- -D warnings`

Exit code: 0

`ConsoleBuilder::custom` delegates to `custom_with_options` inside `msb_krun`; clippy lints only this crate's uses, so
banning both closes the console route whichever one a future edit reaches for.

### 05-07 helper rlimits: stderr must be a pipe

Requirement: KRUN-07 (D-01; 05-RESEARCH Assumption A4), T-05-07-03

The helper sets `RLIMIT_CORE` 0, `RLIMIT_NOFILE` 1024, and `RLIMIT_FSIZE` 0 before it reads its arguments. No VM was
run (the backend's serializer still writes v1 until 05-21); these runs only exercise the rlimit and error path on this
host. `S` is the session scratch directory.

Command: `bash -c 'target/debug/minos-vmm /nonexistent.json 2>&1 | cat; echo "exit: ${PIPESTATUS[0]}"'`

Exit code: 0 (the helper's own exit is the printed one)

```text
minos-vmm: error: reading launch config /nonexistent.json: No such file or directory (os error 2)
exit: 65
```

Command: `bash -c 'target/debug/minos-vmm /nonexistent.json 2>"$S/stderr.txt" | cat; echo "exit: ${PIPESTATUS[0]}"; wc -c < "$S/stderr.txt"'`

Exit code: 0 (the helper's own exit is the printed one)

```text
exit: 153
       0
```

With stderr on a pipe, every limit is set and the config error reaches the parent with exit 65. With stderr on a
regular file, the first stderr write passes `RLIMIT_FSIZE` 0 and the helper dies by `SIGXFSZ` (153 = 128 + 25),
writing nothing. That fails closed (the supervisor counts a signal it did not send as an infrastructure failure), but
it loses the diagnostic, so the supervisor (05-15) must give the helper stderr on a pipe, as 05-RESEARCH Pattern 7
already requires; anyone running the helper by hand must pipe its stderr too.

## 05-15 EPERM on a zombie-only process group

Requirement: KRUN-05 (05-RESEARCH Pitfall 11; 04 deferred-items, the flaky process-group test), T-05-15-06

Step 1, before any change: the Phase 2 test `supervise_run_kills_the_whole_process_group_at_the_host_deadline` was run
20 times by itself (`target/debug/deps/minos_backend_krun-31cf7fc38794285f supervise_run_kills_the_whole_process_group_at_the_host_deadline --exact ...`).
Result: 20 passed, 0 failed; no `EPERM`.

Step 2, after the rewrite: 100 runs of the new `supervise` tests failed 3 times, every time in `drop_kills_group` with
`probing the process group: Operation not permitted (os error 1)`. In that test the leader was SIGKILLed by
`Helper::drop` but not yet reaped (the console reader still held the shared process), so the group's only member was
the test's own zombie child.

Step 3, the proof: a throwaway test spawned `/bin/sleep 30` in its own group, probed it with `kill(-pgid, 0)`, sent
SIGKILL, slept 200 ms without reaping, probed again, listed the group with `/bin/ps -ax -o pid=,pgid=,stat=`, then
reaped and probed a third time. Five rounds, macOS 27.0.1 (26A434):

```text
TEMPDIAG round=0 pid=83357 alive=Ok(()) zombie=Err(Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }) members=["83357 83357 Z   "] reaped=Err(Os { code: 3, kind: Uncategorized, message: "No such process" })
TEMPDIAG round=1 pid=83359 alive=Ok(()) zombie=Err(Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }) members=["83359 83359 Z   "] reaped=Err(Os { code: 3, kind: Uncategorized, message: "No such process" })
(rounds 2 to 4 identical in shape)
```

Answer: on macOS, `kill(-pgid, sig)` on a group whose only remaining members are zombies returns `EPERM`, not `ESRCH`,
every time; it returns `ESRCH` once the zombie is reaped. The throwaway test was removed. Changes: `Helper::drop` now
reaps the helper right after the kill; `Reaper::kill` accepts `EPERM` only when `waitid(WNOWAIT)` shows the unreaped
helper has exited (so it is one of the zombies); the tests' group probe treats `EPERM` as "keep waiting" only when
`ps` lists no live member of the group, and still ends only on `ESRCH`. After the change, 200 runs of the `supervise`
tests and 50 runs of the whole crate's unit tests passed with no failure. The sweep (05-11) is unchanged: it still
reports `EPERM` as Unverified.

## 05-17 first production boot and the write-only console rule

Requirement: GUEST-01, KRUN-04 (05-09 note: the narrowed `(allow file-write* (literal "/dev/fd/1"))` console rule had
never been booted)

The first VM launched through `KrunBackend` (`experiment_applies_diff_and_runs_as_candidate_uid`, HEAD 017fc11 plus the
uncommitted `tests/vm.rs`) booted under the embedded deny-default profile with the write-only console rule, returned a
valid authenticated stream, and ended `Exited` after 547 ms. No exit 66, no missing console: the narrowed rule holds.
The root image was the test image packed by `rootfs::pack_root_image` and named from
`<tmp>/proj/.minos/blobs/sha256/<2>/<62>` (store-path branch, D-14), and the job root was `<tmp>/minos-501` under a
fresh `/private/tmp/mvt*` directory.

## 05-17 helper `RLIMIT_NOFILE` and the vsock reaper

Requirement: GUEST-04 (S03-ANSWER asked for the chunk cost in production), KRUN-07 (the helper's own rlimits)

Symptom: the first full suite run passed, but `output_16_mib_is_reassembled` took 20.528531416 s for the launch, against
about 0.59 s of helper run time for the same 16 MiB in W0-CHUNKS. The over-cap run (17000000 bytes, nothing streamed)
took 0.35 s, so the time was in the stream.

Diagnosis, with throwaway instrumentation that was reverted before any commit (`git checkout` of `chunks.rs`,
`linux.rs`, `sink.rs`, and the harness; the committed guest prints no diagnostic):

| Run | Host receiver total | Waiting on the listener | Reading connections | Guest connect total | Connects over 2 ms | Guest connect retries |
|---|---|---|---|---|---|---|
| NOFILE 1024, 2048 MiB VM | 20.47 s | 20.25 s | 0.19 s | 20.07 s | 4 | 300 (302 in an earlier run) |
| NOFILE 1024, 1024 MiB VM | 20.53 s | 20.32 s | 0.18 s | not instrumented | | |
| NOFILE 1024, 640 MiB VM | 20.49 s | 20.28 s | 0.18 s | not instrumented | | |
| NOFILE 8192, 2048 MiB VM | 0.85 s | 0.67 s | 0.16 s | 0.21 s | 0 | 0 |

The host was idle 99% of the time and VM memory made no difference. Four connects in 3909 took about 5 s each, made of
about 75 reset-and-retry attempts on the guest's 1 ms to 64 ms backoff. The cause is in the pinned crate's source,
`msb_krun_devices-0.1.41/src/virtio/vsock/reaper.rs`: `const TIMEOUT: Duration = Duration::new(5, 0);` the reaper
keeps every released connection's proxy, and with it the host-side socket, for 5 s after the connection closes. With
the helper's `RLIMIT_NOFILE` at 1024 (05-07's `NOFILE_LIMIT`), every thousand or so connections exhausted the helper's
descriptors, new guest connects were reset until the reaper freed the oldest ones, and the stream stalled about 5 s
each time (3909 connections, 4 stalls, 20 s). W0-CHUNKS ran before 05-07 added the limit, under the shell's 1048576.

Fix (Minos configuration, no dependency change): `NOFILE_LIMIT` in `crates/minos-vmm/src/main.rs` is 8192, above one
whole output stream (`MAX_OUTPUT_STREAM_BYTES` over `MAX_CHUNK_PAYLOAD` is about 4353 connections) inside one reaper
window, and still a bound on the helper's descriptors. `output_16_mib_is_reassembled` now asserts the launch takes less
than the 15 s W0 selection bound. After the fix the production 16 MiB launch took 0.606650500 s, then 0.602410 s.

## 05-17 VM suite

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1`

Run after `cargo build -p minos-vmm`, the CLAUDE.md `codesign` argv (entitlements listed:
`com.apple.security.cs.disable-library-validation`, `com.apple.security.hypervisor`), and
`cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`, at HEAD 754ed85, macOS 27.0.1, rustc 1.97.1.
Guest-derived text is printed only as parsed probe JSON or through `LogExcerpt`. Afterwards `pgrep -fl minos-vmm`
printed nothing.

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/vm.rs (target/debug/deps/vm-69052a93a1f98219)

running 10 tests
test apply_mismatch_is_rejected_and_runs_nothing ... == candidate: ended Exited after 486.721917ms
ok
test candidate_output_never_carries_a_scorecard ... == candidate: ended Exited after 334.317375ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
ok
test experiment_applies_diff_and_runs_as_candidate_uid ... == candidate: ended Exited after 338.918417ms
== out/build.json: {"cmdline":"reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792364216231627000  --","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":["Max cpu time              61                   61                   seconds   ","Max file size             1073741824           1073741824           bytes     ","Max core file size        0                    0                    bytes     ","Max processes             512                  512                  processes ","Max open files            4096                 4096                 files     ","Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
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
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.19s

```

## 05-18 env build

`minos env build` against the real Docker builder (KRUN-09, CLI-02, D-09 to D-12), at HEAD 47293e4 plus the
uncommitted 05-18 Task 2 edit to `crates/minos-cli/tests/env_build.rs` (the ignored test prints both digests and the
lock), macOS 27.0.1, rustc 1.97.1, on 2026-10-19. Docker Desktop was started by the user (Task 1). `$S` is the
session scratchpad `/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/5ea70c04-77ce-4b1d-a148-31fff4f6674e/scratchpad`,
outside the repository; the CLI runs use a copy of the test's fixture project at `$S/fixture` (T-05-18-04).

A2 fallback: **not needed.** `docker buildx build ... -f minos.Dockerfile -` took the snapshot tarball on stdin as
its context, with `-f` relative to the tarball's root, on the first run; `DockerBuildx::build_argv` is unchanged.

### Docker daemon

Command: `docker info --format '{{.ServerVersion}}'`

Exit code: 0

```text
29.6.1
```

Command: `docker buildx version`

Exit code: 0

```text
github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094
```

### Guest build

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

Command: `file target/aarch64-unknown-linux-musl/release/minos-guest`

Exit code: 0

```text
target/aarch64-unknown-linux-musl/release/minos-guest: ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV), statically linked, not stripped
```

### Reproducibility test (D-12)

Command: `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest cargo test -p minos-cli --test env_build docker_env_build_reproduces -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running tests/env_build.rs (target/debug/deps/env_build-e701da1cb81a0218)

running 1 test
test docker_env_build_reproduces ... first image:  41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
second image: 41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
minos.lock:
v = 1

[[image]]
backend = "krun"
arch = "arm64"
root_image = "41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b"
dockerfile_sha256 = "0181ab39fcf9a471656dc2fa53d78b19e45e045e98d57022ab41168159050d34"
base_images = ["docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6"]
guest_sha256 = "b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65"

[image.builder]
name = "docker-buildx"
version = "v0.35.0-desktop.2"

ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 4.98s
```

The two image digests are equal (`41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b`). The test also
asserted that the lock entry carries every D-11 field (`root_image`, `dockerfile_sha256`, `base_images`,
`guest_sha256`, `builder`), that the first build wrote the lock and the second did not, and that neither build context
held a `judge/` path or the `.env` file (T-05-18-01). The builder's tars are not compared by this test (D-12).

### CLI wiring on the scratch fixture

Command: `cargo build -p minos-cli`

Exit code: 0

Commands run in `$S/fixture` with `M=~/Repos/minos/target/debug/minos` (the transcript prints each command after `$`
and its exit code after it):

```text
$ minos env build --allow-dirty   (run 1)
root image: sha256 41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
minos.lock: written
packed: 86 files, 99 directories, 335 symlinks; /init sha256 b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65
base image: docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
exit code: 0
5faadeef914e994c97d50b58073c67e05c3aa4b4ead11f2245e7542c7f905bcd  minos.lock
v = 1

[[image]]
backend = "krun"
arch = "arm64"
root_image = "41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b"
dockerfile_sha256 = "0181ab39fcf9a471656dc2fa53d78b19e45e045e98d57022ab41168159050d34"
base_images = ["docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6"]
guest_sha256 = "b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65"

[image.builder]
name = "docker-buildx"
version = "v0.35.0-desktop.2"
$ minos env build --allow-dirty   (run 2)
root image: sha256 41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
minos.lock: unchanged
packed: 86 files, 99 directories, 335 symlinks; /init sha256 b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65
base image: docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
exit code: 0
5faadeef914e994c97d50b58073c67e05c3aa4b4ead11f2245e7542c7f905bcd  minos.lock
```

The second run printed `minos.lock: unchanged`, and the lock's SHA-256 is the same before and after it. The image is
stored under its digest with mode 0600 in the 0700 store, and the stored bytes hash to the digest:

```text
$ ls -l .minos/blobs/sha256/41/ ; shasum -a 256 of the blob
total 9048
-rw-------@ 1 seanmcauliffe  wheel  4632576 Oct 19 17:08 cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b  .minos/blobs/sha256/41/cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
$ stat -f '%Sp' .minos
drwx------
$ printf 'RUN echo minos-05-18 > /usr/local/share/minos-fixture/edit
' >> minos.Dockerfile
FROM docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
COPY program.md /usr/local/share/minos-fixture/program.md
RUN date > /usr/local/share/minos-fixture/stamp
RUN echo minos-05-18 > /usr/local/share/minos-fixture/edit
$ minos env build --allow-dirty   (run 3, Dockerfile edited, no --update)
minos: error: the built root image differs from minos.lock
  locked: sha256 41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
  built:  sha256 6848b2e1f167aa555d573f75d8a44bb76df143827cf45b61c6bb0a050ed776f3
rerun with --update to accept the new image
exit code: 1
5faadeef914e994c97d50b58073c67e05c3aa4b4ead11f2245e7542c7f905bcd  minos.lock
$ minos env build --allow-dirty --update   (run 4)
root image: sha256 6848b2e1f167aa555d573f75d8a44bb76df143827cf45b61c6bb0a050ed776f3
minos.lock: written
packed: 87 files, 99 directories, 335 symlinks; /init sha256 b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65
base image: docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
exit code: 0
5149dc03fde163dd32de0ece0058847dfb53d29f039271d2352dc5e014d33e86  minos.lock
v = 1

[[image]]
backend = "krun"
arch = "arm64"
root_image = "6848b2e1f167aa555d573f75d8a44bb76df143827cf45b61c6bb0a050ed776f3"
dockerfile_sha256 = "cbe6c7e19c06ed1d80d47d07a8c1241129b7b0f2a7521d33980b0690b34864c7"
base_images = ["docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6"]
guest_sha256 = "b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65"

[image.builder]
name = "docker-buildx"
version = "v0.35.0-desktop.2"
```

Without `--update` the changed image is refused with exit code 1, both digests printed, and the lock's SHA-256
unchanged (D-10). With `--update` it exits 0 and rewrites the lock with the new `root_image` and `dockerfile_sha256`.

### What the equal digests do and do not prove

The fixture's `RUN date > .../stamp` writes the wall-clock time into a file's contents, not only its mtime. The
control below runs the builder argv directly on the same two-file context (`program.md` and the original three-line
`minos.Dockerfile`), twice through the layer cache and twice with `--no-cache`, in `$S/cachectl`:

```text
$ docker buildx build --platform linux/arm64 --provenance=false --sbom=false  --output type=tar,dest=- -f minos.Dockerfile - < context.tar > out/cached-1.tar
exit code: 0
CACHED lines in stderr: 1
$ docker buildx build --platform linux/arm64 --provenance=false --sbom=false  --output type=tar,dest=- -f minos.Dockerfile - < context.tar > out/cached-2.tar
exit code: 0
CACHED lines in stderr: 4
$ docker buildx build --platform linux/arm64 --provenance=false --sbom=false --no-cache --output type=tar,dest=- -f minos.Dockerfile - < context.tar > out/nocache-1.tar
exit code: 0
CACHED lines in stderr: 3
$ docker buildx build --platform linux/arm64 --provenance=false --sbom=false --no-cache --output type=tar,dest=- -f minos.Dockerfile - < context.tar > out/nocache-2.tar
exit code: 0
CACHED lines in stderr: 3
$ shasum -a 256 out/*.tar
9f1a0e33d3f0f707b68259f42629c0e29ee66cb9b93ae170c117336ba0283552  out/cached-1.tar
9f1a0e33d3f0f707b68259f42629c0e29ee66cb9b93ae170c117336ba0283552  out/cached-2.tar
c8999ba3fdcb66f88e6fe0f147ee0c048626193c2cdac3707e4cc9003d04a157  out/nocache-1.tar
be39ea1560997a62c43e24478cbf482de6c8946bdd290bbba1196b4a286781d8  out/nocache-2.tar
$ tar -xOf out/cached-1.tar usr/local/share/minos-fixture/stamp
Mon Oct 19 21:08:27 UTC 2026
$ tar -xOf out/cached-2.tar usr/local/share/minos-fixture/stamp
Mon Oct 19 21:08:27 UTC 2026
$ tar -xOf out/nocache-1.tar usr/local/share/minos-fixture/stamp
Mon Oct 19 21:08:27 UTC 2026
$ tar -xOf out/nocache-2.tar usr/local/share/minos-fixture/stamp
Mon Oct 19 21:08:28 UTC 2026
```

The cached builds give byte-identical tars with the same stamp, so the equal image digests above come from
BuildKit's layer cache, not from the packer. The `--no-cache` builds give different tars, and the stamp's contents
changed between them (21:08:27, 21:08:28), so a `--no-cache` rebuild of this Dockerfile yields a different image.
`cached-1` and `nocache-1` hold the same stamp, by landing in the same second, yet their tars differ. A Python
`tarfile` comparison of the two (run with `python3 -I` in `$S/cachectl`, comparing each entry's type, mode, uid, gid,
link target, content SHA-256, and mtime) shows that only mtimes differ:

Exit code: 0

```text
entries: 517 517 same names: True
entries differing in type, mode, owner, link target, or content: []
entries differing in mtime only: ['etc', 'usr/local/share', 'usr/local/share/minos-fixture', 'usr/local/share/minos-fixture/stamp']
```

That is the metadata the packer zeroes. In general no packer can make content that a `RUN` step computes from the
clock reproduce. What the packer guarantees is that
builder tars differing only in metadata pack to the same image. Phase 2 showed this against the real builder with
`--no-cache` and a fixed-content `RUN` (02-SPIKE-EVIDENCE.md RI-04: tars differ, images match), and the production
packer's unit test proves it for tars that differ only in mtimes:

Command: `cargo test -p minos-backend-krun --lib rootfs_tar_mtimes_do_not_change_the_image`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-31cf7fc38794285f)

running 1 test
test rootfs::tests::rootfs_tar_mtimes_do_not_change_the_image ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 0.01s
```

So the claim recorded in CLAUDE.md (D-12) is: the packed SquashFS image reproduces when the builder's rootfs
contents reproduce, whatever the tar metadata; the builder's tar is not claimed to reproduce.

## Final gate

Run on 2026-10-19 from the repository root against one final build at HEAD `4cc176a` (docs(05-20): reconcile CLAUDE.md with the shipped Phase 5 code; no code changed in this plan), in this order, with `TRYBUILD` unset. `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14); `sw_vers -productVersion`: 27.0.1; Docker daemon 29.6.1 (`docker info --format '{{.ServerVersion}}'`, exit 0). Every step exited 0. Output is verbatim; the only edit is the home directory written as `~`.

### The six gate commands

1. `cargo fmt --all --check`: exit 0, no output.

2. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```

3. `cargo test --workspace`: exit 0. 63 result lines; summed over them, 1139 passed, 0 failed, 19 ignored. Each binary or doc-test run and its result line, verbatim:

```text
Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-de667b6d468a05d5)
  test result: ok. 103 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.01s
Running tests/exit.rs (target/debug/deps/exit-89797bbe80950b05)
  test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/vm.rs (target/debug/deps/vm-e5c499f08fd31caf)
  test result: ok. 0 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_cli-91159a87e1164b69)
  test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
Running unittests src/main.rs (target/debug/deps/minos-b5389aa8bbbd812d)
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/env_build.rs (target/debug/deps/env_build-8fb091108c333e81)
  test result: ok. 10 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.29s
Running tests/host_cmd.rs (target/debug/deps/host_cmd-f992bd4a302a1362)
  test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.60s
Running unittests src/lib.rs (target/debug/deps/minos_core-15efc0247bb89ee9)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/blob_hasher.rs (target/debug/deps/blob_hasher-404aa5f8c53c3c0e)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.20s
Running tests/breaker.rs (target/debug/deps/breaker-a65177c074fe7c4f)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running tests/chunk.rs (target/debug/deps/chunk-171fba7cfe1d6412)
  test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.43s
Running tests/golden_ids.rs (target/debug/deps/golden_ids-f39770f0b885d95d)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/golden_wire.rs (target/debug/deps/golden_wire-8653368d6a99caa8)
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/guest_wire.rs (target/debug/deps/guest_wire-51c78d87bb923b94)
  test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s
Running tests/lock.rs (target/debug/deps/lock-19d347079632309b)
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/manifest.rs (target/debug/deps/manifest-a5565b731af1e4aa)
  test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/money.rs (target/debug/deps/money-f530f79b61383759)
  test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/prop_apply.rs (target/debug/deps/prop_apply-5b70179c09232b5e)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running tests/prop_diff.rs (target/debug/deps/prop_diff-bdf434a540fc4a97)
  test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
Running tests/prop_fold.rs (target/debug/deps/prop_fold-ff32a195a2b6776d)
  test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s
Running tests/prop_keys.rs (target/debug/deps/prop_keys-dbee27f417c8b69b)
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
Running tests/prop_metric.rs (target/debug/deps/prop_metric-7848825970330f01)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/prop_path.rs (target/debug/deps/prop_path-2b3958736776e24d)
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/prop_policy.rs (target/debug/deps/prop_policy-87ff2a4a298fa784)
  test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
Running tests/render.rs (target/debug/deps/render-c8dc92c5bb3c0c21)
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
Running tests/report.rs (target/debug/deps/report-da118aa3a96756e6)
  test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
Running tests/route.rs (target/debug/deps/route-e11fa91934e0004f)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/sweep.rs (target/debug/deps/sweep-1509ad9dfccf5702)
  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running unittests src/lib.rs (target/debug/deps/minos_guest-ff9cca34304ea5ea)
  test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
Running unittests src/main.rs (target/debug/deps/minos_guest-2d97dc2b4d1632e5)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/bin/minos-probe.rs (target/debug/deps/minos_probe-5627d13d51fedf66)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_loop-2ba942a298f65ca1)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_propose-b8c8b45c57cfff05)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_runner-dfbc0be15af3b718)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_sandbox-4b7e5db11ee5745b)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/backend.rs (target/debug/deps/backend-abb00b226a85bc2a)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/blob_source.rs (target/debug/deps/blob_source-d424f0dbb56fc55a)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/compile_fail.rs (target/debug/deps/compile_fail-0c7906e0638ae0d5)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.36s
Running tests/output_tar.rs (target/debug/deps/output_tar-adbd864b297ee011)
  test result: ok. 35 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.54s
Running tests/preflight.rs (target/debug/deps/preflight-8a0a614f85e9d937)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/spec.rs (target/debug/deps/spec-939ba99885943f40)
  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/untrusted.rs (target/debug/deps/untrusted-5b1c00c016a9cea1)
  test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Running unittests src/lib.rs (target/debug/deps/minos_source-5429adc68d00303c)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/clean.rs (target/debug/deps/clean-4d8227224d71bbcc)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/denylist.rs (target/debug/deps/denylist-9d6bd90c60079a07)
  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/pack.rs (target/debug/deps/pack-c81bffdb36d1cb5a)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/refusals.rs (target/debug/deps/refusals-ecc47c6e3342518b)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.32s
Running tests/scan.rs (target/debug/deps/scan-72ef2660ffb8f8c1)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running unittests src/lib.rs (target/debug/deps/minos_store-f91087a99a88b128)
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/contract.rs (target/debug/deps/contract-0ccd6f1ff8a87df3)
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.71s
Running tests/file_blobs.rs (target/debug/deps/file_blobs-7940e65c52fed831)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
Running tests/file_ledger.rs (target/debug/deps/file_ledger-498662032decf0ee)
  test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.29s
Running unittests src/main.rs (target/debug/deps/minos_vmm-04be6e1974430353)
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Doc-tests minos_backend_krun
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_cli
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_core
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_guest
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_loop
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_propose
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_runner
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_sandbox
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_source
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_store
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The ignored tests are the VM suites (`exit` 4, `vm` 10), the Docker test, and hand-run fixtures; the VM suites and the Docker test run below.

4. `cargo deny check`: exit 0. Last line and every warning header:

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
warning[duplicate]: found 2 duplicate entries for crate 'nix'
warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'syn'

advisories ok, bans ok, licenses ok, sources ok
```

The seven `duplicate` warnings predate Phase 5 (multiple-versions stays `warn`, Phase 1).

5. `cargo doc --workspace --no-deps`: exit 0, no warning or error line.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
   Generated ~/Repos/minos/target/doc/minos_backend_krun/index.html and 12 other files
```

6. `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

### C-01 and C-02

C-01, `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`: eleven `ok` lines, no `MISSING` line.

```text
ok crates/minos-backend-krun/Cargo.toml
ok crates/minos-cli/Cargo.toml
ok crates/minos-core/Cargo.toml
ok crates/minos-guest/Cargo.toml
ok crates/minos-loop/Cargo.toml
ok crates/minos-propose/Cargo.toml
ok crates/minos-runner/Cargo.toml
ok crates/minos-sandbox/Cargo.toml
ok crates/minos-source/Cargo.toml
ok crates/minos-store/Cargo.toml
ok crates/minos-vmm/Cargo.toml
```

C-02, `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: printed nothing, grep exit 1 (0 bytes of output).

### Build, sign, guest

`cargo build -p minos-vmm -p minos-cli`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

`cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`: exit 0.

```text
    Finished `release` profile [optimized] target(s) in 0.05s
```

`cargo test -p minos-backend-krun --test vm --test exit --no-run` (built before signing, so no VM run below relinks the helper): exit 0.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
  Executable tests/exit.rs (target/debug/deps/exit-c99de225879ade1b)
  Executable tests/vm.rs (target/debug/deps/vm-69052a93a1f98219)
```

`codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`: exit 0.

```text
target/debug/minos-vmm: replacing existing signature
```

`codesign --display --entitlements - target/debug/minos-vmm`: exit 0. Re-checked before each VM run below (two `[Key]` lines each time).

```text
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

### `minos host doctor` (CLI-03)

`target/debug/minos host doctor`: exit 0, no refusal.

```text
binaries (fixed locations; no configuration file or environment variable):
  found     ~/Repos/minos/target/debug/minos-vmm  a regular file
  rejected  ~/Repos/minos/target/debug/minos-guest  not an ELF file (the macOS build of minos-guest is Mach-O)
  found     ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest  a static AArch64 ELF
  kernel    /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib  checked below

checks:
ok       helper present  a regular file of 18009536 bytes
  checked: ~/Repos/minos/target/debug/minos-vmm
ok       helper entitlements  both entitlements are set to true
  checked: ~/Repos/minos/target/debug/minos-vmm
  checked: /usr/bin/codesign
ok       guest kernel  sha256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  checked: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
ok       hypervisor  kern.hv_support 1, hw.ncpu 16, hw.memsize 51539607552; largest VM 8 vCPUs and 16384 MiB, leaving half the host's memory to the host
  checked: /usr/sbin/sysctl
ok       job socket path  41 of at most 103 bytes
  checked: /private/tmp/minos-501/mnsXXXXXXXX/o.sock
ok       sandbox-exec present  a regular file
  checked: /usr/bin/sandbox-exec
ok       Seatbelt denies network  the control connected; under the profile nc exited 1 and the listener accepted nothing: no stderr
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       Seatbelt denies unlisted files  the listed file read back; the unlisted read exited 1: cat: /private/tmp/mnpIc8Wm3r9/unlisted: Operation not permitted
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       guest binary  a static aarch64 ELF of 1488864 bytes
  checked: ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest

confinement: hardware, seatbelt, rlimits
dedicated user: missing (deferred, reduced hardening)

job sweep: /private/tmp/minos-501
  no job directories left over
builder: docker buildx v0.35.0-desktop.2, daemon 29.6.1 (/Applications/Docker.app/Contents/Resources/bin/docker)

result: every check passed
```

The `rejected` line is the locator skipping the Mach-O `target/debug/minos-guest` before it finds the musl build; it is not a refusal.

### VM runs

Environment for every VM run: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`, plus `MINOS_EXIT_CANARY=$(uuidgen)` for `--test exit`. `MINOS_EXIT_HOST_IP` was unset, so the LAN target of `exit_egress` did not run (the test says so).

Fast smoke first, `cargo test -p minos-backend-krun --test exit -- --ignored --exact exit_env --nocapture --test-threads=1`: exit 0.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/exit.rs (target/debug/deps/exit-c99de225879ade1b)

running 1 test
test exit_env ... == candidate: ended Exited after 527.558792ms
== out/env.json: {"cmdline":"reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792445288058807000  --","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":["Max cpu time              61                   61                   seconds   ","Max file size             1073741824           1073741824           bytes     ","Max core file size        0                    0                    bytes     ","Max processes             512                  512                  processes ","Max open files            4096                 4096                 files     ","Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
== the canary appears nowhere in env.json
== the host HOME appears nowhere in env.json
== the host USER appears nowhere in env.json
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 1.40s
```

Functional suite, `cargo test -p minos-backend-krun --test vm -- --ignored --nocapture --test-threads=1`: exit 0, 10 passed.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/vm.rs (target/debug/deps/vm-69052a93a1f98219)

running 10 tests
test apply_mismatch_is_rejected_and_runs_nothing ... == candidate: ended Exited after 366.683083ms
ok
test candidate_output_never_carries_a_scorecard ... == candidate: ended Exited after 372.074625ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
ok
test experiment_applies_diff_and_runs_as_candidate_uid ... == candidate: ended Exited after 381.222459ms
== out/build.json: {"cmdline":"reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792445308619726000  --","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":["Max cpu time              61                   61                   seconds   ","Max file size             1073741824           1073741824           bytes     ","Max core file size        0                    0                    bytes     ","Max processes             512                  512                  processes ","Max open files            4096                 4096                 files     ","Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
== out/run.json: {"exists":true,"path":"/scratch/src/src/value.txt"}
ok
test judge_role_returns_a_scorecard ... == judge: ended Exited after 365.492834ms
ok
test output_16_mib_is_reassembled ... == candidate: ended Exited after 692.4715ms
== output status Packed { files: 2, content_bytes: 16000000 }
== 16 MiB output: 2 files, 16000000 content bytes, launch elapsed 692.4715ms
ok
test output_over_cap_is_refused_by_the_guest ... == candidate: ended Exited after 376.074166ms
ok
test over_budget_candidates_are_stopped_in_the_vm ... == candidate: ended Exited after 486.658459ms
== out/a.json: {"allocated_mib":764,"error":"allocation refused","requested_mib":832}
== RLIMIT_AS allowance 768 MiB: the probe allocated 764 of 832 MiB
== candidate: ended Exited after 360.986958ms
== out/s.json: {"os_error":11,"requested":2000,"started":511}
== RLIMIT_NPROC 512: the probe started 511 of 2000 children
== candidate: ended Exited after 442.729ms
== out/f.json: {"bytes_written":134213632,"os_error":28,"path":"/scratch/tmp/minos-probe-fill"}
== scratch 128 MiB: the probe wrote 134213632 bytes before a write failed
ok
test stage_timeout_is_reported_by_the_guest ... == candidate: ended Exited after 2.365203667s
== a 2 s run budget: the launch took 2.365203667s
ok
test two_sequential_experiments_share_no_build_output ... == candidate: ended Exited after 346.9165ms
== out/m.json: {"exists":true,"path":"/scratch/src/target/marker"}
== candidate: ended Exited after 346.323792ms
== out/m.json: {"exists":false,"path":"/scratch/src/target/marker"}
ok
test writes_outside_scratch_fail ... == candidate: ended Exited after 365.10775ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.18s
```

Exit suite, `cargo test -p minos-backend-krun --test exit -- --ignored --nocapture --test-threads=1`: exit 0, 4 passed.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/exit.rs (target/debug/deps/exit-c99de225879ade1b)

running 4 tests
test exit_egress ... == LAN target: not run (MINOS_EXIT_HOST_IP is unset)
== candidate: ended Exited after 346.92575ms
== out/egress.json: {"attempts":[{"ok":false,"os_error":101,"target":"127.0.0.1:59734"},{"ok":false,"os_error":99,"target":"[::1]:59735"},{"ok":false,"os_error":101,"target":"1.1.1.1:443"},{"ok":false,"os_error":101,"target":"8.8.8.8:53"},{"ok":false,"os_error":99,"target":"[2606:4700:4700::1111]:443"}],"net_ifaces":["lo"],"virtio_devices":[{"device":"0x0004","name":"virtio0"},{"device":"0x0003","name":"virtio1"},{"device":"0x0002","name":"virtio2"},{"device":"0x0002","name":"virtio3"},{"device":"0x0013","name":"virtio4"},{"device":"0x4d47","name":"virtio5"}]}
== connect 127.0.0.1:59734: connected false, errno Some(101)
== connect [::1]:59735: connected false, errno Some(99)
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
test exit_env ... == candidate: ended Exited after 361.907542ms
== out/env.json: {"cmdline":"reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1792445331186840000  --","cwd":"/scratch/src","egid":64000,"euid":64000,"gid":64000,"groups":[],"limits":["Max cpu time              61                   61                   seconds   ","Max file size             1073741824           1073741824           bytes     ","Max core file size        0                    0                    bytes     ","Max processes             512                  512                  processes ","Max open files            4096                 4096                 files     ","Max address space         805306368            805306368            bytes     "],"uid":64000,"vars":{"HOME":"/scratch/home","LANG":"C","PATH":"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","TMPDIR":"/tmp"}}
== the canary appears nowhere in env.json
== the host HOME appears nowhere in env.json
== the host USER appears nowhere in env.json
ok
test exit_forge ... == candidate: ended Exited after 349.163583ms
== out/w.json: {"inside":[{"ok":true,"os_error":null,"target":"/tmp/minos-probe-write"},{"ok":true,"os_error":null,"target":"/out/minos-probe-write"}],"outside":[{"ok":false,"os_error":30,"target":"/minos-probe-write"},{"ok":false,"os_error":30,"target":"/etc/minos-probe-write"},{"ok":false,"os_error":13,"target":"/root/minos-probe-write"},{"ok":false,"os_error":30,"target":"/usr/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/judge/minos-probe-write"},{"ok":false,"os_error":2,"target":"/scratch/candidate/minos-probe-write"}],"reads":[{"ok":false,"os_error":2,"target":"/proc/1/environ"},{"ok":false,"os_error":2,"target":"/dev/vdb"}]}
== open /proc/1/environ: opened false, errno Some(2)
== open /dev/vdb: opened false, errno Some(2)
== candidate: ended ChunkProtocol after 368.589291ms
== candidate: console tail
r interface
[    0.023278] rtc-pl031 a001000.rtc: registered as rtc0
[    0.023331] rtc-pl031 a001000.rtc: setting system clock to 2026-10-19T21:28:52 UTC (1792445332)
[    0.023410] i2c_dev: i2c /dev entries driver
[    0.023472] pps_ldisc: PPS line discipline registered
[    0.023712] NET: Registered PF_INET6 protocol family
[    0.023828] Segment Routing with IPv6
[    0.023870] In-situ OAM (IOAM) with IPv6
[    0.023923] NET: Registered PF_PACKET protocol family
[    0.023977] Bridge firewalling registered
[    0.024016] can: controller area network core
[    0.024074] NET: Registered PF_CAN protocol family
[    0.024128] can: raw protocol
[    0.024176] can: broadcast manager protocol
[    0.024241] can: netlink gateway - max_hops=1
[    0.024346] NET: Registered PF_KCM protocol family
[    0.024481] NET: Registered PF_VSOCK protocol family
[    0.024900] NET: Registered PF_TSI protocol family
[    0.024971] NET: Registered PF_TSI6 protocol family
[    0.025035] NET: Registered PF_TSIU protocol family
[    0.025333] registered taskstats version 1
[    0.025906] Btrfs loaded, zoned=yes, fsverity=no
[    0.026029] Key type encrypted registered
[    0.036580] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036711] alg: No test for 842 (842-scomp)
[    0.036781] alg: No test for 842 (842-generic)
[    0.038609] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.039826] clk: Disabling unused clocks
[    0.039871] PM: genpd: Disabling unused power domains
[    0.039915] ALSA device list:
[    0.039953]   No soundcards found.
[    0.052419] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.052607] devtmpfs: mounted
[    0.052731] Freeing unused kernel memory: 2496K
[    0.052834] Run /init as init process
[    0.052875]   with arguments:
[    0.052914]     /init
[    0.052947]   with environment:
[    0.052987]     HOME=/
[    0.053019]     TERM=linux
[    0.053051]     KRUN_BOOT_START_NS=1792445332781704000
MINOS-GUEST-BOOT-OK

== judge: ended Exited after 344.202875ms
ok
test exit_kill9 ... launcher: 
launcher: running 1 test
launcher: test exit_kill9 ... 
launcher: HELPER_PID=57900 JOB_DIR=/private/tmp/mvt4TTBNN/minos-501/mnsr8PVTNI4
== launcher 57891 killed: signal: 9 (SIGKILL)
== the helper 57900 exited 279.638083ms after the launcher's SIGKILL
== sweep: [Removed { dir: "/private/tmp/mvt4TTBNN/minos-501/mnsr8PVTNI4" }]
== removed /private/tmp/mvt4TTBNN
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.84s
```

The same exit suite with libtest capturing output, `cargo test -p minos-backend-krun --test exit -- --ignored --test-threads=1`, so each result is on its own line: exit 0.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running tests/exit.rs (target/debug/deps/exit-c99de225879ade1b)

running 4 tests
test exit_egress ... ok
test exit_env ... ok
test exit_forge ... ok
test exit_kill9 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.90s
```

No `/private/tmp/mvt*` directory remained after either exit run. The six virtio devices `exit_egress` lists are rng (0x0004), console (0x0003), block twice (0x0002, the root image and the input disk), vsock (0x0013), and 0x4d47, which is msb_krun's own VM-generation-ID device: `msb_krun_devices` 0.1.41 `src/virtio/vmgenid/mod.rs` line 20, `pub const VIRTIO_ID_MSB_VMGENID: u32 = 0x4d47;`. It is neither a network (0x0001) nor a filesystem (0x001a) device, and `net_ifaces` is exactly `["lo"]`. The guest `/proc/cmdline` carries the one `KRUN_BOOT_START_NS=<digits>` token, the recorded exception the user chose at the 05-19 checkpoint on 2026-10-19; every other token matches exactly.

### Docker env build (KRUN-09, CLI-02; the manual-only item)

`MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest cargo test -p minos-cli --test env_build docker_env_build_reproduces -- --ignored --nocapture --test-threads=1`: exit 0, 1 passed. The equal digests come from BuildKit's layer cache as 05-18 recorded; the packed image reproduces whenever the builder's rootfs contents reproduce.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running tests/env_build.rs (target/debug/deps/env_build-e701da1cb81a0218)

running 1 test
test docker_env_build_reproduces ... first image:  41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
second image: 41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b
minos.lock:
v = 1

[[image]]
backend = "krun"
arch = "arm64"
root_image = "41cfea1bed5b65fa796a5632081d24eca361ec3395d56c2a245e5e772be9276b"
dockerfile_sha256 = "0181ab39fcf9a471656dc2fa53d78b19e45e045e98d57022ab41168159050d34"
base_images = ["docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6"]
guest_sha256 = "b56895000b3e7b2b358ba241dcac74e8022d501944bea24be1ed7b6edfc93c65"

[image.builder]
name = "docker-buildx"
version = "v0.35.0-desktop.2"

ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 6.92s
```

Signature after the last cargo command above: `codesign --display --entitlements - target/debug/minos-vmm` still lists both keys.

### cargo clean (GATE-06)

Every step above exited 0, so `cargo clean` ran next, as the last cargo command of the phase; no cargo command ran after it.

```
$ cargo clean
     Removed 165992 files, 17.3GiB total
```

cargo clean: exit 0, run after the final gate (GATE-06)
