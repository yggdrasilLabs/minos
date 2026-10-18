---
phase: 05-guest-krun-backend-and-environment-build
plan: 10
subsystem: minos-guest
tags: [minos-guest, unpack, strict-apply, output-tar, nofollow, run-job, d-05, d-07, d-08, guest-02, guest-03, guest-04]
status: complete
requires:
  - "05-08: JobSource/DiskJobSource, OutputSink/ChunkSink, stage.rs (StageLimits::for_stage, SCRATCH_MOUNT and the scratch directory constants), test_fixture.rs"
  - "05-04: GuestJob (base, diff, writable, exp, stages), GuestStatus::new, ApplyStatus, StageEnd, OutputStatus, SetupStep, STATUS_PATH, MAX_JUDGE_DATA_BYTES, write_input_disk"
  - "minos-core: Diff::parse, diff::apply::{Tree, apply}, ApplyError::reason, ExperimentId::of, the output caps"
provides:
  - "minos_guest::nofollow: NOFOLLOW_FLAGS (O_NOFOLLOW | O_NONBLOCK), open_nofollow, open_nofollow_rewrite, same_file"
  - "minos_guest::workspace: UnpackCaps (BASE, JUDGE, CANDIDATE_OUT; getters), unpack_tar, UnpackReport, UnpackError, apply_diff, ApplyOutcome { Applied, Failed { path, hunk } }, ApplySetupError"
  - "minos_guest::pack: BUILD_LOG, RUN_LOG, JUDGE_LOG, SCORECARD, OUT_PREFIX, SCORECARD_FILE, LOG_CAP_BYTES = 262_144, LOG_TRUNCATED_LINE, PID1_ENTRIES, MAX_OUT_FILES, MAX_OUT_DIRS, log_path, cap_log, out_budget, walk_out, read_scorecard, build_output, PackError"
  - "minos_guest::run: SRC, OUT, HOME, TMP, JUDGE, CANDIDATE, StageCommand, StageRun, StageExecutor, Handover, JobError, JobEnd { status, sent, error }, run_job"
  - "crates/minos-guest/clippy.toml: tar unpack method bans and HashMap/HashSet type bans, each proven to fire"
affects:
  - "05-12: the Linux entry reads the input first (to unlink /dev/vdb and size the tmpfs), builds ChunkSink::new(VsockConnector, token), then calls run_job with a source that returns the already-read input, the vsock sink, a trampoline StageExecutor, and an lchown Handover; root is /scratch"
  - "05-12: the bind mounts of /scratch/tmp on /tmp and /scratch/out on /out go in the Handover step, which run_job calls after creating the directories and before the first stage"
  - "05-17: the VM tests read .minos/status.json, the logs, out/**, and .minos/scorecard.json back through minos-sandbox's exits, which checks that pack.rs's restated layout matches untrusted.rs"
tech-stack:
  added:
    - "tar (workspace pin 0.4.46, default features off) as a minos-guest dependency"
    - "tempfile (workspace pin 3.27.0) as a minos-guest dev-dependency"
    - "rustix moved from the Linux-only target table to [dependencies]; Cargo.lock gains only edges, no new package"
  patterns:
    - "Unpack in raw tar mode, writing each entry with create_new and O_NOFOLLOW; never Archive::unpack"
    - "Walk a candidate-written tree with symlink_metadata, a one-link check, an O_NOFOLLOW open, and a device and inode match"
    - "One orchestration function generic over four traits, tested end to end with a real ChunkSink and core's Reassembler"
key-files:
  created:
    - crates/minos-guest/clippy.toml
    - crates/minos-guest/src/nofollow.rs
    - crates/minos-guest/src/workspace.rs
    - crates/minos-guest/src/pack.rs
    - crates/minos-guest/src/run.rs
  modified:
    - crates/minos-guest/Cargo.toml
    - crates/minos-guest/src/lib.rs
    - crates/minos-guest/src/test_fixture.rs
    - Cargo.lock
decisions:
  - "JobEnd carries status, sent, and error: Option<JobError>, where JobError::Source holds the source error. That is the plan's 'equivalent closed shape': it also names unpack, apply, handover, status, and pack failures"
  - "A source failure that kept the token sends a status with role candidate, apply baseline, setup failed {input}, no stages, and output not_packed, because the job may not have parsed. This is the form core's own golden test uses. The host reads a failed setup as an infrastructure failure and consults neither field"
  - "Every setup failure (input, mount, unpack, apply, output) sends a status with no stages and output not_packed. Before the diff is applied, a candidate's apply field is baseline"
  - "A diff that does not apply skips the hand-over as well as every stage, since no command runs"
  - "A judge gets no src directory. Its judge and candidate directories stay root-owned (0755 directories, 0644 or 0755 files), and only out, home, and tmp are handed over"
  - "The /out walk keeps PID1_ENTRIES (3) entries free, so the whole tar stays within the host's 1024-entry cap. Its content budget is the content cap minus MAX_STATUS_BYTES and the capped logs (out_budget)"
  - "A judge's OutputStatus::Packed counts the scorecard: 1 file and its bytes, or 0 and 0 when there is no scorecard"
  - "build_output refuses a scorecard for a candidate, out entries for a judge, an out entry that is not under out/, and a log for a stage the role does not run"
  - "open_nofollow also passes O_NONBLOCK, so a FIFO swapped in after the check cannot block PID 1 on the open"
metrics:
  duration: "about 16 minutes"
  completed: "2026-10-18"
  tasks: 3
  files: 9
---

# Phase 5 Plan 10: Guest workspace, link-free output packer, and run_job Summary

The guest's host-compilable half is complete:

- It unpacks the base tarball itself, in raw mode: regular files only, with `RelPath` names, created new with mode 0644 or 0755, under entry, per-file, and total caps.
- It applies the canonical diff with minos-core's one strict applier, after `Diff::parse` against the job's writable set and a check that `ExperimentId::of(base, diff)` equals the job's experiment.
- It packs a role-shaped output tar from `/out` and never follows a link.
- `run_job` orders the work for both roles through `JobSource`, `OutputSink`, `StageExecutor`, and `Handover`.

What remains for 05-12 is the Linux syscalls behind those four traits.

## Tasks

| Task | Name | Commits | Files |
| ---- | ---- | ------- | ----- |
| 1 | Unpack the base tree and apply the diff; guest clippy bans | 4bd4f87 (RED), 933700d (GREEN) | Cargo.toml, Cargo.lock, clippy.toml, nofollow.rs, workspace.rs, lib.rs, test_fixture.rs |
| 2 | Pack the output tar from /out without following links | 34d05f7 (RED), 0a63694 (GREEN) | pack.rs, lib.rs |
| 3 | run_job orchestration behind StageExecutor and Handover | 8581294 (RED), 4bb4429 (GREEN) | run.rs, lib.rs |

## What was built

**workspace.rs (GUEST-02)**
- `unpack_tar` iterates `Archive::entries().raw(true)` over the in-memory tarball. For each entry it checks, in order:
  - the entry cap;
  - a ustar header;
  - typeflag `0`;
  - a `RelPath` name;
  - mode exactly 0644 or 0755 (setuid and 0600 are refused);
  - the header size against the per-file cap, before any byte is read;
  - the running total.
- It then creates the parent directories one component at a time. Each existing ancestor must be a directory, checked with `symlink_metadata`. New directories get mode 0755.
- Each file is written with `create_new` and `O_NOFOLLOW` through `take(size)`, and its exact mode is set afterwards, so the umask cannot change it.
- The first refused entry stops the unpack. A duplicate or a file-versus-directory clash is `Collision`.
- The caps: `UnpackCaps::BASE` is the snapshot's (10,000 files, 16 MiB, 64 MiB). `JUDGE` uses `MAX_JUDGE_DATA_BYTES` per file and in total. `CANDIDATE_OUT` uses core's output caps.
- `apply_diff` runs these steps:
  - `Diff::parse(text, job.writable(), &Limits::standard())`; a failure is `ApplySetupError::Parse`;
  - the `ExperimentId` check; a failure is `Identity`;
  - it reads only the touched files, as one-link regular files, with a device and inode match after an `O_NOFOLLOW` open; a missing file is left out so the applier reports hunk 0;
  - it calls `minos_core::diff::apply::apply`;
  - on success it rewrites each touched file in place (O_NOFOLLOW, truncate, no create, mode kept);
  - on `ApplyError` it returns `ApplyOutcome::Failed { path, hunk }` from `reason()` and leaves every file unchanged.

**pack.rs (GUEST-04)**
- `walk_out` descends `/out` in sorted name order, up to `MAX_PATH_DEPTH`:
  - every entry is checked with `symlink_metadata`;
  - a directory is recursed into;
  - a file must have one link and a name that is both a `RelPath` and fits a ustar header; it is opened with `O_NOFOLLOW` and must match by device and inode;
  - anything else (a symlink, hard link, socket, FIFO, device, or a bad name) is `Invalid`.
- Caps: `MAX_OUT_FILES` (1021) files, `MAX_OUTPUT_ENTRY_BYTES` per file (checked from the metadata and again after a `take(cap + 1)` read), and the content budget. Going over any of them is `TooLarge`.
- On `Invalid` or `TooLarge`, no `out/` file is returned.
- `read_scorecard` applies the same checks to `/out/scorecard.json`, with a cap of `MAX_SCORECARD_BYTES`.
- `build_output` writes the tar:
  - headers built by hand: ustar, mtime, uid, and gid 0, empty owner names, mode 0644;
  - entries in path order, followed by two zero blocks;
  - each log cut to `LOG_CAP_BYTES` and ending with `[minos-guest: log truncated]`;
  - its own entry, per-file, and content cap checks.

**run.rs (GUEST-02, GUEST-03, GUEST-04; D-05, D-07, D-08)**
- `run_job` creates the per-run directories with `create_dir`, so an existing directory is a `Mount` setup failure.
- A candidate:
  - unpacks into `src` and drops the base buffer;
  - applies the diff and drops the diff text;
  - hands over `src`, `out`, `home`, and `tmp`;
  - runs build, then run, with cwd `src`, `StageLimits::for_stage`, and a deadline equal to the stage budget;
  - stops at the first stage that does not exit 0 and marks the rest `Skipped`;
  - walks `out` and sends.
- A judge:
  - unpacks its data into `judge` and the candidate output into `candidate`, dropping each buffer;
  - hands over `out`, `home`, and `tmp`;
  - runs the judge stage in `judge`;
  - packs the scorecard.
- If `build_output` fails, `run_job` sends a fallback status with `setup: failed {output}` that keeps the stage reports.
- `run_dirs_match_stage_paths` pins `SCRATCH_MOUNT` joined with each directory name to stage.rs's `SRC_DIR`, `SCRATCH_OUT_DIR`, `HOME_DIR`, `SCRATCH_TMP_DIR`, `JUDGE_DIR`, and `CANDIDATE_DIR`.

## Clippy bans fire

A temporary `ban_probe` function, appended to workspace.rs and then removed (the file was restored from a copy, `grep -c ban_probe` printed 0), produced:

```
error: use of a disallowed method `tar::Archive::unpack`
   --> crates/minos-guest/src/workspace.rs:655:13
error: use of a disallowed method `tar::Entry::unpack`
   --> crates/minos-guest/src/workspace.rs:659:15
error: use of a disallowed method `tar::Entry::unpack_in`
   --> crates/minos-guest/src/workspace.rs:660:15
error: use of a disallowed type `std::collections::HashMap`
   --> crates/minos-guest/src/workspace.rs:662:48
```

## Verification

- `cargo test -p minos-guest`: 76 unit tests pass. That is 10 workspace, 3 nofollow, 8 pack, and 11 run tests, plus the earlier 44.
- `cargo clippy -p minos-guest --all-targets -- -D warnings` and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: clean.
- Workspace gates all pass:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `env -u INSTA_UPDATE cargo test --workspace` (64 suites ok);
  - `cargo deny check` (advisories, bans, licenses, sources ok);
  - `cargo doc --workspace --no-deps` (0 warnings).
- C-01: no MISSING line. C-02: prints nothing.
- Acceptance greps: each listed signature, constant, and dev-dependency line prints one line.

## Requirements

GUEST-02, GUEST-03, and GUEST-04 are not marked complete. This plan delivers their host-compilable logic. They hold end to end only once the Linux entry exists (05-12: the trampoline executor, lchown handover, `/dev/vdb` source, and vsock sink) and the VM tests pass (05-17).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] `open_nofollow_rewrite` and `NOFOLLOW_FLAGS`**
- **Found during:** Task 1
- **Issue:** `apply_diff` writes the touched files back. A plain `fs::write` would follow a symlink and could create a file.
- **Fix:** Added `open_nofollow_rewrite` (write, truncate, no create, `O_NOFOLLOW`). `NOFOLLOW_FLAGS` is now public so `unpack_tar`'s `create_new` open uses the same flag. Both opens also pass `O_NONBLOCK`.
- **Commit:** 4bd4f87, 933700d

**2. [Rule 2 - Correctness] Entry reserve and a directory cap in the /out walk**
- **Found during:** Task 2
- **Issue:** The host's reader caps the whole tar at 1024 entries, and PID 1 adds up to 3 of its own. An unbounded number of empty directories, or of names in one directory, would also make the walk unbounded.
- **Fix:** Added `PID1_ENTRIES`, `MAX_OUT_FILES = 1021`, and `MAX_OUT_DIRS = 1024`. A directory listing longer than files plus directories stops the walk with `TooLarge`.
- **Commit:** 0a63694

**3. [Rule 2 - Correctness] Strict role checks in `build_output`**
- **Found during:** Task 2
- **Fix:** `build_output` refuses:
  - a scorecard for a candidate (`PackError::Role`);
  - `out/` entries for a judge;
  - an `out` entry not under `out/` (`PackError::OutPath`), so a forged `.minos/scorecard.json` in the list is refused;
  - a log for another role's stage (`PackError::Stage`);
  - an over-cap scorecard.
- **Commit:** 0a63694

**4. [Rule 3 - Blocking] The test fixture builds real tarballs and jobs**
- **Found during:** Task 1
- **Issue:** The 05-08 fixture's base is not a tar, so `run_job` and `apply_diff` could not unpack it.
- **Fix:** Added `PARSE_RS`, `PARSE_RS_AFTER`, `tar_of`, `candidate_job_for`, and `baseline_job_for` to `test_fixture.rs`, and made `DIFF` crate-visible.
- **Commit:** 4bd4f87

**5. [Rule 3 - Blocking] The token copy is dropped unbound**
- **Found during:** Task 3
- **Issue:** `drop(token)` is `clippy::drop_non_drop`, because `ChunkToken` has no `Drop`.
- **Fix:** The token part of `into_parts()` is bound to `_`. The sink keeps its own copy. `ChunkToken` does not zero its bytes; that is unchanged.
- **Commit:** 4bb4429

Tests beyond the behavior blocks:
- `unpack_refuses_duplicates_and_collisions`, `unpack_caps_are_core_caps`, `apply_refuses_a_symlinked_target`;
- `nofollow_same_file_compares_inode` and `nofollow_rewrite_never_creates`;
- `read_scorecard_refusals`;
- `run_refuses_existing_state`, `run_invalid_out_packs_no_out_files` (a symlink planted in `out` makes the output `Invalid`, and the run log is still packed), and `run_dirs_match_stage_paths`.

## TDD Gate Compliance

Each task has a `test(05-10)` RED commit, then a `feat(05-10)` GREEN commit. In each RED run the new tests failed against stub bodies: 9 of 10 workspace tests, 8 of 8 pack tests, and 10 of 11 run tests. The tests that passed in RED check constants only. No refactor commit was needed.

## Known Stubs

None. The four traits have no Linux implementation yet. That is 05-12's scope by design.

## Threat Flags

None. The plan's threat register covers the files this plan touched: T-05-10-01 through T-05-10-05 are all mitigated as planned.

## Self-Check: PASSED

- FOUND: crates/minos-guest/clippy.toml, crates/minos-guest/src/nofollow.rs, crates/minos-guest/src/workspace.rs, crates/minos-guest/src/pack.rs, crates/minos-guest/src/run.rs
- FOUND commits: 4bd4f87, 933700d, 34d05f7, 0a63694, 8581294, 4bb4429
