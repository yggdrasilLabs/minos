---
phase: 02-platform-spikes
fixed_at: 2026-10-10T17:50:16Z
review_path: .planning/phases/02-platform-spikes/02-REVIEW.md
iteration: 2
findings_in_scope: 15
fixed: 15
skipped: 0
status: all_fixed
---

# Phase 02: Code Review Fix Report

**Fixed at:** 2026-10-10T17:50:16Z
**Source review:** .planning/phases/02-platform-spikes/02-REVIEW.md
**Iteration:** 2

**Summary:**
- Findings in scope: 15 (CR-01 and WR-01 to WR-07 in iteration 1 under `fix_scope: critical_warning`; IN-01 to IN-07 in iteration 2 under `fix_scope: all`)
- Fixed: 15
- Skipped: 0

Every fix was verified against the claim in the code first, then gated with `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, and the C-02 `#[allow]` grep before each commit; `cargo deny check` and `cargo doc --workspace --no-deps` ran at the end of each iteration, and `cargo deny check` again after the one Cargo.toml change (IN-01). The `#[ignore]`d VM spikes were not run in either iteration. `.planning/config.json` was left out of every commit. Across both iterations backend unit tests went from 59 to 74 and minos-vmm from 46 to 50.

Each iteration was done in an isolated worktree on a temp branch and fast-forwarded onto `main` (iteration 1: `aab5c2c` -> `7bab78f`; iteration 2: `9b08c2f` -> `4b659e2`); the worktree, temp branch, and recovery sentinel were removed afterwards.

## Fixed Issues

### CR-01: Supervisor follows symlinks when reading the helper-writable console log

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`, `crates/minos-backend-krun/Cargo.toml`, `Cargo.lock`
**Commit:** 1683453
**Applied fix:** Verified the claim: `read_capped_file` did `File::open(path)` on `console.log`, `vmm.stdout`, and `vmm.stderr` after the helper exited, and the Seatbelt profile grants `file-write*` on the console literal. `vmm.stdout` and `vmm.stderr` are now read back through `try_clone`d handles the supervisor created with `create_new` (rewound, never re-opened by path). The console, which only the helper creates, is opened with `O_NOFOLLOW` (`rustix::fs::OFlags::NOFOLLOW`, converted to the `custom_flags` form behind a const assert) and read only if `fstat` on the open descriptor shows a regular file with `nlink == 1`; `ELOOP` and any other node type become the new `SuperviseError::NotPlainFile`. `rustix` (already a workspace dependency, bindings not compiled C) was added to `minos-backend-krun`; `cargo deny check` stays clean. Tests: a console the helper wrote is read; a symlink, a hardlink, and a directory at the console path are each refused.

Deliberately not applied from the suggestion: pre-creating the console with `create_new` and narrowing the profile to `file-write-data`. `minos-vmm`'s preflight (`console_absent`) refuses a pre-existing console file, and a profile change cannot be validated here without running the Seatbelt VM spike. Both remain available for Phase 5; the `O_NOFOLLOW` plus `fstat` check closes the hole on its own.

### WR-01: The display allowlist passes Unicode format characters that `char::is_control` does not cover

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** d5c4500 (doc-link follow-up in 7bab78f)
**Applied fix:** `displayable` now also rejects U+00AD, U+061C, U+180E, U+200B to U+200F, U+2028 to U+202E, U+2060 to U+206F, U+FEFF, U+FFF9 to U+FFFB, and U+E0000 to U+E007F, as suggested. The `excerpt` doc lists the classes. A new test feeds one character from every range and expects them all dropped while printable non-ASCII, combining marks, and emoji survive. Phase 5 should move this table into the core renderer so both sides use one list, as the review notes.

### WR-02: `GuestBytes::status_frame` returns an unvalidated `serde_json::Value`

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** 143c802
**Applied fix:** The five spike test files navigate the frame by field name at about sixty call sites, so replacing the JSON exit with a closed report type now would mean rewriting VM tests that cannot be run here; that typed mirror is a Phase 5 item, as the review itself proposes. Instead the decoder now rebuilds the tree before returning it: a string value over `MAX_FRAME_STRING_BYTES` (64 KiB) is refused with `FrameError::StringTooLong`; every kept string is passed through the `excerpt` allowlist (so the value's `Display`/`Debug` cannot emit an escape or a bidi trick); every object key must be at most `MAX_FRAME_KEY_BYTES` (64) and already displayable, else `FrameError::BadKey`. The method doc states that this JSON exit exists for the spike evidence and that Phase 5 replaces it with a `deny_unknown_fields` report type shared through `minos-core`. Tests cover sanitizing nested strings, an over-cap string, an exact-cap string, and bad keys at the top level and nested. The only spike assertion on a string value compares `stdout` to `minos-spike-06\n`, which the allowlist leaves unchanged.

### WR-03: The helper is spawned in its own process group but only the leader is ever killed

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** a4bd119
**Applied fix:** Verified: both the deadline path and `Reaper::drop` called `Child::kill`. A `kill_group` helper sends `SIGKILL` with `rustix::process::kill_process_group(Pid::from_child(..), Signal::KILL)` (treating `ESRCH` as already empty) and then kills the leader. The deadline path uses it before `wait`; `Reaper::drop` uses it only while `try_wait` reports the leader unreaped, so a reused pid is never signalled. The new test backgrounds a grandchild `sleep` under a leader that `exec`s another `sleep`, hits a 200 ms deadline, then probes the group with signal 0 until `ESRCH`; it was confirmed to fail against a leader-only kill before the fix was restored.

### WR-04: Root-image packing holds the entire rootfs in memory with a 4 GiB default cap, and tar long names are buffered before the path cap applies

**Files modified:** `crates/minos-backend-krun/src/rootfs.rs`
**Commit:** e77ea89 (doc-link follow-up in 7bab78f)
**Applied fix:** Verified against tar 0.4.46 source (`archive.rs` `next_entry`: `EntryFields::read_all` on `L`, `K`, and `x` entries with no cap). `PackLimits::default()` is now 1 GiB of tar, 200,000 entries, 256 MiB per entry, 4,096 bytes per path, and the module doc states the peak-memory bound and the Phase 5 streaming follow-up. A `HeaderGuard` reader sits under the byte counter and tracks the 512-byte headers the way the crate does (octal or base-256 size field, data padded to whole blocks, all-zero block ends the archive); an `L`/`K` header declaring more than the path cap plus one (the trailing NUL the crate strips) or an `x`/`g` header over `PAX_HEADER_BYTES` (64 KiB) is refused from the header alone, before the crate allocates, surfacing as `PackError::ExtensionHeaderTooLarge`. The doc records the two cases the guard does not mirror (a pax `size` override, which only a file over 8 GiB needs, and GNU sparse entries, which `read_tar` refuses anyway) and that after either the guard can only be stricter than the crate. Tests: each header kind over its cap is refused with the declared size; a 4095-byte GNU long name and a pax `path` record within the cap are accepted and land in the image; a regular file with a base-256 size is followed correctly so the next entry still parses.

### WR-05: `PackError::Io` conflates malformed-tar parse errors with host I/O failure

**Files modified:** `crates/minos-backend-krun/src/rootfs.rs`
**Commit:** 761e34b
**Applied fix:** Verified, with one correction to the review's sketch: tar 0.4.46 reports a checksum mismatch, a non-numeric field, and a block that ended early as `ErrorKind::Other` (its `other()` helper), not `InvalidData`, so a kind-based mapper would have missed them. The split is by `raw_os_error()` instead: an error carrying an errno is `PackError::Io` (infrastructure), anything the parser synthesized is the new `PackError::MalformedTar { what }` with the parser's message, lossy and cut to 256 characters and shown escaped. `#[from]` was removed from `Io` so every `io::Error` on the tar path is sorted explicitly through `tar_io` (applied at `entries()`, `entry`, `size()`, `mode()`, `uid()`, `gid()`, and `read_data`); the squashfs writer's own `StdIo` still maps to `Io`. Tests: a cut inside entry data, a cut inside a header, and a bad checksum are `MalformedTar`; a reader that fails with `EIO` is `Io`.

### WR-06: The guest's `run` step has no timeout, and `capture` blocks until every holder of the pipe exits

**Files modified:** `crates/minos-guest/src/job.rs`, `crates/minos-guest/src/report.rs`, `crates/minos-guest/src/linux.rs`, `crates/minos-backend-krun/tests/spike_rootimg.rs`
**Commit:** 38886a6
**Applied fix:** The `run` step now requires `timeout_ms` (1 to `MAX_RUN_TIMEOUT_MS` = 3,600,000, the launcher's `max_duration_ms` ceiling), validated in `Run::try_from` with a `timeout()` accessor; `RunResult` gains `timed_out: bool` after `signal` (the golden field-order test is updated). In `linux.rs` the command is spawned with `process_group(0)`; both pipes are made non-blocking with `rustix::fs::fcntl_setfl(.., OFlags::NONBLOCK)` and drained by a `Capture` that keeps up to the cap and discards the rest; the loop polls `try_wait` every 5 ms, stops when the leader exits (a grandchild holding a pipe no longer holds the step), and at the deadline kills the group, waits, and sets `timed_out`. Stray group members are killed after every run (the guest forks nothing between the reap and the kill), and `reap_orphans()` now runs before every step as well as after a run. The only spike job with a `run` step (`DOCKERFILE_JOB`) gains `"timeout_ms":30000`. Host-compilable code stayed out of `#[cfg(target_os = "linux")]`; the Linux path was verified with the musl clippy gate and a release build of the static guest, since it cannot execute here. **Wire-format note:** the job's `run` object has a new required field, so the 02-01-PLAN.md step table (which this run did not edit) is now one field behind.

### WR-07: `root=/dev/vda` is smuggled through `KernelBuilder::init_path`, and the only guard is an ignored integration test

**Files modified:** `crates/minos-vmm/src/launch.rs`, `Cargo.toml`, `CLAUDE.md`
**Commit:** eea8a5c
**Applied fix:** Verified against msb_krun 0.1.41 `api/vm.rs` (`"{default}{ user_cmdline}{ debug} root=/dev/root init={init}"`) and `msb_krun_vmm` `kernel_cmdline.rs` (the non-x86 default). Two unit tests in `minos-vmm` mirror that assembly with the pinned default text and assert that the last `root=` token is `/dev/vda`, the last `rootfstype=` is `squashfs`, `init=/init` is a token, `ro` follows the default's `rw`, that `ROOT_DISK_INIT` splits into exactly `/init` and `root=/dev/vda`, and that no `root=` or `init=` hides in `ROOT_DISK_CMDLINE`. The `ROOT_DISK_INIT` doc names the two msb_krun facts it rides on and the VM spike that proves them; the `msb_krun` pin in `Cargo.toml` carries a comment that a bump requires re-running `spike_boot::boot_root_disk` (S01-CMDLINE). CLAUDE.md's sentence claiming the boot test stops a bump from changing this silently was corrected in the same commit, since that test is ignored. The msb_krun default cmdline is hardcoded in the test because the crate does not re-export it and `minos-vmm` must not link `msb_krun_vmm` directly.

## Fixed Issues (iteration 2: Info findings)

The line numbers cited in REVIEW.md for these findings predate the iteration 1 commits; each site was located by description and the claim re-verified against the current code before anything was changed. Every one of the seven was confirmed as described; none was judged not-a-bug.

### IN-01: Declared dependencies that no source file uses

**Files modified:** `crates/minos-guest/Cargo.toml`, `crates/minos-backend-krun/Cargo.toml`, `Cargo.lock`
**Commit:** 756c838
**Applied fix:** Verified: `grep` finds no `minos_core`, `minos_sandbox`, or `tar` reference in `minos-guest`, and no `minos_core` or `minos_sandbox` reference in `minos-backend-krun` (its `tar` use in `rootfs.rs` is real); iteration 1 did not make any of the four live (CR-01 added `rustix`, WR-06 touched `job.rs`, `report.rs`, and `linux.rs` only). The robust option, `unused_crate_dependencies = "warn"` in `[workspace.lints.rust]`, was tried first and cannot pass the `-D warnings` gate today: both clippy commands stop at `minos-core`, whose one-line placeholder `lib.rs` declares nine Phase 3 dependencies (`serde`, `serde_json`, `toml`, `thiserror`, `sha2`, `base16ct`, `diffy`, `rand_chacha`, `rand_core`), and `store`, `sandbox`, `propose`, `runner`, `loop`, and `cli` are placeholders in the same way; stripping all of those would then trip `deny.toml`'s `[bans.workspace-dependencies] unused = "deny"` on the workspace entries nothing would use any more, a restructuring well beyond this finding. So the fallback was taken, as the task allowed: the four dead edges are removed and the lint is not added. The `deny.toml` `wrappers` entries that allow these edges were deliberately kept, because they encode the dependency-direction table in CLAUDE.md that Phase 4 and 5 code will use (guest and backend on core, backend on sandbox); trimming them would make `cargo deny check` refuse exactly the edges those phases add. Consequence to know about: `cargo deny check` now prints three warn-level `unused-wrapper` lines ("wrapper for banned crate was not encountered") for `minos-core` -> `minos-backend-krun`, `minos-core` -> `minos-guest`, and `minos-sandbox` -> `minos-backend-krun`, on top of the seven duplicate-version warnings `main` already had; they disappear when the real dependencies land. `cargo deny check` still passes (`advisories ok, bans ok, licenses ok, sources ok`). No CLAUDE.md change: its dependency list states the allowed direction, which is unchanged.

### IN-02: `LaunchError::Watchdog` exits 70, which the exit-code table reserves for "msb_krun refused to build or start the VM"

**Files modified:** `crates/minos-vmm/src/main.rs`, `CLAUDE.md`
**Commit:** d9048f0
**Applied fix:** Verified: `exit_code` mapped `Build`, `Watchdog`, and `Enter` together to `EXIT_VMM` (70) and the table had no row for the watchdog. `LaunchError::Watchdog` now exits the new `EXIT_WATCHDOG` (71) with its own table row ("the watchdog thread could not be started (a host thread or memory limit), so the VM was never entered"). A unit test, `launch_error_codes_match_the_table`, pins every pre-takeover code the launcher can construct without an `msb_krun::Error` (64, 65 for both `Read` and `Config`, 66, 67, 71) and asserts 71 differs from 70. CLAUDE.md's classification sentence now reads "pre-takeover codes 64, 65, 66, 67, 70, and 71" in the same commit; the classification itself is unchanged (all are infrastructure failures).

### IN-03: The entitlement check needs `</key><true/>` to be byte-adjacent

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** db5eb40
**Applied fix:** Verified: `entitled` was a `windows()` search for the literal `<key>com.apple.security.hypervisor</key><true/>`. It now searches for the key element (`ENTITLED_KEY`) and, at each match, requires `<true/>` (`ENTITLED_TRUE`) after `trim_ascii_start` of the remainder, with no slicing (`get(..)` plus `unwrap_or_default`). `<false/>`, a `<string>` value, a later key set to true after this key set to false, and a key at the very end of the output (with or without a trailing newline) still read as unentitled. The existing test gained one assertion per case. A minimal plist parser was not written: the output is one `codesign` invocation's XML, and the whitespace-tolerant match is the smallest change that removes the brittleness the review named.

### IN-04: The watchdog deadline starts at `watchdog::spawn`, not at `main` entry

**Files modified:** `crates/minos-vmm/src/main.rs`, `crates/minos-vmm/src/watchdog.rs`, `crates/minos-vmm/src/config.rs`, `CLAUDE.md`
**Commit:** ad29d6b
**Applied fix:** Verified: `spawn` did `let start = Instant::now();` after the config read, preflight (which hashes the libkrunfw dylib), and `launch::build`. `main` now records `Instant::now()` as its second statement, directly after `parent_id()`, and passes both through `run(parent, started)` to `watchdog::spawn(.., parent, started)`, which no longer reads the clock. The `spawn` and `Trip::Deadline` docs and the `VmmConfig::max_duration` accessor doc say the limit is counted from `main` entry and bounds the helper's whole lifetime; a new test, `watchdog_deadline_counts_from_the_recorded_start`, hands `tripped` a start two seconds in the past and expects `Deadline` against a one-second limit and nothing against an hour. CLAUDE.md's sentence on what `minos-vmm` records first was reworded in the same commit to name both the parent PID and the start instant and to state that `max_duration_ms` counts from that instant. No spike test asserts `elapsed` against `max_duration_ms` (they only print it), so none needed a change.

### IN-05: `Launch` does not relate `host_deadline` to the config's `max_duration_ms`

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`, `crates/minos-backend-krun/src/launch_config.rs`, `crates/minos-backend-krun/tests/spike_boot.rs`, `crates/minos-backend-krun/tests/spike_exits.rs`, `crates/minos-backend-krun/tests/spike_net.rs`, `crates/minos-backend-krun/tests/spike_rootimg.rs`, `crates/minos-backend-krun/tests/spike_seatbelt.rs`
**Commit:** 3baae1d
**Applied fix:** Verified: `Launch::host_deadline` was a free `Duration` and every spike set the pair by hand. `LaunchConfig` gains a `max_duration()` accessor. `Launch::for_config(argv, files, listener, &config)` builds a launch whose host deadline is `max_duration_ms` plus the new `HOST_DEADLINE_GRACE` (30 s, documented as the time for the helper's own watchdog to exit 124, print its marker, and flush the console before the host gives up) and records `max_duration`; `run` then refuses, before creating any file, a launch whose explicit `host_deadline` is not longer than that `max_duration` with the new `SuperviseError::DeadlineTooShort { deadline, max_duration }`. One deviation from the suggestion's shape: the rejection lives in `run` rather than in the `host_deadline` setter, because the setter is a `const` builder used at about twenty call sites and `run` already returns `Result`; the effect is the same and the setter doc says so. `Launch::new` is unchanged and carries no `max_duration`, so a hand-built launch is not checked. All five VM spikes now build from the config they already write, keeping their explicit deadlines (each already exceeds its `max_duration_ms`: 30 s/60 s, 60 s/90 s, 60 s/120 s, 5 s/30 s, 120 s/150 s); `spike_exits`'s `Job` carries its `LaunchConfig` for that. They compile under the gate but, being `#[ignore]`d VM tests, were not run. Unit tests cover the derived deadline, the refusal at 1 s and at exactly 60 s (with no `vmm.stdout`/`vmm.stderr` created), and acceptance at 60.001 s with `/usr/bin/true`.

### IN-06: The guest mounts `/proc` and `/sys` with no restriction flags

**Files modified:** `crates/minos-guest/src/linux.rs`
**Commit:** 21d3ec9
**Applied fix:** Verified: `mount_pseudo` passed `MountFlags::empty()` for both. `/proc` and `/sys` are now mounted with `NOSUID | NODEV | NOEXEC` (`PSEUDO_FLAGS`, built with the `const` `union`), `/sys` additionally `RDONLY`, and a tmpfs is mounted on `/tmp` (which the packer's `MOUNT_POINTS` already creates, mode 1777) with `NOSUID | NODEV` and `size=25%` of guest memory. Two judgement calls, both documented in the constants' docs: `/proc` is not read-only, because programs legitimately write their own `/proc/self/*` entries and the review asked for `RDONLY` on sysfs only; and `/tmp` keeps exec, because build-time checks compile and run small programs in a temporary directory, while the size cap stops a command from filling guest memory through it. The mount helper became `mount_at(source, target, fstype, flags, data)`, and the module doc names the three mounts. This is Linux-only code and cannot execute here; it was verified with the musl clippy gate and a release build of the static guest (`ELF 64-bit LSB executable, ARM aarch64, statically linked`). The host-side guest tests (18) are unaffected. The boot report's `net_ifaces` and `virtio_devices` listings still read `/sys` (reads are unaffected by `RDONLY`), so the spikes that assert on them are not changed by this; re-running `spike_boot::boot_root_disk` with a rebuilt guest is the Phase 5 confirmation.

### IN-07: `supervise::run` resolves a relative `argv[0]` through the supervisor's own `PATH`

**Files modified:** `crates/minos-backend-krun/src/supervise.rs`
**Commit:** 4b659e2
**Applied fix:** Verified: `run` passed `program` straight to `Command::new` after only the `EmptyArgv` check, and std resolves a relative name through the parent's `PATH` regardless of `env_clear()`. `run` now returns the new `SuperviseError::RelativeProgram { program }` for any program that is not an absolute path, checked immediately after `EmptyArgv` and before the deadline check and any file creation; the `Launch::new` and `run` docs state the requirement. A unit test refuses `true`, `bin/true`, `./true`, and `../usr/bin/true` and confirms no `vmm.stdout`/`vmm.stderr` was created. Every existing caller passes an absolute path (`/usr/bin/true`, `/usr/bin/env`, `/bin/echo`, `/bin/cat`, the spikes' `MINOS_VMM`, which `env_path` requires absolute, and `/usr/bin/sandbox-exec`).

## Skipped Issues

None.

## Process note (iteration 2)

While applying IN-06 and IN-07 in one chained command, the musl clippy gate for IN-06 failed on `clippy::doc_markdown` (`SquashFS` in a new doc comment needed backticks) but the failure was masked by a pipe into `tail`, so a commit with a failing gate was made and IN-07 was committed on top of it. Both commits existed only on the temp branch; the branch was reset to the IN-05 commit, the doc comment corrected, and IN-06 and IN-07 re-applied and re-committed under a strict chain (`set -o pipefail`, unpiped clippy runs, negated `#[allow]` grep). The commits that reached `main` (`21d3ec9`, `4b659e2`) are the gated ones; the masked ones were discarded before the fast-forward.

---

_Fixed: 2026-10-10T17:50:16Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 2_
