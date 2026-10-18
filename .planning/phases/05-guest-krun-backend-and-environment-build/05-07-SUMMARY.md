---
phase: 05-guest-krun-backend-and-environment-build
plan: 07
subsystem: minos-vmm
tags: [minos-vmm, launch-config-v2, console-pipe, virtio-rng, logger, rlimits, clippy-bans, golden-file, d-01, d-05, krun-01, krun-02, krun-04, krun-07, diag-01]
status: complete
requires:
  - "05-03: Wave 0 mechanism selections (console: pipe), the STDOUT_CONSOLE literal, and the split preflight::check / check_files"
  - "05-01: the D-05 memory budget and [environment] scratch_mib that make scratch a guest tmpfs"
provides:
  - "minos-vmm launch config v2: CONFIG_VERSION = 2, RawDisks { root, input }, no scratch, boot, or console field; a v1 config is ConfigError::Version(1) and every removed field is an unknown field"
  - "crates/minos-vmm/golden/config-v2.json, SHA-256 0b1a1208888bbb6da4d77d0947a24993b31210aef4f788fb366549ee445f4a7b, read by config.rs with include_str!"
  - "Console fixed to STDOUT_CONSOLE (/dev/fd/1); preflight refuses a non-FIFO fd 1 with PreflightError::ConsoleNotPipe { found } (exit 66), decided by the pure console_pipe_ok(FileType)"
  - "Watchdog trips only Deadline (124) and ParentGone (126); the 125 console-cap trip and console_len are gone"
  - "launch::build: two read_only raw disks, MachineBuilder .balloon(false).rng(true).msb_metrics(false), root-disk boot only (INITRAMFS_CMDLINE removed)"
  - "minos-vmm log module: format_line, LineBudget, Admit, LOG_CAP_BYTES = 65536, TRUNCATED_LINE, install() at Warn"
  - "main::apply_rlimits: RLIMIT_CORE 0, RLIMIT_NOFILE min(1024, inherited hard), RLIMIT_FSIZE 0, current = maximum, before argv or config; PreflightError::Rlimit { resource, source } and LaunchError::Logger both exit 66"
  - "clippy.toml bans ConsoleBuilder::custom, ConsoleBuilder::custom_with_options, VsockBuilder::custom, VsockBuilder::custom_dgram, each proven to fire (05-EVIDENCE.md 05-07 clippy bans fire)"
  - "Workspace dependency log = \"=0.4.34\" (already in Cargo.lock through msb_krun)"
affects:
  - "05-09: minos-vmm.sb still carries the SCRATCH_IMG and CONSOLE_LOG params; the helper no longer opens a scratch disk or a console file"
  - "05-15: the supervisor must give the helper stdout and stderr on pipes (RLIMIT_FSIZE 0 kills a helper whose stderr is a regular file with SIGXFSZ); no 125 or console-cap marker exists any more; logged lines start with 'minos-vmm: log '"
  - "05-21: binds the backend serializer (still v1 in crates/minos-backend-krun/src/launch_config.rs) byte for byte to golden/config-v2.json; until then the ignored spike and wave0 VM tests speak v1 and must not be run"
  - "05-11: lists ConfinementLayer::Rlimits only when the helper and guest halves both apply"
tech-stack:
  added:
    - "log 0.4.34 as a direct dependency of minos-vmm (exact pin; previously transitive through msb_krun)"
    - "rustix (workspace pin 1.1.5) as a direct dependency of minos-vmm"
  patterns:
    - "A config constant instead of a config value for the console, so the only entitled binary cannot be pointed at a host file"
    - "A pure decision helper over the fstat file type, so the pipe check is unit-tested without depending on how cargo test wires stdout"
    - "A CAS byte budget that closes after admitting one truncation line, so concurrent loggers cannot write past the cap or truncate twice"
    - "Bounded formatting of log::Record args into at most 65536 characters before sanitizing"
key-files:
  created:
    - crates/minos-vmm/golden/config-v2.json
    - crates/minos-vmm/src/log.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/minos-vmm/Cargo.toml
    - crates/minos-vmm/clippy.toml
    - crates/minos-vmm/src/config.rs
    - crates/minos-vmm/src/launch.rs
    - crates/minos-vmm/src/main.rs
    - crates/minos-vmm/src/preflight.rs
    - crates/minos-vmm/src/watchdog.rs
    - crates/minos-core/src/ledger.rs
    - CLAUDE.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
decisions:
  - "Console branch: pipe (05-EVIDENCE.md Mechanism selections). The console is the constant /dev/fd/1, not a config field; its cap belongs to the supervisor"
  - "RLIMIT_NOFILE is min(1024, the inherited hard limit), so an unprivileged helper with a lower hard limit still confines itself instead of failing with EPERM"
  - "A logger install failure is LaunchError::Logger (exit 66), shown with Display only, because log's std feature is off and SetLoggerError has no Error impl here"
  - "With RLIMIT_FSIZE 0 the helper's stdout and stderr must be pipes: a regular-file stderr kills it with SIGXFSZ (exit 153 in a shell) on the first write, recorded in 05-EVIDENCE.md and CLAUDE.md"
  - "virtio-rng stays attached and is set explicitly with .rng(true) (05-RESEARCH Open Question 5); the CLAUDE.md open question on it is closed"
metrics:
  duration: "about 14 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 16
---

# Phase 5 Plan 07: minos-vmm v2 production helper Summary

minos-vmm now reads launch config v2 (two read-only raw disks, the vsock route, the machine, and a deadline, with no scratch disk, boot mode, or console object), writes the guest console only to its stdout pipe, attaches virtio-rng explicitly, confines itself with rlimits before reading anything, forwards msb_krun warnings through a capped, sanitizing stderr logger, and refuses four more msb_krun APIs at lint time. The D-05 scratch amendment is recorded in REQUIREMENTS.md, ROADMAP.md, and CLAUDE.md.

## What was built

**Task 1: config v2, the pipe console, explicit rng, the golden file.** `config.rs` drops `RawBoot`, `RawInitramfs`, `Boot`, `RawScratch`, `RawConsole`, `MAX_SCRATCH_CAP_BYTES`, and `MAX_CONSOLE_CAP_BYTES`. `CONFIG_VERSION` is 2. `golden/config-v2.json` is one compact line in the serializer's field order (`v`, `kernel`, `disks`, `vsock`, `max_duration_ms`, `machine`) plus a newline, and its SHA-256 is pinned in `config_v2_golden`. Preflight checks the environment, then requires fd 1 to be a FIFO (`rustix::fs::fstat`), then the files, socket, and libkrunfw hash. The watchdog keeps only the deadline and parent-gone trips. `launch::build` attaches root then input, both `read_only(true)`, sets `.rng(true)`, and boots only from the root disk.

**Task 2: logger, rlimits, bans.** `log.rs` writes each `Warn` or `Error` record as `minos-vmm: log <level>: ` plus the message with every character outside 0x20..=0x7e replaced by `?`. It stops at 64 KiB of lines, newlines included, and the first line that would pass the cap becomes one `minos-vmm: log truncated`. `main::run` calls `apply_rlimits()` and then `log::install()` before it parses argv. The four new clippy bans were proven with a throwaway path-expression probe (exit 101, four distinct `disallowed method` errors), then reverted, and clippy is clean.

**Task 3: design of record.** KRUN-04 and GUEST-03 in REQUIREMENTS.md, and ROADMAP Phase 5 criterion 3, now say scratch is size-capped space in guest memory, with the superseded wording dated 2026-10-18. CLAUDE.md Local backends now describes the following:
- the two read-only disks and the tmpfs scratch
- the root-disk-only boot
- the console pipe, in place of the 50 ms watchdog bullet
- classification without 125
- the logger, in place of the DIAG-01 bullet
- the two-sided `Rlimits` layer and the pipe requirement on stderr
- explicit virtio-rng and the new bans

The console-cap and virtio-rng open questions are removed. The `ConfinementLayer::Rlimits` doc names both halves and quotes "in-guest rlimits" from D-01.

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | 3290aee | test(05-07): add failing tests for launch config v2 and its golden file |
| 1 GREEN | a84c7ab | feat(05-07): launch config v2 with the stdout pipe console and explicit rng |
| 2 RED | 7f32a9f | test(05-07): add failing tests for the capped stderr logger and the rlimit exit code |
| 2 GREEN | 1db439f | feat(05-07): capped sanitizing logger, the helper's own rlimits, and four new clippy bans |
| 3 | 4613efa | docs(05-07): record the D-05 scratch amendment, the two-sided Rlimits layer, and the helper decisions |

## Verification

- `cargo fmt --all --check`: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: every suite ok, no failure; `minos-vmm` runs 38 tests, `config_v2` 14, `log_` 6
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok (one `unmatched wrapper` warning was already there before this plan, from not-yet-existing wrapper crates)
- `cargo doc --workspace --no-deps`: no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no MISSING line. C-02: printed nothing
- `target/debug/minos-vmm` rebuilt and re-signed with the CLAUDE.md codesign argv; `codesign --display --entitlements -` lists `com.apple.security.hypervisor`
- No VM was run (the plan forbids the ignored VM tests until 05-21)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] main.rs test updated in Task 1**
- **Found during:** Task 1
- **Issue:** `launch_error_codes_match_the_table` built `PreflightError::ConsoleExists`, which v2 removes, so the crate stopped compiling.
- **Fix:** The test uses `PreflightError::ConsoleNotPipe` instead. main.rs is a Task 2 file; only that one test line changed in Task 1.
- **Commit:** a84c7ab

**2. [Rule 2 - Correctness] RLIMIT_NOFILE never exceeds the inherited hard limit**
- **Found during:** Task 2
- **Issue:** Setting a fixed 1024 as both limits fails with EPERM when the parent's hard limit is lower, and an unprivileged process cannot raise its hard limit.
- **Fix:** The value is `min(1024, getrlimit(NOFILE).maximum)`. It stays bounded, and both limits stay equal.
- **Commit:** 1db439f

**3. [Rule 2 - Correctness] Log messages are formatted under a bound**
- **Found during:** Task 2
- **Issue:** `record.args().to_string()` would build an unbounded string from a message that may be guest-influenced.
- **Fix:** `message_text` formats into a writer that stops after `MAX_MESSAGE_CHARS` (65536) characters, and `format_line` sanitizes the result. Added `log_message_is_bounded` and `log_line_replaces_everything_outside_printable_ascii`.
- **Commit:** 1db439f

**4. [Rule 3 - Blocking] The logger install error is shown, not chained**
- **Found during:** Task 2
- **Issue:** `log` is built without its `std` feature, so `SetLoggerError` has no `Error` impl and cannot be a `#[source]`.
- **Fix:** Added `LaunchError::Logger(SetLoggerError)` with a Display-based message, mapped to exit 66. No feature was added to `log`.
- **Commit:** 1db439f

**5. [Rule 2 - Documentation] One more D-05 wording fix in CLAUDE.md**
- **Found during:** Task 3
- **Issue:** The "Environments and source" paragraph still said each experiment's build writes to "its own scratch disk".
- **Fix:** It now says "its own scratch space, a fresh size-capped tmpfs in guest memory (05-CONTEXT D-05)".
- **Commit:** 4613efa

### Notes

- **TDD:** in Task 2 RED, the rlimit exit-code assertion passed at once. `LaunchError::exit_code` maps every `PreflightError` except `Hash` to 66, so adding the `Rlimit` variant was enough. The five `log_` tests failed as intended, and the gate sequence holds: `test` commits 3290aee and 7f32a9f each precede their `feat` commits a84c7ab and 1db439f.
- **Finding for 05-15:** with `RLIMIT_FSIZE` 0, a helper whose stderr is a regular file dies by `SIGXFSZ` on its first stderr write and leaves no diagnostic. The supervisor plan already uses pipes for both stdout and stderr. Recorded in 05-EVIDENCE.md "05-07 helper rlimits: stderr must be a pipe" and in CLAUDE.md.
- **Left for 05-09:** `minos-vmm.sb` still declares `SCRATCH_IMG` and `CONSOLE_LOG`, and the CLAUDE.md Seatbelt bullet still describes them, which matches the profile on HEAD. 05-09 owns the profile.

## Known Stubs

None. The backend's v1 serializer is intentionally out of step until 05-21, as the plan states.

## Threat Flags

None. The change removes surface: no writable disk, no console file, no boot mode, and four more banned APIs. Nothing new crosses a trust boundary beyond the stderr log lines that T-05-07-01 already covers.

## Self-Check: PASSED

- FOUND: crates/minos-vmm/golden/config-v2.json
- FOUND: crates/minos-vmm/src/log.rs
- FOUND: 3290aee, a84c7ab, 7f32a9f, 1db439f, 4613efa
