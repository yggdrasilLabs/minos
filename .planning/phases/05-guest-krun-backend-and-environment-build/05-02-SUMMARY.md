---
phase: 05-guest-krun-backend-and-environment-build
plan: 02
subsystem: minos-core
tags: [chunk-protocol, vsock, token, minos-lock, golden, guest-04, cli-02, d-10, d-11]
status: complete
requires: []
provides:
  - "minos-core::chunk: CHUNK_MAGIC, TOKEN_BYTES, CHUNK_HEADER_BYTES, MAX_CHUNK_PAYLOAD, MAX_CHUNK_BYTES, END_PAYLOAD_BYTES, KIND_DATA, KIND_END"
  - "ChunkToken (from_bytes, constant-time ct_eq, redacted Debug), ChunkStream (new, chunk_count, Iterator), ChunkFault (16 variants, is_incomplete, reason), Reassembler (new, accept, is_complete, connections, finish)"
  - "minos-core::lock: LOCK_VERSION, MAX_LOCK_BYTES, MAX_BUILDER_TEXT_BYTES, BuilderText, BuilderId, LockedImage, Lock (empty, parse_toml, to_toml, image, with_image, check), RawLock, RawLockedImage, RawBuilder, LockError, LockMismatch"
  - "Crate-root re-exports ChunkToken, Reassembler, Lock"
affects:
  - minos-guest (sends output with ChunkStream, 05-08 onward)
  - minos-backend-krun (receives with Reassembler, classifies NoEnd separately from ChunkProtocol)
  - minos-cli env build (writes minos.lock with with_image, refuses a changed digest unless --update)
  - Phase 7 and 8 (call Lock::check before a run)
tech-stack:
  added: []
  patterns:
    - "One wire codec in minos-core shared by guest sender and host receiver"
    - "Receiver poisoned after its first fault: later accept and finish return the same fault"
    - "Golden TOML snapshot plus a SHA-256 pin computed outside the code with awk and shasum"
key-files:
  created:
    - crates/minos-core/src/chunk.rs
    - crates/minos-core/src/lock.rs
    - crates/minos-core/tests/chunk.rs
    - crates/minos-core/tests/lock.rs
    - crates/minos-core/tests/snapshots/lock__minos_lock_v1.snap
  modified:
    - crates/minos-core/src/lib.rs
decisions:
  - "A valid chunk stream has one form: every DATA chunk is full except the last and none is empty, so the receiver refuses an empty DATA at accept and a short DATA before the last at finish, both as ChunkFault::Length"
  - "An END whose seq differs from its own DATA count is ChunkFault::Totals at accept; an END claiming a total over max_total is ChunkFault::OverCap at accept"
  - "ChunkStream::new borrows the token for the stream's lifetime (&'a ChunkToken) rather than copying it, so the token is never duplicated"
  - "RawBuilder holds plain Strings and BuilderText validation runs in try_from, so a bad builder text is LockError::BuilderText rather than an opaque TOML error"
  - "An empty Lock serializes as `v = 1` alone (image is skipped when empty and defaults on parse); [[image]] order on parse is free, and serialization is always sorted by (backend, arch)"
metrics:
  duration: "about 9 minutes"
  completed: 2026-10-18
  tasks: 2
  files: 6
---

# Phase 5 Plan 02: Authenticated chunk codec and minos.lock Summary

`minos-core` now has the guest-to-host output codec and the `minos.lock` type. Each output chunk is a 48-byte header (magic `MINOSCK1`, a 32-byte per-launch token, seq, kind, reserved, len) plus at most 4096 payload bytes, so it fits one vsock connection of at most 4144 bytes. The END trailer binds the whole stream with its length, SHA-256, and DATA count. `minos.lock` is a validated TOML wire form pinned by a golden snapshot, and it has the `check` and `with_image` operations that D-10 and D-11 need.

## What was built

**Task 1: chunk codec (commits f17654d RED, 4df5781 GREEN)**
- `chunk.rs`:
  - Constants, with const assertions that the header is 48 bytes, the END payload is 44 bytes, `MAX_CHUNK_BYTES == 48 + 4096`, and `MAX_CHUNK_BYTES <= 8192`.
  - `ChunkToken`: `Clone` only. It has no `Copy`, `PartialEq`, `Display`, or serde, and its `Debug` prints `ChunkToken(..)`. `ct_eq` checks the length, then folds an OR of XORs over all 32 bytes behind `black_box`, so there is no early return.
  - `ChunkStream`: a fused iterator built on `slice::chunks`. A stream too long for a `u32` seq ends without an END, which the receiver reports as NoEnd.
  - `ChunkFault`: 16 variants with fixed, numbers-only messages. `is_incomplete` is true only for `NoEnd`. `reason` matches every variant explicitly and returns `Reason::ChunkProtocol`.
  - `Reassembler`: counts the connection first, then checks it, then buffers DATA by seq in a `BTreeMap`. The byte cap and `max_connections = ceil(max_total/4096)+1` are checked before anything is buffered. Its first fault poisons it for every later call.
- `tests/chunk.rs`: 29 tests. They cover the byte layout of DATA and END chunks, `chunk_count`, the empty stream, a proptest that every chunk fits one connection, a proptest that any arrival order reassembles to the same bytes, one test for each fault, poisoning, the fault classes, the redacted Debug, and `ct_eq`.

**Task 2: minos.lock (commits 396c49c RED, 3eff22e GREEN)**
- `lock.rs`:
  - Each validated type (`Lock`, `LockedImage`, `BuilderId`, `BuilderText`) deserializes through `try_from` and serializes through `into`.
  - The raw types `RawLock`, `RawLockedImage`, and `RawBuilder` use `deny_unknown_fields`.
  - Parsing refuses text over 64 KiB before it reaches the TOML parser. It also refuses `v != 1`, a duplicate (backend, arch) pair, `base_images` that are not strictly ascending, and builder text that is not 1 to 128 ASCII graphic bytes.
  - `LockedImage::new` sorts and dedups `base_images`.
  - `Lock::check` returns `NoImage`, `Dockerfile { locked, found }`, or `Guest { locked, found }`. The message prints both digests and says to rerun `minos env build`.
- `tests/lock.rs`: 14 tests, including the golden snapshot with its SHA-256 pin (`a495cdba...43dfdb`, computed with `awk 'f;/^---$/{c++; if(c==2) f=1}' ... | shasum -a 256`), round trip, refusals, check, replace, and deserializing through validation.
- The snapshot was written once with `INSTA_UPDATE=always`, reviewed (`v = 1`, one `[[image]]`, an `[image.builder]` subtable), and then the tests were rerun with `INSTA_UPDATE` unset.

## Verification

- `cargo fmt --all --check`: passes
- `cargo clippy --workspace --all-targets -- -D warnings`: passes
- `cargo test --workspace` (INSTA_UPDATE unset): 59 suites, all `ok`
- `cargo deny check`: advisories, bans, licenses, and sources all ok. The existing "unmatched wrapper" warning is unrelated, and this plan changed no dependencies.
- `cargo doc --workspace --no-deps`: 0 warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: passes
- C-01: no MISSING lines. C-02: the grep prints nothing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Correctness] A valid stream has exactly one form**
- **Found during:** Task 1
- **Issue:** The plan's fault list did not cover empty DATA chunks or short DATA chunks before the last. Without those checks, many different streams could reassemble to the same bytes, and a stream could spend its connection budget on tiny chunks.
- **Fix:** An empty DATA chunk is `ChunkFault::Length` at accept. A non-full DATA chunk before the last is `ChunkFault::Length` at finish. `ChunkStream` never produces either. The `Length` doc lists every mis-sized case.
- **Files modified:** crates/minos-core/src/chunk.rs, crates/minos-core/tests/chunk.rs
- **Commit:** 4df5781

**2. [Rule 2 - Correctness] The END's own fields are checked at accept**
- An END whose seq differs from its DATA count is `Totals`. An END claiming a total over the cap is `OverCap`. DATA that arrived before the END with a seq at or past its count is `AfterEnd` when the END arrives.
- **Commit:** 4df5781

**3. [Signature adjustment] `ChunkStream::new(token: &'a ChunkToken, ...)`**
- The stream borrows the token for its lifetime instead of copying the bytes, which keeps the rule that the token is never duplicated silently.
- **Commit:** 4df5781

**4. [Test coverage] Extra lock tests**
- `lock_refuses_missing_keys`, `lock_refuses_oversize_text`, `lock_sorts_base_images`, `lock_golden_is_v1_with_one_image`, and `lock_deserializes_through_validation`, beyond the nine behaviors the plan listed.
- **Commit:** 396c49c

### Not changed here

- CLAUDE.md: plans 05-13 (chunk protocol and token) and 05-20 (core module list with `chunk.rs` and `lock.rs`) own these edits, so this plan left CLAUDE.md untouched.

## Known Stubs

None.

## TDD Gate Compliance

Each task has a `test(...)` RED commit followed by a `feat(...)` GREEN commit: f17654d then 4df5781, and 396c49c then 3eff22e. In both RED runs the tests failed to compile because the module did not exist yet. Task 2's RED commit held a placeholder pin, which was replaced by the hash computed outside the code once the snapshot was accepted.

## Self-Check: PASSED

- FOUND: crates/minos-core/src/chunk.rs, crates/minos-core/src/lock.rs, crates/minos-core/tests/chunk.rs, crates/minos-core/tests/lock.rs, crates/minos-core/tests/snapshots/lock__minos_lock_v1.snap
- FOUND commits: f17654d, 4df5781, 396c49c, 3eff22e
