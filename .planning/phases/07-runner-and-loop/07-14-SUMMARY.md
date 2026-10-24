---
phase: 07-runner-and-loop
plan: 14
subsystem: minos-cli (runner composition), minos-guest (probe fixture)
status: complete
tags: [cli, runner, loop, vm, krun, LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04]
requires:
  - phase: 07-07
    provides: "minos_loop::{JobRunner, JobFailure}"
  - phase: 07-08
    provides: "minos_source::snapshot(.., judge_files) and the judge-overlap refusal"
  - phase: 07-11
    provides: "minos_cli::judge_push, minos_cli::lock_file::read_lock"
  - phase: 07-12
    provides: "minos_runner::Runner::{new, run, check}, RunnerError::{is_infra, what}"
provides:
  - "minos_cli::jobs::RunnerJobs<B: Backend> (new, runner, impl JobRunner) over Runner<B, FileStore>"
  - "minos_cli::jobs::judge_inputs(lock, manifest) -> (JudgeData, JudgeVersion), LockMismatch::NoJudge on a missing label"
  - "minos-probe scorecard <gate> <score> (test fixture only): probe_ok and probe_score"
  - "Ignored VM test target crates/minos-cli/tests/runner_vm.rs (3 ignored VM tests, 1 gate test)"
  - "CLAUDE.md Testing and Commands entries for runner_vm"
  - "07-EVIDENCE.md ## Runner on real VMs (07-14): 3 passed on this Mac"
affects: [07-15, 07-16, 07-17, 08]
tech-stack:
  added: []
  patterns:
    - "The CLI is the one place the runner and the loop meet; errors cross by RunnerError::is_infra alone, with fixed text"
    - "VM tests that need the krun backend and the store together live in minos-cli and copy the krun harness's setup text and rootfs helper"
key-files:
  created:
    - crates/minos-cli/src/jobs.rs
    - crates/minos-cli/tests/runner_vm.rs
  modified:
    - crates/minos-cli/src/lib.rs
    - crates/minos-guest/src/bin/minos-probe.rs
    - CLAUDE.md
    - .planning/phases/07-runner-and-loop/07-EVIDENCE.md
decisions:
  - "RunnerJobs maps RunnerError onto JobFailure only through is_infra and what, so a new runner cause can never cross the seam with the wrong blame"
  - "The VM test builds its backend with KrunBackend::new (the production job root and its sweep), not with_job_root, so it exercises the composition minos run will use"
  - "The VM test's project lives in a fresh directory under /private/tmp, as the krun harness's does, with judge data in a sibling directory outside the project"
  - "runner_vm.rs adds one non-ignored test, fixture_manifests_parse, so a fixture manifest mistake fails the gate rather than a VM run"
  - "The probe's gated scorecard form returns its own ExitCode, so an unparsable argument is a usage error (exit 2) and the bare scorecard form and SCORECARD_JSON stay byte-identical"
metrics:
  duration: "about 10 minutes (2026-10-24T13:35:45Z to 13:45:18Z)"
  completed: 2026-10-24
  tasks: 3
  files: 6
---

# Phase 7 Plan 14: The production runner on real VMs Summary

`RunnerJobs<B>` adapts the production `Runner<B, FileStore>` to the loop's `JobRunner` seam in `minos-cli`, and `judge_inputs` turns the lock's `[[judge]]` entry into the runner's judge data and judge version. An ignored VM test drives that exact composition, with `KrunBackend::new` over `FileBlobSource`, judge data from `judge_push`, and a base from `snapshot`. It passed 3 of 3 on this Mac: a baseline scores, a candidate's `out/` files reach the judge VM, and a failed gate is `Rejected(GateFailed)`. Every result records krun on arm64 and host-measured usage for both VMs.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | `RunnerJobs`, `judge_inputs`, the probe's gated scorecard | `bc5dd48` | src/jobs.rs, src/lib.rs, minos-probe.rs |
| 2 | The ignored VM test and its CLAUDE.md entries | `c215775` | tests/runner_vm.rs, CLAUDE.md |
| 3 | Run the VM test on this Mac and record it | `60f2f66` | 07-EVIDENCE.md |

## What was built

- **`jobs.rs`**:
  - `RunnerJobs<B: Backend>` with `new` and `runner`.
  - `impl<B: Backend> JobRunner for RunnerJobs<B>`. Both `run` and `check` delegate to the runner and map errors through a private `failure`: `is_infra` gives `JobFailure::Infra { what }`, and anything else gives `Config { what }`. The `what` comes from `RunnerError::what`.
  - `judge_inputs` goes through `Lock::check_judge(manifest.judge().label())`.
  - Four unit tests: both directions of `failure`, plus `judge_inputs` for a label that is present and one that is missing.
- **`minos-probe`**: a new arm, `["scorecard", gate, score] => return scorecard_with(gate, score)`.
  - Both arguments are parsed as `u32`; an unparsable one gives the usage exit.
  - It writes the two-metric card to `/out/scorecard.json`.
  - The usage line and module docs say the form is a test fixture only.
- **`tests/runner_vm.rs`**:
  - Copied from the krun harness: `SETUP`, `env_path` (an error carrying `SETUP`, never a skip), and the fixture rootfs.
  - Setup steps:
    - packs the root image with `pack_root_image` after `check_guest_elf` on the guest and the probe, then stores it with `put_reader` under `MAX_ROOT_IMAGE_BYTES`;
    - pushes judge data from a sibling directory with `judge_push`;
    - reads the lock with `read_lock` and checks that `judge_inputs` agrees with the push report's judge version;
    - packs the base with `snapshot(.., CleanPolicy::AllowDirty, &lock.judge_file_digests())`;
    - refuses a host with refusals or without the Seatbelt and Rlimits layers;
    - builds `Runner::new(..)` inside `RunnerJobs::new`.
  - Every VM test calls `check` before `run`.
  - Three ignored tests and one gate test (`fixture_manifests_parse`).
- **CLAUDE.md**:
  - The Testing VM bullet names `runner_vm.rs` and what it proves.
  - The Commands block and the VM-test paragraph name `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1`, with the same build, signing, musl build, and variables.

## Verification

- **VM run:** recorded verbatim in `07-EVIDENCE.md` `## Runner on real VMs (07-14)`.
  - `host doctor` exited 0 ("every check passed"), and `codesign --display` lists both entitlements.
  - `test result: ok. 3 passed; 0 failed`.
  - Candidate and judge usage satisfies `mem_mib_ms == vcpu_ms / 2 * 2048` in every row.
- **Gate:** every command exited 0.
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`: 1649 passed, 0 failed, 26 ignored. The known `minos-propose` flake did not occur.
  - `cargo deny check`: advisories, bans, licenses, and sources ok.
  - `cargo doc --workspace --no-deps`
  - guest musl clippy
  - C-01 printed 11 `ok` lines, and the C-02 grep printed nothing.
- **Acceptance greps:**
  - `impl<B: Backend> JobRunner for RunnerJobs<B>` and `pub fn judge_inputs`: 1 line each.
  - The probe arm: 1 line. `probe_score`: present.
  - The `SCORECARD_JSON` removal count: 0.
  - `cargo test -p minos-cli --test runner_vm` reports `1 passed; 3 ignored`.
  - The `#[ignore` count is 3.
  - `KrunBackend::new`, `RunnerJobs::new`, `judge_push(`, and `snapshot(` all appear.
  - `runner_vm` appears 3 times in CLAUDE.md.
- **Production code reads no `MINOS_*` variable:** in `minos-cli/src`, they appear only in a pre-existing doc comment in `locate.rs`.

## Deviations from Plan

**1. [Rule 2 - correctness] One non-ignored test in `runner_vm.rs`.** `fixture_manifests_parse` parses the fixture manifest for both run argvs and both gates. A manifest mistake then fails the gate instead of surfacing only on a VM host. The acceptance criterion `3 ignored` still holds. Commit `c215775`.

**2. Extra Commands line.** Besides the VM-test paragraph, the Commands code block also gets the full `runner_vm` command line with its three variables. Commit `c215775`.

The plan was otherwise executed as written. The VM test passed on its first run, so no Minos defect needed fixing, and no assertion was weakened.

## TDD Gate Compliance

Task 1 is `tdd="true"` in a `type: execute` plan. The unit tests and the adapter were written in the same change and committed as one `feat(07-14)` commit, as 07-08 did. I showed RED by hand: with `failure` inverted (`!error.is_infra()`), both mapping tests failed, and they passed again once restored. The probe form is Linux-only, so no host test can run it; the VM run checks it end to end (`probe_score` 7, and `probe_ok` 0 gives `GateFailed`).

## Known Stubs

None.

## Threat Flags

None. Every new surface is in the plan's threat register:
- **T-07-14-01:** `failure` maps by `is_infra` only, with unit tests in both directions.
- **T-07-14-02:** the production runner attaches judge data only through `secrets`, and the candidate test shows the judge consumed the re-packed input.
- **T-07-14-03:** only `tests/runner_vm.rs` reads `MINOS_*`.
- **T-07-14-04:** the probe form is documented as fixture-only.
- **T-07-14-05:** the verbatim run is recorded.

## Self-Check: PASSED

- FOUND: crates/minos-cli/src/jobs.rs, crates/minos-cli/tests/runner_vm.rs, .planning/phases/07-runner-and-loop/07-EVIDENCE.md (section present)
- FOUND: commits bc5dd48, c215775, 60f2f66
