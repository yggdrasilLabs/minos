---
phase: 4
slug: store-source-snapshot-and-sandbox-types
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-14
completed: 2026-10-15
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Built-in `cargo test` + `proptest` 1.11.0 + `trybuild` 1.0.122 (minos-sandbox compile-fail suite) |
| **Config file** | `.config/insta.yaml` (from Phase 3, `update: no`); `crates/minos-store/clippy.toml`, `crates/minos-sandbox/clippy.toml`, `crates/minos-source/clippy.toml` (new in 04-01, every entry proven to fire in 04-EVIDENCE.md) |
| **Quick run command** | `cargo test -p minos-store`, `cargo test -p minos-sandbox`, `cargo test -p minos-source` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps`, then `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` and the C-01 and C-02 checks (`grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing) |
| **Estimated runtime** | ~5 seconds for `cargo test -p minos-store` (every file append ends in `F_FULLFSYNC`); under a minute per crate quick run on a warm build; several minutes for the full gate |

---

## Sampling Rate

- **After every task commit:** Run the quick command of the crate the task touched plus `cargo clippy -p <crate> --all-targets -- -D warnings`
- **After every plan wave:** Run the full gate plus the musl guest clippy, C-01, and C-02
- **Before `/gsd-verify-work`:** Full suite must be green, then `cargo clean` (GATE-06)
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

Filled in from plans 04-01 to 04-09 and their SUMMARY files. Task IDs are `4-<plan>-<task>`. Every command runs from the repository root and is the exact `<automated>` command of the plan's verify block; the final gate in 04-EVIDENCE.md ran all of them together through `cargo test --workspace`. Pipes inside commands are escaped as `\|` for the table.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 4-01-01 | 04-01 | 1 | CLI-11 (groundwork) | T-04-01-01, T-04-01-03 | `minos-source` links `minos-core` only and only `minos-cli` may link it; a deliberate `minos-loop` edge is refused by `cargo deny check bans`; no new registry package or build script | supply-chain gate | `cargo build -p minos-source && cargo deny check && cargo clippy -p minos-source -p minos-cli -p minos-sandbox --all-targets -- -D warnings` | ✅ `crates/minos-source/Cargo.toml`, `deny.toml` | ✅ green |
| 4-01-02 | 04-01 | 1 | SBX-05, STORE-02 (groundwork) | T-04-01-02, T-04-01-04 | 24 crate clippy bans (tar unpack, `File::create`, `fs::write`, tar builder conveniences, `set_metadata`, `max_filesize`, env reads, hash collections) each fire exactly once on a probe; no probe file committed | lint probe | `cargo clippy -p minos-store -p minos-sandbox -p minos-source --all-targets -- -D warnings && test -z "$(ls crates/minos-store/tests crates/minos-sandbox/tests crates/minos-source/tests 2>/dev/null \| grep zz_clippy_probe)"` | ✅ three `clippy.toml` files | ✅ green |
| 4-01-03 | 04-01 | 1 | CLI-11 (groundwork) | T-04-01-01 | CLAUDE.md Workspace section names `minos-source` and its layering rule | docs + full gate | `grep -n 'minos-source/' CLAUDE.md && grep -n 'only .cli. links it' CLAUDE.md && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps` | ✅ `CLAUDE.md` | ✅ green |
| 4-02-01 | 04-02 | 2 | STORE-01 | T-04-02-01, T-04-02-02 | `Ledger` and `Blobs` are synchronous `&self` traits; `Appended` is `#[must_use]` with `New`, `Duplicate`, `Conflict`; `StoreError::Io` is the only retryable variant; `Corrupt` names run, line, and kind, never bytes | unit + doc | `cargo test -p minos-store --lib && cargo clippy -p minos-store --all-targets -- -D warnings && cargo doc -p minos-store --no-deps` | ✅ `crates/minos-store/src/lib.rs` | ✅ green |
| 4-02-02 | 04-02 | 2 | STORE-01, STORE-03, STORE-04 | T-04-02-01 to T-04-02-05 | `MemoryStore` passes 10 contract cases and the first-write-wins property; a same-key append writes nothing; `get` refuses an over-cap size before copying and rehashes; a poisoned lock is `StoreError::Poisoned` | contract + proptest | `cargo test -p minos-store && cargo clippy -p minos-store --all-targets -- -D warnings` | ✅ `crates/minos-store/tests/contract.rs`, `src/memory.rs` | ✅ green |
| 4-03-01 | 04-03 | 2 | SBX-01, SBX-06 | T-04-03-01, T-04-03-06 | Sealed `Role` with uninhabited `Candidate` and `Judge` (tags 1 and 2, 3 reserved); `LaunchKey` golden vectors computed with shasum outside the code; changing any field changes the key | golden + proptest | `cargo test -p minos-sandbox --test spec && cargo test -p minos-sandbox --lib && cargo clippy -p minos-sandbox --all-targets -- -D warnings` | ✅ `crates/minos-sandbox/tests/spec.rs` | ✅ green |
| 4-03-02 | 04-03 | 2 | SBX-01, SBX-03, SBX-06 | T-04-03-01, T-04-03-02, T-04-03-05 | `secrets` exists only on `SandboxSpec<Judge>`; no network field or method on any role; `seal` is the only `LaunchPlan` constructor; hard maximum is budgets plus 30 s, refused (never clamped) over 3,600,000 ms | unit | `cargo test -p minos-sandbox --test spec && cargo clippy -p minos-sandbox --all-targets -- -D warnings && cargo doc -p minos-sandbox --no-deps` | ✅ `crates/minos-sandbox/tests/spec.rs` | ✅ green |
| 4-03-03 | 04-03 | 2 | SBX-03 | T-04-03-03, T-04-03-04 | `preflight` is the only constructor of `Cleared<R>`, which owns its plan and derives nothing; 12 refusals collected sorted and unique; judge data equal to the input disk is refused | unit | `cargo test -p minos-sandbox && cargo clippy -p minos-sandbox --all-targets -- -D warnings && cargo doc -p minos-sandbox --no-deps` | ✅ `crates/minos-sandbox/tests/preflight.rs` | ✅ green |
| 4-04-01 | 04-04 | 2 | CLI-11 | T-04-04-01, T-04-04-02, T-04-04-04, T-04-04-07 | Nine denylist rules, ASCII case-insensitive, look-alikes not denied; nine anchored linear-time secret patterns report path, line, and pattern, never bytes; test vectors joined at run time; the crate's own sources scan clean | unit | `cargo test -p minos-source --test denylist --test scan && cargo clippy -p minos-source --all-targets -- -D warnings` | ✅ `crates/minos-source/tests/denylist.rs`, `tests/scan.rs` | ✅ green |
| 4-04-02 | 04-04 | 2 | CLI-11 | T-04-04-03, T-04-04-05, T-04-04-06, T-04-04-08 | git runs with a cleared environment and a four-variable allowlist, a 30 s deadline, and capped pipes; packed-but-untracked files and every git failure are `Dirty`; git stderr is drained and never shown | integration + unit | `cargo test -p minos-source --test clean && cargo test -p minos-source --lib && cargo clippy -p minos-source --all-targets -- -D warnings` | ✅ `crates/minos-source/tests/clean.rs`, `src/clean.rs` tests | ✅ green |
| 4-04-03 | 04-04 | 2 | CLI-11 | T-04-04-03 | CLAUDE.md records the read-only git carve-out and its environment | docs + full gate | `grep -n 'The clean check is the one place Minos runs git' CLAUDE.md && grep -n 'The read-only clean check on the user' CLAUDE.md && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps` | ✅ `CLAUDE.md` | ✅ green |
| 4-05-01 | 04-05 | 3 | STORE-01, STORE-02, STORE-03 | T-04-05-01, T-04-05-04, T-04-05-06, T-04-05-08 | `FileStore` root created 0700, symlinked root refused; exclusive `File::lock` per append with the key set rebuilt under it; `sync_data` before return; the file arms pass every contract case | contract | `cargo test -p minos-store --test contract && cargo clippy -p minos-store --all-targets -- -D warnings` | ✅ `crates/minos-store/tests/contract.rs`, `src/file/` | ✅ green |
| 4-05-02 | 04-05 | 3 | STORE-02, STORE-03 | T-04-05-01, T-04-05-02, T-04-05-03, T-04-05-04, T-04-05-07, T-04-05-09 | A torn tail is read as a count and truncated by the next append; `BadJson`, `KeyMismatch`, `SeqGap`, `UnknownVersion`, `LineTooLong` refuse read and append at the exact 1-based line; first line byte-identical to core's golden; four-thread appends serialized; append-only prefix and memory-file agreement properties | integration + proptest | `cargo test -p minos-store --test file_ledger && cargo clippy -p minos-store --all-targets -- -D warnings` | ✅ `crates/minos-store/tests/file_ledger.rs` | ✅ green |
| 4-05-03 | 04-05 | 3 | STORE-04 | T-04-05-05, T-04-05-06, T-04-05-07 | Blobs written temp-then-rename; size checked before opening; rehashed on every get, so a tampered blob is `BlobTampered`; a put heals it; a symlinked blob or shard is `Unsafe` | integration | `cargo test -p minos-store && cargo clippy -p minos-store --all-targets -- -D warnings` | ✅ `crates/minos-store/tests/file_blobs.rs` | ✅ green |
| 4-06-01 | 04-06 | 3 | SBX-04, SBX-05 | T-04-06-01 to T-04-06-04, T-04-06-06, T-04-06-07 | `Untrusted<T>` has a private field, a crate-private `new`, and no Debug, Display, Deref, Clone, or Default; the raw-mode reader takes ustar typeflag `b'0'` only, `RelPath` names, and checks entry, size, total, and stream caps before reading data; `Finished<R>` is built only from a `Cleared<R>` | unit + integration | `cargo test -p minos-sandbox --test output_tar && cargo clippy -p minos-sandbox --all-targets -- -D warnings && cargo doc -p minos-sandbox --no-deps` | ✅ `crates/minos-sandbox/tests/output_tar.rs` | ✅ green |
| 4-06-02 | 04-06 | 3 | SBX-05 | T-04-06-01 to T-04-06-04, T-04-06-06, T-04-06-08 | 20 SHA-256-pinned hostile fixtures, each rejected with its exact fault; generated count, stream, and total bombs; two never-panic properties | fuzz corpus + proptest | `cargo test -p minos-sandbox --test output_tar && test "$(ls crates/minos-sandbox/tests/fixtures/output_tar/*.tar \| wc -l \| tr -d ' ')" = "20"` | ✅ `crates/minos-sandbox/tests/fixtures/output_tar/` (20 files) | ✅ green |
| 4-06-03 | 04-06 | 3 | SBX-04 | T-04-06-05, T-04-06-07 | `into_scorecard` exists only on `Untrusted<GuestOutput<Judge>>`; `into_log_excerpt` and `into_diff_texts` are capped (16 texts, `max_diff_bytes`, UTF-8, no NUL or CR) | unit | `cargo test -p minos-sandbox && cargo clippy -p minos-sandbox --all-targets -- -D warnings && cargo doc -p minos-sandbox --no-deps` | ✅ `crates/minos-sandbox/tests/untrusted.rs` | ✅ green |
| 4-07-01 | 04-07 | 3 | CLI-11 | T-04-07-02, T-04-07-03, T-04-07-04, T-04-07-05 | Exactly four exclusion sources; denylist and judge-path drops reported with their rule; hand-built ustar headers with zeroed metadata; identical bytes across runs and creation orders; golden TreeId `bb2319a3...c0f0`; system tar lists the tarball | integration + golden | `cargo test -p minos-source --test pack && cargo clippy -p minos-source --all-targets -- -D warnings && cargo doc -p minos-source --no-deps` | ✅ `crates/minos-source/tests/pack.rs` | ✅ green |
| 4-07-02 | 04-07 | 3 | CLI-11 | T-04-07-01, T-04-07-03, T-04-07-04, T-04-07-06, T-04-07-07, T-04-07-08 | Offenders listed together with sanitized names; per-file, file-count, and total caps; a `.minosignore` negation (also behind a BOM) is refused; a missing surface file is refused; a planted secret blocks the pack even under `AllowDirty`; a dirty tree is refused | integration | `cargo test -p minos-source --test refusals && cargo test -p minos-source --test scan && cargo clippy -p minos-source --all-targets -- -D warnings` | ✅ `crates/minos-source/tests/refusals.rs` | ✅ green |
| 4-07-03 | 04-07 | 3 | CLI-11 | T-04-07-04 | CLAUDE.md Source paragraph records the snapshot rules | docs + full gate | `grep -n 'Four exclusion sources apply as a union' CLAUDE.md && grep -n 'line starting with `!` fails the pack' CLAUDE.md && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps` | ✅ `CLAUDE.md` | ✅ green |
| 4-08-01 | 04-08 | 4 | SBX-03, SBX-06 | T-04-08-01, T-04-08-06 | `Backend::launch` takes `Cleared<R>` by value and `wait` returns `Finished<R>`; `LaunchError` and `InfraError` are retried infrastructure failures; a refused plan never launches; every launch carries the plan's key and hard maximum | integration (fake backend) | `cargo test -p minos-sandbox --test backend && cargo clippy -p minos-sandbox --all-targets -- -D warnings && cargo doc -p minos-sandbox --no-deps` | ✅ `crates/minos-sandbox/tests/backend.rs` | ✅ green |
| 4-08-02 | 04-08 | 4 | SBX-01, SBX-02, SBX-03, SBX-04 | T-04-08-01 to T-04-08-05 | Twelve compile-fail cases, each with exactly one reviewed `error` line, plus a runnable judge-flow pass case; `TRYBUILD` is never set in the gate | compile-fail (trybuild) | `env -u TRYBUILD cargo test -p minos-sandbox --test compile_fail && test "$(ls crates/minos-sandbox/tests/ui/fail/*.stderr \| wc -l \| tr -d ' ')" = "12"` | ✅ `crates/minos-sandbox/tests/compile_fail.rs`, `tests/ui/` | ✅ green |
| 4-08-03 | 04-08 | 4 | SBX-01, SBX-03, SBX-06 | T-04-08-01 | CLAUDE.md sandbox signatures, the `SpecError` hand-off, and the `minos/launch/v1` key match what shipped | docs + full gate | `grep -n 'launch(ok: Cleared<R>)' CLAUDE.md && grep -n 'host: &Capabilities, plan: LaunchPlan<R>' CLAUDE.md && grep -n 'minos/launch/v1' CLAUDE.md && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps` | ✅ `CLAUDE.md` | ✅ green |
| 4-09-01 | 04-09 | 5 | STORE-01 to STORE-04 (design of record) | — (design of record; no threat in the 04-09 register) | CLAUDE.md `## minos-store` states the shipped store, and Open questions name the three questions this phase leaves open; this map is complete | docs | `grep -n '^## minos-store' CLAUDE.md && grep -n 'id_ed25519' CLAUDE.md && grep -n 'nyquist_compliant: true' .planning/phases/04-store-source-snapshot-and-sandbox-types/04-VALIDATION.md && grep -c '^\| 4-0' .planning/phases/04-store-source-snapshot-and-sandbox-types/04-VALIDATION.md` | ✅ `CLAUDE.md`, `04-VALIDATION.md` | ✅ green |
| 4-09-02 | 04-09 | 5 | STORE-02, STORE-04, CLI-11, SBX-01 to SBX-04 (evidence) | T-04-09-01, T-04-09-02 | Each claimed rejection is shown with its real output; no probe edit is committed; no secret bytes are pasted | evidence capture | `grep -c '^## ' .planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md && test -z "$(git status --porcelain crates)"` | ✅ `04-EVIDENCE.md` | ✅ green |
| 4-09-03 | 04-09 | 5 | GATE-06 | T-04-09-03, T-04-09-04 | Full gate, musl clippy, C-01, C-02, and unchanged clippy bans pass; only then `cargo clean`, with no cargo command after it | gate + `cargo clean` | `grep -n '^## Final gate' .planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md && tail -5 .planning/phases/04-store-source-snapshot-and-sandbox-types/04-EVIDENCE.md \| grep -n 'cargo clean' && test ! -d target/debug` | ✅ `04-EVIDENCE.md` | ✅ green (executor cargo clean done; the verifier reruns cargo clean last) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Deliberate-violation evidence (the Phase 1 to 3 standard): 04-EVIDENCE.md records the layering refusal and every clippy ban firing (04-01), and the real output of a torn final line, an unknown `v` tag, a tampered blob, a planted secret (location only), a path outside the grammar, and the first `error` line of every trybuild compile-fail case (04-09).

---

## Wave 0 Requirements

- [x] `crates/minos-source` skeleton with `[lints] workspace = true`, linked only by `minos-cli` through a `deny.toml` wrapper entry (04-01, 1e209d0)
- [x] `crates/minos-store/clippy.toml`, `crates/minos-sandbox/clippy.toml`, `crates/minos-source/clippy.toml`, each entry proven to fire once by a throwaway probe recorded in 04-EVIDENCE.md (04-01, 56467aa)
- [x] Dev dependencies: `proptest` for all three crates, `trybuild` for `minos-sandbox`, `tempfile` for `minos-source`; every Phase 4 edge in `Cargo.lock` with no new registry package (04-01, 1e209d0)
- [x] No `tests/common` module: each test file defines its own helpers, because a shared helper module trips `dead_code` under `-D warnings` in binaries that do not use every helper and no allow attribute may silence it (C-02)
- [x] No framework install needed

---

## Manual-Only Verifications

All phase behaviors have automated verification. `cargo clean` (GATE-06) is a command, not a test; it is recorded in 04-EVIDENCE.md `## Final gate`, and the phase verifier reruns it as its last command after its own verification passes.

---

## Deviations

Copied from the plan SUMMARYs. None weakens an invariant, and none edited `Cargo.toml`, `Cargo.lock`, or `deny.toml` outside 04-01.

- **04-01:** requirement checkboxes CLI-11, SBX-05, and STORE-02 were left open, because 04-05, 04-06, and 04-07 deliver them; the layering evidence was captured a second time because the first JSON query ran after the edge was reverted.
- **04-02:** `LedgerRead::new` and `pub mod memory` moved from Task 1 to Task 2 (a constructor with no caller trips `dead_code`); a contract case that dropped an `#[must_use]` `Appended` was fixed; only STORE-01 was marked complete.
- **04-03:** the `preflight` signature is wrapped over four lines by rustfmt, so its one-line acceptance grep cannot match; one test switched `Duration::from_millis(210_000)` to `from_secs(210)` for clippy.
- **04-04:** `DENYLIST` is `[DenyRule; 9]` with a private `name_pattern`, because a public const cannot expose the private `NamePattern`; the timeout, cap, exit-code, and environment paths of the clean check are unit-tested through a private `run_capped`.
- **04-05:** store files are opened after a `symlink_metadata` check with a device and inode match, and created with `create_new` (closing a check-then-open window); `StoreError::LineTooLong` refuses an over-cap record on append; a newline-terminated torn tail must be invalid JSON, and unterminated bytes over the line cap are `LineTooLong`.
- **04-06:** `OutputFile<R>` landed in Task 3 with its first user; the ignored corpus writer carries a reason; infallible fixture builders return `Vec<u8>`; test-only lint fixes; extra boundary tests.
- **04-07:** an unreadable or unparsable ignore file fails the pack, and the `.minosignore` negation check strips a line-1 BOM; the `snapshot` signature is wrapped by rustfmt; Task 2's tests could not fail first because Task 1 had built the behavior.
- **04-08:** intra-doc links to `preflight` use the function form; clippy `duration_suboptimal_units` fixes; `launch_bare_plan.rs` has an empty `main` because its generic probe cannot be coerced to a function pointer; UI files were run through rustfmt and re-verified with `TRYBUILD` unset.

### Known limitations carried forward

- `minos-backend-krun` `supervise::tests::supervise_run_kills_the_whole_process_group_at_the_host_deadline` is flaky under the command sandbox (EPERM while probing the process group); mechanism unproven (deferred-items.md).
- The git clean check's deadline kills the `git` process, not its process group, so a submodule child could keep a pipe open past the deadline; the result is still `TimedOut` and dirty (04-04).
- A regular file swapped for a FIFO between `lstat` and `open` would hang the snapshot; fixing it needs `O_NOFOLLOW | O_NONBLOCK`, which needs a `rustix` edge that the frozen `Cargo.toml` did not allow this phase (04-07).
- `LedgerRead::new` is crate-private; an out-of-crate store (the AWS milestone) needs it loosened (04-02).
- Compile-fail cases for `Untrusted::new` from outside the crate and `Finished::new` with a `Cleared` of the wrong role were suggested by 04-06 and not added by 04-08.
- `cargo deny check` prints `unused-wrapper` warnings for layering entries whose edges do not exist yet; it exits 0.
- tar's `Header::path_bytes` joins the ustar `prefix` field, so the Phase 5 guest writer should emit plain names with no prefix (04-06).
- The denylist's `id_*` rule also drops ordinary files such as `id_map.rs`; every drop is reported, and the narrowing is a user decision (CLAUDE.md Open questions).

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-15 (04-09, after the full gate in 04-EVIDENCE.md)
