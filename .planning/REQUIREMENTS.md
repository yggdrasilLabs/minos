# Requirements: Minos

**Defined:** 2026-10-09
**Core Value:** Untrusted experiment code runs in a hardware-virtualized sandbox with no network, cannot touch the judge that scores it, and nothing it produces reaches the user's machine without an acknowledged report.

## v1 Requirements

Milestone v1 is local-only on macOS arm64. Every requirement below is testable on the dev machine with no AWS account.

### Workspace and Gates (GATE)

- [x] **GATE-01**: Workspace builds on rustc 1.97.1 / edition 2024 with `unsafe_code` and clippy `unwrap_used`, `expect_used`, `indexing_slicing` all `forbid` in `[workspace.lints]`, repeated as a `.cargo/config.toml` `[build] rustflags` backstop for a member that omits `[lints] workspace = true`
- [x] **GATE-02**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps` all pass as five plain cargo commands with no script in front of them, and `Cargo.lock` is committed
- [x] **GATE-03**: `deny.toml` requires `rev` on git dependencies, denies unknown registries and git sources, and allowlists build scripts
- [x] **GATE-04**: `cargo deny check` fails if `minos-loop` or `minos-core` gains a dependency on `minos-sandbox` or any backend crate
- [x] **GATE-05**: `minos-core` has a clippy `disallowed-methods` rule banning `SystemTime::now` and a `disallowed-types` rule banning `HashMap`; no `allow(clippy::...)` appears anywhere (only `#[expect(..., reason = ...)]`, and not even that for the `forbid` lints `unwrap_used`, `expect_used`, and `indexing_slicing`; for lints that are not `forbid`, the per-phase C-02 grep enforces this, not clippy)
- [ ] **GATE-06**: Every phase ends with `cargo clean` after verification passes

### Spikes (SPIKE)

Each spike produces a recorded result in CLAUDE.md before code depends on it.

- [x] **SPIKE-01**: The safe libkrun wrapper crate boots libkrunfw on this machine from a raw read-only SquashFS root disk whose `/init` is a static musl binary written in by the trusted packer, with no initramfs and no virtiofs, and the boot test asserts the exact `/proc/cmdline`. *Superseded wording (2026-10-09): "from an initramfs whose `/init` is a static musl binary, with a raw read-only root disk and no virtiofs". Stock libkrunfw 5.6.2 has `CONFIG_BLK_DEV_INITRD` unset, so the kernel cannot unpack an initramfs; the user chose the root-disk boot (02-CONTEXT.md D-18).*
- [x] **SPIKE-02**: A guest `connect()` to host loopback and to a public IP both fail when the launcher disables implicit vsock and adds a plain vsock
- [x] **SPIKE-03**: A vsock port maps to a host Unix socket and carries a byte-capped guest-to-host connection end to end. On the pinned `msb_krun_devices` 0.1.41 one connection is reliable up to 8192 bytes (02-SPIKE-EVIDENCE.md S03-WITHIN, DIAG-01), so guest output is a sequence of short connections, each one length-prefixed, sequence-numbered chunk of at most 4 KiB, reassembled by the host under per-chunk and total caps; Phase 5 builds and tests that protocol (reworded 2026-10-10 by the user's decision, S03-ANSWER: "work within the limits for now, if we need to reassess later fine")
- [x] **SPIKE-04**: An ad-hoc codesigned helper with the `com.apple.security.hypervisor` entitlement starts a VM, and an unsigned one is refused
- [x] **SPIKE-05**: Whether a Seatbelt deny-default profile still permits VM creation is answered with evidence, and the fallback (dedicated unprivileged user) is recorded
- [x] **SPIKE-06**: A root image can be produced from a user Dockerfile on macOS by a documented tool chain
- [x] **SPIKE-07**: `claude -p` with all tools disabled, `--json-schema`, `--no-session-persistence`, an empty cwd, and an allowlisted environment returns `structured_output` under subscription login; usage field names and `--max-turns 1` behavior are recorded
- [x] **SPIKE-08**: If SPIKE-01 fails, the run stops and asks the user; no `unsafe` exception is added

### Core Types (CORE)

- [x] **CORE-01**: `ExperimentId`, `BlobId`, `ManifestId`, `TreeId`, `PricesId`, `JudgeVersion` are distinct SHA-256 newtypes; composite IDs use a domain-separation prefix and length-prefixed fields; golden vectors guard every hash
- [x] **CORE-02**: `RelPath` rejects `..`, absolute paths, empty strings, and non-normal components, and is property-tested
- [x] **CORE-03**: `Manifest::try_from(RawManifest)` enforces: `writable` non-empty and disjoint from `readable` and judge paths, policy matches objective count, budgets within `limits.rs`, at least one eligible proposer tier; `RawManifest` uses `deny_unknown_fields`. Images are digests: `ImageRef` and `image::base_images` reject any Dockerfile `FROM` line that is not digest-pinned (amended 2026-10-14, user decision: the v1 manifest names a Dockerfile rather than an image, so the check runs on the Dockerfile text and `Manifest::try_from` stays pure)
- [x] **CORE-04**: `Diff::parse(text, &manifest)` enforces a size cap, requires every touched path in `writable`, rejects renames, mode changes, symlinks, and binary patches, and re-serializes to canonical bytes that are what `ExperimentId` hashes
- [x] **CORE-05**: A strict exact-position patch applier in core applies a canonical `Diff` to an in-memory tree and fails on any context mismatch; the guest and `adopt` use this same applier
- [x] **CORE-06**: `Metric` rejects NaN and infinities, normalizes `-0.0`, and implements `Ord` via `total_cmp`; a scorecard metric can be marked `held_out`
- [x] **CORE-07**: `Outcome` is `Scored | Rejected(Reason) | Crashed | TimedOut`; failure is a value, never an error
- [x] **CORE-08**: The `Event` set is frozen before golden files and includes `RunStarted`, `ManifestChanged`, `Proposed` (with parent, tier, rationale blob, model ID, prompt hash), `Judged` (with judge version, trial index, backend info), `Selected`, `Spent`, `Steered`, `StopRequested`, `ReviewRequested`, `Reviewed`, `ReportAcknowledged`; every event has a documented content-derived `EventKey`
- [x] **CORE-09**: Run state is only ever rebuilt by folding events; the fold is property-tested for determinism and golden-file tested for `Record::V1`
- [x] **CORE-10**: `select_greedy` applies `min_improvement`, breaks ties on `ExperimentId`, and is property-tested as permutation-invariant
- [x] **CORE-11**: Routing is a pure function of manifest and ledger that moves one tier up after `STALL_STEP` generations without improvement; `ProposerSpec::eligible` reports a reason for every ineligible tier
- [x] **CORE-12**: The sweep sampler is seeded, uses its own range mapping, and yields identical samples for identical seeds across runs
- [x] **CORE-13**: `JobRequest` and `JobResult` are defined in core as the loop-to-runner contract
- [x] **CORE-14**: Money is pico-dollar `u128` rates parsed from strings and integer micro-dollar totals; no float touches a dollar value
- [x] **CORE-15**: A single allowlist renderer strips C0, C1, and ESC sequences and renders bidi and zero-width characters visibly; every sandbox- or model-originated string passes through it before display
- [x] **CORE-16**: `build_report` is a pure function of ledger and blobs, produces a stable hash, and includes summary vs noise floor, cumulative diff, lineage, provenance (isolation level, judge version, image digests, model ID, prompt hash), and the v1 flags: anomalous jump, held-out divergence, literals matching visible inputs, new dependencies, encoded blobs
- [x] **CORE-17**: A circuit-breaker rule in core returns a stop decision after K generations with no valid candidate or K consecutive infrastructure failures

### Store (STORE)

- [x] **STORE-01**: `Ledger` and `Blobs` are synchronous traits with in-memory implementations used by all tests above the store
- [x] **STORE-02**: The file ledger is append-only JSONL under an exclusive file lock, tolerates a torn final line, refuses unknown `v` tags, and never rewrites a record
- [x] **STORE-03**: Appending an event with an already-recorded `EventKey` is a no-op, so steps are idempotent on resume
- [x] **STORE-04**: The file blob store is content-addressed by SHA-256, writes temp-then-rename, and verifies the hash on read

### Sandbox Types (SBX)

- [x] **SBX-01**: `SandboxSpec<R: Role>` exists for `Candidate` and `Judge`; only `SandboxSpec<Judge>` has a method to mount judge data; no role has a method to attach a network
- [x] **SBX-02**: `trybuild` compile-fail tests prove a `Candidate` spec cannot receive judge data and that `launch` cannot be called without `Cleared`
- [x] **SBX-03**: `seal()` is the only constructor of `LaunchPlan`; `preflight(host, plan) -> Result<Cleared, Vec<Refusal>>` is the only constructor of `Cleared`, and `Cleared` owns the plan
- [x] **SBX-04**: `Untrusted<T>` has no `Deref`, `Display`, or `Debug`; its only exits are `into_scorecard`, `into_log_excerpt`, and `into_diff_texts`, each with caps
- [x] **SBX-05**: The output tar reader accepts regular files only, enforces caps on entry count, entry size, and total bytes, rejects path traversal, and never unpacks to disk; it has a fuzz corpus
- [x] **SBX-06**: Launches are idempotent: the launch key derives from `ExperimentId` plus role plus trial, and every launch carries a hard maximum duration

### Guest (GUEST)

- [x] **GUEST-01**: `minos-guest` is a static `aarch64-unknown-linux-musl` binary that runs as PID 1, reaps children, and creates nothing per-run at startup
- [x] **GUEST-02**: The guest reads the job (base tarball, canonical diff, manifest subset) from a read-only input disk, applies the diff with the core applier, and fails the job as `Rejected` on any mismatch
- [x] **GUEST-03**: Manifest commands run as argv arrays under a non-root uid with rlimits on CPU, memory, processes, and file size, writing only to scratch space. *Superseded wording (2026-10-18): "writing only to the scratch disk". Scratch moved into guest memory by 05-CONTEXT.md D-05.*
- [x] **GUEST-04**: Guest output is a tar stream over vsock capped by bytes; the judge scorecard is only accepted from the judge-role channel
- [x] **GUEST-05**: The guest environment contains no host variables, verified by a test that reads `env` and `/proc/cmdline` inside the VM
  *Allowance, a recorded exception (user decision at the 05-19 checkpoint, 2026-10-19): the guest's /proc/cmdline carries one host-derived token, `KRUN_BOOT_START_NS=<digits>`, which msb_krun 0.1.41 always appends; exit_env allows exactly that key with an all-digit value and fails on any other host value. Removing it is a CLAUDE.md open question.*

### Backend (KRUN)

- [x] **KRUN-01**: Each VM runs in its own `minos-vmm` helper process, spawned with a cleared environment, stdin from `/dev/null`, and a capped console pipe; only this binary links the VMM crate and carries the hypervisor entitlement
- [x] **KRUN-02**: Every launch disables implicit vsock and adds a plain vsock with no socket proxying; the hijack and host-directory-sharing APIs are banned by clippy `disallowed-methods`
- [x] **KRUN-03**: A permanent integration test proves guest egress to loopback and to a public IP fails closed
- [x] **KRUN-04**: Inputs are a shared read-only root disk and a per-job read-only input disk, both raw images; the candidate's scratch is size-capped space in guest memory; no host directory is ever shared; scratch is deleted unread. *Superseded wording (2026-10-18): "a size-capped scratch disk, all raw images". Scratch moved into guest memory by 05-CONTEXT.md D-05.*
- [x] **KRUN-05**: The supervisor enforces the hard maximum duration by killing the helper's process group; a parent-death watchdog and pidfile sweep ensure `kill -9` on `minos run` leaves no VM running
- [x] **KRUN-06**: Preflight checks the entitlement, kernel presence and hash, Hypervisor.framework support, socket path length, and active confinement layers, and refuses with a reason for each failure
- [x] **KRUN-07**: The run records which confinement layers were active (dedicated user, Seatbelt, rlimits) in `BackendInfo`, and never falls back silently
- [x] **KRUN-08**: After every build, the helper is signed with the entitlement by the one `codesign` argv documented in CLAUDE.md Commands (no `xtask`, script, `build.rs`, or cargo alias), and the signature is verified with `codesign -d --entitlements -`; `minos host setup` may run the same argv from Rust. *Superseded wording (2026-10-09): "An `xtask` signs the helper with the entitlement after build". Phase 1 D-01 and D-07 rule out an `xtask` (02-CONTEXT.md D-11).*
- [x] **KRUN-09**: `minos env build` turns the project Dockerfile into a raw read-only SquashFS root image with `/init` = `minos-guest` written in by the trusted packer, records digests in `minos.lock`, and the build context contains no judge data and no secrets. *Superseded wording (2026-10-09): "a root image and an initramfs". Stock libkrunfw 5.6.2 cannot unpack an initramfs (02-CONTEXT.md D-18).*
- [x] **KRUN-10**: Two sequential experiments on the same host both record outcomes and share no build output

### Propose (PROP)

- [x] **PROP-01**: A `ModelProvider` abstraction (trait or closed enum) with request and response types for prompt parts, a structured JSON schema, model ID, and token usage; adding a Bedrock or API-key provider requires no change to callers
- [x] **PROP-02**: The `ClaudeCli` provider execs the user's `claude` binary from a fresh empty temp directory outside any repo, with an allowlisted environment (`HOME`, `PATH`, `USER`, `LOGNAME`, `TMPDIR`, `LANG`), all built-in tools disabled, a strict empty MCP config, session persistence off, the prompt on stdin, and `--json-schema`; the argv is guarded by a golden test
- [x] **PROP-03**: A fixture test with a hostile `.claude/settings.json`, hooks, and `.mcp.json` in the project directory proves none of them run during a proposal
- [x] **PROP-04**: The provider never reads, stores, or forwards Claude credentials; a preflight confirms the active auth method is the subscription login and refuses if an API key variable would be used unintentionally
- [x] **PROP-05**: Prompt assembly is deterministic with a stable cacheable prefix (`program.md`, surface files) followed by a dynamic suffix; golden tests and shuffled-input tests guard it
- [x] **PROP-06**: The dynamic suffix includes a bounded history table (rationale, outcome, delta vs incumbent) and bounded, sanitized failure excerpts from recent experiments, labelled as untrusted text; held-out metric values never appear in any prompt, proven by a test
- [x] **PROP-07**: `single_shot` returns several `{rationale, files}` candidates, parsed into closed `deny_unknown_fields` types, from which Minos derives the canonical diff; invalid candidates are dropped, not repaired. *Superseded wording (2026-10-19): "returns several `{rationale, diff}` candidates parsed into closed `deny_unknown_fields` types; invalid diffs are dropped, not repaired". The model returns the full contents of each writable file it changes and Minos derives the canonical diff with `diff_trees`, so the model never writes hunk headers (06-CONTEXT.md D-10, D-11).*
- [x] **PROP-08**: Token usage per call is recorded as a `Spent` event; any client-side dollar estimate from the CLI is ignored
- [x] **PROP-09**: A rate-limit or usage-cap response from the CLI is an infrastructure failure that stops the run with a `StopRequested` event, never an `Outcome`
- [x] **PROP-10**: The sweep tier produces proposals from declared params with no model call

### Runner and Loop (LOOP)

- [ ] **LOOP-01**: `minos-runner` runs the candidate VM then the judge VM for a `JobRequest`, converts `Untrusted` output to an `Outcome` via the validating exits, and is the only crate where that conversion happens
- [ ] **LOOP-02**: The judge VM mounts judge data read-only; the candidate VM never does; judge and candidate never share a process or a disk
- [ ] **LOOP-03**: Time and resource usage are measured by the host, never read from candidate output
- [ ] **LOOP-04**: `minos-loop` drives generations as: route tier, propose, validate diffs, run jobs, fold `Judged`, `select_greedy`, append `Selected`; it depends only on core, store, and propose
- [ ] **LOOP-05**: The loop records before acting and keys every effect by content ID, so `minos run` on an interrupted run resumes from the ledger and produces the same result as an uninterrupted run (tested with fakes)
- [ ] **LOOP-06**: A run holds an exclusive lock so two drivers cannot act on the same run
- [ ] **LOOP-07**: Spend cap and model-call or token cap are checked before each generation and each model call; exceeding either stops the run with a recorded reason
- [ ] **LOOP-08**: The circuit breaker stops the run after K generations with no valid candidate or K consecutive infrastructure failures and records `StopRequested` with the reason
- [ ] **LOOP-09**: The loop is fully testable against the in-memory store, a fake provider, and a fake job runner with no VM

### Judge (JUDGE)

- [ ] **JUDGE-01**: `minos judge push <dir>` uploads judge data into the local blob store as a new judge version and records its hash; judge data is never in the source tarball or the image build context
- [ ] **JUDGE-02**: `minos check` judges the baseline several times with the trial index on `Judged` and reports a noise floor the user can compare against `min_improvement`
- [ ] **JUDGE-03**: Gates run before objectives and short-circuit: a failed gate yields `Rejected` without scoring objectives
- [ ] **JUDGE-04**: Every `Judged` event records backend and architecture, and a run with a speed objective is pinned to one backend class

### CLI (CLI)

- [ ] **CLI-01**: `minos init --kind <tune|optimize|explore>` scaffolds `minos.toml`, `program.md`, and `minos.Dockerfile`
- [x] **CLI-02**: `minos env build` writes `minos.lock`
- [x] **CLI-03**: `minos host doctor` runs the launch preflight and prints every refusal with an explanation and a fix
- [ ] **CLI-04**: `minos judge push <dir>` registers judge data
- [ ] **CLI-05**: `minos check` validates the manifest, builds, judges the baseline repeatedly, prints the noise floor and the route table with reasons, and gives actionable errors
- [ ] **CLI-06**: `minos run --generations N` starts or resumes a run and prints one sanitized progress line per experiment (ID, tier, outcome, delta, elapsed)
- [ ] **CLI-07**: `minos status`, `minos log`, and `minos show <exp>` display progress, the ledger, and one experiment's diff, rationale, outcome, and lineage, all through the sanitizing renderer
- [ ] **CLI-08**: `minos report <exp>` renders the hashed report and prints its hash
- [ ] **CLI-09**: `minos adopt <exp> --reviewed <hash>` refuses unless the hash matches an acknowledged report for that exact experiment, then writes a canonical patch file that both `git apply` and `patch -p1` accept (golden test), and never applies it
- [ ] **CLI-10**: `minos stop <run>` records `StopRequested` and the driver exits at the next safe point
- [x] **CLI-11**: `minos run` packs the working tree into a deterministic tarball (sorted paths, zeroed metadata, regular files only) respecting `.gitignore` and `.minosignore`, applies a hard denylist, runs a secret scan that blocks on a hit, and requires a clean commit unless `--allow-dirty`

### Cost (COST)

- [x] **COST-01**: The ledger stores usage only (vCPU-seconds, GB-seconds, tokens in and out); dollars are derived from `~/.config/minos/prices.toml` with rates as strings
- [ ] **COST-02**: Exactly one type formats cost, always as `approx. cost: $X.XX` with token counts beside it; no command prints an estimate or the word "exact"

### Sample Project and Docs (DOCS)

- [ ] **DOCS-01**: An `examples/` `optimize` project ships in the repo whose judge measures from the harness and computes metrics from candidate output files, never from candidate-reported numbers
- [ ] **DOCS-02**: `minos check`, `minos run --generations N`, `minos report`, and `minos adopt` complete end to end on the sample project with every experiment in a VM
- [ ] **DOCS-03**: `SECURITY.md` states the threat model, the trust base (libkrun plus macOS confinement layers), what preflight checks, and does not claim ironclad isolation
- [ ] **DOCS-04**: The README states that the `claude` CLI provider only execs the user's own install under their own login, never handles credentials, and that shared or hosted use must go through an API key or Bedrock
- [x] **DOCS-05**: CLAUDE.md is amended for v1: local-first build order, `ModelProvider` decision, no tokio in v1, pico-dollar money, `libkrun/krun` tap, the libkrun networking default, the `minos-loop` crate, and each spike result

## v2 Requirements

Deferred. Tracked but not in the v1 roadmap.

### v1.x (after v1 ships)

- **NEXT-01**: `minos steer <run> "<note>"` command (the `Steered` event ships in v1)
- **NEXT-02**: `minos rejudge <run> --judge <v>` command (the data model ships in v1)
- **NEXT-03**: `minos params add <names...>`
- **NEXT-04**: Concurrent candidate VMs within a generation
- **NEXT-05**: Champion re-measurement or confirmation runs before `Selected`
- **NEXT-06**: `minos log --json`
- **NEXT-07**: `minos env shell`
- **NEXT-08**: Guest-recorded denied operations as a report flag

### Later milestones

- **AWS-01**: Durable Lambda orchestrator, Lambda MicroVM backend, DynamoDB ledger, S3 blobs, Bedrock `ModelProvider`
- **AWS-02**: API-key `ModelProvider`
- **TIER-01**: `planned` and `agentic` proposer tiers with the MCP tool bridge into an agent VM
- **POL-01**: Pareto and `review` policies, `DesignJudge` phase, `inbox` and `answer`
- **GPU-01**: GPU daemon, Cloud Hypervisor with VFIO, EC2 instances
- **LNX-01**: libkrun on Linux and the labelled `WeakIsolation` dev mode
- **RPT-01**: Runtime-behavior flags (spawned processes, sockets, environment reads) and a static sanitized HTML report export

## Out of Scope

| Feature | Reason |
|---------|--------|
| Off-the-shelf coding agent driving the loop on the host | Puts untrusted code, a credential, and the home directory in one trust domain |
| Auto-applying the winner | Skips the review gate; sandbox output would land on the user's machine |
| Git in the loop | Git on candidate-controlled trees brings hooks and config into the trusted path |
| Network access for candidates | Exfiltration and non-reproducibility; dependencies are baked at image build |
| Web dashboard | A local server rendering hostile strings; CLI commands cover v1 |
| MAP-Elites, islands, model ensembles | Suited to thousands of cheap samples; Minos runs are few and VM-bound |
| In-loop repair of broken proposals | Turns single_shot into a hidden agent loop; failures feed the next generation instead |
| LLM-as-judge | The judge would read hostile candidate output; mechanical judges only |
| Web search for the proposer | Untrusted web text in the prompt and network in the trusted process |
| Dollar estimates before a run | Forbidden by design; only measured cost is shown |
| Unbounded runs | Generation count, caps, and circuit breakers are mandatory |
| Claims of ironclad isolation | The target is "escape needs a hypervisor zero-day", stated honestly |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| GATE-01 | Phase 1 | Complete |
| GATE-02 | Phase 1 | Complete |
| GATE-03 | Phase 1 | Complete |
| GATE-04 | Phase 1 | Complete |
| GATE-05 | Phase 1 | Complete |
| GATE-06 | All phases (cross-cutting) | Pending |
| SPIKE-01 | Phase 2 | Complete |
| SPIKE-02 | Phase 2 | Complete |
| SPIKE-03 | Phase 2 | Complete |
| SPIKE-04 | Phase 2 | Complete |
| SPIKE-05 | Phase 2 | Complete |
| SPIKE-06 | Phase 2 | Complete |
| SPIKE-07 | Phase 2 | Complete |
| SPIKE-08 | Phase 2 | Complete |
| CORE-01 | Phase 3 | Complete |
| CORE-02 | Phase 3 | Complete |
| CORE-03 | Phase 3 | Complete |
| CORE-04 | Phase 3 | Complete |
| CORE-05 | Phase 3 | Complete |
| CORE-06 | Phase 3 | Complete |
| CORE-07 | Phase 3 | Complete |
| CORE-08 | Phase 3 | Complete |
| CORE-09 | Phase 3 | Complete |
| CORE-10 | Phase 3 | Complete |
| CORE-11 | Phase 3 | Complete |
| CORE-12 | Phase 3 | Complete |
| CORE-13 | Phase 3 | Complete |
| CORE-14 | Phase 3 | Complete |
| CORE-15 | Phase 3 | Complete |
| CORE-16 | Phase 3 | Complete |
| CORE-17 | Phase 3 | Complete |
| STORE-01 | Phase 4 | Complete |
| STORE-02 | Phase 4 | Complete |
| STORE-03 | Phase 4 | Complete |
| STORE-04 | Phase 4 | Complete |
| SBX-01 | Phase 4 | Complete |
| SBX-02 | Phase 4 | Complete |
| SBX-03 | Phase 4 | Complete |
| SBX-04 | Phase 4 | Complete |
| SBX-05 | Phase 4 | Complete |
| SBX-06 | Phase 4 | Complete |
| GUEST-01 | Phase 5 | Complete |
| GUEST-02 | Phase 5 | Complete |
| GUEST-03 | Phase 5 | Complete |
| GUEST-04 | Phase 5 | Complete |
| GUEST-05 | Phase 5 | Complete |
| KRUN-01 | Phase 5 | Complete |
| KRUN-02 | Phase 5 | Complete |
| KRUN-03 | Phase 5 | Complete |
| KRUN-04 | Phase 5 | Complete |
| KRUN-05 | Phase 5 | Complete |
| KRUN-06 | Phase 5 | Complete |
| KRUN-07 | Phase 5 | Complete |
| KRUN-08 | Phase 5 | Complete |
| KRUN-09 | Phase 5 | Complete |
| KRUN-10 | Phase 5 | Complete |
| PROP-01 | Phase 6 | Complete |
| PROP-02 | Phase 6 | Complete |
| PROP-03 | Phase 6 | Complete |
| PROP-04 | Phase 6 | Complete |
| PROP-05 | Phase 6 | Complete |
| PROP-06 | Phase 6 | Complete |
| PROP-07 | Phase 6 | Complete |
| PROP-08 | Phase 6 | Complete |
| PROP-09 | Phase 6 | Complete |
| PROP-10 | Phase 6 | Complete |
| LOOP-01 | Phase 7 | Pending |
| LOOP-02 | Phase 7 | Pending |
| LOOP-03 | Phase 7 | Pending |
| LOOP-04 | Phase 7 | Pending |
| LOOP-05 | Phase 7 | Pending |
| LOOP-06 | Phase 7 | Pending |
| LOOP-07 | Phase 7 | Pending |
| LOOP-08 | Phase 7 | Pending |
| LOOP-09 | Phase 7 | Pending |
| JUDGE-01 | Phase 7 | Pending |
| JUDGE-02 | Phase 8 | Pending |
| JUDGE-03 | Phase 7 | Pending |
| JUDGE-04 | Phase 7 | Pending |
| CLI-01 | Phase 8 | Pending |
| CLI-02 | Phase 5 | Complete |
| CLI-03 | Phase 5 | Complete |
| CLI-04 | Phase 7 | Pending |
| CLI-05 | Phase 8 | Pending |
| CLI-06 | Phase 8 | Pending |
| CLI-07 | Phase 8 | Pending |
| CLI-08 | Phase 8 | Pending |
| CLI-09 | Phase 8 | Pending |
| CLI-10 | Phase 8 | Pending |
| CLI-11 | Phase 4 | Complete |
| COST-01 | Phase 3 | Complete |
| COST-02 | Phase 8 | Pending |
| DOCS-01 | Phase 8 | Pending |
| DOCS-02 | Phase 8 | Pending |
| DOCS-03 | Phase 8 | Pending |
| DOCS-04 | Phase 8 | Pending |
| DOCS-05 | Phase 2 | Complete |

**Coverage:**

- v1 requirements: 97 total
- Mapped to phases: 97 (96 to exactly one phase; GATE-06 is cross-cutting and applies to every phase)
- Unmapped: 0 ✓

---
*Requirements defined: 2026-10-09*
*Last updated: 2026-10-19 after the Phase 5 final gate (GUEST-01 to GUEST-05, KRUN-01 to KRUN-10, CLI-02, CLI-03 complete; GUEST-05 with its one recorded exception)*
*Earlier update: 2026-10-19, PROP-07 reworded to `{rationale, files}` candidates (06-CONTEXT.md D-11)*
*Earlier update: 2026-10-09 after Phase 1 gap closure (GATE-01, GATE-02, GATE-05 reworded: three clippy lints at forbid, a rustflags backstop, and `cargo doc --workspace --no-deps` as the fifth gate command)*
