# Phase 3: Core Types and Pure Logic - Context

**Gathered:** 2026-10-10
**Status:** Ready for planning
**Amended:** 2026-10-14 (D-04, D-09, D-10; user decisions via orchestrator after the plan check)

<domain>
## Phase Boundary

Phase 3 fills `minos-core`, the crate every other crate depends on and that depends on nothing internal. It delivers every validated, pure, deterministic value the loop decides on: the SHA-256 ID newtypes and their hashing, `RelPath`, `Manifest::try_from`, the canonical `Diff` and the strict exact-position applier, `Metric` and the scorecard, `Outcome`, the frozen `Event` set with a documented `EventKey` per event and the ledger fold to run state, `select_greedy`, routing and tier eligibility, the seeded sweep sampler, the `JobRequest`/`JobResult` loop-to-runner contract, pico-dollar money, the allowlist renderer, `build_report`, and the circuit breaker. With them come the property tests, the golden hash vectors, and the golden wire-format files that freeze what persists.

It delivers no I/O, no clock, no store, no sandbox type, no VM code, no CLI command, and nothing from the deferred tiers or policies (`planned`, `agentic`, pareto, `review`, `DesignJudge`). Core's `clippy.toml` already bans the clock, the environment, the filesystem, process spawning, the network, printing, and `HashMap`/`HashSet`; the phase writes within those bans rather than around them.

Requirements: CORE-01 through CORE-17 and COST-01. Cross-cutting: GATE-06 (`cargo clean` after verification passes) and `rules/` adherence. Ordering constraint from the roadmap: CORE-08 (freeze the `Event` set with a documented `EventKey` per event) lands before any golden wire-format file is committed. The roadmap marks this phase "standard patterns; skip research-phase".

</domain>

<decisions>
## Implementation Decisions

### Diff and surface semantics
- **D-01:** A `writable` entry names exactly one file. No directory prefixes, no globs. Matching is bytewise on the canonical `RelPath`, so a homoglyph path never matches a listed one. The disjointness check in `Manifest::try_from` (`writable` against `readable` and judge paths) is therefore exact-path set disjointness.
- **D-02:** A diff may only modify a writable file. Every path a diff touches must be in `writable` and must exist in the base tree. A `--- /dev/null` or `+++ /dev/null` side (create or delete) is rejected at parse, like a rename, mode change, symlink, or binary patch. The applier has no create or delete branch, `adopt`'s patch is always file-to-file, and no experiment can leave a hole where the judge expects a file.
- **D-03:** A diff that changes nothing (no hunks, or hunks whose removed and added lines are identical after canonicalization) is invalid. `Diff::parse` rejects it, so a no-op proposal is dropped with the other invalid proposals and never becomes an `Experiment` or costs a VM.
- **D-04:** The baseline is identified by `ExperimentId::baseline(&TreeId)`: the same domain prefix (`minos/experiment/v1\0`), the base `TreeId`, and a zero-length diff (length prefix 0, no bytes). It is a dedicated constructor, so a parsed `Diff` can never be empty and `Experiment` cannot hold one (an enum, or `Option<Diff>` behind a private field, is the planner's call). `RunStarted { baseline }`, `Proposed { parent }` for first-generation experiments, and the noise-floor `Judged { exp: baseline, trial }` events all use this ID. It has its own golden vector.
  - **Amendment (2026-10-14, user decision via orchestrator):** an experiment's diff is cumulative from the run's base tree, never relative to its parent (03-RESEARCH Open Question 1). `parent` is lineage only. A child's diff is composed by applying the parent's cumulative diff and then the proposal to the base, and re-deriving one canonical diff against the base, so the guest always applies exactly one diff to the base tarball and two proposals that reach the same tree get the same `ExperimentId`. `Proposed.diff` and `JobRequest.diff` both name that cumulative diff. Implemented by 03-02 (`ExperimentId::of` over base plus canonical diff; CLAUDE.md Diff paragraph), 03-06 (`compose`, `diff_trees`), 03-07 (`Proposed.diff`), and 03-09 (`JobRequest.diff`).

### Event set freeze (CORE-08)
- **D-05:** `StopRequested { reason: StopReason }` where `StopReason` is a closed `deny_unknown_fields` enum: `UserStop`, `SpendCap`, `TokenCap`, `NoValidCandidates`, `InfraFailures`, `ProviderLimit`. The circuit breaker (CORE-17, LOOP-08), the spend and token caps (LOOP-07), `minos stop` (CLI-10), and a provider rate limit (PROP-09) each map to one variant. No free text from the shell enters the ledger through this event; display strings are fixed per variant.
- **D-06:** `RunId::of(&ManifestId, &TreeId, started_at)` is a domain-separated, length-prefixed hash of the manifest ID, the baseline tree, and a shell-supplied start time (Unix seconds). `RunStarted` records `started_at` so the fold can recompute and verify the ID. Core never reads a clock: the CLI passes the reading in. Two runs of the same project a second apart are distinct runs.
- **D-07:** `Steered { generation: u32, ordinal: u32, note: BlobId }` with `EventKey = H(run, generation, ordinal)`. The note text is a blob, never inline, so the ledger line carries no hostile text. The CLI folds the ledger, counts the notes already recorded for the upcoming generation, and appends with the next ordinal; identical notes stay distinct, and the prompt assembler (Phase 6) reads a generation's notes in ordinal order.
- **D-08:** `ReviewRequested { request: ReviewId, kind: ReviewKind }` and `Reviewed { request: ReviewId, answer: ReviewAnswer }` take minimal closed enums now: `ReviewKind::{JudgeDesign, Params, Dependencies}` and `ReviewAnswer::{Approved, Declined}`, keyed `H(run, request)`. Nothing in v1 acts on them; the fold records them and the golden file covers one of each. POL-01 (the inbox, `answer`, `DesignJudge`) adds variants and payloads later without a `V2` record, because adding a serde enum variant leaves every existing line parseable.
- **D-09:** `Spent` is keyed by `SpendId = H(run, generation, source)` where `source: SpendSource` is a closed enum with an index: `ModelCall { call: u32 }`, `Candidate { exp: ExperimentId, trial: u32 }`, `Judge { exp: ExperimentId, trial: u32 }`. One model call or one VM run is one `Spent` record, so a resumed generation that re-runs a model call gets a new index and nothing is double-counted or silently dropped (Pitfall 19). `exp` is derivable from the source; whether the field stays on the event for readability is the planner's call, decided before the golden file.
  - **Amendment (2026-10-14, user decision via orchestrator):** `SpendSource::ModelCall` carries the model, because prices are per model (COST-01): `SpendSource::{ModelCall { call: u32, model: ModelId }, Candidate { exp: ExperimentId, trial: u32 }, Judge { exp: ExperimentId, trial: u32 }}`. `Spent` drops `exp`: the event is `Spent { generation: u32, source: SpendSource, usage: Usage }`, and `SpendSource::exp()` derives the experiment when there is one. The key is unchanged in shape, `SpendId = H(run, generation, source)`, and the model call's source hashes `u64(1), u64(call), bytes(model)`. Implemented by 03-07.
- **D-10:** The rest of the set is as REQUIREMENTS.md CORE-08 and `.planning/research/ARCHITECTURE.md` Amendment A9 state: `RunStarted { run, manifest, baseline, started_at }`, `ManifestChanged { manifest }`, `Proposed { exp, parent, tier, rationale: BlobId, model: ModelId, prompt: PromptHash }`, `Judged { exp, judge, trial, outcome, backend }`, `Selected { generation, decision }`, `ReportAcknowledged { exp, report }`. Keys: `RunStarted` to the run, `ManifestChanged` to the manifest, `Proposed` to the experiment, `Judged` to (exp, judge, trial), `Selected` to the generation, `ReportAcknowledged` to (exp, report). Provider info lives on `Proposed` (model ID, prompt hash), not in a separate `ProviderUsed` event. Every key is documented in the `ledger` module docs in the same change that freezes the set.
  - **Amendment (2026-10-14, user decision via orchestrator):** the following changes to the field lists and keys above are approved before the freeze (03-RESEARCH "Decide before the freeze" items 1 to 7 and Open Questions 3 and 7). The frozen set is:
    - `RunStarted { run, manifest, base: TreeId, baseline, started_at }`: `base` is added, because `baseline` is a hash and cannot be inverted, and the fold must recompute `RunId::of(manifest, base, started_at)` (D-06) and `ExperimentId::baseline(&base)` (D-04). Key unchanged (one per run).
    - `Proposed { exp, parent, generation: u32, tier, diff: BlobId, rationale: BlobId, origin: Origin }` with `Origin::{Sweep, Model { model: ModelId, prompt: PromptHash }}`. `generation` and `diff` are added (selection, routing, the breaker, resume, `JobRequest`, and the report cannot derive them), and `Origin` replaces the flat `model` and `prompt` fields, because a sweep proposal has neither. Key unchanged (the experiment).
    - `ManifestChanged { manifest, from_generation: u32 }`, keyed by (manifest, `from_generation`) instead of the manifest alone, so a change A to B to A is three distinct records instead of collapsing onto one key.
    - `StopRequested { generation: u32, reason: StopReason }`, keyed by (`generation`, reason), so a repeated `minos stop` in one generation is a no-op and a run resumed after a raised cap can stop again later. D-05 gave it no key.
    - `Selected { generation, decision: Decision }` with `Decision { kind: DecisionKind, proposed: u32, dropped: u32 }` and `DecisionKind::{Improved { winner: ExperimentId }, Kept { incumbent: ExperimentId }, NoValidCandidates}`, so the breaker and `minos status` read the kind and the invalid-proposal count from the ledger (PITFALLS Pitfall 34).
    - `Judged { exp, judge: JudgeVersion, trial, outcome, backend }` where `JudgeVersion = H("minos/judge/v1", judge-data TreeId, canonical judge spec)` and the canonical spec is the command argv, the gates, and the objectives with goal and held-out pairing. The manifest's `[judge] version` integer is a local label naming which pushed judge data the project uses; it and `min_improvement` are excluded from the hash (03-RESEARCH Open Question 3). Key unchanged (exp, judge, trial).
    - `Outcome::Crashed { stage: Stage }` and `Outcome::TimedOut { stage: Stage }` with `Stage::{Build, Run, Judge}` (03-RESEARCH Open Question 7).
    - Not a change: `ReviewRequested` and `Reviewed` keep D-08's shape and key fields. Every key starts with a per-event tag (the `EventKey` encoding is Claude's discretion), so a request and its answer hash differently.
  The full key table, with each event's tag and hashed fields, is in 03-07-PLAN.md and goes verbatim into the `ledger` module docs. Implemented by 03-03 (`Outcome`, `Stage`), 03-05 (`JudgeVersion`), and 03-07 (the freeze).

### Selection under noise
- **D-11:** Several `Judged { exp, judge, trial }` events for one experiment under one judge version reduce to one score per metric by the median, using the lower median for an even count so two `Metric`s are never averaged. A single trial is its own median. `select_greedy` works on the reduced scores, so the permutation-invariance property covers trial order as well as experiment order.
- **D-12:** The noise-floor check is a pure core function, `policy::noise_check(floor, objective) -> NoiseVerdict::{Ok, Warn, Refuse}`, with the multiple of the spread fixed in `limits.rs`. The floor is itself folded from the baseline's `Judged` trials, so it is a pure function of the ledger. `minos check` prints the verdict; `minos run` refuses to start a run whose `min_improvement` is inside the floor unless an explicit override flag is passed (Phase 8 names it; `--allow-noisy` is the working name). Selection never silently raises the manifest's threshold.
- **D-13:** Routing resets after an improvement. The tier for a generation is a pure function of manifest and ledger: the lowest eligible tier plus one step per `stall_step` generations without an improving `Selected` since the last one, clamped to the top of the eligible ladder. An improvement resets the stall count and the tier, so cheap tiers get another chance from the new incumbent.
- **D-14:** `stall_step` and the circuit breaker's limits are manifest fields with defaults, each range-checked against `limits.rs` in `Manifest::try_from`: `[policy] stall_step` and `[budget] max_empty_generations` and `max_infra_failures` (working names). The bounds stop a value that would disable the breaker. The circuit breaker (CORE-17) returns a stop decision after `max_empty_generations` consecutive generations with no valid candidate or `max_infra_failures` consecutive infrastructure failures, mapping to `StopReason::NoValidCandidates` and `StopReason::InfraFailures`.

### Report flags and provenance (CORE-16)
- **D-15:** The visible inputs for the literals-matching flag are the `[surface]` files (`readable` and `writable`) from the base tree, which `build_report` already receives as blobs. A literal added by the diff that also appears in a surface file outside the hunk's own context is flagged. No new manifest field, nothing judge-side, and the set is identical for every reader of the report. Judge data is never an input to `build_report`.
- **D-16:** An improvement is anomalous when the winner's delta over its parent exceeds `k` times the baseline's measured spread, with `k` in `limits.rs`. When no noise floor was recorded, the fallback is exceeding `k` times the largest earlier accepted delta in the lineage. Both inputs come from the ledger, so the flag is pure and stable.
- **D-17:** The summary section is one row per objective and gate: baseline median, baseline spread (min to max across trials), winner median, delta as absolute and percent, trial counts on each side, and whether the delta clears `min_improvement` and the noise multiple. Held-out metrics get their own rows marked as held out, and the held-out divergence flag reads from the same rows.
- **D-18:** The weak-isolation field is rendered in v1 even though it is always "hardware". Each `Judged` carries `BackendInfo` with the isolation level and the active confinement layers (KRUN-07). The report walks the lineage, prints the weakest level seen and any reduced-hardening layer (for example the `_minos` user not active), and the path is golden-tested, so a later dev mode cannot ship without it and adding it later cannot change existing report hashes.

### Carried forward (decided in earlier phases or the design of record; not re-opened)
- Raw struct with `deny_unknown_fields`, `try_from` into a private-field type, and the same constructor for hand-built and deserialized values (CLAUDE.md Code rules; `crates/minos-guest/src/job.rs` and `crates/minos-vmm/src/config.rs` are the existing models).
- No `unwrap`, `expect`, or slice indexing anywhere, including tests and property tests; tests return `Result` and use `?` (Phase 1 D-12, memory rule).
- `diffy` 0.5.2 in `gitdiff` mode for parsing, so renames, copies, modes, and binary patches are detectable; our own exact-position applier; never `diffy::apply` (`.claude/CLAUDE.md` Question 2).
- `BlobId` and `TreeId` are the plain SHA-256 of content; composite IDs (`ExperimentId`, `RunId`, `EventKey`, `SpendId`, `LaunchKey`, `ReportHash`) use a domain-separation prefix and length-prefixed fields (ARCHITECTURE.md A10, Anti-Pattern 6). Golden hash vectors are plain `assert_eq!` against hex literals, never insta.
- `Metric` is a finite `f64`, `-0.0` normalized to `0.0`, `Ord` via `total_cmp`; a scorecard metric can be `held_out`.
- Money: rates parsed from strings into pico-dollar `u128` (fail closed on more than 12 fractional digits, exponents, negatives, overflow), totals in integer micro-dollars with round-half-up at display, one type owns the `approx. cost: $X.XX` string (`.claude/CLAUDE.md` Question 8; COST-01, COST-02). The ledger stores usage only.
- Sweep: `rand_chacha` 0.10 seeded from `sha256("minos/sweep/v1" ‖ run ‖ generation)`, our own unbiased range mapping, never `rand` distributions or `gen_range`; golden-tested samples.
- Renderer: a hand-rolled allowlist in core that keeps printable characters plus `\n` and `\t`, drops every C0 and C1 control including ESC and DEL, and renders bidi controls (U+202A to U+202E, U+2066 to U+2069) and zero-width characters visibly rather than silently (PITFALLS Pitfall 14; `.claude/CLAUDE.md` What NOT to Use).
- Golden wire files use `insta` with `INSTA_UPDATE=no` in the gate; `proptest-regressions/` files are committed; `BTreeMap`/`BTreeSet` only.
- Every plan ends with the five gate commands (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`) plus the C-01 and C-02 checks from Phase 1 D-15; the phase ends with `cargo clean`.

### Claude's Discretion
- The diff size cap, per-file hunk cap, CRLF handling, and `\ No newline at end of file` handling, as long as each is decided explicitly, tested, and the canonical form round-trips (`parse(serialize(d)) == d`).
- The canonical diff header format, bounded by CLI-09: the bytes `adopt` writes must be accepted by both `git apply` and `patch -p1`, which the Phase 8 golden test proves. Plain `--- a/path` / `+++ b/path` with no timestamps and no extended headers is the expected shape.
- How the applier reports a context mismatch (which file, hunk, and line), as long as it is a candidate fault (`Rejected`), never an error.
- The JSONL line shape (`{"seq":N,"key":"<hex>","record":{"v":"1",...}}` per ARCHITECTURE.md), the `EventKey` encoding, `Decision`'s fields on `Selected`, and `BackendInfo`'s field list beyond isolation level and confinement layers.
- The `JobRequest` and `JobResult` field lists (ARCHITECTURE.md Data Flow sketches `JobRequest { exp, base, diff_blob, judge, trial }` and `JobResult { outcome, usage, backend_info, blob ids }`), golden-filed as wire formats.
- The `prices.toml` shape (which usage dimensions and per-model token rates in v1), as long as COST-01 and the pico-dollar rule hold.
- The renderer's visible form for bidi and zero-width characters (`<U+202E>` is the suggested form) and its treatment of other non-printable code points.
- The numeric defaults and bounds in `limits.rs`: `stall_step`, `max_empty_generations`, `max_infra_failures`, the noise multiple, the anomaly multiple `k`, the encoded-blob literal length, diff and prompt caps.
- The "new dependencies" and "encoded blobs" flag heuristics, the report's section order, and the canonical encoding the report hash covers (never `serde_json::Value`; PITFALLS Pitfall 17).
- Whether `generation::{plan, admit, conclude}` and `spend_ok` (ARCHITECTURE.md Pattern 1) ship in this phase or with the loop in Phase 7. They are pure, so core is their home either way; the planner decides what Phase 7 needs in place to be glue.
- Module layout within `minos-core`. CLAUDE.md lists `id.rs path.rs manifest.rs experiment.rs outcome.rs policy.rs route.rs ledger.rs require.rs usage.rs report.rs tool.rs limits.rs`; ARCHITECTURE.md adds `generation.rs`, the sweep, money, and `SafeText`. `tool.rs` (the agentic `ToolCall` enum) belongs to the deferred agentic tier (TIER-01) and is not a Phase 3 deliverable.
- Plan count and ordering, subject to the roadmap's constraint that CORE-08 lands before any golden wire file.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record and stack of record
- `CLAUDE.md` — Code rules (deserialization is validation, private fields, errors separate blame, stable order, commands as argv); the `minos-core` section (module list, `ExperimentId::of` hashing sketch, `RelPath`, `Manifest`, `Diff`, `Metric`, `Outcome`, `Policy`, `Ledger` and the `Event` sketch, wire formats are an API); Projects (the `minos.toml` example, the proposer ladder, `STALL_STEP`, the judge rules); Cost (usage never dollars, pico-dollar rates, micro-dollar totals, `approx. cost`); Report and adopt (`build_report` pure and hashed, the flag list, the renderer); Testing; Commands.
- `.claude/CLAUDE.md` — Decisions at a glance 2 (diffy plus own applier), 3 (tar, sha2, base16ct), 4 (serde patterns), 8 (pico-dollar money); Question 2 (diff crates compared); Question 4 (`try_from` plus `deny_unknown_fields`, no `flatten`, internally tagged enums); Question 6 (testing tools); Question 8 (exact money arithmetic, the nano-to-pico correction); Supporting Libraries (`rand_chacha` seeding and the ban on `rand` distributions); What NOT to Use (`diffy::apply`, fuzzy patching, `gen_range`, `strip-ansi-escapes`, the allowlist renderer rule); Version Compatibility (sha2 0.11 and hybrid-array, rand_core 0.10).
- `rules/` — every rule applies. In particular `rules/api-parse-dont-validate.md`, `rules/type-newtype-validated.md`, `rules/type-newtype-ids.md`, `rules/api-newtype-safety.md`, `rules/type-enum-states.md`, `rules/err-thiserror-lib.md`, `rules/err-custom-type.md`, `rules/api-must-use.md`, `rules/api-common-traits.md`, `rules/api-non-exhaustive.md`, `rules/test-proptest-properties.md`, `rules/doc-all-public.md`, `rules/doc-errors-section.md`.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — CORE-01 through CORE-17 and COST-01 (this phase); COST-02, LOOP-04, LOOP-05, LOOP-07, LOOP-08, JUDGE-02, JUDGE-03, CLI-09, CLI-10, PROP-08, PROP-09, STORE-03, SBX-06 (what later phases build on these types, so the shapes should serve them); POL-01 and TIER-01 (deferred; D-08 and the `tool.rs` note).
- `.planning/ROADMAP.md` — Phase 3 goal, the five success criteria, the ordering constraint (CORE-08 before golden files), and "skip research-phase"; Cross-Cutting Rules; Dependencies and Parallelism (Phase 4 and Phase 6 depend on this phase).
- `.planning/PROJECT.md` — Core Value; Key Decisions (local-first, `ModelProvider`, sweep and `single_shot` only, chunked vsock output, which bounds `JobResult` sizes).

### Prior phases
- `.planning/phases/01-workspace-and-gates/01-CONTEXT.md` — D-01 (policy as Cargo-native configuration), D-12 (lint set, forbid lints, no clap derive), D-13 (exact pins), D-15 (C-01 and C-02 checks every plan runs).
- `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` — R-01 to R-03 (what `#[expect]` can and cannot do), V-09 and V-10 (the core `clippy.toml` bans proven to fire).
- `.planning/phases/02-platform-spikes/02-CONTEXT.md` — D-04 (the launch config as the model of a versioned, validated wire type), D-18 (the boot path, which fixes what a job carries).
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` — K7 table and the classification rule (what the runner will map into `Outcome::Crashed` and `TimedOut` versus infrastructure failure; the `Outcome` variants must serve it); S03-ANSWER (4 KiB chunked output, which caps what `into_scorecard` and `JobResult` can carry); SPIKE-07 contract (the `usage` field names `Spent` must carry: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`).

### Research
- `.planning/research/ARCHITECTURE.md` — Pattern 1 (`generation::{plan, admit, conclude, spend_ok}`), Pattern 2 (record before act; `LaunchKey`, `EventKey`, `SpendId`), Data Flow (one generation; order-insensitivity of fold, conclude, and report; the trial index), State management, Key data flow 4 (report and adopt recompute the hash), Invariant Check table, Anti-Pattern 6 (`BlobId` is not domain-separated), Amendments A9 (events) and A10 (IDs).
- `.planning/research/PITFALLS.md` — Pitfall 10 (judge integrity and candidate-controlled numbers; `Metric`), 11 (adaptive overfitting; held-out metrics), 14 (hostile strings; the renderer and `RelPath`), 15 (diff validation a different parser can bypass; canonical form, create/delete/empty decisions), 16 (noisy metrics; noise floor, repeat measurements as separate `Judged`), 17 (non-deterministic ordering; canonical encodings, `-0.0`, seeds), 19 (spend caps), 24 (serde types that skip validation), 34 (invalid-diff rate).
- `.planning/research/FEATURES.md` — Report flags: v1 subset vs later (the five v1 flags and which inputs each needs).
- `.planning/research/STACK.md` — Question 2 (diffy), Question 3 (hashing and hex), Question 4 (serde), Question 6 (proptest, insta, trybuild), Question 8 (money), Supporting Libraries (`rand_chacha`, `base16ct`).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/minos-core/` exists as a stub: `Cargo.toml` already declares `serde`, `serde_json`, `toml`, `thiserror`, `sha2`, `base16ct`, `diffy`, `rand_chacha`, `rand_core`, with `proptest` and `insta` as dev-dependencies; `clippy.toml` bans the clock, environment, filesystem, process, network, printing, and `HashMap`/`HashSet`; `src/lib.rs` holds one doc comment. Every dependency this phase needs is pinned in `[workspace.dependencies]`.
- `crates/minos-guest/src/job.rs` and `crates/minos-vmm/src/config.rs` — the established Raw-struct, `deny_unknown_fields`, `try_from`, private-field, `InvalidField`-style error pattern, with a version check on `v`, range constants at the top of the module, and `const fn` accessors. Core's manifest, ledger record, job types, and prices follow the same shape.
- `crates/minos-guest/src/pattern.rs` — the test style in use: `type TestResult = Result<(), Box<dyn std::error::Error>>`, `?` instead of `unwrap`, `checked_add(...).ok_or(...)?` for arithmetic.
- `crates/minos-backend-krun/src/rootfs.rs` and `input_disk.rs` — hostile-tar handling with size caps checked before reading; the same discipline applies to every cap in core.

### Established Patterns
- Validated types with private fields and no setters; deserialization goes through the same constructor as hand-built values.
- Typed `thiserror` enums per module that separate candidate faults (become an `Outcome` or a parse rejection) from infrastructure failures; `anyhow` only in `minos-cli`.
- No `unwrap`, `expect`, or slice indexing anywhere, tests included; `#[expect]` cannot silence the three forbid lints; no `#[allow]` at all (C-02 grep).
- Every persisted or hashed structure uses a canonical encoding under our control, never `serde_json::Value`; `BTreeMap`/`BTreeSet` wherever order affects output.
- Constants for caps live at the top of the module that enforces them and are documented with their reason.

### Integration Points
- Phase 4 (`minos-store`, `minos-sandbox`, the source tarball) consumes `Record`, `EventKey`, `BlobId`, `TreeId`, `RelPath`, `Limits`, and the `Untrusted` exit targets (`Scorecard`, `LogExcerpt`, diff texts).
- Phase 6 (`minos-propose`) consumes `Manifest`, the surface file list, `Diff::parse`, `Proposed`'s fields (model ID, prompt hash, rationale blob), `Spent` with `SpendSource::ModelCall`, `Steered` notes in ordinal order, and the sweep sampler.
- Phase 7 (`minos-runner`, `minos-loop`) consumes `JobRequest`/`JobResult`, the fold, `select_greedy`, routing, the circuit breaker, `noise_check`, `StopReason`, and `SpendId`.
- Phase 8 (`minos-cli`) consumes `build_report`, `ReportHash`, the renderer, `RunId::of` with the shell's clock reading, `ExperimentId::baseline` for `minos check`, and the `approx. cost` type.
- The guest (Phase 5) links `minos-core` for the applier, `RelPath`, and `Limits`; the host gate builds `minos-guest` for the host target, so core must stay free of Linux-only code.

</code_context>

<specifics>
## Specific Ideas

- Every decision above was chosen as the option that keeps a wrong value unrepresentable or a decision explainable in the report: exact-file surfaces, modify-only diffs, closed enums for every reason and source, and a manifest threshold that selection never silently changes.
- The frozen event set must be documented, with its `EventKey` per event, in the same change that introduces it, before the first golden wire file exists. The planner sequences this explicitly.
- Evidence standard from Phases 1 and 2 carries over: verification captures actual command output, including the failing output of a deliberate violation where a test proves a rejection.

</specifics>

<deferred>
## Deferred Ideas

- **`tool.rs` and the `ToolCall` enum** — the agentic tier (TIER-01, later milestone). Not a Phase 3 deliverable even though CLAUDE.md lists the file.
- **Rich `ReviewRequested`/`Reviewed` payloads, the inbox, `answer`, and `DesignJudge`** — POL-01, later milestone; D-08 leaves room by enum extension.
- **Pareto and `review` policies** — POL-01; `select_pareto` is not in CORE-10.
- **Runtime-behavior report flags** (spawned processes, sockets, environment reads, operations the guest denied) — RPT-01, v1.x or later; the five v1 flags are D-15 through D-18.
- **The `--allow-noisy` override flag's name and the default baseline repeat count** — Phase 8, where the noise floor is measured (STATE.md blocker for Phase 8).
- **`git apply` and `patch -p1` golden test on the canonical patch bytes** — Phase 8, CLI-09; this phase fixes the canonical form it will test.
- **Power source and thermal state in `BackendInfo`** (PITFALLS Pitfall 16) — Phase 5 or later; the `BackendInfo` type should not preclude it.

</deferred>

---

*Phase: 03-core-types-and-pure-logic*
*Context gathered: 2026-10-10*
