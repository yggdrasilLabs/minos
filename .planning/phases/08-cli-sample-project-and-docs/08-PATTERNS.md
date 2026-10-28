# Phase 8: CLI, Sample Project, and Docs - Pattern Map

**Mapped:** 2026-10-28
**Files analyzed:** 33 (new and modified)
**Analogs found:** 29 / 33

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/minos-cli/src/main.rs` (modify: 9 subcommands, G9 help text) | controller (clap tree + dispatch) | request-response | itself, lines 66-245 (`cli()`, `run()`, `env_build_command`, `judge_push_command`) | exact |
| `crates/minos-cli/src/lib.rs` (modify: new `pub mod`s) | config | n/a | itself, lines 1-29 | exact |
| `crates/minos-cli/src/project.rs` | service (manifest, lock, prices, snapshot, `RunInputs`) | file-I/O | `crates/minos-cli/src/judge_push.rs` (manifest read under cap, lock read) + `crates/minos-cli/tests/runner_vm.rs:330-352` (snapshot + store base) | role-match |
| `crates/minos-cli/src/address.rs` | utility (run and experiment prefix resolution) | transform | `crates/minos-cli/src/locate.rs` (pure lookup, no I/O beyond reads) | partial |
| `crates/minos-cli/src/render.rs` (Observer impl, 12-hex IDs, progress line) | component | event-driven | `crates/minos-loop/src/observe.rs:20-36` (`Observer`, `NoopObserver`) | role-match |
| `crates/minos-cli/src/check.rs`, `run.rs` | service (drives `Driver`) | request-response / batch | `crates/minos-cli/tests/runner_vm.rs` composition + `crates/minos-cli/src/jobs.rs` | role-match |
| `crates/minos-cli/src/status.rs`, `log.rs`, `show.rs`, `report.rs` | service (read-only fold + render) | file-I/O read | `crates/minos-cli/src/judge_push.rs` structure; data access via `Ledger::read` | role-match |
| `crates/minos-cli/src/stop.rs` | service (append `StopRequested`) | CRUD append | `crates/minos-loop` `record::append` usage; `lock_run` probe from `crates/minos-loop/tests/driver/lock.rs` | partial |
| `crates/minos-cli/src/adopt.rs` | service (hash gate + `create_new` patch) | file-I/O write | `crates/minos-cli/src/lock_file.rs` (synced write) + store's `create_new` rule | partial |
| `crates/minos-cli/src/init.rs` | service (scaffold, `create_new`) | file-I/O write | `crates/minos-cli/src/lock_file.rs` | partial |
| `crates/minos-cli/tests/progress_golden.rs` | test | transform | `crates/minos-cli/tests/judge_push.rs` (TestResult style) + insta golden pattern in core | role-match |
| `crates/minos-cli/tests/patch_tools.rs` | test (execs `git apply`, `patch -p1`) | file-I/O | `crates/minos-cli/tests/judge_push.rs` (uses `std::process::Command` + `TempDir`) | role-match |
| `crates/minos-cli/tests/sample_manifest.rs` | test | file-I/O | `crates/minos-cli/tests/judge_push.rs` | role-match |
| `crates/minos-cli/tests/docs_words.rs` | test | file-I/O | none close | no analog |
| `crates/minos-cli/tests/sample_vm.rs` (ignored) | test (VM e2e) | batch | `crates/minos-cli/tests/runner_vm.rs` | exact |
| `crates/minos-cli/tests/{check,run,status,stop,adopt,...}.rs` | test | request-response | `crates/minos-cli/tests/judge_push.rs` | exact |
| G1 `crates/minos-core/src/money.rs` (`ApproxCost` + tokens) | model | transform | itself, lines 519-552 | exact |
| G2 `crates/minos-store/src/lib.rs` + `file/ledger.rs` + `memory.rs` (`Ledger::runs`) | store trait + impls | CRUD read | `Ledger::read` (`lib.rs:280`, `file/ledger.rs:246`, `memory.rs:81-84`) | exact |
| G2 test `crates/minos-store/tests/contract.rs` | test | CRUD | itself, `runs_are_independent` line 115, `unknown_run_reads_empty` line 137 | exact |
| G3 `crates/minos-source` (`Snapshot::surface`, `read_files`) | utility | transform | `minos_source::snapshot` / `pack_judge_data`; host-side tar reader in `minos-sandbox` `Finished::into_output` | role-match |
| G4 `crates/minos-core/src/state.rs` (`RunState::proposals`) | model | transform | existing `proposals_in` in the same file | exact |
| G5 percent delta | component | event-driven | `crates/minos-loop/src/observe.rs:61-79` + `policy::reduce_trials` | role-match |
| G6 `crates/minos-core/src/policy.rs` (`run_cost`) | model | transform | `spend_ok`, `policy.rs:609-633` (extract) | exact |
| G7 prices path | utility | file-I/O | `judge_push.rs` manifest read under `MAX_MANIFEST_BYTES` | role-match |
| G8 `crates/minos-core/src/limits.rs` constants | config | n/a | existing consts in `limits.rs` | exact |
| G9 `ENV_BUILD_HELP` | config | n/a | `main.rs:31-45` | exact |
| `examples/logparse/{minos.toml,program.md,minos.Dockerfile,.gitignore,Cargo.toml,Cargo.lock,src/*,data/*}` | config + sample crate | batch | `MANIFEST` const in `crates/minos-cli/tests/judge_push.rs:25-60` (same `logparse` shape) | role-match |
| `examples/logparse/judge/{Cargo.toml,Cargo.lock,src/main.rs,answers.tsv}` | sample judge binary | file-I/O | none in repo (scorecard format in core `metric.rs`) | no analog |
| `README.md`, `SECURITY.md` | docs | n/a | `CLAUDE.md` (source of truth) | no analog |

## Pattern Assignments

### `crates/minos-cli/src/main.rs` (controller, request-response)

**Analog:** itself.

**Imports and error boundary** (lines 17-27, 64-74): `anyhow` only here; every error printed through `SafeText`:
```rust
fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("minos: error: {}", SafeText::new(&format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}
```

**Subcommand with flags** (lines 91-112): builder API, `Arg::new(..).long(..).action(ArgAction::SetTrue).help(..)`, `.long_about(CONST_HELP)`; positional with `value_parser!(PathBuf)` (lines 123-130). Add `check`, `run --generations N` (`value_parser!(u32)`), `stop <run>`, `status`, `log`, `show <exp>`, `report <exp>`, `adopt <exp> --reviewed <hash>`, `init --kind`.

**Dispatch** (lines 145-165): nested `match matches.subcommand()` with `Some(("name", m)) => fn_command(m)` and an `_ => Err(anyhow!(...))` fallback.

**Command body** (lines 169-224): `env::current_dir().context(..)`, build options struct with `::new(flags)`, call the library fn, print each field with `println!`, special-case one typed error variant to a fixed multi-line stderr message and `ExitCode::from(1)`, else `Err(error.into())`. Untrusted text goes through `SafeText::inline` (line 216).

**Tests** (lines 260-278): `cli().debug_assert()` and `try_get_matches_from` positive/negative pairs, no unwrap (`assert!(parsed.is_ok(), "{parsed:?}")`).

---

### Library command modules (`check.rs`, `run.rs`, `status.rs`, `log.rs`, `show.rs`, `report.rs`, `stop.rs`, `adopt.rs`, `init.rs`, `project.rs`)

**Analog:** `crates/minos-cli/src/judge_push.rs`.

- **Module doc** (lines 1-30): numbered list of pipeline steps in order, each citing its decision ID. Copy that form.
- **Imports** (lines 32-47): `minos_core::{...}` by module path, `minos_store::{Blobs, FileStore, STORE_DIR, StoreError}`, sibling `crate::env_build::{LOCK_FILE, MANIFEST_FILE}` and `crate::lock_file::{read_lock, write_lock}`. Reuse those consts; do not redefine file names.
- **Options type** (lines 49-62): `#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)] pub struct XOptions { flag: bool }` with `pub const fn new(..)`.
- **Report type** (lines 64-80+): private fields, `#[must_use] pub const fn` getters; `main.rs` prints from getters only.
- **Error enum** (lines 122-150+): `thiserror`, fixed text with `&'static str` `what`, numeric caps, `#[source]` inner errors, never file or model bytes:
```rust
#[error("could not read {what}")]
Read { what: &'static str, #[source] source: io::Error },
#[error("{what} is over {max} bytes")]
TooLarge { what: &'static str, max: u64 },
#[error("minos.toml is refused")]
Manifest(#[source] ManifestError),
```
  Errors for `check`/`run` should name the next command (`run minos env build`, `run minos judge push <dir>`), per RESEARCH Pattern 2.

### `project.rs` (service, file-I/O) and `check.rs`/`run.rs`

**Analog:** `crates/minos-cli/tests/runner_vm.rs:330-352` (the production composition):
```rust
let store = FileStore::open(&project.join(STORE_DIR))?;
let snap = snapshot(&project, &manifest, CleanPolicy::AllowDirty, &lock.judge_file_digests())?;
let tree = snap.tree();
if store.put(snap.tarball())? != tree.to_blob_id() {
    return Err("the base blob is not stored under its tree ID".into());
}
let base = BaseTree::new(tree, surface);   // surface from G3 Snapshot::surface, not hand-built
let backend = KrunBackend::new(HostPaths::new(vmm, guest_path, krunfw), FileBlobSource::open(&project)?)?;
```
Then `RunnerJobs` (`crates/minos-cli/src/jobs.rs`, `judge_inputs` at line 70) and `Driver::start|resume` + `calibrate|run_to` (RESEARCH Pattern 3). Keep commands generic over `P: ModelProvider, B: Backend` (RESEARCH Pattern 1) so `sample_vm.rs` injects `Proposer::without_provider()` or `FakeProvider`. In production `CleanPolicy` comes from `--allow-dirty`, never hard-coded `AllowDirty`.

### `render.rs` (component, event-driven)

**Analog:** `crates/minos-loop/src/observe.rs:20-36, 40-90`. Implement `Observer::observe(&mut self, event: &LoopEvent)` with a `match` over `Calibrated`, `Proposed`, `Judged { generation, exp, trial, tier, outcome, delta }`, `Selected`, `Retrying { streak, max }`, `Stopped`. Writer is `&mut impl Write`; the observer may hold `&FileStore` to re-fold for G5's percent delta. Cost only via `ApproxCost` (G1).

### `stop.rs`

`lock_run` probe pattern: `crates/minos-loop/tests/driver/lock.rs` and store `RunLock` (`crates/minos-store/src/lib.rs:307`); a held run is `StoreError::RunLocked`. Resolve the prefix through G2 first (Pitfall 8: `lock_run` creates directories). Ordinal from `RunState::next_user_stop_ordinal`; report `Appended::Duplicate` as already recorded.

### Tests for commands (`crates/minos-cli/tests/*.rs`)

**Analog:** `crates/minos-cli/tests/judge_push.rs:1-60`: module doc listing every case; `type TestResult = Result<(), Box<dyn Error>>;`; `tempfile::TempDir` project; an inline `MANIFEST` const; `?` everywhere, no unwrap. For loop-driving commands without VMs, reuse the fake pattern of `crates/minos-loop/tests/driver/support.rs`: `FakeJobs::new(closure)` (line 240-295), `scored(request, throughput)` (328), `deterministic` (362), `backend()` (307).

### `sample_vm.rs` (ignored VM e2e)

**Analog:** `crates/minos-cli/tests/runner_vm.rs`. Copy the `#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST ..., MINOS_KRUNFW; see SETUP"]` attribute (line 492) and its setup; per CLAUDE.md it copies the harness text rather than importing it.

### G1 `money.rs`

**Analog:** `crates/minos-core/src/money.rs:519-552`. Add a companion type whose `Display` delegates to `ApproxCost`'s (`write!(f, "{} ...", self.cost)`), keep the half-up cents rule, update the "COST-02 adds token counts" doc line.

### G2 `Ledger::runs`

**Analog:** `Ledger::read` across `crates/minos-store/src/lib.rs:280`, `file/ledger.rs:246`, `memory.rs:81-84`; path helper `file/mod.rs:102-105` (`run_dir`). File impl: list `root/runs`, `symlink_metadata` each entry, parse names as 64-hex `RunId`, refuse anything else as `StoreError::Unsafe`/`Corrupt`; return `BTreeSet<RunId>`. Contract test: add a generic `fn runs_lists_appended<L: Ledger>(ledger: &L) -> TestResult` next to `runs_are_independent` (contract.rs:115) and register it for both stores.

### G6 `run_cost`

Extract the body of `spend_ok` (`crates/minos-core/src/policy.rs:609-633`) into `pub fn run_cost`, and have `spend_ok` call it; existing policy tests must stay unchanged.

### `examples/logparse/minos.toml`

Start from the `MANIFEST` const in `crates/minos-cli/tests/judge_push.rs:25-60`, then apply RESEARCH Pattern 5 (judge argv `["cargo","run","--release","--frozen","--quiet","--target-dir","/tmp/judge-target"]`, `exclude = ["judge/", "target/"]`) and Pattern 6 commands. Dockerfile verbatim from RESEARCH Pattern 6. Not a workspace member (Anti-Patterns).

## Shared Patterns

### Untrusted text
**Source:** `crates/minos-cli/src/main.rs:70, 216, 233` (`SafeText::new`, `SafeText::inline`). **Apply to:** every printed path, rationale, log excerpt, error chain.

### Errors separate blame, fixed text
**Source:** `crates/minos-cli/src/judge_push.rs:122-150`. **Apply to:** every new library module.

### Lock and manifest file access
**Source:** `crate::lock_file::{read_lock, write_lock}`, `crate::env_build::{LOCK_FILE, MANIFEST_FILE}`, manifest read under `MAX_MANIFEST_BYTES` (judge_push.rs step 1). **Apply to:** `project.rs`, `check`, `run`, G7 prices read.

### Test style
**Source:** `crates/minos-cli/tests/judge_push.rs:23` `type TestResult`; no unwrap/expect/indexing/allow; no `#[allow]` anywhere (C-02 grep).

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `examples/logparse/judge/src/main.rs` | sample judge | file-I/O | No judge program exists in repo; follow scorecard wire form `{"v":1,"metrics":[...]}` (CLAUDE.md Judge) and RESEARCH Pattern 5 paths (`/scratch/candidate/out/`, `/out/scorecard.json`) |
| `crates/minos-cli/tests/docs_words.rs` | test | file-I/O | No doc-lint test exists; plain `fs::read_to_string` + banned-word list |
| `README.md`, `SECURITY.md` | docs | n/a | Derive from CLAUDE.md; no overclaiming |
| `crates/minos-cli/src/address.rs` | utility | transform | No prefix resolver exists; build on G2 + G4 |

## Metadata

**Analog search scope:** `crates/minos-cli/{src,tests}`, `crates/minos-store/{src,tests}`, `crates/minos-loop/{src,tests/driver}`, `crates/minos-core/src/money.rs`
**Files scanned:** ~20
**Pattern extraction date:** 2026-10-28
