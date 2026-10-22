# Phase 7: Runner and Loop - Context

**Gathered:** 2026-10-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Generations run end to end, and an interrupted, over-budget, or failing run resumes or stops from the ledger with the same result as an uninterrupted one. Concretely, this phase delivers:

- `minos-runner`: for one `JobRequest`, build and seal the Candidate spec, launch, wait, store the candidate's output as a blob, build and seal the Judge spec with the judge data attached through `secrets`, launch, wait, and turn the two `Untrusted<GuestOutput<R>>` values into one `JobResult` (an `Outcome`, host-measured `Usage`, `BackendInfo`, per-stage log blobs). It is generic over `B: Backend`, is the only crate that converts `Untrusted` output into an `Outcome`, and never reads time or resource numbers from candidate output (LOOP-01, LOOP-02, LOOP-03, JUDGE-03, JUDGE-04).
- `minos-loop`: the generation driver over `Ledger`, `Blobs`, `Proposer<P>`, and a `JobRunner` seam the loop defines: route the tier, propose, admit diffs, run jobs, fold `Judged`, `conclude`, append `Selected`, with every effect keyed by content ID and recorded before the next act; the whole-run `driver.lock`; the spend, token, breaker, and stop checks at fixed safe points; resumable from the ledger; tested entirely against `MemoryStore`, `FakeProvider`, and a fake job runner with no VM (LOOP-04 to LOOP-09). It depends only on `minos-core`, `minos-store`, and `minos-propose`.
- `minos judge push <dir>`: judge data packed into the blob store as a judge version under the manifest's `[judge] version` label, recorded in `minos.lock`, never in the source tarball or the image build context (JUDGE-01, CLI-04).
- The core and store changes the decisions below name: `[judge] trials`, `Judged { logs }`, `RunStarted { root_image }`, the `[[judge]]` lock table, `CumulativeDiff`, and `StoreError::Conflict`, each with its golden case.

Out of scope: the `minos run`, `status`, `log`, `show`, `stop`, `check`, `report`, and `adopt` commands and the progress line (Phase 8; this phase ships the library driver and `judge push` only), concurrent candidates within a generation (NEXT-04), `minos rejudge` and `minos steer` (NEXT-01, NEXT-02; the events exist), the `planned` and `agentic` tiers (TIER-01), and every AWS piece.

Requirements: LOOP-01 to LOOP-09, JUDGE-01, JUDGE-03, JUDGE-04, CLI-04. Cross-cutting: GATE-06 (`cargo clean` after verification passes) and `rules/` adherence. The roadmap marks this phase "standard patterns; skip research-phase".

</domain>

<decisions>
## Implementation Decisions

### Judge data push and versioning (JUDGE-01, CLI-04)
- **D-01:** The label comes from the manifest. `minos judge push <dir>` reads `[judge] version` from `minos.toml` and registers the directory under that integer label. Publishing a new version means bumping the manifest, then pushing. Pushing different data under a label that is already registered is refused unless `--update` is passed, and the refusal prints both tree digests, the same gate `minos env build` applies to the root image (05-CONTEXT D-10). No `--version` flag and no auto-increment.
- **D-02:** The label-to-data mapping lives in `minos.lock`, as one `[[judge]]` table per label beside the `[[image]]` tables: the label, the data tree digest (the `TreeId`), the file count, the total bytes, the per-file path and SHA-256 list (D-04), and the `minos` version that pushed it. The lock stays TOML with its `v` tag, `deny_unknown_fields`, and `try_from`; a missing `[[judge]]` array parses as empty, so existing lock files stay valid, and the golden file gains a case. `Lock::check` grows to refuse a run whose manifest label has no entry. The data itself stays in `.minos/blobs` (04-CONTEXT D-10); the lock holds digests only, so committing it reveals nothing, and a fresh clone must push the data again.
- **D-03:** The judge directory is packed by `minos-source`'s deterministic ustar packer, exactly as the source snapshot is: regular files only, sorted paths, zeroed metadata, the hard denylist, and the secret scan, with no override. The tarball's SHA-256 is the `TreeId` that `JudgeVersion::of(data, judge_spec)` hashes, so the recorded `JudgeVersion` is a pure function of the pushed bytes and the manifest's judge spec. Judge data sits unencrypted in `.minos/blobs` and inside every judge VM, so the same fail-closed hygiene applies. A directory over `MAX_JUDGE_DATA_BYTES` (96 MiB) is refused, never truncated (CLAUDE.md Open questions).
- **D-04:** `push` records per-file digests, and the source pack refuses overlap. The `[[judge]]` entry lists each file's path and SHA-256 (bounded, because the data is capped). `minos run`'s source pack receives the judge file digest set from the lock through the CLI and refuses, naming every offending path, when a packed file's digest equals any judge file's digest (PITFALLS Pitfall 28): a judge fixture copied into the surface would let a candidate see what it is scored on. This closes the "waits for Phase 7" note in CLAUDE.md's Source paragraph.

### Baseline calibration and resume (LOOP-05, LOOP-06, LOOP-09, JUDGE-02 groundwork)
- **D-05:** The baseline trial count is a manifest field, `[judge] trials` (working name), default 3, range-checked in `Manifest::try_from` against `MAX_TRIALS`. Generation 0 of every run performs that many baseline trials, each a `Judged { baseline, trial }` with its `Spent` records, and the noise floor folds from them (03-CONTEXT D-12). It is a project fact, like the memory budget (05-CONTEXT D-06), so `minos check` can print it beside the floor.
- **D-06:** Every run calibrates itself. `run` always performs its own generation 0 before generation 1, so a run's ledger is self-contained and `build_report` never reads another run. `minos check` (Phase 8) drives the same generation-0 code on a diagnostic run of its own to print the noise floor and the route table; it never creates the run that `run` later resumes. The loop therefore exposes generation 0 as a callable step.
- **D-07:** Resume stages from the ledger and never re-proposes. A generation's stage is a pure function of the fold: no `Proposed` for it means propose, with the call's `Spent` appended first and then one `Proposed` per surviving item; `Proposed` present means run every `(exp, judge, trial)` that has no `Judged`, keyed by `LaunchKey`; every trial judged means `conclude` and append `Selected`. A crash between the `Spent` and the first `Proposed` makes the next attempt propose again under the next call index, so spend is counted and nothing is recorded twice. With fakes, an interrupted run folds to the same state as an uninterrupted one (LOOP-05), and a second driver on the same run is refused by the exclusive `driver.lock` under `runs/<run-hex>/` (LOOP-06; 04-CONTEXT D-11).
- **D-08:** `Appended::Conflict` becomes a store error. `Appended` keeps `New` and `Duplicate`; a same-key append with a different record returns `Err(StoreError::Conflict { seq })`, non-retryable like `Corrupt`, with the contract test updated for both stores (04-REVIEW IN-03). The loop cannot `?` past it: the driver exits naming the run, the seq, and the event kind, records nothing, and `minos status` (Phase 8) says the ledger needs a human. The first write still wins on disk.

### Ledger provenance additions (LOOP-04, JUDGE-04, 05 and 06 deferrals)
- **D-09:** `Judged` gains `logs: Option<BTreeMap<Stage, BlobId>>`, naming the capped raw stage logs the runner stored as blobs from `JobResult::logs`. An `Option` field keeps every existing line parseable under `"v":"1"` (CLAUDE.md ledger rule); the key stays `(exp, judge, trial)`; the golden file gains a case. Every reader, the prompt's failure excerpts (06-CONTEXT D-08), `minos show`, and the report, still goes through `LogExcerpt`, so the blob's hostile bytes never reach a terminal or a model unstripped. This resolves 06-RESEARCH Open Question 4.
- **D-10:** `RunStarted` gains `root_image: Option<ImageDigest>`, the lock's digest from `Lock::check` at start; `RunId::of` is unchanged and the fold still recomputes the run ID. On every start and resume the loop requires the lock's current root image to equal the recorded one, else it refuses with both digests, so `env build --update` means a new run. Judge data is already pinned through `JudgeVersion`, and each `Judged` still records `root_image` in `BackendInfo`. This implements the lineage half of 05-CONTEXT D-10 that plan 05-16 deferred.
- **D-11:** Every run is pinned to the backend class of its first `Judged`. The loop refuses to record a `Judged` whose `BackendInfo` `(backend, arch)` differs from the run's first one, whatever the objective; the refusal is a configuration error that stops the driver, never an `Outcome`. No manifest change, strictly stronger than JUDGE-04, and free in v1 where krun on arm64 is the only backend; when Lambda arrives a run still never mixes backends.
- **D-12:** Core gains the `CumulativeDiff` newtype (03-REVIEW WR-03). It is produced only by `compose` and by `diff_trees` taken against the run's base tree; `Experiment` and `JobRequest::candidate` take it, so a diff relative to the parent tree, or a bare `Diff::parse` result, cannot become an `ExperimentId` by mistake. No wire change: the blob holds the same canonical text.

### Caps, stops, and prices (LOOP-07, LOOP-08)
- **D-13:** Prices are required before a run starts or resumes. The CLI parses `~/.config/minos/prices.toml` into the validated `Prices` value and the loop takes it as input. A missing or unparseable file, or a table lacking the compute rates or a rate for any model the manifest's proposers name, refuses the run with the path and the missing keys named, before any VM or model call. Never zero, never a built-in default; `minos init` (Phase 8) may write a template.
- **D-14:** Safe points are every generation, model call, and launch. Before a generation starts the loop re-folds the ledger and checks `spend_ok`, `circuit_breaker`, and any `StopRequested`; before each model call `spend_ok` and `StopRequested`; before each VM launch `StopRequested` (compute spend is checked once its `Spent` lands). A VM already running is awaited (it has a hard maximum duration) and its `Judged` and `Spent` are recorded, then the driver exits. `minos stop` (Phase 8) only appends the event; the driver never kills a VM on a stop.
- **D-15:** The spend cap is metered only. `spend_ok` prices recorded `Spent` events; the loop never estimates a job or a call before it runs, so the cap can be exceeded by at most one VM job or one model call, which the docs state. Reserving a job's worst case before launch was rejected for v1.
- **D-16:** An infrastructure failure is retried at the same step after a short fixed pause. A launch error, an `InfraError` from `wait`, a store `Io`, or a provider infrastructure failure increments the in-memory streak, and the loop retries the same step (the same `LaunchKey`, or the next model call index) after a pause that is one constant with its reason. Any success resets the streak to 0. When the streak reaches `max_infra_failures` the loop appends `StopRequested { InfraFailures }` and exits (03-CONTEXT D-14). Nothing is recorded for a failed attempt. `ProviderLimit` and `ProviderAuth` are stops, never retries (06-CONTEXT D-13, D-15).

### Carried forward (decided in earlier phases or the design of record; not re-opened)
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record
- `CLAUDE.md` — Threat model; Architecture (the orchestrator and runner roles, the proposer stays outside the sandbox); Workspace and dependency direction (`runner` and `loop` lanes); Code rules; `minos-core` (Policy, Ledger, the `Event` set and its keys, `RunId::of`, `JudgeVersion`); `minos-store` (`driver.lock` sits above `ledger.lock`); `minos-sandbox` (`SandboxSpec::new`, `secrets`, `seal`, `preflight`, `Finished::into_output`, the `Untrusted` exits); Projects (the `[judge]` table, `trials` joins it); Judge (gates pass only at exactly 1; separate sandboxes); Environments and source (the Source paragraph's "waits for Phase 7" note that D-04 closes; `minos.lock` and `Lock::check`); Local backends (the K7 classification rule, host `TimedOut` attribution, `MAX_JUDGE_DATA_BYTES`); Cost; Open questions (judge data over 96 MiB; `RLIMIT_AS` at the default allowance).
- `.claude/CLAUDE.md` — Decisions at a glance 10 (sync primitives); Question 8 (money: pico-dollar rates, micro-dollar totals, `ApproxCost`); Question 10 (sync table: `std::thread::scope`, `File::lock`); What NOT to Use.
- `deny.toml` — the `[bans]` wrapper entries for `minos-runner`, `minos-loop`, `minos-propose`, `minos-sandbox`, and `minos-source`, and the comment that the loop defines `JobRunner`.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — LOOP-01 to LOOP-09, JUDGE-01, JUDGE-03, JUDGE-04, CLI-04 (this phase); JUDGE-02, CLI-05, CLI-06, CLI-10, COST-02 (Phase 8 consumers of the driver); NEXT-01, NEXT-02, NEXT-04, NEXT-05 (deferred).
- `.planning/ROADMAP.md` §Phase 7 — the goal, the five success criteria, the fakes-first rule for the loop plans, and "skip research-phase"; §Phase 8 — what the CLI expects from the driver.
- `.planning/PROJECT.md` — Core Value; Constraints; Key Decisions (local loop in the CLI process; sweep and single_shot only; done means the full loop on a sample project).

### Prior phase decisions this phase must honor
- `.planning/phases/06-proposers/06-CONTEXT.md` — D-05 (parent tree from the incumbent's cumulative diff), D-07 and D-08 (history rows and failure excerpts the loop supplies), D-09 (steer notes in ordinal order), D-12 (empty call semantics), D-13 and D-15 (provider stops), the Claude's Discretion note on the `Proposer` interface, and the Deferred Ideas entry on failure excerpts after a restart (resolved by D-09 here).
- `.planning/phases/06-proposers/06-RESEARCH.md` — Open Question 3 (`Spent` per model), Open Question 4 (log blobs; resolved by D-09).
- `.planning/phases/05-guest-krun-backend-and-environment-build/05-CONTEXT.md` — D-05 (the memory budget the `Machine` comes from), D-09 to D-11 (root images and the lock), D-13 to D-15 (`BlobSource`, verified path, the backend writes the input disk), and the Deferred Ideas entry on D-10 lineage (resolved by D-10 here).
- `.planning/phases/05-guest-krun-backend-and-environment-build/05-RESEARCH.md` — Open Question 4 (lock digest in `RunStarted`), Pitfall 12 (host timeout attribution), Pattern 11 (`Lock::check`).
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md` — D-02 (the clean check), D-03 to D-09 (the packer, denylist, and scan D-03 reuses), D-10 to D-13 (store layout, locking, durability, torn tail), D-14 and D-15 (role-typed output, `LaunchKey`), D-17 (specs carry IDs), and the Deferred Ideas entry on `CumulativeDiff` (resolved by D-12 here).
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-REVIEW.md` — IN-03 (`Appended::Conflict`; resolved by D-08).
- `.planning/phases/03-core-types-and-pure-logic/03-CONTEXT.md` — D-04 (cumulative diffs, the baseline ID), D-05 (`StopReason` closed), D-06 (`RunId` takes the start time from the shell), D-07 (steer ordinals), D-09 and D-10 (`Spent` keys, the event set), D-11 to D-14 (trials, noise, routing, the breaker), D-18 (`BackendInfo` in every `Judged`).
- `.planning/phases/03-core-types-and-pure-logic/03-REVIEW.md` — WR-03 (`CumulativeDiff`).
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` — K7 table and "Classification rule for Phase 5" (what the runner maps onto `Outcome`).

### Code this phase builds on
- `crates/minos-core/src/policy.rs` — `check_scorecard`, `reduce_trials`, `select_greedy`, `conclude`, `noise_floor`, `noise_check`, `circuit_breaker`, `spend_ok`.
- `crates/minos-core/src/route.rs` — `route`, `route_table`, `SurfaceStats`.
- `crates/minos-core/src/state.rs` — `fold`, `RunState` and its accessors (`incumbent_before`, `trials`, `decision`, `last_concluded_generation`, `spends`, `stop_requested`, `notes`).
- `crates/minos-core/src/ledger.rs` — `Event` (D-09 and D-10 add optional fields), `Event::key`, `Origin`, `SpendSource`, `SpendId::of`, `StopReason`, `Decision`, `BackendInfo`.
- `crates/minos-core/src/job.rs` — `JobRequest::{baseline, candidate, verify_diff}`, `JobResult::new` (refuses tokens), `JobBudget`, `MAX_TRIALS`.
- `crates/minos-core/src/guest.rs` — `GuestJob::{candidate, judge}`, `GuestStatus`, `StageReport`, `StageEnd`, `OutputStatus`.
- `crates/minos-core/src/diff/compose.rs` and `diff/apply.rs` — `compose`, `diff_trees`, `Tree`, the applier the loop uses for the parent tree.
- `crates/minos-core/src/lock.rs` — `Lock`, `LockedImage`, `Lock::check` (D-02 adds the `[[judge]]` table).
- `crates/minos-core/src/manifest.rs` — `JudgeSpec`, `JudgeVersion::of`, `Budget`, `ProposerSpec`, `RawJudge` (D-05 adds `trials`).
- `crates/minos-core/src/money.rs` and `usage.rs` — `Prices`, `Cost`, `ApproxCost`, `Usage::{tokens, compute}`.
- `crates/minos-store/src/lib.rs` — `Ledger`, `Blobs`, `Appended` (D-08 removes `Conflict`), `StoreError`; `crates/minos-store/tests/contract.rs` — the contract test both stores pass.
- `crates/minos-sandbox/src/spec.rs`, `preflight.rs`, `finished.rs`, `untrusted.rs`, `backend.rs`, `blob_source.rs` — `SandboxSpec::new`, `InputDisk::{candidate, judge}`, `JudgeData`, `secrets`, `seal`, `preflight`, `Capabilities::backend_info`, `Finished::{into_output, console_excerpt}`, `Untrusted::{take_status, take, into_scorecard, into_log_excerpt}`, `Backend`, `RunningVm::wait`, `BlobSource`.
- `crates/minos-propose/src/proposer.rs`, `prompt.rs`, `proposal.rs`, `fake.rs` — `Proposer::propose`, `ProposeInput`, `Proposals`, `History`, `HistoryEntry`, `SteerNote`, `Proposal::into_parts`, `FakeProvider`.
- `crates/minos-source/` — the packer, denylist, scan, and clean check D-03 and D-04 reuse (CLI-only lane).
- `crates/minos-cli/src/blob_source.rs`, `locate.rs`, `env_build.rs`, `main.rs` — `FileBlobSource`, `host_paths`, the lock write path `judge push` follows, and the clap builder skeleton `judge push` joins.
- `crates/minos-backend-krun/src/backend.rs` — `KrunBackend::new`, `KrunRunning::wait` (the elapsed time the runner's host measurement starts from).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/minos-runner/` and `crates/minos-loop/` are stubs with the right dependencies already declared (`minos-core`, `minos-store`, `minos-sandbox` for the runner; `minos-core`, `minos-store`, `minos-propose` for the loop) and `deny.toml` wrapper entries in place, so no layering change is needed.
- Every pure decision the loop makes already exists in core: `route`, `conclude`, `circuit_breaker`, `spend_ok`, `noise_check`, `fold`. The loop is glue that reads the fold, calls these, and appends.
- `JobRequest`, `JobResult`, `GuestJob::{candidate, judge}`, `SandboxSpec::new` with `InputDisk::{candidate, judge}`, `preflight`, `Backend::launch`, `RunningVm::wait`, `Finished::into_output`, and the role-typed `Untrusted` exits are the runner's whole vocabulary; `JobResult::new` already refuses tokens in a job's usage (LOOP-03 at the type level).
- `Proposer<P>::propose` with `ProposeInput` and `Proposals`, and `FakeProvider` with scripted replies, give the loop its propose step and its test double.
- `MemoryStore` passes the same contract test as `FileStore`, so every loop test runs against it (LOOP-09).
- `minos-source`'s packer, denylist, and scan are reused as-is by `judge push` (D-03); `env_build.rs` shows the lock read-compare-write path and the `--update` gate that `judge push` mirrors (D-01).
- `Lock` is a versioned validated TOML type with a golden file; adding the `[[judge]]` table follows the same raw-struct pattern.

### Established Patterns
- Raw struct with `deny_unknown_fields` and `try_from` into a private-field type; optional fields added to a frozen event keep `"v":"1"` (CLAUDE.md ledger rule; D-09, D-10).
- Errors separate blame: `Outcome` for candidate faults, retried infrastructure errors, and configuration refusals that stop the driver (D-08, D-10, D-11, D-13).
- Record before act, keyed by content IDs, with the fold as the only source of state (D-07).
- Hostile bytes cross a boundary only through a validating exit with a cap; blobs may hold hostile bytes, readers may not show them unstripped (D-09).
- Constants with reasons beside the code that enforces them; no `allow` attributes; the three clippy lints are `forbid` in tests too.
- Ignored VM tests read `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`; production code never does (05-CONTEXT D-04).

### Integration Points
- `minos-cli` composes `Runner<KrunBackend, FileStore>` through `FileBlobSource`, implements the loop's `JobRunner` over it, parses `prices.toml` (D-13), reads `minos.lock` and the manifest, and hands the driver a validated `Prices`, `Manifest`, `Lock`, and `Proposer<ClaudeCli>` (Phase 8 wires `minos run`; this phase proves the composition compiles and runs in one ignored VM test).
- `judge push` joins the clap builder skeleton in `main.rs` next to `env build` and `host doctor`, reuses `minos-source` and the lock write path, and stores the tarball through `Blobs::put_reader` under the 96 MiB cap.
- Phase 8's `minos check` calls the loop's generation-0 step on a diagnostic run (D-06); `minos stop` appends `StopRequested` and the driver honors it at the D-14 safe points; `minos status` reports `StoreError::Conflict` (D-08) and the image or backend pin refusals (D-10, D-11).
- The prompt's `History` rows get their failure excerpts from the `Judged { logs }` blobs through `LogExcerpt` (D-09), so a resumed run shows the same history as an uninterrupted one.
- The source pack gains the judge file digest set as an input (D-04); `minos run` passes it from the lock.

</code_context>

<specifics>
## Specific Ideas

- Every choice was the fail-closed one again: a missing price refuses the run rather than pricing at zero (D-13), a conflicting append is an error the type system will not let a caller drop (D-08), a rebuilt image or a different backend refuses to resume rather than silently continuing (D-10, D-11), and judge data gets the full source hygiene with no override (D-03).
- One source of truth per fact: the manifest names the judge label (D-01) and the trial count (D-05); the lock records what was pushed and built (D-02); the ledger records what ran (D-09, D-10). Nothing is derived from the clock or the session except `started_at`, which the shell passes in.
- Resume is a fold, not a journal: the loop never remembers where it was, it re-derives the stage from the ledger every time (D-07), which is the same discipline the AWS orchestrator will need under replay.
- The spend cap may be overshot by one step and the docs say so (D-15); "approx. cost" stays measured, never estimated.

</specifics>

<deferred>
## Deferred Ideas

- **The `minos run`, `status`, `log`, `show`, `stop`, `check`, `report`, and `adopt` commands and the per-experiment progress line** — Phase 8 (CLI-05 to CLI-10, JUDGE-02, COST-02). This phase ships the driver as a library, the generation-0 step `check` will call (D-06), and the observer seam the progress line will use.
- **Concurrent candidates (`width`) and the K8 launch-rate and memory measurements** — NEXT-04; the launch key, per-append locking, and the sequential driver are designed so concurrency needs no store or ledger change.
- **Reserving a job's worst-case compute before launch** — rejected for v1 (D-15); reconsider if one-step overshoot proves material on a GPU backend.
- **An objective flag (`kind = "speed"`) or a `[environment] requires.backend` field** — rejected for v1 in favor of pinning every run (D-11); return to it only when a multi-backend milestone wants hardware-independent runs to roam.
- **A separate judge lock file** — rejected (D-02); the one `minos.lock` holds both lineages.
- **Judge data over 96 MiB** — unchanged open question (CLAUDE.md): needs a streaming unpack in PID 1; `push` refuses it meanwhile (D-03).
- **`minos rejudge <run> --judge <v>`** (NEXT-02) and **`minos steer`** (NEXT-01) — the `Judged` key and the `Steered` event already support them; the loop's history view reads notes in ordinal order.
- **Champion re-measurement before `Selected`** — NEXT-05.
- **A `StopReason` for an unscored baseline** — not added; the driver reports it as an error (Claude's Discretion). Revisit if `minos status` needs a recorded reason.

</deferred>

---

*Phase: 07-runner-and-loop*
*Context gathered: 2026-10-22*
