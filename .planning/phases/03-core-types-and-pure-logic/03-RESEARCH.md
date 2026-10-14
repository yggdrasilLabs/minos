# Phase 3: Core Types and Pure Logic - Research

**Researched:** 2026-10-10
**Domain:** Pure Rust domain modelling: validated newtypes, a canonical diff format and strict applier, an event-sourced ledger wire format, deterministic policy, exact money, and a hashed review report, all inside `minos-core` with no I/O
**Confidence:** HIGH for crate APIs and behaviours (read in the pinned sources and confirmed by offline compile probes); MEDIUM for the design recommendations that fill gaps the locked decisions leave open (flagged in Open Questions)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Diff and surface semantics
- **D-01:** A `writable` entry names exactly one file. No directory prefixes, no globs. Matching is bytewise on the canonical `RelPath`, so a homoglyph path never matches a listed one. The disjointness check in `Manifest::try_from` (`writable` against `readable` and judge paths) is therefore exact-path set disjointness.
- **D-02:** A diff may only modify a writable file. Every path a diff touches must be in `writable` and must exist in the base tree. A `--- /dev/null` or `+++ /dev/null` side (create or delete) is rejected at parse, like a rename, mode change, symlink, or binary patch. The applier has no create or delete branch, `adopt`'s patch is always file-to-file, and no experiment can leave a hole where the judge expects a file.
- **D-03:** A diff that changes nothing (no hunks, or hunks whose removed and added lines are identical after canonicalization) is invalid. `Diff::parse` rejects it, so a no-op proposal is dropped with the other invalid proposals and never becomes an `Experiment` or costs a VM.
- **D-04:** The baseline is identified by `ExperimentId::baseline(&TreeId)`: the same domain prefix (`minos/experiment/v1\0`), the base `TreeId`, and a zero-length diff (length prefix 0, no bytes). It is a dedicated constructor, so a parsed `Diff` can never be empty and `Experiment` cannot hold one (an enum, or `Option<Diff>` behind a private field, is the planner's call). `RunStarted { baseline }`, `Proposed { parent }` for first-generation experiments, and the noise-floor `Judged { exp: baseline, trial }` events all use this ID. It has its own golden vector.

### Event set freeze (CORE-08)
- **D-05:** `StopRequested { reason: StopReason }` where `StopReason` is a closed `deny_unknown_fields` enum: `UserStop`, `SpendCap`, `TokenCap`, `NoValidCandidates`, `InfraFailures`, `ProviderLimit`. The circuit breaker (CORE-17, LOOP-08), the spend and token caps (LOOP-07), `minos stop` (CLI-10), and a provider rate limit (PROP-09) each map to one variant. No free text from the shell enters the ledger through this event; display strings are fixed per variant.
- **D-06:** `RunId::of(&ManifestId, &TreeId, started_at)` is a domain-separated, length-prefixed hash of the manifest ID, the baseline tree, and a shell-supplied start time (Unix seconds). `RunStarted` records `started_at` so the fold can recompute and verify the ID. Core never reads a clock: the CLI passes the reading in. Two runs of the same project a second apart are distinct runs.
- **D-07:** `Steered { generation: u32, ordinal: u32, note: BlobId }` with `EventKey = H(run, generation, ordinal)`. The note text is a blob, never inline, so the ledger line carries no hostile text. The CLI folds the ledger, counts the notes already recorded for the upcoming generation, and appends with the next ordinal; identical notes stay distinct, and the prompt assembler (Phase 6) reads a generation's notes in ordinal order.
- **D-08:** `ReviewRequested { request: ReviewId, kind: ReviewKind }` and `Reviewed { request: ReviewId, answer: ReviewAnswer }` take minimal closed enums now: `ReviewKind::{JudgeDesign, Params, Dependencies}` and `ReviewAnswer::{Approved, Declined}`, keyed `H(run, request)`. Nothing in v1 acts on them; the fold records them and the golden file covers one of each. POL-01 (the inbox, `answer`, `DesignJudge`) adds variants and payloads later without a `V2` record, because adding a serde enum variant leaves every existing line parseable.
- **D-09:** `Spent` is keyed by `SpendId = H(run, generation, source)` where `source: SpendSource` is a closed enum with an index: `ModelCall { call: u32 }`, `Candidate { exp: ExperimentId, trial: u32 }`, `Judge { exp: ExperimentId, trial: u32 }`. One model call or one VM run is one `Spent` record, so a resumed generation that re-runs a model call gets a new index and nothing is double-counted or silently dropped (Pitfall 19). `exp` is derivable from the source; whether the field stays on the event for readability is the planner's call, decided before the golden file.
- **D-10:** The rest of the set is as REQUIREMENTS.md CORE-08 and `.planning/research/ARCHITECTURE.md` Amendment A9 state: `RunStarted { run, manifest, baseline, started_at }`, `ManifestChanged { manifest }`, `Proposed { exp, parent, tier, rationale: BlobId, model: ModelId, prompt: PromptHash }`, `Judged { exp, judge, trial, outcome, backend }`, `Selected { generation, decision }`, `ReportAcknowledged { exp, report }`. Keys: `RunStarted` to the run, `ManifestChanged` to the manifest, `Proposed` to the experiment, `Judged` to (exp, judge, trial), `Selected` to the generation, `ReportAcknowledged` to (exp, report). Provider info lives on `Proposed` (model ID, prompt hash), not in a separate `ProviderUsed` event. Every key is documented in the `ledger` module docs in the same change that freezes the set.

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

### Deferred Ideas (OUT OF SCOPE)

- **`tool.rs` and the `ToolCall` enum** — the agentic tier (TIER-01, later milestone). Not a Phase 3 deliverable even though CLAUDE.md lists the file.
- **Rich `ReviewRequested`/`Reviewed` payloads, the inbox, `answer`, and `DesignJudge`** — POL-01, later milestone; D-08 leaves room by enum extension.
- **Pareto and `review` policies** — POL-01; `select_pareto` is not in CORE-10.
- **Runtime-behavior report flags** (spawned processes, sockets, environment reads, operations the guest denied) — RPT-01, v1.x or later; the five v1 flags are D-15 through D-18.
- **The `--allow-noisy` override flag's name and the default baseline repeat count** — Phase 8, where the noise floor is measured (STATE.md blocker for Phase 8).
- **`git apply` and `patch -p1` golden test on the canonical patch bytes** — Phase 8, CLI-09; this phase fixes the canonical form it will test.
- **Power source and thermal state in `BackendInfo`** (PITFALLS Pitfall 16) — Phase 5 or later; the `BackendInfo` type should not preclude it.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-01 | Distinct SHA-256 newtypes (`ExperimentId`, `BlobId`, `ManifestId`, `TreeId`, `PricesId`, `JudgeVersion`); composite IDs domain-separated and length-prefixed; golden vectors for every hash | Pattern 2 (one `IdHasher` builder); `sha2` 0.11 `finalize().into()` verified; golden vectors computed independently with `shasum` (Code Examples); `JudgeVersion` definition is Open Question 3 |
| CORE-02 | `RelPath` rejects `..`, absolute, empty, non-normal; property-tested | Pattern 3 (conservative ASCII component grammar); proptest strategies in Validation Architecture |
| CORE-03 | `Manifest::try_from(RawManifest)` cross-field checks; `deny_unknown_fields` | Pattern 1 (Raw mirror and `try_from`); toml 1.1.8 error locations verified; schema gaps (judge paths, image field, sweep params) are Open Questions 4, 5, 6 |
| CORE-04 | `Diff::parse` caps, writable-only paths, rejects renames/modes/symlinks/binary; canonical bytes are what `ExperimentId` hashes | Pattern 4 (seven-stage parse pipeline); diffy 0.5.2 gitdiff behaviour verified line by line (Verified API Facts table) |
| CORE-05 | Strict exact-position applier; guest and `adopt` share it | Pattern 5 (hunk start semantics verified against `diffy::create_patch` output, `git apply`, and `patch -p1`) |
| CORE-06 | `Metric` finite, `-0.0` normalized, `Ord` via `total_cmp`; `held_out` marking | Pattern 6; serde_json f64 round-trip failure found and fixed with `float_roundtrip` (Pitfall 1) |
| CORE-07 | `Outcome` is `Scored | Rejected(Reason) | Crashed | TimedOut` | Pattern 6; maps the Phase 2 K7 classification rule |
| CORE-08 | Frozen `Event` set with a documented `EventKey` per event, before any golden file | Pattern 7; the event-set gap list ("Decide before the freeze") |
| CORE-09 | Fold is the only source of run state; property-tested and golden-tested for `Record::V1` | Pattern 7; v-tag shape verified (unknown fields, unknown variants, unknown `v` all rejected) |
| CORE-10 | `select_greedy` honours `min_improvement`, tie-breaks on `ExperimentId`, permutation-invariant | Pattern 8 (median reduction per D-11, multiply-not-divide threshold) |
| CORE-11 | Routing climbs one tier per `stall_step` stalled generations; `eligible` gives a reason per ineligible tier | Pattern 8 (D-13 formula) |
| CORE-12 | Seeded sweep with own range mapping, identical samples for identical seeds | Pattern 9; rand_core 0.10 trait rename verified (`Rng`, not `RngCore`) |
| CORE-13 | `JobRequest` and `JobResult` contract | Pattern 10 (field lists, no `u128` inside tagged enums) |
| CORE-14 | Pico-dollar `u128` rates from strings; micro-dollar totals; no float | Pattern 11 (deferred-division accumulation) |
| CORE-15 | Allowlist renderer: strip C0/C1/ESC, show bidi and zero-width visibly | Pattern 12 (explicit code-point table) |
| CORE-16 | `build_report` pure, stable hash, summary, cumulative diff, lineage, provenance, five flags | Pattern 13 (hash the rendered bytes the user reviews) |
| CORE-17 | Circuit breaker stop decision | Pattern 8; Open Question 8 (where the infra-failure streak comes from) |
| COST-01 | Ledger stores usage only; dollars from `prices.toml` with string rates | Pattern 11; toml rejects a float where a string is expected, with a location (verified) |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Directives the planner must verify every task against. They carry the same authority as the locked decisions.

- **Lints:** `unsafe_code`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing` are `forbid` workspace-wide and in the `.cargo/config.toml` backstop. Neither `#[allow]` nor `#[expect]` can silence them, tests included. `indexing_slicing` also forbids string and slice ranges (`&s[a..b]`, `a[0]`): use `.get(..)`, `split_at_checked`, `strip_prefix`, iterators, `to_be_bytes`.
- **No `#[allow(` anywhere.** The C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` must print nothing. `allow_attributes` and `allow_attributes_without_reason` are `deny`.
- **`missing_docs` is warn and the gate runs `-D warnings`,** so every public item needs a doc comment; pedantic `missing_errors_doc` needs a `# Errors` section on every public fallible function (rules/doc-errors-section.md); `must_use_candidate` wants `#[must_use]` on pure functions.
- **Deserialization is validation.** Every parsed type uses `#[serde(try_from = "Raw...")]` through the same constructor as hand-built values; raw structs use `#[serde(deny_unknown_fields)]`; never `#[serde(flatten)]` on raw input.
- **Validated types have private fields and no setters.**
- **Errors separate blame:** `thiserror` enums distinguish candidate faults (become an `Outcome` or a rejection, never retried) from infrastructure faults (retried). No `anyhow` and no `BoxError` in core (`deny.toml` fences `anyhow` to `minos-cli`).
- **Stable order:** `BTreeMap`/`BTreeSet` or a sorted `Vec` wherever order affects output, hashing, or decisions. `HashMap`/`HashSet` are banned in core by `clippy.toml`.
- **Core purity:** no async, no AWS, no filesystem, no clock, no environment, no printing, no process spawning, no unseeded randomness. `crates/minos-core/clippy.toml` bans `SystemTime::now`, `Instant::now`, `thread::sleep`, `env::var`, `env::var_os`, `env::args`, `fs::read`, `fs::read_to_string`, `fs::write`, `io::stdin`, the types `HashMap`, `HashSet`, `fs::File`, `process::Command`, `TcpStream`, `TcpListener`, `UdpSocket`, and the macros `println!`, `eprintln!`, `dbg!`. These bans apply to core's tests too: golden fixtures load with `include_str!`/`include_bytes!`, never `std::fs`.
- **No tokio** in any `minos-*` crate. **No C**, no OpenSSL, no libgit2, **no git in the trusted path.**
- **Commands are argv arrays;** never build a shell string from manifest or model input.
- **Wire formats are an API:** golden files guard serialization, golden vectors guard hashing, the `v` tag is how a format changes.
- **Money:** usage in the ledger, never dollars; rates are exact decimals with pico-dollar precision; totals are integer micro-dollars; output says `approx. cost: $X.XX` and never "exact".
- **Supply chain:** `Cargo.lock` committed; `cargo deny check` gates every build; every dependency is an exact pin in `[workspace.dependencies]`.
- **clap derive is off** (not relevant to core, but no derive macro that emits `#[allow]` may be added).
- **Never weaken a security invariant to make something compile or pass;** stop and ask.
- **Check external APIs against their source before writing code against them** (done in this research; see Verified API Facts).
- **Keep CLAUDE.md current** in the same change when a decision changes (for example the `RngCore` sketch, the `[judge] version` integer, the `spend_usd` literal).
- **Gate commands** at the end of every plan: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo doc --workspace --no-deps`, plus the C-01 and C-02 checks; **`cargo clean` at the end of the phase** after verification passes (GATE-06).
- **No scripts:** policy lives in Cargo-native config (`Cargo.toml`, `clippy.toml`, `deny.toml`), never an xtask, script, `build.rs`, or cargo alias.

## Summary

Phase 3 adds no new crate. Every dependency it needs is already pinned in `[workspace.dependencies]` and declared in `crates/minos-core/Cargo.toml` (serde, serde_json, toml, thiserror, sha2, base16ct, diffy, rand_chacha, rand_core; proptest and insta as dev-dependencies). The work is domain modelling under very strict lints, so the research effort went into reading the pinned crate sources and running offline compile probes to find where those crates behave differently from what the design of record assumes. Five findings change what the planner must write:

1. **serde_json 1.0.151 does not round-trip `f64` with default features.** In a probe of 2,000,000 typical metric values (uniform in [0, 1e6)), 206,407 came back from `to_string` then `from_str` with different bits (for example `969660.5160613243`). Enabling the `float_roundtrip` feature brought mismatches to 0 in the same 2,000,000-value probe and in a second probe of 2,000,000 arbitrary finite bit patterns. Without it, a `Judged` metric written to the ledger is not the metric read back, which breaks the golden round-trip, the fold's "same events, same state" property, and resume equivalence (LOOP-05). Add `features = ["float_roundtrip"]` to the workspace `serde_json` pin.
2. **diffy's `gitdiff` mode only starts a patch at a `diff --git` line,** and returns `no valid patches found` for a plain `--- a/x` / `+++ b/x` unified diff. It also silently skips prose before the first `diff --git` and extra lines after a complete hunk, accepts zero hunks, accepts the same file twice, keeps `\r` as line content, ignores a `diff --git` path that disagrees with `---`/`+++`, and keeps the `@@ ... @@ function context` text. `unidiff` mode is worse: it silently ignores `rename from` and `new mode 120000` headers entirely. So the locked choice (gitdiff mode) is right, but it is one stage of a pipeline, not the whole validator. The canonical form must therefore include a `diff --git a/P b/P` line so that it re-parses in the same mode; `git apply` and `patch -p1` both accept that form (verified).
3. **`insta` is not byte-exact.** Its comparison trims trailing whitespace, converts CRLF to LF, and dedents multi-line inline snapshots, so a golden snapshot cannot catch a lost final newline or a stray `\r` in canonical diff or report bytes. Pin exact bytes with a SHA-256 hex literal (`assert_eq!`) beside every insta wire snapshot. Separately, `INSTA_UPDATE=no` can live in a committed `.config/insta.yaml` (`behavior: update: no`), so the plain `cargo test --workspace` gate never writes `.snap.new` files with no environment variable set (verified).
4. **rand_core 0.10 renamed `RngCore` to `Rng`.** `RngCore` still exists as a `#[deprecated]` stub, so code written against the orchestrator note's `RngCore::next_u64` fails the gate under `-D warnings`. Use `rand_core::{Rng, SeedableRng}` with `ChaCha8Rng::from_seed([u8; 32])` and `next_u64()`.
5. **The locked event field lists leave gaps the fold cannot fill.** `Proposed` carries no diff blob reference and no generation; `RunStarted` cannot recompute `RunId` (D-06) without the base `TreeId`; `StopRequested` has no `EventKey`; `ManifestChanged` keyed by the manifest collapses A to B to A. These must be decided in the same change that freezes the set, before any golden file (see "Decide before the freeze").

`insta` and `proptest` macros pass `cargo clippy --all-targets -- -D warnings` under the three `forbid` lints and pedantic (verified with a probe crate carrying the workspace lint table), so neither tool forces an escape hatch.

**Primary recommendation:** Build `minos-core` in waves that respect the freeze ordering. Wave 1: IDs and hashing, `RelPath`, `limits.rs`, `Metric`/`Scorecard`/`Outcome`, money, and the renderer. Wave 2: `Manifest` and the `Diff` pipeline with the applier. Wave 3: freeze the `Event` set and its keys, plus the fold. Wave 4: golden wire files, then policy, routing, sweep, the circuit breaker, and `build_report`. Do this with the `float_roundtrip` feature and `.config/insta.yaml` in place first.

## Architectural Responsibility Map

All of Phase 3 lives in one tier: the pure core library. The table records which later tier consumes each capability and where the boundary sits, so a plan never pulls I/O into core.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| ID types and hashing | Core (pure) | Store (Phase 4) verifies blob hashes on read | Hashing is deterministic computation; the store only compares |
| `RelPath`, `Manifest` validation | Core (pure) | CLI reads `minos.toml` text and passes it in | Core never touches the filesystem; the shell supplies bytes |
| `Diff` parse, canonical form, applier, compose | Core (pure) | Guest (Phase 5) applies in the VM; CLI `adopt` writes bytes | One implementation shared by guest, report, and adopt (CORE-05) |
| `Metric`, `Scorecard`, `Outcome` | Core (pure) | Sandbox `Untrusted::into_scorecard` (Phase 4) is the only exit that builds a `Scorecard` from VM bytes | The validated type and its raw mirror live in core; the capped exit lives in sandbox |
| `Event`, `Record`, `EventKey`, fold, `RunState` | Core (pure) | Store (Phase 4) appends keyed lines and dedups on key | Keys are derivable from the event plus `RunId`, so the store can verify them |
| `select_greedy`, routing, noise check, circuit breaker, spend check | Core (pure) | Loop (Phase 7) calls them and appends the resulting events | ARCHITECTURE.md Pattern 1: any `if` that decides experiment fate belongs in core |
| Sweep sampler | Core (pure) | Propose (Phase 6) wraps it as a tier | Seeded and value-stable |
| `JobRequest`/`JobResult` | Core (types only) | Loop sends, runner (Phase 7) answers | Contract types; no behaviour |
| Money (`Rate`, `Usage` pricing, `ApproxCost`) | Core (pure) | CLI reads `prices.toml` text, prints `ApproxCost` | The ledger stores usage only |
| Renderer (`SafeText`) | Core (pure) | CLI prints only through it | One type owns sanitization |
| `build_report`, `ReportHash` | Core (pure) | CLI fetches blobs, renders, records acknowledgement; `adopt` recomputes | The hash covers exactly the bytes shown |

## Standard Stack

No new crates. Everything below is already pinned and declared; versions confirmed in `Cargo.lock` this session.

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sha2` | =0.11.0 | Every ID hash | RustCrypto, pure Rust; `Sha256::new()`, `update`, `finalize().into()` into `[u8; 32]` verified by probe [VERIFIED: compile probe against pinned crate] |
| `base16ct` | =1.0.0 (`alloc`) | Lowercase hex for IDs | `base16ct::lower::encode_string(&[u8]) -> String`; `base16ct::lower::decode(src, &mut [u8]) -> Result<&[u8], Error>` (rejects uppercase) [VERIFIED: crate source] |
| `diffy` | =0.5.2, `default-features = false`, `features = ["std"]` | Parse git diffs (`patch_set::PatchSet::parse(text, ParseOptions::gitdiff())`); generate cumulative diffs (`create_patch`, `DiffOptions`) | Detects renames, copies, modes, binary markers [VERIFIED: probe] |
| `serde` / `serde_json` | =1.0.229 / =1.0.151 **plus `float_roundtrip`** | Ledger records, job contract, scorecards | See Summary finding 1 [VERIFIED: probe, 0 mismatches with the feature] |
| `toml` | =1.1.8+spec-1.1.0 | `minos.toml`, `prices.toml` | Unknown fields, duplicate tables, wrong types all rejected with line and column [VERIFIED: probe] |
| `thiserror` | =2.0.21 | Typed errors | Already the pattern in `minos-guest` and `minos-vmm` |
| `rand_chacha` / `rand_core` | =0.10.0 (`default-features = false`) / =0.10.1 | Seeded sweep stream | `ChaCha8Rng` is documented deterministic and portable; traits are `rand_core::{Rng, SeedableRng}` [VERIFIED: crate source] |

### Supporting (dev only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | =1.11.0 | Properties for parsers, round trips, permutation invariance, fold | `proptest!`, `prop_assert_eq!`, `Just(v).prop_shuffle()`, `collection::vec`, regex string strategies; clean under the forbid lints [VERIFIED: probe] |
| `insta` | =1.49.0 (`json`, `toml`) | Human-reviewable golden wire snapshots | `assert_snapshot!`, `assert_json_snapshot!`; clean under the forbid lints; not byte-exact (pair with a hash literal) [VERIFIED: probe and source] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `serde_json` `float_roundtrip` | Serialize `Metric` as a decimal string formatted by Rust's `{}` and parsed by `str::parse::<f64>` (both exact) | Independent of serde_json's float code entirely, but makes ledger metrics strings, diverges from the scorecard's JSON numbers, and still needs `float_roundtrip` for `into_scorecard` to keep the judge's exact value. Use the feature. |
| insta for byte-exact goldens | Plain `assert_eq!(bytes, include_bytes!("golden/x.jsonl"))` | Byte-exact and tool-free, but CONTEXT locks insta. Keep insta and add the hash literal. |
| diffy `Patch::to_bytes`/`Display` for the canonical form | Own serializer | diffy's formatter calls `.unwrap()` internally (harmless in a dependency) and its output is not under our version control; the canonical bytes are an ID input, so own them. |

**Installation:** one edit, no new crate:

```toml
# Cargo.toml [workspace.dependencies]
serde_json   = { version = "=1.0.151", features = ["float_roundtrip"] }
```

`float_roundtrip` adds no dependency (it compiles serde_json's internal `lexical` module) [VERIFIED: serde_json 1.0.151 Cargo.toml features table]. Feature unification makes every crate's serde_json exact, which is harmless. `cargo deny check` is unaffected.

## Package Legitimacy Audit

Phase 3 installs no new package. The seam was run over the crates core already uses, for the record.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| diffy | crates.io | 6 yrs | 357k/wk | github.com/bmwill/diffy | OK | Approved (already pinned) |
| sha2 | crates.io | 10 yrs | 23M/wk | github.com/RustCrypto/hashes | OK | Approved (already pinned) |
| base16ct | crates.io | 4 yrs | 5.9M/wk | github.com/RustCrypto/formats | OK | Approved (already pinned) |
| rand_chacha | crates.io | 8 yrs | 26M/wk | github.com/rust-random/rand | OK | Approved (already pinned) |
| rand_core | crates.io | 9 yrs | 38M/wk | github.com/rust-random/rand | OK | Approved (already pinned) |
| toml | crates.io | 12 yrs | 19.7M/wk | github.com/toml-rs/toml | OK | Approved (already pinned) |
| serde / serde_json | crates.io | 11 yrs | 27.6M/wk each | github.com/serde-rs | OK | Approved (already pinned; adds a feature only) |
| thiserror | crates.io | 7 yrs | 32.6M/wk | github.com/dtolnay/thiserror | OK | Approved (already pinned) |
| proptest | crates.io | 9 yrs | 4.3M/wk | github.com/proptest-rs/proptest | OK | Approved (already pinned, dev) |
| insta | crates.io | 7 yrs | 2.4M/wk | github.com/mitsuhiko/insta | OK | Approved (already pinned, dev) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Verified API Facts

Each row was read in the pinned source under `~/.cargo/registry/src/index.crates.io-*/` and, where marked "probe", executed in an offline scratch crate (`cargo +1.97.1 run --offline`) built from a copy of this repo's `Cargo.lock`.

| Crate | Fact | How verified |
|---|---|---|
| diffy 0.5.2 | Entry point: `diffy::patch_set::{PatchSet, ParseOptions}`; `PatchSet::parse(&str, ParseOptions::gitdiff())` is an `Iterator<Item = Result<FilePatch<'_, str>, PatchSetParseError>>` | source |
| diffy 0.5.2 | `FilePatch::{operation(), patch(), old_mode(), new_mode()}`; `patch()` returns `&PatchKind` with `is_binary()`, `as_text() -> Option<&Patch>` | source |
| diffy 0.5.2 | `FileOperation::{Delete(p), Create(p), Modify{original, modified}, Rename{from, to}, Copy{from, to}}` plus `is_create()`, `is_delete()`, `is_modify()`, `is_rename()`, `is_copy()`, `strip_prefix(n)` | source |
| diffy 0.5.2 | `FileMode::{Regular, Executable, Symlink, Gitlink}` from `100644/100755/120000/160000`; `old_mode`/`new_mode` come from `old mode`/`new mode` or `new file mode`/`deleted file mode` | source, probe (`new mode 120000` gives `new_mode = Some(Symlink)`) |
| diffy 0.5.2 | Plain unified diff (no `diff --git`) in gitdiff mode: `Err(no valid patches found)` | probe |
| diffy 0.5.2 | In **unidiff** mode a rename or mode header is silently ignored (`op = Modify`, modes `None`) | probe |
| diffy 0.5.2 | `Binary files ... differ` and `GIT binary patch` both give `is_binary() == true` with the `binary` feature off | probe |
| diffy 0.5.2 | `--- /dev/null` gives `FileOperation::Create`; paths keep the `a/` and `b/` prefixes (`"b/src/new.rs"`) | probe |
| diffy 0.5.2 | Rename op paths have **no** prefix (`from: "src/parse.rs"`), because they come from `rename from` lines | probe |
| diffy 0.5.2 | Prose before the first `diff --git`, and extra `+` lines or prose after a complete hunk, are silently skipped | probe |
| diffy 0.5.2 | A file section with `---`/`+++` and zero hunks parses (`hunks = []`) | probe |
| diffy 0.5.2 | The same path in two sections yields two `FilePatch`es | probe |
| diffy 0.5.2 | A `diff --git a/judge/j.py b/judge/j.py` line followed by `--- a/src/parse.rs` is reported as `src/parse.rs` (the `diff --git` path is ignored) | probe |
| diffy 0.5.2 | CRLF input: line content keeps `\r` (`Delete("old\r\n")`) | probe |
| diffy 0.5.2 | Tab-separated timestamps after a filename are dropped; quoted names with octal escapes are decoded (`"b/sp\303\251ace"` gives `b/spéace`) | probe |
| diffy 0.5.2 | A bare empty line inside a hunk counts as a context line `"\n"` | probe |
| diffy 0.5.2 | Hunk count mismatch: `Err(hunk header does not match hunk)`; overlapping hunks: `Err(hunks not in order or overlap)`; adjacent hunks allowed | probe, source (`verify_hunks_in_order`) |
| diffy 0.5.2 | `Hunk::{old_range(), new_range(), function_context(), lines()}`; `HunkRange::{start(), len(), end(), is_empty()}`; `start` is the 1-based number written in the header; for `len == 0` it is the line *after which* the change sits (`-4,0` inserts after line 4; `+1,0` on the new side of a delete-all) | probe of `create_patch` output |
| diffy 0.5.2 | `Line::{Context(&str), Delete(&str), Insert(&str)}` each **include** the trailing `"\n"`; a following `\ No newline at end of file` marker strips it from the previous line | source, probe |
| diffy 0.5.2 | `create_patch(original, modified) -> Patch<str>`; `DiffOptions::new()` defaults `context_len = 3`; `set_context_len(n)`; dropping a final newline shows as `-b` / `+b` plus `\ No newline at end of file` | probe |
| diffy 0.5.2 | `diffy::apply` locates hunks by `new_range().start()` then searches interleaved offsets: never use it for candidates | source (`find_position`) |
| git 2.50.1 / patch 2.0-12u11-Apple | Both accept `diff --git a/P b/P` + `--- a/P` + `+++ b/P` + hunks, and the same without the `diff --git` line; both accept `\ No newline at end of file`. `git apply` **refuses zero-context hunks not at the start of the file** ("patch does not apply") where `patch -p1` applies them | probe |
| sha2 0.11.0 | `let out: [u8; 32] = h.finalize().into();` and `Sha256::digest(b"...").into()` compile | probe |
| rand_core 0.10.1 | Trait is `rand_core::Rng` (`next_u32`, `next_u64`, `fill_bytes`); `RngCore` is `#[deprecated(since = "0.10.0", note = "use Rng instead")]` | source |
| rand_chacha 0.10.0 | `rand_chacha::ChaCha8Rng`; seed with `rand_core::SeedableRng::from_seed([u8; 32])`; re-exports `rand_chacha::rand_core` | source, probe |
| toml 1.1.8 | Unknown field: `TOML parse error at line 4, column 1 ... unknown field `extra``; duplicate `[project]` table: `duplicate key`; `width = 1.5` into `u32`: `invalid type: floating point `1.5``; a `try_from` error on a nested table is reported at that table's header line | probe |
| serde_json 1.0.151 | Duplicate key into a struct: `Err(duplicate field `x`)`; into a `BTreeMap`: **last value wins silently**; `1e400`: `Err(number out of range)`; `NaN`: `Err(expected value)`; `-0.0` parses to negative zero and serializes as `-0.0` | probe |
| serde_json 1.0.151 | Float **formatting** uses the `zmij` crate (dependency in its Cargo.toml) | source |
| serde 1.0.229 | `#[serde(tag = "v")] enum Record { #[serde(rename = "1")] V1(Event) }` with an externally tagged, `deny_unknown_fields` `Event` gives `{"v":"1","run_started":{...}}`; an extra field in a variant, an unknown variant, and `"v":"2"` are all rejected | probe |
| serde 1.0.229 | A `u128` field inside an internally tagged enum fails: `u128 is not supported` | probe |
| insta 1.49.0 | Mode resolution: env `INSTA_UPDATE` first, then `.config/insta.yaml` / `insta.yaml` / `.insta.yaml` key `behavior.update`, default `auto`; `no` never writes files; any non-`InPlace` result panics the test | source, probe |
| insta 1.49.0 | Comparison normalizes: `trim_end()`, `\r\n` to `\n`, and inline multi-line dedent | source (`TextSnapshotContents::normalize`) |
| clippy 1.97.1 | `usize as u64` does not trigger `cast_possible_truncation` under pedantic | probe |

## Architecture Patterns

### System Architecture Diagram

Data enters core only as bytes or values handed in by a shell; nothing leaves except values. Hostile inputs are marked (H).

```
 shell-supplied inputs                        minos-core (pure)                                  outputs (values only)
 ─────────────────────                        ─────────────────                                  ─────────────────────
 minos.toml text ─────────► toml + RawManifest ──try_from──► Manifest ─────────────┐
 prices.toml text ────────► toml + RawPrices ───try_from──► Prices (pico u128) ─────┼──► spend_ok / ApproxCost
 surface file bytes ──────┐                                                        │
 model or sweep text (H) ─┴► Diff::parse pipeline ─► Diff (canonical bytes) ──┐     │
                                 │ reject: caps, \r, NUL, grammar,            │     │
                                 │ rename/copy/mode/binary/create/delete,     ▼     │
                                 │ non-writable, no-op, unconsumed lines   compose(base, parent, proposal)
                                 │                                            │ apply (strict) + create_patch
                                 ▼                                            ▼
                              Rejected(reason)                 Experiment{base TreeId, cumulative Diff}
                                                                    │ ExperimentId::of(base, diff)
 scorecard JSON (H) ─► RawScorecard ─try_from─► Scorecard ─► Outcome│
                                                                    ▼
 ledger records ─────────► Record::V1(Event) ──fold──► RunState ──┬──► select_greedy (median per D-11) ─► Decision
   (keyed by EventKey)                                            ├──► route (D-13) ─► Tier + reasons
                                                                  ├──► noise_check (D-12) ─► NoiseVerdict
                                                                  ├──► circuit_breaker (D-14) ─► Option<StopReason>
                                                                  ├──► sweep::sample(seed, params) ─► proposal texts
                                                                  └──► build_report(state, blobs) ─► Report bytes ─► ReportHash
                                                                                                      │
 any hostile string (H) ───────────────────────────────────────────────► SafeText (allowlist) ────────┘ (every string in the report)
```

### Recommended Module Layout

CLAUDE.md lists `id.rs path.rs manifest.rs experiment.rs outcome.rs policy.rs route.rs ledger.rs require.rs usage.rs report.rs tool.rs limits.rs`. `tool.rs` is deferred (TIER-01). `require.rs` has no stated content in any planning document; fold it into `manifest.rs` (the `requires = { arch = "arm64" }` table) unless the planner finds a use. Recommended:

```
crates/minos-core/
├── .. clippy.toml                     # exists; do not weaken
├── src/
│   ├── lib.rs                         # module docs, re-exports
│   ├── limits.rs                      # every cap and default, each with its reason
│   ├── id.rs                          # IdHasher + ExperimentId, BlobId, TreeId, ManifestId, PricesId,
│   │                                  #   JudgeVersion, RunId, EventKey, SpendId, ReviewId, ReportHash, PromptHash
│   ├── path.rs                        # RelPath
│   ├── metric.rs                      # Metric, MetricName, Scorecard (+ RawScorecard)
│   ├── outcome.rs                     # Outcome, Reason
│   ├── manifest.rs                    # RawManifest -> Manifest, Goal, MinImprovement, ProposerSpec, Tier,
│   │                                  #   CommandArgv, Budget, ImageRef, SweepParam
│   ├── diff/                          # mod.rs (Diff, parse pipeline, canonical serializer), apply.rs (strict
│   │                                  #   applier, Tree), compose.rs (cumulative diff via create_patch)
│   ├── experiment.rs                  # Experiment (base + Diff, or baseline)
│   ├── usage.rs                       # Usage (integer usage only)
│   ├── money.rs                       # Rate (pico u128), Prices, cost, ApproxCost
│   ├── ledger.rs                      # Event, Record, event keys (documented), StopReason, Review*, BackendInfo
│   ├── state.rs                       # fold -> RunState
│   ├── policy.rs                      # median reduction, select_greedy, noise_check, circuit_breaker, spend_ok
│   ├── route.rs                       # eligibility with reasons, route()
│   ├── sweep.rs                       # seed derivation, unbiased below(n), sample()
│   ├── job.rs                         # JobRequest, JobResult
│   ├── text.rs                        # SafeText renderer
│   └── report.rs                      # build_report, flags, ReportHash
└── tests/
    ├── golden_ids.rs                  # hash vectors (plain assert_eq! on hex literals)
    ├── golden_wire.rs                 # insta snapshots + byte-hash pins (only after the freeze)
    ├── fixtures/                      # *.toml, *.diff, *.jsonl loaded with include_str!
    ├── snapshots/                     # insta .snap files (committed)
    └── prop_*.rs                      # property tests per area
```

Integration tests in `tests/` see only the public API, which forces every fixture through the public validating constructors. That is the point.

### Pattern 1: Raw mirror, `try_from`, private fields (the repo's established shape)

**What:** Each parsed type has a `Raw*` struct with `#[serde(deny_unknown_fields)]` and private fields, and a validated type with `#[serde(try_from = "Raw*")]` (and `into = "Raw*"` or a hand-written `Serialize` for round-tripping). Range constants sit at the top of the module with their reason, accessors are `const fn` where possible, and errors carry the dotted field path (`InvalidField { field: &'static str, limit: &'static str }`).
**When to use:** `Manifest`, `Prices`, `Scorecard`, every `Event` payload that holds a validated type, `JobRequest`/`JobResult`.
**Source:** `crates/minos-guest/src/job.rs` (`RawJob`, `Job`, `InvalidField`, `MIN_STEPS`/`MAX_STEPS`), `crates/minos-vmm/src/config.rs`.

Nested validated types deserialize through their own `try_from`, so a `Manifest` field of type `RelPath` validates on parse with no extra code. Never `#[serde(flatten)]` on raw input; never `#[serde(default)]` that fills a value the constructor would reject.

### Pattern 2: One hashing builder for every composite ID

**What:** A crate-private builder that writes a NUL-terminated domain string, then each field as either a fixed 32-byte digest or a `u64` big-endian length prefix followed by bytes. Every composite ID constructor uses it, so no constructor can forget a length prefix.

```rust
// Source: CLAUDE.md minos-core IDs sketch; sha2 0.11 API verified by probe.
use sha2::{Digest, Sha256};

pub(crate) struct IdHasher(Sha256);

impl IdHasher {
    /// `domain` must end in a NUL byte, e.g. `b"minos/experiment/v1\0"`.
    pub(crate) fn new(domain: &'static [u8]) -> Self {
        let mut h = Sha256::new();
        h.update(domain);
        Self(h)
    }
    pub(crate) fn digest(mut self, d: &[u8; 32]) -> Self {
        self.0.update(d);
        self
    }
    pub(crate) fn bytes(mut self, b: &[u8]) -> Self {
        self.0.update((b.len() as u64).to_be_bytes()); // usize -> u64 never truncates; clippy-clean (probe)
        self.0.update(b);
        self
    }
    pub(crate) fn u32(mut self, v: u32) -> Self {
        self.0.update(v.to_be_bytes());
        self
    }
    pub(crate) fn u64(mut self, v: u64) -> Self {
        self.0.update(v.to_be_bytes());
        self
    }
    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
```

Plain digests (no domain): `BlobId`, `TreeId` (the tarball's `BlobId`), and the stored-bytes IDs `ManifestId` and `PricesId` (ARCHITECTURE.md A10, Anti-Pattern 6: S3's `ChecksumSHA256` is the plain digest). Domain-separated: `ExperimentId` (`minos/experiment/v1`), `RunId` (`minos/run/v1`), `EventKey` (`minos/event/v1` plus a per-kind tag byte so keys of different kinds never collide), `SpendId`, `ReviewId`, `ReportHash`, `PromptHash`, `JudgeVersion` (see Open Question 3), and later Phase 4's `LaunchKey`. Make the builder `pub` (documented) if Phase 4 needs it for `LaunchKey`; otherwise keep it `pub(crate)`.

Serde for IDs: a 64-character lowercase hex string, parsed with `base16ct::lower::decode` into `[u8; 32]` after an explicit length check of 64, with any other length or case rejected.

### Pattern 3: `RelPath` as a conservative ASCII grammar

**What:** Accept only `component ("/" component)*` where each component matches `[A-Za-z0-9._+-]+`, is not `.` or `..`, and does not start with `-`. Reject empty input, a leading or trailing `/`, `//`, `\`, NUL, any non-ASCII byte, more than `MAX_PATH_BYTES` bytes (1024 recommended), more than `MAX_PATH_DEPTH` components (32), and any component over 255 bytes. Store as a `String`; `Ord` is bytewise, so `BTreeSet<RelPath>` order is stable.
**Why ASCII:** PITFALLS Pitfall 14 (homoglyph paths). It also makes `RelPath`'s `Display` safe without the renderer.
**Disjointness (D-01):** exact-path set disjointness, plus a fail-closed ASCII case-fold collision check (`src/Parse.rs` vs a judge path `src/parse.rs`), because the dev host's APFS is case-insensitive by default. [ASSUMED: the case-fold check is a recommendation beyond D-01; it only rejects more manifests.]
**Cross-phase note:** Phase 4's source tarball may meet project files with non-ASCII names. That is Phase 4's decision (refuse with a clear error, or use a broader tar path type); the strict `RelPath` stays strict for surface, diff, and output-tree paths.

### Pattern 4: `Diff::parse` as a seven-stage pipeline

diffy's gitdiff parser is one stage. The pipeline, in order:

1. **Caps and bytes, before any parsing:** `text.len() <= MAX_DIFF_BYTES`; no `\0`; no `\r` anywhere (LF-only canonical form, PITFALLS Pitfall 15; a CRLF surface file therefore cannot be edited in v1, a documented limitation the planner may revisit); line count `<= MAX_DIFF_LINES`.
2. **Line-kind state machine (our code):** per file section, `diff --git a/P b/P` exactly, then optionally one `index <hex>..<hex>[ <mode>]` line, then `--- a/P`, then `+++ b/P` (the same `P` on all three, byte-equal), then one or more hunks, each a `@@ -a[,b] +c[,d] @@[ ...]` line followed by lines starting with ` `, `+`, `-`, a bare `\n` (context), or exactly `\ No newline at end of file`. Any other line is a rejection with its line number: prose, `rename from/to`, `copy from/to`, `old mode`, `new mode`, `new file mode`, `deleted file mode`, `similarity index`, `dissimilarity index`, `Binary files`, `GIT binary patch`. This closes every gap diffy silently skips.
3. **diffy parse:** `PatchSet::parse(text, ParseOptions::gitdiff())`; any `Err` is a rejection.
4. **Per-file semantic checks (defense in depth, even though stage 2 already excludes most):** `!patch().is_binary()`; `old_mode().is_none() && new_mode().is_none()`; `operation()` is `Modify { original, modified }` with `original == "a/" + P` and `modified == "b/" + P`; `P` parses as `RelPath` and is in `writable` (exact, D-01); each path appears in at most one section; at least one hunk (D-03); no hunk whose old-side line sequence equals its new-side sequence, comparing full line content including the presence of the final newline (D-03; `-b\n` / `+b` is a real change); per-file hunk count `<= MAX_HUNKS_PER_FILE`; new-side starts consistent with old-side starts plus the running delta (reject otherwise, so the applier and `git apply` can never disagree about which number to use).
5. **Line accounting:** the number of input lines must equal the sum over sections of (1 `diff --git` + 0 or 1 `index` + 2 file headers + for each hunk: 1 header + `lines().len()` + the count of lines lacking a trailing `\n`, which each had a `\ No newline` marker). A mismatch means diffy skipped something; reject.
6. **Canonical serialization (our code):** sections sorted by `RelPath`; each section is `diff --git a/P b/P\n--- a/P\n+++ b/P\n`; each hunk header is `@@ -a,b +c,d @@\n` with both lengths always explicit and no function context (it is hostile text that would otherwise be hashed and displayed); body lines as ` `/`-`/`+` plus content; a line without a final `\n` is followed by `\n\ No newline at end of file\n`. No `index` line, no timestamps.
7. **Round-trip check:** re-run stages 3 to 5 on the canonical bytes and require identical parsed content. Treat a failure as an internal defect, reported with its own error variant, never silently accepted.

`ExperimentId::of(base, diff)` hashes the stage-6 bytes. The property `parse(serialize(d)) == d` is a proptest (Validation Architecture). The canonical form was checked against `git apply --check` and `patch -p1 --dry-run`: both accept it.

**Signature note:** `Diff::parse(text, &manifest)` is the sketch. A narrower `Diff::parse(text, writable: &BTreeSet<RelPath>, limits: &Limits)` (with `Manifest::writable()` supplying the set) makes property tests far easier and keeps the same invariant. The diff never needs the rest of the manifest.

### Pattern 5: Strict exact-position applier and composition

**Tree:** `BTreeMap<RelPath, Vec<u8>>` in memory (the guest builds it from the base tarball in Phase 5; the report builds it from surface blobs).
**Apply one file:** split the base content into lines that keep their `\n` (the last may lack one). Walk hunks in order, keeping a cursor. For a hunk with `old_len > 0`, its first old line is at index `old_start - 1`; with `old_len == 0`, insertion happens after line `old_start` (so at index `old_start`). Copy unchanged lines up to that index, then require every `Context` and `Delete` line to equal the next base line **byte for byte, including the trailing newline or its absence**, emit `Context` and `Insert` lines, and advance. After the last hunk, copy the rest. Any mismatch, missing file, or out-of-range start returns `ApplyError::{MissingFile{path}, ContextMismatch{path, hunk, line}, OutOfRange{path, hunk}}`, which the runner maps to `Outcome::Rejected(Reason::ApplyFailed { .. })`, a candidate fault (CONTEXT discretion). No create branch, no delete branch, no fuzz, no offset search. Use `.get()` and iterators throughout; no indexing.
**Compose (cumulative diffs, see Open Question 1):** `compose(base: &Tree, parent: &Diff, proposal: &Diff) -> Result<Diff, ComposeError>` = apply `parent` to the base surface, apply `proposal` to that, then for each changed file `diffy::create_patch(base_text, candidate_text)` (default context 3), canonical-serialize, and re-parse through `Diff::parse`. Because `create_patch` output depends only on file contents, two proposals that reach the same candidate tree get the same `ExperimentId`, and the cumulative diff always carries context, which keeps `adopt`'s bytes `git apply`-compatible.

### Pattern 6: `Metric`, `Scorecard`, `Outcome`

- `Metric(f64)`: `Metric::new(x) -> Result<Metric, MetricError>` rejects `!x.is_finite()`, and maps negative zero to `0.0` (`if x.to_bits() == (-0.0_f64).to_bits() { 0.0 }`). `Ord`/`PartialOrd` by `total_cmp`, `Eq`, and `Hash` by `to_bits` if ever needed. Serde via `try_from = "f64"`, `into = "f64"`.
- `MetricName`: ASCII identifier `[a-z][a-z0-9_]{0,63}` (PITFALLS Pitfall 10: no control characters in names).
- `Scorecard`: the raw wire form is an **array** of `{ "name", "value", "held_out" }` entries, not a JSON object, because serde_json silently keeps the last duplicate key in a map (verified). `try_from` rejects duplicates, more than `MAX_METRICS`, and names not declared by the manifest's judge (a check the runner performs with the manifest; the type-level parse checks shape only), then sorts into a `BTreeMap<MetricName, ScoredMetric { value: Metric, held_out: bool }>`. Gates: `[[judge.gate]] metric` names a metric whose pass rule must be fixed now (recommend `value == 1` passes and every other value fails, with a separate `gates` array of bools as the alternative). [ASSUMED: gate encoding is not specified anywhere.]
- `Outcome::{Scored(Scorecard), Rejected(Reason), Crashed, TimedOut}` with `Reason` a closed enum (`GateFailed { gate }`, `ApplyFailed { path, hunk }`, `ScorecardInvalid`, `OutputInvalid`, `OutputTooLarge`, `ChunkProtocol`, `BuildFailed`, `RunFailed`, ...). Every string in a `Reason` is a validated bounded type (`RelPath`, `MetricName`), never free text. Decide before the freeze whether `Crashed`/`TimedOut` carry the stage (build, run, judge); the Phase 2 K7 rule distinguishes console-cap crashes from silent ones only in `minos-vmm` markers, so a stage field is useful but optional.

### Pattern 7: Event set, keys, and the fold

- `Record`: `#[serde(tag = "v")] enum Record { #[serde(rename = "1")] V1(Event) }`; `Event` is externally tagged, `rename_all = "snake_case"`, `deny_unknown_fields`. Verified wire shape: `{"v":"1","run_started":{...}}`.
- `EventKey`: `Event::key(&self, run: &RunId) -> EventKey` is a pure method, so the store can recompute and verify the key on read (a tampered or mismatched line is detectable) and the fold can ignore a duplicate key defensively. Every key hashes the `RunId` first so keys are globally unique when the AWS milestone shares one table.
- **Key table to document in `ledger` module docs** (D-05 to D-10, plus recommendations marked R):

| Event | Key fields after `run` | Note |
|---|---|---|
| `RunStarted` | kind only (one per run) | R: carry `base: TreeId` so the fold can recompute `RunId::of(manifest, base, started_at)` (D-06) |
| `ManifestChanged` | `manifest` | Collapses A to B to A (Pitfall 5); R: add `from_generation` to fields and key |
| `Proposed` | `exp` | Dedups a re-proposal across generations (intended); R: add `diff: BlobId` and `generation: u32` |
| `Judged` | `exp, judge, trial` | D-10 |
| `Selected` | `generation` | D-10 |
| `Spent` | `SpendId = H(run, generation, source)` | D-09 |
| `Steered` | `generation, ordinal` | D-07 |
| `StopRequested` | not specified | R: add `generation: u32`; key `(generation, reason)` so a resumed run can stop again later |
| `ReviewRequested` / `Reviewed` | `request` (`ReviewId`) | D-08; R: make the two kind tags differ so a request and its answer never share a key |
| `ReportAcknowledged` | `exp, report` | D-10 |

- **Evolution rule (document it):** adding an enum variant keeps every old line parseable (D-08's reasoning, verified: unknown variants are rejected by old readers, known ones still parse). Adding a field to an existing V1 variant is only compatible if the field is `Option<T>` (an absent field deserializes to `None`); a required new field, a renamed field, or a changed type needs `"v":"2"`.
- **No `u128` in any ledger or tagged wire type** (verified failure). Usage counters are `u64`; pico-dollar values never persist.
- **Fold:** `fold(records: impl IntoIterator<Item = &Record>, run) -> Result<RunState, FoldError>`. Validation inside the fold: exactly one `RunStarted`, first; every `Judged`, `Selected`, `Spent`, `Steered` refers to a known run and a known experiment (or the baseline); `RunId` recomputes. A violation is ledger corruption, an infrastructure error, never an `Outcome`. `RunState` holds only `BTreeMap`/`BTreeSet`/sorted `Vec`. Order-sensitive content (incumbent per generation, stall counts) derives from `generation` numbers carried in events, not from line order, so permuting `Judged` events within a generation cannot change state.

### Pattern 8: Policy, routing, noise, circuit breaker

- **Reduce trials (D-11):** per experiment and judge version, collect `Scored` trials, per metric sort with `Ord` (`total_cmp`), take the lower median (`sorted.get((n - 1) / 2)`). A non-`Scored` trial makes the experiment's reduced outcome non-scored (recommend: any `Rejected` trial rejects; planner decides).
- **`select_greedy`:** greedy requires exactly one objective (pareto is deferred). Candidates are the generation's experiments with a reduced score that passes all gates. Best = max by (objective value oriented by `Goal`, then smallest `ExperimentId`). Improved iff strictly better than the incumbent **and** clears `min_improvement`, computed by comparison, never division: maximize `cand >= inc + t`, minimize `cand <= inc - t`, with `t = |inc| * pct` (relative) or the absolute value. If `inc + t` overflows to infinity, the comparison is simply false (no improvement), which is deterministic. IEEE-754 `+ - * /` and `sqrt` are correctly rounded and identical on every platform Rust supports without fast-math; avoid `powf`, `ln`, `exp` in any decision path (libm-dependent).
- **`noise_check` (D-12):** spread = max minus min of the baseline's trials for the objective (needs at least 2 trials, else `Warn`); `Refuse` when the absolute threshold at the baseline median is at or below `NOISE_REFUSE_MULTIPLE * spread`, `Warn` up to `NOISE_WARN_MULTIPLE * spread`, else `Ok`. Multiples live in `limits.rs`. Never mutate the manifest threshold.
- **Routing (D-13):** `eligible_ladder` = declared tiers that pass `ProposerSpec::eligible`, ordered by fixed tier rank (sweep, single_shot; planned and agentic deferred). `stall` = concluded generations since the last `Selected` with an improvement (or since `RunStarted`). Tier = `ladder.get(min(stall / stall_step, len - 1))`. An empty ladder is unrepresentable, because `Manifest::try_from` requires at least one tier that passes the manifest-only checks. Eligibility that needs surface sizes (single-shot prompt budget) takes a `SurfaceStats` argument from the shell.
- **Circuit breaker (D-14):** consecutive `Selected` decisions with no valid candidate, counted back from the latest, compared against `max_empty_generations`; the infrastructure-failure streak is Open Question 8.

### Pattern 9: Seeded sweep

```rust
// Source: rand_core 0.10.1 / rand_chacha 0.10.0 sources (trait names verified).
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// Unbiased draw in [0, n) by rejection; n must be non-zero.
fn below(rng: &mut ChaCha8Rng, n: u64) -> Option<u64> {
    if n == 0 {
        return None;
    }
    let threshold = n.wrapping_neg() % n; // 2^64 mod n
    loop {
        let x = rng.next_u64();
        if x >= threshold {
            return Some(x % n);
        }
    }
}
```

Seed: `IdHasher::new(b"minos/sweep/v1\0").digest(run.as_bytes()).u32(generation).finish()`, then `ChaCha8Rng::from_seed(seed)`. Draw params in declaration order, one proposal after another, so a batch is a pure function of (run, generation, params, batch). Restrict v1 params to integer ranges (`min`, `max`, `step`, all integers) and choice lists of literal strings; no float ranges and no log scale, because those need `powf`/`ln` (libm-dependent) or float formatting. Choices are picked with `.get(index)`. Golden-test a fixed seed's first samples as literals.

### Pattern 10: Job contract

`JobRequest { exp: ExperimentId, base: TreeId, diff: Option<BlobId> (None for the baseline), judge: JudgeVersion, trial: u32, budget: JobBudget { build_ms, run_ms, judge_ms } }` and `JobResult { exp, judge, trial, outcome: Outcome, usage: Usage, backend: BackendInfo, logs: BTreeMap<LogKind, BlobId> }`. Both carry `v` and use the Raw/`try_from` pattern. Keep both well under the 4 KiB per-chunk output budget only where they cross a vsock boundary (the runner returns `JobResult` in-process in v1, so the cap matters for the scorecard, not for `JobResult`). [ASSUMED: field lists are CONTEXT discretion; these follow ARCHITECTURE.md Data Flow.]

### Pattern 11: Money

- `Rate::parse(&str) -> Result<Rate, RateError>`: `digits ["." digits]`, at least one integer digit, at most 12 fractional digits (fail closed, never round), no sign, no exponent, no whitespace, no `_`, bounded integer digits; converted with `checked_mul`/`checked_add` into `u128` pico-dollars per unit.
- `Usage` (ledger, `u64` fields): `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` (SPIKE-07 names), `vcpu_ms`, `mem_mb_ms` (local VMs cost $0 in v1, but the dimensions exist for AWS).
- Cost: accumulate `rate_pico * usage` in a `u128` with `checked_mul`; when a rate is "per second" and usage is in milliseconds, keep the `/1000` until the end (one division) so no precision is lost. Convert pico to micro once: `(pico + 500_000) / 1_000_000` (round half up, the CONTEXT rule), with a property test that it never decreases and never overflows below the documented bound.
- `ApproxCost(micro: u128)` owns the only `Display`: `approx. cost: $31.40` (micro to cents with round half up). COST-02's token counts beside it are a Phase 8 formatting concern, but the type lives here.
- `prices.toml`: a `v`, then per-dimension rates as **strings**; a numeric literal is a toml type error with a location (verified), which is the helpful failure COST-01 wants.
- `spend_usd` in the manifest: CLAUDE.md's example writes an integer (`spend_usd = 40`). Recommend accepting a TOML integer of whole dollars (a float is a type error) or switching the example to a string decimal in the same change. [ASSUMED]

### Pattern 12: The renderer (`SafeText`)

Allowlist per `char`:

| Input | Output |
|---|---|
| `\n`, `\t` | kept |
| Any other `char::is_control()` (C0 U+0000 to U+001F, DEL U+007F, C1 U+0080 to U+009F), including ESC and `\r` | dropped |
| Bidi: U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069 | visible `<U+XXXX>` |
| Zero-width and invisible format: U+00AD, U+034F, U+115F, U+1160, U+180E, U+200B to U+200D, U+2060 to U+2064, U+3164, U+FEFF, U+FFA0 | visible `<U+XXXX>` |
| Line and paragraph separators U+2028, U+2029 | visible `<U+XXXX>` |
| Tag characters U+E0000 to U+E007F (used for "ASCII smuggling" into models) and variation selectors U+FE00 to U+FE0F, U+E0100 to U+E01EF | visible `<U+XXXX>` |
| Everything else | kept |

Dropping ESC alone leaves inert residue such as `[31m`, which is visible evidence rather than a hidden effect; that matches the decision ("drops every C0 and C1 control including ESC"). Std Rust has no Unicode general-category API beyond `is_control`, so the visible set is an explicit, golden-tested range table. Golden corpus: OSC 52, OSC 8, CSI cursor moves, `\r` overwrite, Trojan Source samples, zero-width joiners, tag-character strings. [CITED: CVE-2021-42574 Trojan Source; PITFALLS.md Pitfall 14] The tag-character and variation-selector rows extend the decision's list. [ASSUMED: they are additions within the discretion "treatment of other non-printable code points"]

### Pattern 13: `build_report` and `ReportHash`

- Inputs: the records (or `RunState`), the target `ExperimentId`, and a `BTreeMap<BlobId, Vec<u8>>` of fetched blobs (lineage diffs, base surface files). Never judge data (D-15).
- Output: a `Report` value plus its canonical bytes: the exact UTF-8 text the CLI shows, with every hostile string passed through `SafeText`, LF only, fixed section order. `ReportHash = IdHasher::new(b"minos/report/v1\0").bytes(&canonical).finish()`. Hashing the rendered bytes means the user acknowledges exactly what they saw (CLAUDE.md Report and adopt; ARCHITECTURE.md Key data flow 4), and it removes any dependency on `serde_json::Value` (PITFALLS Pitfall 17).
- Metric display in the report: a fixed formatter under our control (Rust `{}` for magnitudes in [1e-4, 1e15), `{:e}` outside, both shortest-round-trip and std-defined), golden-tested. Never route report numbers through serde_json.
- Sections: summary rows per D-17; cumulative diff (Pattern 5 composition of the lineage, canonical bytes); lineage (baseline to winner via `Proposed.parent`); provenance (isolation level and layers per D-18, judge version, image digests, model ID, prompt hash); flags.
- Flags (v1 five, each a pure function with its threshold in `limits.rs`): anomalous jump (D-16); held-out divergence (rows from D-17); literals matching surface files outside the hunk's own context (D-15); new dependencies (diff touches a dependency manifest or lockfile by file name, a fixed list in `limits.rs`); encoded blobs (an added line holds a run of at least `ENCODED_RUN_MIN` base64 or hex characters). Recommend also flagging any added line that contains a bidi or zero-width code point (PITFALLS Pitfall 14), reported under encoded or hidden content.
- Weak isolation (D-18): the weakest level across the lineage's `Judged.backend`, always rendered even though v1 is always hardware.

### Anti-Patterns to Avoid

- **Hashing raw model text:** `ExperimentId` hashes canonical bytes only (Pitfall 15).
- **`diffy::apply`, fuzzy or offset-searching application:** breaks judge integrity and reproducibility.
- **`serde_json::Value` in any hashed or persisted path,** and any `BTreeMap<String, f64>` parsed from hostile JSON (duplicate keys silently overwrite).
- **Float math beyond `+ - * /` and comparisons in decisions.**
- **`#[serde(default)]` on validated fields,** `flatten` on raw structs, `Deserialize` derived directly on a validated type.
- **Assuming insta catches whitespace drift:** pair every wire snapshot with a byte-hash literal.
- **`RngCore` imports:** deprecated in rand_core 0.10, fails `-D warnings`.
- **Reading fixtures with `std::fs` in core tests:** banned by core's `clippy.toml`; use `include_str!`/`include_bytes!`.

## Decide before the freeze (CORE-08 gaps) (RESOLVED)

**Resolved 2026-10-14:** the user approved all seven items before the freeze (03-CONTEXT.md D-09 and D-10 amendments, user decision via orchestrator). Plan 03-07 freezes them; plan 03-05 implements item 6.

The locked field lists in D-05 to D-10 are incomplete in ways the fold cannot work around. Each item is small, but each changes the golden wire format, so all of them must be settled in the plan that freezes the set, before any golden file. Items 1 to 4 follow logically from decisions already made, so the planner can treat them as corollaries. Items 5 to 7 change a locked field list and should go to the user if the planner is unsure.

1. **`RunStarted` needs `base: TreeId`.** D-06 requires the fold to recompute and verify `RunId::of(&ManifestId, &TreeId, started_at)`; `baseline: ExperimentId` is a hash and cannot be inverted to a `TreeId`. **RESOLVED:** adopted (D-10 amendment); 03-07.
2. **`Proposed` needs `diff: BlobId`.** `ExperimentId` is a domain-separated hash of (base, diff bytes); the diff's blob ID is the plain digest of the same bytes; neither can be derived from the other. Resume (reuse recorded proposals), `JobRequest.diff`, and `build_report` (lineage diffs) all need it. **RESOLVED:** adopted (D-10 amendment); 03-07.
3. **`Proposed` needs `generation: u32`.** Selection, routing stall counts, and the circuit breaker all group experiments by generation. Inferring it from ledger position works only for a strictly sequential writer and is order-sensitive. **RESOLVED:** adopted (D-10 amendment); 03-07.
4. **`StopRequested` has no key.** Recommend `StopRequested { generation, reason }` keyed by (generation, reason): a repeated `minos stop` is a no-op, and a run resumed after a raised cap can stop again at a later generation. **RESOLVED:** adopted, keyed (generation, reason) (D-10 amendment); 03-07.
5. **`ManifestChanged` keyed by manifest alone collapses A to B to A.** Recommend adding `from_generation: u32` to the event and the key. v1 may never emit it, but the format freezes now. **RESOLVED:** user-approved 2026-10-14; event and key both carry `from_generation` (D-10 amendment); 03-07.
6. **`JudgeVersion` is a SHA-256 type (CORE-01, JUDGE-01) while CLAUDE.md's manifest shows `[judge] version = 1`.** See Open Question 3. **RESOLVED:** user-approved 2026-10-14; the hash of the judge-data `TreeId` plus the canonical judge spec, with the manifest integer as a local label (D-10 amendment); 03-05.
7. **`Selected.decision` should carry the dropped-proposal count** (PITFALLS Pitfall 34: invalid-diff rate) and an explicit kind (`Improved { winner }`, `Kept`, `NoValidCandidates`), so the breaker and `minos status` read it from the ledger. **RESOLVED:** user-approved 2026-10-14; `Decision { kind, proposed, dropped }` (D-10 amendment); 03-07.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unified-diff hunk parsing | A hunk-body parser with count checks, quoting, escapes | `diffy::patch_set::PatchSet` in gitdiff mode | Counts, order, escapes, `\ No newline`, and git header detection are already right; wrap it, do not replace it |
| Diff generation for the cumulative diff | A Myers implementation | `diffy::create_patch` / `DiffOptions` | Pinned and deterministic; the output only feeds our canonical serializer |
| SHA-256 | Anything | `sha2` | Never hand-roll crypto |
| Hex | A formatter loop | `base16ct::lower` | Constant-time, strict lowercase decode |
| PRNG stream | A custom generator | `ChaCha8Rng` | Documented portable and deterministic |
| TOML and JSON syntax | Parsers | `toml`, `serde_json` (with `float_roundtrip`) | Locations, duplicates, and type errors come free |
| Property testing and snapshots | Ad hoc loops | `proptest`, `insta` | Shrinking, regressions files, reviewable snapshots |

**Hand-roll on purpose (decided):** the canonical diff serializer, the strict applier, the line-kind grammar, `RelPath`, the decimal `Rate` parser, the range mapping for the sweep, the renderer's code-point table, and the report's metric formatter. Each is small, sits on the trust boundary, and must be value-stable under our control.

**Key insight:** in this phase, hand-rolling is right exactly where a library's leniency or version drift would change an ID, a hash, or a security decision, and wrong everywhere else.

## Common Pitfalls

### Pitfall 1: Metrics that change bits on their way through the ledger
**What goes wrong:** about 10% of typical-range `f64` values come back from serde_json 1.0.151 with different bits (measured 206,407 of 2,000,000).
**Why it happens:** serde_json's default float parser is fast, not correctly rounded; correct rounding is behind `float_roundtrip`.
**How to avoid:** enable `float_roundtrip` on the workspace pin before any golden file; add a proptest that `Metric` JSON round-trips bit-exactly (it fails without the feature, which proves the test has teeth).
**Warning signs:** a golden ledger file that re-serializes differently; resumed and uninterrupted runs that select differently.

### Pitfall 2: A diff validator that trusts the parser to see everything
**What goes wrong:** prose, extra hunk lines, an inconsistent `diff --git` path, a zero-hunk section, or a duplicate section passes diffy and is silently ignored.
**How to avoid:** stages 2, 4, and 5 of Pattern 4; test each case listed in Verified API Facts as a rejection.
**Warning signs:** two different texts with the same canonical bytes where one contains material the other lacks.

### Pitfall 3: The canonical form does not re-parse in the same mode
**What goes wrong:** a canonical form of plain `---`/`+++` headers fails gitdiff parsing with "no valid patches found", so `parse(serialize(d))` cannot hold.
**How to avoid:** include `diff --git a/P b/P` in the canonical form (accepted by `git apply` and `patch -p1`).

### Pitfall 4: insta snapshot passes while the bytes changed
**What goes wrong:** a lost trailing newline or an added `\r` in canonical diff, report, or record bytes passes `assert_snapshot!`.
**How to avoid:** every wire golden gets a second assertion: `assert_eq!(hex(sha256(bytes)), "<64-hex literal>")` computed independently (Code Examples).

### Pitfall 5: Event keys that merge distinct events
**What goes wrong:** `ManifestChanged` A to B to A; two `StopRequested` in one run; a `ReviewRequested` and its `Reviewed` with the same key.
**How to avoid:** "Decide before the freeze"; a property test that distinct events in a generated valid ledger have distinct keys.

### Pitfall 6: Policy that is order-sensitive in a hidden way
**What goes wrong:** tie-breaking by first-seen, median of trials in arrival order, or floating sums accumulated in ledger order.
**How to avoid:** sort before every reduction; break ties on `ExperimentId`; proptest with `prop_shuffle` over experiments and trials together (D-11).

### Pitfall 7: Relative thresholds at zero or negative baselines
**What goes wrong:** `(cand - base) / base` divides by zero or flips sign (PITFALLS Pitfall 10).
**How to avoid:** the multiply-and-compare form in Pattern 8; proptest over baselines of `0.0`, negative values, and near-`f64::MAX`.

### Pitfall 8: Deprecated or renamed APIs in fast-moving crates
**What goes wrong:** `RngCore` (deprecated), `GenericArray` (gone in sha2 0.11), `rand::gen_range` (not value-stable).
**How to avoid:** use only the names in Verified API Facts.

### Pitfall 9: Lint traps specific to this phase
**What goes wrong:** `&text[a..b]` slicing, `bytes[0]`, array indexing in parsers (forbid); `missing_errors_doc` on every fallible public function; `doc_markdown` on identifiers in prose; `float_cmp` on `Metric` tests; `wildcard_imports` from `use super::*` (allowed in test modules, but existing code imports explicitly).
**How to avoid:** `strip_prefix`, `split_once`, `split_at_checked`, `.get()`, `chunks_exact`, `to_be_bytes`; compare floats with `to_bits()` or `total_cmp` in tests; follow `crates/minos-guest/src/job.rs` style.

### Pitfall 10: Hostile text inside error values
**What goes wrong:** a `DiffError` that embeds the offending raw line, later printed without the renderer.
**How to avoid:** errors carry line numbers, lengths, and validated types; if any carries hostile text, its `Display` must route through `SafeText`, and that is golden-tested.

## Code Examples

### Golden hash vector computed outside the code under test

```bash
# Source: verified in this session with /usr/bin/shasum. Baseline ExperimentId for a stand-in TreeId:
TREE=$(printf '' | shasum -a 256 | cut -d' ' -f1)          # e3b0c442...b855
{ printf 'minos/experiment/v1\0'; printf '%s' "$TREE" | xxd -r -p; printf '\0\0\0\0\0\0\0\0'; } | shasum -a 256
# 5b161f655168a2b16645e03b77d61b0bfba4935d1be19c6293ec41a5998b6f86   (60 input bytes: 20 + 32 + 8)
```

The test then asserts `ExperimentId::baseline(&tree).to_hex() == "5b161f65..."`. Record each vector's shell recipe in a comment beside the literal so a reviewer can recompute it. (D-04: the baseline uses the same domain prefix with a zero-length diff.)

### Test shape that satisfies the forbid lints

```rust
// Source: crates/minos-guest/src/pattern.rs and job.rs test modules (repo convention).
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn rejects_rename_header() -> TestResult {
    let text = include_str!("fixtures/rename.diff");
    let writable = writable_set(&["src/parse.rs"])?;
    let err = Diff::parse(text, &writable, &Limits::default()).err().ok_or("rename accepted")?;
    assert!(matches!(err, DiffError::ForbiddenHeader { line: 3, .. }));
    Ok(())
}
```

### Property test with permutation (verified clean under the forbid lints)

```rust
// Source: proptest 1.11 API; probe crate compiled under the workspace lint table.
use proptest::prelude::*;

proptest! {
    #[test]
    fn select_greedy_is_permutation_invariant(results in arb_generation_results()
        .prop_flat_map(|v| Just(v.clone()).prop_shuffle().prop_map(move |s| (v.clone(), s))))
    {
        let (original, shuffled) = results;
        prop_assert_eq!(select_greedy(&original, &spec()), select_greedy(&shuffled, &spec()));
    }
}
```

### insta snapshot plus byte pin

```rust
// Source: insta 1.49 macros (probe); normalization behaviour from insta source.
#[test]
fn record_v1_golden() -> TestResult {
    let line = serde_json::to_string(&kitchen_sink_record()?)?;
    insta::assert_snapshot!("record_v1_run_started", line);
    assert_eq!(sha256_hex(line.as_bytes()), "<64-hex literal, recomputed by hand>");
    Ok(())
}
```

### `.config/insta.yaml` (workspace root)

```yaml
# Plain `cargo test --workspace` (the gate) never writes or accepts snapshots.
# Accept new snapshots deliberately with `INSTA_UPDATE=always cargo test -p minos-core` or `cargo insta review`.
behavior:
  update: no
```

Verified: with this file and no environment variable, a missing snapshot fails the test and writes nothing; `INSTA_UPDATE=always` still overrides it for deliberate updates.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rand_core::RngCore` | `rand_core::Rng` (`RngCore` deprecated stub) | rand_core 0.10.0 | Import `Rng` |
| `sha2` returning `GenericArray` | `hybrid-array`; `finalize().into()` to `[u8; 32]` | sha2 0.11 | Verified conversion |
| serde_json float formatting via `ryu` | via `zmij` | by serde_json 1.0.151 | Another reason to keep report numbers out of serde_json |
| `insta` env-only update control | `.config/insta.yaml` `behavior.update` | insta 1.x | Gate stays a plain cargo command |

**Deprecated/outdated in our own planning docs:**
- The orchestrator note's `RngCore::next_u64`: use `Rng::next_u64`.
- STACK.md says a toml `try_from` error "has no location": on a nested table it reports that table's header line; it is still not field-level, so keep wrapping with the dotted field path.
- CLAUDE.md `ExperimentId::of` sketch uses `diff.len() as u64`: correct and clippy-clean as written.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Experiment diffs are cumulative from the run's base tree, composed via apply plus `create_patch` | Pattern 5, Open Question 1 | If diffs are meant to be parent-relative, the guest needs the diff chain and `JobRequest` changes; decide before the freeze |
| A2 | ASCII case-fold collision check in manifest disjointness | Pattern 3 | Rejects a few more manifests than D-01 requires; low risk |
| A3 | Gate pass rule `value == 1` | Pattern 6 | The judge contract (Phase 5/7 sample project) must match it |
| A4 | Sweep params limited to integer ranges and string choices | Pattern 9, Open Question 6 | A user wanting float or log ranges must list choices |
| A5 | `spend_usd` stays a whole-dollar TOML integer | Pattern 11 | A string decimal is equally valid; needs a CLAUDE.md edit either way if changed |
| A6 | Job contract field lists | Pattern 10 | Discretion; golden-filed, so changes later need a `v` bump |
| A7 | Renderer also shows tag characters and variation selectors visibly | Pattern 12 | Additive; only more visible output |
| A8 | `LogExcerpt` and its truncation live in core beside `SafeText` | Architectural map | Phase 4 may prefer it in sandbox; harmless either way |

A1 was confirmed by the user on 2026-10-14 (03-CONTEXT.md D-04 amendment). A3 and A4 are recorded in CLAUDE.md's Projects section by plan 03-07.

## Open Questions (RESOLVED)

Every question below was resolved before execution. Questions 1, 3, and 7 and the freeze items were decided by the user on 2026-10-14 (03-CONTEXT.md amendments under D-04, D-09, and D-10); the rest were decided by the planner within the CONTEXT discretion and are recorded in the plan named.

1. **What is an experiment's diff relative to?**
   - What we know: an experiment is "a base tarball hash plus a unified diff, applied inside the guest"; the guest receives one base tarball and one diff; `parent` is lineage; the report shows "the cumulative diff from baseline to winner".
   - What's unclear: whether a child's diff is cumulative from the run's base or relative to its parent. No document says.
   - Recommendation: cumulative from the run's base (Pattern 5 `compose`). The guest then applies exactly one diff to the base, `ExperimentId` depends only on the candidate's content change from base, identical outcomes dedup for free, and `adopt`'s bytes always carry context. Confirm with the user before the event freeze, because `Proposed.diff` and `JobRequest.diff` mean different things under the two answers.
   - **RESOLVED (user decision 2026-10-14, 03-CONTEXT.md D-04 amendment):** cumulative from the run's base tree. Implemented by 03-02 (`ExperimentId::of` over base plus canonical diff, CLAUDE.md Diff paragraph), 03-06 (`compose`, `diff_trees`), 03-07 (`Proposed.diff`), and 03-09 (`JobRequest.diff`).
2. **Which diff dialect will Phase 6 ask the model for?** gitdiff mode requires `diff --git` lines. Recommend the prompt's stable prefix shows the exact canonical form (PITFALLS Pitfall 34). Accepting plain unified diffs as well would mean a second parse mode (unidiff), which is only safe behind the stage-2 grammar; not recommended for v1.
   - **RESOLVED (planner, CONTEXT discretion on the canonical form):** gitdiff only. 03-02's stage-2 grammar rejects a plain unified diff with no `diff --git` line, and the canonical form is the one dialect; showing it in the prompt's stable prefix is Phase 6 work.
3. **What is `JudgeVersion`?** CORE-01 and JUDGE-01 make it a hash; CLAUDE.md's manifest has `[judge] version = 1` and `minos rejudge --judge <v>`. Recommend `JudgeVersion = H("minos/judge/v1", judge data TreeId, canonical judge spec (command argv, gates, objectives))`, with the manifest integer dropped or kept as a label only, and CLAUDE.md updated in the same change. Needs the user's confirmation.
   - **RESOLVED (user decision 2026-10-14, 03-CONTEXT.md D-10 amendment):** `JudgeVersion` hashes the judge-data `TreeId` plus the canonical judge spec (argv, gates, objectives with goal and held-out pairing); the `[judge] version` integer stays as a local label and is excluded from the hash. Implemented by 03-05 Task 2; CLAUDE.md updated by 03-07 Task 2.
4. **What are "judge paths" in the manifest?** CORE-03 and D-01 require `writable` to be disjoint from them, but the schema in CLAUDE.md has only `[judge] command` and `[source] exclude = ["judge/", ...]` (directory prefixes). Recommend checking `writable` and `readable` against `[source] exclude` prefixes and against the judge command's program path when it is relative, and documenting that judge data never lives in the project. Planner decides the exact field.
   - **RESOLVED (planner, D-01 discretion; widened after the 2026-10-14 plan check):** judge paths are the `[source] exclude` entries plus every judge argv entry that names a project-relative path: the program and every later argument (or the value of a `--name=value` option) that parses as a `RelPath` after stripping one leading `./`. A path-like entry that is not a valid `RelPath` is rejected, never ignored. Implemented by 03-05 Task 2.
5. **Where are the image digests the manifest must check?** The example manifest names a Dockerfile, not an image. Recommend a validated `ImageRef` type (`name@sha256:<64 hex>`) for any image field and for `minos.lock`, plus a pure `check_dockerfile_from_pins(&str)` in core that Phase 5's `env build` calls, so "images are digests" is enforced by core either way.
   - **RESOLVED (user decision 2026-10-14):** `ImageRef` plus `image::base_images` on the Dockerfile's `FROM` lines are the enforcement point, and `Manifest::try_from` stays pure; no `Manifest::check_dockerfile` is added. ROADMAP Phase 3 success criterion 1 and REQUIREMENTS CORE-03 were reworded to match. Implemented by 03-05 Task 1 (the recommended `check_dockerfile_from_pins` is named `base_images`).
6. **Sweep param schema.** Not defined anywhere, yet the manifest freezes now. Recommend `[[param]] name, file (must be writable), anchor (a line prefix that must match exactly one line of the base file), and either { min, max, step } integers or choices = ["...", ...]`. The sweep replaces the remainder of the anchored line and emits a canonical diff via `compose`. Eligibility: at least one param, each `file` writable; the anchor match is checked against base files at plan time.
   - **RESOLVED (planner, assumption A4):** adopted as recommended, integer ranges and string choices only. Implemented by 03-05 (types and checks) and 03-08 (sampler and render); CLAUDE.md's Projects section records the limit (03-07 Task 2).
7. **Does `Crashed`/`TimedOut` carry a stage?** Optional; decide before the freeze.
   - **RESOLVED (user decision 2026-10-14, 03-CONTEXT.md D-10 amendment):** yes, `Crashed { stage }` and `TimedOut { stage }` with `Stage::{Build, Run, Judge}`. Implemented by 03-03; frozen by 03-07.
8. **Where does the infrastructure-failure streak for the breaker come from?** The frozen event set has no infrastructure-failure event (infra failures are retried, not `Outcome`s). Recommend `circuit_breaker(state, manifest, infra_streak: u32)` with the streak supplied by the loop (in memory; a restart resets it, which is acceptable because a restart is a human action), and the trip recorded as `StopRequested { reason: InfraFailures }`. The alternative, a `Decision` variant for an all-infra generation, puts it in the ledger.
   - **RESOLVED (planner):** adopted as recommended, `circuit_breaker(state, budget, infra_streak)` with the streak supplied by the loop in memory and reset on restart. Implemented by 03-11; CLAUDE.md records it (03-07 Task 2).
9. **Ship `generation::{plan, admit, conclude}` and `spend_ok` now or in Phase 7?** CONTEXT leaves it to the planner. Recommend shipping `conclude` (it is `select_greedy` over folded state) and `spend_ok` now, since they are short and fully testable here, and leaving `plan`/`admit` to Phase 7, where their inputs (surface stats, proposal texts) take final shape.
   - **RESOLVED (planner, CONTEXT discretion):** `conclude` and `spend_ok` ship in 03-11; `plan` and `admit` wait for Phase 7.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc / cargo | everything | yes | 1.97.1 (pinned) | none needed |
| cargo-deny | gate | yes | 0.20.2 | none needed |
| cargo-insta | reviewing snapshots (optional) | yes | 1.48.0 (library is 1.49.0) | `INSTA_UPDATE=always cargo test -p minos-core`; the gate does not need the CLI |
| cargo-nextest | optional | yes | 0.9.85 | `cargo test` |
| shasum, xxd | computing golden vectors by hand | yes | system | any SHA-256 tool |
| git, patch | checking canonical-form compatibility by hand (Phase 8 owns the golden test) | yes | 2.50.1, 2.0-12u11-Apple | none needed in Phase 3 |
| aarch64-unknown-linux-musl target | core must stay buildable for the guest in Phase 5 | yes | installed | none needed |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none. `cargo-insta` trails the library by one minor version, which matters only for interactive review.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Built-in `cargo test` + `proptest` 1.11.0 + `insta` 1.49.0 |
| Config file | `.config/insta.yaml` (Wave 0, new); `crates/minos-core/clippy.toml` (exists) |
| Quick run command | `cargo test -p minos-core` |
| Full suite command | the five gate commands: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps`, then the C-01 and C-02 checks |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CORE-01 | Each ID type's hash equals a hand-computed hex literal; composite IDs differ when any field changes; baseline vector (D-04) | unit (golden vectors) + proptest | `cargo test -p minos-core --test golden_ids` | ❌ Wave 0 |
| CORE-02 | `RelPath` rejects `..`, `.`, absolute, empty, `//`, trailing `/`, `\`, NUL, non-ASCII, over-length; accepts the grammar; round-trips through serde | proptest + unit | `cargo test -p minos-core --test prop_path` | ❌ Wave 0 |
| CORE-03 | Each manifest rule rejects its violation (unknown field, overlap, case-fold collision, image tag, budget out of range, policy/objective mismatch, no eligible tier, `stall_step`/breaker bounds per D-14) | unit with TOML fixtures | `cargo test -p minos-core --test manifest` | ❌ Wave 0 |
| CORE-04 | Every Verified-API-Facts leniency is a rejection; create/delete/no-op/empty rejected (D-02, D-03); `parse(serialize(d)) == d`; canonical bytes stable | unit + proptest (generate base and edits, `create_patch`, serialize, parse) | `cargo test -p minos-core --test prop_diff` | ❌ Wave 0 |
| CORE-05 | `apply(base, d) == modified` for generated pairs; any single-byte context change fails with `ContextMismatch`; no-newline cases; `old_len == 0` insert positions | proptest + unit | `cargo test -p minos-core --test prop_apply` | ❌ Wave 0 |
| CORE-06 | NaN and infinities rejected; `-0.0` becomes `0.0`; `Ord` agrees with `total_cmp`; JSON round-trip bit-exact | proptest | `cargo test -p minos-core --test prop_metric` | ❌ Wave 0 |
| CORE-07 | Every `Outcome` variant serializes and parses; no error type carries an outcome | unit + golden | `cargo test -p minos-core --test golden_wire` | ❌ Wave 0 |
| CORE-08 | Every event's key is documented and distinct across a generated ledger; ledger module docs list the table | proptest + `cargo doc` | `cargo test -p minos-core --test prop_keys` | ❌ Wave 0 |
| CORE-09 | Golden `Record::V1` line per event variant (insta + byte hash); fold is identical under permutation of `Judged` within a generation and under duplicate-key replay; corrupt ledgers return `FoldError` | golden + proptest | `cargo test -p minos-core --test golden_wire --test prop_fold` | ❌ Wave 0 |
| CORE-10 | `select_greedy` permutation-invariant over experiments and trials; respects `min_improvement` (relative and absolute, zero and negative baselines); tie on `ExperimentId` | proptest | `cargo test -p minos-core --test prop_policy` | ❌ Wave 0 |
| CORE-11 | Route climbs one tier per `stall_step`, clamps, resets after improvement (D-13); `eligible` returns a reason for each ineligible tier | unit + proptest | `cargo test -p minos-core --test route` | ❌ Wave 0 |
| CORE-12 | Fixed seed gives literal samples; `below(n)` stays in range; same inputs give identical batches | golden + proptest | `cargo test -p minos-core --test sweep` | ❌ Wave 0 |
| CORE-13 | `JobRequest`/`JobResult` golden JSON; unknown fields rejected | golden + unit | `cargo test -p minos-core --test golden_wire` | ❌ Wave 0 |
| CORE-14 | `Rate::parse` rejects negative, exponent, 13 fractional digits, overflow, `_`, whitespace; exact known products (AWS rates `0.0000276944`, `0.0000036667`, `0.0000166667`); round-half-up micro conversion | unit + proptest | `cargo test -p minos-core --test money` | ❌ Wave 0 |
| CORE-15 | Escape corpus renders inert; bidi and zero-width shown as `<U+XXXX>`; `\n`/`\t` kept | golden | `cargo test -p minos-core --test render` | ❌ Wave 0 |
| CORE-16 | Same inputs give identical bytes and hash (including shuffled ledger order); each of the five flags fires on its fixture and stays quiet on a clean one; weak-isolation path golden (D-18) | golden + proptest | `cargo test -p minos-core --test report` | ❌ Wave 0 |
| CORE-17 | Breaker stops after exactly `max_empty_generations` empty generations and `max_infra_failures` infra failures, not one before | unit + proptest | `cargo test -p minos-core --test breaker` | ❌ Wave 0 |
| COST-01 | `Spent` has no dollar field (golden); `prices.toml` numeric rate is a parse error with a location | golden + unit | `cargo test -p minos-core --test money` | ❌ Wave 0 |
| GATE-06 | `cargo clean` after verification | manual step in the last plan | `cargo clean` | n/a |

Deliberate-violation evidence (the Phase 1 and 2 standard): for at least `Diff::parse` (a rename), `Manifest::try_from` (an overlap), `Metric::new` (NaN), `Rate::parse` (13 digits), and the fold (a duplicate `RunStarted`), the verification records the actual failing output.

### Sampling Rate
- **Per task commit:** `cargo test -p minos-core` (seconds) plus `cargo clippy -p minos-core --all-targets -- -D warnings`
- **Per wave merge:** the full five-command gate plus C-01 and C-02
- **Phase gate:** full suite green, then `cargo clean` (GATE-06), before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `Cargo.toml`: `serde_json` gains `features = ["float_roundtrip"]` (before any metric round-trip test or golden file)
- [ ] `.config/insta.yaml` with `behavior: update: no`
- [ ] `crates/minos-core/tests/` with `fixtures/` (TOML manifests, diff texts, escape corpus) loaded by `include_str!`
- [ ] A shared test helper module (`tests/common/mod.rs`) with `TestResult`, `sha256_hex`, and builders that go through public constructors only
- [ ] `crates/minos-core/proptest-regressions/` is created by proptest on first failure; commit it when it appears
- [ ] No framework install needed

## Security Domain

`security_enforcement` is on (ASVS level 1, block on high).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Core has no identities |
| V3 Session Management | no | none |
| V4 Access Control | yes, as data | `writable` exact-path allowlist enforced at `Diff::parse`; judge data is never an input to core functions |
| V5 Input Validation | yes (primary) | Raw/`try_from` with `deny_unknown_fields`; `RelPath` grammar; seven-stage diff pipeline; `Metric` finiteness; scorecard array with duplicate rejection; exact decimal parser; caps before parsing |
| V6 Cryptography | yes | `sha2` only, never hand-rolled; domain separation and length prefixes on composite IDs |
| V7 Error Handling and Logging | yes | `thiserror` enums separating candidate fault from infrastructure fault; no hostile text in errors without `SafeText` |
| V8 Data Protection | partial | Usage never dollars; held-out metrics marked so Phase 6 can keep them out of prompts |
| V12 Files and Resources | yes, in memory | Size, line, hunk, path-depth, metric-count caps in `limits.rs`; no filesystem access in core |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Parser differential between core's diff validation and the guest's or `git apply`'s interpretation | Tampering | One canonical form re-serialized by core; guest and adopt consume only canonical bytes; line accounting rejects skipped text |
| Edits to judge files or new files where the judge expects one | Tampering, Elevation | Exact writable allowlist; create, delete, rename, copy, mode, symlink, binary all rejected; applier has no create or delete branch |
| NaN, infinity, or overflow in candidate-influenced metrics | Tampering | `Metric` rejects non-finite; comparison-form thresholds; no division |
| Hash ambiguity from concatenated fields | Spoofing | Length-prefixed fields, NUL-terminated domain strings, per-kind event tags; golden vectors |
| Terminal escape, bidi, zero-width, tag-character smuggling in report or logs | Spoofing, Tampering | `SafeText` allowlist renderer; report hash covers the sanitized bytes shown |
| Duplicate JSON keys overwriting metrics | Tampering | Scorecard as an array with uniqueness check |
| Float money and rounding drift | Tampering, Repudiation | Pico-dollar `u128` rates from strings, checked arithmetic, one documented rounding rule |
| Resource exhaustion from huge diffs or scorecards | Denial of service | Caps checked before parsing; `create_patch` only on capped surface files |
| Non-deterministic decisions breaking resume and review | Repudiation | `BTreeMap` only, sorted reductions, seeded ChaCha8, correctly rounded IEEE ops only |

## Sources

### Primary (HIGH confidence; read in the pinned source and/or executed offline this session)
- diffy 0.5.2 source: `src/patch_set/{mod.rs, parse.rs}`, `src/patch/{mod.rs, parse.rs}`, `src/utils.rs`, `src/apply.rs`, `src/diff/mod.rs`, `src/lib.rs`, `Cargo.toml` features; probe of 20 diff shapes
- sha2 0.11.0, base16ct 1.0.0 (`src/lower.rs`, `src/error.rs`): probe of `finalize().into()` and hex encode
- rand_core 0.10.1 `src/lib.rs` (trait `Rng`, deprecated `RngCore`), rand_chacha 0.10.0 `src/lib.rs`: probe of seeding and `next_u64`
- serde_json 1.0.151 `Cargo.toml` (features, `zmij`), `src/de.rs`, `src/lib.rs`: 4,000,000-value round-trip probe with and without `float_roundtrip`
- serde 1.0.229 tagged-enum behaviour: probe (`v` tag, unknown field, unknown variant, `u128`)
- toml 1.1.8: probe of unknown field, duplicate table, type mismatch, `try_from` error location
- insta 1.49.0 `src/env.rs` (update resolution, config file names), `src/runtime.rs` (`finalize`), `src/snapshot.rs` (`normalize`): probe of `.config/insta.yaml`
- proptest 1.11.0 and insta macros under the workspace lint table: probe clippy run with `-D warnings`, exit 0
- `git apply --check` (git 2.50.1) and `patch -p1 --dry-run` (patch 2.0-12u11-Apple) on candidate canonical forms
- Repository: `CLAUDE.md`, `.claude/CLAUDE.md`, `Cargo.toml`, `deny.toml`, `crates/minos-core/{Cargo.toml, clippy.toml}`, `crates/minos-guest/src/{job.rs, pattern.rs}`, `.planning/{REQUIREMENTS.md, ROADMAP.md, STATE.md}`, `.planning/research/{ARCHITECTURE.md, PITFALLS.md, FEATURES.md}`, `rules/{api-non-exhaustive.md, api-must-use.md, doc-errors-section.md, test-proptest-properties.md}`
- GSD package-legitimacy seam: all eleven crates `OK`

### Secondary (MEDIUM confidence)
- CVE-2021-42574 (Trojan Source) and the escape-sequence classes in PITFALLS.md Pitfall 14, for the renderer table

### Tertiary (LOW confidence)
- None relied on. The design recommendations for the open questions are marked `[ASSUMED]` or listed in Open Questions.

Note on the confidence seam: `gsd-tools query classify-confidence --provider codebase` returns `LOW` for any provider that is not a docs or web service. The facts above are rated HIGH because each was executed against the exact pinned crate versions in this repository's `Cargo.lock`, which is stronger evidence than documentation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. No new crates; one feature flag, justified by a measured failure.
- Architecture: MEDIUM-HIGH. Patterns follow the locked decisions and the repo's existing shapes; the cumulative-diff model, `JudgeVersion`, judge paths, image field, and sweep schema are open questions.
- Pitfalls: HIGH. Each was reproduced in a probe or read in source.

**Research date:** 2026-10-10
**Valid until:** 2026-11-09 for the crate facts (all pins are exact, so they hold until a pin changes); re-verify the diffy and serde_json rows on any bump.
