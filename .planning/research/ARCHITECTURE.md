# Architecture Research

**Domain:** Secure autonomous experiment-loop lab (local-first v1 of Minos, Rust, macOS arm64, libkrun)
**Researched:** 2026-10-09
**Confidence:** MEDIUM overall. Crate boundaries and data flow are derived from the design of record and are HIGH. The libkrun facts were read directly from the official source at tag `v1.19.6`, and the Claude CLI facts from the official docs. Both are primary sources, but the research seam rates the fetch provider LOW, so every libkrun and CLI claim below is listed as a spike item before code depends on it.

---

## Standard Architecture

### The one-sentence version

v1 has a functional core and a replaceable imperative shell. Every decision (routing, admission, scoring, selection, report) is a pure function in `minos-core`. Every effect lives in thin drivers that sit behind Cargo-enforced boundaries: model calls, VM launches, and ledger and blob I/O. The later durable Lambda orchestrator then becomes a second shell over the same core, not a rewrite.

### System Overview (v1, all on the user's Mac)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         minos (CLI binary, host)                          │
│  wires concrete impls: FileStore, KrunBackend, ClaudeCli, LocalRunner     │
├──────────────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────┐        ┌──────────────────────────────────┐   │
│  │ minos-loop            │ Job-   │ minos-runner (local runner)       │   │
│  │ generation driver     │ Request│ candidate -> judge pipeline       │   │
│  │ deps: core, store,    │──────► │ deps: core, store, sandbox        │   │
│  │       propose         │ ◄──────│ generic over B: Backend           │   │
│  │ NEVER links sandbox   │ Job-   │ the ONLY place Untrusted<T> is    │   │
│  └──────────┬────────────┘ Result │ turned into Outcome / Scorecard   │   │
│             │ (validated values)  └───────────────┬──────────────────┘   │
│  ┌──────────▼────────────┐                        │ sealed LaunchPlan     │
│  │ minos-propose         │             ┌──────────▼──────────────────┐   │
│  │ prompt assembly,      │             │ minos-sandbox               │   │
│  │ ModelProvider trait,  │             │ Role typestate, seal(),      │   │
│  │ ClaudeCli impl        │             │ preflight -> Cleared,        │   │
│  └──────────┬────────────┘             │ Untrusted<T>, capped readers │   │
│             │ exec, empty env,         └──────────┬──────────────────┘   │
│             │ empty cwd, tools off                │ Cleared plan          │
│  ┌──────────▼────────────┐             ┌──────────▼──────────────────┐   │
│  │ user's own `claude`   │             │ minos-backend-krun          │   │
│  │ binary (print mode)   │             │ builds disks, listens on     │   │
│  └───────────────────────┘             │ unix socket, spawns helper   │   │
│                                        └──────────┬──────────────────┘   │
│  ┌───────────────────────┐                        │ argv + config file    │
│  │ minos-store           │             ┌──────────▼──────────────────┐   │
│  │ Ledger + Blobs traits │             │ krun helper process (1/VM)  │   │
│  │ file + memory impls   │             │ libkrun, HVF; exit() = VM    │   │
│  └───────────────────────┘             │ exit; killed = VM destroyed  │   │
│                                        └──────────┬──────────────────┘   │
├───────────────────────────────────────────────────┼──────────────────────┤
│                   HARDWARE BOUNDARY (Hypervisor.framework)                │
├───────────────────────────────────────────────────┼──────────────────────┤
│  Guest VM: no NIC, no TSI, no shared host dir      ▼                      │
│  vda root image (ro) | vdb job disk (ro) | vdc judge data (ro, Judge only)│
│  vdd scratch (rw, capped) | vsock port -> host unix socket (output tar)   │
│  libkrun init.krun -> minos-guest (static musl) -> argv commands as uid   │
│  1000                                                                     │
└──────────────────────────────────────────────────────────────────────────┘

Persistent state (host, outside every project dir):
  $DATA/blobs/sha256/ab/<hex>      content-addressed, write-temp-then-rename
  $DATA/runs/<run>/ledger.jsonl    append-only, v-tagged records, File::lock
  $DATA/runs/<run>/driver.lock     one loop driver per run
  $DATA/images/<digest>.img        root disk images (raw)
  $DATA/vm/<launch-key>/           ephemeral per-VM dir, 0700, deleted after
  ~/.config/minos/prices.toml      user-edited price table
```

### Component Responsibilities

| Component | Owns | Must never | v1? |
|---|---|---|---|
| `minos-core` | IDs and hashing, `RelPath`, `Manifest`, `Diff`, `Metric`, `Outcome`, `Event`/`Record` and event keys, ledger fold to `RunState`, `select_greedy`, routing and `eligible`, the sweep sampler, generation planning and admission, the `JobRequest`/`JobResult` wire types, usage and cost math, `build_report`, `SafeText` | do I/O, async, clock reads, or unseeded randomness | Yes |
| `minos-store` | `Ledger` and `Blobs` traits; `MemoryStore`; `FileStore` | link the AWS SDK (DynamoDB and S3 go in a later `minos-store-aws`) | Yes |
| `minos-sandbox` | `Role` (sealed), `SandboxSpec<R>`, `seal() -> LaunchPlan`, the `Backend` trait, `preflight() -> Cleared`, `Untrusted<T>` and its validating exits, the capped stream reader and tar parser | expose a network attachment or a public constructor for `LaunchPlan`/`Cleared`, or implement `Deref`/`Debug`/`Display` on `Untrusted` | Yes |
| `minos-backend-krun` | turning a `Cleared` plan into disk images, a private socket dir, and a helper invocation; enforcing the hard deadline; host checks for preflight | call any `krun_add_net*`, `krun_set_root` (virtiofs host dir), or pass a host env | Yes |
| krun helper (binary) | the libkrun FFI only: build ctx, attach disks, disable the implicit vsock, add a no-TSI vsock with one port, `krun_start_enter` | contain logic beyond translating its versioned config into libkrun calls | Yes (spike decides its form) |
| `minos-guest` | job intake through a transport, base-tree unpack, diff apply (pure-Rust patch), argv commands under per-phase budgets, packing `/out` into a capped tar | create per-run state at startup (Lambda snapshot rule) | Yes |
| `minos-runner` | the per-experiment pipeline: candidate VM, validate its output, judge VM, `into_scorecard`, building `JobResult` from validated values; caching job results by key | return raw bytes upward | Yes (local; the Lambda CPU runner later wraps the same function) |
| `minos-propose` | deterministic prompt assembly, the `ModelProvider` trait, `ClaudeCli`, the `single_shot` tier, parsing model output into candidate diff texts | repair invalid proposals, or give the model tools | Yes |
| `minos-loop` | the local generation driver: fold, plan, propose, record, run, record, conclude, record; the driver lock; resume | link `minos-sandbox` or any backend | Yes (new crate) |
| `minos-cli` | argument parsing, wiring, rendering (escape-stripped), `report` acknowledgement, `adopt` patch writing, `env build`, `check` | apply a patch, or print a bare dollar figure | Yes |
| `minos-orchestrator`, `minos-backend-lambda`, `minos-backend-chv`, GPU daemon, `minos-store-aws` | — | — | No (later milestones) |

---

## Recommended Project Structure

```
Cargo.toml                    # workspace, [workspace.lints], [workspace.dependencies]
Cargo.lock                    # committed
deny.toml                     # cargo deny gate
clippy.toml                   # disallowed-methods (SystemTime::now etc.), scoped to core via crate attr
crates/
  minos-core/                 # sync, pure; the only crate both shells share
    src/{id,path,manifest,experiment,outcome,policy,route,ledger,require,
         usage,report,tool,limits,job,generation,sweep,text}.rs
    tests/golden/             # golden hash vectors + golden wire files (v-tagged)
  minos-store/                # traits + memory + file impls
    src/{lib,ledger,blobs,file,memory,error}.rs
  minos-sandbox/              # roles, spec, seal, Backend, preflight, Untrusted, capped tar
    src/{role,spec,plan,backend,preflight,untrusted,stream,tar}.rs
    tests/compile_fail/       # trybuild: Candidate+secrets, launch without Cleared, plan literal
  minos-backend-krun/         # host side of the krun backend (forbid unsafe holds)
    src/{lib,disk,socket,helper,preflight,host}.rs
  minos-guest/                # static aarch64-unknown-linux-musl binary + lib for tests
    src/{main,transport/{mod,disk_vsock},job,apply,exec,pack}.rs
  minos-runner/               # local runner pipeline, generic over Backend + Blobs
  minos-propose/              # prompt, provider trait, claude_cli, single_shot
    src/{prompt,provider,claude_cli,single_shot,schema}.rs
  minos-loop/                 # local generation driver
  minos-cli/                  # installs `minos`
helpers/
  krun-helper/                # ONLY if the spike picks "own helper"; see Pattern 6 + Amendment A7
examples/
  logparse/                   # the sample optimize project used for E2E
```

### Structure Rationale

- **`minos-loop` is its own crate, not a module of `minos-cli`.** The CLI binary must link everything, so a loop living inside it could reach `Untrusted` by accident or by a careless `use`. As a separate crate whose `Cargo.toml` lists only `core`, `store`, and `propose`, the loop cannot name a sandbox type, and the compiler enforces the design-of-record rule that "nothing above core sees raw sandbox output". This is the local analogue of "the orchestrator depends only on core and store".
- **The runner/loop seam is `JobRequest -> JobResult`, defined in core.** It is the same wire contract the durable orchestrator will use with `ctx.invoke` of the CPU runner function. `minos-loop` defines a small `JobRunner` trait. The CLI holds a newtype adapter that implements it over `minos-runner`, so `minos-runner` never depends on `minos-loop` (a runner must not depend on its orchestrator).
- **AWS implementations get their own crates** (`minos-store-aws`, `minos-backend-lambda`) for the same reason the design of record keeps backends out of feature flags: a local binary should not link the AWS SDK, and a Lambda binary should not link libkrun.
- **The guest depends on `minos-core` only,** for `JobSpec`, `RelPath`, and limits. Core is pure, so linking it into a static musl guest costs nothing and keeps one definition of the job wire format.

### Dependency direction (enforced by Cargo)

```
                         minos-core
          ┌──────────┬──────┴─────┬──────────────┬─────────────┐
     minos-store  minos-sandbox  minos-propose  minos-guest(bin)
          │          │      │          │
          │          │  minos-backend-krun ──► krun helper (process boundary)
          │          │
          ├──────► minos-runner   (core, store, sandbox; generic over Backend)
          │
          └──────► minos-loop     (core, store, propose)   ✗ sandbox ✗ backends
                         │
               minos-cli (all of the above; the only place concrete types meet)
```

A CI check (a small `cargo metadata` script, or `cargo deny`'s `[bans]` with `wrappers`) should assert that `minos-loop` has no path to `minos-sandbox`. That turns the boundary from a convention into a gate.

---

## Architectural Patterns

### Pattern 1: Functional core, replaceable shell ("decide / act / record")

**What:** Core exposes generation-level pure functions. A shell (the local loop now, the durable orchestrator later) only moves data between them and the effects.

```rust
// minos-core/src/generation.rs  (sketch: intent, not compiled)
pub fn plan(state: &RunState, m: &Manifest, gen: Generation) -> Result<GenPlan, PlanError>;
//   -> tier (route), parent(s), batch size, seed = H("minos/seed/v1", run, gen)
pub fn admit(plan: &GenPlan, m: &Manifest, state: &RunState,
             texts: Vec<String>, limits: &Limits) -> Admitted;
//   -> Diff::parse each, drop invalid (with reasons), dedup vs tried, cap batch,
//      build Experiment, sort by ExperimentId
pub fn conclude(state: &RunState, m: &Manifest, gen: Generation) -> Decision;
//   -> select_greedy over recorded Judged results; pure, permutation-invariant
pub fn spend_ok(state: &RunState, m: &Manifest, prices: &Prices) -> SpendCheck;
```

**When to use:** Always. Any `if` in a shell that decides experiment fate belongs in core.
**Trade-offs:** Core functions take more arguments (prices, limits, and blob contents arrive as parameters). In return, durable replay safety and property tests come for free, and the Lambda milestone is glue.

### Pattern 2: Record before act; key every effect by content

**What:** The local loop follows the durable rules, even though nothing replays it, because a Ctrl-C or crash mid-generation must resume to the same result.

1. Nondeterministic outputs (model text) are written to blobs, and their `Proposed` events are appended, before any VM launches. On resume, a generation with `Proposed` events but no `Selected` event re-uses the recorded proposals and never re-proposes.
2. Every side effect is keyed:
   - VM launch: `LaunchKey = H("minos/launch/v1", ExperimentId, Role, JudgeVersion, Trial)`. Its result is cached as a blob indexed by the key, so a resumed run skips completed jobs.
   - Ledger append: `EventKey` (see the `Ledger` trait below). A duplicate append is a no-op.
   - Spend: `SpendId = H(run, generation, source)`.
3. A `driver.lock` (an exclusive `File::lock`) ensures one driver per run, the local analogue of a single durable execution.

**Trade-offs:** Slightly more ledger writes. It is also the exact discipline the durable version needs, so it gets exercised from day one.

### Pattern 3: Async store traits with explicit `Send` futures, static dispatch

**What:** `Ledger` and `Blobs` are async traits written in the desugared return-position form, which has been stable since Rust 1.75, so `Send` is stated explicitly. There is no `async-trait` macro and no `dyn`.

```rust
// minos-store/src/ledger.rs  (sketch)
pub trait Ledger: Send + Sync {
    /// Reads the run's records in sequence order. Torn trailing line => ignored + reported.
    fn read(&self, run: &RunId) -> impl Future<Output = Result<Vec<Stored>, StoreError>> + Send;

    /// Appends iff no record with the same EventKey exists for this run.
    /// The store assigns the sequence number under its own concurrency control.
    fn append(&self, run: &RunId, rec: Record)
        -> impl Future<Output = Result<Appended, StoreError>> + Send;
}
pub enum Appended { New(Seq), AlreadyPresent(Seq) }
pub struct Stored { seq: Seq, record: Record }   // private fields, validated on read

pub trait Blobs: Send + Sync {
    fn put(&self, bytes: Bytes) -> impl Future<Output = Result<BlobId, StoreError>> + Send;
    fn put_file(&self, path: &Path) -> impl Future<Output = Result<BlobId, StoreError>> + Send;
    /// Verifies SHA-256 on read; refuses blobs larger than `max`.
    fn get(&self, id: &BlobId, max: ByteLimit) -> impl Future<Output = Result<Bytes, StoreError>> + Send;
    fn get_to_file(&self, id: &BlobId, dst: &Path, max: ByteLimit)
        -> impl Future<Output = Result<(), StoreError>> + Send;
    fn contains(&self, id: &BlobId) -> impl Future<Output = Result<bool, StoreError>> + Send;
}
```

**Why async, not sync:** The durable SDK is async on tokio: the handler is `async fn(event, DurableContext)`, and step bodies are `async` closures. The DynamoDB and S3 SDKs are async too. A sync trait would force `block_on` inside async step bodies, which panics inside a tokio runtime or deadlocks it. The CLI needs tokio anyway (subprocess with timeout, unix socket accept, child wait with deadline, bounded VM concurrency with `JoinSet`).

**Consequences:**
- These traits are not object-safe. Use generics (`Runner<B: Backend, S: Blobs>`), plus a per-binary enum if runtime selection is ever needed. This matches the design of record's "enum, not trait objects".
- Per the `async-tokio-fs` and `async-spawn-blocking` rules, the file impl uses `tokio::fs` and wraps the blocking `File::lock` (std, stable since Rust 1.89) in `spawn_blocking`.
- Core stays sync: shells `read()` records, then call `core::ledger::fold(&records)`.

**How each impl satisfies the contract:**

| Contract | FileStore | MemoryStore | DynamoDB/S3 (later) |
|---|---|---|---|
| Sequence order | line number under exclusive lock | `Vec` index under `Mutex` (lock never held across `.await`) | `PK=run`, `SK=seq`, conditional put `attribute_not_exists(SK)`, retry on conflict |
| Idempotent by `EventKey` | in-memory key index rebuilt on open, checked under lock | `BTreeSet` | `TransactWriteItems`: seq item plus `PK=run#key` item, both conditional |
| Durability | `write_all` of one line + `\n`, then `sync_data` | n/a | service |
| Torn write | trailing line without `\n` is ignored on read and truncated on the next locked append | n/a | n/a |
| Multi-writer (`steer`, `stop`, `answer` from another terminal) | the same lock | the same mutex | conditional writes |
| Blob atomicity | hash while copying to a temp file in the same dir, `sync_all`, `rename` | `BTreeMap` | `PutObject` with `ChecksumSHA256` = BlobId |
| Blob verification | rehash on `get` | n/a | S3 checksum plus rehash |

**Event keys (pure, in core):** `RunStarted` maps to the run; `ManifestChanged` to the manifest; `Proposed` to the exp; `Judged` to (exp, judge, trial); `Selected` to the generation; `Spent` to the SpendId; `ReviewRequested`/`Reviewed` to the request; `ReportAcknowledged` to (exp, report). JSONL line: `{"seq":N,"key":"<hex>","record":{"v":"1",...}}`. Golden-file it from the first commit.

### Pattern 4: Role typestate, sealing, and a preflight token that owns the plan

```rust
// minos-sandbox (sketch)
mod sealed { pub trait Sealed {} }
pub trait Role: sealed::Sealed { const KIND: RoleKind; }
pub enum Candidate {}  pub enum Judge {}  pub enum Agent {}   // uninhabited markers

pub struct SandboxSpec<R: Role> {
    image: RootImage, job: JobDisk, budget: HardBudget,
    judge_data: Option<JudgeDisk>,          // only reachable via SandboxSpec<Judge>::secrets
    key: LaunchKey, _role: PhantomData<R>,
}
impl SandboxSpec<Candidate> { pub fn candidate(img: RootImage, job: CandidateJob, b: HardBudget) -> Self; }
impl SandboxSpec<Judge> {
    pub fn judge(img: RootImage, job: JudgeJob /* holds a ValidatedOutputTree */, b: HardBudget) -> Self;
    pub fn secrets(self, s: JudgeSecrets) -> Self;   // read-only disk
}
impl<R: Role> SandboxSpec<R> { pub fn seal(self) -> LaunchPlan; }   // the only LaunchPlan ctor

pub struct LaunchPlan { /* private */ }          // read-only accessors for backends; no network field exists
pub struct Cleared { plan: LaunchPlan }          // private ctor; not Clone

pub trait Backend: Send + Sync {
    type Running: RunningVm;                     // Drop destroys the VM
    fn capabilities(&self) -> Capabilities;
    fn refusals(&self, plan: &LaunchPlan) -> Vec<Refusal>;     // backend-specific checks
    fn launch(&self, ok: Cleared) -> impl Future<Output = Result<Self::Running, LaunchError>> + Send;
}
/// The only constructor of Cleared. Lives in minos-sandbox so backends cannot mint one.
pub fn preflight<B: Backend>(b: &B, plan: LaunchPlan) -> Result<Cleared, (LaunchPlan, Vec<Refusal>)>;

pub trait RunningVm: Send {
    fn wait(self) -> impl Future<Output = Result<Untrusted<GuestOutput>, InfraError>> + Send;
}
```

**Why `Cleared` owns the plan:** The design-of-record signature `launch(plan, _ok: Cleared)` lets a `Cleared` from plan A authorize plan B. Moving the plan into the token removes that class of bug at the type level. Keeping `Cleared`'s constructor private to `minos-sandbox` (backends only supply `refusals`) means no backend crate can skip preflight. See Amendment A5.

**`AnyBackend` with one backend:** Don't build it. `minos-runner` is generic over `B: Backend`, and the CLI instantiates `Runner<KrunBackend, FileStore>`. A one-variant enum is pure ceremony (`anti-over-abstraction`). It would also have to live in a crate that links every backend, which contradicts "a Lambda binary never links hypervisor code". When Cloud Hypervisor arrives, add `enum LocalBackend { Krun(KrunBackend), Chv(ChvBackend) }` in the CLI-side crate. The Lambda runner binary instantiates `Runner<LambdaBackend, S3Blobs>` directly. See Amendment A4.

**Compile-fail tests (trybuild), from the first sandbox commit:**
1. `SandboxSpec::<Candidate>::candidate(..).secrets(s)` must not compile.
2. `LaunchPlan { .. }` and `Cleared { .. }` struct literals outside the crate must not compile.
3. `backend.launch(plan)` with a bare `LaunchPlan` must not compile.
4. `Untrusted<Bytes>` used with `{:?}`, `{}`, `*u`, or `.0` must not compile.

### Pattern 5: Guest protocol: disks in, one capped tar out, the host keeps the clock

**Device layout (krun):**

| Device | Contents | Mode | Who writes it |
|---|---|---|---|
| `vda` | root image (user Dockerfile + minos final stage), raw | read-only | `minos env build` |
| `vdb` | job disk: `MINOSJOB` magic, `u32` version, `u64` length, then a tar of `job.json` (`JobSpec`), `base.tar` (`TreeId`), `patch.diff`, and for a judge `candidate/` (a re-packed, validated candidate output tree) | read-only | host, without unpacking or applying anything |
| `vdc` | judge data, same tar-on-block format | read-only | host; attached only for `Judge` plans |
| `vdd` | scratch, sparse raw file of capped size, formatted by the guest | read-write | guest; the host never mounts it, only deletes it |
| vsock port `N` | maps to host unix socket `$DATA/vm/<key>/out.sock` (`krun_add_vsock_port`; the guest connects) | stream | guest writes a tar; the host reads it with caps |

Tar-on-block for host-written disks means the host never needs `mkfs` or a filesystem writer on macOS, and the guest reads `/dev/vdb` as a byte stream with no filesystem mount. Root uses `krun_set_root_disk_remount`. In v1.19.6 it boots libkrun's `init.krun` from a NullFs virtiofs with no host directory, then pivots to the block-device root (source verified).

**Output stream:** One tar, reserved prefix `.minos/`:
- `.minos/status.json`: per-phase exit status and which phase timed out. Candidate-influenced; used only to label failure kinds.
- `.minos/build.log` and `.minos/run.log`: capped by the guest, capped again by the host.
- `out/...`: the candidate's `/out`.

The host reader in `minos-sandbox` enforces total bytes, entry count, per-entry size, `RelPath` names, regular files only (no symlinks, hardlinks, or devices), and a read deadline. It produces `Untrusted<GuestOutput>`. A judge VM never sees candidate bytes as the candidate wrote them: the host re-serializes the validated entries into a fresh tar on the judge's `vdb`.

**Budgets and exit:**
- **Host (authoritative):** the hard deadline is `build + run + boot allowance`, clamped by `limits.rs`. On expiry the backend kills the helper process (`tokio::process::Command::kill_on_drop(true)` plus an explicit `kill`). Process death is VM death, which gives "Dropping a `Running` handle destroys the VM" for free. This maps to `Outcome::TimedOut`.
- **Guest (informational):** `minos-guest` enforces per-phase budgets so it can report which phase overran. It runs candidate commands as an unprivileged uid in their own cgroup, and kills the cgroup (`cgroup.kill`) before packing `/out`, so the candidate cannot race the packer. This in-guest privilege separation protects measurement integrity only. The VM remains the security boundary.
- **Exit code:** libkrun `exit()`s the helper with the workload's exit code (125, 126, or 127 mean init failures). Treat it as a hint. `Crashed` means the stream is missing or invalid; `Rejected(reason)` means the stream is valid but the status reports a build or command failure.

**Identical across backends:** `minos-guest` is one binary with one `execute(JobBundle) -> OutputTree` core and a `Transport` seam:

```rust
trait Transport {          // inside minos-guest; not shared with host crates
    fn next_job(&mut self) -> Result<JobBundle, GuestError>;   // krun: read /dev/vdb; Lambda: `run` hook
    fn send(&mut self, out: OutputTree) -> Result<(), GuestError>; // krun: vsock tar; Lambda: hook response/upload
}
```

The transport is chosen by an explicit argv flag set in the launch plan (`--transport=disk-vsock`), never by sniffing. `main` only parses argv and blocks in `next_job()`, so nothing per-run exists before the job arrives. That satisfies the Lambda snapshot rule without special-casing.

### Pattern 6: The libkrun helper process is mandatory, not optional

These facts come from the libkrun `v1.19.6` header and source:
- `krun_start_enter` "only returns if an error happens before starting the microVM. Otherwise, the VMM ... will call to exit() with the workload's exit code". The VMM takes over the process. The same holds for the `msb_krun` native Rust API (`Vm::enter()` never returns on success).
- Implicit vsock turns on TSI `HIJACK_INET` whenever no virtio-net is configured: `enable_tsi = net.list.is_empty() && legacy_net_cfg.is_none()`. TSI gives the guest outbound sockets through the host. **A VM with no NIC still has network unless the implicit vsock is disabled.** The required sequence is `krun_disable_implicit_vsock(ctx)` followed by `krun_add_vsock(ctx, 0)`, then `krun_add_vsock_port(ctx, N, path)`.
- When `envp` is NULL, `krun_set_exec` "will auto-generate an array collecting the variables currently present in the environment". That would copy the host environment into the guest. The helper must always pass an explicit empty `envp`, and should itself be spawned with `env_clear()`.
- `krun_set_root(path)` shares a host directory over virtiofs, which the design of record forbids. Use `krun_add_disk` (raw, with the `read_only` flag) plus `krun_set_root_disk_remount`.
- `main` is libkrun 2.0, with an incompatible, still-changing object API. Pin the `stable-1.19.x` ABI and check the version in preflight. The repo has moved to `github.com/libkrun/libkrun`.

So every VM is one child process. Its form is a spike decision that touches a code rule. **Stop and ask the user** before picking either of the first two options:
1. **A helper built on a published safe Rust API** (evaluate `msb_krun`: it has a builder API and its examples are safe Rust, but disk, no-TSI vsock, and macOS support are undocumented). If that works, the helper can live in the workspace with `unsafe_code = "forbid"` intact.
2. **A tiny out-of-workspace FFI helper** over `krun-sys` (raw bindgen, `unsafe`), roughly 150 lines, audited, with its own lints. This is an exception to `unsafe_code = "forbid"` and needs explicit approval, documented the way the rustls exception is.
3. **`krunkit`** as the helper (EFI boot). Our tree has no unsafe code, but the root image then needs its own kernel and bootloader, and krunkit's behaviour around implicit vsock and TSI must be verified.

Whichever wins, the backend talks to it through a versioned config file (`HelperConfig { v, disks, vsock_port, socket_path, vcpus, mem_mib, exec_argv }`) and argv, never a shell. The backend checks the helper's SHA-256 against a pinned value in preflight. The helper must be codesigned with `com.apple.security.hypervisor` (libkrun ships `hvf-entitlements.plist`).

### Pattern 7: `ModelProvider` as a trait for the contract, an enum per binary for selection

```rust
// minos-propose/src/provider.rs (sketch)
pub trait ModelProvider: Send + Sync {
    fn describe(&self) -> ProviderInfo;          // name, version (e.g. `claude -v`), recorded in ledger
    fn complete(&self, req: &ModelRequest)
        -> impl Future<Output = Result<ModelResponse, ModelError>> + Send;
}

pub struct ModelRequest {                        // built only by prompt::assemble (deterministic)
    model: ModelAlias,                           // manifest "mid" -> provider config -> "sonnet"
    stable: PromptPrefix,                        // instructions + program.md + surface files sorted by RelPath
    dynamic: PromptSuffix,                       // lineage summary, steer notes, SafeText log excerpts
    schema: OutputSchema,                        // {"proposals":[{"rationale":str,"diff":str}], maxItems: batch}
    max_output: TokenLimit,
}
pub struct ModelResponse {
    raw: RawModelOutput,                         // hostile; only exit is Proposals::parse
    usage: TokenUsage,                           // in, out, cache_read, cache_write
    served_by: ModelId,
}
pub enum ModelError { Infra(..) /* retry */, Refused(..), Malformed(..) /* no retry, drop */ }
```

- **Implementations:** `ClaudeCli` in v1. Later `Bedrock` (in an AWS crate) and `AnthropicApi`. Tiers are generic over `P: ModelProvider`. The CLI picks the provider from config. If runtime choice among several is needed, it uses `enum AnyProvider`, defined in the binary crate, mirroring backends.
- **Model output is hostile input:** `Proposals::parse(raw, &manifest, &limits)` uses `deny_unknown_fields`, caps the count and per-diff size, then calls `core::generation::admit`, which runs `Diff::parse`. Invalid diffs are dropped with a recorded reason, never repaired.
- **`ClaudeCli` subprocess contract** (flags verified in official docs on 2026-10-09; behaviour to spike):
  - `tokio::process::Command::new(<resolved absolute claude path>)`. Use `env_clear()` and then an allowlist (`HOME`, `USER`, `LOGNAME`, `PATH=/usr/bin:/bin`, `TMPDIR`, `LANG`). An allowlist is stronger than stripping `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and `CLAUDE_CODE_USE_*`, and covers `AWS_*` too. Spike: the subscription login must still resolve via Keychain under this env.
  - The cwd is a fresh empty 0700 temp dir, removed afterwards. The docs say a `-p` session without `--bare` "runs the hooks in a project's `.claude/settings.json` and connects the servers in its `.mcp.json`, even in a folder you've never trusted", so it must never run in the project dir. `--bare` is not an option because it "never reads OAuth credentials".
  - Flags: `-p --output-format json --json-schema <schema> --model <alias> --tools "" --disallowedTools "*" --strict-mcp-config --mcp-config '{"mcpServers":{}}' --disable-slash-commands --no-session-persistence --permission-mode dontAsk --permission-prompts none --system-prompt-file <stable.txt>`. Evaluate `--restricted` (v2.1.248+: removes command tools, loads only managed settings plus `--settings`) and `--safe-mode` (disables hooks, plugins, and MCP while keeping auth). Spike whether either works with subscription auth and with `--json-schema`.
  - The stable prefix goes in the system prompt file, above a `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` line if the dynamic part is also placed there, for cache reuse. The per-generation prompt goes via **stdin** (capped at 10 MB by the CLI, and kept out of `ps`).
  - Read `structured_output` (documented) and `usage` from stdout through a capped reader. Ignore `total_cost_usd`: it is a client estimate, and the ledger stores usage, never dollars.
  - Wall-clock timeout with `kill_on_drop`. A non-zero exit or missing `structured_output` is a `ModelError`.

---

## Data Flow

### One generation (local loop)

```
minos-loop (core+store+propose only)                 minos-runner (core+store+sandbox)
───────────────────────────────────                  ────────────────────────────────
records = ledger.read(run)
state   = core::ledger::fold(records)
core::generation::spend_ok(state, m, prices)? ─ no ─► append StopRequested(reason)
plan    = core::generation::plan(state, m, g)
            │ tier = route(m, ledger)  (STALL_STEP)
            ├─ sweep:       texts = core::sweep::sample(seed, params, base files)  [pure]
            └─ single_shot: req   = prompt::assemble(m, program, surface, state)  [pure]
                            resp  = provider.complete(req)          ── host subprocess
                            blobs.put(transcript); append Spent{SpendId, usage}
                            texts = Proposals::parse(resp.raw)
adm     = core::generation::admit(plan, m, state, texts, limits)   [pure, sorted by ExpId]
for exp in adm (sorted): blobs.put(diff); append Proposed{exp, parent, tier}
                                     │
results = JoinSet bounded by host capacity, one task per exp:
          runner.run(JobRequest{exp, base, diff_blob, judge, trial}) ───────────────►
                                                     key = LaunchKey(exp, Candidate, ..)
                                                     if blobs has result(key): return it
                                                     spec = SandboxSpec::<Candidate>::candidate(..)
                                                     c    = preflight(&backend, spec.seal())?
                                                     out  = backend.launch(c).await?.wait().await?
                                                     tree = out.into_output_tree(limits)  // Untrusted exit
                                                       └─ Err => Outcome::Rejected / Crashed
                                                     jspec = SandboxSpec::<Judge>::judge(.., tree)
                                                                .secrets(judge_secrets)
                                                     jout = launch(preflight(jspec.seal()))..wait()
                                                     card = jout.into_scorecard(limits)   // Untrusted exit
                                                     blobs.put(log excerpts, scorecard)
          ◄──────────────────────────────────────── JobResult{outcome, usage, backend_info, blob ids}
on each completion: append Judged{exp, judge, trial, outcome, backend}; append Spent{exp, usage}
records = ledger.read(run); state = fold(records)
decision = core::generation::conclude(state, m, g)                [pure, tie-break ExperimentId]
append Selected{generation: g, decision}
```

**Where order and idempotency matter, even locally:**
- `admit` output is sorted by `ExperimentId`. Launch order, `Proposed` append order, and prompt contents are all deterministic.
- `Judged` events are appended as jobs complete, so ledger order varies between runs. The fold, `conclude`, and `build_report` must therefore be insensitive to order within a generation. Policy already is (permutation-invariant by design), and property tests must also cover the fold and the report.
- Resume: on restart the loop folds, finds the last generation without `Selected`, reuses its `Proposed` set, and re-requests only experiments without `Judged`. The runner's result cache short-circuits completed VMs.
- `minos check` noise floor: K baseline runs of the same `ExperimentId` need a `trial` index in both `LaunchKey` and the `Judged` key. Otherwise idempotency collapses them into one sample (Amendment A9).

### State management

```
ledger.jsonl (append-only, v-tagged) ──read──► fold (pure) ──► RunState ──► plan/admit/conclude/report
        ▲                                                                         │
        └────────────── append (keyed, locked) ◄── shells only ◄──────────────────┘
blobs/ (content-addressed): diffs, transcripts, log excerpts, output trees, scorecards, reports, base tarballs
```

There is no stored run state anywhere: `status`, `log`, `show`, and `report` all fold the ledger.

### Key data flows

1. **Source in:** working tree, then a deterministic tarball (sorted, zeroed metadata, regular files, `.gitignore` + `.minosignore` + hard denylist + secret scan), giving a `TreeId` blob, which goes into the job disk inside `base.tar`. The guest unpacks and applies the diff.
2. **Output out:** guest `/out` and logs, then a tar over vsock, then the capped host reader, then `Untrusted<GuestOutput>`, then a validating exit (`into_output_tree`, `into_scorecard`, `into_log_excerpt`), then validated values, then blobs and ledger.
3. **Model:** core-assembled prompt, then the `claude` subprocess (no tools, empty cwd, allowlisted env), then `structured_output`, then `Proposals::parse`, then `Diff::parse`, then `Experiment`.
4. **Review gate:** `minos report <exp>` computes `build_report(records, fetched blobs)`, which is pure and hashed, renders it through `SafeText` (escape-stripped), takes the user's acknowledgement, and appends `ReportAcknowledged{exp, hash}`. `minos adopt <exp> --reviewed <hash>` checks that the acknowledgement exists for that exact exp and hash, **recomputes the report and checks the hash still matches** (it can drift after `rejudge` or a manifest change), then writes a patch file without overwriting. It never applies.

---

## Invariant Check (every proposal against the three invariants and code rules)

| Proposal | Isolation | Judge integrity | Review gate | Code rules |
|---|---|---|---|---|
| `minos-loop` crate without sandbox deps | n/a | Raw output is unreachable from the loop by Cargo graph | n/a | Strict dependency direction |
| Async store traits, static dispatch | n/a | n/a | Report is still a pure fold | No `dyn`, no `BoxError` outside handlers; `BTreeMap` in memory impl |
| `EventKey` idempotent append | n/a | `Judged` never rewritten, only appended (adds `trial`) | Acknowledgements keyed by (exp, hash) | Stable order via seq |
| `Cleared` owns plan, minted only in sandbox | Preflight cannot be skipped or reused | n/a | n/a | Private fields, no escape hatch |
| No `AnyBackend` in v1 | Lambda binary never links krun | n/a | n/a | Avoids over-abstraction |
| Tar-on-block input, vsock tar output | No shared host dir, guest-written disk never mounted | Judge sees only re-serialized validated tree | n/a | Pure-Rust tar, capped |
| Disable implicit vsock (TSI) | **Required.** Otherwise a NIC-less VM still has network | Candidate cannot exfiltrate or fetch | n/a | n/a |
| Explicit empty `envp`, helper `env_clear` | No host env (secrets) leaks into the guest | n/a | n/a | Commands are argv |
| In-guest uid separation and `cgroup.kill` | Defense in depth only; VM is the boundary | Candidate cannot forge status or race the packer without a guest-kernel LPE | n/a | n/a |
| `ClaudeCli` empty cwd, env allowlist, tools off | Model never in the VM; no tools on host | Model never sees judge data (not in prompt) | Model output goes through `Diff::parse` | No credential read or stored by Minos |
| Adopt recomputes report hash | n/a | n/a | Stale acknowledgements rejected | Pure `build_report` |
| Helper FFI option 2 | Helper is the hypervisor host process | n/a | n/a | **Touches `unsafe_code = "forbid"`; requires the user's approval** |

---

## Suggested Build Order (v1 phases and dependencies)

```
P0 spikes ──────────────┐ (answers questions P5/P6/P8 depend on)
P1 workspace + gates ───┼──► P2 minos-core ──┬──► P3 minos-store ───────────┐
                        │                     ├──► P4 minos-sandbox ──┐      │
                        │                     ├──► P5 minos-guest ────┤      │
                        │                     └──► P8 minos-propose ──┼──┐   │
                        └──► P7 env build (root image) ───────────────┤  │   │
                                              P6 minos-backend-krun ◄─┘  │   │
                                              P9 minos-runner ◄── P3,P4,P6   │
                                              P10 minos-loop ◄── P2,P3,P8 (fake runner first)
                                              P11 minos-cli ◄── all; P12 sample project E2E
```

| Phase | Delivers | Depends on | Notes |
|---|---|---|---|
| **P0 Spikes** | (a) libkrun on macOS: helper boots a raw root disk via `set_root_disk_remount`; implicit vsock disabled; `ip link` shows only `lo`; outbound connect fails; vsock port reaches a host unix socket; exit code propagates; kill-on-deadline destroys the VM; codesign entitlement. (b) Helper form (Pattern 6, user decision). (c) `claude` CLI: tools off really means no tool use; `structured_output` and `usage` field names; subscription auth under `env_clear` + allowlist; `--restricted`/`--safe-mode` compatibility. (d) Root-image production on macOS from a Dockerfile. | none | Plays the role of design-of-record build step 2 ("settles the most important open questions"). Write results into `CLAUDE.md`. |
| P1 Workspace and gates | Lints, `deny.toml`, `clippy.toml` (`disallowed-methods` for core), CI script incl. the loop-has-no-sandbox check | none | Parallel with P0 |
| P2 `minos-core` | All core types, event keys, fold, policy, routing, sweep, `generation::{plan, admit, conclude}`, `JobRequest`/`JobResult`/`JobSpec`, usage/cost, `build_report`, `SafeText`; proptest, golden vectors, golden wire files | P1 | Freeze the `Event` set (A9) before golden files |
| P3 `minos-store` | Traits, MemoryStore, FileStore; golden JSONL; crash and torn-line tests | P2 | |
| P4 `minos-sandbox` | Typestate, seal, `Backend`, `preflight`, `Untrusted`, capped tar reader; trybuild tests | P2 | |
| P5 `minos-guest` | `execute()` as a library tested on the host with a fake transport; `disk-vsock` transport; musl cross-build | P2, P0(a) | |
| P6 `minos-backend-krun` | Disk builders, socket listener, helper invocation, deadline, preflight checks; integration test boots P5 in a hand-built test image | P4, P5, P0(a,b) | Highest-risk phase; flag for deeper research |
| P7 Environment build | `minos env build` produces a root image and `minos.lock`; the minos final stage installs the guest | P0(d), P5 | Can start after P0(d); P6 uses a hand-built image until then |
| P8 `minos-propose` | Prompt assembly (golden prompts), `ModelProvider`, `ClaudeCli`, `single_shot`, `Proposals::parse` | P2, P0(c) | Parallel with P4–P7 |
| P9 `minos-runner` | Candidate-to-judge pipeline, result cache by `LaunchKey` | P3, P4, P6 | |
| P10 `minos-loop` | Driver with lock and resume; tested against MemoryStore, fake provider, and fake `JobRunner` before any VM exists | P2, P3, P8 | Can be built before P9 lands |
| P11 `minos-cli` | `init`, `env build`, `check` (noise floor), `run`, `status`, `log`, `show`, `report`, `adopt`, `stop` | all | |
| P12 Sample project E2E | `examples/logparse` through the full loop, every experiment in a VM | P11 | Definition of done |

---

## Scaling Considerations

This is a single-user local tool. Scaling here means "what breaks first on one Mac".

| Concern | First limit | Mitigation |
|---|---|---|
| VM concurrency | Host RAM and cores, and any HVF limits on VM count (unverified) | Semaphore sized from preflight's host capacity; candidate and judge share it |
| Model calls | Subscription rate limits and latency | Batch per call (`batch = 4`); record-before-act means a rate-limit failure is retried without losing work |
| Blob growth | Base tarballs and output trees per experiment | Content addressing dedupes; add `minos gc` later (mark from ledgers) |
| Ledger size | Thousands of lines per run | Folding is linear and cheap; no index needed in v1 |

---

## Anti-Patterns

### Anti-Pattern 1: Trusting "no NIC" to mean "no network" in libkrun
**What people do:** Omit `krun_add_net_*` and assume the VM is offline.
**Why it's wrong:** libkrun's implicit vsock enables TSI `HIJACK_INET` exactly when no NIC is configured, which proxies guest sockets through the host (verified in `v1.19.6` source).
**Do this instead:** Call `krun_disable_implicit_vsock`, then `krun_add_vsock(ctx, 0)`. Preflight asserts the helper version. An integration test asserts that outbound connect fails from inside the guest.

### Anti-Pattern 2: Passing NULL `envp` or inheriting the CLI's environment
**What people do:** Call `krun_set_exec(ctx, path, argv, NULL)`, or spawn the helper or `claude` with the inherited env.
**Why it's wrong:** NULL `envp` copies the host environment into the guest, where `AWS_*` or `ANTHROPIC_*` would be readable by candidate code. An inherited env lets an API key override the user's login for `claude`.
**Do this instead:** Explicit empty `envp`; `env_clear()` plus an allowlist on every child process.

### Anti-Pattern 3: Putting the loop inside the CLI crate "for now"
**Why it's wrong:** The boundary that keeps raw output out of decision code then rests on convention in the one crate that links everything.
**Do this instead:** `minos-loop` as a separate crate plus the CI dependency check.

### Anti-Pattern 4: A sync `Ledger` trait "because core is sync"
**Why it's wrong:** The durable SDK and AWS SDKs are async. A sync trait means `block_on` in step bodies later, which is the rewrite this milestone exists to avoid.
**Do this instead:** Async traits with explicit `Send` futures; core stays sync over loaded records.

### Anti-Pattern 5: Running `claude -p` in the project directory
**Why it's wrong:** Print mode loads project `.claude/settings.json` hooks and `.mcp.json` servers without a trust prompt, and the project is the very tree the loop is modifying.
**Do this instead:** An empty temp cwd, an empty strict MCP config, tools off, and slash commands off.

### Anti-Pattern 6: Domain-separating `BlobId`
**Why it's wrong:** The design of record says the SHA-256 "doubles as the upload checksum". S3's `ChecksumSHA256` is the plain digest of the object, so a prefixed hash breaks that property.
**Do this instead:** `BlobId = SHA-256(content)`. Domain separation applies to composite IDs (`ExperimentId`, `LaunchKey`, `EventKey`, `ReportHash`) (Amendment A10).

### Anti-Pattern 7: Building `AnyBackend` with one variant in `minos-sandbox`
**Why it's wrong:** It inverts the dependency direction (sandbox would depend on backends) and makes every binary link every backend.
**Do this instead:** Generic runner; a per-binary enum when a second backend exists.

---

## Integration Points

### External services and binaries

| Integration | Pattern | Notes |
|---|---|---|
| libkrun (`stable-1.19.x`) + libkrunfw | One helper process per VM, versioned config file | `main` is the 2.0 API, incompatible and unstable; the Homebrew `slp/krun` tap is not core Homebrew; the helper needs the hypervisor entitlement |
| `claude` CLI (user's install) | `tokio::process`, stdin prompt, JSON stdout | Minos never reads or stores credentials; README points to API key or Bedrock beyond personal use |
| Container engine for `env build` (spike P0(d)) | Build phase has registry access and no secrets | Whether Docker or Podman is required on macOS, or the build runs in a builder VM, is open |
| Rust musl cross-compile for the guest | `aarch64-unknown-linux-musl` via zig (`cargo-zigbuild`, or the zig that cargo-lambda already uses) | Verify in P5 |

### Internal boundaries

| Boundary | Communication | Notes |
|---|---|---|
| loop and runner | `JobRequest` / `JobResult` (core types, serde, golden-filed) | The same contract as the future `ctx.invoke` of the CPU runner |
| loop and propose | `ModelProvider::complete`, `Proposals::parse` | Proposals are validated values |
| runner and backend | `preflight(plan) -> Cleared`, `launch`, `wait -> Untrusted` | Typestate-enforced |
| backend and helper | argv + `HelperConfig` file (v-tagged) + exit status | The helper is hash-pinned |
| host and guest | Job disk (tar-on-block), output tar over vsock | Capped on both ends; the host's caps are authoritative |
| CLI and ledger (concurrent `steer`, `stop`, `answer`) | Keyed appends under the file lock | The loop reads them at the next generation boundary |

---

## Design-of-Record Amendments (proposed `CLAUDE.md` edits)

Each needs to land in the same change as the code it describes, per "Keep this file current".

**A1. Architecture intro.** Replace "There is no always-on service. The CLI talks to AWS with the user's own credentials." with:
> There is no always-on service. In milestone v1 the loop runs inside the `minos` CLI process on the user's machine, against a file-backed ledger and blob store, with libkrun sandboxes. The AWS deployment (durable Lambda orchestrator, Lambda MicroVMs, DynamoDB, S3, Bedrock) is a later milestone that reuses `minos-core` unchanged. When it exists, the CLI talks to AWS with the user's own credentials.

**A2. "The model stays outside the sandbox".** Replace "There is no API key anywhere." with:
> Minos never reads, stores, or passes a model credential. On AWS the agent loop calls Bedrock with the function's IAM role. Locally, the `ModelProvider` execs the user's own unmodified `claude` binary in print mode. It runs with an empty working directory, a cleared and allowlisted environment, and all tools, MCP servers, and slash commands disabled, so the binary authenticates with the user's own login. For anything beyond personal use, the README directs users to an API-key or Bedrock provider.

**A3. Workspace.** Add `minos-loop/  local generation driver; depends on core, store, propose`. Mark `minos-orchestrator`, `minos-backend-lambda`, `minos-backend-chv`, and the GPU daemon as later milestones. Move DynamoDB and S3 to a later `minos-store-aws` crate. Change the dependency rule to:
> `core <- store, sandbox, propose`; `sandbox <- backends`; `core, store, sandbox <- runner`; `core, store, propose <- loop, orchestrator`. Neither the loop nor the orchestrator ever links `minos-sandbox` or a backend, so neither can see raw sandbox output. A CI check enforces this.

(The existing "orchestrator depends only on core and store" is already wrong for `single_shot`, which calls a model inside a generation step.)

**A4. `AnyBackend`.** Replace "With three backends, use an `AnyBackend` enum, not trait objects." with:
> The runner is generic over `B: Backend`. Each binary picks its backend concretely, and a binary that can choose among several at runtime defines its own enum (for example `LocalBackend { Krun, Chv }`). Never use trait objects, and never put a backend enum in `minos-sandbox`.

**A5. Preflight.** Replace the two signatures under Local backends with:
```rust
pub fn preflight<B: Backend>(b: &B, plan: LaunchPlan) -> Result<Cleared, (LaunchPlan, Vec<Refusal>)>;
// Cleared owns the plan and is constructed only in minos-sandbox; backends supply `refusals`.
fn launch(&self, ok: Cleared) -> impl Future<Output = Result<Self::Running, LaunchError>> + Send;
```
Preflight applies to every backend, not just local ones.

**A6. Local backends (macOS).** Add:
> - libkrun creates an implicit vsock device with TSI enabled whenever no NIC is configured. Every launch calls `krun_disable_implicit_vsock` and then `krun_add_vsock(ctx, 0)`. An integration test proves outbound connections fail.
> - Never call `krun_set_root` or `krun_add_virtiofs*` with a host path. Root is a raw read-only disk via `krun_set_root_disk_remount`.
> - Always pass an explicit empty `envp`, and spawn helpers with a cleared environment.
> - `krun_start_enter` takes over and exits the process, so each VM is one helper child process. Killing it destroys the VM.
> - macOS has neither seccomp nor cgroups. The "dedicated unprivileged user with its own seccomp filter and cgroup limits" requirement is met on macOS by [decision pending P0: dedicated user and/or Seatbelt profile, plus rlimits]. Until then, local runs record which hardening applied.
> - Pin libkrun to the `stable-1.19.x` ABI and check its version in preflight.

**A7. Code rules.** If P0 picks an FFI helper, add one sentence that names the exception and its location, as with rustls:
> The libkrun helper at `helpers/krun-helper` is the one crate allowed `unsafe`, limited to FFI calls, audited, and outside the workspace lints.

Do not add this without the user's explicit approval.

**A8. Build order.** Replace with a v1 local order (P0 to P12 above), and keep the AWS order as "Milestone 2":
> 1. Spikes: libkrun on macOS with no network and vsock output; `claude` print-mode contract; root image from Dockerfile. 2. Workspace and gates. 3. `minos-core`. 4. `minos-store` (file and memory). 5. `minos-sandbox`. 6. `minos-guest`. 7. `minos-backend-krun`. 8. Environment build. 9. `minos-propose` with the `claude` provider. 10. `minos-runner`. 11. `minos-loop`. 12. CLI and the sample `optimize` project end to end.
> Milestone 2 (AWS): durable function spike, Lambda MicroVM spike, `minos-store-aws`, `minos-backend-lambda`, `minos-orchestrator`, Bedrock provider.

**A9. Ledger events.** These must be decided before golden files freeze the format:
- `Judged` gains `trial: u32` (the noise floor and repeated noisy measurements need it). Alternatively add `Calibrated { exp, judge, samples }`.
- Add `Steered { note: BlobId }` (`minos steer` currently has no event).
- Add `StopRequested { reason }` (`minos stop` and spend-cap stops).
- Add `ProviderUsed { info: ProviderInfo }`, or put provider info into `Spent`, so the report's provenance names the model path.
- Document each event's `EventKey`.

**A10. IDs.** Add:
> `BlobId` (and `TreeId`, which is the `BlobId` of the source tarball) is the plain SHA-256 of the content, so it matches S3's `ChecksumSHA256`. Domain-separated, length-prefixed hashing applies to composite IDs.

**A11. Open questions.** Mark the AWS questions "deferred to Milestone 2" and add the P0 items: the helper's form; whether TSI is fully disabled; HVF concurrent VM limits; macOS host hardening; root image production; `claude` tools-off verification, usage field names, and `--restricted` compatibility; whether `--max-turns 1` is safe on a plain reply.

**A12. Verified facts.** Add a dated libkrun entry (`v1.19.6` header and source facts above; repo moved to `libkrun/libkrun`; `main` is 2.0) and a Claude CLI entry (`structured_output`, `--bare` skips OAuth, project hooks run under `-p`, `--restricted`, `--tools ""` does not cover MCP). Add the guest cross-compile command to Commands, and note that `cargo lambda build` is not used in v1.

---

## Sources

- libkrun `v1.19.6` header, read in full: `include/libkrun.h` at https://github.com/libkrun/libkrun/tree/v1.19.6. Covers `krun_start_enter`, `krun_add_vsock`, `krun_disable_implicit_vsock`, `krun_set_exec` envp, `krun_add_disk`/`krun_add_disk2` security note, and `krun_set_root_disk_remount`.
- libkrun `v1.19.6` source, `src/libkrun/src/lib.rs`: TSI auto-enable condition, the NullFs root for block boot, `krun-sys` bindings. Primary source. Seam tier for the fetch provider is LOW, so it is listed as P0 spike verification.
- libkrun README (https://github.com/libkrun/libkrun): "TSI ... is automatically enabled when no network interface is added"; `main` is libkrun 2.0, not backwards compatible.
- crates.io search for libkrun bindings (`krun-sys`, `libkrun-sys`, `msb_krun`, `krunkit`) and docs.rs for `msb_krun` (`Vm::enter()` never returns on success). LOW.
- Claude Code headless docs: https://code.claude.com/docs/en/headless (`structured_output`, `--bare` skips OAuth, `-p` runs project hooks and MCP without trust). LOW tier per seam, official source.
- Claude Code CLI reference: https://code.claude.com/docs/en/cli-reference (`--tools`, `--restricted`, `--safe-mode`, `--strict-mcp-config`, `--disable-slash-commands`, `--no-session-persistence`, `--permission-prompts`, `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`). LOW tier per seam, official source.
- AWS durable execution SDK for Rust: https://github.com/aws/aws-durable-execution-sdk-rust (async tokio handler, async step closures, `Serialize + DeserializeOwned` step outputs, `test-util` `LocalRunner`). LOW tier per seam, official source.
- Rust `File::lock` family stabilized in 1.89: [rust-lang/rust PR 148322](https://github.com/rust-lang/rust/pull/148322), [std TryLockError docs mirror](https://docs.w3cub.com/rust/std/fs/enum.trylockerror). LOW.
- Design of record: `/Users/seanmcauliffe/Repos/minos/CLAUDE.md`. Project context: `/Users/seanmcauliffe/Repos/minos/.planning/PROJECT.md`. Rules: `/Users/seanmcauliffe/Repos/minos/rules/` (anti-over-abstraction, anti-type-erasure, api-sealed-trait, api-typestate, async-tokio-fs, async-spawn-blocking, proj-workspace-large).

---
*Architecture research for: Minos local-first v1 (secure autonomous experiment loop)*
*Researched: 2026-10-09*
