---
phase: 06-proposers
reviewed: 2026-10-20T00:00:00Z
depth: standard
files_reviewed: 42
files_reviewed_list:
  - .claude/CLAUDE.md
  - crates/minos-core/src/ledger.rs
  - crates/minos-core/tests/golden_wire.rs
  - crates/minos-core/tests/prop_keys.rs
  - crates/minos-core/tests/snapshots/golden_wire__record_v1_stop_requested_provider_auth.snap
  - crates/minos-propose/Cargo.toml
  - crates/minos-propose/clippy.toml
  - crates/minos-propose/src/claude_cli/classify.rs
  - crates/minos-propose/src/claude_cli/command.rs
  - crates/minos-propose/src/claude_cli/env.rs
  - crates/minos-propose/src/claude_cli/locate.rs
  - crates/minos-propose/src/claude_cli/mod.rs
  - crates/minos-propose/src/claude_cli/preflight.rs
  - crates/minos-propose/src/claude_cli/process.rs
  - crates/minos-propose/src/fake.rs
  - crates/minos-propose/src/lib.rs
  - crates/minos-propose/src/limits.rs
  - crates/minos-propose/src/prompt.rs
  - crates/minos-propose/src/prompt/history.rs
  - crates/minos-propose/src/proposal.rs
  - crates/minos-propose/src/proposer.rs
  - crates/minos-propose/src/provider.rs
  - crates/minos-propose/src/schema.rs
  - crates/minos-propose/src/single_shot.rs
  - crates/minos-propose/src/sweep.rs
  - crates/minos-propose/tests/argv_golden.rs
  - crates/minos-propose/tests/classify.rs
  - crates/minos-propose/tests/claude_cli.rs
  - crates/minos-propose/tests/held_out.rs
  - crates/minos-propose/tests/history.rs
  - crates/minos-propose/tests/live.rs
  - crates/minos-propose/tests/locate.rs
  - crates/minos-propose/tests/parse.rs
  - crates/minos-propose/tests/prompt.rs
  - crates/minos-propose/tests/proposer.rs
  - crates/minos-propose/tests/provider.rs
  - crates/minos-propose/tests/schema.rs
  - crates/minos-propose/tests/single_shot.rs
  - crates/minos-propose/tests/snapshots/argv_golden__claude_argv.snap
  - crates/minos-propose/tests/snapshots/prompt__golden_prompt.snap
  - crates/minos-propose/tests/snapshots/schema__output_schema_batch_4.snap
  - crates/minos-propose/tests/sweep.rs
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 06: Code Review Report

**Reviewed:** 2026-10-20
**Depth:** standard
**Files Reviewed:** 42
**Status:** issues_found

## Summary

Reviewed the new `minos-propose` crate (prompt assembly, the strict proposal parser, the `claude` CLI provider with its locator, preflight, subprocess runner, and classifier, the fake provider, and the sweep and single-shot tiers), the `StopReason::ProviderAuth` addition to the core ledger with its golden and property tests, and the Phase 6 corrections block in `.claude/CLAUDE.md`. Every source file was read in full and the core APIs the crate leans on (`Tree`, `diff_trees`, `SafeText`, `RelPath`, `ModelId`, `reduce_trials`, `Reason`) were checked against their definitions. No `cargo` command was run, by instruction; the pattern scan for `#[allow(`, debug artifacts, and shell strings in `src/` is clean.

The invariants the task asked about hold where I could trace them. In `proposal.rs` no unvalidated model text escapes: every path goes through `RelPath::try_from` and a byte-equal `writable` check, content is CR/NUL-scanned and capped before it enters a tree, and the only exit is `diff_trees` into a validated canonical `Diff`; the rationale is the one raw model string retained, and every renderer that shows it passes it through `SafeText`. The fence tokens in `prompt.rs` are 64 bits of a domain-separated hash of the fenced content, so an excerpt cannot close its own block, and every label that reaches a BEGIN line is either a `RelPath` (ASCII `[A-Za-z0-9._+-/]` only, so it cannot carry a space or newline) or `SafeText::inline` output. The environment is cleared and rebuilt from an allowlist of six variables; refused variables are recorded by name only. The argv is an array whose variable elements (`ModelId`, the schema JSON) cannot be read as flags. Token counts are the only usage read from the envelope; no `total_cost_usd` or `costUSD` field is declared. `Cargo.toml` adds no tokio.

The findings below are where the implementation falls short of its own stated contract or where blame is misassigned. The most consequential: the per-call working directory is documented as 0700 but is created with default permissions; the classifier consults model-authored `result` text to decide a run-ending stop before it checks for a `refusal`; an over-cap model output is classified as an infrastructure failure, which feeds the circuit breaker; and the subprocess runner can block without a deadline after the leader exits if a group-escaped straggler holds a pipe.

## Warnings

### WR-01: The per-call working directory is not 0700 as documented

**File:** `crates/minos-propose/src/claude_cli/mod.rs:147-150`, `crates/minos-propose/src/claude_cli/preflight.rs:403-406` (claims at `mod.rs:15` and `preflight.rs:20`)
**Issue:** Both call sites use `tempfile::Builder::new().prefix(RUN_DIR_PREFIX).tempdir_in(..)` with no `.permissions(..)`. In tempfile 3.27.0 (`src/dir/imp/unix.rs`), `DirBuilder::mode` is applied only when `permissions` is `Some`; otherwise the directory gets the process default (umask-derived, typically 0755), and the crate's own docs say so ("temporary directories are created with the default permissions and will therefore be ... readable by other users"). The module contract states the CLI runs in "a fresh, empty, private (0700) `minos-claude-*` directory". On the default macOS `TMPDIR` the 0700 parent masks this, but a user with `TMPDIR=/tmp` (or any shared root) gets a world-listable cwd for the `claude` process. The krun backend's job directories are explicitly 0700; this path should match it rather than rely on the parent.
**Fix:**
```rust
use std::os::unix::fs::PermissionsExt;

let dir = tempfile::Builder::new()
    .prefix(RUN_DIR_PREFIX)
    .permissions(std::fs::Permissions::from_mode(0o700))
    .tempdir_in(&call.work_root)
    .map_err(|_| infra(InfraCause::WorkDir, None))?;
```
Apply the same to `run_step` in `preflight.rs`, and add an assertion in `tests/claude_cli.rs` (the fake already records `pwd -P`; have it also record `stat -f %Lp .`) that the mode is `700`.

### WR-02: Model-authored `result` text can decide a run-ending stop before the `refusal` check runs

**File:** `crates/minos-propose/src/claude_cli/classify.rs:218-234`
**Issue:** In the `is_error: true` branch, when `api_error_status` is neither 429 nor 401/403 (including `null`), the code falls to `envelope.result.as_deref().and_then(message_stop)` and returns `ProviderError::Stop` on a prefix match, and only afterwards checks `stop_reason == "refusal"`. For a `refusal` turn the `result` string is the model's own text (the test fixture at `tests/classify.rs:234` uses `"I can't help with that."`). The prompt deliberately carries untrusted sandbox output, so a candidate's log can steer the model's wording; a refusal whose text begins with `You've hit your`, `Not logged in`, or any other listed prefix is classified as `ProviderStop::Limit` or `ProviderStop::Auth`, and the loop records `StopRequested { ProviderLimit | ProviderAuth }` and ends the run. That lets sandbox-influenced text choose a stop reason that the design reserves for the provider. The CLI-authored limit and auth messages the fallback exists for do not arrive with `stop_reason: "refusal"`, so reordering loses nothing.
**Fix:**
```rust
Some(true) => {
    if envelope.stop_reason.as_deref() == Some("refusal") {
        return empty(EmptyCause::Refusal, &envelope, usage);
    }
    let status = envelope.api_error_status.and_then(|s| u16::try_from(s).ok());
    let stop = match status {
        Some(429) => Some(ProviderStop::Limit),
        Some(401 | 403) => Some(ProviderStop::Auth),
        _ => envelope.result.as_deref().and_then(message_stop),
    };
    if let Some(stop) = stop {
        return Err(ProviderError::Stop { stop, usage });
    }
    Err(infra(InfraCause::ApiError { status }, usage))
}
```
Add a table test: `stop_reason: "refusal"` with `result: "You've hit your limit"` must be `Completion::Empty(Refusal)`, not a stop. Update the doc comment order at lines 136-139.

### WR-03: Over-cap model output is blamed on infrastructure and feeds the circuit breaker

**File:** `crates/minos-propose/src/claude_cli/process.rs:83-91`, `crates/minos-propose/src/claude_cli/mod.rs:162-164`, `crates/minos-propose/src/limits.rs:52-55`
**Issue:** `RunError::StdoutTooLarge` maps to `InfraCause::OutputTooLarge`, so `ClaudeCli::complete` returns `ProviderError::Infra`, which the loop counts toward `max_infra_failures` (default 3) and retries. But the size of stdout on a successful `-p` call is determined by the model: the envelope carries the structured output twice (as the object and as the `result` string), and the schema permits up to `batch` (16) proposals, each with up to `writable.len()` (64) files of up to `MAX_RETURNED_FILE_BYTES` (256 KiB). A perfectly schema-valid batch of 4 proposals each returning two 256 KiB files is already over the 4 MiB `MAX_ENVELOPE_BYTES`, and a model steered by hostile sandbox text can exceed it at will. Under "Errors separate blame", output the model chose to write is a candidate-side outcome (zero valid proposals), not a retried infrastructure failure; as written, three oversized replies stop the run with `StopReason::InfraFailures` and misreport the cause. The `MAX_ENVELOPE_BYTES` doc comment ("twice the largest output plus room") understates the schema's maximum by two orders of magnitude.
**Fix:** Treat an over-cap reply as an empty call rather than an infrastructure failure: add `EmptyCause::OutputTooLarge` in `provider.rs`, and in `ClaudeCli::complete` map `RunError::StdoutTooLarge` to `Ok(ModelResponse::new(Completion::Empty(EmptyCause::OutputTooLarge), Usage::default(), BTreeMap::new()))` (usage is unknown because the envelope was never read; document that). Keep `InfraCause::OutputTooLarge` only for the preflight runs, where the cap is 16 KiB and an overflow is a genuine install fault. Correct the `MAX_ENVELOPE_BYTES` comment to state the actual bound it enforces and that `parse_output` is what enforces the per-file cap.

### WR-04: After the leader exits, the reader joins have no deadline

**File:** `crates/minos-propose/src/claude_cli/process.rs:138-155`
**Issue:** `watch` returns on leader exit, overflow, or the deadline; `kill_and_reap` then SIGKILLs the process group and reaps the leader; then `out.join()` and `err.join()` block until the pipes reach EOF. A descendant that left the group (`setsid`/`setpgid`) and still holds the inherited stdout or stderr fd is not killed, so the join blocks indefinitely and `CALL_DEADLINE` no longer bounds the call. The function doc acknowledges this ("except one that left the group on its own, which the provider's programs do not"), but that is an assumption about a closed-source binary that spawns background work (its auto-updater is what moves the versions link). D-16 says the deadline bounds the call; this path does not honor it.
**Fix:** Bound the post-kill drain. Simplest in std: after `kill_and_reap`, if `started.elapsed()` has reached the deadline or a short grace (say 5 s) passes without the readers finishing, stop waiting on them. Because `std::thread::scope` must join, do the bounded wait inside the reader threads instead: set a `read_timeout` on the pipe by wrapping the raw fd in a `UnixStream`-like poller (`rustix::event::poll` with a timeout in `read_capped`/`keep_tail`, checking a shared `AtomicBool` the watcher sets after the group kill), and return what was read so far when the flag is set and no data arrived within the grace. Add a test: `sh -c 'setsid /bin/sleep 30 & exit 0'` (or `/usr/bin/nohup` with its own pgid) must still return within the grace.

### WR-05: The locator checks the target's write bits but not the directories that control replacing it

**File:** `crates/minos-propose/src/claude_cli/locate.rs:192-220`
**Issue:** Step 5 refuses a target with any group or world write bit, with the stated reason that such a file "could be replaced by another user". But replacing a file does not require write permission on the file; it requires write permission on its directory (unlink and create, or rename over). Neither the canonical versions directory, nor `$HOME/.local/bin` (which controls who can repoint the symlink), nor any intermediate hop that `fs::canonicalize` follows (the `a_link_chain_into_versions_is_accepted` test shows hops anywhere are accepted) is checked for ownership or write bits. The check is therefore incomplete for its own purpose: a group-writable `versions/` with a 0755 binary passes, and the attacker it names can still swap the binary. The krun job root applies the directory-level rule (refuse if "another user's, or not mode 0700").
**Fix:** After step 2, `symlink_metadata` the canonical versions directory and the link's parent (`home.join(".local/bin")`) and refuse unless `uid == getuid()` and `mode & 0o022 == 0` (new `LocateFailure::DirWritable` / `DirWrongOwner`). Replace `fs::canonicalize(&link)` with a single `read_link` and require the result to be a direct absolute or `../share/claude/versions/X.Y.Z`-relative path into the versions directory, so no intermediate hop is followed; or keep `canonicalize` but also check each ancestor of the target up to `$HOME`. Add locate tests for a group-writable versions directory and a hop through a world-writable directory.

## Info

### IN-01: Rationale cap is characters in the schema and bytes in the parser

**File:** `crates/minos-propose/src/schema.rs:90-94`, `crates/minos-propose/src/proposal.rs:331-336`
**Issue:** The schema's `maxLength: 2000` is a JSON Schema character count; `check_rationale` enforces `rationale.len() > MAX_RATIONALE_BYTES` in bytes. A 2000-character rationale with any multi-byte characters passes the CLI's validator and is then dropped as `RationaleInvalid`, wasting an otherwise valid proposal. The `limits.rs` doc for `MAX_RATIONALE_BYTES` says "Also the output schema's `maxLength`", which conflates the two units.
**Fix:** Either count chars in `check_rationale` (`rationale.chars().count() > MAX_RATIONALE_CHARS`) with a separate byte ceiling of 4x for storage, or set the schema's `maxLength` to `MAX_RATIONALE_BYTES / 4` so every schema-valid rationale fits the byte cap. Update the doc comment to name the unit.

### IN-02: A failed directory removal in `run_step` masks the process error

**File:** `crates/minos-propose/src/claude_cli/preflight.rs:418-423`
**Issue:** `process::run` is mapped to `PreflightError::Process` and stored in `output`, then `dir.close().map_err(|_| unusable())?` returns early. When both the run and the removal fail, the user sees `WorkRootUnusable` and never the `Process { step, cause }` that is the more useful diagnosis. `ClaudeCli::complete` handles the same pair with an explicit `finish` match; `run_step` does not.
**Fix:** Mirror `finish`: `match (dir.close(), output) { (Ok(()), output) => output, (Err(_), Err(process)) => Err(process), (Err(_), Ok(_)) => Err(unusable()) }`.

### IN-03: The compile-time prompt-size proof assumes a fence overhead the code does not bound

**File:** `crates/minos-propose/src/limits.rs:71-79`, `crates/minos-propose/src/prompt.rs:367-398`, `crates/minos-propose/src/prompt/history.rs:159-169`
**Issue:** The `const` assertion sums the four content caps plus `PROMPT_OVERHEAD_BYTES` (64 KiB) to prove a prompt fits `MAX_PROMPT_BYTES`. But `STEER_NOTES_MAX_BYTES` and `EXCERPTS_MAX_BYTES` cap sanitized text bytes only, not the number of blocks: each kept note or excerpt adds roughly 70 to 150 bytes of BEGIN/END lines and label regardless of its length, and an excerpt of zero bytes costs nothing against its cap. Thousands of one-byte notes, or several hundred unscored rows with empty excerpts, push the fence overhead past 64 KiB, and `Prompt::new` returns `PromptError::TooLarge`, which the loop treats as a non-retried configuration error. The 1 MiB ceiling leaves enough slack that this needs pathological input (notes are user-authored), so it is not a correctness bug today, but the assertion's premise is not enforced.
**Fix:** Cap block counts as well as bytes: keep at most N notes (for example 64) and skip excerpts whose text is empty, and state in the `PROMPT_OVERHEAD_BYTES` doc what per-block overhead and block counts it covers.

### IN-04: A successful completion is discarded when the call directory cannot be removed

**File:** `crates/minos-propose/src/claude_cli/mod.rs:172-182`
**Issue:** `finish` turns `(Err(removal), Ok(response))` into `InfraCause::WorkDir` carrying the usage. The tokens were spent and the structured output was valid, but the loop sees an infrastructure failure, counts it toward `max_infra_failures`, and makes another model call. The design intent ("reported, never ignored") is sound; the cost is one wasted call per occurrence and a misattributed streak entry. With `--tools ""` the CLI should write nothing into its cwd, so this should be rare.
**Fix:** Consider returning the `Ok(response)` and surfacing the leaked directory through a separate channel (a `ClaudeCli` counter the CLI's `host doctor` or a sweep at the next preflight reports and removes), as the krun backend does for leftover job directories, so a successful call is never retried for a cleanup failure.

---

_Reviewed: 2026-10-20_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
