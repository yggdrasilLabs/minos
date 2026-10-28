---
phase: 08-cli-sample-project-and-docs
verified: 2026-10-28T21:30:00Z
status: human_needed
score: 4/4 roadmap success criteria verified (GATE-06 criterion 5 excluded as cross-cutting); 13/13 requirement IDs accounted for
behavior_unverified: 0
overrides_applied: 0
gaps: []
deferred: []
human_verification:
  - test: "README quick start with the real binary and a real model call"
    expected: "From examples/logparse, minos env build, judge push judge, check, run --generations 3, status, report <exp>, adopt <exp> --reviewed <hash> each work as the README says, with the real single_shot haiku provider (not the scripted stand-in), approx. cost shows real token counts, and the patch applies with patch -p1 and with the printed git apply --directory form"
    why_human: "Spends a real model call on the user's own subscription; the recorded VM end to end used a scripted provider, and the live provider suite exercised the provider separately (08-EVIDENCE.md). Also confirms the README reads correctly to a person."
  - test: "Read README.md and SECURITY.md as a new user"
    expected: "Neither document claims more than the code does; the trust base, preflight list, recorded exceptions (KRUN_BOOT_START_NS, missing _minos user), and provider-terms paragraph read clearly"
    why_human: "The docs_words test only scans for banned words and required names; whether prose overclaims is a judgment call"
  - test: "Re-run the VM suite and live provider suite if the code changes before milestone close"
    expected: "cargo test -p minos-cli --test sample_vm -- --ignored ... passes; MINOS_CLAUDE_LIVE=1 live suite passes on the installed claude"
    why_human: "Needs a rebuild, signing, the musl guest, Docker, and the user's logged-in claude; verified here from the recorded evidence only (run 2026-10-28, claude 2.1.312)"
---

# Phase 8: CLI, Sample Project, and Docs Verification Report

**Phase Goal:** A user takes the shipped sample project from `minos check` through `run`, `report`, and `adopt` with every experiment in a VM, sees only sanitized output and measured cost, receives a patch file only after acknowledging the report, and reads docs that do not overclaim.
**Verified:** 2026-10-28
**Status:** human_needed (no gaps; three human items)
**Re-verification:** No, initial verification

The final tree differs from the gated commit `b93a1a2` only in planning files (`git diff --stat b93a1a2..HEAD` lists six `.planning/` files), so the orchestrator's `cargo test --workspace` (1839 passed, 0 failed, 28 ignored) applies to the code as it stands. I also ran `cargo test -p minos-cli` myself: every non-ignored target is green (lib 25, bin 11, address 7, check 13, cli_binary 8, docs_words 4, init 8, inspect 14, patch_tools 3, progress_golden 4, project 13, report_adopt 9, run 8, sample_manifest 4, stop 6, plus the earlier-phase targets), and `sample_vm` reports 2 ignored.

## Goal Achievement

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | On the shipped `examples/` `optimize` project, with a judge that scores harness measurements and candidate output files only, `check`, `run --generations N`, `report`, `adopt` complete end to end with every experiment in a VM | VERIFIED | `examples/logparse/` exists (`minos.toml` kind optimize, one writable `src/parse.rs`, judge crate in `judge/` under `[source] exclude`). `judge/src/main.rs` reads only the pushed key and `/scratch/candidate/out/fields.tsv` and writes the scorecard; its doc states it never reads a candidate-reported number. 08-EVIDENCE.md "Sample end to end on real VMs (08-13)" records `minos check`, `check` again (reused, no VM), `run --generations 2`, `report`, and `patch -p1` yielding the fixture byte for byte; the test asserted `backend_classes == {(Krun, Arm64)}`; a rerun with the final budgets passed (205.86 s). Host-side `sample_manifest` (4 tests) runs in the gate. |
| 2 | `minos init --kind <tune\|optimize\|explore>` scaffolds the manifest, program, and Dockerfile; `minos check` validates, judges the baseline several times with the trial index on each `Judged`, prints noise floor beside `min_improvement` and the route table with reasons, with actionable errors (CLI-01, CLI-05, JUDGE-02) | VERIFIED | `init_command` takes the three kinds; `tests/init.rs::init_scaffolds_four_valid_files_for_each_kind` (the four files are the three required plus `.gitignore`), refusal/no-overwrite tests, price table fail-closed. `check` (13 tests, 519-line `check.rs`) refuses naming `minos env build` / `minos judge push` (D-06, noted in REQUIREMENTS.md). VM evidence shows trials 0..2 with indexes 0, 1, 2, "noise floor: median ... spread 0 over 3 trials", "min_improvement: 5% (threshold ...)", "routes: 0 single_shot haiku: eligible". |
| 3 | `minos run` prints one sanitized progress line per experiment and resumes; `minos stop` records `StopRequested` and the driver exits at the next safe point; `status`/`log`/`show` render only through the sanitizer; cost only as `approx. cost: $X.XX` with token counts; no estimate/"exact" (CLI-06, CLI-07, CLI-10, COST-02) | VERIFIED | `run.rs`, `stop.rs`, `status.rs`, `log.rs`, `show.rs` present and wired in `main.rs` (`run_cmd`, `stop_cmd`, ...). `progress_golden` pins the line format (4), `run` (8 incl. resume via `--run`, refusals), `stop` (6: refused without a driver, fresh ordinal, live driver honours it), `inspect` (14 incl. hostile rationale and log bytes). Cost goes only through `minos_core::money::CostLine` (`render.rs::cost_line`; no other `$` formatting in the CLI sources). `main.rs` has a unit test that no help text says "estimate" or "exact"; VM output shows `g01 approx. cost: $0.01 (in 1000 tokens, out 2000 tokens)`. |
| 4 | `minos report <exp>` prints report and hash; `minos adopt <exp> --reviewed <hash>` refuses unless the hash matches, writes a canonical patch accepted by `git apply` and `patch -p1` (golden), never applies it; `SECURITY.md` states threat model, trust base, preflight without claiming ironclad isolation; README states the `claude` provider execs only the user's own install and shared/hosted use needs an API key or Bedrock (CLI-08, CLI-09, DOCS-03, DOCS-04) | VERIFIED | `report.rs` prints text, `report hash: <64 hex>`, and the adopt command, appends nothing. `adopt.rs` rebuilds the report with the same `report::build`, returns `HashMismatch` before any append or write, appends `ReportAcknowledged`, writes with `create_new` (refuses existing file or symlink, removes a partial file), and only prints the apply commands, never running them. `patch_tools` (3 tests, host `git apply` and `patch -p1`) and `report_adopt` (9) pass; the plain-`git apply`-in-subdirectory no-op is pinned and documented. `SECURITY.md` (162 lines) has Threat model, What Minos isolates, Trust base (Hypervisor.framework, msb_krun 0.1.41, libkrunfw 5.6.2, Seatbelt, rlimits, guest, builder, claude CLI), What the preflight checks (with explicit non-checks), and Known limits (KRUN_BOOT_START_NS, missing `_minos` user, etc.); "zero-day" target stated as a goal, "not a guarantee". README "The model provider" says own unmodified install under own login, never reads/stores/forwards a credential, and that shared or hosted use must go through an API key or Bedrock. `docs_words` (4) enforces no `ironclad`, no whole word `exact`, no `cannot be escaped`, and the required names, over README, SECURITY, and the sample README. |
| 5 | `cargo clean` after verification (GATE-06) | N/A | Cross-cutting by roadmap decision; not reported as a gap. The orchestrator runs it last. |

**Score:** 4/4 verifiable roadmap criteria.

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/minos-cli/src/{init,check,run,stop,status,log,show,report,adopt,address,render,project}.rs` | VERIFIED | Substantive (e.g. adopt 238, check 519, run 616 lines) and wired through `main.rs` command tree and `lib.rs` |
| `examples/logparse/` (manifest, program.md, Dockerfile, candidate crate, judge crate, data) | VERIFIED | Own `Cargo.lock`s; excluded from the workspace; sample gate in 08-EVIDENCE.md "The sample gate" |
| `crates/minos-cli/tests/*.rs` (14 new targets) | VERIFIED | All present and passing; `sample_vm` ignored by design |
| `README.md`, `SECURITY.md`, `LICENSE-APACHE`, `LICENSE-MIT`, `examples/logparse/README.md` | VERIFIED | All exist; README links resolve to existing files |
| `08-EVIDENCE.md` (VM runs, live rerun, final gate) | VERIFIED | Sections present; live rerun by the user on claude 2.1.312 recorded |

### Key Link Verification

| From | To | Via | Status |
|------|----|-----|--------|
| `main.rs` | each command module | `Some(("run", run)) => run_cmd(run)` etc. for init, check, run, stop, status, log, show, report, adopt | WIRED |
| `adopt` | `report::build` | same builder as `report`, so the hash a user read is the hash checked | WIRED |
| `adopt` | ledger | `Event::ReportAcknowledged` appended before the patch file is created | WIRED |
| cost output | `CostLine` / `ApproxCost` | `render::cost_line`, `run.rs`, `check.rs` | WIRED |
| printed sandbox/model strings | `SafeText` / `write_line` | `render.rs`; `inspect` hostile-byte tests | WIRED |
| `RunnerJobs<KrunBackend>` | loop `JobRunner` seam | `krun_jobs` in `main.rs`; real-VM run in evidence | WIRED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| minos-cli non-ignored suite | `cargo test -p minos-cli` | all targets ok, 0 failed | PASS |
| Workspace regression | orchestrator: `cargo test --workspace` | 1839 passed, 0 failed, 28 ignored | PASS (cited) |
| C-02 `#[allow(` grep over `crates` and `examples` | `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'` | no output | PASS |
| Debt markers (TBD/FIXME/XXX/TODO/HACK/todo!) in minos-cli, examples, README, SECURITY | grep | no output | PASS |
| `.unwrap()` / `.expect(` in minos-cli and examples | grep | no output | PASS |
| `MINOS_VMM`/`MINOS_GUEST`/`MINOS_KRUNFW`/`MINOS_CLAUDE_LIVE` in `crates/*/src` | grep | no output | PASS |
| Real-VM end to end | recorded run (not rerun, needs rebuild, signing, Docker) | `2 passed` (408.44 s), then `1 passed` (205.86 s) | PASS (recorded) |

### Requirements Coverage

All 13 phase IDs appear in PLAN frontmatter and in REQUIREMENTS.md as Complete; none orphaned.

| Requirement | Plans | Status | Evidence |
|-------------|-------|--------|----------|
| CLI-01 | 08-11, 08-12, 08-15 | SATISFIED | `init` + `tests/init.rs`, `cli_binary` |
| CLI-05 | 08-01, 02, 03, 06, 07, 12, 15 | SATISFIED | `check.rs`, `tests/check.rs`, `project.rs`; D-06 (check builds nothing, names `env build`) is recorded in REQUIREMENTS.md |
| CLI-06 | 08-01, 06, 08, 12, 15 | SATISFIED | `run.rs`, `progress_golden`, `run` tests |
| CLI-07 | 08-02, 06, 09, 12, 15 | SATISFIED | `status/log/show`, `inspect`, `address` |
| CLI-08 | 08-03, 10, 12, 15 | SATISFIED | `report.rs`, `report_adopt` |
| CLI-09 | 08-10, 12, 15 | SATISFIED | `adopt.rs`, `report_adopt`, `patch_tools` |
| CLI-10 | 08-02, 06, 11, 12, 15 | SATISFIED | `stop.rs`, `tests/stop.rs` |
| JUDGE-02 | 08-07, 13, 15 | SATISFIED (D-07 partial, accepted) | Trial index on `Judged` 0..2 in evidence; noise floor printed beside `min_improvement`. D-07 (baseline count from measured variance) was met partially and accepted by the user on 2026-10-28; the STATE.md blocker is restated open by design, not a gap |
| COST-02 | 08-01, 08, 09, 10, 11, 12, 14, 15 | SATISFIED | single `CostLine`/`ApproxCost` formatter; help-text and docs word scans |
| DOCS-01 | 08-04, 05, 13, 15 | SATISFIED | `examples/logparse`, judge reads only the key and the output file |
| DOCS-02 | 08-13, 15 | SATISFIED | recorded real-VM runs |
| DOCS-03 | 08-14, 15 | SATISFIED | `SECURITY.md`, `docs_words` |
| DOCS-04 | 08-13, 14, 15 | SATISFIED | README "The model provider", live rerun, `docs_words` |

GATE-06 is cross-cutting and intentionally left unchecked.

### Anti-Patterns Found

None blocking. No `#[allow(`, no debt markers, no `unwrap`/`expect`, no stubs found in the Phase 8 files read. Note (info): the recorded VM end to end used a scripted provider in place of the model, which the README states plainly ("a scripted stand-in replaced the model, so its token counts are not real"), so the docs do not overclaim.

### Gaps Summary

No gaps. Every roadmap criterion is backed by code I read and by tests I ran, plus recorded VM and live-provider evidence for the parts that need a signed helper, Docker, and a logged-in `claude`. Remaining items are human confirmations only: the real-model quick start, a prose read of the two public documents, and rerunning the VM and live suites if code changes before the milestone closes.

---

_Verified: 2026-10-28_
_Verifier: Claude (gsd-verifier)_
