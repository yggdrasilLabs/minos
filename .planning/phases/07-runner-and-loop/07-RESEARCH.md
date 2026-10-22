# Phase 7: Runner and Loop - Research

**Researched:** 2026-10-22
**Domain:** `minos-runner` (candidate VM then judge VM per `JobRequest`, `Untrusted` output to `Outcome`), `minos-loop` (fold-driven generation driver, resumable, locked, capped), `minos judge push`, and the core, store, sandbox, and source changes the locked decisions name (synchronous Rust, no new crates)
**Confidence:** HIGH for current signatures and mechanisms (read from source; three run probes); MEDIUM for the API shapes this document recommends (discretion areas); several locked decisions need a user answer before planning (see Summary)

## Summary

Phase 7 is mostly composition. Every pure decision the loop makes already exists in `minos-core` (`fold`, `route`, `conclude`, `circuit_breaker`, `spend_ok`, `check_scorecard`, `noise_floor`). The runner's whole vocabulary already exists in `minos-sandbox` (`SandboxSpec::new`, `secrets`, `seal`, `preflight`, `Backend::launch`, `RunningVm::wait`, `Finished::into_output`, the role-typed `Untrusted` exits). The propose step exists in `minos-propose` (`Proposer<P>::propose`, `FakeProvider`). No new external crate is needed: every dependency is an existing workspace pin. The loop's stage is a pure function of the fold (D-07). Model it as a pure `next_step(&RunState, ..) -> Step` function, so resume is "fold, then ask what to do next", which is also the discipline the AWS orchestrator will need under replay.

Reading the current source against the locked decisions found gaps the planner cannot route around. Five need a user answer or a confirmed interpretation before plans are written:

1. **The judge has no way to receive the candidate's output, and D-09's raw logs cannot be stored.** `Untrusted<GuestOutput<R>>` has exactly four exits: `take_status`, `take`, `into_diff_texts`, and `into_scorecard` (judge only). `Untrusted<OutputFile<R>>` has one: `into_log_excerpt(max)`, a sanitized excerpt of at most 4096 bytes. `Finished`'s stream is private. Nothing lets the runner turn candidate artifacts into the `candidate_out` blob that `InputDisk::judge(job, candidate_out)` needs. So LOOP-01 cannot work, and no raw stage log can become a `Judged { logs }` blob. Both need a new capped exit on `Untrusted`. CLAUDE.md says to stop and ask before changing that type. Pattern 4 proposes the narrowest exits: a canonical repack of the candidate's `out/` files only, and a capped log tail, each handed to a blob-put closure and returning only a `BlobId`. The user must approve their exact signatures. [VERIFIED: `crates/minos-sandbox/src/untrusted.rs`, `finished.rs`, `spec.rs`; `crates/minos-core/src/guest.rs` `MAX_CANDIDATE_OUT_BYTES` "the candidate's whole output stream, as the judge receives it"]
2. **D-07 as written cannot meet LOOP-05 at every crash point.** `Decision::dropped` comes from the propose step and is written only in `Selected`. A crash between the first `Proposed` and `Selected` loses it, and D-07 forbids re-proposing. Proposals are also appended one line at a time, so a crash partway through a batch leaves a partial batch that the resumed run will never complete. An uninterrupted run would have had the whole batch. For the sweep tier this is fixable without a ledger change, because the sweep is deterministic (seeded by run and generation), so the loop can recompute the batch. For the model tier it needs one additive `Option` field on `Proposed` naming a "batch blob" (Pattern 6), or an agreed, documented exception. [VERIFIED: `crates/minos-core/src/ledger.rs` `Decision`, `crates/minos-core/src/policy.rs` `conclude(.., dropped)`, `crates/minos-propose/src/sweep.rs`]
3. **`StopRequested` is permanent in the ledger, so D-14's "any StopRequested" check would block every resume.** `RunState::stop_requested()` returns the earliest stop ever recorded. A run stopped by `minos stop`, `ProviderLimit`, or `ProviderAuth` could never be resumed after the user logs in again or the limit resets. Recommended semantics: the driver records which stops existed when its session started and exits only on a stop added during the session. Breaker and cap conditions are re-evaluated from the ledger at every safe point (Pattern 8). [VERIFIED: `crates/minos-core/src/state.rs` `stop_requested`; ledger key table "a resumed run can stop again later"]
4. **D-02's per-file judge digest list does not fit `minos.lock` as built.** `MAX_LOCK_BYTES` is 65,536. A probe of the `toml` 1.1.8 output puts each `[[judge.file]]` entry at about 100 bytes plus the path, so roughly 550 files fill the lock, while judge data may hold 10,000 files. Either the cap rises (to about 4 MiB) or a judge file-count cap is lowered. Separately, a plain SHA-256 of a low-entropy judge file (for example an expected output of `1\n`) can be reversed by guessing, so "committing it reveals nothing" (D-02) overclaims for the per-file list. [VERIFIED: `crates/minos-core/src/lock.rs` `MAX_LOCK_BYTES`; toml probe in this session]
5. **`minos_source::snapshot` cannot be reused unchanged for judge data (D-03).** It takes a `Manifest` and refuses a tree missing the `[surface]` and program paths. Its walk also drops every path the judge argv names, which would drop the judge program from its own data. Its caps (16 MiB per file, 64 MiB total) are below the 96 MiB judge cap. `judge push` needs a sibling entry point that reuses the same walker, packer, denylist, and scan, but with judge rules and caps (Pattern 11). [VERIFIED: `crates/minos-source/src/lib.rs` `snapshot`, `walk.rs` caps and `keep`]

Smaller findings also change what a plan must contain:

- `JobResult` carries one `Usage`, but the frozen event set has separate `Spent { Candidate }` and `Spent { Judge }` sources. Add an optional `judge_usage` field.
- A crash between a trial's `Spent` and its `Judged` makes the re-run append a different `Spent` under the same key, which D-08 turns into a stopping `StoreError::Conflict`. The loop must append a `Spent` only when its `SpendId` is absent.
- D-13 checks prices for the model names in the manifest, but `Spent` records the resolved model ID from `modelUsage` (for example the alias `haiku` resolves to a full ID), so `spend_ok` can hit `UnpricedModel` after the first call.
- D-04's equal-digest refusal would refuse any source tree with an empty file whenever the judge data holds one.
- `core::sweep::render` returns a diff relative to the parent through `diff_trees`, so D-12 cannot simply retype `diff_trees` as `CumulativeDiff` (Pattern 10).

**Primary recommendation:** Get the user's answers on findings 1 to 5 first, then build in this order:

1. Core, store, and source changes, each with its golden, contract, or unit test.
2. `minos-loop` against `MemoryStore`, `FakeProvider`, and a fake `JobRunner`, with a pure stage function and an exhaustive crash-point resume test.
3. `minos-runner` against an in-test fake `Backend`.
4. `minos judge push`.
5. One ignored VM test in `minos-cli` that drives the production runner on the Phase 5 fixture image.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Judge data push and versioning (JUDGE-01, CLI-04)
- **D-01:** The label comes from the manifest. `minos judge push <dir>` reads `[judge] version` from `minos.toml` and registers the directory under that integer label. Publishing a new version means bumping the manifest, then pushing. Pushing different data under a label that is already registered is refused unless `--update` is passed, and the refusal prints both tree digests, the same gate `minos env build` applies to the root image (05-CONTEXT D-10). No `--version` flag and no auto-increment.
- **D-02:** The label-to-data mapping lives in `minos.lock`, as one `[[judge]]` table per label beside the `[[image]]` tables: the label, the data tree digest (the `TreeId`), the file count, the total bytes, the per-file path and SHA-256 list (D-04), and the `minos` version that pushed it. The lock stays TOML with its `v` tag, `deny_unknown_fields`, and `try_from`; a missing `[[judge]]` array parses as empty, so existing lock files stay valid, and the golden file gains a case. `Lock::check` grows to refuse a run whose manifest label has no entry. The data itself stays in `.minos/blobs` (04-CONTEXT D-10); the lock holds digests only, so committing it reveals nothing, and a fresh clone must push the data again.
- **D-03:** The judge directory is packed by `minos-source`'s deterministic ustar packer, exactly as the source snapshot is: regular files only, sorted paths, zeroed metadata, the hard denylist, and the secret scan, with no override. The tarball's SHA-256 is the `TreeId` that `JudgeVersion::of(data, judge_spec)` hashes, so the recorded `JudgeVersion` is a pure function of the pushed bytes and the manifest's judge spec. Judge data sits unencrypted in `.minos/blobs` and inside every judge VM, so the same fail-closed hygiene applies. A directory over `MAX_JUDGE_DATA_BYTES` (96 MiB) is refused, never truncated (CLAUDE.md Open questions).
- **D-04:** `push` records per-file digests, and the source pack refuses overlap. The `[[judge]]` entry lists each file's path and SHA-256 (bounded, because the data is capped). `minos run`'s source pack receives the judge file digest set from the lock through the CLI and refuses, naming every offending path, when a packed file's digest equals any judge file's digest (PITFALLS Pitfall 28): a judge fixture copied into the surface would let a candidate see what it is scored on. This closes the "waits for Phase 7" note in CLAUDE.md's Source paragraph.

#### Baseline calibration and resume (LOOP-05, LOOP-06, LOOP-09, JUDGE-02 groundwork)
- **D-05:** The baseline trial count is a manifest field, `[judge] trials` (working name), default 3, range-checked in `Manifest::try_from` against `MAX_TRIALS`. Generation 0 of every run performs that many baseline trials, each a `Judged { baseline, trial }` with its `Spent` records, and the noise floor folds from them (03-CONTEXT D-12). It is a project fact, like the memory budget (05-CONTEXT D-06), so `minos check` can print it beside the floor.
- **D-06:** Every run calibrates itself. `run` always performs its own generation 0 before generation 1, so a run's ledger is self-contained and `build_report` never reads another run. `minos check` (Phase 8) drives the same generation-0 code on a diagnostic run of its own to print the noise floor and the route table; it never creates the run that `run` later resumes. The loop therefore exposes generation 0 as a callable step.
- **D-07:** Resume stages from the ledger and never re-proposes. A generation's stage is a pure function of the fold: no `Proposed` for it means propose, with the call's `Spent` appended first and then one `Proposed` per surviving item; `Proposed` present means run every `(exp, judge, trial)` that has no `Judged`, keyed by `LaunchKey`; every trial judged means `conclude` and append `Selected`. A crash between the `Spent` and the first `Proposed` makes the next attempt propose again under the next call index, so spend is counted and nothing is recorded twice. With fakes, an interrupted run folds to the same state as an uninterrupted one (LOOP-05), and a second driver on the same run is refused by the exclusive `driver.lock` under `runs/<run-hex>/` (LOOP-06; 04-CONTEXT D-11).
- **D-08:** `Appended::Conflict` becomes a store error. `Appended` keeps `New` and `Duplicate`; a same-key append with a different record returns `Err(StoreError::Conflict { seq })`, non-retryable like `Corrupt`, with the contract test updated for both stores (04-REVIEW IN-03). The loop cannot `?` past it: the driver exits naming the run, the seq, and the event kind, records nothing, and `minos status` (Phase 8) says the ledger needs a human. The first write still wins on disk.

#### Ledger provenance additions (LOOP-04, JUDGE-04, 05 and 06 deferrals)
- **D-09:** `Judged` gains `logs: Option<BTreeMap<Stage, BlobId>>`, naming the capped raw stage logs the runner stored as blobs from `JobResult::logs`. An `Option` field keeps every existing line parseable under `"v":"1"` (CLAUDE.md ledger rule); the key stays `(exp, judge, trial)`; the golden file gains a case. Every reader, the prompt's failure excerpts (06-CONTEXT D-08), `minos show`, and the report, still goes through `LogExcerpt`, so the blob's hostile bytes never reach a terminal or a model unstripped. This resolves 06-RESEARCH Open Question 4.
- **D-10:** `RunStarted` gains `root_image: Option<ImageDigest>`, the lock's digest from `Lock::check` at start; `RunId::of` is unchanged and the fold still recomputes the run ID. On every start and resume the loop requires the lock's current root image to equal the recorded one, else it refuses with both digests, so `env build --update` means a new run. Judge data is already pinned through `JudgeVersion`, and each `Judged` still records `root_image` in `BackendInfo`. This implements the lineage half of 05-CONTEXT D-10 that plan 05-16 deferred.
- **D-11:** Every run is pinned to the backend class of its first `Judged`. The loop refuses to record a `Judged` whose `BackendInfo` `(backend, arch)` differs from the run's first one, whatever the objective; the refusal is a configuration error that stops the driver, never an `Outcome`. No manifest change, strictly stronger than JUDGE-04, and free in v1 where krun on arm64 is the only backend; when Lambda arrives a run still never mixes backends.
- **D-12:** Core gains the `CumulativeDiff` newtype (03-REVIEW WR-03). It is produced only by `compose` and by `diff_trees` taken against the run's base tree; `Experiment` and `JobRequest::candidate` take it, so a diff relative to the parent tree, or a bare `Diff::parse` result, cannot become an `ExperimentId` by mistake. No wire change: the blob holds the same canonical text.

#### Caps, stops, and prices (LOOP-07, LOOP-08)
- **D-13:** Prices are required before a run starts or resumes. The CLI parses `~/.config/minos/prices.toml` into the validated `Prices` value and the loop takes it as input. A missing or unparseable file, or a table lacking the compute rates or a rate for any model the manifest's proposers name, refuses the run with the path and the missing keys named, before any VM or model call. Never zero, never a built-in default; `minos init` (Phase 8) may write a template.
- **D-14:** Safe points are every generation, model call, and launch. Before a generation starts the loop re-folds the ledger and checks `spend_ok`, `circuit_breaker`, and any `StopRequested`; before each model call `spend_ok` and `StopRequested`; before each VM launch `StopRequested` (compute spend is checked once its `Spent` lands). A VM already running is awaited (it has a hard maximum duration) and its `Judged` and `Spent` are recorded, then the driver exits. `minos stop` (Phase 8) only appends the event; the driver never kills a VM on a stop.
- **D-15:** The spend cap is metered only. `spend_ok` prices recorded `Spent` events; the loop never estimates a job or a call before it runs, so the cap can be exceeded by at most one VM job or one model call, which the docs state. Reserving a job's worst case before launch was rejected for v1.
- **D-16:** An infrastructure failure is retried at the same step after a short fixed pause. A launch error, an `InfraError` from `wait`, a store `Io`, or a provider infrastructure failure increments the in-memory streak, and the loop retries the same step (the same `LaunchKey`, or the next model call index) after a pause that is one constant with its reason. Any success resets the streak to 0. When the streak reaches `max_infra_failures` the loop appends `StopRequested { InfraFailures }` and exits (03-CONTEXT D-14). Nothing is recorded for a failed attempt. `ProviderLimit` and `ProviderAuth` are stops, never retries (06-CONTEXT D-13, D-15).

#### Carried forward (decided in earlier phases or the design of record; not re-opened)
- Record before act, every effect keyed by a content-derived ID: `EventKey`, `SpendId`, `LaunchKey` (03-CONTEXT D-09, D-10; 04-CONTEXT D-15). A same-key equal append is a `Duplicate` no-op.
- Diffs are cumulative from the run's base tree; `compose` and `diff_trees` in core; `ExperimentId::of(base, diff)` and `ExperimentId::baseline` (03-CONTEXT D-04 and amendment).
- Trials reduce by the lower median; the incumbent reduces over scored trials only (`TrialRule`); `conclude`, `noise_check`, `route`, `circuit_breaker`, and `spend_ok` are the pure core functions the loop calls (03-CONTEXT D-11 to D-14; `crates/minos-core/src/policy.rs`, `route.rs`).
- The `JobRunner` seam is defined by `minos-loop`; the CLI adapts `minos-runner` to it, so neither crate depends on the other (`deny.toml` `[bans]`). The runner is generic over `B: Backend`; never a trait object, never a backend enum in `minos-sandbox` (04-CONTEXT carried forward).
- `KrunBackend::new(paths, blob_source)`; the backend writes the per-job input disk from `BlobSource` bytes under caps; the runner passes IDs only; the host never unpacks the tarball or applies the diff (05-CONTEXT D-13, D-15). The CLI composes `Runner<KrunBackend, FileStore>` through `FileBlobSource`.
- `ProposeInput` and `Proposals` as `minos-propose` built them: the loop supplies the base and parent trees, the program bytes, the rendered `History`, and the steer notes; it records one `Proposed` per item with `Proposals::origin`, `Spent { SpendSource::ModelCall }` per model in `Proposals::models`, adds `DropCounts::total` to `Decision::dropped`, and treats an empty item list as a generation with no valid candidate (06-CONTEXT D-12; `crates/minos-propose/src/proposer.rs`).
- A host `TimedOut` is attributed to `Run` for a Candidate and `Judge` for a Judge; `Ended::ChunkProtocol` is `Rejected(Reason::ChunkProtocol)`; the K7 classification is the backend's, and the runner maps `Ended` plus the guest status onto `Outcome` (CLAUDE.md Local backends; 05-RESEARCH Pitfall 12).
- Gates run before objectives and short-circuit (JUDGE-03): `check_scorecard` in core yields `Reason::GateFailed` first; a failed gate is `Rejected` and the objectives are never read.
- Candidates run sequentially in v1; `width` and the K8 measurements are NEXT-04 (05-CONTEXT Deferred).
- v1 is synchronous: `std::thread::scope`, `std::process`, `std::fs::File::lock`; no tokio in any `minos-*` crate.
- No `unwrap`, `expect`, or slice indexing anywhere, tests included; no `#[allow]` at all; `BTreeMap`/`BTreeSet` wherever order affects output; golden wire files as insta snapshots with a SHA-256 pin and `update: no`; golden hashes as plain `assert_eq!`.
- Every plan ends with the five gate commands plus the musl clippy line, then the C-01 and C-02 checks; the phase ends with `cargo clean` (GATE-06).

### Claude's Discretion
- The `JobRunner` trait's shape in `minos-loop` (synchronous, `&self`, a `JobRequest` in, a `JobResult` or an infrastructure error out), the fake job runner used by the loop tests, and the CLI adapter over `minos-runner`.
- The runner's internals: how it stores the candidate output blob for the Judge input, how `JudgeData` is attached, the host-measured usage (`vcpu_ms` and `mem_mib_ms` from the host's elapsed time and the plan's `Machine`, never a guest number), the per-stage log blob cap, which guest output files are the stage logs, and the exact mapping table from `(Ended, GuestStatus, output exits)` to `Outcome`, following the K7 rule.
- The exact `CumulativeDiff` API (D-12): its two constructors, whether `Proposal` carries it or the loop mints it at admission, and the test that a parent-relative diff cannot reach `Experiment`.
- The `[[judge]]` lock entry's exact field names, how the per-file digest list is encoded, what `push` prints (the label, the tree digest, the `JudgeVersion`), and the `--update` flow's wording.
- The retry pause constant (D-16), the `[judge] trials` upper bound within `MAX_TRIALS` (D-05), and the per-stage log blob cap (D-09), each a constant with its reason beside the code that enforces it.
- How the driver reports an unscored baseline after generation 0 (`PolicyError::IncumbentUnscored`): an error from the driver that stops the run; no `StopReason` variant fits and none is added without asking.
- An observer or event-sink seam the driver exposes so Phase 8 can print one progress line per experiment without the loop knowing about terminals; synchronous, and every string from a sandbox or a model passes through `SafeText` or `LogExcerpt` before it leaves the loop.
- The `driver.lock` wording and what a refused second driver reports (LOOP-06).
- Plan count and ordering. The loop plans (LOOP-04 to LOOP-09) against fakes can run as one wave with the core and store changes; the runner plans (LOOP-01 to LOOP-03, JUDGE-03, JUDGE-04) and `judge push` follow, with one ignored VM test that drives the production runner on the Phase 5 fixture image.

### Deferred Ideas (OUT OF SCOPE)
- **The `minos run`, `status`, `log`, `show`, `stop`, `check`, `report`, and `adopt` commands and the per-experiment progress line** — Phase 8 (CLI-05 to CLI-10, JUDGE-02, COST-02). This phase ships the driver as a library, the generation-0 step `check` will call (D-06), and the observer seam the progress line will use.
- **Concurrent candidates (`width`) and the K8 launch-rate and memory measurements** — NEXT-04; the launch key, per-append locking, and the sequential driver are designed so concurrency needs no store or ledger change.
- **Reserving a job's worst-case compute before launch** — rejected for v1 (D-15); reconsider if one-step overshoot proves material on a GPU backend.
- **An objective flag (`kind = "speed"`) or a `[environment] requires.backend` field** — rejected for v1 in favor of pinning every run (D-11); return to it only when a multi-backend milestone wants hardware-independent runs to roam.
- **A separate judge lock file** — rejected (D-02); the one `minos.lock` holds both lineages.
- **Judge data over 96 MiB** — unchanged open question (CLAUDE.md): needs a streaming unpack in PID 1; `push` refuses it meanwhile (D-03).
- **`minos rejudge <run> --judge <v>`** (NEXT-02) and **`minos steer`** (NEXT-01) — the `Judged` key and the `Steered` event already support them; the loop's history view reads notes in ordinal order.
- **Champion re-measurement before `Selected`** — NEXT-05.
- **A `StopReason` for an unscored baseline** — not added; the driver reports it as an error (Claude's Discretion). Revisit if `minos status` needs a recorded reason.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LOOP-01 | `minos-runner` runs the candidate VM then the judge VM for a `JobRequest`, converts `Untrusted` output to an `Outcome` via the validating exits, and is the only crate where that conversion happens | Pattern 1 (pipeline), Pattern 2 (outcome table), Pattern 4 (the two new exits it needs: Finding 1); a source grep check in Validation |
| LOOP-02 | The judge VM mounts judge data read-only; the candidate VM never does; judge and candidate never share a process or a disk | Already enforced by the types: only `SandboxSpec<Judge>::secrets` attaches `JudgeData` (trybuild-proven, 04-08), `preflight` refuses `JudgeDataMissing` and `JudgeDataIsInput`, and every launch has its own job directory and input disk. The judge sees only a repack of `out/` (Pattern 4) |
| LOOP-03 | Time and resource usage are measured by the host, never read from candidate output | Pattern 3: `Finished::elapsed()` (host `Instant` from spawn to exit) times the plan's `Machine`; `JobResult::new` already refuses tokens |
| LOOP-04 | `minos-loop` drives route, propose, validate, run, fold `Judged`, `select_greedy`, append `Selected`; depends only on core, store, propose | Pattern 5 (pure stage function), Pattern 6 (admission); `deny.toml` wrappers already enforce the lane |
| LOOP-05 | Records before acting and keys every effect by content ID, so an interrupted run resumes to the same result (tested with fakes) | Pattern 5 and 6, Pitfalls 1 to 3 (Findings 2 and 6), the crash-injection test in Validation |
| LOOP-06 | A run holds an exclusive lock so two drivers cannot act on the same run | Pattern 7: a `RunLock` store trait; the file impl uses `File::try_lock` on `runs/<hex>/driver.lock` (probe: a second handle in the same process gets `WouldBlock`) |
| LOOP-07 | Spend cap and token cap checked before each generation and each model call; exceeding stops with a recorded reason | Pattern 8 safe points; `spend_ok` exists; Pitfall 7 (resolved model IDs) |
| LOOP-08 | Circuit breaker stops after K empty generations or K consecutive infra failures and records `StopRequested` | `circuit_breaker(state, budget, infra_streak)` exists; Pattern 9 (streak, injectable pause) |
| LOOP-09 | Fully testable against the in-memory store, a fake provider, and a fake job runner with no VM | `MemoryStore` passes the store contract; `FakeProvider` exists; `FakeJobRunner` and a crash-injecting store wrapper are Wave 0 |
| JUDGE-01 | `minos judge push <dir>` uploads judge data as a new judge version and records its hash; never in the source tarball or build context | Pattern 11 (judge pack entry point, lock write mirroring `env_build`), Pattern 12 (overlap refusal, passed to both `minos run`'s and `env build`'s snapshot) |
| JUDGE-03 | Gates run before objectives and short-circuit | `check_scorecard` checks gates first; the runner records `Rejected(GateFailed)` with no scorecard (Pattern 2) |
| JUDGE-04 | Every `Judged` records backend and architecture; a speed-objective run is pinned to one backend class | `Capabilities::backend_info(root)`; D-11 pin through a new `RunState` accessor (Pattern 8) |
| CLI-04 | `minos judge push <dir>` registers judge data | Pattern 11; a clap builder subcommand beside `env build` |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- `unsafe_code = "forbid"`. `unwrap_used`, `expect_used`, and `indexing_slicing` are `forbid` everywhere, tests included. No `#[allow(...)]` or `#![allow(...)]` anywhere. The C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` must print nothing. Never ask the user to accept an `unwrap` as a probe.
- **Never weaken a security invariant to make something compile or pass.** No escape hatch, `pub` field, `Deref`, or "temporary" bypass on `Untrusted<T>`, `SandboxSpec`, or `LaunchPlan`. If an invariant blocks you, stop and ask (this applies to Finding 1).
- Deserialization is validation: `#[serde(try_from = ...)]`, raw structs `#[serde(deny_unknown_fields)]`, private fields, no setters.
- Errors separate blame: `thiserror` enums distinguish a candidate fault (an `Outcome`, never retried), an infrastructure failure (retried), and a configuration refusal (stops, never retried). `anyhow` appears only in `minos-cli`'s `main`.
- Order must be stable: `BTreeMap`/`BTreeSet` or a sorted `Vec` wherever order affects output, hashing, or which effects get created.
- No tokio and no async in any `minos-*` crate. Use `std::process`, `std::thread::scope`, `std::os::unix::net`, and `std::fs::File::lock`.
- No C, no OpenSSL, no libgit2, no git in the trusted path. Commands are argv arrays.
- Supply chain: `Cargo.lock` committed, `cargo deny check` gates every build, and `[bans.workspace-dependencies] unused = "deny"` means a dependency nobody uses fails the gate.
- Layering (`deny.toml`): `minos-loop` may depend only on core, store, and propose. `minos-runner` may depend only on core, store, and sandbox. Only `minos-cli` may depend on runner, loop, source, or `backend-krun`. Dev-dependency edges are in the checked graph, so a runner or loop test cannot link `minos-backend-krun`.
- Wire formats are an API: golden insta snapshots plus a SHA-256 pin of the exact bytes, `.config/insta.yaml` `update: no`, golden hashes as plain `assert_eq!`. Adding an enum variant or an `Option` field keeps `"v":"1"`. A required field, a rename, or a type change needs `"v":"2"`.
- Core has no clock, filesystem, environment, or process. Time arrives as an argument (`RunId::of` takes `started_at` from the shell).
- Everything leaving a sandbox is hostile. It is shown only through `SafeText` or `LogExcerpt`.
- Docs and output must not overclaim isolation. Cost prints only as `approx. cost: $X.XX`.
- Check external APIs against their docs before coding. Prove a mechanism from source before proposing any dependency fork or patch.
- Gate per plan: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, then the C-01 (`[lints] workspace = true` in every crate) and C-02 checks. `cargo clean` at the end of the phase (GATE-06).
- Keep CLAUDE.md current in the same change as any decision change (the Source paragraph's "waits for Phase 7" note, the ledger event list, `minos.lock`'s description, the `Untrusted` exit list, the Open questions list).
- `rules/` (179 files) binds all code. The rules that matter most here:
  - `err-thiserror-lib`, `err-custom-type`, and `err-doc-errors`: every pub fallible fn gets an `# Errors` section.
  - `api-typestate` and `api-sealed-trait`: the `RunLock` guard and `Cleared` pattern.
  - `api-newtype-safety` and `type-newtype-validated`: `CumulativeDiff`, `BaseTree`, and the judge lock types.
  - `api-must-use`: `Appended`, the lock guard.
  - `test-mock-traits`: `JobRunner`, the fake backend, and the crash store.
  - `test-fixture-raii`: temp dirs and lock guards.
  - `test-proptest-properties`: the stage function and resume equivalence.
  - `anti-over-abstraction`: one trait per seam, no trait objects.
  - `doc-all-public` and `lint-missing-docs` (`missing_docs` is `warn`, so `-D warnings` makes it an error).
  - `name-consts-screaming`, `type-no-stringly`, and `type-enum-states`: the `Step` enum and the blame enums.
  - `api-serde-optional` (serde as an optional feature) does not apply: the project made serde a hard dependency of core for the wire formats.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Which step comes next (calibrate, propose, run trial, conclude, done) | `minos-loop` (pure fn over `RunState`) | `minos-core` (`fold`, `conclude`, `route`) | D-07: the stage is a pure function of the fold; core keeps the decisions, the loop keeps the ordering |
| Selection, noise, breaker, spend, routing | `minos-core::policy`, `route` | — | Already pure and tested; the loop never re-implements an `if` that decides fate |
| Record before act, idempotent appends, presence checks | `minos-loop` | `minos-store` (`EventKey` dedupe, `Conflict` error) | The loop knows which effect it is about to take; the store enforces key uniqueness |
| Whole-run exclusion (LOOP-06) | `minos-store` (`RunLock` trait, `driver.lock`) | `minos-loop` (holds the guard for the session) | Locking is storage-specific, and the loop must be testable on `MemoryStore` |
| Candidate VM, judge VM, `Untrusted` to `Outcome` | `minos-runner` | `minos-sandbox` (specs, preflight, exits), `minos-backend-krun` (K7 classify) | The runner is the only crate allowed to open `Untrusted` (LOOP-01) |
| Candidate artifacts to judge input, raw logs to blobs | `minos-sandbox` (new capped exits) | `minos-runner` (puts the blobs) | Only the sandbox crate may open `Untrusted`, and it may not open files, so it hands bytes to a store-put closure |
| Host-measured usage | `minos-runner` | `minos-backend-krun` (the `Instant` in `KrunRunning::wait`) | `Finished::elapsed()` is trusted, and the plan's `Machine` is known to the runner |
| Judge data pack, overlap refusal | `minos-source` | `minos-cli` (reads the lock, passes digest sets) | Only the CLI links `minos-source`; the walker, packer, denylist, and scan live there |
| `minos.lock` `[[judge]]` wire format, `Lock::check` | `minos-core::lock` | `minos-cli` (read-compare-write, `--update`) | Pure and validated in core; I/O in the CLI, mirroring `env_build` |
| Composition (`Runner<KrunBackend, FileStore>`, `JobRunner` adapter, prices file read) | `minos-cli` | — | The only crate where concrete types meet |

## Standard Stack

### Core (all existing workspace pins; nothing new to install)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std | 1.97.1 (`rust-toolchain.toml`) | `File::try_lock`/`TryLockError` for `driver.lock`, `thread::sleep` for the retry pause, `Duration` | `File::lock` family stable since 1.89.0 [VERIFIED: pinned std source `library/std/src/fs.rs` lines 145-152, 983] |
| `thiserror` | =2.0.21 | Runner and loop error enums with blame | Workspace standard [VERIFIED: Cargo.toml] |
| `serde`, `serde_json` (`float_roundtrip`) | =1.0.229, =1.0.151 | New `Option` fields on `Judged`, `RunStarted`, `JobResult`; the batch blob (if adopted) | Workspace standard [VERIFIED: Cargo.toml] |
| `toml` | =1.1.8 | `[[judge]]` lock tables, `[judge] trials` | Already parses `minos.lock` and `minos.toml` [VERIFIED: Cargo.toml; probe this session] |
| `tar` | =0.4.46, `default-features = false` | The `minos-sandbox` repack exit's writer (hand-built ustar headers, as `minos-source` `pack.rs` does) | Already a `minos-sandbox` dependency [VERIFIED: `crates/minos-sandbox/Cargo.toml`] |

### Supporting (dev-dependencies, all existing pins)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | =1.11.0 | Stage-function properties; resume equivalence over random crash points; permutation of proposals | `minos-loop` dev-dep |
| `insta` | =1.49.0 | New golden cases (`judged` with logs, `run_started` with root image, `job_result` with judge usage, lock with `[[judge]]`) | `minos-core` (exists) |
| `tar` | =0.4.46 | Building fake guest output streams in runner tests | `minos-runner` dev-dep |
| `tempfile` | =3.27.0 | Temp project dirs for `judge push` and FileStore `RunLock` tests | `minos-cli`, `minos-store` (exist) |
| `trybuild` | =1.0.122 | Optional: a compile-fail proof that a parent `Tree` cannot mint a `CumulativeDiff` (Pattern 10) | `minos-core` dev-dep if adopted (workspace pin exists) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `RunLock` store trait | Lock taken by the CLI around the driver | Fails LOOP-09: the "second driver refused" test must run on `MemoryStore` |
| Repack of `out/` for the judge input | Store the candidate's raw validated stream | No tar writer needed, but the judge also sees the candidate's `.minos/` status and logs (more hostile input next to judge data); still needs a new exit on `Finished` |
| Raw capped log blobs (D-09) | Store the `LogExcerpt` text as the blob | Needs no new `Untrusted` exit, but the user explicitly rejected "store only the sanitized excerpt" (DISCUSSION-LOG) |
| Batch blob on `Proposed` (Finding 2) | Recompute sweep batches; document a model-tier exception | No ledger change, but LOOP-05 equality would hold only for the sweep tier |

**Installation:** none. Wire the existing pins into the new manifests (`minos-runner` dev-deps: `tar`, `proptest`; `minos-loop` dev-deps: `proptest`).

**Version verification:** every pin above is in the committed `Cargo.lock` and already used by another workspace crate. No `npm view`, `pip`, or `cargo search` applies: nothing new enters the graph. [VERIFIED: `Cargo.toml` `[workspace.dependencies]`]

## Package Legitimacy Audit

No external package is added in this phase. Every crate named above is an existing workspace pin already in `Cargo.lock` and already gated by `cargo deny check` (licenses, advisories, bans, sources, build-script allowlist).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none new) | — | — | — | — | — | — |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                 minos-cli (Phase 8 wires `run`; Phase 7 ships `judge push` + composition test)
   prices.toml ──► Prices ─┐       minos.toml ──► Manifest ─┐      minos.lock ──► root image, judge tree
                           ▼                                 ▼                     │
                ┌───────────────────────── Driver (minos-loop) ──────────────────────────────┐
                │ acquire RunLock(run)  ──► refused? → DriverError::RunLocked                 │
                │ loop:                                                                       │
                │   read ledger ──► fold ──► RunState                                         │
                │   safe point: new StopRequested? breaker? spend_ok? ──► append Stop, exit   │
                │   next_step(state, trials, judge, target)  (pure)                           │
                │     ├─ Calibrate{trial} / RunTrial{exp,trial} ──► JobRunner::run(request)   │
                │     │      infra ─► streak++, pause, retry same request (or Stop at limit) │
                │     │      config ─► DriverError (no record)                                │
                │     │      ok ─► append Spent(cand) if absent, Spent(judge) if absent,      │
                │     │            pin check (backend class), append Judged{.., logs}         │
                │     ├─ Propose{g} ──► route ─► Proposer::propose(spec, input)               │
                │     │      Spent(ModelCall{next call}) per model ─► admit (CumulativeDiff, │
                │     │      dedupe) ─► put diff+rationale blobs ─► append Proposed per item  │
                │     ├─ Conclude{g} ──► conclude(state, manifest, judge, g, dropped)         │
                │     │                  ─► append Selected                                   │
                │     └─ Done ──► return RunEnd                                               │
                └──────────────┬──────────────────────────────────────────────────────────────┘
                               │ JobRequest (IDs only)                 ▲ JobResult (Outcome, usages, BackendInfo, log blobs)
                               ▼                                       │
                ┌──────────────── Runner<B: Backend, S: Blobs> (minos-runner) ────────────────┐
                │ verify diff blob ─► put GuestJob(candidate) ─► SandboxSpec<Candidate>::new  │
                │ ─► seal ─► preflight ─► launch ─► wait ─► Finished<Candidate>                │
                │   elapsed × Machine ─► Usage (host-measured)                                 │
                │   map Ended + status + output faults ─► Outcome (or stop here)               │
                │   logs ─► capped tail blobs;  out/ ─► repack ─► candidate_out blob           │
                │ put GuestJob(judge) ─► SandboxSpec<Judge>::new(InputDisk::judge(job, out))   │
                │   .secrets(JudgeData(judge tree blob)) ─► seal ─► preflight ─► launch ─► wait│
                │   ─► status ─► into_scorecard ─► check_scorecard (gates first) ─► Outcome   │
                └──────────────┬───────────────────────────────────────────────────────────────┘
                               ▼
                KrunBackend (fetches blobs via FileBlobSource under section caps, writes the input
                disk, Seatbelt + minos-vmm, chunked vsock output, K7 classify) ──► hardware VM
```

### Recommended Module Structure

```
crates/minos-core/src/
├── lock.rs            # + LockedJudge, RawLockedJudge, JudgeFile; Lock::judge(label), with_judge, judge_file_digests;
│                      #   LockMismatch::NoJudge { label }; MAX_LOCK_BYTES raised (Finding 4)
├── manifest.rs        # + RawJudge.trials: Option<u32>, JudgeSpec::trials(); excluded from JudgeVersion
├── ledger.rs          # + Judged.logs, RunStarted.root_image (Option, default, skip_serializing_if);
│                      #   + Proposed.batch (Option<BlobId>) only if the user accepts Finding 2
├── state.rs           # + RunState::root_image(), stops(), backend_class(), has_spend(&SpendId),
│                      #   next_model_call(g), judged(exp, judge, trial)
├── diff/cumulative.rs # CumulativeDiff + BaseTree (Pattern 10); compose returns CumulativeDiff
├── job.rs             # JobRequest::candidate takes CumulativeDiff via Experiment; JobResult + judge_usage
├── money.rs           # + Prices::missing_rates(&Manifest) -> Vec<..> (D-13, pure)
└── policy.rs          # + baseline_ready(state, manifest, judge) (pub wrapper over valid_score)
crates/minos-store/src/
├── lib.rs             # Appended {New, Duplicate}; StoreError::Conflict { seq } and RunLocked { run }; RunLock trait
├── memory.rs          # RunLock over Mutex<BTreeSet<RunId>> with a Drop guard
└── file/run_lock.rs   # driver.lock: open_or_create (0600, symlink-checked) + File::try_lock
crates/minos-sandbox/src/
└── untrusted.rs       # + the two capped sink exits (Finding 1, needs user approval)
crates/minos-source/src/
├── judge.rs           # pack_judge_data(root) -> JudgePack (tarball, TreeId, per-file digests, report)
└── lib.rs             # snapshot(.., judge_digests: &BTreeSet<BlobId>) + SnapshotError::JudgeOverlap
crates/minos-runner/
├── clippy.toml        # HashMap/HashSet, SystemTime::now/Instant::now, env::var, process::Command bans
└── src/{lib.rs, error.rs, map.rs, usage.rs, pipeline.rs}
crates/minos-loop/
├── clippy.toml        # same bans plus no fs::File (the loop touches storage only through traits)
└── src/{lib.rs, job.rs, stage.rs, driver.rs, admit.rs, record.rs, inputs.rs, history.rs, observe.rs, error.rs}
crates/minos-cli/src/
├── judge_push.rs      # mirrors env_build.rs: manifest, pack, put_reader, lock read-compare-write
├── jobs.rs            # RunnerJobs<B>: impl minos_loop::JobRunner over minos_runner::Runner
└── main.rs            # + `judge push <dir> [--update]`
crates/minos-cli/tests/
├── judge_push.rs
└── runner_vm.rs       # the one ignored VM test (MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW)
```

### Pattern 1: The runner pipeline and the `JobRunner` seam

**What:** `minos-loop` defines a synchronous seam whose errors carry blame. `minos-runner` implements the real pipeline. The CLI adapts one to the other, so neither crate depends on the other.

```rust
// minos-loop/src/job.rs (recommended shape; discretion)
pub trait JobRunner {
    /// One trial: the candidate VM, then (if the candidate passed) the judge VM.
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure>;
    /// Builds and preflights the baseline's Candidate and Judge plans without launching, so a SpecError or a
    /// host refusal stops the run before generation 0 (minos-sandbox spec.rs "Handlers" note).
    fn check(&self, request: &JobRequest) -> Result<(), JobFailure>;
}
#[non_exhaustive]
pub enum JobFailure {
    /// Retried after the pause and counted toward max_infra_failures (D-16).
    Infra { what: &'static str },
    /// Stops the driver with no record: a spec over the ceiling, a preflight refusal, a full scratch,
    /// a missing root image or judge data blob, a diff that does not match the request.
    Config { what: ConfigCause },
}
```

The runner itself: `pub struct Runner<B: Backend, S: Blobs> { backend: B, store: S, manifest: Manifest, root: ImageDigest, judge_data: JudgeData, judge: JudgeVersion }` with `pub fn run(&self, request: &JobRequest) -> Result<JobResult, RunnerError>`. `RunnerError` separates `Infra` from `Config`, and the CLI maps it onto `JobFailure`. Steps, all against current signatures:

1. Fetch the diff blob under `MAX_DIFF_BYTES`, run `Diff::parse(text, manifest.writable(), &Limits::standard())`, and require `request.verify_diff(&diff)` (or the `CumulativeDiff::recorded` check, Pattern 10). A mismatch is `Config`: the loop built a wrong request, which is a bug, not the candidate's fault.
2. `store.put(&GuestJob::candidate(&manifest, request).to_json()?)` gives the job blob. Build `InputDisk::candidate(job, request.base(), request.diff())`.
3. `SandboxSpec::<Candidate>::new(request, root, input, Machine::new(VCPUS, manifest.environment().memory_mib())?, Arch::Arm64)?.seal()`. A `SpecError` is `Config`.
4. `preflight(&backend.capabilities(), plan)`. A `Vec<Refusal>` is `Config`.
5. `backend.launch(cleared)`. `LaunchError::Io` and `LaunchError::Backend` are `Infra`. `LaunchError::Blob` is `Config`, except `BlobSourceError::Io`, which is `Infra`. `RunningVm::wait()`: `InfraError` is `Infra`.
6. Map the candidate (Pattern 2). If it passed, repack `out/` into the `candidate_out` blob (Pattern 4). Then build the judge: `GuestJob::judge(&manifest, request)` gives the job blob, then `SandboxSpec::<Judge>::new(request, root, InputDisk::judge(job, out), machine, arch)?.secrets(judge_data).seal()`, then preflight, launch, wait, and map.
7. `JobResult::new(request, outcome, candidate_usage, backend_info, logs)` with the added `judge_usage`, where `backend_info = backend.capabilities().backend_info(root)`.

[VERIFIED: signatures read from `crates/minos-sandbox/src/{spec,preflight,backend,finished,untrusted}.rs`, `crates/minos-core/src/{job,guest}.rs`]

### Pattern 2: `(Ended, GuestStatus, output faults)` to `Outcome`

The backend already classified the helper's exit by K7. The runner maps only what it receives. Read `setup` before `role` (review CR-02). [VERIFIED: `supervise.rs` `classify`, `untrusted.rs` `take_status` docs, `guest.rs` `SetupStep` docs]

| Observation (in order) | Candidate result | Judge result |
|---|---|---|
| `Ended::TimedOut` | `TimedOut { stage: Run }` | `TimedOut { stage: Judge }` |
| `Ended::Crashed(_)` (no status frame, console cap) | `Crashed { stage: Run }` (recommended, mirroring the timeout attribution) | `Crashed { stage: Judge }` |
| `Ended::ChunkProtocol` | `Rejected(ChunkProtocol)` | `Rejected(ChunkProtocol)` |
| `Ended::Exited`, `into_output()` fails with an `OutputFault` | `Rejected(fault.reason())` | same |
| `take_status()` gives `SetupStatus::Failed { Input }` (any role field) | **Infra** (retry) | **Infra** |
| `SetupStatus::Failed { ScratchFull }` | **Config**: report `[environment] scratch_mib`, never retry (guest.rs WR-02) | **Config** |
| `SetupStatus::Failed { Mount, Unpack, Apply, Output }` | **Infra** | **Infra** |
| `take_status()` gives `StatusFault::{Missing, Invalid, WrongRole}` | `Rejected(OutputInvalid)` | same |
| `ApplyStatus::Failed { path, hunk }` | `Rejected(ApplyFailed { path, hunk })` | n/a |
| Build `StageEnd::TimedOut` | `TimedOut { stage: Build }` | — |
| Build `Exited { code != 0 }`, `Signaled`, or `SpawnFailed` | `Rejected(BuildFailed)` | — |
| Run `TimedOut` | `TimedOut { stage: Run }` | — |
| Run `Exited { code != 0 }`, `Signaled`, or `SpawnFailed` | `Rejected(RunFailed)` | — |
| `OutputStatus::TooLarge` or `Invalid` | `Rejected(OutputTooLarge)` / `Rejected(OutputInvalid)` | — |
| Candidate passed (build and run exit 0, output `Packed`) | → launch the judge | — |
| Judge stage `TimedOut` | — | `TimedOut { stage: Judge }` |
| Judge stage `Exited { != 0 }`, `Signaled`, or `SpawnFailed` | — | `Crashed { stage: Judge }` (no `Reason` names a judge failure; adding one needs the user) |
| Judge stage exit 0, then `into_scorecard` fails (`ScorecardFault`) | — | `Rejected(ScorecardInvalid)` |
| `check_scorecard(manifest.judge(), &card)` gives `Err(reason)` (gates first: `GateFailed`) | — | `Rejected(reason)`, so objectives are never recorded (JUDGE-03) |
| Otherwise | — | `Scored(card)` |

Implement the table as a pure function in `minos-runner/src/map.rs` over already-validated values (`Ended`, `GuestStatus`, `Option<OutputFault>`), with one table test row per line. The baseline calibration catches a broken judge: if no baseline trial scores, the driver stops with `IncumbentUnscored`.

### Pattern 3: Host-measured usage (LOOP-03)

```rust
// minos-runner/src/usage.rs: the guest reports nothing about itself (Pitfall 9).
fn host_usage(elapsed: Duration, machine: &Machine) -> Usage {
    let ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX); // saturate: over every cap, fails closed
    Usage::compute(
        ms.saturating_mul(u64::from(machine.vcpus())),
        ms.saturating_mul(u64::from(machine.memory_mib())),
    )
}
```

`Finished::elapsed()` is the host `Instant` measured from spawn to helper exit inside `KrunRunning::wait`, so it includes boot. That is honest metering of what the VM held. [VERIFIED: `crates/minos-backend-krun/src/backend.rs` lines 223-260] `unwrap_or(u64::MAX)` on a `Result` is not `unwrap`, so the forbid lint allows it. The vCPU count has no manifest field. Pick one constant with its reason (Assumption A5). The test: a fake backend reports `elapsed = 1500 ms`, and the candidate output holds a forged `.minos/usage.json` and `out/timing.json`. Assert usage equals `1500 × vcpus` and `1500 × memory_mib`.

### Pattern 4: The two capped exits the runner needs (Finding 1, needs user approval)

Both exits keep the invariants that matter. Neither has `Debug`, `Display`, `Deref`, or `Clone`. Bytes never come back to the caller as a value: they go only to a closure that stores them as a content-addressed blob, and that blob is later read only through `LogExcerpt::from_bytes` (logs) or by another VM (judge input). Both apply hard caps.

```rust
// minos-sandbox/src/untrusted.rs (proposed; exact names are the user's call)
impl Untrusted<GuestOutput<Candidate>> {
    /// The candidate's artifacts as the judge receives them: every remaining file under `out/`, re-packed as a
    /// canonical ustar (sorted RelPaths, mode 0644, mtime/uid/gid 0, no other entry), handed to `store`.
    /// Files outside `out/` (the status, the logs, anything else) are never passed on.
    pub fn store_judge_input<E>(self, store: impl FnOnce(&[u8]) -> Result<BlobId, E>) -> Result<BlobId, E>;
}
impl<R: Role> Untrusted<OutputFile<R>> {
    /// The last `max` bytes of the file (a stage log), handed to `store`; the caller gets only the BlobId.
    pub fn store_tail<E>(self, max: usize, store: impl FnOnce(&[u8]) -> Result<BlobId, E>) -> Result<BlobId, E>;
}
```

- The repack must fit `MAX_CANDIDATE_OUT_BYTES`. It does by construction: a subset of an output that already passed `read_output`'s caps.
- The judge's guest unpacks it into `/candidate` under `UnpackCaps::CANDIDATE_OUT`, so the judge reads `/candidate/out/...`, the layout the Phase 5 VM test already used (`out/result.json`).
- Golden-test the repack bytes.
- Add trybuild cases: neither exit exists on a type it should not, and `store_judge_input` does not exist on judge output.
- Log blob cap: mirror the guest's `LOG_CAP_BYTES` (262,144). The guest is hostile once candidate code runs, so the host must cap independently even though the output entry cap is 8 MiB. Keep the tail, because `LogExcerpt` shows the tail.

[VERIFIED: `crates/minos-guest/src/pack.rs` `LOG_CAP_BYTES`, `run.rs` judge unpack into `CANDIDATE`, `workspace.rs` `UnpackCaps::CANDIDATE_OUT`]

### Pattern 5: Resume as a pure stage function (D-07, LOOP-04, LOOP-05)

```rust
// minos-loop/src/stage.rs: a pure function of the fold and fixed run inputs; property-tested.
pub enum Step {
    Calibrate { trial: u32 },                       // generation 0: baseline trials 0..trials without Judged
    BaselineReady,                                  // all baseline trials judged; check baseline_ready once
    Propose { generation: u32 },                    // no Proposed and no Selected for `generation`
    RunTrial { generation: u32, exp: ExperimentId, trial: u32 }, // Proposed present, trial unjudged
    Conclude { generation: u32 },                   // every trial of every proposal judged, no Selected
    Done,                                           // last_concluded_generation() >= target
}
pub fn next_step(state: &RunState, judge: &JudgeVersion, baseline_trials: u32,
                 candidate_trials: u32, target: u32) -> Step
```

- Generation `g` is `state.last_concluded_generation() + 1`. That function returns 0 when nothing has concluded, and generation 0 is never concluded.
- Trials are walked in `ExperimentId` order (`proposals_in` iterates a `BTreeMap`), then by trial index, so the order of launches is a pure function of the ledger.
- The empty-call case: `Propose` yields zero items. Record the call's `Spent`, then go straight to `Conclude` with the propose step's `dropped` count; `conclude` returns `NoValidCandidates`.
- **Candidate trial count:** D-05 sets the baseline count only. Recommend 1 trial per candidate in v1 (`CANDIDATE_TRIALS = 1`), because champion re-measurement is NEXT-05 (Assumption A4).
- **Target:** take an absolute target generation, not "N more", so a resumed session computes the same `Done`. Phase 8 maps `--generations N` onto it (Open Question 7).

### Pattern 6: Admission and the propose-step records (LOOP-04, Findings 2 and 6)

Order within one propose step:

1. Safe point: `spend_ok`, plus a stop recorded in this session.
2. `call = state.next_model_call(g)`: one past the highest `SpendSource::ModelCall { call }` recorded in `g`, else 0.
3. Run `Proposer::propose(spec, &input)`.
4. On `Err(e)`: if `e.usage()` is `Some`, append `Spent { g, ModelCall { call, spec model }, usage }` so the spend is metered. Then `e.stop_reason()` is `Some` for a stop: append `StopRequested { g, reason }` and exit. `e.is_infra()` means streak++, pause, and retry under `call + 1`. Anything else is a `Config` error.
5. On `Ok(p)`: append `Spent` once per `p.models()` entry under `call` (or once from `p.usage()` under the spec's model when `models` is empty; a sweep has neither).
6. For each item in order: build `exp = ExperimentId::of(base, &cumulative)`. If `state.proposal(&exp)` is `Some`, count a drop: it was proposed in an earlier generation, and a second `Proposed` with different fields would fail the fold with `DuplicateProposal`. Otherwise `put` the diff and rationale blobs, then append `Proposed { exp, parent: state.incumbent_before(g), generation: g, tier, diff, rationale, origin }`.

**Finding 2 fix (recommended, needs the user):** before step 6, `put` one canonical JSON "batch blob" holding every admitted item `(exp, diff blob, rationale blob)`, the `DropCounts` total, the tier, the origin, and the parent. Every `Proposed` of the batch carries an additive `batch: Option<BlobId>` naming it (`#[serde(default, skip_serializing_if = "Option::is_none")]`, so existing lines and goldens are unchanged; probe below). On resume, any one recorded `Proposed` recovers the whole batch: append the missing `Proposed`, and take `dropped` for `conclude` from the blob. That gives exact LOOP-05 equality at every crash point after the first `Proposed`. A crash before the first `Proposed` re-proposes under the next call index, exactly as D-07 states. The fallback without a ledger change: recompute sweep batches (deterministic) and document that a model-tier crash inside the batch loses the unrecorded items and the drop count.

### Pattern 7: The whole-run lock (LOOP-06)

```rust
// minos-store/src/lib.rs (recommended)
pub trait RunLock {
    type Guard<'a>: Sized where Self: 'a;   // released on drop (RAII; rules/test-fixture-raii)
    /// # Errors
    /// StoreError::RunLocked { run } when another driver holds the run (never retried), Io, Unsafe, Poisoned.
    fn lock_run(&self, run: &RunId) -> Result<Self::Guard<'_>, StoreError>;
}
```

- **File:** `runs/<hex>/driver.lock`, created through the store's existing `open_or_create`, which runs the symlink and inode checks, mode 0600. Call `File::try_lock()`. `Err(TryLockError::WouldBlock)` becomes `RunLocked`. Keep the `File` in the guard. It is a separate file from `ledger.lock`, so `minos stop` and `minos steer` can still append while a driver runs (04-CONTEXT D-11).
- **Memory:** a `Mutex<BTreeSet<RunId>>`. Insert or refuse; the guard removes on `Drop`.
- Add it to `tests/contract.rs` for both stores: a second lock is refused, and a lock taken after the first guard drops succeeds.

`flock` locks attach to the open file description. A probe this session showed that a second handle in the same process gets `WouldBlock` and succeeds after the first handle drops, so the file case is testable in-process. [VERIFIED: probe; pinned std `fs.rs` `try_lock` docs "flock ... LOCK_EX | LOCK_NB"] The driver takes the guard before its first ledger read, including before appending `RunStarted`, and holds it for the whole session. Suggested message: "run <hex> is being driven by another minos process; wait for it to finish or stop it with minos stop".

### Pattern 8: Safe points, stop semantics, and pins (D-10, D-11, D-14, Finding 3)

- **Session start:**
  - Lock.
  - Fold.
  - Refuse unless `state.root_image() == Some(lock image)` (D-10).
  - Refuse unless `state.manifest() == ManifestId::of(current manifest bytes)` (recommended; Assumption A6).
  - Refuse unless the baseline's recorded `Judged` events, if any, are under the current `JudgeVersion`. A judge re-pushed with `--update` under the same label would otherwise surface later as a confusing `IncumbentUnscored`.
  - `JobRunner::check` on the baseline request.
  - Record `session_stops = state.stops().collect::<BTreeSet<_>>()`.
- **Before each generation:** re-fold. A stop not in `session_stops` means exit. `circuit_breaker(&state, budget, streak)` returning `Some(r)` means append `StopRequested { g, r }` and exit. `spend_ok` returning `Stop(r)` does the same. `Err(CostError)` means exit with an error and record nothing.
- **Before each model call:** re-fold, check new stops and `spend_ok`.
- **Before each launch:** re-fold and check new stops.
- **After each trial's `Spent`:** `spend_ok`. Compute overshoot is at most one job (D-15).
- **D-11:** the `RunState::backend_class()` accessor returns `(BackendKind, Arch)` of the recorded judgments. Before appending `Judged`, refuse a `JobResult` whose class differs. That is a configuration error.

A breaker or cap stop re-trips on resume because the condition is re-evaluated, and the key `(generation, reason)` makes the repeat a `Duplicate`. That is correct, fail-closed behavior: the user must change something (prices, a new run). A `UserStop`, `ProviderLimit`, or `ProviderAuth` from an earlier session no longer blocks a resume.

### Pattern 9: Infrastructure retry with an injectable pause (D-16)

`DriverOptions { retry_pause: Duration }`, with the production default one named constant (for example `RETRY_PAUSE = Duration::from_secs(5)`, with its reason) and tests passing `Duration::ZERO`. The loop sleeps with `std::thread::sleep(options.retry_pause)`. A clippy ban on `thread::sleep` is therefore not wanted in `minos-loop`; ban `SystemTime::now` and `Instant::now` instead. The streak lives in the driver value. A store `Io` during `append` is safe to retry: a line that was written but whose `sync` failed is found by key on the retry and returns `Duplicate`. [VERIFIED: `crates/minos-store/src/file/ledger.rs` append path]

### Pattern 10: `CumulativeDiff` without mislabeling (D-12)

`core::sweep::render(parent, ..)` calls `diff_trees(parent, candidate)` to get a parent-relative step. `minos-propose::sweep::propose` then applies that step and calls `finish(base, parent, candidate)` for the cumulative diff. [VERIFIED: `crates/minos-core/src/sweep.rs` line 283, `crates/minos-propose/src/sweep.rs` lines 49-55] So retyping `diff_trees`'s return as `CumulativeDiff` would label the sweep's parent-relative step as cumulative, which is exactly the WR-03 hazard. Recommended API:

```rust
pub struct BaseTree { id: TreeId, tree: Tree }           // BaseTree::new(id, tree): the run's base surface
pub struct CumulativeDiff { base: TreeId, diff: Diff }   // private; no From<Diff>
pub fn diff_trees(base: &Tree, candidate: &Tree, ..) -> Result<Diff, ComposeError>;          // unchanged, generic
pub fn cumulative(base: &BaseTree, candidate: &Tree, ..) -> Result<CumulativeDiff, ComposeError>;
pub fn compose(base: &BaseTree, parent: Option<&CumulativeDiff>, proposal: &Diff, ..) -> Result<CumulativeDiff, ComposeError>;
impl CumulativeDiff {
    /// A stored diff re-read from its blob: parsed, then accepted only if it hashes to `exp` under `base`.
    pub fn recorded(base: TreeId, diff: Diff, exp: &ExperimentId) -> Result<Self, NotRecorded>;
    pub fn as_diff(&self) -> &Diff;  pub fn base(&self) -> TreeId;
}
impl ExperimentId { pub fn of(diff: &CumulativeDiff) -> Self /* hashes base then canonical bytes, unchanged */ }
impl Experiment { pub fn candidate(diff: CumulativeDiff) -> Self }
```

The hash bytes are unchanged, so `golden_ids` stays green. `recorded` serves every re-read site that today recomputes `ExperimentId::of(base, &diff)`: the guest (`workspace.rs:331`), the report (`report.rs:801`), `JobRequest::verify_diff`, and the loop rebuilding the incumbent's parent tree. A parent `Tree` passed where `&BaseTree` is required fails to compile, which is the D-12 test (a trybuild case or a doc compile-fail). Callers to update: the `Experiment::candidate` sites in `minos-guest/src/test_fixture.rs`, `minos-backend-krun/src/input_disk.rs` tests and `tests/harness/mod.rs`, `minos-core/tests/{golden_wire,guest_wire,prop_*}.rs`, and `minos-propose` (`Proposal` carries `CumulativeDiff`). The `minos-guest` change must still pass the musl clippy line.

### Pattern 11: `minos judge push` (D-01 to D-03, JUDGE-01, CLI-04)

Mirror `env_build.rs` step for step: read the manifest under `MAX_MANIFEST_BYTES`, pack, store, then read, compare, and write the lock through the synced temporary file plus rename (`write_lock`). [VERIFIED: `crates/minos-cli/src/env_build.rs`] The new `minos-source` entry point:

```rust
pub fn pack_judge_data(root: &Path) -> Result<JudgePack, SnapshotError>;  // no Manifest, no surface check, no clean check
pub struct JudgePack { tarball: Vec<u8>, tree: TreeId, files: BTreeMap<RelPath, BlobId>, report: PackReport }
```

- **Shared:** the walker (`.gitignore` and `.minosignore` inside `root`), the fixed 9-rule `DENYLIST`, the offender rules (symlink, hardlink, special, grammar, ustar, raced), the scanner, and `pack::tarball`.
- **Different:**
  - No manifest excludes, and **not** the judge-argv-path rule: that rule would drop the judge program from its own data.
  - Caps: 10,000 files, and a tarball of at most `MAX_JUDGE_DATA_BYTES` (100,663,296) checked on the tarball length, because that is the section the input disk carries. Per-file size up to that same cap, matching `UnpackCaps::JUDGE`.
  - No clean check: D-03 lists the hygiene and leaves the clean check out, and judge data is private, so it is usually not committed.
- **Recommended extra refusal:** every path in `manifest.judge().paths()` must be in the pushed tree, because the judge stage runs with its working directory at the unpacked data root (`run.rs` `stages(&judge_dir, ..)`). A missing judge program would otherwise fail every judge VM.
- **Store:** `store.put_reader(tarball.as_slice(), MAX_JUDGE_DATA_BYTES)`. The blob ID equals `TreeId::to_blob_id()`.
- **Lock:** `lock.judge(label)` equal means unchanged. A different tree with no `--update` is refused, printing both digests. Otherwise write `with_judge(entry)`.
- **Print:** the label, `sha256 <tree>`, the file count and bytes, and `JudgeVersion::of(&tree, manifest.judge())`.

The `[[judge]]` wire shape, from a `toml` 1.1.8 probe this session (it round-trips, and a lock with no `judge` key parses as empty):

```toml
[[judge]]
label = 1
tree = "<64 hex>"
files = 2
bytes = 10
minos = "0.1.0"

[[judge.file]]
path = "expected.txt"
sha256 = "<64 hex>"
```

Keep the `file` list strictly ascending by path, and refuse it otherwise, as `base_images` is, so each lock has one wire form. Labels are unique. `Lock::check` gains a `label` argument, or a sibling `check_judge(label)`, returning `LockMismatch::NoJudge { label }`.

### Pattern 12: The overlap refusal (D-04)

`snapshot(root, manifest, policy, judge_files: &BTreeSet<BlobId>)`. After the walk, before the scan, collect every packed path whose `BlobId::of(bytes)` is in `judge_files` and fail with `SnapshotError::JudgeOverlap { paths }` (sorted, listing capped as elsewhere at 20 plus "and N more"). The CLI passes the union of every `[[judge]]` entry's file digests. Pass it from `env build` too: its snapshot is the image build context, which JUDGE-01 also protects. The digest test must not flag empty files (Pitfall 8; Open Question 4).

### Anti-Patterns to Avoid

- **Re-proposing on resume because "fewer than batch were recorded".** D-07 forbids it, and it double-spends.
- **`ledger.append(..)?` on `Spent` without a presence check.** After D-08, a re-run trial's different usage is a stopping `Conflict` (Pitfall 1).
- **Reading time or resource numbers from guest output** (`OutputStatus::Packed { content_bytes }`, stage logs, `out/` files) into `Usage`. Use only `Finished::elapsed()` (LOOP-03).
- **Treating `SetupStatus::Failed { ScratchFull }` as infrastructure.** It fails identically every time. It is configuration.
- **Passing the candidate's whole output, logs included, to the judge**, or any file from the candidate to the host terminal unsanitized.
- **A backend enum or trait object** to make the runner testable. Use an in-test `FakeBackend` that implements `Backend`, as `crates/minos-sandbox/tests/backend.rs` does.
- **Storing derived run state.** The fold is the only state. The driver keeps in memory only the streak, the session's stop set, the pause, and the propose step's `dropped` until `Selected` is appended (or the batch blob, if adopted).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Selection, trial reduction, noise floor, breaker, spend | Loop-local `if`s | `minos_core::policy::{conclude, circuit_breaker, spend_ok, noise_floor, noise_check}` | Permutation-invariant and property-tested; resume and review see identical decisions |
| Tier routing | A stall counter in memory | `minos_core::route::route(manifest, state, surface)` | Pure function of the ledger (03-CONTEXT D-13) |
| Run state | Fields on the driver | `minos_core::state::fold` | Order-independent; the same fold the report and status read |
| Diff composition and the parent tree | String edits | `diff::apply::apply`, `compose`, `diff_trees` (with `CumulativeDiff`) | One strict applier shared by guest, adopt, and report |
| Output tar parsing | A reader in the runner | `Finished::into_output` (raw mode, caps) | Already fuzzed and property-tested (SBX-05) |
| Scorecard parsing and gate check | JSON in the runner | `Untrusted<GuestOutput<Judge>>::into_scorecard`, then `check_scorecard` | Byte cap before parse, duplicate refusal, gates first |
| Hostile text to a model or terminal | Escape stripping | `LogExcerpt::from_bytes`, `SafeText` | Allowlist renderer, Trojan-Source safe |
| Deterministic tarball for judge data | A second packer | `minos-source`'s walker plus `pack::tarball` (new judge entry point) | Same denylist, offender rules, scan, and ustar recipe |
| Lock file write | `fs::write` | `env_build.rs` `write_lock` (temp in `.minos/`, `sync_all`, `persist`, directory fsync) | Readers never see a partial lock |
| Process-level exclusion | PID files | `File::try_lock` (std, stable 1.89) | Released by the kernel on crash; no stale-PID logic |
| Money | Float math | `Cost::of`, `ApproxCost` | Pico-dollar exact; one display type |

**Key insight:** Everything that decides an outcome or a selection is already pure and tested in core, and everything that touches hostile bytes is already wrapped in sandbox. The new code is ordering, idempotency, and blame classification. Bugs here come from appending in the wrong order or without a presence check, not from computing the wrong number.

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `.minos/runs/<hex>/` gains `driver.lock` (no data; 0600). `.minos/blobs` gains judge data tarballs, guest job JSON blobs, candidate-out repacks, stage log tails, and batch blobs (if adopted). No existing ledger needs migration: no `minos run` has ever written one, and the new fields are additive `Option`s | Code only |
| Live service config | None. v1 has no service. Verified: no AWS pieces in the workspace | None |
| OS-registered state | None. No launchd, cron, or `_minos` user (deferred) | None |
| Secrets and env vars | `~/.config/minos/prices.toml` is user config, read by the CLI (D-13), not a secret. The VM test reads `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` only in ignored tests | None |
| Build artifacts | `target/` relinks `minos-vmm` and drops its entitlement; re-sign before the VM test. `cargo clean` at phase end (GATE-06) | Re-sign before the ignored VM test |

## Common Pitfalls

### Pitfall 1: A crash between a trial's `Spent` and its `Judged` turns resume into a stopping `Conflict`
**What goes wrong:** On resume the trial has no `Judged`, so it is re-run. Its host-measured usage differs, so `Spent { Candidate { exp, trial } }`, keyed only by `(run, generation, source)`, conflicts. After D-08 that is `Err(StoreError::Conflict)` and the driver stops "needing a human."
**How to avoid:** Append each `Spent` only when `state.has_spend(&SpendId::of(run, g, source))` is false. The first attempt's measurement stands, and the re-run's usage goes unrecorded: an undercount of exactly one job, which the docs should state beside D-15's overshoot. Keep the order Spent, Spent, then Judged (record before act).
**Warning signs:** A resume test that crashes between those appends and expects `Ok`.

### Pitfall 2: The drop count and partial batches vanish on resume (Finding 2)
**What goes wrong:** `Decision::dropped` lives only in memory until `Selected`, and `Proposed` lines are appended one at a time. A crash inside that stretch yields a different `Decision` or fewer experiments than an uninterrupted run.
**How to avoid:** The batch blob (Pattern 6), or the sweep-recompute fallback plus a documented exception.
**Warning signs:** The LOOP-05 property test fails only for crash points between the first `Proposed` and `Selected`.

### Pitfall 3: Old `StopRequested` records block every resume (Finding 3)
**What goes wrong:** `stop_requested()` returns the earliest stop forever.
**How to avoid:** Use session-relative stop handling (Pattern 8).
**Warning signs:** "Resume after `minos stop`" exits immediately.

### Pitfall 4: `Option` fields that are serialized as `null` change every golden
**What goes wrong:** Without `skip_serializing_if`, a `None` writes `"logs":null` or `"root_image":null`. The old golden lines no longer match, and the SHA-256 pin breaks.
**How to avoid:** `#[serde(default, skip_serializing_if = "Option::is_none")]` on each new field. A probe this session, in the exact ledger shape (a `Record` with `tag = "v"` over an externally tagged `Event` with `deny_unknown_fields`), confirmed four things: an absent field reads as `None`; `None` is omitted when serialized; unknown fields and bad `Stage` map keys are still refused; and `Some` round-trips. Add new golden cases (`record_v1_judged_logs`, `record_v1_run_started_root_image`, `job_result_judge_usage`, `lock` with `[[judge]]`) and recompute the pins. Update about 30 struct-literal sites in core and store tests (`Event::Judged {` and `Event::RunStarted {` in `prop_fold.rs`, `prop_keys.rs`, `prop_policy.rs`, `golden_wire.rs`, `report.rs`, `route.rs`, `breaker.rs`, and `minos-store/tests/file_ledger.rs`). [VERIFIED: probe; grep counts]

### Pitfall 5: `MAX_LOCK_BYTES` (64 KiB) overflows with per-file judge digests (Finding 4)
**What goes wrong:** Each `[[judge.file]]` entry takes about 100 bytes plus its path (toml probe). Around 550 files fill the lock, after which `env build`, `judge push`, and every run refuse it as too large.
**How to avoid:** Raise the cap with a const assertion tying it to the judge file cap times the per-entry worst case, or lower the judge file cap. The user decides (Open Question 3).

### Pitfall 6: Reusing `snapshot()` for judge data (Finding 5)
**What goes wrong:** `SurfaceMissing` on every judge directory, the judge program dropped by the judge-path rule, and data between 64 and 96 MiB refused by the source caps.
**How to avoid:** Pattern 11's sibling entry point.

### Pitfall 7: D-13 checks manifest model names, but `Spent` records resolved IDs
**What goes wrong:** `Proposals::models()` is keyed by the resolved IDs from `modelUsage` (CLAUDE.md: `Origin::Model` records the resolved model ID). `spend_ok` prices `SpendSource::ModelCall { model }` with `Cost::of`, which returns `CostError::UnpricedModel` when the table has only the alias. The run stops with an error after paying for its first call.
**How to avoid:** Keep D-13's start check, which is required. After every model call, re-run `spend_ok` before anything else, and turn `UnpricedModel { model }` into a clear refusal naming the model and `prices.toml`. Document that `prices.toml` must list the resolved IDs (for example the full model ID the CLI reports). The user may prefer recording `Spent` under the manifest's alias instead (Open Question 5). [VERIFIED: `crates/minos-propose/src/proposer.rs` `Proposals::models` docs; `money.rs` `Cost::of`]

### Pitfall 8: Empty and tiny files make D-04 refuse benign trees
**What goes wrong:** Every empty file has one SHA-256. If the judge data holds any empty file (or `1\n`), every source tree with an empty `__init__.py` or `.gitkeep` fails to pack.
**How to avoid:** Exempt zero-length files, which reveal nothing, or apply a minimum size. `limits::LITERAL_MIN_BYTES = 6` is the existing threshold for the related literal-match flag. This refines D-04, so it needs the user's agreement (Open Question 4).

### Pitfall 9: The secret scan and the denylist on judge data
**What goes wrong:** Datasets can hit the anchored secret patterns (fixtures with fake keys, long base64), and the denylist drops a file named `id_*` or `.env*`. D-03 grants no override.
**How to avoid:** List every drop in `push`'s output (`PackReport::denied`). Name the file, pattern, and line for scan hits, as `snapshot` does. The user then restructures the data. Never add an override silently.

### Pitfall 10: The guest's `Input` setup failure carries a placeholder role
**What goes wrong:** Mapping by `role` first turns an infrastructure failure into `Rejected(OutputInvalid)`.
**How to avoid:** Read `setup` first. `take_status` already skips its role check for `Input` failures (CR-02).

### Pitfall 11: Holding a store lock across a VM run
**What goes wrong:** Holding `ledger.lock` (the per-append lock) across a launch would block `minos stop`.
**How to avoid:** Only the whole-run `driver.lock` is held for the session. Appends take and release `ledger.lock` inside `append`.

### Pitfall 12: `IncumbentUnscored` surfaces late
**What goes wrong:** If no baseline trial scored, the run discovers it only when generation 1 concludes, after paying for a model call and VMs.
**How to avoid:** At `Step::BaselineReady`, call a pure `policy::baseline_ready` (a public wrapper over the private `valid_score` with `TrialRule::ScoredOnly` and the objective metric). Stop with a driver error that names the baseline trials' outcomes.

### Pitfall 13: The ignored VM test cannot live in `minos-runner`
**What goes wrong:** `deny.toml` lets only `minos-cli` depend on `minos-backend-krun`, dev-dependencies included.
**How to avoid:** Put it in `crates/minos-cli/tests/runner_vm.rs`. Rebuild the fixture image there with the public `minos_backend_krun::rootfs::{pack_root_image, check_guest_elf, PackLimits}` and a test rootfs holding `minos-probe` (the backend harness's `test_rootfs` is test-private). The probe's `scorecard` command writes only `probe_ok`, so a manifest with an objective gets `Rejected(MissingMetric)`. Add a `scorecard <value>` form to the probe (test fixture only) so the test can assert `Scored`, and keep a gate-failing case for JUDGE-03. [VERIFIED: `crates/minos-guest/src/bin/minos-probe.rs` dispatch; harness `test_rootfs`; `deny.toml`]

## Code Examples

Current signatures the plans build against, read this session. [VERIFIED: source paths given]

```rust
// crates/minos-core/src/state.rs
pub fn fold<'a>(records: impl IntoIterator<Item = &'a Record>) -> Result<RunState, FoldError>;
impl RunState {
    pub const fn run(&self) -> &RunId;          pub fn manifest(&self) -> &ManifestId;
    pub fn manifest_at(&self, generation: u32) -> &ManifestId;
    pub const fn base(&self) -> &TreeId;        pub const fn baseline(&self) -> &ExperimentId;
    pub fn proposal(&self, exp: &ExperimentId) -> Option<&Proposal>;
    pub fn proposals_in(&self, generation: u32) -> impl Iterator<Item = (&ExperimentId, &Proposal)>;
    pub fn trials(&self, exp: &ExperimentId, judge: &JudgeVersion) -> impl Iterator<Item = (u32, &Judgment)>;
    pub fn decision(&self, generation: u32) -> Option<&Decision>;
    pub fn last_concluded_generation(&self) -> u32;   // 0 when none
    pub fn incumbent_before(&self, generation: u32) -> ExperimentId;
    pub fn notes(&self, generation: u32) -> Vec<BlobId>;
    pub fn stop_requested(&self) -> Option<(u32, StopReason)>;  // earliest only (Pitfall 3)
    pub fn spends(&self) -> impl Iterator<Item = (&SpendId, &Spend)>;
    pub const fn total_usage(&self) -> &Usage;
}
// crates/minos-core/src/policy.rs
pub fn check_scorecard(judge: &JudgeSpec, card: &Scorecard) -> Result<(), Reason>;
pub fn conclude(state: &RunState, manifest: &Manifest, judge: &JudgeVersion, generation: u32, dropped: u32)
    -> Result<Decision, PolicyError>;              // GenerationZero | IncumbentUnscored | TooManyProposals
pub fn circuit_breaker(state: &RunState, budget: &Budget, infra_streak: u32) -> Option<StopReason>;
pub fn spend_ok(state: &RunState, manifest: &Manifest, prices: &Prices) -> Result<SpendCheck, CostError>;
pub fn noise_floor(state: &RunState, judge: &JudgeVersion, metric: &MetricName) -> Option<NoiseFloor>;
// crates/minos-core/src/route.rs
pub fn route(manifest: &Manifest, state: &RunState, surface: &SurfaceStats) -> Result<Route, RouteError>;
// crates/minos-core/src/job.rs
impl JobRequest {
    pub fn baseline(base: TreeId, judge: JudgeVersion, trial: u32, budget: JobBudget) -> Result<Self, JobError>;
    pub fn candidate(experiment: &Experiment, diff_blob: BlobId, judge: JudgeVersion, trial: u32,
                     budget: JobBudget) -> Result<Self, JobError>;
    pub fn verify_diff(&self, diff: &Diff) -> bool;
}
impl JobResult { pub fn new(request: &JobRequest, outcome: Outcome, usage: Usage, backend: BackendInfo,
                            logs: BTreeMap<Stage, BlobId>) -> Result<Self, JobError>; } // refuses tokens
impl JobBudget { pub const fn from_budget(budget: &Budget) -> Self; }
// crates/minos-core/src/guest.rs
impl GuestJob { pub fn candidate(manifest: &Manifest, request: &JobRequest) -> Self;
                pub fn judge(manifest: &Manifest, request: &JobRequest) -> Self;
                pub fn to_json(&self) -> Result<Vec<u8>, GuestJobError>; }
pub const MAX_JUDGE_DATA_BYTES: u64 = 100_663_296;  pub const MAX_CANDIDATE_OUT_BYTES: u64 = MAX_OUTPUT_STREAM_BYTES;
// crates/minos-sandbox/src
impl<R: Role> SandboxSpec<R> { pub fn new(job: &JobRequest, root: ImageDigest, input: InputDisk, machine: Machine,
                                          arch: Arch) -> Result<Self, SpecError>;
                               pub fn seal(self) -> LaunchPlan<R>; }
impl SandboxSpec<Judge> { pub fn secrets(self, data: JudgeData) -> Self; }
impl InputDisk { pub const fn candidate(job: BlobId, base: TreeId, diff: Option<BlobId>) -> Self;
                 pub const fn judge(job: BlobId, candidate_out: BlobId) -> Self; }
impl JudgeData { pub const fn new(blob: BlobId) -> Self; }
impl Machine { pub const fn new(vcpus: u8, memory_mib: u16) -> Result<Self, SpecError>; }
pub fn preflight<R: Role>(host: &Capabilities, plan: LaunchPlan<R>) -> Result<Cleared<R>, Vec<Refusal>>;
impl Capabilities { pub fn backend_info(&self, root_image: ImageDigest) -> BackendInfo; }
pub trait Backend { type Running<R: Role>: RunningVm<R>; fn capabilities(&self) -> Capabilities;
                    fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>; }
pub trait RunningVm<R: Role> { fn wait(self) -> Result<Finished<R>, InfraError>; }
impl<R: Role> Finished<R> { pub const fn ended(&self) -> Ended; pub const fn elapsed(&self) -> Duration;
    pub fn console_excerpt(&self, max: usize) -> LogExcerpt;
    pub fn into_output(self) -> Result<Untrusted<GuestOutput<R>>, OutputFault>; }
impl<R: Role> Untrusted<GuestOutput<R>> { pub fn take_status(&mut self) -> Result<GuestStatus, StatusFault>;
    pub fn take(&mut self, path: &RelPath) -> Option<Untrusted<OutputFile<R>>>; }
impl Untrusted<GuestOutput<Judge>> { pub fn into_scorecard(self, limits: &Limits) -> Result<Scorecard, ScorecardFault>; }
impl<R: Role> Untrusted<OutputFile<R>> { pub fn into_log_excerpt(self, max: usize) -> LogExcerpt; }
pub const BUILD_LOG_PATH: &str = ".minos/build.log"; // RUN_LOG_PATH, JUDGE_LOG_PATH, SCORECARD_PATH, OUT_DIR = "out"
// crates/minos-store/src/lib.rs
pub trait Ledger { fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError>;
                   fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError>; }
pub trait Blobs { fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError>;
                  fn put_reader<R: Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError>;
                  fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, StoreError>;
                  fn contains(&self, id: &BlobId) -> Result<bool, StoreError>; }
// crates/minos-propose/src/proposer.rs
impl<P: ModelProvider> Proposer<P> { pub fn propose(&self, spec: &ProposerSpec, input: &ProposeInput<'_>)
                                         -> Result<Proposals, ProposeError>; }
pub struct ProposeInput<'a> { pub manifest: &'a Manifest, pub run: &'a RunId, pub generation: u32,
    pub base: &'a Tree, pub parent: &'a Tree, pub program: &'a [u8], pub history: &'a History,
    pub notes: &'a [SteerNote] }
impl ProposeError { pub const fn stop_reason(&self) -> Option<StopReason>; pub const fn is_infra(&self) -> bool;
                    pub const fn usage(&self) -> Option<&Usage>; }
// crates/minos-backend-krun/src/backend.rs, host.rs ; crates/minos-cli/src/blob_source.rs
impl<B: BlobSource> KrunBackend<B> { pub fn new(paths: HostPaths, blobs: B) -> Result<Self, LaunchError>; }
impl HostPaths { pub fn new(vmm: impl Into<PathBuf>, guest: impl Into<PathBuf>, krunfw: impl Into<PathBuf>) -> Self; }
impl FileBlobSource { pub fn open(project: &Path) -> Result<Self, StoreError>; }
```

The new-field serde shape (verified by probe):

```rust
Judged {
    exp: ExperimentId, judge: JudgeVersion, trial: u32, outcome: Outcome, backend: BackendInfo,
    /// The capped raw stage logs (D-09). Absent on lines written before Phase 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logs: Option<BTreeMap<Stage, BlobId>>,
},
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Appended::Conflict` as an `Ok` value | `Err(StoreError::Conflict { seq })` | D-08, this phase | Every `append` call site must handle it; the contract test's first-write-wins property is rewritten |
| `fd-lock`/`fs2` for advisory locks | `std::fs::File::{lock, try_lock}` | Rust 1.89 (2025) | No dependency; already used for `ledger.lock` |
| One `usage` per job | Candidate and judge usage separately | This phase | Matches the frozen `SpendSource::{Candidate, Judge}` |

**Deprecated/outdated:** CLAUDE.md's Source paragraph note that comparing packed files with judge data "waits for Phase 7". D-04 implements it, so update the note in the same change.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The user approves two new capped sink exits on `Untrusted` (Pattern 4) | Pattern 4 | Without them LOOP-01 is unimplementable and D-09 can store only sanitized excerpts |
| A2 | A judge stage that exits non-zero maps to `Crashed { stage: Judge }`; a candidate `Ended::Crashed` maps to stage `Run` | Pattern 2 | Wrong blame wording in history and report; a new `Reason` variant would need the user |
| A3 | Stage `Signaled` and `SpawnFailed` map to `BuildFailed` or `RunFailed` (not `Crashed`) | Pattern 2 | Only labels change; all are candidate faults, never retried |
| A4 | Candidates get 1 trial each in v1 | Pattern 5 | More trials cost VMs; one trial leaves selection noisier (mitigated by `min_improvement` and the noise check) |
| A5 | A fixed vCPU constant (recommend 2, within `MAX_VCPUS` 8) for both roles, no manifest field | Pattern 3 | Builds may be slow at 1; usage scales with it |
| A6 | Resume refuses a changed manifest (`ManifestId` mismatch) and a changed `JudgeVersion` | Pattern 8 | A user who edits a budget mid-run must start a new run |
| A7 | `[judge] trials` range is 1 to `MAX_TRIALS` (32), default 3; excluded from `JudgeVersion` | Standard Stack | Including it would change `JudgeVersion` goldens; a range of 2 minimum would forbid a fast single-trial setup |
| A8 | Retry pause 5 s; log blob cap 262,144 bytes (tail) | Patterns 4 and 9 | Tuning only |
| A9 | `judge push` applies `.gitignore` and `.minosignore` inside the pushed directory, and refuses a push missing a judge argv path | Pattern 11 | A user's ignore file could silently exclude data; mitigated by listing drops and counts in the output |

## Open Questions (RESOLVED)

All seven were answered at the planning checkpoint on 2026-10-22 and recorded in 07-CONTEXT.md under "Research amendments" (D-17 to D-20 and the resolved-as-discretion list).

1. **Approve the new `Untrusted` exits (Finding 1)?** RESOLVED: approved as recommended (07-CONTEXT D-17).
   - What we know: no current exit can produce the judge's `candidate_out` blob or a raw log blob. CLAUDE.md requires stopping to ask.
   - Recommendation: approve `store_judge_input` (an `out/`-only canonical repack) and `store_tail(max)`, both closure sinks returning only a `BlobId`. Make the first runner plan open with a `checkpoint:human-verify` task on the exact signatures.
2. **How should LOOP-05 hold across the propose-to-select window (Finding 2)?** RESOLVED: `Proposed { batch: Option<BlobId> }` plus the batch blob (07-CONTEXT D-18).
   - Recommendation: add `Proposed { batch: Option<BlobId> }` (additive, `"v":"1"`) plus a batch blob. The fallback is sweep recompute plus a documented model-tier exception.
3. **Lock size for per-file judge digests (Finding 4).** RESOLVED: raise `MAX_LOCK_BYTES` with a const assertion and reword D-02 (07-CONTEXT resolved-as-discretion list).
   - Recommendation: raise `MAX_LOCK_BYTES` to about 4 MiB with a const assertion, and reword D-02's "reveals nothing" to say the per-file list exposes paths, and that the digests of low-entropy files are guessable. Alternatively keep the per-file list in `.minos/` and only the tree digest in the lock, which reverses part of D-02.
4. **D-04 and empty or tiny files.** RESOLVED: zero-length files only are exempt (07-CONTEXT D-20).
   - Recommendation: exempt zero-length files at least. Possibly also files under `LITERAL_MIN_BYTES`.
5. **Model naming for spend.** RESOLVED: `Spent` stays under the resolved model ID; the price check is extended (07-CONTEXT resolved-as-discretion list).
   - Recommendation: keep `Spent` under the resolved ID (06 decision), require `prices.toml` to price it, and fail clearly after the first call if not (Pitfall 7). The alternative is recording under the manifest alias.
6. **Stop semantics on resume (Finding 3).** RESOLVED: session-relative stops (07-CONTEXT D-19).
   - Recommendation: session-relative stops (Pattern 8). This refines D-14's "any `StopRequested`".
7. **Phase 8 interface points to fix now:** RESOLVED: the first four bullets are fixed as listed (07-CONTEXT resolved-as-discretion list); the last bullet is DEFERRED to Phase 8.
   - The driver takes an absolute target generation.
   - The generation-0 step returns the noise floor for `check`.
   - `minos run` applies its `--allow-noisy` refusal after generation 0.
   - The observer events carry IDs, the tier, the `Outcome`, and the reduced delta; the CLI supplies elapsed time from its own clock.
   - DEFERRED to Phase 8: whether the judge needs host-measured timing for speed objectives. Phase 8's sample says "harness measurements". If so, the repack could later add a host-written entry, which is a design question for Phase 8.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All | ✓ | 1.97.1 (pinned), `aarch64-unknown-linux-musl` listed in `rust-toolchain.toml` | — |
| cargo-deny | Gate | ✓ | 0.20.2 | — |
| cargo-insta | Accepting new goldens deliberately | ✓ | installed | `INSTA_UPDATE=always cargo test` |
| libkrunfw | Ignored VM test | ✓ | 5.6.2 at `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib` | — |
| `/usr/bin/sandbox-exec`, `/usr/bin/codesign` | Ignored VM test, sign `minos-vmm` | ✓ | macOS 27.0.1 | — |
| Docker buildx | Not needed (the VM test packs its own fixture rootfs) | — | — | — |
| `claude` login | Not needed (no live model call in this phase) | — | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (toolchain 1.97.1), `proptest` 1.11.0, `insta` 1.49.0 (`.config/insta.yaml` `update: no`), `trybuild` 1.0.122 |
| Config file | `.config/insta.yaml`; snapshots in `crates/minos-core/tests/snapshots/` (extend) |
| Quick run command | `cargo test -p minos-loop` / `cargo test -p minos-runner` (per touched crate) |
| Full suite command | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, then the C-01 loop and the C-02 grep |
| VM (manual, after build and sign) | `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LOOP-01 | Candidate then judge; each table row of Pattern 2 maps exactly; judge launched only after a passing candidate | unit (pure map) + integration (fake `Backend`) | `cargo test -p minos-runner --test pipeline --test map` | ❌ Wave 0 |
| LOOP-01 | Only `minos-runner` opens `Untrusted` in production code | grep check | `grep -rlnE 'take_status\|into_scorecard\|into_output\|store_judge_input\|store_tail' crates/*/src` lists only `minos-runner/src` and `minos-sandbox/src` | ❌ (command) |
| LOOP-02 | Judge plan carries judge data; candidate plan's input names none; judge input holds only `out/` files | integration (fake backend records plans) + existing trybuild | `cargo test -p minos-runner --test pipeline` ; `cargo test -p minos-sandbox --test compile_fail` | ❌ / ✅ |
| LOOP-03 | Usage equals elapsed × machine; forged timing files ignored; tokens refused | integration | `cargo test -p minos-runner --test pipeline host_usage` | ❌ Wave 0 |
| LOOP-04 | Generation flow over `MemoryStore` + `FakeProvider` (sweep and single-shot) + `FakeJobRunner`; admission dedupe; `Selected` appended | integration | `cargo test -p minos-loop --test flow` | ❌ Wave 0 |
| LOOP-04 | `next_step` is a pure function of the fold (permuted records give the same step) | proptest | `cargo test -p minos-loop --test stage` | ❌ Wave 0 |
| LOOP-05 | Crash after the Nth append, for every N: the resumed run folds to the same `RunState` as an uninterrupted one (sweep, exact); model tier: same decisions, extra `Spent` only for a pre-`Proposed` crash | integration (crash-injecting store wrapper) | `cargo test -p minos-loop --test resume` | ❌ Wave 0 |
| LOOP-06 | Second `lock_run` refused (`RunLocked`) on both stores; released on drop; second driver refused | contract + integration | `cargo test -p minos-store --test contract run_lock` ; `cargo test -p minos-loop --test lock` | ❌ Wave 0 |
| LOOP-07 | Spend and token caps stop before a generation and before a model call with `StopRequested { SpendCap \| TokenCap }`; overshoot at most one step | integration | `cargo test -p minos-loop --test caps` | ❌ Wave 0 |
| LOOP-08 | K empty generations then `NoValidCandidates`; K consecutive infra failures (fake runner and provider) then `InfraFailures`; a success resets the streak; zero pause in tests | integration | `cargo test -p minos-loop --test breaker` | ❌ Wave 0 |
| LOOP-09 | No loop test needs a VM or a login | structural | `cargo test -p minos-loop` runs in the gate | ❌ Wave 0 |
| JUDGE-01 | Push packs, stores, and writes `[[judge]]`; `--update` gate; source and env-build snapshots refuse overlap naming paths; over-96-MiB refused | integration (temp project) + unit | `cargo test -p minos-cli --test judge_push` ; `cargo test -p minos-source --test judge` | ❌ Wave 0 |
| JUDGE-01 | `[[judge]]` golden + pin; empty array omitted; duplicate label and non-ascending files refused; `Lock::check` `NoJudge` | golden + unit | `cargo test -p minos-core --test lock` | ✅ (extend) |
| JUDGE-03 | Gate value ≠ 1 gives `Rejected(GateFailed)` with no scorecard recorded | unit + integration | `cargo test -p minos-runner --test map gate` ; `cargo test -p minos-core --test prop_policy` | ❌ / ✅ |
| JUDGE-04 | `Judged.backend` from `Capabilities::backend_info`; a differing `(backend, arch)` is refused as configuration | integration | `cargo test -p minos-loop --test pins` | ❌ Wave 0 |
| CLI-04 | `minos judge push <dir> [--update]` in the command tree; `debug_assert` | unit | `cargo test -p minos-cli` | ✅ (extend) |
| D-05 | `[judge] trials` default 3, range-checked, absent from `JudgeVersion` (golden IDs unchanged) | unit | `cargo test -p minos-core --test manifest --test golden_ids` | ✅ (extend) |
| D-08 | `Conflict` is `Err`, non-retryable, and writes nothing (both stores) | contract | `cargo test -p minos-store --test contract` | ✅ (rewrite cases) |
| D-09, D-10 | New `Option` fields: old goldens byte-identical; new golden cases; fold carries `root_image` | golden | `cargo test -p minos-core --test golden_wire --test prop_fold` | ✅ (extend) |
| D-12 | Parent tree cannot mint a `CumulativeDiff`; `recorded` refuses a mismatched ID | trybuild or unit | `cargo test -p minos-core --test cumulative` | ❌ Wave 0 |
| End to end | Production runner on real VMs: baseline `Scored`; a gate-failing judge `Rejected(GateFailed)`; `Judged` backend krun/arm64 | ignored VM | `cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched crate>` (and `-p minos-core --test golden_wire --test lock` for wire tasks)
- **Per wave merge:** the full gate command list
- **Phase gate:** full gate green, C-01 and C-02 clean, the ignored VM test run once by hand after build, sign, and the musl guest build (results recorded in an evidence file), then `cargo clean` (GATE-06)

### Wave 0 Gaps
- [ ] `crates/minos-loop/tests/support/mod.rs`: `FakeJobRunner` (scripted `JobResult` or `JobFailure` per `(exp, trial)`), `CrashingStore<S>` (wraps `Ledger + Blobs + RunLock`, fails the Nth append with a non-retryable error), and fixture manifests (sweep and single-shot). Every item must be used by each target that declares `mod support;`, or `-D warnings` fails on dead code (the `tests/harness` lesson).
- [ ] `crates/minos-runner/tests/support/mod.rs`: `FakeBackend` returning per-role scripted `Finished` (pattern from `crates/minos-sandbox/tests/backend.rs`), and a ustar output builder (`tar` dev-dep).
- [ ] `crates/minos-runner/clippy.toml` and `crates/minos-loop/clippy.toml`: HashMap/HashSet, `SystemTime::now`, `Instant::now`, `std::env::var`, `std::process::Command`. Prove each ban fires once and record the proof in an evidence file, as earlier phases did.
- [ ] `crates/minos-core/tests/snapshots/`: new golden cases plus pin updates (`golden_wire`, `lock`).
- [ ] `crates/minos-cli/tests/runner_vm.rs`: fixture rootfs with `minos-probe`, plus a `scorecard <value>` probe form in `crates/minos-guest/src/bin/minos-probe.rs`.

## Security Domain

### Applicable ASVS Categories (Level 1)

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V1 Architecture | yes | Strict crate lanes (`deny.toml`); role typestate; the loop never sees sandbox types |
| V2 Authentication | no | No authentication in this phase (provider auth is Phase 6's preflight) |
| V3 Session Management | no | — |
| V4 Access Control | yes | Only `SandboxSpec<Judge>` attaches judge data (trybuild); `preflight` fail-closed; `driver.lock` exclusion; the judge sees only `out/` |
| V5 Input Validation | yes | `Untrusted` exits with caps; `Diff::parse` plus `CumulativeDiff::recorded`; `GuestStatus::parse_json`; `Scorecard::parse_json` then `check_scorecard`; `RelPath` grammar in `judge push`; lock and manifest `try_from` plus `deny_unknown_fields` |
| V6 Cryptography | yes (hashing only) | SHA-256 through `IdHasher`/`BlobId::of` (`sha2`); never hand-rolled |
| V7 Error Handling and Logging | yes | Errors carry IDs, fixed text, and validated paths, never sandbox or model bytes; stage logs reach readers only through `LogExcerpt` |
| V8 Data Protection | yes | Judge data never on a candidate's disk, in the source tarball, or in the image context (D-04 overlap); lock digest disclosure caveat (Open Question 3) |
| V11 Business Logic | yes | Spend and token caps at safe points; circuit breaker; record before act; idempotent keys |
| V12 Files and Resources | yes | `driver.lock` through the store's symlink and inode checks; judge walk refuses symlinks, hardlinks, and specials and re-checks identity on read; caps before allocation |
| V14 Configuration | yes | Prices required (D-13); root image, manifest, judge, and backend pins on resume; no override flags for hygiene |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Candidate forges a scorecard or timing in its own output | Spoofing / Tampering | Scorecard only from `Untrusted<GuestOutput<Judge>>`; usage only from `Finished::elapsed()`; candidate `out/scorecard.json` is just an artifact |
| Candidate crafts artifacts or logs to attack the judge VM or a later prompt | Tampering / Elevation | Separate judge VM, `out/` only, canonical repack under caps; schema-fixed scorecard; logs through `LogExcerpt` and untrusted fences in prompts |
| Judge data leaks into the candidate's view | Information disclosure | Typestate (`secrets` only on Judge), `JudgeDataIsInput` refusal, source and env-build overlap refusal, denylist plus judge-path exclusion |
| Two drivers interleave appends on one run | Tampering | `driver.lock` (`try_lock`), per-append `ledger.lock`, key dedupe, `Conflict` stops |
| Resumed step rewrites history | Repudiation / Tampering | First write wins; `Conflict` is an error; presence checks before `Spent` and `Proposed` |
| Unbounded spend through retries or loops | Denial of service (billing) | Metered `Spent` for failed calls with usage; caps before each call and generation; breaker on empty generations and the infra streak; `ProviderLimit` and `ProviderAuth` stop, never retry |
| Oversized judge data or logs exhaust memory or disk | Denial of service | 96 MiB tarball cap checked before store; log tail cap; output caps already enforced by `read_output` |
| Path tricks in the pushed judge directory | Tampering | Walker refuses symlinks, hardlinks, and specials, the grammar, and ustar overflow; identity re-check (raced) |
| Low-entropy judge file digests in a public lock | Information disclosure | Open Question 3 (document, or keep per-file digests out of the committed lock) |

## Sources

### Primary (HIGH confidence)
- Source read this session: `crates/minos-core/src/{state,ledger,policy,job,experiment,guest,lock,money,usage,outcome,route,sweep,report,manifest,id,limits,text}.rs` and `diff/compose.rs`; `crates/minos-store/src/{lib,memory,file/ledger}.rs` and `tests/contract.rs`; `crates/minos-sandbox/src/{lib,spec,preflight,finished,backend,untrusted,output,blob_source}.rs` and `tests/backend.rs`; `crates/minos-propose/src/{proposer,proposal,provider,fake,sweep,single_shot,prompt/history}.rs`; `crates/minos-source/src/{lib,walk}.rs`; `crates/minos-cli/src/{main,lib,env_build,blob_source}.rs`; `crates/minos-backend-krun/src/{backend,supervise}.rs` and `tests/{vm.rs,harness/mod.rs}`; `crates/minos-guest/src/{run,workspace,pack}.rs` and `bin/minos-probe.rs`; `Cargo.toml`, `deny.toml`, the crate `clippy.toml` files, `.config/insta.yaml`
- Pinned std 1.97.1 source `library/std/src/fs.rs`: `TryLockError`, `File::try_lock` (flock `LOCK_EX|LOCK_NB`)
- Run probes this session (scratchpad, `--offline`, workspace pins): serde `Option` field evolution in the ledger shape; `File::try_lock` across two handles in one process; `toml` 1.1.8 `[[judge]]`/`[[judge.file]]` serialization and size per entry
- Planning record: 07-CONTEXT.md and 07-DISCUSSION-LOG.md; REQUIREMENTS.md; ROADMAP.md §Phase 7 and §Phase 8; 03-REVIEW WR-03; 04-CONTEXT D-11 and Deferred; 04-REVIEW IN-03; 04-RESEARCH Pitfall 4; 06-RESEARCH; `.planning/research/PITFALLS.md` 9, 10, 11, 18, 19, 28

### Secondary (MEDIUM confidence)
- None needed. Every external fact used is a pinned crate or std behavior, verified by source or probe.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. No new crates; every pin is in `Cargo.lock` with production precedent.
- Architecture: HIGH for mechanisms and current signatures; MEDIUM for the recommended API shapes, which are discretion areas.
- Pitfalls: HIGH. Each is derived from current code paths and, where noted, a probe.
- Open questions: five need the user (Findings 1 to 5) before plans are locked.

**Research date:** 2026-10-22
**Valid until:** 30 days (stable internal code); re-check if any of Phases 1 to 6's crates change before planning
