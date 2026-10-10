# Phase 3: Core Types and Pure Logic - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-10
**Phase:** 3-core-types-and-pure-logic
**Areas discussed:** Diff and surface semantics, Event set freeze details, Selection under noise, Report flags and provenance

---

## Diff and surface semantics

### What may a `writable` entry name?

| Option | Description | Selected |
|--------|-------------|----------|
| Exact files only | One existing file per entry; bytewise match on the canonical `RelPath` | ✓ |
| Files and directory prefixes | An entry ending in `/` admits any path under it; disjointness becomes prefix overlap | |
| Files, prefixes, and globs | Full glob syntax; needs a glob crate in core; overlap check falls back to conservative | |

**User's choice:** Exact files only (recommended).

### May a diff create or delete a writable file?

| Option | Description | Selected |
|--------|-------------|----------|
| Modify only | Every writable path must exist in the base tree; `/dev/null` on either side rejected | ✓ |
| Modify or create | A writable path may be absent and created; deletion rejected | |
| Modify, create, or delete | Both `/dev/null` directions accepted for writable paths | |

**User's choice:** Modify only (recommended).

### Empty or no-op diffs

| Option | Description | Selected |
|--------|-------------|----------|
| Reject as invalid | Dropped like any invalid proposal; baseline represented separately | ✓ |
| Accept, treat as the baseline | Empty diff valid; `ExperimentId::of(base, empty)` is the baseline | |

**User's choice:** Reject as invalid (recommended).

### How is the baseline identified?

| Option | Description | Selected |
|--------|-------------|----------|
| Same hash, dedicated constructor | `ExperimentId::baseline(&TreeId)`: same prefix, base, zero-length diff; `Experiment` cannot hold an empty parsed diff | ✓ |
| Baseline is the TreeId itself | `parent` becomes `Baseline(TreeId) \| Experiment(ExperimentId)` | |

**User's choice:** Same hash, dedicated constructor (recommended).

---

## Event set freeze details

### What does `StopRequested { reason }` carry?

| Option | Description | Selected |
|--------|-------------|----------|
| Closed enum | `StopReason::{UserStop, SpendCap, TokenCap, NoValidCandidates, InfraFailures, ProviderLimit}` | ✓ |
| Closed enum plus a note | Same variants plus an optional `note: BlobId` | |
| Free text | `reason: String`, hostile text in every consumer | |

**User's choice:** Closed enum (recommended).

### Where does `RunId` come from?

| Option | Description | Selected |
|--------|-------------|----------|
| Hash of manifest, baseline, and a shell-supplied start time | `RunId::of(&ManifestId, &TreeId, started_at)`; `RunStarted` records `started_at` | ✓ |
| Shell-supplied opaque 32 bytes | CLI generates random bytes; no provenance | |
| User-chosen name | `minos run --name <slug>`; uniqueness is the user's problem | |

**User's choice:** Hash of manifest, baseline, and start time (recommended).

### `Steered` key and storage

| Option | Description | Selected |
|--------|-------------|----------|
| Blob plus generation and ordinal | `Steered { generation, ordinal, note: BlobId }`, key `H(run, generation, ordinal)` | ✓ |
| Inline text, keyed by generation and ordinal | Same key, note as a `String` in the record | |
| Keyed by note hash only | Identical notes collapse into one | |

**User's choice:** Blob plus generation and ordinal (recommended).

### `ReviewRequested` / `Reviewed` shape with no v1 consumer

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal closed enums, extended later | `ReviewKind::{JudgeDesign, Params, Dependencies}`, `ReviewAnswer::{Approved, Declined}`, key `H(run, request)` | ✓ |
| Rich shapes designed now | Model the full inbox item today | |

**User's choice:** Minimal closed enums, extended later (recommended).

### What is `source` in `SpendId = H(run, generation, source)`?

| Option | Description | Selected |
|--------|-------------|----------|
| Closed enum with an index | `SpendSource::{ModelCall { call }, Candidate { exp, trial }, Judge { exp, trial }}`; one `Spent` per call or VM run | ✓ |
| Per-generation aggregate | One `Spent` per generation per kind, summed by the shell | |

**User's choice:** Closed enum with an index (recommended).

---

## Selection under noise

### How do repeated trials reduce to one score?

| Option | Description | Selected |
|--------|-------------|----------|
| Median per metric | Lower median for even counts; a single trial is its own median | ✓ |
| Worst trial | Min for maximize, max for minimize | |
| Mean per metric | Uses every sample; needs a rounding rule | |

**User's choice:** Median per metric (recommended).

### Where does the `min_improvement` versus noise floor check live?

| Option | Description | Selected |
|--------|-------------|----------|
| Pure core function, called by check and run | `policy::noise_check -> NoiseVerdict::{Ok, Warn, Refuse}`; `check` prints, `run` refuses unless overridden | ✓ |
| Warning in check only | `run` never refuses | |
| Fold it into selection | `select_greedy` raises the effective threshold automatically | |

**User's choice:** Pure core function, called by check and run (recommended).

### What happens to the tier after an improvement?

| Option | Description | Selected |
|--------|-------------|----------|
| Reset to the lowest eligible tier | Tier = lowest + stalled generations / `stall_step` since the last improvement, clamped | ✓ |
| Stay at the current tier | Tier only climbs | |

**User's choice:** Reset to the lowest eligible tier (recommended).

### Where do `STALL_STEP` and the breaker's K live?

| Option | Description | Selected |
|--------|-------------|----------|
| Manifest with defaults bounded by limits.rs | `[policy] stall_step`, `[budget] max_empty_generations`, `max_infra_failures`, range-checked in `Manifest::try_from` | ✓ |
| Constants in limits.rs only | Fixed for every project | |

**User's choice:** Manifest with defaults bounded by limits.rs (recommended).

---

## Report flags and provenance

### Where do the visible inputs for literal matching come from?

| Option | Description | Selected |
|--------|-------------|----------|
| Readable and writable surface files from the base tree | Already blobs `build_report` has; no manifest field; never judge data | ✓ |
| A manifest list of visible input files | `[judge] visible = [...]` resolved from the base tree | |
| Skip this flag in v1 | Render 'not checked' | |

**User's choice:** Readable and writable surface files from the base tree (recommended).

### What is an anomalous jump?

| Option | Description | Selected |
|--------|-------------|----------|
| Multiple of the noise floor spread, fallback to lineage | Delta over parent exceeds `k` times the spread; without a floor, `k` times the largest earlier accepted delta | ✓ |
| Fixed percentage | Flag any single-step improvement over a constant | |
| Noise floor only, else never | Flag only when a floor exists | |

**User's choice:** Multiple of the noise floor spread, fallback to lineage (recommended).

### What does the summary against the noise floor show?

| Option | Description | Selected |
|--------|-------------|----------|
| Per objective: baseline median and spread, winner median, delta, verdict | One row per objective and gate with trial counts; held-out rows marked | ✓ |
| Delta and verdict only | One line per objective | |

**User's choice:** Per objective full row (recommended).

### Render the weak-isolation field in v1?

| Option | Description | Selected |
|--------|-------------|----------|
| Render it, sourced from BackendInfo | Weakest level in the lineage plus any reduced-hardening layer; golden-tested | ✓ |
| Omit until a weak mode exists | No field, no code; adding later changes every report hash | |

**User's choice:** Render it, sourced from BackendInfo (recommended).

---

## Claude's Discretion

- Diff size cap, per-file hunk cap, CRLF and no-newline-at-EOF handling, the canonical header format (bounded by CLI-09), and how the applier reports a context mismatch.
- JSONL line shape, `EventKey` encoding, `Decision` fields, `BackendInfo` fields beyond isolation and confinement layers, whether `exp` stays on `Spent`.
- `JobRequest` and `JobResult` field lists; the `prices.toml` shape.
- The renderer's visible form for bidi and zero-width characters.
- Every numeric default and bound in `limits.rs`.
- The new-dependencies and encoded-blob heuristics, report section order, and the report hash's canonical encoding.
- Whether `generation::{plan, admit, conclude}` and `spend_ok` ship in this phase or Phase 7; module layout; plan count and order (CORE-08 before golden files).

## Deferred Ideas

- `tool.rs` and `ToolCall` (agentic tier, TIER-01).
- Rich review payloads, inbox, `answer`, `DesignJudge`, pareto and `review` policies (POL-01).
- Runtime-behavior report flags (RPT-01).
- The `--allow-noisy` flag name and the default baseline repeat count (Phase 8).
- The `git apply` and `patch -p1` golden test (Phase 8, CLI-09).
- Power source and thermal state in `BackendInfo` (Phase 5 or later).
