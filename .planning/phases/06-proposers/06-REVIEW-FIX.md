---
phase: 06-proposers
fixed_at: 2026-10-20T14:10:23Z
review_path: .planning/phases/06-proposers/06-REVIEW.md
iteration: 1
findings_in_scope: 9
fixed: 9
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-10-20T14:10:23Z
**Source review:** .planning/phases/06-proposers/06-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 9 (WR-01 to WR-05 in the first pass under `critical_warning`; IN-01 to IN-04 in the second
  pass under `all`; no Critical findings)
- Fixed: 9
- Skipped: 0

Two passes, one iteration. The first pass (commits 20a27c8, d8533c6, 78ede35, 6bc336f, cd42f22) fixed the five
Warning findings; the second (7e583ad, a07e098, 2028113, f42af68) fixed the four Info findings. Each pass ran in
its own git worktree on a temporary branch that was fast-forwarded into `main` and removed, with the recovery
sentinel dropped afterwards.

No golden snapshot changed in either pass: the argv, schema, and prompt goldens and their SHA-256 pins are
untouched (IN-01 kept the schema's `maxLength` at 2000, and IN-03's `golden_prompt_bytes_pinned` still passes). No
new dependency. The ignored live tests in `crates/minos-propose/tests/live.rs` were not run. `cargo clean` was not
run.

## Fixed Issues

### WR-01: The per-call working directory is not 0700 as documented

**Files modified:** `crates/minos-propose/src/claude_cli/mod.rs`, `crates/minos-propose/src/claude_cli/preflight.rs`, `crates/minos-propose/tests/claude_cli.rs`
**Commit:** 20a27c8
**Applied fix:** Both `tempfile::Builder` sites (`ClaudeCli::complete` and `preflight::run_step`) now pass
`.permissions(Permissions::from_mode(0o700))`, with a comment on why `tempfile`'s default (umask-derived) is not
enough on a shared `TMPDIR`.
**Tests added:** The fake `claude` script records `stat -f %Lp .` into `$HOME/fake/<step>.mode`, and the
`fresh_directory` helper asserts `700` for every step (`version`, `auth`, and `model`), so the two existing
fresh-directory tests now cover the mode. The fake is macOS-only, as the test module doc now states.

### WR-02: Model-authored `result` text can decide a run-ending stop before the `refusal` check runs

**Files modified:** `crates/minos-propose/src/claude_cli/classify.rs`, `crates/minos-propose/tests/classify.rs`
**Commit:** d8533c6
**Applied fix:** In the `is_error: true` branch, `stop_reason == "refusal"` is checked first, exactly as the
review's fix block shows, before `api_error_status` and before the message-prefix fallback reads `result`. The
`classify` doc comment's bullet for that branch is reordered to match.
**Tests added:** `a_refusal_whose_text_mimics_a_limit_or_login_message_is_still_an_empty_call`: four refusal
envelopes whose `result` begins with `You've hit your limit`, `You’ve reached your limit` (typographic
apostrophe), `Not logged in · Please run /login`, and `Failed to authenticate` are each
`Completion::Empty(EmptyCause::Refusal)` with usage, never a stop.

### WR-03: Over-cap model output is blamed on infrastructure and feeds the circuit breaker

**Files modified:** `CLAUDE.md`, `crates/minos-propose/src/claude_cli/mod.rs`, `crates/minos-propose/src/limits.rs`, `crates/minos-propose/src/provider.rs`, `crates/minos-propose/tests/claude_cli.rs`
**Commit:** 78ede35
**Applied fix:** New `EmptyCause::OutputTooLarge` (with `Display` text and a doc saying the usage is unknown and
recorded as zero). `ClaudeCli::complete` matches `process::run`'s result and maps `RunError::StdoutTooLarge` to
`Ok(ModelResponse::new(Completion::Empty(EmptyCause::OutputTooLarge), Usage::default(), BTreeMap::new()))`;
every other `RunError` still becomes `InfraCause`. `InfraCause::OutputTooLarge` is kept and its doc now says it
is for the 16 KiB preflight runs only. The `MAX_ENVELOPE_BYTES` doc states the bound it enforces, that the
schema's maximum (`BATCH_MAX` proposals of every writable file at `MAX_RETURNED_FILE_BYTES`, carried twice) is
far larger, and that `parse_output` enforces the per-file cap. The module and `complete` docs describe the new
outcome.
**CLAUDE.md:** the classifier sentence in "The model stays outside the sandbox" now lists a reply over the 4 MiB
stdout cap as an empty call (`EmptyCause::OutputTooLarge`, zero usage), and names the preflight-only
`InfraCause::OutputTooLarge`.
**Tests added:** `a_reply_over_the_envelope_cap_is_an_empty_call_with_zero_usage`: the fake `claude` prints
`MAX_ENVELOPE_BYTES + 1` bytes; the call is `Completion::Empty(EmptyCause::OutputTooLarge)` with
`Usage::default()`, no per-model usage, no resolved model, exactly one model run, and the call directory removed.
Nothing outside `minos-propose` consumes `EmptyCause` yet, so the new variant breaks no other crate.

### WR-04: After the leader exits, the reader joins have no deadline

**Files modified:** `CLAUDE.md`, `crates/minos-propose/src/claude_cli/mod.rs`, `crates/minos-propose/src/claude_cli/process.rs`, `crates/minos-propose/src/limits.rs`
**Commit:** 6bc336f
**Applied fix:** A bounded drain, as the review suggested. New `limits::DRAIN_GRACE` (5 s). In `process.rs` a
`Drain` helper waits on a pipe only in `rustix::event::poll` with a 100 ms timeout (`DRAIN_POLL`); `run` sets a
shared `killed: AtomicBool` right after `kill_and_reap`, and `DRAIN_GRACE` after a pipe thread first observes
the flag it abandons the pipe and returns what it has. `read_capped` (stdout, with the same overflow semantics:
one byte over the cap sets `overflow`, drops the kept bytes, and keeps draining) and `keep_tail` (stderr) go
through it. The stdin writer (`write_input`) is bounded the same way: the pipe is made non-blocking with
`ioctl_fionbio` so a write larger than the pipe buffer cannot block until the child reads it, and the thread
owns the pipe so it is closed when the writer finishes. A call therefore returns within its deadline plus the
grace. Module, `run`, and `ClaudeCli` docs describe the bound; the old "which the provider's programs do not"
assumption is removed.
**CLAUDE.md:** the D-16 sentence now records the `poll`-only pipe threads and the 5 s `DRAIN_GRACE`.
**Straggler mechanism:** macOS has no `setsid`, and `/usr/bin/perl` lacks `POSIX::setsid` without `use POSIX`,
so the tests use `set -m` in `/bin/sh`, which was verified on this host to give a background job its own process
group (`ps -o pgid` showed the job's own pid, not the shell's). Under `set -m` a *foreground* job also gets its
own group, so the deadline test's leader waits with the `wait` builtin rather than a foreground `sleep`; this
was found when a first version left a stray `sleep 30`.
**Tests added (in `process.rs`'s test module):**
`a_straggler_outside_the_group_cannot_hold_the_run_past_the_grace` (leader exits 0 at once with 1 MiB of unread
stdin while an escaped `sleep 30` holds all three pipes; the run returns with the leader's `ok` output in
`[DRAIN_GRACE, DRAIN_GRACE + 5 s)`, and the straggler is proven still alive, i.e. it escaped the kill, before
the test kills it) and `the_deadline_holds_with_a_straggler_outside_the_group` (300 ms deadline;
`Err(RunError::Deadline)` within the deadline plus the grace). The lib test suite now takes about 5.5 s because
each waits out the real grace.

### WR-05: The locator checks the target's write bits but not the directories that control replacing it

**Files modified:** `CLAUDE.md`, `crates/minos-propose/src/claude_cli/locate.rs`, `crates/minos-propose/tests/locate.rs`
**Commit:** cd42f22
**Applied fix:** The review's preferred form. After the versions directory canonicalizes, `check_dir` runs on
`$HOME/.local/bin` (by `symlink_metadata`: a real directory, not a symlink) and on the canonical versions
directory: each must be owned by the current uid with `mode & 0o022 == 0`. New `LocateFailure::DirNotADirectory`,
`DirWrongOwner`, and `DirWritable`, each carrying a `LocateDir` (`LinkDir` or `Versions`) so the message names
which directory; `LocateFailure` stays `Copy`. `fs::canonicalize(&link)` is replaced by one `read_link`: the text
must name a direct child of the versions directory, absolutely (the parent equals the versions path as `HOME`
gives it, or its canonical form) or as the relative `../share/claude/versions/<name>`, with a normal last
component; the target is `<canonical versions>/<name>` and is checked with `symlink_metadata`, so a symlink inside
the versions directory is refused as `NotAFile`. No intermediate hop is ever followed. The real native install
on this host writes an absolute link text (`~/.local/bin/claude -> /Users/<user>/.local/share/claude/versions/2.1.304`,
both directories 0755 and user-owned), so it is accepted. `LocateError::target()` is now the link's own text when
the text was refused or dangles, else the canonical target; two existing `OutsideVersions` tests were updated to
expect the text rather than a canonicalized path.
**Deliberate test change:** `a_link_chain_into_versions_is_accepted_and_follows_a_move` is replaced by
`a_link_through_an_intermediate_hop_is_refused` (the hop layout is now `OutsideVersions`, the direction the
review called for) and `a_moved_link_is_followed` (the auto-updater's relink still works).
**Tests added:** `a_relative_link_into_versions_is_accepted`, `a_relative_link_of_any_other_shape_is_refused`
(bare name, subdirectory, `../../` form, trailing `..`, trailing `.`), `a_link_naming_the_versions_directory_itself_is_refused`,
`a_symlink_inside_the_versions_directory_is_refused`, `a_group_writable_versions_directory_is_refused` (0775),
`a_world_writable_link_directory_is_refused` (0777), `a_link_directory_that_is_itself_a_symlink_is_refused`,
`directory_checks_run_before_the_link_is_read`, and the new variants in `failures_display_fixed_text`.
**Not testable:** `DirWrongOwner` cannot be exercised without creating a directory owned by another user, which
needs privileges; it is covered by construction only (the same `uid` comparison as the target's `WrongOwner`
check, which is likewise untested for the same reason). The test module doc says so.
**CLAUDE.md:** the D-01 sentence records the never-followed link text, the direct-child rule, and the two
directory checks.

### IN-01: Rationale cap is characters in the schema and bytes in the parser

**Files modified:** `crates/minos-propose/src/limits.rs`, `crates/minos-propose/src/proposal.rs`, `crates/minos-propose/src/schema.rs`, `crates/minos-propose/src/sweep.rs`, `crates/minos-propose/tests/parse.rs`, `crates/minos-propose/tests/schema.rs`
**Commit:** 7e583ad
**Applied fix:** The review's first option, which leaves the schema unchanged. The cap is renamed
`MAX_RATIONALE_CHARS` (2000, the schema's `maxLength`, so `tests/snapshots/schema__output_schema_batch_4.snap`
and its pin are untouched) and documented as a character count; `MAX_RATIONALE_BYTES` is now
`MAX_RATIONALE_CHARS * 4`, the UTF-8 ceiling, kept as the storage bound. `check_rationale` refuses an empty
string, a string over the byte ceiling (checked first, so a huge string is never counted), one over the character
cap (`chars().count()`), or one holding NUL. `OutputSchema::for_batch` takes `maxLength` from
`MAX_RATIONALE_CHARS`. The sweep still cuts its own `name=value` description at 2000 bytes at a char boundary,
which is at most 2000 characters, so its proposals are unchanged. `Proposal::rationale`'s doc names both units.
**Tests added:** `rationale_cap_counts_characters_not_bytes` in `tests/parse.rs`: a rationale of
`MAX_RATIONALE_CHARS` two-byte characters (over the old byte cap) and one of `MAX_RATIONALE_CHARS` four-byte
characters (exactly `MAX_RATIONALE_BYTES`) are each accepted with no drop; one more character of either, and an
ASCII string of `MAX_RATIONALE_BYTES + 1`, are `RationaleInvalid`. The existing `drops_rationale_invalid` and the
schema test now use `MAX_RATIONALE_CHARS`.

### IN-02: A failed directory removal in `run_step` masks the process error

**Files modified:** `crates/minos-propose/src/claude_cli/preflight.rs`, `crates/minos-propose/tests/claude_cli.rs`
**Commit:** a07e098
**Applied fix:** `run_step` now matches `(dir.close(), output)` exactly as the review's fix block shows: the
query's output when the removal succeeded, the query's own `PreflightError::Process` when both failed, and
`WorkRootUnusable` only when the query succeeded and the directory still could not be removed. The function doc
states the rule.
**Harness:** The fake `claude` gains a per-step leak control: when `$HOME/fake/<step>.leak` exists it leaves a
read-only (0500) `keep` directory holding a file in its working directory, which the provider cannot remove
(unlinking needs write permission on `keep`). A `LeakGuard` in the test file repairs and removes every leftover
`minos-claude-*` directory under the work root when dropped, whether or not the assertions passed, and is
declared after the fixture so it runs before the fixture's `TempDir` is dropped. The module doc records the
control.
**Tests added:** `a_failed_query_is_reported_even_when_its_directory_cannot_be_removed` (`--version` prints
`MAX_PREFLIGHT_STDOUT_BYTES + 1` bytes with a leak: the refusal is
`Process { step: Version, cause: InfraCause::OutputTooLarge }`, `auth status` never ran, and the directory is
proven still present) and `a_directory_that_cannot_be_removed_after_a_successful_query_is_unusable` (a
successful `--version` with a leak is `WorkRootUnusable { path: <canonical TMPDIR> }`).

### IN-03: The compile-time prompt-size proof assumes a fence overhead the code does not bound

**Files modified:** `crates/minos-propose/src/limits.rs`, `crates/minos-propose/src/prompt.rs`, `crates/minos-propose/src/prompt/history.rs`, `crates/minos-propose/tests/prompt.rs`, `crates/minos-propose/tests/history.rs`
**Commit:** f42af68
**Applied fix:** Block counts are capped as well as bytes, and the proof now derives from named constants. New
`STEER_NOTES_MAX_COUNT` (64; `render_suffix` keeps the highest ordinals with `.take(..)` before the byte
`take_while`, and the omitted line counts the rest) and `EXCERPTS_MAX_COUNT` (32; `History::new` keeps the newest
with `.take(..)` before the byte loop). `render_row` filters out an excerpt whose sanitized text is empty, so
every kept excerpt costs at least one byte against `EXCERPTS_MAX_BYTES`. In `limits.rs`, `FENCE_FIXED_BYTES` is
derived from `push_block`'s shape, `block_overhead(kind, label_max)` is a `const fn`, and the public
`NOTE_BLOCK_OVERHEAD_BYTES` and `EXCERPT_BLOCK_OVERHEAD_BYTES` bound each block kind with its longest label (the
excerpt label at a `Reason` display naming a `MAX_PATH_BYTES` path). `PROMPT_OVERHEAD_BYTES` (unchanged at
64 KiB) now documents exactly what it covers: the fixed text (`FIXED_TEXT_MAX_BYTES`, 8 KiB, about 1.5 KiB as
written), the objective and gate lines, the `HISTORY` fence, `EXCERPTS_MAX_COUNT` excerpt fences, and
`STEER_NOTES_MAX_COUNT` note fences, with a const assertion that this sum fits. The surface-proportional fences
(the writable list, the program heading and `PROGRAM` fence, and one `FILE` fence per writable and readable
path, every path at `MAX_PATH_BYTES`) are a separate `SURFACE_OVERHEAD_MAX_BYTES` term in the whole-prompt
assertion, which still ends well under `MAX_PROMPT_BYTES`. The `assemble` and `History::new` docs state the new
rules. Nothing changes for ordinary inputs: `golden_prompt` and `golden_prompt_bytes_pinned` pass unchanged.
**Tests added:** `notes_are_capped_by_count` and `a_note_block_costs_its_documented_overhead` (a block at
ordinal `u32::MAX` costs exactly `NOTE_BLOCK_OVERHEAD_BYTES` besides its text, pinning the constant to the real
render) in `tests/prompt.rs`; `empty_excerpts_are_skipped` (an empty excerpt and an ESC-only one are skipped
while their rows stay) and `excerpts_are_capped_by_count` (the newest `EXCERPTS_MAX_COUNT` of 40 one-byte
excerpts, in table order) in `tests/history.rs`.

### IN-04: A successful completion is discarded when the call directory cannot be removed

**Files modified:** `CLAUDE.md`, `crates/minos-propose/src/claude_cli/mod.rs`, `crates/minos-propose/src/provider.rs`, `crates/minos-propose/tests/claude_cli.rs`
**Commit:** 2028113
**Applied fix:** The smallest sound version the task described. A removal failure never changes a call's result:
`finish` is gone, `complete` returns the classified result as is and, when `dir.close()` fails, records the
directory's path in a new `ClaudeCli`-owned `Mutex<BTreeSet<PathBuf>>` (a poisoned lock is recovered with
`PoisonError::into_inner`, since the set is only ever replaced whole; no `unwrap`). `ClaudeCli::leaked_dirs()`
returns the recorded paths in order, `Debug` shows their count, and every `complete` first calls `remove_leaked`,
which retries `remove_dir_all` on each recorded path and forgets those removed or already gone, keeping the
rest; only directories this provider created are ever named. `InfraCause::WorkDir` now covers creation only
(doc and message updated).
**Follow-up, not done here:** a cross-process sweep of leftover `minos-claude-*` directories under the work root
(for `minos host doctor` or the next preflight) would need an ownership protocol like the krun job root's
`owner.lock`, because another `minos` process may be mid-call in a directory of the same shape; that is more
design than this finding should carry. CLAUDE.md records it as a follow-up.
**CLAUDE.md:** the "removed afterwards" sentence in "The model stays outside the sandbox" now states that a
directory that cannot be removed never changes the call's result, that the provider records it and
`ClaudeCli::leaked_dirs` names it, that the next call retries the removal, and the follow-up.
**Tests added:** `a_call_whose_directory_cannot_be_removed_still_returns_its_completion`: with the fake's
`model.leak` control the call returns `Completion::Structured` with the verified usage and resolved model,
`leaked_dirs()` names exactly the recorded `model.pwd`, `Debug` shows `leaked: 1`; a second call (leak control
removed, directory still read-only) keeps it recorded and removes its own directory; after the test repairs the
permissions a third call removes and forgets it, and nothing is left under the work root.

## Skipped Issues

None.

## Gate commands

Per fix, before its commit (each passed): `cargo fmt --all --check`,
`cargo clippy -p minos-propose --all-targets -- -D warnings`, `cargo test -p minos-propose`, and
`cargo doc -p minos-propose --no-deps` for the fixes that added rustdoc links (WR-03, WR-04, WR-05, IN-03,
IN-04).

After the last fix, on the final tree (commit f42af68):

| Command | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (no warnings) |
| `cargo test --workspace` | pass: 1378 passed, 0 failed, 23 ignored (the VM and live tests); 1370 before the second pass plus the 8 new tests |
| `cargo deny check` | pass: advisories ok, bans ok, licenses ok, sources ok |
| `cargo doc --workspace --no-deps` | pass (no warnings) |
| `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | pass (no warnings) |
| `grep -rnE '#!?\[allow\(' crates --include='*.rs'` (C-02) | prints nothing |

Not run: the ignored VM tests (need the signed helper and libkrunfw), the ignored live provider tests (real model
calls), and `cargo clean`.

---

_Fixed: 2026-10-20T14:10:23Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
