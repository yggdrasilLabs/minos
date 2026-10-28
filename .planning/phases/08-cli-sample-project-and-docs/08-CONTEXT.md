# Phase 8: CLI, Sample Project, and Docs - Context

**Gathered:** 2026-10-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 8 is the v1 integration phase. It delivers:

- The user-facing `minos` commands over the Phase 7 driver: `init`, `check`, `run --generations N`, `stop`, `status`, `log`, `show <exp>`, `report <exp>`, and `adopt <exp> --reviewed <hash>` (CLI-01, CLI-05 to CLI-10, JUDGE-02, COST-02). `host doctor`, `host setup`, `env build`, and `judge push` already exist (Phase 5 and 7) and are reused, not rewritten.
- One shipped `examples/` `optimize` project that runs `check`, `run`, `report`, and `adopt` end to end with every experiment in a VM (DOCS-01, DOCS-02).
- `README.md` and `SECURITY.md` that state the threat model, the trust base, what preflight checks, and the `claude` CLI provider's terms, without claiming ironclad isolation (DOCS-03, DOCS-04).
- `cargo clean` after verification passes (GATE-06).

Out of scope: `minos rejudge` (NEXT-02), `minos steer` (NEXT-01), `minos inbox`, `answer`, `params add`, and `env shell` (POL-01 and later milestones), concurrent candidates (NEXT-04), the `planned` and `agentic` tiers (TIER-01), the Bedrock and API-key providers, a speed objective measured by the trusted guest (see D-01), and any change to the ledger wire format. The roadmap marks this phase "standard patterns; skip research-phase".

</domain>

<decisions>
## Implementation Decisions

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Scope and requirements
- `.planning/ROADMAP.md` §Phase 8 — the goal, the five success criteria, "skip research-phase".
- `.planning/REQUIREMENTS.md` — CLI-01, CLI-05 to CLI-10, JUDGE-02, COST-02, DOCS-01 to DOCS-04, GATE-06 (this phase); CLI-02, CLI-03, CLI-04 (already shipped and reused); NEXT-01, NEXT-02, NEXT-04, POL-01, TIER-01 (deferred).
- `.planning/PROJECT.md` — Core Value; Constraints; Key Decisions (CLI provider terms sentence for the README; clap builder API only; done means the full loop on a sample project).

### Design of record
- `CLAUDE.md` — §CLI and §Report and adopt (the command list, `build_report`, the renderer's allowlist); §Projects (the `logparse` manifest sketch the sample follows, the optional keys and defaults); §Judge (scorecard wire form, gate rule, judge data rules); §Cost (`ApproxCost`, "never estimate, never 'exact'"); §minos-runner and minos-loop (`Driver::start`, `resume`, `calibrate`, `run_to`, safe points, D-19 stops with the ordinal rule, the CR-01 judge-scratch rule); §Environments and source (`env build`, `snapshot`, the clean check); §Local backends (the trust base and the recorded exception for SECURITY.md); §Open questions (the Phase 8 items: `RLIMIT_AS`, the batch output cap, `minos stop`); §Commands (the gate and the VM test invocations).
- `rules/` — the ~180 Rust rules; all code must follow them.

### Prior decisions this phase builds on
- `.planning/phases/07-runner-and-loop/07-CONTEXT.md` — D-05 (trials), D-06 (every run calibrates; `check` drives generation 0 on a diagnostic run), D-08 (Conflict is an error), D-10 and D-11 (image and backend pins), D-13 (prices required), D-14 and D-19 (safe points, session-relative stops), and the deferred list naming Phase 8.
- `.planning/phases/07-runner-and-loop/07-REVIEW.md` and `07-REVIEW-FIX.md` — WR-01 (resolved 2026-10-24 by the optional `StopRequested.ordinal`; only the `minos stop` command remains), CR-01 (`JudgeExtent`, `judge_scratch_needed_bytes`), WR-02 (post-call safe point after admission), WR-03 (`ManifestRecordUnreadable`).
- `.planning/quick/261024-i81-wr-01-add-an-optional-ordinal-to-stopreq/261024-i81-SUMMARY.md` — `RecordedStop`, `RunState::next_user_stop_ordinal`, and the key rule `stop` relies on.
- `.planning/phases/06-proposers/06-CONTEXT.md` — D-01 to D-04 (the provider preflight `check` runs), D-10 (full-file proposals), the README terms note; `06-RESEARCH.md` Pitfall 5 (the output cap D-08 advises on).
- `.planning/phases/05-guest-krun-backend-and-environment-build/05-CONTEXT.md` — D-04 (binary locations), D-05 and D-06 (memory budget as manifest facts `check` prints), D-10 (`env build --update`); `05-RESEARCH.md` Pitfall 9 (toolchain prefixes for the sample Dockerfile) and Pitfall 10 (`RLIMIT_AS`).
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md` — D-02 (the clean check and `--allow-dirty` that `run` passes through), D-07 (the hard denylist the sample must not trip).

### Evidence to extend
- `.planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md` and `.planning/phases/07-runner-and-loop/07-EVIDENCE.md` (if present) — the format for recording the D-02 `RLIMIT_AS` measurement, the D-04 judge compile time, and the D-07 boot-to-boot variance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/minos-cli/src/main.rs` — the clap builder skeleton with `host`, `env build`, and `judge push`; new subcommands slot in beside them. `lib.rs` exposes `locate`, `lock_file`, `env_build`, `judge_push`, `blob_source`, and `jobs` (`RunnerJobs<B>`, `judge_inputs` returning the judge data, version, and `JudgeExtent`).
- `crates/minos-loop/src/driver.rs` — `Driver::start`, `resume`, `calibrate` (returns the noise floor and advisory verdict; idempotent), `run_to(target)`; `inputs.rs` `RunInputs`, `check_start`, `check_resume`, `same_frame`. `check` (D-05) and `run` are thin shells over these.
- `crates/minos-loop/src/observe.rs` — `Observer` with `LoopEvent::{Calibrated, Proposed, Judged (objective delta), Selected, Retrying, Stopped}`; the progress line (D-14) is an `Observer` implementation in the CLI, and `NoopObserver` serves `check`.
- `crates/minos-core/src/report.rs` — `build_report(&ReportInput) -> Report` with `text()`, `flags()`, `summary()`, hashed for `ReportHash`; `report` and `adopt` (D-11) both call it.
- `crates/minos-core/src/text.rs` — `SafeText` (allowlist renderer, `inline`), `LogExcerpt::from_bytes`; every CLI line passes through them.
- `crates/minos-core/src/money.rs` — `Prices::parse_toml`, `missing_models`, `Cost::of`, `ApproxCost` (the only dollar formatter, COST-02).
- `crates/minos-core/src/state.rs` — `RunState` accessors: `incumbent`, `decisions`, `lineage`, `is_acknowledged`, `stops`, `next_user_stop_ordinal`, `spends`, `backend_classes`, `last_concluded_generation`; `status`, `show`, and `stop` read these.
- `crates/minos-core/src/policy.rs` — `noise_check` (D-07's verdict), `route` and `ProposerSpec::eligible` (the route table), `circuit_breaker`, `spend_ok`.
- `crates/minos-core/src/diff/` — `CumulativeDiff::recorded` and the canonical serializer (the patch bytes, D-12), `apply` for the report.
- `crates/minos-store/src/file/` — `FileStore` (`runs/<run>/ledger.jsonl`, `driver.lock`, `ledger.lock`, `verified_path`), `RunLock::lock_run`; `stop`'s probe (D-13) and `status`'s liveness (D-15) use `driver.lock`'s `try_lock`.
- `crates/minos-source` — `snapshot` with the judge digest set and the clean check (`run` and `check` pack through it); `pack_judge_data` (the sample's `judge push`).
- `crates/minos-cli/tests/runner_vm.rs` — the ignored VM test harness pattern (fixture image packed by the production packer, judge data pushed by `judge push`) to extend for DOCS-02's end-to-end run on the sample.

### Established Patterns
- Commands are argv arrays; the environment is cleared with allowlists (`env build`, the provider); the same applies to the sample's `cargo` invocations inside the VM.
- Every error type carries IDs, numbers, closed enums, or fixed text, never guest or model bytes; CLI messages are built the same way, with `anyhow` only in `main`.
- Wire formats are frozen under `"v":"1"` with golden snapshots and SHA-256 pins; Phase 8 adds no ledger event and no field.
- Fail closed with the command to run next: `env build` refuses a changed digest without `--update`; `judge push` refuses a changed label without `--update`; D-06 follows the same shape.
- Ignored VM tests under `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW`, run after the build, the signing, and the musl guest build; the sample's end-to-end proof is one of them.

### Integration Points
- `main.rs` dispatch gains `init`, `check`, `run`, `stop`, `status`, `log`, `show`, `report`, `adopt`; each is a module in `minos-cli` composing `FileStore`, `KrunBackend` through `RunnerJobs`, `ClaudeCli::preflight`, and `Driver`.
- `examples/<project>/` is a project directory (`minos.toml`, `program.md`, `minos.Dockerfile`, the candidate crate, visible data) with `judge/` as a `[source] exclude` path holding the judge crate and the answer key; the repo's own gate must not try to build it as a workspace member unless the planner decides it should be one.
- `README.md` and `SECURITY.md` at the repo root; CLAUDE.md's design sections stay where they are until real docs exist (the CLAUDE.md preamble allows moving them into `docs/` once they do; that move is Claude's discretion and not required by this phase).

</code_context>

<specifics>
## Specific Ideas

- The user rejected a shell-and-awk judge outright: everything in Minos, samples and judges included, is Rust. The options for a judge are only different ways to build or deliver a Rust binary.
- Every choice again fell to the fail-closed, explicit side: `check` refuses rather than builds; `stop` refuses rather than records a no-op; `adopt` refuses an existing file rather than overwriting; the hash the user typed must equal the report rebuilt now.
- The sample should read as a real optimize project: a real build, a secret the judge data protects, a held-out metric, and numbers that come only from the harness or from output files.
- Plain text everywhere in the CLI, no color, IDs as 12-hex prefixes, one measured `approx. cost` line.

</specifics>

<deferred>
## Deferred Ideas

- **A speed objective measured by the trusted guest** — PID 1 timing the run stage, a new input-disk section or file carrying the host-trusted number into the judge VM, and runner plumbing; a later milestone, or earlier if a user project needs a throughput objective. Rejected for the Phase 8 sample (D-01).
- **`check` building the image or pushing judge data itself**, or a `--build` flag — rejected (D-06); the build and push stay explicit commands.
- **An adaptive baseline trial count** — rejected (D-07); 07-CONTEXT D-05 stands.
- **Making an over-cap `single_shot` batch ineligible in `ProposerSpec::eligible`** — rejected for v1 (D-08); the advisory may become a gate once the bytes-per-token constant is validated against live calls.
- **`report --ack` as a separate acknowledgment step** — rejected (D-11); revisit only if an inbox or review workflow (POL-01) needs acknowledged-but-not-adopted state.
- **Patches under `.minos/patches/`** — rejected (D-12).
- **`minos rejudge` (NEXT-02), `minos steer` (NEXT-01), `inbox`, `answer`, `params add`, `env shell`** — later milestones, unchanged.
- **Moving CLAUDE.md's design sections into `docs/`** — allowed by the CLAUDE.md preamble once real docs exist; Claude's discretion, not required here.
- **Cross-generation prompt caching via `--system-prompt-file`**, a **model-call deadline knob**, and **recording the `claude` CLI version in the ledger** — carried from 06-CONTEXT, unchanged.

</deferred>

---

*Phase: 08-cli-sample-project-and-docs*
*Context gathered: 2026-10-28*
