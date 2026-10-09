# Phase 1: Workspace and Gates - Context

**Gathered:** 2026-10-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 creates the Cargo workspace from nothing: every v1 crate as an empty stub, and the configuration files that make the workspace lints, the supply-chain policy, and the crate-layering rule hold from the first commit. The gate is the four plain cargo commands listed in CLAUDE.md Commands (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`). There is no script, helper crate, or CI workflow in front of them.

Requirements: GATE-01, GATE-02, GATE-03, GATE-04, GATE-05. Cross-cutting: GATE-06 (`cargo clean` after verification passes) and `rules/` adherence.

**Wording superseded by this discussion:** REQUIREMENTS.md GATE-02 and ROADMAP.md Phase 1 success criteria 1 through 3 say "a single `ci` script". The user decided there is no script. Read "the `ci` script passes/fails" as "the four cargo commands pass/fail". The planner should include a task that corrects that wording in REQUIREMENTS.md and ROADMAP.md.

</domain>

<decisions>
## Implementation Decisions

### Gate mechanism: configuration only, no scripts
- **D-01:** No `ci` script and no `xtask` crate. All policy lives in `Cargo.toml` (`[workspace.lints]`), `crates/minos-core/clippy.toml`, `deny.toml`, and `rust-toolchain.toml`. Cargo enforces it when the four commands run.
- **D-02:** The `allow(clippy::...)` ban (GATE-05) is a lint, not a grep: enable clippy `allow_attributes` and `allow_attributes_without_reason` so only `#[expect(..., reason = "...")]` passes.
- **D-03:** The crate-layering rule (GATE-04) is enforced in `deny.toml` `[bans]` using `deny` entries with `wrappers`, so a dependency edge from `minos-core` or `minos-loop` to `minos-sandbox` or any backend crate fails `cargo deny check`. No metadata script.
- **D-04:** No GitHub Actions workflow in Phase 1. Nothing under `.github/`.
- **D-05:** `rust-toolchain.toml` pins 1.97.1 with `rustfmt` and `clippy` components. Edition 2024. `Cargo.lock` committed. `resolver = "3"`.

### Crate skeleton scope
- **D-06:** All v1 crates exist as empty stubs after Phase 1: `minos-core`, `minos-store`, `minos-sandbox`, `minos-backend-krun`, `minos-vmm`, `minos-guest`, `minos-propose`, `minos-runner`, `minos-loop`, `minos-cli`. Each has `[lints] workspace = true` and declares its real dependency edges from research/ARCHITECTURE.md so the layering rule covers the whole graph on day one.
- **D-07:** No AWS crates (`minos-orchestrator`, `minos-backend-lambda`, `minos-backend-chv`, `minos-store-aws`) and no `xtask` member. Later milestones add them.
- **D-08:** License is `Apache-2.0 OR MIT`. Both `LICENSE-APACHE` and `LICENSE-MIT` at the repo root; `license` set in `[workspace.package]`.
- **D-09:** Phase 1 builds and checks for the host target (`aarch64-apple-darwin`) only. The `aarch64-unknown-linux-musl` guest cross-build arrives in Phase 5. `deny.toml` `[graph] targets` lists both triples so the dependency graph is checked for both.

### Gate self-test
- **D-10:** One-time manual proof. During phase verification each deliberate violation is applied, the failing command output is captured into VERIFICATION.md, and the violation is reverted. No fixture crates or gate self-test code live in the repo.
- **D-11:** Violations to prove: an `unsafe` block; `unwrap`, `expect`, and a slice index; `#[allow(clippy::...)]`; `SystemTime::now` and `HashMap` in `minos-core`; a git dependency without `rev`; an unknown registry or git source; a build script not on the allowlist; a dependency from `minos-core` or `minos-loop` on `minos-sandbox` or a backend crate.

### Lint baseline breadth
- **D-12:** The full `rules/` lint set goes into `[workspace.lints]`, on top of the four locked lints (`unsafe_code = "forbid"`; `unwrap_used`, `expect_used`, `indexing_slicing` denied): clippy `correctness` deny; `complexity`, `perf`, `style`, `suspicious` warn; `pedantic` warn with the carve-outs named in `rules/lint-pedantic-selective.md`, each with a reason; `missing_docs` warn; rustdoc `broken_intra_doc_links` deny; clippy `cargo` warn. Because the gate runs with `-D warnings`, every warn is a hard failure.
- **D-13:** Every entry in `[workspace.dependencies]` uses an exact `=x.y.z` pin. Upgrades are explicit reviewed changes.
- **D-14:** The full v1 dependency table from `.planning/research/STACK.md` (Installation section) is declared in Phase 1, and each stub crate pulls in what it will use. `Cargo.lock`, the `deny.toml` build-script allowlist, and the C-crate bans are therefore exercised against the real graph now, including `msb_krun` in `minos-vmm`. Feature names must be verified against each pinned version before the manifest is written.

### Claude's Discretion
- Lib versus bin shape of each stub (`minos-cli`, `minos-vmm`, `minos-guest` are binaries; the rest are libraries) and the minimal file contents needed to satisfy `missing_docs`.
- `rustfmt.toml` contents, if any beyond edition.
- `deny.toml` `multiple-versions` level and license allowlist, starting from the research sketch.
- The exact pedantic carve-out list, as long as each carve-out carries a reason and follows `rules/lint-pedantic-selective.md`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record and rules
- `CLAUDE.md` — Code rules section (workspace lints, deserialization, errors, ordering, no C, no git, argv commands, supply chain); Workspace section (crate list and strict dependency direction); Commands section (the four gate commands and `cargo clean` at end of phase).
- `.claude/CLAUDE.md` — Decisions at a glance; Question 7 (cargo-deny config); Installation (workspace `Cargo.toml` sketch, `.cargo/config.toml`, core `clippy.toml`); Version Compatibility table.
- `rules/lint-workspace-lints.md`, `rules/lint-deny-correctness.md`, `rules/lint-pedantic-selective.md`, `rules/lint-missing-docs.md`, `rules/lint-rustfmt-check.md`, `rules/lint-warn-complexity.md`, `rules/lint-warn-perf.md`, `rules/lint-warn-style.md`, `rules/lint-warn-suspicious.md`, `rules/lint-cargo-metadata.md`, `rules/lint-unsafe-doc.md` — the lint set D-12 adopts.
- `rules/err-thiserror-lib.md`, `rules/err-anyhow-app.md` — which crates may depend on `anyhow` (only `minos-cli`).

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — GATE-01 through GATE-06 (GATE-02 wording superseded per D-01).
- `.planning/ROADMAP.md` — Phase 1 goal and success criteria; Cross-Cutting Rules; Dependencies and Parallelism.

### Research
- `.planning/research/STACK.md` — Question 7 (full `deny.toml`), Installation (workspace manifest with exact pins, dependency table, `.cargo/config.toml`, core `clippy.toml`), Version Compatibility, Corrections.
- `.planning/research/ARCHITECTURE.md` — crate list including `minos-loop` and `minos-vmm`, the dependency-direction diagram, and amendment A3 (which crates are deferred). Source of truth for each stub's dependency edges in D-06.
- `.planning/research/PITFALLS.md` — Pitfall 17 (`HashMap` and feature unification; `clippy.toml` replaces rather than merges), Pitfall 25 (lint escape via `allow`), Pitfall 26 (no `anyhow` in library crates), Pitfall 27 (cargo-deny defaults do not gate).

</canonical_refs>

<code_context>
## Existing Code Insights

The repository contains no Rust code. It holds `CLAUDE.md`, `.claude/CLAUDE.md`, `rules/` (179 rule files), and `.planning/`. There is no `Cargo.toml`, `Cargo.lock`, `deny.toml`, `rust-toolchain.toml`, `.cargo/`, `crates/`, or `.github/`.

### Reusable Assets
- `.planning/research/STACK.md` Installation section: a near-complete workspace `Cargo.toml`, `deny.toml`, `.cargo/config.toml`, and core `clippy.toml` to start from.
- `rules/lint-*.md`: the lint tables to transcribe into `[workspace.lints]`.

### Established Patterns
- Policy as configuration: the user wants Cargo-native files, never scripts or helper crates, wherever a config file can express the rule.
- A crate-level `clippy.toml` replaces the workspace one rather than merging with it, so core's bans must live in `crates/minos-core/clippy.toml` and any workspace-wide `clippy.toml` must not be assumed to apply there.

### Integration Points
- Phase 2 and Phase 3 both start from this workspace. Phase 3 fills `minos-core`; Phase 2's spike code must also pass the gate, with no `unsafe` exception.
- Phase 5 was planned to codesign `minos-vmm` through an `xtask`. With no `xtask` crate, Phase 5 must choose another mechanism.

</code_context>

<specifics>
## Specific Ideas

- The user's position, in their words: if the lints are in `Cargo.toml`, there is no need for a `ci` script. The gate is cargo.
- Verification output for each deliberate violation is the proof; capture the actual failing command text, not a summary.

</specifics>

<deferred>
## Deferred Ideas

- **Phase 5 codesign mechanism.** Research assumed an `xtask` signs `minos-vmm` after build. The user prefers no custom tooling, so Phase 5 must decide how `codesign` runs (for example a documented command in CLAUDE.md, or `minos host setup`). Not a Phase 1 concern.
- **GitHub Actions workflow.** Not in Phase 1. Add when there is a reason to run the gate on another machine.
- **musl cross-build of `minos-guest`.** Phase 5.

</deferred>

---

*Phase: 01-workspace-and-gates*
*Context gathered: 2026-10-09*
