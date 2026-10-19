---
phase: 05-guest-krun-backend-and-environment-build
verified: 2026-10-19T22:00:00Z
status: passed
score: 5/5 roadmap success criteria verified (17/17 requirements satisfied; GATE-06 cross-cutting, unchecked by design)
behavior_unverified: 0
overrides_applied: 0
re_verification: none (initial verification)
gaps: []
deferred: []
human_verification: []
verification_method: >
  No cargo command was run (hard rule: Phase 5 ended with `cargo clean`, `target/` is intentionally gone,
  confirmed absent). Verified from the recorded evidence (05-EVIDENCE.md "## Final gate"), the source and test
  source, deny.toml / clippy.toml, REQUIREMENTS.md, ROADMAP.md, CLAUDE.md, and git history. Behavior-dependent
  truths are marked "verified from evidence" with the section cited. Read-only greps (C-02, debt markers,
  unwrap/expect) were run live.
---

# Phase 5: Guest, krun Backend, and Environment Build Verification Report

**Phase Goal:** An experiment runs inside a libkrun microVM on this Mac with no network path, no host environment, read-only disk-image inputs, and a capped tar stream out, and no VM outlives the process that launched it.
**Verified:** 2026-10-19
**Status:** passed
**Re-verification:** No, initial verification

## Evidence base

- Code freshness: the final gate ran at HEAD `4cc176a`. `git diff --stat f791e2b..HEAD -- crates Cargo.toml Cargo.lock deny.toml` is empty, and `git diff --stat 4cc176a..HEAD` touches only `.planning/` files. So the recorded gate output describes the code now in the tree.
- `target/` does not exist (`ls target` fails), consistent with `cargo clean` as the last command (05-EVIDENCE.md "### cargo clean (GATE-06)").
- Final gate section (05-EVIDENCE.md line 1428 onward) contains, in order: the six gate commands (fmt, clippy, test, deny, doc, musl guest clippy), C-01 (11 ok lines), C-02 (empty), build and sign, `minos host doctor` (exit 0), `exit_env` smoke, the `vm` suite (10 passed), the `exit` suite (4 passed, twice, with and without `--nocapture`), the Docker reproducibility test (1 passed, equal digests `41cfea1b...9276b`), then `cargo clean` (exit 0, last). `cargo test --workspace` totals: 1139 passed, 0 failed, 19 ignored; the ignored set is the VM suites (exit 4, vm 10), the Docker test and hand-run fixtures.

## Goal Achievement: Roadmap Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1a | Guest egress fails closed (loopback and public IP, v4 and v6) | VERIFIED (from evidence) | `exit_egress` (tests/exit.rs:179-247) asserts every connect fails with an OS errno, both host canaries (127.0.0.1, [::1]) see no connection, guest interfaces are `lo` only, and no forbidden virtio device. Final gate: 127.0.0.1 and 1.1.1.1 and 8.8.8.8 errno 101, [::1] and 2606:4700:4700::1111 errno 99, ifaces `["lo"]`, six virtio devices (rng, console, 2x blk, vsock, msb vmgenid). Structural backstop: minos-vmm `clippy.toml` bans `enable_inet_hijack`, `inet_hijack`, `tcp_listen_remap`, `FsBuilder::*`, `ExecBuilder::env(s)`, custom console/vsock backends; `launch.rs` builds only two RO raw disks, one `unix_connect` vsock route, explicit rng, no `net` feature. |
| 1b | No host values in the guest env or /proc/cmdline (one recorded exception) | VERIFIED (from evidence), with the recorded exception | `exit_env` (tests/exit.rs:263-343) asserts the stage env equals exactly `STAGE_ENV`, `/proc/cmdline` equals an 18-token list token for token, exactly one `KRUN_` token which must be `KRUN_BOOT_START_NS=<ASCII digits>`, and that a per-run canary, host HOME and host USER appear nowhere in the dump. Final gate: ok. Exception stated in REQUIREMENTS.md, CLAUDE.md, and the 05-19 SUMMARY (see Known Facts). |
| 1c | `kill -9` of the launcher leaves no VM or helper | VERIFIED (from evidence) | `exit_kill9` (tests/exit.rs:433-528) spawns a real launcher child on the production `KrunBackend`, SIGKILLs only the launcher, asserts the helper pid disappears within the bound, `ps` shows no process naming the job dir, and a new backend's sweep reports `Removed{dir}` and the dir is gone. Final gate: helper exited 279.6 ms after SIGKILL; sweep removed the dir; no `/private/tmp/mvt*` left. Mechanisms present in source: `watchdog.rs` (parent-gone and deadline trips, 50 ms poll, exit 126/124), `supervise.rs` (`process_group(0)`, `kill_process_group`, `waitid(WNOWAIT)`), `jobdir` owner lock + pid file + sweep. |
| 1d | A candidate cannot forge the output stream | VERIFIED (from evidence) | `exit_forge` (tests/exit.rs:553-598) asserts the candidate cannot open `/dev/vdb` or `/proc/1/environ`, a probe stream under a guessed token ends `Ended::ChunkProtocol`, and PID 1's genuine judge stream is accepted (control: scorecard `probe_ok` = 1). Final gate: `candidate: ended ChunkProtocol`, then the judge control ok. Source: 32-byte token from `/dev/urandom` (input_disk.rs `new_token`, character-device check), written only in the input disk header; `linux.rs` unlinks `/dev/vdb` (fallback chmod 0) after PID 1 reads it; `minos_core::chunk` is the shared codec; `classify` maps chunk faults to `ChunkProtocol`, never an infra retry. |
| 2 | Each VM runs in its own signed `minos-vmm` helper: cleared env, stdin /dev/null, capped console pipe; only binary linking the VMM; implicit vsock off, plain vsock; clippy bans; process-group kill at hard max duration | VERIFIED | `supervise.rs:320-324` `.env_clear()`, `.stdin(Stdio::null())`, `.process_group(0)`; console is the helper stdout pipe read through cap+1. `deny.toml` bans `msb_krun` except wrapper `minos-vmm`; `minos-backend-krun/Cargo.toml` has no msb_krun dependency. `minos-vmm.entitlements` carries exactly `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`; final gate `codesign --display` lists both. Deadline: watchdog code 124 plus host deadline in `classify`. |
| 3 | Two sequential experiments: static musl PID 1, shared RO root + per-job RO input disk, strict diff apply (mismatch is `Rejected`), non-root uid under rlimits writing only to capped in-memory scratch, byte-capped tar over vsock, no shared build output, only judge channel yields a scorecard | VERIFIED (from evidence) | `vm` suite 10/10 in the final gate: `experiment_applies_diff_and_runs_as_candidate_uid` (euid/egid 64000, no groups), `apply_mismatch_is_rejected_and_runs_nothing`, `two_sequential_experiments_share_no_build_output` (marker exists in run 1, absent in run 2), `writes_outside_scratch_fail` (errno 30/13 outside, ok only in /tmp and /out), `over_budget_candidates_are_stopped_in_the_vm` (RLIMIT_AS 764 of 832 MiB, RLIMIT_NPROC 511 of 2000, scratch 128 MiB cap fills), `output_16_mib_is_reassembled`, `output_over_cap_is_refused_by_the_guest`, `candidate_output_never_carries_a_scorecard`, `judge_role_returns_a_scorecard`, `stage_timeout_is_reported_by_the_guest`. Source: `linux.rs` (PID 1: waitpid reaping, `reboot(PowerOff)`, trampoline refuses to run as PID 1/root, setrlimit AS/FSIZE/NPROC/CPU/NOFILE/CORE=0, env_clear, uid/gid 64000), `stage.rs`, `workspace.rs`, `pack.rs`, clippy bans on `tar::*unpack*`. Disks: launch.rs attaches only `read_only(true)` raw disks; no scratch disk exists. |
| 4 | `minos env build` -> raw RO SquashFS root image with `/init` written by the trusted packer, digests in `minos.lock`, context with no judge data/secrets; `minos host doctor` prints each refusal with a fix; layers recorded in `BackendInfo`, no silent fallback | VERIFIED | `env_build.rs` pipeline (snapshot with denylist and secret scan, digest-pinned FROM, static aarch64 ELF check, packer writes guest last, `put_reader`, lock written after blob, `--update` gate). `tests/env_build.rs`: 10 pass + 1 ignored Docker test (passed in final gate; both builds digest `41cfea1b...9276b`, lock printed). Named tests: `context_has_no_judge_or_secrets`, `secret_blocks_build`, `wrong_guest_refused`, `tagged_base_image_refused`, `image_has_init_and_mount_points`, `changed_image_refused`. `main.rs` wires `host doctor`, `host setup`, `env build`. Final-gate doctor: exit 0, nine named checks each with `checked:` paths, `confinement: hardware, seatbelt, rlimits`, `dedicated user: missing (deferred, reduced hardening)`. `backend.rs` refuses launch without Seatbelt and records layers `{Seatbelt, Rlimits}` into `BackendInfo`. |
| 5 | `cargo clean` run after verification passes (GATE-06) | VERIFIED as executed; see Process note | 05-EVIDENCE.md "### cargo clean (GATE-06)": exit 0, "Removed 165992 files, 17.3GiB". `target/` absent now. REQUIREMENTS.md leaves GATE-06 unchecked by design (cross-cutting). |

**Score:** 5/5 roadmap success criteria verified. Behavior-dependent truths (1a to 1d, 3) are carried as VERIFIED because a named passing test exists and is recorded on the final build; I could not re-run them under the no-cargo rule, so they are "verified from evidence". `behavior_unverified: 0`.

## Known Facts (explicit checks requested)

| Fact | Result |
|------|--------|
| GUEST-05 exception stated in REQUIREMENTS.md | PRESENT. Line 75, directly under GUEST-05 (line 74, marked `[x]`): "Allowance, a recorded exception (user decision at the 05-19 checkpoint, 2026-10-19): ... `KRUN_BOOT_START_NS=<digits>`, which msb_krun 0.1.41 always appends; exit_env allows exactly that key with an all-digit value and fails on any other host value. Removing it is a CLAUDE.md open question." |
| GUEST-05 exception stated in CLAUDE.md | PRESENT in three places: Local backends boot bullet (line 413: exact token list, mechanism `build_kernel_cmdline` unconditional from `SystemTime::now()`, no opt-out, 0.1.41 newest release), Testing (line 473: `exit_env` "with the one `KRUN_BOOT_START_NS` exception"), and Open questions (line 533: removal path). |
| GUEST-05 exception stated in the 05-19 SUMMARY | PRESENT (lines 15, 18, 32, 45, 49-59, 77, 95, 98). Records the decision history: first answer no-allowance, then the user's 2026-10-19 decision to allow the one token, with the proven mechanism. |
| Test enforces the exception narrowly | `is_boot_start` (exit.rs:249) accepts only `KRUN_BOOT_START_NS=` plus one or more ASCII digits; the 18-token list is otherwise exact; exactly one `KRUN_` token; canary, HOME, USER absent. The token also reaches PID 1's environment but PID 1 clears it before any stage (the stage env equals exactly `STAGE_ENV`). |
| Final gate recorded verbatim in 05-EVIDENCE.md `## Final gate` | CONFIRMED at line 1428; contents and order as listed in Evidence base. |
| GATE-06 unchecked by design | CONFIRMED: REQUIREMENTS.md line 17 `[ ]`, traceability row 202 "All phases (cross-cutting) / Pending", total line 298 notes it is cross-cutting. |
| SPIKE-03 as the user left it | CONFIRMED unchanged: line 25 `[x]`, the 2026-10-10 reworded chunk-protocol text, traceability row 205 "Phase 2 / Complete". Phase 5 did not touch it. |

## Required Artifacts

| Artifact | Expected | Status |
|----------|----------|--------|
| `crates/minos-guest/src/{linux,stage,workspace,pack,run,sink,source,nofollow}.rs`, `src/bin/minos-probe.rs` | PID 1, trampoline, unpack/apply, output packer, run orchestration, probe fixture | VERIFIED (exists, substantive, wired; spike `job.rs`/`report.rs` removed per 05-12) |
| `crates/minos-vmm/src/{main,config,launch,watchdog,preflight,log}.rs`, `clippy.toml`, `minos-vmm.entitlements` | Helper, launch config v2, bans, entitlements | VERIFIED |
| `crates/minos-backend-krun/src/{backend,supervise,jobdir,host,input_disk,chunks,rootfs,seatbelt,launch_config}.rs`, `minos-vmm.sb` | Backend, K7 classifier, job dirs and sweep, probes, input disk writer, chunk receiver, packer, embedded profile | VERIFIED. Profile SHA-256 `cda08b0f...8490` matches the pin in `seatbelt.rs:149`; only network allowance is the AF_UNIX connect to `SOCK`; profile lives at the new path and the old `crates/minos-vmm/*.sb` is gone. |
| `crates/minos-backend-krun/tests/{vm,exit}.rs`, `tests/harness/mod.rs` | Permanent VM and exit tests | VERIFIED (10 and 4 ignored tests, all named in the final gate) |
| `crates/minos-cli/src/{main,host,env_build,builder,locate,blob_source}.rs`, `tests/{env_build,host_cmd}.rs` | `host doctor/setup`, `env build`, locator | VERIFIED |
| `minos-core` `chunk`, `guest`, `lock`, `limits`, `BlobHasher`; `minos-store` `put_reader`, `verified_path`; `minos-sandbox` `BlobSource`, `Finished`, `Ended::ChunkProtocol`, `take_status` | Shared codecs and types | VERIFIED via gate test counts (chunk 29, guest_wire 41, lock 14, blob_hasher 4, sandbox backend/untrusted/output_tar suites) |
| `deny.toml` layering | `msb_krun` wrapper `minos-vmm`; tokio only via `msb-imago`; per-crate wrappers; `minos-backend-krun` wrapper `minos-cli` only | VERIFIED; `cargo deny check` "advisories ok, bans ok, licenses ok, sources ok" in the final gate |

## Key Link Verification

| From | To | Via | Status |
|------|----|-----|--------|
| `minos-cli` | `KrunBackend` | `FileBlobSource` implements `BlobSource`; backend fetches plan blobs and root image path through it, never links `minos-store` | WIRED (deny.toml and Cargo.toml: backend-krun depends on core and sandbox only) |
| `KrunBackend::launch` | `minos-vmm` helper | `seatbelt::argv` -> `sandbox-exec -p <embedded profile>`; `env_clear`, null stdin, `process_group(0)` | WIRED |
| input disk token | chunk receiver | `new_token` -> disk header -> `minos_core::chunk` codec on both ends | WIRED (proved by `exit_forge` control and `vm` output tests) |
| `Finished` -> runner | `Ended::ChunkProtocol` / `take_status` | host classification in `supervise::classify` | WIRED |
| `env build` -> store -> `minos.lock` | `put_reader`, `verified_path`, `Lock` | lock written after blob stored, atomic rename | WIRED |
| `host doctor` | backend probes | same `Capabilities` probes the launch uses | WIRED |

## Requirements Coverage

Every ID in the phase's requirements list appears in at least one PLAN frontmatter `requirements:` field, in REQUIREMENTS.md as `[x]`, and in the traceability table as "Phase 5 / Complete". The ROADMAP Phase 5 requirement list is the same 17 IDs. No orphans.

| Requirement | Plans claiming | Status | Evidence |
|-------------|----------------|--------|----------|
| GUEST-01 PID 1, reaps, nothing per-run at startup | 05-12, 05-17 | SATISFIED | `linux.rs` waitpid reaping; `vm` suite boots and runs two sequential jobs; probe fixture |
| GUEST-02 job from RO input disk, core applier, `Rejected` on mismatch | 05-04, 05-08, 05-10, 05-17 | SATISFIED | `apply_mismatch_is_rejected_and_runs_nothing`; `guest_wire` suite; `workspace.rs` uses core applier |
| GUEST-03 argv commands, non-root uid, rlimits, scratch only | 05-01, 05-04, 05-08, 05-10, 05-12, 05-17 | SATISFIED | uid 64000, `RLIMIT_AS/NPROC/FSIZE/CPU/NOFILE/CORE`, `over_budget_candidates_are_stopped_in_the_vm`, `writes_outside_scratch_fail` |
| GUEST-04 tar over vsock, byte-capped, scorecard only from judge channel | 05-02, 05-03, 05-04, 05-08, 05-10, 05-13, 05-17, 05-19, 05-22 | SATISFIED | `output_16_mib_is_reassembled`, `output_over_cap_is_refused_by_the_guest`, `candidate_output_never_carries_a_scorecard`, `judge_role_returns_a_scorecard`, `exit_forge`; `take_status` / `into_scorecard` (judge only) |
| GUEST-05 no host variables in env or /proc/cmdline | 05-12, 05-19, 05-20 | SATISFIED WITH ONE RECORDED EXCEPTION | `exit_env`; exception documented in all three required places; user decision 2026-10-19 |
| KRUN-01 own helper, cleared env, stdin null, capped console pipe, sole VMM linker | 05-03, 05-07, 05-09, 05-15, 05-21, 05-22 | SATISFIED | supervise.rs; deny.toml; entitlements |
| KRUN-02 implicit vsock off, plain vsock, clippy bans | 05-07 | SATISFIED | launch.rs, clippy.toml, `exit_egress` |
| KRUN-03 permanent egress test | 05-19, 05-20 | SATISFIED | `exit_egress` |
| KRUN-04 RO root + RO input disk, in-memory scratch, no shared dir | 05-01, 05-03, 05-07, 05-09, 05-13, 05-15, 05-17, 05-21 | SATISFIED | launch.rs (two RO raw disks), no virtio-fs (clippy ban), `writes_outside_scratch_fail` |
| KRUN-05 hard max duration, watchdog, sweep, kill -9 leaves nothing | 05-11, 05-15, 05-19, 05-20 | SATISFIED | `exit_kill9`, watchdog.rs, jobdir sweep, `stage_timeout_is_reported_by_the_guest` |
| KRUN-06 preflight checks with reasons | 05-06, 05-11, 05-14 | SATISFIED | doctor output lists helper, entitlements, kernel hash, hypervisor, socket path, sandbox-exec, both Seatbelt denial probes, guest ELF |
| KRUN-07 layers recorded, no silent fallback | 05-06, 05-07, 05-11, 05-15 | SATISFIED | `backend.rs` refuses without Seatbelt; `BackendInfo` layers; doctor prints `dedicated user: missing (deferred, reduced hardening)` |
| KRUN-08 sign with the one documented argv, verify with codesign | 05-11, 05-14 | SATISFIED | CLAUDE.md Commands argv; `host setup`; final gate re-checked both keys before every VM run |
| KRUN-09 `env build` root image, `/init` by packer, lock, clean context | 05-01, 05-05, 05-09, 05-16, 05-18 | SATISFIED | env_build.rs and tests; Docker test equal digests |
| KRUN-10 two sequential experiments share no build output | 05-17 | SATISFIED | `two_sequential_experiments_share_no_build_output` |
| CLI-02 `env build` writes `minos.lock` | 05-02, 05-16, 05-18 | SATISFIED | `first_build_writes_lock`, `same_build_keeps_lock`, Docker test lock output |
| CLI-03 `host doctor` prints every refusal with explanation and fix | 05-14 | SATISFIED | `host_cmd` tests; doctor exit 0 (no refusal path exercised live; refusal-with-fix wording covered by `refusals` unit tests in the gate) |
| GATE-06 (cross-cutting, plan 05-20) | 05-20 | EXECUTED; unchecked by design | `cargo clean` recorded; requirement stays Pending |

## Behavioral Spot-Checks (read-only, run live)

| Check | Command | Result | Status |
|-------|---------|--------|--------|
| C-02 no `allow` attributes | `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | no output | PASS |
| Debt markers in crates | `grep -rnE '\b(TBD\|FIXME\|XXX)\b' crates --include='*.rs'` | no output | PASS |
| `unwrap()` / `.expect(` in crates | `grep -rn 'unwrap()\|\.expect(' crates --include='*.rs'` | no output | PASS |
| Seatbelt profile pin | `shasum -a 256 crates/minos-backend-krun/src/minos-vmm.sb` vs `seatbelt.rs:149` | both `cda08b0f...8490` | PASS |
| Code unchanged since final-gate build | `git diff --stat f791e2b..HEAD -- crates Cargo.toml Cargo.lock deny.toml` | empty | PASS |
| No cargo command | not run, per hard rule | n/a | SKIPPED by instruction |

## Probe Execution

No `scripts/*/tests/probe-*.sh` exists and no plan declares a shell probe (the repo forbids scripts; probes are the ignored Rust tests). Step 7c: not applicable. The Rust equivalents (`exit_*`, `vm`, `docker_env_build_reproduces`) are covered under verified-from-evidence above.

## Anti-Patterns Found

None blocking. Grep for debt markers, `allow` attributes, `unwrap`/`expect` over `crates/` is clean. All 22 SUMMARYs report "Known Stubs: None" or a justified non-stub note (05-12's `X` placeholders are a deliberate path-length measurement). 05-20 recorded two evidence-only deviations, "No code changed. No assertion was weakened."

## Findings

No BLOCKER. Warnings and notes (none change the verdict):

1. **INFO, GUEST-05 exception scope.** ROADMAP.md Phase 5 Success Criterion 1 text ("no host values") and the phase goal do not themselves mention the `KRUN_BOOT_START_NS` exception; only the plan-list line 305 does. The required three documents (REQUIREMENTS.md, CLAUDE.md, 05-19 SUMMARY) all state it, so this is not a gap. Consider one clause in SC1 so a reader of the roadmap alone is not misled. The exception is a host wall-clock value in the guest's kernel command line, accepted by the user; removal path (vendoring the single `msb_krun` crate under `[patch.crates-io]`) is recorded in CLAUDE.md Open questions and is owned by the phase that next bumps the `msb_krun` pin.
2. **INFO, LAN leg of `exit_egress` did not run.** `MINOS_EXIT_HOST_IP` was unset in the final gate, so only the 127.0.0.1, [::1], and public v4/v6 targets were exercised. KRUN-03 requires loopback and a public IP, so the requirement is met; the optional LAN target is unproven on this run.
3. **INFO, Linux-only guest code is not exercised by `cargo test`.** `crates/minos-guest/src/linux.rs` is covered only by the ignored VM suites (as CLAUDE.md states). Those ran green on the final build.
4. **INFO, process ordering for GATE-06.** `cargo clean` ran as the last step of plan 05-20, before this verification report. That matches the project's in-phase sequence (final gate, then clean); this verification deliberately ran no cargo command. GATE-06 stays unchecked by design.
5. **INFO, stale generated text.** `.claude/CLAUDE.md` (GSD-generated stack block) still names the Seatbelt profile as `crates/minos-vmm/minos-vmm.sb`; the shipped and CLAUDE.md-documented path is `crates/minos-backend-krun/src/minos-vmm.sb`. Noted by 05-20; outside this phase's files.
6. **INFO, documented limits carried forward.** `env build` holds the packed image in memory (D-09 limit, recorded in CLAUDE.md Open questions); the dedicated `_minos` user is deferred (D-01) and every v1 run reports reduced hardening; `RLIMIT_AS` against a real toolchain build is owned by Phase 8; the D-10 lineage half is deferred to Phase 7 (05-CONTEXT Deferred). All are recorded in the design of record, none is a Phase 5 must-have.

## Human Verification Required

None. The manual-only items in 05-VALIDATION.md (entitlements via `codesign`, `host doctor` wording, `env build` against Docker) were executed and recorded in the final gate.

## Gaps Summary

No gaps. Every roadmap success criterion, every one of the 17 requirement IDs (all claimed by a plan, none orphaned), and the three stated facts about GUEST-05, the Final gate record, and GATE-06 / SPIKE-03 check out against the source, test source, config, documents, and git history.

---

_Verified: 2026-10-19_
_Verifier: Claude (gsd-verifier)_
