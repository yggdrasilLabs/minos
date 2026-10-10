---
phase: 02-platform-spikes
fixed_at: 2026-10-10T16:56:59Z
review_path: .planning/phases/02-platform-spikes/02-REVIEW.md
iteration: 1
findings_in_scope: 8
fixed: 8
skipped: 0
status: all_fixed
---

# Phase 02: Code Review Fix Report

**Fixed at:** 2026-10-10T16:56:59Z
**Source review:** .planning/phases/02-platform-spikes/02-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 8 (CR-01, WR-01 to WR-07; `fix_scope: critical_warning`, so IN-01 to IN-07 were not attempted)
- Fixed: 8
- Skipped: 0

Every fix was verified against the claim in the code first, then gated with `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, and the C-02 `#[allow]` grep before each commit; `cargo deny check` and `cargo doc --workspace --no-deps` ran at the end (deny reports the same seven warn-level duplicate-version entries as `main`; doc is clean after the follow-up commit). The `#[ignore]`d VM spikes were not run. `.planning/config.json` was left out of every commit. Backend unit tests went from 59 to 70, minos-vmm from 46 to 48.

Work was done in an isolated worktree on a temp branch and fast-forwarded onto `main` (`aab5c2c` -> `7bab78f`); the worktree, temp branch, and recovery sentinel were removed afterwards.

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

## Skipped Issues

None.

---

_Fixed: 2026-10-10T16:56:59Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
