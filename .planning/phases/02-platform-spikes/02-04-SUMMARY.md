---
phase: 02-platform-spikes
plan: 04
subsystem: infra
tags: [rust, squashfs, backhand, tar, newc, initramfs, docker, buildx, determinism, golden-vector]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: static musl guest (/init payload, SHA-256 10e59803...3a89), backend-krun module layout, backhand/tar/sha2/base16ct edges in Cargo.lock"
provides:
  - "rootfs::pack_root_image: untrusted rootfs tar plus trusted guest bytes become a gzip SquashFS root image, in memory, under caps"
  - "rootfs::minimal_root_image with golden SHA-256 8e4f12d45a06ef4bf20fe0f37e74ca06e2b7b108a8c4325ed2c7f6a6600e4e98"
  - "initramfs::initramfs_image: newc archive holding only /init, golden SHA-256 6b2629df82fc94f9798c3853a508935c6a448a7e48fabd58e4e730a99148ad1a for b\"hello\""
  - "crates/minos-backend-krun/clippy.toml banning tar unpacking and backhand's clock setter, each ban proven"
  - "tests/spike_rootimg.rs produce helper and ignored produce_twice: digest-pinned Dockerfile -> buildx tar -> SquashFS image"
affects: [02-05, 02-06, 02-11, phase-05-env-build]

tech-stack:
  added: []
  patterns:
    - "Hostile archive parsing: counting take(cap + 1), cap checked whatever the parse returned, size checked from the header before data is read"
    - "Validated tree in a BTreeMap keyed by normalized path; implicit parents created in sorted order; trusted /init inserted last"
    - "backhand errors mapped by exhaustive variant match, because its Display is empty without error-strings"
    - "Integration-test artifacts kept under CARGO_TARGET_TMPDIR so cargo clean removes them"

key-files:
  created:
    - crates/minos-backend-krun/clippy.toml
    - crates/minos-backend-krun/tests/spike_rootimg.rs
    - crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile
    - .planning/phases/02-platform-spikes/evidence/02-04.md
  modified:
    - crates/minos-backend-krun/src/rootfs.rs
    - crates/minos-backend-krun/src/initramfs.rs

key-decisions:
  - "PackLimits private fields are tar_bytes, entries, entry_bytes, path_bytes: clippy::struct_field_names rejects four fields sharing max_; the constructor parameters keep the E-2 names"
  - "File content in the image, counting each hardlink copy, is capped by max_tar_bytes (PackError::ContentTooLarge), so hardlinks cannot amplify a small tar"
  - "The per-entry cap compares max(header size, PAX size) because the tar crate lets a PAX size record override the header"
  - "Base image pinned to alpine:3 index digest sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 (Alpine 3.24.2)"
  - "D-16 result: the two buildx tars differ only in the RUN step's mtimes on /etc and /etc/minos-spike; the packed images are byte-identical"

patterns-established:
  - "Golden vectors frozen after one run, then re-run in a second test process before commit"
  - "Probe edits are appended to a committed file and removed with git checkout -- <file>"

# SPIKE-06 is answered only when the produced image boots (D-15, plan 02-06); SPIKE-01's boot is also 02-06.
# This plan supplies the image and the initramfs, so REQUIREMENTS.md is left unchanged.
requirements-completed: []

coverage:
  - id: D1
    description: "Root image packer: caps, path rules, entry-type rules, hardlink copies, mount points, trusted /init, determinism, golden vector"
    requirement: SPIKE-06
    verification:
      - kind: unit
        ref: "cargo test -p minos-backend-krun rootfs (21 tests; evidence/02-04.md U-PACK)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Tar unpacking and the clock setter are banned in minos-backend-krun, and each ban fires"
    requirement: SPIKE-06
    verification:
      - kind: other
        ref: "evidence/02-04.md C-03d, D-PROBE-BK (exit 101 with four distinct disallowed-method errors, exit 0 after revert)"
        status: pass
    human_judgment: false
  - id: D3
    description: "newc initramfs writer with golden vector, cross-checked by macOS bsdcpio"
    requirement: SPIKE-01
    verification:
      - kind: unit
        ref: "cargo test -p minos-backend-krun initramfs (4 tests)"
        status: pass
      - kind: other
        ref: "evidence/02-04.md IR-01 (cpio -itv lists one init, -rwxr-xr-x; extracted init SHA-256 equals the guest)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Digest-pinned arm64 Dockerfile becomes a SquashFS root image through buildx tar export and the Rust packer, twice"
    requirement: SPIKE-06
    verification:
      - kind: integration
        ref: "MINOS_GUEST=... MINOS_DOCKER=... cargo test -p minos-backend-krun --test spike_rootimg -- --ignored --nocapture --test-threads=1 (evidence/02-04.md RI-03)"
        status: pass
    human_judgment: false
  - id: D5
    description: "The produced image boots and runs a command from the image (D-15)"
    requirement: SPIKE-06
    verification: []
    human_judgment: true
    rationale: "Booting is plan 02-06's job; this plan only produces and inspects the image"

duration: 12min
completed: 2026-10-10
---

# Phase 2 Plan 04: Root Image and Initramfs Writers, Image Production Summary

**A capped, fail-closed Rust packer turns a hostile rootfs tar plus the trusted musl guest into a byte-deterministic gzip SquashFS root image. A hand-rolled newc writer agrees with macOS cpio. A digest-pinned alpine Dockerfile goes through buildx tar export and the packer twice, and both runs give the same image even though the two tars differ.**

## Performance

- **Duration:** about 12 min
- **Started:** 2026-10-10T13:21:55Z
- **Completed:** 2026-10-10T13:34Z
- **Tasks:** 3
- **Files modified:** 6 (4 created, 2 modified)

## Accomplishments

- `rootfs.rs` reads the tar entry by entry in memory and never unpacks it. Input passes through `take(max_tar_bytes + 1)` and a byte counter, and the counter is checked whatever the parse returned, so input past the cap is always `TarTooLarge` and never a silent truncation.
  - Before any data is read it checks the entry count, the path length, and the declared size. A test with a header claiming 1 MiB and no data gets `EntryTooLarge`, not an I/O error.
  - Paths are normalized component by component. One leading `.` and a trailing slash are allowed. `..`, absolute paths, NUL, non-UTF-8, empty or interior `.` components, and names over 255 bytes are refused.
  - Device nodes, FIFOs, sparse files, and any other entry type are refused. A hardlink becomes a copy of an earlier regular file (`Rc` shared, so no extra memory).
  - The packer creates `/dev`, `/proc`, `/sys`, `/mnt`, and `/tmp` (mode 1777). A non-directory at any of them is refused.
  - Anything the tar put at `init` or under `init/` is removed, and `/init` is written from the trusted bytes (0755, uid 0, gid 0). `init_overridden` reports when this happened.
  - The writer uses gzip only, root mode 0755, and mtime 0 everywhere, and adds nodes in `BTreeMap` order.
- Golden vector: `minimal_root_image(b"minos-golden-init")` gives `8e4f12d45a06ef4bf20fe0f37e74ca06e2b7b108a8c4325ed2c7f6a6600e4e98`, stable across two test processes.
- `initramfs.rs` writes two records, `init` (0100755, nlink 1, all else 0) and `TRAILER!!!`, padded to 4 bytes and then to 512. Golden vector for `b"hello"`: `6b2629df82fc94f9798c3853a508935c6a448a7e48fabd58e4e730a99148ad1a`. For the guest it produces a 1,022,464-byte archive. bsdcpio 3.5.3 lists one `init`, `-rwxr-xr-x`, root/wheel, mtime 0, and extracts an `init` whose SHA-256 equals the guest (`10e59803...3a89`).
- `clippy.toml` bans `tar::Archive::unpack`, `tar::Entry::unpack`, `tar::Entry::unpack_in`, and backhand's `FilesystemWriter::set_current_time`. All four paths resolve. The probe gave exit 101 with four distinct errors, and the re-check after `git checkout --` gave exit 0.
- SPIKE-06 production:
  - Base: `alpine:3` pinned to its index digest, `sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6` (Alpine 3.24.2).
  - Build: buildx v0.35.0-desktop.2 on `desktop-linux` (BuildKit v0.31.1, Docker 29.6.1, daemon already running).
  - Image: 4,456,448 bytes from an 8,942,592-byte tar. It holds 85 files, 96 dirs, and 335 symlinks, with no hardlinks and no setid files.
  - Checks: `/etc/minos-spike` reads `minos-spike-06\n` and `/init` is the guest.
- D-16:
  - **Tars differ.** The only difference is 6 bytes: the mtime and checksum of `etc/` and `etc/minos-spike`, which the `RUN` step stamped 13:31:31 and 13:31:34 UTC.
  - **Images match:** `d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae` both times, and again when the first tar was re-packed.
- Engines: the Docker CLI with BuildKit was checked. lima, podman, and Apple's container tool were not checked. The `orbstack` and `podman` Docker contexts exist but their engines are down; they were not touched.

## Task Commits

1. **Task 1: Root image packer and clippy bans:** RED `f960095` (test), GREEN `cd743ed` (feat), evidence `4bb7649` (docs). No refactor commit.
2. **Task 2: newc initramfs writer:** RED `61d6a54` (test), GREEN `48774bb` (feat), evidence `3b607eb` (docs).
3. **Task 3: SPIKE-06 production:** `3f6cfe4` (feat; fixture, test, and RI-01 to RI-04 evidence).

## Files Created/Modified

- `crates/minos-backend-krun/clippy.toml`: E-1, four `disallowed-methods` entries
- `crates/minos-backend-krun/src/rootfs.rs`: `PackLimits`, `PackReport` (Serialize), `PackedImage`, `PackError`, `pack_root_image`, `minimal_root_image`, 21 tests
- `crates/minos-backend-krun/src/initramfs.rs`: `initramfs_image`, `InitramfsError`, 4 tests plus the ignored cpio cross-check writer
- `crates/minos-backend-krun/tests/spike_rootimg.rs`: `produce` and the ignored `produce_twice`
- `crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile`: E-4, the only file in the build context
- `.planning/phases/02-platform-spikes/evidence/02-04.md`: U-PACK, C-03d, D-PROBE-BK, IR-01, RI-01 to RI-04

## Decisions Made

See `key-decisions` above. Also:
- `PackError` carries hostile paths as lossy UTF-8, cut to 256 characters, and prints them with `{:?}`, which escapes control and bidi characters.
- Setuid and setgid bits are kept and counted (`setid_files`). Phase 5 decides whether to strip them.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `PackLimits` field names**
- **Found during:** Task 1 (crate clippy)
- **Issue:** E-2 names the private fields `max_tar_bytes`, `max_entries`, `max_entry_bytes`, `max_path_bytes`. `clippy::struct_field_names` (pedantic, `-D warnings`) rejects four fields with a common prefix, and the no-`allow` rule leaves no way to keep the names.
- **Fix:** The fields are now `tar_bytes`, `entries`, `entry_bytes`, `path_bytes`. `PackLimits::new` keeps the E-2 parameter names and order, and `Default` keeps the E-2 values. The fields are private, so no caller sees the change.
- **Files modified:** crates/minos-backend-krun/src/rootfs.rs
- **Commit:** cd743ed

**2. [Rule 2 - Critical] A content cap that counts hardlink copies**
- **Found during:** Task 1
- **Issue:** Hardlinks are copied (backhand has no hardlink API). Without a cap, 200,000 hardlinks to a 1 GiB file would ask the writer to compress 200 TB from a small tar (T-02-24).
- **Fix:** File content in the image, counting each copy, may not exceed `max_tar_bytes`. Exceeding it is `PackError::ContentTooLarge`, a variant beyond E-2's list, with its own test.
- **Commit:** cd743ed

**3. [Rule 1 - Bug] The entry-size check also covers a PAX size**
- **Found during:** Task 1 (reading tar 0.4.46 `archive.rs`)
- **Issue:** The tar crate lets a PAX `size` record override the header size. Checking `header().size()` alone, as the plan wrote it, would miss a size declared through PAX.
- **Fix:** The check compares `max(header().size()?, entry.size())` against `max_entry_bytes` before reading. Exactly `entry.size()` bytes are then read, and a short read is an error.
- **Commit:** cd743ed

**4. [Rule 2 - Critical] Extra path and tree rules**
- **Found during:** Task 1
- **Fix:** On top of the plan's rules, the packer also refuses an empty or interior `.` component, a name over 255 bytes (the SquashFS name limit), and an entry whose parent in the tree is not a directory. It removes a tar-supplied `init/` subtree along with `init`. All of these are `BadPath` reasons, and the `init/` case has a test.
- **Commit:** cd743ed

**5. [Rule 3 - Blocking] backhand errors have no text**
- **Found during:** Task 1 (reading backhand `error.rs`)
- **Issue:** With `error-strings` off (gzip-only feature set), `BackhandError`'s `Display` and `Debug` print nothing.
- **Fix:** `PackError::Squashfs { what }` names the variant through an exhaustive match. `StdIo` becomes `PackError::Io`, and no source is attached.
- **Commit:** cd743ed

**6. [TDD] The initramfs determinism test was strengthened in RED**
- **Found during:** Task 2 RED
- **Issue:** `initramfs_is_deterministic` passed against the stub, which returned an empty vector both times.
- **Fix:** The test also asserts that the archive contains the init bytes, so it failed in RED. The RED commit has all 4 tests failing.
- **Commit:** 61d6a54

**7. [Minor] Test-harness choices the plan left open**
- The ignored IR-01 writer reads the guest from `MINOS_GUEST` as well as the output path from `MINOS_SPIKE_OUT`.
- `produce` returns `(Vec<u8>, PackedImage)` and writes its tars under `CARGO_TARGET_TMPDIR` (`target/tmp/spike_rootimg`), so RI-04 could inspect them and `cargo clean` removes them.
- `PATH` skips the repeated `/usr/local/bin`.
- **Commits:** 61d6a54, 3f6cfe4

## Known Stubs

None in this plan's files. `input_disk.rs`, `launch_config.rs`, and `supervise.rs` still hold one `//!` line each; plan 02-05 fills them.

## Things to Know (for 02-05, 02-06, and Phase 5)

- **02-06** boots `produce(...)`'s image. Copy the helper or call it from the boot test, and keep its `env_clear`/`PATH` rules.
- The image depends only on the tar's content, not on its mtimes. The current fixture gives `d1e93ef0...a4ae` with guest `10e59803...3a89`; a rebuilt guest changes it.
- **Memory:** the packer holds all file data and the finished image in memory, so peak memory is about tar content plus image size. The tar crate buffers GNU long-name and PAX records with no limit of its own; only the total tar cap bounds them. Phase 5 should give those records a smaller cap of their own (parse with `entries().raw(true)`) and decide whether to stream the image to a file.
- The initramfs is negative-evidence input only (D-18: stock libkrunfw has no `CONFIG_BLK_DEV_INITRD`).
- `cargo clean` was not run, per the plan; 02-11 runs it.

## Issues Encountered

None beyond the deviations above. Docker was already running, and no credential-helper error occurred.

## User Setup Required

None.

## Next Phase Readiness

- 02-05 can proceed: it touches only `input_disk.rs`, `launch_config.rs`, and `supervise.rs` in this crate.
- 02-06 has the root image path (SquashFS on `/dev/vda` per D-18) and the initramfs for the negative-evidence run.
- Gate: `cargo fmt --all --check`, workspace clippy, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`, and musl clippy all exit 0. C-01 prints ten `ok` lines. The C-02, unsafe, and `mksquashfs|"cpio"` greps print nothing. No Cargo.toml, Cargo.lock, or deny.toml changed (205 packages).

## Self-Check: PASSED

- All 7 created or modified files exist on disk.
- Commits f960095, cd743ed, 4bb7649, 61d6a54, 48774bb, 3b607eb, 3f6cfe4, and 96438a6 are present in `git log`.
