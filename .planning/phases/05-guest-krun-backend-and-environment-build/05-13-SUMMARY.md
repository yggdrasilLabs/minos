---
phase: 05-guest-krun-backend-and-environment-build
plan: 13
subsystem: minos-backend-krun
tags: [input-disk, token, blob-source, chunk-receiver, vsock, poll, guest-04, krun-04, d-13, d-15]
status: complete
requires:
  - "05-02: minos_core::chunk (ChunkToken, ChunkStream, Reassembler, ChunkFault, MAX_CHUNK_BYTES)"
  - "05-04: minos_core::guest (GuestJob, SectionKind caps, write_input_disk, read_input_disk, check_role)"
  - "05-06: BlobSource, BlobSourceError, InputDisk part IDs, LaunchPlan accessors, LaunchKey::of, LaunchError::Blob"
  - "05-11: jobdir::JobRoot / JobDir (0700 job directory, input() and socket() paths)"
  - "05-21: the spike tests that used the v1 writer and the initramfs writer are gone"
provides:
  - "input_disk::new_token: 32 bytes from /dev/urandom (character device checked)"
  - "input_disk::FetchedParts (job, job_bytes, base, diff, judge_data, candidate_out; Debug shows lengths only)"
  - "input_disk::fetch_parts<B: BlobSource, R: Role>: every part under its SectionKind cap, cap and hash re-checked, job checked against the plan before anything else is fetched"
  - "input_disk::write_input_disk(&JobDir, &ChunkToken, &FetchedParts) -> input.img, 0600, persist_noclobber"
  - "chunks::{CONNECTION_DEADLINE, ReceiveOutcome, ReceiveResult, ChunkReceiver (spawn, faulted, ended, stop_and_join)}"
affects:
  - "05-15: the supervisor fetches parts, draws a token, writes input.img, binds the listener at JobDir::socket, spawns ChunkReceiver, stops the VM early on faulted(), and maps Complete / Incomplete / Fault into Finished (Incomplete with a dead PID 1 is Crashed, Fault is ChunkProtocol); it also replaces supervise.rs's MINOSST1 frame code and removes the 'How a backend receives blob bytes' open question"
  - "05-17, 05-19: VM tests exercise the receiver end to end, including the forged-token probe"
  - "Phase 7: minos judge push refuses judge data over 96 MiB (CLAUDE.md Open questions)"
tech-stack:
  added: []
  patterns:
    - "Fetch caps are the disk section caps (SectionKind::cap), so the writer never refuses what the fetch accepted"
    - "A BlobSource's contract is re-checked at the trust boundary: length against the cap and SHA-256 against the ID"
    - "A connection is read non-blocking against one total deadline with rustix poll, never SO_RCVTIMEO (02-05)"
    - "Receiver thread returns io::Result: host failures are io errors, candidate faults are ReceiveOutcome values"
key-files:
  created:
    - crates/minos-backend-krun/src/chunks.rs
  modified:
    - crates/minos-backend-krun/src/input_disk.rs
    - crates/minos-backend-krun/src/lib.rs
    - CLAUDE.md
  deleted:
    - crates/minos-backend-krun/src/initramfs.rs
decisions:
  - "A connection still open at its deadline is ChunkFault::Length (its chunk did not arrive whole), and one over MAX_CHUNK_BYTES is ChunkFault::Oversize; both are detected by the receiver before the reassembler sees the bytes"
  - "fetch_parts re-checks every fetched blob's length and SHA-256 even though BlobSource promises both, failing closed as BlobSourceError::TooLarge or Tampered"
  - "The JOB section is the job blob's bytes exactly as fetched, not a re-serialization"
  - "A Judge plan without judge data is LaunchError::Backend 'the judge plan has no judge data' (preflight already refuses it; this is defence in depth)"
  - "The chunk-cost open question stays open, narrowed to concurrent VMs, with the single-VM W0-CHUNKS measurement recorded"
metrics:
  duration: "about 7 minutes (wall clock from start stamp)"
  completed: 2026-10-18
  tasks: 3
  files: 5
---

# Phase 5 Plan 13: Input disk writer and chunk receiver Summary

The krun backend now has both byte paths of a launch on the host: a per-job input disk bound to its plan and carrying a fresh `/dev/urandom` token, and a poll-driven receiver that reassembles the guest's token-authenticated chunks and turns every forgery or abuse into a `ChunkProtocol` candidate fault.

## What was built

**Task 1: plan-bound input disk (commits 57fc6c5 RED, 6aae5b9 GREEN)**
- `new_token` opens `/dev/urandom`, checks that it is a character device, and reads 32 bytes into a `ChunkToken`.
- `fetch_parts` fetches the job blob under the JOB cap and parses it. It then compares it with the plan:
  - the role;
  - the `LaunchKey` recomputed from the job's experiment, judge version, and trial;
  - the base and diff;
  - every stage budget;
  - the memory.
- Only after that does it fetch the base and diff (candidate) or the judge data and candidate output (judge), each under its `SectionKind` cap. The cap and the hash are re-checked on every blob. A mismatch is `LaunchError::Backend { what: "the guest job does not match the plan" }`.
- `write_input_disk` writes core's v2 disk through a `BufWriter` into a 0600 `tempfile` in the job directory, then flushes, runs `sync_all`, and calls `persist_noclobber` to `input.img`. A second write fails as `LaunchError::Io`, leaves the first disk unchanged, and leaves no temporary file behind.
- Removed: the v1 `MINOSJB1` writer, and `initramfs.rs` with its `pub mod` line. Neither had a caller left (`grep -rn 'input_disk_image\|initramfs::' crates` printed nothing). The D-18 negative evidence stays in 02-SPIKE-EVIDENCE.md S01-INITRAMFS.
- 9 tests:
  - fetch for a candidate, the baseline, and a judge;
  - the six mismatch classes plus a judge plan with no judge data;
  - over-cap, unparsable, and missing blobs;
  - a source that breaks its contract (wrong bytes, ignored cap);
  - a disk round trip through `read_input_disk` and `check_role` for both roles (mode 0600, token preserved);
  - no overwrite;
  - token randomness.

**Task 2: chunk receiver (commits 01d3d9c RED, 6b7e9ae GREEN)**
- `ChunkReceiver::spawn` makes the listener non-blocking and moves it to a named thread. The thread loops:
  1. waits on the listener with `rustix::event::poll` for 50 ms;
  2. accepts until `WouldBlock`;
  3. reads each connection through `MAX_CHUNK_BYTES + 1` bytes against its own deadline;
  4. feeds the bytes to `Reassembler::accept`.
- The receiver stops on the first fault (setting the fault flag) or once the stream is complete. After `stop`, it drains the connections already pending once more. The thread closes the listener when it ends, so later connects are refused.
- `ReceiveOutcome` is `Complete(bytes)`, `Incomplete` (no END), or `Fault(ChunkFault)`. Its `Debug` shows lengths only. `ReceiveResult` carries the connection count and the time from the first connection to the one that completed the stream.
- 9 tests:
  - a complete 100 KiB stream (26 connections);
  - out-of-order arrival;
  - a 5000-connection backlog with client connect retry;
  - a wrong token (`Token`, flag raised, listener closed);
  - an empty connection (`Short`);
  - an oversize connection (`Oversize`);
  - a trickle under a 200 ms deadline (ends well before the client closes);
  - no END (`Incomplete`);
  - no stream at all (`Incomplete`).
- The chunk tests ran 5 times in a row with no flake.

**Task 3: CLAUDE.md (commit e3801af)**
- The vsock bullet describes the protocol as built:
  - the `MINOSCK1` 48-byte header and the END trailer;
  - the per-launch token, how PID 1 removes the disk node, and why the token exists;
  - the fault classes and the `Crashed` case for a missing END;
  - poll with a 5 s per-connection deadline;
  - the W0-CHUNKS measurement, which replaces "unmeasured until Phase 5".
- The disk bullet describes `MINOSJB2`: how the backend writes it from capped `BlobSource` bytes, and the per-role bound (`MAX_GUEST_INPUT_BYTES` 128 MiB, `MAX_JUDGE_DATA_BYTES` 96 MiB).
- Open questions:
  - the chunk-cost bullet is narrowed to concurrent VMs and carries the measured numbers;
  - a new bullet covers a streaming unpack for judge data over 96 MiB, with Phase 7 as its owner.

## Verification

- `cargo fmt --all --check`: passes
- `cargo clippy --workspace --all-targets -- -D warnings`: passes
- `env -u INSTA_UPDATE cargo test --workspace`: 57 result lines, all ok
- `cargo deny check`: advisories, bans, licenses, and sources all ok. The duplicate-version warnings were already there.
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: passes
- C-01: no MISSING lines. C-02: the grep prints nothing.
- Acceptance greps for all three tasks print their lines. `initramfs.rs` is gone, and `mod initramfs` appears 0 times in `lib.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] fetch_parts re-checks the BlobSource contract**
- **Found during:** Task 1
- **Issue:** A `BlobSource` that ignored the cap or returned the wrong bytes would put unchecked bytes on a disk.
- **Fix:** Each fetched blob's length is checked against the cap (`TooLarge`) and its SHA-256 against the ID (`Tampered`). The caps come from `SectionKind::cap`, so they equal the constants the plan names and the disk writer's own caps.
- **Commit:** 6aae5b9

**2. [Rule 2 - Correctness] new_token checks that /dev/urandom is a character device**
- **Issue:** A planted regular file could otherwise supply a known token.
- **Commit:** 6aae5b9

**3. [Rule 2 - Correctness] Host failures in the receiver thread are io errors, not results**
- **Issue:** The plan's `JoinHandle<ReceiveResult>` had no way to report a failed `poll` or `accept` without blaming the candidate.
- **Fix:** The thread returns `io::Result<ReceiveResult>`, and `stop_and_join` returns that error (and a panic) as an `io::Error`, an infrastructure failure.
- **Commit:** 6b7e9ae

**4. [API additions] `ChunkReceiver::ended`, `ReceiveResult::into_outcome`, and `Drop` for `ChunkReceiver`**
- `ended()` lets the supervisor see that the receiver stopped on its own, on a fault or a complete stream.
- `Drop` sets the stop flag, so a dropped receiver cannot leave its thread holding the listener forever.
- **Commit:** 6b7e9ae

**5. [Signature adjustment] The poll interval is a `Duration`, not `POLL_TIMEOUT_MS: i32`**
- rustix 1.1.5's `poll` takes a `Timespec`, so the constant is `POLL_INTERVAL: Duration = 50 ms`.
- **Commit:** 6b7e9ae

**6. [Test coverage] Extra tests**
- `fetch_parts_baseline_has_no_diff`, `fetch_parts_refuses_over_cap_and_unparsable_jobs`, `fetch_parts_refuses_a_source_that_breaks_its_contract`, `receive_out_of_order`, and `receive_nothing`.

### Not changed here

- The crate doc in `lib.rs` still describes the Phase 2 spike modules, initramfs included. The plan leaves it for 05-15, which rewrites it around the launch flow.
- `supervise.rs` still decodes the `MINOSST1` frame. That module belongs to 05-15.
- The Open question "How a backend receives blob bytes" is now answered in code. 05-15's acceptance criteria remove it, so it was left alone.
- Requirements: GUEST-04 stays pending, because 05-15, 05-17, and 05-19 finish it end to end. KRUN-04 was already complete. No requirement was marked.

## Known Stubs

None.

## Threat Flags

None. The two new surfaces, the blob-to-disk write and the guest-to-receiver socket, are T-05-13-01 to T-05-13-05 in the plan's threat model, and each is mitigated as planned.

## TDD Gate Compliance

Task 1 has RED 57fc6c5 then GREEN 6aae5b9. Task 2 has RED 01d3d9c then GREEN 6b7e9ae. In both RED runs the module failed to compile because the API under test did not exist yet (E0432 unresolved imports).

## Self-Check: PASSED

- FOUND: crates/minos-backend-krun/src/chunks.rs, crates/minos-backend-krun/src/input_disk.rs, crates/minos-backend-krun/src/lib.rs, CLAUDE.md
- GONE as intended: crates/minos-backend-krun/src/initramfs.rs
- FOUND commits: 57fc6c5, 6aae5b9, 01d3d9c, 6b7e9ae, e3801af
