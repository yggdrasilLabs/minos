---
phase: 03-core-types-and-pure-logic
verified: 2026-10-14T15:45:00Z
status: passed
score: 4/4 roadmap success criteria verified (SC5, `cargo clean`, is the orchestrator's post-verification step); 18/18 requirement IDs satisfied
behavior_unverified: 0
overrides_applied: 0
re_verification: false
---

# Phase 3: Core Types and Pure Logic Verification Report

**Phase Goal:** Every value the loop decides on exists only as a validated, pure, deterministic type in `minos-core`. An experiment that edits the judge, a NaN metric, or a float dollar amount cannot be represented, and the ledger wire format is frozen and guarded from its first golden file.
**Verified:** 2026-10-14
**Status:** passed
**Re-verification:** No, initial verification

I started from the hypothesis that the phase had missed its goal. I read the code in `crates/minos-core` and ran the gate commands myself. I did not rely on the SUMMARY files. No gap survived.

## Goal Achievement

### Observable Truths (ROADMAP success criteria, plus the goal sentence)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Invalid input cannot become a value: `RelPath`, `Manifest::try_from`, `ImageRef` and `base_images`, `Diff::parse`, `Metric`, and `Outcome` all reject what they should | VERIFIED | `Metric::new` (`metric.rs:64`) returns `NotFinite` for NaN and infinities and maps `-0.0` bits to `0.0`. `Ord` uses `total_cmp`, and `Hash` and `Eq` agree with it. `Metric` deserializes through `#[serde(try_from = "f64")]`, so a NaN in a ledger line fails at parse. `Scorecard` uses `try_from = "RawScorecard"` and rejects duplicate names. `RawManifest` and all 13 raw tables carry `deny_unknown_fields` (14 occurrences in `manifest.rs`), and `Manifest::try_from` is the only constructor. `Diff::parse` (`diff/mod.rs:438`) has a seven-stage pipeline. It rejects rename, copy, mode, similarity, binary, symlink (120000), and gitlink (160000) headers, rejects create and delete sections and CR/NUL bytes, enforces the byte, line, file, and hunk caps, and requires each path to equal a `writable` entry. `Diff` has private fields and no `Deserialize`, so it has one entry point. `Experiment` can only be built from a parsed `Diff`, or as the baseline with no diff. The goal's "experiment that edits the judge cannot be represented" holds: `Manifest::try_from` keeps `writable` disjoint from judge argv paths, the program, the Dockerfile, and `readable`, and `Diff::parse` accepts only `writable` paths. `Outcome` is the closed enum `Scored \| Rejected \| Crashed{stage} \| TimedOut{stage}`, and `Reason` holds only validated names, paths, or numbers. Tests `prop_path`, `manifest` (89 tests), `prop_diff`, `prop_metric`, and `golden_wire` all pass. |
| 2 | Identity and wire formats are stable: canonical diff bytes feed `ExperimentId`, the strict applier fails on any context mismatch, golden vectors cover every ID type, the 11-event set is frozen with an `EventKey` per event, golden `Record::V1` files exist, and the fold is deterministic | VERIFIED | `Diff::as_bytes()` returns the canonical text, and `ExperimentId::of` hashes it through `IdHasher` (domain, 32-byte digest, length prefix). `diff::apply::apply` has no offset search. `prop_apply` proves `ContextMismatch` and `OutOfRange`. `golden_ids` has golden vectors for baseline, `IdHasher`, `PromptHash`, `ReportHash`, `RunId`, `JudgeVersion`, and `ExperimentId::of`. `ledger.rs` defines exactly the 11 frozen events: RunStarted, ManifestChanged, Proposed, Judged (with `trial`), Selected, Spent, Steered, StopRequested, ReviewRequested, Reviewed, ReportAcknowledged. The key table is in the module docs, and `prop_keys` plus six golden key vectors guard it. `Record` is `#[serde(tag = "v")]` with `V1` renamed `"1"`. There are 26 insta snapshots under `tests/snapshots` (24 golden wire files: one per event variant and outcome, plus line, job, and scorecard; and 2 report files). `golden_wire::all_golden_bytes_pinned` adds a SHA-256 byte pin because insta normalizes whitespace. `.config/insta.yaml` sets `update: no`. `prop_fold` tests permutation invariance, idempotence under duplicate replay, and corrupt-ledger refusal (`FoldError`). |
| 3 | Pure decisions are deterministic: `select_greedy`, routing, the sweep sampler, the circuit breaker, `build_report` | VERIFIED | `select_greedy` takes the maximum of a total order over `(value, ExperimentId)` and applies `min_improvement` by multiply-and-compare (`improves`, `policy.rs:212`). `conclude` re-checks every recorded scorecard against the judge. `prop_policy` covers permutation invariance. `route.rs` climbs one rung per `stall_step`, clamps, resets on improvement, and `route_table` gives a reason for each ineligible tier. The sweep sampler has a ChaCha8 known-answer test, a seed golden vector, and its own range mapping. `circuit_breaker` (`policy.rs:454`) trips at exactly `max_empty_generations` empty generations or `max_infra_failures` (tests `breaker_trips_at_exactly_*` and `breaker_never_trips_early`). `build_report` produces a stable `ReportHash`, and `clean_report.snap` shows the summary against the noise floor, the cumulative diff, lineage, and provenance (isolation, judge, backends, root images, kernels, models, prompts). Flags are `AnomalousJump`, `HeldOutDivergence`, `LiteralMatch`, `NewDependency`, `EncodedBlob`, and two extras (`HiddenCharacters`, `WeakIsolation`). Blobs are checked against their IDs, and the target diff is checked to hash to the target. |
| 4 | Money, contracts, and display are safe by construction: `Spent` carries usage only, rates are pico-dollar `u128` with integer micro-dollar totals and no float, `JobRequest` and `JobResult` exist, the renderer neutralizes C0, C1, ESC, bidi, and zero-width characters | VERIFIED | There is no `f64` or `f32` in `money.rs`, `usage.rs`, or `ledger.rs`. `Usage` is six `u64` fields with `deny_unknown_fields`, so a dollar field is a parse error. `Event::Spent` holds `Usage` and a `SpendSource`, and the golden test `spent_carries_no_dollar_field` passes. The ledger holds no 128-bit integer. `Rate::parse` rejects negatives, exponents, a 13th fractional digit (never rounded), and overflow. A numeric rate in `prices.toml` is a located type error (`numeric_rate_is_a_located_type_error`). `job.rs` defines `JobBudget`, `JobRequest`, and `JobResult` through raw structs with `try_from`, and `JobRequest::candidate` checks that the diff blob addresses the canonical diff. `SafeText` (`text.rs`) is an allowlist: it keeps `\n` and `\t`, drops every `char::is_control` character (C0, DEL, C1, ESC, CR), and shows bidi and zero-width characters as `<U+XXXX>`. It has no `Deref` and no `From<String>`. The `render` tests pass. |
| 5 | `cargo clean` after verification passes (GATE-06) | PENDING by design | Per the orchestrator's note, `cargo clean` runs once after this verification passes. I did not run it, and `target/` is still warm. |

**Score:** 4/4 verifiable roadmap truths verified. SC5 is the orchestrator's post-verification action and is not a code truth.

### Behavior-dependent truths

The state-transition and ordering invariants in this phase are the fold (permutation invariance and duplicate replay), `select_greedy` (permutation invariance), the breaker (exact trip point), and the stall step in routing. A passing test exercises each one: `prop_fold`, `prop_policy`, `breaker`, and `route`. None is left PRESENT_BEHAVIOR_UNVERIFIED. The cancellation and durable-resume invariants belong to later phases.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/minos-core/src/{id,path,text,metric,outcome,usage,money,image,manifest,experiment,ledger,state,policy,route,sweep,job,report,limits}.rs` | Module set per CLAUDE.md | VERIFIED | All exist and are substantive (10,787 lines in total). Every module is declared in `lib.rs`, and the principal types are re-exported at the crate root. |
| `crates/minos-core/src/diff/{mod,apply,compose}.rs` | Parse, strict apply, compose | VERIFIED | The substantive implementation is in place. `compose` is tested against sequential apply. |
| `crates/minos-core/tests/*.rs` (19 files) and 26 `.snap` files | Property, golden, and unit tests | VERIFIED | `cargo test --workspace` passed all 41 result lines. |
| `crates/minos-core/tests/prop_fold.proptest-regressions` and `prop_policy.proptest-regressions` | Committed regression seeds | VERIFIED | Present, and `git status` is clean. |
| `.config/insta.yaml` | `update: no` | VERIFIED | Present with that content. |
| Workspace `serde_json` with `float_roundtrip` | Bit-exact metric round trip | VERIFIED | `Cargo.toml:82`. `metric_json_round_trips_extreme_literals` passes. |
| `crates/minos-core/clippy.toml` | Bans clock, env, fs, `HashMap`, `HashSet`, I/O in core | VERIFIED | Present. A separate grep finds no `HashMap`, `HashSet`, `SystemTime`, `Instant`, `std::fs`, `std::env`, `std::io`, or `tokio` in `src/`. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `Diff::parse` | `ExperimentId::of` | Canonical bytes (`as_bytes`) | WIRED | The canonical form is re-parsed (stage 7) before a `Diff` exists. |
| `Manifest::writable()` | `Diff::parse` writable argument | `BTreeSet<RelPath>` | WIRED | |
| `Scorecard` and `Outcome` | `Event::Judged` | Validated types inside the ledger event | WIRED | Deserialization re-validates. |
| `Event::key` | `Line::verify` and `fold` | `IdHasher` with per-variant tag | WIRED | `prop_keys` shows keys are distinct across a generated ledger. |
| `fold` | `RunState` | Records to state only | WIRED | The fold is the only state source. |
| `conclude`, `route`, `circuit_breaker`, `spend_ok`, `build_report` | `RunState`, `Manifest` | Pure functions over folded state | WIRED | Used in the tests. No downstream crate consumes them yet, which is expected: Phases 4, 6, and 7 do. |
| `Usage` | `spend_ok` and `Prices` | Integer usage times pico-dollar rate | WIRED | |

### Data-Flow Trace (Level 4)

Not applicable: `minos-core` renders nothing from a data source. Its values flow from function arguments, and the tests pin real outputs through golden files.

### Behavioral Spot-Checks and Gate Run

I ran these myself in this session. The artifacts were not reused.

| Check | Command | Result | Status |
|-------|---------|--------|--------|
| Format | `cargo fmt --all --check` | exit 0 | PASS |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no warning | PASS |
| Tests | `cargo test --workspace` | exit 0: 713 passed, 0 failed, 25 ignored (the VM spike tests and one cpio cross-check that need a signed `minos-vmm`) | PASS |
| Supply chain and layering | `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` | PASS |
| Docs | `cargo doc --workspace --no-deps` | exit 0 | PASS |
| Guest target | `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | exit 0 | PASS |
| C-02 no `allow` | `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | no output | PASS |
| Extra escape scan | grep for `.unwrap(`, `.expect(`, `unsafe`, and `#[expect` in `crates/minos-core`, excluding comments | no hit | PASS |

### Probe Execution

The phase declares no `probe-*.sh` scripts. Step 7c skipped: no probes declared.

### Requirements Coverage

Every requirement ID in the PLAN frontmatter is mapped in REQUIREMENTS.md. No Phase 3 requirement is orphaned, because REQUIREMENTS.md maps exactly CORE-01 to CORE-17 and COST-01 to Phase 3, and the plans claim all of them.

| Requirement | Source Plan(s) | Status | Evidence |
|-------------|----------------|--------|----------|
| CORE-01 | 03-01, 03-02, 03-05, 03-07 | SATISFIED | Distinct digest newtypes through the `digest_id!` macro, `IdHasher`, and the `golden_ids` vectors. |
| CORE-02 | 03-01 | SATISFIED | `RelPath` and `prop_path`. |
| CORE-03 | 03-05 | SATISFIED | `Manifest::try_from`, `ImageRef`, and `image::base_images`, per the 2026-10-14 amendment. |
| CORE-04 | 03-02 | SATISFIED | `Diff::parse` and the canonical form. |
| CORE-05 | 03-06 | SATISFIED | Strict applier with no offset search. |
| CORE-06 | 03-03 | SATISFIED | `Metric` and the held-out flag on `ScoredMetric`. |
| CORE-07 | 03-03, 03-09 | SATISFIED | `Outcome`, `Reason`, `Stage`. |
| CORE-08 | 03-07 | SATISFIED | 11 events and a documented `EventKey` for each. |
| CORE-09 | 03-09, 03-10 | SATISFIED | `fold`, `prop_fold`, `golden_wire`. |
| CORE-10 | 03-11 | SATISFIED | `select_greedy` and `prop_policy`. |
| CORE-11 | 03-05, 03-11 | SATISFIED | `route`, `route_table`, and the `IneligibleReason` texts. |
| CORE-12 | 03-08 | SATISFIED | `sweep.rs` with a ChaCha8 known-answer test. |
| CORE-13 | 03-09 | SATISFIED | `JobRequest` and `JobResult` goldens. |
| CORE-14 | 03-04 | SATISFIED | `Rate`, `Cost`, `ApproxCost`, `money` tests. |
| CORE-15 | 03-04 | SATISFIED | `SafeText` and `render` tests. |
| CORE-16 | 03-12 | SATISFIED | `build_report`, `ReportHash`, golden report snapshots. |
| CORE-17 | 03-11 | SATISFIED | `circuit_breaker` and the `breaker` tests. |
| COST-01 | 03-04, 03-07, 03-09 | SATISFIED | `Usage` is integers only, `Spent` has no dollar field, and `prices.toml` rates must be strings. |
| GATE-06 | 03-13 | PENDING by design | `cargo clean` follows this verification. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | | `TBD`, `FIXME`, `XXX`, `unwrap`, `expect`, `unsafe`, `allow`, `HashMap`, clock or I/O use | n/a | No hit in `crates/minos-core`. The `allow` grep hits were `allowed` in identifiers and error text only. |

### CLAUDE.md Honesty Check

The on-disk CLAUDE.md matches the code. It lists `Steered` and `StopRequested`, the `Record::V1` form, the `Spent` and `SpendSource` shape, the Diff, Manifest, Policy, and Outcome paragraphs, and the real module list. It states that `tool.rs` is deferred to TIER-01 and that Pareto is deferred to POL-01. It does not claim the runtime-behavior report flags (spawned processes, sockets, environment reads, denied operations), which it defers to RPT-01. It does not overclaim. The CLAUDE.md copy in the session system prompt shows the older Event sketch, which is stale context only, not the file on disk.

### Observations (informational, not gaps)

- **Float in `MinImprovement`.** `MinImprovement` stores an `f64`, as do `Goal::oriented`, `NoiseFloor::spread`, and the report delta fields. They are metric-domain values, not dollars. `MinImprovement::parse` rejects non-finite and negative amounts and signs, exponents, and over-1000% values, and `threshold_at` returns `None` on overflow. This does not conflict with "no float touches a dollar value".
- **Infrastructure-failure streak is caller-supplied.** `circuit_breaker` takes the infrastructure-failure streak as an argument, because infrastructure failures are not ledger records. This is documented in the function docs (03-RESEARCH Open Question 8), and a restart resets the streak. It satisfies CORE-17 as a pure rule in core. The Phase 7 loop must feed the streak.
- **No downstream consumer yet.** No crate other than the guest consumes the new `minos-core` types. That is expected, because Phases 4, 6, and 7 consume them.

### Human Verification Required

None. Every phase truth is a pure-function or type-level property, and each has automated evidence.

### Gaps Summary

No gaps. The phase goal is achieved in the codebase. A NaN metric, a float dollar amount, and an experiment that edits the judge or a non-writable path cannot be constructed through any public constructor or deserializer. The ledger wire format is frozen with an eleven-event set, per-event keys, a `v` tag, 24 golden wire snapshots, and byte pins. The full gate is green on a fresh run. The only open item is GATE-06 (`cargo clean`), which the orchestrator runs next.

---

_Verified: 2026-10-14_
_Verifier: Claude (gsd-verifier)_
