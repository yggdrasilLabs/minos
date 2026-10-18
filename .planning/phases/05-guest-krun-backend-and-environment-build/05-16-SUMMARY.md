---
phase: 05-guest-krun-backend-and-environment-build
plan: 16
subsystem: minos-cli
tags: [env-build, docker, buildx, rootfs, squashfs, lock, krun-09, cli-02, d-09, d-10, d-11, d-12]
status: complete
requires:
  - "05-02: minos_core::lock::{Lock, LockedImage, BuilderId, BuilderText, LockError}"
  - "05-05: Blobs::put_reader with PutTooLarge; FileStore::open"
  - "05-09: rootfs::{pack_root_image, PackLimits, PackReport, check_guest_elf}"
  - "05-14: the clap builder command tree, the minos_cli library target, locate::{locate, GUEST_MAX_BYTES}"
provides:
  - "builder::{RootfsBuilder (identity, start), RunningBuild (completed, rootfs_mut, finish), DockerBuildx (find, in_dir, docker, build_argv, child_env), BuilderError, parse_buildx_version, BUILD_DEADLINE, IDENTITY_DEADLINE, BUILDER_STDERR_CAP, DOCKER_BUILDX_NAME}"
  - "env_build::{env_build, EnvBuildOptions (new), EnvBuildReport (image, lock_written, pack, base_images), EnvBuildError, MAX_ROOT_IMAGE_BYTES, MAX_MANIFEST_BYTES, MAX_DOCKERFILE_BYTES, MANIFEST_FILE, LOCK_FILE}"
  - "minos env build [--update] [--allow-dirty]"
  - "tests/env_build.rs: FakeBuilder, Recording<B>, ten pipeline tests, ignored docker_env_build_reproduces"
  - "05-CONTEXT.md Deferred Ideas: D-10 lineage deferred to Phase 7"
affects:
  - "05-18: runs docker_env_build_reproduces with Docker started (MINOS_GUEST set); confirms buildx accepts a piped tar context with -f relative to its root; records the D-09 packer-to-writer streaming follow-up in CLAUDE.md Open questions"
  - "Phase 7: minos run calls Lock::check before a run and decides how the lock digest enters the run's identity (D-10 lineage)"
  - "Phase 8: the sample project's Dockerfile installs toolchains under /usr/local (Pitfall 9)"
tech-stack:
  added: []
  patterns:
    - "Builder seam as a trait with an associated Rootfs reader; RunningBuild::completed for in-process builders, so no trait object"
    - "A watchdog thread owns and reaps the builder child, killing its process group at the deadline or when the RunningBuild is dropped, so no kill reaches a reused pid"
    - "The Dockerfile digest is taken from the snapshot tarball, the exact bytes the builder sees"
    - "Lock rewritten through a temp file in .minos/ (denylisted), synced, renamed, and the project directory synced"
key-files:
  created:
    - crates/minos-cli/src/builder.rs
    - crates/minos-cli/src/env_build.rs
    - crates/minos-cli/tests/env_build.rs
  modified:
    - crates/minos-cli/Cargo.toml
    - crates/minos-cli/src/lib.rs
    - crates/minos-cli/src/main.rs
    - Cargo.lock
    - .planning/phases/05-guest-krun-backend-and-environment-build/05-CONTEXT.md
decisions:
  - "env build rewrites the lock entry without --update when the image digest is unchanged but another recorded input changed (a Dockerfile comment, a builder upgrade), so Lock::check before a run sees the inputs on disk; --update gates only a changed image digest (D-10)"
  - "env build refuses a manifest whose [environment] arch is not arm64: the krun backend boots AArch64 guests only and the buildx argv is fixed to linux/arm64"
  - "A truncated rootfs tar (MalformedTar or Io) from a builder that also failed reports the builder's error; cap refusals report the packer's error"
metrics:
  duration: "about 12 minutes"
  completed: 2026-10-18
  tasks: 3
  files: 8
---

# Phase 5 Plan 16: minos env build Summary

`minos env build` turns the project's Dockerfile into a read-only SquashFS root image and records it in `minos.lock`. The source snapshot is the whole build context, which Docker buildx receives on stdin (argv array, environment cleared to HOME and a fixed PATH). The builder's tar streams into the trusted packer, which writes the checked static AArch64 `minos-guest` last as `/init`. The image is stored with `put_reader` under its digest. A changed digest is refused unless `--update` is passed.

## What was built

- **builder.rs**: the `RootfsBuilder` trait (`identity`, `start`) and `RunningBuild` (`completed`, `rootfs_mut`, `finish`). `DockerBuildx` finds the first executable `docker` in an absolute `PATH` directory and resolves its canonical path. It builds the exact argv `docker buildx build --platform linux/arm64 --provenance=false --sbom=false --output type=tar,dest=- -f <dockerfile> -`, and its environment is exactly `HOME` plus a `PATH` of the docker directory, its canonical directory, and `/usr/bin:/bin:/usr/sbin:/sbin`. The context goes in on stdin from a writer thread. stderr is kept up to 64 KiB, the rest is drained, and it is shown only through `SafeText`. The builder runs in its own process group under a watchdog that owns the child: it kills the group at 30 minutes, or as soon as the `RunningBuild` is dropped without `finish`. `parse_buildx_version` turns `docker buildx version` output into the `BuilderId` `docker-buildx` plus the version token, and refuses control characters and output with no version.
- **env_build.rs**: `env_build<B: RootfsBuilder>` runs these steps in order:
  1. Read the manifest (1 MiB cap) and refuse any architecture other than arm64.
  2. Build the source snapshot (`RequireClean`, or `AllowDirty` with `--allow-dirty`).
  3. Read the Dockerfile out of the snapshot tarball (1 MiB cap), so its digest covers exactly the bytes the builder sees, and check `base_images` pins.
  4. Read the guest (64 MiB cap) and run `check_guest_elf`.
  5. Get the builder identity, start the build, run `pack_root_image` over the streamed stdout, then `finish`.
  6. Store the image with `put_reader` into `.minos/blobs` (`MAX_ROOT_IMAGE_BYTES` = 1 GiB) and take `ImageDigest::from_blob_id`.
  7. Apply the lock rule: an identical entry leaves the lock alone, a different digest without `update` is `LockMismatch` (both digests and "rerun with --update"), and anything else rewrites the lock atomically.

  The module doc states the D-09 limit (an in-memory packed image, bounded by PackLimits' 1 GiB tar cap, then streamed into the store; packer-to-writer streaming is a follow-up), the D-12 reproducibility claim, the D-10 lineage deferral, and Pitfall 9.
- **main.rs**: `minos env build [--update] [--allow-dirty]`. Its long help covers the build context, digest pins, the lock, the ignored `USER`, uid 64000, and `/usr/local` with `CARGO_HOME` and `RUSTUP_HOME`. The guest comes from `locate`; if it is missing, the error is the `Refusal::GuestMissing` fix. On success the command prints the image digest, whether the lock was written, the pack counts, and the base images. A `LockMismatch` prints the locked and built digests on separate lines and exits 1.
- **tests/env_build.rs**: ten tests, all passing, with Docker not needed. They are `first_build_writes_lock` (every D-11 field), `same_build_keeps_lock` (the lock stays byte-identical), `changed_image_refused` (refused, then accepted with `--update`), `context_has_no_judge_or_secrets` (no `judge/`, `.env`, `.minos/`, or `minos.lock` with all four present), `secret_blocks_build`, `dirty_tree_blocks_build_without_allow_dirty`, `wrong_guest_refused` (Mach-O), `tagged_base_image_refused`, `image_has_init_and_mount_points` (`/init` from the guest bytes overriding the tar's own, the seven mount points, `hsqs` magic, the blob rehashes to the report), and `cli_wiring`. The ignored `docker_env_build_reproduces` builds a fixture with a time-stamping `RUN` twice through `DockerBuildx`. It asserts equal digests, every D-11 field, and a clean context.
- **builder unit tests** (7): the exact argv; the environment (HOME and PATH only, with symlink and canonical directories); relative and empty directories; the version parse; and a scripted `docker` that echoes the context back, one that fails with code 3 and an escape sequence on stderr (sanitized), and one that answers `buildx version`.
- **05-CONTEXT.md**: one bullet appended to Deferred Ideas, "D-10 lineage: the lock's digest in `ManifestId`", owned by Phase 7.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | b8f940d | builder trait, Docker buildx, and the env build pipeline |
| 2 | 7e616b4 | env build subcommand and fake-builder pipeline tests |
| 3 | 182ed85 | D-10 lineage deferral recorded in 05-CONTEXT.md |

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (61 result lines, all ok), `cargo deny check` (advisories, bans, licenses, sources ok), `cargo doc --workspace --no-deps`, and the guest musl clippy all pass.
- C-01: all 11 crate manifests inherit the workspace lints (no MISSING line). C-02 prints nothing.
- `cargo test -p minos-cli --lib builder`: 7 passed. `cargo test -p minos-cli --test env_build`: 10 passed, 1 ignored.
- Smoke run: `minos env build` in an empty directory prints `could not read minos.toml` and exits 1.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Same image from changed inputs would leave a stale lock**
- **Found during:** Task 1
- **Issue:** Per the plan, "equal digest leaves the lock unchanged". But a Dockerfile edit that does not change the image (a comment) or a builder upgrade would then leave old `dockerfile_sha256` or builder fields in the lock. `Lock::check` before a run would refuse forever, and `env build` would never fix it.
- **Fix:** The lock is left untouched only when the whole entry is identical. When the image digest is unchanged but another field differs, the entry is rewritten without `--update`. `--update` still gates a changed image digest, which is D-10's rule.
- **Files modified:** crates/minos-cli/src/env_build.rs
- **Commit:** b8f940d

**2. [Rule 2 - Correctness] Arch refusal and the Dockerfile read from the context**
- **Found during:** Task 1
- **Issue:** The plan reads the Dockerfile from disk, and nothing checked the manifest arch. A Dockerfile dropped from the snapshot by an ignore rule would fail only inside the builder, and the hashed bytes could differ from what the builder saw. An `x86_64` manifest would get an arm64 image recorded under the wrong arch.
- **Fix:** The Dockerfile is read from the snapshot tarball. If it is not there, the error is `DockerfileMissing`, which names the path. A non-arm64 manifest is refused with `EnvBuildError::Arch`.
- **Files modified:** crates/minos-cli/src/env_build.rs
- **Commit:** b8f940d

**3. [Rule 2 - Correctness] Deadline enforced by a watchdog, not inside `finish`**
- **Found during:** Task 1
- **Issue:** The plan enforces the deadline in `finish`. But `finish` runs only after the packer reaches EOF, so a builder that hangs without writing would block the packer forever.
- **Fix:** A watchdog thread owns the child from spawn. It kills the process group at the deadline or when the build is abandoned, and it is the only code that reaps the child. Extra `BuilderError` variants `Context` and `Wait` cover a context write failure on a successful build and a wait failure. `lib.rs` declares the two new modules so the integration tests can reach them.
- **Files modified:** crates/minos-cli/src/builder.rs, crates/minos-cli/src/lib.rs
- **Commit:** b8f940d

## Known limits

- D-09: the image is packed in memory (bounded by `PackLimits`' 1 GiB tar cap) and then streamed into the store. Plan 05-18 records packer-to-writer streaming in CLAUDE.md Open questions. This plan does not edit CLAUDE.md.
- D-10: the lineage sentence is deferred to Phase 7, as recorded in 05-CONTEXT.md.
- After a normal exit, the builder's process group is not swept. Docker's buildx plugin exits before the docker CLI does. If a grandchild kept stderr open, `finish` would wait for it. The watchdog's deadline applies only while the direct child runs.
- With Docker down, buildx's handling of a piped tar context with `-f` relative to its root is not yet observed (05-RESEARCH ASSUMED). 05-18 checks it.

## Requirements

KRUN-09 and CLI-02 are left Pending in REQUIREMENTS.md. Plan 05-18 also lists both and owns the run with Docker started that proves them: a real build, the reproducibility evidence, and the piped-context check. This plan delivers the implementation and the tests that need no Docker.

## TDD Gate Compliance

The tasks were marked `tdd="true"`, but the plan is `type: execute`, so the plan-level RED/GREEN gate does not apply. Each task's tests and code went into one commit, with no separate RED commit.

## Threat Flags

None. The builder subprocess and the lock write are the surfaces in the plan's threat model (T-05-16-01 to T-05-16-07), and each is mitigated as listed.

## Self-Check: PASSED
