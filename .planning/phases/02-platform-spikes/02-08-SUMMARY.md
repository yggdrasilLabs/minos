---
phase: 02-platform-spikes
plan: 08
subsystem: infra
tags: [rust, libkrun, msb_krun, msb_krun_devices, vsock, tsi, egress, spike, k3, k4]
status: complete

requires:
  - phase: 02-platform-spikes
    provides: "02-01: minos-guest connect, vsock_connect, vsock_stream steps; 02-03: minos-vmm and its inet-hijack clippy bans; 02-05: supervise::run conn_caps, GuestBytes::matches_pattern; 02-06: E-1 environment, E-4 signing, root-disk boot"
provides:
  - "Ignored tests egress_fails_closed, vsock_stream_within_cap, vsock_stream_over_cap, vsock_throughput_64mib (spike_net.rs)"
  - "SPIKE-02 answer from a real run: no guest egress on any path, canaries see 0 connections (S02-EGRESS)"
  - "SPIKE-03 answer with a limit: the vsock route carries at most 8192 bytes per connection guest to host on the pinned VMM; three msb_krun_devices 0.1.41 defects proven (DIAG-01)"
  - "User decision for Phase 5: a chunked, one-directional output protocol of short vsock connections, at most 4 KiB per connection (S03-ANSWER)"
affects: [02-10, 02-11, phase-05-krun-backend]

tech-stack:
  added: []
  patterns:
    - "Root-cause before dependency change: every Minos-side suspect is ruled out by test from a scratch copy before any fix is proposed; the repository's dependencies stay untouched"

key-files:
  created:
    - crates/minos-backend-krun/tests/spike_net.rs
    - .planning/phases/02-platform-spikes/evidence/02-08.md
  modified: []

key-decisions:
  - "SPIKE-02: every guest connect fails (IPv4 and 127.0.0.1 ENETUNREACH 101; IPv6 and ::1 EADDRNOTAVAIL 99); both canaries accept 0; net_ifaces is [\"lo\"]; no virtio-net. TSI (PF_TSI, PF_TSI6, PF_TSIU) is compiled into the guest kernel, so the result holds only while minos-vmm keeps inet hijack off; egress_fails_closed is the guard and becomes KRUN-03"
  - "SPIKE-03: the route works up to 8192 bytes per connection. Above that, msb_krun_devices 0.1.41 loses data by three proven defects: H1 shutdown race above the 8 KiB macOS send buffer, H2 one-descriptor parser drops payloads over 32 KiB, H3 credit update every 4 MiB deadlocks streams over 256 KiB. None is in Minos (DIAG-01 S1 to S5)"
  - "User decision 2026-10-10, \"work within the limits for now, if we need to reassess later fine\": Phase 5 output is a sequence of short vsock connections, each one chunk of at most 4 KiB, length-prefixed and sequence-numbered, reassembled by the host under per-chunk and total caps; guest to host only; no minos-vmm or dependency change"
  - "Forking msb_krun_devices was considered and rejected for now: it would replace the one trusted dangerous component with a copy only Minos reviews, and chunking is better hostile-input hygiene anyway. Upstream reporting is left to the user. Reassess only if chunking proves a real bottleneck"
  - "K4: the 64 MiB single-connection transfer never completed on the stock VMM; DIAG-01's instrumented runs put one connection at about 6 MiB/s, paced by the 8 KiB send buffer and the supervisor's 1 ms poll"

patterns-established:
  - "A spike test that fails against a proven external defect stays committed and ignored, unchanged, as the record of the limit; the evidence section points at the diagnosis and the decision"

requirements-completed: [SPIKE-02]

duration: about 70 min (three executor sessions, with the DIAG-01 diagnosis and a user decision between them)
completed: 2026-10-10
---

# Phase 2 Plan 08: egress and vsock

**SPIKE-02 passes. A guest with msb_krun's inet hijack off, no port remap, and only the `unix_connect` vsock route cannot reach anything: public IPv4 and IPv6, the host LAN address, and its own loopback all fail, and the host canaries accept 0 connections. SPIKE-03 is answered with a limit rather than passed. The route carries any guest-to-host stream of 8192 bytes or less per connection, but the pinned `msb_krun_devices` 0.1.41 loses or stalls anything larger, for three causes DIAG-01 proved, none of them in Minos. The user chose to work within that limit: Phase 5's output channel will be a sequence of short connections of at most 4 KiB each. No dependency, manifest, or Minos source was changed.**

## Performance

- **Duration:** about 70 min, over three executor sessions (tests and runs, then the DIAG-01 diagnosis, then this close-out after the user's decision)
- **Started:** 2026-10-10T14:28:56Z (plan base `455edef`)
- **Completed:** 2026-10-10T15:40Z
- **Tasks:** 2 of 2
- **Files modified:** 2 (`tests/spike_net.rs`, `evidence/02-08.md`)

## Accomplishments

- `spike_net.rs` holds the four E-1 tests, all ignored. `egress_fails_closed` passes, including a rerun on the final tree against a freshly signed helper.
- SPIKE-02 is answered from a real run (S02-EGRESS).
- DEFECT-01 was found, then explained end to end by DIAG-01 (three defects, each proven with device-side instrumentation and a counterfactual build, all in scratch copies), and closed by a recorded user decision (S03-ANSWER).

## SPIKE-02: per-target result (S02-EGRESS)

| Target | `ok` | `raw_os_error` | errno |
|---|---|---|---|
| `1.1.1.1:443` | false | 101 | `ENETUNREACH` |
| `8.8.8.8:53` | false | 101 | `ENETUNREACH` |
| `127.0.0.1:<v4 canary>` | false | 101 | `ENETUNREACH` |
| `<host-lan-ip>:<v4 canary>` | false | 101 | `ENETUNREACH` |
| `[::1]:<v6 canary>` | false | 99 | `EADDRNOTAVAIL` |
| `[2606:4700:4700::1111]:443` | false | 99 | `EADDRNOTAVAIL` |
| vsock CID 2 port 1025 (no route) | false | 104 | `ECONNRESET` |

Canaries: IPv4 0 and IPv6 0 connections. `net_ifaces` is `["lo"]`, and `lo` has no address. No `0x0001` (virtio-net) device is present. The guest kernel registers `PF_TSI`, `PF_TSI6`, and `PF_TSIU`, so isolation depends on the VMM never enabling inet hijack. msb_krun 0.1.41 defaults it off, and the minos-vmm clippy bans from 02-03 forbid the methods that turn it on. This is not an isolation finding: no attempt succeeded.

## SPIKE-03: the vsock stream (S03-WITHIN, S03-OVER, K4-THROUGHPUT, DEFECT-01, DIAG-01, S03-ANSWER)

| Guest stream per connection | Host receives (stock VMM) | Cause in `msb_krun_devices` 0.1.41 |
|---|---|---|
| 8192 bytes or less | all of it, pattern matches | none; the stream fits the proxy socket's 8 KiB send buffer |
| 8193 to 32768 bytes | exactly 8192, then a clean EOF | H1: `UnixProxy::shutdown` applies the guest's `SHUT_WR` while `pending_write` still holds bytes, and the `HANG_UP` path discards them (40 of 40 runs lost) |
| over 32768 bytes per write | 0, then a clean EOF | H2: `from_tx_virtq_head` accepts one data descriptor only; the 6.12.109 guest spreads larger payloads over 32 KiB descriptors, and the packet is dropped silently |
| over 262144 bytes total | as above, then the guest blocks until the watchdog | H3: credit is returned every 4 MiB, but the guest caps unacknowledged bytes at 256 KiB, and dropped bytes are never credited |

Overflow behaviour: the plan's S03-OVER contract (the host keeps exactly the 1 MiB cap, flags `overflowed`, and the guest sees EPIPE or ECONNRESET) was not observed. No stream reached the cap, so the host-side cap (`take(cap + 1)`) was never exercised in a VM run. The loss is silent on both sides: guest writes and `shutdown` succeed, and the host sees an ordinary EOF.

Throughput (K4): no figure from `vsock_throughput_64mib`, because 0 of 67108864 bytes arrived before the 120 s watchdog. DIAG-01's instrumented and counterfactual builds put one connection at about 6 MiB/s: one 8 KiB buffer fill per `OUT` event about 1.2 ms apart (about 6.5 MiB/s), 262144 bytes in about 51 ms (about 4.9 MiB/s), and 4 MiB in 1.02 s including boot. The chunked protocol's rate is not measured.

Upstream status (read 2026-10-10, nothing built): `containers/libkrun` has fixed H2 (`0ecf4d5f79`) and H3 (`4b5b451e6b`), and H1 does not exist there. The fork's open PR `superradcompany/libkrun#138` covers H2 and H3, not H1. Nobody has reported H1.

Decision (S03-ANSWER, 2026-10-10). Four options were considered: fork and pin, a guest-side workaround on one connection, wait for upstream, and work within the limits. The user chose the last: "work within the limits for now, if we need to reassess later fine."

## Research corrections (for 02-10, D-09)

- **IPv6 errno.** 02-RESEARCH.md Pattern 5 expected `ENETUNREACH` (101) for every target. The IPv6 targets (`::1` and the public address) fail with `EADDRNOTAVAIL` (99). IPv4 matched.
- **vsock limits.** The research assumed a capped stream of any length works over `unix_connect`. On msb_krun_devices 0.1.41 with macOS's default `net.local.stream.sendspace` of 8192, the exact limits are:
  - 8192 bytes per connection is reliable.
  - 8193 to 32768 bytes are cut to 8192.
  - A single write over 32768 bytes is dropped.
  - A stream over 262144 bytes deadlocks.
- **EPIPE or ECONNRESET at the cap.** The research expected the guest to see one of these when the host cuts the stream at the cap. This is untested.
- **Guest kernel.** The kernel is 6.12.109, and TSI is compiled in (already recorded by 02-06; restated here because the egress result depends on it).

## DIAG-01 smaller findings

- minos-vmm installs no `log` backend, so every msb_krun `warn!` and `error!` is discarded, including "error reading TX packet". H2's drops are silent because of this.
- The supervisor's 1 ms non-blocking poll paces throughput. It does not cause any loss (S1 ruled it out), but it sets the ceiling together with the 8 KiB send buffer.
- Latent, not causal here:
  - `UnixProxy::sendmsg` forwards `pkt.buf()` rather than `pkt.payload()`.
  - Credit is returned when bytes enter the unbounded `pending_write`, not when they reach the socket.
  - The `RESPONSE` packet carries stale `fwd_cnt` and `flags`.
- Raising the receiving socket's `SO_RCVBUF` has no effect on macOS. The writer's (the VMM's) `SO_SNDBUF` bounds the queue, and Minos does not own that socket.

## Carry-forward for 02-10, 02-11, and Phase 5

- **CLAUDE.md (02-11 E-4 and Verified facts, local) must record:**
  - The TSI fact: the guest kernel has `PF_TSI`/`PF_TSI6`/`PF_TSIU`, so no egress holds only while inet hijack stays off. `egress_fails_closed` guards it.
  - The exact vsock limits above, with the 8 KiB figure tied to `net.local.stream.sendspace`.
  - The chunked output protocol decision: short connections, at most 4 KiB each, length-prefixed, sequence-numbered, guest to host only, host-side per-chunk and total caps.
  - The fork rejection and its reasons.
  - The upstream status: H2 and H3 are fixed in `containers/libkrun`; PR #138 in the fork is open; H1 is unreported, and reporting it is the user's to do.
- **02-11's placeholders assume a passing stream.** Its E-4 text ("dropping the stream at the cap ({S03-WITHIN}, {S03-OVER}; {K4-THROUGHPUT: MiB per second})") and its success line ("The vsock route carries a capped stream at and over the cap") assume S03 passed. They need the S03-ANSWER wording instead.
- **02-11 Task 2 step 4 will hit the three known failures.** It re-runs `spike_net` and treats any failure as a regression. `vsock_stream_within_cap`, `vsock_stream_over_cap`, and `vsock_throughput_64mib` fail against the pinned VMM for the proven reasons. The orchestrator should decide how 02-11 records them (known limit, S03-ANSWER), since this plan does not edit 02-11.
- **02-10's mapping row needs the decision IDs.** The row `SPIKE-03 | {S03-WITHIN, S03-OVER, K4-THROUGHPUT}` should also cite DIAG-01 and S03-ANSWER.
- **Phase 5:**
  - Build the chunked writer in `minos-guest` and the reassembling reader (GUEST-04).
  - Replace the three single-connection vsock tests with chunked-protocol tests: within the cap, over the cap, and 64 MiB.
  - Measure throughput again. A 64 MiB transfer is 16384 connections, and the supervisor has only handled two per run so far.
  - Have minos-vmm install a logger, so msb_krun's warnings and errors reach its stderr.
  - Consider a `minos host doctor` preflight that reads `net.local.stream.sendspace` and refuses if it is below the chunk size plus header.
  - Promote `egress_fails_closed` to KRUN-03.
- **Reassessment trigger:** if chunking proves a real bottleneck in Phase 5, or an msb_krun release fixes H1 to H3. In the second case, re-run the three vsock tests as written.

## Task Commits

1. **Task 1: Egress and vsock spike tests:** `f964a21` (test)
2. **Task 2: Run SPIKE-02, SPIKE-03, and K4, and record the results:** `3b2358b` (docs: S02-EGRESS, the failing S03 and K4 runs, DEFECT-01), `ac42247` (docs: DIAG-01 root-cause diagnosis), `fb6654c` (docs: the SPIKE-03 answer and the user's decision)

## Files Created/Modified

- `crates/minos-backend-krun/tests/spike_net.rs`: the four ignored E-1 tests and local env and launch helpers. Unchanged since `f964a21`.
- `.planning/phases/02-platform-spikes/evidence/02-08.md`: 08-00, S02-EGRESS, S03-WITHIN, S03-OVER, K4-THROUGHPUT, DEFECT-01, the SPIKE-02 and SPIKE-03 answers, DIAG-01, and S03-ANSWER.

## Decisions Made

See `key-decisions` in the frontmatter. S03-ANSWER in the evidence has the full options table and reasoning.

## Deviations from Plan

### Rule 4 stop (resolved by the user)

**1. [Rule 4 - External defect] DEFECT-01: guest-to-host vsock data past 8 KiB is silently lost in msb_krun_devices 0.1.41**
- **Found during:** Task 2, S03-WITHIN, S03-OVER, K4-THROUGHPUT, with a size sweep for the boundary.
- **Issue:** see the SPIKE-03 table. The defect is in the pinned third-party VMM crate, not in shared Minos code.
- **Handling:**
  - The plan stopped and returned a checkpoint.
  - DIAG-01 then diagnosed the cause from a scratch copy only. The instrumented crate copy, the scratch harness, and the guest variants were deleted afterwards. `target/debug/minos-vmm` was rebuilt from unchanged sources and re-signed, and a `strings` check found no instrumentation in it.
  - The user decided to work within the limit.
  - No fork, vendoring, `[patch]`, or Cargo, deny, or workaround change was made.
- **Effect on acceptance:**
  - The S03-WITHIN, S03-OVER, and K4-THROUGHPUT acceptance criteria are not met.
  - The plan's verification step "VM spike re-run: `spike_net` passes" is not met for those three tests. They were not re-run, because they fail for proven reasons.
  - `egress_fails_closed` was re-run on the final tree and passes.
- **Commits:** `3b2358b`, `ac42247`, `fb6654c`

### Evidence wording revised (not run output)

**2. The SPIKE-03 paragraph and status notes were updated.** The SPIKE-03 paragraph under "SPIKE-02 and SPIKE-03 answers" was rewritten after DIAG-01, and the rewrite says it was revised and what the first version said. Dated status notes were appended under S03-WITHIN, S03-OVER, K4-THROUGHPUT, and DEFECT-01. No verbatim run output and no DIAG-01 content was changed.

## Requirements

- **SPIKE-02:** complete (S02-EGRESS meets every acceptance criterion).
- **SPIKE-03:** left **Pending** in REQUIREMENTS.md. The requirement says "a byte-capped stream from guest to host works end to end". The plan's acceptance for it (1 MiB at the cap, 4 MiB over it with the overflow detected, and 64 MiB intact) was not met on the pinned VMM. SPIKE-03 is answered with a limit and a decision, not passed. Whether the requirement is reworded to the chunked protocol or closed by Phase 5's chunked-stream tests is the user's call. This plan does not invent a status for it.

## Issues Encountered

- `cargo build -p minos-vmm -v` reported `Fresh` but still copied the unsigned binary over the signed one, which dropped the entitlement (recorded at the end of DIAG-01). Re-sign after every cargo command that might touch `minos-vmm`, not only after a visible relink.

## Verification (final tree, `fb6654c`)

- `cargo fmt --all --check` 0; `cargo clippy --workspace --all-targets -- -D warnings` 0.
- `cargo test --workspace` 0 (119 passed, 0 failed, 22 ignored, over 23 test binaries).
- `cargo deny check` 0 (advisories, bans, licenses, and sources ok).
- `cargo doc --workspace --no-deps` 0.
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` 0.
- C-01 prints ten `ok` lines.
- The C-02 allow grep and the unsafe grep print nothing.
- `minos-vmm` was rebuilt and re-signed with the 02-06 E-4 argv, and `codesign --display` shows `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`.
- `egress_fails_closed`: 1 passed, in 0.93 s.
- The Task 2 verify block passes: all five section headings are present, and no private IPv4 address is in the fragment.
- `git diff --name-only 455edef HEAD` lists only `spike_net.rs` and the fragment, plus this SUMMARY once committed.
- `cargo clean` was not run (02-11 does it, GATE-06).

## Next Phase Readiness

- 02-10 can merge the fragment. Its conclusion sections are `SPIKE-02 and SPIKE-03 answers` and `S03-ANSWER`.
- Phase 5 has a defined output-channel design that needs no VMM change. Its open items are listed under carry-forward.

## Self-Check: PASSED

- FOUND: `crates/minos-backend-krun/tests/spike_net.rs`
- FOUND: `.planning/phases/02-platform-spikes/evidence/02-08.md`
- FOUND commits: `f964a21`, `3b2358b`, `ac42247`, `fb6654c`
