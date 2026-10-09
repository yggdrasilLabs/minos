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
- **Check external APIs against their docs before writing code against them.** Lambda MicroVMs (launched June 2026) and the Rust durable execution SDK (experimental preview) are newer than most training data. Do not guess method names. Links are in [Verified facts](#verified-facts-as-of-2026-10-08).
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

There is no always-on service. The CLI talks to AWS with the user's own credentials.

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

The agent loop runs in trusted code, which calls Bedrock using the function's IAM role. There is no API key anywhere. Only tool calls enter the agent VM, where `minos-guest` executes them. Model output is deserialized into a closed enum, so an out-of-bounds call fails validation and is returned to the model as an error:

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

## Durable execution rules

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

**Environment.** The user's Dockerfile feeds every backend. Dependencies are baked in during a build phase that has registry access and no secrets; the experiment phase is offline. Base images are pinned by digest and lockfiles are required. `minos` appends a final stage that installs `minos-guest` as the entrypoint. `minos.lock` records one image digest per backend, since Lambda is arm64 and a GPU box is usually x86_64.

An optional `[environment.prepare]` step runs once with allowlisted network access. Its output is frozen as a read-only blob identified by hash.

Lambda snapshots capture memory as well as disk. `minos-guest` must not create anything per-run at startup; that belongs in the run hook.

**Source.** `minos run` packs the working tree into a deterministic tarball (sorted paths, zeroed metadata, regular files only) whose hash is the `TreeId`. It respects `.gitignore` and `.minosignore`, applies a hard denylist, and runs a secret scan that blocks the upload on a hit. It expects a clean commit unless `--allow-dirty` is passed.

Build output is never shared between experiments. Pre-compiled dependencies sit read-only in the image; each experiment's build writes to its own scratch disk.

## Local backends

Local is where a breakout costs the most, because it lands next to the user's keys and home network. Local must be at least as strict as Lambda.

- Hardware virtualization is the default. Process-only isolation (Landlock plus seccomp) is a labelled dev mode, and runs made with it are marked `WeakIsolation` in the ledger and the report.
- The hypervisor process runs as a dedicated unprivileged user with its own seccomp filter and cgroup limits.
- Files go in as disk images: a shared read-only root, a per-job read-only input disk, and a size-capped scratch disk. Never share a host directory. The host writes the tarball and diff to the input disk without unpacking or applying them.
- Output comes back as a tar stream over vsock, parsed in Rust with caps. Never mount a disk the guest wrote.
- GPU passthrough requires an IOMMU and a GPU the host does not use for display. Reset the device between runs.
- Preflight fails closed and is enforced by the type system:

```rust
pub fn preflight(host: &Host, plan: &LaunchPlan) -> Result<Cleared, Vec<Refusal>>;
pub async fn launch(&self, plan: LaunchPlan, _ok: Cleared) -> Result<Running, LaunchError>;
```

`minos host doctor` runs the same preflight and explains each refusal.

## Cost

The ledger stores usage, never dollars: vCPU-seconds, GB-seconds, snapshot megabytes read, GPU seconds, tokens in and out, durable operations, and the spot price at launch when spot was used. Dollars are derived from usage and a user-editable price table at `~/.config/minos/prices.toml`.

- Rates are parsed from strings as exact decimals with nano-dollar precision. Totals are integer micro-dollars. No floats.
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
cargo lambda build --release --arm64     # Lambda functions; plain cargo build links the wrong glibc
```

Run `cargo clean` at the end of every phase, after verification passes and before the phase is marked complete. No exceptions.

The durable SDK requires Rust 1.94.1 or newer and edition 2024.

## Build order

1. A durable function in Rust, pinned by `rev`, with one step and one callback. Count the real operations it records.
2. A Lambda MicroVM launched with no egress connector and no execution role, running `minos-guest`, that takes a job and returns output. This settles the two most important open questions.
3. `minos-core` with its tests.
4. One CPU-only `optimize` project end to end on Lambda, single-shot proposer, greedy policy, including `report` and `adopt`.
5. Sweep and planned tiers, then the agentic tier.
6. Local libkrun backend.
7. GPU: the daemon, the Cloud Hypervisor backend, and autoresearch itself as a `tune` project.

## Open questions

Unverified. Each needs a test or a doc check before code depends on it.

- Can a MicroVM be launched with no egress connector at all? The API lists egress connectors as a parameter and the docs show a launch without one, but this has not been tested. If not, candidates must run on the user's own hardware.
- Can a MicroVM run with no execution role, or a role with zero permissions?
- Exact builder method names in `aws-sdk-lambdamicrovms`.
- How many durable operations each SDK call records, and whether each `wait_for_condition` poll counts as one. This decides how the CPU runner waits for a VM to finish.
- Whether EC2 GPU instance types support nested virtualization. The design assumes they do not.
- Whether MicroVM and Bedrock rates are available from the AWS price list API. Until checked, `prices.toml` is edited by hand.

## Verified facts (as of 2026-10-08)

- Durable execution SDK: [aws/aws-durable-execution-sdk-rust](https://github.com/aws/aws-durable-execution-sdk-rust). Experimental preview, installed as a git dependency. Operations: `step`, `wait`, `invoke`, `run_in_child_context`, `with_retry`, `wait_for_condition`, `create_callback`, `wait_for_callback`, `map`, `parallel`, and four combinators. The function's role needs `lambda:CheckpointDurableExecution` and `lambda:GetDurableExecutionState`.
- MicroVM API: [RunMicrovm reference](https://docs.aws.amazon.com/lambda/latest/microvm-api/API_RunMicrovm.html) and [running MicroVMs](https://docs.aws.amazon.com/lambda/latest/dg/microvms-launching.html). Images are built from a Dockerfile into a snapshot. Lifecycle hooks are HTTP paths under `/aws/lambda-microvms/runtime/v1/` (`ready`, `run`, `resume`, `suspend`, `terminate`).
- Rust client: [aws-sdk-lambdamicrovms](https://docs.rs/aws-sdk-lambdamicrovms).
- Quotas: [Lambda quotas](https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html).
- Prices: [Lambda pricing](https://aws.amazon.com/lambda/pricing/). Durable functions bill per operation, per GB written, and per GB-month retained, on top of normal compute. MicroVMs bill per vCPU-second and GB-second, plus snapshot reads, writes, and storage.
