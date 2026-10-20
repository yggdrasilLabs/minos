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

The agent loop runs in trusted code, which calls Bedrock using the function's IAM role. Minos never reads, stores, or forwards a model credential. Models are reached through a `ModelProvider` abstraction in `minos-propose`. In v1 the personal default execs the user's own unmodified `claude` binary in print mode (02-SPIKE-EVIDENCE.md SPIKE-07 contract): the canonical binary under `~/.local/share/claude/versions/`, never whatever `claude` is first on `PATH` (it may be a wrapper that injects settings), run from a fresh empty directory outside any repository, with the environment cleared to `HOME`, `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `USER`, `LOGNAME`, `TMPDIR`, and `LANG`, the prompt on stdin, and `-p --model <model> --tools "" --output-format json --json-schema '<schema>' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1` (the spike ran `--model haiku`). Its `structured_output` is model output and is parsed strictly; only token counts are recorded, never the CLI's dollar estimate. Anthropic's terms allow this only for a user running their own install under their own subscription. Shared or hosted use goes through an API-key or Bedrock provider, later implementations of the same abstraction; on AWS the agent loop calls Bedrock with the function's IAM role.

The provider as built is `ClaudeCli` (06-CONTEXT D-01 to D-17 with its research amendments; 06-RESEARCH). `ClaudeCli::preflight(&ParentEnv)` is its only constructor, so holding one proves the per-run preflight passed, the way a `Cleared<R>` proves the sandbox preflight did. The binary is `$HOME/.local/bin/claude` resolved to a direct child of `$HOME/.local/share/claude/versions/`: a regular file owned by the user, not group- or world-writable, named by its version (D-01). It is resolved again before every call, and `--version` and `auth status` run again whenever the target changed, because the auto-updater keeps about four versions and moves the link (D-01 and D-02 amended; 06-RESEARCH Finding 3, Pitfall 1); a login refusal then stops the run with `StopReason::ProviderAuth`, and any other refusal or a vanished binary is an infrastructure failure. The per-run preflight refuses a parent environment that sets any of the 13 credential or redirect variables the CLI reads or any `CLAUDE_CODE_USE_*` variable, naming them by name only, never by value (D-03 extended; 06-RESEARCH Pitfall 4); a `$TMPDIR` inside a repository or project configuration (a `.git`, `CLAUDE.md`, or `.mcp.json` in it or an ancestor, or a `.claude` other than `$HOME/.claude`), because Claude Code reads project configuration up the directory tree; a version below 2.1.296 or unequal to the file name (D-04); and an `auth status` other than exit 0 with `loggedIn` true, `authMethod` `claude.ai`, `apiProvider` `firstParty`, and no `apiKeySource` key, which is absent on a clean subscription login (D-02 amended; 06-RESEARCH Finding 2). The provider never reads, stores, or forwards a credential: the login is the CLI's own, checked only through `auth status`, whose account fields are never bound. Each call runs in a fresh 0700 `minos-claude-*` directory under `$TMPDIR`, removed afterwards, under a 15-minute deadline (`CALL_DEADLINE`) that kills the process group, with stdout capped at 4 MiB (`MAX_ENVELOPE_BYTES`) and stderr drained and never shown (D-16). The result classifier keys on `is_error` plus `api_error_status`, which exists only on the `success` variant (D-13 amended; 06-RESEARCH Finding 1, Pattern 7): status 429 or a usage-limit message stops the run with `StopReason::ProviderLimit`; 401, 403, or an auth message stops it with `StopReason::ProviderAuth` (D-15); `error_max_turns`, `error_max_structured_output_retries`, or a refusal is an empty call, zero valid proposals and never infrastructure (D-12); and anything else is an infrastructure failure. The provider never retries a call. The model returns `{rationale, files:[{path, content}]}`, the full contents of each writable file a proposal changes, and Minos derives the canonical diff with `diff_trees`, so the model never writes hunk headers (D-10). `Origin::Model { model }` records the resolved model ID from `modelUsage` when the call names exactly one model, else the manifest's model, and `Spent` is recorded per model (06-RESEARCH Open Question 3). The prompt goes on stdin as one block, so in v1 its stable prefix is not cached across generations and no caching saving is claimed (06-RESEARCH Pitfall 7).

Only tool calls enter the agent VM, where `minos-guest` executes them. Model output is deserialized into a closed enum, so an out-of-bounds call fails validation and is returned to the model as an error:

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
  minos-source/          the deterministic source snapshot: walk, denylist, secret scan, clean check, ustar tarball
  minos-backend-krun/    LaunchPlan -> libkrun VM, driven through the minos-vmm process: KrunBackend, the host probes,
                         job directories and the sweep, the supervisor, the chunk receiver, the embedded Seatbelt
                         profile, the input disk writer, and the root image packer
  minos-vmm/             per-VM launcher; the only binary that links the VMM crate
  minos-guest/           static binary inside every VM; reads its job through JobSource, sends output through
                         OutputSink (D-07); the minos-probe fixture binary is test-only
  minos-propose/         the `ModelProvider` abstraction, `ClaudeCli` and its preflight, `FakeProvider`, deterministic
                         prompt assembly, and the sweep and single_shot tiers behind `Proposer<P>` (planned and agentic
                         arrive with TIER-01)
  minos-runner/          local runner pipeline, generic over Backend
  minos-loop/            local generation driver: propose, run, judge, select
  minos-cli/             installs the `minos` binary; the only place concrete types meet (lib: locator, FileBlobSource);
                         ships `env build`, `host doctor`, and `host setup`
```

Later milestones, not in the workspace yet: `minos-backend-lambda` (LaunchPlan -> Lambda MicroVM), `minos-backend-chv` (Cloud Hypervisor VM with VFIO), `minos-orchestrator` (the durable functions: run, generation), `minos-store-aws` (DynamoDB and S3), and the GPU daemon.

Dependency direction is strict, and `deny.toml` enforces it with `[bans]` wrapper entries, so a crate that links outside its lane is refused by `cargo deny check`:

- `core` depends on no internal crate; `store`, `sandbox`, `source`, `propose`, and `guest` depend on `core`.
- `backend-krun` depends on `sandbox` (and `core`). It is the only crate that knows about `minos-vmm`.
- `runner` depends on `core`, `store`, and `sandbox`, and is generic over `Backend`. It never names a backend crate.
- `loop` depends on `core`, `store`, and `propose`. It never links `sandbox` or a backend, so it never sees raw sandbox output. The orchestrator, when it arrives, follows the same rule.
- `vmm` depends on no internal crate.
- `source` depends on `core` only and only `cli` links it. It reads the user's own working tree and execs the user's `git` read-only for the clean check (04-CONTEXT D-02), so it never sits in the loop's or the runner's graph.
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
- **No git in the trusted path.** An experiment is a base tarball hash plus a unified diff, applied inside the guest with a pure-Rust patch crate. The read-only clean check on the user's own working tree is the one exception; see Environments and source.
- **No tokio in v1.** No `minos-*` crate depends on tokio; v1 is synchronous (`std::process`, `std::thread::scope`, `std::os::unix::net`, `std::fs::File::lock`). The pinned VMM links tokio internally through `msb-imago`, and `deny.toml` fences tokio to that one edge, so a `minos-*` crate that adds it is refused. The AWS milestone revisits this.
- **Commands are argv arrays.** Never build a shell string from manifest or model input.
- **Supply chain.** Commit `Cargo.lock`. `cargo deny check` gates every build. Pin git dependencies by `rev`.

## minos-core

Core has no async, no AWS, no filesystem, no clock, and no randomness beyond seeded generators. That makes it safe to call during durable replay and easy to property-test. A clippy `disallowed-methods` rule bans `SystemTime::now` in this crate; time arrives as an argument.

```
lib.rs  limits.rs  id.rs  path.rs  text.rs  metric.rs  outcome.rs  usage.rs  money.rs  image.rs  manifest.rs
diff/{mod,apply,compose}.rs  experiment.rs  ledger.rs  state.rs  policy.rs  route.rs  sweep.rs  job.rs  chunk.rs
lock.rs  guest.rs  report.rs
```

`chunk.rs` is the authenticated chunk codec that carries guest output to the host over short vsock connections, used by both the guest sender and the host receiver. `guest.rs` holds the guest job, the guest status, and the `MINOSJB2` input disk codec that the backend writes and PID 1 reads. `lock.rs` is `minos.lock`: one locked image per backend and architecture, its versioned TOML codec, and `Lock::check`, which a run calls before it starts.

`tool.rs` (the agentic `ToolCall` enum) arrives with the agentic tier (TIER-01), and the `requires` table lives in manifest.rs. `lib.rs` re-exports the principal types at the crate root, and every module stays public.

**IDs.** `ExperimentId`, `BlobId`, `ManifestId`, `TreeId`, `PricesId`, `ImageDigest`, `JudgeVersion`, `RunId`, `EventKey`, `SpendId`, `ReviewId`, `PromptHash`, and `ReportHash` are distinct types over a SHA-256 digest, each printed and serialized as 64 lowercase hex characters. Plain IDs (`BlobId`, `TreeId`, `ManifestId`, `PricesId`, `ImageDigest`) are the bare SHA-256 of content, so they match the checksum S3 verifies. Composite IDs are hashed only through one builder, `IdHasher`: a NUL-terminated domain, then each field as a fixed 32-byte digest, a `u64` big-endian integer, or a `u64` big-endian length prefix followed by the bytes:

```rust
impl ExperimentId {
    pub fn of(base: &TreeId, diff: &Diff) -> Self {
        Self::from_digest(
            IdHasher::new("minos/experiment/v1") // the domain, then one NUL
                .digest(base.as_bytes())
                .bytes(diff.as_bytes()) // u64 big-endian length, then the canonical diff
                .finish(),
        )
    }
}
```

`ExperimentId::baseline(&TreeId)` hashes the base tree with a zero-length diff, so no experiment built from a parsed diff can share its ID (03-CONTEXT.md D-04). `RunId::of(manifest, base, started_at)` takes the start time as an argument the shell reads, because core has no clock (D-06).

SHA-256 is deliberate: S3 verifies it natively, so the ID doubles as the upload checksum.

**Paths.** `RelPath` accepts only normal components: no `..`, no absolute paths, no empty string.

**Manifest.** `RawManifest` mirrors `minos.toml`; every raw table refuses unknown keys, and `Manifest::try_from` is the only constructor of `Manifest`. Its cross-field checks: `writable` holds 1 to 64 exact files, never a directory or a glob, and is exact-path disjoint from `readable` (03-CONTEXT.md D-01); `writable` never contains `program.md` (the `[project] program` path), the Dockerfile, `minos.toml`, `minos.lock` (nor does `readable` contain `minos.lock`), or any project path the judge command names, which is `argv[0]` and every later argument, or the value of a `--name=value` option, that parses as a `RelPath` after one leading `./` is stripped (a bare word with no `/` names no path, as for a command's program), and a path-like judge argument that is not a valid `RelPath` is rejected rather than ignored; `readable` never contains a judge path, and every judge path lies under a `[source] exclude` entry, so the judge program reaches neither the prompt nor the source snapshot (the judge runs from its own pushed data); no surface path lies under a `[source] exclude` entry; distinct surface, program, Dockerfile, and judge paths must not collide under ASCII case folding; greedy is the only v1 policy and needs exactly one objective; budgets, `[policy] stall_step`, and `[budget] max_empty_generations` and `max_infra_failures` are bounded by `limits.rs` and have defaults, with minimums of 1 so the circuit breaker cannot be disabled (D-13, D-14); `[environment] memory_mib` and `scratch_mib` lie within the VM machine bounds, which live in `minos-core::limits` because core depends on no internal crate, and the memory must hold the 256 MiB guest reserve, the scratch cap, and at least 256 MiB for the candidate (`ManifestError::MemoryBudget`; 05-CONTEXT.md D-05, D-06); and at least one proposer is eligible on manifest facts alone. The `[judge] version` integer is a local label naming which pushed judge data the project uses; the recorded `JudgeVersion` hashes the judge data tree plus the canonical judge spec (the argv, the gates, and the objectives with goal and held-out pairing), and excludes the label and `min_improvement`. The v1 manifest names a Dockerfile, not an image, so "images are digests" is enforced by `ImageRef` and by `image::base_images` on the Dockerfile's `FROM` lines, and `Manifest::try_from` stays pure: it reads no file.

**Diff.** `Diff::parse(text, writable, limits)` (the writable set comes from `Manifest::writable()`) enforces byte, line, file, and hunk caps from `limits.rs`; the text is LF only, so a CR or NUL byte is rejected and a CRLF surface file cannot be edited in v1. Every touched path must equal a `writable` entry byte for byte (03-CONTEXT.md D-01). Renames, copies, mode changes, symlink and gitlink modes, binary patches, and `/dev/null` create or delete sections are rejected (D-02: a diff may only modify an existing writable file). A diff that changes nothing, with no hunks or with a hunk whose old and new lines are identical, is rejected (D-03). Diffy's parser is one stage of seven: our own line grammar runs first and rejects every line diffy would skip (prose, extra lines, a `diff --git` path that disagrees with `---`/`+++`, quoted paths, timestamps), and a line-accounting check proves diffy read every line. The result is re-serialized to a canonical form (sections sorted by path, `diff --git a/P b/P`, `--- a/P`, `+++ b/P`, `@@ -a,b +c,d @@` with both lengths and no function context, no `index` line, `\ No newline at end of file` after a line lacking its newline), which must re-parse to the same diff, and `ExperimentId::of(base, &diff)` hashes those canonical bytes, never the text a model wrote. An experiment's diff is cumulative from the run's base tree, because `ExperimentId` takes only the base `TreeId` and the diff (user decision 2026-10-14, 03-CONTEXT.md D-04 amendment; 03-RESEARCH Open Question 1 RESOLVED). The baseline is `ExperimentId::baseline(&TreeId)`, and no parsed `Diff` is ever empty (D-04). An `Experiment` can only be built from a parsed `Diff`, so an experiment that edits the judge cannot be represented. One strict applier, `diff::apply::apply`, serves the guest, `adopt`, and the report: every context and deleted line must equal the base line at the hunk's stated position byte for byte, newline presence included, with no offset search, no fuzz, and no create or delete branch; a hunk may not append after a line that lacks its newline, and a new side that ends without one must also end the base. `diff::compose::compose` applies the parent's cumulative diff and then the proposal, and `diff_trees` re-derives one canonical diff from file contents, so equal candidate trees get equal IDs and a proposal that restores the base is `NoChange`.

**Metrics.** `Metric` wraps an `f64` that must be finite, and implements `Ord` with `total_cmp`. A candidate can influence the numbers a judge emits; a NaN must never reach policy code. `serde_json` is built with `float_roundtrip`, so a ledger metric keeps its exact bits through a JSON round trip. The scorecard wire form is a versioned array, which rejects a duplicate metric name, and each metric can be marked held-out (measured on data the proposer never sees).

**Outcome.** Failure is a value:

```rust
pub enum Outcome { Scored(Scorecard), Rejected(Reason), Crashed { stage: Stage }, TimedOut { stage: Stage } }
pub enum Stage { Build, Run, Judge }
```

Every `Reason` payload is a validated name or path (`MetricName`, `RelPath`) or a number, never free text.

**Policy.** `select_greedy` is a pure function of recorded results. Repeated trials of one experiment reduce to the lower median per metric, never a mean (03-CONTEXT.md D-11); a candidate with any unscored trial is invalid (`TrialRule::AllScored`), while the incumbent and the baseline reduce over their scored trials and the report shows how many were skipped (`TrialRule::ScoredOnly`), so one failed calibration trial cannot block a run. It applies `min_improvement` from the manifest by multiply-and-compare (the incumbent plus or minus its threshold, never a division) and breaks ties on `ExperimentId`, so the result is identical under any permutation of the input. `conclude` turns one generation into a `Decision`, re-checking every recorded scorecard against the judge spec first (each gate exactly 1, every objective present, held-out flags as declared). `noise_check` compares the objective's threshold with the baseline's spread and only advises (D-12); `route` picks the proposer tier (D-13); `circuit_breaker` stops a run once `max_empty_generations` consecutive generations end with no valid candidate, or once the loop's streak of infrastructure failures reaches `max_infra_failures` (D-14); and `spend_ok` prices metered usage from `prices.toml` and checks it and the token total against the manifest caps. All of them live in core. `select_pareto` is deferred with POL-01.

**Ledger.** Run state is never stored. It is rebuilt by folding events. The ledger is append-only JSON Lines of `Line { seq, key, record }`, and each record is `{"v":"1","<snake_case_variant>":{...}}`. The event set is frozen (CORE-08):

```rust
#[serde(tag = "v")]
pub enum Record { #[serde(rename = "1")] V1(Event) }

pub enum Event {
    RunStarted { run: RunId, manifest: ManifestId, base: TreeId, baseline: ExperimentId, started_at: u64 },
    ManifestChanged { manifest: ManifestId, from_generation: u32 },
    Proposed { exp: ExperimentId, parent: ExperimentId, generation: u32, tier: Tier, diff: BlobId, rationale: BlobId, origin: Origin },
    Judged { exp: ExperimentId, judge: JudgeVersion, trial: u32, outcome: Outcome, backend: BackendInfo },
    Selected { generation: u32, decision: Decision },
    Spent { generation: u32, source: SpendSource, usage: Usage },
    Steered { generation: u32, ordinal: u32, note: BlobId },
    StopRequested { generation: u32, reason: StopReason },
    ReviewRequested { request: ReviewId, kind: ReviewKind },
    Reviewed { request: ReviewId, answer: ReviewAnswer },
    ReportAcknowledged { exp: ExperimentId, report: ReportHash },
}
```

`Origin` is `Sweep` or `Model { model, prompt }`. `SpendSource` is `ModelCall { call, model }`, `Candidate { exp, trial }`, or `Judge { exp, trial }`, so `Spent` carries usage only, never dollars. `StopReason` and the review enums are closed, steering notes are blobs, and no shell or model text sits inline in a line. `Decision` records `Improved { winner }`, `Kept { incumbent }`, or `NoValidCandidates` with the proposed and dropped counts, and `BackendInfo` records the backend, architecture, isolation level, active confinement layers, and image digests. Every event's content-derived `EventKey` is documented in `crates/minos-core/src/ledger.rs`; the store verifies each line's key on read and treats an append whose key already exists as a no-op. `Judged` is keyed by experiment, judge version, and trial, so re-scoring appends records and never rewrites history. Generation 0 is baseline calibration; proposals start at generation 1. Adding an enum variant or an `Option` field keeps every existing line parseable; a required new field, a rename, or a type change needs `"v":"2"`. No ledger type holds a 128-bit integer or a dollar amount.

**Wire formats are an API.** These types persist in checkpoints, S3, and the ledger. Golden-file tests guard serialization, golden vectors guard hashing, and the `v` tag is how a format changes. Golden wire files are insta snapshots paired with a SHA-256 pin of the exact bytes, because insta normalizes trailing whitespace and line endings when it compares, and `.config/insta.yaml` sets `update: no` so the gate never writes or accepts a snapshot.

## minos-store

`Ledger` (`append`, `read`) and `Blobs` (`put`, `get` with a size cap, `contains`) are synchronous traits with `&self` methods. `MemoryStore` implements them for every test above the store (LOOP-09), `FileStore` implements them for v1, and the AWS milestone's DynamoDB and S3 stores will implement the same traits. One contract test file, `crates/minos-store/tests/contract.rs`, runs every case against each implementation.

The file store lives in `<project>/.minos/` (`STORE_DIR`). The root is created with mode 0700, a symlinked root is refused, and `.minos` is on the source snapshot's hard denylist. `runs/<run-hex>/ledger.jsonl` holds one ledger per run, and `blobs/sha256/<2 hex>/<62 hex>` holds the blobs, shared by every run, judge data included (04-CONTEXT D-10). Every store directory and file is checked with `symlink_metadata` before use, and an opened file's device and inode must match that check, so a symlink anywhere in the store is `StoreError::Unsafe`; new files are created with `create_new`.

Every append takes an exclusive `std::fs::File::lock` on `runs/<run-hex>/ledger.lock`, rebuilds the key set from the file under the lock, writes one line with its newline, and calls `sync_data` before returning (on macOS std implements that with `F_FULLFSYNC`), and it fsyncs any directory that gained an entry (D-11, D-12). Reads take a shared lock and never write. An append whose key already exists writes nothing and returns `Duplicate` (equal record) or `Conflict` (different record; the first write wins), and `Appended` is `#[must_use]`, so a caller cannot drop a `Conflict` unnoticed. A torn tail is tolerated, and it has one shape: bytes after the last newline. An append writes its line and newline in one `write_all`, and serde_json never emits a raw newline inside a line, so a torn write is always an unterminated prefix (review WR-01). `read` reports it as a count, never its bytes, and the next append truncates only it, with `set_len` under the exclusive lock; that is the only mutation of existing bytes, and the ledger is never opened with truncation. Everything else refuses the ledger with `StoreError::Corrupt` naming the run, the 1-based line, and the kind: invalid JSON or valid JSON that is not a `Line`, the final newline-terminated line included (`BadJson`), a key mismatch, a seq gap, a line over 1 MiB (`LineTooLong`), and any `v` other than `"1"` (`UnknownVersion`), wherever the line sits (D-13). A ledger over 64 MiB is `LedgerTooLarge`, and a record whose serialized line would exceed 1 MiB is refused on append with `StoreError::LineTooLong`, because the ledger is not corrupt. `StoreError::Io` is the only retryable variant; no store error is an `Outcome`, and none carries file bytes.

Blobs are written temp-then-rename in their shard directory (the temporary file is synced, renamed, and the shard fsynced), so a reader sees the whole blob or none of it. A get checks the stored size against the caller's cap from the metadata before opening, reads at most one byte more than the cap, and rehashes what it read, so a flipped byte is `BlobTampered` and never reaches a caller. A put always writes, so storing the bytes again heals a tampered blob. `Blobs::put_reader` streams a reader into the store under a cap, hashing with core's `BlobHasher` as it writes, so it names a blob exactly as `put` of the same bytes would (05-CONTEXT D-09): its temporary file sits in `blobs/staging` until the digest is known, is synced, and is renamed into its shard, and both directories are fsynced; a stream over its cap is `PutTooLarge` and a reader error is `Io`, and either stores nothing. `FileStore::verified_path` returns a blob's canonical absolute path after the symlink and inode checks, without rehashing (D-14: `env build` verified the hash when it wrote the blob), and is how a root image reaches the VM. Phase 7's whole-run `driver.lock` (LOOP-06) is a separate file on top of the per-append `ledger.lock`.

## minos-sandbox

Roles are types. Policy is enforced when a spec is built, and backends only ever receive a sealed plan:

```rust
pub struct SandboxSpec<R: Role> { /* image, mounts, budget, PhantomData<R> */ }

impl SandboxSpec<Judge> {
    pub fn secrets(self, s: JudgeData) -> Self { /* read-only mount */ }
}

// No role has a method that attaches a network. Only Judge can mount judge data.
// A candidate with network access or judge data must not compile.

impl<R: Role> SandboxSpec<R> {
    pub fn new(job: &JobRequest, root: ImageDigest, input: InputDisk, machine: Machine, arch: Arch) -> Result<Self, SpecError>;
    pub fn seal(self) -> LaunchPlan<R> { /* the only constructor of LaunchPlan */ }
}
```

A spec carries IDs and digests only: the root image digest, the input disk blob, the judge data blob for a Judge, the `JobBudget`, the `Machine`, the `Arch`, the launch key, and the hard maximum duration, which is build + run + 30 s for a Candidate and judge + 30 s for a Judge, refused above one hour. A `SpecError` such as `DurationOverCeiling` is a configuration error, neither an `Outcome` nor a retried infrastructure failure: each stage budget may be up to an hour, so an over-ceiling sum fails every experiment of the run alike. The runner (Phase 7) therefore builds the baseline's Candidate and Judge specs before generation 0 and refuses to start the run, and `minos check` (Phase 8, CLI-05) reports it. `LaunchKey` is `IdHasher::new("minos/launch/v1").digest(exp).u64(role tag).digest(judge).u64(trial)`, with role tags Candidate 1, Judge 2, and 3 reserved for Agent (04-CONTEXT D-15). `preflight(host: &Capabilities, plan: LaunchPlan<R>) -> Result<Cleared<R>, Vec<Refusal>>` is the only constructor of `Cleared<R>`, which owns its plan.

Sandbox output is wrapped so it cannot be used unvalidated:

```rust
/// No Deref, Display, or Debug. The only exits are validating parsers.
pub struct Untrusted<T>(T);

impl Untrusted<GuestOutput<Judge>> { // judge output only (D-14)
    pub fn into_scorecard(self, limits: &Limits) -> Result<Scorecard, ScorecardFault>;
}

impl<R: Role> Untrusted<GuestOutput<R>> {
    pub fn into_diff_texts(self, limits: &Limits) -> Result<Vec<String>, OutputFault>;
    pub fn take(&mut self, path: &RelPath) -> Option<Untrusted<OutputFile<R>>>; // a projection, not an exit
}

impl<R: Role> Untrusted<OutputFile<R>> {
    pub fn into_log_excerpt(self, max: usize) -> LogExcerpt; // truncated, control chars stripped
}
```

`Backend` has two methods, `capabilities()` and `launch(ok: Cleared<R>)`, and a generic associated type `Running<R>`; `RunningVm::wait(self) -> Result<Finished<R>, InfraError>`. `Finished<R>` holds the host-observed `Ended` (`Exited`, `Crashed`, `TimedOut`, from the K7 classification), the launch key, and the hostile stream, which leaves only through `into_output` (the raw-mode tar reader: ustar regular files only, `RelPath` names, 1024 entries, 8 MiB per entry, 16 MiB total, two zero end blocks and nothing after). `Finished::new` consumes a `Cleared<R>`, so output of a role exists only for a cleared launch of that role, and one clearance yields at most one `Finished` (refinement of 04-CONTEXT D-17; review WR-03). The runner is generic over `B: Backend`. Each binary picks its backend concretely, and a binary that can choose among several at runtime defines its own enum (for example `LocalBackend { Krun, Chv }`). Never use trait objects, and never put a backend enum in `minos-sandbox`. Dropping a `Running` handle destroys the VM. The trait takes no store: a backend is constructed with a `BlobSource`, so `KrunBackend::new(paths, blobs)` takes the CLI's `FileStore`-backed source and fetches the plan's blobs under caps and the verified root image path through it (05-CONTEXT D-13); `deny.toml`'s layering is unchanged, and `minos-backend-krun` still never links `minos-store`. `KrunBackend::new` sweeps the job root and probes the host once, `capabilities()` returns those cached results, and `launch` refuses a host whose probes failed or that lacks Seatbelt, whatever capabilities the plan was cleared against.

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

Optional keys and their defaults: `[policy] stall_step` (3), `[budget] judge` (2m), `max_tokens` (5000000), `max_empty_generations` (3), and `max_infra_failures` (3), and `[environment] memory_mib` (2048) and `scratch_mib` (1024), bounded by `limits.rs` (memory 128 to 16384 MiB, scratch 64 to 15872 MiB) and by the memory budget (05-CONTEXT.md D-05): the 256 MiB guest reserve plus the tmpfs scratch cap plus at least 256 MiB for the candidate must fit `memory_mib`, and what remains is the candidate's allowance; sweep knobs are `[[param]]` tables with `name`, `file` (a writable file), and `anchor`, then either `min`, `max`, and `step` or `choices`. Sweep params are integer ranges (`min`, `max`, `step`) or string `choices` only, with no float or log ranges, so the sweep needs no float math. `max_infra_failures` counts consecutive infrastructure failures, which are retried rather than recorded as outcomes, so the loop supplies that streak from memory to `circuit_breaker` and a restart resets it.

### Proposer ladder

The loop is fixed code. A model appears only in the propose step, and only where a tier calls for it.

| Tier | What proposes | Needs an agent sandbox |
|---|---|---|
| `sweep` | Seeded sampling over declared params; no model; pure code in core | No |
| `single_shot` | One model call with no tools, returning several diffs | No |
| `planned` | Fixed two-call pipeline: list ideas, drop those already tried, implement the rest | No |
| `agentic` | A tool loop against an agent VM | Yes |

The sweep tier draws each declared `[[param]]` value from a ChaCha8 stream seeded by run and generation, with Minos's own unbiased range mapping (never `rand`'s distributions), and renders a sample into a canonical diff by rewriting the remainder of each param's anchored line.

Each project declares its ladder. `ProposerSpec::eligible` checks prerequisites (sweep needs params, single-shot needs the surface to fit the prompt budget), and `minos check` prints the route table with reasons. Routing is a pure function of manifest and ledger: move one tier up the eligible ladder after each `STALL_STEP` generations without improvement.

Phase comes before tier. A project with no judge version starts in `DesignJudge`: the proposer drafts metrics and the run waits for the user's approval. The agent may propose judge changes, params, and dependencies. It never approves them. Each goes to the inbox.

Prompt assembly is deterministic, with the stable prefix (`program.md`, surface files) first so it can be cached. Invalid proposals are dropped, not repaired. Token and turn caps are enforced by the loop, outside the model.

### Judge

The judge is a command plus private data that runs in its own sandbox and writes a small scorecard. The harness launches it and records the result. Rules:

- The candidate reports nothing about itself. The harness measures time and enforces budgets from outside.
- Judge data is never mounted for an agent or a candidate, and is excluded from the source snapshot. The manifest requires every path the judge command names to lie under a `[source] exclude` entry and keeps it out of `readable`, so the judge program is in neither the snapshot nor the prompt.
- When scoring requires executing candidate code (for example, loading a trained model), the judge and candidate run in separate sandboxes and talk over a mediated channel. They never share a process.
- Noisy metrics are measured repeatedly. `minos check` runs the baseline several times to establish the noise floor, which informs `min_improvement`.
- Every `Judged` event records backend and architecture. A run with a speed objective is pinned to one backend class.
- A gate passes only when its metric is exactly 1. The judge writes its scorecard as `{"v":1,"metrics":[{"name":"<metric>","value":<number>,"held_out":<bool>}]}`, an array so a duplicate metric name is a rejection rather than a silent overwrite.

## Environments and source

**Environment.** The user's Dockerfile feeds every backend. Dependencies are baked in during a build phase that has registry access and no secrets; the experiment phase is offline. Base images are pinned by digest and lockfiles are required. The trusted packer writes `minos-guest` into the root image as `/init` after every file from the user's image, so the Dockerfile cannot replace it, and the VM never runs a Docker entrypoint (02-CONTEXT.md D-18). `minos.lock` records, per backend and architecture, the root image digest (its blob ID in `.minos/blobs`, 05-CONTEXT.md D-09), the Dockerfile SHA-256 (of the bytes in the snapshot the builder received), every `FROM` digest, the SHA-256 of the `minos-guest` written as `/init`, and the builder's name and version (D-11); the lock is TOML with a `v` tag, `deny_unknown_fields`, and `try_from`, and is rewritten through a synced temporary file in `.minos/`. `minos env build` refuses an image digest that differs from the lock unless `--update` is passed, printing both digests (D-10); when the image digest is unchanged but another recorded input changed (a Dockerfile comment, a builder upgrade), it rewrites the entry without `--update`, so the lock always describes the inputs on disk. A run refuses to start when the Dockerfile or the guest binary on disk no longer matches the lock (Phase 7 and 8 call `Lock::check`). D-10's lineage half, the lock's digest feeding the manifest's image reference and hence `ManifestId`, is deferred to Phase 7, which wires `minos run` (05-CONTEXT.md Deferred Ideas, recorded by 05-16); meanwhile every `Judged` event records the root image in `BackendInfo`. `env build` refuses a manifest whose arch is not arm64, because the krun backend boots AArch64 guests only; Lambda is arm64 too, and a GPU box, usually x86_64, will get its own entry.

**Root images.** The requirement is any OCI builder that can export a rootfs tar (02-CONTEXT.md D-14). The checked tool chain is the Docker CLI with BuildKit, after which Minos's own pure-Rust packer (`minos-backend-krun`, gzip SquashFS) adds `/init` and writes the raw read-only root image (02-SPIKE-EVIDENCE.md RI-03, S06-BOOT). Docker Desktop is one way to get that builder, not a requirement; lima, podman, and Apple's container tool are untested alternatives. Base images are pinned by digest. `minos env build` runs the argv `docker buildx build --platform linux/arm64 --provenance=false --sbom=false --output type=tar,dest=- -f <dockerfile> -` with the environment cleared to `HOME` and a `PATH` of the docker directory, its canonical directory, and the system directories, in its own process group under a 30-minute watchdog; the A2 fallback to `docker build` was not needed, because buildx takes a tar context on stdin with `-f` relative to its root (05-EVIDENCE.md 05-18 env build). The build context is the source snapshot tarball on stdin, the same deterministic, secret-scanned tarball `minos run` packs, with judge paths and the hard denylist excluded; never a host directory. Provenance and SBOM attestations are disabled so the tar export holds only the rootfs. The builder's tar is hostile input to the packer, and its stderr is shown only through `SafeText`. The Dockerfile's `USER` is ignored: every file in the image is owned by root and the build and run stages run as uid 64000, so toolchains belong in world-readable prefixes such as `/usr/local`, with `CARGO_HOME` and `RUSTUP_HOME` there, never under `/root` (05-RESEARCH Pitfall 9). The reproducibility claim, decided (05-CONTEXT.md D-12): the packed SquashFS image reproduces whenever the builder's rootfs contents reproduce, because the packer zeroes every mtime, uid, and gid; the builder's tar may differ and is not claimed to reproduce, because not every OCI builder honors `SOURCE_DATE_EPOCH`. Two builds of one Dockerfile gave tars that differed only in mtimes and root images that matched byte for byte (02-SPIKE-EVIDENCE.md RI-04), and two `minos env build` runs gave equal image digests (05-EVIDENCE.md 05-18 env build). Content a `RUN` step computes from the clock is not reproducible by any packer: the 05-18 fixture's `RUN date > stamp` reproduces only through BuildKit's layer cache, and a `--no-cache` rebuild yields a new image. How the image reaches the store (D-09), stated without overclaiming: the packer builds the whole SquashFS image in memory, bounded by `PackLimits` (the 1 GiB tar cap), and `Blobs::put_reader` then streams those bytes into `.minos/blobs`, hashing as it writes, with no second whole-slice copy; the store's put streams, the packer does not.

An optional `[environment.prepare]` step runs once with allowlisted network access. Its output is frozen as a read-only blob identified by hash.

Lambda snapshots capture memory as well as disk. `minos-guest` must not create anything per-run at startup; that belongs in the run hook.

**Source.** `minos run` packs the working tree into a deterministic tarball (sorted paths, zeroed metadata, regular files only) whose hash is the `TreeId`. Four exclusion sources apply as a union: `.gitignore` files inside the project (no repository needed), `.minosignore` files, the manifest's `[source] exclude`, and the hard denylist. A parent directory's `.gitignore`, `.ignore` files, and git's global and `info/exclude` files do not apply. A `.minosignore` line starting with `!` fails the pack, because a negation there would re-include a gitignored file; an ignore file the walker cannot read or parse also fails it. The hard denylist (`.git/`, `.minos/`, `.claude/`, `.aws/`, `.ssh/`, `.env*`, `*.pem`, `id_*`, `minos.lock`, and every path the judge argv names) matches names ASCII case-insensitively, cannot be overridden, and every drop is listed in the pack report. The tarball holds regular files only: a symlink, hardlink, FIFO, socket, device, a name outside the `RelPath` grammar, a path that does not fit a ustar header (255 bytes, final component 100), or a file that changes while it is read fails the pack with one error listing every offender; nothing is skipped or followed. Caps are fixed in code: 10,000 files, 16 MiB per file, 64 MiB total; a tree over a cap fails naming the largest files. Every `[surface]` path and the program file must be in the snapshot. The secret scan runs on exactly the bytes that will be packed, binary files included, with nine anchored patterns and no override; a hit names the file, the pattern, and the line, never the bytes. `--allow-dirty` skips only the clean check. Headers are hand-built ustar with mtime, uid, and gid 0, empty owner names, and mode 0644 or 0755. Comparing packed files with pushed judge data by digest waits for Phase 7, when `minos judge push` records per-file digests. It expects a clean commit unless `--allow-dirty` is passed. The clean check is the one place Minos runs git: `minos-source` execs the user's own `git` read-only on the user's own working tree, as argv arrays with the environment cleared to `PATH`, `LC_ALL=C`, `GIT_CONFIG_NOSYSTEM=1`, and `GIT_CONFIG_GLOBAL=/dev/null`, with `core.fsmonitor` off and `core.hooksPath=/dev/null`. It runs `git status --porcelain` and `git ls-files` and requires every packed path to be tracked, so a file ignored only by `.git/info/exclude` or a global excludes file cannot slip into a snapshot that git calls clean. Not a repository, no `git` on that `PATH`, a non-zero exit, or a timeout all count as dirty. This does not break the no-git rule: that rule exists because git on a candidate-controlled tree brings hooks and config into the trusted path, and no candidate ever writes the user's working tree (`adopt` writes a patch file only). The moment a tree could be candidate-controlled, the rule applies in full. Git never contributes a byte to the tarball (04-CONTEXT D-02).

Build output is never shared between experiments. Pre-compiled dependencies sit read-only in the image; each experiment's build writes to its own scratch space, a fresh size-capped tmpfs in guest memory (05-CONTEXT D-05).

## Local backends

Local is where a breakout costs the most, because it lands next to the user's keys and home network. Local must be at least as strict as Lambda.

- Hardware virtualization is the default. Process-only isolation (Landlock plus seccomp) is a labelled dev mode, and runs made with it are marked `WeakIsolation` in the ledger and the report.
- macOS has no seccomp and no cgroups. On macOS the hypervisor process is confined by a Seatbelt deny-default profile, `crates/minos-backend-krun/src/minos-vmm.sb`, which lives beside the code that launches the helper. `minos-backend-krun`'s `seatbelt` module embeds it with `include_str!` and passes the text with `sandbox-exec -p`, and a test pins its SHA-256, so there is no profile file on disk to tamper with at run time; every job path arrives as an absolute, canonical `-D` parameter, and a test proves the profile's parameters equal the ones the argv sets. The profile as shipped allows reads of the system libraries and of the helper's own binary, config, and libkrunfw; reads of the job's two read-only disk images, the root image and the input disk; writes of the console literal `/dev/fd/1`, the helper's stdout pipe (05-EVIDENCE.md Mechanism selections, `console: pipe`); metadata of the ancestors of libkrunfw, its link, the config, and the root image, which sits at its store path under `.minos/blobs` (05-CONTEXT D-14, 05-EVIDENCE.md W0-ROOT-DENY); a short list of sysctls; and exactly one network operation, the AF_UNIX connect to the vsock socket. There is no scratch image and no console file. `hv_vm_create` needs no hypervisor-specific rule (02-SPIKE-EVIDENCE.md SPIKE-05 answer, S05-BOOT, S05-DENY-NET, S05-DENY-FILE). `sandbox-exec` is deprecated by Apple but works on macOS 27.0. Seatbelt is mandatory in v1 (05-CONTEXT D-03): `minos-backend-krun`'s `host` module probes once, at backend start and in `minos host doctor`, and a host without `/usr/bin/sandbox-exec`, or whose profile no longer denies a loopback TCP connect and an unlisted file read, gets a preflight Refusal, so no run is recorded without Seatbelt. The denial probes run offline against the production profile in a private directory: `cat` under the profile must read a listed file back and fail on an unlisted one, and an unsandboxed `nc -z` must reach a probe-owned 127.0.0.1 listener before the same `nc` under the profile must fail with the listener accepting nothing; a failed control is inconclusive and fails closed as `SeatbeltNotEnforcing`. The dedicated unprivileged `_minos` user is deferred (05-CONTEXT D-01): v1 runs record it as missing, so every v1 run shows reduced hardening in the report's provenance; S05-RECIPE stays the starting point for the phase that adds it, and `minos host setup` signs the helper and verifies both entitlement keys with `codesign --display --entitlements - --xml` (D-02). Every run records which layers were active in `BackendInfo` and never falls back silently (KRUN-07). The recorded layers are Seatbelt and Rlimits: Seatbelt only when `sandbox-exec` is present and both denial probes pass, and Rlimits only when the helper (present, signed with both keys) and the guest (a static aarch64 ELF) pass their probes. The `Rlimits` confinement layer covers both halves D-01 names: the helper's own limits (no core files, bounded open files and file size), set before it reads its config, and the guest's per-stage limits on the candidate (address space, file size, processes, CPU, open files, no core files), set by the trampoline before exec; each half fails closed (the helper exits 66, the trampoline never execs an unlimited stage), so the layer is recorded only for launches where both were applied (05-CONTEXT D-01). The helper's `RLIMIT_FSIZE` is 0, so its stdout and stderr must be pipes: a regular-file stderr kills it with `SIGXFSZ` on the first write (05-EVIDENCE.md 05-07 helper rlimits).
- Files go in as two read-only raw disk images, the shared root and the per-job input disk. Scratch is a size-capped tmpfs in guest memory sized by `[environment] scratch_mib`, so scratch bytes never exist on the host, scratch is deleted unread by construction, and every VM starts empty (05-CONTEXT D-05); `minos-vmm`'s launch config v2 has no scratch disk. Never share a host directory. The input disk is a `MINOSJB2` header carrying the per-launch token, then a section table giving each section's kind, offset, length, and SHA-256, then the sections (the guest job, the base tarball, and the diff for a candidate; the guest job, the judge data, and the candidate's output for a judge), each on a 512-byte boundary; `minos_core::guest` is the one codec, which the backend writes and PID 1 reads (05-04). The backend fetches each part through its `BlobSource` under the part's section cap, checks the guest job against the plan (role, launch key, base, diff, stage budgets, memory), and writes the disk into the 0700 job directory as a 0600 file renamed to `input.img` without replacing anything; it is never stored as a blob and is deleted with the job directory (05-CONTEXT D-13, D-15). The host writes the tarball and diff to the input disk without unpacking or applying them. PID 1 reads its role's sections whole into memory, so each role's section caps sum to at most half of the 256 MiB guest reserve, 128 MiB (`MAX_GUEST_INPUT_BYTES`), which caps judge data at 96 MiB (`MAX_JUDGE_DATA_BYTES`); const assertions in core enforce the bound, and PID 1 drops each buffer once it is unpacked, so the stages run with the whole reserve (05-04, 05-10).
- The VM boots straight from the raw read-only SquashFS root image on `/dev/vda` (02-CONTEXT.md D-18; 02-SPIKE-EVIDENCE.md S01-ROOTDISK). Stock libkrunfw has `CONFIG_BLK_DEV_INITRD` unset, so there is no initramfs. `/init` is the static musl `minos-guest`, written into the image last by the trusted packer, so a Dockerfile cannot supply its own. `minos-vmm` passes `rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0` through `KernelBuilder::cmdline` and `/init root=/dev/vda` through `KernelBuilder::init_path`, which lands after msb_krun's own `root=/dev/root`; Linux keeps the last `root=`. `dummy.numdummies=0` stops the kernel's built-in dummy driver from creating `dummy0`, by the user's decision (S01-NETIFACES). That rides on msb_krun 0.1.41 formatting the init path unquoted and after the user cmdline. The guest's `/proc/cmdline` holds exactly the recorded tokens (`reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda`, then `--` last) and one host-derived value, `KRUN_BOOT_START_NS=<host wall-clock ns>`, which msb_krun 0.1.41 always writes: `build_kernel_cmdline` in its `src/api/vm.rs` puts it in the `krun_env` segment unconditionally, from `SystemTime::now()` at VM build, and no builder method, cargo feature, or launch option removes it (`MSB_KRUN_KERNEL_CMDLINE` only appends to the prolog, and the helper's environment is cleared anyway); 0.1.41 is the newest release, so no pin bump removes it. The kernel also hands it to PID 1's environment, which PID 1 clears before any stage. `exit_env` allows exactly that key with an all-digit value and matches every other token exactly. This is a recorded exception, by the user's decision at the 05-19 checkpoint on 2026-10-19 (05-RESEARCH Open Question 2), not an endorsement: the user first chose no allowance, then, once the mechanism was proven from the pinned source and a fork declined, chose to allow it as a recorded exception. GUEST-05 is met with this one stated exception. The token reveals the host's clock at boot, which the guest's own clock also reveals. Removing it is an open question (Open questions). The boot test asserts the exact `/proc/cmdline` tokens (S01-CMDLINE), but it is an ignored VM test, so a bump of the `msb_krun` pin must re-run `exit_env` (`crates/minos-backend-krun/tests/exit.rs`, 05-19) by hand (the pin in `Cargo.toml` says so); a unit test in `minos-vmm` mirrors the pinned version's cmdline assembly so the gate catches a change on our side. No VM has a virtio-fs device. The helper accepts only the root-disk boot; the initramfs attempt is negative evidence in 02-SPIKE-EVIDENCE.md S01-INITRAMFS, and launch config v2 has no boot field.
- The packer creates `/dev`, `/proc`, `/sys`, `/tmp`, `/mnt`, `/out`, and `/scratch` in every root image, because nothing can create a directory on a read-only SquashFS root: the guest mounts its scratch tmpfs on `/scratch` and bind-mounts the stage's output directory on `/out` (05-CONTEXT D-05, D-08). `minos env build` refuses a guest binary that is not a static `aarch64` ELF (`rootfs::check_guest_elf`: 64-bit, little-endian, `EM_AARCH64`, no `PT_INTERP`), because the macOS build of `minos-guest` packed as `/init` never boots and the run would be blamed on the candidate (05-RESEARCH Pitfall 8).
- The helper's exit code is not the guest's result. With no virtio-fs device the guest has no way to set it: guest power-off, a guest whose init returns, and a host SIGTERM all end `minos-vmm` with 0 (K7-01, K7-02, K7-05). A returning PID 1 does not panic: libkrunfw patches its exit into an orderly reboot (K7-02). A kernel panic also ends in 0, because of `panic=-1` (S01-INITRAMFS). The guest's status frame over vsock is the only result channel. The host classifies a run from what it observed itself, by the refined K7 rule in `minos-backend-krun`'s `supervise::classify`, a pure, exhaustively tested function (02-SPIKE-EVIDENCE.md K7 table, Classification rule for Phase 5). A host kill decides first: the host deadline (the plan's hard maximum plus 30 s) means `TimedOut`, a console over its cap (read from the console pipe, then a kill of the whole process group) means `Crashed`, never retried, and a chunk fault (a kill when the chunk receiver faults) means `ChunkProtocol`, never retried. Otherwise exit 0 or exit 124 with `minos-vmm: watchdog deadline` and a faulted stream means `ChunkProtocol`; exit 0 with a complete stream means the guest decided, and its status file says what happened; exit 0 with no END chunk means `Crashed`; exit 124 with `minos-vmm: watchdog deadline` means `TimedOut`; and exit 124 without that line, 126 with or without `minos-vmm: watchdog parent-gone`, `minos-vmm`'s pre-takeover codes 64 to 71, a spawn failure, any signal the host did not send, and any other code mean an infrastructure failure. A host `TimedOut` is the backstop: PID 1 enforces each stage's deadline itself, so the runner attributes a host timeout to `Run` for a Candidate and `Judge` for a Judge (05-RESEARCH Pitfall 12). The markers count only as exact whole lines of the helper's stderr together with their exit code, and a stderr line with any control character never equals one. They are written by `minos-vmm`, never by the guest, which reaches only the console pipe; a logged line always starts with `minos-vmm: log `, so it can never equal a marker. The supervisor keeps 128 KiB of stderr, twice the helper's 64 KiB log budget, so the markers after a full log still fit, and drains the rest. It observes the helper's exit with `waitid(WNOWAIT)`, kills whatever is left of the process group while the zombie helper still holds the group id, and only then reaps it. On macOS, `kill(-pgid, 0)` on a group whose only members are unreaped zombies returns `EPERM`, not `ESRCH` (proven in 05-15), so `EPERM` is never read as "gone" without that proof. `wait` deletes the job directory, with its input disk, when the run ends, and dropping the running value kills the group and deletes it too (D-15). Under Seatbelt the helper also exits 0 when its socket connect is denied, so success is always judged by the frame (S05-CTRL-SOCK).
- libkrunfw comes from the Homebrew tap `libkrun/krun` (`brew tap libkrun/krun`, `brew trust libkrun/krun`, `brew install libkrunfw`); Minos needs only libkrunfw from it, not libkrun, krunkit, or krunvm (02-SPIKE-EVIDENCE.md H-01). `minos-vmm` loads it by absolute path and checks its SHA-256 first (K5). Binaries and the kernel sit at fixed locations with no config file (05-CONTEXT D-04): `minos-vmm` and `minos-guest` next to `minos`, or in the cargo target directory of a development checkout, and libkrunfw at `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`, checked against its pinned SHA-256 `59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217` (`host::KRUNFW_SHA256`); `minos host doctor` names every path it checked, and only the ignored VM tests read `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`. libkrunfw is loaded from the Cellar path after its hash check and is not copied into a private directory first: the window between the hash and the `dlopen` is open only to a host writer of `/opt/homebrew/Cellar`, and the threat model's attacker is the guest (05-RESEARCH Open Question 6; a recorded assumption, flagged for the user).
- `minos-vmm` records its parent PID and its start instant as the first statements of `main`, before the config read, preflight, and the VM build. Its watchdog treats a changed parent, or a parent of PID 1 (launchd never legitimately starts it), as gone, so a helper orphaned at any point stops with 126 (02-07 DEFECT-01, K7-07, K7-07b), and it counts `max_duration_ms` from that instant, so the limit bounds the helper's whole lifetime and not only the VM's run time. Every job runs in its own 0700 directory `/private/tmp/minos-<uid>/mns*/`, under a job root that is refused if it is a symlink, another user's, or not mode 0700; the directory holds `vmm.json`, `input.img`, `o.sock`, `owner.lock`, which the supervisor locks exclusively for the job's life, and a `pid` file naming the helper (`minos-backend-krun`'s `jobdir` module; KRUN-05). The watchdog is the primary mechanism; the sweep, at backend start and in `minos host doctor`, is the backstop and also deletes leftover input disks, which can hold judge data. It touches only `mns` directories whose owner lock it can take, kills an orphaned helper's process group only after `ps` shows ppid 1 and exactly `<vmm> <job dir>/vmm.json`, removes the directory once `ps` shows no row, and never treats `EPERM` or any unproved state as gone: the directory is left and reported unverified (05-RESEARCH Pitfall 11).
- The console is the helper's stdout pipe (`/dev/fd/1`, allowed by one Seatbelt literal), read by the supervisor through cap + 1 bytes, so the cap is exact and the console never touches host disk (05-EVIDENCE.md W0-CONSOLE). A console over its cap is a host-observed `Crashed`. Preflight refuses to launch unless fd 1 is a pipe. The Phase 2 file console, polled by the watchdog every 50 ms, overshot a 262144-byte cap by 30000 to 48954 bytes (K7-04); that mechanism is gone.
- `minos-vmm` installs a capped stderr logger for msb_krun's `log` output at `Warn` (DIAG-01): every line is `minos-vmm: log <level>: ` followed by the message with every character outside printable ASCII replaced by `?`, at most 64 KiB of lines in total, then one `minos-vmm: log truncated` line and nothing after. Before Phase 5 these warnings were discarded, including the vsock drops below.
- No network device does not mean no network code in the guest kernel. The libkrunfw kernel has `CONFIG_TSI=y` and `CONFIG_DUMMY=y` (H-02, S01-NETIFACES). Upstream libkrun's networking default is not air-gapped: it turns on TSI, which proxies guest INET sockets through the host, whenever no virtio-net device is configured. Minos drives libkrun through `msb_krun`, which reverses that default (`inet_hijack` off). The guest has no egress only because the VMM never enables inet hijack: `msb_krun`'s `net` feature stays off, and `crates/minos-vmm/clippy.toml` bans the hijack, port-remap, virtio-fs, non-raw-format, guest-environment, and host-console methods (D-PROBE), and the custom console and vsock backends (`ConsoleBuilder::custom`, `custom_with_options`, `VsockBuilder::custom`, `custom_dgram`; 05-EVIDENCE.md 05-07 clippy bans fire). Every VM attaches virtio-rng explicitly with `.rng(true)`: data flows host to guest only, and early `getrandom` in candidate builds needs entropy (05-RESEARCH Open Question 5). `egress_fails_closed` (Phase 2, promoted to `exit_egress` in `crates/minos-backend-krun/tests/exit.rs`, 05-19) guards it: every guest TCP connect fails, IPv4 with `ENETUNREACH` (101) and IPv6 with `EADDRNOTAVAIL` (99), no host canary sees a connection, and the guest sees only `lo` and no virtio-net device (S02-EGRESS). The root-disk arguments pass `dummy.numdummies=0`, so the kernel creates no `dummy0`.
- Output comes back as a tar stream over vsock, parsed in Rust with caps. Never mount a disk the guest wrote. On the pinned `msb_krun_devices` 0.1.41 one vsock connection carries at most 8192 bytes guest to host (8192 is the macOS `net.local.stream.sendspace` default); above that, data is silently lost or the guest stalls, for three defects in that crate and none in Minos: a shutdown race discards bytes queued above the 8 KiB send buffer, a one-descriptor packet parser drops payloads over 32 KiB, and a credit update sent only every 4 MiB deadlocks streams over 256 KiB (02-SPIKE-EVIDENCE.md S03-WITHIN, S03-OVER, K4-THROUGHPUT, DIAG-01). By the user's decision (S03-ANSWER, 2026-10-10: "work within the limits for now, if we need to reassess later fine"), guest output is a sequence of short vsock connections, guest to host only, each carrying one chunk. As built (`minos_core::chunk` is the one codec; `minos-backend-krun`'s `chunks` module is the receiver), a chunk is a 48-byte header (magic `MINOSCK1`, a 32-byte per-launch token, a big-endian `u32` sequence number, a kind byte for DATA or END, a reserved zero byte, and a big-endian `u16` length) followed by at most 4096 payload bytes, so one connection carries at most 4144 bytes. The END chunk, sent last, carries the total payload length, the SHA-256 of the payload, and the DATA chunk count. The per-launch token is 32 bytes from the host's `/dev/urandom`, written only into the header of the input disk, a root-only device that PID 1 reads and then removes (it unlinks the node, or makes it mode 0); the token never enters a blob, a log, the launch config, or an argv. It exists because any guest process, the candidate included, can connect to the output port: only PID 1 can send a chunk the host accepts, so the candidate cannot forge the output stream (05-RESEARCH Pattern 2, Pitfall 1). A wrong token, a gap, a duplicate, an over-cap chunk or stream, an empty connection, or a bad END is `ChunkProtocol` on the candidate's own run, never an infrastructure retry, and the receiver's first fault raises a flag so the supervisor can stop the VM early; a stream with no END is `Crashed`. Out-of-order arrival is absorbed by buffering by sequence number under the total cap. The host waits on the listener with `poll` and gives each connection `MAX_CHUNK_BYTES + 1` bytes and 5 s (`CONNECTION_DEADLINE`); a connection over the cap or still open at its deadline is a fault. Forking the VMM crate was considered and rejected for now; reporting the defects upstream is the user's call; reassess only if chunking proves a bottleneck. One connection measured about 6 MiB/s in DIAG-01's instrumented runs; the chunked protocol carried 16 MiB in about 247 ms, about 65 MiB/s of payload, over 4097 connections with no guest connect retry (05-EVIDENCE.md W0-CHUNKS). The stream also depends on the helper's open-file limit: the pinned crate's vsock reaper keeps each closed connection's proxy, with its host socket, for 5 s (`virtio/vsock/reaper.rs`), so `minos-vmm` sets `RLIMIT_NOFILE` to 8192, above one whole stream of about 4353 connections. At the earlier 1024 the table filled every thousand or so connections, PID 1's connects were reset until the reaper caught up, and a 16 MiB output took 20.5 s with 300 guest connect retries; at 8192 the production path's 16 MiB launch, boot to power-off, takes about 0.6 to 0.9 s with no retry (05-EVIDENCE.md "05-17 helper `RLIMIT_NOFILE` and the vsock reaper").
- GPU passthrough requires an IOMMU and a GPU the host does not use for display. Reset the device between runs.
- Preflight fails closed and is enforced by the type system:

```rust
pub fn preflight<R: Role>(host: &Capabilities, plan: LaunchPlan<R>) -> Result<Cleared<R>, Vec<Refusal>>;
fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>;
```

`minos host doctor` runs the same preflight and explains each refusal.

## Cost

The ledger stores usage, never dollars: vCPU-seconds, GB-seconds, snapshot megabytes read, GPU seconds, tokens in and out, durable operations, and the spot price at launch when spot was used. Dollars are derived from usage and a user-editable price table at `~/.config/minos/prices.toml`.

- Rates are parsed from strings as exact decimals with pico-dollar precision (10^-12 USD; published AWS rates have 10 fractional digits). Totals are integer micro-dollars. No floats.
- Output shows one figure, always measured: `approx. cost: $31.40`. Never print an estimate, and never use the word "exact". One type owns that formatting, so no command prints a bare dollar figure.
- Spend caps compare the manifest limit against metered spend, checked before each generation.
- In v1 `Usage` records `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `vcpu_ms`, and `mem_mib_ms`. Every rate in `prices.toml` is a string, so a numeric rate is a located parse error, and `ApproxCost` is the one dollar formatter.

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

`build_report` is a pure function in core, so the same ledger and blobs always give the same report, and the report can be hashed. It contains the summary against the noise floor, the cumulative diff from baseline to winner, the lineage, provenance (isolation level, judge version, image digests), and flags. Flags direct attention; they are not verdicts: new dependencies, spawned processes, sockets, environment reads, encoded blobs, literals that match visible benchmark inputs, operations the guest denied, held-out metrics diverging from visible ones, and weak isolation anywhere in the lineage. v1 renders the anomalous-jump, held-out-divergence, literal-match, new-dependency, encoded-blob, and hidden-character flags, plus weak isolation, with the weakest isolation and any missing confinement layers shown in provenance; the runtime-behavior flags (spawned processes, sockets, environment reads, denied operations) arrive with RPT-01.

`minos adopt` refuses without the hash of a report the user acknowledged for that exact experiment. It writes a patch file and never applies one. The renderer strips terminal escape sequences from every string that originated in a sandbox or a model.

## Testing

- Property tests (`proptest`) for parsers, `RelPath`, `Diff::parse`, policy permutation-invariance, and the ledger fold.
- Golden vectors for every ID hash and golden files for every persisted wire format.
- Compile-fail tests (`trybuild`) proving a `SandboxSpec<Candidate>` cannot get judge data and that `launch` cannot be called without `Cleared`; that no role has a network method; that `LaunchPlan` and `Cleared` cannot be built by literal and `Cleared` cannot be cloned; that one `Cleared` cannot build two `Finished` values; that `Untrusted` refuses `{:?}`, `{}`, `*`, and `.0`; and that candidate output has no `into_scorecard`.
- `LocalRunner` tests for orchestrator logic, including the per-generation operation budget.
- In-memory `Ledger` and `Blobs` implementations for everything above the store.
- VM tests on the production backend and guest, both ignored and run on a VM host after the build, the signing, and the musl guest build (Commands): the permanent exit tests `exit_egress` (KRUN-03), `exit_env` (GUEST-05, with the one `KRUN_BOOT_START_NS` exception in Local backends), `exit_kill9` (KRUN-05: after SIGKILL of the process that launched a VM, its helper exits on its own within 5 s, no process names the job, and a new backend's sweep removes the job directory), and `exit_forge` (GUEST-04: the candidate cannot open `/dev/vdb` or `/proc/1/environ`, and a stream under a guessed token is `ChunkProtocol`) in `crates/minos-backend-krun/tests/exit.rs` (05-19), and the functional VM tests in `tests/vm.rs` (05-17). Both share `tests/harness/mod.rs`, so every harness item must be used by each target that declares `mod harness;`, or `-D warnings` fails on dead code.
- Provider tests in `crates/minos-propose/tests/` run in the gate against `FakeProvider` and a fake `claude` shell script in a temporary `HOME`, so `cargo test --workspace` makes no model call and needs no login. The live suite `tests/live.rs` is ignored and gated by `MINOS_CLAUDE_LIVE=1`, which only those tests read, and has three cases, each one real model call on the user's own subscription: the production provider end to end (`ClaudeCli::preflight`, then `Proposer<ClaudeCli>`'s single-shot path), run from a process whose working directory is a hostile fixture project; the golden argv with the fixture as the CLI's own working directory; and a positive control without `--safe-mode`, `--strict-mcp-config`, and the empty `--mcp-config` that proves the fixture's hooks fire. The fixture holds `SessionStart` and `UserPromptSubmit` command hooks and a `.mcp.json` server, each writing its own canary file, and a CLAUDE.md demanding a marker word in every rationale. The second and third cases carry the PROP-03 proof; the first is not cwd-isolation evidence, because the provider always runs the CLI in its own fresh directory (06-CONTEXT D-17). One non-ignored test in the same file checks the fixture and the control's argv without running anything.

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
MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1   # live provider tests: real model calls on your own subscription; run by hand on a logged-in machine
cargo lambda build --release --arm64     # AWS milestone only; not used in v1
```

Signing (02-CONTEXT.md D-11): every `cargo build` relinks `minos-vmm` and drops its entitlement, so the sequence is always build, then sign, then run. `cargo test --workspace` can relink it too, so re-sign before a VM run. There is no xtask, script, `build.rs`, or cargo alias; `minos host setup` runs the same argv from Rust (05-14), with the entitlements path pointing at a private 0600 copy of the committed plist that is byte-identical to it, then verifies both keys with `codesign --display --entitlements - --xml`. `minos host doctor` runs every launch probe and prints each check, its fix, and every path it checked; it exits 0 only when nothing is refused. Both find `minos-vmm` next to `minos`, and `minos-guest` next to it only when it is a static aarch64 ELF, else at `<target>/aarch64-unknown-linux-musl/release/minos-guest` (05-CONTEXT D-04). The hardened runtime is on, and the entitlement set is `com.apple.security.hypervisor` plus `com.apple.security.cs.disable-library-validation`, because the hardened runtime's library validation refuses the Homebrew libkrunfw dylib ("mapping process and mapped file (non-platform) have different Team IDs"); `minos-vmm`'s preflight SHA-256 check of libkrunfw, before the `dlopen`, is the integrity check that remains (02-SPIKE-EVIDENCE.md K5). Only `minos-vmm` is signed; the `minos` CLI has no entitlement. A helper without the entitlement cannot create a VM: `hv_vm_create` fails and `minos-vmm` exits 70 before the VM starts (S04-UNSIGNED). The VM tests are ignored integration tests in `crates/minos-backend-krun/tests/`, `vm.rs` (05-17) and `exit.rs` (05-19), run with `cargo test -p minos-backend-krun --test <vm|exit> -- --ignored --nocapture --test-threads=1` after the build, the signing, and the musl guest build, with `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` set. `--test exit` also needs `MINOS_EXIT_CANARY`, any random string of at least 16 characters (for example `MINOS_EXIT_CANARY=$(uuidgen)`), and reads the optional `MINOS_EXIT_HOST_IP`, a host LAN address that becomes one more egress target; `exit_kill9` sets `MINOS_EXIT_KILL9_CHILD=1` only for the launcher process it spawns and kills. Those variables are read only by ignored tests, never by production code (05-CONTEXT D-04). `MINOS_CLAUDE_LIVE` is likewise read only by ignored tests, never by production code, as `MINOS_VMM` and its siblings are; without it equal to `1` each live provider test fails before running anything (06-CONTEXT D-17). The Phase 2 spike tests and the Phase 5 Wave 0 tests were promoted to these and retired (05-21); their evidence stays in 02-SPIKE-EVIDENCE.md and 05-EVIDENCE.md.

The sixth command runs from Phase 2 onward (02-CONTEXT.md D-03). The guest is always Linux, because libkrunfw ships a Linux kernel, so `minos-guest` needs a Linux target even though Minos runs on macOS. `/init` is written into whatever root image the user's Dockerfile produces, so it must be a fully static binary that loads no libc from that image: hence `aarch64-unknown-linux-musl`, listed in `rust-toolchain.toml` and linked with `rust-lld`. `.cargo/config.toml` gives that target a `linker` key and nothing else. A flags key there would silently replace the `[build]` forbid backstop for the guest (01-GATE-EVIDENCE.md G-02-CLOSED). The host gate still builds `minos-guest` for `aarch64-apple-darwin`, so keep everything the host can compile (the job and frame codecs, the tar and `minos-core` paths) out of `#[cfg(target_os = "linux")]`; Linux-only code lives in `crates/minos-guest/src/linux.rs`. `cargo test --workspace` cannot run that Linux code on macOS; it waits for a Linux runner, and the ignored VM tests exercise it meanwhile.

Every phase verification also runs the C-02 check, `grep -rnE '#!?\[allow\(' crates --include='*.rs'`, which must print nothing. An inner `#![allow(..., reason = "...")]` on any lint that is not `forbid` passes `cargo clippy` (01-GATE-EVIDENCE.md R-03), so for those lints this grep, not the gate, is what enforces the no-`allow` rule.

Run `cargo clean` at the end of every phase, after verification passes and before the phase is marked complete. No exceptions.

The durable SDK requires Rust 1.94.1 or newer and edition 2024.

## Build order

Milestone v1 is local-first, on macOS arm64 (02-CONTEXT.md D-07; `.planning/ROADMAP.md`):

1. Workspace and gates: the plain cargo gate commands and Cargo-native policy.
2. Platform spikes (libkrun boot, egress, vsock, signing, Seatbelt, root images, the `claude -p` contract), recorded in `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`. In parallel: `minos-core` with its tests.
3. `minos-store` (file and memory), `minos-sandbox`, and the deterministic source tarball (`minos-source`).
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

- Deferred (D-01): how the helper starts as the dedicated `_minos` user: a sudoers rule pinned to the helper path, or a LaunchDaemon (S05-RECIPE).
- Whether `--permission-prompts none` and `--permission-mode dontAsk` behave differently when a permission request occurs; no call made one with tools off (SPIKE-07 contract, C-08). Moot for v1 (06-RESEARCH Open Question 6): with `--tools ""` no tool can prompt, and `--no-session-persistence` is in the argv; no session directory appeared in 06-RESEARCH's call.
- Whether `claude` writes a session directory without `--no-session-persistence`; there was no control call (SPIKE-07 contract, C-14). Moot for v1 (06-RESEARCH Open Question 6): with `--tools ""` no tool can prompt, and `--no-session-persistence` is in the argv; no session directory appeared in 06-RESEARCH's call.
- Whether the Seatbelt profile's starting allowances can be narrowed: rows 1 to 6 of the SPIKE-05 answer table were not minimized one by one, and only the socket rule was shown to be needed (SPIKE-05 answer).
- Streaming the packer's output straight into `Blobs::put_reader`, so a root image is never whole in memory (05-CONTEXT.md D-09). Today `pack_root_image` returns the whole image, bounded by `PackLimits`' 1 GiB tar cap, and `put_reader` streams it into the store. Owner: the phase that next touches the packer; no packer change was made in Phase 5.
- Judge data larger than 96 MiB (`MAX_JUDGE_DATA_BYTES`) needs a streaming unpack from the input disk in place of PID 1's whole-section read, which keeps every role's input within half the guest reserve. Owner: Phase 7, whose `minos judge push` must refuse judge data over the cap rather than truncate it.
- Launch rate, memory per VM, and the chunked output protocol's cost for concurrent candidates (K8; deferred with NEXT-04). For one VM the protocol's cost is answered: 16 MiB took about 247 ms over 4097 connections with no guest connect retry (05-EVIDENCE.md W0-CHUNKS), and through the production backend a whole 16 MiB launch takes about 0.6 to 0.9 s once the helper's `RLIMIT_NOFILE` covers the vsock reaper's 5 s hold (05-EVIDENCE.md 05-17), so one connection per 4 KiB chunk is not a bottleneck (S03-ANSWER). Several VMs streaming at once on one host are unmeasured.
- Removing `KRUN_BOOT_START_NS` from the guest's `/proc/cmdline`, the one host-derived token `exit_env` allows as a recorded exception (user decision at the 05-19 checkpoint, 2026-10-19; Local backends). The mechanism is proven: msb_krun 0.1.41 `src/api/vm.rs` `build_kernel_cmdline` writes `KRUN_BOOT_START_NS={boot_start_ns}` unconditionally into the `krun_env` segment of `KernelCmdlineConfig`, with `boot_start_ns` read from `SystemTime::now()`; no builder method, cargo feature, or launch option removes it, `MSB_KRUN_KERNEL_CMDLINE` only appends to the prolog, nothing under `crates/` references the token, and 0.1.41 (2026-10-08) is the newest release on crates.io. The removal path the user would accept: vendor only the top-level `msb_krun` crate (13 files, about 7,100 lines, Apache-2.0, no build script) into the repository under `[patch.crates-io]` as a path dependency with a one-line change that drops the token, leaving the `msb_krun_*` sub-crates on crates.io. No git fork of the whole project. Owner: the phase that next bumps the `msb_krun` pin, or earlier at the user's call.
- Whether the hard denylist's `id_*` rule should narrow to the SSH key names (`id_rsa*`, `id_dsa*`, `id_ecdsa*`, `id_ed25519*`, and their `_sk` forms), since it also drops ordinary files such as `src/id_map.rs`, and whether `.netrc`, `.npmrc`, `.pypirc`, `*.key`, and `*.p12` belong on it. D-07 is implemented as written and every drop is reported in the pack report (04-RESEARCH Open Question 4; the user decides).
- Whether `RLIMIT_AS` at the default candidate allowance (768 MiB with the 2048 and 1024 defaults; the guest sets it in 05-08) breaks a real toolchain build, since Go, the JVM, V8, and sanitizers reserve large virtual ranges (05-RESEARCH Pitfall 10, Open Question 3). The Phase 5 VM tests run only small probe programs, so nothing in this phase measures it. Owner: Phase 8's sample project, which measures one real build against the defaults and adjusts them or the sample's own `[environment]` values.
- A future `claude` release may make `--bare` the default for `-p`, and bare mode never reads the subscription login, so every call would end as `ProviderAuth`. The ignored live tests catch it (06-RESEARCH State of the Art).
- `batch` full-file proposals over large writable files can exceed the CLI's default output cap (about 32,000 tokens), which ends each call as an infrastructure failure. Owner: Phase 8's `minos check`, or a smaller `batch` (06-RESEARCH Pitfall 5).
- Cross-generation caching of the stable prefix needs the deferred `--system-prompt-file` delivery (06-CONTEXT Deferred Ideas, 06-RESEARCH Pitfall 7).

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
