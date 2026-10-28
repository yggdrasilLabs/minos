---
phase: 08-cli-sample-project-and-docs
plan: 05
subsystem: sample-project
status: complete
tags: [sample, logparse, manifest, dockerfile, docs, gate]
requires:
  - 08-04 logparse candidate and judge crates, answer key, improved-parser fixture
provides:
  - examples/logparse/minos.toml (the sample manifest)
  - examples/logparse/minos.Dockerfile (rust image pinned by index digest alone, toolchain linked into /usr/local/bin)
  - examples/logparse/.gitignore (.minos/, target/, minos.lock, patch files)
  - examples/logparse/README.md (walkthrough from env build to adopt)
  - crates/minos-cli/tests/sample_manifest.rs (four host gate tests)
  - CLAUDE.md entries for the sample, the stage environment, the digest-only FROM pin, and the sample gate commands
affects: [08-11 init templates, 08-13 sample VM run, 08-14 docs_words scan, DOCS-01]
tech-stack:
  added: []
  patterns:
    - "The sample is checked on the host by copying it into a temp dir and building it with the toolchain cargo test runs under (CARGO, with RUSTC beside it)"
    - "A FROM line names the image by digest alone; the tag lives in a comment"
key-files:
  created:
    - examples/logparse/minos.toml
    - examples/logparse/minos.Dockerfile
    - examples/logparse/.gitignore
    - examples/logparse/README.md
    - crates/minos-cli/tests/sample_manifest.rs
  modified:
    - CLAUDE.md
decisions:
  - "The sample's FROM line is `rust@sha256:8e8cf8f7...` (index digest alone). ImageRef refuses a tag beside a digest, so the planned `rust:1.97.1-slim-trixie@sha256:...` would fail `env build`; the tag is named in a comment"
  - "The sample's minos.lock is gitignored, because it records this machine's minos-guest digest (08-RESEARCH Open Question 3)"
  - "The sample keeps [judge] trials at the default of 3; its objective is deterministic, so the noise floor is zero"
metrics:
  duration: 8m
  completed: 2026-10-28
  tasks: 2
  files: 6
---

# Phase 8 Plan 05: The logparse Sample Project Files Summary

The logparse crates are now a complete Minos project. It has a manifest whose judge argv names no project path, a Dockerfile that pins the rust image by its index digest and links the toolchain into the four-variable stage `PATH`, an ignore file that keeps the project clean after `env build` and `adopt`, and a README walkthrough. A four-test gate target proves the following on the host on every `cargo test --workspace`: the manifest validates, the snapshot holds no judge file, `judge push judge` is accepted, the baseline scores below 1, and the improved parser scores 1.

## What was built

- **`examples/logparse/minos.toml`**
  - Writable: `src/parse.rs`. Readable: `src/main.rs`, `src/record.rs`, `data/input.log`, `data/examples.tsv`.
  - Exclude: `judge/`, `target/`.
  - Build: `cargo build --release --frozen`. Run: `./target/release/logparse data/input.log /out/fields.tsv`.
  - Budgets: build 5m, run 1m, judge 5m, spend_usd 5.
  - Judge argv: `cargo run --release --frozen --quiet --target-dir /tmp/judge-target -- answers.tsv /scratch/candidate/out/fields.tsv /out/scorecard.json`.
  - Gate `rows_match`. Objective `accuracy` (maximize, 5%) with held-out `accuracy_held_out`.
  - Greedy policy, width 2. One `single_shot` `haiku` proposer, batch 2.
  - Memory, scratch, and trials stay at their defaults.
  - Comments explain why the `judge/` exclude, not the argv, keeps the key out of the snapshot, and why the judge builds into `/tmp`.
- **`minos.Dockerfile`**: a comment block giving the four stage variables, uid 64000, and that `ENV` and `USER` never apply. Then `FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c`, then one `RUN` that symlinks the 1.97.1 toolchain's `cargo` and `rustc` into `/usr/local/bin`.
- **`.gitignore`**: `/.minos/`, `/target/`, `/minos.lock`, `/minos-*.patch`, with a comment explaining why the sample's lock is not committed. Its bytes differ from `judge/.gitignore`, so the snapshot's judge-overlap check passes. The gate test proves this.
- **`README.md`**:
  - What the sample is.
  - That `judge/answers.tsv` is public here, so the sample shows the mechanism, not secrecy.
  - Prerequisites, including that `prices.toml` must price `haiku` and the resolved model ID.
  - The walkthrough: `env build`, `judge push judge`, `check`, `run --generations 3`, `status`, `report`, `adopt`.
  - Two ways to apply the patch: `patch -p1` and `git apply --directory="$(git rev-parse --show-prefix)"`.
  - A warning that plain `git apply` from a subdirectory skips every file and still exits 0.
  - It contains none of the banned words (`exact`, `ironclad`, `estimate`).
- **`crates/minos-cli/tests/sample_manifest.rs`**:
  - `sample_manifest_parses`: the writable set is exactly `src/parse.rs`, `judge().paths()` is empty, the gate is `rows_match`, the one objective is `accuracy` paired with `accuracy_held_out`, and `base_images` of the Dockerfile is exactly `rust@sha256:8e8c...`.
  - `sample_crates_have_no_dependencies`: neither `Cargo.toml` has a `dependencies`, `dev-dependencies`, `build-dependencies`, or `workspace` table, and both set `lints.clippy.unwrap_used = "forbid"`.
  - `sample_snapshot_holds_no_judge_file`: on a temp copy, `judge_push` succeeds. The snapshot under the lock's judge digests has no `judge/` entry and does have `minos.Dockerfile`, `src/parse.rs`, and `Cargo.lock`.
  - `sample_scores_on_the_host`: builds both crates with `$CARGO` and the `rustc` beside it, outside the repository. The baseline has `rows_match` 1 and both accuracies below 1. With the fixture as `src/parse.rs`, all three metrics are 1.
  - The copy helper skips `target` and `.minos` and errors on a symlink. Commands are argv arrays only, and every exit status is checked.
- **CLAUDE.md**:
  - Workspace: the sample paragraph, covering the crates, `exclude = ["examples"]`, the literal `[lints]` tables, and `sample_manifest.rs`.
  - Manifest: an absolute judge argument names no project path.
  - Projects: the sample follows the sketch, and its judge is compiled from pushed source.
  - Environments and source: the four-variable stage environment replaces the `CARGO_HOME`/`RUSTUP_HOME` advice; the sample's symlinks; `FROM` is pinned by digest alone.
  - Commands: the six sample gate lines.
  - The C-02 grep is widened to `crates examples`.

## Verification

- `cargo fmt --all --check`: pass.
- `cargo test -p minos-cli --test sample_manifest`: 4 passed.
- `cargo test -p minos-cli --test judge_push`: 12 passed.
- `cargo clippy -p minos-cli --all-targets -- -D warnings`: clean.
- Sample gate, as now listed in CLAUDE.md: fmt `--check` is clean for both crates. Clippy `--all-targets --target-dir target/examples -D warnings` is clean for both. Tests pass: 8 for the candidate and 12 for the judge.
- `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: prints nothing.
- Acceptance greps all print the expected lines, except the `^FROM rust:1.97.1-slim-trixie@sha256:` grep (see Deviations). `grep -c '/usr/local/bin/'` on the Dockerfile is 2. `target/examples` appears in 4 lines of CLAUDE.md.
- No docker run and no VM boot. That work belongs to 08-13.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | a9f5650 | feat(08-05): add the logparse sample manifest, Dockerfile, ignore file, and README |
| 2 | b9845a8 | test(08-05): check the logparse sample in the workspace gate and record it in CLAUDE.md |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The planned `FROM` line would be refused by `env build`**
- **Found during:** Task 2. `sample_manifest_parses` ran `minos_core::image::base_images` on the Dockerfile, and it failed with `Unpinned { line: 12 }`.
- **Issue:** The plan's line was `FROM rust:1.97.1-slim-trixie@sha256:8e8cf8f7...`. `ImageRef::check_name` refuses a tag in the last name segment "even beside a digest, because the pin must be the only identity" (`ImageError::Tag`, which `base_images` surfaces as `Unpinned`). This rule is deliberate and covered by a test in `crates/minos-core/tests/manifest.rs:82` (`alpine:3.20@sha256:...` is rejected). Loosening the parser would weaken the image-pin invariant, so the parser stays as it is.
- **Fix:** `FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c`, the same index digest with no tag. The tag `rust:1.97.1-slim-trixie` is named in a comment. The gate test now asserts that the Dockerfile's single base image is exactly this reference. CLAUDE.md "Environments and source" records the digest-only rule.
- **Effect on acceptance:** the plan's grep `^FROM rust:1.97.1-slim-trixie@sha256:...` prints nothing. The artifact check `contains: "@sha256:8e8cf8f7..."` still holds.
- **Files modified:** `examples/logparse/minos.Dockerfile`, `crates/minos-cli/tests/sample_manifest.rs`, `CLAUDE.md`.
- **Commits:** a9f5650 (Dockerfile), b9845a8 (test and CLAUDE.md).

### Process notes

- **TDD gate, Task 2.** This task tests the artifacts Task 1 had just written, so there was no failing state to commit before them. The tests and the CLAUDE.md edits went in one `test(...)` commit. The one real failure the tests found was the `FROM` refusal above, and it was fixed before Task 1 was committed.
- **Extra coverage beyond the behavior list:**
  - `sample_manifest_parses` also checks the gate set and the Dockerfile's base image through `base_images`.
  - `sample_crates_have_no_dependencies` also refuses a `[workspace]` table.

## Notes for later plans

- **08-11 (`minos init`):** its `DOCKERFILE_TEMPLATE` is specified as `FROM rust:1.97.1-slim-trixie@sha256:...`, which `env build` refuses for the same reason. Use the sample's form, `FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c`, with the tag in a comment. A test that runs `base_images` on the template would catch this.
- **08-13:** the manifest keeps memory, scratch, and trials at their defaults, and has generous build and judge budgets (5m each). Adjust them only if the in-VM measurement requires it.
- **Pitfall 11 is not fixed here:** CLAUDE.md §minos-sandbox and `untrusted.rs` still say the judge reads `/candidate/out/<file>`. This plan did not touch that sentence. The sample uses `/scratch/candidate/out/`.

## Known Stubs

None.

## Threat Flags

None. The files add no new surface beyond the plan's threat register. T-08-05-01 is held by the snapshot test. T-08-05-SC is held by the digest-only pin, which `base_images` checks in the gate. T-08-05-02 is covered by the Dockerfile comment and CLAUDE.md. T-08-05-03 is covered by the README's `--directory` and `patch -p1` forms.

## Self-Check: PASSED

- All five created files exist on disk; CLAUDE.md is modified in b9845a8.
- Commits a9f5650 and b9845a8 are in `git log`.
