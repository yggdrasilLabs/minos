---
phase: 05-guest-krun-backend-and-environment-build
plan: 09
subsystem: minos-backend-krun
tags: [seatbelt, sandbox-exec, include-str, rootfs, mount-points, elf-check, krun-01, krun-04, krun-09, d-14, pitfall-8]
status: complete
requires:
  - "05-03: Wave 0 mechanism selections (console: pipe, root image: store-path) and the ROOT_IMG path-ancestors metadata rule"
  - "05-07: launch config v2 with no scratch disk and the console fixed to /dev/fd/1"
  - "05-08: the guest's SCRATCH_MOUNT /scratch and OUT_MOUNT /out"
provides:
  - "crates/minos-backend-krun/src/minos-vmm.sb (moved from crates/minos-vmm/), SHA-256 cda08b0f6f5f40385f13b682b36cba30167d7d70d50caea9043912bd08b08490"
  - "minos_backend_krun::seatbelt: PROFILE (include_str!), SANDBOX_EXEC, profile_sha256, ProfileParams::new (7 absolute params), argv, SeatbeltError { RelativePath, ParentComponent }"
  - "rootfs::MOUNT_POINTS has seven entries (dev, mnt, out, proc, scratch, sys, tmp); minimal root image golden 97928bf08a816a881769335b02c05162a599eb20601667279e6d5d5cab77360d"
  - "rootfs::check_guest_elf and ElfError { NotElf, Not64Bit, NotLittleEndian, NotAarch64, Dynamic, Truncated }"
  - "CLAUDE.md: the embedded profile and its shipped allowances, the seven mount points and the ELF check, exit_env and the vm/exit ignored tests in place of the spikes"
affects:
  - "05-11, 05-16, 05-17: call check_guest_elf before packing a guest or recording guest_sha256"
  - "05-15: the supervisor builds the helper argv with seatbelt::argv and ProfileParams::new (no SCRATCH_IMG or CONSOLE_LOG)"
  - "05-17: the first deny-default VM boot under the narrowed console rule (write only on /dev/fd/1)"
  - "05-21: the ignored spike and wave0 tests still read ../minos-vmm/minos-vmm.sb by path; 05-21 deletes them in this wave"
tech-stack:
  added: []
  patterns:
    - "A security profile compiled into the binary with include_str!, passed as text, and pinned by digest in a unit test"
    - "A param-set equality test between a config text and the code that supplies its parameters"
    - "Fixed-offset binary header parsing with a const-generic field reader over get and try_from, no indexing"
key-files:
  created:
    - crates/minos-backend-krun/src/seatbelt.rs
  modified:
    - crates/minos-backend-krun/src/minos-vmm.sb
    - crates/minos-backend-krun/src/rootfs.rs
    - crates/minos-backend-krun/src/lib.rs
    - CLAUDE.md
decisions:
  - "Console branch pipe: the profile's console rule is (allow file-write* (literal \"/dev/fd/1\")), write only; the read half of the old CONSOLE_LOG rule is dropped"
  - "Root image branch store-path: the ROOT_IMG path-ancestors metadata rule stays"
  - "ProfileParams also refuses a path with a .. component (SeatbeltError::ParentComponent), so every parameter is the canonical form a literal rule compares against"
  - "check_guest_elf refuses PN_XNUM and program header entries under 56 bytes as Truncated, so a table it cannot fully walk never passes"
metrics:
  duration: "about 20 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 5
---

# Phase 5 Plan 09: Embedded Seatbelt profile, seven mount points, and the static-ELF check Summary

The helper's deny-default Seatbelt profile now sits beside the backend code, is compiled in with `include_str!`, goes to `sandbox-exec` as text through `-p`, and has its SHA-256 pinned. A test proves its parameters equal the seven `-D` names the argv builder supplies. Root images now carry `/out` and `/scratch`. `check_guest_elf` refuses anything other than a 64-bit little-endian `aarch64` ELF with no `PT_INTERP`.

## What was built

**Task 1: the embedded profile.** `git mv crates/minos-vmm/minos-vmm.sb crates/minos-backend-krun/src/minos-vmm.sb`. The `SCRATCH_IMG` and `CONSOLE_LOG` read-write rule is gone. In its place is `(allow file-write* (literal "/dev/fd/1"))`, with a comment citing W0-CONSOLE and the selected mechanism. The ROOT_IMG ancestor rule stays (store-path). Two comments were reworded so that the token `network-outbound` appears only on the one rule. `seatbelt.rs` provides:
- `PROFILE`, `SANDBOX_EXEC`, and `profile_sha256()`
- `ProfileParams::new(vmm, config, krunfw, krunfw_link, root_img, input_img, sock)`, which refuses relative and `..` paths
- `argv(params, program, args)`: `/usr/bin/sandbox-exec -p <PROFILE>`, then `-D NAME=value` in ascending name order, then the program and its arguments

Tests: `profile_params_match_argv`, `profile_is_pinned` (pin computed with `shasum -a 256 crates/minos-backend-krun/src/minos-vmm.sb`), `profile_network_is_socket_only`, `argv_shape`, `params_must_be_absolute`.

**Task 2: mount points and the ELF check.** `MOUNT_POINTS` gains `("out", 0o755)` and `("scratch", 0o755)`. The module doc and the existing mount-point test are updated (dirs 8 -> 10), and the minimal root image golden digest changes from `8e4f12d4...` to `97928bf0...` because of the two new directories. `check_guest_elf` checks, in order: the magic, `EI_CLASS` 2, `EI_DATA` 1, and `e_machine` 183. It then walks the program headers with checked arithmetic and `get`, refusing `PT_INTERP`. Fixtures are built with `extend_from_slice`.

**Task 3: CLAUDE.md.** Each of these now names something that exists, or that this wave and the next plans create:
- The Seatbelt bullet: the profile's new path, `include_str!`, `sandbox-exec -p`, the pinned digest, and the allowances as shipped.
- The packer bullet: the seven mount points and the ELF check.
- The D-18 boot bullet: `exit_env` in place of `spike_boot::boot_root_disk`.
- The egress bullet: `exit_egress`.
- The Commands VM-test paragraph: the `vm`/`exit` ignored tests and the D-04 note that the `MINOS_*` variables are read only by ignored tests.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | b46db59 | test(05-09): add failing tests for the embedded Seatbelt profile and its argv |
| 1 GREEN | 4b634de | feat(05-09): embed the Seatbelt profile, pin its digest, and build the sandbox-exec -p argv |
| 2 RED | 75875e0 | test(05-09): add failing tests for the /out and /scratch mount points and the static-ELF check |
| 2 GREEN | 478207c | feat(05-09): root images gain /out and /scratch, and check_guest_elf refuses a non-static or non-aarch64 guest |
| 3 | dab86e8 | docs(05-09): point CLAUDE.md at the embedded profile, the seven mount points, and the VM tests |

## Verification

- `cargo fmt --all --check`: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 1063 passed, 0 failed; `seatbelt` 5 tests, `rootfs` 31 tests
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok. The `unmatched wrapper` warning was already there.
- `cargo doc --workspace --no-deps`: no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no MISSING line. C-02: printed nothing
- Acceptance greps:
  - `include_str!("minos-vmm.sb")` is present.
  - `SCRATCH_IMG` count is 0, and `network-outbound` count is 1.
  - `pub mod seatbelt;`, `pub fn check_guest_elf`, `("out", 0o755)`, and `("scratch", 0o755)` are present.
  - CLAUDE.md has 0 `crates/minos-vmm/minos-vmm.sb` and 0 `spike_boot`, and it has `sandbox-exec -p`, `exit_env`, and `/scratch`.
- No VM was run. 05-07 forbids the ignored spike and wave0 VM tests until 05-21 moves the serializer to v2.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Security] ProfileParams refuses `..` components**
- **Found during:** Task 1
- **Issue:** An absolute path such as `/private/tmp/job/../x` would pass the absolute-path check. It is not the canonical path a Seatbelt literal or `path-ancestors` rule compares against.
- **Fix:** Added `SeatbeltError::ParentComponent { name }` (the enum is `#[non_exhaustive]`) and a case in `params_must_be_absolute`.
- **Commit:** b46db59, 4b634de

**2. [Rule 2 - Correctness] check_guest_elf fails closed on a program header table it cannot walk**
- **Found during:** Task 2
- **Issue:** Two inputs could otherwise pass without being fully inspected: a `PT_INTERP` hidden behind PN_XNUM extended numbering (`e_phnum` 0xffff), and an `e_phentsize` too small to hold a 64-bit program header.
- **Fix:** Both are refused as `Truncated`, as are table offsets that overflow. Added `elf_check_refuses_an_unreadable_program_header_table`.
- **Commit:** 75875e0, 478207c

**3. [Rule 2 - Documentation] Profile comments reworded, and two more CLAUDE.md references repointed**
- **Found during:** Tasks 1 and 3
- **Issue:**
  - Two profile comments contained the token `network-outbound`, which breaks the plan's "exactly one" check.
  - The header comment said every path is a parameter, but `/dev/fd/1` is now a literal.
  - CLAUDE.md named `egress_fails_closed` and "the VM spikes", which 05-21 retires.
- **Fix:**
  - The comments now say "outbound network", and the header names the console literal.
  - CLAUDE.md now names `exit_egress` (05-19) and "the ignored VM tests".
- **Commit:** b46db59, dab86e8

### Notes

- **Golden change:** `rootfs_minimal_root_image_golden` moves to `97928bf08a816a881769335b02c05162a599eb20601667279e6d5d5cab77360d`. This is intended: the image gains two directories. The ignored `spike_rootimg` test prints an image digest but pins none, and 05-21 retires it.
- **Narrowed console rule, not yet booted:** W0-CONSOLE booted under the old `file-read* file-write*` literal. The plan's write-only rule matches how msb_krun opens the console (`File::create`, write only), and the research verified the literal against an allow-default base. Preflight checks fd 1 with `fstat`, which does not go through a path rule. No deny-default VM boot has run under the write-only rule yet. The first one is 05-17's VM harness, and a denial there would show as exit 66 or a missing console.
- **TDD:** in both RED commits, the tests that depended only on data already present passed at once (`profile_network_is_socket_only` and `params_must_be_absolute` in Task 1, since the profile edit and the validation landed in the RED commit). The tests for the new behavior failed as intended, and each `test` commit precedes its `feat` commit.

## Known Stubs

None.

## Threat Flags

None. The change removes surface: no scratch image rule, and no read on the console. T-05-09-01 to 03 are mitigated as planned.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/seatbelt.rs
- FOUND: crates/minos-backend-krun/src/minos-vmm.sb
- FOUND: b46db59, 4b634de, 75875e0, 478207c, dab86e8
