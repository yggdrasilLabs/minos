---
phase: 7
slug: runner-and-loop
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-10-22
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Derived from 07-RESEARCH.md "Validation Architecture".

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (toolchain 1.97.1), `proptest` 1.11.0, `insta` 1.49.0 (`.config/insta.yaml` `update: no`), `trybuild` 1.0.122 |
| **Config file** | `.config/insta.yaml`; snapshots in `crates/minos-core/tests/snapshots/` (extend) and the new crates' `tests/snapshots/` (Wave 0 creates) |
| **Quick run command** | `cargo test -p minos-loop` / `cargo test -p minos-runner` (per touched crate; `cargo test -p minos-core --test golden_wire --test lock` for wire tasks) |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, then the C-01 loop and the C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` (must print nothing) |
| **VM (manual, after build, sign, and the musl guest build)** | `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` |
| **Estimated runtime** | quick ~30 s per crate; full gate ~5 min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched crate>` (and `cargo test -p minos-core --test golden_wire --test lock` for wire or lock tasks)
- **After every plan wave:** Run the full suite command
- **Before `/gsd-verify-work`:** Full suite must be green, the C-01 and C-02 checks clean, the ignored VM test run once by hand after build, sign, and the musl guest build with the result recorded in an evidence file; then `cargo clean` (GATE-06)
- **Max feedback latency:** 300 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-01-T1 | 07-01 | 1 | LOOP-05 (D-09, D-10, D-18) | T-07-01-01, T-07-01-02, T-07-01-03, T-07-01-04, T-07-01-05 | Old ledger lines and goldens byte-identical; logs only as blob IDs; keys unchanged | golden | `cargo test -p minos-core --test golden_wire` | ❌ W0 (4 snapshots) | ⬜ pending |
| 07-01-T2 | 07-01 | 1 | LOOP-04, JUDGE-04 (D-07, D-11, D-19) | T-07-01-03 | New `RunState` accessors are order-independent | proptest + unit | `cargo test -p minos-core --test prop_fold && cargo test -p minos-store --test file_ledger` | ✅ extend | ⬜ pending |
| 07-01-T3 | 07-01 | 1 | LOOP-04 (Pitfall 12) | T-07-01-06 | `baseline_ready` agrees with `conclude` | proptest | `cargo test -p minos-core --test prop_policy` | ✅ extend | ⬜ pending |
| 07-02-T1 | 07-02 | 1 | LOOP-07 (D-05, D-13) | T-07-02-04, T-07-02-05 | `trials` range-checked and outside `JudgeVersion`; unpriced models named | unit | `cargo test -p minos-core --test manifest --test money --test golden_ids` | ✅ extend | ⬜ pending |
| 07-02-T2 | 07-02 | 1 | JUDGE-01 (D-02, D-04) | T-07-02-01, T-07-02-02, T-07-02-03 | One canonical `[[judge]]` wire form; malformed tables refused; cap before parse | golden + unit | `cargo test -p minos-core --test lock` | ❌ W0 (1 snapshot) | ⬜ pending |
| 07-03-T1 | 07-03 | 1 | LOOP-04 | T-07-03-04, T-07-03-SC | Lanes unchanged; no new package | structural | `cargo deny check && cargo build --workspace` | ✅ | ⬜ pending |
| 07-03-T2 | 07-03 | 1 | LOOP-03 (D-12) | T-07-03-01, T-07-03-02, T-07-03-03 | Clock, env, process, file, HashMap, and `ExperimentId::of` bans fire | clippy proof | `cargo clippy -p minos-runner -p minos-loop --all-targets -- -D warnings` | ❌ W0 (2 clippy.toml) | ⬜ pending |
| 07-04-T1 | 07-04 | 2 | LOOP-01 (D-17) | T-07-04-01 | User approves the exact exit signatures before code | checkpoint (manual) | `test -z "$(git status --porcelain crates/)"` | n/a | ⬜ pending |
| 07-04-T2 | 07-04 | 2 | LOOP-01, LOOP-02 (D-09, D-17) | T-07-04-01, T-07-04-02, T-07-04-03, T-07-04-04, T-07-04-05, T-07-04-06 | Only `out/` repacked canonically; tails capped; digest checked; no exit on the wrong role | unit + golden pin + trybuild | `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ✅ extend | ⬜ pending |
| 07-04-T3 | 07-04 | 2 | LOOP-01 (D-17) | T-07-04-01 | Design of record names the approved exits | doc check | `grep -n 'store_judge_input' CLAUDE.md` | ✅ | ⬜ pending |
| 07-05-T1 | 07-05 | 2 | LOOP-05 (D-08) | T-07-05-01 | A conflicting append is a non-retryable error that writes nothing | contract + proptest | `cargo test -p minos-store` | ✅ rewrite | ⬜ pending |
| 07-05-T2 | 07-05 | 2 | LOOP-06 | T-07-05-02, T-07-05-03, T-07-05-04, T-07-05-05 | One run lock at a time on both stores; symlink refused; appends still allowed | contract | `cargo test -p minos-store --test contract` | ✅ extend | ⬜ pending |
| 07-06-T1 | 07-06 | 2 | LOOP-04 (D-12) | T-07-06-01, T-07-06-02, T-07-06-03 | A parent tree or bare diff cannot mint an experiment | unit + trybuild | `cargo test -p minos-core --lib && cargo test -p minos-core --test cumulative --test compile_fail && cargo clippy -p minos-core --lib -- -D warnings` (scoped: the workspace compiles again only after 07-06-T3) | ❌ W0 | ⬜ pending |
| 07-06-T2 | 07-06 | 2 | LOOP-05 (D-12) | T-07-06-04 | No golden or ID moves | regression | `cargo test -p minos-core && cargo clippy -p minos-core --all-targets -- -D warnings && cargo test -p minos-guest && cargo test -p minos-backend-krun && cargo clippy -p minos-guest -p minos-backend-krun --all-targets -- -D warnings` (scoped per crate) | ✅ | ⬜ pending |
| 07-06-T3 | 07-06 | 2 | LOOP-04 (D-12) | T-07-06-01 | Proposals carry minted cumulative diffs; the workspace compiles again | regression | `cargo test -p minos-propose && cargo clippy -p minos-propose --all-targets -- -D warnings && cargo build --workspace --all-targets` | ✅ | ⬜ pending |
| 07-07-T1 | 07-07 | 2 | LOOP-04 (D-08, D-16) | T-07-07-02, T-07-07-03, T-07-07-04 | Seam and errors carry blame; no free text in events | compile + docs | `cargo clippy -p minos-loop --all-targets -- -D warnings && cargo doc -p minos-loop --no-deps` | n/a | ⬜ pending |
| 07-07-T2 | 07-07 | 2 | LOOP-05 (D-06, D-07) | T-07-07-01 | The next step is a pure, permutation-invariant function of the fold | proptest | `cargo test -p minos-loop --test stage` | ❌ W0 | ⬜ pending |
| 07-08-T1 | 07-08 | 2 | JUDGE-01 (D-03) | T-07-08-02, T-07-08-03, T-07-08-04, T-07-08-05 | Judge data packed with full hygiene under its caps | integration | `cargo test -p minos-source --test judge` | ❌ W0 | ⬜ pending |
| 07-08-T2 | 07-08 | 2 | JUDGE-01 (D-04, D-20) | T-07-08-01 | Snapshots and the image context refuse judge overlap; zero-length exempt | integration | `cargo test -p minos-source && cargo test -p minos-cli --test env_build` | ✅ extend | ⬜ pending |
| 07-09-T1 | 07-09 | 3 | LOOP-03 | T-07-09-01, T-07-09-05 | Usage from host elapsed only; errors carry blame | compile + docs | `cargo clippy -p minos-runner --all-targets -- -D warnings && cargo doc -p minos-runner --no-deps` | n/a | ⬜ pending |
| 07-09-T2 | 07-09 | 3 | LOOP-01, JUDGE-03 | T-07-09-02, T-07-09-03, T-07-09-04 | Every observation maps to one verdict; gates short-circuit | unit + proptest | `cargo test -p minos-runner --test map` | ❌ W0 | ⬜ pending |
| 07-10-T1 | 07-10 | 3 | LOOP-05 (D-08, D-10, D-13, D-19) | T-07-10-02, T-07-10-03, T-07-10-06, T-07-10-07 | Start and resume refusals; conflicts surface; spends never doubled | integration | `cargo test -p minos-loop --test blocks` | ❌ W0 | ⬜ pending |
| 07-10-T2 | 07-10 | 3 | LOOP-05 (D-18) | T-07-10-04 | Batch blob golden; partial batches completed; mismatches refused | golden + integration | `cargo test -p minos-loop --test blocks` | ❌ W0 (1 snapshot) | ⬜ pending |
| 07-10-T3 | 07-10 | 3 | LOOP-04 (D-09) | T-07-10-01, T-07-10-05 | Excerpts only through `LogExcerpt`; parent tree from a verified diff | integration | `cargo test -p minos-loop --test blocks` | ❌ W0 | ⬜ pending |
| 07-11-T1 | 07-11 | 3 | JUDGE-01 (D-01, D-02, D-03) | T-07-11-01, T-07-11-02, T-07-11-03, T-07-11-04, T-07-11-05 | Label from the manifest; `--update` gate; placement and judge-path checks; atomic lock write | integration | `cargo test -p minos-cli --test judge_push --test env_build` | ❌ W0 | ⬜ pending |
| 07-11-T2 | 07-11 | 3 | CLI-04 | T-07-11-06 | Subcommand parses; paths printed through `SafeText` | unit | `cargo test -p minos-cli --bin minos` | ✅ extend | ⬜ pending |
| 07-11-T3 | 07-11 | 3 | JUDGE-01 (D-01 to D-05, D-20) | T-07-02-03 | Docs state the lock's exposure honestly | doc check | `grep -n 'judge push' CLAUDE.md` | ✅ | ⬜ pending |
| 07-12-T1 | 07-12 | 4 | LOOP-01, LOOP-02 | T-07-12-01, T-07-12-02, T-07-12-05 | Judge data only through `secrets`; only `out/` to the judge | compile + docs | `cargo clippy -p minos-runner --all-targets -- -D warnings && cargo doc -p minos-runner --no-deps` | n/a | ⬜ pending |
| 07-12-T2 | 07-12 | 4 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-12-01, T-07-12-02, T-07-12-03, T-07-12-04, T-07-12-05, T-07-12-06, T-07-12-07 | Order, isolation, metering, gate rule, provenance, and blame over a fake backend; LOOP-01 grep empty | integration + grep | `cargo test -p minos-runner --test pipeline` | ❌ W0 | ⬜ pending |
| 07-13-T1 | 07-13 | 4 | LOOP-04 (D-06, D-07, D-10, D-13, D-19) | T-07-13-01, T-07-13-02, T-07-13-05 | Lock before read; checks before any record; record before act | compile + docs | `cargo clippy -p minos-loop --all-targets -- -D warnings && cargo doc -p minos-loop --no-deps` | n/a | ⬜ pending |
| 07-13-T2 | 07-13 | 4 | LOOP-04, LOOP-09 (D-05, D-08, D-18) | T-07-13-01, T-07-13-03, T-07-13-04, T-07-13-06 | Whole generations on fakes for both tiers; provider stops; conflicts stop | integration | `cargo test -p minos-loop --test driver flow::` | ❌ W0 | ⬜ pending |
| 07-14-T1 | 07-14 | 5 | LOOP-01 (D-02) | T-07-14-01, T-07-14-04 | Adapter maps blame by `is_infra`; judge inputs from the lock | unit | `cargo test -p minos-cli --lib` | ✅ extend | ⬜ pending |
| 07-14-T2 | 07-14 | 5 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-14-02, T-07-14-03 | Ignored VM test compiles in the gate and launches nothing there | ignored VM | `cargo test -p minos-cli --test runner_vm` | ❌ W0 | ⬜ pending |
| 07-14-T3 | 07-14 | 5 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-14-05 | Production runner on real VMs, recorded | manual VM run (executor; a host refusal becomes a `checkpoint:human-action` for the user) | `cargo run -q -p minos-cli -- host doctor`, then `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` (after build, sign, musl build) | ❌ W0 | ⬜ pending |
| 07-15-T1 | 07-15 | 5 | LOOP-07, LOOP-08, JUDGE-04 (D-11, D-13 to D-16, D-19) | T-07-15-01, T-07-15-02, T-07-15-03, T-07-15-04, T-07-15-05, T-07-15-06, T-07-15-07 | Safe points, streak, pause, pin, and post-call price check | integration | `cargo test -p minos-loop --test driver` | ✅ extend | ⬜ pending |
| 07-15-T2 | 07-15 | 5 | LOOP-07, LOOP-08, JUDGE-04 | T-07-15-01, T-07-15-02, T-07-15-04, T-07-15-05, T-07-15-06, T-07-15-07 | Caps, breaker, session stops, and pin proven on fakes | integration | `cargo test -p minos-loop --test driver -- caps:: breaker:: stops:: pins::` | ❌ W0 | ⬜ pending |
| 07-16-T1 | 07-16 | 6 | LOOP-05 (D-07, D-18) | T-07-16-01, T-07-16-02 | Every crash point resumes to the uninterrupted result | integration (crash injection) | `cargo test -p minos-loop --test driver resume::` | ❌ W0 | ⬜ pending |
| 07-16-T2 | 07-16 | 6 | LOOP-06 | T-07-16-03 | A second driver is refused on both stores | integration | `cargo test -p minos-loop --test driver lock::` | ❌ W0 | ⬜ pending |
| 07-16-T3 | 07-16 | 6 | LOOP-04 to LOOP-08 (D-07 to D-19) | T-07-16-04 | Design of record matches the build | doc check | `grep -n '## minos-runner and minos-loop' CLAUDE.md` | ✅ | ⬜ pending |
| 07-17-T1 | 07-17 | 7 | all phase requirements | T-07-17-01, T-07-17-02, T-07-17-03, T-07-17-04 | Full gate, LOOP-01 grep, lane check, and test-variable grep on the final code | full gate | the Full suite command above | ✅ | ⬜ pending |
| 07-17-T2 | 07-17 | 7 | GATE-06 | n/a | cargo clean last | structural | `test ! -d target/debug` | n/a | ⬜ pending |

Requirement to test mapping (from 07-RESEARCH.md "Phase Requirements → Test Map", rewritten by the planner to the targets the plans create):

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| LOOP-01 | Candidate then judge; each row of the `(Ended, GuestStatus, output exits)` → `Outcome` table maps exactly; the judge is launched only after a passing candidate | unit (pure map) + integration (fake `Backend`) | `cargo test -p minos-runner --test map --test pipeline` | ❌ W0 (07-09, 07-12) |
| LOOP-01 | Only `minos-runner` and `minos-sandbox` call an `Untrusted` exit in production code | grep check | `grep -rnE '\.(take_status\|into_scorecard\|into_output\|into_log_excerpt\|into_diff_texts\|store_judge_input\|store_tail)\(' crates/*/src --include='*.rs' \| grep -vE '^crates/minos-(runner\|sandbox)/'` prints nothing | command (07-12, 07-17) |
| LOOP-02 | The judge plan carries judge data; the candidate plan's input names none; the judge input holds only `out/` files; judge output has no `store_judge_input` | integration (fake backend records plans) + trybuild | `cargo test -p minos-runner --test pipeline` ; `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ❌ W0 (07-12) / ✅ extend (07-04) |
| LOOP-03 | Usage equals elapsed × machine for both VMs; forged timing files ignored; tokens refused | integration | `cargo test -p minos-runner --test pipeline` | ❌ W0 (07-12) |
| LOOP-04 | Generation flow over `MemoryStore` + `FakeProvider` (sweep and single_shot) + `FakeJobs`; admission dedupe; `Selected` appended | integration | `cargo test -p minos-loop --test driver flow::` | ❌ W0 (07-13) |
| LOOP-04 | The next step is a pure function of the fold (permuted records give the same step; progress; termination) | proptest | `cargo test -p minos-loop --test stage` | ❌ W0 (07-07) |
| LOOP-05 | Crash before the Nth append, for every N: the resumed run folds to the same `RunState` as an uninterrupted one (sweep exact; single_shot with only D-07's extra call spend) | integration (crash injection through `FaultStore`) | `cargo test -p minos-loop --test driver resume::` | ❌ W0 (07-16) |
| LOOP-05 | Batch blob golden and completion; spend dedupe; conflict classification | golden + integration | `cargo test -p minos-loop --test blocks` | ❌ W0 (07-10) |
| LOOP-06 | Second run lock refused on both stores; released on drop; a second driver refused on `MemoryStore` and on two `FileStore` handles | contract + integration | `cargo test -p minos-store --test contract` ; `cargo test -p minos-loop --test driver lock::` | ✅ extend (07-05) / ❌ W0 (07-16) |
| LOOP-07 | Spend and token caps stop at generation, model-call, and after-trial safe points with `StopRequested { SpendCap \| TokenCap }`; overshoot at most one step; an unpriced resolved model stops after one call | integration | `cargo test -p minos-loop --test driver caps::` | ❌ W0 (07-15) |
| LOOP-08 | K empty generations then `NoValidCandidates`; K consecutive infra failures (fake job runner, provider, store) then `InfraFailures`; a success resets the streak; zero pause in tests | integration | `cargo test -p minos-loop --test driver breaker::` | ❌ W0 (07-15) |
| LOOP-09 | No loop test needs a VM or a login | structural | `cargo test -p minos-loop` runs in the gate | ❌ W0 (07-07 to 07-16) |
| JUDGE-01 | Push packs, stores, and writes `[[judge]]`; `--update` gate; placement and judge-path checks; source and env-build snapshots refuse overlap naming paths (zero-length exempt); over 96 MiB refused | integration (temp project) + unit | `cargo test -p minos-cli --test judge_push --test env_build` ; `cargo test -p minos-source --test judge` | ❌ W0 (07-08, 07-11) |
| JUDGE-01 | `[[judge]]` golden + pin; empty array omitted; duplicate label and non-ascending files refused; `check_judge` `NoJudge` | golden + unit | `cargo test -p minos-core --test lock` | ✅ extend (07-02) |
| JUDGE-03 | A gate not exactly 1 gives `Rejected(GateFailed)` with no scorecard recorded and objectives unread | unit + integration | `cargo test -p minos-runner --test map --test pipeline` ; `cargo test -p minos-core --test prop_policy` | ❌ W0 (07-09, 07-12) / ✅ |
| JUDGE-04 | `Judged.backend` from `Capabilities::backend_info`; a differing `(backend, arch)` is refused as configuration | integration | `cargo test -p minos-loop --test driver pins::` ; `cargo test -p minos-runner --test pipeline` | ❌ W0 (07-15, 07-12) |
| CLI-04 | `minos judge push <dir> [--update]` in the command tree; `debug_assert`; parse cases | unit | `cargo test -p minos-cli --bin minos` | ✅ extend (07-11) |
| D-05 | `[judge] trials` default 3, range-checked, absent from `JudgeVersion` (golden IDs unchanged) | unit | `cargo test -p minos-core --test manifest --test golden_ids` | ✅ extend (07-02) |
| D-08 | `Conflict` is `Err`, non-retryable, and writes nothing (both stores) | contract | `cargo test -p minos-store --test contract` | ✅ rewrite (07-05) |
| D-09, D-10, D-18 | New `Option` fields: old goldens byte-identical; new golden cases with their pin; the fold carries them | golden + proptest | `cargo test -p minos-core --test golden_wire --test prop_fold` | ✅ extend (07-01) |
| D-12 | A parent tree or a bare diff cannot mint an experiment; `recorded` refuses a mismatched ID | unit + trybuild | `cargo test -p minos-core --test cumulative --test compile_fail` | ❌ W0 (07-06) |
| D-17 | The two sink exits exist exactly as approved, capped, digest-checked, and role-bound | unit + golden pin + trybuild | `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ✅ extend (07-04) |
| D-19 | Old stops do not block a resume; new stops end the session after the running trial is recorded; a cap stop lifts after a budget edit | integration | `cargo test -p minos-loop --test driver stops::` | ❌ W0 (07-15) |
| End to end | Production runner on real VMs: baseline `Scored`; a candidate's `out/` reaches the judge; a gate-failing judge `Rejected(GateFailed)`; krun/arm64 and host usage recorded | ignored VM | `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` | ❌ W0 (07-14) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Loop test support: one test target `crates/minos-loop/tests/driver/` (`main.rs`, `support.rs` with `FakeJobs`, `FaultStore`, and fixtures; modules `flow`, `caps`, `breaker`, `stops`, `pins`, `resume`, `lock`), so every support item is used inside one crate and `-D warnings` sees no dead code (the `tests/harness` lesson). Crash injection is `FaultStore` failing the Nth append with a non-retryable error, then a fresh driver resuming on the inner store (07-13, 07-15, 07-16). The pure stage function has its own target `tests/stage.rs` (07-07) and the building blocks `tests/blocks.rs` (07-10).
- [ ] Runner test support: the scripted `FakeBackend` and the ustar output builder live inside `crates/minos-runner/tests/pipeline.rs` (07-12); the pure map has `tests/map.rs` (07-09). No shared support module.
- [ ] `crates/minos-runner/clippy.toml` and `crates/minos-loop/clippy.toml` with every ban proven to fire in 07-EVIDENCE.md, including the loop's `minos_core::id::ExperimentId::of` ban (07-03).
- [ ] `crates/minos-core/tests/snapshots/`: four new `golden_wire` cases with their own pin (07-01) and the `minos_lock_v1_judge` case with its pin (07-02); `crates/minos-loop/tests/snapshots/blocks__batch_v1.snap` (07-10).
- [ ] `crates/minos-cli/tests/runner_vm.rs` with its fixture rootfs and the `minos-probe scorecard <gate> <score>` form (07-14); the ignored VM test lives in `minos-cli` because `deny.toml` lets only the CLI depend on `minos-backend-krun`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Production runner on real VMs (07-14 Task 3): the baseline and a candidate run on a fixture image packed by the production packer, the baseline is `Scored`, the candidate's `out/` reaches the judge, a gate-failing judge yields `Rejected(GateFailed)`, and every result records krun/arm64 and host usage | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | Needs the signed `minos-vmm`, libkrunfw, and the musl `minos-guest` on a VM host; ignored in the gate (05-CONTEXT D-04) | After build, sign, and the musl guest build: `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1`; record the result in 07-EVIDENCE.md |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
