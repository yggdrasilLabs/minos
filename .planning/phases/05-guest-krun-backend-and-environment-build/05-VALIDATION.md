---
phase: 5
slug: guest-krun-backend-and-environment-build
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-10-18
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + insta 1.49.0 (`.config/insta.yaml` `update: no`) + proptest 1.11.0 + trybuild 1.0.122 |
| **Config file** | `.config/insta.yaml`; per-crate `clippy.toml` |
| **Quick run command** | `cargo test -p <touched crate>` plus `cargo clippy -p <touched crate> --all-targets -- -D warnings` (guest changes also run the musl clippy command from CLAUDE.md Commands) |
| **Full suite command** | The six gate commands in CLAUDE.md Commands, then the VM suite: build `minos-vmm`, sign it with the documented `codesign` argv, build `minos-guest` for `aarch64-unknown-linux-musl`, then `cargo test -p minos-backend-krun --test <name> -- --ignored --nocapture --test-threads=1` with `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW` set |
| **Estimated runtime** | ~120 seconds for the host gate; VM tests add about 30 seconds each |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched crate>` and `cargo clippy -p <touched crate> --all-targets -- -D warnings`
- **After every plan wave:** Run the six gate commands, then the VM suite after build, sign, and guest build
- **Before `/gsd-verify-work`:** Full suite must be green; the three permanent exit tests (`exit_egress`, `exit_env`, `exit_kill9`) pass and are recorded in `05-EVIDENCE.md`; the C-02 grep prints nothing; then `cargo clean` (GATE-06)
- **Max feedback latency:** 180 seconds
- **VM-suite tasks (05-17, 05-19, 05-20):** each task's `<verify>` runs one named ignored test after build and sign (`cargo test -p minos-backend-krun --test <vm|exit> -- --ignored --exact <test> --nocapture --test-threads=1`), which stays inside the latency target; the full VM and exit suites take minutes and are phase-end gates, listed in each plan's acceptance criteria and run in full by 05-20's final gate

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| *(filled by the planner from 05-RESEARCH.md "Phase Requirements → Test Map")* | | | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/minos-backend-krun/tests/wave0_chunks.rs` — chunked output throughput and backlog at the 16 MiB cap (S03-ANSWER measurement)
- [ ] `crates/minos-backend-krun/tests/wave0_console_pipe.rs` — `/dev/fd/1` console under the deny-default Seatbelt profile, exact cap
- [ ] `crates/minos-backend-krun/tests/wave0_store_root.rs` — root image read from a store path with ancestor metadata under Seatbelt
- [ ] A probe fixture binary (second `[[bin]]` in `minos-guest`, never packed into user images) that the exit tests run as the candidate: egress connects, env and cmdline dump, forged chunk, write outside scratch. **Relocated:** it lands in 05-12 (wave 5), not in 05-03, because the Phase 2 spike guest that 05-03's proofs run has no candidate stage to run it in (05-03 objective); no test needs it before 05-17
- [ ] `05-EVIDENCE.md` skeleton
- [ ] Framework install: none; existing infrastructure covers everything else

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Signed helper carries both entitlement keys | KRUN-08 | `codesign` runs outside cargo | `codesign --display --entitlements - target/debug/minos-vmm` lists `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation` |
| `minos host doctor` output wording | CLI-03 | Human reads refusal and fix text | Run `minos host doctor` on this Mac; every refusal names a fix |
| `minos env build` against Docker | KRUN-09, CLI-02 | Needs the Docker daemon running | Start Docker Desktop, run the ignored `env_build` test twice, assert equal image digests |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
