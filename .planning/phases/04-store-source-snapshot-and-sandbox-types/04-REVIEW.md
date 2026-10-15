---
phase: 04-store-source-snapshot-and-sandbox-types
reviewed: 2026-10-15T15:18:17Z
depth: standard
files_reviewed: 41
files_reviewed_list:
  - crates/minos-cli/Cargo.toml
  - crates/minos-sandbox/Cargo.toml
  - crates/minos-sandbox/clippy.toml
  - crates/minos-sandbox/src/backend.rs
  - crates/minos-sandbox/src/finished.rs
  - crates/minos-sandbox/src/key.rs
  - crates/minos-sandbox/src/lib.rs
  - crates/minos-sandbox/src/output.rs
  - crates/minos-sandbox/src/preflight.rs
  - crates/minos-sandbox/src/role.rs
  - crates/minos-sandbox/src/spec.rs
  - crates/minos-sandbox/src/untrusted.rs
  - crates/minos-sandbox/tests/backend.rs
  - crates/minos-sandbox/tests/compile_fail.rs
  - crates/minos-sandbox/tests/output_tar.rs
  - crates/minos-sandbox/tests/preflight.rs
  - crates/minos-sandbox/tests/spec.rs
  - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
  - crates/minos-sandbox/tests/untrusted.rs
  - crates/minos-source/Cargo.toml
  - crates/minos-source/clippy.toml
  - crates/minos-source/src/clean.rs
  - crates/minos-source/src/denylist.rs
  - crates/minos-source/src/lib.rs
  - crates/minos-source/src/pack.rs
  - crates/minos-source/src/scan.rs
  - crates/minos-source/src/walk.rs
  - crates/minos-source/tests/clean.rs
  - crates/minos-source/tests/denylist.rs
  - crates/minos-source/tests/pack.rs
  - crates/minos-source/tests/refusals.rs
  - crates/minos-source/tests/scan.rs
  - crates/minos-store/clippy.toml
  - crates/minos-store/src/file/blobs.rs
  - crates/minos-store/src/file/ledger.rs
  - crates/minos-store/src/file/mod.rs
  - crates/minos-store/src/lib.rs
  - crates/minos-store/src/memory.rs
  - crates/minos-store/tests/contract.rs
  - crates/minos-store/tests/file_blobs.rs
  - crates/minos-store/tests/file_ledger.rs
findings:
  critical: 0
  warning: 3
  info: 6
  total: 9
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-10-15T15:18:17Z
**Depth:** standard
**Files Reviewed:** 41
**Status:** issues_found

## Summary

Reviewed the Phase 4 crates with the stance that each hostile-input boundary has a hole: the sandbox output tar reader and `Untrusted` exits (`minos-sandbox`), the file ledger and blob tree (`minos-store`), and the source walk, packer, secret scan, and git clean check (`minos-source`). For each boundary I traced the code against the pinned dependency sources rather than their docs: `tar` 0.4.46 (`next_entry_raw` verifies the checksum, ends raw iteration at the first zero block because `ignore_zeros` defaults to false, and applies no PAX size in raw mode, so the reader's `raw_file_position + padded size` end computation and its two-zero-block check are sound), and `ignore` 0.4.33 (`filter_entry` runs inside `skip_entry`, which exempts depth 0, so the project root can never be denylisted away; `matched_ignore` returns `m_custom_ignore.or(m_ignore).or(m_gi)`, so a `.gitignore` negation cannot re-include a `.minosignore` exclusion, and the only union-breaking shape is the `!` line the walk refuses). `cargo clippy -p minos-store -p minos-sandbox -p minos-source --all-targets -- -D warnings` is clean, and the C-02 grep for `#[allow(` prints nothing.

I did not find a provable bypass of any stated invariant: `LaunchPlan` and `Cleared` have only their one constructor and no `Clone`; `Untrusted` has no `Debug`, `Display`, `Deref`, or public field and its `new` is crate-private; `into_scorecard` exists only on judge output; the ledger mutates existing bytes only in the documented torn-tail truncation; blobs are rehashed on every `get` and refused when a symlink or wrong file type sits at any path component; the snapshot opens a file only after `lstat`, compares device, inode, type, and length on the opened handle, never follows a link, and always runs the denylist, caps, and secret scan before `--allow-dirty` can skip the git check; git runs as an argv array under `env_clear()` with a fixed four-variable environment.

The three warnings are places where the design is weaker than it could be at low cost: the ledger's tolerance for a newline-terminated non-JSON final line erases a fully synced line that this store can never have produced by a torn write; the launcher bounds that `minos-sandbox` restates from `minos-vmm` are tied together only by a comment; and `Finished::new` lets one clearance mint any number of "guest outputs" because it borrows the `Cleared` rather than consuming it. The info items are dead dependencies in the CLI stub, hard-coded copies of cap values in error text, and a few test-portability and defense-in-depth notes.

## Warnings

### WR-01: A newline-terminated garbage tail is dropped, then truncated on the next append, although this store can never produce that shape by a torn write

**File:** `crates/minos-store/src/file/ledger.rs:108-117` (tolerance), `crates/minos-store/src/file/ledger.rs:209-213` (truncation)
**Issue:** `parse` tolerates two tail shapes (D-13): bytes after the last newline, and a final newline-terminated line that is not JSON at all. The first is the only shape a crash can leave: `append` serializes the line, pushes the `\n`, and issues one `write_all` (lines 226-237), and serde_json never emits a raw newline inside a line, so a torn write is always a strict prefix of the buffer with no terminating newline. A final line that ends in `\n` but is not JSON therefore cannot be a torn write of this store. It can only be bit rot, a foreign writer, or tampering of a line that `sync_data` had already reported durable. `read` reports it as `torn_tail = 1` and hides the bytes; the next `append` then `set_len`s it away (line 210), after which the count is 0 forever and the event is gone. Every other line in the file gets `CorruptKind::BadJson` and a human for the same damage; only the last line is quietly erased. For a `Judged` or `Selected` line that is the last thing written before a pause, this rewrites history, which the ledger promises never to do. The tests `torn_tail_with_newline_is_tolerated` and `a_duplicate_append_still_truncates_the_torn_tail` lock the current behavior in.
**Fix:** Tolerate only the shape a crash can produce. Treat a newline-terminated line that fails to parse as `Corrupt { kind: BadJson }` regardless of position, and keep the unterminated-tail tolerance as is:

```rust
let line = match serde_json::from_slice::<Line>(text) {
    Ok(line) => line,
    Err(_not_a_line) => return Err(corrupt(run, number, CorruptKind::BadJson)),
};
```

and drop the `is_tail` / `serde_json::Value` probe. If D-13's second shape is kept on purpose (for example to tolerate a future writer that pads with a newline), record that reasoning in the module docs and make `append` refuse rather than truncate when the torn tail ends in `\n`, so a synced line is never removed silently. Update the two tests either way.

### WR-02: Launcher bounds are restated by hand in `minos-sandbox` with nothing but a comment keeping them equal to `minos-vmm`

**File:** `crates/minos-sandbox/src/spec.rs:21-36`
**Issue:** `MAX_VCPUS`, `MIN_MEMORY_MIB`, `MAX_MEMORY_MIB`, and `MAX_LAUNCH_MS` restate `crates/minos-vmm/src/config.rs:31-37`, and `BOOT_ALLOWANCE_MS` restates `HOST_DEADLINE_GRACE` in `crates/minos-backend-krun/src/supervise.rs:70`. They agree today (8, 128, 16 384, 3 600 000, 30 s). The comment says "change both together", but `minos-vmm` depends on no internal crate, so no code path and no test ties them. If `minos-vmm` ever lowers a bound, `preflight` keeps clearing plans the launcher then refuses with exit 64/65, which the K7 table classifies as an infrastructure failure, so the runner retries a plan that can never launch until `max_infra_failures` trips the circuit breaker, with no message naming the cause.
**Fix:** Pin the literals on both sides so drift fails the gate. In `crates/minos-vmm` add a unit test that asserts the four config constants equal the literal values `minos-sandbox` documents, and in `crates/minos-sandbox/tests/spec.rs` assert the sandbox constants equal the same literals with a comment pointing at the vmm test; a change to either file then has to touch both. Alternatively, move the bounds into `minos_core::limits` (both `minos-sandbox` and `minos-backend-krun` already depend on core) and have `minos-vmm`'s test pin against the core literal values by hand, since vmm may not link core.

### WR-03: `Finished::new` borrows the clearance, so one `Cleared` can mint unlimited fabricated outputs of its role

**File:** `crates/minos-sandbox/src/finished.rs:63`, `crates/minos-sandbox/src/backend.rs:77`
**Issue:** The module docs state the invariant "output of a role exists only for a cleared launch of that role", and `Finished::new(cleared: &Cleared<R>, ended: Ended, stream: Vec<u8>)` is `pub`. Because it takes `&Cleared<R>`, any holder of a clearance (every backend, and the runner between `preflight` and `launch`) can build as many `Finished<R>` values as it likes, each with any `Ended` and any bytes, without a launch ever happening, and `into_output` then labels those bytes as role-`R` guest output. The invariant that holds today is the weaker "a `Cleared<R>` existed at some point". The fake backends in `tests/backend.rs:77` and `tests/ui/pass/judge_flow.rs:42` already hold the `Cleared` by value inside `wait(self)`, so consuming it costs nothing.
**Fix:** Take the clearance by value so a launch yields at most one `Finished`:

```rust
pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self {
    Self { key: *cleared.plan().key(), ended, stream, role: PhantomData }
}
```

and update `RunningVm::wait(self)` implementers to move their stored `Cleared` in. This also closes the path where a backend reports `Exited` for one plan and reuses the same clearance to report a second, different `Finished` for it.

## Info

### IN-01: The CLI manifest declares eleven dependencies for a three-line stub

**File:** `crates/minos-cli/Cargo.toml:17-34`
**Issue:** `main.rs` is `fn main() {}`. `clap`, `anyhow`, `tracing`, `tracing-subscriber`, `tar`, `sha2`, `toml`, and `serde` are unused direct dependencies, and `minos-backend-krun`, `minos-propose`, `minos-runner`, `minos-loop`, `minos-store`, `minos-sandbox`, and `minos-source` are linked for nothing. Cargo and clippy do not flag unused crate dependencies, so these will only be noticed if they go stale. `tar`, `sha2`, `toml`, and `serde` in particular look like leftovers: the CLI is "the only place concrete types meet", and hashing, tar, and TOML parsing belong to `minos-core` and `minos-source`.
**Fix:** Add dependencies to `minos-cli` in the plan that uses them (Phase 8), or at least drop `tar`, `sha2`, `toml`, and `serde` now and re-add them with their first use.

### IN-02: Cap values are copied into user-facing text instead of formatted from the constants

**File:** `crates/minos-source/src/lib.rs:93`, `crates/minos-source/src/lib.rs:99`, `crates/minos-source/src/lib.rs:101`, `crates/minos-source/src/clean.rs:100`, `crates/minos-source/src/clean.rs:113`, `crates/minos-source/src/clean.rs:116`
**Issue:** "16 MiB per-file cap", "255 bytes, final component 100", the grammar text, "30 s", "more than 16 MiB", and "/usr/bin:/bin:/usr/sbin:/sbin" are literals in `Display` and `#[error]` strings while `MAX_SNAPSHOT_FILE_BYTES`, `GIT_DEADLINE`, `GIT_STDOUT_CAP`, and `GIT_PATH` hold the real values. A change to a constant leaves the message wrong, and no test compares them.
**Fix:** Interpolate the constants (`{MAX_SNAPSHOT_FILE_BYTES}`, `{GIT_PATH}`, `GIT_DEADLINE.as_secs()`), or add one test per message asserting it contains the constant's rendered value.

### IN-03: `Appended::Conflict` is an `Ok` value whose docs say the caller has a bug

**File:** `crates/minos-store/src/lib.rs:55-63`, `crates/minos-store/src/lib.rs:231-243`
**Issue:** A same-key append with a different record returns `Ok(Appended::Conflict { seq })`. The docs call that a caller bug (a re-run trial with different metrics), but nothing stops a caller writing `ledger.append(..)?;` and continuing as if the record were stored. `#[must_use]` on the enum only fires when the whole result is discarded, not when it is `?`-propagated and dropped.
**Fix:** Either return `Err(StoreError::Conflict { seq })` (non-retryable, "needs a human", like `Corrupt`), or add `Appended::is_written(&self) -> bool` and state in `Ledger::append`'s docs that the loop must stop the run on `Conflict`. Phase 7's loop should be written against whichever is chosen.

### IN-04: The clean-check unit tests hard-code macOS tool paths and never prove that `git` is resolved through `GIT_PATH` alone

**File:** `crates/minos-source/src/clean.rs:294-375`
**Issue:** The tests spawn `/bin/sleep`, `/usr/bin/yes`, `/usr/bin/false`, and `/usr/bin/env`. On a Linux runner (which CLAUDE.md says `cargo test --workspace` is waiting for) `/usr/bin/false` and `/usr/bin/yes` depend on usrmerge and `yes` may not exist at that path. Separately, the module docs promise "resolved through `GIT_PATH` only, so a `git` elsewhere is not found", which rests on std's Unix `Command` using the child's `PATH` for lookup once `env("PATH", ..)` is set (it does: a modified `PATH` forces the fork/exec path where `environ` is replaced before `execvp`), but no test exercises it; `a_program_that_does_not_exist_is_unavailable` uses an absolute path.
**Fix:** Resolve the helper programs at test time (`which`-style search over a few candidates, skipping the test when none exists), and add a test that puts a fake `git` on the parent process's `PATH` only (a shell script in a `TempDir` that exits 0 and prints nothing) and asserts `run_git("git", ..)` still returns `GitUnavailable` or `GitFailed`, proving the parent `PATH` is not consulted.

### IN-05: Ledger and lock files are created with the process umask while every directory is 0700

**File:** `crates/minos-store/src/file/mod.rs:222`, `crates/minos-store/src/file/ledger.rs:179-183`, `crates/minos-store/src/file/ledger.rs:203-206`
**Issue:** `open_or_create` uses `create_new(true)` on the caller's `OpenOptions` without `.mode(0o600)`, so `ledger.jsonl` and `ledger.lock` get `0666 & ~umask`, usually `0644`. Blob temp files from `tempfile` are `0600`, so the store is inconsistent. The 0700 directories protect the files today, which is why this is informational, but a store root created under a looser parent (or a user who later relaxes the run directory) would expose the ledger.
**Fix:** Add `use std::os::unix::fs::OpenOptionsExt;` and `.mode(0o600)` on the `create_new` clone in `open_or_create`, and assert the mode in `open_creates_a_missing_root_with_mode_0700`.

### IN-06: The snapshot reads file bytes before the clean check, so a write between the two can pass "clean" with uncommitted bytes

**File:** `crates/minos-source/src/lib.rs:340-378`
**Issue:** `walk::collect` reads every file into memory, then `check_clean` runs `git status` and `git ls-files`. A file modified after the read and reverted before `git status` (or modified after `git status` runs and before the process ends, which does not affect the bytes already read) yields a `TreeId` over bytes that are not the committed ones, while the ledger records the run as having started from a clean tree. The window is small and the tree is the user's own, so this is a note, not a defect; `--allow-dirty` exists precisely because the check is advisory.
**Fix:** If a stronger statement is wanted later, run `check_clean` both before the walk and after it and require both to pass, or compare each packed file's bytes against `git cat-file` for its index blob (which would reintroduce git output into the trusted path, so probably not). Otherwise document the window in `snapshot`'s docs.

---

_Reviewed: 2026-10-15T15:18:17Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
