# Minos

Minos is a lab for autonomous experiment loops, written in Rust. It generalizes the pattern in
[karpathy/autoresearch](https://github.com/karpathy/autoresearch): propose a change, run it under a fixed budget,
score it, keep or discard it, repeat. A project can be ML training, a performance problem, or an open-ended design
question; you describe it in a manifest, and Minos runs the loop.

Minos exists for three properties, and protects them over convenience:

1. **Isolation.** Every experiment runs in its own hardware-virtualized VM with no network device and no shared
   host directory.
2. **Judge integrity.** The code being scored runs in a different VM from the judge that scores it, and never sees
   the judge's private data.
3. **A review gate.** Nothing the lab produces reaches your working tree until you pass `minos adopt` the hash of
   the report you read, and even then Minos only writes a patch file for you to apply.

None of these is perfect. [SECURITY.md](SECURITY.md) says what each one rests on, what Minos trusts, what its
preflight checks, and the recorded exceptions.

## Status

The first milestone runs locally on macOS arm64 (Apple silicon), using Hypervisor.framework through libkrun. It
ships the `sweep` and `single_shot` proposers and the greedy policy. An AWS deployment (Lambda MicroVMs, a durable
orchestrator, and a Bedrock provider), GPU projects, and the `planned` and `agentic` proposers are later
milestones. Minos is new and has not had an outside security audit.

## Install

You need a Mac with Apple silicon, Homebrew, Docker, and rustup. This milestone was built and tested on macOS 27.

1. **The guest kernel.** Minos uses only libkrunfw from the libkrun tap:

   ```sh
   brew tap libkrun/krun
   brew trust libkrun/krun
   brew install libkrunfw
   ```

   Minos loads it from `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib` and checks its SHA-256 against
   a pinned value, so version 5.6.2 is required.

2. **Docker with buildx.** `minos env build` turns your project's Dockerfile into a root image with
   `docker buildx`. Docker Desktop provides it and is the setup Minos was tested with; any other OCI builder that
   offers the Docker CLI with buildx and can export a root filesystem tar should work, but is untested.

3. **Build Minos.** The repository pins its Rust toolchain (1.97.1, with the `aarch64-unknown-linux-musl` target)
   in `rust-toolchain.toml`, and rustup installs it on first use. From the repository root:

   ```sh
   cargo build --release -p minos-cli -p minos-vmm
   cargo build --release -p minos-guest --target aarch64-unknown-linux-musl
   ```

   This puts `minos` and its VM helper `minos-vmm` in `target/release/`, and the static guest binary in
   `target/aarch64-unknown-linux-musl/release/`, where `minos` finds it. Put `target/release` on your `PATH`, or
   call `minos` by its full path; keep `minos-vmm` next to `minos`.

4. **Sign the helper and check the host.**

   ```sh
   minos host setup
   minos host doctor
   ```

   `host setup` ad-hoc signs `minos-vmm` with the two entitlements it needs (the hypervisor, and loading the
   Homebrew libkrunfw). Every `cargo build` that relinks `minos-vmm` drops its signature, so run `host setup` again
   after rebuilding it. `minos host doctor` runs every launch probe and must end with `result: every check
   passed`; it names each path it checked and how to fix any refusal. It also reports `dedicated user: missing
   (deferred, reduced hardening)`, which is expected in this milestone (see SECURITY.md).

5. **Prices.** Minos records usage and derives dollars from `~/.config/minos/prices.toml`. `minos init` writes a
   starter table there when none exists: compute is priced at AWS Lambda MicroVM list rates so a local run is
   never priced at zero, and the model table is empty. Fill in the rates of every model your proposers name and
   of every resolved model ID the `claude` CLI reports (for example `haiku` and `claude-haiku-5-5`). A run whose
   proposer names an unpriced model refuses to start. To get the starter table without scaffolding a project you
   want, run `minos init --kind optimize` in an empty scratch directory.

6. **The model provider.** The `single_shot` proposer calls your own `claude` CLI. It must be the native install
   (`~/.local/bin/claude`, a link into `~/.local/share/claude/versions/`), version 2.1.296 or later, logged in with
   your claude.ai subscription. Your shell must not set an Anthropic API key or any other credential or redirect
   variable the CLI reads, and `$TMPDIR` must not lie inside a repository or a project with its own Claude
   configuration. Minos refuses to start a run otherwise, and names a refused credential variable but never its
   value. See
   [The model provider](#the-model-provider).

## Quick start: the `logparse` sample

`examples/logparse` is an `optimize` project: a small Rust log parser whose one writable file, `src/parse.rs`,
the proposer improves, scored by a Rust judge against an answer key. Its
[README](examples/logparse/README.md) explains it in full. In this repository the answer key is public, so the
sample shows the mechanism that keeps judge data from the code being scored, not a secret being kept.

Run every command from `examples/logparse`:

```sh
minos env build                  # build the root image from minos.Dockerfile and write minos.lock
minos judge push judge           # pack judge/ and record it under [judge] version 1
minos check                      # validate, then judge the baseline three times and print the noise floor
minos run --generations 3        # propose, build, run, and judge candidates up to generation 3
minos status                     # progress, the incumbent, and approx. cost so far
minos report <exp>               # the review report for one experiment, and its hash
minos adopt <exp> --reviewed <hash>
```

`minos check` prints the project's facts, the memory budget, the route table, each baseline trial, and the noise
verdict. A second `check` with nothing changed reuses the first and boots no VM. These excerpts come from a
recorded run of this sample on real VMs, in which a scripted stand-in replaced the model, so its token counts are
not real (`.planning/phases/08-cli-sample-project-and-docs/08-EVIDENCE.md`):

```text
baseline: run 988820162f00, 3 trials (each boots a candidate VM and then a judge VM)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
```

`minos run` prints one line per trial, one per generation's decision, and one `approx. cost:` line per
generation:

```text
g01 proposed single_shot admitted 1 dropped 0
g01 8acd42f0d7f1 single_shot scored                     +231.0%   0m02s
g01 selected improved 8acd42f0d7f1
g01 approx. cost: $0.01 (in 1000 tokens, out 2000 tokens)
```

`minos report <exp>` prints the report (summary against the baseline, flags, provenance, lineage, and the diff),
then `report hash: <64 hex>` and the `minos adopt` command that accepts it. Read the report: its flags direct your
attention, they are not verdicts. `minos adopt` rebuilds the report, refuses any hash but the current one,
records your acknowledgment, and writes `minos-<12 hex>.patch` in the project without overwriting anything. It
never applies the patch. Apply it yourself, from the project directory:

```sh
patch -p1 -i minos-<12 hex>.patch
git apply --directory="$(git rev-parse --show-prefix)" minos-<12 hex>.patch
```

Do not run plain `git apply` here: from a subdirectory of a repository it skips every file and still exits 0.

## Commands

Available in this milestone:

```
minos init --kind <tune|optimize|explore>    scaffold minos.toml, program.md, minos.Dockerfile, .gitignore; a starter prices.toml
minos env build [--update] [--allow-dirty]   build the root image and write minos.lock
minos host setup                             sign the minos-vmm helper and verify its entitlements
minos host doctor                            run every launch probe and explain each refusal
minos judge push <dir> [--update]            register judge data under the manifest's [judge] version label
minos check [--fresh] [--allow-dirty]        validate, judge the baseline several times, print routes and advice
minos run --generations N [--run <run>] [--allow-dirty]   start a run, or resume one, up to generation N
minos stop [<run>]                           ask the driver holding a run to stop at its next safe point
minos status [--run <run>] [--full-ids]      progress, liveness, and ledger health
minos log [--run <run>] [--generation N] [--exp <exp>] [--json] [--full-ids]   the ledger, one line per event
minos show <exp> [--run <run>] [--logs] [--full-ids]   one experiment: trials, lineage, rationale, diff
minos report <exp> [--run <run>]             the review report and its hash
minos adopt <exp> --reviewed <hash> [--run <run>] [--out <path>]   acknowledge the report and write the patch
```

Runs and experiments are named by a unique lowercase hex prefix, as in git; a command that takes a run defaults to
the latest one. `minos run` starts a new run unless `--run` names one to resume, and `--generations` is the
generation to reach, not a count to add. `run` exits 0 when it reaches the target or `minos stop` ended it, and 2
when a spend or token cap, the circuit breaker, or the model provider stopped it; `check` exits 1 only when
something refuses, never for advice. `stop` refuses when no driver holds the run, and never kills a VM: a running
trial finishes first.

`status`, `log`, `show`, and `report` never write the ledger. `status` is not strictly read-only: its liveness
probe briefly takes the run's driver lock, which creates the run's empty `driver.lock` file if no driver has held
the run before.

Not yet available (later milestones): `minos env shell`, `minos steer`, `minos inbox`, `minos answer`,
`minos params add`, and `minos rejudge`.

## Cost

Every dollar figure Minos prints has the form `approx. cost: $X.XX`, with the token counts beside it. It is
derived from usage Minos measured (VM time on the host, and the token counts the model call reported) and the
rates in your `prices.toml`. It is never a forecast: nothing is priced before the work runs. Local VM time is
priced at the starter table's AWS rates as a stand-in, not at what your electricity costs. Spend and token caps
in the manifest are checked against measured spend, so a run may pass a cap by at most one VM job or one model
call, the one that crossed it.

## The model provider

Models are reached through a `ModelProvider` abstraction. In this milestone the one implementation runs the
`claude` CLI, and it only ever execs your own unmodified install under your own login: it runs the native binary
it resolves under `~/.local/share/claude/versions/`, never whatever `claude` comes first on your `PATH`, from a
fresh empty directory, with the environment cleared to a short allowlist, all tools off, and safe mode on. Minos
never reads, stores, or forwards a credential; the login is the CLI's own, checked only through `claude auth
status`. Only token counts are recorded, never the CLI's own dollar figure.

Anthropic's terms allow this only for a user running their own install under their own subscription. Shared or
hosted use, such as running Minos for other people or as a service, must go through an API key or Bedrock
provider, later implementations of the same `ModelProvider` abstraction.

The provider's behavior was last checked live on `claude` 2.1.312 on 2026-10-28: its preflight passed, it returned
proposals, and a hostile fixture project's hooks, MCP server, and CLAUDE.md did not take effect. The CLI updates
itself; a later version needs that check rerun
(`MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1`, which
makes three real model calls on your subscription).

## Security

The target is that escaping a sandbox requires a zero-day in the hypervisor stack, never only a mistake in Minos's
code or configuration. Read [SECURITY.md](SECURITY.md) for the trust base, what the preflight checks, the recorded
exceptions, and how to report a vulnerability privately.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your
option.
