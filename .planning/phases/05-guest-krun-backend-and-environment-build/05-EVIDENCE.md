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
