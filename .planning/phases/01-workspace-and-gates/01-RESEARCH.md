# Phase 1: Workspace and Gates - Research

**Researched:** 2026-10-09
**Domain:** Cargo workspace setup, clippy and rustc lint policy, cargo-deny supply-chain and layering policy
**Confidence:** HIGH. Almost every claim below comes from a scratch copy of the planned workspace, built and run on this machine with rustc 1.97.1, clippy 0.1.97, and cargo-deny 0.20.2. Each of the four gate commands was run against it, along with every violation in D-11.

## Summary

A scratch workspace was built exactly as CONTEXT.md describes, with 10 stub crates and the full STACK.md dependency table pinned with `=`. With the corrections below, it passes all four gate commands in about 13 seconds from a clean target directory. Each violation in D-11 fails the expected command, and the exact output was captured. The design holds, but five findings change the plan:

1. **STACK.md's `deny.toml` fails against the real graph.**
   - The build-script allowlist is missing 11 crates.
   - The license list is missing `BSL-1.0` (needed by `xxhash-rust`, which `backhand` pulls in) and includes two licenses the graph never uses (`MPL-2.0`, `CC0-1.0`).
   - Internal path dependencies without a `version` count as wildcards and fail `wildcards = "deny"`.
2. **clippy `multiple_crate_versions` fails the gate on day one.** msb_krun, backhand, and proptest pull in two versions each of `syn`, `bitflags`, `miniz_oxide`, `nix`, `getrandom`, `rand_core`, and `rand_chacha`. It needs a carve-out in `[workspace.lints.clippy]`. Cargo rejects a `reason` key there, so the reason must be a TOML comment.
3. **The layering ban only covers direct dependency edges.** `wrappers` lets a listed crate depend directly on the banned crate. It does not follow transitive paths: `minos-loop -> minos-runner -> minos-sandbox` passed when only `minos-sandbox` was banned. Every internal crate needs its own `deny` entry listing its allowed direct dependents. With that, the whole graph from ARCHITECTURE.md is encoded and every forbidden edge fails, including dev-dependency edges. The same mechanism enforces "`anyhow` only in `minos-cli`" (Pitfall 26) and "only `minos-vmm` links `msb_krun`" (KRUN-01).
4. **Three gaps that the four commands do not catch (need a user decision):**
   - (a) `allow_attributes` ignores inner attributes, so `#![allow(clippy::unwrap_used, reason = "...")]` passes.
   - (b) A crate that leaves out `[lints] workspace = true` silently loses every workspace lint. Clippy accepts an `unwrap` in it.
   - (c) `[workspace.lints.rustdoc] broken_intra_doc_links = "deny"` is only checked by `cargo doc`. Clippy and `cargo test` ignore it.
5. **`publish = false` is needed** (or full crates.io metadata plus a repository URL that does not exist yet). With `publish = false`, clippy's `cargo_common_metadata` skips the crate. Without it, each crate fails for missing `readme`, `keywords`, and `categories`.

The Linux-only guest crates (`rustix` with `mount`, `process`, and `system`, and `vsock` 0.5.4) compile on macOS when a stub depends on them. `msb_krun =0.1.41` with `["blk"]` also compiles on macOS arm64 without libkrunfw installed. Every pinned version and feature named in D-14 exists. One caveat: the `toml` version string on crates.io is `1.1.8+spec-1.1.0`, published today, and `=1.1.8` resolves to it.

**Primary recommendation:** Use the exact `Cargo.toml`, `deny.toml`, `crates/minos-core/clippy.toml`, and `rust-toolchain.toml` in Code Examples. They are the configuration that passed. Then prove each violation with the commands in the Validation Architecture section.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Gate mechanism: configuration only, no scripts
- **D-01:** No `ci` script and no `xtask` crate. All policy lives in `Cargo.toml` (`[workspace.lints]`), `crates/minos-core/clippy.toml`, `deny.toml`, and `rust-toolchain.toml`. Cargo enforces it when the four commands run.
- **D-02:** The `allow(clippy::...)` ban (GATE-05) is a lint, not a grep: enable clippy `allow_attributes` and `allow_attributes_without_reason` so only `#[expect(..., reason = "...")]` passes.
- **D-03:** The crate-layering rule (GATE-04) is enforced in `deny.toml` `[bans]` using `deny` entries with `wrappers`, so a dependency edge from `minos-core` or `minos-loop` to `minos-sandbox` or any backend crate fails `cargo deny check`. No metadata script.
- **D-04:** No GitHub Actions workflow in Phase 1. Nothing under `.github/`.
- **D-05:** `rust-toolchain.toml` pins 1.97.1 with `rustfmt` and `clippy` components. Edition 2024. `Cargo.lock` committed. `resolver = "3"`.

#### Crate skeleton scope
- **D-06:** All v1 crates exist as empty stubs after Phase 1: `minos-core`, `minos-store`, `minos-sandbox`, `minos-backend-krun`, `minos-vmm`, `minos-guest`, `minos-propose`, `minos-runner`, `minos-loop`, `minos-cli`. Each has `[lints] workspace = true` and declares its real dependency edges from research/ARCHITECTURE.md so the layering rule covers the whole graph on day one.
- **D-07:** No AWS crates (`minos-orchestrator`, `minos-backend-lambda`, `minos-backend-chv`, `minos-store-aws`) and no `xtask` member. Later milestones add them.
- **D-08:** License is `Apache-2.0 OR MIT`. Both `LICENSE-APACHE` and `LICENSE-MIT` at the repo root; `license` set in `[workspace.package]`.
- **D-09:** Phase 1 builds and checks for the host target (`aarch64-apple-darwin`) only. The `aarch64-unknown-linux-musl` guest cross-build arrives in Phase 5. `deny.toml` `[graph] targets` lists both triples so the dependency graph is checked for both.

#### Gate self-test
- **D-10:** One-time manual proof. During phase verification each deliberate violation is applied, the failing command output is captured into VERIFICATION.md, and the violation is reverted. No fixture crates or gate self-test code live in the repo.
- **D-11:** Violations to prove: an `unsafe` block; `unwrap`, `expect`, and a slice index; `#[allow(clippy::...)]`; `SystemTime::now` and `HashMap` in `minos-core`; a git dependency without `rev`; an unknown registry or git source; a build script not on the allowlist; a dependency from `minos-core` or `minos-loop` on `minos-sandbox` or a backend crate.

#### Lint baseline breadth
- **D-12:** The full `rules/` lint set goes into `[workspace.lints]`, on top of the four locked lints (`unsafe_code = "forbid"`; `unwrap_used`, `expect_used`, `indexing_slicing` denied): clippy `correctness` deny; `complexity`, `perf`, `style`, `suspicious` warn; `pedantic` warn with the carve-outs named in `rules/lint-pedantic-selective.md`, each with a reason; `missing_docs` warn; rustdoc `broken_intra_doc_links` deny; clippy `cargo` warn. Because the gate runs with `-D warnings`, every warn is a hard failure.
- **D-13:** Every entry in `[workspace.dependencies]` uses an exact `=x.y.z` pin. Upgrades are explicit reviewed changes.
- **D-14:** The full v1 dependency table from `.planning/research/STACK.md` (Installation section) is declared in Phase 1, and each stub crate pulls in what it will use. `Cargo.lock`, the `deny.toml` build-script allowlist, and the C-crate bans are therefore exercised against the real graph now, including `msb_krun` in `minos-vmm`. Feature names must be verified against each pinned version before the manifest is written.

### Claude's Discretion
- Lib versus bin shape of each stub (`minos-cli`, `minos-vmm`, `minos-guest` are binaries; the rest are libraries) and the minimal file contents needed to satisfy `missing_docs`.
- `rustfmt.toml` contents, if any beyond edition.
- `deny.toml` `multiple-versions` level and license allowlist, starting from the research sketch.
- The exact pedantic carve-out list, as long as each carve-out carries a reason and follows `rules/lint-pedantic-selective.md`.

### Deferred Ideas (OUT OF SCOPE)
- **Phase 5 codesign mechanism.** Research assumed an `xtask` signs `minos-vmm` after build. The user prefers no custom tooling, so Phase 5 must decide how `codesign` runs (for example a documented command in CLAUDE.md, or `minos host setup`). Not a Phase 1 concern.
- **GitHub Actions workflow.** Not in Phase 1. Add when there is a reason to run the gate on another machine.
- **musl cross-build of `minos-guest`.** Phase 5.

**Wording superseded by D-01:** REQUIREMENTS.md GATE-02 and the ROADMAP.md Phase 1 goal and success criteria 1 through 3 say "a single `ci` script". Read that as "the four cargo commands". The planner must include a task that corrects the wording.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GATE-01 | Workspace builds on rustc 1.97.1 / edition 2024 with `unsafe_code = "forbid"` and clippy `unwrap_used`, `expect_used`, `indexing_slicing` denied in `[workspace.lints]` | Verified `[workspace.lints]` block (Code Examples). The `unsafe` proof fails with `usage of an unsafe block` and `-F unsafe-code`. The unwrap, expect, and index proofs fail with clippy messages (Validation Architecture). |
| GATE-02 | The four commands pass and `Cargo.lock` is committed (the "single `ci` script" wording is superseded by D-01) | All four passed on the scratch workspace: fmt 0.06s, clippy 5.7s, test 6.9s, and deny 0.6s from a clean target. Requires the deny.toml corrections, the `multiple_crate_versions` carve-out, and `publish = false`. |
| GATE-03 | `deny.toml` requires `rev` on git dependencies, denies unknown registries and git sources, and allowlists build scripts | `[sources]` keys verified. A branch dependency gives `git-source-underspecified`. A rev dependency gives `source-not-allowed`. An alternative registry gives `source-not-allowed`. The complete 25-crate build-script allowlist was derived from `cargo metadata` for both targets. |
| GATE-04 | The gate fails if `minos-loop` or `minos-core` gains a dependency on `minos-sandbox` or any backend crate | There is one `[bans] deny` entry with `wrappers` per internal crate. `wrappers` covers direct edges only, as verified. A dev-dependency edge is also caught. A normal `core -> sandbox` edge is a Cargo cycle, which fails even earlier, in every cargo command. |
| GATE-05 | `minos-core` bans `SystemTime::now` and `HashMap`; `allow(clippy::...)` never appears, only `#[expect(..., reason = ...)]` | `crates/minos-core/clippy.toml` uses `disallowed-methods` and `disallowed-types`, both in the style group, so `-D warnings` fails them. `allow_attributes` plus `allow_attributes_without_reason` catch every outer `#[allow]` and every inner `#![allow]` without a reason. **Gap:** an inner `#![allow(..., reason = ...)]` passes. See Open Question 1. |
| GATE-06 (cross-cutting) | `cargo clean` after verification passes | A plain command. The last task of the phase. |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- `unsafe_code = "forbid"`; clippy `unwrap_used`, `expect_used`, `indexing_slicing` = deny in `[workspace.lints]`.
- **"ALL CODE MUST ADHERE TO RULES"** (`rules/`), with no exceptions.
- Never weaken a security invariant to make something compile or pass. No escape hatch and no "temporary" bypass. If blocked, stop and ask.
- No C in our own crates. No OpenSSL, no libgit2, no git in the trusted path. The rustls crypto provider is the only accepted exception, and it arrives in a later milestone.
- Supply chain: commit `Cargo.lock`. `cargo deny check` gates every build. Pin git dependencies by `rev`.
- `BTreeMap`/`BTreeSet` wherever order affects output. `minos-core` has no async, AWS, filesystem, clock, or unseeded randomness. Clippy `disallowed-methods` bans `SystemTime::now` in core.
- Dependency direction is strict. Backends are separate crates, not feature flags.
- `anyhow` only in `minos-cli`'s `main` (`rules/err-anyhow-app.md`). Library crates use `thiserror`.
- Gate commands: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`.
- Run `cargo clean` at the end of every phase, after verification passes. No exceptions.
- Rust 1.94.1 or newer and edition 2024. `.claude/CLAUDE.md` pins 1.97.1.
- Keep CLAUDE.md current: when a decision changes, edit it in the same change.
- The user's memory notes **"Cargo-native config, no scripts"**: no ci script and no xtask. Policy lives in Cargo.toml, clippy.toml, and deny.toml.

## Architectural Responsibility Map

This phase has no runtime tiers. The "tiers" are the enforcement layers that own each policy.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Workspace-wide lint levels | `Cargo.toml` `[workspace.lints]` | each crate's `[lints] workspace = true` | Cargo passes the levels to rustc and clippy for every member that opts in |
| Core-only bans (clock, hash order) | `crates/minos-core/clippy.toml` | — | Clippy reads the nearest `clippy.toml` up from `CARGO_MANIFEST_DIR`, so only core picks it up |
| Toolchain pin | `rust-toolchain.toml` | `[workspace.package] rust-version` | rustup selects 1.97.1 (verified: "overridden by rust-toolchain.toml"). `rust-version` drives the MSRV-aware resolver 3 |
| Supply chain (sources, licenses, advisories, C bans, build scripts) | `deny.toml` | `Cargo.lock` | cargo-deny reads the lockfile graph for both target triples |
| Crate layering | `deny.toml` `[bans] deny` + `wrappers` | Cargo's own cycle detection | Edges are data in the lockfile graph. Cargo itself rejects a normal edge that would form a cycle |
| Exact pins, centralized | `[workspace.dependencies]` | `deny.toml` `[bans.workspace-dependencies]` | cargo-deny fails a crate that declares a workspace dependency directly, and fails an unused workspace dependency |

## Standard Stack

### Core (Phase 1 tooling)
| Tool | Version | Purpose | Status on this machine |
|------|---------|---------|------------------------|
| rustc / cargo | 1.97.1 (8bab26f4f 2026-07-14) | Compiler | Installed: `1.97.1-aarch64-apple-darwin` [VERIFIED: rustup toolchain list] |
| clippy | 0.1.97 | Lints | Installed for 1.97.1 [VERIFIED: `rustup component list --installed`] |
| rustfmt | 1.9.0-stable | Formatting | Installed for 1.97.1 [VERIFIED] |
| cargo-deny | 0.20.2 (latest) | Supply chain and layering | Installed [VERIFIED: `cargo deny --version`, crates.io max_stable 0.20.2] |

### Workspace dependencies (D-14). Every version and feature was checked against the crates.io API on 2026-10-09
| Crate | Pin | Features verified at that version | Notes |
|-------|-----|-----------------------------------|-------|
| msb_krun | =0.1.41 | `blk` ✓, `net` ✓ (default = []) | `default-features = false, features = ["blk"]` compiles on macOS arm64 without libkrunfw [VERIFIED: local build] |
| diffy | =0.5.2 | `std` ✓, `binary` ✓ (default = []) | MSRV 1.85 |
| tar | =0.4.46 | default = `["xattr"]` | `default-features = false` drops xattr |
| sha2 | =0.11.0 | default = `alloc, oid` | |
| serde | =1.0.229 | `derive` ✓ | |
| serde_json | =1.0.151 | default `std` | Do not enable `preserve_order` or `arbitrary_precision` (Pitfall 17) |
| toml | =1.1.8 | default `std, serde, parse, display` | **The crates.io version string is `1.1.8+spec-1.1.0`, published 2026-10-09.** `"=1.1.8"` resolves to it, as verified by the lockfile |
| thiserror | =2.0.21 | default `std` | |
| clap | =4.6.7 | `derive` ✓ | MSRV 1.85 |
| anyhow | =1.0.104 | default `std` | `minos-cli` only (enforced by a deny ban) |
| base16ct | =1.0.0 | `alloc` ✓ (no default features) | `features = ["alloc"]` is valid |
| ignore | =0.4.33 | only `simd-accel` | License `Unlicense OR MIT` |
| regex | =1.13.1 | defaults | |
| rand_chacha | =0.10.0 | `std`, `serde` | `default-features = false` is valid |
| rand_core | =0.10.1 | none | |
| backhand | =0.25.5 | `gzip` ✓ (default = xz, gzip, zstd, lz4, parallel, error-strings) | `default-features = false, features = ["gzip"]` avoids `zstd-sys`/`liblzma-sys` (verified: neither is in the graph). It brings in `xxhash-rust` (**BSL-1.0**) |
| tempfile | =3.27.0 | default `getrandom` | |
| tracing | =0.1.44 | defaults | |
| tracing-subscriber | =0.3.23 | defaults | |
| rustix | =1.1.5 | `fs` ✓, `mount` ✓, `process` ✓, `system` ✓ | Compiles on macOS with these features [VERIFIED: local build] |
| vsock | =0.5.4 | no features | Compiles on macOS [VERIFIED: local build] |
| proptest | =1.11.0 | default `std, fork, timeout, bit-set` | dev only |
| insta | =1.49.0 | `json` ✓, `toml` ✓ | dev only |
| trybuild | =1.0.122 | `diff` | dev only. MSRV 1.88 |

No mismatches. Every version exists, none is yanked, and every named feature exists at its pinned version [VERIFIED: crates.io API `/api/v1/crates/<name>/<version>`].

### Alternatives Considered
None. The stack is locked by D-13/D-14 and STACK.md.

**Installation:** none. Cargo fetches everything on the first `cargo generate-lockfile` or `cargo clippy`.

## Package Legitimacy Audit

| Package | Registry | First published | Recent downloads | Source repo | Verdict | Disposition |
|---------|----------|-----------------|------------------|-------------|---------|-------------|
| msb_krun | crates.io | 2026 | 61k | github.com/zerocore-ai/libkrun (sub-crates point to containers/libkrun) | OK | Approved (locked by STACK.md; compile verified) |
| diffy | crates.io | 2020 | 4.6M | github.com/bmwill/diffy | OK | Approved |
| tar | crates.io | 2014 | 61M | github.com/composefs/tar-rs | OK | Approved |
| sha2 | crates.io | 2016 | 295M | github.com/RustCrypto/hashes | OK | Approved |
| serde / serde_json | crates.io | 2014/2015 | 353M | github.com/serde-rs | OK | Approved |
| toml | crates.io | 2014 | 251M | github.com/toml-rs/toml | OK | Approved (1.1.8 published today) |
| thiserror / anyhow | crates.io | 2019 | 416M / 238M | github.com/dtolnay | OK | Approved |
| clap | crates.io | 2015 | 253M | github.com/clap-rs/clap | OK | Approved |
| base16ct | crates.io | 2022 | 76M | github.com/RustCrypto/formats | OK | Approved |
| ignore | crates.io | 2016 | 43M | github.com/BurntSushi/ripgrep | OK | Approved |
| regex | crates.io | 2014 | 275M | github.com/rust-lang/regex | OK | Approved |
| rand_chacha / rand_core | crates.io | 2018/2017 | 333M / 492M | github.com/rust-random/rand | OK | Approved |
| backhand | crates.io | 2023 | 111k | github.com/wcampbell0x2a/backhand | OK | Approved |
| tempfile | crates.io | 2015 | 204M | github.com/Stebalien/tempfile | OK | Approved |
| tracing / tracing-subscriber | crates.io | 2017/2019 | 216M / 159M | github.com/tokio-rs/tracing | OK | Approved |
| rustix | crates.io | 2021 | 288M | github.com/bytecodealliance/rustix | OK | Approved |
| vsock | crates.io | 2019 | 4.0M | github.com/rust-vsock/vsock-rs | OK | Approved |
| proptest / insta / trybuild | crates.io | 2017/2019/2019 | 55M / 31M / 17M | proptest-rs / mitsuhiko / dtolnay | OK | Approved |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none. The `gsd-tools query package-legitimacy check --ecosystem crates` seam returned `OK` for all 24.
**Build scripts:** Rust has no `postinstall`. The equivalent risk is `build.rs`, which the `[bans.build]` allowlist controls. The 11 build scripts newly added to the allowlist were skimmed:
- `msb_krun_hvf` only links `Hypervisor.framework`.
- `zmij`, `rustversion`, and `radium` probe the rustc version or target.
- `target-tuple` writes a generated file into `OUT_DIR`.
- None makes a network call or writes outside `OUT_DIR`.

## Architecture Patterns

### System Architecture Diagram (how a change flows through the gate)

```
 developer edit (src/*.rs, crates/*/Cargo.toml, Cargo.lock)
        │
        ├─► cargo fmt --all --check ──────────────► rustfmt (style_edition from edition 2024)
        │
        ├─► cargo clippy --workspace --all-targets -- -D warnings
        │        │  Cargo injects [workspace.lints] as -F/-D/-W/-A flags for each member
        │        │  that has `[lints] workspace = true` (other members get none)
        │        ├─► rustc lints: unsafe_code (forbid), missing_docs, unfulfilled_lint_expectations
        │        ├─► clippy lints: groups + unwrap/expect/indexing + allow_attributes*
        │        └─► clippy.toml lookup: CARGO_MANIFEST_DIR, then parents. First file wins.
        │               crates/minos-core/clippy.toml → disallowed-methods/types (core only)
        │
        ├─► cargo test --workspace ──────────────► unit, integration, and doc tests (no rustdoc lint checks)
        │
        └─► cargo deny check  (reads Cargo.lock + cargo metadata for both [graph] targets)
                 ├─► advisories  (fetches RustSec DB; `cargo deny --offline check` uses the cache)
                 ├─► bans        (C crates, layering wrappers, anyhow/msb_krun wrappers,
                 │                wildcards, workspace-dependencies, build-script allowlist)
                 ├─► licenses    (allowlist)
                 └─► sources     (crates.io only; git needs rev and an allow-git entry)
```

### Recommended Project Structure
```
Cargo.toml              # [workspace], [workspace.package], [workspace.lints], [workspace.dependencies]
Cargo.lock              # committed
deny.toml
rust-toolchain.toml
rustfmt.toml            # optional: style_edition = "2024"
.gitignore              # /target  (no .gitignore exists yet; without it target/ gets committed)
LICENSE-APACHE
LICENSE-MIT
crates/
  minos-core/          Cargo.toml, clippy.toml, src/lib.rs
  minos-store/         Cargo.toml, src/lib.rs
  minos-sandbox/       Cargo.toml, src/lib.rs
  minos-backend-krun/  Cargo.toml, src/lib.rs
  minos-vmm/           Cargo.toml, src/main.rs
  minos-guest/         Cargo.toml, src/main.rs
  minos-propose/       Cargo.toml, src/lib.rs
  minos-runner/        Cargo.toml, src/lib.rs
  minos-loop/          Cargo.toml, src/lib.rs
  minos-cli/           Cargo.toml ([[bin]] name = "minos"), src/main.rs
```
There is no `xtask/`, no `.cargo/config.toml` (the musl linker setting is Phase 5), no root `clippy.toml`, and no `.github/`.

### Stub dependency map (verified: this exact graph passes all four commands)
| Crate | Shape | Internal deps | External deps | Dev deps |
|-------|-------|---------------|---------------|----------|
| minos-core | lib | — | serde, serde_json, toml, thiserror, sha2, base16ct, diffy, rand_chacha, rand_core | proptest, insta |
| minos-store | lib | core | serde, serde_json, thiserror, tempfile, base16ct | proptest |
| minos-sandbox | lib | core | thiserror, tar | trybuild |
| minos-backend-krun | lib | core, sandbox | thiserror, serde_json, tempfile, backhand | — |
| minos-vmm | bin | — | msb_krun, serde, serde_json, thiserror (optionally tracing) | — |
| minos-guest | bin | core | thiserror, tar; `[target.'cfg(target_os = "linux")'.dependencies]` rustix, vsock | — |
| minos-propose | lib | core | serde, serde_json, thiserror | insta |
| minos-runner | lib | core, store, sandbox | thiserror | — |
| minos-loop | lib | core, store, propose | thiserror | — |
| minos-cli | bin `minos` | core, store, sandbox, backend-krun, propose, runner, loop | clap, anyhow, tracing, tracing-subscriber, ignore, regex, tar, sha2, toml, serde | — |

Every `[workspace.dependencies]` entry must be used by at least one crate. Otherwise `[bans.workspace-dependencies] unused = "deny"` fails (verified). The table above uses all 24.

**Where backhand goes (discretion):** it builds the krun root image, so `minos-backend-krun` is the natural owner. If Phase 5 moves environment building into `minos-cli`, move the dependency then. **Whether minos-vmm needs core:** the stub has no internal dependencies. If Phase 5 puts `VmmConfig` in core, add `"minos-vmm"` to core's `wrappers`.

### Pattern 1: one `deny` entry per internal crate (allowed direct dependents)
**What:** `wrappers` lists the crates allowed to depend on the banned crate *directly*. Any other direct dependent fails. Transitive paths are not followed. Verified: with only `minos-sandbox` banned, `minos-loop -> minos-runner -> minos-sandbox` passed. Adding a `minos-runner` entry made the `loop -> runner` edge fail.
**When:** for every workspace library crate, plus `anyhow` (CLI only) and `msb_krun` (vmm only).
**Effect:** the whole ARCHITECTURE.md DAG becomes data. `core -> anything internal` fails, because core appears in no `wrappers` list. `loop -> sandbox`, `loop -> runner`, `loop -> backend-krun`, and `propose -> sandbox` all fail. Dev-dependency edges are included in cargo-deny's graph (`exclude-dev` defaults to false) and also fail (verified with `minos-loop [dev-dependencies] minos-sandbox`).

### Pattern 2: path dependencies carry an exact version
`minos-core = { path = "crates/minos-core", version = "=0.1.0" }` in `[workspace.dependencies]`. Without `version`, cargo-deny treats a path dependency as a wildcard and `wildcards = "deny"` fails every internal edge (verified: 8 errors). `allow-wildcard-paths = true` also works, but only for `publish = false` crates. Adding the version keeps `wildcards = "deny"` with no exception, fits D-13, and is what crates.io would require later.

### Anti-Patterns to Avoid
- **A `reason = "..."` key in a `[workspace.lints]` entry.** Cargo prints `unused manifest key: lints.clippy.<lint>.reason` in every crate and ignores it. Put the reason in a TOML comment.
- **A group without `priority = -1`.** `clippy::lint_groups_priority` is correctness (deny) and fails: "lint group `pedantic` has the same priority (0) as a lint".
- **`[graph] all-features = true` in deny.toml.** It would turn on backhand's default `zstd`/`xz` features (C) and msb_krun `net` in the checked graph. Keep it `false` so the graph matches the build.
- **Adding an x86_64 triple to `[graph] targets` without a plan.** `msb_krun_vmm` depends on `bzip2`/`zstd` (and so `cc`, `bzip2-sys`, `zstd-sys`) under `cfg(target_arch = "x86_64")`. The `cc` and `zstd-sys` bans would then fail. That is correct behavior for the GPU milestone, but it is a surprise to know about.
- **A root `clippy.toml` assumed to apply to core.** Verified: with a root `clippy.toml` banning `std::env::var`, `minos-store` failed and `minos-core` passed, because core's own file replaces the root one.
- **`[lints.clippy]` overrides inside a crate.** Cargo rejects the combination: "cannot override `workspace.lints` in `lints`, either remove the overrides or `lints.workspace = true`". That is good: no crate can locally weaken a lint while it still inherits.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Layering check | `cargo metadata` + jq/script | `deny.toml` `[bans] deny` with `wrappers` | D-01/D-03. Verified to catch normal and dev edges |
| `allow` ban | grep for `allow(clippy` | `clippy::allow_attributes` + `allow_attributes_without_reason` | D-02. Caveat in Open Question 1 |
| Centralized pins | review convention | `[bans.workspace-dependencies]` `duplicates = "deny"`, `unused = "deny"` | Verified: a direct `serde_json = "=1.0.151"` in a member gives `workspace-duplicate`. An unused workspace dependency gives `unused-workspace-dependency` |
| Build-script audit | manual list | `[bans.build] allow-build-scripts` | Reports every unlisted `build.rs` with its path and checksum |
| Toolchain pin | README instruction | `rust-toolchain.toml` | rustup honors it automatically |

**Key insight:** every policy in this phase already has a native Cargo, clippy, or cargo-deny knob. The only gaps are the three in Open Questions 1 to 3, and none of them has a native knob on stable 1.97.1.

## Common Pitfalls

### Pitfall 1: STACK.md's deny.toml does not pass the real graph
**What goes wrong:** `cargo deny check` fails with 11 `build-script-not-allowed` errors, a `rejected` error for BSL-1.0, 8 `wildcard` errors, and `license-not-encountered` warnings for MPL-2.0 and CC0-1.0.
**Why:** STACK's probe covered msb_krun only, not the full D-14 table, and it left versions off the path dependencies.
**How to avoid:** use the deny.toml in Code Examples. It was derived from `cargo metadata --filter-platform` for both triples.
**Warning signs:** any `build-script-not-allowed` after a dependency change is a review point, not something to add to the list automatically.

### Pitfall 2: `multiple_crate_versions` fails clippy on stub crates
**What goes wrong:** `error: multiple versions for dependency syn: 2.0.119, 3.0.6` and others, from the `cargo` group under `-D warnings`. Clippy's help text even suggests `#[allow(clippy::multiple_crate_versions)]`, which the gate forbids.
**How to avoid:** `multiple_crate_versions = "allow"` in `[workspace.lints.clippy]`, with a comment. cargo-deny `multiple-versions = "warn"` keeps the duplicates visible. The current duplicates come from msb_krun (`bitflags` 1, `miniz_oxide` 0.8, `nix` 0.30, `syn` 3 via `async-trait`), proptest (`rand_core`/`rand_chacha` 0.9), and `getrandom` 0.3/0.4.

### Pitfall 3: a lint name typo silently disables the lint
**What goes wrong:** `unwarp_used = "deny"` gives `warning[E0602]: unknown lint: clippy::unwarp_used ... the unknown_lints lint ignores -D warnings`. clippy exits 0. Adding `unknown_lints = "deny"` to `[workspace.lints.rust]` does not change this (verified).
**How to avoid:** the D-11 violation proofs are the real check that each security lint is active. The planner should also add an acceptance criterion that clippy output contains no `unknown lint`.
**Similar:** a typo in a `std::` path in `clippy.toml` gives only `warning: ... does not refer to a reachable type` and exits 0. Paths into crates core does not depend on (`rand::thread_rng`, `hashbrown::HashMap`) are silently ignored.

### Pitfall 4: a crate without `[lints] workspace = true` escapes everything
**What goes wrong:** verified. With the `[lints]` table removed from `minos-loop`, a `s.parse().unwrap()` passed `cargo clippy -- -D warnings` (rc=0).
**How to avoid:** every crate manifest carries `[lints]\nworkspace = true`. The plan's verification should include a one-time check that all 10 crate manifests contain it. See Open Question 2 for an optional native backstop.

### Pitfall 5: rustdoc lints are not part of the four commands
**What goes wrong:** verified. A doc comment linking `` [`DoesNotExist`] `` passes `cargo clippy` and `cargo test`. Only `cargo doc --no-deps` fails ("unresolved link").
**How to avoid:** see Open Question 3.

### Pitfall 6: `#[expect]` that does not fire fails the gate
**What goes wrong:** `#[expect(clippy::indexing_slicing, reason = "...")]` on code that does not index gives `error: this lint expectation is unfulfilled` under `-D warnings`. That is the intended behavior, and it keeps expectations honest.
**Also:** `#[expect]` cannot be used on a `forbid` lint: `error[E0453]: expect(unsafe_code) incompatible with previous forbid`. A reasonless `#[expect(...)]` fails `allow_attributes_without_reason` ("`expect` attribute without specifying a reason").

### Pitfall 7: violation proofs that leave residue
**What goes wrong:** the git and registry proofs rewrite `Cargo.lock`. The registry proof needs a temporary `.cargo/config.toml`. Forgetting to revert either one poisons later commits.
**How to avoid:** revert `Cargo.lock`, the touched `Cargo.toml`, and any `.cargo/` after each proof, then re-run the full gate green before committing.

### Pitfall 8: `core -> sandbox` as a normal dependency is a Cargo cycle, not a deny error
**What goes wrong:** the D-11 proof "dependency from minos-core on minos-sandbox" put under `[dependencies]` makes every cargo command fail with `error: cyclic package dependency: package minos-core ... depends on itself`. That includes `cargo deny check`, through `cargo metadata`. It still proves the gate fails, but the error is not the ban.
**How to avoid:** prove the core edge as a dev-dependency (`[dev-dependencies] minos-sandbox.workspace = true` in core). Cargo allows dev cycles, and cargo-deny reports `error[banned]`. Prove the loop edge as a normal dependency (`minos-loop -> minos-sandbox`), and record the cycle error as well.

## Code Examples

All verified by running the four commands on a scratch workspace with this exact content.

### `rust-toolchain.toml`
```toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy"]
profile = "minimal"
# targets: add "aarch64-unknown-linux-musl" in Phase 5 (D-09). The musl std is not installed today.
```

### Root `Cargo.toml`
```toml
[workspace]
resolver = "3"          # edition 2024 default; MSRV-aware: incompatible-rust-versions = "fallback"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.97.1"  # patch versions are accepted
license = "Apache-2.0 OR MIT"
publish = false          # v1 publishes nothing; also makes clippy::cargo_common_metadata skip the crate

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
# Groups. priority -1 so the single-lint entries below (priority 0) override them.
correctness = { level = "deny", priority = -1 }
suspicious  = { level = "warn", priority = -1 }
complexity  = { level = "warn", priority = -1 }
perf        = { level = "warn", priority = -1 }
style       = { level = "warn", priority = -1 }
pedantic    = { level = "warn", priority = -1 }
cargo       = { level = "warn", priority = -1 }
# Locked by CLAUDE.md
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"
# D-02 / GATE-05: only #[expect(..., reason = "...")] may suppress a lint
allow_attributes = "deny"
allow_attributes_without_reason = "deny"
# rules/lint-unsafe-doc.md (moot while unsafe_code is forbid, but kept as defense in depth)
undocumented_unsafe_blocks = "deny"
# Carve-outs. Cargo rejects a `reason` key, so reasons are comments.
# pedantic, rules/lint-pedantic-selective.md: an arbitrary line-count threshold says nothing about correctness
too_many_lines = "allow"
# pedantic, rules/lint-pedantic-selective.md: domain pairs such as base/baseline and exp/expr are intentional
similar_names = "allow"
# pedantic, rules/lint-pedantic-selective.md: config and capability structs legitimately carry several flags
struct_excessive_bools = "allow"
# cargo: msb_krun, proptest, and backhand pull duplicate versions we cannot unify; cargo-deny multiple-versions = "warn" reports them
multiple_crate_versions = "allow"

[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"   # enforced only by `cargo doc` (see Open Question 3)

[workspace.dependencies]
minos-core         = { path = "crates/minos-core", version = "=0.1.0" }
minos-store        = { path = "crates/minos-store", version = "=0.1.0" }
minos-sandbox      = { path = "crates/minos-sandbox", version = "=0.1.0" }
minos-backend-krun = { path = "crates/minos-backend-krun", version = "=0.1.0" }
minos-propose      = { path = "crates/minos-propose", version = "=0.1.0" }
minos-runner       = { path = "crates/minos-runner", version = "=0.1.0" }
minos-loop         = { path = "crates/minos-loop", version = "=0.1.0" }
serde        = { version = "=1.0.229", features = ["derive"] }
serde_json   = "=1.0.151"
toml         = "=1.1.8"
thiserror    = "=2.0.21"
anyhow       = "=1.0.104"
clap         = { version = "=4.6.7", features = ["derive"] }
sha2         = "=0.11.0"
base16ct     = { version = "=1.0.0", features = ["alloc"] }
diffy        = { version = "=0.5.2", default-features = false, features = ["std"] }
tar          = { version = "=0.4.46", default-features = false }
ignore       = "=0.4.33"
regex        = "=1.13.1"
rand_chacha  = { version = "=0.10.0", default-features = false }
rand_core    = "=0.10.1"
rustix       = { version = "=1.1.5", features = ["fs", "mount", "process", "system"] }
vsock        = "=0.5.4"
msb_krun     = { version = "=0.1.41", default-features = false, features = ["blk"] }
backhand     = { version = "=0.25.5", default-features = false, features = ["gzip"] }
tempfile     = "=3.27.0"
tracing      = "=0.1.44"
tracing-subscriber = "=0.3.23"
proptest     = "=1.11.0"
insta        = { version = "=1.49.0", features = ["json", "toml"] }
trybuild     = "=1.0.122"
```
**Pedantic carve-out choice (discretion):** `rules/lint-pedantic-selective.md` also lists `missing_errors_doc`, `missing_panics_doc`, and `must_use_candidate`. **Keep those three enabled.** `rules/err-doc-errors.md` sets `missing_errors_doc = "warn"`, `rules/doc-panics-section.md` requires `# Panics`, and `rules/api-must-use.md` sets `must_use_candidate = "warn"`. Carving them out would leave those rules unenforced. `module_name_repetitions` is in the **restriction** group in clippy 1.97 (not pedantic), so it needs no entry [VERIFIED: rust-clippy rust-1.97.0 lint index].

### Member `Cargo.toml` (library example: `crates/minos-loop/Cargo.toml`)
```toml
[package]
name = "minos-loop"
description = "Local generation driver for Minos"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
minos-core.workspace = true
minos-store.workspace = true
minos-propose.workspace = true
thiserror.workspace = true
```
For `minos-cli`, add `[[bin]]\nname = "minos"\npath = "src/main.rs"`. For `minos-guest`, put rustix and vsock under `[target.'cfg(target_os = "linux")'.dependencies]` (verified: cargo-deny still sees them through the musl graph target, and the unused-dependency check passes). Plain `[dependencies]` also compiles on macOS (verified). The target table is recommended because the guest only ever runs on Linux, and Phase 5 guest code calling `rustix::mount` must be cfg-gated either way.

### Stub source files
```rust
// crates/<lib>/src/lib.rs (missing_docs fires on a lib crate root with no //! doc: "missing documentation for the crate")
//! Minos core: pure types, validation, policy, and reports. No I/O.
```
```rust
// crates/<bin>/src/main.rs (missing_docs does NOT fire on a bin root (verified), but keep //! per rules/doc-module-inner.md)
//! `minos` command-line interface.

fn main() {}
```
Nothing else is needed. `cargo test --workspace` reports `0 passed` for each crate and exits 0.

### `crates/minos-core/clippy.toml`
```toml
# Replaces (does not merge with) any ancestor clippy.toml. Clippy uses the first file found
# walking up from CARGO_MANIFEST_DIR.
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "minos-core has no clock: time arrives as an argument" },
  { path = "std::time::Instant::now", reason = "minos-core has no clock: time arrives as an argument" },
]
disallowed-types = [
  { path = "std::collections::HashMap", reason = "order must be stable: use BTreeMap" },
  { path = "std::collections::HashSet", reason = "order must be stable: use BTreeSet" },
]
```
`disallowed_methods` and `disallowed_types` are in the **style** group (warn by default), so `-D warnings` makes them fail [VERIFIED: rust-clippy rust-1.97.0 lint index and local run]. Optional extra core bans (Pitfall 17): `std::env::var`, `std::env::var_os`, `std::thread::sleep`. Each must be a real `std` path, or it only warns.

### `deny.toml` (passes the D-14 graph; every error path verified)
```toml
[graph]
targets = ["aarch64-apple-darwin", "aarch64-unknown-linux-musl"]
all-features = false   # never true: it would pull backhand zstd/xz (C) into the checked graph

[advisories]
# Defaults fetch the RustSec DB over the network. `cargo deny --offline check` uses the cached copy.
unmaintained = "workspace"
yanked = "deny"

[licenses]
# Exactly what the v1 graph needs (each entry was removed in turn to confirm it is required):
#   BSD-3-Clause: msb_krun_* sub-crates ("Apache-2.0 AND BSD-3-Clause"), vm-fdt, virtio-bindings
#   BSL-1.0:      xxhash-rust (via backhand)
#   ISC:          libloading (via msb_krun)
#   Unicode-3.0:  unicode-ident
#   Zlib:         foldhash (via diffy -> hashbrown), zlib-rs (via flate2)
allow = ["Apache-2.0", "MIT", "BSD-3-Clause", "BSL-1.0", "ISC", "Unicode-3.0", "Zlib"]

[bans]
multiple-versions = "warn"
wildcards = "deny"
deny = [
  # No C in our tree (CLAUDE.md). `cc` must get a wrapper when the rustls provider arrives (AWS milestone).
  { crate = "openssl-sys", reason = "no OpenSSL" },
  { crate = "openssl", reason = "no OpenSSL" },
  { crate = "native-tls", reason = "no OpenSSL" },
  { crate = "git2", reason = "no libgit2" },
  { crate = "libgit2-sys", reason = "no libgit2" },
  { crate = "gix", reason = "no git in the trusted path" },
  { crate = "zstd-sys", reason = "C; backhand uses gzip only" },
  { crate = "liblzma-sys", reason = "C; backhand uses gzip only" },
  { crate = "cc", reason = "no C compiled in our tree" },
  # Who may link what
  { crate = "anyhow", wrappers = ["minos-cli"], reason = "anyhow only at the CLI main (rules/err-anyhow-app.md)" },
  { crate = "msb_krun", wrappers = ["minos-vmm"], reason = "only minos-vmm links the VMM (KRUN-01)" },
  # Crate layering (GATE-04). wrappers = the crates allowed to depend on it DIRECTLY.
  # wrappers does not follow transitive paths, so every internal crate needs its own entry.
  { crate = "minos-core", wrappers = ["minos-store", "minos-sandbox", "minos-backend-krun", "minos-guest", "minos-propose", "minos-runner", "minos-loop", "minos-cli"], reason = "core depends on no internal crate" },
  { crate = "minos-store", wrappers = ["minos-runner", "minos-loop", "minos-cli"], reason = "layering" },
  { crate = "minos-sandbox", wrappers = ["minos-backend-krun", "minos-runner", "minos-cli"], reason = "core and loop never see sandbox types" },
  { crate = "minos-backend-krun", wrappers = ["minos-cli"], reason = "only the CLI picks a concrete backend" },
  { crate = "minos-propose", wrappers = ["minos-loop", "minos-cli"], reason = "layering" },
  { crate = "minos-runner", wrappers = ["minos-cli"], reason = "loop reaches the runner only through its JobRunner trait" },
  { crate = "minos-loop", wrappers = ["minos-cli"], reason = "layering" },
]

[bans.workspace-dependencies]
duplicates = "deny"               # a member may not redeclare a workspace dep without `workspace = true`
include-path-dependencies = true
unused = "deny"                   # every [workspace.dependencies] entry must be used

[bans.build]
# Every crate in the aarch64-apple-darwin + aarch64-unknown-linux-musl graph that has a build.rs.
allow-build-scripts = [
  { crate = "anyhow" }, { crate = "crc32fast" }, { crate = "crossbeam-deque" }, { crate = "crossbeam-epoch" },
  { crate = "crossbeam-utils" }, { crate = "getrandom" }, { crate = "kvm-ioctls" },      # kvm-ioctls: linux-musl only (msb_krun)
  { crate = "libc" }, { crate = "memoffset" }, { crate = "msb-imago" }, { crate = "msb_krun_hvf" },  # msb_krun_hvf: links Hypervisor.framework
  { crate = "nix" }, { crate = "num-traits" }, { crate = "proc-macro2" }, { crate = "quote" }, { crate = "radium" },
  { crate = "rustix" }, { crate = "rustversion" }, { crate = "serde" }, { crate = "serde_core" }, { crate = "serde_json" },
  { crate = "target-tuple" }, { crate = "thiserror" }, { crate = "zerocopy" }, { crate = "zmij" },
]
executables = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
required-git-spec = "rev"
allow-git = []   # the AWS milestone adds aws-durable-execution-sdk-rust
```
**Key validity (0.20.2, verified):** `[graph] targets`/`all-features`, `[advisories] unmaintained`/`yanked`/`unsound`/`ignore`/`db-path`/`db-urls`, `[licenses] allow`/`confidence-threshold`/`exceptions`, `[bans] multiple-versions`/`wildcards`/`allow-wildcard-paths`/`deny`, `[bans.workspace-dependencies]`, `[bans.build] allow-build-scripts`/`executables`, and `[sources] unknown-registry`/`unknown-git`/`required-git-spec`/`allow-git` are all accepted. The legacy keys `[licenses] unlicensed`, `copyleft`, and `allow-osi-fsf-free`, and `[advisories] vulnerability` and `notice`, each give `error[deprecated]: this key has been removed`. `allow-build-scripts` entries accept `{ crate = "name" }`, as verified. The docs example uses `{ name = ... }`.

### `rustfmt.toml` (optional, discretion)
```toml
style_edition = "2024"
```
`cargo fmt` already passes the package edition, so this file only matters to editors that call `rustfmt` directly. It passed `cargo fmt --all --check` (verified). Use `cargo fmt --all --check` as CLAUDE.md lists it. In a workspace where every crate is a member, `--all` adds nothing, but it matches the documented command.

### `.gitignore`
```
/target
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `[licenses] unlicensed`/`copyleft`/`allow-osi-fsf-free`, `[advisories] vulnerability`/`notice` | `allow` list only; advisories always error | cargo-deny PR #611 (0.14+) | Old keys are hard errors in 0.20.2 |
| `resolver = "2"` | `resolver = "3"` (MSRV-aware `fallback`) | Rust 1.84 / edition 2024 | `cargo generate-lockfile` printed "Locking 197 packages to latest Rust 1.97.1 compatible versions" |
| `#[allow(..)]` | `#[expect(.., reason = "..")]` + `clippy::allow_attributes` | `expect` stable 1.81; lint added 1.70 | Unfulfilled expectations fail under `-D warnings` |
| `module_name_repetitions` in pedantic | moved to restriction | before 1.97 | Not enabled by `pedantic`; no carve-out needed |

## Runtime State Inventory

Not applicable. This is a greenfield phase with no rename, refactor, or migration.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Stable Cargo 1.97.1 has no lint that rejects a member missing `[lints] workspace = true`. A `missing_lints_inheritance`-style Cargo lint exists only on nightly (`-Zcargo-lints`), if it exists at all | Open Question 2 | Low. If a stable lint exists, use it instead of the rustflags backstop |
| A2 | The repository will be published later as `minos-*` crates, and `publish = false` is right for v1 | Code Examples (root Cargo.toml) | Low. Flipping it later requires `readme`, `keywords`, `categories`, and `repository` per crate (the clippy `cargo_common_metadata` output listed all three missing fields) |

## Open Questions (each needs a user decision; none blocks writing the plan)

1. **The inner `#![allow(clippy::x, reason = "...")]` gap (GATE-05 says "no `allow(clippy::...)` appears anywhere").**
   - What we know: `allow_attributes` only lints outer `#[allow]`. Clippy's own docs say: "This lint only warns outer attributes (`#[allow]`), as inner attributes (`#![allow]`) are usually used to enable or disable lints on a global scale". `allow_attributes_without_reason` catches a reasonless `#![allow]`. Verified: `#![allow(clippy::unwrap_used, reason = "...")]` plus an `unwrap()` passed clippy.
   - Option A: accept and document the gap. The D-11 proof (outer `#[allow(clippy::...)]`) passes as written.
   - Option B: raise `unwrap_used`, `expect_used`, and `indexing_slicing` to `"forbid"`. Verified: `#![allow(clippy::unwrap_used, reason=..)]` then gives `error[E0453]: allow(clippy::unwrap_used) incompatible with previous forbid`. The trade-off is that `#[expect]` is impossible for those three as well. That is stricter than CLAUDE.md's `deny`, and still a design-of-record edit.
   - Recommendation: ask the user. If they choose B, update CLAUDE.md's code-rules block in the same change.

2. **A member that leaves out `[lints] workspace = true` escapes all lints.**
   - Verified: unwrap passed. Cargo rejects a partial override but not a missing table.
   - Option A: a one-time check in VERIFICATION.md, plus a plan acceptance criterion that every `crates/*/Cargo.toml` contains it.
   - Option B: a Cargo-native backstop in `.cargo/config.toml`, `[build] rustflags = ["-Funsafe_code", "-Dclippy::unwrap_used", "-Dclippy::expect_used", "-Dclippy::indexing_slicing"]`. Verified: this caught the unwrap in a crate with no `[lints]`, and a plain `cargo build` was unaffected. Costs: the lints are duplicated in two places, a `RUSTFLAGS` env var silently overrides `build.rustflags`, and D-01 does not list `.cargo/config.toml`.
   - Recommendation: Option A for Phase 1. Raise B with the user.

3. **`broken_intra_doc_links = "deny"` (D-12) is not enforced by the four commands.**
   - Verified: only `cargo doc --workspace --no-deps` fails on a broken link.
   - Option: add `cargo doc --workspace --no-deps` as a fifth plain cargo command. That is still no script, but it changes CLAUDE.md's Commands section and ROADMAP's criterion 1. Otherwise, document that the rustdoc table takes effect only when docs are built.
   - Recommendation: ask the user. If accepted, update CLAUDE.md Commands in the same change.

4. **`publish = false`.** Recommended for v1. It requires no repository URL, and none exists (no git remote). CLAUDE.md's "Publish as `minos-*`" stays true for a later milestone.

5. **Test code and `unwrap`.** In Phase 3, `#[cfg(test)]` code that unwraps fails clippy. clippy.toml has `allow-unwrap-in-tests`, `allow-expect-in-tests`, and `allow-indexing-slicing-in-tests` (default false). Because core's `clippy.toml` replaces any root file, such a setting would have to go in both a root `clippy.toml` and core's file. This is not a Phase 1 decision. Flag it for Phase 3.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc/cargo 1.97.1 | GATE-01 | ✓ | 1.97.1 (8bab26f4f 2026-07-14) | — |
| clippy, rustfmt for 1.97.1 | GATE-01/02 | ✓ | clippy 0.1.97, rustfmt 1.9.0 | — |
| cargo-deny | GATE-02/03/04 | ✓ | 0.20.2 | — |
| Network (crates.io, RustSec DB, github.com) | first lockfile, `cargo deny check` advisories, git and registry proofs | ✓ (all fetched during research) | — | `cargo deny --offline check` uses the cached DB at `~/.cargo/advisory-dbs` |
| `aarch64-unknown-linux-musl` std | not needed in Phase 1 (D-09). cargo-deny's `[graph] targets` does not need it installed (verified) | ✗ (only `aarch64-unknown-linux-gnu` installed) | — | Phase 5 adds it |
| libkrunfw | not needed in Phase 1. msb_krun compiles without it, because it is `dlopen`ed at runtime | ✗ | — | Phase 2 installs it |
| cargo-insta | not needed in Phase 1 (no snapshots yet) | ✓ | 1.48.0 (STACK says 1.49.0, which matches `insta =1.49.0`) | Upgrade before Phase 3 golden files |
| cargo-nextest | optional | ✓ | 0.9.85 (old) | Not used by the gate |

**Missing dependencies with no fallback:** none for Phase 1.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | The gate commands themselves. The stubs contain no tests. `cargo test` passes with 0 tests per crate |
| Config files | `Cargo.toml` `[workspace.lints]`, `crates/minos-core/clippy.toml`, `deny.toml`, `rust-toolchain.toml` |
| Quick run command | `cargo clippy --workspace --all-targets -- -D warnings && cargo deny check bans sources` |
| Full suite command | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check` (about 13s cold, under 2s warm) |

### Phase Requirements → Test Map
VERIFICATION.md must capture the **literal command and its stderr** for every row (user's instruction: "capture the actual failing command text, not a summary"). Procedure for each violation:
1. Apply the edit.
2. Run the single command with `2>&1`, and record the command, the exit code, and the key error block.
3. Revert (`git restore` the touched files, plus `Cargo.lock` and any `.cargo/`).
4. After all proofs, re-run the full suite green.

| Req ID | Behavior / violation | Edit to apply (then revert) | Command that must fail | Expected output (verified on 1.97.1 / deny 0.20.2) |
|--------|----------------------|-----------------------------|------------------------|---------------------------------------------------|
| GATE-01/02 | Clean gate passes | — | full suite | `advisories ok, bans ok, licenses ok, sources ok`; clippy `Finished`; every `test result: ok`; fmt exit 0 |
| GATE-02 | Lockfile committed | — | `git ls-files Cargo.lock` | prints `Cargo.lock` |
| GATE-01 | `unsafe` block | in any lib: `pub fn f(x:&[u8])->u8{ unsafe { *x.get_unchecked(0) } }` | `cargo clippy --workspace --all-targets -- -D warnings` (also fails `cargo build`) | `error: usage of an unsafe block` … `note: requested on the command line with -F unsafe-code` |
| GATE-01 | `unwrap` | `s.parse::<u8>().unwrap()` | same | `error: used unwrap() on a Result value` … `-D clippy::unwrap-used` |
| GATE-01 | `expect` | `s.parse::<u8>().expect("n")` | same | `error: used expect() on a Result value` … `-D clippy::expect-used` |
| GATE-01 | slice index | `x[0]` | same | `error: indexing may panic` … `-D clippy::indexing-slicing` |
| GATE-05 | `#[allow(clippy::...)]` | `#[allow(clippy::indexing_slicing)]` on a fn | same | `error: #[allow] attribute found` (`-D clippy::allow-attributes`) **and** `` error: `allow` attribute without specifying a reason `` |
| GATE-05 | positive control: expect with reason passes | `#[expect(clippy::indexing_slicing, reason = "…")]` on a fn that indexes | same | passes (exit 0). The same attribute on a fn that does not index gives `error: this lint expectation is unfulfilled` |
| GATE-05 | `SystemTime::now` in core | `std::time::SystemTime::now()` in `minos-core` | same | `` error: use of a disallowed method `std::time::SystemTime::now` `` + `note: minos-core has no clock…` |
| GATE-05 | `HashMap` in core | `std::collections::HashMap<u8,u8>` in `minos-core` | same | `` error: use of a disallowed type `std::collections::HashMap` `` + `note: order must be stable: use BTreeMap` |
| GATE-03 | git dependency without `rev` | `cfg-if = { git = "https://github.com/rust-lang/cfg-if", branch = "main" }` in `minos-store` `[dependencies]` | `cargo deny check sources` | `error[git-source-underspecified]: 'git' source is underspecified, expected 'rev', but found 'branch'` (plus `source-not-allowed`) |
| GATE-03 | unknown git source | same repo with `rev = "14432a28e180ec717933714aed7828fc7ed2fd29"` (cfg-if HEAD on 2026-10-09) | `cargo deny check sources` | `error[source-not-allowed]: detected 'git' source not explicitly allowed` |
| GATE-03 | unknown registry | temporary `.cargo/config.toml`: `[registries.mirror]\nindex = "sparse+https://rsproxy.cn/index/"`, and `cfg-if = { version = "=1.0.4", registry = "mirror" }` | `cargo deny check sources` | `error[source-not-allowed]: detected 'registry' source not explicitly allowed` (lock line `cfg-if 1.0.4 sparse+https://rsproxy.cn/index/`) |
| GATE-03 | build script not allowlisted | `httparse = "=1.10.1"` in `minos-store` `[dependencies]` | `cargo deny check bans` | `error[build-script-not-allowed]: crate 'httparse = 1.10.1' has a build script but is not allowed to have one` |
| GATE-04 | loop → sandbox | `minos-sandbox.workspace = true` in `minos-loop` `[dependencies]` | `cargo deny check bans` | `error[banned]: crate 'minos-sandbox = 0.1.0' is explicitly banned` with the `reason` |
| GATE-04 | loop → runner (transitive path to sandbox) | `minos-runner.workspace = true` in `minos-loop` | `cargo deny check bans` | `error[banned]: crate 'minos-runner = 0.1.0' is explicitly banned` |
| GATE-04 | loop → backend | `minos-backend-krun.workspace = true` in `minos-loop` | `cargo deny check bans` | `error[banned]: crate 'minos-backend-krun = 0.1.0' is explicitly banned` |
| GATE-04 | core → sandbox (dev edge) | `[dev-dependencies] minos-sandbox.workspace = true` in `minos-core` | `cargo deny check bans` | `error[banned]: crate 'minos-sandbox = 0.1.0' is explicitly banned` |
| GATE-04 | core → sandbox (normal edge) | `minos-sandbox.workspace = true` under core `[dependencies]` | `cargo clippy --workspace` (and `cargo deny check`) | `error: cyclic package dependency: package minos-core v0.1.0 … depends on itself` |
| Pitfall 26 (bonus) | anyhow in a library | `anyhow.workspace = true` in `minos-store` | `cargo deny check bans` | `error[banned]: crate 'anyhow = 1.0.104' is explicitly banned` |
| KRUN-01 (bonus) | msb_krun outside vmm | `msb_krun.workspace = true` in `minos-backend-krun` | `cargo deny check bans` | `error[banned]: crate 'msb_krun = 0.1.41' is explicitly banned` |
| D-06 | every crate inherits lints | — | one-time `grep -L '^workspace = true' crates/*/Cargo.toml` | prints nothing |
| GATE-06 | clean after verification | — | `cargo clean` | `Removed N files` |

### Sampling Rate
- **Per task commit:** full suite (under 15s).
- **Per wave merge:** full suite plus a check that clippy output contains no `unknown lint`.
- **Phase gate:** full suite green, all D-11 proofs captured verbatim in VERIFICATION.md, the tree reverted and green again, then `cargo clean`.

### Wave 0 Gaps
- None. The phase *is* the test infrastructure. There are no fixture crates (D-10).

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no (the crate layering is an architectural access control, enforced by cargo-deny `wrappers`) | deny.toml |
| V5 Input Validation | no code yet | Phases 3+ |
| V6 Cryptography | no code yet (`sha2` declared only) | RustCrypto, never hand-rolled |
| V10 Malicious Code | **yes** | `unsafe_code = "forbid"`; build-script allowlist; `executables = "deny"`; C-crate bans |
| V14 Configuration / Dependencies | **yes** | Exact pins + committed lockfile; `[sources]` crates.io-only + `required-git-spec = "rev"`; RustSec advisories + `yanked = "deny"`; license allowlist |

### Known Threat Patterns for a Rust supply chain
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Typosquatted or slopsquatted crate | Spoofing | Exact pins, legitimacy seam (all OK), review of each new `build-script-not-allowed` |
| Malicious `build.rs` | Tampering / Elevation | `[bans.build] allow-build-scripts` (25 named crates), `executables = "deny"` |
| Moving git ref | Tampering | `required-git-spec = "rev"`, `unknown-git = "deny"`, `allow-git = []` |
| Alternative registry or mirror injection | Spoofing | `unknown-registry = "deny"` |
| C code entering the trusted tree | Tampering | bans on `cc`, `openssl*`, `git2`, `libgit2-sys`, `zstd-sys`, `liblzma-sys`; `all-features = false` |
| Lint escape (`allow`, missing `[lints]`, lint-name typo) | Tampering | `allow_attributes*`, verification of `[lints]` inheritance, the D-11 proofs; residual gaps in Open Questions 1 to 3 |

## Sources

### Primary (HIGH confidence)
- **Local probes on this machine (2026-10-09):** a scratch workspace with all 10 stubs and the D-14 table. The four commands were run and every D-11 violation applied. Outputs are quoted above. [VERIFIED: rustc 1.97.1, clippy 0.1.97, cargo-deny 0.20.2]
- crates.io API `/api/v1/crates/<name>/<version>` for all 24 pins: versions, features, MSRV, licenses, yank status. [VERIFIED]
- `cargo metadata --filter-platform` for `aarch64-apple-darwin` and `aarch64-unknown-linux-musl`: the build-script crate list. [VERIFIED]
- `gsd-tools query package-legitimacy check --ecosystem crates`: all OK.

### Secondary (MEDIUM confidence: official docs, cross-checked by the local runs)
- https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html: `wrappers` ("allows specific crates to have a direct dependency on the banned crate but denies all transitive dependencies on it"), `allow-wildcard-paths` (private crates only), `[bans.workspace-dependencies]`, and `[bans.build]`.
- https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html: lint groups and versions (`allow_attributes` restriction 1.70, inner-attribute exclusion text; `allow_attributes_without_reason` restriction 1.61; `disallowed_methods`/`disallowed_types` style; `multiple_crate_versions`/`cargo_common_metadata` cargo; `module_name_repetitions` restriction; `lint_groups_priority` correctness 1.78; the `cargo` group has 5 lints; `allow-unwrap-in-tests` and related config).
- https://doc.rust-lang.org/clippy/configuration.html: config lookup order (`CLIPPY_CONF_DIR`, then `CARGO_MANIFEST_DIR`, then the current directory, walking up to parents).
- https://doc.rust-lang.org/cargo/reference/resolver.html: resolver "3" means `incompatible-rust-versions = "fallback"`, needs Rust 1.84+, and is the edition 2024 default.

### Tertiary (LOW confidence)
- None used for any recommendation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. Every pin and feature was checked on crates.io and built.
- Architecture (deny layering, lints placement): HIGH. Every allowed and forbidden edge was run.
- Pitfalls: HIGH. Each one was reproduced locally. Open Questions 1 to 3 describe verified gaps, not guesses.

**Research date:** 2026-10-09
**Valid until:** 2026-11-08 for tool behavior. The build-script and license lists are valid until any dependency pin changes. Re-derive them with `cargo deny check` after each pin bump.
