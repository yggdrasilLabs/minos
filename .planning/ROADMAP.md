# Roadmap: Minos

## Overview

Milestone v1 builds a fully local Minos on the author's macOS arm64 machine. Every experiment runs in a libkrun microVM with no network path, a separate judge VM scores it, and nothing reaches the user's machine without an acknowledged report. The work starts with a workspace whose gates hold from the first commit. Spikes then settle the platform's unverified facts while the pure core is built in parallel. Store and sandbox types turn the isolation invariants into compile-time facts before any VM exists. The krun backend and the proposers proceed in parallel. The runner and loop join them, with the loop proven against fakes first. The milestone is done when `minos check`, `minos run`, `minos report`, and `minos adopt` complete end to end on the shipped sample project with every experiment in a VM. AWS, GPU, Linux backends, and the `planned` and `agentic` tiers are later milestones.

## Cross-Cutting Rules

These rules apply to every phase and are not mapped to any single phase.

- **GATE-06: `cargo clean` after verification.** Every phase ends with `cargo clean` once verification passes. It is the last success criterion of every phase below.
- **`rules/` without exception.** All code follows the rules in `rules/` and the workspace lints from Phase 1 (`unsafe_code`, `unwrap_used`, `expect_used`, and `indexing_slicing`, all `forbid`).
- **Security invariants are not negotiable.** Never weaken isolation, judge integrity, or the review gate to make something compile or pass. That means no escape hatch, `pub` field, `Deref`, or temporary bypass on `Untrusted<T>`, `SandboxSpec`, `LaunchPlan`, or `Cleared`. If an invariant blocks progress, stop and ask the user.
- **CLAUDE.md stays current.** When a decision changes, CLAUDE.md is edited in the same change. Spike results are recorded there before code depends on them.

## Dependencies and Parallelism

- Phase 1 (Workspace and Gates) depends on nothing. It unblocks Phases 2 and 3.
- Phase 2 (Platform Spikes) depends on Phase 1 and runs in parallel with Phase 3.
- Phase 3 (Core Types and Pure Logic) depends on Phase 1 and runs in parallel with Phase 2.
- Phase 4 (Store, Source Snapshot, and Sandbox Types) depends on Phase 3. It can overlap Phase 2 and Phase 6.
- Phase 5 (Guest, krun Backend, and Environment Build) depends on Phases 2 and 4. It runs in parallel with Phase 6 and with the loop plans of Phase 7.
- Phase 6 (Proposers) depends on Phases 2 and 3. It runs in parallel with Phases 4 and 5.
- Phase 7 (Runner and Loop) depends on Phases 4, 5, and 6. Its loop plans can start before Phase 5 finishes (see below).
- Phase 8 (CLI, Sample Project, and Docs) depends on Phases 5 and 7.

Facts that matter for scheduling:

- **Spikes and core run in parallel.** Phases 2 and 3 both need only the Phase 1 workspace. No core type depends on a spike result, so the spikes cost the schedule nothing.
- **Proposers run alongside the VM work.** Phase 6 needs only the `claude -p` contract from SPIKE-07 and the core types, so it runs in parallel with Phases 4 and 5.
- **The loop is built against fakes first.** The loop half of Phase 7 (LOOP-04 through LOOP-09) is built and tested against the in-memory store, a fake provider, and a fake job runner with no VM (LOOP-09). It can start once Phases 4 and 6 are done, before Phase 5 finishes. Only the runner's real-VM work (LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04) waits on Phase 5. Phase 7 lists Phase 5 as a dependency so tools do not schedule the real-VM plans early.
- **Event set before golden files.** CORE-08 (freeze the `Event` set with a documented `EventKey` per event) lands before any golden wire-format file in Phase 3.
- **Stop-and-ask in Phase 2.** If the safe libkrun wrapper cannot boot (SPIKE-01), work stops and the user is asked before any `unsafe` exception is added (SPIKE-08). Phase 5 cannot start without that answer.

**Research flags:** Phases 2, 5, and 6 need `/gsd-plan-phase --research-phase <N>` (the spikes, the krun backend, and `claude -p` behavior). Phases 1, 3, 4, 7, and 8 follow standard patterns from the research files and the design of record.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Workspace and Gates** - Five plain cargo commands enforce lints, supply-chain policy, and crate layering from the first commit (completed 2026-10-09)
- [x] **Phase 2: Platform Spikes** - libkrun boot, egress, vsock, signing, confinement, root images, and `claude -p` answered with evidence in CLAUDE.md (completed 2026-10-10)
- [x] **Phase 3: Core Types and Pure Logic** - Validated types, a frozen ledger format, and pure policy, routing, report, and money in `minos-core` (completed 2026-10-14)
- [x] **Phase 4: Store, Source Snapshot, and Sandbox Types** - Crash-safe ledger and blobs, a secret-scanned source tarball, and isolation invariants as compile-time facts (completed 2026-10-15)
- [x] **Phase 5: Guest, krun Backend, and Environment Build** - Experiments run in no-network libkrun VMs from disk images, with `env build` and `host doctor` (completed 2026-10-19)
- [x] **Phase 6: Proposers** - Seeded sweep and a locked-down `claude -p` single_shot behind the `ModelProvider` abstraction (completed 2026-10-20)
- [x] **Phase 7: Runner and Loop** - Candidate-then-judge VMs and a resumable generation driver with caps and a circuit breaker (completed 2026-10-24)
- [ ] **Phase 8: CLI, Sample Project, and Docs** - The full loop on the sample project, hash-gated adopt, measured cost, and honest docs

## Phase Details

### Phase 1: Workspace and Gates

**Goal**: Every line of code in the repo, from the first commit, is held to the workspace lints, the supply-chain policy, and the crate-layering rules by five plain cargo commands (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`), with all policy in Cargo-native configuration files and no script
**Depends on**: Nothing (first phase)
**Parallelism**: Unblocks Phases 2 and 3, which then run in parallel
**Requirements**: GATE-01, GATE-02, GATE-03, GATE-04, GATE-05
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. On a fresh checkout with the v1 crate skeletons (including `minos-core`, `minos-loop`, and `minos-sandbox`), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps` all pass on rustc 1.97.1 with edition 2024, with no script in front of them, and `Cargo.lock` is committed (GATE-01, GATE-02)
  2. Each deliberate code violation makes `cargo clippy --workspace --all-targets -- -D warnings` fail: an `unsafe` block, an `unwrap`, `expect`, or slice index (also in a crate whose manifest omits `[lints] workspace = true`, through the `.cargo/config.toml` backstop), an outer `#[allow(clippy::...)]` with or without a reason, an inner `#![allow(clippy::...)]` without a reason, any `allow` or `#[expect]` of the `forbid` lints `unwrap_used`, `expect_used`, and `indexing_slicing`, and a `SystemTime::now` or `HashMap` in `minos-core`. For other lints `#[expect(..., reason = ...)]` passes, and an inner `#![allow(..., reason = ...)]` is not caught. A broken intra-doc link makes `cargo doc --workspace --no-deps` fail (GATE-01, GATE-05)
  3. Each supply-chain or layering violation makes `cargo deny check` fail: a git dependency without `rev`, an unknown registry or git source, a build script not on the allowlist, and a dependency from `minos-core` or `minos-loop` on `minos-sandbox` or any backend crate; a normal `minos-core` to `minos-sandbox` edge fails earlier, as a Cargo dependency cycle (GATE-03, GATE-04)
  4. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 5/5 plans complete
Plans:
**Wave 1**

- [x] 01-01-PLAN.md — Workspace skeleton: toolchain pin, root manifest with the lint baseline and exact pins, ten stub crates, minos-core clippy.toml, licenses (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 01-02-PLAN.md — deny.toml supply-chain policy and per-crate layering bans; all four gate commands green (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 01-03-PLAN.md — Gate self-test (every D-11 violation captured verbatim, then reverted), gate wording fix in REQUIREMENTS/ROADMAP, open decisions recorded, `cargo clean` (wave 3)

**Wave 4** *(gap closure for UAT tests 1 to 3, blocked on Wave 3 completion)*

- [x] 01-04-PLAN.md — Three clippy lints to forbid, `.cargo/config.toml` rustflags backstop, `cargo doc --workspace --no-deps` as the fifth gate command; closing proofs G-01 to G-03, five-command final gate, `cargo clean` (wave 4)

**Wave 5** *(gap closure for UAT test 6, blocked on Wave 4 completion)*

- [x] 01-05-PLAN.md — F-01 decided (A): drop clap's `derive` feature so `minos-cli` uses the builder API, regenerate `Cargo.lock`, re-check the build-script allowlist, correct the clap lines in `.claude/CLAUDE.md` and `research/STACK.md`; F-01-CLOSED, P-01, B-02, five-command final gate, `cargo clean` (wave 5)

**Research**: Standard patterns (cargo-deny and lint recipes are verified in research/STACK.md); skip research-phase

### Phase 2: Platform Spikes

**Goal**: Every unverified fact that the backend and proposer designs rest on has an evidence-backed answer recorded in CLAUDE.md, and CLAUDE.md describes the v1 local-first design
**Depends on**: Phase 1
**Parallelism**: Runs in parallel with Phase 3; no core work waits on a spike result
**Requirements**: SPIKE-01, SPIKE-02, SPIKE-03, SPIKE-04, SPIKE-05, SPIKE-06, SPIKE-07, SPIKE-08, DOCS-05
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. The safe libkrun wrapper, called from crates under `forbid(unsafe_code)`, boots libkrunfw on this Mac from a raw read-only SquashFS root disk whose `/init` is a static musl binary, with no initramfs and no virtiofs, and the boot test asserts the exact `/proc/cmdline`; an ad-hoc codesigned helper with the `com.apple.security.hypervisor` entitlement starts that VM, and an unsigned copy is refused (SPIKE-01, SPIKE-04) (*superseded wording, 2026-10-09: "from an initramfs whose `/init` is a static musl binary, with a raw read-only root disk"; stock libkrunfw 5.6.2 has `CONFIG_BLK_DEV_INITRD` unset, 02-CONTEXT.md D-18*)
  2. With implicit vsock disabled and a plain vsock added, a guest `connect()` to host loopback and to a public IP both fail, and a vsock port mapped to a host Unix socket carries a byte-capped guest-to-host connection end to end; on the pinned VMM one connection is reliable up to 8192 bytes, so guest output is chunked into short connections of at most 4 KiB each, built and tested in Phase 5 (SPIKE-02, SPIKE-03) (*reworded 2026-10-10 by the user's decision, option 1: the egress half is met (02-SPIKE-EVIDENCE.md S02-EGRESS); the connection half is met up to 8192 bytes per connection, above which the pinned `msb_krun_devices` 0.1.41 loses or stalls data for three causes outside Minos (DIAG-01, S03-ANSWER); the user chose to work within the limit with a chunked output protocol in Phase 5 and rejected a fork for now*)
  3. Three open questions have recorded, evidence-backed answers: whether a Seatbelt deny-default profile still permits VM creation (with the dedicated unprivileged user fallback recorded), which documented tool chain turns a user Dockerfile into a root image on macOS, and whether `claude -p` with all tools disabled, `--json-schema`, `--no-session-persistence`, an empty cwd, and an allowlisted environment returns `structured_output` under subscription login, including the usage field names and `--max-turns 1` behavior (SPIKE-05, SPIKE-06, SPIKE-07)
  4. CLAUDE.md records every spike result and the v1 amendments (local-first build order, the `ModelProvider` decision, no tokio in v1, pico-dollar money, the `libkrun/krun` tap, the libkrun networking default, the `minos-loop` crate), and the workspace contains no `unsafe` exception; if the boot spike failed, work stopped and the user's decision was recorded before any alternative was tried (DOCS-05, SPIKE-08)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 11/11 plans complete
Plans:
**Wave 1**

- [x] 02-01-PLAN.md — Foundation and guest: musl target with the backstop proven, every wave-2 dependency edge, libkrunfw install, the static `/init` and its job and status-frame protocol (wave 1)
- [x] 02-02-PLAN.md — SPIKE-07: `claude -p` contract under subscription login, recorded verbatim with PII and cost estimates redacted (wave 1)

**Wave 2** *(blocked on 02-01)*

- [x] 02-03-PLAN.md — `minos-vmm` launcher: validated launch config, preflight, root-disk and initramfs boot modes, watchdog exit codes, clippy bans, entitlements plist (wave 2)
- [x] 02-04-PLAN.md — Root image (SquashFS) and newc initramfs writers in Rust, clippy bans, Docker buildx image production twice (wave 2)
- [x] 02-05-PLAN.md — Host supervisor, launch-config serializer, job input disk, capped `GuestBytes` (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 02-06-PLAN.md — SPIKE-04 signing, SPIKE-01 root-disk boot with the exact `/proc/cmdline`, initramfs negative evidence, Dockerfile image boot; SPIKE-08 stop-and-ask checkpoint (wave 3, not autonomous)

**Wave 4** *(blocked on 02-06)*

- [x] 02-07-PLAN.md — K7 exit map: how guest exits, watchdog trips, signals, and refusals surface to the parent (wave 4)
- [x] 02-08-PLAN.md — SPIKE-02 egress fails closed and SPIKE-03 capped vsock stream, with K4 throughput (wave 4)
- [x] 02-09-PLAN.md — SPIKE-05 Seatbelt deny-default profile answer and the dedicated-user recipe (wave 4)

**Wave 5** *(blocked on Wave 4 and 02-02)*

- [x] 02-10-PLAN.md — Evidence consolidation, the boot-path and signing decision record with SPIKE-01, KRUN-08, KRUN-09 and roadmap wording superseded, D-09 corrections to both CLAUDE files (wave 5)

**Wave 6** *(blocked on 02-10)*

- [x] 02-11-PLAN.md — Remaining DOCS-05 amendments to CLAUDE.md, final gate, spike re-run, `cargo clean` (wave 6)

**Stop-and-ask point**: If SPIKE-01 fails (the safe libkrun wrapper cannot boot libkrunfw on this machine), stop and ask the user before doing anything else. The remaining options, such as an `unsafe` binding crate, a different VMM, or waiting for upstream, need the user's explicit approval. Never add `#![allow(unsafe_code)]` or any other `unsafe` exception quietly (SPIKE-08).
**Research**: Needs research-phase. The spikes are the research; each SPIKE item is a deliverable with a recorded result.

### Phase 3: Core Types and Pure Logic

**Goal**: Every value the loop decides on exists only as a validated, pure, deterministic type in `minos-core`. An experiment that edits the judge, a NaN metric, or a float dollar amount cannot be represented, and the ledger wire format is frozen and guarded from its first golden file.
**Depends on**: Phase 1
**Parallelism**: Runs in parallel with Phase 2
**Requirements**: CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06, CORE-07, CORE-08, CORE-09, CORE-10, CORE-11, CORE-12, CORE-13, CORE-14, CORE-15, CORE-16, CORE-17, COST-01
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. Invalid input cannot become a value. `RelPath` rejects `..`, absolute, empty, and non-normal paths under property tests. `Manifest::try_from` rejects unknown fields, `writable` paths that overlap readable or judge paths, out-of-limit budgets, a policy that does not match the objectives, and a ladder with no eligible tier. Image tags are rejected by `ImageRef` and by `image::base_images` on the Dockerfile's `FROM` lines, because the v1 manifest names a Dockerfile rather than an image; `Manifest::try_from` stays pure (amended 2026-10-14, user decision). `Diff::parse` rejects non-writable paths, renames, mode changes, symlinks, binary patches, and oversize diffs. `Metric` rejects NaN and infinities and normalizes `-0.0`. `Outcome` carries failure as a value (CORE-02, CORE-03, CORE-04, CORE-06, CORE-07)
  2. Identity and wire formats are stable. An accepted diff re-serializes to canonical bytes that `ExperimentId` hashes, and the strict exact-position applier applies it to an in-memory tree and fails on any context mismatch. Golden vectors pass for every distinct ID type. The `Event` set (including `Steered`, `StopRequested`, the trial index on `Judged`, and model ID, prompt hash, and rationale blob on `Proposed`) is frozen with a documented `EventKey` per event before any golden file exists, after which `Record::V1` golden files pass and property tests show the fold rebuilds identical run state from the same events (CORE-01, CORE-04, CORE-05, CORE-08, CORE-09)
  3. Pure decisions are deterministic. `select_greedy` returns the same choice under any permutation of results while honoring `min_improvement` and breaking ties on `ExperimentId`. Routing climbs one tier after `STALL_STEP` stalled generations and gives a reason for every ineligible tier. The seeded sweep sampler repeats exactly for the same seed. The circuit breaker returns stop after K generations with no valid candidate or K consecutive infrastructure failures. `build_report` yields the same bytes and hash for the same ledger and blobs, with the noise-floor summary, cumulative diff, lineage, provenance, and the five v1 flags (CORE-10, CORE-11, CORE-12, CORE-16, CORE-17)
  4. Money, contracts, and display are safe by construction. `Spent` carries usage only, never dollars. Price rates parse from strings into pico-dollar `u128` with integer micro-dollar totals and no float. `JobRequest` and `JobResult` define the loop-to-runner contract. The renderer strips C0, C1, and ESC sequences and shows bidi and zero-width characters visibly (CORE-13, CORE-14, CORE-15, COST-01)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 13/13 plans complete
Plans:
**Wave 1**

- [x] 03-01-PLAN.md — Foundation: serde_json `float_roundtrip`, `.config/insta.yaml`, the full module skeleton, `limits.rs`, ID hashing with golden vectors, and `RelPath` (wave 1)

**Wave 2** *(blocked on 03-01)*

- [x] 03-02-PLAN.md — `Diff::parse` seven-stage pipeline, canonical form, `ExperimentId::of`, CLAUDE.md Diff paragraph (wave 2)
- [x] 03-03-PLAN.md — `Metric`, `Scorecard`, `Outcome`, `Reason`, `Stage` (wave 2)
- [x] 03-04-PLAN.md — `Usage`, pico-dollar `Rate`, `Prices`, `Cost`, `ApproxCost`, and the `SafeText` allowlist renderer (wave 2)
- [x] 03-05-PLAN.md — `Manifest::try_from` with every cross-field rule, digest-only `ImageRef`, manifest-level eligibility, `JudgeVersion` (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 03-06-PLAN.md — Strict exact-position applier, cumulative `compose`, `Experiment` (wave 3)
- [x] 03-07-PLAN.md — CORE-08 freeze: the eleven-variant `Event` set with a documented `EventKey` per event, `Record::V1`, `Line`, CLAUDE.md ledger and manifest paragraphs (wave 3)

**Wave 4** *(blocked on Wave 3 completion; first golden wire files)*

- [x] 03-08-PLAN.md — Seeded sweep sampler and sample rendering (wave 4)
- [x] 03-09-PLAN.md — `JobRequest`/`JobResult` and the golden wire snapshots with a byte pin (wave 4)
- [x] 03-10-PLAN.md — The fold to `RunState`, permutation-invariant and tamper-detecting (wave 4)

**Wave 5** *(blocked on 03-10)*

- [x] 03-11-PLAN.md — Gate check, median reduction, `select_greedy`, `conclude`, `noise_check`, routing with reasons, circuit breaker, `spend_ok` (wave 5)

**Wave 6** *(blocked on 03-11)*

- [x] 03-12-PLAN.md — `build_report`: summary, flags, provenance, lineage, cumulative diff, `ReportHash`, golden reports (wave 6)

**Wave 7** *(blocked on all prior plans)*

- [x] 03-13-PLAN.md — Crate-root re-exports, CLAUDE.md, validation map, deliberate-violation evidence, full gate, then `cargo clean` (wave 7)

**Ordering constraint**: CORE-08 (freeze the `Event` set and document every `EventKey`) lands before any golden wire-format file is committed
**Research**: Standard patterns; skip research-phase

### Phase 4: Store, Source Snapshot, and Sandbox Types

**Goal**: Run history survives crashes in an append-only ledger and a content-addressed blob store. Project source enters the system only as a deterministic, secret-scanned snapshot. The compiler rejects any sandbox configuration that would give a candidate judge data or a network, before any VM exists.
**Depends on**: Phase 3
**Parallelism**: Can overlap Phase 2 and Phase 6
**Requirements**: STORE-01, STORE-02, STORE-03, STORE-04, SBX-01, SBX-02, SBX-03, SBX-04, SBX-05, SBX-06, CLI-11
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. The file ledger survives a crash: a torn final line is tolerated on reopen, unknown `v` tags are refused, no record is ever rewritten, and appending an already-recorded `EventKey` is a no-op. The file blob store writes temp-then-rename, and a tampered blob fails its SHA-256 check on read. The in-memory `Ledger` and `Blobs` pass the same synchronous trait tests (STORE-01, STORE-02, STORE-03, STORE-04)
  2. `trybuild` compile-fail tests prove that a `SandboxSpec<Candidate>` cannot receive judge data, that no role can attach a network, that a `LaunchPlan` comes only from `seal()`, and that `launch` cannot be called without a `Cleared`, which only `preflight` mints and which owns its plan. Every plan carries a hard maximum duration and a launch key derived from `ExperimentId`, role, and trial (SBX-01, SBX-02, SBX-03, SBX-06)
  3. `Untrusted<T>` exposes no `Deref`, `Display`, or `Debug` and exits only through the capped `into_scorecard`, `into_log_excerpt`, and `into_diff_texts`. The output tar reader accepts regular files only, rejects path traversal and over-cap entry counts, entry sizes, and totals, never writes to disk, and survives its fuzz corpus (SBX-04, SBX-05)
  4. Packing the same working tree twice yields byte-identical tarballs and the same `TreeId` (sorted paths, zeroed metadata, regular files only), honoring `.gitignore`, `.minosignore`, and the hard denylist. A planted secret blocks the pack, and a dirty tree is refused unless `--allow-dirty` is given (CLI-11)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 9/9 plans complete
Plans:
**Wave 1**

- [x] 04-01-PLAN.md — New `minos-source` crate, every Phase 4 dependency edge, `deny.toml` layering, three crate `clippy.toml` files with each ban proven to fire, CLAUDE.md Workspace (wave 1)

**Wave 2** *(blocked on 04-01)*

- [x] 04-02-PLAN.md — `Ledger` and `Blobs` sync traits, `Appended`, `StoreError`, `MemoryStore`, shared contract tests (wave 2)
- [x] 04-03-PLAN.md — Sealed roles, `LaunchKey` with golden vectors, `SandboxSpec`, `seal`, `LaunchPlan`, `Capabilities`, `Refusal`, `Cleared`, `preflight` (wave 2)
- [x] 04-04-PLAN.md — Hard denylist, nine-pattern secret scanner, read-only git clean check, CLAUDE.md git carve-out (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 04-05-PLAN.md — `FileStore`: locked, durable JSONL ledger with torn-tail repair and corruption refusals; atomic, verified blobs (wave 3)
- [x] 04-06-PLAN.md — `Untrusted<T>`, role-typed `GuestOutput<R>`, `Finished<R>`, raw-mode output tar reader with committed hostile corpus and properties, the three capped exits (wave 3)
- [x] 04-07-PLAN.md — `snapshot`: four-source walk, offenders, caps, negation refusal, scan, clean check, deterministic ustar tarball and `TreeId`, CLAUDE.md Source rules (wave 3)

**Wave 4** *(blocked on 04-06)*

- [x] 04-08-PLAN.md — `Backend` and `RunningVm`, fake-backend flow, `trybuild` compile-fail suite (12 cases plus a pass case), CLAUDE.md sandbox signatures (wave 4)

**Wave 5** *(blocked on all prior plans)*

- [x] 04-09-PLAN.md — CLAUDE.md store section and open questions, validation map, deliberate-violation evidence, full gate, then `cargo clean` (wave 5)

**Research**: Standard patterns; skip research-phase
**Note (2026-10-14, from the Phase 3 plan check)**: `RelPath` (Phase 3, CORE-02) is ASCII-only in v1: each component matches `[A-Za-z0-9._+-]+`, is not `.` or `..`, and does not start with `-`. Phase 4's source packing (CLI-11) and hostile-tar parsing (SBX-05) inherit that grammar. Phase 4 must either live with it (refuse a project file or output entry whose name falls outside it, with a clear error) or widen it with a property test that keeps every CORE-02 rejection.

### Phase 5: Guest, krun Backend, and Environment Build

**Goal**: An experiment runs inside a libkrun microVM on this Mac with no network path, no host environment, read-only disk-image inputs, and a capped tar stream out, and no VM outlives the process that launched it
**Depends on**: Phase 2, Phase 4
**Parallelism**: Runs in parallel with Phase 6 and with the loop plans of Phase 7
**Requirements**: GUEST-01, GUEST-02, GUEST-03, GUEST-04, GUEST-05, KRUN-01, KRUN-02, KRUN-03, KRUN-04, KRUN-05, KRUN-06, KRUN-07, KRUN-08, KRUN-09, KRUN-10, CLI-02, CLI-03
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. The three permanent exit tests pass on this Mac. A guest `connect()` to host loopback and to a public IP fails closed (KRUN-03). Reading `env` and `/proc/cmdline` inside the VM shows no host values (GUEST-05). `kill -9` on the process that launched a VM, which stands in for `minos run`, leaves no VM and no `minos-vmm` helper running (KRUN-05)
  2. Every VM runs in its own signed `minos-vmm` helper, spawned with a cleared environment, stdin from `/dev/null`, and a capped console pipe. It is the only binary that links the VMM and carries the hypervisor entitlement, which the documented `codesign` argv in CLAUDE.md Commands applies after every build and `codesign -d --entitlements -` verifies (*superseded wording, 2026-10-09: "which the `xtask` applies"; no xtask, 02-CONTEXT.md D-11*). Every launch disables implicit vsock and adds a plain vsock with no socket proxying, clippy rejects the hijack and host-directory-sharing APIs, and the supervisor kills the helper's process group at the hard maximum duration (KRUN-01, KRUN-02, KRUN-05, KRUN-08)
  3. Two sequential experiments on this host each boot the static musl guest as PID 1, which reaps children and creates nothing per-run at startup, from a shared read-only root disk and a per-job read-only input disk. Each applies the base tarball plus canonical diff with the core applier (a mismatch yields `Rejected`), runs manifest argv commands as a non-root uid under rlimits writing only to size-capped scratch space in guest memory (*superseded wording, 2026-10-18: 'a size-capped scratch disk'; 05-CONTEXT.md D-05*), and returns a byte-capped tar stream over vsock. Both record outcomes and share no build output, scratch is deleted unread, no host directory is ever shared, and only the judge-role channel can deliver a scorecard (GUEST-01, GUEST-02, GUEST-03, GUEST-04, KRUN-04, KRUN-10)
  4. `minos env build` turns the project Dockerfile into a raw read-only SquashFS root image with `/init` written in by the trusted packer (*superseded wording, 2026-10-09: "a root image and an initramfs"; 02-CONTEXT.md D-18*) and writes its digests to `minos.lock`, from a build context with no judge data and no secrets. `minos host doctor` runs the launch preflight (entitlement, kernel presence and hash, Hypervisor.framework support, socket path length, active confinement layers) and prints every refusal with an explanation and a fix. The active confinement layers are recorded in `BackendInfo`, with no silent fallback (KRUN-06, KRUN-07, KRUN-09, CLI-02, CLI-03)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 22/22 plans complete
Plans:
**Wave 1**

- [x] 05-01-PLAN.md — Core machine bounds and D-05 memory budget in limits, `[environment] memory_mib`/`scratch_mib` (D-06), shared output caps, streaming `BlobHasher` (D-09)
- [x] 05-02-PLAN.md — Core authenticated chunk codec with per-launch token and reassembler; `minos.lock` type with golden file (D-10, D-11)

**Wave 2** *(blocked on Wave 1)*

- [x] 05-03-PLAN.md — Wave 0 VM proofs (chunks at 16 MiB with the S03-ANSWER measurement, console on `/dev/fd/1` under Seatbelt, store-path root image) recorded in 05-EVIDENCE.md with mechanism selections and fallbacks (conditional stop: `chunks: stop` ends at a user checkpoint)
- [x] 05-04-PLAN.md — Core guest wire: `GuestJob`, `GuestStatus`, input disk v2 codec with the token (D-15); each role's inputs bound to half the guest reserve
- [x] 05-05-PLAN.md — Store `put_reader` streaming put and `FileStore::verified_path` (D-09, D-14)

**Wave 3** *(blocked on Wave 2)*

- [x] 05-06-PLAN.md — Sandbox: `BlobSource` (D-13), `InputDisk` as part IDs (D-15), Capabilities layers and kernel, Seatbelt-mandatory preflight (D-03), refusals
- [x] 05-07-PLAN.md — minos-vmm v2: no scratch disk or initramfs (D-05), selected console, explicit rng, capped logger, own rlimits, new clippy bans, golden config; KRUN-04 amendment recorded
- [x] 05-08-PLAN.md — Guest `JobSource`/`OutputSink` (D-07), `ChunkSink`, fixed uid 64000 and stage limits, trampoline argv (D-08)

**Wave 4** *(blocked on Wave 3)*

- [x] 05-09-PLAN.md — Embedded Seatbelt profile and argv, root image mount points, static-ELF check; CLAUDE.md for the profile and the VM tests
- [x] 05-10-PLAN.md — Guest unpack and strict apply, link-free output packer, `run_job` orchestration
- [x] 05-21-PLAN.md — Spike and wave0 tests retired with mapping; backend serializer v2 bound to the golden config; supervisor scratch and initramfs paths trimmed
- [x] 05-22-PLAN.md — Sandbox: `Finished` elapsed and console, `Ended::ChunkProtocol`, `take_status`

**Wave 5** *(blocked on Wave 4)*

- [x] 05-11-PLAN.md — Backend host probes into Capabilities (KRUN-06, KRUN-08), job directories with owner lock and sweep (KRUN-05)
- [x] 05-12-PLAN.md — Guest PID 1 and trampoline on Linux, `/dev/vdb` removed after read, `minos-probe` fixture, spike protocol removed

**Wave 6** *(blocked on Wave 5)*

- [x] 05-13-PLAN.md — Backend input disk writer with `/dev/urandom` token and plan binding; poll-based chunk receiver; initramfs writer removed
- [x] 05-14-PLAN.md — CLI skeleton, D-04 locator, `FileBlobSource`, `minos host doctor` (CLI-03) and `minos host setup` (D-02)

**Wave 7** *(blocked on Wave 6)*

- [x] 05-15-PLAN.md — Production supervisor with whole-line marker classification and `KrunBackend` implementing `Backend`
- [x] 05-16-PLAN.md — `minos env build` with Docker buildx, packer, streaming put, `minos.lock`, `--update` (KRUN-09, CLI-02)

**Wave 8** *(blocked on Wave 7)*

- [x] 05-17-PLAN.md — VM harness and functional VM tests (apply, uid and limits, rejected diff, two experiments, 16 MiB output, judge role, stage timeout)
- [x] 05-18-PLAN.md — Docker env build run with reproducibility evidence (checkpoint: start Docker Desktop)

**Wave 9** *(blocked on 05-17)*

- [x] 05-19-PLAN.md — Permanent exit tests `exit_egress`, `exit_env`, `exit_kill9`, plus `exit_forge` (checkpoint: the user confirms the one `KRUN_BOOT_START_NS` cmdline allowance first)

**Wave 10** *(blocked on all prior plans)*

- [x] 05-20-PLAN.md — CLAUDE.md reconciliation, final gate with VM and exit tests recorded, requirements, validation, `cargo clean`

**Exit criteria**: Success criterion 1 (fail-closed egress, no host environment, no VM after `kill -9`) must pass before this phase is marked complete. It is never traded for schedule.
**Research**: Needs research-phase (Seatbelt and `hv_vm_create`, first boot, root-image tooling on macOS, concurrent VM and memory limits)

### Phase 6: Proposers

**Goal**: The loop can get candidate diffs from a seeded sweep with no model, or from one text-only call to the user's own `claude` install that runs no tools, loads no project configuration, and handles no credentials, with model output treated as hostile
**Depends on**: Phase 2, Phase 3
**Parallelism**: Runs in parallel with Phases 4 and 5
**Requirements**: PROP-01, PROP-02, PROP-03, PROP-04, PROP-05, PROP-06, PROP-07, PROP-08, PROP-09, PROP-10
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. The `ClaudeCli` argv matches its golden file. The provider runs the user's own `claude` from a fresh empty temp directory outside any repo, with only `HOME`, `PATH`, `USER`, `LOGNAME`, `TMPDIR`, and `LANG` in its environment, all built-in tools disabled, a strict empty MCP config, session persistence off, and the prompt on stdin. A fixture project with a hostile `.claude/settings.json`, hooks, and `.mcp.json` proves none of them run during a proposal (PROP-02, PROP-03)
  2. A preflight confirms that the subscription login is the active auth method and refuses when an API key variable would take over. No code path reads, stores, or forwards Claude credentials. A fake provider plugs into the `ModelProvider` abstraction with no change to callers, as a later Bedrock or API-key provider will (PROP-01, PROP-04)
  3. Prompts are byte-identical for shuffled inputs under golden tests, with `program.md` and the surface files as the stable prefix and a bounded history table plus sanitized failure excerpts, labelled as untrusted text, in the dynamic suffix. A test proves held-out metric values never appear in any prompt (PROP-05, PROP-06)
  4. `single_shot` returns several `{rationale, files}` candidates, parsed into closed `deny_unknown_fields` types, from which the canonical diff is derived, and invalid candidates are dropped, not repaired. `sweep` produces proposals from declared params with no model call. Each call's token usage is returned for a `Spent` event and any CLI dollar figure is ignored. A rate-limit or usage-cap response surfaces as an infrastructure failure that stops the run with `StopRequested`, never as an `Outcome` (*superseded wording, 2026-10-19: "`{rationale, diff}` candidates ... invalid diffs are dropped"; the model returns full files and Minos derives the diff, 06-CONTEXT.md D-10, D-11*) (PROP-07, PROP-08, PROP-09, PROP-10)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 10/10 plans complete
Plans:
**Wave 1**

- [x] 06-01-PLAN.md — `StopReason::ProviderAuth` in core: variant, key tag 7, Display, prop_keys coverage, one pinned golden line (wave 1)
- [x] 06-02-PLAN.md — `minos-propose` contracts: dependencies and clippy ban, limits, output schema golden, `ModelProvider` and its types, `Prompt`, the strict output parser with closed drop reasons (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 06-03-PLAN.md — Deterministic prompt assembly: history view, stable prefix, fenced untrusted suffix, golden, shuffle, and held-out non-interference tests (wave 2)
- [x] 06-04-PLAN.md — `ClaudeCli` pure parts: parent snapshot and six-variable env, golden argv, version parsing, the corrected result classifier, the native-install locator (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 06-05-PLAN.md — Tiers: sweep over core's sampler, single_shot over any provider, `FakeProvider`, `Proposer<P>` dispatch, PROP-07 rewording (D-11) (wave 3)
- [x] 06-06-PLAN.md — Capped subprocess runner with deadline and process-group kill; the per-run preflight against a fake `claude` (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 06-07-PLAN.md — `ClaudeCli` call path with re-resolution and re-preflight, end-to-end provider-agnostic proof, CLAUDE.md provider wording (wave 4)

**Wave 5** *(blocked on 06-07)*

- [x] 06-08-PLAN.md — Ignored live suite: hostile fixture project, golden argv in the fixture, positive control; CLAUDE.md Testing and Commands (wave 5)

**Wave 6** *(blocked on 06-08; stops at a blocking checkpoint for the user's live run)*

- [x] 06-09-PLAN.md — Live provider suite run by the user on their logged-in machine (checkpoint, D-17), the redacted record, and the verified facts (wave 6)

**Wave 7** *(blocked on all prior plans)*

- [x] 06-10-PLAN.md — Final gate, requirements, validation map checked against the real test targets, then `cargo clean` (wave 7)

**Research**: Needs research-phase (`claude -p` flag interplay and subscription behavior, especially anything SPIKE-07 left open)

### Phase 7: Runner and Loop

**Goal**: Generations run end to end (route, propose, validate, candidate VM, judge VM, select, record), and an interrupted, over-budget, or failing run resumes or stops from the ledger with the same result as an uninterrupted one
**Depends on**: Phase 4, Phase 5, Phase 6
**Parallelism**: The loop plans (LOOP-04 through LOOP-09) are built and tested against the in-memory store, a fake provider, and a fake job runner once Phases 4 and 6 are done, before Phase 5 finishes. Only the runner's real-VM plans wait on Phase 5.
**Requirements**: LOOP-01, LOOP-02, LOOP-03, LOOP-04, LOOP-05, LOOP-06, LOOP-07, LOOP-08, LOOP-09, JUDGE-01, JUDGE-03, JUDGE-04, CLI-04
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. Against the in-memory store, a fake provider, and a fake job runner with no VM, the loop drives generations (route tier, propose, validate diffs, run jobs, fold `Judged`, `select_greedy`, append `Selected`) and records before acting. A run interrupted at any point resumes from the ledger to the same result as an uninterrupted run, a second driver on the same run is refused by the run lock, and `minos-loop` depends only on core, store, and propose (LOOP-04, LOOP-05, LOOP-06, LOOP-09)
  2. The run stops with a recorded `StopRequested` reason when the spend cap or the model-call or token cap would be exceeded (checked before each generation and each model call), after K generations with no valid candidate, or after K consecutive infrastructure failures (LOOP-07, LOOP-08)
  3. On real VMs, the runner runs the candidate VM and then the judge VM for a `JobRequest`. Only the judge mounts judge data, read-only, and the two share no process and no disk. Time and resource usage come from host measurement, never candidate output. A failed gate yields `Rejected` without scoring objectives. Every `Judged` records backend and architecture, a run with a speed objective stays pinned to one backend class, and `minos-runner` is the only crate that turns `Untrusted` output into an `Outcome` (LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04)
  4. `minos judge push <dir>` stores judge data in the local blob store as a new judge version with its hash recorded, and that data never appears in the source tarball or the image build context (JUDGE-01, CLI-04)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 17/17 plans complete

Plans:
**Wave 1**

- [x] 07-01-PLAN.md — Core ledger provenance: `Judged { logs }`, `RunStarted { root_image }`, `Proposed { batch }`, `JobResult` judge usage, the `RunState` accessors the loop reads, `baseline_ready`, four goldens with their pin (wave 1)
- [x] 07-02-PLAN.md — Core judge and price pieces: `[judge] trials`, the `[[judge]]` lock table with `check_judge` and the 4 MiB cap, `Prices::missing_models`, the shared caps (wave 1)
- [x] 07-03-PLAN.md — Wave 0 scaffolding: every new dependency edge from existing pins, the runner and loop clippy policies with ban proofs, 07-EVIDENCE.md (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 07-04-PLAN.md — First runner plan: the user approves the two `Untrusted` sink exits at a blocking checkpoint, then `store_judge_input` and `store_tail` with tests, a trybuild case, and CLAUDE.md (wave 2; checkpoint)
- [x] 07-05-PLAN.md — Store: a conflicting append becomes `StoreError::Conflict`; the `RunLock` trait with `driver.lock` and the memory lock (wave 2)
- [x] 07-06-PLAN.md — `BaseTree` and `CumulativeDiff` in core with compile-fail proofs; guest, krun, and minos-propose callers moved (wave 2)
- [x] 07-07-PLAN.md — Loop contracts: `JobRunner`, `DriverError`, the observer seam, and the pure `next_step` with its properties (wave 2)
- [x] 07-08-PLAN.md — `pack_judge_data` and the judge-overlap refusal in every snapshot, wired into `env build` (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 07-09-PLAN.md — Runner pure parts: blame-carrying errors, host-measured usage, the observation-to-outcome map with the gate short-circuit (wave 3)
- [x] 07-10-PLAN.md — Loop building blocks: run inputs with start and resume checks, the recorder, the batch blob and admission, the history and parent-tree readers (wave 3)
- [x] 07-11-PLAN.md — `minos judge push <dir> [--update]`, the shared lock helpers, and CLAUDE.md for judge data (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 07-12-PLAN.md — The candidate-then-judge pipeline `Runner<B, S>`, tested over a scripted fake backend (wave 4)
- [x] 07-13-PLAN.md — The generation driver: start, resume, calibrate, `run_to`, with flow tests on fakes (wave 4)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 07-14-PLAN.md — CLI `RunnerJobs` adapter and the ignored real-VM test of the production runner, run and recorded (wave 5)
- [x] 07-15-PLAN.md — Safe points, caps, the breaker, retries, session-relative stops, and the backend pin (wave 5)

**Wave 6** *(blocked on 07-14 and 07-15)*

- [x] 07-16-PLAN.md — Exhaustive crash-point resume equivalence, the second-driver refusal, and CLAUDE.md for the runner and loop (wave 6)

**Wave 7** *(blocked on all prior plans)*

- [x] 07-17-PLAN.md — Final gate, requirements, the validation map checked against the real targets, then `cargo clean` (wave 7)

**Research**: Standard patterns; skip research-phase (a research pass ran on 2026-10-22; its findings are D-17 to D-20)

### Phase 8: CLI, Sample Project, and Docs

**Goal**: A user takes the shipped sample project from `minos check` through `run`, `report`, and `adopt` with every experiment in a VM, sees only sanitized output and measured cost, receives a patch file only after acknowledging the report, and reads docs that do not overclaim
**Depends on**: Phase 5, Phase 7
**Parallelism**: None; this is the integration phase
**Requirements**: CLI-01, CLI-05, CLI-06, CLI-07, CLI-08, CLI-09, CLI-10, JUDGE-02, COST-02, DOCS-01, DOCS-02, DOCS-03, DOCS-04
**Cross-cutting**: GATE-06 (`cargo clean` after verification) and `rules/` adherence
**Success Criteria** (what must be TRUE):

  1. On the shipped `examples/` `optimize` project, whose judge computes metrics from harness measurements and candidate output files and never from candidate-reported numbers, `minos check`, `minos run --generations N`, `minos report`, and `minos adopt` complete end to end with every experiment in a VM (DOCS-01, DOCS-02)
  2. `minos init --kind <tune|optimize|explore>` scaffolds `minos.toml`, `program.md`, and `minos.Dockerfile`. `minos check` validates the manifest, builds, judges the baseline several times with the trial index on each `Judged`, and prints the noise floor beside `min_improvement` and the route table with reasons, with actionable errors when something is wrong (CLI-01, CLI-05, JUDGE-02)
  3. `minos run` prints one sanitized progress line per experiment (ID, tier, outcome, delta, elapsed) and resumes an interrupted run. `minos stop <run>` records `StopRequested` and the driver exits at the next safe point. `minos status`, `minos log`, and `minos show <exp>` render only through the sanitizing renderer. Cost appears only as one `approx. cost: $X.XX` line with token counts beside it, and no command prints an estimate or the word "exact" (CLI-06, CLI-07, CLI-10, COST-02)
  4. `minos report <exp>` prints the report and its hash. `minos adopt <exp> --reviewed <hash>` refuses unless the hash matches an acknowledged report for that exact experiment, then writes a canonical patch file that both `git apply` and `patch -p1` accept (golden test) and never applies it. `SECURITY.md` states the threat model, the trust base, and what preflight checks without claiming ironclad isolation, and the README states that the `claude` CLI provider only execs the user's own install under their own login and that shared or hosted use goes through an API key or Bedrock (CLI-08, CLI-09, DOCS-03, DOCS-04)
  5. `cargo clean` has been run after verification passes (GATE-06)

**Plans**: 6/15 plans executed

Plans:
**Wave 1**

- [x] 08-01-PLAN.md — Core and loop seams: `CostLine` with token counts, `run_cost`, the D-08 constants, `RunState::proposals`, the advisory `noise_check` rustdoc, public `recorded_manifest_at` and `surface_bytes` (wave 1)
- [x] 08-02-PLAN.md — `Ledger::runs` on both stores with contract cases; a lock probe never creates a listed run (wave 1)
- [x] 08-03-PLAN.md — `minos_source::read_files` and `Snapshot::surface`: the strict read-back of a snapshot or stored base tarball (wave 1)
- [x] 08-04-PLAN.md — The sample's Rust crates: the `logparse` candidate, the `logparse-judge` judge, the data and answer key, `program.md`, the improved-parser fixture, `exclude = ["examples"]` (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 08-05-PLAN.md — The sample's manifest, Dockerfile, ignore file, and README; the host gate test; CLAUDE.md for the sample and the stage environment (wave 2)
- [x] 08-06-PLAN.md — CLI foundation: `prepare` and `open_runs`, `krun_jobs`, prefix addressing, the shared renderer helpers, the command module files, the `env build` help fix (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 08-07-PLAN.md — `minos check`: facts, routes, the batch advisory, provider status, calibration reuse or a fresh run, the noise floor beside `min_improvement` (wave 3)
- [ ] 08-08-PLAN.md — `minos run`: start or `--run` resume to an absolute target, the progress observer and its golden format (wave 3)
- [ ] 08-09-PLAN.md — `minos status`, `log` (with `--json`), and `show` through the sanitizing renderer (wave 3)
- [ ] 08-10-PLAN.md — `minos report` and `adopt` with the hash gate and `create_new` patch, and the `git apply`/`patch -p1` golden (wave 3)
- [ ] 08-11-PLAN.md — `minos stop` (refused without a driver, fresh ordinal) and `minos init` with a fail-closed price table (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*

- [ ] 08-12-PLAN.md — The `minos` binary: the full clap builder tree, the concrete provider and backend, exit codes, binary-level tests (wave 4)
- [ ] 08-13-PLAN.md — The sample end to end on real VMs, the `RLIMIT_AS`, judge-compile, and boot-variance measurements, and the user's live provider rerun, recorded in 08-EVIDENCE.md (wave 4; checkpoint)

**Wave 5** *(blocked on 08-12 and 08-13)*

- [ ] 08-14-PLAN.md — `SECURITY.md`, `README.md`, the overclaim test, the `/scratch/candidate/out` correction, CLAUDE.md's Phase 8 sections (wave 5)

**Wave 6** *(blocked on all prior plans)*

- [ ] 08-15-PLAN.md — Final gate, requirements, the validation contract, then `cargo clean` (wave 6)

**Research**: A research pass ran on 2026-10-28 after the discuss session (08-RESEARCH.md, experiments E1 to E8)

## Progress

**Execution Order:**
Phases execute in numeric order, using the parallel windows above: 1, then 2 and 3 together, then 4, then 5 and 6 together (with the Phase 7 loop plans), then the rest of 7, then 8.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Workspace and Gates | 5/5 | Complete    | 2026-10-09 |
| 2. Platform Spikes | 11/11 | Complete    | 2026-10-10 |
| 3. Core Types and Pure Logic | 13/13 | Complete    | 2026-10-14 |
| 4. Store, Source Snapshot, and Sandbox Types | 9/9 | Complete    | 2026-10-15 |
| 5. Guest, krun Backend, and Environment Build | 22/22 | Complete    | 2026-10-19 |
| 6. Proposers | 10/10 | Complete    | 2026-10-20 |
| 7. Runner and Loop | 17/17 | Complete    | 2026-10-24 |
| 8. CLI, Sample Project, and Docs | 6/15 | In Progress|  |
