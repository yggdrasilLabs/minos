# Minos

Minos is a secure, general-purpose lab for autonomous experiment loops, written in Rust. It generalizes the pattern in [karpathy/autoresearch](https://github.com/karpathy/autoresearch): propose a change, run it under a fixed budget, score it, keep or discard, repeat. A project can be ML training, a performance problem, or an open-ended design question.

Three things set Minos apart from the existing autoresearch descendants, and they are the reason the project exists. Protect them over convenience:

1. **Isolation.** Untrusted code runs in hardware-virtualized sandboxes with no network device.
2. **Judge integrity.** The code being scored cannot read, edit, or influence the thing that scores it.
3. **A review gate.** Nothing the lab produces reaches the user's machine without a report they have acknowledged.

The repository is new. This file is the design of record until real docs exist. When they do, move the design sections into `docs/` and keep this file to rules, layout, and commands.

## How to work in this repo

- **The sketches here show intent, not compiled code.** Signatures will need adjusting. The invariants will not.
- **Never weaken a security invariant to make something compile or pass.** If an invariant blocks you, stop and ask. Do not add an escape hatch, a `pub` field, a `Deref`, or a "temporary" bypass.
- **Check external APIs against their docs before writing code against them.** Lambda MicroVMs (launched June 2026) and the Rust durable execution SDK (experimental preview) are newer than most training data. Do not guess method names. Links are in [Verified facts, AWS](#verified-facts-aws-as-of-2026-10-08).
- **The [Open questions](#open-questions) are unverified.** Do not build on one as if it were settled. Resolve it with a small experiment first, then update this file.
- **Follow the [Build order](#build-order).** Each step answers questions the next one depends on.
- **Keep this file current.** When a decision changes, edit the relevant section in the same change.

## Threat model

Both of these are in scope:

- A sloppy agent that wanders outside its lane.
- Adversarial code, including experiments from other people run on the user's hardware.

The target: escaping a sandbox requires a zero-day in KVM, Firecracker, or the hypervisor. It must never require only a mistake in our code or configuration. "Ironclad" is not achievable, so do not claim it in docs or output.

Everything that leaves a sandbox is hostile input: logs, results, artifacts, file names, diffs, model output, and tool results.

## Architecture

There is no always-on service. In milestone v1 the loop runs inside the `minos` CLI process on the user's machine, against a file-backed ledger and blob store, with libkrun sandboxes on macOS arm64. The AWS deployment described in the table below (durable Lambda orchestrator, Lambda MicroVMs, DynamoDB, S3, Bedrock) is a later milestone that reuses `minos-core` unchanged; when it exists, the CLI talks to AWS with the user's own credentials.

| Piece | What it is | Holds |
|---|---|---|
| Orchestrator | Durable Lambda functions: `run` (parent) and `generation` (child) | Ledger access, permission to invoke other functions |
| CPU runner | Durable Lambda function, called with `ctx.invoke` | Permission to launch microVMs, read blobs and judge data |
| Agent session | Durable Lambda function, one execution per session | Permission to invoke the named Bedrock models, launch microVMs |
| Sandboxes | Lambda MicroVMs (cloud CPU), libkrun (local CPU), Cloud Hypervisor with VFIO (local GPU) | Nothing: no IAM role, no network device, no secrets |
| GPU runner | Daemon on a GPU machine; long-polls a queue and completes callbacks; outbound connections only | Its queue, a few S3 prefixes, callback completion |
| State | Event-sourced ledger in DynamoDB; content-addressed blobs in S3 | |

On AWS, the GPU sandbox is a fresh EC2 instance per experiment with no IAM role. Cloud Hypervisor is for the user's own hardware.

### Sandbox roles

| Role | Runs | Sees |
|---|---|---|
| Agent | Tool calls from an agentic proposer | Only the `[surface]` files |
| Candidate | The built experiment | Full source tree with the diff applied, plus data blobs, read-only |
| Judge | The project's judge command | Candidate artifacts plus the judge's private data |

No role gets a network device. The trusted side moves all data: the runner pushes inputs in and pulls outputs out over the authenticated endpoint (Lambda) or vsock (local).

### The model stays outside the sandbox

The agent loop runs in trusted code, which calls Bedrock using the function's IAM role. Minos never reads, stores, or forwards a model credential. Models are reached through a `ModelProvider` abstraction in `minos-propose`. In v1 the personal default execs the user's own unmodified `claude` binary in print mode (02-SPIKE-EVIDENCE.md SPIKE-07 contract): the canonical binary under `~/.local/share/claude/versions/`, never whatever `claude` is first on `PATH` (it may be a wrapper that injects settings), run from a fresh empty directory outside any repository, with the environment cleared to `HOME`, `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `USER`, `LOGNAME`, `TMPDIR`, and `LANG`, the prompt on stdin, and `-p --model <model> --tools "" --output-format json --json-schema '<schema>' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1` (the spike ran `--model haiku`). Its `structured_output` is model output and is parsed strictly; only token counts are recorded, never the CLI's dollar estimate. Anthropic's terms allow this only for a user running their own install under their own subscription. Shared or hosted use goes through an API-key or Bedrock provider, later implementations of the same abstraction; on AWS the agent loop calls Bedrock with the function's IAM role. Only tool calls enter the agent VM, where `minos-guest` executes them. Model output is deserialized into a closed enum, so an out-of-bounds call fails validation and is returned to the model as an error:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ToolCall {
    ReadFile { path: RelPath, lines: Option<LineRange> },
    WriteFile { path: RelPath, content: String }, // writable paths only
    ListDir { path: RelPath },
    Search { pattern: String, within: Option<RelPath> },
    Run { command: CommandName }, // a command named in the manifest
    Submit { rationale: String }, // ends the session
}
```

On `Submit`, the guest diffs its working tree against the base, and that diff goes through `Diff::parse` like any other. An agentic session produces exactly what a single model call does: a validated diff. The agent never runs a real experiment and never sees the judge.

Running an off-the-shelf coding agent inside the VM is out of scope. It needs network egress and a credential within reach of untrusted text.

## Workspace

```
crates/
  minos-core/            types, validation, policy, routing, report; sync, no I/O
  minos-store/           Ledger and Blobs traits; file-backed and in-memory impls
  minos-sandbox/         roles, specs, sealed plans, Untrusted<T>, Backend trait
  minos-backend-krun/    LaunchPlan -> libkrun VM, driven through the minos-vmm process
  minos-vmm/             per-VM launcher; the only binary that links the VMM crate
  minos-guest/           static binary inside every VM
  minos-propose/         prompt assembly, model client, the four proposer tiers
  minos-runner/          local runner pipeline, generic over Backend
  minos-loop/            local generation driver: propose, run, judge, select
  minos-cli/             installs the `minos` binary; the only place concrete types meet
```

Later milestones, not in the workspace yet: `minos-backend-lambda` (LaunchPlan -> Lambda MicroVM), `minos-backend-chv` (Cloud Hypervisor VM with VFIO), `minos-orchestrator` (the durable functions: run, generation), `minos-store-aws` (DynamoDB and S3), and the GPU daemon.

Dependency direction is strict, and `deny.toml` enforces it with `[bans]` wrapper entries, so a crate that links outside its lane is refused by `cargo deny check`:

- `core` depends on no internal crate; `store`, `sandbox`, `propose`, and `guest` depend on `core`.
- `backend-krun` depends on `sandbox` (and `core`). It is the only crate that knows about `minos-vmm`.
- `runner` depends on `core`, `store`, and `sandbox`, and is generic over `Backend`. It never names a backend crate.
- `loop` depends on `core`, `store`, and `propose`. It never links `sandbox` or a backend, so it never sees raw sandbox output. The orchestrator, when it arrives, follows the same rule.
- `vmm` depends on no internal crate.
- `cli` composes everything and is the only crate that picks a concrete backend (`Runner<KrunBackend, FileStore>`).

Backends are separate crates, not feature flags, so a Lambda binary never links hypervisor code.

The bare `minos` crate name is taken on crates.io. Publish as `minos-*`; the binary is `minos`.

## Code rules

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "forbid"
expect_used = "forbid"
indexing_slicing = "forbid"
```

The three clippy lints are `forbid`, not `deny`, so neither `#[allow(...)]` nor `#[expect(...)]` can silence them anywhere: write the code so they never fire.

- **Adhere to rules/** ALL CODE MUST ADHERE TO RULES. there are no exceptions, whatsoever.
- **Deserialization is validation.** Any type parsed from a manifest, ledger row, checkpoint, or sandbox output uses `#[serde(try_from = ...)]` and goes through the same constructor as hand-built values. Raw input structs use `#[serde(deny_unknown_fields)]`.
- **Validated types have private fields and no setters.** If a value exists, it is valid.
- **Errors separate blame.** Library crates use `thiserror` enums that distinguish "the candidate did something wrong" (an `Outcome`, never retried) from "infrastructure failed" (retried). `BoxError` appears only at Lambda handler boundaries.
- **Order must be stable.** Use `BTreeMap` and `BTreeSet`, or sort into a `Vec`, anywhere iteration order can affect output, hashing, or which durable operations get created.
- **No C in our own crates.** No OpenSSL, no libgit2. The crypto provider under rustls is the one accepted exception.
- **No git in the trusted path.** An experiment is a base tarball hash plus a unified diff, applied inside the guest with a pure-Rust patch crate.
- **No tokio in v1.** No `minos-*` crate depends on tokio; v1 is synchronous (`std::process`, `std::thread::scope`, `std::os::unix::net`, `std::fs::File::lock`). The pinned VMM links tokio internally through `msb-imago`, and `deny.toml` fences tokio to that one edge, so a `minos-*` crate that adds it is refused. The AWS milestone revisits this.
- **Commands are argv arrays.** Never build a shell string from manifest or model input.
- **Supply chain.** Commit `Cargo.lock`. `cargo deny check` gates every build. Pin git dependencies by `rev`.

## minos-core

Core has no async, no AWS, no filesystem, no clock, and no randomness beyond seeded generators. That makes it safe to call during durable replay and easy to property-test. A clippy `disallowed-methods` rule bans `SystemTime::now` in this crate; time arrives as an argument.

```
id.rs  path.rs  manifest.rs  experiment.rs  outcome.rs  policy.rs  route.rs
ledger.rs  require.rs  usage.rs  report.rs  tool.rs  limits.rs
```

**IDs.** `ExperimentId`, `BlobId`, `ManifestId`, `TreeId`, and `PricesId` are distinct types over a SHA-256 digest. Hashing uses a domain-separation prefix and length-prefixed fields:

```rust
impl ExperimentId {
    pub fn of(base: &TreeId, diff: &Diff) -> Self {
        let mut h = Sha256::new();
        h.update(b"minos/experiment/v1\0");
        h.update(base.as_bytes());
        h.update((diff.len() as u64).to_be_bytes());
        h.update(diff.as_bytes());
        Self(h.finalize().into())
    }
}
```

SHA-256 is deliberate: S3 verifies it natively, so the ID doubles as the upload checksum.

**Paths.** `RelPath` accepts only normal components: no `..`, no absolute paths, no empty string.

**Manifest.** `RawManifest` mirrors `minos.toml`. `Manifest::try_from` adds the cross-field checks: `writable` is non-empty and disjoint from readable and judge paths, the policy matches the number of objectives, budgets are within `limits.rs`, images are digests and not tags, and the proposer ladder has at least one eligible tier.

**Diff.** `Diff::parse(text, &manifest)` enforces a size cap, requires every touched path to be in `writable`, and rejects renames, mode changes, symlinks, and binary patches. An `Experiment` can only be built from a parsed `Diff`, so an experiment that edits the judge cannot be represented.

**Metrics.** `Metric` wraps an `f64` that must be finite, and implements `Ord` with `total_cmp`. A candidate can influence the numbers a judge emits; a NaN must never reach policy code.

**Outcome.** Failure is a value:

```rust
pub enum Outcome { Scored(Scorecard), Rejected(Reason), Crashed, TimedOut }
```

**Policy.** `select_greedy` and `select_pareto` are pure functions of recorded results. They apply `min_improvement` from the manifest and break ties on `ExperimentId`, so the result is identical under any permutation of the input.

**Ledger.** Run state is never stored. It is rebuilt by folding events:

```rust
#[serde(tag = "v")]
pub enum Record { V1(Event) }

pub enum Event {
    RunStarted { run: RunId, manifest: ManifestId, baseline: ExperimentId },
    ManifestChanged { manifest: ManifestId },
    Proposed { exp: ExperimentId, parent: ExperimentId, tier: Tier },
    Judged { exp: ExperimentId, judge: JudgeVersion, outcome: Outcome, backend: BackendInfo },
    Selected { generation: u32, decision: Decision },
    Spent { exp: Option<ExperimentId>, usage: Usage },
    ReviewRequested { request: ReviewId, kind: ReviewKind },
    Reviewed { request: ReviewId, answer: ReviewAnswer },
    ReportAcknowledged { exp: ExperimentId, report: ReportHash },
}
```

`Judged` is keyed by experiment and judge version, so re-scoring appends records and never rewrites history.

**Wire formats are an API.** These types persist in checkpoints, S3, and the ledger. Golden-file tests guard serialization, golden vectors guard hashing, and the `v` tag is how a format changes.

## minos-sandbox

Roles are types. Policy is enforced when a spec is built, and backends only ever receive a sealed plan:

```rust
pub struct SandboxSpec<R: Role> { /* image, mounts, budget, PhantomData<R> */ }

impl SandboxSpec<Judge> {
    pub fn secrets(self, s: JudgeSecrets) -> Self { /* read-only mount */ }
}

// No role has a method that attaches a network. Only Judge can mount judge data.
// A candidate with network access or judge data must not compile.

impl<R: Role> SandboxSpec<R> {
    pub fn seal(self) -> LaunchPlan { /* the only constructor of LaunchPlan */ }
}
```

Sandbox output is wrapped so it cannot be used unvalidated:

```rust
/// No Deref, Display, or Debug. The only exits are validating parsers.
pub struct Untrusted<T>(T);

impl Untrusted<Bytes> {
    pub fn into_scorecard(self, limits: &Limits) -> Result<Scorecard, Rejected>;
    pub fn into_log_excerpt(self, max: usize) -> LogExcerpt; // truncated, control chars stripped
    pub fn into_diff_texts(self, limits: &Limits) -> Vec<String>;
}
```

`Backend` has two methods, `capabilities()` and `launch(plan)`. The runner is generic over `B: Backend`. Each binary picks its backend concretely, and a binary that can choose among several at runtime defines its own enum (for example `LocalBackend { Krun, Chv }`). Never use trait objects, and never put a backend enum in `minos-sandbox`. Dropping a `Running` handle destroys the VM.

Launches are idempotent: derive the client token from `ExperimentId` plus role. Every launch sets a hard maximum duration, so nothing depends on cleanup running.

## Durable execution rules (AWS milestone)

These rules and the platform limits below apply to the later AWS milestone; v1 runs locally and has no durable functions (02-CONTEXT.md D-08).

The orchestrator process is destroyed and replayed every time it waits. No in-memory state survives between operations.

1. **Control flow is deterministic.** Which operations get created, and in what order, must be a pure function of the input event and recorded results. Put clock reads, randomness, and service calls inside step bodies.
2. **Steps are idempotent.** A step body can run more than once. Key every side effect by a content-derived ID.
3. **Checkpoints carry IDs and validated values only.** Diffs, transcripts, and logs go to S3. Raw sandbox bytes never enter a checkpoint.
4. **Anything slow is a wait.** A step body must finish inside one 15-minute invocation. Experiments and agent sessions are jobs the workflow suspends on.
5. **Experiment failure is an `Outcome`, never an error.** By default the first item failure fails a whole `map` batch, so the runner always returns a value. Durable timeouts are a backstop only.
6. **One child execution per generation, one per agent session.** This keeps every execution far below the 3,000-operation cap. The parent spends one operation per generation.
7. **Callback IDs are capabilities.** They never enter a sandbox. Only trusted runners complete callbacks.
8. **Use `ctx.race` and `ctx.select_ok`, never `tokio::select!`,** over durable futures. Never depend on `Drop` order between operations.
9. **Publish a Lambda version per deploy.** A run that has been waiting for days must resume on the code it started with.
10. **Alarm on durable execution status.** A handler failure does not fail the Lambda invocation, so the `Errors` metric stays quiet.
11. **Test the operation budget.** Use the SDK's `LocalRunner` (feature `test-util`) to assert that one generation stays under a fixed operation count.

Job dispatch differs by backend. CPU jobs are a `ctx.invoke` of the runner function. GPU jobs are a `wait_for_callback` whose submitter enqueues the job; the GPU daemon completes the callback.

Platform limits to design within:

| Limit | Value |
|---|---|
| Operations per durable execution | 3,000, not adjustable |
| Data persisted per durable execution | 100 MB, not adjustable |
| Single Lambda invocation | 15 minutes |
| Async invocation payload | 1 MB |
| MicroVM lifetime, running plus suspended | 8 hours, not adjustable |
| MicroVM architecture | arm64 only |
| `RunMicrovm` rate | 5 per second by default |
| MicroVM memory across the account | 400 GB per region by default, 1,024 GB in some regions |

## Projects

A project is a directory with three things: `minos.toml` (the manifest), `program.md` (the user's instructions to the proposer), and the surface files. The judge's private data lives outside the project and is uploaded separately.

```toml
[project]
name = "logparse"
kind = "optimize"            # tune | optimize | explore
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"              # greedy | pareto | review
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
```

### Proposer ladder

The loop is fixed code. A model appears only in the propose step, and only where a tier calls for it.

| Tier | What proposes | Needs an agent sandbox |
|---|---|---|
| `sweep` | Seeded sampling over declared params; no model; pure code in core | No |
| `single_shot` | One model call with no tools, returning several diffs | No |
| `planned` | Fixed two-call pipeline: list ideas, drop those already tried, implement the rest | No |
| `agentic` | A tool loop against an agent VM | Yes |

Each project declares its ladder. `ProposerSpec::eligible` checks prerequisites (sweep needs params, single-shot needs the surface to fit the prompt budget), and `minos check` prints the route table with reasons. Routing is a pure function of manifest and ledger: move one tier up the eligible ladder after each `STALL_STEP` generations without improvement.

Phase comes before tier. A project with no judge version starts in `DesignJudge`: the proposer drafts metrics and the run waits for the user's approval. The agent may propose judge changes, params, and dependencies. It never approves them. Each goes to the inbox.

Prompt assembly is deterministic, with the stable prefix (`program.md`, surface files) first so it can be cached. Invalid proposals are dropped, not repaired. Token and turn caps are enforced by the loop, outside the model.

### Judge

The judge is a command plus private data that runs in its own sandbox and writes a small scorecard. The harness launches it and records the result. Rules:

- The candidate reports nothing about itself. The harness measures time and enforces budgets from outside.
- Judge data is never mounted for an agent or a candidate, and is excluded from the source snapshot.
- When scoring requires executing candidate code (for example, loading a trained model), the judge and candidate run in separate sandboxes and talk over a mediated channel. They never share a process.
- Noisy metrics are measured repeatedly. `minos check` runs the baseline several times to establish the noise floor, which informs `min_improvement`.
- Every `Judged` event records backend and architecture. A run with a speed objective is pinned to one backend class.

## Environments and source

**Environment.** The user's Dockerfile feeds every backend. Dependencies are baked in during a build phase that has registry access and no secrets; the experiment phase is offline. Base images are pinned by digest and lockfiles are required. The trusted packer writes `minos-guest` into the root image as `/init` after every file from the user's image, so the Dockerfile cannot replace it, and the VM never runs a Docker entrypoint (02-CONTEXT.md D-18). `minos.lock` records one image digest per backend, since Lambda is arm64 and a GPU box is usually x86_64.

**Root images.** The requirement is any OCI builder that can export a rootfs tar (02-CONTEXT.md D-14). The checked tool chain is the Docker CLI with BuildKit, `docker buildx build --platform linux/arm64 --output type=tar,dest=<file> <context>`, after which Minos's own pure-Rust packer (`minos-backend-krun`, gzip SquashFS) adds `/init` and writes the raw read-only root image (02-SPIKE-EVIDENCE.md RI-03, S06-BOOT). Docker Desktop is one way to get that builder, not a requirement; lima, podman, and Apple's container tool are untested alternatives. Base images are pinned by digest. Two builds of the same Dockerfile gave tars that differed, because the `RUN` step stamped the file it wrote and its directory with the wall-clock time, and root images that matched byte for byte, because the packer zeroes every mtime (RI-04); byte-identical tars and `minos.lock` are Phase 5 (D-16).

An optional `[environment.prepare]` step runs once with allowlisted network access. Its output is frozen as a read-only blob identified by hash.

Lambda snapshots capture memory as well as disk. `minos-guest` must not create anything per-run at startup; that belongs in the run hook.

**Source.** `minos run` packs the working tree into a deterministic tarball (sorted paths, zeroed metadata, regular files only) whose hash is the `TreeId`. It respects `.gitignore` and `.minosignore`, applies a hard denylist, and runs a secret scan that blocks the upload on a hit. It expects a clean commit unless `--allow-dirty` is passed.

Build output is never shared between experiments. Pre-compiled dependencies sit read-only in the image; each experiment's build writes to its own scratch disk.

## Local backends

Local is where a breakout costs the most, because it lands next to the user's keys and home network. Local must be at least as strict as Lambda.

- Hardware virtualization is the default. Process-only isolation (Landlock plus seccomp) is a labelled dev mode, and runs made with it are marked `WeakIsolation` in the ledger and the report.
- macOS has no seccomp and no cgroups. On macOS the hypervisor process is confined by a Seatbelt deny-default profile (`crates/minos-vmm/minos-vmm.sb`), applied by launching it through `/usr/bin/sandbox-exec`. The profile allows reads of the system libraries and of the helper's own binary, config, and libkrunfw, reads of the job's two read-only disk images, reads and writes of the scratch disk and the console file, a short list of sysctls, and exactly one network operation, the AF_UNIX connect to the vsock socket. `hv_vm_create` needs no hypervisor-specific rule (02-SPIKE-EVIDENCE.md SPIKE-05 answer, S05-BOOT, S05-DENY-NET, S05-DENY-FILE). `sandbox-exec` is deprecated by Apple but works on macOS 27.0, so `minos host doctor` checks that the denials still hold before it reports the layer. A dedicated unprivileged `_minos` user is the further layer; `minos host setup` creates it with the recipe in S05-RECIPE, which is recorded but not yet executed and needs `sudo` or a LaunchDaemon to start the helper as that user. Every run records which layers were active in `BackendInfo` and never falls back silently (KRUN-07).
- Files go in as disk images: a shared read-only root, a per-job read-only input disk, and a size-capped scratch disk. Never share a host directory. The host writes the tarball and diff to the input disk without unpacking or applying them.
- The VM boots straight from the raw read-only SquashFS root image on `/dev/vda` (02-CONTEXT.md D-18; 02-SPIKE-EVIDENCE.md S01-ROOTDISK). Stock libkrunfw has `CONFIG_BLK_DEV_INITRD` unset, so there is no initramfs. `/init` is the static musl `minos-guest`, written into the image last by the trusted packer, so a Dockerfile cannot supply its own. `minos-vmm` passes `rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0` through `KernelBuilder::cmdline` and `/init root=/dev/vda` through `KernelBuilder::init_path`, which lands after msb_krun's own `root=/dev/root`; Linux keeps the last `root=`. `dummy.numdummies=0` stops the kernel's built-in dummy driver from creating `dummy0`, by the user's decision (S01-NETIFACES). The boot test asserts the exact `/proc/cmdline` tokens (S01-CMDLINE), so an msb_krun bump cannot change this silently. No VM has a virtio-fs device.
- The packer creates `/dev`, `/proc`, `/sys`, `/tmp`, and `/mnt` in every root image, because nothing can create a directory on a read-only SquashFS root.
- The helper's exit code is not the guest's result. With no virtio-fs device the guest has no way to set it: guest power-off, a guest whose init returns, and a host SIGTERM all end `minos-vmm` with 0 (K7-01, K7-02, K7-05). A returning PID 1 does not panic: libkrunfw patches its exit into an orderly reboot (K7-02). A kernel panic also ends in 0, because of `panic=-1` (S01-INITRAMFS). The guest's status frame over vsock is the only result channel. The host classifies a run from what it observed itself, exit code and `minos-vmm` stderr marker together (02-SPIKE-EVIDENCE.md K7 table, Classification rule for Phase 5): exit 0 with exactly one valid status frame means the guest decided, and the frame says what happened; exit 0 with no valid frame means `Crashed`; 125 with `minos-vmm: watchdog console-cap` means `Crashed` (the guest flooded the console), never retried; the host deadline, or 124 with `minos-vmm: watchdog deadline`, means `TimedOut`; `minos-vmm`'s pre-takeover codes 64, 65, 66, 67, and 70, 126 with `minos-vmm: watchdog parent-gone`, a spawn failure, any signal the host did not send, and any other code mean an infrastructure failure. The markers are written by `minos-vmm`, never by the guest, which reaches only the console file. Under Seatbelt the helper also exits 0 when its socket connect is denied, so success is always judged by the frame (S05-CTRL-SOCK).
- libkrunfw comes from the Homebrew tap `libkrun/krun` (`brew tap libkrun/krun`, `brew trust libkrun/krun`, `brew install libkrunfw`); Minos needs only libkrunfw from it, not libkrun, krunkit, or krunvm (02-SPIKE-EVIDENCE.md H-01). `minos-vmm` loads it by absolute path and checks its SHA-256 first (K5).
- `minos-vmm` records its parent PID as the first statement of `main`, and its watchdog treats a changed parent, or a parent of PID 1 (launchd never legitimately starts it), as gone, so a helper orphaned at any point stops with 126 (02-07 DEFECT-01, K7-07, K7-07b).
- The watchdog polls the console file every 50 ms, so a flooding guest overshoots the console cap: 30000 to 48954 bytes over a 262144-byte cap (K7-04). Size the console disk budget as the cap plus at least one poll of console throughput, or replace the file console with a capped writer.
- `minos-vmm` installs no `log` backend yet, so msb_krun's warnings and errors are discarded, including the vsock drops below (DIAG-01). Phase 5 installs one.
- No network device does not mean no network code in the guest kernel. The libkrunfw kernel has `CONFIG_TSI=y` and `CONFIG_DUMMY=y` (H-02, S01-NETIFACES). Upstream libkrun's networking default is not air-gapped: it turns on TSI, which proxies guest INET sockets through the host, whenever no virtio-net device is configured. Minos drives libkrun through `msb_krun`, which reverses that default (`inet_hijack` off). The guest has no egress only because the VMM never enables inet hijack: `msb_krun`'s `net` feature stays off, and `crates/minos-vmm/clippy.toml` bans the hijack, port-remap, virtio-fs, non-raw-format, guest-environment, and host-console methods (D-PROBE). `egress_fails_closed` guards it: every guest TCP connect fails, IPv4 with `ENETUNREACH` (101) and IPv6 with `EADDRNOTAVAIL` (99), no host canary sees a connection, and the guest sees only `lo` and no virtio-net device (S02-EGRESS). The root-disk arguments pass `dummy.numdummies=0`, so the kernel creates no `dummy0`.
- Output comes back as a tar stream over vsock, parsed in Rust with caps. Never mount a disk the guest wrote. On the pinned `msb_krun_devices` 0.1.41 one vsock connection carries at most 8192 bytes guest to host (8192 is the macOS `net.local.stream.sendspace` default); above that, data is silently lost or the guest stalls, for three defects in that crate and none in Minos: a shutdown race discards bytes queued above the 8 KiB send buffer, a one-descriptor packet parser drops payloads over 32 KiB, and a credit update sent only every 4 MiB deadlocks streams over 256 KiB (02-SPIKE-EVIDENCE.md S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01). By the user's decision (S03-ANSWER, 2026-10-10: "work within the limits for now, if we need to reassess later fine"), guest output is a sequence of short vsock connections, each carrying one length-prefixed, sequence-numbered chunk of at most 4 KiB, guest to host only. The host reassembles chunks in order under a per-chunk cap and a total cap, and treats a gap, a duplicate, a length mismatch, or an over-cap chunk as a candidate fault, never an infrastructure retry. Forking the VMM crate was considered and rejected for now; reporting the defects upstream is the user's call; reassess only if chunking proves a bottleneck. One connection measured about 6 MiB/s in DIAG-01's instrumented runs; the chunked protocol's throughput is unmeasured until Phase 5.
- GPU passthrough requires an IOMMU and a GPU the host does not use for display. Reset the device between runs.
- Preflight fails closed and is enforced by the type system:

```rust
pub fn preflight(host: &Host, plan: &LaunchPlan) -> Result<Cleared, Vec<Refusal>>;
pub async fn launch(&self, plan: LaunchPlan, _ok: Cleared) -> Result<Running, LaunchError>;
```

`minos host doctor` runs the same preflight and explains each refusal.

## Cost

The ledger stores usage, never dollars: vCPU-seconds, GB-seconds, snapshot megabytes read, GPU seconds, tokens in and out, durable operations, and the spot price at launch when spot was used. Dollars are derived from usage and a user-editable price table at `~/.config/minos/prices.toml`.

- Rates are parsed from strings as exact decimals with pico-dollar precision (10^-12 USD; published AWS rates have 10 fractional digits). Totals are integer micro-dollars. No floats.
- Output shows one figure, always measured: `approx. cost: $31.40`. Never print an estimate, and never use the word "exact". One type owns that formatting, so no command prints a bare dollar figure.
- Spend caps compare the manifest limit against metered spend, checked before each generation.

## CLI

```
minos init --kind <tune|optimize|explore>   scaffold minos.toml, program.md, minos.Dockerfile
minos env build | env shell                 build images and write minos.lock; debug shell with no network
minos host setup | host doctor              one-time local setup; run preflight and explain refusals
minos judge push <dir>                      upload judge data as a new judge version
minos check                                 validate, build, judge the baseline several times, print routes
minos run --generations N                   start a run
minos status | log | show <exp>             progress, ledger, one experiment
minos steer <run> "<note>"                  add a note the next propose step reads
minos inbox | answer <n> ...                decisions waiting on the user
minos params add <names...>                 declare sweep knobs; produces a new manifest version
minos rejudge <run> --judge <v>             re-score the archive under another judge version
minos report <exp>                          the review artifact
minos adopt <exp> --reviewed <hash>         write a patch file
minos stop <run>
```

### Report and adopt

`build_report` is a pure function in core, so the same ledger and blobs always give the same report, and the report can be hashed. It contains the summary against the noise floor, the cumulative diff from baseline to winner, the lineage, provenance (isolation level, judge version, image digests), and flags. Flags direct attention; they are not verdicts: new dependencies, spawned processes, sockets, environment reads, encoded blobs, literals that match visible benchmark inputs, operations the guest denied, held-out metrics diverging from visible ones, and weak isolation anywhere in the lineage.

`minos adopt` refuses without the hash of a report the user acknowledged for that exact experiment. It writes a patch file and never applies one. The renderer strips terminal escape sequences from every string that originated in a sandbox or a model.

## Testing

- Property tests (`proptest`) for parsers, `RelPath`, `Diff::parse`, policy permutation-invariance, and the ledger fold.
- Golden vectors for every ID hash and golden files for every persisted wire format.
- Compile-fail tests (`trybuild`) proving a `SandboxSpec<Candidate>` cannot get judge data and that `launch` cannot be called without `Cleared`.
- `LocalRunner` tests for orchestrator logic, including the per-generation operation budget.
- In-memory `Ledger` and `Blobs` implementations for everything above the store.

## Commands

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo doc --workspace --no-deps          # the only command that enforces [workspace.lints.rustdoc]
cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings   # Phase 2 onward (02-CONTEXT.md D-03)
cargo build -p minos-guest --release --target aarch64-unknown-linux-musl   # the static /init; a build step, not a gate command
cargo build -p minos-vmm
codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm   # after every build of minos-vmm
codesign --display --entitlements - target/debug/minos-vmm   # must list com.apple.security.hypervisor
cargo lambda build --release --arm64     # AWS milestone only; not used in v1
```

Signing (02-CONTEXT.md D-11): every `cargo build` relinks `minos-vmm` and drops its entitlement, so the sequence is always build, then sign, then run. `cargo test --workspace` can relink it too, so re-sign before a VM run. There is no xtask, script, `build.rs`, or cargo alias; `minos host setup` may run the same argv from Rust later. The hardened runtime is on, and the entitlement set is `com.apple.security.hypervisor` plus `com.apple.security.cs.disable-library-validation`, because the hardened runtime's library validation refuses the Homebrew libkrunfw dylib ("mapping process and mapped file (non-platform) have different Team IDs"); `minos-vmm`'s preflight SHA-256 check of libkrunfw, before the `dlopen`, is the integrity check that remains (02-SPIKE-EVIDENCE.md K5). Only `minos-vmm` is signed; the `minos` CLI has no entitlement. A helper without the entitlement cannot create a VM: `hv_vm_create` fails and `minos-vmm` exits 70 before the VM starts (S04-UNSIGNED). The VM spikes are ignored integration tests: `cargo test -p minos-backend-krun --test <spike_boot|spike_exits|spike_net|spike_seatbelt|spike_rootimg> -- --ignored --nocapture --test-threads=1`, with `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` set as in 02-06-PLAN.md E-1 (`spike_net` also needs `MINOS_SPIKE_HOST_IP`).

The sixth command runs from Phase 2 onward (02-CONTEXT.md D-03). The guest is always Linux, because libkrunfw ships a Linux kernel, so `minos-guest` needs a Linux target even though Minos runs on macOS. `/init` is written into whatever root image the user's Dockerfile produces, so it must be a fully static binary that loads no libc from that image: hence `aarch64-unknown-linux-musl`, listed in `rust-toolchain.toml` and linked with `rust-lld`. `.cargo/config.toml` gives that target a `linker` key and nothing else. A flags key there would silently replace the `[build]` forbid backstop for the guest (01-GATE-EVIDENCE.md G-02-CLOSED). The host gate still builds `minos-guest` for `aarch64-apple-darwin`, so keep everything the host can compile (the job and frame codecs, the tar and `minos-core` paths) out of `#[cfg(target_os = "linux")]`; Linux-only code lives in `crates/minos-guest/src/linux.rs`. `cargo test --workspace` cannot run that Linux code on macOS; it waits for a Linux runner, and the VM spikes exercise it meanwhile.

Every phase verification also runs the C-02 check, `grep -rnE '#!?\[allow\(' crates --include='*.rs'`, which must print nothing. An inner `#![allow(..., reason = "...")]` on any lint that is not `forbid` passes `cargo clippy` (01-GATE-EVIDENCE.md R-03), so for those lints this grep, not the gate, is what enforces the no-`allow` rule.

Run `cargo clean` at the end of every phase, after verification passes and before the phase is marked complete. No exceptions.

The durable SDK requires Rust 1.94.1 or newer and edition 2024.

## Build order

Milestone v1 is local-first, on macOS arm64 (02-CONTEXT.md D-07; `.planning/ROADMAP.md`):

1. Workspace and gates: the plain cargo gate commands and Cargo-native policy.
2. Platform spikes (libkrun boot, egress, vsock, signing, Seatbelt, root images, the `claude -p` contract), recorded in `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`. In parallel: `minos-core` with its tests.
3. `minos-store` (file and memory), `minos-sandbox`, and the deterministic source tarball.
4. `minos-guest` and the krun backend (`minos-backend-krun`, `minos-vmm`), `minos env build`, and `minos host doctor`. In parallel: `minos-propose` with the seeded sweep and the `claude` CLI provider.
5. `minos-runner` and `minos-loop`, the loop built against fakes first.
6. The CLI and the sample `optimize` project end to end, including `report` and `adopt`.

Later milestone (AWS). The original build order, kept for when that work starts (02-CONTEXT.md D-08):

1. A durable function in Rust, pinned by `rev`, with one step and one callback. Count the real operations it records.
2. A Lambda MicroVM launched with no egress connector and no execution role, running `minos-guest`, that takes a job and returns output. This settles the two most important open questions.
3. `minos-core` with its tests.
4. One CPU-only `optimize` project end to end on Lambda, single-shot proposer, greedy policy, including `report` and `adopt`.
5. Sweep and planned tiers, then the agentic tier.
6. Local libkrun backend.
7. GPU: the daemon, the Cloud Hypervisor backend, and autoresearch itself as a `tune` project.

## Open questions

Unverified. Each needs a test or a doc check before code depends on it.

- How the console cap should work in Phase 5: the launcher's watchdog polls the console file and overshot a 262144-byte cap by 30000 to 48954 bytes (02-SPIKE-EVIDENCE.md K7-04, K7-11); a custom console backend could cap exactly, if it can serve as the kernel console.
- Whether to keep the default virtio-rng device (`virtio0:0x0004`, attached by default; S01-FRAME).
- libkrunfw is hashed and then loaded by path; Phase 5 decides whether to copy it into a private directory first to close that gap.
- How the helper starts as the dedicated `_minos` user: a sudoers rule pinned to the helper path, or a LaunchDaemon (S05-RECIPE).
- Whether `--permission-prompts none` and `--permission-mode dontAsk` behave differently when a permission request occurs; no call made one with tools off (SPIKE-07 contract, C-08).
- Whether `claude` writes a session directory without `--no-session-persistence`; there was no control call (SPIKE-07 contract, C-14).
- Whether the Seatbelt profile's starting allowances can be narrowed: rows 1 to 6 of the SPIKE-05 answer table were not minimized one by one, and only the socket rule was shown to be needed (SPIKE-05 answer).
- The cost of the chunked vsock output protocol: one connection per 4 KiB chunk, so 64 MiB takes 16384 connections, and the supervisor has handled only two connections per run so far (S03-ANSWER).
- Launch rate and memory per VM for concurrent candidates (K8; deferred with NEXT-04).

### Deferred to the AWS milestone

- Can a MicroVM be launched with no egress connector at all? The API lists egress connectors as a parameter and the docs show a launch without one, but this has not been tested. If not, candidates must run on the user's own hardware.
- Can a MicroVM run with no execution role, or a role with zero permissions?
- Exact builder method names in `aws-sdk-lambdamicrovms`.
- How many durable operations each SDK call records, and whether each `wait_for_condition` poll counts as one. This decides how the CPU runner waits for a VM to finish.
- Whether EC2 GPU instance types support nested virtualization. The design assumes they do not.
- Whether MicroVM and Bedrock rates are available from the AWS price list API. Until checked, `prices.toml` is edited by hand.

## Verified facts, local (as of 2026-10-10)

Each fact cites `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`.

- libkrunfw 5.6.2 from `libkrun/krun`; guest kernel 6.12.109; `CONFIG_BLK_DEV_INITRD` unset (H-01, H-02, S01-CMDLINE).
- msb_krun 0.1.41 boots it on macOS 27 arm64 from crates under `forbid(unsafe_code)`, from a raw read-only SquashFS root with no initramfs and no virtio-fs (S01-ROOTDISK); the initramfs form fails (S01-INITRAMFS).
- A helper without the hypervisor entitlement cannot create a VM: `hv_vm_create` fails and `minos-vmm` exits 70 before the VM starts. The ad-hoc signed helper with the hardened runtime and the entitlements `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation` boots (S04-UNSIGNED, S04-SIGN, K5).
- Guest egress fails to loopback, the LAN, and public IPv4 and IPv6, and no host canary sees a connection (S02-EGRESS).
- One vsock connection delivers any guest-to-host stream of 8192 bytes or less intact. Above that, the pinned `msb_krun_devices` 0.1.41 loses data or stalls the guest, for three defects in that crate, so the 1 MiB, 4 MiB, and 64 MiB single-connection tests fail; by the user's decision, output is carried as short chunked connections (S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01, S03-ANSWER).
- Seatbelt: a deny-default profile around `minos-vmm` still lets it create and run a VM, `hv_vm_create` needs no hypervisor-specific rule, and the same profile denies an outbound TCP connection and an unlisted file (SPIKE-05 answer, S05-BOOT, S05-DENY-NET, S05-DENY-FILE).
- A digest-pinned Dockerfile becomes a bootable root image through Docker buildx and the Rust packer (RI-03, S06-BOOT).
- The helper's exit code does not carry the guest's result (K7 table).
- `claude` 2.1.296 under subscription login returns `structured_output` with all tools off, `--json-schema`, `--no-session-persistence`, an empty directory, and the six-variable environment; `num_turns` is 2 under `--max-turns 1` with a schema, because structured output goes through the `StructuredOutput` tool; `usage` carries `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, and `cache_read_input_tokens`, among other fields (SPIKE-07 contract).

## Verified facts, AWS (as of 2026-10-08)

- Durable execution SDK: [aws/aws-durable-execution-sdk-rust](https://github.com/aws/aws-durable-execution-sdk-rust). Experimental preview, installed as a git dependency. Operations: `step`, `wait`, `invoke`, `run_in_child_context`, `with_retry`, `wait_for_condition`, `create_callback`, `wait_for_callback`, `map`, `parallel`, and four combinators. The function's role needs `lambda:CheckpointDurableExecution` and `lambda:GetDurableExecutionState`.
- MicroVM API: [RunMicrovm reference](https://docs.aws.amazon.com/lambda/latest/microvm-api/API_RunMicrovm.html) and [running MicroVMs](https://docs.aws.amazon.com/lambda/latest/dg/microvms-launching.html). Images are built from a Dockerfile into a snapshot. Lifecycle hooks are HTTP paths under `/aws/lambda-microvms/runtime/v1/` (`ready`, `run`, `resume`, `suspend`, `terminate`).
- Rust client: [aws-sdk-lambdamicrovms](https://docs.rs/aws-sdk-lambdamicrovms).
- Quotas: [Lambda quotas](https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html).
- Prices: [Lambda pricing](https://aws.amazon.com/lambda/pricing/). Durable functions bill per operation, per GB written, and per GB-month retained, on top of normal compute. MicroVMs bill per vCPU-second and GB-second, plus snapshot reads, writes, and storage.
