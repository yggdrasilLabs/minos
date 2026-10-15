---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 06
subsystem: sandbox
tags: [minos-sandbox, untrusted, output-tar, raw-mode, fuzz-corpus, proptest, judge-integrity]
status: complete

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    plan: 03
    provides: sealed Role, Candidate, Judge, LaunchKey, SandboxSpec, seal, LaunchPlan, Capabilities, Cleared, preflight
  - phase: 03-core-types-and-pure-logic
    provides: Scorecard::parse_json, ScorecardError, LogExcerpt::from_bytes, Limits, BATCH_MAX, Reason, RelPath
provides:
  - Untrusted<T> (one private field, crate-private new, no Debug/Display/Deref/Clone/Default)
  - GuestOutput<R> and OutputFile<R>, role-typed and crate-constructed only
  - Layout constants SCORECARD_PATH, BUILD_LOG_PATH, RUN_LOG_PATH, JUDGE_LOG_PATH, DIFFS_DIR, OUT_DIR
  - Exits into_scorecard (judge output only), into_log_excerpt, into_diff_texts; projection take; entry_count and content_bytes
  - ScorecardFault (Missing, Invalid) with reason() = Reason::ScorecardInvalid
  - Crate-private read_output, caps MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRY_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_STREAM_BYTES, MAX_DIFF_TEXTS
  - OutputFault (TooLarge, Invalid) with OutputCap and OutputInvalid; reason() = OutputTooLarge or OutputInvalid
  - Finished<R> (new from &Cleared<R>, ended, key, into_output), Ended (Exited, Crashed, TimedOut), CrashCause (NoStatusFrame, ConsoleCap)
  - 20 committed, SHA-256-pinned hostile output tar fixtures
affects: [04-08 Backend trait and compile-fail cases, phase-5 guest output writer, phase-7 runner]

tech-stack:
  added: []
  patterns:
    - "Hostile tar: refuse the slice by length and block alignment first, iterate entries with raw(true), check count, ustar magic, typeflag, size, name, total, and duplicates before reading data, then require two zero blocks and only zeros after the last entry"
    - "Role-typed output: an exit that only one role may use is an inherent impl on the concrete Untrusted<GuestOutput<Judge>>, never a runtime role check"
    - "Committed binary fixtures: builders in the test file, an ignored writer test, shasum pins pasted into a table, and a corpus_matches_builders test that ties each file to its builder"

key-files:
  created:
    - crates/minos-sandbox/src/untrusted.rs
    - crates/minos-sandbox/src/output.rs
    - crates/minos-sandbox/src/finished.rs
    - crates/minos-sandbox/tests/output_tar.rs
    - crates/minos-sandbox/tests/untrusted.rs
    - crates/minos-sandbox/tests/fixtures/output_tar/*.tar (20 files)
  modified:
    - crates/minos-sandbox/src/lib.rs

key-decisions:
  - "04-06: Finished<R> refines D-17. wait returns Finished, which holds the trusted, host-observed Ended (Exited, Crashed(CrashCause), TimedOut) beside the still-hostile stream, so K7's host-observed results are not hidden behind Untrusted exits. Infrastructure failures stay on the Err path."
  - "04-06: the output reader uses tar raw mode and refuses every header that is not ustar with typeflag b'0'. GNU long names, PAX records, sparse files, links, devices, FIFOs, directories, and legacy b'\\0' regular files are all refused, so the PAX size-desync parser differential cannot occur."
  - "04-06: every output reader fault is a candidate fault (Reason::OutputInvalid or Reason::OutputTooLarge), never retried, because the stream is already in memory when it is read."
  - "04-06: guest output layout: .minos/scorecard.json, .minos/{build,run,judge}.log, diffs/, out/. The status frame over vsock carries the run's result; these files carry only data."

patterns-established:
  - "Untrusted exits delegate to the core validators (Scorecard::parse_json, LogExcerpt::from_bytes); the sandbox crate adds only shape and cap checks"

requirements-completed: [SBX-04, SBX-05]

duration: 9min
completed: 2026-10-15
---

# Phase 4 Plan 06: Untrusted output, the raw-mode tar reader, and Finished Summary

**Guest output is parsed in memory by a capped raw-mode ustar reader into a role-typed `Untrusted<GuestOutput<R>>`. Only a `Finished<R>` built from a `Cleared<R>` can produce one. Its contents leave through three capped exits, and only judge output has `into_scorecard`.**

## Performance

- **Duration:** about 9 min
- **Started:** 2026-10-15T14:28:46Z
- **Completed:** 2026-10-15T14:37:38Z
- **Tasks:** 3 (each in TDD form: a RED commit, then a GREEN commit)
- **Files:** 5 source and test files created, 1 modified, 20 fixtures committed

## Accomplishments

- `Untrusted<T>` has one private field and a crate-private `new`. It implements none of `Debug`, `Display`, `Deref`, `Clone`, or `Default`. `GuestOutput<R>` and `OutputFile<R>` carry the role as `PhantomData<fn() -> R>`, and only this crate constructs them.
- `read_output` (crate-private) checks in this order:
  1. The stream cap and 512-byte alignment, before any parsing.
  2. Entries are read in raw mode. For each entry, before its data is read: the entry count, the ustar magic, a typeflag of exactly `b'0'`, the 8 MiB entry cap, a UTF-8 `RelPath` name, the 16 MiB running total, and duplicate names.
  3. The data is read through `take(size)` with at most 64 KiB preallocated. A short read is `Truncated`.
  4. At least two zero blocks must follow the last entry, and nothing but zeros after them.
- `Finished<R>` holds the plan's `LaunchKey`, the host-observed `Ended`, and the unread stream. `Finished::new(&Cleared<R>, Ended, Vec<u8>)` is its only constructor.
- Corpus: 20 fixtures, each built header by header with `tar::Header` (no system `tar`). Each is pinned by its SHA-256 and rejected with the exact fault in the plan's table. `valid_judge_output.tar` is the positive control and reads with 3 entries.
- Generated bombs, never committed:
  - `entry_count_bomb`: 1025 entries give `TooLarge { cap: Entries }`.
  - `million_entry_stream`: a 34,821-block (17,828,352-byte) prefix gives `TooLarge { cap: Stream }`. A const assertion beside it proves that every million-entry stream is longer than the stream cap.
  - `total_over_cap`: 16 MiB plus 512 bytes of content gives `TooLarge { cap: Total }`.
- Two proptest properties, `structured_tars_never_panic` and `raw_bytes_never_panic`, found no panic. Every accepted output stayed within the entry-count and content caps, and every refusal mapped to an output reason. No regression file was written.
- The exits:
  - `into_scorecard` is implemented only on `Untrusted<GuestOutput<Judge>>` and delegates to `Scorecard::parse_json`.
  - `into_log_excerpt` delegates to `LogExcerpt::from_bytes`.
  - `into_diff_texts` returns at most 16 texts under `diffs/`, in path order. Each is at most `max_diff_bytes`, valid UTF-8, and free of NUL and CR.

## Task Commits

1. **Task 1: Untrusted, GuestOutput, Finished, and the raw-mode tar reader.** RED `995ad1f`, GREEN `5ad3fb5`
2. **Task 2: The committed hostile corpus, generated bombs, and never-panic properties.** RED `15c8725` (builders, replay tests, bombs, and properties, with the pins pending), then `86f346e` (20 fixtures written by `write_corpus` and their pins pasted in)
3. **Task 3: The three capped exits, judge-only scorecard.** RED `1939875`, GREEN `9401f0f`

## Fixture provenance

`cargo test -p minos-sandbox --test output_tar -- --ignored write_corpus` wrote all 20 files from the builder functions in `crates/minos-sandbox/tests/output_tar.rs`. Each builder writes headers through `tar::Header::new_ustar()`, `new_gnu()`, or `new_old()`. Hostile names go straight into the 100-byte name field through `as_old_mut().name`, and each header then gets `set_cksum`. `shasum -a 256 crates/minos-sandbox/tests/fixtures/output_tar/*.tar` produced the pins in `CORPUS`. Two tests keep the files honest: `corpus_is_pinned` checks the bytes on disk, and `corpus_matches_builders` checks that each file still equals its builder's output.

## Decisions Made

- **Finished refines D-17** (RESEARCH Open Question 1), as the plan prescribed. The reasoning is in the `finished.rs` module doc.
- **Cap types:** the byte caps are `u64`, because tar sizes are `u64`. `MAX_OUTPUT_ENTRIES` and `MAX_DIFF_TEXTS` are `usize`, because they are compared with collection lengths. Clippy accepted the two const `as` widenings (`usize` to `u64` and `u32` to `usize`) without a lint.
- **The typeflag check** is written `!matches!(header.as_old().linkflag, [b'0'])`. Clippy's `byte_char_slices` rejects `!= [b'0']`, and the pattern form compares the same raw byte.

## Deviations from Plan

### Ordering and lint adjustments

**1. `OutputFile<R>` was added in Task 3, not Task 1**
- **Found during:** Task 1
- **Issue:** In Task 1, `OutputFile` would have had a constructor and a field that nothing used. The choice was a dead-code warning or an `expect(dead_code)` attribute.
- **Fix:** It was defined in Task 3 next to `take` and `into_log_excerpt`, which use it. No attribute was added.
- **Files:** `crates/minos-sandbox/src/untrusted.rs`

**2. The ignored writer uses `#[ignore = "reason"]`, not a bare `#[ignore]`**
- **Found during:** Task 2
- **Issue:** One acceptance criterion greps for `#\[ignore\]`. Every other ignored test in the repo carries a reason, which pedantic clippy's `ignore_without_reason` asks for.
- **Fix:** `#[ignore = "writes the committed corpus; run by hand, then re-pin"]`. Exactly one `#[ignore` line exists in `output_tar.rs`, so the criterion's intent (one ignored writer) holds.

**3. Infallible fixture builders return `Vec<u8>`**
- **Found during:** Task 2
- **Issue:** Clippy's `unnecessary_wraps` flagged the builders that cannot fail.
- **Fix:** They return `Vec<u8>`, and the `CORPUS` table wraps them in non-capturing closures, for example `|| Ok(build_v7_header())`.

**4. Test-only lint fixes**
- A `ReadResult` type alias satisfies `type_complexity`, and `let ... else` satisfies `manual_let_else`. Neither changes behavior.

**5. Extra test coverage beyond the behavior blocks**
- Added `gnu_and_v7_headers_are_not_ustar`, so the Task 1 commit used every `Magic` variant. Also added `sixteen_diffs_are_accepted` and an at-cap diff case as boundary checks.
- The structured property's names are weighted toward valid paths (`out/a`, `out/b`, `diffs/0.diff`), so accepted outputs and duplicate names are actually exercised.

**Total deviations:** 5, all ordering, lint, or test-coverage adjustments. **Impact:** none on any invariant. No `allow` or `expect` attribute was added, and `Cargo.toml`, `Cargo.lock`, and `deny.toml` are untouched.

## TDD Gate Compliance

Each task has a `test(04-06)` RED commit before its implementation commit.

- Tasks 1 and 3: the RED commits failed to compile, because the types did not exist yet.
- Task 2: the RED commit failed at `corpus_is_pinned`, `corpus_matches_builders`, and every fixture replay, because no fixtures had been written.
- Task 2 is test-only. Its replay assertions run against Task 1's reader, so they passed as soon as the fixtures existed. That is expected: the corpus tests the reader and adds no new behavior. The second Task 2 commit is typed `test`, because it adds fixtures and pins.

## Issues Encountered

None. The full gate passed on the first workspace run, and the known flaky backend-krun supervise test did not fail.

## Notes for later plans

- The tar crate's `Header::path_bytes` stops at the first NUL in the name field and joins in the ustar `prefix` field. A name such as `a\0junk` therefore reads as `a`. The result still goes through `RelPath`, so this is not an escape. Phase 5's guest writer should write plain names with no prefix.
- 04-08 can add the compile-fail cases against these shapes: `into_scorecard` on `Untrusted<GuestOutput<Candidate>>`, `Untrusted::new` from outside the crate, and `Finished::new` with a `Cleared` of the wrong role.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 884 passed, 0 failed, 26 ignored
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`
- `cargo doc --workspace --no-deps`: no warnings
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-01: no workspace member added, so it is unchanged from 04-01
- C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing
- `grep -rn 'std::fs' crates/minos-sandbox/src`: prints nothing
- `grep -c 'impl Untrusted<GuestOutput<Judge>>'` prints 1, and the same grep for `Candidate` prints 0
- 20 fixtures exist, and `output_tar.rs` holds 20 SHA-256 literals

## Threat Mitigations

- T-04-06-01: every name goes through `RelPath::try_from`. Nothing is extracted to disk, and the clippy bans on unpack, `File`, and `OpenOptions` stay in force. Covered by the path_traversal, absolute_path, and non_grammar_name fixtures.
- T-04-06-02: the stream cap and alignment are checked before parsing, and the count, entry, and total caps before any data is read. Covered by the generated bombs.
- T-04-06-03: raw mode, ustar only, and typeflag `b'0'` only. Covered by the pax_size_desync, pax_path_override, and gnu_long_name fixtures.
- T-04-06-04: links, devices, FIFOs, and directories are refused by typeflag, with one fixture for each.
- T-04-06-05: `into_scorecard` exists only for judge output, `Finished<R>` needs a `Cleared<R>`, and `read_output` is `pub(crate)`.
- T-04-06-06: short data is `Truncated`, and two zero end blocks followed only by zeros are required. Covered by the truncated_data, no_end_marker, and trailing_data fixtures.
- T-04-06-07: `Untrusted`, `GuestOutput`, `OutputFile`, and `Finished` implement no formatting traits. `OutputFault` carries no input bytes, and logs leave only through `LogExcerpt`.
- T-04-06-08: there is no indexing or unwrap, and both proptest strategies pass.

## Next Phase Readiness

04-08 can add the `Backend` trait, with `wait` returning `Finished<R>`, and the trybuild cases above.

## Self-Check: PASSED
