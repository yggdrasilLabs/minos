---
phase: 05-guest-krun-backend-and-environment-build
plan: 03
subsystem: minos-vmm, minos-guest, minos-backend-krun
tags: [wave0, vm-proof, seatbelt, console-pipe, chunk-protocol, store-root, krun-01, krun-04, guest-04, d-14, s03-answer]
status: complete
requires:
  - "05-02: minos_core::chunk (ChunkToken, ChunkStream, Reassembler, MAX_CHUNK_BYTES)"
provides:
  - "minos-vmm STDOUT_CONSOLE = \"/dev/fd/1\": the only console path that skips the console-absent and parent checks; the watchdog console-cap trip is inert for it"
  - "minos-vmm.sb: (allow file-read-metadata (path-ancestors (param \"ROOT_IMG\"))), proven needed by W0-ROOT-DENY"
  - "minos-guest spike-only chunk_stream step (RawChunkStream, ChunkStream, StepResult::ChunkStream, ChunkStreamResult)"
  - "Ignored VM proofs: wave0_store_root, wave0_console_pipe, wave0_chunks"
  - "05-EVIDENCE.md with W0-ROOT, W0-ROOT-DENY, W0-CONSOLE, W0-CHUNKS and Mechanism selections: console pipe, root image store-path, chunks proceed"
affects:
  - "05-07 (console: pipe branch)"
  - "05-09 and 05-15 (root image: store-path branch; chunks: proceed branch)"
  - "05-12 (removes the chunk_stream spike step with the spike protocol)"
  - "05-21 (retires the spike harness the wave0 tests use)"
tech-stack:
  added: []
  patterns:
    - "A console literal compared byte for byte, so only /dev/fd/1 is stdout; /dev/stdout and /dev/fd/2 stay ordinary paths that must not exist"
    - "Host accept loops wait with rustix::event::poll on the listener and on each connection instead of sleep polling"
    - "A pipe reader hands the pipe back from its thread instead of dropping it, so a writer is ended by the test's signal and not by EPIPE"
key-files:
  created:
    - crates/minos-backend-krun/tests/wave0_chunks.rs
    - crates/minos-backend-krun/tests/wave0_console_pipe.rs
    - crates/minos-backend-krun/tests/wave0_store_root.rs
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/minos-vmm/src/config.rs
    - crates/minos-vmm/src/preflight.rs
    - crates/minos-vmm/src/watchdog.rs
    - crates/minos-vmm/src/launch.rs
    - crates/minos-vmm/minos-vmm.sb
    - crates/minos-guest/Cargo.toml
    - crates/minos-guest/src/job.rs
    - crates/minos-guest/src/report.rs
    - crates/minos-guest/src/linux.rs
    - crates/minos-backend-krun/Cargo.toml
decisions:
  - "Mechanism selections (05-EVIDENCE.md): console pipe, root image store-path, chunks proceed"
  - "The ROOT_IMG path-ancestors metadata rule stays in minos-vmm.sb: without it preflight exits 66 with disks.root Operation not permitted (W0-ROOT-DENY)"
  - "The console-cap watchdog trip is inert only for the exact literal /dev/fd/1; the supervisor enforces the cap on the pipe through cap + 1"
  - "The chunk_stream step stores its token as raw bytes and returns a ChunkToken from token(), with a Debug that never prints the token, because ChunkToken has no PartialEq and Step derives it"
metrics:
  duration: "about 13 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 16
---

# Phase 5 Plan 03: Wave 0 VM proofs Summary

All three new mechanisms work on this Mac under the deny-default Seatbelt profile, so each later plan takes the main branch:

- **Console on the stdout pipe.** The helper's stdout carries the console under Seatbelt, and the helper creates no file for it. The cap is exact: the reader held 262145 bytes (cap + 1), then the test's SIGKILL ended the helper.
- **Root image read from the store path.** A root image at `.minos/blobs/sha256/..` boots once the `ROOT_IMG` ancestor-metadata rule is in the profile. Without the rule, preflight exits 66 with `Operation not permitted`.
- **Chunks at the 16 MiB cap.** 16 MiB of 4 KiB authenticated chunks reassembled exactly in about 247 ms. That took 4097 connections and 0 connect retries.

## What was built

**Task 1: enable the proofs (commits eb00073 RED, 140659d GREEN)**
- Workspace `rustix` gains the `event` feature for `rustix::event::poll`.
- minos-vmm:
  - `STDOUT_CONSOLE = "/dev/fd/1"`. Preflight's new `console_ok` skips the console-absent and parent-canonical checks only for that exact byte string, and `check` is split into the environment check and `check_files`.
  - `watchdog::Limits` holds `console: Option<PathBuf>`, which is `None` for the stdout literal, so the console-cap trip is inert there. The deadline and parent-gone checks are unchanged.
  - Tests:
    - `config_accepts_stdout_console`
    - `preflight_allows_stdout_console`: a missing libkrunfw is still refused when the console is the literal.
    - `preflight_still_refuses_existing_console_file`: `/dev/stdout`, `/dev/fd/2`, `/dev/fd/0`, and the test binary are all refused with `ConsoleExists`.
    - `watchdog_trips_on_console_file_over_cap`
    - `watchdog_ignores_stdout_console_length`
- `minos-vmm.sb`: `(allow file-read-metadata (path-ancestors (param "ROOT_IMG")))`, with its comment. W0-ROOT-DENY proves it is needed.
- minos-guest:
  - Depends on `minos-core`.
  - New spike-only `chunk_stream` step. `bytes` is 1 to `MAX_VSOCK_STREAM_BYTES`. The token must be exactly 64 lowercase hex characters, decoded without indexing.
  - `ChunkStreamResult { bytes_sent, connections, connect_retries, elapsed_ms, error }`.
  - `linux.rs` `chunk_stream` sends each chunk from `minos_core::chunk::ChunkStream::new` on its own vsock connection. A refused or reset connect is retried with delays of 1, 2, 4, ... up to 64 ms, at most 200 attempts per chunk.
  - Tests cover decode, every refusal, a Debug that hides the token, and the result's field order.

**Task 2: three ignored VM tests (commit 02042a7)**
- `wave0_store_root.rs`:
  - `store_root_boots_under_seatbelt`
  - `store_root_without_ancestor_rule_is_denied`: the profile is rewritten with the rule line and its comment removed.
- `wave0_console_pipe.rs`:
  - `console_pipe_carries_boot_output`: also checks the A4 job-directory listing.
  - `console_pipe_cap_is_exact`
  - The helper is spawned directly with `env_clear`, stdin null, stdout and stderr piped, and `process_group(0)`.
- `wave0_chunks.rs`:
  - `chunked_16_mib_reassembles_and_is_measured`
  - `chunked_1_mib_reassembles`
  - The accept thread waits with `rustix::event::poll` and reads each connection through `MAX_CHUNK_BYTES + 1` with a 5 s deadline. `MINOSCK1` connections go to `Reassembler::new(token, 17 MiB)` and `MINOSST1` to a frame decoder.
- `minos-backend-krun` gains a `minos-core` dev-dependency. `deny.toml` already lists it as a wrapper.

**Task 3: runs and evidence (commit 1c82040)**
- Order: build the musl guest, build `minos-vmm`, prebuild the backend tests, then sign. The runs therefore never relinked the helper.
- Each run, its exit code, and its verbatim output are in `05-EVIDENCE.md`.

| Proof | Result |
|---|---|
| W0-ROOT | Exit 0, `MINOS-GUEST-BOOT-OK`, one valid frame, root at `<job>/proj/.minos/blobs/sha256/d6/666d...` |
| W0-ROOT-DENY | Exit 66. Stderr: `minos-vmm: error: preflight: disks.root <store path>: Operation not permitted (os error 1)` |
| W0-CONSOLE boot | Exit 0, 3634 console bytes on the pipe including the boot marker, one valid frame. Job directory holds only the 5 files the test created (A3 and A4 hold) |
| W0-CONSOLE cap | The reader held exactly 262145 bytes, then the helper ended by signal 9 (the test's SIGKILL) |
| W0-CHUNKS 16 MiB | Exact, no fault. 246.86 ms first chunk accept to END (repeat 247.31 ms), 4097 connections, 0 retries, guest elapsed 258 ms, helper total 588 ms |
| W0-CHUNKS 1 MiB | Exact, no fault. 15.99 ms, 257 connections, 0 retries, guest elapsed 16 ms |

Mechanism selections: `- console: pipe`, `- root image: store-path`, `- chunks: proceed`.

## Verification

- `cargo fmt --all --check`: passes.
- `cargo clippy --workspace --all-targets -- -D warnings`: passes.
- `cargo test --workspace`: 63 suites, all `ok`.
- `cargo deny check`: advisories, bans, licenses, and sources ok. The existing "unmatched wrapper" warning on `minos-sandbox` is unrelated.
- `cargo doc --workspace --no-deps`: 0 warnings.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: passes.
- C-01: 0 MISSING lines. C-02: prints nothing.
- The plan's acceptance greps pass:
  - The `STDOUT_CONSOLE` const, the `path-ancestors (param "ROOT_IMG")` rule, `"event"`, and `ChunkStream::new` in `linux.rs` are all present.
  - Each wave0 file has exactly 2 `#[ignore`.
  - `Reassembler::new`, `event::poll`, `Operation not permitted`, and `262_145` are present.
  - In `05-EVIDENCE.md`: 3 selection lines, the 4 section headings, and 11 `Exit code:` lines.

## Deviations from Plan

### Auto-fixed Issues

**1. [Signature adjustment] The `chunk_stream` step stores token bytes, not a `ChunkToken`**
- **Found during:** Task 1
- **Issue:** `Step` derives `PartialEq`/`Eq`, but `ChunkToken` deliberately has no `PartialEq` (05-02).
- **Fix:** The step keeps `[u8; 32]` privately, and `token()` returns `ChunkToken::from_bytes(..)`. A hand-written `Debug` prints only `bytes`, so the token never reaches a log. The test token is a spike-only value (T-05-03-04, accepted).
- **Files modified:** crates/minos-guest/src/job.rs
- **Commit:** 140659d

**2. [Testability] `preflight::check` split into the environment check plus `check_files`**
- **Found during:** Task 1
- **Issue:** `cargo test` on macOS sets `DYLD_FALLBACK_LIBRARY_PATH`, which the environment check refuses. A unit test of the full `check` could therefore never reach the file checks.
- **Fix:** `check` now calls the environment check and then `check_files`, and `preflight_allows_stdout_console` tests `check_files`. Behavior is unchanged.
- **Commit:** 140659d

**3. [Test coverage] Extra tests beyond the behavior block**
- **Found during:** Task 1
- **What was added:** `watchdog_trips_on_console_file_over_cap`, which shows the file console still trips. `job_chunk_stream_debug_hides_the_token`. The result field-order test. Long-token, non-hex, empty-token, and unknown-field refusals.
- **Commits:** eb00073, 140659d

**4. [Doc accuracy] `launch.rs` module doc**
- **Found during:** Task 1
- **What changed:** The doc now says the console writes to a file or to the helper's stdout when the config names exactly `STDOUT_CONSOLE`. The file is not listed in the plan.
- **Commit:** 140659d

### Not changed here

- **Seatbelt deny lines in the system log:** the plan does not require them, but Phase 2 captured them this way (S05-ITER), so they were looked for. `log show` on macOS 27.0.1 returned no kernel `Sandbox: minos-vmm ... deny(1)` line for either W0-ROOT-DENY run. The denial is recorded from the helper's exit code and stderr, which are the plan's criterion. The empty query output is in the evidence as recorded.
- **CLAUDE.md open questions:** two of them now have evidence: the console cap (pipe, exact at cap + 1, W0-CONSOLE) and the cost of the chunked protocol (16 MiB in about 247 ms, W0-CHUNKS). 05-02 left CLAUDE.md to its owning plans (05-07, 05-13, 05-20), and this plan does the same, so CLAUDE.md is untouched.
- **SPIKE-03 in REQUIREMENTS.md:** left as it was, as the plan directs. W0-CHUNKS is new evidence the user may use for SPIKE-03's wording.
- **Signing:** `cargo test --workspace` ran after the VM runs and relinked `minos-vmm`, so re-sign it (CLAUDE.md Commands) before the next VM test.

## Known Stubs

None. The `chunk_stream` step and the wave0 tests are spike-only by design. 05-12 removes the step with the spike protocol, and the production VM tests in 05-17 and 05-19 supersede the wave0 tests.

## TDD Gate Compliance

Task 1 has a `test(05-03)` RED commit (eb00073) followed by a `feat(05-03)` GREEN commit (140659d). In the RED run both crates failed to compile because `STDOUT_CONSOLE`, `console_ok`, `check_files`, `Limits::new`, `Step::ChunkStream`, and `ChunkStreamResult` did not exist yet. Tasks 2 and 3 are not TDD tasks.

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/tests/wave0_chunks.rs, crates/minos-backend-krun/tests/wave0_console_pipe.rs, crates/minos-backend-krun/tests/wave0_store_root.rs, .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
- FOUND commits: eb00073, 140659d, 02042a7, 1c82040
