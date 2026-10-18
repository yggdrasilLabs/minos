---
phase: 05-guest-krun-backend-and-environment-build
plan: 21
subsystem: krun-backend
status: complete
tags: [minos-backend-krun, minos-vmm, launch-config, golden-file, spike-retirement]
requires:
  - phase: 05-03
    provides: Wave 0 proofs and the 05-EVIDENCE.md Mechanism selections (console: pipe)
  - phase: 05-07
    provides: minos-vmm config v2 and golden/config-v2.json pinned by SHA-256
provides:
  - Launch config v2 serializer bound byte for byte to crates/minos-vmm/golden/config-v2.json
  - Spike-era supervisor with no scratch image and no initramfs path
  - A tree with no test that drives the retired v1 config or the spike guest protocol
affects: [05-13, 05-15, 05-17, 05-19]
tech-stack:
  added: []
  patterns:
    - "One golden wire file shared by producer and consumer crates via include_str!, both pinning its SHA-256"
key-files:
  created: []
  modified:
    - crates/minos-backend-krun/src/launch_config.rs
    - crates/minos-backend-krun/src/supervise.rs
    - crates/minos-backend-krun/Cargo.toml
    - Cargo.lock
    - Cargo.toml
    - crates/minos-vmm/src/launch.rs
  deleted:
    - crates/minos-backend-krun/tests/spike_boot.rs
    - crates/minos-backend-krun/tests/spike_exits.rs
    - crates/minos-backend-krun/tests/spike_net.rs
    - crates/minos-backend-krun/tests/spike_seatbelt.rs
    - crates/minos-backend-krun/tests/spike_rootimg.rs
    - crates/minos-backend-krun/tests/wave0_chunks.rs
    - crates/minos-backend-krun/tests/wave0_console_pipe.rs
    - crates/minos-backend-krun/tests/wave0_store_root.rs
    - crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile
decisions:
  - "Console pipe branch: the backend's LaunchConfig has no Console type or field; minos-vmm v2 refuses a console key"
  - "LaunchConfig::to_json writes the compact JSON line plus a trailing newline, so vmm.json equals the golden file's form byte for byte"
metrics:
  duration: 6min
  completed: 2026-10-18
  tasks: 2
  files: 15
requirements-completed: [KRUN-01, KRUN-04]
---

# Phase 5 Plan 21: Spike retirement and launch config v2 Summary

I removed the Phase 2 spike harness and the Wave 0 proofs, each mapped below to the test that replaces it. The backend's launch config serializer now writes v2: two read-only disks and no console, boot mode, scratch disk, or initramfs. Its test checks the output byte for byte against minos-vmm's golden `config-v2.json` and its SHA-256 pin. The spike-era supervisor no longer creates a scratch image or names an initramfs.

## What was done

### Task 1: retire the spike and wave0 tests (commit eb8cfd6)

One `git rm` removed eight test files and the spike-only `tests/fixtures/rootimg/Dockerfile`. After the deletion, nothing used the `minos-core` dev-dependency that 05-03 had added for the wave0 proofs, so I removed it too (see Deviations). No source file changed, and the crate's tests and clippy stayed clean.

**Retirement mapping (02-CONTEXT D-02).** The evidence stays verbatim in `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` and `.planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md`. It is cited here, not copied.

| Removed test | Evidence it produced | Successor |
|---|---|---|
| `spike_net` (egress) | S02-EGRESS | `exit::exit_egress` (05-19) |
| `spike_boot` root-disk boot and cmdline tokens | S01-ROOTDISK, S01-CMDLINE | `exit::exit_env` (05-19) and every `vm` test (05-17) |
| `spike_boot` unsigned helper | S04-UNSIGNED | the entitlement probe in `host.rs` (05-11) |
| `spike_boot` initramfs attempt | S01-INITRAMFS | negative evidence stays in 02-SPIKE-EVIDENCE.md S01-INITRAMFS; 05-13 removes the initramfs writer |
| `spike_exits` (K7 cases) | K7 table (K7-01 to K7-07b) | the supervisor's classification tests (05-15) and `exit::exit_kill9` (05-19) |
| `spike_seatbelt` | SPIKE-05 answer, S05-BOOT, S05-DENY-NET, S05-DENY-FILE | `seatbelt.rs` tests (05-09), the denial probes (05-11), and every VM test running under the profile |
| `spike_rootimg` | RI-03, RI-04, S06-BOOT | the env build Docker test (05-16, run in 05-18) |
| `wave0_store_root` | W0-ROOT, W0-ROOT-DENY | every VM test boots its root image from a store path (05-17) |
| `wave0_console_pipe` | W0-CONSOLE | the supervisor's console cap test (05-15) |
| `wave0_chunks` | W0-CHUNKS | `vm::output_16_mib_is_reassembled` (05-17) |
| `tests/fixtures/rootimg/Dockerfile` | RI-03 fixture | 05-16's env build test restates the base-image digest without the `:3` tag that `ImageRef` refuses |

### Task 2: serializer v2, supervisor trim, exit_env doc references (TDD)

- RED (638fd92): `launch_config_matches_golden` and `launch_config_field_order` replaced the v1 E-1 and E-1b canonical-text tests. They failed to compile against the v1 constructors.
- GREEN (dd1c06f):
  - `launch_config.rs`: `LAUNCH_CONFIG_VERSION = 2`. `Boot`, `Initramfs`, `Scratch`, and `Console` are removed, following the 05-EVIDENCE.md Mechanism selections line `console: pipe`. The constructors are now `Disks::new(root, input)` and `LaunchConfig::new(kernel, disks, vsock, max_duration_ms, machine)`. `to_json` appends the trailing newline. The module doc records v2 (D-05, 05-RESEARCH Open Question 7) and the binding to the golden file.
  - `supervise.rs`: removed `SCRATCH_FILE`, `INITRAMFS_FILE`, the `scratch` and `initramfs` fields and accessors of `JobFiles`, and the scratch-cap parameter. The signature is now `write_disks(files, root, input)`. The module doc no longer describes a sparse scratch image. The unit tests now assert that no `scratch.img` or `initramfs.cpio` is created. The rest of the spike-era supervisor, including its console-file reader, is unchanged for 05-15 to replace.
  - Root `Cargo.toml` msb_krun pin comment and `ROOT_DISK_INIT` doc in `crates/minos-vmm/src/launch.rs`: both now name "the ignored VM test `exit_env` in crates/minos-backend-krun/tests/exit.rs (plan 05-19), which asserts the guest's exact /proc/cmdline tokens (02-SPIKE-EVIDENCE.md S01-CMDLINE)".
- REFACTOR: none needed. `cargo fmt` reflowed two lines of the RED test code, and that change is in the GREEN commit.

## Verification

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 1095 passed, 0 failed. minos-backend-krun has 83 passed and 1 ignored; minos-vmm has 38 passed, including `config_v2_golden`.
- `cargo deny check`: advisories, bans, licenses, and sources all ok. The `minos-sandbox` unmatched-wrapper warning was there before this plan: minos-backend-krun does not link minos-sandbox yet.
- `cargo doc --workspace --no-deps`: clean.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: clean.
- C-02 grep: prints nothing.
- Acceptance greps:
  - The tests directory is gone, so the `spike|wave0` count is 0.
  - The fixture is absent.
  - `pub const LAUNCH_CONFIG_VERSION: u32 = 2;` is present.
  - The golden path is present.
  - The `SCRATCH_FILE` count is 0.
  - `exit_env` appears in both `Cargo.toml` and `launch.rs`.

No VM test was run, so the helper was not re-signed. I did not run `cargo clean`, because that belongs to 05-20.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Dead dependency] Dropped minos-backend-krun's `minos-core` dev-dependency**
- **Found during:** Task 1
- **Issue:** 05-03 (commit 02042a7) added `[dev-dependencies] minos-core` only for the wave0 proofs. With those proofs removed, nothing used it. The repo removed unused dependencies the same way in 756c838.
- **Fix:** removed the `[dev-dependencies]` table. The one-line `Cargo.lock` update came with it.
- **Files modified:** `crates/minos-backend-krun/Cargo.toml`, `Cargo.lock`
- **Commit:** eb8cfd6

**2. [Rule 1 - Byte equality] `to_json` appends the trailing newline**
- **Found during:** Task 2
- **Issue:** the golden file is one compact line plus `\n`, and the plan requires the serializer to match it byte for byte. v1 `to_json` returned the line without the newline.
- **Fix:** `to_json` pushes `\n`. minos-vmm's parser accepts the golden file with its newline (`config_v2_golden`). `write_config` writes `to_json` unchanged, so `vmm.json` matches the golden form.
- **Commit:** dd1c06f

## Threat Flags

None. No new surface: the serializer can no longer express a scratch disk, an initramfs, or a console file, which mitigates T-05-21-01 and T-05-21-02.

## TDD Gate Compliance

The RED gate is `test(05-21)` commit 638fd92. It is followed by the GREEN gate, `feat(05-21)` commit dd1c06f.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/launch_config.rs (contains `LAUNCH_CONFIG_VERSION: u32 = 2` and `minos-vmm/golden/config-v2.json`)
- FOUND: commits eb8cfd6, 638fd92, dd1c06f
- ABSENT as intended: the nine removed test and fixture files
