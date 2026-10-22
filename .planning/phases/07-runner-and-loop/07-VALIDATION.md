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
| *(filled by the planner from the PLAN.md task list; one row per task)* | | | | | | | | | |

Requirement to test mapping (from 07-RESEARCH.md "Phase Requirements → Test Map"; test target names are the research's proposals and are rewritten to the targets the plans create):

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| LOOP-01 | Candidate then judge; each row of the `(Ended, GuestStatus, output exits)` → `Outcome` table maps exactly; the judge is launched only after a passing candidate | unit (pure map) + integration (fake `Backend`) | `cargo test -p minos-runner --test pipeline --test map` | ❌ Wave 0 |
| LOOP-01 | Only `minos-runner` opens `Untrusted` in production code | grep check | `grep -rlnE 'take_status\|into_scorecard\|into_output\|store_judge_input\|store_tail' crates/*/src` lists only `minos-runner/src` and `minos-sandbox/src` | ❌ (command) |
| LOOP-02 | The judge plan carries judge data; the candidate plan's input names none; the judge input holds only `out/` files | integration (fake backend records plans) + existing trybuild | `cargo test -p minos-runner --test pipeline` ; `cargo test -p minos-sandbox --test compile_fail` | ❌ / ✅ |
| LOOP-03 | Usage equals elapsed × machine; forged timing files ignored; tokens refused | integration | `cargo test -p minos-runner --test pipeline host_usage` | ❌ Wave 0 |
| LOOP-04 | Generation flow over `MemoryStore` + `FakeProvider` (sweep and single-shot) + `FakeJobRunner`; admission dedupe; `Selected` appended | integration | `cargo test -p minos-loop --test flow` | ❌ Wave 0 |
| LOOP-04 | The next step is a pure function of the fold (permuted records give the same step) | proptest | `cargo test -p minos-loop --test stage` | ❌ Wave 0 |
| LOOP-05 | Crash after the Nth append, for every N: the resumed run folds to the same `RunState` as an uninterrupted one | integration (crash-injecting store wrapper) | `cargo test -p minos-loop --test resume` | ❌ Wave 0 |
| LOOP-06 | Second run lock refused on both stores; released on drop; a second driver refused | contract + integration | `cargo test -p minos-store --test contract run_lock` ; `cargo test -p minos-loop --test lock` | ❌ Wave 0 |
| LOOP-07 | Spend and token caps stop before a generation and before a model call with `StopRequested { SpendCap \| TokenCap }`; overshoot at most one step | integration | `cargo test -p minos-loop --test caps` | ❌ Wave 0 |
| LOOP-08 | K empty generations then `NoValidCandidates`; K consecutive infra failures (fake runner and provider) then `InfraFailures`; a success resets the streak; zero pause in tests | integration | `cargo test -p minos-loop --test breaker` | ❌ Wave 0 |
| LOOP-09 | No loop test needs a VM or a login | structural | `cargo test -p minos-loop` runs in the gate | ❌ Wave 0 |
| JUDGE-01 | Push packs, stores, and writes `[[judge]]`; `--update` gate; source and env-build snapshots refuse overlap naming paths; over 96 MiB refused | integration (temp project) + unit | `cargo test -p minos-cli --test judge_push` ; `cargo test -p minos-source --test judge` | ❌ Wave 0 |
| JUDGE-01 | `[[judge]]` golden + pin; empty array omitted; duplicate label and non-ascending files refused; `Lock::check` `NoJudge` | golden + unit | `cargo test -p minos-core --test lock` | ✅ (extend) |
| JUDGE-03 | Gate value ≠ 1 gives `Rejected(GateFailed)` with no scorecard recorded | unit + integration | `cargo test -p minos-runner --test map gate` ; `cargo test -p minos-core --test prop_policy` | ❌ / ✅ |
| JUDGE-04 | `Judged.backend` from `Capabilities::backend_info`; a differing `(backend, arch)` is refused as configuration | integration | `cargo test -p minos-loop --test pins` | ❌ Wave 0 |
| CLI-04 | `minos judge push <dir> [--update]` in the command tree; `debug_assert` | unit | `cargo test -p minos-cli` | ✅ (extend) |
| D-05 | `[judge] trials` default 3, range-checked, absent from `JudgeVersion` (golden IDs unchanged) | unit | `cargo test -p minos-core --test manifest --test golden_ids` | ✅ (extend) |
| D-08 | `Conflict` is `Err`, non-retryable, and writes nothing (both stores) | contract | `cargo test -p minos-store --test contract` | ✅ (rewrite cases) |
| D-09, D-10 | New `Option` fields: old goldens byte-identical; new golden cases; the fold carries `root_image` | golden | `cargo test -p minos-core --test golden_wire --test prop_fold` | ✅ (extend) |
| D-12 | A parent tree cannot mint a `CumulativeDiff`; `recorded` refuses a mismatched ID | trybuild or unit | `cargo test -p minos-core --test cumulative` | ❌ Wave 0 |
| End to end | Production runner on real VMs: baseline `Scored`; a gate-failing judge `Rejected(GateFailed)`; `Judged` backend krun/arm64 | ignored VM | `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` | ❌ Wave 0 |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/minos-loop/tests/support/mod.rs` — `FakeJobRunner` (scripted `JobResult` or infrastructure failure per `(exp, trial)`), a crash-injecting store wrapper (fails the Nth append with a non-retryable error), and fixture manifests (sweep and single-shot). Every item must be used by each target that declares `mod support;`, or `-D warnings` fails on dead code (the `tests/harness` lesson).
- [ ] `crates/minos-runner/tests/support/mod.rs` — `FakeBackend` returning per-role scripted `Finished` (pattern from `crates/minos-sandbox/tests/backend.rs`), and a ustar output builder.
- [ ] `crates/minos-runner/clippy.toml` and `crates/minos-loop/clippy.toml` — HashMap/HashSet, `SystemTime::now`, `Instant::now` (loop), `std::env::var`, `std::process::Command` bans; prove each ban fires once and record the proof in an evidence file, as earlier phases did.
- [ ] `crates/minos-core/tests/snapshots/` — new golden cases plus pin updates (`golden_wire`, `lock`).
- [ ] `crates/minos-cli/tests/runner_vm.rs` — fixture rootfs with `minos-probe`, plus a `scorecard <value>` probe form in `crates/minos-guest/src/bin/minos-probe.rs` (the ignored VM test lives in `minos-cli` because `deny.toml` lets only the CLI depend on `minos-backend-krun`).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Production runner on real VMs: the baseline candidate and judge VMs run on the Phase 5 fixture image, the baseline is `Scored`, a gate-failing judge yields `Rejected(GateFailed)`, and every `Judged` records krun/arm64 | LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04 | Needs the signed `minos-vmm`, libkrunfw, and the musl `minos-guest` on a VM host; ignored in the gate (05-CONTEXT D-04) | After build, sign, and the musl guest build: `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1`; record the result in 07-EVIDENCE.md |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
