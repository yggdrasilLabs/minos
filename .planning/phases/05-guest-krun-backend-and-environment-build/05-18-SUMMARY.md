---
phase: 05-guest-krun-backend-and-environment-build
plan: 18
subsystem: minos-cli
tags: [env-build, docker, buildx, reproducibility, lock, krun-09, cli-02, d-09, d-10, d-11, d-12]
status: complete
requires:
  - "05-16: builder::DockerBuildx, env_build::env_build, minos env build [--update] [--allow-dirty], the ignored docker_env_build_reproduces"
provides:
  - "05-EVIDENCE.md ## 05-18 env build: daemon versions, the real reproducibility run, CLI runs (unchanged lock, --update refusal and acceptance), and a layer-cache vs --no-cache control"
  - "CLAUDE.md Environment and Root images: the lock's D-11 fields, the --update gate, the shipped buildx argv, the decided D-12 claim, and the D-09 in-memory pack then streamed put"
  - "CLAUDE.md Open questions: packer-to-writer streaming follow-up"
affects:
  - "Phase 7: minos run calls Lock::check and decides the D-10 lineage; minos judge push must refuse judge data over the cap"
  - "Phase 8: the sample project's Dockerfile must not compute file content from the clock if its image is to reproduce without the layer cache"
tech-stack:
  added: []
  patterns:
    - "Evidence of a reproducibility claim includes a control that separates the builder's layer cache from the packer's metadata zeroing"
key-files:
  created:
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-18-SUMMARY.md
  modified:
    - crates/minos-cli/tests/env_build.rs
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md
    - CLAUDE.md
decisions:
  - "A2 fallback not needed: docker buildx build takes the snapshot tarball on stdin with -f relative to its root; DockerBuildx::build_argv is unchanged"
  - "D-12 is stated as: the packed image reproduces whenever the builder's rootfs contents reproduce; equal digests for the fixture's RUN date step come from BuildKit's layer cache, and a --no-cache rebuild yields a new image"
metrics:
  duration: "about 15 minutes"
  completed: 2026-10-19
  tasks: 3
  files: 3
---

# Phase 5 Plan 18: env build against the real builder Summary

`minos env build` ran against Docker Desktop's BuildKit and produced a root image that reproduces. Two builds gave the same digest, `41cfea1b…9276b`. The lock records every D-11 field. A Dockerfile edit is refused without `--update`, which prints both digests, and accepted with it. CLAUDE.md now records the lock contents, the reproducibility claim, and how the image reaches the store.

## Task 1 resolution

Task 1 (checkpoint:human-action, "Start Docker Desktop") was cleared by the user on 2026-10-19 with "Docker is running". The orchestrator verified it, and this executor checked again before Task 2. `docker info --format '{{.ServerVersion}}'` printed `29.6.1`, and `docker buildx version` printed `github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094`. Both exited 0.

## What was done

- **Task 2:** built the musl guest and ran `docker_env_build_reproduces` with `MINOS_GUEST` set. It passed on the first run, so the A2 fallback was not needed. The test now prints both digests and the lock.
  - Ran the CLI four times on a scratch copy of the fixture under the session scratchpad:
    1. Wrote the lock.
    2. Printed `minos.lock: unchanged`. The lock's SHA-256 was the same as before.
    3. Ran after adding a `RUN` line to the Dockerfile, without `--update`. It exited 1, naming the locked and built digests, and the lock was unchanged.
    4. Ran with `--update`. It exited 0 and rewrote the lock.
  - Checked the stored blob: it hashes to its digest, has mode 0600, and sits in the 0700 `.minos`.
  - All of this is recorded verbatim in 05-EVIDENCE.md `## 05-18 env build`.
- **Task 3:** updated CLAUDE.md:
  - **"Environment":** what `minos.lock` records per backend and architecture, the `--update` gate, the rewrite when only other inputs change, `Lock::check` before a run, and D-10's lineage half deferred to Phase 7.
  - **"Root images":** the shipped argv and environment, the snapshot tarball on stdin, attestations off, the uid 64000 rule, the decided D-12 claim with its layer-cache limit, and the D-09 in-memory pack followed by the streamed `put_reader`.
  - **Open questions:** the packer-to-writer streaming follow-up.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | none | Checkpoint cleared by the user (no files) |
| 2 | 2715097 | env build against Docker buildx reproduces its image; evidence recorded |
| 3 | 5aadb64 | CLAUDE.md records the lock, the reproducibility claim, and the image's path to the store |

## Verification

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace`: 62 result lines, all ok, none failed.
- `cargo deny check`: advisories, bans, licenses, and sources ok.
- `cargo doc --workspace --no-deps`: exit 0.
- Guest musl clippy: exit 0.
- C-01: no MISSING line. C-02 printed nothing.
- The ignored Docker test passes with Docker running: 1 passed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The ignored test's doc overclaimed what equal digests prove**
- **Found during:** Task 2
- **Issue:** The doc said the image reproduces "even though its `RUN` step stamps a file with the wall-clock time". But `RUN date > stamp` writes the time into the file's contents. A control run of the builder argv showed what actually happens. Two cached builds give byte-identical tars with the same stamp. Two `--no-cache` builds give different tars and different stamp contents. So the equal digests come from BuildKit's layer cache, not the packer. This is the overclaim T-05-18-03 names.
- **Fix:** Rewrote the test's doc comment to say this, and recorded the control in the evidence. The control includes a `tarfile` comparison: a cached tar and a `--no-cache` tar with the same stamp differ only in mtimes. CLAUDE.md's D-12 text states the claim as "the image reproduces whenever the builder's rootfs contents reproduce". Mtime zeroing is cited from RI-04 and from the production unit test `rootfs_tar_mtimes_do_not_change_the_image`, rerun for the evidence.
- **Files modified:** crates/minos-cli/tests/env_build.rs, 05-EVIDENCE.md, CLAUDE.md
- **Commits:** 2715097, 5aadb64

**2. [Rule 2 - Evidence] The ignored test printed no digests**
- **Found during:** Task 2
- **Issue:** The plan needs both image digests and the lock text recorded verbatim, but the test printed nothing.
- **Fix:** Added `println!` of both digests and the lock text before the assertions.
- **Files modified:** crates/minos-cli/tests/env_build.rs
- **Commit:** 2715097

## Known limits

- Through the production packer, reproducibility without the layer cache is shown at the unit level (`rootfs_tar_mtimes_do_not_change_the_image`) and by Phase 2's RI-04 against the real builder. This plan did not pack two `--no-cache` tars through `minos env build`. The CLI argv has no `--no-cache`, and pruning the user's BuildKit cache to force a cache miss is a host-state change this plan did not make.
- D-09: the packer still returns the whole image in memory (bounded by the 1 GiB tar cap). Streaming it straight into `put_reader` is recorded in CLAUDE.md Open questions.
- D-10 lineage stays deferred to Phase 7.
- Docker build cache entries from these runs remain in Docker Desktop. The scratch fixture remains in the session scratchpad, outside the repository.

## Requirements

KRUN-09 and CLI-02 are proven against the real builder by this plan and are marked complete.

## Self-Check: PASSED

- FOUND: .planning/phases/05-guest-krun-backend-and-environment-build/05-EVIDENCE.md (`## 05-18 env build` at line 1188)
- FOUND: commit 2715097
- FOUND: commit 5aadb64
