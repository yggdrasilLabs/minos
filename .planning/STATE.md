---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 02
current_phase_name: platform-spikes
status: executing
stopped_at: Completed 02-04-PLAN.md
last_updated: "2026-10-10T13:34:46.684Z"
last_activity: 2026-10-10
last_activity_desc: Phase 02 execution started
progress:
  total_phases: 8
  completed_phases: 1
  total_plans: 16
  completed_plans: 9
  percent: 13
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-09)

**Core value:** Untrusted experiment code runs in a hardware-virtualized sandbox with no network, cannot touch the judge that scores it, and nothing it produces reaches the user's machine without an acknowledged report.
**Current focus:** Phase 02 — platform-spikes

## Current Position

Phase: 02 (platform-spikes) — EXECUTING
Plan: 5 of 11
Status: Ready to execute
Last activity: 2026-10-10 — Phase 02 execution started

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 5
- Average duration: -
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 5 | - | - |

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

Last session: 2026-10-10T13:34:46.680Z
Stopped at: Completed 02-04-PLAN.md
Resume file: None
