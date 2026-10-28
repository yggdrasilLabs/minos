---
phase: 8
slug: cli-sample-project-and-docs
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-28
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (std harness), `proptest` 1.11.0, `insta` 1.49.0 (`update: no`), `trybuild` 1.0.122 |
| **Config file** | `.config/insta.yaml`; workspace `Cargo.toml` lints; `.cargo/config.toml` forbid backstop; the sample crates' own `[lints]` tables |
| **Quick run command** | `cargo test -p minos-cli` |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` plus the sample gate |
| **Sample gate (examples are excluded from the workspace)** | from the repository root: `cargo fmt --manifest-path examples/logparse/Cargo.toml --check`, the same for `examples/logparse/judge/Cargo.toml`, `cargo clippy --manifest-path <each> --all-targets --target-dir target/examples -- -D warnings`, `cargo test --manifest-path <each> --target-dir target/examples`; C-02 over `crates` and `examples` |
| **VM suite (ignored, by hand)** | `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1` after the build, the signing, the musl guest build, with Docker buildx running (08-13) |
| **Measured runtime** | final gate (08-EVIDENCE.md "Final gate"): `cargo test --workspace` 59 s after the one fix's recompile (44 s on the first pass), 1839 passed, 0 failed, 28 ignored over 104 result lines; the other five gate commands and the six sample gate commands 1 s or less each on a warm target; the whole sequence 65 s (20:40:16 to 20:41:21 UTC). The VM suite: 408.44 s for both tests and 205.86 s for the end-to-end test alone with the final budgets (08-13) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <crate touched>` plus `cargo clippy -p <crate touched> --all-targets -- -D warnings`
- **After every plan wave:** Run the full suite command plus the sample gate
- **Before `/gsd-verify-work`:** Full suite must be green, the ignored `sample_vm` run and the live provider rerun recorded in 08-EVIDENCE.md (done: 08-13)
- **Max feedback latency:** about 120 seconds for any per-task command (the slowest, `cargo test --workspace`, took 59 s in the final gate)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-T1 | 08-01 | 1 | COST-02, CLI-05 (D-08) | T-08-01-02, T-08-01-04 | One dollar formatter; token counts beside it; advisory arithmetic saturates | unit | `cargo test -p minos-core --test money --test cli_seams && cargo clippy -p minos-core --all-targets -- -D warnings` | ✅ created (`cli_seams`), `money` extended | ✅ green |
| 08-01-T2 | 08-01 | 1 | COST-02, CLI-06 (D-07) | T-08-01-01, T-08-01-03 | `run_cost` never prices an unpriced model as free; manifest read-back keeps the WR-03 mapping | unit + regression | `cargo test -p minos-core && cargo test -p minos-loop && cargo clippy -p minos-core -p minos-loop --all-targets -- -D warnings` | ✅ extend | ✅ green |
| 08-02-T1 | 08-02 | 1 | CLI-05, CLI-07, CLI-10 (D-05, D-10) | T-08-02-01, T-08-02-02, T-08-02-03 | Run listing refuses symlinks and foreign entries; a lock probe never creates a listed run; a run is listed by its first complete line | contract | `cargo test -p minos-store --test contract && cargo test -p minos-loop && cargo clippy -p minos-store -p minos-loop --all-targets -- -D warnings` | ✅ extend (`contract`) | ✅ green |
| 08-03-T1 | 08-03 | 1 | CLI-05, CLI-08 | T-08-03-01, T-08-03-02, T-08-03-03 | Strict in-memory tarball read-back under caps | integration | `cargo test -p minos-source --test read && cargo clippy -p minos-source --all-targets -- -D warnings && cargo doc -p minos-source --no-deps` | ✅ created (`read`) | ✅ green |
| 08-04-T1 | 08-04 | 1 | DOCS-01 (D-02, D-03) | T-08-04-02 | Std-only candidate with its own forbid lints; visible answers only for lines 1 to 24 | sample gate | `cargo fmt --manifest-path examples/logparse/Cargo.toml --check && cargo clippy --manifest-path examples/logparse/Cargo.toml --all-targets --target-dir target/examples -- -D warnings && cargo test --manifest-path examples/logparse/Cargo.toml --target-dir target/examples` | ✅ created (`examples/logparse/`) | ✅ green |
| 08-04-T2 | 08-04 | 1 | DOCS-01 (D-01, D-04) | T-08-04-01, T-08-04-03 | The judge scores only output files against the key and survives hostile output | sample gate | `cargo fmt --manifest-path examples/logparse/judge/Cargo.toml --check && cargo clippy --manifest-path examples/logparse/judge/Cargo.toml --all-targets --target-dir target/examples -- -D warnings && cargo test --manifest-path examples/logparse/judge/Cargo.toml --target-dir target/examples` | ✅ created (`examples/logparse/judge/`) | ✅ green |
| 08-05-T1 | 08-05 | 2 | DOCS-01 (D-04) | T-08-05-SC, T-08-05-02, T-08-05-03 | Digest-only image pin; the judge argv names no project path | structural | `cargo test -p minos-cli --test judge_push && test -f examples/logparse/minos.toml` | ✅ | ✅ green |
| 08-05-T2 | 08-05 | 2 | DOCS-01 | T-08-05-01 | No judge file in the snapshot; baseline imperfect, fixture perfect on the host | integration | `cargo test -p minos-cli --test sample_manifest && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`sample_manifest`) | ✅ green |
| 08-06-T1 | 08-06 | 2 | CLI-05, CLI-06 (D-06) | T-08-06-01, T-08-06-05, T-08-06-06 | Every refusal names the next command; nothing launches on a stale lock or a refused host | integration | `cargo test -p minos-cli --test project && cargo test -p minos-cli --lib && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`project`) | ✅ green |
| 08-06-T2 | 08-06 | 2 | CLI-07, CLI-10 (D-10, D-14) | T-08-06-02, T-08-06-03, T-08-06-04 | Every printed line through `SafeText`; prefixes resolve uniquely without side effects | unit + integration | `cargo test -p minos-cli --test address && cargo test -p minos-cli --lib && cargo clippy -p minos-cli --all-targets -- -D warnings && cargo doc -p minos-cli --no-deps` | ✅ created (`address`) | ✅ green |
| 08-07-T1 | 08-07 | 3 | CLI-05 (D-06, D-08, D-09) | T-08-07-03, T-08-07-05 | Advisories never refuse; provider refusal does | integration | `cargo test -p minos-cli --test check && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`check`) | ✅ green |
| 08-07-T2 | 08-07 | 3 | CLI-05, JUDGE-02 (D-05, D-07) | T-08-07-01, T-08-07-02, T-08-07-04 | Reuse only on an equal frame; same-second collision refused; cost only through `CostLine` | integration | `cargo test -p minos-cli --test check && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ | ✅ green |
| 08-08-T1 | 08-08 | 3 | CLI-06, COST-02 (D-14) | T-08-08-01 | The progress format is pinned and `SafeText`-stable | golden | `cargo test -p minos-cli --test progress_golden && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`progress_golden`) | ✅ green |
| 08-08-T2 | 08-08 | 3 | CLI-06 (D-07, D-14) | T-08-08-02, T-08-08-03, T-08-08-04, T-08-08-05 | Start, resume to an absolute target, refusals, and a mid-trial stop | integration | `cargo test -p minos-cli --test run --test progress_golden && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`run`) | ✅ green |
| 08-09-T1 | 08-09 | 3 | CLI-07, COST-02 (D-15, D-16) | T-08-09-04, T-08-09-05 | Health and liveness shown; `--json` equals the stored lines | integration | `cargo test -p minos-cli --test inspect && cargo clippy -p minos-cli --all-targets -- -D warnings && cargo deny check` | ✅ created (`inspect`) | ✅ green |
| 08-09-T2 | 08-09 | 3 | CLI-07 (D-17) | T-08-09-01, T-08-09-02, T-08-09-03 | Hostile rationale and log bytes rendered; logs only with `--logs` | integration | `cargo test -p minos-cli --test inspect && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ | ✅ green |
| 08-10-T1 | 08-10 | 3 | CLI-08, CLI-09 (D-11, D-12) | T-08-10-01, T-08-10-02, T-08-10-03, T-08-10-04, T-08-10-05 | Hash gate before any append or write; `create_new` patch; a failed write leaves no file; never applied | integration | `cargo test -p minos-cli --test report_adopt && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`report_adopt`) | ✅ green |
| 08-10-T2 | 08-10 | 3 | CLI-09 | T-08-10-06 | The patch applies byte for byte with `git apply` and `patch -p1`; the subdirectory no-op is pinned | host-tool golden | `cargo test -p minos-cli --test patch_tools && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`patch_tools`) | ✅ green |
| 08-11-T1 | 08-11 | 3 | CLI-10 (D-13) | T-08-11-01, T-08-11-02, T-08-11-03, T-08-11-07 | Refused without a driver; fresh ordinal; a live driver honors it | integration | `cargo test -p minos-cli --test stop && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`stop`) | ✅ green |
| 08-11-T2 | 08-11 | 3 | CLI-01 | T-08-11-04, T-08-11-05, T-08-11-06 | No overwrite; templates parse; the Dockerfile template is digest-only; price table fails closed | integration | `cargo test -p minos-cli --test init && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`init`) | ✅ green |
| 08-12-T1 | 08-12 | 4 | CLI-01, CLI-05 to CLI-10, COST-02 | T-08-12-02, T-08-12-05 | Every flag parses as decided; help has no banned word | unit | `cargo test -p minos-cli --bin minos && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ extend (`src/main.rs` unit tests) | ✅ green |
| 08-12-T2 | 08-12 | 4 | CLI-01, CLI-05, CLI-06, CLI-07, CLI-10 | T-08-12-01, T-08-12-03, T-08-12-04 | The binary's wiring works without a VM; preflight gates `run` | binary integration | `cargo test -p minos-cli --test cli_binary --bin minos && cargo clippy -p minos-cli --all-targets -- -D warnings && cargo doc -p minos-cli --no-deps` | ✅ created (`cli_binary`) | ✅ green |
| 08-13-T1 | 08-13 | 4 | DOCS-02, JUDGE-02 | T-08-13-03 | Ignored VM tests compile in the gate and launch nothing there | ignored VM | `cargo test -p minos-cli --test sample_vm && cargo clippy -p minos-cli --all-targets -- -D warnings` (final gate: `0 passed; 2 ignored`) | ✅ created (`sample_vm`) | ✅ green |
| 08-13-T2 | 08-13 | 4 | DOCS-02, JUDGE-02, DOCS-01 (D-02, D-04, D-07) | T-08-13-01, T-08-13-02, T-08-13-SC | The sample end to end on real VMs and the three measurements, recorded | manual VM run | `cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1` (after build, sign, musl build, Docker): `2 passed`, then `1 passed` with the final budgets | ✅ | ✅ green (D-07 met partially, accepted by the user) |
| 08-13-T3 | 08-13 | 4 | DOCS-04 | T-08-13-04 | The live provider suite rerun by the user on the installed CLI | checkpoint (manual) | `grep -n '## Live provider tests rerun (08-13)' .planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md` (`claude` 2.1.312: `3 passed`) | n/a | ✅ green |
| 08-14-T1 | 08-14 | 5 | DOCS-03 | T-08-14-01, T-08-14-02, T-08-14-04 | Trust base, preflight, and exceptions stated; candidate path corrected | doc check | `test -f SECURITY.md && grep -n 'KRUN_BOOT_START_NS' SECURITY.md && grep -n '/scratch/candidate/out' CLAUDE.md crates/minos-sandbox/src/untrusted.rs && cargo doc -p minos-sandbox --no-deps` | ✅ created (`SECURITY.md`) | ✅ green |
| 08-14-T2 | 08-14 | 5 | DOCS-03, DOCS-04, COST-02 | T-08-14-01, T-08-14-03 | No overclaim; provider terms and required names present | text scan | `cargo test -p minos-cli --test docs_words && cargo clippy -p minos-cli --all-targets -- -D warnings` | ✅ created (`docs_words`, `README.md`) | ✅ green |
| 08-15-T1 | 08-15 | 6 | all phase requirements | T-08-15-01, T-08-15-02, T-08-15-03, T-08-15-04, T-08-15-05 | Full gate, sample gate, C-01, C-02, LOOP-01 grep, and test-variable grep on the final code; the D-07 blocker restated, never closed | full gate | the Full suite command above plus the sample gate, C-01, C-02 over `crates` and `examples`, the LOOP-01 grep, and the test-variable grep over `crates/*/src` (08-EVIDENCE.md "Final gate") | ✅ | ✅ green |
| 08-15-T2 | 08-15 | 6 | GATE-06 | n/a | cargo clean last | structural | `test ! -d target/debug && test ! -d target/examples` (08-EVIDENCE.md "cargo clean": exit 0, 168360 files removed) | n/a | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Requirement to test mapping (from 08-RESEARCH.md "Phase Requirements → Test Map", with the target names as built):

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `init --kind` writes four files with `create_new`; manifests parse; the price template parses and is never zero | integration + binary | `cargo test -p minos-cli --test init --test cli_binary` | ✅ |
| CLI-05 | `check` refusals name `env build` or `judge push`; reuse versus `--fresh`; advisories never change the exit code; the D-08 arithmetic; the run listing and the snapshot read-back it uses | integration | `cargo test -p minos-cli --test project --test check`; `cargo test -p minos-core --test cli_seams`; `cargo test -p minos-store --test contract`; `cargo test -p minos-source --test read` | ✅ |
| CLI-06 | Progress lines pinned; start, resume via `--run`, refusals, stop | golden + integration | `cargo test -p minos-cli --test progress_golden --test run` | ✅ |
| CLI-07 | `status`, `log`, `log --json`, `show` render hostile bytes safely; logs opt-in; prefix addressing | integration | `cargo test -p minos-cli --test inspect --test address` | ✅ |
| CLI-08 | `report` prints the text and hash, appends nothing | integration | `cargo test -p minos-cli --test report_adopt` | ✅ |
| CLI-09 | `adopt` refuses a wrong or stale hash, appends the acknowledgment, `create_new`; both tools apply the patch in every setting | integration + host-tool golden | `cargo test -p minos-cli --test report_adopt --test patch_tools` | ✅ |
| CLI-10 | `stop` refuses without a driver; fresh ordinal; a live driver honors it | integration | `cargo test -p minos-cli --test stop` | ✅ |
| JUDGE-02 | Each trial's objective, the floor, `min_improvement`, the verdict; trial index on every `Judged` | integration + VM | `cargo test -p minos-cli --test check`; `cargo test -p minos-cli --test sample_vm -- --ignored …` (manual, 08-13: noise floor `spread 0 over 3 trials`; D-07 met partially) | ✅ |
| COST-02 | One formatter with token counts; no command output or help text says estimate or exact | unit + text scan | `cargo test -p minos-core --test money --test cli_seams`; `cargo test -p minos-cli --bin minos --test check --test run --test inspect --test docs_words` | ✅ |
| DOCS-01 | The sample manifest validates, packs without judge files, and scores correctly on the host | integration + sample gate | `cargo test -p minos-cli --test sample_manifest`; the sample gate | ✅ |
| DOCS-02 | `check`, `run`, `report`, `adopt` end to end on real VMs; krun on arm64; the patch applies | ignored VM e2e | `cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1` (manual, 08-13: `2 passed`, then `1 passed`) | ✅ |
| DOCS-03, DOCS-04 | SECURITY.md and README.md hold the trust base and the provider terms and no overclaim; the provider still ignores a hostile project on the installed CLI | text scan + live (manual) | `cargo test -p minos-cli --test docs_words`; `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored …` (manual, 08-13: `3 passed` on 2.1.312) | ✅ |

---

## Wave 0 Requirements

- [x] `crates/minos-core/tests/cli_seams.rs` (08-01). *Landed in 08-01 (`1cb16ae`, `e3b901b`): 9 tests in the final gate.*
- [x] `crates/minos-source/tests/read.rs` (08-03). *Landed in 08-03 (`36d18e6`, `004c9ca`): 14 tests.*
- [x] `examples/logparse/` crates with their own lints and locks (08-04). *Landed in 08-04 (`91d8bf7`, `ac42c2d`, `09bfa06`): the candidate's 8 and the judge's 12 tests run in the sample gate; the project files followed in 08-05.*
- [x] `crates/minos-cli/tests/{sample_manifest,project,address,check,run,progress_golden,inspect,report_adopt,patch_tools,stop,init,cli_binary,sample_vm,docs_words}.rs` (08-05 to 08-14); each carries its own fixture helpers, copied rather than shared, so no target holds unused helper code. *Landed: `sample_manifest` 4 (08-05), `project` 13 and `address` 7 (08-06), `check` 13 (08-07), `progress_golden` 4 and `run` 8 (08-08), `inspect` 14 (08-09), `report_adopt` 9 and `patch_tools` 3 (08-10), `stop` 6 and `init` 8 (08-11), `cli_binary` 8 (08-12), `sample_vm` 2 ignored (08-13), `docs_words` 4 (08-14).*
- [x] Contract cases for `Ledger::runs` in `crates/minos-store/tests/contract.rs` (08-02). *Landed in 08-02 (`3d42ba9`, `0aeeb91`): `contract` runs 59 cases over both stores.*
- [x] No framework install needed.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| The sample end to end on real VMs, and the D-02, D-04, D-07 measurements | DOCS-02, JUDGE-02, DOCS-01 | Needs the signed helper, the musl guest, libkrunfw, Hypervisor.framework, and Docker buildx; the gate launches no VM | 08-13 Task 2: build, sign, musl build, `host doctor`, then `cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1`; record in 08-EVIDENCE.md. **Done 2026-10-28 (08-13):** `2 passed` (408.44 s), then the end-to-end test alone with the final budgets `1 passed` (205.86 s); every `Judged` records krun on arm64 and the adopted patch reproduces the improved parser under `patch -p1`. D-07 met partially: the objective's spread is 0 by construction, the default of 3 trials stays a recorded assumption, VM wall time was recorded instead, and the user accepted that for the milestone; the STATE.md blocker stays open |
| The live `claude` provider suite on the installed CLI | DOCS-04 | Real model calls on the user's own subscription, run by the user | 08-13 Task 3: `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1`. **Done 2026-10-28 by the user:** `claude` 2.1.312, `3 passed`, exit 0; 08-EVIDENCE.md "Live provider tests rerun (08-13)" |
| The README walkthrough with the real binary | DOCS-02, DOCS-04 | Spends one real model call and checks the README reads correctly to a person | 08-14 Task 2 human check, harvested for end-of-phase UAT (`/gsd-verify-work 8`); not run by an agent |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant` set to true in frontmatter

**Approval:** approved 2026-10-28 (08-15 final gate; 08-15 Task 2, `cargo clean`, closes GATE-06)
