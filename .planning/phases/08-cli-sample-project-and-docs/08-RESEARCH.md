# Phase 8: CLI, Sample Project, and Docs - Research

**Researched:** 2026-10-28
**Domain:** Rust CLI composition over the Phase 7 driver, an in-VM Rust sample project, a canonical patch file, and honest security docs
**Confidence:** HIGH for the code seams and the patch tooling (source read and experiments on this Mac); MEDIUM for the in-VM toolchain numbers (Docker proxy, not the VM); LOW for the bytes-per-token constant (no official figure exists)

## Summary

The roadmap marked this phase "skip research". That was written before the 2026-10-28 discuss session, whose decisions rest on mechanisms nobody had verified. This research settled each one with a source read or a small experiment. Every experiment ran in the session scratchpad. No file under `crates/`, `rules/`, or `.planning/` other than this one was changed.

Four findings change what the planner must write.

1. **A Rust toolchain does work under the guest's stage environment, but only if the image is laid out for it.** PID 1 runs every stage as uid 64000 on a read-only root. It sets exactly `PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`, `HOME=/scratch/home`, `TMPDIR=/tmp`, and `LANG=C`. The official `rust:` image's `/usr/local/cargo/bin/cargo` is a rustup proxy that is not on that `PATH` and would need `RUSTUP_HOME`. The fix is two symlinks from `/usr/local/bin` into the toolchain directory, with no rustup involved. With that layout, a std-only release build succeeded in a Docker proxy of the exact stage conditions. `CARGO_HOME` defaults to `/scratch/home/.cargo`, which is writable.
2. **The judge can compile itself in the judge VM with one argv and no project-relative path:** `["cargo", "run", "--release", "--frozen", "--quiet", "--target-dir", "/tmp/judge-target"]`. The judge's working directory is the root-owned, read-only `/scratch/judge`, so cargo must not write there. `--frozen` plus a pushed `Cargo.lock` keeps it from writing. Without a lock, cargo refuses: "cannot create the lock file ... because --frozen was passed". Absolute paths contribute no judge path, so this argv passes rule 6 and `judge push` vacuously.
3. **`RLIMIT_AS` is not a problem at the defaults.** In the proxy, a tiny std-only crate needed more than 384 and at most 400 MiB of address space. A 492-line crate needed more than 400 and at most 448 MiB. The default candidate allowance is 768 MiB. Below about 192 MiB rustc cannot even map `libLLVM.so`. The authoritative in-VM bisection is still Phase 8's job (D-02). It can be done purely through manifest values, because allowance = `memory_mib` - 256 - `scratch_mib`.
4. **`git apply` silently does nothing when run from a subdirectory of a git repository.** It prints "Skipped patch" for every file and exits 0, and so does `--check`. The shipped sample lives at `examples/<project>` inside the Minos repository, so that is exactly where it would be run. The canonical patch, produced by `minos-core`'s own `diff_trees`, is accepted byte for byte by git 2.54.0 (Apple Git-157) and by `/usr/bin/patch` (2.0-12u11-Apple). That holds for every newline edge case, at a repository root, outside any repository, and, for `patch -p1` and `git apply --directory=<prefix>`, inside a subdirectory. `adopt` must print commands that account for this, and the golden test must compare bytes, never exit codes.

**Primary recommendation:** Build each new command as a library function in `minos-cli`, generic over `ModelProvider` and `Backend` the way `Runner` is generic over its backend, with `main.rs` picking `ClaudeCli` and `KrunBackend`. Close the small core and store gaps listed under Architecture first. Ship `examples/logparse` outside the workspace (`exclude = ["examples"]`), with its own literal `[lints]` table and committed `Cargo.lock` files. Prove DOCS-02 with an ignored VM test that injects `FakeProvider` (or uses a sweep tier) and the real Docker buildx `env build`.

## Architectural Responsibility Map

The tiers here are Minos's own layers, not web tiers.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Command parsing, run and experiment prefix resolution, progress rendering | `minos-cli` (binary + lib) | `minos-core::text` (`SafeText`) | Only the CLI meets terminals, the filesystem layout, and concrete types (deny.toml layering) |
| Cost line `approx. cost: $X.XX` with token counts | `minos-core::money` | `minos-cli` (prints it) | COST-02: exactly one type formats cost; core owns it today (`ApproxCost`) |
| Run cost totals from a fold | `minos-core::policy`/`money` | `minos-cli` | `spend_ok` already sums; factor it out so the CLI and the safe points share one sum |
| Bytes-per-token and CLI output cap constants (D-08) | `minos-core::limits` | `minos-cli` (advisory) | D-08 places them in `limits` |
| Run enumeration and liveness probe | `minos-store` (`Ledger`/`RunLock`) | `minos-cli` | D-05, D-10, D-15 need to list `runs/`; must stay implementable by the later DynamoDB store |
| Surface `Tree` and program bytes from a snapshot or a stored base blob | `minos-source` | `minos-cli` | `report`/`show` rebuild from blobs alone; minos-source owns the tarball format |
| Report building and hash | `minos-core::report` | `minos-cli` | Already pure; CLI only gathers inputs |
| Patch bytes | `minos-core::diff` (canonical form) | `minos-cli` (`create_new` write) | The file is the canonical cumulative diff bytes, unchanged |
| Generation 0 and the loop | `minos-loop::Driver` | `minos-cli` (`check`, `run`) | Already built; the CLI only builds `RunInputs` |
| Sample candidate build and run | Guest stages in the Candidate VM | the image (toolchain under `/usr/local`) | Every experiment runs in a VM (DOCS-02) |
| Sample judge compile and scoring | Guest judge stage in the Judge VM | pushed judge data (source plus answer key) | D-04: compiled from pushed source in the judge VM |
| Threat model and trust base docs | `README.md`, `SECURITY.md` | CLAUDE.md (design of record) | DOCS-03, DOCS-04 |

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### The sample project (DOCS-01, DOCS-02)

- **D-01: The objective is quality computed from the candidate's `out/` files, scored against the judge's private data.** The judge VM today receives only judge data plus the candidate's `out/` files, and PID 1's stage report carries no timing, so a speed objective would need a new trusted measurement path. Phase 8 adds nothing to the guest or the runner for the sample: no candidate-reported number is ever read, and the held-out flag exercises the report's divergence check. A speed objective measured by PID 1 and passed into the judge's input is deferred (see Deferred Ideas).
- **D-02: The candidate is a Rust crate, std only, no dependencies**, built by `cargo build --release` offline inside the VM. It exercises a real build stage and measures the `RLIMIT_AS` default (768 MiB candidate allowance at the 2048 and 1024 defaults) against a real toolchain, which closes the CLAUDE.md open question assigned to Phase 8; the sample's `[environment]` values or the defaults are adjusted from that measurement. No vendoring, because there are no dependencies. The base image is digest-pinned arm64 Rust with toolchains in world-readable prefixes (`CARGO_HOME` and `RUSTUP_HOME` under `/usr/local`, never `/root`, per 05-RESEARCH Pitfall 9).
- **D-03: The workload is log field extraction scored against a private answer key**, the `logparse` sketch in CLAUDE.md's Projects section. Visible messy log lines live in the source tree; the parser is the one writable file; the candidate's `run` command writes the extracted fields, one row per input line, to `/out`. Judge data holds the expected fields. The objective is extraction accuracy, `maximize`, marked held-out for the lines whose answers the proposer never sees; a gate checks the output has exactly one row per input line (metric exactly 1). The answer key is the kind of secret judge data exists to protect, and the sample's docs say so.
- **D-04: The judge is a Rust crate under `examples/<project>/judge/`, compiled inside the judge VM from pushed source.** The judge path is a `[source] exclude` entry, so it is in neither the snapshot nor the image build context, as the manifest rules require. `minos judge push` uploads the judge crate's source together with the answer key under the manifest's `[judge] version` label; the `[judge] command` runs `cargo` on that source inside the judge VM using the image's toolchain, with the target directory on `/scratch`, then runs the result, all within `[budget] judge`. Nothing prebuilt is committed or pushed, the judge is reviewable source, and the std-only rule applies to the judge too. The planner measures the in-VM compile time once and sets the sample's `[budget] judge` and `scratch_mib` with the CR-01 judge-scratch rule (`judge_scratch_needed_bytes`) in mind. A shell, awk, or any non-Rust judge is not an option for this project.

### `minos check` (CLI-05, JUDGE-02)

- **D-05: `check` reuses a calibrated diagnostic run when the frame matches.** It looks in `.minos/runs` for a run whose manifest ID, base `TreeId`, root image digest, and baseline judge version equal the current ones and whose generation 0 is complete (`baseline_ready`); if one exists it reports from that ledger without launching a VM, otherwise it starts a new run through `Driver::start` and `calibrate` (07-CONTEXT D-06). `--fresh` forces a new run and new trials. Repeated `check` costs nothing, and the evidence stays in the ledger. A `run` still performs its own generation 0 (D-06 is unchanged).
- **D-06: `check` refuses, naming the exact command, when `minos.lock`, the root image, or the pushed judge data is missing or stale.** It validates the manifest first, then requires `env build` and `judge push` to have been run: a missing or stale lock entry, a Dockerfile or guest binary that no longer matches the lock (`Lock::check`), or an unpushed judge label is an actionable error naming `minos env build` or `minos judge push`. There is no implicit image build inside `check` and no `--build` flag; the build and the push stay explicit, auditable steps. It also runs the provider preflight (`ClaudeCli::preflight`) and prints each refusal (06-CONTEXT D-01 to D-04).
- **D-07: The baseline trial count stays the manifest's `[judge] trials`; `check` reports and advises.** It runs exactly that many trials, prints each trial's objective values, the noise floor (the spread), and `min_improvement` side by side with core's `noise_check` verdict, and advises raising `min_improvement` or `trials` when the spread is near or above the threshold. No adaptive trial count (07-CONTEXT D-05 stands). Phase 8 measures the sample across several boots to set the sample's own `trials` value and to confirm or change the default of 3, closing the STATE.md Phase 8 blocker with data rather than a mechanism; the measurement is recorded in the phase evidence.
- **D-08: `check` prints an advisory for `single_shot`'s full-file batch output size.** It computes the writable files' total bytes times `batch`, converts with a fixed conservative bytes-per-token constant, and prints an advisory beside the route table when the result approaches the CLI's output cap (about 32,000 tokens; 06-RESEARCH Pitfall 5), suggesting a smaller `batch` or surface. Advisory only, like the noise verdict: no model call, and no change to `ProposerSpec::eligible`. The constant and the cap live in `minos_core::limits` with a comment naming their source.
- **D-09: `check` also prints the route table with reasons (`ProposerSpec::eligible`), the memory budget (`memory_mib`, `scratch_mib`, the guest reserve, and the candidate's allowance; 05-CONTEXT D-06), and the judge-scratch requirement from the lock's judge extent.** Its exit code is non-zero only for a refusal, never for an advisory.

### Addressing runs and experiments, and the report-to-adopt flow (CLI-07, CLI-08, CLI-09, CLI-10)

- **D-10: Runs and experiments are named by unique lowercase hex prefix, and the run defaults to the latest.** Any run or experiment argument is a prefix that must match exactly one ID known to `.minos`; an ambiguous or absent prefix is a named error that lists the candidates. Commands that take a run (`status`, `log`, `stop`, and `show`, `report`, `adopt` through `--run`) default to the most recently started run in `.minos/runs` when `--run` is omitted; `show`, `report`, and `adopt` look the experiment up in that run. This mirrors git's short-hash convention. Output prints IDs as 12-hex prefixes everywhere, and the full ID on request.
- **D-11: `adopt --reviewed <hash>` is the acknowledgment.** `report <exp>` renders the report through core's `build_report` and prints it with its `ReportHash`, and appends nothing. `adopt <exp> --reviewed <hash>` rebuilds the report from the ledger and blobs now, requires `--reviewed` to equal that hash byte for byte (so a report that changed since the user read it, for example after a re-score, is refused), appends `ReportAcknowledged { exp, report }`, and only then writes the patch. One deliberate step carries the proof, and the ledger records the acknowledgment at the moment it gated something. A repeated `adopt` for the same experiment and hash is a `Duplicate` append and a fresh refusal if the patch file exists (D-12). There is no `report --ack`.
- **D-12: The patch lands at `<project>/minos-<first 12 hex of the experiment>.patch`, never overwriting.** The file is created with `create_new`, so an existing file is a named refusal; `--out <path>` redirects, also with `create_new`. `adopt` prints the path and the exact `git apply` and `patch -p1` commands. The bytes are the canonical cumulative diff from the run's base tree, the same bytes the CLI-09 golden test checks against both tools, rendered through the escape-stripping renderer (a canonical diff of a writable file may still carry hostile bytes in added lines; the renderer's allowlist applies to the on-screen copy, while the patch file keeps the exact canonical bytes, because altering them would make the hash lie). `adopt` never applies the patch and never writes into the working tree.
- **D-13: `stop` refuses, naming the run, when no driver holds it.** `minos stop [run]` tries the run's `driver.lock`: if the lock can be taken, no driver holds the run, so `stop` exits non-zero with "no driver holds run <id>; nothing to stop" and appends nothing (under 07-CONTEXT D-19 a stop recorded with no session running would never take effect). When the lock is held, it appends `StopRequested { generation, reason: UserStop, ordinal: Some(RunState::next_user_stop_ordinal(generation)) }` (the WR-01 decision of 2026-10-24) and prints that the driver stops at its next safe point and never kills a VM, so a running trial finishes first. `stop` takes the ledger's append lock only, never the driver lock, so it works while a driver holds the run.

### Progress, status, and log rendering (CLI-06, CLI-07, COST-02)

- **D-14: The progress line is fixed-width columns, plain text, no color.** `run` prints one line per recorded trial, for example `g03 3f9a2c1d7e4b single_shot scored +6.2% 1m42s`: the generation prefix, the 12-hex experiment prefix (the same prefix `adopt` names its file by), the tier, the outcome word with the `Reason` name when rejected (`rejected:gate_failed`), the delta as a signed percent of the incumbent's objective (or `n/a` when the trial was not scored), and the elapsed time as the host-measured wall-clock of the candidate plus the judge VMs. One line per `Selected` (`g03 ... selected improved` or `kept` or `no_valid_candidates`), one `Retrying` line per infrastructure retry, and one `approx. cost: $X.XX (in N tokens, out M tokens)` line per generation, printed by the one `ApproxCost` formatter. The format is pinned by a golden test against a fake `Observer` stream. Every string from a sandbox or a model has already passed `SafeText` or `LogExcerpt` inside the loop; the CLI's renderer is applied to every line regardless.
- **D-15: `status` shows the fold plus liveness and health.** It reads the ledger under the shared lock and prints: the run ID and `started_at`, generations concluded and the target when the session recorded one, the incumbent with its objective against the baseline and the noise floor, `approx. cost` with token counts, recorded stops with their reasons, whether a driver currently holds the run (a `try_lock` probe on `driver.lock`, released at once), and any ledger problem (`StoreError::Corrupt` or `Conflict`) with its seq and kind, saying the ledger needs a human. Resume refusals (a changed base, image, judge version, or frame; 07-CONTEXT D-10, D-11, D-19) are reported by `run` at start, not by `status`.
- **D-16: `log` renders one line per event, with `--json` for the stored lines.** Default output is one line per event in `seq` order, `seq kind generation <key fields>`, with IDs as 12-hex prefixes and enums by name, filterable with `--generation N` and `--exp <prefix>`. `--json` writes each stored line exactly as the ledger holds it, which is safe because a ledger line holds only IDs, numbers, and closed enums by construction (rationales, notes, and logs are blobs). Every byte still passes the renderer, which is a no-op on those lines.
- **D-17: `show <exp>` prints the full diff and rationale, the lineage, and every trial's outcome; stage logs only behind `--logs`.** The header names the experiment, its parent, generation, tier, and origin (the model ID or `sweep`); then each trial's outcome with its `Reason` and delta; the lineage chain from the baseline; the rationale through `SafeText` under a fixed cap; and the cumulative diff from the base through the renderer in full (it is bounded by the diff limits already). `--logs` appends each stage's `LogExcerpt` tail (control characters stripped, capped at the stored tail), off by default so a hostile log never prints unasked.

### Claude's Discretion

- `minos init --kind <tune|optimize|explore>`: the scaffold's exact `minos.toml`, `program.md`, and `minos.Dockerfile` contents per kind, which should be the sample project's files generalized, and whether `init` also writes a starter `~/.config/minos/prices.toml` when none exists (it must never write rates that price a run at zero; a template with the compute rates and a commented model table is acceptable).
- `README.md` and `SECURITY.md` structure: install steps (Homebrew tap, Docker with BuildKit, `host setup`, `prices.toml`), the trust base (libkrunfw, msb_krun, Hypervisor.framework, Seatbelt, the `_minos` user recorded as missing), what preflight checks, the one `KRUN_BOOT_START_NS` exception, and the `claude` CLI terms sentence from PROJECT.md. Neither file may say "ironclad", "exact", or claim the sandbox cannot be escaped.
- The sample's `program.md` text, its visible versus held-out line split, its `min_improvement` and `trials` values (set from the D-07 measurement), and the exact wording of `check`'s advisories.
- The layout of `report`'s terminal output around core's `Report::text()`, and how `run` prints a resume refusal.
- The CLI's flag names beyond those fixed above (`--fresh`, `--out`, `--run`, `--json`, `--logs`, `--generation`, `--exp`, `--generations`, `--allow-dirty`, `--reviewed`).

### Deferred Ideas (OUT OF SCOPE)

- **A speed objective measured by the trusted guest** — PID 1 timing the run stage, a new input-disk section or file carrying the host-trusted number into the judge VM, and runner plumbing; a later milestone, or earlier if a user project needs a throughput objective. Rejected for the Phase 8 sample (D-01).
- **`check` building the image or pushing judge data itself**, or a `--build` flag — rejected (D-06); the build and push stay explicit commands.
- **An adaptive baseline trial count** — rejected (D-07); 07-CONTEXT D-05 stands.
- **Making an over-cap `single_shot` batch ineligible in `ProposerSpec::eligible`** — rejected for v1 (D-08); the advisory may become a gate once the bytes-per-token constant is validated against live calls.
- **`report --ack` as a separate acknowledgment step** — rejected (D-11); revisit only if an inbox or review workflow (POL-01) needs acknowledged-but-not-adopted state.
- **Patches under `.minos/patches/`** — rejected (D-12).
- **`minos rejudge` (NEXT-02), `minos steer` (NEXT-01), `inbox`, `answer`, `params add`, `env shell`** — later milestones, unchanged.
- **Moving CLAUDE.md's design sections into `docs/`** — allowed by the CLAUDE.md preamble once real docs exist; Claude's discretion, not required here.
- **Cross-generation prompt caching via `--system-prompt-file`**, a **model-call deadline knob**, and **recording the `claude` CLI version in the ledger** — carried from 06-CONTEXT, unchanged.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `minos init --kind <tune\|optimize\|explore>` scaffolds `minos.toml`, `program.md`, `minos.Dockerfile` | Sample files generalized (Pattern 6); the Dockerfile layout verified under STAGE_ENV (Finding E1); `prices.toml` shape for the optional template (Code Examples) |
| CLI-05 | `minos check` validates, builds, judges the baseline repeatedly, prints noise floor and routes, actionable errors | `Driver::start`/`calibrate` signatures; `Lock::check`, `judge_inputs`, `Runner::check`, `ClaudeCli::preflight`; `route_table`, `noise_check`, `candidate_allowance_mib`, `judge_scratch_needed_bytes`; D-05 reuse needs `Ledger::runs()` (gap G2) |
| CLI-06 | `minos run --generations N` starts or resumes, prints one sanitized progress line per experiment | `Driver::run_to(target)` absolute target; `Observer`/`LoopEvent`; elapsed from `Spent` usage ÷ `RUNNER_VCPUS`; percent delta needs the incumbent value (gap G5); start-vs-resume rule (Open Question 1) |
| CLI-07 | `status`, `log`, `show` through the sanitizing renderer | `RunState` accessors; `SafeText`, `LogExcerpt::from_bytes`; `Judged.logs` blob IDs; `Ledger::read` returns `Line`s for `--json` |
| CLI-08 | `report <exp>` renders the hashed report and prints its hash | `ReportInput::new(state, manifest_at, judge, target, blobs, surface)`; manifest blob at `RunState::manifest_at`; surface tree from the stored base blob (gap G3) |
| CLI-09 | `adopt` refuses without a matching acknowledged hash, writes a canonical patch both tools accept, never applies | Patch experiments E5 (both tools byte-exact; subdirectory pitfall); `ReportAcknowledged` key `digest(exp), digest(report)`; `create_new` |
| CLI-10 | `stop <run>` records `StopRequested`; driver exits at next safe point | `RunLock::lock_run` probe; `RunState::next_user_stop_ordinal`; race noted (Pitfall 7) |
| JUDGE-02 | `check` judges the baseline several times with trial index on `Judged`, reports noise floor vs `min_improvement` | `Calibration { trials, floor, verdict }`; `NoiseFloor` accessors; deterministic sample metric gives spread 0 (Pitfall 9) |
| COST-02 | One type formats cost as `approx. cost: $X.XX` with token counts; no estimate, no "exact" | `ApproxCost` Display exists without tokens; extend in `money.rs` (gap G1); a gate test over CLI output and docs |
| DOCS-01 | `examples/` optimize project whose judge measures from harness and output files | Layout and judge argv (Pattern 5, E2, E3); held-out split design (Pitfall 10) |
| DOCS-02 | `check`, `run`, `report`, `adopt` end to end on the sample, every experiment in a VM | Ignored VM test design (Validation Architecture); host prerequisites (Environment Availability) |
| DOCS-03 | `SECURITY.md`: threat model, trust base, preflight checks, no ironclad claim | Trust base inventory from CLAUDE.md §Local backends; banned-words gate test |
| DOCS-04 | README: `claude` CLI provider only execs the user's own install, never handles credentials; API key or Bedrock for shared use | PROJECT.md line 59 terms sentence; installed CLI is now 2.1.312, so the live suite needs a rerun before the docs cite it |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

These bind every plan. They have the authority of locked decisions.

- `unsafe_code = "forbid"`; `unwrap_used`, `expect_used`, `indexing_slicing` are `forbid` in every target, tests included; no `#[allow]` or `#[expect]` anywhere, and the C-02 grep (`grep -rnE '#!?\[allow\(' crates --include='*.rs'`) must print nothing. The sample crates follow the same rule.
- Never weaken a security invariant to compile or pass; no `pub` field, `Deref`, or bypass on `Untrusted<T>`, `SandboxSpec`, `LaunchPlan`.
- Everything is Rust: the sample candidate, the judge, and every helper. No shell, awk, or Python component (user rule).
- Commands are argv arrays; never build a shell string from manifest or model input. Environments are cleared to an allowlist.
- No tokio in any `minos-*` crate; v1 is synchronous.
- `anyhow` only in `minos-cli`'s `main`; library code uses `thiserror` enums whose variants carry IDs, numbers, closed enums, or fixed text, never guest or model bytes.
- `BTreeMap`/`BTreeSet` or sorted `Vec` wherever order affects output.
- clap builder API only; the `derive` feature stays off (E0453 under forbid).
- No ledger wire change in this phase (no new event, no new field). Wire formats are golden-tested with insta plus a SHA-256 pin; `.config/insta.yaml` has `update: no`.
- Deserialization is validation: `#[serde(try_from)]`, `deny_unknown_fields` on raw structs.
- No git in the trusted path except `minos-source`'s read-only clean check.
- Every new workspace member needs `[lints] workspace = true`, a `deny.toml` layering entry, and the D-15 per-manifest check. New dependencies need `cargo deny check`.
- Check external APIs against docs; prove a mechanism before any dependency fork or patch.
- Docs must not overclaim isolation: no "ironclad", no "exact", no claim the sandbox cannot be escaped.
- `cargo clean` after verification passes, before the phase is marked complete (GATE-06).
- Keep CLAUDE.md current in the same change as any decision change (for example the `/candidate/out` doc drift in Pitfall 11, and the `RLIMIT_AS` open question once measured).

## Standard Stack

No new external crate is required. Everything below is already in `Cargo.lock`.

### Core (already in the workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `clap` | 4.6.7, builder API | Command tree for the nine new subcommands | Locked; derive is E0453 under forbid |
| `anyhow` | 1.0.104 | `main` error boundary only | rules/err-anyhow-app |
| `thiserror` | 2.0.21 | Typed errors in `minos-cli`'s lib modules | rules/err-thiserror-lib |
| `serde_json` | 1.0.151 (`float_roundtrip`) | `log --json` writes stored lines | Already the ledger codec |
| `toml` | 1.1.8 | `prices.toml`, manifest, lock | Already used by `Prices::parse_toml` |
| `tempfile` | 3.27.0 (dev) | Test projects and scratch dirs | Already a dev-dependency |
| `insta` | 1.49.0 (dev) | Golden output for progress lines and `init` scaffolds, if the planner treats them as pinned formats | Workspace convention |

### Host tools the phase uses (not crates)
| Tool | Version on this Mac | Used by |
|------|---------------------|---------|
| `/usr/bin/git` | 2.54.0 (Apple Git-157) | CLI-09 golden test; existing clean check |
| `/usr/bin/patch` | 2.0-12u11-Apple (BSD-derived, not GNU) | CLI-09 golden test |
| Docker Desktop + buildx | Engine 29.6.1 arm64, buildx v0.35.0-desktop.2 | `env build` of the sample image; DOCS-02 VM test |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Symlinking the toolchain into `/usr/local/bin` | Standalone Rust installer `install.sh --prefix=/usr/local` | A download inside the Dockerfile with a checksum to verify; more Dockerfile lines; no benefit when the official image is already digest-pinned |
| `rust:1.97.1-slim-trixie` | `debian:trixie-slim` + standalone toolchain | Same toolchain bytes, more Dockerfile work; the slim rust image already brings `gcc` and `libc6-dev` for linking |
| `cargo run` as the judge argv | `rustc` directly | `rustc` needs two invocations (compile, then run), which needs a shell; `cargo run` is one argv |

**Installation:** none. `cargo deny check` is unaffected unless the planner adds a crate.

## Package Legitimacy Audit

This phase installs no new Rust package and no npm, PyPI, or crates.io dependency. The sample crates are std-only with no `[dependencies]` (D-02). The only external artifact is a container base image, which is not a package-registry install. It is pinned by digest and covered by `env build`'s `FROM` digest rule.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none) | — | — | — | — | — | No package installed |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

Base image, for the record: `rust:1.97.1-slim-trixie@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c`. That is the index digest. Its linux/arm64/v8 manifest is `sha256:3928ba262c79a46d18a4d1c125b24c089963ba1e1d6b203bd3bc169924e71f04`. [VERIFIED: `docker buildx imagetools inspect`, 2026-10-28] The bookworm alternative is `rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3`. Pin the index digest in `FROM`, because buildx resolves `--platform linux/arm64` from it.

## Architecture Patterns

### System Architecture Diagram

```
user argv ──► minos (main.rs, clap builder) ──► minos_cli::<command>(project, opts, P, B) ──► stdout/stderr
                                                       │                               (every line through SafeText;
                                                       │                                cost only via ApproxCost)
         ┌─────────────────────────────────────────────┼──────────────────────────────────────────────┐
         ▼                                             ▼                                              ▼
  read-only commands                         driving commands                               writing commands
  status / log / show / report               check / run                                    stop / adopt / init
         │                                             │                                              │
  FileStore::read(run) ─► fold ─► RunState    manifest + snapshot(project) + Lock::check      stop: lock_run probe
  blobs: manifest@gen, diffs, rationales,     + judge_inputs + prices.toml                    ─► held? append StopRequested
  base tarball ─► surface Tree                ─► store base tarball blob                      adopt: build_report == --reviewed?
  build_report(ReportInput) ─► text + hash    ─► RunInputs ─► Driver::start|resume            ─► append ReportAcknowledged
                                              ─► calibrate (check) | run_to(N) (run)          ─► create_new patch file
                                                       │                                      init: create_new scaffold files
                                                       ▼
                                     RunnerJobs<KrunBackend<FileBlobSource>>  ◄── Proposer<ClaudeCli|none>
                                                       │
                              Candidate VM (build: cargo build --release; run: parser > /out/...)
                                                       │ out/ re-packed (store_judge_input)
                              Judge VM (cwd /scratch/judge: cargo run --frozen --target-dir /tmp/...;
                                        reads answers + /scratch/candidate/out/...; writes /out/scorecard.json)
                                                       │
                                     Judged + Spent records ─► Observer ─► progress line
```

### Recommended Project Structure
```
crates/minos-cli/src/
├── main.rs            # clap tree + dispatch only; picks ClaudeCli and KrunBackend
├── lib.rs             # adds the modules below
├── project.rs         # locate project root, read manifest/lock/prices, snapshot, store base blob, build RunInputs
├── address.rs         # run and experiment prefix resolution, latest-run choice (D-10)
├── render.rs          # 12-hex IDs, progress line formatter, Observer impl (D-14), log line (D-16)
├── check.rs           # D-05..D-09
├── run.rs             # start or resume, run_to
├── status.rs, log.rs, show.rs, report.rs, adopt.rs, stop.rs, init.rs
crates/minos-cli/tests/
├── progress_golden.rs # fake LoopEvent stream -> pinned lines (D-14)
├── patch_tools.rs     # CLI-09 golden: git apply + patch -p1, byte comparison
├── sample_manifest.rs # examples/logparse parses, snapshots, has no deps (gate coverage)
├── docs_words.rs      # README/SECURITY banned words (DOCS-03)
└── sample_vm.rs       # ignored: DOCS-02 end to end on real VMs
examples/logparse/
├── minos.toml  program.md  minos.Dockerfile  .gitignore  Cargo.toml  Cargo.lock
├── src/main.rs  src/parse.rs (the one writable file)  data/input.log  data/examples.tsv (visible answers)
└── judge/                  # [source] exclude; pushed with `minos judge push judge`
    ├── Cargo.toml  Cargo.lock  src/main.rs
    └── answers.tsv         # every line's expected fields plus a visible/held-out column
```

### Pattern 1: Commands as library functions generic over provider and backend
**What:** `pub fn run_command<P: ModelProvider, B: Backend>(project: &Project, proposer: &Proposer<P>, jobs: &RunnerJobs<B>, out: &mut impl Write, ...) -> Result<ExitCode-like, CliError>`. `main.rs` builds `ClaudeCli::preflight(&ParentEnv::...)` and `KrunBackend::new(HostPaths, FileBlobSource)` and calls it.
**When to use:** every command that drives the loop (`check`, `run`).
**Why:** the DOCS-02 VM test can inject `FakeProvider` (or `Proposer::without_provider()` for a sweep-only route) and still use the real `KrunBackend`, with no login and no model spend. This mirrors `Runner<B, S>` and `Driver<…, P, J, O>`, which are never trait objects (CLAUDE.md §minos-sandbox). `Proposer::without_provider()` exists for sweep-only projects. [VERIFIED: crates/minos-propose/src/proposer.rs:262]

### Pattern 2: Building `RunInputs` (what `check` and `run` share)
In order, each step's failure naming the command to run next:
1. Parse `<project>/minos.toml` with `Manifest::parse_toml`, keeping the exact bytes (`manifest_bytes`).
2. `read_lock(<project>/minos.lock)`. `None` means "run `minos env build`". `Lock::check` gives the root image and refuses a changed Dockerfile or guest. `judge_inputs(&lock, &manifest)`: `LockMismatch::NoJudge` means "run `minos judge push <dir>`". [VERIFIED: crates/minos-cli/src/jobs.rs:70]
3. Read `~/.config/minos/prices.toml` (nothing reads it yet; the path is from CLAUDE.md §Cost and 07-CONTEXT D-13) and parse it with `Prices::parse_toml`.
4. `snapshot(project, &manifest, policy, &lock.judge_file_digests())`, then `store.put(snapshot.tarball())`, which must equal `tree.to_blob_id()` (the runner fetches the base by tree; this is the runner_vm.rs pattern). [VERIFIED: crates/minos-cli/tests/runner_vm.rs:340-352]
5. Build `BaseTree::new(tree, surface)` and `program` bytes **from the snapshot**. No helper exists (gap G3).
6. Build `RunInputs { manifest, manifest_bytes, base, program, prices, root_image, judge }`. [VERIFIED: crates/minos-loop/src/inputs.rs:40]

### Pattern 3: Driver entry points (exact signatures)
```rust
// crates/minos-loop/src/driver.rs [VERIFIED]
Driver::start(store: &'a S, proposer: &'a Proposer<P>, jobs: &'a J, observer: O,
              inputs: RunInputs<'a>, options: DriverOptions, started_at: u64) -> Result<Self, DriverError>
Driver::resume(store, proposer, jobs, observer, inputs, options, run: RunId) -> Result<Self, DriverError>
driver.calibrate() -> Result<Calibrated, DriverError>   // Ready(Calibration{trials(), floor(), verdict()}) | Stopped{reason}
driver.run_to(target: u32) -> Result<RunEnd, DriverError> // Reached{generation} | Stopped{generation, reason}
// S: Ledger + Blobs + RunLock; P: ModelProvider; J: JobRunner; O: Observer
```
`start` with inputs equal to an existing run's (same manifest, base, and `started_at`) rejoins it. `RunId::of(manifest, base, started_at)` uses seconds, so two `check --fresh` calls in the same second rejoin one run. `started_at` comes from the CLI's clock (`SystemTime::now` is allowed in the CLI, banned only in core).

### Pattern 4: Report inputs
`ReportInput::new(&state, &manifest_at_target_generation, &judge, &target, &blobs, &surface)` [VERIFIED: crates/minos-core/src/report.rs:154]:
- `manifest` is the one `RunState::manifest_at(proposal.generation())` names. Read its blob by `ManifestId` digest (the driver stores manifest bytes as a blob whose ID has the manifest ID's digest) and parse it.
- `blobs` must hold the target's diff blob and the rationale blob of every lineage proposal, keyed by `BlobId::of`.
- `surface` is the base tree's readable plus writable files, read back from the stored base tarball (gap G3).
- `Report::text()` and `Report::hash()`. `RunState::is_acknowledged(exp, report)`. The `ReportAcknowledged` key is `digest(exp), digest(report)`, so a repeat is `Duplicate`. [VERIFIED: crates/minos-core/src/ledger.rs:26]

### Pattern 5: The sample's judge argv and layout (verified in the proxy)
```toml
[source]
exclude = ["judge/", "target/"]

[judge]
version = 1
command = ["cargo", "run", "--release", "--frozen", "--quiet", "--target-dir", "/tmp/judge-target"]
```
- The judge stage's cwd is `/scratch/judge`, where the pushed judge directory is unpacked. It stays root-owned, so the judge cannot change its inputs. [VERIFIED: crates/minos-guest/src/run.rs:354-405] Cargo finds `Cargo.toml` in the cwd. Its parent search (`/scratch`, `/`) finds nothing.
- `/tmp` is the scratch tmpfs (bind of `/scratch/tmp`, owned by uid 64000); `HOME=/scratch/home` is writable. Cargo's own home lands in `/scratch/home/.cargo`.
- The judge reads its key relative to the cwd (`answers.tsv`) and the candidate output at **`/scratch/candidate/out/<file>`** (not `/candidate/out`; see Pitfall 11). It writes `/out/scorecard.json`, the only file PID 1 reads from a judge's `/out`. [VERIFIED: crates/minos-guest/src/pack.rs:5,59]
- No argv entry is a relative path, so `judge.paths()` is empty. Rule 6 and `judge push`'s missing-path check pass vacuously. The `judge/` exclude still keeps the judge source and key out of the snapshot and the image build context, and `judge push` requires a directory inside the project to lie under an exclude entry. [VERIFIED: crates/minos-core/src/manifest.rs:1757-1789; crates/minos-cli/src/judge_push.rs:261-277]
- **Do not** write `["cargo", "run", "--manifest-path", "judge/Cargo.toml", …]`. `judge/Cargo.toml` would become a judge path, and `judge push judge` would refuse it: the pushed tree holds `Cargo.toml`, not `judge/Cargo.toml` ("at the same project-relative path", main.rs JUDGE_PUSH_HELP).

### Pattern 6: The sample Dockerfile (verified in the proxy)
```dockerfile
# Every stage runs as uid 64000 with PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin,
# HOME=/scratch/home, TMPDIR=/tmp, LANG=C; ENV lines here never reach a stage, and USER is ignored.
FROM rust:1.97.1-slim-trixie@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c
RUN ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/cargo /usr/local/bin/cargo \
 && ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/rustc /usr/local/bin/rustc
```
The rustup proxies in `/usr/local/cargo/bin` are never on the stage `PATH`, so rustup is never consulted. rustc finds its sysroot from `/proc/self/exe`, which resolves the symlink. `cc` is `/usr/bin/cc` (gcc from the slim image). The candidate's commands:
```toml
[commands]
build = ["cargo", "build", "--release", "--frozen"]
run = ["./target/release/logparse", "data/input.log", "/out/fields.tsv"]
```
Commit `examples/logparse/Cargo.lock`. `--frozen` fails closed if it is missing; `--offline` without a lock also works, because cargo writes the lock into the writable `/scratch/src`.

### Anti-Patterns to Avoid
- **Relying on Dockerfile `ENV CARGO_HOME`/`RUSTUP_HOME`:** PID 1 clears the environment and sets only the four `STAGE_ENV` keys. [VERIFIED: crates/minos-guest/src/stage.rs:73-81] The CLI's `ENV_BUILD_HELP` text tells users to "set CARGO_HOME and RUSTUP_HOME there", which is misleading for exactly this reason. Correct it in this phase.
- **Making `examples/*` workspace members:** a member must carry `[lints] workspace = true` (D-15). In the VM the snapshot has no parent workspace, and cargo fails with "error inheriting `lints` from workspace root manifest's `workspace.lints` … failed to find a workspace root". [VERIFIED: Docker proxy E8]
- **Treating `git apply`'s exit code as success:** see Pitfall 1.
- **Writing the on-screen diff into the patch file:** D-12. The file is the canonical bytes; only the screen copy goes through `SafeText`.
- **Taking the driver lock in `stop` and then appending:** D-13 says `stop` never takes the driver lock beyond the probe, and append uses `ledger.lock` only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Terminal-safe text | Own escape stripping | `SafeText::new`/`inline`, `LogExcerpt::from_bytes` | Allowlist renderer with bidi handling, already tested |
| Cost formatting | `format!("${}", …)` | `ApproxCost` (plus a token-count companion in `money.rs`) | COST-02: one type |
| Summing a run's spend | A second loop over spends | Factor the sum out of `policy::spend_ok` | Same pricing rules as the safe points; never priced at zero |
| Patch bytes | Re-serializing a diff | `CumulativeDiff::recorded(base, Diff::parse(...), &exp)` then `as_diff().as_bytes()` | Must hash to the `ExperimentId` |
| Report hash | Hashing printed output | `build_report(...).hash()` | Pure function of ledger and blobs |
| Noise verdict, routes, memory budget | CLI arithmetic | `noise_check`, `route_table`/`ProposerSpec::eligible`, `candidate_allowance_mib`, `judge_scratch_needed_bytes` | Core owns them |
| Run lock probing | `flock` by hand | `RunLock::lock_run`, then drop the guard | Symlink and inode checks already done |
| Snapshot hygiene for the sample | Custom checks | `minos_source::snapshot` and `pack_judge_data` | Denylist, secret scan, judge-overlap check |
| Hostile JSON in `log --json` | Re-serializing `Line` | Write the stored line bytes as read | D-16; avoids any re-encoding drift |

**Key insight:** every value this phase prints already has a validating type in core. The CLI's job is composition and layout. A new formatter or parser in the CLI is a smell.

## Runtime State Inventory

Not a rename or refactor phase. Omitted.

## Common Pitfalls

### Pitfall 1: `git apply` from a repository subdirectory is a silent no-op
**What goes wrong:** Run inside `examples/proj/` of a repository, `git apply minos-xxx.patch` prints "Skipped patch 'src/…'" for every file and exits 0, and `git apply --check` exits 0 too. [VERIFIED: experiment E5, git 2.54.0]
**Why it happens:** git interprets patch paths relative to the top of the working tree and excludes paths outside the current subdirectory.
**How to avoid:** `adopt` prints `patch -p1 < <file>` (run in the project directory; always relative to the cwd) and `git apply --directory=<repo-relative project prefix> <file>`. `--directory=examples/proj/` made git apply every file byte for byte from that subdirectory. If the CLI cannot know the prefix without running git, print the git form with a one-line note that it needs `--directory=$(git rev-parse --show-prefix)` when the project is not the repository root. That text is printed for the user, never executed by Minos. The golden test sets `GIT_CEILING_DIRECTORIES` to the temp directory's parent (verified: git then applies in a temp dir even under a repo), clears the environment like `crates/minos-source/tests/clean.rs` does, and compares the resulting files byte for byte.
**Warning signs:** "Skipped patch" on stderr; an unchanged tree after exit 0.

### Pitfall 2: The rustup proxy layout
**What goes wrong:** With the official image unchanged, `cargo` is `/usr/local/cargo/bin/cargo -> rustup`, which is not on the stage `PATH`. Even on the `PATH`, rustup would look for `$HOME/.rustup` (`/scratch/home/.rustup`) and find no toolchain.
**How to avoid:** Pattern 6's symlinks. Verified working under uid 64000, the cleared environment, a read-only root, and tmpfs scratch.

### Pitfall 3: `--frozen` without a committed `Cargo.lock`
**What goes wrong:** "cannot create the lock file … because --frozen was passed" (and the same text for `--locked`). In the judge VM the cwd is read-only, so `--frozen` is required, and the lock must be in the pushed judge directory. [VERIFIED: E2, E8]
**How to avoid:** Commit both `Cargo.lock` files. Give the candidate and judge crates different package names: their locks must not be byte-identical, because `snapshot` refuses any non-empty packed file whose SHA-256 equals a pushed judge file (07-CONTEXT D-04). The same applies to any other shared file, such as a copied `.gitignore` or README.

### Pitfall 4: Untracked `.minos/`, `minos.lock`, and patch files make the project dirty
**What goes wrong:** The clean check runs `git status --porcelain=v1 -z --untracked-files=all -- .` in the project. [VERIFIED: crates/minos-source/src/clean.rs:57-69] `env build` creates `.minos/` and `minos.lock` inside the project, and `adopt` creates `minos-*.patch` there, so the next `run` or `check` without `--allow-dirty` refuses with "uncommitted changes". The repository's root `.gitignore` has only `/target` and insta patterns.
**How to avoid:** `examples/logparse/.gitignore` lists `/.minos/`, `/target/`, `/judge/target/`, and `/minos-*.patch`. Decide whether the sample's `minos.lock` is committed (it churns whenever `minos-guest` is rebuilt, because the lock records the guest's SHA-256) or ignored. See Open Question 3. `init` should write the same `.gitignore`.

### Pitfall 5: `RLIMIT_AS` and rustc
**What goes wrong:** rustc maps `libLLVM.so.22.1-rust-1.97.1-stable`, and mapped files count against the address space. At ≤ 192 MiB: "error while loading shared libraries … failed to map segment". At 384 MiB: SIGABRT during codegen. [VERIFIED: Docker proxy E3]
**How to avoid:** Keep the default allowance (768 MiB), measured at about 1.7x headroom for a 500-line crate. Measure in-VM by setting `memory_mib`/`scratch_mib` so the allowance steps 448, 512, 640, 768 (`scratch_mib = memory_mib - 256 - allowance`). The proxy showed no dependence on CPU count (8 and 2 CPUs gave the same thresholds).
**Warning signs:** `Crashed { Build }` or a build-stage exit with signal 6 on the baseline, which surfaces as `DriverError::BaselineUnscored`.

### Pitfall 6: The judge's own build is outside the CR-01 scratch formula
**What goes wrong:** `judge_scratch_needed_bytes` counts the judge data, the candidate output cap, and 16 MiB of slack for `HOME`, `TMP`, and the scorecard. A judge that compiles itself also writes a target dir: 492 KiB plus 56 KiB of `CARGO_HOME` for a tiny judge in the proxy. If that fills scratch, the judge stage fails as `Crashed { Judge }`, an `Outcome`, which is blamed on the candidate and on the baseline is `BaselineUnscored`. [VERIFIED: limits.rs:332-352; E2 sizes]
**How to avoid:** At the default `scratch_mib` 1024 this is moot. Do not shrink the sample's scratch toward the minimum. Record the in-VM judge target size in the evidence.

### Pitfall 7: `stop` probe races
**What goes wrong:** `stop` probes `driver.lock` and releases it. The driver can exit between the probe and the `StopRequested` append, and that stop then belongs to no session (D-19), so the next session treats it as old. Separately, `status`'s probe holds the lock for an instant, so a `run` starting at that moment gets `RunLocked`.
**How to avoid:** Accept both, and document them in `stop`'s output and help: "if the run already ended, this stop has no effect". `RunLocked` tells the user to retry. Do not hold the driver lock across the append; that is forbidden by D-13 and pointless.

### Pitfall 8: `lock_run` creates directories
**What goes wrong:** `FileStore::lock_run` calls `ensure_dir` on `runs/<hex>`, so probing a run ID that does not exist creates an empty run directory. [VERIFIED: crates/minos-store/src/file/run_lock.rs:45-60]
**How to avoid:** Resolve the prefix against existing runs first (gap G2), and probe only a resolved run.

### Pitfall 9: A deterministic objective has a noise floor of zero
**What goes wrong:** The sample's accuracy is a pure function of the candidate's output, so every baseline trial scores the same. The spread is 0, `noise_check` returns `Ok`, and D-07's "measure the sample across several boots to set the default of 3" yields no information about boot-to-boot variance. The report's anomalous-jump flag falls back to lineage steps when the spread is 0 (`measured_spread` requires `spread > 0`), so there are no false positives. [VERIFIED: policy.rs:499-520; report.rs:316-319]
**How to avoid:** Record in the evidence that the sample's spread is 0 by construction, set the sample's `trials` to 2 (the smallest that `noise_check` will measure, `MIN_NOISE_TRIALS`), and say plainly that the default of 3 stays a recorded assumption until a timing objective exists (deferred with D-01). Do not claim the STATE.md blocker is closed by this data. See Open Question 4.

### Pitfall 10: Held-out design leaks
**What goes wrong:** If the visible answers file in the source tree (`data/examples.tsv`) is byte-identical to a judge file, `snapshot` refuses the tree. If the held-out inputs are not in the source tree, the candidate cannot produce rows for them and the gate (one row per input line) cannot hold.
**How to avoid:** Put every input line in `data/input.log` (readable). Put answers for a visible subset in `data/examples.tsv` (readable, so the model sees them). The judge's `answers.tsv` holds every line's answer plus a visible/held-out marker. The judge emits `accuracy` (visible lines, the objective) and `accuracy_held_out` (`held_out = "accuracy_held_out"` on the objective), plus a gate `rows_match`. A diff that hard-codes visible answers trips `LiteralMatch`, which is the point of the sample. Keep the whole surface under `SINGLE_SHOT_SURFACE_MAX_BYTES` (256 KiB).

### Pitfall 11: Doc drift about the judge's candidate path
**What goes wrong:** CLAUDE.md §minos-sandbox and `crates/minos-sandbox/src/untrusted.rs:372` say the judge reads `/candidate/out/<file>`. The guest unpacks to `/scratch/candidate/out/<file>` (`CANDIDATE_DIR`), and nothing mounts `/candidate`. [VERIFIED: crates/minos-guest/src/stage.rs:63; run.rs:376-392]
**How to avoid:** The sample's judge and SECURITY.md use `/scratch/candidate/out/`. Fix the CLAUDE.md sentence and the rustdoc in the same change.

### Pitfall 12: The `claude` CLI moved to 2.1.312
**What goes wrong:** The live provider evidence is for 2.1.304. The auto-updater has since installed 2.1.305, .306, .308, and .312, and the link points at 2.1.312. CLAUDE.md says a later CLI version needs a rerun. [VERIFIED: `ls ~/.local/share/claude/versions`, `claude --version`]
**How to avoid:** Re-run `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored …` before DOCS-04 cites the behavior, and record the version.

### Pitfall 13: `noise_check`'s rustdoc promises a refusal D-07 does not want
`policy.rs:497` says "`minos run` refuses a `NoiseVerdict::Refuse` unless the user overrides it (Phase 8)". D-07 makes `check` advisory, and no CONTEXT decision has `run` refuse. See Open Question 2.

## Code Examples

### `prices.toml` that parses (shape from `RawPrices`, `deny_unknown_fields`)
```toml
# Source: crates/minos-core/src/money.rs:288-299 and crates/minos-core/tests/money.rs:244-255
v = 1

# Rates are strings, in US dollars per unit. Local VMs cost you electricity, not these;
# the template uses AWS Lambda MicroVM list rates so a run is never priced at zero.
[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

# Required table. Price every model your [[proposer]] entries name AND every resolved model ID the
# claude CLI reports (for example "claude-haiku-5-5"); an unpriced model refuses the run.
[models]
# [models."haiku"]
# input_token = "..."
# output_token = "..."
# cache_creation_input_token = "..."
# cache_read_input_token = "..."
```
`models` has no serde default, so the `[models]` header must be present. An empty table parses, and then any model proposer refuses with `PricesMissing`, which fails closed as intended. Verify with a unit test that this exact template parses. [VERIFIED: money.rs RawPrices has `models: BTreeMap<ModelId, TokenRates>` with no default]

### The canonical patch, as both tools accept it (generated by `minos-core::diff_trees`)
```text
diff --git a/src/nonl.rs b/src/nonl.rs
--- a/src/nonl.rs
+++ b/src/nonl.rs
@@ -1,3 +1,3 @@
 one
 two
-three
\ No newline at end of file
+THREE
\ No newline at end of file
```
Seven cases were tested in one patch: a mid-file edit, a change on a no-newline last line, adding a final newline, dropping a final newline, an insertion at the head, an append at the tail, and a deletion. `git apply` and `/usr/bin/patch -p1` each produced the expected tree byte for byte (`diff -r` clean), with no `.orig` or `.rej` files. [VERIFIED: E5] `diff_trees` emits whole-file-context hunks for these small files.

### Elapsed time for the progress line
```rust
// Usage is host-measured: vcpu_ms = elapsed_ms * vcpus (minos-runner/src/usage.rs:33)
let elapsed_ms = candidate_usage.vcpu_ms() / u64::from(minos_runner::RUNNER_VCPUS)
    + judge_usage.map_or(0, |u| u.vcpu_ms() / u64::from(minos_runner::RUNNER_VCPUS));
```
Read both `Spent { Candidate { exp, trial } }` and `Spent { Judge { exp, trial } }` from the fold after the `Judged` event; record-before-act guarantees they precede it.

### D-08 advisory arithmetic (integers only)
```rust
// tokens_estimate = ceil(writable_total_bytes * batch / BYTES_PER_TOKEN_CONSERVATIVE)
// advise when tokens_estimate * 10 >= CLI_DEFAULT_OUTPUT_TOKENS * 8   (80 % of the cap)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `CLAUDE_CODE_MAX_OUTPUT_TOKENS` default 32000 for all models | Defaults and caps vary by model; 32000 default and 128000 cap only for model IDs Claude Code cannot resolve | Current env-vars page (undated) | D-08's "about 32,000" is the documented conservative fallback, not necessarily the per-model default |
| Claude tokenizer about 3.5 chars per token (older guidance) | Newer tokenizers count about 30% more tokens for the same text (Sonnet 5, Haiku 5.5 notes) | 2026 model generation | Any bytes-per-token constant must be conservative and is advisory only |
| `claude` 2.1.304 (live evidence) | 2.1.312 installed | Auto-updater, before 2026-10-28 | Live suite rerun needed |

**Deprecated/outdated:**
- `ENV_BUILD_HELP`'s advice to "set CARGO_HOME and RUSTUP_HOME there": stage environments never see image `ENV`.

## Experiments Run (evidence for the planner and the phase evidence file)

Everything ran in `/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/8dc6123f-2e1d-4a27-a45e-9eca3f496f12/scratchpad/p8`.

| ID | What | Result |
|----|------|--------|
| E1 | `docker buildx build --platform linux/arm64 --provenance=false --sbom=false --output type=tar,dest=-` of `rust:1.97.1-slim-trixie@sha256:8e8c…` | Tar 857,614,848 bytes; 8,028 entries; regular content 851,856,127 bytes plus 1 hardlink (3,813,416 bytes); content including copies 855,669,543 bytes = 80% of `PackLimits::default()`'s 1,073,741,824. The largest part is the toolchain `lib` at 465,795,787 bytes. |
| E2 | Judge: crate in a root-owned `/scratch/judge` cwd, uid 64000, `env -i` STAGE_ENV, `--read-only` root, tmpfs `/scratch`, `/tmp`, `/out`, `prlimit --as=805306368`, `cargo run --release --frozen --quiet --target-dir /tmp/jt` | Builds, runs, writes `/out/scorecard.json`; no write to the cwd; target 492 KiB; `/scratch/home` 56 KiB; about 0.1 s wall clock for a 10-line judge on 2 CPUs. Without `Cargo.lock`: refused, cannot create the lock file. |
| E3 | Candidate build under `prlimit --as` bisection, tiny crate and 492-line crate, 8 CPUs and `--cpuset-cpus=0,1` | Tiny: 512, 480, 448, 416, 400 MiB pass; 384 SIGABRT; ≤ 192 libLLVM fails to map. 492-line: 448, 512, 768 pass (about 0.56 s); 400 and 384 fail. No CPU-count dependence. |
| E5 | Canonical patch from `minos-core::diff_trees`, 7 newline cases, applied by git 2.54.0 and `/usr/bin/patch` 2.0-12u11-Apple | Both byte-exact outside a repo. Inside a repo subdirectory, `git apply` skips all files with exit 0 (and `--check` exits 0); `--directory=<prefix>`, `GIT_CEILING_DIRECTORIES`, and `patch -p1` all apply byte-exact. |
| E8 | A member-style `[lints] workspace = true` crate built standalone (as in the VM); `--frozen` vs `--offline`, no network (`--network none`) | Inheritance fails ("failed to find a workspace root"); `--offline` without a lock builds and writes the lock; `--frozen` without a lock refuses. |

The proxy used Docker Desktop's arm64 Linux VM, not libkrunfw's 6.12.109 kernel, a SquashFS root, or the guest's rlimit trampoline. It ran native arm64 with no emulation. The authoritative numbers are the in-VM ones Phase 8 owns (D-02, D-04).

## Gaps in the code the CLI composes (each is a small plan task)

| # | Gap | Where | Recommendation |
|---|-----|-------|----------------|
| G1 | `ApproxCost` prints `approx. cost: $D.CC` with no tokens; COST-02 needs tokens beside it from the one type | `minos-core/src/money.rs:519-552` | Add a companion in `money.rs` (for example `ApproxCost::with_tokens(&self, input, output) -> CostLine` whose `Display` reuses `ApproxCost`'s), keeping one dollar formatter; existing `approx. cost: $…` tests unchanged |
| G2 | No way to list runs | `minos-store` | Add `fn runs(&self) -> Result<BTreeSet<RunId>, StoreError>` to `Ledger` (forward compatible with DynamoDB; contract test for both stores), validating each `runs/<64 hex>` name and refusing symlinks like the rest of the store; "latest" = max `(started_at, RunId)` from each fold's `RunStarted` |
| G3 | No surface `Tree`/program bytes from a snapshot, and no read-back from a stored base tarball | `minos-source` | `Snapshot::surface(&Manifest) -> (Tree, Vec<u8>)` at pack time, plus `minos_source::read_files(tarball, paths) -> Tree`, a strict reader of Minos's own canonical ustar with caps, for `report`/`show` from blobs alone |
| G4 | No accessor over all proposals | `minos-core/src/state.rs` | `RunState::proposals()` iterator for prefix resolution (or loop `proposals_in` over generations) |
| G5 | `LoopEvent::Judged.delta` is absolute; D-14 wants a percent of the incumbent's objective | `minos-loop/src/observe.rs:64-80` | The CLI observer re-folds (the observer can hold `&FileStore`) and reduces the incumbent's objective with `reduce_trials`, or the planner amends D-14 to an absolute delta; no loop change needed either way |
| G6 | Run cost total exists only inside `spend_ok` | `minos-core/src/policy.rs:609-633` | Extract `pub fn run_cost(state, prices) -> Result<(Cost, Usage), CostError>` used by `spend_ok`, `status`, and the progress line |
| G7 | No `prices.toml` path resolution | `minos-cli` | `$HOME/.config/minos/prices.toml` (CLAUDE.md §Cost); read under a byte cap; a missing file names the path and `minos init` |
| G8 | D-08 constants | `minos-core/src/limits.rs` | `CLI_DEFAULT_OUTPUT_TOKENS = 32_000` (cite code.claude.com/docs/en/env-vars) and `BYTES_PER_TOKEN_CONSERVATIVE` (see Assumption A1) |
| G9 | `ENV_BUILD_HELP` text | `minos-cli/src/main.rs:31-45` | Replace the CARGO_HOME/RUSTUP_HOME advice with the STAGE_ENV fact and the symlink layout |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | A conservative bytes-per-token figure for code of **2** (fewer bytes per token than the 3.5 characters per token sometimes attributed to Anthropic for English prose, because code and the JSON escaping of `{files:[{path,content}]}` tokenize denser, and newer tokenizers count about 30% more tokens) | G8, D-08 | The advisory fires too early or too late; it is advisory only, so the risk is noise, not safety. Validate once with a live call's `output_tokens` against the structured output's byte length, and record the ratio in the evidence |
| A2 | The per-model output default for the configured model may be above 32,000 (SPIKE-07 envelopes report `modelUsage.claude-haiku-5-5.maxOutputTokens: 128000`), so 32,000 is conservative; `output_tokens` includes `thinking_tokens`, which eat into the same cap | G8 | Advisory threshold mis-set; low risk |
| A3 | In-VM build and judge-compile times stay within the budgets: the proxy's 0.56 s build of a 492-line crate on 2 CPUs leaves ample room under `build = "2m"` and `judge = "2m"`; the first access to the SquashFS toolchain may add decompression time | Pattern 5/6 | A budget overrun is `TimedOut`, blamed on the candidate; measure in-VM and set budgets with a 10x margin |
| A4 | The in-VM `RLIMIT_AS` threshold matches the proxy within about ±64 MiB | Pitfall 5 | If higher, a 768 MiB allowance could still be close for larger crates; the planned in-VM bisection settles it |
| A5 | `--directory=<prefix>` is the right git form to print for a project in a repository subdirectory | Pitfall 1 | A user copies a command that does nothing; the golden test covers the printed form |

## Open Questions (RESOLVED)

Each question below is decided in the plan named on its RESOLVED line; the planner recorded the decisions at planning time (2026-10-28), and the plans carry the tests that hold them.

1. **How does `minos run` choose between start and resume?**
   - What we know: CLI-06 says "starts or resumes". 07-CONTEXT D-06 says `check` "never creates the run that `run` later resumes". D-10's default "latest run" will often be a `check` run. Nothing in the ledger marks a diagnostic run, and no wire change is allowed.
   - Recommendation: `minos run --generations N` always starts a new run (`Driver::start`); `minos run --run <prefix> --generations N` resumes that run (`Driver::resume`, absolute target). `status`/`log` default to the latest run whatever created it. Confirm with the user. A CLI-side marker outside the ledger (for example a `check` sidecar file) is possible but is unhashed state in `.minos`.
   - RESOLVED: `minos run --generations N` starts a new run by default and `--run <prefix>` resumes that run; N is the absolute generation in both cases, a new run whose `RunId` already exists is refused naming `--run`, and no sidecar marker is added (08-08).
2. **Does `run` refuse a `NoiseVerdict::Refuse`?** `policy.rs:497` says yes "unless the user overrides it (Phase 8)"; D-07 says advisory only. Recommendation: `run` prints the verdict at start and does not refuse (D-07), and the rustdoc is corrected. Confirm.
   - RESOLVED: the verdict is advisory only. `noise_check`'s rustdoc is corrected to say so (08-01), `check` prints it with advice and never changes its exit code for it (08-07), and `run` prints it at start and never refuses (08-08).
3. **Is the sample's `minos.lock` committed?** It records the `minos-guest` SHA-256 and the root image digest, which change with every guest rebuild, and `Lock::check` refuses a guest that no longer matches. Recommendation: ignore it in `examples/logparse/.gitignore` and have the README walk through `env build` then `judge push`. User projects commit theirs.
   - RESOLVED: the sample's `minos.lock` is gitignored (`/minos.lock` in `examples/logparse/.gitignore`) and its README walks through `env build` then `judge push` (08-05); the `.gitignore` that `minos init` writes for a user project does not ignore the lock, so user projects commit theirs (08-11).
4. **D-07's boot-to-boot variance cannot be measured on a deterministic objective.** The planner should record a zero spread for the sample and keep the default of 3 as an assumption, or add a measurement-only run of a timing probe (no new trusted path, numbers not used for scoring) if the user wants the blocker closed now.
   - RESOLVED: the sample's objective is deterministic, so its spread is 0 by construction, and the default of 3 trials is kept as a recorded assumption (08-05). 08-13 records five baseline boots with their objective values and host-measured VM times, and writes a "Decision for the user" paragraph in 08-EVIDENCE.md stating that D-07 was met partially; the user answers it at 08-13's checkpoint. 08-15 restates the STATE.md Phase 8 blocker with that outcome rather than closing it. No timing probe is added in this phase.
5. **"exact" in docs.** CONTEXT bans the word "exact". Does that include "exactly"? Recommendation: ban the whole word `\bexact\b` in README, SECURITY, and CLI output, and allow "exactly". Confirm.
   - RESOLVED: only the whole word `exact` is banned and `exactly` is allowed; `docs_words` enforces it on README.md, SECURITY.md, and the sample's README (08-14), and the CLI output tests apply the same whole-word rule plus the substring `estimate` (08-07 to 08-12).
6. **The sample's proposer ladder for the model-free VM test.** `Proposer::without_provider()` routes only the sweep tier. A sweep needs a `[[param]]` whose anchored line's remainder is rewritten whole (sweep.rs module docs), which drops a Rust line's trailing `;`. Use string `choices` that include the terminator (`["4;", "8;", "16;"]`) or an `include_str!` text file as a second writable file. Alternatively the VM test injects `FakeProvider` with a scripted single_shot reply. Either works through Pattern 1.
   - RESOLVED: the VM test injects `FakeProvider` with scripted single_shot replies (the improved-parser fixture for generation 1); the sample keeps one `single_shot` proposer and no `[[param]]` (08-13).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker Desktop + buildx | `env build` of the sample, DOCS-02 | ✓ | Engine 29.6.1 arm64; buildx v0.35.0-desktop.2 | none (env build has no other checked builder) |
| `/usr/bin/git` | CLI-09 golden, clean check | ✓ | 2.54.0 (Apple Git-157) | — |
| `/usr/bin/patch` | CLI-09 golden | ✓ | 2.0-12u11-Apple | — |
| libkrunfw | VM tests | ✓ | 5.6.2, SHA-256 prefix 59764391005357bb (matches the pin) | — |
| `/usr/bin/sandbox-exec`, `/usr/bin/codesign` | VM tests, host setup | ✓ | system | — |
| `aarch64-unknown-linux-musl` target | guest build | ✓ | installed | — |
| `cargo-deny` | gate | ✓ | 0.20.2 | — |
| `cargo-insta` | snapshot review | ✓ | 1.48.0 (stack lists 1.49.0; the gate does not run it) | — |
| `claude` CLI, logged in | `check`'s preflight on the real binary; live suite | ✓ | 2.1.312; `loggedIn` true, `claude.ai`, `firstParty`, no `apiKeySource` | `FakeProvider` or sweep for the VM test |
| Signed `minos-vmm`, musl `minos-guest` | VM tests | build step | — | build, sign, run (CLAUDE.md Commands) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none. The model login is optional for the DOCS-02 VM test through Pattern 1.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (std test harness), `proptest` 1.11.0, `insta` 1.49.0 (`update: no`), `trybuild` 1.0.122 |
| Config file | `.config/insta.yaml`; workspace `Cargo.toml` lints; `.cargo/config.toml` forbid backstop |
| Quick run command | `cargo test -p minos-cli` (plus `-p minos-core`, `-p minos-store`, `-p minos-source` for gap tasks) |
| Full suite command | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps`, plus the sample gate commands below |
| Sample gate (examples excluded from the workspace) | `cargo clippy --manifest-path examples/logparse/Cargo.toml --all-targets --target-dir target/examples -- -D warnings`, the same for `examples/logparse/judge/Cargo.toml`, `cargo test --manifest-path …` for both, `cargo fmt --manifest-path … --check`. Run from the repository root so `.cargo/config.toml`'s forbid rustflags apply; `target/examples` lets the final `cargo clean` remove it |
| VM suite (ignored, by hand) | `MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1` after build, sign, musl guest build, with Docker running |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `init --kind` writes three files, `create_new`, and the scaffolded manifest parses with `Manifest::parse_toml`; optional prices template parses with `Prices::parse_toml` | unit/integration | `cargo test -p minos-cli --test init` | ❌ Wave 0 |
| CLI-05 | `check` refusals name `env build`/`judge push`; reuse vs `--fresh` (D-05); advisories never change the exit code (D-09); the D-08 arithmetic | integration on `MemoryStore`/`FileStore` with a fake `JobRunner` | `cargo test -p minos-cli --test check` | ❌ Wave 0 |
| CLI-06 | Progress line golden from a fake `LoopEvent` stream (D-14); resume via `--run` | unit golden | `cargo test -p minos-cli --test progress_golden` | ❌ Wave 0 |
| CLI-07 | `status`, `log`, `log --json`, `show` render hostile rationale and log bytes through `SafeText`/`LogExcerpt`; `--logs` off by default | integration with seeded ledger and blobs | `cargo test -p minos-cli --test render` | ❌ Wave 0 |
| CLI-08 | `report` prints `Report::text()` and its hash, appends nothing | integration | `cargo test -p minos-cli --test report_adopt` | ❌ Wave 0 |
| CLI-09 | `adopt` refuses a wrong or stale hash, appends `ReportAcknowledged`, `create_new` refusal on repeat; patch applies byte-exact with `git apply` and `patch -p1`, at the root, with no repo, and in a repo subdirectory with `--directory` | integration + host-tool golden | `cargo test -p minos-cli --test patch_tools` | ❌ Wave 0 |
| CLI-10 | `stop` refuses when no driver holds the run; appends with the next ordinal when held (hold the lock from the test via `lock_run`) | integration | `cargo test -p minos-cli --test stop` | ❌ Wave 0 |
| JUDGE-02 | `check` prints each trial's objective, the floor, `min_improvement`, and the verdict; trial index on each `Judged` | integration (fake runner) + VM | `cargo test -p minos-cli --test check`; `sample_vm` | ❌ Wave 0 |
| COST-02 | One formatter: `approx. cost: $X.XX (in N tokens, out M tokens)`; no CLI output or doc contains `estimate` or the word `exact` | unit + text scan | `cargo test -p minos-core --test money`; `cargo test -p minos-cli --test docs_words` | partial (money.rs exists) |
| DOCS-01 | `examples/logparse/minos.toml` parses; `snapshot` packs it with `AllowDirty`; no `[dependencies]` in either sample crate; judge argv has no judge paths; `judge push examples/logparse/judge` succeeds into a temp store | integration (gate) | `cargo test -p minos-cli --test sample_manifest` | ❌ Wave 0 |
| DOCS-02 | `env build` (Docker buildx), `judge push`, `check`, `run --generations 2`, `report`, `adopt` on a temp copy of the sample; every `Judged` records krun on arm64; patch applies with `patch -p1` | ignored VM e2e | `cargo test -p minos-cli --test sample_vm -- --ignored …` | ❌ Wave 0 |
| DOCS-03/04 | SECURITY.md and README.md exist, contain the trust-base items and the terms sentence, and contain none of `ironclad`, `\bexact\b`, "cannot be escaped" | text scan (Rust test) | `cargo test -p minos-cli --test docs_words` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p <crate touched>` plus `cargo clippy -p <crate> --all-targets -- -D warnings`
- **Per wave merge:** the full suite plus the sample gate commands
- **Phase gate:** full suite green, the C-02 grep empty, the D-15 per-manifest check unchanged (examples are not members), the ignored `sample_vm` run recorded in the phase evidence, the live provider suite rerun on 2.1.312, then `cargo clean` (GATE-06)

### Wave 0 Gaps
- [ ] `crates/minos-cli/tests/{init,check,progress_golden,render,report_adopt,patch_tools,stop,sample_manifest,docs_words}.rs`, with a shared `tests/support/mod.rs` whose items every declaring target uses (dead code fails `-D warnings`, as with the krun harness)
- [ ] `crates/minos-cli/tests/sample_vm.rs` (ignored), copying runner_vm.rs's `env_path`/`SETUP` pattern; project in a `tempdir_in(JOB_ROOT_PARENT)` copy of `examples/logparse`; `CleanPolicy::AllowDirty` because the copy is not a repository
- [ ] Contract test cases for `Ledger::runs()` in `crates/minos-store/tests/contract.rs`
- [ ] No framework install needed

## Security Domain

`security_enforcement` is enabled (ASVS level 1, block on high).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (local CLI; the model login is the `claude` CLI's own, checked by `ClaudeCli::preflight`) | `ClaudeCli::preflight` refuses credential variables by name |
| V3 Session Management | no | — |
| V4 Access Control | yes, the review gate | `adopt` requires `--reviewed` equal to the rebuilt `ReportHash`; `ReportAcknowledged` recorded before the write |
| V5 Input Validation | yes | Hex-prefix grammar for run and experiment args; `Manifest::parse_toml`, `Prices::parse_toml`, `RelPath`; clap value parsers; ledger and blobs read through validating types |
| V6 Cryptography | yes (integrity only) | SHA-256 IDs via `IdHasher`/`BlobId::of`; never hand-rolled |
| V7 Error Handling and Logging | yes | Errors carry IDs and fixed text; every printed string through `SafeText`; `main` prints through `SafeText` already |
| V12 Files and Resources | yes | `create_new` for the patch, scaffold, and prices template (`O_EXCL` refuses an existing file or symlink at the path); store paths through `FileStore`'s symlink and inode checks |
| V14 Configuration | yes | Digest-pinned `FROM`; `Lock::check`; `--frozen` builds |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Terminal escape or bidi injection through a rationale, diff line, file name, or log tail | Tampering / Spoofing | `SafeText`/`LogExcerpt` on every line; `--logs` off by default (D-17) |
| Adopting a patch other than the one reviewed (report changed after reading) | Tampering | Rebuild the report at adopt time and compare the hash byte for byte (D-11) |
| A user believes a patch applied when `git apply` silently skipped it | Repudiation / integrity of the review gate | Print `patch -p1` and the `--directory` git form; never claim success; golden test on bytes (Pitfall 1) |
| A patch file that overwrites a user file | Tampering | `create_new`, `--out` too (D-12) |
| Leaking judge data through the sample (answer key in the source tree) | Information disclosure | `judge/` under `[source] exclude`; snapshot judge-digest refusal; sample docs say the key is public in this repository, so the sample demonstrates the mechanism, not secrecy |
| Pricing a run at zero through a template | Repudiation (cost) | Template carries compute rates; empty `[models]` fails closed with `PricesMissing` |
| A stop that silently does nothing | Denial of service of control | D-13 refusal when no driver holds the run; race documented (Pitfall 7) |
| Overclaiming isolation in public docs | Spoofing of assurance | Banned-words gate test; SECURITY.md lists the trust base, the missing `_minos` user, and the `KRUN_BOOT_START_NS` exception |

## Sources

### Primary (HIGH confidence)
- Repository source read this session: `crates/minos-guest/src/{stage.rs,run.rs,linux.rs,pack.rs}`, `crates/minos-core/src/{manifest.rs,money.rs,policy.rs,report.rs,state.rs,limits.rs,sweep.rs,diff/mod.rs,diff/compose.rs}`, `crates/minos-loop/src/{driver.rs,inputs.rs,observe.rs}`, `crates/minos-cli/src/{main.rs,lib.rs,jobs.rs,judge_push.rs,env_build.rs}`, `crates/minos-cli/tests/runner_vm.rs`, `crates/minos-store/src/{lib.rs,file/mod.rs,file/run_lock.rs}`, `crates/minos-source/src/{lib.rs,clean.rs}`, `crates/minos-backend-krun/src/rootfs.rs`, `crates/minos-propose/src/proposer.rs`, `Cargo.toml`, `.cargo/config.toml`, `deny.toml`
- Experiments E1 to E8 (above), run on this Mac 2026-10-28
- `docker buildx imagetools inspect` for the image digests

### Secondary (MEDIUM confidence)
- [Claude Code environment variables](https://code.claude.com/docs/en/env-vars): `CLAUDE_CODE_MAX_OUTPUT_TOKENS` defaults vary by model; 32000 default and 128000 cap for unresolved model IDs
- 06-RESEARCH Pitfall 5 (bundle constants `tE=32000`, `VU=128000` in 2.1.304); `32000` and `128000` constants still present in 2.1.312 (bundle grep)
- 02-SPIKE-EVIDENCE SPIKE-07 envelopes (`modelUsage.claude-haiku-5-5.maxOutputTokens: 128000`; `output_tokens_details.thinking_tokens`)

### Tertiary (LOW confidence)
- [Claude tokenizer guide (fast.io)](https://www.fast.io/resources/claude-tokenizer-token-counting-guide.md), [DEV: token counting](https://dev.to/claudeguide/token-counting-why-your-estimates-are-wrong-and-how-to-fix-them-2g43), [Sourcegraph token utils](https://cdn.jsdelivr.net/npm/@sourcegraph/cody-shared@0.0.10/src/token/utils.ts): 3.5 characters per token heuristic, not an official figure
- [claudeissues: env-vars page outdated](https://claudeissues.com/issue/35133-docs-environment-variables-page-has-outdated-max-output-token-limits), [claudeissues: capped at 32K](https://claudeissues.com/issue/29488-claude-code-max-output-tokens-has-no-effect-on-opus-4-6-capped-at-32k): history of the default
- Anthropic's bundled claude-api skill notes: newer tokenizers count about 30% more tokens than earlier ones; exact counts via `messages.count_tokens`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. No new crates; versions read from the workspace.
- Architecture: HIGH. Every seam signature read from source; gaps G1 to G9 are concrete.
- Toolchain-in-VM mechanics: MEDIUM. A Docker proxy reproduced uid, env, read-only root, tmpfs, and rlimits, but not libkrunfw, SquashFS, or the trampoline.
- Patch tooling: HIGH. Both tools tested on this Mac with bytes from `minos-core`.
- D-08 constants: LOW for bytes-per-token, MEDIUM for the 32,000 cap.
- Pitfalls: HIGH for 1 to 4, 6 to 8, and 11; MEDIUM for 5 and 9.

**Research date:** 2026-10-28
**Valid until:** 2026-11-27 for the code seams; 7 days for anything about the `claude` CLI (it auto-updates)
