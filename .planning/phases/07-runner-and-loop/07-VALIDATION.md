---
phase: 7
slug: runner-and-loop
status: complete
nyquist_compliant: true
wave_0_complete: true
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
| 07-01-T1 | 07-01 | 1 | LOOP-05 (D-09, D-10, D-18) | T-07-01-01, T-07-01-02, T-07-01-03, T-07-01-04, T-07-01-05 | Old ledger lines and goldens byte-identical; logs only as blob IDs; keys unchanged | golden | `cargo test -p minos-core --test golden_wire` | ✅ created (4 snapshots) | ✅ green |
| 07-01-T2 | 07-01 | 1 | LOOP-04, JUDGE-04 (D-07, D-11, D-19) | T-07-01-03 | New `RunState` accessors are order-independent | proptest + unit | `cargo test -p minos-core --test prop_fold && cargo test -p minos-store --test file_ledger` | ✅ extend | ✅ green |
| 07-01-T3 | 07-01 | 1 | LOOP-04 (Pitfall 12) | T-07-01-06 | `baseline_ready` agrees with `conclude` | proptest | `cargo test -p minos-core --test prop_policy` | ✅ extend | ✅ green |
| 07-02-T1 | 07-02 | 1 | LOOP-07 (D-05, D-13) | T-07-02-04, T-07-02-05 | `trials` range-checked and outside `JudgeVersion`; unpriced models named | unit | `cargo test -p minos-core --test manifest --test money --test golden_ids` | ✅ extend | ✅ green |
| 07-02-T2 | 07-02 | 1 | JUDGE-01 (D-02, D-04) | T-07-02-01, T-07-02-02, T-07-02-03 | One canonical `[[judge]]` wire form; malformed tables refused; cap before parse | golden + unit | `cargo test -p minos-core --test lock` | ✅ created (1 snapshot) | ✅ green |
| 07-03-T1 | 07-03 | 1 | LOOP-04 | T-07-03-04, T-07-03-SC | Lanes unchanged; no new package | structural | `cargo deny check && cargo build --workspace` | ✅ | ✅ green |
| 07-03-T2 | 07-03 | 1 | LOOP-03 (D-12) | T-07-03-01, T-07-03-02, T-07-03-03 | Clock, env, process, file, HashMap, and `ExperimentId::of` bans fire | clippy proof | `cargo clippy -p minos-runner -p minos-loop --all-targets -- -D warnings` | ✅ created (2 clippy.toml) | ✅ green |
| 07-04-T1 | 07-04 | 2 | LOOP-01 (D-17) | T-07-04-01 | User approves the exact exit signatures before code | checkpoint (manual) | `test -z "$(git status --porcelain crates/)"` | n/a | ✅ green |
| 07-04-T2 | 07-04 | 2 | LOOP-01, LOOP-02 (D-09, D-17) | T-07-04-01, T-07-04-02, T-07-04-03, T-07-04-04, T-07-04-05, T-07-04-06 | Only `out/` repacked canonically; tails capped; digest checked; no exit on the wrong role | unit + golden pin + trybuild | `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ✅ extend | ✅ green |
| 07-04-T3 | 07-04 | 2 | LOOP-01 (D-17) | T-07-04-01 | Design of record names the approved exits | doc check | `grep -n 'store_judge_input' CLAUDE.md` | ✅ | ✅ green |
| 07-05-T1 | 07-05 | 2 | LOOP-05 (D-08) | T-07-05-01 | A conflicting append is a non-retryable error that writes nothing | contract + proptest | `cargo test -p minos-store` | ✅ rewrite | ✅ green |
| 07-05-T2 | 07-05 | 2 | LOOP-06 | T-07-05-02, T-07-05-03, T-07-05-04, T-07-05-05 | One run lock at a time on both stores; symlink refused; appends still allowed | contract | `cargo test -p minos-store --test contract` | ✅ extend | ✅ green |
| 07-06-T1 | 07-06 | 2 | LOOP-04 (D-12) | T-07-06-01, T-07-06-02, T-07-06-03 | A parent tree or bare diff cannot mint an experiment | unit + trybuild | `cargo test -p minos-core --lib && cargo test -p minos-core --test cumulative --test compile_fail && cargo clippy -p minos-core --lib -- -D warnings` (scoped: the workspace compiles again only after 07-06-T3) | ✅ created | ✅ green |
| 07-06-T2 | 07-06 | 2 | LOOP-05 (D-12) | T-07-06-04 | No golden or ID moves | regression | `cargo test -p minos-core && cargo clippy -p minos-core --all-targets -- -D warnings && cargo test -p minos-guest && cargo test -p minos-backend-krun && cargo clippy -p minos-guest -p minos-backend-krun --all-targets -- -D warnings` (scoped per crate) | ✅ | ✅ green |
| 07-06-T3 | 07-06 | 2 | LOOP-04 (D-12) | T-07-06-01 | Proposals carry minted cumulative diffs; the workspace compiles again | regression | `cargo test -p minos-propose && cargo clippy -p minos-propose --all-targets -- -D warnings && cargo build --workspace --all-targets` | ✅ | ✅ green |
| 07-07-T1 | 07-07 | 2 | LOOP-04 (D-08, D-16) | T-07-07-02, T-07-07-03, T-07-07-04 | Seam and errors carry blame; no free text in events | compile + docs | `cargo clippy -p minos-loop --all-targets -- -D warnings && cargo doc -p minos-loop --no-deps` | n/a | ✅ green |
| 07-07-T2 | 07-07 | 2 | LOOP-05 (D-06, D-07) | T-07-07-01 | The next step is a pure, permutation-invariant function of the fold | proptest | `cargo test -p minos-loop --test stage` | ✅ created | ✅ green |
| 07-08-T1 | 07-08 | 2 | JUDGE-01 (D-03) | T-07-08-02, T-07-08-03, T-07-08-04, T-07-08-05 | Judge data packed with full hygiene under its caps | integration | `cargo test -p minos-source --test judge` | ✅ created | ✅ green |
| 07-08-T2 | 07-08 | 2 | JUDGE-01 (D-04, D-20) | T-07-08-01 | Snapshots and the image context refuse judge overlap; zero-length exempt | integration | `cargo test -p minos-source && cargo test -p minos-cli --test env_build` | ✅ extend | ✅ green |
| 07-09-T1 | 07-09 | 3 | LOOP-03 | T-07-09-01, T-07-09-05 | Usage from host elapsed only; errors carry blame | compile + docs | `cargo clippy -p minos-runner --all-targets -- -D warnings && cargo doc -p minos-runner --no-deps` | n/a | ✅ green |
| 07-09-T2 | 07-09 | 3 | LOOP-01, JUDGE-03 | T-07-09-02, T-07-09-03, T-07-09-04 | Every observation maps to one verdict; gates short-circuit | unit + proptest | `cargo test -p minos-runner --test map --test usage` (07-09 added `tests/usage.rs`) | ✅ created | ✅ green |
| 07-10-T1 | 07-10 | 3 | LOOP-05 (D-08, D-10, D-13, D-19) | T-07-10-02, T-07-10-03, T-07-10-06, T-07-10-07 | Start and resume refusals; conflicts surface; spends never doubled | integration | `cargo test -p minos-loop --test blocks` | ✅ created | ✅ green |
| 07-10-T2 | 07-10 | 3 | LOOP-05 (D-18) | T-07-10-04 | Batch blob golden; partial batches completed; mismatches refused | golden + integration | `cargo test -p minos-loop --test blocks` | ✅ created (1 snapshot) | ✅ green |
| 07-10-T3 | 07-10 | 3 | LOOP-04 (D-09) | T-07-10-01, T-07-10-05 | Excerpts only through `LogExcerpt`; parent tree from a verified diff | integration | `cargo test -p minos-loop --test blocks` | ✅ created | ✅ green |
| 07-11-T1 | 07-11 | 3 | JUDGE-01 (D-01, D-02, D-03) | T-07-11-01, T-07-11-02, T-07-11-03, T-07-11-04, T-07-11-05 | Label from the manifest; `--update` gate; placement and judge-path checks; atomic lock write | integration | `cargo test -p minos-cli --test judge_push --test env_build` | ✅ created | ✅ green |
| 07-11-T2 | 07-11 | 3 | CLI-04 | T-07-11-06 | Subcommand parses; paths printed through `SafeText` | unit | `cargo test -p minos-cli --bin minos` | ✅ extend | ✅ green |
| 07-11-T3 | 07-11 | 3 | JUDGE-01 (D-01 to D-05, D-20) | T-07-02-03 | Docs state the lock's exposure honestly | doc check | `grep -n 'judge push' CLAUDE.md` | ✅ | ✅ green |
| 07-12-T1 | 07-12 | 4 | LOOP-01, LOOP-02 | T-07-12-01, T-07-12-02, T-07-12-05 | Judge data only through `secrets`; only `out/` to the judge | compile + docs | `cargo clippy -p minos-runner --all-targets -- -D warnings && cargo doc -p minos-runner --no-deps` | n/a | ✅ green |
| 07-12-T2 | 07-12 | 4 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-12-01, T-07-12-02, T-07-12-03, T-07-12-04, T-07-12-05, T-07-12-06, T-07-12-07 | Order, isolation, metering, gate rule, provenance, and blame over a fake backend; LOOP-01 grep empty | integration + grep | `cargo test -p minos-runner --test pipeline` | ✅ created | ✅ green |
| 07-13-T1 | 07-13 | 4 | LOOP-04 (D-06, D-07, D-10, D-13, D-19) | T-07-13-01, T-07-13-02, T-07-13-05 | Lock before read; checks before any record; record before act | compile + docs | `cargo clippy -p minos-loop --all-targets -- -D warnings && cargo doc -p minos-loop --no-deps` | n/a | ✅ green |
| 07-13-T2 | 07-13 | 4 | LOOP-04, LOOP-09 (D-05, D-08, D-18) | T-07-13-01, T-07-13-03, T-07-13-04, T-07-13-06 | Whole generations on fakes for both tiers; provider stops; conflicts stop | integration | `cargo test -p minos-loop --test driver flow::` | ✅ created | ✅ green |
| 07-14-T1 | 07-14 | 5 | LOOP-01 (D-02) | T-07-14-01, T-07-14-04 | Adapter maps blame by `is_infra`; judge inputs from the lock | unit | `cargo test -p minos-cli --lib` | ✅ extend | ✅ green |
| 07-14-T2 | 07-14 | 5 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-14-02, T-07-14-03 | Ignored VM test compiles in the gate and launches nothing there | ignored VM | `cargo test -p minos-cli --test runner_vm` | ✅ created | ✅ green |
| 07-14-T3 | 07-14 | 5 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | T-07-14-05 | Production runner on real VMs, recorded | manual VM run (executor; a host refusal becomes a `checkpoint:human-action` for the user) | `cargo run -q -p minos-cli -- host doctor`, then `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` (after build, sign, musl build) | ✅ created | ✅ green (3 passed, 07-EVIDENCE.md "Runner on real VMs (07-14)") |
| 07-15-T1 | 07-15 | 5 | LOOP-07, LOOP-08, JUDGE-04 (D-11, D-13 to D-16, D-19) | T-07-15-01, T-07-15-02, T-07-15-03, T-07-15-04, T-07-15-05, T-07-15-06, T-07-15-07 | Safe points, streak, pause, pin, and post-call price check | integration | `cargo test -p minos-loop --test driver` | ✅ extend | ✅ green |
| 07-15-T2 | 07-15 | 5 | LOOP-07, LOOP-08, JUDGE-04 | T-07-15-01, T-07-15-02, T-07-15-04, T-07-15-05, T-07-15-06, T-07-15-07 | Caps, breaker, session stops, and pin proven on fakes | integration | `cargo test -p minos-loop --test driver -- caps:: breaker:: stops:: pins::` | ✅ created | ✅ green |
| 07-16-T1 | 07-16 | 6 | LOOP-05 (D-07, D-18) | T-07-16-01, T-07-16-02 | Every crash point resumes to the uninterrupted result | integration (crash injection) | `cargo test -p minos-loop --test driver resume::` | ✅ created | ✅ green |
| 07-16-T2 | 07-16 | 6 | LOOP-06 | T-07-16-03 | A second driver is refused on both stores | integration | `cargo test -p minos-loop --test driver lock::` | ✅ created | ✅ green |
| 07-16-T3 | 07-16 | 6 | LOOP-04 to LOOP-08 (D-07 to D-19) | T-07-16-04 | Design of record matches the build | doc check | `grep -n '## minos-runner and minos-loop' CLAUDE.md` | ✅ | ✅ green |
| 07-17-T1 | 07-17 | 7 | all phase requirements | T-07-17-01, T-07-17-02, T-07-17-03, T-07-17-04 | Full gate, LOOP-01 grep, lane check, and test-variable grep on the final code | full gate | the Full suite command above, then the LOOP-01, lane, and test-variable checks | ✅ | ✅ green (07-EVIDENCE.md "Final gate") |
| 07-17-T2 | 07-17 | 7 | GATE-06 | n/a | cargo clean last | structural | `test ! -d target/debug` | n/a | ⬜ pending |

Requirement to test mapping (from 07-RESEARCH.md "Phase Requirements → Test Map", rewritten by the planner to the targets the plans create). Every `--test` target below was checked against the final `cargo test --workspace` output (07-EVIDENCE.md "Final gate", 2026-10-24): `minos-runner` `map` 53, `pipeline` 18, `usage` 7; `minos-loop` `stage` 13, `blocks` 42, `driver` 38 (`flow::` 14, `caps::` 5, `breaker::` 6, `stops::` 4, `pins::` 3, `resume::` 2, `lock::` 4); `minos-cli` `judge_push` 12, `env_build` 11 (1 ignored), `runner_vm` 1 (3 ignored), bin `minos` 8, lib 17; `minos-source` `judge` 18; `minos-core` `lock` 30, `golden_wire` 40, `prop_fold` 53, `prop_policy` 48, `manifest` 108, `money` 48, `golden_ids` 17, `cumulative` 9, `compile_fail` 1; `minos-sandbox` `untrusted` 27, `compile_fail` 1; `minos-store` `contract` 43, `file_ledger` 25. All passed.

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| LOOP-01 | Candidate then judge; each row of the `(Ended, GuestStatus, output exits)` → `Outcome` table maps exactly; the judge is launched only after a passing candidate | unit (pure map) + integration (fake `Backend`) | `cargo test -p minos-runner --test map --test pipeline` | ✅ created (07-09, 07-12) |
| LOOP-01 | Only `minos-runner` and `minos-sandbox` call an `Untrusted` exit in production code | grep check | `grep -rnE '\.(take_status\|into_scorecard\|into_output\|into_log_excerpt\|into_diff_texts\|store_judge_input\|store_tail)\(' crates/*/src --include='*.rs' \| grep -vE '^crates/minos-(runner\|sandbox)/'` prints nothing | command (07-12, 07-17) |
| LOOP-02 | The judge plan carries judge data; the candidate plan's input names none; the judge input holds only `out/` files; judge output has no `store_judge_input` | integration (fake backend records plans) + trybuild | `cargo test -p minos-runner --test pipeline` ; `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ✅ created (07-12) / ✅ extend (07-04) |
| LOOP-03 | Usage equals elapsed × machine for both VMs; forged timing files ignored; tokens refused | integration + unit | `cargo test -p minos-runner --test pipeline --test usage` | ✅ created (07-12) |
| LOOP-04 | Generation flow over `MemoryStore` + `FakeProvider` (sweep and single_shot) + `FakeJobs`; admission dedupe; `Selected` appended | integration | `cargo test -p minos-loop --test driver flow::` | ✅ created (07-13) |
| LOOP-04 | The next step is a pure function of the fold (permuted records give the same step; progress; termination) | proptest | `cargo test -p minos-loop --test stage` | ✅ created (07-07) |
| LOOP-05 | Crash before the Nth append, for every N: the resumed run folds to the same `RunState` as an uninterrupted one (sweep exact; single_shot with only D-07's extra call spend) | integration (crash injection through `FaultStore`) | `cargo test -p minos-loop --test driver resume::` | ✅ created (07-16) |
| LOOP-05 | Batch blob golden and completion; spend dedupe; conflict classification | golden + integration | `cargo test -p minos-loop --test blocks` | ✅ created (07-10) |
| LOOP-06 | Second run lock refused on both stores; released on drop; a second driver refused on `MemoryStore` and on two `FileStore` handles | contract + integration | `cargo test -p minos-store --test contract` ; `cargo test -p minos-loop --test driver lock::` | ✅ extend (07-05) / ✅ created (07-16) |
| LOOP-07 | Spend and token caps stop at generation, model-call, and after-trial safe points with `StopRequested { SpendCap \| TokenCap }`; overshoot at most one step; an unpriced resolved model stops after one call | integration | `cargo test -p minos-loop --test driver caps::` | ✅ created (07-15) |
| LOOP-08 | K empty generations then `NoValidCandidates`; K consecutive infra failures (fake job runner, provider, store) then `InfraFailures`; a success resets the streak; zero pause in tests | integration | `cargo test -p minos-loop --test driver breaker::` | ✅ created (07-15) |
| LOOP-09 | No loop test needs a VM or a login | structural | `cargo test -p minos-loop` runs in the gate | ✅ created (07-07 to 07-16) |
| JUDGE-01 | Push packs, stores, and writes `[[judge]]`; `--update` gate; placement and judge-path checks; source and env-build snapshots refuse overlap naming paths (zero-length exempt); over 96 MiB refused | integration (temp project) + unit | `cargo test -p minos-cli --test judge_push --test env_build` ; `cargo test -p minos-source --test judge` | ✅ created (07-08, 07-11) |
| JUDGE-01 | `[[judge]]` golden + pin; empty array omitted; duplicate label and non-ascending files refused; `check_judge` `NoJudge` | golden + unit | `cargo test -p minos-core --test lock` | ✅ extend (07-02) |
| JUDGE-03 | A gate not exactly 1 gives `Rejected(GateFailed)` with no scorecard recorded and objectives unread | unit + integration | `cargo test -p minos-runner --test map --test pipeline` ; `cargo test -p minos-core --test prop_policy` | ✅ created (07-09, 07-12) / ✅ |
| JUDGE-04 | `Judged.backend` from `Capabilities::backend_info`; a differing `(backend, arch)` is refused as configuration | integration | `cargo test -p minos-loop --test driver pins::` ; `cargo test -p minos-runner --test pipeline` | ✅ created (07-15, 07-12) |
| CLI-04 | `minos judge push <dir> [--update]` in the command tree; `debug_assert`; parse cases | unit | `cargo test -p minos-cli --bin minos` | ✅ extend (07-11) |
| D-05 | `[judge] trials` default 3, range-checked, absent from `JudgeVersion` (golden IDs unchanged) | unit | `cargo test -p minos-core --test manifest --test golden_ids` | ✅ extend (07-02) |
| D-08 | `Conflict` is `Err`, non-retryable, and writes nothing (both stores) | contract | `cargo test -p minos-store --test contract` | ✅ rewrite (07-05) |
| D-09, D-10, D-18 | New `Option` fields: old goldens byte-identical; new golden cases with their pin; the fold carries them | golden + proptest | `cargo test -p minos-core --test golden_wire --test prop_fold` | ✅ extend (07-01) |
| D-12 | A parent tree or a bare diff cannot mint an experiment; `recorded` refuses a mismatched ID | unit + trybuild | `cargo test -p minos-core --test cumulative --test compile_fail` | ✅ created (07-06) |
| D-17 | The two sink exits exist exactly as approved, capped, digest-checked, and role-bound | unit + golden pin + trybuild | `cargo test -p minos-sandbox --test untrusted --test compile_fail` | ✅ extend (07-04) |
| D-19 | Old stops do not block a resume; new stops end the session after the running trial is recorded; a cap stop lifts after a budget edit | integration | `cargo test -p minos-loop --test driver stops::` | ✅ created (07-15) |
| End to end | Production runner on real VMs: baseline `Scored`; a candidate's `out/` reaches the judge; a gate-failing judge `Rejected(GateFailed)`; krun/arm64 and host usage recorded | ignored VM | `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` | ✅ created (07-14) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] Loop test support: one test target `crates/minos-loop/tests/driver/` (`main.rs`, `support.rs` with `FakeJobs`, `FaultStore`, and fixtures; modules `flow`, `caps`, `breaker`, `stops`, `pins`, `resume`, `lock`), so every support item is used inside one crate and `-D warnings` sees no dead code (the `tests/harness` lesson). Crash injection is `FaultStore` failing the Nth append with a non-retryable error, then a fresh driver resuming on the inner store (07-13, 07-15, 07-16). The pure stage function has its own target `tests/stage.rs` (07-07) and the building blocks `tests/blocks.rs` (07-10). *Landed: `crates/minos-loop/tests/driver/` with `main.rs`, `support.rs`, `flow.rs` (07-13), `caps.rs`, `breaker.rs`, `stops.rs`, `pins.rs` (07-15), `resume.rs`, `lock.rs` (07-16); `tests/stage.rs` (07-07) with its committed `stage.proptest-regressions`; `tests/blocks.rs` (07-10).*
- [x] Runner test support: the scripted `FakeBackend` and the ustar output builder live inside `crates/minos-runner/tests/pipeline.rs` (07-12); the pure map has `tests/map.rs` (07-09). No shared support module. *Landed: `crates/minos-runner/tests/pipeline.rs` (07-12), `tests/map.rs` and `tests/usage.rs` (07-09).*
- [x] `crates/minos-runner/clippy.toml` and `crates/minos-loop/clippy.toml` with every ban proven to fire in 07-EVIDENCE.md, including the loop's `minos_core::id::ExperimentId::of` ban (07-03). *Landed: both files in 07-03; every ban recorded firing in 07-EVIDENCE.md "Clippy bans fire (07-03)".*
- [x] `crates/minos-core/tests/snapshots/`: four new `golden_wire` cases with their own pin (07-01) and the `minos_lock_v1_judge` case with its pin (07-02); `crates/minos-loop/tests/snapshots/blocks__batch_v1.snap` (07-10). *Landed: `golden_wire__record_v1_judged_logs`, `golden_wire__record_v1_run_started_root_image`, `golden_wire__record_v1_proposed_batch`, `golden_wire__job_result_judge_usage` with pin `2da862bd…` (07-01); `lock__minos_lock_v1_judge` with pin `bc21f64f…` (07-02); `blocks__batch_v1.snap` with its pin (07-10).*
- [x] `crates/minos-cli/tests/runner_vm.rs` with its fixture rootfs and the `minos-probe scorecard <gate> <score>` form (07-14); the ignored VM test lives in `minos-cli` because `deny.toml` lets only the CLI depend on `minos-backend-krun`. *Landed in 07-14: three ignored VM tests and one gate test (`fixture_manifests_parse`); the fixture image is packed in the test by the production packer; `minos-probe scorecard` in `crates/minos-guest/src/bin/minos-probe.rs`.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Production runner on real VMs (07-14 Task 3): the baseline and a candidate run on a fixture image packed by the production packer, the baseline is `Scored`, the candidate's `out/` reaches the judge, a gate-failing judge yields `Rejected(GateFailed)`, and every result records krun/arm64 and host usage | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | Needs the signed `minos-vmm`, libkrunfw, and the musl `minos-guest` on a VM host; ignored in the gate (05-CONTEXT D-04) | After build, sign, and the musl guest build: `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1`; record the result in 07-EVIDENCE.md. **Done 2026-10-24 (07-14, commit `c215775`):** `3 passed; 0 failed`, host doctor exit 0, every result `Krun` on `Arm64` with host-measured usage; 07-EVIDENCE.md "Runner on real VMs (07-14)" |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 300s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-24 (07-17 final gate; 07-17 Task 2, `cargo clean`, closes GATE-06)
