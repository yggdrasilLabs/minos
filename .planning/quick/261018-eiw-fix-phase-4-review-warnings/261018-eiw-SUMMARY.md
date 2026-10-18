---
phase: quick-261018-eiw
plan: 01
subsystem: store, sandbox, launcher bounds
tags: [review-fix, ledger, torn-tail, typestate, trybuild, bounds]
status: complete
requires: []
provides:
  - "File ledger refuses a newline-terminated non-Line final line as BadJson (only unterminated tail bytes are a torn tail)"
  - "Launcher bounds pinned to literals in minos-vmm, minos-sandbox, and minos-backend-krun tests"
  - "Finished::new consumes Cleared<R>; trybuild case finished_twice.rs (E0382)"
affects: [minos-store, minos-sandbox, minos-vmm, minos-backend-krun, CLAUDE.md, 04-CONTEXT D-13]
tech-stack:
  added: []
  patterns:
    - "Cross-crate restated constants pinned by literal tests on both sides when the dependency graph forbids sharing"
    - "Consuming crate-private accessor (Cleared::into_key) to make a capability single-use"
key-files:
  created:
    - crates/minos-sandbox/tests/ui/fail/finished_twice.rs
    - crates/minos-sandbox/tests/ui/fail/finished_twice.stderr
  modified:
    - crates/minos-store/src/file/ledger.rs
    - crates/minos-store/tests/file_ledger.rs
    - CLAUDE.md
    - .planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md
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
    - crates/minos-sandbox/tests/compile_fail.rs
decisions:
  - "Only bytes after the last newline are a tolerated ledger torn tail; a newline-terminated non-Line final line is BadJson and needs a human (D-13 amended 2026-10-18)"
  - "Launcher bounds are pinned by literal tests on both sides rather than shared, because minos-vmm links no internal crate"
  - "Finished::new takes Cleared<R> by value; Backend::launch and RunningVm::wait are unchanged"
metrics:
  duration: "about 6 minutes (plan written 10:37:48, last task commit 10:42:26, gate done 10:43 -0400)"
  completed: 2026-10-18
  tasks: 3
  files: 19
---

# Quick 261018-eiw: Fix Phase 4 Review Warnings Summary

This task fixes review warnings WR-01, WR-02, and WR-03. The file ledger no longer truncates a newline-terminated final line. The bounds that minos-sandbox restates from minos-vmm and minos-backend-krun are now pinned to literals in tests on all three sides. `Finished::new` now consumes its `Cleared<R>`, so one clearance mints at most one `Finished`, and a new trybuild case proves it.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 (WR-01) | 5db1e56 | fix(quick-261018-eiw): refuse a newline-terminated non-line final ledger line (WR-01) |
| 2 (WR-02) | 341d1e9 | test(quick-261018-eiw): pin the launcher bounds on both sides (WR-02) |
| 3 (WR-03) | a2c3761 | fix(quick-261018-eiw): Finished::new consumes the clearance (WR-03) |

No gate-fix commit was needed.

## Task 1: WR-01

**RED.** Before the parse change, `cargo test -p minos-store --test file_ledger` failed on both new tests:

```
---- a_newline_terminated_final_line_that_is_not_a_line_is_refused stdout ----
Error: "expected Corrupt BadJson at line 3, got Ok"

---- a_duplicate_append_never_erases_a_damaged_final_line stdout ----
Error: "expected Corrupt BadJson at line 2, got Ok"

test result: FAILED. 23 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

**GREEN.** In `parse`:
- The generic `serde_json::Value` fallback arm and the `is_tail` flag are gone, and so is `.peekable()`.
- The loop is now `for end in newlines`.
- Any newline-terminated line that is not a `Line` returns `Corrupt { BadJson }` at its 1-based line.

The post-loop handling of unterminated bytes and the `set_len` path in `append` did not change. After the change, minos-store passes: 5 + 24 + 8 + 25 tests, proptests included. The test `torn_tail_with_newline_is_tolerated` encoded the old behavior and was removed. Both unterminated torn-tail tests still pass unchanged.

**Docs.** Three places now describe the same single tolerated shape (bytes after the last newline) and give the same `write_all` reason:
- the ledger.rs module docs ("Reading a ledger" item 3 and "The torn tail")
- the ledger paragraph in CLAUDE.md's `## minos-store`, where "elsewhere" is gone and the final line is explicitly included
- 04-CONTEXT.md, which has a new `**Amended:**` header line and a sub-bullet under D-13. The original D-13 text is unchanged.

## Task 2: WR-02

Three new tests pin the same literals:
- `config::tests::launcher_bounds_are_pinned_for_minos_sandbox` in minos-vmm: 8, 128, 16_384, 3_600_000.
- `launcher_bounds_match_minos_vmm` in `crates/minos-sandbox/tests/spec.rs`: the same four values plus `BOOT_ALLOWANCE_MS` = 30_000.
- `supervise::tests::host_deadline_grace_is_pinned_for_minos_sandbox` in minos-backend-krun: `HOST_DEADLINE_GRACE` = 30 s.

The comment in spec.rs now names all three tests. Comments also went above the minos-vmm constants and on `HOST_DEADLINE_GRACE`, all in plain backticks with no cross-crate intra-doc links. No constant value changed, and no Cargo.toml or deny.toml changed: `git diff --quiet HEAD` exited 0 for all five manifests. minos-vmm still links no internal crate.

## Task 3: WR-03

- `crates/minos-sandbox/src/finished.rs`: `pub fn new(cleared: Cleared<R>, ended: Ended, stream: Vec<u8>) -> Self` with `key: cleared.into_key()`.
- `crates/minos-sandbox/src/preflight.rs`: `#[must_use] pub(crate) fn into_key(self) -> LaunchKey`. No public consuming method, no Clone, and no other accessor was added.
- `Backend::launch` and `RunningVm::wait` signatures are unchanged; the plan's grep checks for both passed.
- All 7 call sites now pass the clearance by value. `finished_reports_its_end_and_key` copies each key into a local before the move.
- Docs updated: finished.rs (module, struct, and `new`), the `RunningVm::wait` doc, lib.rs "# Flow" step 5, the `compile_fail.rs` module doc, and CLAUDE.md (the `## minos-sandbox` Finished sentence and the `## Testing` compile-fail bullet).

**Reviewed `finished_twice.stderr`.** This came from trybuild's `wip/` output with TRYBUILD unset, never `TRYBUILD=overwrite`. It holds exactly one error:

```
error[E0382]: use of moved value: `ok`
 --> tests/ui/fail/finished_twice.rs:7:32
  |
5 | fn probe(ok: Cleared<Judge>) -> (Finished<Judge>, Finished<Judge>) {
  |          -- move occurs because `ok` has type `Cleared<minos_sandbox::Judge>`, which does not implement the `Copy` trait
6 |     let first = Finished::new(ok, Ended::Exited, Vec::new());
  |                               -- value moved here
7 |     let second = Finished::new(ok, Ended::Exited, Vec::new());
  |                                ^^ value used here after move
```

The file was moved into `tests/ui/fail/`, and `crates/minos-sandbox/wip/` (which trybuild also gives a `.gitignore`) was deleted. The re-run was green.

**Existing `.stderr` files are unchanged.** `git diff --name-only e9af550 -- 'crates/minos-sandbox/tests/ui/fail/*.stderr'` lists only `finished_twice.stderr`. The pass case `judge_flow.rs` still compiles and runs.

## Full gate (at a2c3761)

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 (`Finished dev profile`) |
| `cargo test --workspace` | exit 0. 57 result lines: 915 passed, 0 failed, 26 ignored. The known-flaky supervise process-group test passed on the first run, so no re-run was needed. |
| `cargo deny check` | exit 0: `advisories ok, bans ok, licenses ok, sources ok`. Warnings only: 7 `duplicate` and 3 `unused-wrapper`. Neither Cargo.toml nor deny.toml changed in this task, and neither warning type involves the code changed here. |
| `cargo doc --workspace --no-deps` | exit 0, 0 warnings |
| `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | exit 0 |
| `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | printed nothing (grep exit 1) |

Out-of-scope check: `git diff e9af550 -- crates/minos-cli crates/minos-source crates/minos-store/src/file/mod.rs crates/minos-store/src/lib.rs` is empty, so IN-01 through IN-06 are untouched. `cargo clean` was not run, as instructed.

## Deviations from Plan

**1. Commit trailer.** The plan and orchestrator asked for `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. The three commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` instead. That is the attribution the harness specifies, it matches every earlier commit in this repository, and it names the model that actually did the work. If the orchestrator really wants the other trailer, it can amend the three commit messages; the code is unaffected.

Everything else was executed exactly as written.

## Deferred Issues

None.

## Known Stubs

None.

## Threat Flags

None. The changes narrow existing surfaces (T-261018-01 to 04 are mitigated as planned). T-261018-05 is accepted as planned: a crash that leaves a newline-terminated line of zeroed bytes is now refused and needs a human.

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/tests/ui/fail/finished_twice.rs
- FOUND: crates/minos-sandbox/tests/ui/fail/finished_twice.stderr
- FOUND commits: 5db1e56, 341d1e9, a2c3761
