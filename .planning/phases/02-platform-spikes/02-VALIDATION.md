---
phase: 2
slug: platform-spikes
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-10-09
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`): unit tests in `#[cfg(test)]` modules, plus `#[ignore]`d host-dependent integration tests in `crates/minos-backend-krun/tests/spike_*.rs` (D-02) |
| **Config file** | none beyond `Cargo.toml`, `rust-toolchain.toml` (musl target added by 02-01), `.cargo/config.toml` (musl linker only), `crates/minos-vmm/clippy.toml`, `crates/minos-backend-krun/clippy.toml` |
| **Quick run command** | `cargo test -p minos-guest -p minos-vmm -p minos-backend-krun` (unit tests: guest codecs, launch config, packers, initramfs, input disk, supervisor) |
| **Full suite command** | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, then C-01 and C-02 |
| **VM spike command** | build the musl guest and `minos-vmm`, sign with 02-06 E-4, then `cargo test -p minos-backend-krun --test <spike_boot\|spike_rootimg\|spike_exits\|spike_net\|spike_seatbelt> -- --ignored --nocapture --test-threads=1` with `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW` (and `MINOS_DOCKER`, `MINOS_VMM_UNSIGNED`, `MINOS_SPIKE_HOST_IP` where a test needs them; 02-06-PLAN.md E-1) |
| **Estimated runtime** | quick run about 30 s; full suite about 3 min (cargo deny fetches advisories); each VM spike file 1 to 5 min (Docker builds and the 64 MiB stream are the slowest) |

---

## Sampling Rate

- **After every task commit:** the task's `<automated>` verify command (compile, unit tests, clippy, or evidence greps; under 120 s except tasks that run VM spikes, whose verify greps the evidence they just recorded)
- **After every plan wave:** the full suite command plus C-01 and C-02; VM waves (3 and 4) also re-run their spike files with `-- --ignored`
- **Before `/gsd-verify-work`:** full suite green, every spike file re-run green from the final tree (02-11 Task 2), evidence consolidated, then `cargo clean` (GATE-06)
- **Max feedback latency:** 180 seconds for task-level verify commands

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | SPIKE-01, SPIKE-04 | T-02-01, T-02-05 | musl table keeps the forbid backstop (B-MUSL); lock gains edges only | config probe + build | `grep -qxF 'targets = ["aarch64-unknown-linux-musl"]' rust-toolchain.toml && test "$(awk '/^\[target\.aarch64-unknown-linux-musl\]/,0' .cargo/config.toml \| grep -c rustflags)" = 0 && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings && file target/aarch64-unknown-linux-musl/release/minos-guest \| grep -q 'statically linked' && grep -q '^### B-MUSL' evidence/02-01.md` | ✅ | ⬜ pending |
| 02-01-02 | 01 | 1 | SPIKE-01, SPIKE-04 | T-02-06, T-02-07 | only libkrunfw installed from the tap, canonical path and SHA-256 recorded; D-18 kernel config line recorded; no engine started | host check + evidence grep | `F=evidence/02-01.md; for s in H-01 H-02 H-03; do grep -q "^### $s" "$F" \|\| exit 1; done; test -e /opt/homebrew/lib/libkrunfw.5.dylib && grep -q 'CONFIG_BLK_DEV_INITRD' "$F" && test "$(brew list --formula \| grep -cE '^(krunkit\|krunvm\|libkrun)$')" = 0` | ✅ | ⬜ pending |
| 02-01-03 | 01 | 1 | SPIKE-01 | T-02-02, T-02-03 | job header and JSON validated with caps; golden frame bytes | unit (TDD) | `cargo test -p minos-guest && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-01-04 | 01 | 1 | SPIKE-01 | T-02-03, T-02-04 | PID 1 prints marker first, sends one frame, powers off; Linux code isolated | build (musl) + clippy | `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl && cargo clippy --workspace --all-targets -- -D warnings` | ✅ | ⬜ pending |
| 02-02-01 | 02 | 1 | SPIKE-07 | T-02-08, T-02-09, T-02-10, T-02-11 | canonical binary, empty cwd, six-variable env; PII and cost redacted | manual CLI + evidence grep | `grep -q '^### C-03' evidence/02-02.md && ! grep -nE '"(total_cost_usd\|costUSD)": *[0-9]' evidence/02-02.md` | ✅ | ⬜ pending |
| 02-02-02 | 02 | 1 | SPIKE-07 | T-02-12, T-02-13 | at most 10 model calls; contract table cites evidence | manual CLI + evidence grep | `grep -q '^### SPIKE-07 contract' evidence/02-02.md` | ✅ | ⬜ pending |
| 02-03-01 | 03 | 2 | SPIKE-01 | T-02-21 | launch config rejects every E-2 violation; canonical text parses | unit (TDD) | `cargo test -p minos-vmm config` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-03-02 | 03 | 2 | SPIKE-01, SPIKE-04 | T-02-17, T-02-18, T-02-20 | preflight refuses bad files and MSB_KRUN_/DYLD_ env; watchdog codes; exit 64/65/66 recorded | build + smoke run | `cargo build -p minos-vmm && cargo clippy -p minos-vmm --all-targets -- -D warnings && { target/debug/minos-vmm; test $? -eq 64; }` | ✅ | ⬜ pending |
| 02-03-03 | 03 | 2 | SPIKE-02 | T-02-14, T-02-15, T-02-16 | 13 dangerous msb_krun methods banned and each proven to fire | gate probe | `cargo clippy -p minos-vmm --all-targets -- -D warnings && git diff --quiet -- crates/minos-vmm/src` | ✅ | ⬜ pending |
| 02-04-01 | 04 | 2 | SPIKE-06, SPIKE-01 | T-02-22 to T-02-26 | hostile tar parsed in memory under caps; /init injected last; deterministic; unpack and clock banned | unit (TDD) + gate probe | `cargo test -p minos-backend-krun rootfs && cargo clippy -p minos-backend-krun --all-targets -- -D warnings` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-04-02 | 04 | 2 | SPIKE-01 | T-02-26 | newc archive holds only /init, zeroed metadata, golden hash, cpio agrees | unit (TDD) + tool cross-check | `cargo test -p minos-backend-krun initramfs` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-04-03 | 04 | 2 | SPIKE-06 | T-02-27, T-02-28 | digest-pinned base, context holds only the Dockerfile; two builds compared | integration (ignored) | `cargo test -p minos-backend-krun --test spike_rootimg --no-run` (run: `-- --ignored produce_twice`) | ❌ W0 (created in task) | ⬜ pending |
| 02-05-01 | 05 | 2 | SPIKE-01 | T-02-34 | host config and job disk match the vmm and guest golden bytes | unit (TDD) | `cargo test -p minos-backend-krun launch_config && cargo test -p minos-backend-krun input_disk` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-05-02 | 05 | 2 | SPIKE-03 | T-02-29 to T-02-35 | env_clear, null stdin, own process group, per-connection cap plus overflow flag, host deadline, sanitized excerpt | unit (TDD, no VM) | `cargo test -p minos-backend-krun supervise` | ❌ W0 (tests written first in task) | ⬜ pending |
| 02-06-01 | 06 | 3 | SPIKE-01, SPIKE-04, SPIKE-06 | T-02-38, T-02-40 | boot tests check entitlement first, assert exact cmdline and no virtio-fs device | integration (ignored), compile | `cargo test -p minos-backend-krun --tests --no-run && cargo clippy -p minos-backend-krun --all-targets -- -D warnings` | ❌ W0 (created in task) | ⬜ pending |
| 02-06-02 | 06 | 3 | SPIKE-01, SPIKE-04, SPIKE-06 | T-02-36, T-02-37, T-02-39, T-02-41 | unsigned refused, signed boots, minimal entitlements, initramfs negative, Dockerfile image boots; exactly one S08 heading | integration (ignored; the action runs `spike_boot` and `boot_from_dockerfile` with `--ignored`) + evidence grep | `F=evidence/02-06.md; for s in S04-UNSIGNED S04-SIGN K5 S01-ROOTDISK; do grep -q "^### $s" "$F" \|\| exit 1; done; grep -qE '^### S08: SPIKE-08 (not triggered\|triggered)$' "$F" && test "$(grep -c '^### S08' "$F")" = 1 && plutil -lint crates/minos-vmm/minos-vmm.entitlements && cargo clippy -p minos-backend-krun --all-targets -- -D warnings` | ✅ (after 02-06-01) | ⬜ pending |
| 02-06-03 | 06 | 3 | SPIKE-08 | T-02-36 | no alternative tried before the user answers | checkpoint:decision (conditional) | `F=evidence/02-06.md; grep -qE '^### S08: SPIKE-08 (not triggered\|decision)' "$F" && test "$(grep -c '^### S08' "$F")" = 1` | ✅ | ⬜ pending |
| 02-07-01 | 07 | 4 | SPIKE-01 | T-02-48 | orphaning without a shell; signals via argv | integration (ignored), compile | `cargo test -p minos-backend-krun --test spike_exits --no-run` | ❌ W0 (created in task) | ⬜ pending |
| 02-07-02 | 07 | 4 | SPIKE-01 | T-02-43 to T-02-47 | exit 0 for power-off and init exit; 124/125/126 watchdog; 66/67 refusals; no helper left | integration (ignored; the action runs `spike_exits` with `--ignored`) + evidence grep | `F=evidence/02-07.md; for s in K7-00 K7-01 K7-02 K7-03 K7-04 K7-05 K7-06 K7-07 K7-08 K7-09 K7-10 K7-11 'K7 table' 'Classification rule for Phase 5'; do grep -q "^### $s" "$F" \|\| exit 1; done; ! /usr/bin/pgrep -x minos-vmm` | ✅ (after 02-07-01) | ⬜ pending |
| 02-08-01 | 08 | 4 | SPIKE-02, SPIKE-03 | T-02-49, T-02-50 | egress probes and canaries; capped stream tests | integration (ignored), compile | `cargo test -p minos-backend-krun --test spike_net --no-run` | ❌ W0 (created in task) | ⬜ pending |
| 02-08-02 | 08 | 4 | SPIKE-02, SPIKE-03 | T-02-49 to T-02-53 | every connect fails, canaries see 0; cap enforced; LAN IP redacted | integration (ignored; the action runs `spike_net` with `--ignored`) + evidence grep | `F=evidence/02-08.md; for s in S02-EGRESS S03-WITHIN S03-OVER K4-THROUGHPUT 'SPIKE-02 and SPIKE-03 answers'; do grep -q "^### $s" "$F" \|\| exit 1; done; ! grep -nE '(^\|[^0-9])(10\|172\.(1[6-9]\|2[0-9]\|3[01])\|192\.168)\.[0-9]+\.[0-9]+' "$F"` | ✅ (after 02-08-01) | ⬜ pending |
| 02-09-01 | 09 | 4 | SPIKE-05 | T-02-54, T-02-56 | deny-default profile with canonical parameters | integration (ignored), compile | `cargo test -p minos-backend-krun --test spike_seatbelt --no-run && grep -q '^(deny default)' crates/minos-vmm/minos-vmm.sb` | ❌ W0 (created in task) | ⬜ pending |
| 02-09-02 | 09 | 4 | SPIKE-05 | T-02-55, T-02-57, T-02-58 | allowances only from logged denials; profile denies net and unlisted files; no user created | integration (ignored; the action runs `spike_seatbelt` with `--ignored`) + denial log + evidence grep | `F=evidence/02-09.md; for s in S05-ITER-1 S05-DENY-NET S05-DENY-FILE S05-RECIPE 'SPIKE-05 answer'; do grep -q "^### $s" "$F" \|\| exit 1; done; ! grep -nE '\(allow network\*\|\(allow default' crates/minos-vmm/minos-vmm.sb && ! dscl . -read /Users/_minos >/dev/null 2>&1 && cargo clippy -p minos-backend-krun --all-targets -- -D warnings` | ✅ (after 02-09-01) | ⬜ pending |
| 02-10-01 | 10 | 5 | DOCS-05 | T-02-59 | fragments consolidated verbatim (hash-checked) | evidence check | `grep -qx '<!-- fragments-start -->' 02-SPIKE-EVIDENCE.md && test ! -d evidence` | ✅ | ⬜ pending |
| 02-10-02 | 10 | 5 | DOCS-05, SPIKE-01, SPIKE-08 | T-02-60, T-02-61, T-02-63 | decision and superseded wording in one commit; research unchanged | doc grep | `grep -c 'Superseded wording (2026-10-09)' ../../REQUIREMENTS.md` prints 3; `git diff --quiet {BASE} -- .planning/research` | ✅ | ⬜ pending |
| 02-10-03 | 10 | 5 | DOCS-05 | T-02-60, T-02-62 | both CLAUDE files corrected together; overturned claims listed | doc grep | `! grep -q 'MEDIUM (compiles; not yet booted)' .claude/CLAUDE.md && grep -q '^## Research claims overturned (D-09)' 02-SPIKE-EVIDENCE.md` | ✅ | ⬜ pending |
| 02-11-01 | 11 | 6 | DOCS-05 | T-02-65, T-02-66 | every D-07 amendment present; AWS content kept (D-08); no overclaim | doc grep checklist | E-10 checklist greps (`ModelProvider`, `libkrun/krun`, `inet_hijack`, `rust-lld`, codesign line, `tokio`, `pico-dollar`, `minos-loop`, local and AWS headings; `nano` count 0) | ✅ | ⬜ pending |
| 02-11-02 | 11 | 6 | DOCS-05, SPIKE-08 | T-02-64, T-02-67, T-02-68 | final gate green; no unsafe exception vs {BASE}; spikes re-run green; cargo clean last | full suite + invariant + VM re-run | full suite command, C-01, C-02, no-unsafe grep, spike re-run, then `cargo clean` and `test ! -d target` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Rows 02-06-02, 02-07-02, 02-08-02, and 02-09-02: the `--ignored` VM runs happen inside the task action, which records each run in the plan's evidence fragment, and again in the plan's `<verification>` re-run and in 02-11 Task 2. The Automated Command column is the task's `<verify>` block, which checks the evidence those runs produced and boots no VM itself.

---

## Wave 0 Requirements

No separate Wave 0 plan: the Rust test harness already exists (Phase 1), and each test scaffold is created by the task that needs it, before the code it checks (TDD tasks write their failing tests first; spike tasks write the ignored test, compile it with `--no-run`, then run it).

- [ ] `crates/minos-guest/src/{job,report,pattern}.rs` unit tests — 02-01-03
- [ ] `crates/minos-vmm/src/config.rs` unit tests — 02-03-01
- [ ] `crates/minos-backend-krun/src/rootfs.rs` and `initramfs.rs` unit tests with golden vectors — 02-04-01, 02-04-02
- [ ] `crates/minos-backend-krun/src/launch_config.rs`, `input_disk.rs`, `supervise.rs` unit tests — 02-05-01, 02-05-02
- [ ] `crates/minos-backend-krun/tests/spike_rootimg.rs` — 02-04-03 (boot test added in 02-06-01)
- [ ] `crates/minos-backend-krun/tests/spike_boot.rs` — 02-06-01
- [ ] `crates/minos-backend-krun/tests/spike_exits.rs` — 02-07-01
- [ ] `crates/minos-backend-krun/tests/spike_net.rs` — 02-08-01
- [ ] `crates/minos-backend-krun/tests/spike_seatbelt.rs` — 02-09-01
- [ ] `rust-toolchain.toml` musl target and `.cargo/config.toml` linker — 02-01-01 (musl std already installed during research)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `claude -p` flag matrix under subscription login | SPIKE-07 | Spends the user's subscription and depends on their login; not a repeatable unit test (the provider's golden argv test is Phase 6) | 02-02 Tasks 1 and 2: run C-01 to C-11 from fresh empty directories with the six-variable environment; record redacted output |
| Signing state of the helper (unsigned versus signed) | SPIKE-04 | `codesign` acts on a build artifact that every rebuild resets; the refusal and the boot are then automated in `spike_boot` | 02-06 Task 2 steps 1 to 3: copy before signing, `codesign -dvv`, sign with E-4, `codesign --display --entitlements -` |
| Seatbelt denial iteration | SPIKE-05 | Reading `log show` denials and choosing the narrowest allowance is judgment; the resulting profile is then tested automatically | 02-09 Task 2: run, read denials, add one commented allowance, repeat (at most eight iterations) |
| Reproducibility of two Docker builds | SPIKE-06 (D-16) | Recorded, not required; the cause of any difference is investigated by reading `tar -tvf` listings | 02-04 Task 3 step 5 |
| SPIKE-08 decision if the root-disk boot fails | SPIKE-08 | The user decides; no option may be tried first | 02-06 Task 3 checkpoint:decision |
| Accuracy of CLAUDE.md prose | DOCS-05 | The checklist greps prove presence, not that each sentence matches the evidence | Review 02-10 and 02-11 edits against the cited 02-SPIKE-EVIDENCE.md sections during `/gsd-verify-work` |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 180s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
