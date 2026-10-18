---
phase: 05-guest-krun-backend-and-environment-build
plan: 14
subsystem: minos-cli
tags: [cli, clap-builder, host-doctor, host-setup, codesign, locator, blob-source, cli-03, krun-06, krun-08, d-01, d-02, d-03, d-04, d-13, d-14]
status: complete
requires:
  - "05-05: FileStore::verified_path and Blobs::put_reader in minos-store"
  - "05-06: BlobSource, BlobSourceError, VerifiedPath in minos-sandbox"
  - "05-09: rootfs::check_guest_elf, the embedded Seatbelt profile the probes run"
  - "05-11: host::{probe_host, HostPaths, HostReport, Probe, ProbeOutcome, Check, signed_with_entitlements, entitlements_present, KRUNFW_PATH, REQUIRED_ENTITLEMENTS, CODESIGN, TOOL_PATH}, jobdir::{JobRoot, sweep, SweepEntry, JOB_ROOT_PARENT}"
provides:
  - "minos_cli (new library target): locate::{Candidate, Located, locate, host_paths, probe_paths, VMM_NAME, GUEST_NAME, GUEST_TARGET, GUEST_MAX_BYTES}"
  - "minos_cli::blob_source::FileBlobSource (open(project), impl BlobSource: get through Blobs::get, root_image through FileStore::verified_path)"
  - "minos binary: clap builder tree `minos host doctor | host setup`, main -> run() -> anyhow::Result<ExitCode>, errors printed as `minos: error: ...` through SafeText"
  - "host.rs (binary module): doctor, setup, setup_codesign_argv, DoctorReport, render_doctor, BuilderStatus, builder_status"
affects:
  - "05-15: KrunBackend is constructed from host_paths(&locate(current_exe)) and a FileBlobSource"
  - "05-16: `minos env build` adds its subcommand to cli() in main.rs and reuses locate for the guest; builder_status's docker discovery (PATH lookup, canonical binary, HOME plus the docker dirs and system dirs as PATH) is the RI-03 recipe env build also needs"
  - "05-20: records `minos host setup` and `minos host doctor` output on this Mac and marks CLI-03, KRUN-06, KRUN-08 complete after the final gate"
tech-stack:
  added: []
  patterns:
    - "minos-cli is a lib plus bin: reusable, anyhow-free parts in the `minos_cli` library; commands and anyhow in the binary"
    - "Doctor gathers a DoctorReport first and renders it with a pure function, so wording, order, and escape stripping are unit-tested without a host"
    - "A binary not found is probed at the last path the locator checked, so every other probe still runs and the refusal names that path"
    - "Host tools: absolute path, argv array, env_clear plus an explicit list, stdin null, capped stdout and stderr, own process group killed at the deadline"
key-files:
  created:
    - crates/minos-cli/src/lib.rs
    - crates/minos-cli/src/locate.rs
    - crates/minos-cli/src/blob_source.rs
    - crates/minos-cli/src/host.rs
    - crates/minos-cli/tests/host_cmd.rs
  modified:
    - crates/minos-cli/Cargo.toml
    - crates/minos-cli/src/main.rs
    - Cargo.lock
    - CLAUDE.md
decisions:
  - "minos-cli gains a library target (minos_cli) holding locate and FileBlobSource; host.rs stays a binary module so anyhow never leaves the binary (rules/proj-lib-main-split, rules/err-anyhow-app)"
  - "doctor probes a missing binary at the last path the locator checked instead of skipping probe_host, so one run shows every refusal"
  - "doctor exits 1 when the job root cannot be opened, as well as on any refused probe: nothing can launch without it, and doctor never falls back silently"
  - "setup exits 1 when the libkrunfw probe refuses, after signing and verifying the helper"
  - "The builder line is informational only: docker missing, buildx missing, or the daemon down never refuses a launch"
metrics:
  duration: 14min
  completed: 2026-10-18
  tasks: 2
  files: 9
---

# Phase 5 Plan 14: minos host doctor, host setup, the binary locator, and FileBlobSource Summary

`minos` now has a clap builder command tree with `host doctor`, which runs every krun launch probe and prints each check, its fix, and every path it looked at, and `host setup`, which signs `minos-vmm` with CLAUDE.md's documented `codesign` argv and checks both entitlement keys. The plan also adds a locator that finds binaries at fixed paths with no configuration, and a `FileBlobSource` over the project's `FileStore`.

## What was built

- **Command tree** (`src/main.rs`): `Command::new("minos")` with `subcommand_required` and `arg_required_else_help`, a `host` group with `doctor` and `setup`, and `cli().debug_assert()` in a unit test. The derive feature stays off (F-01). `main` returns `ExitCode`, and `run()` returns `anyhow::Result<ExitCode>`. Errors are printed as `minos: error: {error:#}` through `SafeText`.
- **Locator** (`minos_cli::locate`, D-04): checks `<dir>/minos-vmm` (must be a regular file), then `<dir>/minos-guest` (accepted only if `check_guest_elf` passes, read under a 64 MiB cap and stat-checked before open), then `<dir>/../aarch64-unknown-linux-musl/release/minos-guest`. libkrunfw is always `KRUNFW_PATH`. Every candidate is kept with its path, whether it was accepted, and a fixed reason; the Mach-O dev build reports "not an ELF file (the macOS build of minos-guest is Mach-O)". It reads no environment variable and no config file (`grep -c env::var` gives 0). `host_paths` returns `Some` only when both binaries were found. `probe_paths` falls back to the last checked path.
- **FileBlobSource** (`minos_cli::blob_source`, D-13, D-14): `open(project)` opens `<project>/.minos`. `get` goes through `Blobs::get` and maps BlobMissing, BlobTooLarge, and BlobTampered to Missing, TooLarge, and Tampered; Unsafe stays Unsafe; Io and anything else become Io. `root_image` calls `verified_path(&digest.to_blob_id())`, maps BlobMissing to `RootImageMissing` (its Display says "run minos env build"), and wraps the path with `VerifiedPath::new`.
- **host doctor** (`src/host.rs`, CLI-03, KRUN-06, D-01, D-03, D-04): runs `locate(current_exe)`, then `JobRoot::open` and `sweep`, then `probe_host(&probe_paths, JOB_ROOT_PARENT)`, then `builder_status`. `render_doctor` is pure and prints sections in this order:
  1. the binaries section, one found or rejected line per candidate, plus the kernel path
  2. one `ok` or `refused` line per check, each refusal followed by its `Display` fix, a `saw:` line, and `checked:` lines
  3. `confinement: hardware, seatbelt, rlimits`
  4. `dedicated user: missing (deferred, reduced hardening)`, plus a line for any other expected layer that is absent
  5. the sweep entries
  6. `builder: ...`
  7. a result line

  Every path and tool string passes through `SafeText::inline`. It exits 0 only when nothing failed.
- **host setup** (D-02, KRUN-08): writes `include_bytes!("../../minos-vmm/minos-vmm.entitlements")` to a `create_new` 0600 `entitlements.plist` in a 0700 tempdir. It runs `setup_codesign_argv` (`/usr/bin/codesign --force --sign - --options runtime --entitlements <copy> <vmm>`) with the environment cleared to `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, stdin null, capped output, and a 1-minute deadline. It then requires `signed_with_entitlements(vmm)` to be true and prints `signed:`, `verified:`, the libkrunfw probe line, and the builder line. Any failure gives a non-zero exit.
- **builder_status**: finds the first executable `docker` in an absolute directory on the user's PATH and runs its canonical path. The environment is cleared to `HOME` and a `PATH` of the docker dir, its canonical dir, and the system dirs (RI-03). It runs `docker buildx version`, then `docker info --format {{.ServerVersion}}`, each with a 10 s deadline. The result is Ready, NoBuildx, DaemonDown, or NotFound, each worded with a fix.

## Verification on this Mac

After `cargo build -p minos-cli -p minos-vmm` and `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`:

`target/debug/minos host setup` (exit 0):

```
signed: /Users/seanmcauliffe/Repos/minos/target/debug/minos-vmm
verified: com.apple.security.hypervisor, com.apple.security.cs.disable-library-validation
ok       guest kernel  sha256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  checked: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
builder: docker found at /Applications/Docker.app/Contents/Resources/bin/docker, daemon not running: start Docker Desktop before minos env build
```

`codesign --display --entitlements - target/debug/minos-vmm` (exit 0) lists `com.apple.security.cs.disable-library-validation` and `com.apple.security.hypervisor`, both `[Bool] true`.

`target/debug/minos host doctor` (exit 0): binaries found at `target/debug/minos-vmm` and `target/aarch64-unknown-linux-musl/release/minos-guest`. `target/debug/minos-guest` was rejected as Mach-O. All nine checks passed:

- helper present (18009536 bytes)
- helper entitlements
- guest kernel (pinned sha256)
- hypervisor (16 CPUs, 48 GiB; largest VM 8 vCPUs and 16384 MiB)
- job socket path (41 of 103 bytes)
- sandbox-exec present
- Seatbelt denies network
- Seatbelt denies unlisted files
- guest binary (static aarch64 ELF, 1488832 bytes)

Other lines in the output: `confinement: hardware, seatbelt, rlimits`, `dedicated user: missing (deferred, reduced hardening)`, `job sweep: /private/tmp/minos-501` with no job directories left over, the builder line as above, and `result: every check passed`. The Docker daemon really was down (`docker info` from the user's shell said the same), and buildx v0.35.0-desktop.2 is installed. The ignored `doctor_runs_on_this_mac` test passes.

Gates:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | 0 failures |
| `cargo deny check` | advisories, bans, licenses, and sources ok |
| `cargo doc --workspace --no-deps` | no warnings |
| `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | pass |
| C-01 | no MISSING line |
| C-02 | prints nothing |

## Tests

- **Unit, lib:**
  - `locate_installed_layout`
  - `locate_dev_checkout` (the Mach-O guest is listed with its reason)
  - `locate_missing` (every checked path is listed; `probe_paths` falls back)
  - `locate_rejects_a_guest_that_is_not_a_regular_file`
  - `file_blob_source_get` (under the cap, over it, absent)
  - `file_blob_source_root_image` (equals `verified_path`; absent is `RootImageMissing`)
- **Unit, bin:**
  - `render_doctor_lists_everything` (order asserted)
  - `render_doctor_strips_escapes` (ESC and BEL in a probe detail, checked paths, the binary path, the sweep dir, and the builder path)
  - `doctor_exit_code` (refusals, clear, sweep failure)
  - `setup_argv_is_documented`
  - `setup_entitlements_are_the_committed_plist` (byte equality and `entitlements_present`)
  - `buildx_version_picks_the_version_token`
  - `command_tree_is_well_formed`
- **Integration** (`tests/host_cmd.rs`):
  - `cli_help_lists_host_commands`
  - `doctor_without_binaries_refuses_and_names_every_path` (a bare copy of `minos` exits 1 with the helper and guest refused and all three candidate paths named)
  - `setup_without_helper_refuses`
  - `doctor_runs_on_this_mac` (ignored)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a `minos_cli` library target (`src/lib.rs`)**
- **Found during:** Task 1
- **Issue:** `FileBlobSource` and `host_paths` have no caller in the binary yet (the backend that takes them arrives in 05-15 and Phase 7). As private binary modules they would fail `-D warnings` on dead code.
- **Fix:** `src/lib.rs` exposes `pub mod locate; pub mod blob_source;`. `host.rs` stays a binary module, so `anyhow` stays in the binary, as rules/err-anyhow-app and the deny.toml wrapper intend.
- **Files modified:** crates/minos-cli/src/lib.rs (new)
- **Commit:** 65a145b, a3f336c

**2. [Rule 2 - Missing critical] doctor still runs every probe when a binary is missing**
- **Found during:** Task 2
- **Issue:** The plan said to build `VmmMissing` or `GuestMissing` directly and skip `probe_host` when a binary is not found. That would hide kernel, hypervisor, and Seatbelt refusals until a second run.
- **Fix:** `locate::probe_paths` passes the last checked path for a missing binary. `probe_host` then refuses with `VmmMissing` or `GuestMissing` naming that path, and every other probe still runs.
- **Commit:** 22e6d9c

**3. [Rule 2 - Missing critical] A job root that cannot be opened fails doctor**
- **Found during:** Task 2
- **Issue:** `JobRoot::open` can fail on a symlink, the wrong owner, or the wrong mode. Nothing can launch then, and printing it while exiting 0 would be a silent fallback.
- **Fix:** The failure is printed as `job sweep: failed: <error with its fix>` and counts toward exit 1.
- **Commit:** 22e6d9c

**4. [Rule 3 - Blocking] The signing flags are split into three constants**
- **Issue:** `rustfmt` laid the six-element flag array out vertically, which broke the acceptance grep `"--options", "runtime"`.
- **Fix:** The flags are now three constants: `SIGN_FLAGS`, `HARDENED_RUNTIME = ["--options", "runtime"]`, and `ENTITLEMENTS_FLAG`. The argv is unchanged, and `setup_argv_is_documented` pins it.
- **Commit:** 22e6d9c

**5. [Rule 3 - Blocking] main.rs doc wording**
- **Issue:** A doc comment quoting clap_derive's generated `#[allow(...)]` attribute matched the C-02 grep.
- **Fix:** Reworded as "an allow attribute for `clippy::restriction`".
- **Commit:** 22e6d9c

### Additions within scope

- `BuilderStatus::NoBuildx` sits alongside the three planned states, so a docker without buildx gets its own fix.
- Two integration tests run the real binary from a bare directory: doctor refuses and names every path, and setup refuses without a helper.
- CLAUDE.md Commands now says `host setup` runs the documented argv (the "may run it later" wording is gone), what doctor prints, and where the binaries are found. The Workspace line notes the CLI's library half.
- `rustix` was added to minos-cli for process-group kill at a tool's deadline; it is already in the workspace and in deny.toml's build-script allowlist. `tempfile` was added as the plan said.

## Requirements

CLI-03, KRUN-06, and KRUN-08 are implemented and verified on this Mac. They are not marked complete here, because 05-20 marks them, with the rest of Phase 5, only after the final gate passes (05-20-PLAN must_haves).

## Known Stubs

None.

## Threat Flags

None. The threat register's mitigations are all in place:

- **T-05-14-01:** the guest is accepted only after `check_guest_elf`.
- **T-05-14-02:** the embedded plist is pinned byte for byte, and both keys are verified after signing.
- **T-05-14-03:** the locator reads no environment variable.
- **T-05-14-04:** everything printed passes through `SafeText`.
- **T-05-14-05:** `verified_path` plus `VerifiedPath::new`.

`builder_status` reads the user's `PATH` and `HOME` only to find and run docker, which is informational and never selects a launch binary.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/lib.rs, locate.rs, blob_source.rs, host.rs, tests/host_cmd.rs
- FOUND commits: 65a145b, a3f336c, 45bcf46, 22e6d9c, 8c1ce04
