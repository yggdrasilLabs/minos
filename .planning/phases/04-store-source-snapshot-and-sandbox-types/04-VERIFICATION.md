---
phase: 04-store-source-snapshot-and-sandbox-types
verified: 2026-10-15T16:00:00Z
status: passed
score: 4/4 roadmap success criteria verified (SC5, `cargo clean`, is the orchestrator's post-verification step); 11/11 requirement IDs satisfied
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 4: Store, Source Snapshot, and Sandbox Types Verification Report

**Phase Goal:** Run history survives crashes in an append-only ledger and a content-addressed blob store. Project source enters the system only as a deterministic, secret-scanned snapshot. The compiler rejects any sandbox configuration that would give a candidate judge data or a network, before any VM exists.
**Verified:** 2026-10-15
**Status:** passed
**Re-verification:** No, initial verification

I started from the hypothesis that the phase had missed its goal. I read the code in `crates/minos-store`, `crates/minos-sandbox`, and `crates/minos-source`, read the compile-fail cases and their `.stderr` files, and ran the full gate myself on a fresh invocation. I did not rely on the SUMMARY files. No gap survived.

## Goal Achievement

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The file ledger survives a crash: a torn final line is tolerated, unknown `v` tags refused, no record rewritten, a repeated `EventKey` is a no-op. File blobs are temp-then-rename and a tampered blob fails its SHA-256 on read. Memory and file stores pass the same synchronous trait tests (STORE-01..04) | VERIFIED | `crates/minos-store/src/file/ledger.rs`: `parse` tolerates only an unterminated tail or a newline-terminated non-JSON final line (CONTEXT D-13) and returns `torn_start`. A first probe on `record.v != "1"` returns `CorruptKind::UnknownVersion` on any line, final line included. `Line::verify` gives `KeyMismatch`, and `seq` must be contiguous (`SeqGap`). `append` takes an exclusive `File::lock`, rebuilds the key set under the lock, and returns `Appended::Duplicate` (or `Conflict`) without writing when the key exists. Otherwise it does one `write_all` plus `sync_data`. The only mutation of existing bytes is `set_len` on the torn tail. `file/blobs.rs`: `NamedTempFile::new_in(shard)`, `write_all`, `sync_all`, `persist`, then `fsync_dir`. `get` checks `symlink_metadata`, enforces a size cap, reads through `take(max+1)`, and rehashes, returning `BlobTampered` on mismatch. `tests/contract.rs` runs one macro-generated case set against `MemoryStore::new()` and `FileStore::open(..)` (24 tests, plus a first-write-wins property for both). `tests/file_ledger.rs` (24) covers torn tail with and without newline, unknown `v` mid-file and as the final line, edited key, edited record, seq gap, over-cap line, concurrent appends, and an append-only prefix proptest. `tests/file_blobs.rs` (8) covers tampering, healing, symlinks, and no leftover temp. All pass in my run. No `async`, `tokio`, or `unsafe` in the crate. |
| 2 | `trybuild` compile-fail tests prove a `SandboxSpec<Candidate>` cannot receive judge data, no role can attach a network, a `LaunchPlan` comes only from `seal()`, and `launch` cannot be called without a `Cleared`, which only `preflight` mints and which owns its plan. Every plan carries a hard maximum duration and a launch key derived from `ExperimentId`, role, and trial (SBX-01, 02, 03, 06) | VERIFIED | `tests/ui/fail/` holds 12 `.rs`/`.stderr` pairs. Each `.stderr` has exactly one `error` line, and each is the intended one: `candidate_secrets` E0599 (no `secrets` on `SandboxSpec<Candidate>`), `candidate_network` and `judge_network` E0599 (no `network` method on either role), `launch_plan_literal` E0451 (all `LaunchPlan` fields private), `cleared_literal` E0451, `cleared_clone` E0599, `launch_bare_plan` E0308 (`Backend::launch` takes `Cleared<R>`, not a plan), `candidate_scorecard` E0599, and four `Untrusted` cases (Debug, Display, Deref, private field). `tests/ui/pass/judge_flow.rs` is the positive control. The suite ran green in my gate (`TRYBUILD` unset). Source: `spec.rs` has `secrets` only in `impl SandboxSpec<Judge>`, no network field or method anywhere, and `LaunchPlan {` is constructed once, in `seal`. `Cleared { plan }` is constructed once, at `preflight.rs:224`, and `Cleared` has no `Clone`. `Role` is sealed and `Candidate` and `Judge` are uninhabited. `SandboxSpec::new` computes `max_duration_ms` for every role and refuses values over 3,600,000 ms (`DurationOverCeiling`) instead of clamping. `LaunchKey::of::<R>` hashes the `minos/launch/v1` domain, the experiment, the role tag, the judge version, and the trial. That is a superset of the required experiment, role, and trial, so it is stricter, not weaker. |
| 3 | `Untrusted<T>` has no `Deref`, `Display`, or `Debug` and exits only through the capped `into_scorecard`, `into_log_excerpt`, and `into_diff_texts`. The output tar reader accepts regular files only, rejects traversal and over-cap entry count, entry size, and total, never writes to disk, and survives its fuzz corpus (SBX-04, SBX-05) | VERIFIED | `untrusted.rs`: `pub struct Untrusted<T>(T);` has no derives and no trait impls. `new` is `pub(crate)`, and the three exits are `into_scorecard` (judge output only, through `Scorecard::parse_json(.., limits)`), `into_log_excerpt` (through `LogExcerpt::from_bytes(.., max)`), and `into_diff_texts` (16 texts, `max_diff_bytes`, UTF-8, no NUL or CR). The remaining methods (`entry_count`, `content_bytes`, `take`) return counts or another `Untrusted` and expose no bytes. `output.rs::read_output` takes `&[u8]` and checks the stream cap, 512-byte alignment, a ustar header, linkflag `b'0'` only, a `RelPath`-valid name, entry-count (1024), per-entry (8 MiB), and total (16 MiB) caps before reading data, rejects duplicates, and requires a zero end marker with no trailing data. `clippy.toml` bans `tar::Archive::unpack`, `Entry::unpack`, `Entry::unpack_in`, `std::fs::File`, and `OpenOptions` in this crate. `tests/output_tar.rs` runs 35 tests: 20 committed hostile `.tar` fixtures whose bytes are pinned by SHA-256 in `corpus_is_pinned`, generated count, stream, and total bombs, and never-panic proptests. Fixture count on disk is 20. `tests/untrusted.rs` runs 11. All pass. |
| 4 | Packing the same tree twice yields byte-identical tarballs and the same `TreeId`, honoring `.gitignore`, `.minosignore`, and the hard denylist. A planted secret blocks the pack and a dirty tree is refused unless `--allow-dirty` (CLI-11) | VERIFIED (library level; see note) | `pack.rs` builds ustar headers by hand: `uid`/`gid`/`mtime` 0, mode 0644/0755, regular type, over a `BTreeMap<RelPath, FileEntry>`, so paths are sorted. `snapshot()` order is `walk::collect`, surface check, secret scan, clean check (`RequireClean` only), then pack. `tests/pack.rs` includes `same_tree_twice_is_byte_identical`, `creation_order_does_not_matter`, `golden_project_tree`, `headers_are_zeroed`, and `system_tar_lists_the_tarball` (8 pass). The walk uses `ignore::WalkBuilder` with `standard_filters(false)`, `.git_ignore(true)`, `.require_git(false)`, `.add_custom_ignore_filename(".minosignore")`, and `follow_links(false)`. A `!` line in `.minosignore` is refused. `denylist.rs` has nine rules matched ASCII case-insensitively, plus judge paths. `tests/refusals.rs` (12) includes `planted_secret_blocks`, `allow_dirty_never_skips_the_scan`, `require_clean_outside_a_repository_is_dirty` (RequireClean is refused, AllowDirty packs), and a `RequireClean` result of `Dirty::Changes` for changed files. `tests/clean.rs` has 10 more. **Note:** the `--allow-dirty` flag is `CleanPolicy::AllowDirty` in the library. `minos-cli/src/main.rs` is still the empty stub, and the CLI flag wiring is Phase 8 (ROADMAP Phase 8, `minos run`). This matches how Phase 3 treated CLI-facing requirements, and REQUIREMENTS.md marks CLI-11 as satisfied by the snapshot behavior. |
| 5 | `cargo clean` has been run after verification passes (GATE-06) | PENDING BY DESIGN | The orchestrator runs `cargo clean` once after this verification, as for Phase 3. I did not run it. The gate is green on a fresh run (below). GATE-06 stays unticked in REQUIREMENTS.md by design. |

**Score:** 4/4 verifiable roadmap truths verified; SC5 is the post-verification `cargo clean` step.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/minos-store/src/{lib,memory}.rs`, `src/file/{mod,ledger,blobs}.rs` | Sync `Ledger` and `Blobs` traits, memory and file impls | VERIFIED | 1,107 lines, substantive, no stub patterns. Linked by `minos-runner`, `minos-loop`, and `minos-cli` only (`deny.toml:66`). |
| `crates/minos-sandbox/src/{role,spec,key,preflight,backend,finished,output,untrusted}.rs` | Roles, specs, sealed plans, `Cleared`, backend trait, `Untrusted` | VERIFIED | All exported from `lib.rs`. `Backend::launch` takes `Cleared<R>` and `RunningVm::wait` returns `Finished<R>`. |
| `crates/minos-source/src/{lib,walk,pack,scan,clean,denylist}.rs` | Snapshot, denylist, secret scan, git clean check | VERIFIED | Linked only by `minos-cli` (`deny.toml:68`). |
| `crates/*/clippy.toml` (store, sandbox, source) | Hostile-input bans | VERIFIED | Present. Firing of each entry is recorded in `04-EVIDENCE.md`. |
| `crates/minos-sandbox/tests/ui/fail/*.{rs,stderr}` (12), `ui/pass/judge_flow.rs` | trybuild suite | VERIFIED | Count is 12. |
| `crates/minos-sandbox/tests/fixtures/output_tar/*.tar` (20) | Hostile corpus | VERIFIED | Count is 20, SHA-256 pinned. |
| `CLAUDE.md` Phase 4 sections | Design of record | VERIFIED | `## minos-store` (line 211), the `minos-source/` workspace line (86), the layering rule (105), the `Cleared<R>` and `Capabilities` signatures (263, 419), and the `id_ed25519` open question (527) are all present. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `SandboxSpec::seal` | `LaunchPlan` | only constructor | WIRED | One struct literal, in `spec.rs`. The compile-fail case `launch_plan_literal` proves outsiders cannot build one. |
| `preflight` | `Cleared` | only constructor | WIRED | One literal, at `preflight.rs:224`. Private field, no `Clone`. |
| `Backend::launch` | `Cleared<R>` | by-value parameter | WIRED | `backend.rs` signature. `launch_bare_plan` fails with E0308. |
| `Finished<R>` | `read_output` | `into_output` | WIRED | `Untrusted<GuestOutput<R>>` is created only here and in `take`. |
| `snapshot` | `check_clean`, `Scanner`, `pack::tarball` | direct calls | WIRED | The scan runs before the clean-policy match, so `AllowDirty` cannot skip it. |
| `minos-store` and `minos-source` | dependency direction | `deny.toml` wrappers | WIRED | `cargo deny check` exits 0. A deliberate `minos-loop` edge to `minos-source` was refused (`04-EVIDENCE.md`). |

### Data-Flow Trace (Level 4)

No dynamic-rendering artifacts. The data paths are append -> file -> parse -> `LedgerRead`, bytes -> `read_output` -> `Untrusted` -> validating exit, and tree -> walk -> scan -> tar -> `TreeId`. Each is exercised end to end by the integration tests above.

### Behavioral Spot-Checks and Gate (run by me on a fresh invocation)

| Command | Result | Status |
|---------|--------|--------|
| `cargo fmt --all --check` | exit 0 | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | PASS |
| `cargo test --workspace` | exit 0: 911 passed, 0 failed, 26 ignored | PASS |
| `cargo deny check` | exit 0: `advisories ok, bans ok, licenses ok, sources ok`. It prints unused-wrapper warnings (known, recorded) | PASS |
| `cargo doc --workspace --no-deps` | exit 0 | PASS |
| `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | exit 0 | PASS |
| `grep -rnE '#!?\[allow\(' crates --include='*.rs'` (C-02) | prints nothing (grep exit 1) | PASS |
| `git status --short` after the gate | clean, so no tracked file changed | PASS |

Per-suite counts: `minos-store` contract 24, file_ledger 24, file_blobs 8, lib 5. `minos-sandbox` spec 10, preflight 6, backend 6, untrusted 11, output_tar 35 (1 ignored writer), compile_fail 1 (12 fail cases plus 1 pass case), lib 1. `minos-source` pack 8, refusals 12, clean 10, scan 9, denylist 5, lib 7. The flaky `minos-backend-krun` process-group test passed in this run.

### Probe Execution

Step 7c: SKIPPED. The phase declares no `probe-*.sh` scripts, and `find scripts` has no matches.

### Requirements Coverage

Cross-referenced the plans' requirement IDs against REQUIREMENTS.md. All 11 phase IDs are accounted for, and none is orphaned. REQUIREMENTS.md maps no other ID to Phase 4. GATE-06 is cross-cutting and left Pending by design.

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| STORE-01 | Sync `Ledger`/`Blobs` traits with in-memory impls | SATISFIED | `lib.rs` traits, `MemoryStore`, and the contract suite shared across both stores |
| STORE-02 | Append-only JSONL under an exclusive lock, torn tail tolerated, unknown `v` refused, no rewrite | SATISFIED | `file/ledger.rs`, `tests/file_ledger.rs` |
| STORE-03 | Same-`EventKey` append is a no-op | SATISFIED | `Appended::Duplicate`/`Conflict` before any write; contract cases |
| STORE-04 | Content-addressed blobs, temp-then-rename, hash verified on read | SATISFIED | `file/blobs.rs`, `tests/file_blobs.rs` |
| SBX-01 | `SandboxSpec<R>`; only Judge mounts judge data; no network | SATISFIED | `spec.rs`, `candidate_secrets`, `candidate_network`, `judge_network` |
| SBX-02 | Compile-fail tests for judge data and `Cleared` | SATISFIED | 12 trybuild cases |
| SBX-03 | `seal()` and `preflight` are the only constructors, and `Cleared` owns the plan | SATISFIED | See key links; `launch_plan_literal`, `cleared_literal`, `launch_bare_plan` |
| SBX-04 | `Untrusted<T>` has no `Deref`/`Display`/`Debug`; capped exits | SATISFIED | `untrusted.rs`, four `untrusted_*` compile-fail cases |
| SBX-05 | Hostile tar reader, caps, no disk, fuzz corpus | SATISFIED | `output.rs`, the clippy bans, 20 pinned fixtures, proptests |
| SBX-06 | Launch key from experiment, role, and trial; hard maximum duration | SATISFIED | `key.rs` (adds the judge version), `spec.rs` ceiling |
| CLI-11 | Deterministic, denylisted, secret-scanned, clean-checked snapshot | SATISFIED | `minos-source`; CLI flag wiring is Phase 8 |

### Anti-Patterns Found

No `TBD`, `FIXME`, `XXX`, `TODO`, or `HACK` markers, no `unsafe`, no `async`, and no `tokio` in the three crates. `#[allow(` is absent workspace-wide. No stubs or hollow data paths.

### Known and Recorded Items (confirmed present, not gaps)

| Item | Where it is recorded |
|------|----------------------|
| Flaky `supervise_run_kills_the_whole_process_group_at_the_host_deadline` (Phase 2 code) | `deferred-items.md`; `04-VALIDATION.md` Known limitations |
| The git clean-check deadline kills only the git process | `04-VALIDATION.md` Known limitations (04-04) |
| An lstat/open FIFO swap would hang the snapshot | `04-VALIDATION.md` Known limitations (04-07) |
| `LedgerRead::new` is crate-private | `04-VALIDATION.md` Known limitations (04-02) |
| Two suggested compile-fail cases not added (count fixed at 12) | `04-VALIDATION.md` Known limitations (04-06 to 04-08) |
| `cargo deny check` prints unused-wrapper warnings | `04-VALIDATION.md` Known limitations; I saw 10 warnings, exit 0 |

### Code Review Findings (advisory, not gaps)

`04-REVIEW.md` reports 0 critical, 3 warnings, 6 info, and no invariant bypass. My reading of the code agrees. Two notes for follow-up:

- **WR-01:** A newline-terminated non-JSON final ledger line is treated as a torn tail and truncated by the next append. This is the deliberate CONTEXT D-13 behavior ("incomplete JSON object at the tail"), so it meets the roadmap criterion. Narrowing it so only an unterminated tail is tolerated would be stricter against bit rot.
- **WR-03:** `Finished::new` borrows `&Cleared<R>`, so one clearance could mint several `Finished` values. No path exists today that lets untrusted code do this, since only backend authors hold a `Cleared`. Consuming it by value would close the gap before Phase 5 writes the real backend.
- WR-02 (launcher bounds restated in `minos-sandbox` with no drift test) is worth pinning when Phase 5 touches the launcher.

### Human Verification Required

None. Every must-have has automated, deterministic evidence, and the phase touches no UI, no VM, and no external service.

### Gaps Summary

None. All four verifiable roadmap success criteria hold in the code, and I confirmed each by reading the implementation and running the tests. All 11 requirement IDs are satisfied. The full gate (fmt, clippy, test, deny, doc, musl guest clippy, and the C-02 grep) is green on a fresh run. SC5 (`cargo clean`, GATE-06) is pending by design and is for the orchestrator to run after this verification. I did not run it.

---

_Verified: 2026-10-15_
_Verifier: Claude (gsd-verifier)_
