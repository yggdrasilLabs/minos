# Project Research Summary

**Project:** Minos (v1: local-only milestone on macOS arm64)
**Domain:** Secure autonomous experiment-loop lab (autoresearch family), Rust, libkrun microVMs
**Researched:** 2026-10-09
**Confidence:** MEDIUM overall. Crate versions and APIs are HIGH (crates.io and source read, compile probes passed). The libkrun runtime recipe, macOS confinement, and `claude -p` flag interplay are unverified until spiked. The ecosystem feature claims are LOW-to-MEDIUM (web sources).

## Executive Summary

Minos is a fixed, deterministic experiment loop (propose, run under a harness-enforced budget, score in a separate judge sandbox, keep or discard) wrapped in three invariants that no autoresearch descendant offers: hardware-virtualized isolation with no network, judge integrity, and a hash-gated review gate. Peers (autoresearch, OpenEvolve, Weco, AI Scientist) run candidate code in-process or on the host and rely on prompts or docs for safety. The documented incidents (evaluator edited by the agent, timers overwritten, self-relaunching agents) all came from that gap. The expert approach here is a functional core (`minos-core`, pure and sync) with thin effectful shells behind Cargo-enforced boundaries, so the later AWS durable orchestrator is a second shell over the same core.

Recommended v1: a synchronous (no tokio) Rust workspace. Each experiment runs in a libkrun microVM via `msb_krun =0.1.41`, a safe-Rust VMM builder that keeps `unsafe_code = "forbid"` intact. Every VM gets its own signed helper process, `minos-vmm`, because the libkrun start call exits the host process. Inputs go in as raw read-only disk images and outputs come back as a capped tar stream over vsock, parsed in memory into `Untrusted<T>`. The loop driver, file-backed ledger (JSONL, `File::lock`), and blob store live in the CLI process. Proposers are `sweep` (offline, seeded) and `single_shot`, which execs the user's own `claude -p` from an empty temp directory with an allowlisted environment, all tools off, and a strict empty MCP config. `--bare` is not usable with subscription login.

The dominant risks are all in the sandbox and propose phases, and every researcher flagged the same ones.

1. libkrun turns on TSI socket proxying by default when no network device exists, so "no NIC" is not "no network".
2. libkrun is not a security boundary on its own, and macOS confinement (dedicated user plus Seatbelt) is an unverified spike.
3. `claude -p` runs project hooks and `.mcp.json` unless the cwd and flags are locked down.
4. The ledger's wire format must be frozen before golden files, or every missing event costs a `V2`.

Mitigation: a spike phase before any backend code, with a fail-closed egress test as an exit criterion. Core work proceeds in parallel.

## Cross-Cutting Findings (all four research files agree)

These drive phase structure and exit criteria. The roadmapper should treat them as hard requirements.

1. **TSI is on by default.** libkrun enables TSI (`HIJACK_INET`) whenever no virtio-net device is added. Every launch must explicitly disable the implicit vsock and add a plain vsock with `tsi_features = 0`. With `msb_krun`, build without the `net` feature, leave `inet_hijack` at its default `false`, and ban the hijack methods with clippy `disallowed-methods`. **A permanent fail-closed egress test (guest `connect()` to host loopback and a public IP must both fail) is an exit criterion of the backend phase and runs in `minos host doctor`.**
2. **One helper process per VM.** The libkrun start call (`krun_start_enter` / `Vm::enter()`) never returns and `exit()`s the host process. Each VM is a child `minos-vmm`. Only that binary links the VMM and gets the `com.apple.security.hypervisor` entitlement, via ad-hoc `codesign` from an `xtask` (every `cargo build` drops it, so verify with `codesign -d --entitlements -`). The `minos` CLI stays unsigned and never links `msb_krun`. The supervisor owns the deadline and kills the process group. Spawn the helper with `env_clear()`, stdin `/dev/null`, and a capped console pipe, never the TTY.
3. **libkrun alone is not a boundary.** libkrun states that the guest and VMM share one security context. macOS has no seccomp or cgroups, so confinement must come from a dedicated unprivileged user and/or a Seatbelt (`sandbox-exec`) profile plus rlimits. Whether Seatbelt deny-default still permits `hv_vm_create` is **unverified** (spike K6). If it breaks, fall back to a dedicated `_minos` user and record the reduced hardening in provenance. Never fall back silently. Preflight must report which layers are active and refuse, or mark the run `WeakIsolation`, when none apply. Docs must not overclaim.
4. **`claude -p` lockdown.** Run from a fresh, empty, private temp dir outside any repo. Use `env_clear()` plus an allowlist (`HOME`, `PATH`, `USER`, `LOGNAME`, `TMPDIR`, `LANG`). Disable all tools (`--tools ""` plus `--disallowedTools "*"` as belt and braces), pass a strict empty MCP config, and send the prompt on stdin. `--bare` is **not** usable with subscription login because it never reads OAuth or the keychain. Evaluate `--safe-mode` and `--restricted` in the spike. Re-parse `structured_output` into closed `deny_unknown_fields` types and record tokens only, never `total_cost_usd`.
5. **Spikes before backend code; core in parallel.** Spike results settle the helper form, `msb_krun` boot, TSI off, vsock semantics, Seatbelt, root-image production, and `claude` flag behavior. `minos-core`, store, and gates have no dependency on them and proceed in parallel.
6. **Freeze the ledger event set before golden files.** Add `trial` on `Judged` (the noise floor needs it, and idempotent keys would otherwise collapse repeats). Add `Steered { note: BlobId }`, `StopRequested { reason }`, provider provenance (model ID, prompt hash, CLI version), and a rationale blob reference on `Proposed`. Document each event's `EventKey`. Otherwise each missing field costs a `V2`.
7. **Corrections to CLAUDE.md and PROJECT.md** (apply in the same change as the code, per "keep this file current"):
   - Homebrew tap is `libkrun/krun` (`brew tap libkrun/krun && brew trust libkrun/krun`), not `slp/krun`. The upstream repo is now `github.com/libkrun/libkrun`.
   - Money needs **pico-dollars (`u128`)**, not nano-dollar precision. Published AWS rates such as `$0.0000276944` have 10 fractional digits. Totals stay integer micro-dollars, and `prices.toml` rates must be TOML strings.
   - **v1 needs no tokio.** Use std threads, `std::process`, `UnixListener`, and `File::lock`. This overrides the async store traits proposed in ARCHITECTURE.md, which assumed tokio for the later durable SDK. Keep `Ledger`, `Blobs`, and `Backend` synchronous in v1 and revisit at the AWS milestone. The trait split by host (local supervisor vs Lambda runner) is flagged for then.
   - "No NIC means no network" is false for libkrun defaults (see finding 1).
   - A `minos-loop` crate is needed (core, store, propose only, never sandbox) so raw sandbox output is unreachable from decision code. Enforce it with a CI dependency check.

**Process rule from the user:** `cargo clean` runs at the end of every phase.

**User decision (2026-10-09):** reach libkrun through the safe wrapper crate (`msb_krun`), keeping `unsafe_code = "forbid"` everywhere. If the boot spike fails, stop and ask again; never add an `unsafe` exception quietly.

## Key Findings

### Recommended Stack (see STACK.md)

Exact `=` pins are deliberate: golden vectors and later durable replay need output-stable dependencies. Toolchain is rustc 1.97.1, edition 2024, with targets `aarch64-apple-darwin` and `aarch64-unknown-linux-musl`. The guest builds static with `rust-lld`, with no C toolchain (verified).

**Core technologies:**
- `msb_krun =0.1.41` (`default-features = false`, `features = ["blk"]`): the only maintained safe-Rust libkrun VMM API, so our crates keep `forbid(unsafe_code)`. Compiles with no `cc` or `-sys` in the tree. Not yet booted (spike K1).
- libkrunfw 5.6.2 (Homebrew `libkrun/krun`): guest kernel dylib (Linux 6.12.91). Pass the absolute path and record its SHA-256.
- `diffy 0.5.2` for parsing (`gitdiff` mode detects rename, copy, mode, binary) plus a **strict exact-position applier written in `minos-core`**. Never use `diffy::apply` (offset search) or fuzzy patchers.
- `tar 0.4.46` (no default features, hand-built headers, never `unpack`), `sha2 0.11.0`, `base16ct 1.0.0`, `serde 1.0.229`, `serde_json 1.0.151`, `toml 1.1.8`, `thiserror 2.0.21`, `clap 4.6.7`. `anyhow` only in `minos-cli` main.
- `rustix` and `vsock` in the guest. `backhand 0.25.5` (gzip only, to avoid C) for a pure-Rust SquashFS root image. A hand-rolled `newc` initramfs writer. `ignore` and `regex` for source packing and the secret scan. `rand_chacha 0.10` with own range mapping (no `gen_range`).
- Tests and supply chain: `proptest`, `insta` (`INSTA_UPDATE=no` in the gate), `trybuild`, `cargo-deny 0.20.2` with `required-git-spec = "rev"`, `unknown-git = "deny"`, and a build-script allowlist.
- Hand-rolled integer money (pico-dollar `u128`) and a hand-rolled allowlist terminal renderer in core (strip C0/C1 and ESC, render bidi and zero-width characters visibly).

### Expected Features (see FEATURES.md)

**Must have (table stakes, all v1):**
- The loop, with width, greedy policy, `min_improvement`, and outcomes as values.
- A harness-enforced time budget.
- **Resume** of an interrupted run. This is a gap in the design; the ledger makes it nearly free.
- Bounded runs: spend cap, model-call or token cap, stall stop, and a **circuit breaker** (K generations with zero valid candidates, or K infra failures in a row).
- `single_shot` with history, per-experiment rationale, and bounded failure excerpts in the prompt.
- Sweep tier, writable surface enforced by `Diff::parse`, and `status`, `log`, `show`, and live progress lines.
- Cascade (build or check fails, then `Rejected`), diff and lineage views, and a patch that `git apply` accepts.

**Should have (differentiators, v1):**
- Candidate and judge in separate no-network VMs, with roles as types and compile-fail tests.
- Harness-measured time (never self-reported) and a noise floor from repeated baseline runs in `minos check`.
- Judge-data registration outside the project tree. Either `judge push` into the local blob store or a defined external-path hash. Pick one in requirements.
- `minos host doctor`, hashed pure `build_report`, and hash-gated `adopt` (patch file only).
- Report flags subset: anomalous jump, held-out divergence, literals matching visible inputs, encoded blobs. `held_out` metric marking so the held-out flag has input and the proposer never sees those values.
- Model provenance, usage-only cost ledger with one `approx. cost` line, and tokens shown beside it (subscription users see $0.00 otherwise).

**Defer (v1.x):** `steer` command (event added in v1), `rejudge` command (data model in v1), `params add`, champion re-measurement, concurrent VMs per generation, `log --json`, `env shell`.
**Defer (v2+):** `planned` and `agentic` tiers, Pareto and `review` policies, `DesignJudge`, `inbox`, guest runtime-tracing flags, AWS, GPU.
**Do not build:** web dashboard, auto-apply, git in the loop, network for candidates, LLM-as-judge, in-loop repair, "never stop" runs, dollar estimates.

### Architecture Approach (see ARCHITECTURE.md)

Functional core, replaceable shell: routing, admission, selection, fold, and report are pure functions in `minos-core`; shells only move data. Follow record-before-act and key every effect by content (`LaunchKey`, `EventKey`, `SpendId`), so a Ctrl-C resumes to the same result. The runner/loop seam is `JobRequest -> JobResult`, defined in core, which is the same contract the future `ctx.invoke` will use. `Cleared` owns the plan and is minted only in `minos-sandbox`, so preflight cannot be skipped or reused. No `AnyBackend` in v1. `BlobId` is the plain SHA-256 of content (S3 checksum parity). Domain separation applies to composite IDs only.

**Major components:**
1. `minos-core` — pure types, validation, policy, routing, sweep, `generation::{plan, admit, conclude}`, fold, `build_report`, usage and money math, renderer
2. `minos-store` — sync `Ledger` and `Blobs` traits, memory and file impls (torn-line handling, `flock`, temp-then-rename blobs)
3. `minos-sandbox` — role typestate, `seal`, `preflight -> Cleared`, `Untrusted<T>`, capped tar reader
4. `minos-backend-krun` plus `minos-vmm` helper — disk builders, 0700 socket dir (short path, under 104 bytes), spawn, deadline, watchdog, preflight checks, entitlement check
5. `minos-guest` — static musl PID 1; job intake from `/dev/vdb`, strict apply, argv commands under non-root uid and rlimits, capped tar out; no per-run work at startup
6. `minos-propose` — deterministic prompt assembly, `ModelProvider` trait, `ClaudeCli`, `single_shot`, `Proposals::parse`
7. `minos-runner` and `minos-loop` — candidate-to-judge pipeline with result cache; generation driver with lock and resume
8. `minos-cli` — wiring, `init`, `env build`, `check`, `run`, `status`, `log`, `show`, `report`, `adopt`, `stop`, `host doctor`

### Critical Pitfalls (see PITFALLS.md; 34 total)

1. **Implicit TSI (P1)** — `krun_disable_implicit_vsock` then `add_vsock(0)` as the only code path; fail-closed egress test.
2. **libkrun not a boundary alone (P2) and process takeover (P3)** — one signed helper per VM, dedicated user and Seatbelt (spike), preflight reports active layers, host-side `SIGKILL` deadline plus a parent-death watchdog and pidfile sweep (macOS has no `PDEATHSIG`).
3. **Host env, TTY, and virtiofs leakage (P4, P5, P6)** — explicit empty `envp`, `env_clear`, no virtiofs, raw read-only disks only with the format hard-coded, scratch deleted unread.
4. **Hostile output and forgeable guest (P8, P9, P10)** — never `tar::unpack`, regular files only, caps by bytes read, scorecard only from the judge channel, host-measured timing, `Metric` rejects NaN or inf and normalizes `-0.0`, and the sample judge must not read candidate-reported objectives.
5. **`claude -p` hooks, env, and cwd (P12) and prompt injection (P13)** — empty cwd outside the repo, golden argv, hostile `.claude/settings.json` fixture test, model output treated as hostile.
6. **Diff parser differential (P15)** — canonical re-serialized diff hashed into `ExperimentId`; the guest applies only canonical bytes with the same crate and the same strict applier; `adopt` writes canonical bytes.
7. **Ordering, ledger integrity, spend caps (P17, P18, P19)** — canonical hashing (never `serde_json::Value`), CI check against `preserve_order`, append-only ledger with `flock` and a torn-final-line rule, pre-generation spend check, string-only prices.

Also gate in CI: no `allow(clippy::...)` (use `#[expect]` with reason), no `anyhow` in library crates, and `cargo deny` set to deny.

## Implications for Roadmap

Seven phases. Spikes and core run in parallel at the start. Each phase ends with `cargo clean`.

### Phase 1: Spikes and Workspace Gates
**Rationale:** The backend and propose designs depend on unverified facts (K1 to K8 plus the `claude` flags). Settling them first prevents building on guesses. Gates are cheap and should exist before any code. This plays the role of CLAUDE.md build-order step 2 and runs alongside Phase 2.
**Delivers:** Workspace, lints, `deny.toml`, crate-level `clippy.toml` files, pinned `rust-toolchain.toml`, CI script (including the loop-has-no-sandbox dependency check and the feature-unification check). Spike write-ups folded into CLAUDE.md: `msb_krun` boots libkrunfw 5.6.2 from an initramfs with a raw root disk and no virtiofs (K1, K2), egress denied (K3), vsock to a host Unix socket (K4), signing without `disable-library-validation` (K5), Seatbelt vs dedicated user (K6), exit codes (K7), concurrent VM limits and memory (K8), root-image production from a Dockerfile, and the `claude -p` contract (tools-off with `--json-schema` still yields `structured_output`, usage field names, `--safe-mode` and `--restricted` with subscription auth under `env_clear`, `--max-turns 1`).
**Avoids:** P1, P2, P21 (decides the FFI question), P22, and the CLAUDE.md corrections.
**Stop-and-ask point:** if `msb_krun` fails the boot spike, the remaining options are an `unsafe` binding crate or waiting for upstream. That needs explicit user approval, and `#![allow(unsafe_code)]` must never be added quietly.

### Phase 2: minos-core (parallel with Phase 1 spikes)
**Rationale:** Everything depends on the validated types, and core has no dependency on the VM spikes.
**Delivers:** IDs and domain-separated hashing with golden vectors, `RelPath` (conservative charset), `Manifest` (`Raw` plus `try_from`), canonical `Diff::parse` plus the strict applier, `Metric`, `Outcome`, fold, `select_greedy`, routing and `eligible`, the sweep sampler, `generation::{plan, admit, conclude}`, `JobRequest`/`JobResult`, pico-dollar money and `ApproxCost`, the allowlist renderer, and `build_report` with the v1 flag subset. **The Event set is frozen first** (finding 6), then the golden files. Property tests, golden hashes, and wire files.
**Addresses:** the loop semantics, noise-aware policy, anomalous-jump and held-out flags, rationale and provenance fields.
**Avoids:** P10, P11, P14, P15, P17, P19 (money types), P24, P25, P26.
**Standard patterns:** skip research-phase; the stack file has verified recipes.

### Phase 3: minos-store and minos-sandbox
**Rationale:** Both depend only on core and are prerequisites for the backend and runner. The sandbox types are where the isolation invariants become compile-time facts.
**Delivers:** Sync `Ledger` and `Blobs` with memory and file impls (`flock`, torn-line rule, `fsync` before irreversible actions, temp-then-rename blobs, unknown `v` refuses). `SandboxSpec<R>`, sealed `LaunchPlan` (no format field, no network field), `Cleared` owning the plan, `Backend` trait, `Untrusted<T>` with validating exits, capped tar reader and fuzz corpus, and trybuild compile-fail tests.
**Avoids:** P6, P8 (reader), P18.
**Standard patterns:** skip research-phase.

### Phase 4: Guest, krun Backend, and Environment Build
**Rationale:** This is the highest-risk phase, and it can only start after the Phase 1 spikes. The guest, backend, and image pipeline are tightly coupled through the disk and vsock protocol.
**Delivers:** `minos-guest` (strict apply, non-root candidate commands, rlimits, PID 1 reaping, capped output), `minos-backend-krun` plus `minos-vmm` (disks, 0700 short socket dir, spawn with `env_clear`, deadline, watchdog, pidfile sweep, preflight, signing `xtask`), `minos host doctor`, `minos env build` and `minos.lock` (build context is the tarball, no secrets), and the SquashFS and initramfs writers.
**Exit criteria (all must pass):** the egress-denial test fails closed. Guest `env` and `/proc/cmdline` show no host values. `kill -9` on `minos run` leaves no VM. Two sequential VMs both record outcomes. Output-reader fuzz passes. The helper's confinement layers are reported by `doctor`.
**Avoids:** P1 to P9, P22, P23, P29, P30 to P33.
**Needs research-phase:** yes (Seatbelt and `hv_vm_create`, first boot, root-image tooling on macOS).

### Phase 5: minos-propose
**Rationale:** Independent of the VM work once Phase 1 has fixed the `claude` contract. It can run in parallel with Phase 4. The sweep tier already works with no model, which allows a fully offline end-to-end earlier.
**Delivers:** Deterministic prompt assembly (stable cacheable prefix, then dynamic suffix; golden and shuffled-input tests), `ModelProvider` and `ClaudeCli` with golden argv and a hostile-settings fixture test, `auth status` preflight, `single_shot` returning `{rationale, diff}` per candidate, `Proposals::parse` into closed types, bounded history and `LogExcerpt` sections, and a held-out-never-in-prompt test. Token usage is recorded; `total_cost_usd` is ignored. Rate-limit or credit exhaustion is an infra failure that stops the run.
**Avoids:** P11, P12, P13, P17 (prompt), P19, P34.
**Needs research-phase:** yes (flag interplay unverified until the Phase 1 spike lands; re-check if that spike leaves gaps).

### Phase 6: minos-runner and minos-loop
**Rationale:** Needs the backend for real runs, but the loop is built and tested first against `MemoryStore`, a fake provider, and a fake `JobRunner`. That is possible before Phase 4 finishes.
**Delivers:** The candidate-to-judge pipeline with `LaunchKey` result cache and no shared output between experiments. The loop driver with `driver.lock`, record-before-act, and resume (`--resume` or idempotent `run`). Spend and call caps checked before each generation and each model call. Circuit breaker, stall escalation, `StopRequested`, and confirmation runs where noise demands. Judge-data registration.
**Avoids:** P7 (stale sweep), P16, P18, P19, P23.
**Standard patterns:** skip research-phase; the architecture file gives the flow.

### Phase 7: CLI, Sample Project, and Docs
**Rationale:** Integration and the definition of done: the full loop on a sample project with every experiment in a VM.
**Delivers:** `init`, `check` (baseline noise floor, actionable errors), `run`, `status`, `log`, `show`, `report`, `adopt`, `stop`, and the sanitizing renderer on every output path. The `examples/logparse` `optimize` project, whose judge measures from outside and never reads candidate-reported objectives. The secret-scan and denylist packer. `SECURITY.md` with the threat model, trust base, and honest wording. README directing shared or hosted use to an API key or Bedrock, and a statement of the Claude subscription terms. Final pass over CLAUDE.md and PROJECT.md corrections.
**Avoids:** P14, P16, P20, P28.
**Standard patterns:** skip research-phase.

### Phase Ordering Rationale

- Spikes precede the backend because the helper form, TSI behavior, and the Seatbelt question can invalidate the backend design. Core and gates have no such dependency, so they run in parallel and the schedule loses nothing.
- Core freezes the event set before golden files, since the wire format is an API from the first commit.
- Sandbox types precede the backend so the compiler enforces roles and preflight from the first VM launch.
- The loop crate depends on core, store, and propose only. This is the Cargo-level guarantee that nothing above the runner sees raw sandbox output.
- The sweep tier needs no model, so an offline end-to-end (sweep, greedy, report, adopt) can be demonstrated before `claude -p` integration, which de-risks ordering.

### Research Flags

Needs deeper research during planning (`/gsd-plan-phase --research-phase <N>`):
- **Phase 1:** the spikes are the research. Treat each K item and each `claude` flag question as a deliverable with a recorded result.
- **Phase 4:** the VMM boot, Seatbelt confinement, root-image production on macOS, and concurrency limits. Highest risk in the project.
- **Phase 5:** `claude -p` flag interplay and subscription behavior, if the Phase 1 spike leaves open questions.

Standard patterns (skip research-phase): Phases 2, 3, 6, and 7. Recipes are verified or follow directly from the design of record.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH (MEDIUM for libkrun runtime) | Versions from crates.io. Source read for `msb_krun`, libkrun, `diffy`, and `tar`. Four compile probes passed. No VM has been booted. |
| Features | MEDIUM | Categorization is judgment grounded in primary READMEs and the design of record. Ecosystem claims are web-sourced, so LOW individually. |
| Architecture | MEDIUM | Crate boundaries and data flow are HIGH (derived from the design). libkrun and CLI facts come from primary sources but remain spike items. The async-store proposal is superseded by the no-tokio decision. |
| Pitfalls | HIGH for libkrun and Claude Code, MEDIUM for macOS confinement and noise | Header and `lib.rs` read at the release tag. Docs read directly. Seatbelt with HVF and benchmark variance need spikes. |

**Overall confidence:** MEDIUM. It rises to HIGH on the backend once Phase 1 spikes pass.

### Gaps to Address

- **Seatbelt vs `hv_vm_create` (K6):** spike in Phase 1. Fallback is a dedicated `_minos` user with the reduced hardening recorded in provenance.
- **`msb_krun` boot (K1, K2) and initramfs behavior:** if it needs a NullFs virtiofs root, use `FsBuilder::custom` serving only `/init`. If `msb_krun` fails entirely, stop and ask the user.
- **`claude -p` flag combination:** `--tools ""` with `--json-schema`, `--safe-mode` and `--restricted` with subscription auth, Keychain access under `env_clear`, and `--max-turns 1`. Whether the CLI reports a notional cost field is unverified, and it is ignored regardless.
- **Subscription billing and terms:** sources disagree on `claude -p` billing. Document and tell users to check their plan and the extra-usage setting. Cap model calls or tokens separately from `spend_usd`.
- **Judge-data registration:** `judge push` into the local blob store vs hashing an external path. Decide in requirements.
- **Guest-recorded denied operations for report flags:** decide early, since the guest should stay identical across backends.
- **Benchmark noise on a laptop:** measure variance across about 20 VM boots before fixing the default repeat count. Pin speed objectives to concurrency 1.
- **Container engine for `env build` on macOS:** open, because it depends on the root-image spike.
- **Async trait split at the AWS milestone:** v1 stays sync, so decide the host-based `Backend` split then.
- **Report hash stability:** verify across two machines, and bump the `v` tag if it ever drifts.

## Sources

### Primary (HIGH confidence)
- crates.io API, 2026-10-09 — every version, MSRV, and feature
- `msb_krun 0.1.41`, libkrun `v1.19.6` (`libkrun.h`, `lib.rs`), `diffy 0.5.2`, and `tar 0.4.46` crate sources — API shapes, TSI heuristic, process takeover, disk security notes
- libkrun README and `libkrun/homebrew-krun` — security model, tap name, versions, ad-hoc `codesign`
- Claude Code docs (headless, CLI reference, permissions, Agent SDK) and local `claude --help` 2.1.295 — flags, `structured_output`, bare-mode auth, hooks under `-p`
- cargo-deny docs, `std::fs::File::lock` docs, AWS Lambda pricing page — gating config, 1.89 lock stability, pico-dollar evidence
- Local compile probes — `forbid(unsafe_code)` against `msb_krun`, static musl guest link, `sha2 0.11`, `diffy` rename detection, and `toml` `try_from` with `deny_unknown_fields`

### Secondary (MEDIUM confidence)
- Apple Developer Forums (HV_DENIED, hypervisor in an App Sandbox), `applevisor`, QEMU hvf entitlement patch — signing behavior
- RUSTSEC-2021-0080, RUSTSEC-2026-0067, GHSA-3pv8-6f4r-ffg2, CVE-2026-53600 — tar and PAX parser hazards
- METR reward-hacking write-up, Sakana AI Scientist coverage — evidence for judge-integrity design
- `claude -p` subscription billing articles (they disagree in detail)

### Tertiary (LOW confidence, needs validation)
- Autoresearch ecosystem survey, SkyPilot, Tensorlake, Tennis XGBoost case, OpenEvolve and ShinkaEvolve issues, Weco docs — feature landscape claims, each cited in FEATURES.md
- libkrun main-branch v2 Rust API — direction only, timing unknown
- Seatbelt deny-default with Hypervisor.framework — no source found, spike required
- `composefs` EROFS writer — alternative only

---
*Research completed: 2026-10-09*
*Ready for roadmap: yes*
