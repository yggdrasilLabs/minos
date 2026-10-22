# Phase 7: Runner and Loop - Pattern Map

**Mapped:** 2026-10-22
**Files analyzed:** 30 (new or modified, from 07-RESEARCH "Recommended Module Structure" and 07-CONTEXT D-01 to D-16)
**Analogs found:** 28 / 30

All excerpts below already pass the workspace lints (no `unwrap`, `expect`, slice indexing, or `#[allow]`), tests included. Tests return `TestResult = Result<(), Box<dyn Error>>` and use `?` and `ok_or`/`ok_or_else`, never panicking helpers.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/minos-core/src/lock.rs` (+ `LockedJudge`, `RawLockedJudge`, `JudgeFile`, `Lock::judge`, `with_judge`, `LockMismatch::NoJudge`, `MAX_LOCK_BYTES`) | model (wire) | transform | same file, `LockedImage`/`RawLockedImage`/`RawLock` (lines 230-497) | exact |
| `crates/minos-core/src/manifest.rs` (+ `RawJudge.trials`, `JudgeSpec::trials()`) | model (wire) | transform | same file, `RawJudge` (line 1005) and the budget defaults | exact |
| `crates/minos-core/src/ledger.rs` (+ `Judged.logs`, `RunStarted.root_image`, optional `Proposed.batch`) | model (wire) | event-driven | `RawLock.image` `#[serde(default, skip_serializing_if = ...)]` (lock.rs 449-455) | role-match |
| `crates/minos-core/src/state.rs` (+ `root_image()`, `stops()`, `backend_class()`, `has_spend`, `next_model_call`, `judged`) | model (fold) | transform | same file, `RunState` accessors (lines 588-705) | exact |
| `crates/minos-core/src/diff/cumulative.rs` (`CumulativeDiff`, `BaseTree`) | model (newtype) | transform | `diff/compose.rs` `compose`/`diff_trees` (lines 65, 99) + private-field validated types | role-match |
| `crates/minos-core/src/job.rs` (candidate takes `CumulativeDiff`, judge usage) | model | request-response | same file (golden-tested in `tests/golden_wire.rs`) | exact |
| `crates/minos-core/src/money.rs` (+ `Prices::missing_rates`) | utility (pure) | transform | same file | exact |
| `crates/minos-core/src/policy.rs` (+ `baseline_ready`) | service (pure) | transform | same file, `valid_score` (227), `circuit_breaker` (517), `spend_ok` (570) | exact |
| `crates/minos-core/tests/golden_wire.rs`, `tests/lock.rs`, `tests/snapshots/*` | test (golden) | transform | `tests/golden_wire.rs` `all_golden_bytes_pinned` (761) | exact |
| `crates/minos-store/src/lib.rs` (`Appended {New, Duplicate}`, `StoreError::Conflict`, `RunLocked`, `RunLock` trait) | model / trait | CRUD | same file, `Appended` (43-75), `StoreError` (145-236) | exact |
| `crates/minos-store/src/memory.rs` (append returns `Err(Conflict)`, `RunLock` over `Mutex<BTreeSet<RunId>>`) | store | CRUD | same file (1-110) | exact |
| `crates/minos-store/src/file/ledger.rs` (append returns `Err(Conflict)`) | store | file-I/O | same file, `append` (186-244) | exact |
| `crates/minos-store/src/file/run_lock.rs` (`driver.lock`) | store | file-I/O | `file/ledger.rs` `open_lock` + `lock_options` (170-184), `file/mod.rs` `open_or_create` (218-242) | exact |
| `crates/minos-store/tests/contract.rs` (Conflict as error, run-lock cases) | test (contract) | CRUD | same file, `ledger_contract!` (286-304), `conflicting_append_is_not_written` (88-106) | exact |
| `crates/minos-sandbox/src/untrusted.rs` (+ `store_judge_input`, `store_tail`; needs user approval) | model (hostile wrapper) | transform | same file, `into_log_excerpt` (297-304), `into_scorecard` (273-296) | exact |
| `crates/minos-sandbox/tests/ui/*` + `compile_fail.rs` (new trybuild cases) | test (compile-fail) | n/a | existing `tests/compile_fail.rs` + `tests/ui/` | exact |
| `crates/minos-source/src/judge.rs` (`pack_judge_data`) | service | file-I/O | `lib.rs` `snapshot` (335+), `walk.rs` `collect` (89), `pack.rs` `tarball` (27) | role-match |
| `crates/minos-source/src/lib.rs` (`snapshot(.., judge_digests)` + `SnapshotError::JudgeOverlap`) | service | file-I/O | same file, `missing` check (342-347) | exact |
| `crates/minos-runner/clippy.toml`, `crates/minos-loop/clippy.toml` | config | n/a | `crates/minos-core/clippy.toml`, `crates/minos-propose/clippy.toml` | exact |
| `crates/minos-runner/src/{lib,error,map,usage,pipeline}.rs` (`Runner<B: Backend, S>`) | service | request-response | `minos-propose/src/proposer.rs` `Proposer<P>` (245-300); `minos-sandbox/tests/backend.rs` launch/wait | role-match |
| `crates/minos-runner/tests/*` (fake backend) | test | request-response | `crates/minos-sandbox/tests/backend.rs` `FakeBackend` (35-130) | exact |
| `crates/minos-loop/src/job.rs` (`JobRunner` seam) | trait | request-response | `minos-propose/src/provider.rs` `ModelProvider` + `ProposeError` blame split (proposer.rs 140-175) | role-match |
| `crates/minos-loop/src/{stage,admit,record,history,observe,inputs}.rs` | service (pure over fold) | transform | `minos-core/src/state.rs` fold + `policy.rs` | role-match |
| `crates/minos-loop/src/driver.rs` | service (orchestrator) | event-driven | none: first driver; nearest is `minos-cli/src/env_build.rs` numbered step flow (235-343) | partial |
| `crates/minos-loop/src/error.rs` | utility | n/a | `StoreError` (store lib.rs 145-236) + `ProposeError` | exact |
| `crates/minos-loop` test fakes (`FakeJobs`) | test | request-response | `crates/minos-propose/src/fake.rs` `FakeProvider` (1-152) | exact |
| `crates/minos-cli/src/judge_push.rs` | controller (command) | file-I/O | `crates/minos-cli/src/env_build.rs` (233-432) | exact |
| `crates/minos-cli/src/jobs.rs` (`RunnerJobs<B>` adapter) | adapter | request-response | `crates/minos-cli/src/blob_source.rs` (concrete-meets-trait adapter) | role-match |
| `crates/minos-cli/src/main.rs` (+ `judge push <dir> [--update]`) | controller | request-response | same file, `env build` subcommand (73-96) and `env_build_command` (115-150) | exact |
| `crates/minos-cli/tests/judge_push.rs` | test | file-I/O | `crates/minos-cli/tests/env_build.rs` | exact |
| `crates/minos-cli/tests/runner_vm.rs` (ignored VM test) | test (VM) | request-response | `crates/minos-backend-krun/tests/vm.rs` + `tests/harness/mod.rs` | exact (must copy; harness is test-private) |

## Pattern Assignments

### `crates/minos-core/src/lock.rs` - `[[judge]]` table (model, transform)

**Analog:** same file. Copy the `LockedImage` triple exactly: validated struct with private fields and `try_from`/`into` the raw struct; raw struct with `deny_unknown_fields`; `TryFrom` checks canonical order.

Validated + raw pair (lines 233-243, 314-324):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawLockedImage", into = "RawLockedImage")]
pub struct LockedImage {
    backend: BackendKind,
    arch: Arch,
    root_image: ImageDigest,
    ...
}

/// [`LockedImage`] as written: one `[[image]]` table.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLockedImage { ... }
```

Canonical-order check without indexing (lines 329-336) - reuse for the per-file `JudgeFile` list (sorted by path, no duplicates):
```rust
let canonical = raw
    .base_images
    .windows(2)
    .all(|pair| matches!(pair, [earlier, later] if earlier < later));
if !canonical {
    return Err(LockError::BaseImagesNotCanonical);
}
```

Backward-compatible array (lines 449-455) - add `judge` the same way so existing lock files parse:
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLock {
    v: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    image: Vec<RawLockedImage>,
}
```

Duplicate-key refusal into a `BTreeMap` (lines 462-471) - copy for `DuplicateJudge { label }`:
```rust
let mut images = BTreeMap::new();
for raw_image in raw.image {
    let image = LockedImage::try_from(raw_image)?;
    let key = (image.backend, image.arch);
    if images.insert(key, image).is_some() {
        return Err(LockError::DuplicateImage { backend: key.0, arch: key.1 });
    }
}
```

Lookup / replace / check (lines 410-447) - `judge(label)`, `with_judge(entry)`, and `check` gaining `LockMismatch::NoJudge { label }` mirror `image`, `with_image`, and the `ok_or(LockMismatch::NoImage { backend, arch })?` line. Raise `MAX_LOCK_BYTES` (Pitfall 5) and keep `parse_toml`'s pre-parse length check (391-397).

---

### `crates/minos-core/src/manifest.rs` - `[judge] trials` (model, transform)

**Analog:** `RawJudge` (line 1005) plus the optional-with-default budget fields (`max_empty_generations`, `max_infra_failures`). Add `trials: Option<u32>` to `RawJudge`; default 3 and range-check against `MAX_TRIALS` in `Manifest::try_from`, as the budget defaults are. Exclude it from the canonical judge spec that `JudgeVersion` hashes (the same exclusion already applied to `version` and `min_improvement`), and pin that with a golden-ID test.

---

### `crates/minos-core/src/ledger.rs` - optional fields (model, event-driven)

**Analog:** the `RawLock.image` attribute above. Every new field is
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
logs: Option<BTreeMap<Stage, BlobId>>,
```
so existing `"v":"1"` lines and their goldens are byte-identical (Pitfall 4). Event keys do not change (`Judged` stays keyed by `(exp, judge, trial)`). Add one golden case per new field to `tests/golden_wire.rs`.

---

### `crates/minos-core/tests/golden_wire.rs`, `tests/lock.rs` (test, golden)

**Analog:** `tests/golden_wire.rs`. Module doc (lines 7-11) states the rule: insta snapshot per golden line plus one SHA-256 pin over all bytes, `update: no`, snapshots created deliberately with `INSTA_UPDATE=always cargo test -p minos-core --test golden_wire`. Extend `all_golden_bytes_pinned` (line 761) by appending the new snapshot names to its documented `for n in ...` list and recomputing the literal outside the code under test. Imports (lines 13-35) show the fixture style: build values only through public constructors from fixed bytes.

---

### `crates/minos-core/src/diff/cumulative.rs` (model, transform)

**Analog:** `diff/compose.rs` `diff_trees` (line 65) and `compose` (line 99, `parent: Option<&Diff>`). `CumulativeDiff` is a private-field newtype whose only constructors are those two functions (change their return types), plus `Experiment`/`JobRequest::candidate` taking it. No wire change; the golden `job.rs` bytes must not move.

---

### `crates/minos-store/src/lib.rs` - `Conflict` becomes an error, `RunLock` (trait, CRUD)

**Analog:** same file.

Current `Appended` (lines 43-75): remove the `Conflict` variant and its arm in `seq()`; keep `#[must_use]`.

New `StoreError` variants follow the existing shape (lines 159-168):
```rust
/// A run's ledger is corrupt and is refused; it needs a human (04-CONTEXT D-13).
#[error("the ledger of run {run} is corrupt at line {line}: {kind}")]
Corrupt {
    /// The run.
    run: RunId,
    ...
},
```
`Conflict { seq }` and `RunLocked { run }` carry IDs and numbers only, never bytes. `is_retryable` (lines 232-235) stays `matches!(self, Self::Io { .. })`, so both are non-retryable automatically; add them to the `only_io_is_retryable` unit test (line 340).

`RunLock` trait sits beside `Ledger`/`Blobs` (lines 239-308): synchronous, `&self`, documented `# Errors`.

---

### `crates/minos-store/src/memory.rs` (store, CRUD)

**Analog:** same file. Conflict branch to change (append, lines 44-57):
```rust
if let Some(existing) = lines.iter().find(|line| line.key() == &key) {
    let seq = existing.seq();
    return Ok(if existing.record() == &record {
        Appended::Duplicate { seq }
    } else {
        Appended::Conflict { seq }   // becomes: return Err(StoreError::Conflict { seq })
    });
}
```
Poison handling for the new `Mutex<BTreeSet<RunId>>` run lock (lines 32-35):
```rust
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, StoreError> {
    mutex.lock().map_err(|_poisoned| StoreError::Poisoned)
}
```
The guard holds `&Mutex<BTreeSet<RunId>>` and the `RunId`, and removes it in `Drop` (a poisoned lock in `Drop` is ignored, never a panic).

---

### `crates/minos-store/src/file/ledger.rs` (store, file-I/O)

**Analog:** same file, `append` lines 210-217 (identical branch to memory.rs above). Nothing else in the append path changes: the lock is taken before the key check, so the first write still wins.

---

### `crates/minos-store/src/file/run_lock.rs` (store, file-I/O)

**Analog:** `file/ledger.rs` lines 170-184 and `file/mod.rs` `open_or_create` (lines 218-242).
```rust
/// How the lock file is opened: read and write so `flock` works on every platform, never truncated.
fn lock_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).truncate(false);
    options
}

impl FileStore {
    fn open_lock(&self, run: &RunId) -> Result<File, StoreError> {
        open_or_create(&self.lock_path(run), "ledger lock file", &lock_options())
            .map(|(file, _created)| file)
    }
}
```
Add `driver_lock_path` beside `lock_path` in `file/mod.rs` (line 106; `LOCK_FILE` const at line 51 shows the naming). Create the run directory first with `ensure_dir` exactly as `append` does (lines 189-192), then `file.try_lock()`; map `TryLockError::WouldBlock` to `RunLocked { run }` and `TryLockError::Error(e)` through `io_error("locking the run")`. Keep the `File` inside the guard. Never take `ledger.lock` and `driver.lock` in a way that holds `ledger.lock` across a VM run (Pitfall 11).

---

### `crates/minos-store/tests/contract.rs` (test, contract)

**Analog:** same file. Every case is generic over the trait and expanded for both stores by the macro (lines 286-304):
```rust
macro_rules! ledger_contract {
    ($case:ident) => {
        mod $case {
            #[test]
            fn memory() -> super::TestResult {
                super::$case(&minos_store::MemoryStore::new())
            }

            #[test]
            fn file() -> super::TestResult {
                let dir = tempfile::tempdir()?;
                super::$case(&minos_store::FileStore::open(dir.path())?)
            }
        }
    };
}
```
Rewrite `conflicting_append_is_not_written` (lines 88-106) to `assert!(matches!(ledger.append(..), Err(StoreError::Conflict { seq: 0 })))` and keep its `after == before` check. Add a `run_lock_contract!` with "second lock refused" and "lock after drop succeeds".

---

### `crates/minos-sandbox/src/untrusted.rs` - capped sink exits (hostile wrapper; pending user approval)

**Analog:** same file, the existing exits. The pattern: a method on one role-specific `impl` block, consuming `self`, never returning bytes or implementing `Debug`/`Display`/`Deref`/`Clone`.
```rust
impl<R: Role> Untrusted<OutputFile<R>> {
    /// The sanitized tail of the file as a log excerpt, through [`LogExcerpt::from_bytes`]: ...
    #[must_use]
    pub fn into_log_excerpt(self, max: usize) -> LogExcerpt {
        LogExcerpt::from_bytes(&self.0.bytes, max)
    }
}
```
`store_tail` goes in this same `impl<R: Role>` block; `store_judge_input` goes in a new `impl Untrusted<GuestOutput<Candidate>>` block, mirroring the judge-only block at line 273 (`impl Untrusted<GuestOutput<Judge>>` holding `into_scorecard`). Add trybuild cases under `tests/ui/` (pattern of the existing "candidate output has no `into_scorecard`" case): judge output has no `store_judge_input`. Do not implement until the user approves (07-RESEARCH Finding 1).

---

### `crates/minos-source/src/judge.rs` and `lib.rs` overlap refusal (service, file-I/O)

**Analog:** `snapshot` (lib.rs 335-347), `walk::collect` (walk.rs 89), `pack::tarball` (pack.rs 27).
```rust
pub fn snapshot(root: &Path, manifest: &Manifest, policy: CleanPolicy) -> Result<Snapshot, SnapshotError> {
    let collected = walk::collect(root, manifest)?;

    let missing: BTreeSet<&RelPath> = manifest
        .writable()
        .iter()
        .chain(manifest.readable())
        .chain([manifest.program()])
        .filter(|path| !collected.files.contains_key(*path))
        ...
```
The `JudgeOverlap` refusal copies this collect-every-offender-into-a-`BTreeSet` shape, then one error naming all paths. `pack_judge_data` reuses `pack::tarball` and the denylist and secret scan, but not `snapshot()` itself (Pitfall 6: `walk::collect` takes a `Manifest` and applies surface/program rules that do not fit a judge directory; factor a manifest-free walk entry or a judge-specific walker). Refuse over `MAX_JUDGE_DATA_BYTES`, never truncate.

---

### `crates/minos-runner/clippy.toml`, `crates/minos-loop/clippy.toml` (config)

**Analog:** `crates/minos-core/clippy.toml` (header comment and entry form) and `crates/minos-propose/clippy.toml`:
```toml
# Replaces (does not merge with) any ancestor clippy.toml. Clippy uses the first file found
# walking up from CARGO_MANIFEST_DIR.
# Every path below is a real std path, proven to fire once (...).
# A mistyped path only warns, so re-prove any entry you change.
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "..." },
  { path = "std::env::var",              reason = "..." },
]
disallowed-types = [
  { path = "std::collections::HashMap",  reason = "order must be stable: use BTreeMap" },
  { path = "std::process::Command",      reason = "..." },
]
```
Each entry must be proven to fire once and recorded in phase evidence, as 06-EVIDENCE did. The loop adds `std::fs::File`. Note the pause in D-16 needs a clock-free seam (injectable pause), so `std::thread::sleep` is banned in the loop and lives behind a trait the CLI implements.

---

### `crates/minos-runner/src/*` - `Runner<B: Backend, S>` (service, request-response)

**Analog:** `minos-propose/src/proposer.rs` `Proposer<P>` (lines 245-300) for the generic-not-trait-object shape:
```rust
/// Generic over the provider, exactly as the runner is generic over its sandbox backend, and never a trait object:
/// each binary picks its provider concretely ...
#[derive(Debug, Clone)]
pub struct Proposer<P> {
    provider: Option<P>,
}
```
and `minos-sandbox/tests/backend.rs` for the launch sequence (`SandboxSpec::new` -> `seal` -> `preflight` -> `backend.launch(cleared)` -> `running.wait()` -> `Finished`). Map `(Ended, GuestStatus, faults)` to `Outcome` in `map.rs` as a pure, exhaustively tested function (07-RESEARCH Pattern 2), following the style of `minos-backend-krun/src/supervise.rs` `classify`. Errors split blame per `ProposeError`'s doc (proposer.rs 138-143): an `is_infra`-style predicate, everything else non-retried.

---

### `crates/minos-runner/tests/*` (test, fake backend)

**Analog:** `crates/minos-sandbox/tests/backend.rs` lines 35-130. Copy `FakeBackend` (configurable `ended`, `stream`, `elapsed`, `console`, `launched: Mutex<Vec<Launched>>`) and `FakeRunning<R>`:
```rust
impl<R: Role> RunningVm<R> for FakeRunning<R> {
    fn wait(self) -> Result<Finished<R>, InfraError> {
        Ok(Finished::new(self.cleared, self.ended, self.stream, self.elapsed, self.console))
    }
}
```
Poisoned-lock handling in test code (lines 77-84):
```rust
let guard = self
    .launched
    .lock()
    .map_err(|_| "the launch record lock is poisoned")?;
```
For a runner fake, make the script a `Mutex<VecDeque<...>>` keyed per launch so candidate and judge launches can return different streams (FakeProvider's script shape below).

---

### `crates/minos-loop/src/job.rs` + loop test fakes (trait + test, request-response)

**Analog:** `crates/minos-propose/src/fake.rs` (full file, 152 lines). Copy its structure for `FakeJobs`: a scripted `Mutex<VecDeque<Reply>>`, a `Mutex<Vec<RecordedRequest>>`, an exhausted script or poisoned lock mapping to an infrastructure error, and a hand-written `Debug` that prints sizes only.
```rust
const fn fake_failure() -> ProviderError {
    ProviderError::Infra { cause: InfraCause::Fake, usage: None }
}

fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, ProviderError> {
    mutex.lock().map_err(|_poisoned| fake_failure())
}

impl ModelProvider for FakeProvider {
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ProviderError> {
        lock(&self.requests)?.push(RecordedRequest { ... });
        let reply = lock(&self.script)?.pop_front().ok_or_else(fake_failure)?;
        match reply { ... }
    }
}
```
Loop tests combine `FakeJobs`, `FakeProvider`, and `MemoryStore` (LOOP-09). The resume equivalence test (LOOP-05) folds both ledgers with `minos_core::state::fold` (state.rs 307) and compares.

---

### `crates/minos-loop/src/{stage,admit,record,history,observe,inputs}.rs` (pure over the fold)

**Analog:** `minos-core/src/state.rs` accessors (`proposals_in` 629, `trials` 639, `decision` 652, `stop_requested` 705) and `policy.rs` (`conclude` 337, `circuit_breaker(state, budget, infra_streak)` 517, `spend_ok` 570). Stage selection is a pure function of `RunState`, like these: no I/O, arguments in, value out, `BTreeMap`/`BTreeSet` for any ordered output. Put accessors the stage needs (`judged`, `has_spend`, `next_model_call`) in `state.rs`, not in the loop.

---

### `crates/minos-loop/src/driver.rs` (orchestrator, event-driven) - partial analog

No driver exists yet. Use `env_build.rs` lines 235-343 for the numbered-step style (each step a commented block, each failure mapped to one typed error variant with `map_err`, side effects only after validation). The loop holds the run guard from `RunLock` before its first ledger read and for the whole session (07-RESEARCH Pattern 7), and stops on `StoreError::Conflict` naming run, seq, and event kind (D-08). Follow 07-RESEARCH Patterns 5, 6, 8, and 9 for the stage logic.

---

### `crates/minos-cli/src/judge_push.rs` (command, file-I/O)

**Analog:** `crates/minos-cli/src/env_build.rs`. Copy, in order:

1. Manifest read (lines 241-247):
```rust
let manifest_text = read_text(&project.join(MANIFEST_FILE), MANIFEST_FILE, MAX_MANIFEST_BYTES)?;
let manifest = Manifest::parse_toml(&manifest_text).map_err(EnvBuildError::Manifest)?;
```
2. Store the packed bytes with `put_reader` under a cap (lines 296-300):
```rust
let store = FileStore::open(&project.join(STORE_DIR)).map_err(EnvBuildError::Store)?;
let blob = store
    .put_reader(packed.bytes(), MAX_ROOT_IMAGE_BYTES)
    .map_err(EnvBuildError::Store)?;
```
3. Lock read-compare-write with the `--update` gate, blob first, lock last (lines 313-338):
```rust
let lock = read_lock(&lock_path)?;
let lock_written = match lock.as_ref().and_then(|lock| lock.image(BACKEND, ARCH)) {
    Some(locked) if *locked == entry => false,
    Some(locked) if *locked.root_image() != image && !options.update => {
        return Err(EnvBuildError::LockMismatch { locked: *locked.root_image(), built: image });
    }
    _ => {
        let text = lock
            .unwrap_or_else(Lock::empty)
            .with_image(entry)
            .to_toml()
            .map_err(EnvBuildError::Lock)?;
        write_lock(&project.join(STORE_DIR), &lock_path, text.as_bytes())
            .map_err(EnvBuildError::LockWrite)?;
        true
    }
};
```
For judge push the key is `lock.judge(manifest.judge().version())` and the mismatch error prints both data tree digests (D-01).
4. `read_lock` (393-415) and `write_lock` (421-432, temp in `.minos/`, `sync_all`, `persist`, fsync the directory) are private to `env_build.rs`. Move them to a shared CLI module (for example `lock_file.rs`) and call them from both commands rather than duplicating them.
5. Error enum: mirror `EnvBuildError::LockMismatch` (lines 215-224) with its message ending in "rerun with --update to accept ...".

---

### `crates/minos-cli/src/main.rs` (command dispatch)

**Analog:** same file. Builder-API clap only (no derive). Subcommand shape (lines 73-96):
```rust
.subcommand(
    Command::new("env")
        .about("Build the project's environment")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("build")
                .about("build the root image from the Dockerfile and write minos.lock")
                .arg(
                    Arg::new("update")
                        .long("update")
                        .action(ArgAction::SetTrue)
                        .help("accept a root image whose digest differs from minos.lock"),
                ),
        ),
)
```
Dispatch `Some(("judge", judge)) => match judge.subcommand() { Some(("push", push)) => judge_push_command(push), _ => Err(anyhow!("unknown judge command")) }` as at lines 107-110, and the handler follows `env_build_command` (115-150): `env::current_dir().context(...)`, call the library function, print a short report. The existing `command_tree_is_well_formed` test (line 181) covers the new subcommand.

---

### `crates/minos-cli/src/jobs.rs` (adapter)

**Analog:** `crates/minos-cli/src/blob_source.rs`, the existing place where a concrete store meets a trait from another crate (`FileBlobSource` implements `minos_sandbox::BlobSource` over `FileStore`). `RunnerJobs<B>` wraps `minos_runner::Runner<B, FileStore>` and implements `minos_loop::JobRunner`, mapping the runner's error to the loop's blame split. The CLI is the only crate that names `KrunBackend`.

---

### `crates/minos-cli/tests/judge_push.rs` (test)

**Analog:** `crates/minos-cli/tests/env_build.rs` (temp project directory, call the library function, assert on the report and the lock file). Cases: first push writes, same push unchanged, different data refused without `--update` naming both digests, accepted with it, over-cap refused, secret-scan hit refused.

---

### `crates/minos-cli/tests/runner_vm.rs` (ignored VM test)

**Analog:** `crates/minos-backend-krun/tests/vm.rs` (lines 1-50) and `tests/harness/mod.rs`. Copy the module doc that names the required variables and the exact run command, `#[ignore]` on every test, and the env lookup that errors instead of panicking (harness line 93-94):
```rust
pub fn env_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {SETUP}"))?;
```
The harness's `test_rootfs` (line 213) and `vm()` (line 243) are test-private, so rebuild the fixture image with the public `minos_backend_krun::rootfs::{pack_root_image, check_guest_elf, PackLimits}` (Pitfall 13). If a `harness` module is shared by more than one target, every item must be used by each target or `-D warnings` fails on dead code. The probe needs a `scorecard <value>` form to produce `Scored` (Pitfall 13).

## Shared Patterns

### Validated wire types
**Source:** `crates/minos-core/src/lock.rs` lines 233-243, 314-347
**Apply to:** every new or changed type in `lock.rs`, `manifest.rs`, `ledger.rs`, `job.rs`
Private fields, `#[serde(try_from = "Raw..", into = "Raw..")]`, raw struct with `#[serde(deny_unknown_fields)]`, `#[must_use]` accessors, constructors that sort and dedup so there is one wire form.

### Backward-compatible additions
**Source:** `RawLock.image` (lock.rs 452-454)
**Apply to:** `Judged.logs`, `RunStarted.root_image`, `Proposed.batch`, `RawLock.judge`
`#[serde(default, skip_serializing_if = "...")]`, so old lines and old goldens keep their bytes.

### Errors that separate blame
**Source:** `StoreError` + `is_retryable` (store lib.rs 145-236); `ProposeError` doc (proposer.rs 138-143)
**Apply to:** `minos-runner/src/error.rs`, `minos-loop/src/error.rs`, the `JobRunner` seam
`#[non_exhaustive] #[derive(Debug, thiserror::Error)]`, fixed `&'static str` op text, IDs and numbers only, never hostile bytes; one predicate decides retry.

### Mutex poisoning fails closed
**Source:** `memory.rs` lines 32-35; `fake.rs` lines 112-121
**Apply to:** `MemoryStore` run lock, `FakeJobs`, any fake backend
Map `PoisonError` to a typed error; never `unwrap`.

### Golden bytes
**Source:** `crates/minos-core/tests/golden_wire.rs` lines 1-11 and 761+
**Apply to:** ledger and lock additions, the `store_judge_input` repack bytes, the judge tarball
insta snapshot plus one external SHA-256 pin; never auto-accept.

### Two-implementation contract tests
**Source:** `crates/minos-store/tests/contract.rs` lines 286-321
**Apply to:** `Conflict` and `RunLock` cases

### Atomic file replace
**Source:** `env_build.rs` `write_lock` lines 421-432
**Apply to:** `judge push` lock write

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/minos-loop/src/driver.rs` | orchestrator | event-driven | No generation driver exists; use 07-RESEARCH Patterns 5-9 with the step style of `env_build.rs` |
| injectable pause seam (D-16) | trait | n/a | No clock seam exists outside core; design per 07-RESEARCH Pattern 9 |

## Metadata

**Analog search scope:** `crates/minos-core`, `minos-store`, `minos-sandbox`, `minos-source`, `minos-propose`, `minos-backend-krun/tests`, `minos-cli`
**Files scanned:** about 25
**Pattern extraction date:** 2026-10-22
