---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 4
current_phase_name: Store, Source Snapshot, and Sandbox Types
status: verifying
stopped_at: Phase 4 context gathered
last_updated: "2026-10-14T16:54:42.861Z"
last_activity: 2026-10-14
last_activity_desc: Phase 03 complete, transitioned to Phase 4
progress:
  total_phases: 8
  completed_phases: 3
  total_plans: 29
  completed_plans: 29
  percent: 38
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-09)

**Core value:** Untrusted experiment code runs in a hardware-virtualized sandbox with no network, cannot touch the judge that scores it, and nothing it produces reaches the user's machine without an acknowledged report.
**Current focus:** Phase 03 — core-types-and-pure-logic

## Current Position

Phase: 4 — Store, Source Snapshot, and Sandbox Types
Plan: Not started
Status: Phase complete — ready for verification
Last activity: 2026-10-14 — Phase 03 complete, transitioned to Phase 4

Progress: [██████████] 100% (29 of 29 plans)

## Performance Metrics

**Velocity:**

- Total plans completed: 29
- Average duration: -
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 5 | - | - |
| 02 | 11 | - | - |
| 03 | 13 | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*
| Phase 01 P01 | 3min | 3 tasks | 28 files |
| Phase 01 P02 | 1min | 2 tasks | 1 files |
| Phase 01 P03 | 4min | 3 tasks | 3 files |
| Phase 01 P04 | 5min | 3 tasks | 9 files |
| Phase 01 P05 | 6 min | 3 tasks | 8 files |
| Phase 02 P01 | 12min | 4 tasks | 21 files |
| Phase 02 P02 | 7min | 2 tasks | 1 files |
| Phase 02 P03 | 8min | 3 tasks | 8 files |
| Phase 02 P04 | 12min | 3 tasks | 6 files |
| Phase 02 P05 | 10min | 2 tasks | 3 files |
| Phase 02 P06 | 24min | 3 tasks | 6 files |
| Phase 02 P07 | 15min | 2 tasks | 4 files |
| Phase 02 P08 | 70min | 2 tasks | 2 files |
| Phase 02 P09 | 10 min | 2 tasks | 3 files |
| Phase 02 P10 | 7min | 3 tasks | 5 files |
| Phase 02 P11 | 9min | 2 tasks | 2 files |
| Phase 03 P01 | 6min | 3 tasks | 26 files |
| Phase 03 P02 | 9 min | 2 tasks | 4 files |
| Phase 03 P03 | 4 min | 2 tasks | 3 files |
| Phase 03 P04 | 6 min | 3 tasks | 5 files |
| Phase 03 P05 | 15 min | 2 tasks | 3 files |
| Phase 03 P06 | 9 min | 2 tasks | 5 files |
| Phase 03 P07 | 9 min | 2 tasks | 3 files |
| Phase 03 P08 | 5 min | 2 tasks | 2 files |
| Phase 03 P09 | 5 min | 2 tasks | 26 files |
| Phase 03 P10 | 7 min | 2 tasks | 3 files |
| Phase 03 P11 | 8 min | 3 tasks | 6 files |
| Phase 03 P12 | 8min | 2 tasks | 4 files |
| Phase 03 P13 | 6 min | 3 tasks | 4 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Workspace gates are their own phase so Phase 2 (spikes) and Phase 3 (core) both depend only on Phase 1 and run in parallel
- [Roadmap]: GATE-06 (`cargo clean` after verification) is cross-cutting: the last success criterion of every phase, never checked off by a single phase
- [Roadmap]: DOCS-05 (CLAUDE.md v1 amendments) lands in Phase 2 with the spike results, so later phases build from a corrected design of record
- [Roadmap]: CLI-11 (deterministic, secret-scanned source tarball) lands in Phase 4 because `env build` (Phase 5) and every experiment need it
- [Roadmap]: Phase 7 loop plans run against fakes before Phase 5 finishes; only the runner's real-VM plans wait on the backend
- [Phase 01]: Workspace config taken verbatim from 01-RESEARCH.md Code Examples; lint carve-outs limited to too_many_lines, similar_names, struct_excessive_bools, multiple_crate_versions
- [Phase 01]: rustix and vsock sit under a cfg(target_os = linux) target table in minos-guest
- [Phase 01]: deny.toml encodes crate layering as one wrappers entry per internal library crate (direct edges only)
- [Phase 01]: cargo-deny multiple-versions stays warn; 7 duplicates from msb_krun, proptest, getrandom
- [Phase 01]: 01-03: all 23 gate violations rejected with verbatim evidence in 01-GATE-EVIDENCE.md; V-23 zstd-sys ban fired as expected
- [Phase 01]: 01-03: three gate gaps (inner allow with reason, missing [lints], rustdoc only under cargo doc) left open for the user, not resolved
- [Phase 01]: 01-04: unwrap_used, expect_used, indexing_slicing raised to forbid (UAT test 1); #[expect] can no longer silence them
- [Phase 01]: 01-04: .cargo/config.toml [build] rustflags forbid backstop plus D-15 per-manifest lints check (UAT test 2)
- [Phase 01]: 01-04: cargo doc --workspace --no-deps is the fifth gate command (UAT test 3)
- [Phase 01]: UAT test 6 (A): minos-cli uses the clap builder API; clap derive feature off; re-enabling it is a user decision, never a lint change
- [Phase 02]: 02-01: Cargo rejects panic in a package profile (P-PANIC); minos-guest keeps unwinding and a PID 1 panic surfaces as no status frame (Crashed)
- [Phase 02]: 02-01: the guest reaps orphans with rustix wait(NOHANG); waitpid(None) in rustix 1.x covers only the caller's process group
- [Phase 02]: 02-01: libkrunfw 5.6.2 canonical path /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib, SHA-256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
- [Phase 02]: SPIKE-07: claude -p with --tools "" and --json-schema returns structured_output under subscription login, six-variable env allowlist, empty temp dir (evidence/02-02.md C-03)
- [Phase 02]: SPIKE-07: unsatisfiable schema ends in error_max_turns (exit 1), never error_max_structured_output_retries; read top-level usage, not usage.iterations (C-11, C-12)
- [Phase 02]: SPIKE-07: --disable-slash-commands empties slash_commands and skills but agents stay listed; --tools "" leaves only StructuredOutput (C-04, C-05)
- [Phase 02]: 02-03: all 13 E-5 msb_krun clippy bans fire with definition paths (msb_krun::api::builder/builders); no re-export form needed; FsBuilder::custom fires as a bare path
- [Phase 02]: 02-03: minos-vmm pre-takeover exits 64/65/66/67/70 (67 = SHA-256 mismatch); watchdog 124/125/126 via Vm::exit_code + ExitHandle::trigger
- [Phase 02]: 02-03: launch config parse calls VmmConfig::try_from directly so ConfigError stays typed; the serde try_from attribute routes other paths through the same rules
- [Phase 02]: 02-04: root image packer caps file content including hardlink copies at max_tar_bytes, and checks max(header, PAX) size before reading
- [Phase 02]: 02-04: D-16 result: two buildx tars of the alpine fixture differ only in RUN-step mtimes; the packed SquashFS images are byte-identical
- [Phase 02]: 02-05: supervisor connection reads poll a non-blocking stream against one total deadline, not SO_RCVTIMEO — macOS returns EINVAL from setsockopt after the peer closes, and a per-read timeout is renewable by a trickling guest
- [Phase 02]: 02-05: has_hypervisor_entitlement matches the exact codesign XML key followed by <true/>; an overflowed connection is never decoded as a status frame
- [Phase 02]: 02-06 K5: minos-vmm entitlements carry com.apple.security.cs.disable-library-validation beside the hypervisor key; the hardened runtime refuses Homebrew libkrunfw (different Team IDs)
- [Phase 02]: 02-06 user decision (option 2): minos-vmm appends dummy.numdummies=0 to the root-disk kernel arguments and boot_root_disk asserts no virtio-net (0x0001) device; net_ifaces must be empty or exactly lo
- [Phase 02]: 02-06: guest kernel is 6.12.109, not 6.12.91; SPIKE-08 not triggered; the D-18 root-disk boot works
- [Phase 02]: K7: the helper exit status never carries the guest result; power-off, init return, and SIGTERM all exit 0; Phase 5 classifies from the vsock frame (overturns ARCHITECTURE.md Pattern 5)
- [Phase 02]: DEFECT-01 fixed (orchestrator-approved): minos-vmm records its parent PID at main entry and a PID 1 parent counts as gone (exit 126)
- [Phase 02]: SPIKE-02: no guest egress (IPv4 and 127.0.0.1 ENETUNREACH 101, IPv6 and ::1 EADDRNOTAVAIL 99, canaries 0, net_ifaces lo); TSI is compiled into the guest kernel, so isolation holds only while inet hijack stays off; egress_fails_closed becomes KRUN-03
- [Phase 02]: SPIKE-03: vsock guest-to-host is reliable to 8192 bytes per connection; msb_krun_devices 0.1.41 has three proven defects above it (H1 shutdown race, H2 multi-descriptor drop over 32 KiB, H3 4 MiB credit threshold deadlock over 256 KiB) (DIAG-01); SPIKE-03 left Pending in REQUIREMENTS
- [Phase 02]: User decision 2026-10-10 (work within the limits): Phase 5 output is short vsock connections of at most one 4 KiB chunk, length-prefixed and sequence-numbered, reassembled under per-chunk and total caps; guest to host only; msb_krun fork rejected for now; upstream reporting left to the user
- [Phase 02]: SPIKE-05: minos-vmm boots under a deny-default Seatbelt profile (crates/minos-vmm/minos-vmm.sb) with three allowances from logged denials; hv_vm_create needs no hypervisor rule; the only network allowance is the literal vsock route socket
- [Phase 02]: Phase 5 may report Seatbelt as an active local confinement layer (KRUN-07) when launched via /usr/bin/sandbox-exec; the dedicated _minos user is recipe only (D-10), so runs record reduced local hardening for that layer
- [Phase 02]: Phase 2 evidence is one file, 02-SPIKE-EVIDENCE.md, consolidated verbatim (region SHA-256 equals the fragment concatenation, 69494b03...bb41)
- [Phase 02]: CLAUDE.md records the 8192-byte vsock limit of msb_krun_devices 0.1.41 and the user's chunked output protocol (S03-ANSWER); SPIKE-03 and ROADMAP Phase 2 criterion 2 await the user's wording decision
- [Phase 02]: E-4's vsock bullet dropped and its TSI bullet merged into 02-10's no-network bullet; CLAUDE.md Verified facts, local records SPIKE-03 as the 8192-byte limit (S03-ANSWER)
- [Phase 03]: 03-01: serde_json float_roundtrip on workspace-wide; .config/insta.yaml update: no, so the gate never writes snapshots
- [Phase 03]: 03-01: IdHasher is public (Phase 4 LaunchKey); digest_id! is pub(crate) for JudgeVersion, EventKey, SpendId, ReviewId in later plans
- [Phase 03]: 03-01: RelPath is an ASCII [A-Za-z0-9._+-] grammar with bytewise Ord; ascii_case_key exists only for the 03-05 case-fold collision check
- [Phase 03]: 03-02: HunkLine::text excludes the trailing newline; has_newline reports it
- [Phase 03]: 03-02: a no-newline marker closes its side for the rest of the section (no later line of that side, no later hunk)
- [Phase 03]: 03-02: stage 7 re-runs stages 1-5 on the canonical text so every Diff re-parses under the same caps; file and hunk caps fire in the stage 2 grammar
- [Phase 03]: 03-03: Scorecard wire form is a versioned array of {name,value,held_out}; duplicates rejected, byte cap checked before parsing, JSON errors keep only line, column, and category
- [Phase 03]: 03-03: Outcome::Crashed and TimedOut carry a Stage; Reason payloads are MetricName or RelPath only; Reason is not a std Error
- [Phase 03]: 03-04: TokenRates private fields drop the _token suffix and serde renames them to the TOML keys; accessors keep the *_token names
- [Phase 03]: 03-04: Cost::of with Some(model) always requires the model in prices.toml (fails closed even at zero tokens); Cost and ApproxCost have no serde, so no dollar type can be persisted
- [Phase 03]: 03-04: LogExcerpt drops leftover UTF-8 continuation bytes after a cut, then trims leading chars if <U+XXXX> expansion passes the cap
- [Phase 03]: 03-05: Leaf parsers return ValueError (allowed range only); the manifest attributes it to a dotted field path as InvalidField
- [Phase 03]: 03-05: base_images checks every comment line for escape and syntax directives and fails closed; ImageError gains MalformedFrom and EscapeDirective
- [Phase 03]: 03-05: Judge paths also refuse . and .., ./, and an option without = containing a path (JudgeArgPath)
- [Phase 03]: 03-05: One proposer per tier, stored in tier order; spend_usd may be 0; judge label 1 to MAX_JUDGE_LABEL
- [Phase 03]: 03-05: JudgeVersion golden vector 3a2f24af...bc12 over TreeId::of(b"judge") and the CLAUDE.md example judge spec
- [Phase 03]: 03-06: The applier returns OutOfRange when a hunk appends after a base line lacking its newline or ends the new side without a newline before the base ends; only a file's last line can lack a newline
- [Phase 03]: 03-06: diff_trees renders sections from diffy's parsed Patch structure, not its Display, then validates with Diff::parse, so equal candidate trees give byte-identical cumulative diffs
- [Phase 03]: 03-07: BackendInfo deserializes through RawBackendInfo with strictly ascending layers, so each value has one wire form
- [Phase 03]: 03-07: Record and Event are non_exhaustive; Isolation and DecisionKind stay exhaustive for the report and breaker
- [Phase 03]: 03-07: Ledger frozen with eleven events and documented EventKeys before any golden wire file (CORE-08)
- [Phase 03]: 03-08: render refuses a sample whose names are not exactly the params (Undeclared, Unassigned) or whose value is outside its domain (Domain)
- [Phase 03]: 03-08: sweep anchors match base lines only, never an earlier param's edit, so render is independent of edit order
- [Phase 03]: 03-08: the sweep's ChaCha8 stream is pinned to the published TC1 8-round vector; the golden samples were reproduced by an independent Python ChaCha8
- [Phase 03]: 03-09: A job trial index must be below MAX_TRIALS (0 to 31); JobRequest::candidate refuses a diff blob that is not BlobId::of the canonical diff (DiffMismatch)
- [Phase 03]: 03-09: Job JSON uses an integer v (1); ledger records keep the string tag; JobResult.logs is a stage-keyed JSON object
- [Phase 03]: 03-09: The first golden wire files are 24 reviewed insta snapshots plus one SHA-256 pin over their bodies, computed with shasum from the .snap files
- [Phase 03]: a second RunStarted equal to the first is a skipped duplicate; a different one is DuplicateRunStarted
- [Phase 03]: FoldError gained ManifestConflict, since two ManifestChanged for one generation have different keys
- [Phase 03]: next_note_ordinal returns Option<u32>; None once ordinal u32::MAX is used, so a note is never silently dropped
- [Phase 03]: the fold refuses a Kept whose incumbent is not incumbent_before(generation), matching 03-11 conclude
- [Phase 03]: 03-11: conclude re-checks every recorded Scored trial with check_scorecard; a failing trial invalidates the experiment
- [Phase 03]: 03-11: circuit_breaker checks the empty-generation streak before the infra streak; the loop supplies the infra streak in memory
- [Phase 03]: 03-11: PolicyError::GenerationZero; generation 0 is calibration and is never concluded
- [Phase 03]: 03-12: build_report refuses a target whose diff blob does not hash to its ExperimentId, and any blob that does not match its BlobId
- [Phase 03]: 03-12: summary clears_noise is the delta magnitude above NOISE_WARN_MULTIPLE x spread; a zero spread counts as no noise floor, so the anomaly flag falls back to the largest earlier lineage step
- [Phase 03]: 03-12: provenance lists every EXPECTED_KRUN_LAYERS member missing from any lineage trial as reduced hardening; only weak isolation raises a flag
- [Phase 03]: 03-13: minos-core exposes explicit crate-root pub use re-exports and keeps every module public
- [Phase 03]: 03-13: GATE-06 is marked green only after cargo clean runs; no cargo command runs after it in the phase

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 2] Stop-and-ask: if the safe libkrun wrapper fails to boot (SPIKE-01), stop and ask the user before any `unsafe` exception (SPIKE-08); Phase 5 cannot start without the answer
- [Phase 2] Seatbelt deny-default vs VM creation is unverified; fallback is a dedicated unprivileged user, recorded in provenance, never silent
- [Phase 2/6] `claude -p` behavior under subscription login is unverified (tools off with `--json-schema`, `--max-turns 1`, usage field names)
- [Phase 5] The container engine for `env build` on macOS depends on the SPIKE-06 answer
- [Phase 8] The default baseline repeat count for the noise floor should come from measured variance across VM boots, not a guess
- [Phase 8] F-01 decided (01-UAT.md test 6, option A; plan 01-05): minos-cli uses the clap builder API, and clap's `derive` feature is off, because clap_derive's `#[allow(clippy::restriction)]` is E0453 under forbid. Any later wish for a clap derive goes back to the user; no lint level changes for it

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-10-14T16:54:42.857Z
Stopped at: Phase 4 context gathered
Resume file: .planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md
