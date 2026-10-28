# logparse: the Minos sample project

An `optimize` project. The candidate is a small Rust program that reads `data/input.log` and writes one row of four
fields per line: the timestamp, the level, the component, and the latency. The lines come in three formats and are
messy on purpose (runs of spaces, lowercase and synonym levels, missing latencies). The starting parser in
`src/parse.rs` handles only the tidiest lines, and that file is the only one a proposer may change. `program.md`
is the task the proposer reads.

The judge is a second Rust program in `judge/`. Minos compiles and runs it in its own VM, separate from the
candidate's, and it compares the candidate's output with the answer key `judge/answers.tsv`. It reports three
metrics:

- `rows_match`, a gate: 1 only when the output has one well-formed row per input line.
- `accuracy`, the objective: the share of fields right on the lines whose answers `data/examples.tsv` shows.
- `accuracy_held_out`: the same share on the lines whose answers only the judge has. The report flags a change
  whose visible score rises while this one does not, which is what hard-coding the visible answers looks like.

Both crates use only the standard library, so they build offline in the VM from their committed `Cargo.lock`
files.

## What this sample shows, and what it does not

In a real project the judge's data is a secret: answers, test inputs, or a benchmark the candidate must not see.
Minos keeps it away from the candidate. `judge/` is excluded from the source snapshot and from the image build
context, the judge data is attached only to the judge's VM, and a source file whose bytes equal a judge file
refuses the snapshot.

Here, `judge/answers.tsv` is public, because it is part of this repository. Anyone, including a model, could have
seen it. The sample shows the mechanism that keeps judge data away from the code being scored. It does not show a
secret being kept.

## Before you start

- A Mac set up as the main README describes, with `minos host doctor` passing.
- Docker with BuildKit, for `minos env build`.
- A logged-in `claude` CLI, because the proposer is `single_shot` with the `haiku` model.
- `~/.config/minos/prices.toml` must price `haiku` and the resolved model ID the CLI reports for it (for example
  `claude-haiku-5-5`). A run refuses to start with an unpriced proposer model, and stops after the first call that
  reports an unpriced model.

## Walkthrough

Run every command from this directory.

```sh
minos env build                 # builds the root image from minos.Dockerfile and writes minos.lock
minos judge push judge          # packs judge/ and records it in minos.lock under [judge] version 1
minos check                     # validates the project and judges the baseline several times
minos run --generations 3       # proposes, builds, runs, and judges candidates for three generations
minos status                    # the run's progress, the incumbent, and the cost so far
minos report <exp>              # the review artifact for one experiment, with its report hash
minos adopt <exp> --reviewed <hash>
```

`minos report` prints the report and its hash. `minos adopt` takes that hash. It refuses a hash that does not
match the report as it stands now, records that you acknowledged the report, and writes the patch to
`minos-<12 hex>.patch` in this directory. It never applies the patch.

Apply it yourself, from this directory, with either command:

```sh
patch -p1 < minos-<12 hex>.patch
git apply --directory="$(git rev-parse --show-prefix)" minos-<12 hex>.patch
```

Do not run plain `git apply` here. This directory is inside a repository, and from a subdirectory `git apply`
skips every file and still exits 0, so it looks like it worked when it changed nothing. The `--directory` form
tells git where the project sits in the repository.

`.minos/`, `target/`, `minos.lock`, and the patch files are in `.gitignore`, so the project stays clean for the
next `minos check` or `minos run` after a build or an adopt. The sample does not commit its `minos.lock`, because
the lock records the digest of the `minos-guest` binary on your machine; `minos env build` writes a fresh one. A
project created with `minos init` commits its lock.
