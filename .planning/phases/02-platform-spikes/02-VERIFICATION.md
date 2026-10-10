---
phase: 02-platform-spikes
verified: 2026-10-10T00:00:00Z
status: gaps_found
score: 4/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: false
gaps:
  - truth: "With implicit vsock disabled and a plain vsock added, a guest connect() to host loopback and to a public IP both fail, and a vsock port mapped to a host Unix socket carries a byte-capped guest-to-host stream end to end (SPIKE-02, SPIKE-03)"
    status: partial
    reason: "The egress half is met (S02-EGRESS). The stream half is met only up to 8192 bytes per connection. Above that, the pinned msb_krun_devices 0.1.41 loses or stalls guest-to-host data (S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01). This is a recorded platform limit, not a Minos code defect. The user decided on 2026-10-10 to work within the limit with a chunked output protocol in Phase 5 and rejected a fork for now (S03-ANSWER). SPIKE-03 is correctly left [ ] Pending in REQUIREMENTS.md, and the ROADMAP criterion carries a status note but unchanged wording."
    artifacts:
      - path: ".planning/ROADMAP.md"
        issue: "Phase 2 criterion 2 still reads 'carries a byte-capped guest-to-host stream end to end'. It is only true for streams of 8192 bytes or less on one connection."
      - path: ".planning/REQUIREMENTS.md"
        issue: "SPIKE-03 is Pending. Its wording does not describe the chunked-connection design the user chose."
    missing:
      - "User decision on wording: either reword SPIKE-03 and ROADMAP criterion 2 around the chunked short-connection protocol (then the recorded evidence supports closing it), or keep the wording and close SPIKE-03 with Phase 5's chunked-stream tests."
      - "Phase 5 must build and test the chunked protocol (one length-prefixed, sequence-numbered chunk of at most 4 KiB per connection). Its throughput is unmeasured. CLAUDE.md Open questions records this."
deferred: []
human_verification: []
---

# Phase 2: Platform Spikes Verification Report

**Phase Goal:** Every unverified fact that the backend and proposer designs rest on has an evidence-backed answer recorded in CLAUDE.md, and CLAUDE.md describes the v1 local-first design.
**Verified:** 2026-10-10
**Status:** gaps_found (one partial roadmap criterion, awaiting a user wording decision)
**Re-verification:** No, initial verification

Per the hard constraint, no `cargo` command was run. The gate, invariant, and spike re-run results come from the recorded sections of `02-SPIKE-EVIDENCE.md` (`## Final gate (Phase 2)`, `## Invariant check (Phase 2)`, `## Spike re-run (Phase 2)`, `## cargo clean`). Everything else below is a read-only grep or read of the repository.

## Goal Achievement

The goal has two halves.

1. **Every unverified fact has an evidence-backed answer recorded in CLAUDE.md.** Eight of nine spike items and the amendments are in place. The one exception is the vsock stream limit. It has an evidence-backed answer, but the answer is "limit", not "pass".
2. **CLAUDE.md describes the v1 local-first design.** This holds.

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The safe libkrun wrapper boots libkrunfw from a raw read-only SquashFS root with a static musl `/init`, no initramfs and no virtiofs, with the exact `/proc/cmdline` asserted. A codesigned helper starts the VM and an unsigned copy is refused (SPIKE-01, SPIKE-04) | VERIFIED, judged against the D-18 wording | See criterion 1 detail below. |
| 2 | A guest `connect()` to host loopback and a public IP fail, and a vsock port mapped to a host Unix socket carries a byte-capped guest-to-host stream end to end (SPIKE-02, SPIKE-03) | PARTIAL (gap) | Egress half VERIFIED. Stream half holds only to 8192 bytes per connection. See the gap above. |
| 3 | Seatbelt, root-image tool chain, and `claude -p` each have a recorded, evidence-backed answer (SPIKE-05, SPIKE-06, SPIKE-07) | VERIFIED | See criterion 3 detail below. |
| 4 | CLAUDE.md records every spike result and the v1 amendments, the workspace has no `unsafe` exception, and if the boot spike had failed, work would have stopped (DOCS-05, SPIKE-08) | VERIFIED | See criterion 4 detail below. |
| 5 | `cargo clean` has been run after verification passes (GATE-06) | VERIFIED | `test ! -d target` holds, and the `## cargo clean` section records exit 0 and `Removed 19876 files, 2.6GiB total`. |

**Score:** 4/5 truths verified. The criterion 2 gap is a partial, and it is not a code defect.

### Criterion 1 detail (SPIKE-01, SPIKE-04)

The ROADMAP wording "from an initramfs whose `/init` is a static musl binary" was superseded by D-18 on 2026-10-09. `02-10` recorded a superseded note beside it in ROADMAP.md and REQUIREMENTS.md. I judged the criterion against the D-18 wording (root-disk boot).

- **Root-disk boot.** S01-ROOTDISK run 2 booted from the SquashFS root on `/dev/vda` with a guest marker, a v1 status frame, and helper exit 0. Run 1 had failed on the library-validation entitlement (K5), which is how the second entitlement was forced.
- **Exact `/proc/cmdline`.** S01-CMDLINE gives the observed token list, equal to the prediction. `crates/minos-backend-krun/tests/spike_boot.rs` holds `EXPECTED_CMDLINE` and `assert_eq!(tokens, EXPECTED_CMDLINE)` at line 358.
- **No initramfs.** S01-INITRAMFS holds the negative evidence. The kernel panics with `VFS: Unable to mount root fs`, and libkrunfw has `CONFIG_BLK_DEV_INITRD` unset.
- **No virtio-fs.** `crates/minos-vmm/clippy.toml` bans `VmBuilder::fs` and the `FsBuilder` methods. CLAUDE.md records "No VM has a virtio-fs device", cited to the S01-FRAME device list.
- **Static musl `/init`.** `rust-toolchain.toml` lists `aarch64-unknown-linux-musl`. The `.cargo/config.toml` musl table sets `linker` only, so it does not replace the `[build]` forbid backstop (invariant check I3, I3b).
- **Unsigned refusal.** S04-UNSIGNED shows a linker-signed helper with no entitlements. The spike test `unsigned_helper_is_refused` passed with helper exit 70 before the VM started. The signed helper boots (S04-SIGN). The entitlements plist `crates/minos-vmm/minos-vmm.entitlements` has exactly two keys, `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`. CLAUDE.md Commands documents the second key and why K5 forced it.
- **Caveat in the evidence.** The first unsigned-style failure was a libkrunfw dlopen refusal, not a `HV_DENIED` from Hypervisor.framework. The evidence records that `HV_DENIED` is not observable through msb_krun (it surfaces as `VmSetup(VmCreate)`, exit 70). CLAUDE.md states this accurately.

### Criterion 3 detail (SPIKE-05, SPIKE-06, SPIKE-07)

- **SPIKE-05.** The `SPIKE-05 answer` section says Yes: a deny-default Seatbelt profile still permits VM creation. Support is S05-BOOT, S05-DENY-NET, and S05-DENY-FILE. The profile exists at `crates/minos-vmm/minos-vmm.sb`. The dedicated `_minos` user fallback is recorded as a recipe (S05-RECIPE), not executed, and CLAUDE.md says so. The residual caveat, that rows 1 to 6 of the allowance table were not minimized, is in Open questions.
- **SPIKE-06.** RI-03 and S06-BOOT show a digest-pinned Dockerfile becoming a bootable root image through `docker buildx --output type=tar` plus the Rust SquashFS packer. A command from the image ran and its output came back. RI-04 records that two tars differed because of a `RUN` timestamp, while the two images matched byte for byte. This was recorded as a note, as D-16 required. CLAUDE.md "Root images" documents the tool chain and names the untested alternatives.
- **SPIKE-07.** The `SPIKE-07 contract` table gives 10 recorded calls (C-03 to C-12). It shows `structured_output` under subscription login with an environment of six variables, `num_turns` of 2 under `--max-turns 1` with a schema, and the usage field names. CLAUDE.md "The model stays outside the sandbox" and the Verified facts bullet reproduce the argv and the allowlist. Two items are honestly left open: the permission-flag difference when a permission request occurs (C-08), and session-directory behavior without `--no-session-persistence` (C-14).

### Criterion 4 detail (DOCS-05, SPIKE-08)

I checked each v1 amendment in CLAUDE.md directly.

| Amendment | Where in CLAUDE.md | Status |
|-----------|--------------------|--------|
| Local-first build order up front | `## Build order` opens with "Milestone v1 is local-first, on macOS arm64", and the AWS build order is kept below it as a labelled later milestone | VERIFIED |
| `ModelProvider` decision, `claude` CLI as personal default | Architecture, "The model stays outside the sandbox" | VERIFIED |
| No tokio in v1 | Code rules, line 131 | VERIFIED. No `crates/*/Cargo.toml` mentions tokio. |
| Pico-dollar money | Cost section, line 393. `grep nano` in CLAUDE.md returns nothing. | VERIFIED |
| `libkrun/krun` tap | Local backends, line 373, and Verified facts | VERIFIED |
| libkrun networking default (TSI on when no NIC, msb_krun reverses it) | Local backends, line 376 | VERIFIED |
| `minos-loop` crate and loop never links sandbox | Workspace listing and dependency rule at line 102 | VERIFIED |
| Every spike result with date | `## Verified facts, local (as of 2026-10-10)` has 9 bullets citing evidence IDs | VERIFIED |
| Codesign line and musl clippy line in Commands | `## Commands` | VERIFIED |
| AWS content labelled, not removed (D-08) | `## Durable execution rules (AWS milestone)`, `## Verified facts, AWS (as of 2026-10-08)`, `### Deferred to the AWS milestone` | VERIFIED |
| Research claims overturned recorded (D-09) | Evidence section `## Research claims overturned`, and the `.claude/CLAUDE.md` corrections. Examples: kernel is 6.12.109, not 6.12.91; `dummy0` exists; helper exit code does not carry the guest result. | VERIFIED |

- **No isolation overclaim.** CLAUDE.md says "Ironclad is not achievable" (the one pre-existing occurrence). The no-network statements are scoped honestly. The document says TSI is compiled into the kernel (`CONFIG_TSI=y`) and that no egress holds only while inet hijack stays off. The Seatbelt layer is described as deprecated-but-working, and the `_minos` user layer as "recorded but not yet executed". Layers are recorded in `BackendInfo` and never fall back silently.
- **SPIKE-08.** The stop-and-ask was not triggered: S08 records that the D-18 root-disk boot succeeded. The user's D-18 choice (root-disk boot) was honored. The decision is recorded before any alternative was tried, and the literal initramfs attempt was run once as negative evidence. Static checks found no `unsafe` and no `allow` exceptions in the workspace.
  - `grep -rnE '#!?\[allow\(' crates --include='*.rs'` prints nothing (C-02).
  - `grep -rnE 'unsafe[[:space:]]*(\{|fn |impl|extern|trait)|unsafe_code' crates --include='*.rs'` prints nothing.
  - I4 in the invariant check shows all four workspace lints at `forbid`. I5 shows `msb_krun` with `features = ["blk"]` only.
  - The only `XXX` match in `crates/` is the `mnsXXXXXXXX` temp-directory name template in `supervise.rs:183`. It is not a debt marker.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` | Verbatim evidence plus a requirement-to-evidence index | VERIFIED | The index maps all 9 requirement IDs to section IDs that exist in the file. |
| `crates/minos-vmm/minos-vmm.entitlements` | Two-key plist | VERIFIED | Exactly the two documented keys. |
| `crates/minos-vmm/minos-vmm.sb` | Seatbelt profile | VERIFIED | Exists. Evidence S05-BOOT used it. |
| `crates/minos-vmm/clippy.toml` | Bans on hijack, virtio-fs, non-raw, host-env, and console methods | VERIFIED | 13 entries. D-PROBE proved each fires. |
| `crates/minos-backend-krun/tests/spike_{boot,exits,net,rootimg,seatbelt}.rs` | Ignored VM spike tests (D-02) | VERIFIED | All 5 present. Final gate shows 3, 12, 4, 2, and 3 ignored tests respectively. |
| `crates/minos-guest` | Static `/init` with job protocol | VERIFIED | `linux.rs`, `job.rs`, and `report.rs` have real bodies (506, 838, and 313 lines). |
| `rust-toolchain.toml`, `.cargo/config.toml` | musl target with linker only | VERIFIED | Matches invariant check I3. |
| `CLAUDE.md`, `.claude/CLAUDE.md` | Amended design of record | VERIFIED | See criterion 4. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| CLAUDE.md Verified facts, local | `02-SPIKE-EVIDENCE.md` | Evidence IDs (S01-ROOTDISK, S02-EGRESS, K5, and others) | WIRED | Every ID I sampled exists as a heading or table row in the evidence file. |
| `spike_boot.rs` `EXPECTED_CMDLINE` | S01-CMDLINE observation | `assert_eq!` on normalized tokens | WIRED | The test was passing in the Phase 2 re-run (3 of 3). |
| `minos-vmm` clippy bans | `msb_krun` hijack and virtio-fs methods | `disallowed-methods` | WIRED | `egress_fails_closed` guards the behavior (1 of 1 in the re-run). |
| REQUIREMENTS.md traceability | Phase 2 plans | Plan frontmatter `requirements:` | WIRED | See below. |

### Requirements Coverage

All 9 IDs in the phase are claimed by at least one plan's `requirements:` frontmatter and appear in REQUIREMENTS.md. No orphaned requirement maps to Phase 2.

| Requirement | Source plans | Description | Status | Evidence |
|-------------|--------------|-------------|--------|----------|
| SPIKE-01 | 02-01, 02-03, 02-04, 02-05, 02-06, 02-07, 02-10 | Boot from raw read-only SquashFS root, exact `/proc/cmdline` | SATISFIED (D-18 wording) | S01-ROOTDISK, S01-CMDLINE, S01-INITRAMFS |
| SPIKE-02 | 02-03, 02-08 | Guest connect to loopback and public IP fail | SATISFIED | S02-EGRESS, `egress_fails_closed` passed in the re-run |
| SPIKE-03 | 02-05, 02-08 | Byte-capped guest-to-host stream end to end | NOT SATISFIED, recorded LIMIT | Works to 8192 bytes. Fails above that on three defects in the pinned `msb_krun_devices` 0.1.41. The user chose chunking. REQUIREMENTS.md `[ ]` Pending is correct. |
| SPIKE-04 | 02-01, 02-03, 02-06 | Codesigned helper boots, unsigned refused | SATISFIED | S04-UNSIGNED, S04-SIGN, K5 |
| SPIKE-05 | 02-09 | Seatbelt deny-default answer plus fallback recorded | SATISFIED | SPIKE-05 answer, S05-RECIPE |
| SPIKE-06 | 02-04, 02-06 | Dockerfile to root image on macOS | SATISFIED | RI-03, S06-BOOT |
| SPIKE-07 | 02-02 | `claude -p` contract | SATISFIED | C-01 to C-14, SPIKE-07 contract |
| SPIKE-08 | 02-06, 02-10, 02-11 | Stop and ask if the boot fails, no `unsafe` exception | SATISFIED (not triggered) | S08, the no-`unsafe` grep, invariant check I4 and I5 |
| DOCS-05 | 02-10, 02-11 | CLAUDE.md amended for v1 | SATISFIED | DOCS-05 checklist, and my own check of each item |

REQUIREMENTS.md marks SPIKE-03 Pending and the other eight Complete. That matches what I found.

### Gate and Hygiene (from recorded evidence, plus read-only checks)

| Check | Result | Source |
|-------|--------|--------|
| `cargo fmt --all --check` | exit 0 | Final gate |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | Final gate |
| `cargo test --workspace` | exit 0. 55 backend-krun, 18 guest, and 46 vmm unit tests pass; spike tests are ignored. | Final gate |
| `cargo deny check` | exit 0 (`advisories ok, bans ok, licenses ok, sources ok`) | Final gate |
| `cargo doc --workspace --no-deps` | exit 0 | Final gate |
| musl guest clippy | exit 0 | Final gate |
| C-01 (lints inherit workspace) | `ok` for all 10 crates | Final gate |
| C-02 (`allow` grep) | No output. I re-ran it myself and it also printed nothing. | Final gate, my grep |
| No `unsafe` | No output. I re-ran it myself and it also printed nothing. | Final gate, my grep |
| Spike re-run from the final tree | spike_boot 3/3, spike_rootimg 2/2, spike_exits 12/12, spike_seatbelt 3/3, spike_net `egress_fails_closed` 1/1; no `minos-vmm` left running | Spike re-run |
| `target/` absent | Confirmed by `test ! -d target` | My check |
| Debt markers in `crates/` | None (the one `XXX` match is a path template) | My grep |

### Behavioral Spot-Checks

Step 7b: SKIPPED. Running the VM spikes would need cargo, which the phase's rule forbids after `cargo clean`. The VM spike tests are `#[ignore]`d and need a signed helper and libkrunfw. The recorded re-run from the final tree is the evidence. The vsock-limit tests (`vsock_stream_within_cap`, `vsock_stream_over_cap`, `vsock_throughput_64mib`) were intentionally not re-run, because they encode the single-connection contract that the pinned crate cannot meet. This is stated in the evidence, and it is the correct reading of S03-ANSWER.

### Anti-Patterns Found

None blocking. All the debt-marker, `allow`, and `unsafe` scans came back clean. The evidence file is about 466 KB because it is verbatim. That is by design (D-06).

### Warnings and Notes

- **Warning, ROADMAP status.** The Phase 2 line in ROADMAP.md is marked `[x]` "completed 2026-10-10", but criterion 2 is partial. The criterion text is unchanged and carries only a status note. Do not treat the phase as fully clean until the user resolves the wording.
- **Note, `cargo clean` ordering.** `cargo clean` ran at the end of plan 02-11, before this verification pass. That is consistent with the instruction that it is the last cargo command of the phase. `target/` is confirmed absent. The 02-11 follow-up commit notes that the editor's rust-analyzer may rebuild `target/` and that this is not a phase action.
- **Note, open items carried forward.** CLAUDE.md `## Open questions` honestly lists these:
  - the console-cap overshoot of 30000 to 48954 bytes;
  - virtio-rng;
  - the libkrunfw copy-before-load gap;
  - how the `_minos` user is started;
  - the two `claude` open items;
  - the Seatbelt allowance minimization;
  - the chunked-vsock cost;
  - K8 launch rate and memory.

  None of these invalidates a Phase 2 truth. Each belongs to Phase 5.
- **Note, defects found and fixed.**
  - 02-07 DEFECT-01 was a watchdog parent-gone defect, fixed in shared code (`8bd6881`) as an orchestrator-approved deviation. K7-07 and K7-07b pass in the re-run.
  - The vsock defects are in a third-party crate, and no fix was made.

### Gaps Summary

One gap, and it is a decision, not a defect. ROADMAP criterion 2 and SPIKE-03 promise a byte-capped guest-to-host stream on a vsock port mapped to a host Unix socket. The spikes proved the port-to-Unix-socket mapping and a working stream up to 8192 bytes per connection. They also proved, with instrumented diagnosis (DIAG-01), that larger single-connection streams fail on three defects in `msb_krun_devices` 0.1.41, with none in Minos code. The user chose on 2026-10-10 to work within the limit with a chunked output protocol and not to fork the VMM. CLAUDE.md records the limit, the three causes, and the chunked design accurately.

**Decision needed from the user.** Choose one wording path:

1. Reword SPIKE-03 and ROADMAP criterion 2 around the chunked short-connection protocol, so Phase 2 can close on the recorded evidence.
2. Keep the wording and close SPIKE-03 with Phase 5's chunked-stream tests.

Until then, SPIKE-03 stays Pending, as it is in REQUIREMENTS.md. Everything else in the phase is verified. Phase 5 inherits one mandatory design constraint: the chunked output protocol.

---

_Verified: 2026-10-10_
_Verifier: Claude (gsd-verifier)_
