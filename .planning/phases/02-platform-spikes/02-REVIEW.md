---
phase: 02-platform-spikes
reviewed: 2026-10-10T16:22:40Z
depth: standard
files_reviewed: 33
files_reviewed_list:
  - .cargo/config.toml
  - Cargo.toml
  - crates/minos-backend-krun/Cargo.toml
  - crates/minos-backend-krun/clippy.toml
  - crates/minos-backend-krun/src/initramfs.rs
  - crates/minos-backend-krun/src/input_disk.rs
  - crates/minos-backend-krun/src/launch_config.rs
  - crates/minos-backend-krun/src/lib.rs
  - crates/minos-backend-krun/src/rootfs.rs
  - crates/minos-backend-krun/src/supervise.rs
  - crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile
  - crates/minos-backend-krun/tests/spike_boot.rs
  - crates/minos-backend-krun/tests/spike_exits.rs
  - crates/minos-backend-krun/tests/spike_net.rs
  - crates/minos-backend-krun/tests/spike_rootimg.rs
  - crates/minos-backend-krun/tests/spike_seatbelt.rs
  - crates/minos-guest/Cargo.toml
  - crates/minos-guest/src/job.rs
  - crates/minos-guest/src/lib.rs
  - crates/minos-guest/src/linux.rs
  - crates/minos-guest/src/main.rs
  - crates/minos-guest/src/pattern.rs
  - crates/minos-guest/src/report.rs
  - crates/minos-vmm/Cargo.toml
  - crates/minos-vmm/clippy.toml
  - crates/minos-vmm/minos-vmm.entitlements
  - crates/minos-vmm/minos-vmm.sb
  - crates/minos-vmm/src/config.rs
  - crates/minos-vmm/src/launch.rs
  - crates/minos-vmm/src/main.rs
  - crates/minos-vmm/src/preflight.rs
  - crates/minos-vmm/src/watchdog.rs
  - rust-toolchain.toml
findings:
  critical: 1
  warning: 7
  info: 7
  total: 15
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-10-10T16:22:40Z
**Depth:** standard
**Files Reviewed:** 33
**Status:** issues_found

## Summary

Static review (no `cargo` was run, per the phase rule) of the Phase 2 spike code: the host side of the libkrun backend (`minos-backend-krun`: root-image packer, job input disk, launch config, supervisor), the per-VM launcher (`minos-vmm`: config validation, preflight, VM assembly, watchdog, Seatbelt profile), the PID 1 guest (`minos-guest`), and the workspace configuration. Facts the code depends on were checked against the pinned crate sources in the local registry: `msb_krun` 0.1.41 `DiskBuilder` does default to `DiskImageFormat::Raw` (`builders.rs:1382`), `init_path` is formatted unquoted as `init={init}` (`vm.rs:933`), `ExitHandle::trigger` writes the exit eventfd, rustix `waitpid(None, ..)` is the process-group form so the guest's use of `wait` is correct, and tar 0.4.46 buffers GNU long names and pax headers with `read_all` and no cap (`archive.rs:418,440`).

The code is in good shape for spike code: no `unwrap`/`expect`/indexing/`as` casts/`unsafe`/`#[allow]` anywhere, caps are applied before reads throughout, errors are typed, iteration order is stable, and the invariants listed in CLAUDE.md (raw disks only, no virtio-fs, no inet hijack, env cleared, argv arrays, guest bytes behind a type with no `Debug`/`Display`) hold in the files reviewed. The known items (8 KiB vsock limit, known-failing `spike_net` tests, `disable-library-validation`, `dummy.numdummies=0`, ignored integration tests) were not re-reported.

One finding is classified Critical: the supervisor re-opens the console log by path after the helper exits and follows symlinks, which lets a Seatbelt-confined (compromised) VMM use the unconfined supervisor to read any host file into the "console" bytes. The warnings are mostly Phase 5 hardening points the spike code will carry forward: the display allowlist misses Unicode format characters, `status_frame` lets guest strings out as a bare `serde_json::Value`, the process group is created but never killed, the root-image packer's memory is bounded only by a 4 GiB default, `PackError::Io` conflates hostile-input parse errors with host I/O failure, the guest's `run` step has no timeout and can be held open by a grandchild, and the `init_path` parameter smuggling is guarded only by an ignored test.

## Critical Issues

### CR-01: Supervisor follows symlinks when reading the helper-writable console log (confused deputy around the Seatbelt layer)

**File:** `crates/minos-backend-krun/src/supervise.rs:565`, `crates/minos-backend-krun/src/supervise.rs:850-863`
**Issue:** After the helper exits, `run` calls `read_capped_file(files.console(), ..)`, which does `File::open(path)` and reads up to the cap. `File::open` follows symlinks. `console.log` is one of exactly two paths the Seatbelt profile lets the VMM write (`minos-vmm.sb:11`, `file-write*` on the `CONSOLE_LOG` literal, which includes creating any node type at that path). A VMM that has been compromised from the guest and is confined by the profile cannot read `~/.ssh/id_ed25519` itself, but it can `unlink` its console file and `symlink` that path to it; the unconfined supervisor then reads the target's first `console_cap` bytes into `GuestBytes`, and `excerpt()` makes them printable in the run's evidence or report. The same pattern applies to `vmm.stdout` and `vmm.stderr` (re-opened by path at lines 566-567) if a future profile ever makes the job directory writable. This is exactly the kind of hole the Seatbelt layer exists to close ("Local must be at least as strict as Lambda"), and CLAUDE.md names file names from a sandbox as hostile input. Precondition is a prior VMM compromise, but the second layer is there for that case and the fix is a few lines, so it must not reach Phase 5 as-is.
**Fix:**
```rust
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

/// Opens a helper-written regular file without following a symlink, and refuses anything that is not a
/// regular file. The fstat after open closes the lstat-then-open window.
fn open_helper_file(path: &Path) -> io::Result<Option<File>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32) // or libc::O_NOFOLLOW
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let meta = file.metadata()?;
    if !meta.file_type().is_file() || meta.nlink() != 1 {
        return Err(io::Error::new(ErrorKind::InvalidData, "helper file is not a plain regular file"));
    }
    Ok(Some(file))
}
```
(`OFlags::NOFOLLOW.bits()` is a `u32` that fits `i32`; if `rustix` is not wanted in this crate, add `libc` and use `libc::O_NOFOLLOW`, which is bindings, not compiled C.) Use it in `read_capped_file`, and make `write_disks`/`run` create the stdout, stderr, and (ideally) console files themselves with `create_new` so the helper only ever gets an fd, never the right to create the path. The Seatbelt profile should then grant `file-write-data` (not `file-write*`) on `CONSOLE_LOG`.

## Warnings

### WR-01: The display allowlist passes Unicode format characters (`Cf`) that `char::is_control` does not cover

**File:** `crates/minos-backend-krun/src/supervise.rs:775-779`
**Issue:** `displayable` keeps everything that is not `is_control()` (category `Cc` only) and not one of the eight Trojan Source bidi controls. That leaves through U+2028/U+2029 (line and paragraph separators, which split a log line in many terminals and log viewers), U+200B–U+200D (zero-width space/joiners, which hide characters inside identifiers and hashes), U+200E/U+200F and U+061C (LRM/RLM/ALM bidi marks), U+2060–U+2064, U+FEFF, U+180E, U+00AD (soft hyphen), and the Unicode tag characters U+E0000–U+E007F. The function is the one display exit for every guest byte, and the stack decision describes it as an allowlist that fails closed; today it is a denylist with gaps.
**Fix:** Reject the whole `Cf` class explicitly, plus the two separators:
```rust
fn displayable(c: char) -> bool {
    if matches!(c, '\n' | '\t') { return true; }
    !(c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}' | '\u{E0000}'..='\u{E007F}'))
}
```
and add the new characters to `supervise_excerpt_strips_controls_and_bidi_overrides`. Phase 5's core renderer should own this table so both sides use one list.

### WR-02: `GuestBytes::status_frame` returns an unvalidated `serde_json::Value`, so guest strings leave the wrapper with `Debug` and `Display`

**File:** `crates/minos-backend-krun/src/supervise.rs:739-771`
**Issue:** `status_frame` validates the 20-byte header, the length, and that the payload is an object with `v == 1`, then hands back the whole `serde_json::Value`. Every string inside it (`stdout`, `stderr`, `error`, `cmdline`, `env`, `target`, …) is guest-controlled, uncapped in length (up to the 1 MiB frame), and now has `Display`/`Debug` (which escape only `< 0x20`, not C1 or bidi) outside the `GuestBytes` discipline the module header promises ("the bytes leave only through … a validating decoder"). The spike tests cope by wrapping the output in their own `printable()`; production callers will not. This is the shape of the `Untrusted<T>` rule in CLAUDE.md: the exit must be a parser into a closed type.
**Fix:** Deserialize into a closed, `#[serde(deny_unknown_fields)]` report type mirroring `minos-guest::report` (shared through `minos-core` in Phase 5), with per-string caps enforced in `TryFrom` (for example 64 KiB per captured stream, 4 KiB per error string) and strings stored behind a sanitizing newtype whose only accessor is the allowlist renderer. Until then, make `status_frame` `pub(crate)` or document it as spike-only so it does not become the Phase 5 API by default.

### WR-03: The helper is spawned in its own process group but only the leader is ever killed

**File:** `crates/minos-backend-krun/src/supervise.rs:517`, `crates/minos-backend-krun/src/supervise.rs:543`, `crates/minos-backend-krun/src/supervise.rs:787`
**Issue:** `run` sets `.process_group(0)` and the stack decision (Q10) names `kill_process_group` as the kill primitive, but both the deadline path and `Reaper::drop` call `Child::kill`, which signals only the leader pid. Today `sandbox-exec` execs in place and `msb_krun` uses threads, so nothing leaks; the first time Phase 5 wraps the helper in anything that forks (a `_minos`-user launcher, `sudo`, a future msb_krun worker process) the deadline will leave a VM running with the user's hardware and the job disks open, and `killed_at_deadline` will say it was stopped.
**Fix:**
```rust
use rustix::process::{Pid, Signal, kill_process_group};

fn kill_group(child: &Child) {
    if let Some(pid) = Pid::from_child(child) {
        // The group id equals the leader pid because of process_group(0).
        kill_process_group(pid, Signal::Kill).ok();
    }
}
```
Call it before `child.0.wait()` at the deadline and in `Reaper::drop` (then `kill()` the leader as well for the already-reaped case). Add `rustix` (workspace dep, `process` feature) to `minos-backend-krun`.

### WR-04: Root-image packing holds the entire rootfs in memory with a 4 GiB default cap, and tar long names are buffered before the path cap applies

**File:** `crates/minos-backend-krun/src/rootfs.rs:57`, `crates/minos-backend-krun/src/rootfs.rs:415`, `crates/minos-backend-krun/src/rootfs.rs:606-608`
**Issue:** `PackLimits::default()` allows 4 GiB of tar and 1 GiB per entry. Every regular file becomes an `Rc<[u8]>` in `Tree`, and `write_image` then writes the whole SquashFS into a `Cursor<Vec<u8>>`, so a Dockerfile output near the default caps needs 4 GiB plus the compressed image resident in the `minos` CLI. Separately, the `path_bytes` cap (4096) is enforced in `normalize` only after the tar crate has already read a GNU long-name or pax record into memory: tar 0.4.46 `Archive::entries` calls `EntryFields::read_all()` on `GNULongName`, `GNULongLink`, and `XHeader` entries with no size limit (`archive.rs:418,429,440`), so one hostile header can declare up to `tar_bytes` of "name" and the packer will allocate it before rejecting the path. The module header says caps apply "before anything is read"; for names that is not true. The tar is the user's own environment build, so this is robustness rather than isolation, but a wedged or OOM-killed CLI is the wrong failure for hostile input.
**Fix:** Lower the defaults to what a v1 environment needs (for example 1 GiB tar, 256 MiB per entry, 65 536 entries) and state in the doc comment that peak memory is bounded by `tar_bytes` plus the image. For the long-name case, pre-check the header before letting the crate consume it: wrap the reader so that any entry whose header type is `GNULongName`/`GNULongLink`/`XHeader`/`XGlobalHeader` and whose declared size exceeds `path_bytes` (or a small pax cap such as 64 KiB) is refused by the `Counted` reader (track the current header and return `InvalidData`), or iterate with `archive.entries_with_seek()` over a file and inspect raw headers first. Phase 5 should stream file data to a temp file rather than `Rc<[u8]>`.

### WR-05: `PackError::Io` conflates malformed-tar parse errors with host I/O failure

**File:** `crates/minos-backend-krun/src/rootfs.rs:239-241`, `crates/minos-backend-krun/src/rootfs.rs:402-403`, `crates/minos-backend-krun/src/rootfs.rs:428`, `crates/minos-backend-krun/src/rootfs.rs:676`, `crates/minos-backend-krun/src/rootfs.rs:693-703`
**Issue:** The tar crate reports a corrupt header, a bad octal size, a checksum mismatch, or a truncated archive as `io::Error` (`InvalidData`/`UnexpectedEof`), and `read_data` itself synthesizes `UnexpectedEof`. All of these flow through `#[from] std::io::Error` into `PackError::Io`, the same variant a failing disk read or a full `/tmp` produces. CLAUDE.md requires library errors to separate "the input did something wrong" from "infrastructure failed" because the former must never be retried; a caller matching on `PackError` cannot tell them apart today.
**Fix:** Keep `Io` for the reader and writer, but intercept the tar crate's data errors:
```rust
#[error("rootfs tar is malformed: {what}")]
MalformedTar { what: &'static str },

fn tar_io(error: io::Error) -> PackError {
    match error.kind() {
        io::ErrorKind::InvalidData => PackError::MalformedTar { what: "invalid header or field" },
        io::ErrorKind::UnexpectedEof => PackError::MalformedTar { what: "archive ended early" },
        _ => PackError::Io(error),
    }
}
```
and apply it at `entries()?`, `entry?`, `header().size()?`, `header.mode()?`, `header.uid()?`, `header.gid()?`, and in `read_data`. Add a test that feeds a truncated tar and expects `MalformedTar`, not `Io`.

### WR-06: The guest's `run` step has no timeout, and `capture` blocks until every holder of the pipe exits

**File:** `crates/minos-guest/src/linux.rs:371-393`, `crates/minos-guest/src/linux.rs:395-457`
**Issue:** `run_command` spawns the program, then `capture` reads each pipe to EOF and `child.wait()` blocks indefinitely. Two consequences for a hostile or merely sloppy command: (1) a program that sleeps forever stops the job with no status frame, so the host only learns `TimedOut` at its own deadline and loses the results of every earlier step; (2) a program that forks a daemon which inherits stdout and exits itself keeps the pipe open, so `capture` never returns even though the direct child is gone, and `reap_orphans` never runs because it is after the join. Phase 5 needs per-step budgets, and the structure here (blocking `read_to_end` on the pipe, then `wait`) cannot be given one without a rewrite, so it is worth getting right now while the step set is small.
**Fix:** Spawn with `.process_group(0)`, read both pipes with non-blocking fds and a deadline (poll with `try_wait` the way `supervise::run` does), and on expiry `kill_process_group` then `wait`:
```rust
let deadline = Instant::now() + step.timeout();           // add `timeout_ms` (1..=max_duration) to RawRun
let child = Command::new(..).process_group(0).spawn()?;
// non-blocking reads via rustix::io::fcntl_setfl(fd, OFlags::NONBLOCK), poll both pipes until EOF or deadline
if Instant::now() >= deadline {
    kill_process_group(Pid::from_child(&child).., Signal::Kill).ok();
    timed_out = true;
}
let status = child.wait()?;   // the leader is dead or exited; EOF on the pipes follows once the group is gone
reap_orphans();
```
Record `timed_out: bool` in `RunResult`. Also call `reap_orphans()` at the top of every step, not only after `run`, so a daemon left behind by one step is reaped before the next.

### WR-07: `root=/dev/vda` is smuggled through `KernelBuilder::init_path`, and the only guard is an ignored integration test

**File:** `crates/minos-vmm/src/launch.rs:27`, `crates/minos-vmm/src/launch.rs:61`
**Issue:** `ROOT_DISK_INIT = "/init root=/dev/vda"` works because msb_krun 0.1.41 writes `init={init}` with no quoting (`vm.rs:933`) and Linux keeps the last `root=`. `init_path` takes `impl AsRef<Path>`; a future msb_krun release that validates the path (no whitespace), percent-encodes it, or moves `init=` before the user `cmdline` would silently boot with `root=/dev/root` and `rootfstype=squashfs` against a non-existent device, and every VM would fail to boot. The assertion that pins the exact `/proc/cmdline` tokens (`spike_boot.rs:358`) is `#[ignore]`d and runs only by hand with a signed helper, so `cargo test` and the gate cannot catch the regression; the `=0.1.41` pin is the real protection, and a deliberate bump is exactly when someone will not think to re-run the spike.
**Fix:** Make the dependency explicit and testable without a VM: add a unit test in `minos-vmm` that builds the kernel cmdline the way msb_krun does (or, better, expose and call msb_krun's `build_kernel_cmdline` if the pinned fork accepts a small PR adding `KernelBuilder::root_device`) and asserts the last `root=` token is `/dev/vda`. At minimum, add a `const _: () = assert!(..)`-style check that `ROOT_DISK_INIT` contains exactly one space and starts with `/init`, and a comment in `Cargo.toml` next to the `msb_krun` pin: "bumping requires re-running `spike_boot::boot_root_disk` (S01-CMDLINE)". Phase 5 should promote `boot_root_disk` to the VM test target that runs before any release.

## Info

### IN-01: Declared dependencies that no source file uses

**File:** `crates/minos-guest/Cargo.toml:14-16`, `crates/minos-backend-krun/Cargo.toml:14-15`
**Issue:** `minos-guest` declares `minos-core` and `tar`; `minos-backend-krun` declares `minos-core` and `minos-sandbox`. No `use`/path reference to any of the four exists in those crates (`grep -rn 'minos_core\|minos_sandbox\|tar::'` finds only `rootfs.rs`'s own `tar` use in the backend). `deny.toml`'s `unused = "deny"` only checks that each workspace dependency is used by *some* member, so these dead edges pass the gate. They are harmless now but they mean the dependency-direction table in CLAUDE.md is asserted by `deny.toml` rather than by real code, and `tar` in the guest adds code to the static `/init` only if something later references it.
**Fix:** Remove them until the Phase 5 code that needs them lands, or add `unused_crate_dependencies = "warn"` to `[workspace.lints.rust]` so the gate (`-D warnings`) refuses the next one.

### IN-02: `LaunchError::Watchdog` exits 70, which the exit-code table reserves for "msb_krun refused to build or start the VM"

**File:** `crates/minos-vmm/src/main.rs:16`, `crates/minos-vmm/src/main.rs:98`
**Issue:** A failed `thread::Builder::spawn` is a host resource failure, not an msb_krun refusal, but it maps to 70 and the module doc does not mention it. Phase 5's classification rule (CLAUDE.md, "pre-takeover codes 64, 65, 66, 67, and 70") treats 70 as infrastructure either way, so classification is unaffected; the table is just wrong.
**Fix:** Add "or the watchdog thread could not start" to the 70 row, or give it its own code (71) and a table row.

### IN-03: The entitlement check needs `</key><true/>` to be byte-adjacent

**File:** `crates/minos-backend-krun/src/supervise.rs:87`, `crates/minos-backend-krun/src/supervise.rs:408-411`
**Issue:** `entitled` is a substring match for `<key>com.apple.security.hypervisor</key><true/>`. `codesign --xml` currently emits compact XML, but any whitespace or newline between the two elements (a different macOS build, a different plist writer) yields `false` for a correctly signed helper. It fails closed (the spike tests refuse to run), so it is not a security issue, but it will present as "the helper lost its entitlement" after an OS update and cost someone an afternoon.
**Fix:** Tolerate whitespace: search for `<key>com.apple.security.hypervisor</key>` and then skip ASCII whitespace before requiring `<true/>`; or parse with a minimal plist/XML reader.

### IN-04: The watchdog deadline starts at `watchdog::spawn`, not at `main` entry

**File:** `crates/minos-vmm/src/watchdog.rs:114`
**Issue:** `start = Instant::now()` is taken after the config read, preflight (which hashes the 10+ MB libkrunfw dylib), and `launch::build`. `max_duration_ms` therefore bounds VM run time, not helper lifetime, and the two differ by however long preflight takes on a cold disk. The parent's host deadline is authoritative, so this is only a drift between the two clocks.
**Fix:** Take `Instant::now()` in `main` next to `parent_id()` and pass it to `spawn` alongside `parent`, so both watchdog inputs are recorded as the first statement of `main`.

### IN-05: `Launch` does not relate `host_deadline` to the config's `max_duration_ms`

**File:** `crates/minos-backend-krun/src/supervise.rs:448-451`
**Issue:** `Launch::host_deadline` is independent of the `LaunchConfig` the caller wrote. If the host deadline is shorter than `max_duration_ms` plus boot and shutdown time, the supervisor SIGKILLs the helper before the watchdog's 124 and the console file is never flushed; the run is still classified `TimedOut` via `killed_at_deadline`, so the outcome is right, but the evidence (console tail, `minos-vmm: watchdog deadline` marker) is lost. Every spike test sets the pair by hand.
**Fix:** Derive the host deadline from the config: `Launch::for_config(&config)` sets `host_deadline = max_duration + GRACE` (for example 30 s) and the explicit setter asserts `deadline > max_duration`.

### IN-06: The guest mounts `/proc` and `/sys` with no restriction flags

**File:** `crates/minos-guest/src/linux.rs:44-45`, `crates/minos-guest/src/linux.rs:94-98`
**Issue:** `mount_pseudo` passes `MountFlags::empty()`, so both are read-write and allow suid/dev/exec. Inside the VM this cannot affect the host, but a candidate `run` command can write `/sys/power/state`, `/proc/sysrq-trigger`, or `/proc/sys/*` and turn a scored failure into an unexplained power-off or hang that the host then classifies as `Crashed`/`TimedOut`. Phase 5 wants failures to be attributable.
**Fix:** `MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC` on both, and `MountFlags::RDONLY` on sysfs; mount a `tmpfs` on `/tmp` with a size cap at the same time, since `MOUNT_POINTS` already reserves it.

### IN-07: `supervise::run` resolves a relative `argv[0]` through the supervisor's own `PATH`

**File:** `crates/minos-backend-krun/src/supervise.rs:495`, `crates/minos-backend-krun/src/supervise.rs:511-513`
**Issue:** `Command::new(program)` with `env_clear()` still performs `PATH` lookup for a relative program name using the *parent's* `PATH` (std resolves before `execvp` with the parent's environment). All callers today pass absolute paths, but the API accepts anything `Into<OsString>`, and `EmptyArgv` is the only argv check. A relative `minos-vmm` would run whatever a writable directory earlier in the user's `PATH` provides.
**Fix:** Refuse a non-absolute program in `run`:
```rust
if !Path::new(program).is_absolute() {
    return Err(SuperviseError::RelativeProgram { program: program.into() });
}
```

---

_Reviewed: 2026-10-10T16:22:40Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
