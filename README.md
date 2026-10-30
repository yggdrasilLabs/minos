# Minos

Minos is a laboratory for autonomous experiment loops. It is written in Rust. It applies the pattern of
[karpathy/autoresearch](https://github.com/karpathy/autoresearch) to more types of project. The pattern is this:
propose a change, run it with a fixed budget, score it, keep it or discard it, then do the sequence again. A project
can be ML training, a performance problem, or an open design question. You describe the project in a manifest, and
Minos runs the loop.

This document is written in ASD-STE100 Simplified Technical English.

Minos exists for three properties. Minos keeps these properties, also when they make the tool less easy to use:

1. **Isolation.** Each experiment runs in its own hardware-virtualized VM. The VM has no network device and no
   shared host directory.
2. **Judge integrity.** The code that Minos scores runs in a different VM from the judge that scores it. The code
   does not see the private data of the judge.
3. **A review gate.** Nothing that the lab makes goes into your working tree by itself. When you accept a result,
   you give `minos adopt` the hash of the report that you read. Then Minos writes a patch file only. You apply the
   patch.

These properties are not perfect. [SECURITY.md](SECURITY.md) tells you what each property depends on, what Minos
trusts, what the preflight examines, and the recorded exceptions.

## Status

The first milestone runs locally on macOS arm64 (Apple silicon). It uses Hypervisor.framework through libkrun. It
has the `sweep` proposer, the `single_shot` proposer, and the greedy policy. These items are for later milestones:

- An AWS deployment: Lambda MicroVMs, a durable orchestrator, and a Bedrock provider.
- GPU projects.
- The `planned` proposer and the `agentic` proposer.

Minos is new. It did not have an external security audit.

## Install

You must have a Mac with Apple silicon, Homebrew, and Docker. This milestone was built and tested on macOS 27.

The short path is the Homebrew formula. It builds Minos from source, signs the helper, and installs the three
binaries:

```sh
brew tap libkrun/krun
brew trust libkrun/krun
brew tap yggdrasilLabs/tap
brew install yggdrasilLabs/tap/minos
minos host doctor
```

The formula installs the pinned Rust toolchain into its build directory. Thus, the build takes some minutes. Then
do `brew pin libkrunfw`, because Minos works only with the libkrunfw release it was built against. After that,
continue at step 5. Steps 1 to 4 are the path for a source checkout, and they need rustup.

1. Install the guest kernel. Minos uses only libkrunfw from the libkrun tap:

   ```sh
   brew tap libkrun/krun
   brew trust libkrun/krun
   brew install libkrunfw
   ```

   Minos loads the kernel from `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`. Minos compares the
   SHA-256 of the file with a pinned value. Thus, version 5.6.2 is necessary.

2. Install Docker with buildx. `minos env build` uses `docker buildx` to make a root image from the Dockerfile of
   your project. Docker Desktop supplies buildx, and Minos was tested with Docker Desktop. A different OCI builder
   can possibly work if it has the Docker CLI with buildx and can export a root filesystem tar. This is not tested.

3. Build Minos. The repository pins its Rust toolchain in `rust-toolchain.toml`: version 1.97.1, with the
   `aarch64-unknown-linux-musl` target. Rustup installs the toolchain when you first use it. From the repository
   root:

   ```sh
   cargo build --release -p minos-cli -p minos-vmm
   cargo build --release -p minos-guest --target aarch64-unknown-linux-musl
   ```

   This puts `minos` and its VM helper `minos-vmm` in `target/release/`. It puts the static guest binary in
   `target/aarch64-unknown-linux-musl/release/`, where `minos` finds it. Add `target/release` to your `PATH`, or
   use the full path of `minos`. Keep `minos-vmm` in the same directory as `minos`.

4. Sign the helper and examine the host:

   ```sh
   minos host setup
   minos host doctor
   ```

   `minos host setup` signs `minos-vmm` with an ad-hoc signature. The signature has the two necessary
   entitlements: the hypervisor entitlement, and the entitlement to load the Homebrew libkrunfw. Each
   `cargo build` that relinks `minos-vmm` removes the signature. Thus, do `minos host setup` again after each
   rebuild.

   `minos host doctor` does each launch probe. Its last line must be `result: every check passed`. It
   shows each path that it examined, and it tells you how to repair a refusal. It also shows
   `dedicated user: missing (deferred, reduced hardening)`. This is correct in this milestone (see SECURITY.md).

5. Set the prices. Minos records usage and calculates dollars from `~/.config/minos/prices.toml`. `minos init`
   writes a starter table there if there is none. In the starter table, compute has the AWS Lambda MicroVM list
   rates. Thus, a local run is not priced at zero. The model table is empty.

   Add the rate of each model that your proposers name. Also add the rate of each resolved model ID that the
   `claude` CLI reports. Examples are `haiku` and `claude-haiku-5-5`. A run does not start if its proposer names a
   model that has no price. To get the starter table without a new project, run `minos init --kind optimize` in an
   empty scratch directory.

6. Prepare the model provider. The `single_shot` proposer uses your own `claude` CLI. The CLI must be the native
   install: `~/.local/bin/claude`, a link into `~/.local/share/claude/versions/`. The version must be 2.1.296 or
   later. The CLI must be logged in with your claude.ai subscription.

   Your shell must not set an Anthropic API key. It must not set a different credential variable or redirect
   variable that the CLI reads. `$TMPDIR` must not be in a repository. It must not be in a project that has its
   own Claude configuration.

   If one of these conditions is not satisfied, Minos does not start a run. Minos shows the name of a refused
   credential variable, but not its value. See [The model provider](#the-model-provider).

## Quick start: the `logparse` sample

`examples/logparse` is an `optimize` project. It is a small Rust log parser. The proposer changes one writable
file, `src/parse.rs`. A Rust judge scores the result against an answer key. The sample
[README](examples/logparse/README.md) describes it in full.

In this repository, the answer key is public. Thus, the sample shows the mechanism that keeps the judge data away
from the scored code. It does not show a secret that is kept.

Run each command from `examples/logparse`:

```sh
minos env build                  # make the root image from minos.Dockerfile and write minos.lock
minos judge push judge           # pack judge/ and record it as [judge] version 1
minos check                      # validate, then judge the baseline three times and show the noise floor
minos run --generations 3        # propose, build, run, and judge candidates up to generation 3
minos status                     # progress, the incumbent, and the approx. cost until now
minos report <exp>               # the review report for one experiment, and its hash
minos adopt <exp> --reviewed <hash>
```

`minos check` shows the facts of the project, the memory budget, the route table, each baseline trial, and the
noise verdict. If you do `check` again with no change, it uses the first result and boots no VM. The text below
comes from a recorded run of this sample on real VMs. In that run, a scripted stand-in replaced the model. Thus,
the token counts are not real:

```text
baseline: run 988820162f00, 3 trials (each boots a candidate VM and then a judge VM)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
```

`minos run` shows one line for each trial, one line for the decision of each generation, and one `approx. cost:`
line for each generation:

```text
g01 proposed single_shot admitted 1 dropped 0
g01 8acd42f0d7f1 single_shot scored                     +231.0%   0m02s
g01 selected improved 8acd42f0d7f1
g01 approx. cost: $0.01 (in 1000 tokens, out 2000 tokens)
```

`minos report <exp>` shows the report: the summary against the baseline, the flags, the provenance, the lineage,
and the diff. Then it shows `report hash: <64 hex>` and the `minos adopt` command that accepts the hash. Read the
report. Its flags show you where to look. They are not verdicts.

`minos adopt` makes the report again, and it rejects each hash that is not the current one. Then it records your
acknowledgment and writes
`minos-<12 hex>.patch` in the project. It does not overwrite a file, and it does not apply the patch. Apply the
patch yourself, from the project directory:

```sh
patch -p1 -i minos-<12 hex>.patch
git apply --directory="$(git rev-parse --show-prefix)" minos-<12 hex>.patch
```

NOTE: Do not use plain `git apply` here. In a subdirectory of a repository, plain `git apply` skips each file and
exits with 0.

## Commands

These commands are available in this milestone:

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

A run or an experiment is identified by a unique lowercase hex prefix, as in git. A command that takes a run uses
the latest run if you give none. `minos run` starts a new run, or it continues the run that `--run` names.
`--generations` is the generation to get to, not a count to add.

`run` exits with 0 when it gets to the target or when `minos stop` stopped it. `run` exits with 2 when a spend
cap, a token cap, the circuit breaker, or the model
provider stopped it. `check` exits with 1 only when something is refused, not for advice. `stop` is refused when
no driver holds the run. `stop` does not kill a VM: a trial that runs completes first.

`status`, `log`, `show`, and `report` do not write the ledger. `status` is not fully read-only: its liveness probe
takes the driver lock of the run for a short time. This makes the empty `driver.lock` file of the run if no driver
held the run before.

These commands are for later milestones and are not available: `minos env shell`, `minos steer`, `minos inbox`,
`minos answer`, `minos params add`, and `minos rejudge`.

## Cost

Each dollar figure that Minos shows has the form `approx. cost: $X.XX`, with the token counts beside it. Minos
calculates the figure from the usage that it measured and the rates in your `prices.toml`. The measured usage is
the VM time on the host, and the token counts that the model call reported. The figure is not a forecast: Minos
does not price the work before the work runs.

Local VM time is priced at the AWS rates of the starter table, as a stand-in. It is not priced at the cost of your
electricity.

Minos compares the spend caps and the token caps in the manifest with the measured spend. Thus, a run can go past
a cap by at most one VM job or one model call, the one that went past the cap.

## The model provider

Minos gets to a model through a `ModelProvider` abstraction. In this milestone, the one implementation runs the
`claude` CLI. It executes only your own unmodified install with your own login. It runs the native binary that it
finds in `~/.local/share/claude/versions/`, not the first `claude` on your `PATH`. It runs the binary from a new
empty directory, with the environment cleared to a short allowlist, all tools off, and safe mode on.

Minos does not read, store, or forward a credential. The login is the login of the CLI, and Minos examines it only
through `claude auth status`. Minos records only token counts, not the dollar figure of the CLI.

Anthropic's terms permit this only when you run your own install with your own subscription. Shared or hosted use
must go through an API key provider or a Bedrock provider. Examples of shared or hosted use are Minos operated for
other people, and Minos operated as a service. The API key provider and the Bedrock provider are later
implementations of the same `ModelProvider` abstraction.

The behavior of the provider was last examined live with `claude` 2.1.312 on 2026-10-28. Its preflight passed, and
it returned proposals. The hooks, the MCP server, and the CLAUDE.md of a hostile fixture project had no effect. The
CLI updates itself. When a later version is installed, do the live test again:

```sh
MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1
```

This test makes three real model calls with your subscription.

## Security

The target is this: an escape from a sandbox must make a zero-day in the hypervisor stack necessary. A mistake in
the code or the configuration of Minos must not be sufficient. Read [SECURITY.md](SECURITY.md) for the trust base,
the preflight checks, the recorded exceptions, and the procedure to report a vulnerability in private.

## License

This software is available under the [MIT license](LICENSE).
