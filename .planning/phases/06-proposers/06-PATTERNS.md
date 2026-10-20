# Phase 6: Proposers - Pattern Map

**Mapped:** 2026-10-19
**Files analyzed:** 27 (17 source/module files including modified core files, 10 test files)
**Analogs found:** 25 / 27

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/minos-propose/Cargo.toml` | config | n/a | `crates/minos-propose/Cargo.toml` (stub) + root `Cargo.toml` lines 92-114 | exact |
| `crates/minos-propose/src/lib.rs` | module root | n/a | `crates/minos-source/src/clean.rs` lines 1-22 (trust-boundary module docs) | role-match |
| `src/limits.rs` | config (constants) | n/a | `crates/minos-core/src/limits.rs` lines 1-27; `minos-cli/src/builder.rs` lines 42-60 | exact |
| `src/provider.rs` | trait + types | request-response | `minos-sandbox` `Backend` trait / `minos-store` `Ledger` trait | role-match |
| `src/fake.rs` | test double (pub) | request-response | `crates/minos-store/src/memory.rs` | exact |
| `src/prompt.rs` | transform | pure | `crates/minos-core/src/sweep.rs` (`render`), `minos-core/src/text.rs` (`SafeText`) | partial |
| `src/schema.rs` | transform (const text) | pure | hand-written const + golden test, like `minos-backend-krun` Seatbelt profile SHA pin | partial |
| `src/single_shot.rs` | service | request-response + transform | `crates/minos-core/src/sweep.rs` lines 186-262 (sample -> render -> Diff), `diff/compose.rs::diff_trees` | role-match |
| `src/sweep.rs` | service wrapper | transform | `crates/minos-core/src/sweep.rs` `sample` + `render` | exact (thin wrapper) |
| `src/proposal.rs` | model (validated types) | n/a | `crates/minos-core/src/lock.rs` lines 135-235 (private fields, `try_from`) | exact |
| `src/claude_cli/mod.rs` | provider (typestate) | request-response | `crates/minos-sandbox/src/preflight.rs` lines 252-290 (`Cleared<R>` / `preflight`) | exact |
| `src/claude_cli/locate.rs` | utility | file-I/O | `minos-cli` locator (finds `minos-vmm` next to `minos`), `minos-store` symlink_metadata checks | role-match |
| `src/claude_cli/preflight.rs` | utility | request-response (subprocess) | `crates/minos-cli/src/builder.rs` identity run (`IDENTITY_DEADLINE`, `IDENTITY_STDOUT_CAP`) | exact |
| `src/claude_cli/command.rs` | utility | subprocess, stdin in, capped out | `crates/minos-cli/src/builder.rs` lines 187-320 (`BuildProcess::spawn`, `watch`, `kill_and_reap`, `drain_capped`) + `minos-source/src/clean.rs` lines 196-285 | exact |
| `src/claude_cli/classify.rs` | pure classifier | transform | `minos-backend-krun/src/supervise.rs` `classify` (pure, table-tested) | exact |
| `crates/minos-core/src/ledger.rs` (modify: `StopReason::ProviderAuth`) | model | n/a | itself, lines 34-36, 157-183 | exact |
| `crates/minos-core/tests/golden_wire.rs` + snapshot (modify: new stop case, new pin) | test | n/a | itself, lines 659-665, 755-777 | exact |
| `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `CLAUDE.md` (D-11 wording, model ID choice) | docs | n/a | n/a | n/a |
| `tests/argv_golden.rs` | test (golden) | n/a | `crates/minos-core/tests/golden_wire.rs` | exact |
| `tests/schema.rs` | test (golden + round trip) | n/a | `golden_wire.rs` | exact |
| `tests/prompt_golden.rs` | test (golden + pin) | n/a | `golden_wire.rs` | exact |
| `tests/prop_prompt.rs` | test (proptest, shuffle invariance) | n/a | `crates/minos-core/tests/prop_policy.rs` (permutation invariance) | exact |
| `tests/held_out.rs` | test | n/a | `crates/minos-core/tests/report.rs` / `route.rs` style unit integration | role-match |
| `tests/classify.rs` | test (table) | n/a | `supervise::classify` tests in `minos-backend-krun/src/supervise.rs` | exact |
| `tests/single_shot.rs`, `tests/sweep.rs` | test against FakeProvider | n/a | `crates/minos-core/tests/sweep.rs`; `minos-store/tests/contract.rs` (MemoryStore usage) | role-match |
| `tests/fake_claude.rs` | test (spawn path with shell-script fake) | subprocess | `minos-source/src/clean.rs` tests lines 293-376 (`/bin/sleep`, `/usr/bin/yes`, `/usr/bin/env`) | exact |
| `tests/live.rs` | ignored live test | subprocess | `crates/minos-backend-krun/tests/exit.rs` lines 180, 264-279 + `tests/harness/mod.rs` lines 85-100 | exact |

## Pattern Assignments

### `src/claude_cli/command.rs` (subprocess with stdin, deadline, process-group kill, caps)

**Analog:** `crates/minos-cli/src/builder.rs` (the docker spawn). It already does every piece D-06/D-16 need: `env_clear` + explicit env, stdin fed from a thread, `process_group(0)`, watchdog thread, `kill_process_group`. `minos-source/src/clean.rs` is the simpler variant (no stdin, no pgroup, scoped threads, `read_capped` with cap+1 detection) and supplies the over-cap detection and the env test.

Imports (builder.rs line 39): `use rustix::process::{Pid, Signal, kill_process_group};` -> add `rustix.workspace = true` (workspace pins `=1.1.5` with `process`; rustix is already on deny.toml's allowed list line 89). Constants beside code, builder.rs lines 42-60:
```rust
pub const BUILD_DEADLINE: Duration = Duration::from_mins(30);
pub const IDENTITY_DEADLINE: Duration = Duration::from_secs(10);
pub const BUILDER_STDERR_CAP: u64 = 64 * 1024;
const IDENTITY_STDOUT_CAP: u64 = 4096;
const POLL: Duration = Duration::from_millis(20);
```
Spawn (builder.rs lines 197-221):
```rust
let mut command = Command::new(program);
command
    .args(args)
    .env_clear()
    .envs(env.iter().map(|(key, value)| (key, value)))
    .current_dir("/")            // ClaudeCli: the fresh empty per-call temp dir
    .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .process_group(0);
...
(Some(bytes), Some(mut pipe)) => Some(thread::spawn(move || {
    // Dropping the pipe at the end of the thread closes it, which ends the context.
    pipe.write_all(&bytes)
})),
```
Watchdog and kill (builder.rs lines 270-310): `watch(child, started, deadline, &stop)` polling `try_wait`, then
```rust
fn kill_and_reap(child: &mut Child) {
    kill_process_group(Pid::from_child(child), Signal::KILL).ok();
    child.kill().ok();
    child.wait().ok();
}
```
Capped reads: use clean.rs lines 247-253 for stdout (`pipe.take(cap.saturating_add(1)).read_to_end`, then `len > cap` -> error, as at lines 233-235) and builder.rs `drain_capped` (lines 312-320) for stderr (keep cap, drain rest). Note builder.rs `finish` uses `unwrap_or_else`/`unwrap_or_default` on joins, never `unwrap` (forbid lints).

Keep `argv()` and `env()` as pure builders returning `Vec<OsString>` / `[(OsString, OsString)]` so `tests/argv_golden.rs` can pin them without spawning (mirrors clean.rs's `pub const STATUS_ARGS: [&str; 11]`, lines 55-67, and `GIT_PATH` line 39 = the same `/usr/bin:/bin:/usr/sbin:/sbin`).

Error mapping: every spawn/wait/timeout/over-cap failure becomes `ProviderError::Infra { cause, usage: None }` (like `BuilderError::{Spawn, Wait, TimedOut}` and clean.rs `Dirty::{GitUnavailable, TimedOut, OutputTooLarge}`).

---

### `src/claude_cli/preflight.rs` (`--version`, `auth status`, parent-env scan)

**Analog:** builder.rs identity run (short `IDENTITY_DEADLINE`, `IDENTITY_STDOUT_CAP`), using the same `command.rs` spawn. Parent-env scan takes an injected `ParentEnv` snapshot (no `set_var`; edition 2024). The `auth status` JSON binds only `loggedIn`, `authMethod`, `apiProvider`, `apiKeySource: Option<..>` leniently (no `deny_unknown_fields`; the one documented exception, as for the envelope). Refusal enum shaped like clean.rs `Dirty` (lines 89-134): `#[derive(Debug, thiserror::Error)] #[non_exhaustive]`, each variant with an actionable message naming the fix and every path checked.

---

### `src/claude_cli/mod.rs` (`ClaudeCli`, typestate)

**Analog:** `crates/minos-sandbox/src/preflight.rs` lines 252-290.
```rust
pub struct Cleared<R: Role> {
    plan: LaunchPlan<R>,
}
...
pub fn preflight<R: Role>(host: &Capabilities, plan: LaunchPlan<R>)
    -> Result<Cleared<R>, Vec<Refusal>> {
    let mut refusals = BTreeSet::new();
```
Apply: `ClaudeCli` fields private, no `Clone`/`Default`, `ClaudeCli::preflight(&ParentEnv)` the only constructor; collect refusals into a `BTreeSet` then `Vec` for stable order. The re-resolve-per-call rule (D-01/D-02 amended) lives in `complete`.

---

### `src/claude_cli/classify.rs` (envelope -> Completion | ProviderError)

**Analog:** `crates/minos-backend-krun/src/supervise.rs` `classify` (a pure, exhaustively table-tested function over host-observed facts). Input: exit status + capped stdout bytes; lenient envelope struct (no `deny_unknown_fields`, never binds `total_cost_usd` or `costUSD`); output closed enums. Usage via `minos_core::usage::Usage::tokens(..)`.

---

### `src/proposal.rs` and the strict `structured_output` parser in `single_shot.rs`

**Analog:** `crates/minos-core/src/lock.rs` lines 135-235, 314-317.
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawBuilder", into = "RawBuilder")]
pub struct BuilderId { name: BuilderText, version: BuilderText }

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBuilder { name: String, version: String }

impl TryFrom<RawBuilder> for BuilderId {
    type Error = LockError;
    fn try_from(raw: RawBuilder) -> Result<Self, Self::Error> { ... }
}
```
Apply: `RawProposals { proposals: Vec<RawProposal> }`, `RawProposal { rationale, files: Vec<RawFile> }`, `RawFile { path, content }`, all `deny_unknown_fields`, no `flatten`. Parse each item separately so one bad item is dropped (counted by `DropReason`), not the batch. `ModelOutput` should copy `Untrusted<T>`'s shape (`crates/minos-sandbox/src/untrusted.rs` lines 48-56): one private field, no `Debug`/`Display`/`Deref`/`Clone`, crate-private constructor.

---

### `src/single_shot.rs` and `src/sweep.rs` (tiers)

**Analog:** `crates/minos-core/src/sweep.rs` lines 186-262 and `diff/compose.rs` line 65.
```rust
pub fn sample(run: &RunId, generation: u32, params: &[SweepParam], batch: u32)
    -> Result<Vec<Sample>, SweepError>;
pub fn render(base: &Tree, params: &[SweepParam], sample: &Sample,
    writable: &BTreeSet<RelPath>, limits: &Limits) -> Result<Diff, SweepError>;
pub fn diff_trees(base: &Tree, candidate: &Tree,
    writable: &BTreeSet<RelPath>, limits: &Limits) -> Result<Diff, ComposeError>;
```
The sweep wrapper calls `sample` then `render(parent_tree, ...)`, mapping each `SweepError` (all variants are "drop the sample", sweep.rs lines 119-183, incl. `NoChange`) to a drop count. `single_shot` builds a candidate `Tree` from the parent `Tree` (`diff/apply.rs` lines 34-44, `BTreeMap<RelPath, Vec<u8>>`) plus returned files, then `diff_trees` against the parent; `ComposeError` -> drop. The loop composes the cumulative diff (compose.rs line 99), not the proposer.

---

### `src/fake.rs` (`FakeProvider`)

**Analog:** `crates/minos-store/src/memory.rs` lines 1-40: public, documented as the test double for everything above, `Mutex<..>` state with poisoned lock mapped to a typed error:
```rust
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, StoreError> {
    mutex.lock().map_err(|_poisoned| StoreError::Poisoned)
}
```
Script is a `Mutex<VecDeque<Result<raw serde_json::Value response, ProviderError>>>`; an exhausted script returns an Infra error (never panics). Only raw outputs, never `Proposal`s.

---

### `src/limits.rs`

**Analog:** `crates/minos-core/src/limits.rs` lines 1-27 (module doc says module-only caps live beside their module "in the same style"):
```rust
/// Longest model identifier, in bytes. Long enough for a Bedrock inference-profile ID such as
/// `us.anthropic.claude-...-v1:0`.
pub const MAX_MODEL_ID_BYTES: usize = 128;
```
One doc comment with unit and reason per constant; section separator comments `// ----` as in core. Includes history N, byte caps, `CALL_DEADLINE` (~15 min, `Duration::from_mins(15)` as builder.rs uses `from_mins`), stdout/stderr caps, `MIN_VERSION` (2.1.296). Reuse core's `RATIONALE_EXCERPT_BYTES`, `MAX_LOG_EXCERPT_BYTES`, `SINGLE_SHOT_SURFACE_MAX_BYTES`, `MAX_STEER_NOTE_BYTES`, `BATCH_MIN/MAX` rather than redefining.

---

### `crates/minos-core/src/ledger.rs` (add `StopReason::ProviderAuth`)

**Analog:** itself. Add the variant after `ProviderLimit` (line 169), the tag arm `Self::ProviderAuth => 7,` in `tag()` (lines 174-182, explicit arms, never a discriminant cast), and update the module doc tag list (lines 34-35: "...`ProviderLimit` 6" -> add "`ProviderAuth` 7").

### `crates/minos-core/tests/golden_wire.rs` (new stop case)

Add a `("record_v1_stop_requested_provider_auth", Event::StopRequested { generation: 3, reason: StopReason::ProviderAuth })` entry near lines 659-665, bump `assert_eq!(lines.len(), 24)` (line 742), add the name to the shell loop comment (lines 758-768), accept the snapshot with `INSTA_UPDATE=always` and recompute the pin (line 775) outside the code under test.

---

### Test files

- **Golden tests (`argv_golden.rs`, `schema.rs`, `prompt_golden.rs`):** copy `golden_wire.rs` lines 1-11 (doc explaining insta + SHA pin), `type TestResult = Result<(), Box<dyn Error>>;` (line 37), `sha256_hex` (lines 727-730 using `sha2` + `base16ct::lower::encode_string`), and the `golden_snapshots` / `all_golden_bytes_pinned` pair (lines 739-777). Add dev-deps `sha2`, `base16ct`, `proptest`. `.config/insta.yaml` already sets `update: no` workspace-wide.
- **`prop_prompt.rs`:** follow `crates/minos-core/tests/prop_policy.rs` permutation-invariance style (shuffle history input, assert identical prompt bytes and `PromptHash`); commit any `proptest-regressions` file.
- **`fake_claude.rs`:** follow clean.rs tests lines 293-376: drive the real `run` function against system binaries / a shell-script fake in a tempdir (`/bin/sleep` for deadline + assert `elapsed() < 5s`, `/usr/bin/yes` for over-cap, `/usr/bin/env` for exact allowlisted env sorted and compared to the six variables). Add the pgroup check (the fake spawns a grandchild; after timeout it must be gone). Needs `tempfile` dev-dep.
- **`live.rs`:** copy `exit.rs` line 180 `#[ignore = "needs ..."]` reason strings, and `harness/mod.rs` lines 85-100 `env_path` style (`std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {SETUP}"))?`). Gate on `MINOS_CLAUDE_LIVE`, read only here. If a `tests/harness/mod.rs` is shared between targets, every item must be used by each target (CLAUDE.md Testing note).

## Shared Patterns

### Errors separate blame
**Source:** `crates/minos-source/src/clean.rs` lines 89-134 (`Dirty`), `minos-cli/src/builder.rs` `BuilderError`.
**Apply to:** `ProviderError` (Stop{Limit,Auth} / Infra), `PreflightError`, `DropReason`. `thiserror`, `#[non_exhaustive]`, messages that name the fix, no free model text in a variant (use `SafeText` or counts).

### Allowlisted env + argv arrays
**Source:** clean.rs lines 204-215 and its test lines 352-375 (asserting the exact sorted env).
**Apply to:** model call, `--version`, `auth status` (all use the same six variables).

### Deterministic order
`BTreeMap`/`BTreeSet` for per-model usage, refusals, history rows (as `Tree`, `preflight` refusals).

### No unwrap anywhere
Thread joins and locks use `unwrap_or_else`/`map_err` as in builder.rs `finish` and memory.rs `lock`; tests return `TestResult` and use `?`.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/prompt.rs` (fenced untrusted blocks, history table) | transform | pure | No prompt assembly exists; use 06-RESEARCH patterns, `SafeText`/`LogExcerpt` from `minos-core/src/text.rs`, `PromptHash::of` |
| `src/schema.rs` (JSON Schema text) | const | pure | No JSON Schema in the codebase; hand-written const per `.claude/CLAUDE.md` Question 9, guarded like the Seatbelt profile SHA pin |

## Metadata

**Analog search scope:** `crates/minos-core`, `minos-source`, `minos-cli`, `minos-sandbox`, `minos-store`, `minos-backend-krun/tests`, `deny.toml`, `Cargo.toml`, `.config/insta.yaml`
**Files scanned:** ~20
**Pattern extraction date:** 2026-10-19
