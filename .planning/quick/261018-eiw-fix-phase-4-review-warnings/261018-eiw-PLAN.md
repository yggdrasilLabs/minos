---
phase: quick-261018-eiw
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/minos-store/src/file/ledger.rs
  - crates/minos-store/tests/file_ledger.rs
  - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md
  - CLAUDE.md
  - crates/minos-vmm/src/config.rs
  - crates/minos-sandbox/src/spec.rs
  - crates/minos-sandbox/tests/spec.rs
  - crates/minos-backend-krun/src/supervise.rs
  - crates/minos-sandbox/src/finished.rs
  - crates/minos-sandbox/src/preflight.rs
  - crates/minos-sandbox/src/backend.rs
  - crates/minos-sandbox/src/lib.rs
  - crates/minos-sandbox/tests/backend.rs
  - crates/minos-sandbox/tests/untrusted.rs
  - crates/minos-sandbox/tests/output_tar.rs
  - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
  - crates/minos-sandbox/tests/ui/fail/finished_twice.rs
  - crates/minos-sandbox/tests/ui/fail/finished_twice.stderr
  - crates/minos-sandbox/tests/compile_fail.rs
autonomous: true
requirements: [STORE-02, SBX-02, SBX-03, SBX-06]

must_haves:
  truths:
    - "WR-01: a ledger whose final line ends in a newline but is not a valid Line is refused by both read and append as StoreError::Corrupt { kind: BadJson } at that 1-based line, and the file bytes stay unchanged, including when the append is a Duplicate of an existing key"
    - "WR-01: bytes after the last newline are still the only tolerated torn tail: read reports torn_tail 1, and the next append (New or Duplicate) truncates only those bytes"
    - "WR-01: CLAUDE.md's minos-store paragraph, the ledger.rs module docs, and an amendment under 04-CONTEXT D-13 all describe the same single torn-tail shape the code accepts"
    - "WR-02: changing any launcher bound (minos-vmm MAX_VCPUS, MIN_MEMORY_MIB, MAX_MEMORY_MIB, MAX_DURATION_MS; minos-sandbox MAX_VCPUS, MIN_MEMORY_MIB, MAX_MEMORY_MIB, MAX_LAUNCH_MS, BOOT_ALLOWANCE_MS; minos-backend-krun HOST_DEADLINE_GRACE) without also changing the other side and both pin tests fails cargo test --workspace"
    - "WR-02: minos-vmm still depends on no internal crate, and no Cargo.toml or deny.toml changes"
    - "WR-03: Finished::new takes Cleared<R> by value, so one clearance yields at most one Finished, and a trybuild compile-fail case proves a second Finished::new from the same Cleared does not compile (E0382)"
    - "WR-03: the Backend::launch and RunningVm::wait signatures are unchanged, every existing trybuild .stderr is byte-identical, and the pass case judge_flow.rs still compiles and runs"
    - "The full gate passes: fmt, workspace clippy, workspace tests, cargo deny, cargo doc, musl guest clippy, and the C-02 allow grep prints nothing"
  artifacts:
    - path: "crates/minos-store/src/file/ledger.rs"
      provides: "parse() that refuses every newline-terminated non-Line as BadJson and tolerates only unterminated tail bytes"
      contains: "CorruptKind::BadJson"
    - path: "crates/minos-store/tests/file_ledger.rs"
      provides: "refusal tests for newline-terminated garbage final lines and the duplicate-append case"
      contains: "fn a_duplicate_append_never_erases_a_damaged_final_line"
    - path: "crates/minos-vmm/src/config.rs"
      provides: "unit test pinning the four launcher bounds to literals"
      contains: "fn launcher_bounds_are_pinned_for_minos_sandbox"
    - path: "crates/minos-sandbox/tests/spec.rs"
      provides: "test pinning the restated sandbox bounds to the same literals"
      contains: "fn launcher_bounds_match_minos_vmm"
    - path: "crates/minos-backend-krun/src/supervise.rs"
      provides: "unit test pinning HOST_DEADLINE_GRACE to 30 s"
      contains: "fn host_deadline_grace_is_pinned_for_minos_sandbox"
    - path: "crates/minos-sandbox/src/finished.rs"
      provides: "Finished::new consuming its Cleared"
      contains: "pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self"
    - path: "crates/minos-sandbox/src/preflight.rs"
      provides: "crate-private consuming accessor used by Finished::new"
      contains: "pub(crate) fn into_key(self) -> LaunchKey"
    - path: "crates/minos-sandbox/tests/ui/fail/finished_twice.rs"
      provides: "compile-fail case: one Cleared cannot build two Finished values"
    - path: "crates/minos-sandbox/tests/ui/fail/finished_twice.stderr"
      provides: "the reviewed E0382 stderr for the new case"
      contains: "E0382"
  key_links:
    - from: "crates/minos-store/src/file/ledger.rs parse()"
      to: "Ledger::append set_len truncation"
      via: "Parsed.torn_start is Some only for bytes after the last newline"
      pattern: "torn_start"
    - from: "crates/minos-sandbox/src/finished.rs Finished::new"
      to: "crates/minos-sandbox/src/preflight.rs Cleared::into_key"
      via: "by-value call that moves the clearance"
      pattern: "into_key\\(\\)"
    - from: "crates/minos-sandbox/tests/spec.rs launcher_bounds_match_minos_vmm"
      to: "crates/minos-vmm/src/config.rs launcher_bounds_are_pinned_for_minos_sandbox"
      via: "the same literal values (8, 128, 16384, 3600000) pinned on both sides"
      pattern: "3_600_000"
---

<objective>
Fix the three warnings in `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-REVIEW.md`, one task each, and close with the full gate.

- WR-01: the file ledger stops treating a newline-terminated, non-JSON final line as a torn tail. Only bytes after the last newline are tolerated, so a line that `sync_data` may already have reported durable is never silently truncated.
- WR-02: the launcher bounds that `minos-sandbox` restates from `minos-vmm` (and the 30 s figure it shares with `minos-backend-krun`) get pinned to literals in tests on both sides, so drift fails the gate. `minos-vmm` keeps depending on no internal crate.
- WR-03: `Finished::new` consumes the `Cleared<R>`, so one clearance yields at most one `Finished`. A new trybuild compile-fail case proves it.

Purpose: each fix strengthens a stated invariant: the ledger never rewrites history, preflight never clears a plan the launcher refuses, and output exists only once per cleared launch. None of them weakens one.
Output: code, tests, the doc-of-record updates (CLAUDE.md, 04-CONTEXT D-13 amendment), three atomic commits, and a green full gate.

Out of scope: IN-01 through IN-06 in 04-REVIEW.md. Do not touch them. Do not run `cargo clean` (an end-of-phase rule; this is a quick task).
</objective>

<execution_context>
@$HOME/.claude/gsd-core/workflows/execute-plan.md
@$HOME/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@./CLAUDE.md
@./.claude/CLAUDE.md
@.planning/phases/04-store-source-snapshot-and-sandbox-types/04-REVIEW.md
@crates/minos-store/src/file/ledger.rs
@crates/minos-sandbox/src/finished.rs
@crates/minos-sandbox/src/backend.rs

Rules that bind every task (CLAUDE.md "Code rules", `rules/`):
- `unwrap_used`, `expect_used`, and `indexing_slicing` are `forbid` in tests too. Tests return `Result<(), Box<dyn Error>>` (the local `TestResult` alias) and use `?`, `.get(..)`, and `.ok_or(..)`.
- Workspace clippy turns on `pedantic` at warn, and the gate runs with `-D warnings`. No allow or expect attribute may be added anywhere: the C-02 grep must print nothing. Write code so the lint never fires.
- `cargo doc` denies broken intra-doc links. When a doc comment names an item in a crate that is not a dependency (for example, `minos-sandbox` naming a `minos-vmm` constant), use plain backticks, not a bracketed link.
- Never weaken a security invariant to make something compile or pass. If one blocks you, stop and report it.

<interfaces>
Current shapes the executor changes (from the files above, verified 2026-10-18):

```rust
// crates/minos-store/src/file/ledger.rs, inside parse(run, bytes) (lines 78-119 today)
let mut newlines = bytes.iter().enumerate()
    .filter_map(|(at, byte)| (*byte == b'\n').then_some(at))
    .peekable();
while let Some(end) = newlines.next() {
    // ... text = bytes.get(start..end); after = end + 1;
    let is_tail = newlines.peek().is_none() && after == bytes.len();
    // ... line-cap check, version probe ...
    let line = match serde_json::from_slice::<Line>(text) {
        Ok(line) => line,
        Err(_not_a_line) if is_tail && /* text is not any JSON value */ => {
            return Ok(Parsed { lines, torn_start: Some(len_u64(start)) });
        }
        Err(_not_a_line) => return Err(corrupt(run, number, CorruptKind::BadJson)),
    };
    // ... verify, seq ...
}
// after the loop: unterminated rest > MAX_LEDGER_LINE_BYTES is LineTooLong; rest > 0 sets torn_start.
```

```rust
// crates/minos-sandbox/src/finished.rs (today)
impl<R: Role> Finished<R> {
    #[must_use]
    pub fn new(cleared: &Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self; // borrows: WR-03
}

// crates/minos-sandbox/src/preflight.rs (today)
pub struct Cleared<R: Role> { plan: LaunchPlan<R> }          // private field, no Clone, no Default
impl<R: Role> Cleared<R> { pub const fn plan(&self) -> &LaunchPlan<R>; }
// LaunchPlan::key(&self) -> &LaunchKey, and LaunchKey is Copy.

// crates/minos-sandbox/src/backend.rs (today, and UNCHANGED by this plan)
pub trait RunningVm<R: Role> { fn wait(self) -> Result<Finished<R>, InfraError>; }
pub trait Backend {
    type Running<R: Role>: RunningVm<R>;
    fn capabilities(&self) -> Capabilities;
    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>;
}
```

Resulting signatures after WR-03:

```rust
// crates/minos-sandbox/src/finished.rs
pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self; // body: key: cleared.into_key()
// crates/minos-sandbox/src/preflight.rs
impl<R: Role> Cleared<R> { #[must_use] pub(crate) fn into_key(self) -> LaunchKey; } // returns *self.plan.key()
// Backend::launch and RunningVm::wait: unchanged.
```

Bounds pinned by WR-02 (all agree today):

| Side | Constant | Value |
|---|---|---|
| `crates/minos-vmm/src/config.rs` | `MAX_VCPUS: u8` | 8 |
| | `MIN_MEMORY_MIB: u16` | 128 |
| | `MAX_MEMORY_MIB: u16` | 16_384 |
| | `MAX_DURATION_MS: u64` | 3_600_000 |
| `crates/minos-sandbox/src/spec.rs` | `MAX_VCPUS`, `MIN_MEMORY_MIB`, `MAX_MEMORY_MIB` | 8, 128, 16_384 |
| | `MAX_LAUNCH_MS: u64` (mirrors vmm `MAX_DURATION_MS`) | 3_600_000 |
| | `BOOT_ALLOWANCE_MS: u64` | 30_000 |
| `crates/minos-backend-krun/src/supervise.rs` | `HOST_DEADLINE_GRACE: Duration` | 30 s |

Call sites of `Finished::new` (all in `minos-sandbox` tests; no backend crate calls it or implements `Backend` yet): `tests/untrusted.rs` (2), `tests/backend.rs` (1, inside `FakeRunning::wait`), `tests/output_tar.rs` (3), `tests/ui/pass/judge_flow.rs` (1, inside `FakeRunning::wait`).
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: WR-01, refuse a newline-terminated non-Line final ledger line; keep only the unterminated torn tail</name>
  <files>crates/minos-store/src/file/ledger.rs, crates/minos-store/tests/file_ledger.rs, CLAUDE.md, .planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md</files>
  <behavior>
    - A ledger of two store-written lines plus an incomplete JSON object and a newline: read and append both return Corrupt { line: 3, kind: BadJson }, and the file bytes are unchanged.
    - A ledger of one line plus a blank line (a lone newline): Corrupt { line: 2, kind: BadJson } for read and append, bytes unchanged.
    - A ledger of one line plus a few NUL bytes and a newline: Corrupt { line: 2, kind: BadJson } for read and append, bytes unchanged.
    - A ledger whose first line is steered(1, 0, "note") and whose second line is an incomplete JSON object plus a newline: appending steered(1, 0, "note") again (an existing key) returns Corrupt { line: 2, kind: BadJson } and leaves the bytes unchanged. Before this fix, it returned Duplicate and truncated the line.
    - Unchanged: unterminated bytes after the last newline still read as torn_tail 1 and are truncated by the next append, both New and Duplicate (the existing tests for those cases stay as they are).
  </behavior>
  <action>
RED first. In `crates/minos-store/tests/file_ledger.rs`, delete the second test under the "Torn tails" banner: the one that appends an incomplete JSON object followed by a newline and asserts it is tolerated and healed. It encodes the behavior WR-01 removes. Keep `torn_tail_without_newline_is_tolerated` and `a_duplicate_append_still_truncates_the_torn_tail` unchanged, because both use unterminated tails, which stay tolerated. Under the "Refusals" banner, add two tests built from the existing helpers (`encoded`, `joined`, `steered`, `write_raw`, `ledger_path`, `ledger_bytes`, `expect_corrupt`, `assert_refused`):
(a) `a_newline_terminated_final_line_that_is_not_a_line_is_refused`: three `assert_refused` calls for the three garbage shapes in the behavior block: an incomplete object plus a newline at line 3 after two encoded lines, a lone newline at line 2, and NUL bytes plus a newline at line 2, each with `CorruptKind::BadJson`.
(b) `a_duplicate_append_never_erases_a_damaged_final_line`: in a temp dir, open a FileStore, `write_raw` one encoded steered(1, 0, "note") line followed by an incomplete object and a newline, then call `expect_corrupt(store.append(&run(), steered(1, 0, b"note")), 2, CorruptKind::BadJson)` and assert `ledger_bytes` equals the bytes written.
Give each test a one-line doc comment saying why: an append writes its line and newline in one write_all, so a crash cannot leave a newline-terminated non-line, and truncating one could erase a line already reported durable (review WR-01). Run `cargo test -p minos-store --test file_ledger`. The two new tests must FAIL, with expect_corrupt reporting Ok or Duplicate. Record that output for the SUMMARY.

GREEN. In `crates/minos-store/src/file/ledger.rs` `parse`, delete the guarded fallback arm that returned `Parsed { torn_start: Some(..) }` for a final newline-terminated line. That arm re-parsed the line as an arbitrary JSON value. Also delete the last-line flag computed from the peeked iterator. Any newline-terminated line that fails to deserialize as `Line` now returns `corrupt(run, number, CorruptKind::BadJson)`, wherever it sits. Drop `.peekable()` from the newline iterator, and turn the loop into a `for end in newlines` loop, since a while-let over `next()` with no peek trips clippy's `while_let_on_iterator`. The line-cap check, the version probe, verify, the seq check, the post-loop unterminated-rest handling (LineTooLong over the cap, else `torn_start`), and `append`'s `set_len` path all stay as they are. Rewrite the module docs. Item 3 of "Reading a ledger" becomes: a line that is not a `Line` is `CorruptKind::BadJson` wherever it is, the final line included. "The torn tail" section becomes: `append` serializes one line, pushes its newline, and issues one `write_all`, and serde_json never emits a raw newline inside a line, so a torn write is always a strict prefix with no newline after it. Bytes after the last newline are therefore the one tolerated shape (04-CONTEXT D-13 as amended 2026-10-18; review WR-01). `read` drops them and reports a count, never the bytes. The next append truncates them with `set_len` under the exclusive lock, which is the only mutation of existing bytes and removes only bytes no append reported written. A newline-terminated final line that is not a `Line` is not a torn write of this store. The store cannot tell it from damage to a line `sync_data` already reported durable, so it is refused as BadJson and needs a human, because truncating it would erase history. Keep the existing sentence about unterminated bytes over the line cap.

Docs of record, in the same change (CLAUDE.md "Keep this file current"). In the CLAUDE.md `## minos-store` section's ledger paragraph, rewrite the sentence that starts "A torn tail is tolerated:" so the only shape it names is bytes after the last newline. Add the reason: an append writes its line and newline in one `write_all`, and serde_json never emits a raw newline. In the same paragraph's refusal list, drop the word "elsewhere" after "valid JSON that is not a `Line`", and say the final newline-terminated line is included. In `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md`, add a header line `**Amended:** 2026-10-18 (D-13; quick 261018-eiw, review WR-01)` under `**Status:**`. Add a sub-bullet directly under D-13, in the style of 03-CONTEXT's amendments, beginning `  - **Amendment (2026-10-18, quick 261018-eiw, review WR-01):**`. It states that only bytes after the last newline are a torn tail, and that a final line ending in a newline that is not a `Line` is BadJson like any other line, with the write_all reason and the erase-history reason. Do not rewrite the original D-13 text.

Then run the verify commands and commit only these four files with the message `fix(quick-261018-eiw): refuse a newline-terminated non-line final ledger line (WR-01)`. Add a short body naming the review item, and end the message with the line `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
  </action>
  <verify>
    <automated>cargo test -p minos-store && cargo clippy -p minos-store --all-targets -- -D warnings && cargo fmt --all --check && ! grep -q 'serde_json::Value' crates/minos-store/src/file/ledger.rs && ! grep -q 'is_tail' crates/minos-store/src/file/ledger.rs && ! grep -q 'not valid JSON at all' crates/minos-store/src/file/ledger.rs && ! grep -q 'not valid JSON at all' CLAUDE.md && ! grep -q 'fn torn_tail_with_newline_is_tolerated' crates/minos-store/tests/file_ledger.rs && grep -q 'fn a_duplicate_append_never_erases_a_damaged_final_line' crates/minos-store/tests/file_ledger.rs && grep -q 'fn a_newline_terminated_final_line_that_is_not_a_line_is_refused' crates/minos-store/tests/file_ledger.rs && grep -q 'fn torn_tail_without_newline_is_tolerated' crates/minos-store/tests/file_ledger.rs && grep -q 'Amendment (2026-10-18, quick 261018-eiw, review WR-01)' .planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md && test -z "$(grep -rnE '#!?\[allow\(' crates --include='*.rs')"</automated>
  </verify>
  <done>The two new refusal tests failed before the parse change (RED output recorded) and pass after it. Every other minos-store test, including both unterminated torn-tail tests and the two proptests, passes. ledger.rs has no generic-JSON fallback and no last-line flag. CLAUDE.md, the ledger.rs module docs, and the D-13 amendment all name only the unterminated tail as tolerated. One commit `fix(quick-261018-eiw): ...` touches exactly the four listed files.</done>
</task>

<task type="auto">
  <name>Task 2: WR-02, pin the launcher bounds to literals on both sides so drift fails the gate</name>
  <files>crates/minos-vmm/src/config.rs, crates/minos-sandbox/src/spec.rs, crates/minos-sandbox/tests/spec.rs, crates/minos-backend-krun/src/supervise.rs</files>
  <action>
Use the review's first option (pinned literals on both sides). A shared constant is not possible without breaking the deny.toml layering. `minos-vmm` is a binary with no library target and depends on no internal crate, so neither side can import the other's constant. `minos-backend-krun` does not depend on `minos-sandbox` yet, and Phase 5 decides that wiring. Add no dependency, and change no Cargo.toml or deny.toml.

(1) `crates/minos-vmm/src/config.rs`: in the existing `#[cfg(test)] mod tests`, add `#[test] fn launcher_bounds_are_pinned_for_minos_sandbox()`. It asserts `super::MAX_VCPUS == 8`, `super::MIN_MEMORY_MIB == 128`, `super::MAX_MEMORY_MIB == 16_384`, and `super::MAX_DURATION_MS == 3_600_000` with `assert_eq!`. That form passes clippy here, as the precedent `assert_eq!(EXIT_WATCHDOG, 71)` in main.rs shows. Give it a doc comment: `crates/minos-sandbox/src/spec.rs` restates these four bounds (`MAX_DURATION_MS` as `MAX_LAUNCH_MS`) so preflight never clears a plan this launcher refuses with exit 64 or 65. minos-vmm may not link an internal crate, so both sides pin the same literals, and the sandbox side is `launcher_bounds_match_minos_vmm` in `crates/minos-sandbox/tests/spec.rs`. Change both files and both tests together (review WR-02). Above the four constant definitions, add a short `//` comment saying the same in one or two sentences.

(2) `crates/minos-sandbox/tests/spec.rs`: add `#[test] fn launcher_bounds_match_minos_vmm()`. It asserts with `assert_eq!` that `minos_sandbox::spec::MAX_VCPUS == 8`, `MIN_MEMORY_MIB == 128`, `MAX_MEMORY_MIB == 16_384`, `MAX_LAUNCH_MS == 3_600_000`, and `BOOT_ALLOWANCE_MS == 30_000`. Import them from `minos_sandbox::spec` or use full paths. Its doc comment points at `launcher_bounds_are_pinned_for_minos_sandbox` in minos-vmm's config tests for the first four, and at `host_deadline_grace_is_pinned_for_minos_sandbox` in minos-backend-krun's supervise tests for `BOOT_ALLOWANCE_MS`.

(3) `crates/minos-backend-krun/src/supervise.rs`: in the existing `#[cfg(test)] mod tests`, add `#[test] fn host_deadline_grace_is_pinned_for_minos_sandbox()` asserting `super::HOST_DEADLINE_GRACE == Duration::from_secs(30)`. Duration is already imported there. Its doc comment says `minos_sandbox::spec::BOOT_ALLOWANCE_MS` documents the same 30 s figure and pins it in `launcher_bounds_match_minos_vmm`. Extend the doc comment on `HOST_DEADLINE_GRACE` with one sentence saying the same, in plain backticks.

(4) `crates/minos-sandbox/src/spec.rs`: replace the `//` comment block above `MAX_VCPUS` (the one ending "change both together"). The new comment names the three pin tests by name and file. It says minos-vmm links no internal crate, so the bounds cannot share a constant, and instead each side pins the same literals, so changing one side fails the gate until the other side and both tests change. It also says `BOOT_ALLOWANCE_MS` restates `HOST_DEADLINE_GRACE` in `crates/minos-backend-krun/src/supervise.rs`. Do not change any constant's value.

Tests use no unwrap, expect, or indexing. Run the verify commands, then commit only these four files with the message `test(quick-261018-eiw): pin the launcher bounds on both sides (WR-02)`, a short body naming the review item, and the final line `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
  </action>
  <verify>
    <automated>cargo test -p minos-vmm launcher_bounds_are_pinned_for_minos_sandbox && cargo test -p minos-sandbox --test spec && cargo test -p minos-backend-krun --lib host_deadline_grace_is_pinned_for_minos_sandbox && cargo clippy -p minos-vmm -p minos-sandbox -p minos-backend-krun --all-targets -- -D warnings && cargo fmt --all --check && grep -q 'fn launcher_bounds_are_pinned_for_minos_sandbox' crates/minos-vmm/src/config.rs && grep -q 'fn launcher_bounds_match_minos_vmm' crates/minos-sandbox/tests/spec.rs && grep -q 'fn host_deadline_grace_is_pinned_for_minos_sandbox' crates/minos-backend-krun/src/supervise.rs && git diff --quiet HEAD -- Cargo.toml deny.toml crates/minos-vmm/Cargo.toml crates/minos-sandbox/Cargo.toml crates/minos-backend-krun/Cargo.toml && test -z "$(grep -rnE '#!?\[allow\(' crates --include='*.rs')"</automated>
  </verify>
  <done>Three new pin tests exist and pass: vmm config, sandbox spec, and krun supervise. Every constant keeps its value. No Cargo.toml or deny.toml changed, and minos-vmm still links no internal crate. The sandbox comment names the pin tests instead of "change both together" alone. One commit `test(quick-261018-eiw): ...` touches exactly the four listed files.</done>
</task>

<task type="auto">
  <name>Task 3: WR-03, Finished::new consumes the clearance; prove it with a compile-fail case; run the full gate</name>
  <files>crates/minos-sandbox/src/finished.rs, crates/minos-sandbox/src/preflight.rs, crates/minos-sandbox/src/backend.rs, crates/minos-sandbox/src/lib.rs, crates/minos-sandbox/tests/backend.rs, crates/minos-sandbox/tests/untrusted.rs, crates/minos-sandbox/tests/output_tar.rs, crates/minos-sandbox/tests/ui/pass/judge_flow.rs, crates/minos-sandbox/tests/ui/fail/finished_twice.rs, crates/minos-sandbox/tests/ui/fail/finished_twice.stderr, crates/minos-sandbox/tests/compile_fail.rs, CLAUDE.md</files>
  <action>
Source change (strengthens the invariant, per review WR-03). In `crates/minos-sandbox/src/preflight.rs`, add to `impl<R: Role> Cleared<R>` a crate-private consuming accessor, `#[must_use] pub(crate) fn into_key(self) -> LaunchKey`, that returns the plan's key by copy. Import `crate::key::LaunchKey`. Its doc comment says it consumes the clearance and that only `Finished::new` calls it, so a clearance ends in at most one `Finished`. It must stay `pub(crate)`. Add no public consuming method, no Clone, and no other accessor on `Cleared`. In `crates/minos-sandbox/src/finished.rs`, change the constructor to exactly `pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self`, keep `#[must_use]`, and set `key: cleared.into_key()`. The parameter must be consumed in the body, not only borrowed. Workspace clippy has pedantic `needless_pass_by_value` at warn under `-D warnings`, so a by-value parameter that is only borrowed fails the gate, and an allow attribute is not an option. Update the docs. The `Finished::new` doc says it consumes the launch's clearance, so one cleared launch yields at most one `Finished`. The `Finished` struct doc line says it consumes the launch's `Cleared<R>`. The module docs' last paragraph adds that a `Finished<R>` is built by consuming a `Cleared<R>`, so one clearance mints at most one. `Backend::launch` and `RunningVm::wait` signatures stay exactly as they are. In `crates/minos-sandbox/src/backend.rs`, only extend the `RunningVm::wait` doc with one sentence: an implementer moves the launch's `Cleared` into `Finished::new`, which consumes it. In `crates/minos-sandbox/src/lib.rs` "# Flow" step 5, add that the `Finished` is built by `Finished::new`, which consumes the launch's `Cleared`.

Call sites (drop the borrow, pass by value). In `tests/backend.rs` and `tests/ui/pass/judge_flow.rs`, `FakeRunning::wait(self)` passes `self.cleared` by value. In `tests/untrusted.rs`, both helpers pass `cleared` by value. In `tests/output_tar.rs`, the `read` helper passes `judge_cleared()?` by value. In `finished_reports_its_end_and_key`, copy each plan's key into a local (`*cleared.plan().key()`) before the clearance moves into `Finished::new`, then compare `finished.key()` with that local, for both the judge and the candidate halves. Change nothing else in those tests.

New compile-fail case. Create `crates/minos-sandbox/tests/ui/fail/finished_twice.rs` in the style of `cleared_clone.rs`: a leading `//` comment (Invariant: `Finished::new` consumes the clearance, so one `Cleared` yields at most one `Finished`; review WR-03; SBX-03). It imports `Cleared`, `Ended`, `Finished`, and `Judge` from minos_sandbox. It has `fn probe(ok: Cleared<Judge>) -> (Finished<Judge>, Finished<Judge>)` that binds `first` from `Finished::new(ok, Ended::Exited, Vec::new())`, binds `second` from the same call again, and returns the pair. `main` assigns `probe` to a `_probe` function-pointer binding of that type, so no warning joins the stderr. Run `cargo test -p minos-sandbox --test compile_fail` with TRYBUILD unset. trybuild writes the new case's output to `crates/minos-sandbox/wip/finished_twice.stderr` and fails. Read it. It must hold exactly one error, `error[E0382]: use of moved value: `ok``, with the move at the first call and the use at the second. Then move it to `crates/minos-sandbox/tests/ui/fail/finished_twice.stderr`, delete `crates/minos-sandbox/wip/`, and re-run until green. Never set TRYBUILD=overwrite. Every existing `.stderr` under `tests/ui/fail/` must stay byte-identical. If one changes, do not accept it: restore the committed text, rework the source change until the committed stderr matches again, and report the deviation in the SUMMARY. In `tests/compile_fail.rs`, add "one `Cleared` cannot build two `Finished` values" to the module doc's list of proven invariants.

CLAUDE.md, in the same change. In `## minos-sandbox`, in the paragraph on `Backend` and `Finished<R>`, rewrite the sentence "It can only be built from a `Cleared<R>`, ..." to say `Finished::new` consumes a `Cleared<R>`, so output of a role exists only for a cleared launch of that role and one clearance yields at most one `Finished` (refinement of 04-CONTEXT D-17; review WR-03). In `## Testing`, extend the compile-fail bullet with "that one `Cleared` cannot build two `Finished` values".

Commit these files only, after the per-crate checks pass, with the message `fix(quick-261018-eiw): Finished::new consumes the clearance (WR-03)`, a short body naming the review item and the unchanged `launch` and `wait` signatures, and the final line `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

Full gate, last, from the repo root, each command must exit 0: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `cargo deny check`; `cargo doc --workspace --no-deps`; `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`; and the C-02 grep from CLAUDE.md "Commands" (the recursive grep of `crates` for inner or outer allow attributes), which must print nothing. Record each command's result in the SUMMARY. If the gate surfaces a problem in a Task 1 or Task 2 file, fix it in a separate `fix(quick-261018-eiw): ...` commit with the same trailer. Do not run `cargo clean`.
  </action>
  <verify>
    <automated>cargo test -p minos-sandbox && grep -q 'pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self' crates/minos-sandbox/src/finished.rs && grep -q 'pub(crate) fn into_key(self) -> LaunchKey' crates/minos-sandbox/src/preflight.rs && ! grep -q 'cleared: &Cleared' crates/minos-sandbox/src/finished.rs && test -z "$(grep -rn 'Finished::new(&' crates/minos-sandbox)" && grep -q 'fn wait(self) -> Result<Finished<R>, InfraError>;' crates/minos-sandbox/src/backend.rs && grep -q 'fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>;' crates/minos-sandbox/src/backend.rs && grep -q 'E0382' crates/minos-sandbox/tests/ui/fail/finished_twice.stderr && test ! -e crates/minos-sandbox/wip && test -z "$(git diff --name-only e9af550 -- 'crates/minos-sandbox/tests/ui/fail/*.stderr' | grep -v 'finished_twice.stderr')" && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings && test -z "$(grep -rnE '#!?\[allow\(' crates --include='*.rs')"</automated>
  </verify>
  <done>`Finished::new` takes `Cleared<R>` by value and consumes it through the crate-private `Cleared::into_key`. `Backend::launch` and `RunningVm::wait` are unchanged. No call site borrows a clearance. `finished_twice.rs` fails to compile with exactly E0382, and its reviewed `.stderr` is committed. Every pre-existing `.stderr` is byte-identical, and the pass case runs. CLAUDE.md's minos-sandbox and Testing sections match the code. All seven full-gate checks pass, and their results are recorded in the SUMMARY.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| disk -> FileStore::read/append | Ledger bytes on disk may be damaged by bit rot, a foreign writer, or tampering after `sync_data` reported them durable |
| minos-sandbox preflight -> minos-vmm launcher | A plan that preflight clears must also pass the launcher's own config validation; the two crates share no code |
| backend / runner -> Finished::new | Any holder of a `Cleared<R>` can label bytes as role-`R` guest output; the number of such labels per clearance is the invariant |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-261018-01 | Tampering / Repudiation | `crates/minos-store/src/file/ledger.rs` `parse` + `append` `set_len` | high | mitigate | Task 1: a newline-terminated final line that is not a `Line` is `Corrupt { BadJson }` for read and append (Duplicate included); only unterminated tail bytes, which no append ever reported written, are truncated. Tests `a_newline_terminated_final_line_that_is_not_a_line_is_refused` and `a_duplicate_append_never_erases_a_damaged_final_line`. |
| T-261018-02 | Denial of Service | `minos-sandbox` spec bounds vs `minos-vmm` config bounds | medium | mitigate | Task 2: both sides pin the same literals in tests; drift fails `cargo test --workspace` instead of surfacing as retried exit 64/65 infrastructure failures until the circuit breaker trips. |
| T-261018-03 | Spoofing / Tampering | `crates/minos-sandbox/src/finished.rs` `Finished::new` | medium | mitigate | Task 3: `Finished::new` consumes `Cleared<R>` (via crate-private `into_key`), so one clearance yields at most one `Finished`; trybuild case `finished_twice.rs` (E0382) proves it at compile time. |
| T-261018-04 | Elevation of Privilege | trybuild `.stderr` files | medium | mitigate | Task 3: existing `.stderr` files must stay byte-identical to base commit e9af550 (`git diff --name-only e9af550` check); new stderr comes from trybuild's `wip/` output after review, never `TRYBUILD=overwrite`. |
| T-261018-05 | Denial of Service | ledger availability after a crash on a filesystem that reorders page writes | low | accept | A crash could in principle leave a newline-terminated line with zeroed bytes; it is now refused and needs a human (fail closed). The store cannot tell it from damage to a durable line, and integrity outranks availability; `minos repair` stays out of v1 per 04-CONTEXT Deferred. |
</threat_model>

<verification>
- Task 1, 2, 3 automated verify blocks all exit 0.
- Three commits on the branch, in order: `fix(quick-261018-eiw): ... (WR-01)`, `test(quick-261018-eiw): ... (WR-02)`, `fix(quick-261018-eiw): ... (WR-03)`, each ending with the Co-Authored-By trailer line; plus any gate-fix commit with the same prefix and trailer.
- The full gate (fmt, workspace clippy, workspace test, deny, doc, musl guest clippy, C-02 grep) passes at HEAD.
- `git diff e9af550 -- crates/minos-cli crates/minos-source crates/minos-store/src/file/mod.rs crates/minos-store/src/lib.rs` is empty (e9af550 is the base commit this plan was written against): IN-01 to IN-06 untouched.
- `git diff --name-only e9af550 -- 'crates/minos-sandbox/tests/ui/fail/*.stderr'` lists only `finished_twice.stderr`.
</verification>

<success_criteria>
- WR-01 resolved: no newline-terminated line is ever truncated; only unterminated tail bytes are; docs of record (CLAUDE.md, ledger.rs module docs, 04-CONTEXT D-13 amendment) match.
- WR-02 resolved: launcher bounds pinned by tests in minos-vmm, minos-sandbox, and minos-backend-krun; no new crate edges.
- WR-03 resolved: `Finished::new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self`; `Backend::launch` and `RunningVm::wait` unchanged; compile-fail case committed; all existing trybuild stderr unchanged.
- No allow attribute anywhere in `crates/`; no unwrap, expect, or indexing in new code or tests.
</success_criteria>

## Source coverage audit

| Source | Item | Covered by |
|---|---|---|
| GOAL | Fix Phase 4 review warnings WR-01, WR-02, WR-03 | Tasks 1, 2, 3 |
| REQ | STORE-02 (tolerates a torn final line, never rewrites a record) | Task 1 |
| REQ | SBX-03 (`Cleared` owns the plan; one clearance, one launch) and SBX-02 (trybuild proofs) | Task 3 |
| REQ | SBX-06 (every launch carries a hard maximum duration within the launcher ceiling) | Task 2 |
| CONTEXT | Orchestrator constraint: WR-01 BadJson at any position, unterminated tail only, update tests, CLAUDE.md, D-13 | Task 1 |
| CONTEXT | Orchestrator constraint: WR-02 first option unless a shared constant fits the layering (it does not: vmm is a bin with no internal deps) | Task 2 |
| CONTEXT | Orchestrator constraint: WR-03 state `Finished::new` signature and whether `launch`/`wait` change; keep existing trybuild green; new stderr only for new cases | Task 3 (interfaces block, verify) |
| CONTEXT | Full gate in the final task, no `cargo clean`, commit prefixes and trailer | Task 3, every task's commit step |
| Excluded | IN-01 to IN-06 | Out of scope by instruction |
| RESEARCH | None (quick task, no research phase) | n/a |

<output>
Create `.planning/quick/261018-eiw-fix-phase-4-review-warnings/261018-eiw-SUMMARY.md` when done. Include: the RED output of Task 1's new tests before the fix; the reviewed text of `finished_twice.stderr`; confirmation that every pre-existing `.stderr` is unchanged; and the result line of each of the seven full-gate checks.
</output>
