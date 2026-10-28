---
phase: 08-cli-sample-project-and-docs
reviewed: 2026-10-28T00:00:00Z
depth: standard
files_reviewed: 69
files_reviewed_list:
  - crates/minos-backend-krun/src/host.rs
  - crates/minos-backend-krun/tests/vm.rs
  - crates/minos-cli/Cargo.toml
  - crates/minos-cli/src/address.rs
  - crates/minos-cli/src/adopt.rs
  - crates/minos-cli/src/check.rs
  - crates/minos-cli/src/init.rs
  - crates/minos-cli/src/jobs.rs
  - crates/minos-cli/src/lib.rs
  - crates/minos-cli/src/log.rs
  - crates/minos-cli/src/main.rs
  - crates/minos-cli/src/project.rs
  - crates/minos-cli/src/render.rs
  - crates/minos-cli/src/report.rs
  - crates/minos-cli/src/run.rs
  - crates/minos-cli/src/show.rs
  - crates/minos-cli/src/status.rs
  - crates/minos-cli/src/stop.rs
  - crates/minos-cli/tests/address.rs
  - crates/minos-cli/tests/check.rs
  - crates/minos-cli/tests/cli_binary.rs
  - crates/minos-cli/tests/docs_words.rs
  - crates/minos-cli/tests/fixtures/logparse_improved_parse.rs
  - crates/minos-cli/tests/init.rs
  - crates/minos-cli/tests/inspect.rs
  - crates/minos-cli/tests/patch_tools.rs
  - crates/minos-cli/tests/progress_golden.rs
  - crates/minos-cli/tests/project.rs
  - crates/minos-cli/tests/report_adopt.rs
  - crates/minos-cli/tests/run.rs
  - crates/minos-cli/tests/sample_manifest.rs
  - crates/minos-cli/tests/sample_vm.rs
  - crates/minos-cli/tests/stop.rs
  - crates/minos-core/src/limits.rs
  - crates/minos-core/src/money.rs
  - crates/minos-core/src/policy.rs
  - crates/minos-core/src/state.rs
  - crates/minos-core/tests/cli_seams.rs
  - crates/minos-core/tests/money.rs
  - crates/minos-loop/src/driver.rs
  - crates/minos-loop/src/history.rs
  - crates/minos-loop/src/inputs.rs
  - crates/minos-loop/tests/blocks.rs
  - crates/minos-loop/tests/driver/support.rs
  - crates/minos-sandbox/src/untrusted.rs
  - crates/minos-source/src/lib.rs
  - crates/minos-source/src/read.rs
  - crates/minos-source/tests/read.rs
  - crates/minos-store/src/file/ledger.rs
  - crates/minos-store/src/lib.rs
  - crates/minos-store/src/memory.rs
  - crates/minos-store/tests/contract.rs
  - examples/logparse/.gitignore
  - examples/logparse/Cargo.lock
  - examples/logparse/Cargo.toml
  - examples/logparse/data/examples.tsv
  - examples/logparse/data/input.log
  - examples/logparse/judge/.gitignore
  - examples/logparse/judge/answers.tsv
  - examples/logparse/judge/Cargo.lock
  - examples/logparse/judge/Cargo.toml
  - examples/logparse/judge/src/main.rs
  - examples/logparse/minos.Dockerfile
  - examples/logparse/minos.toml
  - examples/logparse/program.md
  - examples/logparse/README.md
  - examples/logparse/src/main.rs
  - examples/logparse/src/parse.rs
  - examples/logparse/src/record.rs
findings:
  critical: 0
  warning: 3
  info: 4
  total: 7
status: issues_found
---

# Phase 8: Code Review Report

**Reviewed:** 2026-10-28
**Depth:** standard
**Files Reviewed:** 69
**Status:** issues_found

## Summary

The review read every Phase 8 CLI module (`address`, `adopt`, `check`, `init`, `jobs`, `log`, `main`, `project`,
`render`, `report`, `run`, `show`, `status`, `stop`), the phase's diffs to core, loop, store, sandbox, source, and
the krun backend, the new `minos-source::read` module in full, the sample project (manifest, Dockerfile, candidate
and judge crates, data files, README, program), and the test targets, with the three invariants weighed first.

What the review set out to disprove, and could not:

- **Sandbox, blob, and model bytes reach the terminal only through `SafeText` or `LogExcerpt`.** Every Phase 8
  command writes through `render::write_line`; rationales and stage logs go through `LogExcerpt::from_bytes` under
  a cap and are indented; the diff shown by `show` and `adopt` is the canonical text re-read through
  `CumulativeDiff::recorded`; `main` wraps every error and warning in `SafeText` before `eprintln!`. `Reason`,
  `StopReason`, `ModelId`, `MetricName`, and `RelPath` are validated types, so their `Display` carries no free text.
- **`adopt` cannot write a patch without the matching acknowledged hash, overwrite a file, or follow a symlink.**
  `report::build` is rebuilt at the moment of `adopt`, `--reviewed` must equal its `ReportHash` byte for byte
  before any append, the patch bytes are `CumulativeDiff::recorded` output (which refuses a blob that does not hash
  to the experiment), and the file is opened with `create_new` (`O_EXCL`), which refuses a dangling or live symlink.
  `tests/report_adopt.rs` covers the mismatch, the stale hash, the existing file, the symlink, and `--out`.
- **Read-only commands create no ledger state.** `report`, `log`, and `show` open an existing `.minos` through
  `open_runs` and never append; `status`'s only side effect is the documented empty `driver.lock`; `stop` appends
  only when the driver lock is held; `Ledger::runs` lists without creating.
- **No credential or environment variable reaches the `claude` subprocess from the CLI.** Phase 8 only passes
  `ParentEnv::from_vars(env::vars_os())` to `ClaudeCli::preflight`, the Phase 6 refusal gate; the CLI builds no
  subprocess environment of its own.
- **File handling is TOCTOU-safe where it matters.** `init` and `adopt` use `create_new` and remove only what the
  same call created; `project::prepare` reads the surface and program back from the packed tarball, so a file
  edited after the pack cannot reach the prompt or the base; `read_files` is a bounded in-memory ustar reader
  (block alignment, entry count, per-entry and total caps checked from the header before data is read, `RelPath`
  names, no duplicates, end marker).
- **Hostile input is bounded.** Every blob read names a cap; the ledger read is capped by the store; `read_capped`
  bounds the manifest, guest, and price table; `utc_text` and the percent helpers cannot panic (saturating or
  `is_finite`-filtered).

What remains are three warnings about robustness and operator diagnosis and four informational items. None weakens
isolation, judge integrity, or the review gate.

## Warnings

### WR-01: A stray regular file under `.minos/runs/` (for example macOS's `.DS_Store`) makes every run-listing command fail, and the error names no entry

**File:** `crates/minos-store/src/file/ledger.rs:200-212` (`run_entry`) and `crates/minos-store/src/file/ledger.rs:307-321` (`runs`)
**Issue:** `FileStore::runs` calls `run_entry` on every directory entry and returns `StoreError::Unsafe { what: "run directory" }` for anything that is not a directory named by 64 hex characters. The contract test `a_regular_file_under_runs_is_refused` shows this is deliberate for a planted file named like a run, but the same arm fires for a regular file that can never be a run. On the v1 target platform, Finder writes `.DS_Store` into any directory the user browses, and `.minos` is the user's own 0700 directory, so one glance at `.minos/runs` in Finder leaves every subsequent `minos check`, `run`, `stop`, `status`, `log`, `show`, `report`, and `adopt` failing with `the store in .minos could not be read: ... run directory`. The message carries no entry name, so the user cannot tell which of the entries is at fault or that deleting one file fixes it. Fail-closed is the right default for a symlink or a misnamed directory (the tampering shapes); a regular file whose name is not a `RunId` is neither a run nor a path a reader could follow.
**Fix:** Keep refusing symlinks and misnamed directories, skip a regular file whose name does not parse as a `RunId`, and put the entry name (sanitized) into the refusal so a human can act on it:

```rust
fn run_entry(path: &Path, name: &OsStr) -> Result<Option<RunId>, StoreError> {
    let meta = fs::symlink_metadata(path).map_err(io_error("inspecting a run directory"))?;
    let parsed = name.to_str().and_then(|text| RunId::from_str(text).ok());
    match (meta.file_type().is_dir(), parsed) {
        (true, Some(run)) => Ok(Some(run)),
        // A regular file that no RunId names cannot be a run and is never opened: a Finder artifact, for
        // example. A symlink or a misnamed directory is still refused.
        (false, None) if meta.file_type().is_file() => Ok(None),
        _ => Err(StoreError::UnsafeEntry {
            what: "run directory",
            entry: SafeText::inline(&name.to_string_lossy()),
        }),
    }
}
```

and in `runs`, `let Some(run) = Self::run_entry(..)? else { continue };`. Add a contract case for a `.DS_Store`
beside a real run (listed, with the run still present) and adjust `a_regular_file_under_runs_is_refused` to keep
refusing a regular file that *is* named like a run.

### WR-02: With no `--run`, `log`, `show`, `report`, `adopt`, and `stop` silently fall back to an older run when the latest ledger is unreadable

**File:** `crates/minos-cli/src/address.rs:197-215` (`resolve_run`); callers `crates/minos-cli/src/log.rs:115`, `crates/minos-cli/src/show.rs:111`, `crates/minos-cli/src/report.rs:127`, `crates/minos-cli/src/adopt.rs:142` (through `report::build`), `crates/minos-cli/src/stop.rs:105`
**Issue:** `resolve_run` with no prefix folds every run, skips one whose ledger cannot be read or folded, and returns the latest of the rest, recording the skipped runs in `Resolved::unreadable`. Only `status` (`status.rs:119-127`) prints that list. The other five commands drop it: after the newest run's `ledger.jsonl` is damaged (a flipped byte, a line over 1 MiB, a seq gap), `minos log` prints the previous run's events, `minos show <prefix>` and `minos report <prefix>` resolve the prefix inside the previous run, and `minos stop` probes the previous run, all with no indication that the run the user is looking at is not the latest. The user reads an older run's diff or report believing it is the current one. The review gate still holds (the hash must match the report of whatever run was resolved), but the silent substitution contradicts the module's own promise that "a prefix never silently picks the wrong run" for the no-prefix case.
**Fix:** Either fail closed, which matches the codebase's bias, or surface the list everywhere. Fail-closed version in `resolve_run`:

```rust
match latest {
    Some((_, run)) if unreadable.is_empty() => Ok(Resolved { run, unreadable }),
    Some(_) => Err(AddressError::LatestAmbiguous { runs: unreadable }), // "name the run with --run"
    None if unreadable.is_empty() => Err(AddressError::NoRuns),
    None => Err(AddressError::Unreadable { runs: unreadable }),
}
```

with `status` continuing to read through its own path so it can still report the damage. If the fallback is kept,
each of the five callers must write one `warning: run <id>: ledger could not be read` line per entry of
`resolved.unreadable()` before its own output, as `status` does, and a test in `tests/inspect.rs` and
`tests/report_adopt.rs` should plant a corrupt newest run and assert the warning.

### WR-03: `check` loses the per-trial diagnosis lines on the most common failure, an unscored baseline

**File:** `crates/minos-cli/src/check.rs:251` and `crates/minos-cli/src/check.rs:262`
**Issue:** `driver.calibrate().map_err(CheckError::Driver)?` returns before `report_calibration` runs. When no baseline trial scores (the build fails, the gate fails, the judge crashes), the driver returns `DriverError::BaselineUnscored { trials }` and `check` prints only `calibration failed: none of the 3 baseline trials scored; fix the baseline before proposing`. The trial outcomes are already in the ledger, and step 7 of the module docs (D-07) promises "each baseline trial's objective value or its outcome word", which is exactly the line (`trial 0: rejected:gate_failed`, `trial 0: crashed:build`) that tells the user what to fix. Today they have to find the run with `minos log` and read `judged ... outcome=` lines, or `minos show <baseline> --logs`, which `check` never names. The test `a_baseline_that_never_scores_is_baseline_unscored` asserts only the error, not the output.
**Fix:** Report from the fold before returning the error:

```rust
match driver.calibrate() {
    Ok(Calibrated::Stopped { reason }) => { say(out, &format!("calibration stopped: {}", stop_reason_name(reason)))?; refused = true; }
    Ok(Calibrated::Ready(_)) => {}
    Err(error @ DriverError::BaselineUnscored { .. }) => {
        // The trials are recorded; show each outcome before refusing, so the user sees what to fix.
        report_calibration(prepared, &run, out)?;
        say(out, &format!("show the baseline's logs with: minos show {} --logs", id_text(&ExperimentId::baseline(&prepared.base().id()).to_hex(), IdStyle::Short)))?;
        return Err(CheckError::Driver(error));
    }
    Err(error) => return Err(CheckError::Driver(error)),
}
```

and extend the test to assert the `trial 0: rejected:gate_failed` line is in `text`.

## Info

### IN-01: `adopt`'s printed apply commands do not quote the path

**File:** `crates/minos-cli/src/adopt.rs:190-196`
**Issue:** The default name `minos-<12 hex>.patch` is shell-safe, but with `--out` the full path the user gave is interpolated into `patch -p1 -i {file}` and `git apply ... {file}` unquoted. A path with a space or a shell metacharacter yields a command that fails or, with `$`, expands. The commands are printed text, never run by Minos, so this is a usability nit.
**Fix:** Print the `--out` form single-quoted (`'{file}'`, with any `'` in the path rendered as `'\''`), or print only the default-name form and say "pass the path you gave to --out".

### IN-02: `read_files` accepts any number of trailing zero blocks, while its docs say "two zero blocks and nothing else"

**File:** `crates/minos-source/src/read.rs:147-157`
**Issue:** After the last entry, `rest` is required to be at least 1024 bytes and all zero; a stream padded with three or more zero blocks passes. The packer writes exactly two and the blob is content-addressed, so nothing exploits this, but the reader is documented as strict and the test `data_after_the_end_marker_is_malformed` only covers non-zero trailing data.
**Fix:** `if rest.len() != END_MARKER { return Err(malformed("the archive does not end after its end marker")); }` and a test with 1536 trailing zero bytes.

### IN-03: The sample judge builds per-row structures over the whole 16 MiB output before the gate has decided

**File:** `examples/logparse/judge/src/main.rs:152-171`
**Issue:** `score` collects every line of the candidate's output into a `Vec<&str>` and a `BTreeMap` keyed by the row number before `rows_match` is checked. A candidate that writes 16 MiB of newlines produces about 16 million rows (hundreds of MB of fat pointers and map nodes) inside the judge VM's allowance, so a hostile candidate can push the judge into OOM. That is recorded against the candidate (`Crashed { stage: Judge }`), never retried, and never reaches policy as a score, so judge integrity holds; but the module doc claims "a broken candidate is scored, never a judge crash", and the sample is the template users copy.
**Fix:** Bound the rows before building anything: `let rows: Vec<&str> = text.split_terminator('\n').take(key.len().saturating_add(1)).collect();` (one more than the key, so `rows.len() == key.len()` still detects an over-long output), and only build `by_number` from those.

### IN-04: `check` still launches the baseline VMs after the provider preflight refused

**File:** `crates/minos-cli/src/check.rs:201-259`; `crates/minos-cli/src/main.rs:639-662`
**Issue:** `report_facts` records the refusal and `check` goes on to `Driver::start` and `calibrate`, booting `[judge] trials` candidate and judge VMs for a project whose `minos run` would refuse to start until the CLI is logged in. The exit is 1 either way, so the diagnostic run is time the user spends before fixing the provider. This follows D-06 ("prints each refusal") and D-09 (exit non-zero only for a refusal), so it is a design choice rather than a defect; the calibration data is still useful on its own.
**Fix:** Optional: say so in one line before the baseline block (`provider refused; the baseline is still calibrated so the noise floor is known`), or reuse-only when refused (`reusable_run` still runs, a new run is skipped with a line naming the refusal). No change is required for correctness.

---

_Reviewed: 2026-10-28_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
