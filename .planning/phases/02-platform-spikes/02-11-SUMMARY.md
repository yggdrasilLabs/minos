---
phase: 02-platform-spikes
plan: 11
subsystem: docs
status: complete
tags: [docs-05, spike-08, gate, design-of-record, cargo-clean]
requires:
  - phase: 02-platform-spikes
    provides: "02-SPIKE-EVIDENCE.md (02-10 consolidation) and the CLAUDE.md corrections 02-10 made"
provides:
  - "CLAUDE.md as the v1 local-first design of record: every DOCS-05 amendment, with the AWS content kept and labelled"
  - "02-SPIKE-EVIDENCE.md closing sections: DOCS-05 checklist, Final gate (Phase 2), Invariant check (Phase 2), Spike re-run (Phase 2), cargo clean"
affects: [phase-03-core, phase-05-guest-krun-backend, phase-06-proposers, phase-08-docs]
tech-stack:
  added: []
  patterns: ["Phase-end gate, invariant diff against the phase BASE, spike re-run, then cargo clean as the last cargo command"]
key-files:
  created:
    - .planning/phases/02-platform-spikes/02-11-SUMMARY.md
  modified:
    - CLAUDE.md
    - .planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md
key-decisions:
  - "E-4's vsock bullet is dropped: 02-10's Local backends vsock paragraph already records the 8192-byte limit and the chunked protocol (S03-ANSWER), and the bullet's passing-stream and socket-path claims have no evidence"
  - "E-4's TSI bullet is merged into 02-10's no-network bullet instead of duplicating it; the merged text adds the upstream TSI default, inet_hijack, and the D-PROBE ban categories"
  - "Verified facts, local records SPIKE-03 as the limit it is, not the passing stream E-9 assumed"
requirements-completed: [DOCS-05, SPIKE-08]
duration: 9min
completed: 2026-10-10
---

# Phase 2 Plan 11: DOCS-05 amendments, final gate, and cargo clean Summary

**CLAUDE.md now describes the v1 local-first design: the `ModelProvider` and `claude` CLI provider, no tokio, the `libkrun/krun` tap, root images, local open questions, and dated local verified facts. The AWS design stays, labelled. The phase closes on a green gate, unchanged invariants, a green spike re-run, and `cargo clean`.**

## Performance

- **Duration:** about 9 min
- **Started:** 2026-10-10T16:01:30Z
- **Completed:** 2026-10-10T16:10:00Z
- **Tasks:** 2 of 2
- **Files modified:** 2

## Accomplishments

- **CLAUDE.md (DOCS-05).** Every edit was a scoped replacement:
  - E-1: the Architecture sentence.
  - E-2: `ModelProvider` and the `claude` provider, with the SPIKE-07 base argv, the six-variable environment, the canonical binary path, and the terms-of-use limit.
  - E-3: no tokio in v1.
  - E-4: the tap bullet, and the TSI default merged into the existing no-network bullet.
  - E-5: the Root images paragraph.
  - E-6: the `(AWS milestone)` label and its first line.
  - E-7: the v1 build order, with the seven AWS items kept verbatim.
  - E-8: nine local open questions, and `### Deferred to the AWS milestone` holding the six AWS bullets verbatim.
  - E-9: `## Verified facts, local (as of 2026-10-10)`, nine bullets that each cite evidence IDs.
  - The AWS facts heading is renamed, and the `cargo lambda` comment is changed.
- **D-08 check.** I compared all 43 non-empty BASE (`dbd1a87`) lines of the durable execution rules (with the limits table), build order, open questions, and AWS verified facts against the new CLAUDE.md with `grep -cF`. None is missing. `grep -ci ironclad` gives 1, the same as BASE. No slot marker is left.
- **Final gate.** All six gate commands exit 0. `cargo test --workspace` passes 55 + 18 + 46 tests and ignores the 25 spike tests. C-01 prints ten `ok` lines. C-02 and the no-unsafe grep print nothing.
- **Invariants against BASE.**
  - `Cargo.toml` differs only in the P-PANIC comment.
  - `git diff --quiet` over deny.toml, rustfmt.toml, the core clippy.toml, `rules/`, and `.planning/research` prints 0.
  - `.cargo/config.toml` gained only the musl `linker` table. Its `[build]` line is unchanged.
  - All four lints are `forbid`, and `msb_krun` has `features = ["blk"]` only.
  - So SPIKE-08 holds: there is no unsafe exception anywhere.
- **Spike re-run (D-02).** I rebuilt the guest, whose SHA-256 equals G-GUEST, and the helper. I signed the helper with the E-4 argv (two entitlement keys). Every run used the E-1 environment and left no helper running.
  - `spike_boot`: 3/3
  - `spike_rootimg`: 2/2. The image hash is `d1e93ef0…`, the same as RI-03.
  - `spike_exits`: 12/12
  - `spike_seatbelt`: 3/3
  - `spike_net`: `egress_fails_closed` 1/1
- **cargo clean.** `cargo clean` removed 19876 files (2.6 GiB), and `test ! -d target` printed 0. It is the last `## ` section of the evidence file. No cargo command ran after it.

## DOCS-05 checklist results (E-10)

| Item | Result |
|---|---|
| Local-first build order up front | `## Build order` opens with E-7's first line |
| `ModelProvider`, `claude` CLI provider | `grep -c ModelProvider` 1 |
| No tokio in v1 | E-3 bullet present |
| Pico-dollar money | `pico-dollar` 1, `nano` 0 (already true, no edit) |
| `libkrun/krun` tap | 2 |
| Networking default and counter-default | `inet_hijack` 1 |
| `minos-loop` and loop-never-links-sandbox | `minos-loop/` 1; the `loop` bullet says it never links `sandbox` (already true, no edit) |
| Spike results in dated Verified facts | 9 bullets, SPIKE-01 to SPIKE-07 |
| Codesign and musl clippy lines | 1 and 2 (already true, no edit) |
| Open questions | local list; 6 AWS bullets deferred |
| AWS labelled, nothing removed | both headings present; 43/43 BASE lines present |

## Task Commits

1. **Task 1: The remaining DOCS-05 amendments to CLAUDE.md** - `235a1cf` (docs)
2. **Task 2: Final gate, invariant checks, spike re-run, evidence** - `d734387` (docs)
3. **Task 2: cargo clean (GATE-06)** - `8c42af3` (docs)

## Deviations from Plan

### Adjustments from the orchestrator

**1. [Adjustment 1] E-4 vsock bullet dropped, TSI bullet merged**
- **What the plan assumed:** E-4's third bullet assumed a passing capped stream, with `{S03-WITHIN}`, `{S03-OVER}`, and `{K4-THROUGHPUT MiB/s}` slots.
- **Why it was dropped:** SPIKE-03 is a recorded limit, and 02-10's Local backends vsock paragraph already states the 8192-byte limit, the three defects, and the user's chunked-protocol decision (S03-ANSWER). The bullet's other claims are not in the evidence: "binds the listener before launch" and "socket path under 104 bytes". The bullet was dropped and nothing else was invented.
- **TSI bullet:** E-4's second bullet overlapped 02-10's no-network bullet, so I merged it into that bullet instead of adding a duplicate. The merged bullet now has:
  - the upstream TSI default
  - `msb_krun`'s counter-default (`inet_hijack` off)
  - the D-PROBE ban categories: hijack, port-remap, virtio-fs, non-raw-format, guest-environment, and host-console
- **Tap bullet:** E-4's first bullet was added as written, with `{H-01}` filled.

**2. [Adjustment 2] Spike re-run skips the three vsock-limit tests**
- I ran `spike_net` with `-- --ignored --test-threads=1 --skip vsock_`. `egress_fails_closed` passed (`1 passed; ... 3 filtered out`).
- `vsock_stream_within_cap`, `vsock_stream_over_cap`, and `vsock_throughput_64mib` were intentionally not re-run (DIAG-01, S03-ANSWER). The evidence says so.
- The "five `test result: ok.` lines" acceptance is met as five files with five file-level `ok` lines. `spike_net`'s line covers only the egress test.
- `spike_exits` also prints two nested `ok` lines, from its `orphan_launcher` child runs. The evidence explains them.

**3. [Adjustment 3] Signed before the VM runs**
- I rebuilt the guest and the helper after the gate, because `cargo test --workspace` relinks the helper. I signed the helper with the E-4 argv and checked it with `codesign --display --entitlements -`. Before the later runs I checked again that both keys were still present.
- The `MINOS_VMM_UNSIGNED` copy for `unsigned_helper_is_refused` was made from the fresh build before signing. I deleted it from the scratch directory afterwards.

**4. [Adjustment 4] Requirements.** DOCS-05 and SPIKE-08 are closed. SPIKE-03 stays Pending, and no box was unchecked.

**5. [Adjustment 5] ROADMAP criterion 2.** I did not change its wording.

### Exact texts changed to match the evidence

**6. [Rule 1] E-9 vsock and signing bullets**
- **vsock:** E-9's bullet ("carries a capped stream at and over the cap") is false. The bullet now records the 8192-byte-per-connection limit and the user's chunked decision (S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01, S03-ANSWER).
- **Signing:** "refused by Hypervisor.framework" is reworded to what S04-UNSIGNED shows: `hv_vm_create` fails and `minos-vmm` exits 70. This matches 02-10's correction.

**7. [Rule 1] E-5 wording**
- **Requirement sentence:** E-5's "Any OCI builder ... works" fails the plan's own case-sensitive verify grep (`any OCI builder`) and D-14's quoted requirement. It now reads "The requirement is any OCI builder that can export a rootfs tar".
- **Reproducibility slot:** filled from RI-04. The tars differed because of the `RUN` step's wall-clock mtimes, and the images matched.
- **Phase 5 scope:** the closing clause says "byte-identical tars", not "images", because the images already match.

**8. [Rule 1] Broken anchor.** Renaming `## Verified facts (as of 2026-10-08)` broke the "How to work in this repo" link `#verified-facts-as-of-2026-10-08`. The link now points to `#verified-facts-aws-as-of-2026-10-08`.

**9. [Rule 2] E-8 open questions filled from the evidence**
- **Placeholder slots:**
  - SPIKE-07 left two items open: the behavior of the permission flag (C-08), and session persistence without the flag (C-14).
  - SPIKE-05 left one: rows 1 to 6 of its profile were not minimized.
- **Added beyond E-8:** S03-ANSWER's unverified cost of the chunked protocol (16384 connections for 64 MiB).
- **Overshoot:** K7-04's overshoot is given as 30000 to 48954 bytes (K7-04, K7-11).
- **Device list:** the slot cites S01-FRAME, which holds S01-ROOTDISK run 2's device list.

**10. E-2 argv model.** The SPIKE-07 base argv has `--model haiku`. CLAUDE.md shows `--model <model>` and notes that the spike ran `haiku`, because the provider's model is a manifest choice.

## Issues Encountered

- The gate, the invariant checks, and all five spike files passed on the first run.
- After `cargo clean` (16:09 UTC), the user's editor rebuilt `target/` on its own at 16:10:29 UTC. The process was Zed's rust-analyzer flycheck, pid 43264, running since 09:52 local. It left `target/flycheck0/stderr` ending in `Checking minos-vmm ... Finished`. No command from this plan ran cargo.
- I removed that rebuilt directory with `rm -rf target`. That is not a cargo command, and the directory is gitignored build output. `test ! -d target` printed 0 straight away and again 30 seconds later.
- The evidence file's `## cargo clean` section is accurate for the moment it was recorded. While Zed is open on this repository, rust-analyzer may rebuild `target/` again.

## Open decisions for the user (carried from 02-10, unchanged)

1. **SPIKE-03 wording.** Reword it around the chunked protocol, or close it with Phase 5's chunked-stream tests. It stays Pending.
2. **ROADMAP Phase 2 criterion 2** is partly met (egress yes; the stream holds only up to 8192 bytes per connection).
3. **Upstream reporting** of the three `msb_krun_devices` defects (S03-ANSWER).

## Known Stubs

None.

## Next Phase Readiness

- Phase 2's plans are all executed. Phase 3 (core) and Phase 6 (proposers) can build from CLAUDE.md as amended. Phase 5 has the boot path, signing, exit classification, Seatbelt layer, and chunked-output decision recorded.
- SPIKE-03 and ROADMAP criterion 2 wait on the user, as listed above.

## Self-Check: PASSED

- FOUND: CLAUDE.md, .planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md (last `## ` section is `## cargo clean`)
- FOUND: 235a1cf, d734387, 8c42af3
- `target/` absent; no cargo command from this plan ran after `cargo clean`. The editor's rust-analyzer rebuilt `target/` once, and I removed it (see Issues Encountered)
