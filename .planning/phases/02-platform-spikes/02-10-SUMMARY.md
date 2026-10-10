---
phase: 02-platform-spikes
plan: 10
subsystem: docs
status: complete
tags: [evidence, design-of-record, d-18, d-11, d-09, spike-03]
requires:
  - phase: 02-platform-spikes
    provides: "evidence fragments 02-01, 02-02, 02-03, 02-04, 02-06, 02-07, 02-08, 02-09 and their plan summaries"
provides:
  - "02-SPIKE-EVIDENCE.md: requirement index, the eight fragments verbatim, Boot-path decision (D-18), Research claims overturned (D-09)"
  - "CLAUDE.md and .claude/CLAUDE.md agree with every spike result (boot path, signing, exit codes, Seatbelt, egress, vsock limit)"
  - "SPIKE-01, KRUN-08, KRUN-09, ROADMAP Phase 2 criterion 1 and Phase 5 criteria 2 and 4 reworded with superseded notes"
affects: [02-11, phase-05-guest-krun-backend]
tech-stack:
  added: []
  patterns: ["Corrections to the generated stack block are repeated after GSD:stack-end so regeneration cannot drop them"]
key-files:
  created:
    - .planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md
  modified:
    - CLAUDE.md
    - .claude/CLAUDE.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
  deleted:
    - .planning/phases/02-platform-spikes/evidence/ (eight fragments, consolidated verbatim)
key-decisions:
  - "Phase 2 evidence lives in one file, 02-SPIKE-EVIDENCE.md; the region between the fragment markers hashes to 69494b0379698d9e51eecc479f3f58d06a13a489cbe274e0e0d1d1a57a04bb41, equal to the fragments concatenated"
  - "CLAUDE.md Local backends records the chunked guest-to-host output protocol (S03-ANSWER, user decision 2026-10-10) and the 8192-byte vsock limit of the pinned msb_krun_devices 0.1.41"
  - "SPIKE-03 stays Pending and ROADMAP Phase 2 criterion 2 keeps its wording with a factual status note; both await the user"
requirements-completed: []
duration: 7min
completed: 2026-10-10
---

# Phase 2 Plan 10: Evidence consolidation and design-of-record corrections Summary

**One verbatim evidence file for Phase 2 with a requirement index, and CLAUDE.md, `.claude/CLAUDE.md`, REQUIREMENTS.md, and ROADMAP.md brought in line with the spike results: root-disk boot (D-18), the two-key signing set (K5), the K7 exit-code rule, the Seatbelt layer, the TSI and dummy kernel facts, and the 8192-byte vsock limit with the user's chunked-output decision.**

## Performance

- **Duration:** about 7 min
- **Started:** 2026-10-10T15:52:53Z
- **Completed:** 2026-10-10T15:59:54Z
- **Tasks:** 3 of 3
- **Files modified:** 5 (plus 8 fragments removed)

## Accomplishments

- `02-SPIKE-EVIDENCE.md` holds the eight fragments byte for byte, in plan order, between `<!-- fragments-start -->` and `<!-- fragments-end -->`, with an index from DOCS-05 and SPIKE-01 to SPIKE-08 to their evidence IDs. `evidence/` is removed.
- Consolidation hashes (SHA-256): region between the markers `69494b0379698d9e51eecc479f3f58d06a13a489cbe274e0e0d1d1a57a04bb41`; the eight fragments concatenated with one blank line after each `69494b0379698d9e51eecc479f3f58d06a13a489cbe274e0e0d1d1a57a04bb41`. Equal.
- One commit (3f54757) records D-18 and D-11 across the evidence file, both CLAUDE files, REQUIREMENTS.md, and ROADMAP.md. Three `Superseded wording (2026-10-09)` notes in REQUIREMENTS.md and three `superseded wording, 2026-10-09` notes in ROADMAP.md.
- One commit (1a743fc) corrects every contradicted statement in both CLAUDE files and adds the 20-row `## Research claims overturned (D-09)` table. `.planning/research/` is unchanged against BASE `dbd1a87`.

## Task Commits

1. **Task 1: Consolidate the fragments** - `d7b4e19` (docs)
2. **Task 2: Boot-path and signing decisions, superseded wording** - `3f54757` (docs)
3. **Task 3: D-09 corrections and overturned research claims** - `1a743fc` (docs)

## Overturned research claims (listed in 02-SPIKE-EVIDENCE.md)

Sorted by file, then by section:

- `02-RESEARCH.md` Pattern 3: a returning `/init` panics ("Attempted to kill init"). It does not: libkrunfw turns it into an orderly reboot, and the exit is still 0 (K7-02).
- `02-RESEARCH.md` Pattern 5: every egress connect fails with `ENETUNREACH`. IPv6 fails with `EADDRNOTAVAIL` (99) (S02-EGRESS).
- `02-RESEARCH.md` Pattern 5: a capped stream of any length works on one vsock connection. The limit is 8192 bytes per connection; the 64 MiB transfer never completed (S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01).
- `02-RESEARCH.md` Pattern 7: three items. The `usage` field list misses `inference_geo`, `speed`, and `fallback_credit` (C-03). `--disable-slash-commands` leaves `agents` listed (C-05). An unsatisfiable schema ends in `error_max_turns`, not `error_max_structured_output_retries` (C-11, C-12).
- `ARCHITECTURE.md` Pattern 5: four items. The root image has no "minos final stage" (D-18, S06-BOOT). Output is not "one tar" on one connection (S03-ANSWER). The root does not use `krun_set_root_disk_remount` and a NullFs virtiofs (S01-ROOTDISK, S01-FRAME). The "Exit code" bullet is wrong: the helper's exit code never carries the guest's result (K7 table).
- `ARCHITECTURE.md` Suggested Build Order: the P0 spike expected "`ip link` shows only `lo`", but the kernel creates `dummy0` (S01-NETIFACES).
- `PITFALLS.md` Pitfall 22: an `xtask` or Makefile signing step (D-11).
- `STACK.md` and `SUMMARY.md`: the kernel is 6.12.109, not 6.12.91 (S01-CMDLINE).
- `STACK.md` decision 11: the hypervisor entitlement alone is not enough under the hardened runtime (K5). The refusal is not observable as `HV_DENIED` through msb_krun (S04-UNSIGNED). There is no codesign `xtask` (D-11).
- `STACK.md`: no initramfs boot. Launcher item 4, K1, K2, Alternatives, and What NOT to Use all change (H-02, S01-INITRAMFS).
- `STACK.md`: launcher item 5 ("one capped tar stream") is overturned (S03-ANSWER). There is no `cargo xtask vm-test`. The spikes ran claude 2.1.296, not 2.1.295, and the flag interplay is verified (C-01).

## Deviations from Plan

### Adjustments from the orchestrator (evidence wins over placeholder text)

**1. [Adjustment 1] SPIKE-01 stays checked**
- E-3a and its verify grep assume `- [ ] **SPIKE-01**`. 02-07 had already checked the box, so the reworded line keeps `[x]`. The verify grep ran against `- [x] **SPIKE-01**: ...` and passed. No other box changed.

**2. [Adjustment 2] SPIKE-03 is a recorded limit, not a pass**
- The index row cites S03-WITHIN, S03-OVER, K4-THROUGHPUT, 02-08 DEFECT-01, DIAG-01, the SPIKE-02 and SPIKE-03 answers, and S03-ANSWER, and says the requirement is a limit, not a pass.
- E-8 asked for decision 1c's confidence to say "capped stream proven in S03-WITHIN, S03-OVER". That claim is false. It now records the route, the 8192-byte limit, and the chunked decision.
- E-8 asked for the K4 row to give a one-line result. It now records the three defects, the measured rate (about 6 MiB/s), and the user's decision.
- REQUIREMENTS.md SPIKE-03 is untouched and stays `[ ]` Pending.

**3. [Adjustment 3] Research facts corrected where they appear**
- Kernel 6.12.109; `CONFIG_TSI=y` and `CONFIG_DUMMY=y`; the second entitlement forced by K5; no `CONFIG_BLK_DEV_INITRD`, so the boot is from the root disk.
- A PID 1 exit is an orderly reboot that exits 0. IPv6 connects fail with errno 99.
- claude 2.1.296 behind a cmux shim, with the canonical binary under `~/.local/share/claude/versions/`.
- The extra `usage` fields, and `error_max_turns` with exit 1.

**4. [Adjustment 4] Spike facts recorded in CLAUDE.md Local backends.** Six things go beyond E-9a and E-9b:
- the parent-PID watchdog (02-07 DEFECT-01)
- the console-cap overshoot and budget rule (K7-04)
- no logger in `minos-vmm` (DIAG-01)
- the TSI and dummy facts beside the no-network claim (H-02, S01-NETIFACES, S02-EGRESS)
- the vsock limit and the chunked output protocol, quoting the user's words (S03-ANSWER)
- the Seatbelt answer (SPIKE-05)

**5. [Adjustment 5] ROADMAP Phase 2 criterion 2 keeps its wording**
- A factual status note now follows the criterion: the egress half is met, and the stream half is met only up to 8192 bytes per connection. The note uses neither superseded marker, so the counts stay at three.

### Exact texts changed to match the evidence

**6. [Rule 1] E-5a cmdline.** The plan text gave `rootfstype=squashfs ro`. The constant is `rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0` (S01-NETIFACES, user's "option 2"; `crates/minos-vmm/src/launch.rs` `ROOT_DISK_CMDLINE`). The bullet names `dummy.numdummies=0` and the reason. E-2 notes it as well.

**7. [Rule 1] E-5c refusal wording.** The plan text says the unentitled helper is "refused by Hypervisor.framework". S04-UNSIGNED shows `hv_vm_create` failing as msb_krun's `VmSetup(VmCreate)` with exit 70, and the code is discarded. The text says that instead. E-5c also gained two notes: `cargo test --workspace` can relink the helper (02-06 Verification), and `spike_net` needs `MINOS_SPIKE_HOST_IP`.

**8. [Rule 1] E-6 Stack Patterns.** The plan text says `minos-vmm`'s clippy.toml "bans every `FsBuilder` method". It bans `FsBuilder::root`, `path`, and `custom`, and the bullet names those three (D-PROBE).

**9. [Rule 1] E-9b exit-code rule.** It now covers all of the K7 table and the classification rule:
- 125 with the console-cap marker counts as `Crashed`.
- 126 with the parent-gone marker counts as infrastructure.
- A kernel panic also exits 0 (S01-INITRAMFS).
- Under Seatbelt the helper exits 0 even when its socket is denied (S05-CTRL-SOCK).

**10. [Rule 1] E-9a Seatbelt wording.** "allows only its own files" was inaccurate. The profile also allows system libraries and a list of sysctls, so the bullet lists the allowances as the SPIKE-05 answer table gives them. It also says the `_minos` recipe is recorded but not executed, and that `host doctor` re-checks the denials because `sandbox-exec` is deprecated.

**11. [Rule 2] More stale boot and signing lines in `.claude/CLAUDE.md`.** E-6 did not list these, so I fixed them too:
- the Alternatives Considered row "Initramfs `/init` + virtio-blk root"
- the plist bullet "add ... only if K5 forces it"
- the `HV_DENIED` signing bullet
- the `sandbox-exec` tool row
- the Version Compatibility "Unverified at runtime (K1)" row
- two Sources lines: kernel 6.12.91, and claude 2.1.295 with "flag interplay is unverified"

**12. [Rule 1] Verification date.** E-8's Question 9 text says "Verified 2026-10-09". The 02-02 fragment is dated 2026-10-10, so the bullet says 2026-10-10. The corrections heading keeps the plan's date, which is the D-18 and D-11 decision date.

**13. Index ordering.** The index rows are sorted (DOCS-05 first, then SPIKE-01 to SPIKE-08), following the project's sorted-list rule. E-1 listed DOCS-05 last. The index also says which IDs repeat across fragments (`DEFECT-01` is in both 02-07 and 02-08).

## Open decisions for the user

1. **SPIKE-03 wording.** REQUIREMENTS.md still says "a byte-capped stream from guest to host works end to end", and the box is unchecked. Two options:
   - Reword it around the chunked protocol and close it on S03-ANSWER.
   - Keep the wording and close it with Phase 5's chunked-stream tests.

   This plan does neither.
2. **ROADMAP Phase 2 success criterion 2 is only partly met.** Egress is proven (S02-EGRESS). The "byte-capped guest-to-host stream end to end" part holds only up to 8192 bytes per connection. The criterion's text is unchanged, with a status note next to it. Phase 2 cannot be called fully met on criterion 2 until you decide.
3. **Upstream reporting** of H1 (fork only) and H2/H3 (fixed upstream, open as fork PR #138) is still yours to do (S03-ANSWER). Nothing was filed.

## Issues for 02-11 (the orchestrator decides)

- 02-11's E-4 vsock bullet ("dropping the stream at the cap ({S03-WITHIN}, {S03-OVER}; {K4-THROUGHPUT: MiB per second})") assumes a passing stream. CLAUDE.md Local backends now records the limit and the chunked decision, so E-4's third bullet needs the S03-ANSWER wording or should be dropped. Its TSI bullet partly overlaps the no-network bullet added here. Both are correct, but merging them would read better.
- 02-11 Task 2 step 4 re-runs `spike_net`. `vsock_stream_within_cap`, `vsock_stream_over_cap`, and `vsock_throughput_64mib` fail for the reasons S03-ANSWER proved. They are a known limit, not a regression.
- DOCS-05 and SPIKE-08 are left Pending in REQUIREMENTS.md. 02-11 completes the DOCS-05 amendments and runs the end-of-phase no-unsafe check that closes SPIKE-08.
- `cargo test --workspace` ran in this plan's verification and may have relinked `target/debug/minos-vmm`. Re-sign with the documented argv before the spike re-run.

## Verification

- Task verify blocks: Task 1, Task 2 (with the SPIKE-01 grep adjusted to `[x]`), and Task 3 all pass.
- Gate, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (119 passed, 0 failed), `cargo deny check`, `cargo doc --workspace --no-deps`, and `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`.
- C-01 prints ten `ok` lines. C-02 prints nothing.
- `git diff --quiet dbd1a87 -- .planning/research` exits 0.
- `grep -ci ironclad`: CLAUDE.md 1 (the existing "do not claim it" sentence), `.claude/CLAUDE.md` 0, the same as BASE. No new "secure" or "fully isolated" claim. Every isolation statement cites its spike, and the no-egress claim carries the TSI condition.
- `cargo clean` was not run; 02-11 does that (GATE-06).

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: .planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md; `evidence/` absent
- FOUND: d7b4e19, 3f54757, 1a743fc
