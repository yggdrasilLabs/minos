# Minos

## What This Is

Minos is a secure, general-purpose lab for autonomous experiment loops, written in Rust. It generalizes the pattern in karpathy/autoresearch: propose a change, run it under a fixed budget, score it, keep or discard, repeat. A project can be ML training, a performance problem, or an open-ended design question. It is public open source from day one; the first milestone is a fully local loop on the author's macOS arm64 machine.

## Core Value

Untrusted experiment code runs in a hardware-virtualized sandbox with no network, cannot touch the judge that scores it, and nothing it produces reaches the user's machine without an acknowledged report. If the loop produces no improvement, that is acceptable. If isolation, judge integrity, or the review gate is weakened for convenience, the project has failed.

## Requirements

### Validated

- [x] Repo-wide gates: `cargo fmt --all --check`, clippy with `-D warnings` and the workspace lints (`unsafe_code`, `unwrap_used`, `expect_used`, `indexing_slicing`, all `forbid`), `cargo test`, `cargo deny check`, `cargo doc --no-deps`, committed `Cargo.lock` — Validated in Phase 1: Workspace and Gates (2026-10-09; see 01-GATE-EVIDENCE.md and 01-VERIFICATION.md)

### Active

- [ ] `minos-core` exists with validated types (IDs, `RelPath`, `Manifest`, `Diff`, `Metric`, `Outcome`), pure policy and routing, the ledger fold, and `build_report`, with property tests, golden hash vectors, and golden wire-format files
- [ ] `minos-sandbox` encodes roles as types: a `SandboxSpec<Candidate>` cannot receive judge data or a network device, and this is proven by compile-fail tests
- [ ] A libkrun backend on macOS arm64 launches a VM from a sealed `LaunchPlan`, with no network device, input delivered as read-only disk images, and output returned as a capped tar stream over vsock wrapped in `Untrusted<T>`
- [ ] `minos-guest` runs inside the VM: applies the base tarball plus diff with a pure-Rust patch crate, runs the manifest's argv commands under budget, and streams results out
- [ ] A local loop driver in the CLI process runs generations against a file-backed ledger and blob store, using the same pure core functions the Lambda orchestrator will use later
- [ ] A `ModelProvider` abstraction in `minos-propose` with one implementation that execs the user's own `claude` binary in print mode, text-only, and parses structured JSON output; Bedrock and API-key implementations are later variants of the same trait or enum
- [ ] Sweep tier (seeded sampling over declared params, no model) and `single_shot` tier (one model call returning several diffs) both route through `ProposerSpec::eligible`
- [ ] Greedy policy with `min_improvement`, tie-broken on `ExperimentId`, permutation-invariant
- [ ] `minos init`, `env build`, `check`, `run`, `status`, `log`, `show`, `report`, `adopt`, `stop` work for the local backend
- [ ] `minos check` judges the baseline several times and reports a noise floor
- [ ] `minos report` is a pure function of ledger and blobs, hashed, and `minos adopt` refuses without the acknowledged hash and only writes a patch file
- [ ] Cost is recorded as usage, never dollars, and displayed as one `approx. cost` figure from a user-editable price table
- [ ] A sample `optimize` project ships in the repo and the full loop runs on it end to end: `minos check`, `minos run --generations N`, `minos report`, `minos adopt`

- [ ] The rules in `rules/` are followed in all code

### Out of Scope

- AWS orchestration (durable Lambda functions, Lambda MicroVMs, DynamoDB ledger, S3 blobs, Bedrock provider) — deferred to a later milestone; the user wants local builds and runs first. The core is designed so these slot in without rewriting
- `planned` and `agentic` proposer tiers — deferred; the agentic tier needs the MCP bridge into the agent VM and is its own milestone
- GPU: the GPU daemon, Cloud Hypervisor with VFIO, EC2 instances — Cloud Hypervisor does not run on macOS and GPU work is build-order step 7
- Local Linux backends (libkrun on Linux, Landlock plus seccomp dev mode) — the dev machine is macOS; Linux support is a later milestone
- Running an off-the-shelf coding agent inside the VM — needs network egress and a credential within reach of untrusted text
- Price list API integration — `prices.toml` is edited by hand until checked
- Claims of "ironclad" isolation — the stated target is that escape requires a hypervisor zero-day, never only a mistake in our code

## Context

- `CLAUDE.md` in the repo root is the design of record and must be kept current. It describes the AWS-first architecture; this milestone reorders the build so the local backend and a CLI-process loop driver come first, and the Lambda pieces follow. `CLAUDE.md` needs an edit recording that reordering and the model provider decision.
- `rules/` holds ~180 Rust coding rules (naming, ownership, errors, async, API design, memory, perf, lint, testing). All code must follow them without exception.
- Toolchain on the dev machine: rustc 1.97.1, cargo 1.97.1, cargo-lambda 1.8.6, Claude Code 2.1.295, Homebrew 7.0.6. Hypervisor.framework is available (`kern.hv_support: 1`). libkrun is not installed; it is available from the `slp/krun` Homebrew tap, not core Homebrew. AWS CLI is installed but credentials were not confirmed in this shell.
- Model access verified against Claude Code docs on 2026-10-09: `claude -p` with `--tools ""`, `--output-format json`, `--json-schema`, `--model`, `--no-session-persistence`, and `--permission-mode dontAsk` gives a text-only structured call. The child environment must strip `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and `CLAUDE_CODE_USE_*`, since an API key always wins over the login in print mode. There is no Rust Agent SDK; the subprocess route is the documented path. Unverified and to be spiked: `--max-turns 1` behavior on a plain reply, exact usage field names in the JSON result, and whether `--tools ""` also removes the Agent and Skill tools.
- Anthropic's terms permit an end user to run the unmodified `claude` binary under their own subscription, and prohibit third-party products from routing requests through Pro or Max credentials on behalf of their users. The CLI provider therefore only ever execs the user's own install, never reads or stores credentials, and the README says that API key or Bedrock is the path for anything beyond personal use.
- libkrun is a C-ABI library. With `unsafe_code = "forbid"` in our crates, the binding must live in an external `-sys`/wrapper crate or the backend must drive a helper process. This is a research item for the backend phase, not a settled decision.
- Lambda snapshots capture memory, so `minos-guest` must not create per-run state at startup even in the local build; keep the guest identical across backends.

## Constraints

- **Security**: Never weaken an invariant to make something compile or pass. No escape hatches, `pub` fields, `Deref`, or temporary bypasses on `Untrusted<T>`, `SandboxSpec`, or `LaunchPlan`. Stop and ask if blocked
- **Tech stack**: Rust, edition 2024, workspace lints as in `CLAUDE.md`. No C in our own crates (rustls crypto provider excepted). No OpenSSL, no libgit2, no git in the trusted path. Commands are argv arrays. `BTreeMap`/`BTreeSet` wherever order affects output
- **Core purity**: `minos-core` has no async, AWS, filesystem, clock, or unseeded randomness. `SystemTime::now` is banned by a clippy `disallowed-methods` rule
- **Platform**: dev and first target is macOS arm64 with Hypervisor.framework. Local must be at least as strict as the Lambda design: disk images in, tar over vsock out, never a shared host directory, never mount a guest-written disk
- **Forward compatibility**: the file-backed ledger and blob store implement the same `Ledger` and `Blobs` traits the DynamoDB and S3 implementations will; wire formats carry a `v` tag and are guarded by golden files from the first commit
- **Supply chain**: `Cargo.lock` committed, `cargo deny check` on every build, git dependencies pinned by `rev`
- **Public**: open source from day one, so docs and README must be accurate and must not overclaim isolation

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Milestone v1 is local-only; AWS deferred | User wants to build and run locally first; the pure core makes the Lambda orchestrator a later consumer rather than a rewrite | — Pending |
| Local loop lives in the CLI process | No durable runtime locally; the loop is a plain driver over pure core functions, file-backed ledger and blobs | — Pending |
| libkrun on macOS via Hypervisor.framework is the first backend | Hardware virtualization is the default; the dev machine is macOS arm64; matches the `minos-backend-krun` crate | — Pending |
| `ModelProvider` abstraction with a `claude` CLI subprocess as the first implementation | Uses the user's Claude Code subscription with no API key; Bedrock and API-key providers are later implementations of the same abstraction | — Pending |
| CLI provider is the documented personal default | Terms allow an end user to run the unmodified binary under their own login; README directs anything beyond personal use to API key or Bedrock | — Pending |
| Sweep and single_shot tiers only in v1 | Sweep is fully offline; single_shot is a single text-only call; planned and agentic need more infrastructure | — Pending |
| Done means the full loop on a sample project | `check`, `run`, `report`, `adopt` end to end with every experiment in a VM | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-10-09 after Phase 1 (Workspace and Gates) completion*
