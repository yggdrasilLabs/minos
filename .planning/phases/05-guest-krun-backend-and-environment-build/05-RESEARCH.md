# Phase 5: Guest, krun Backend, and Environment Build - Research

**Researched:** 2026-10-18
**Domain:** libkrun microVM backend on macOS arm64 (msb_krun 0.1.41), static musl PID 1 guest, chunked vsock output, Seatbelt confinement, Docker-built SquashFS root images
**Confidence:** HIGH for everything Phase 2 proved on this Mac and for the APIs read from the pinned sources here; MEDIUM for the four new mechanisms this research proposes (console pipe, chunk token, `/init` trampoline, store-path root image under Seatbelt), each of which carries a Wave 0 VM proof.

## Summary

Phase 5 turns the Phase 2 spikes into the real backend. Most of the hard questions were answered on this machine in Phase 2 and are now fixed facts: msb_krun 0.1.41 boots libkrunfw 5.6.2 from a raw read-only SquashFS root (S01-ROOTDISK), the guest has no egress while inet hijack stays off (S02-EGRESS), a deny-default Seatbelt profile allows `hv_vm_create` (SPIKE-05), the helper's exit code never carries the guest's result (K7), one vsock connection carries at most 8192 bytes (DIAG-01), and Docker buildx plus the Rust packer gives byte-identical images across rebuilds (RI-04). The user has locked the remaining design (D-01 to D-15). This phase is integration work across seven crates. Research was needed to find where the locked decisions meet code that does not yet support them, and where a new mechanism opens a hole the spikes never exercised.

The most important finding is a judge-integrity hole that the chunked output protocol opens unless it is closed explicitly. Any guest process, including the unprivileged candidate, can `connect()` an `AF_VSOCK` socket to host CID 2 on the output port. msb_krun routes that connection to the supervisor's Unix socket like any other. Under S03-ANSWER's rules (length plus sequence number only), a candidate can send a complete, well-formed chunk sequence with a forged status. If it then crashes the guest before PID 1 sends (an OOM with only init left panics the kernel, and the helper still exits 0), the host sees exit 0 with exactly one valid stream and classifies the run as "guest decided" on the forged status. The fix stays inside the user's decisions. Every chunk carries a 32-byte per-launch token that the backend generates from `/dev/urandom` and writes only into the input disk. The input disk is a root-only `0600` device node, and PID 1 holds the token in memory that another uid cannot read. A chunk with the wrong token is a candidate fault (`Reason::ChunkProtocol`).

The other findings are integration seams the plans must handle:

- The Seatbelt profile allows metadata reads of path ancestors only for `KRUNFW`, `KRUNFW_LINK`, and `CONFIG`. D-14 moves the root image into `<project>/.minos/blobs/...`, so preflight's `canonicalize` of `ROOT_IMG` would be denied (the same failure as S05-ITER-2) unless the profile gains `(path-ancestors (param "ROOT_IMG"))`.
- `CommandExt::pre_exec` is `unsafe`, so in-guest rlimits need a re-exec trampoline. `/init` re-execs itself as the candidate uid, lowers its own limits with `rustix::process::setrlimit`, then calls `CommandExt::exec`.
- `kill(-1, SIGKILL)` from PID 1 (`kill_process_group(Pid::INIT, Signal::KILL)`) is the safe way to kill every candidate process, including ones that called `setsid`, before packing.
- KRUN-01's "capped console pipe" is buildable without changing the VMM. `ConsoleBuilder::output("/dev/fd/1")` makes the console the helper's stdout pipe, and Seatbelt matches the literal `/dev/fd/1`, which this research verified locally.
- D-06 asks `Manifest::try_from` to check memory against "the `Machine` bounds in `minos-sandbox`". `minos-core` cannot depend on `minos-sandbox`, so those bounds must move into `minos-core::limits`.
- `Backend` has exactly two methods, so `BackendInfo`'s layers and kernel digest must reach the runner through `Capabilities` (or `Finished`).
- `InputDisk(BlobId)` contradicts D-15 ("the input disk is not itself stored as a blob") and must be reshaped.

**Primary recommendation:** Build Phase 5 in four strands that can partly run in parallel: core and store additions, guest PID 1, the backend and helper, and the CLI plus `env build`. Gate them on a Wave 0 VM proof of the three new mechanisms: the authenticated 4 KiB chunk protocol with its throughput measurement, the console on the helper's stdout pipe under Seatbelt, and a root image read from the store path under Seatbelt. Add no new crates. Use `/dev/urandom` (std) for the token, `log 0.4.34` (already in `Cargo.lock`) for the helper's capped logger, and the existing `rustix`, `tar`, `sha2`, and `minos-core` for the guest.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### The `_minos` user layer and `host setup`
- **D-01:** The dedicated unprivileged `_minos` user is **deferred**. Phase 5 runs with hardware virtualization, the Seatbelt profile, and in-guest rlimits as the active layers. `BackendInfo` records the user layer as missing, so every v1 run shows reduced hardening in the report's provenance (03 D-18, `EXPECTED_KRUN_LAYERS`). Nothing in Phase 5 needs `sudo`. The S05-RECIPE stays recorded for the phase that implements it; the sudoers-rule versus LaunchDaemon choice is not made now.
- **D-02:** `minos host setup` **signs the helper and verifies it**: it runs the one documented `codesign` argv from CLAUDE.md Commands against the built `minos-vmm`, then verifies with `codesign -d --entitlements -` that `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation` are present. It also reports libkrunfw and the container builder the way `doctor` does. No xtask, script, `build.rs`, or cargo alias (02 D-11); `setup` is the "may run the same argv from Rust later" that D-11 allowed.
- **D-03:** **Seatbelt is mandatory in v1.** A host where `/usr/bin/sandbox-exec` is missing, or where `doctor`'s denial checks (an outbound TCP connect and an unlisted file read must both be denied under the profile) no longer hold, gets a preflight `Refusal` and `doctor` prints the fix. No run is ever recorded without the Seatbelt layer; there is no "launch and record it missing" path for Seatbelt, unlike the user layer.
- **D-04:** Binaries and the kernel are found at **fixed locations with no config file**: `minos-vmm` and `minos-guest` next to the `minos` binary, or in the cargo target directory for a development checkout; libkrunfw at the Homebrew canonical path (`/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`) checked against its pinned SHA-256 before use (K5). `doctor` names every path it checked. The `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` environment variables remain only for the ignored VM tests, never for production launches.

#### Scratch space for the candidate
- **D-05:** Scratch is a **size-capped tmpfs in guest memory**, chosen as the most secure option that behaves identically locally and on Lambda MicroVMs. Scratch bytes never exist on the host, so "scratch is deleted unread" is structural rather than a cleanup step; every VM starts empty, so build output cannot leak between experiments (KRUN-10); and Lambda MicroVMs attach no per-job block device, so a raw scratch disk would have been a local-only mechanism with a second guest code path. The VM's memory is a **budget**: guest reserve + tmpfs cap + the candidate's allowance, with the candidate's processes under `RLIMIT_AS`, `RLIMIT_FSIZE`, `RLIMIT_NPROC`, and `RLIMIT_CPU`. Over-budget candidates are killed inside the VM and reported as a stage `Outcome`, never as an infrastructure failure. Consequences: KRUN-04's "size-capped scratch disk, all raw images" is amended to "size-capped scratch space in guest memory", the scratch disk leaves the `minos-vmm` launch config (02 D-04 shape, one field fewer), and the Phase 2 scratch round-trip step retargets to the tmpfs. The amendment is recorded in REQUIREMENTS.md and CLAUDE.md in the same change.
- **D-06:** VM memory and the scratch cap are **manifest fields now**, in `[environment]` (working names `memory_mib` and `scratch_mib`), parsed and range-checked in `Manifest::try_from` against the `Machine` bounds in `minos-sandbox`, with defaults, so a project sizes itself and the sample project can declare what it needs. The budget rule (reserve + scratch + allowance must fit the machine) is a `Manifest::try_from` cross-field check. The `RawManifest` table keeps `deny_unknown_fields`.
- **D-07:** The guest gets a **`JobSource` / `OutputSink` abstraction now**: traits in `minos-guest` with the input-disk `JobSource` and the chunked-vsock `OutputSink` implemented in Phase 5, so the AWS milestone adds a lifecycle-hook pair without touching the decoders, the applier, or the command runner. The decoders stay byte-oriented behind the traits; only the Linux entry point names `/dev/vdb` and the vsock port. The host-compilable half keeps building for `aarch64-apple-darwin` (02 D-03).
- **D-08:** **One fixed unprivileged uid/gid** runs both the build and the run commands. The guest creates it in memory (no `/etc/passwd` dependency on the user's image), it owns the scratch mount and `/out`, and build and run share it because build output must be runnable. PID 1 and the output packer stay root and the candidate's process tree is killed before packing, so a candidate cannot race the packer. The Dockerfile `USER` is ignored. Separate uids for build and run were rejected: they buy no host security (the VM is the boundary) and add permission handling between stages.

#### Root images and `minos.lock`
- **D-09:** Built root images live in **`.minos/blobs` through a streaming put**. The `ImageDigest` recorded in `BackendInfo` and `minos.lock` is the blob ID. This needs a `Blobs::put` variant that streams from a reader and hashes as it writes, with the hashing constructor in `minos-core` (the open question about large blobs is closed this way: a streaming SHA-256 builder in core, the file store writing temp-then-rename in the shard as today). The vmm reads the image by its blob path (D-14).
- **D-10:** `minos env build` **refuses when the produced digest differs from `minos.lock` unless `--update` is passed**, like `cargo --locked`. The refusal prints both digests. `--update` rewrites the lock; the next run records a new `ManifestId` lineage because the lock's digest feeds the manifest's image reference.
- **D-11:** `minos.lock` records, **per backend and architecture**: the root image digest, the Dockerfile's SHA-256, every `FROM` base-image digest (as `image::base_images` extracts them), the `minos-guest` binary's SHA-256 that was written in as `/init`, and the builder's name and version. A run refuses to start when the Dockerfile or guest binary on disk no longer matches the lock. The lock is TOML with a `v` tag, `deny_unknown_fields`, `try_from`, and a golden file.
- **D-12:** Reproducibility claim: **the packed SquashFS image must reproduce; the builder's tar may differ.** The packer zeroes every mtime, uid, and gid (RI-04), so an `env build` test builds the same Dockerfile twice and asserts equal image digests. Byte-identical rootfs tars are not required and not claimed in docs, because not every OCI builder honors `SOURCE_DATE_EPOCH`.

#### Blob hand-off to the backend
- **D-13:** A **`BlobSource` trait in `minos-sandbox`**, defined next to `Backend`, is how a backend obtains bytes for the IDs in a `LaunchPlan`: a read-only `get(&BlobId, cap)` and a way to obtain a verified readable path for a large blob. `minos-cli` implements it over `FileStore`; tests use an in-memory map. `deny.toml` layering is unchanged (`minos-store` stays out of backend crates), and `Backend::launch` keeps its signature because `KrunBackend` is constructed with its `BlobSource`. The Lambda backend will implement the same trait over S3.
- **D-14:** A **root image reaches the VM by a read-only path into the store, verified once**: the store returns the blob's path only after its `symlink_metadata` and inode check, `env build` verified the hash when it wrote the blob, and the launch config names that path under a `read_only` raw disk. No copy per job. The Seatbelt profile lists the exact path, as it does today for the two read-only images. A per-launch rehash of a multi-hundred-MB image was rejected as a copy per experiment.
- **D-15:** The **backend writes the per-job input disk** (base tarball, canonical diff, job JSON) into the per-job 0700 job directory from `BlobSource` bytes fetched under caps, temp-then-rename, and deletes it with the job. The runner passes IDs only. The host never unpacks the tarball or applies the diff (04 D-17); the guest does both. The input disk is not itself stored as a blob.

### Claude's Discretion
- The exact rlimit values, the guest reserve, and the default `memory_mib` and `scratch_mib`, each a constant with a reason near the code that enforces it.
- The chunked vsock framing (chunk header layout, sequence numbering, per-chunk and total caps within the S03-ANSWER rules: at most 4 KiB payload per connection, length-prefixed, sequence-numbered, gaps and duplicates are candidate faults), and the throughput measurement the S03-ANSWER asked for.
- The console cap mechanism (polling watchdog with a sized console budget versus a capped writer), given K7-04's overshoot figures.
- The `log` backend `minos-vmm` installs (DIAG-01), and what of the console reaches the report as a `LogExcerpt`.
- The pidfile sweep's layout and the parent-death watchdog's wiring into the supervisor, within KRUN-05.
- `doctor` output wording and ordering, within "every refusal with an explanation and a fix".
- Whether `minos-cli` gets its real clap skeleton in this phase (two subcommands: `env build`, `host doctor`, `host setup`) or a minimal entry that Phase 8 restructures.
- Concurrency defaults: this phase proves two sequential experiments (KRUN-10); `width` and K8 launch-rate and memory measurements stay with Phase 7 and NEXT-04.
- Whether the default virtio-rng device stays attached (open question in CLAUDE.md).
- Whether libkrunfw is copied into a private directory before `dlopen` (open question in CLAUDE.md); either answer is recorded in CLAUDE.md.

### Deferred Ideas (OUT OF SCOPE)
- **Dedicated `_minos` user** (sudoers rule pinned to the helper path, or a LaunchDaemon), with job-directory ACLs and socket permissions: a later phase; the S05-RECIPE is the starting point (D-01).
- **Local-only ext4 scratch disk** with a Rust formatter, for projects whose build output exceeds what guest memory can hold (D-05). Only if tmpfs proves limiting in practice.
- **Concurrent candidates (`width`) and K8 measurements** (launches per second, memory per VM): Phase 7 and NEXT-04.
- **Narrowing the Seatbelt profile's starting allowances** (rows 1 to 6 of the SPIKE-05 answer): still an open question; not required to pass Phase 5.
- **Reporting the three `msb_krun_devices` vsock defects upstream**: the user's call, unchanged since S03-ANSWER.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GUEST-01 | Static musl PID 1, reaps children, creates nothing per-run at startup | Pattern 3 (boot then job), Pitfall 9 (reaping races `Child::try_wait`), existing `rust-lld` musl link (L-MUSL) |
| GUEST-02 | Reads job (base tarball, canonical diff, manifest subset) from read-only input disk, applies with core applier, `Rejected` on mismatch | Pattern 4 (input disk v2 with sections), `minos_core::diff::apply::apply` and `Diff::parse(text, writable, limits)` verified in source; guest gains a `minos-core` dependency (already allowed by `deny.toml`) |
| GUEST-03 | Argv commands as non-root uid under rlimits, writing only to scratch | Pattern 5 (`/init` trampoline: `Command::uid/gid` then `setrlimit` then `CommandExt::exec`), Pattern 6 (one tmpfs at `/scratch`, bind-mounted `/tmp` and `/out`) |
| GUEST-04 | Output is a byte-capped tar over vsock; scorecard only from the judge-role channel | Pattern 2 (authenticated chunk protocol), the existing `Untrusted<GuestOutput<Judge>>::into_scorecard` type gate, and the Security Domain token threat |
| GUEST-05 | No host variables in the guest environment, tested via `env` and `/proc/cmdline` | `build_kernel_cmdline` read in msb_krun source (only `KRUN_BOOT_START_NS` is host-derived); canary-value test design (Validation Architecture) |
| KRUN-01 | One `minos-vmm` per VM, cleared env, stdin `/dev/null`, capped console pipe; only binary linking the VMM and carrying the entitlement | Pattern 7 (console on helper stdout via `/dev/fd/1`, verified under Seatbelt locally), `deny.toml` `msb_krun` wrapper (exists) |
| KRUN-02 | Implicit vsock disabled, plain vsock, hijack and host-directory-sharing APIs banned by clippy | msb_krun `configure_vsock`: no vsock device unless routes or TSI flags exist; `compute_tsi_flags` is empty with `net` off and hijack off (source read). 13 clippy bans exist (D-PROBE). Add `MachineBuilder::rng` usage explicitly; consider banning `ConsoleBuilder::custom` |
| KRUN-03 | Permanent test: guest egress to loopback and public IP fails closed | Promote `spike_net::egress_fails_closed`, run the probe as the candidate uid (Validation Architecture) |
| KRUN-04 (amended by D-05) | Shared read-only root, per-job read-only input disk, size-capped scratch in guest memory; no host directory shared; scratch deleted unread | D-05 tmpfs, vmm config loses `disks.scratch`, Seatbelt loses `SCRATCH_IMG` |
| KRUN-05 | Supervisor kills helper process group at hard max; parent-death watchdog and pidfile sweep; `kill -9` leaves no VM | Existing watchdog (126, DEFECT-01 fix), Pattern 8 (job-dir owner lock plus pidfile sweep), Pitfall 11 (`EPERM` on zombie groups) |
| KRUN-06 | Preflight: entitlement, kernel presence and hash, Hypervisor.framework, socket path length, active layers; refuses with reason | `Refusal` variants exist for five of these; add Seatbelt refusals (D-03); `sysctl -n kern.hv_support` returned `1` here |
| KRUN-07 | Records active layers in `BackendInfo`, never falls back silently | `ConfinementLayer` and `EXPECTED_KRUN_LAYERS` exist; layers flow through `Capabilities` (Pattern 9); Open Question 1 on what `Rlimits` means |
| KRUN-08 | Helper signed by the documented `codesign` argv, verified with `codesign -d --entitlements -` | Existing `has_hypervisor_entitlement` plus `entitled()` XML matcher; extend to the second key (D-02) |
| KRUN-09 | `env build`: Dockerfile to raw read-only SquashFS with `/init` by the trusted packer, digests in `minos.lock`, build context with no judge data and no secrets | Pattern 10 (snapshot tarball as the build context over stdin, `--provenance=false --sbom=false`), existing `rootfs::pack_root_image` |
| KRUN-10 | Two sequential experiments both record outcomes and share no build output | D-05 (fresh tmpfs per VM), a two-run VM test with a marker file |
| CLI-02 | `minos env build` writes `minos.lock` | Pattern 11 (lock type in `minos-core`, golden file) |
| CLI-03 | `minos host doctor` runs launch preflight and prints every refusal with explanation and fix | Pattern 9 (probe once, cached `Capabilities`), `Refusal`'s `Display` already carries the fix |
| GATE-06 | `cargo clean` after verification | Last task of the phase; nothing runs after it |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

These carry the same authority as the locked decisions. A plan that contradicts one is wrong.

- **Never weaken a security invariant to make something compile or pass.** No escape hatch, `pub` field, `Deref`, or temporary bypass on `Untrusted<T>`, `SandboxSpec`, `LaunchPlan`, `Cleared`, or `Finished`. Stop and ask if blocked.
- **Lints:** `unsafe_code = "forbid"`. `unwrap_used`, `expect_used`, and `indexing_slicing` are `forbid` everywhere, tests included. No `#[allow]` or `#[expect]` of any lint (the C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` must print nothing). Pedantic is on.
- **ALL CODE MUST ADHERE TO `rules/`.** That includes `err-thiserror-lib`, `err-anyhow-app` (anyhow only in `minos-cli` main), `api-parse-dont-validate`, `api-must-use`, `doc-all-public`, `anti-unwrap-abuse`, and `anti-index-over-iter`.
- **Deserialization is validation:** `#[serde(try_from)]` plus `deny_unknown_fields` on every raw input (vmm config v2, `minos.lock`, guest job, status file, chunk codec types).
- **Validated types have private fields and no setters.**
- **Errors separate blame:** candidate faults are `Outcome`s, never retried. Infrastructure failures are `LaunchError` and `InfraError`, retried. `SpecError` is configuration.
- **Order must be stable:** `BTreeMap` and `BTreeSet`, never `HashMap` (clippy `disallowed-types` in core, store, and sandbox).
- **No C in our crates.** No OpenSSL, no libgit2, no git in the trusted path. **No tokio in any `minos-*` crate** (`deny.toml` fences it to `msb-imago`).
- **Commands are argv arrays,** never shell strings (docker, codesign, sandbox-exec, sysctl, ps, and guest commands alike).
- **Supply chain:** `Cargo.lock` committed, `cargo deny check` gates every build, git deps pinned by `rev`, and every new `[workspace.dependencies]` entry must be used (`unused = "deny"`).
- **No xtask, script, `build.rs`, or cargo alias.** Signing is the one documented `codesign` argv, run after every build of `minos-vmm`, before any VM run.
- **Check external APIs against their docs before writing code.** The msb_krun, rustix, vsock, tar, and backhand facts below were read from the sources in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.
- **Keep CLAUDE.md current in the same change** (KRUN-04 amendment, scratch disk removal, console mechanism, chunk protocol, rng and libkrunfw-copy decisions, resolved open questions, new core modules).
- **Gate commands:** `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `cargo deny check`; `cargo doc --workspace --no-deps`; `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`.
- **Run `cargo clean` at the end of the phase,** after verification passes (GATE-06).
- **User memory rule:** before any dependency fork or `[patch]`, prove the mechanism and rule out Minos configuration first. Nothing in this research needs a fork.

## Architectural Responsibility Map

The tiers here are not web tiers. They are the trust zones of the design of record.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Job wire types, status file type, chunk codec, input-disk layout, lock type, streaming hasher | `minos-core` (pure, no I/O) | — | One implementation shared by guest and host instead of two bound by golden bytes. Core is the only crate both sides may link |
| Streaming blob put; verified blob path | `minos-store` | `minos-core` (hasher) | Store owns files and the symlink and inode checks (D-09, D-14) |
| `BlobSource` trait, `Capabilities` fields, `Refusal` variants, `Finished` additions, `InputDisk` reshape | `minos-sandbox` | — | Next to `Backend` (D-13). The type invariants live here |
| Job dir, input disk writer, launch config, Seatbelt argv, supervisor, chunk reassembly, classification, pidfile sweep, host probes, root-image packer | `minos-backend-krun` | — | Only crate that knows `minos-vmm` and the krun specifics |
| VM assembly, preflight, watchdog, capped logger, own rlimits | `minos-vmm` (signed, entitled, Seatbelt-confined) | — | Only binary linking msb_krun (KRUN-01) |
| PID 1: mounts, job read, untar plus apply, trampoline, rlimits, kill-all, pack, chunked send, power-off | `minos-guest` (in-VM, root) | — | The guest is the only place source is unpacked and diffs are applied (04 D-17) |
| Candidate build and run | Guest, unprivileged uid | — | Untrusted. Only reaches scratch tmpfs, `/out`, `/tmp` |
| `env build`, `host doctor`, `host setup`, `BlobSource` over `FileStore`, binary location | `minos-cli` | `minos-backend-krun` (packer, probes) | The CLI is the only place concrete types meet (`deny.toml`) |

## Standard Stack

### Core

No new crates. Every crate below is already pinned in `[workspace.dependencies]` and present in `Cargo.lock`. Phase 5 adds edges and one feature.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `msb_krun` | =0.1.41, `default-features = false`, `["blk"]` | VMM in `minos-vmm` | Boots here (S01-ROOTDISK). Builder API verified in `src/api/builders.rs`: `MachineBuilder::{vcpus, memory_mib, balloon, rng, msb_metrics}`, `VsockBuilder::unix_connect`, `ConsoleBuilder::output`, `DiskBuilder::{path, read_only}`, `KernelBuilder::{krunfw_path, cmdline, init_path}` [VERIFIED: msb_krun 0.1.41 source] |
| `rustix` | =1.1.5 | Guest: `mount`, `mount_bind`, `setrlimit`, `kill_process_group(Pid::INIT, …)`, `wait`/`waitpid`, `reboot`. Host: `setrlimit` in `minos-vmm`, `poll` in the supervisor | All safe. `process::setrlimit` is compiled for Apple too (the `rlimit` module is gated only off `espidf`, `fuchsia`, `horizon`, `redox`, `vita`, and `wasi`). `mkfifoat` and `mknodat` are **not** available on Apple [VERIFIED: rustix 1.1.5 source] |
| `vsock` | =0.5.4 | Guest `VsockStream::connect_with_cid_port(VMADDR_CID_HOST, port)` | Already used by the spike guest [VERIFIED: vsock 0.5.4 source] |
| `tar` | =0.4.46, no default features | Guest: read base tarball and judge data; write output tar with hand-built ustar headers | Same crate and header rules as `minos-source` and the host reader |
| `minos-core` | workspace | Guest: `Diff::parse`, `diff::apply::{Tree, apply}`, `Limits`, `RelPath`, and the new shared wire modules | `deny.toml` already lists `minos-guest` as an allowed `minos-core` wrapper. Core's deps (serde, serde_json, toml, sha2, base16ct, diffy, rand_chacha, rand_core) are pure Rust and in the musl-checked graph |
| `log` | =0.4.34 (already in `Cargo.lock` via msb_krun) | Capped stderr logger in `minos-vmm` (DIAG-01) | msb_krun logs through `log`. With no logger installed, every warning is discarded, including "error reading TX packet" |
| `backhand` | =0.25.5, `["gzip"]` | SquashFS packer (exists in `rootfs.rs`) | Proven reproducible (RI-04) |
| `sha2` | =0.11.0 | Streaming `BlobHasher` in core; guest verifies section digests | Existing |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` | =4.6.7, builder API only | `minos env build [--update]`, `minos host doctor`, `minos host setup` | Real skeleton now (discretion). Phase 8 adds subcommands to it |
| `tempfile` | =3.27.0 | Streaming put staging; job dir; entitlements file for `codesign` | Existing |
| `serde`, `serde_json`, `toml`, `thiserror`, `base16ct` | pinned | Wire types, lock, errors | Existing |
| `insta`, `proptest`, `trybuild` | pinned | Golden files (vmm config v2, lock, input disk v2, chunk codec, status file), properties (chunk reassembly, trampoline argv), compile-fail (`BlobSource` cannot mint `Cleared`) | Existing |

### Feature and edge changes

| Change | Where | Why |
|--------|-------|-----|
| Add `"event"` to the workspace `rustix` features | `Cargo.toml` | `rustix::event::poll` for the supervisor's accept and read waits instead of the 10 ms and 1 ms sleep polls (throughput, Pitfall 6). `poll` is available on Apple [VERIFIED: rustix 1.1.5 `src/event/poll.rs`] |
| `minos-guest` gains `minos-core`, `tar`, `sha2` (host-compilable) | `crates/minos-guest/Cargo.toml` | GUEST-02. Keep `rustix` and `vsock` under the `cfg(target_os = "linux")` table |
| `minos-vmm` gains `log` and `rustix` | `crates/minos-vmm/Cargo.toml` | Capped logger; own rlimits at `main` entry |
| Add `log = "=0.4.34"` to `[workspace.dependencies]` | `Cargo.toml` | Exact pin like every other dependency |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Per-launch bearer token in each chunk | HMAC per chunk (`hmac` crate) | HMAC also protects against a reader that can see the bytes, but nothing in the guest except PID 1 and the kernel can observe a vsock stream. A token adds no dependency. Revisit if a later backend sends output through a channel another party can read |
| Token from `/dev/urandom` (`std::fs`) | `getrandom` crate | `std::random` is still unstable on 1.97.1 (E0658, verified by compile). `/dev/urandom` is a regular file read and adds no dependency |
| Console on helper stdout (`/dev/fd/1`) | Keep the console file plus 50 ms watchdog | The file path is proven (K7-04) but overshoots by 30 to 49 KB and is not a "pipe" as KRUN-01 words it. Keep it as the fallback if Wave 0 fails |
| `/init` re-exec trampoline for rlimits | `CommandExt::pre_exec` | `pre_exec` is `unsafe`, so it is forbidden |
| `kill(-1)` via `kill_process_group(Pid::INIT, …)` | Process groups only | A candidate can `setsid()` or `setpgid()` out of its group; `kill(-1)` from root reaches every process except PID 1 |
| Cgroups `cgroup.kill` (ARCHITECTURE.md Pattern 5) | — | libkrunfw is built with `CGROUPS` off (STACK), so it is not available |

**Installation:** nothing to install. Edits only:

```toml
# Cargo.toml [workspace.dependencies]
rustix = { version = "=1.1.5", features = ["fs", "mount", "process", "system", "event"] }
log    = "=0.4.34"
```

**Version verification:** `cargo search log --limit 1` printed `log = "0.4.34"`. Every other version is already locked in `Cargo.lock` and pinned in the workspace.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| log | crates.io | since 2014-12-13 | ~25.7M/wk | github.com/rust-lang/log | [OK] | Approved. Already in `Cargo.lock` as a transitive dependency of msb_krun; this phase only makes the edge direct |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

No other new package. `rustix`, `vsock`, `tar`, `sha2`, `backhand`, `minos-core`, and `tempfile` are existing pins audited in Phases 1 to 4.

## Architecture Patterns

### System Architecture Diagram

```
                    minos-cli (unsigned, user's shell)
                      │  env build            │  host doctor / setup         │  (Phase 7: runner)
                      ▼                       ▼                              ▼
  snapshot tar ──► docker buildx ──► rootfs tar ──► packer (/init last) ──► streaming put ──► .minos/blobs/sha256/..  ──► minos.lock
  (minos-source:   (argv, env         (stdout)        minos-backend-krun     (core BlobHasher)     (ImageDigest = BlobId)
   no judge data,   allowlist,                                                                    │
   secret-scanned)  stdin context)                                                                │ verified path (D-14)
                                                                                                  ▼
  LaunchPlan<R> ──► preflight(Capabilities) ──► Cleared<R> ──► KrunBackend::launch ─────────────────────────────┐
  (IDs only)        (cached host probes:                         │ BlobSource.get(base, diff, job, judge data)  │
                     entitlement, krunfw hash,                   │ token ← /dev/urandom (32 B)                  │
                     hv_support, sandbox-exec,                   ▼                                              │
                     Seatbelt denials)            0700 job dir /private/tmp/minos-<uid>/mnsXXXX/                │
                                                  input.img (header+token+sections), vmm.json v2,              │
                                                  o.sock (bound), owner.lock (held), pid                       │
                                                                 │                                              │
                       /usr/bin/sandbox-exec -p <profile> -D … minos-vmm vmm.json                               │
                       env cleared, stdin /dev/null, stdout = console pipe, stderr = pipe, own pgrp            │
                                                                 ▼                                              │
                    ┌──────────────────── minos-vmm (signed, Seatbelt, own rlimits, capped log) ─────────────┐ │
                    │ preflight → msb_krun VM: vda root (ro) · vdb input (ro) · vsock 1024→o.sock · console  │ │
                    │ → /dev/fd/1 · rng · no net · no fs · watchdog (deadline 124, parent-gone 126)          │ │
                    └──────────────┬────────────────────────────────────────────────────────────┬─────────┘ │
                                   ▼  (Linux guest, PID 1 = minos-guest, root)                    │ console    │
       boot: mount /proc (hidepid) /sys(ro) ── JobSource(/dev/vdb) ── tmpfs /scratch (scratch_mib) │ bytes      │
       untar base → /scratch/src · Diff::parse + apply (Rejected on mismatch)                      ▼            │
       stage(build) ─► /init trampoline (uid 64000, setrlimit, exec argv) ─► kill(-1) ─► reap      supervisor  │
       stage(run)   ─► same ─────────────────────────────────────────────► kill(-1) ─► reap      threads:    │
       pack tar(.minos/status.json, logs, out/**) ── OutputSink: chunks ≤4 KiB, token, seq ──────► accept/poll ─┘
       power off                                         one vsock connection per chunk           reassemble,
                                                                                                    verify token,
                                                                                                    seq, END digest
                                                                                                         ▼
                                    classify (K7 rule, refined) ──► Finished<R>{Ended, stream} ──► into_output → Untrusted
```

### Recommended Project Structure

```
crates/minos-core/src/
├── guest.rs            # NEW: guest job v2 (role, writable, argv, budgets, scratch, uid), status file type, input-disk section layout
├── chunk.rs            # NEW: chunk header codec (magic, token, seq, kind, len), END trailer, reassembly state machine (pure)
├── lock.rs             # NEW: minos.lock v1 (TOML, try_from, deny_unknown_fields), golden file
├── id.rs               # + BlobHasher (streaming SHA-256 -> BlobId / ImageDigest)
├── limits.rs           # + MIN/MAX_MEMORY_MIB, MAX_VCPUS, scratch and guest reserve bounds (moved from sandbox; sandbox re-exports)
└── manifest.rs         # + [environment] memory_mib, scratch_mib with defaults and the budget check
crates/minos-store/src/file/blobs.rs   # + put_reader (stream, hash, temp in staging dir, rename into shard), verified_path
crates/minos-sandbox/src/
├── blob_source.rs      # NEW: BlobSource trait, BlobSourceError, VerifiedPath newtype
├── spec.rs             # InputDisk reshaped (base, diff, job blob; judge data stays separate)
├── preflight.rs        # + Refusal::{SeatbeltMissing, SeatbeltNotEnforcing, VmmMissing, GuestMissing, …}; Capabilities + layers, kernel, backend kind
└── finished.rs         # + host-measured elapsed; Ended variant for chunk-protocol faults; capped console bytes as Untrusted
crates/minos-vmm/
├── src/{config.rs (v2: no scratch; console pipe), launch.rs (+ .rng(true) explicit), log.rs (NEW capped logger), main.rs (own rlimits)}
└── tests? none (unit tests in-module); golden config file
crates/minos-backend-krun/src/
├── backend.rs          # NEW: KrunBackend { paths, blob_source, caps }, impl Backend, RunningVm
├── host.rs             # NEW: probes (codesign, krunfw hash, kern.hv_support, sandbox-exec, Seatbelt denial checks)
├── seatbelt.rs         # NEW: profile text (include_str!), argv builder, profile SHA-256
├── jobdir.rs           # NEW: /private/tmp/minos-<uid>/ layout, owner lock, pidfile, sweep
├── chunks.rs           # NEW: accept loop + reassembly over minos_core::chunk
├── input_disk.rs       # v2 writer (sections + token)
├── launch_config.rs    # v2
├── rootfs.rs           # + /out and /scratch mount points; stream file data (Phase 2 note)
└── supervise.rs        # threads for console pipe, stderr pipe, chunks; classification
crates/minos-guest/src/
├── lib.rs, job.rs (v2 decode via core), source.rs (JobSource trait + DiskJobSource), sink.rs (OutputSink + ChunkSink),
├── stage.rs (trampoline argv, capture), pack.rs (output tar from /out, O_NOFOLLOW walk)
└── linux.rs            # entry, mounts, trampoline mode, kill-all, power-off
crates/minos-cli/src/{main.rs, env_build.rs, host.rs, locate.rs, blob_source.rs}
```

### Pattern 1: Phase order and the Wave 0 proof

**What:** Three mechanisms in this phase have no Phase 2 evidence: the authenticated chunk protocol at volume, the console on `/dev/fd/1` under the deny-default profile, and the root image read from a store path under Seatbelt. Prove each one with an ignored VM test before the production code depends on it, and record the run in a `05-EVIDENCE.md` in the Phase 2 evidence style (command, exit code, verbatim output).
**When to use:** The first plan of the phase, before the backend plan.
**Wave 0 runs:**
1. **Chunk throughput:** the guest sends 16 MiB (the output cap) as ≤4 KiB authenticated chunks; the host reassembles. Record the elapsed time, the connection count, and that no gap or refused connect occurred. S03-ANSWER asked for this measurement.
2. **Console pipe under Seatbelt:** `console.path = "/dev/fd/1"`, helper stdout a pipe, profile literal `/dev/fd/1`. Boot, flood past the cap, confirm the supervisor reads exactly `cap + 1` bytes and classifies console-cap.
3. **Store-path root image:** `ROOT_IMG` under a project's `.minos/blobs/...` with `(path-ancestors (param "ROOT_IMG"))` added. Boot succeeds, and without the ancestor rule preflight fails (exit 66, "Operation not permitted").

### Pattern 2: Authenticated chunk protocol (S03-ANSWER plus token)

**What:** The reassembled stream is exactly the output tar. PID 1's status is the file `.minos/status.json` inside it, beside `.minos/build.log`, `.minos/run.log`, `.minos/scorecard.json` (judge role only), and `out/**`. The `Untrusted` layout constants already reserve the `.minos/` prefix. Each vsock connection carries one chunk.

```
DATA chunk (one connection, ≤ 4144 bytes total, under the 8192 limit)
  0   8  magic        b"MINOSCK1"
  8  32  token        per-launch, from the input disk; never stored, never logged
 40   4  seq          u32 BE, 0-based, DATA and END share one counter
 44   1  kind         0 = DATA, 1 = END
 45   1  reserved     0
 46   2  len          u16 BE, ≤ 4096 for DATA
 48 len  payload
END chunk payload (len = 44): total_len u64 BE ‖ sha256(concatenated DATA payloads) 32 B ‖ data_chunks u32 BE
```

**Host rules:**
- A connection whose header is short, whose magic or token is wrong, whose `len` exceeds 4096, or whose length disagrees with the bytes received is `Reason::ChunkProtocol`, a candidate fault. Only a guest process other than PID 1 can produce a wrong token.
- A duplicate `seq`, a gap at END, a second END, an END whose totals or digest disagree, a total over `MAX_OUTPUT_STREAM_BYTES`, or more connections than `ceil(MAX_OUTPUT_STREAM_BYTES / 4096) + 1` is also `ChunkProtocol`.
- Out-of-order arrival is tolerated by buffering by `seq` under the total cap. A reorder caused by the VMM must not be blamed on the candidate.
- Compare the token in constant time over all 32 bytes. Timing does not matter much here, because the first mismatch ends the run.

**Why each defect stays dormant:** H1 because each connection is under the 8 KiB send buffer, H2 because each payload is under 32 KiB, and H3 because each connection is under 256 KiB (S03-ANSWER).

**Guest rules:**
- Connect, write the whole chunk, `shutdown(Write)`, close.
- On `ECONNREFUSED` or `ECONNRESET` at connect, retry the same chunk with bounded backoff, because the host listener backlog can fill.
- Send the END chunk last.

### Pattern 3: PID 1 lifecycle (GUEST-01, D-07)

**What:** `main` (PID 1 only; check `std::process::id() == 1`) does boot-time work that is the same for every run. Per-run state is created only after the job arrives, which keeps the Lambda snapshot rule ("create nothing per-run at startup").

```
boot():  console BOOT_MARKER; mount proc (nosuid,nodev,noexec, hidepid=invisible); mount sysfs ro
run(src: &mut impl JobSource, sink: &mut impl OutputSink):
  job   = src.read_job()?                         // v2 decode in core; sections hashed and checked
  mount tmpfs /scratch size=<scratch_mib>m,mode=0755; mkdir src home tmp out (chown uid)
  bind /scratch/tmp -> /tmp; bind /scratch/out -> /out
  untar base -> /scratch/src (regular files, RelPath, 0644/0755, create_new)
  apply   = Diff::parse(diff, writable, limits) then apply(tree_of_touched_files, &diff)
            failure -> status{apply: Failed{path,hunk}} and skip stages
  for stage in role.stages(): run_stage(trampoline) ; kill_all() ; reap_all()
  tar     = pack(/out, logs, status)              // after kill_all: nothing can race the walk
  sink.send(&tar)?                                 // chunked, authenticated
power_off()
```

**Linux entry only** names `/dev/vdb` and port 1024. `JobSource` and `OutputSink` are traits in `minos-guest` with in-memory test impls on the host.

### Pattern 4: Input disk v2 (D-15)

**What:** One read-only disk per job, written by the backend from `BlobSource` bytes under caps, temp-then-rename in the 0700 job dir, deleted with the job.

```
header:   b"MINOSJB2" | u32 version | u32 section_count | token[32]
table:    section_count × { u32 kind, u64 offset, u64 length, [u8;32] sha256 }   (kinds: JOB=1, BASE=2, DIFF=3, JUDGE_DATA=4, CANDIDATE_OUT=5)
sections: each starts on a 512-byte boundary, zero padding between
```

The guest verifies each section's SHA-256 before use and checks that `BASE` hashes to the job's `TreeId`. A mismatch is a host bug and an infrastructure failure, never a candidate fault. The table layout lives in `minos-core::guest` so the writer and reader share one codec. `InputDisk` in `minos-sandbox` becomes the IDs of those parts (base `BlobId` = `TreeId::to_blob_id()`, optional diff `BlobId`, guest-job `BlobId`). Judge data stays the separate `JudgeData` that only `SandboxSpec<Judge>::secrets` can attach.

### Pattern 5: The `/init` trampoline (GUEST-03, no `unsafe`)

**What:** PID 1 spawns `/init --minos-stage <limits…> -- <argv…>` with `Command::uid(UID).gid(GID)` (std drops supplementary groups when the parent is root and no groups were set), `env_clear()` plus a fixed environment, `current_dir`, `process_group(0)`, stdin null, stdout and stderr piped. The trampoline (not PID 1) lowers its own rlimits, which any process may do, then `CommandExt::exec`s the argv. rlimits survive `exec`.

```rust
// Source: rustix 1.1.5 src/process/rlimit.rs (setrlimit), std::os::unix::process::CommandExt::exec
use std::os::unix::process::CommandExt as _;
use rustix::process::{Resource, Rlimit, setrlimit};

fn stage_exec(limits: &StageLimits, argv: &[String]) -> std::io::Error {
    for (resource, value) in [
        (Resource::As, limits.as_bytes()),
        (Resource::Fsize, limits.fsize_bytes()),
        (Resource::Nproc, limits.nproc()),
        (Resource::Cpu, limits.cpu_secs()),
        (Resource::Nofile, limits.nofile()),
        (Resource::Core, 0),
    ] {
        let limit = Rlimit { current: Some(value), maximum: Some(value) };
        if let Err(errno) = setrlimit(resource, limit) {
            return std::io::Error::from(errno);
        }
    }
    let Some((program, args)) = argv.split_first() else {
        return std::io::Error::from(std::io::ErrorKind::InvalidInput);
    };
    std::process::Command::new(program).args(args).exec() // returns only on failure
}
```

A candidate can also run `/init --minos-stage …`, but only as its own uid with limits it may only lower. The trampoline must refuse to run as PID 1 or as root.

### Pattern 6: Kill everything, then pack (D-08)

```rust
// Source: rustix 1.1.5 src/process/kill.rs: kill_process_group(Pid::INIT, sig) is kill(-1, sig):
// "sends a signal to all processes the current process has permission to send signals to, except process Pid::INIT"
use rustix::process::{Pid, Signal, WaitOptions, kill_process_group, wait};

fn kill_all_and_reap(deadline: std::time::Instant) -> bool {
    let _ = kill_process_group(Pid::INIT, Signal::KILL); // ESRCH when nothing is left is fine
    loop {
        match wait(WaitOptions::empty()) {
            Ok(Some(_)) => continue,
            Ok(None) | Err(rustix::io::Errno::CHILD) => return true, // no children: nothing can race the packer
            Err(_) if std::time::Instant::now() < deadline => continue,
            Err(_) => return false,
        }
    }
}
```

The output walk then needs no race defense. It still opens every file with `O_NOFOLLOW` and refuses anything but a one-link regular file with a `RelPath` name under the host reader's caps, because `/out` was written by the candidate.

### Pattern 7: Console on the helper's stdout pipe (KRUN-01)

**What:** `launch.rs` keeps `.console(|c| c.output(config.console()))` with `console = "/dev/fd/1"`. msb_krun opens it with `File::create`, gives the console `input_empty()`, and attaches no `krun-stdin` or `krun-stdout` port (source read: `autoconfigure_console_ports`). The supervisor spawns the helper with `stdout(Stdio::from(PipeWriter))` from `std::io::pipe()` (stable on 1.97.1, verified by compile) and drains the reader on its own thread through `take(cap + 1)`.

**Verified locally:** under `(deny file-write* (subpath "/dev"))`, opening `/dev/fd/1` and `/dev/stdout` (which Seatbelt resolves to `/dev/fd/1`) was denied. With `(allow file-write* (literal "/dev/fd/1"))` added, both succeeded and the bytes reached the pipe. A literal `/dev/stdout` rule did not match [VERIFIED: local sandbox-exec run, allow-default base profile]. The deny-default profile needs a Wave 0 VM boot.

**Consequences:**
- The console never touches host disk.
- The cap is exact.
- The watchdog's file-length console check no longer applies, because `metadata("/dev/fd/1")` describes the pipe. Remove that check or keep it inert.
- Console-cap becomes a supervisor-side, host-observed `Crashed(ConsoleCap)` after a host-sent kill. That refines K7 rule 4 (exit 125), and the `minos_core::outcome` and `minos_sandbox::finished` docs must say so in the same change.
- stderr becomes a second capped pipe, so the helper writes no regular file at all.

### Pattern 8: Job dirs, owner lock, pidfile sweep (KRUN-05)

**Layout:**
- `/private/tmp/minos-<uid>/` with mode 0700, refused if it is a symlink or owned by someone else.
- Under it, per job: `mnsXXXXXXXX/{owner.lock, pid, vmm.json, input.img, o.sock}`.
- The socket path is about 45 bytes, well under 103.

**Supervisor:** holds an exclusive `File::lock` on `owner.lock` for the job's life (std `File::try_lock` and `File::lock` are stable on 1.97.1, verified by compile) and writes `pid` (the helper pid, which is also its pgid).

**Sweep** runs at `KrunBackend::new` and in `host doctor`. For each job dir whose `owner.lock` `try_lock` succeeds, the supervisor is dead, so:
- read `pid`;
- if `/bin/ps -o ppid=,command= -p <pid>` (argv, env cleared) shows ppid 1 and exactly `<vmm> <job>/vmm.json`, send SIGKILL to the group;
- then remove the dir.

The `ps` match guards against PID reuse. The parent-death watchdog (exit 126 within about 300 ms, K7-11) is the primary mechanism, and the sweep is the backstop. It also removes leftover input disks, which can hold judge data.

### Pattern 9: Host probes once, cached in `Capabilities` (KRUN-06, KRUN-07, CLI-03)

**What:** `KrunBackend::new(locations, blob_source)` probes once and stores `Capabilities`. `capabilities()` returns the clone, so the runner does not re-run `codesign` and hash 24 MB per launch. The helper still re-hashes libkrunfw at every launch (minos-vmm preflight).

**Probes** (each failure is one `Refusal` with a fix in its `Display`):
- helper present and signed with both keys (`codesign --display --entitlements - --xml`, argv, env cleared, capped output);
- libkrunfw at the Cellar path with the pinned SHA-256 `59764391…e5217` (re-verified here today);
- `/usr/sbin/sysctl -n kern.hv_support` = `1` (verified here today);
- the job-root socket path fits in 103 bytes;
- `/usr/bin/sandbox-exec` exists (D-03);
- two Seatbelt denial probes with the production profile:
  - `nc` must fail to connect to a 127.0.0.1 listener the probe opened, with an unsandboxed control that succeeds first, so the probe works offline;
  - `cat` must fail on an unlisted file while a listed file reads;
- the guest binary is a static aarch64 ELF (Pitfall 8).

**Plumbing:** `Capabilities` gains the active `layers`, the kernel `ImageDigest`, and `BackendKind`, so the generic runner can build `BackendInfo` with the trait still at two methods. `DedicatedUser` is always absent (D-01). Seatbelt is never absent on a host that clears preflight (D-03).

### Pattern 10: `env build` (KRUN-09, CLI-02, D-09 to D-12)

1. Build the source snapshot with `minos_source::snapshot`. It is secret-scanned, judge paths are excluded, and `.minos/` is denylisted.
2. Check `image::base_images` on the Dockerfile (digest pins).
3. Run `docker buildx build --platform linux/arm64 --provenance=false --sbom=false --output type=tar,dest=- -f <dockerfile RelPath> -`, with the snapshot tarball on stdin as the build context. Use an argv; clear the environment to `HOME` plus a `PATH` holding the docker binary's directory, its canonical directory, and the system directories (the RI-03 recipe).
4. Stream stdout into the packer, with the minos-guest bytes (verified static aarch64 ELF) written last as `/init`.
5. `put_reader` the image into `.minos/blobs`.
6. Compare with `minos.lock` (refuse unless `--update`), then write the lock.

The tar exporter writes attestations as JSON files in the export root when they are enabled, so disable both explicitly [CITED: docs.docker.com/build/metadata/attestations]. `-f` is relative to the root of a piped tarball [CITED: docs.docker.com/build/concepts/context]. Buildx's acceptance of a piped tar context is documented only for `docker build` [ASSUMED for `docker buildx build`], so verify in Wave 0 with Docker running.

### Pattern 11: `minos.lock` (D-11)

**What:** A core type, `Lock`, with `v = 1` and an array of `[[image]]` tables, one per (backend, arch). Each table holds `backend`, `arch`, `root_image` (ImageDigest hex), `dockerfile_sha256`, `base_images` (sorted `ImageRef`s), `guest_sha256`, and `builder = { name, version, engine }`. Raw types use `deny_unknown_fields`, `try_from` refuses duplicate (backend, arch) pairs, and the golden file is an insta snapshot plus a SHA-256 pin. The check API is `Lock::check(&self, backend, arch, dockerfile_sha256, guest_sha256) -> Result<&ImageEntry, LockMismatch>`, which Phase 7 and 8 call before a run.

### Anti-Patterns to Avoid

- **Trusting sequence numbers alone on the vsock route.** The candidate can connect to the same port. See Pattern 2.
- **A status frame with free text.** The spike frame carried candidate stdout. The real `.minos/status.json` carries enums, numbers, and `RelPath` only, and logs go in their own files.
- **Matching stderr markers by substring.** A logged message containing a newline could forge `minos-vmm: watchdog parent-gone` (an infrastructure failure, so retried forever). Match whole lines exactly, and make the logger prefix every line with `minos-vmm: log ` and strip control characters.
- **Reading the root image by path for every launch to rehash it** (rejected by D-14).
- **`HashMap` anywhere** in core, store, or sandbox (clippy bans).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unified diff parse and apply in the guest | A guest-local patcher | `minos_core::diff::{Diff::parse, apply::apply}` | One applier for the guest, `adopt`, and the report (CORE-05) |
| Hostile tar reading on the host | A new reader | `minos_sandbox::output::read_output` via `Finished::into_output` | Fuzzed and capped (SBX-05) |
| SquashFS | A new writer | `rootfs::pack_root_image` (backhand gzip) | Proven reproducible (RI-04) |
| Kill a process tree | A `/proc` walk with signals | `kill_process_group(Pid::INIT, Signal::KILL)` plus `wait` until `ECHILD` | Reaches `setsid` escapees, and is race-free once reaping ends |
| Random token | A seeded PRNG | 32 bytes from `/dev/urandom` | `std::random` is unstable; seeded generators are not secrets |
| Process lock | PID files alone | `std::fs::File::lock` / `try_lock` | PID reuse; the kernel drops the lock when the owner dies |
| Entitlement parse | A plist parser | Existing `entitled()` XML matcher, extended to the second key | Already tested |
| Seatbelt profile loading | A profile on disk next to the binary | `include_str!` of the profile into `minos-backend-krun`, passed with `sandbox-exec -p`, its SHA-256 recorded | No file to tamper with; the profile is part of the trusted binary |

**Key insight:** Every byte the guest sends, and every byte of the helper's stdout and stderr, is hostile. The candidate is not the only writer inside the VM that the host must distrust. Any guest process can reach the output port, so authentication has to come from a secret only PID 1 holds.

## Runtime State Inventory

Not a rename phase, so this is a short inventory of state that outlives code changes:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `.minos/blobs` gains root images (hundreds of MB each); no ledger change | None; `env build --update` writes a new blob, old blobs stay |
| Live service config | Docker Desktop builder state (`desktop-linux`, BuildKit v0.31.1 per RI-02); **the daemon is not running now** | `env build` tests need it started by the user |
| OS-registered state | None: no LaunchDaemon, no `_minos` user (D-01) | None, verified by D-01 |
| Secrets/env vars | `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW` stay test-only (D-04) | Production code must not read them |
| Build artifacts | Signed `target/debug/minos-vmm` loses its entitlement on every relink; leftover `/private/tmp/mns*` dirs from Phase 2 may exist | Re-sign after every build; the new sweep only looks under `/private/tmp/minos-<uid>/` |

## Common Pitfalls

### Pitfall 1: Candidate forges the output stream over vsock
**What goes wrong:** A candidate process connects to CID 2, port 1024 and sends a well-formed stream. If PID 1 never sends (guest panic), the host accepts the forgery as "guest decided".
**Why it happens:** `AF_VSOCK` connect needs no privilege, and msb_krun routes every guest connection on the port to the same host socket.
**How to avoid:** Use the per-launch token (Pattern 2). Keep `/dev/vdb` at root-only `0600` (devtmpfs default [ASSUMED]). Keep the token out of argv, env, files, and logs.
**Warning signs:** A VM test where a probe running as the candidate sends a forged chunk and the host returns anything other than `ChunkProtocol`. Make this a permanent test.

### Pitfall 2: Seatbelt denies the root image's ancestors
**What goes wrong:** With D-14, `ROOT_IMG` lives under `<project>/.minos/blobs/sha256/..`. Preflight canonicalizes it and stats each ancestor, which the profile denies, so the helper exits 66 with "Operation not permitted".
**Why it happens:** S05-ITER-2 added ancestor metadata only for `KRUNFW`, `KRUNFW_LINK`, and `CONFIG`, and in the spike `ROOT_IMG` sat in the job dir.
**How to avoid:** Add `(allow file-read-metadata (path-ancestors (param "ROOT_IMG")))` with a `;` comment naming the denial. Pass the canonical path; the store must canonicalize its root, because `/var` resolves to `/private/var`.
**Warning signs:** Exit 66 with `disks.root … Operation not permitted`.

### Pitfall 3: Machine bounds in the wrong crate
**What goes wrong:** D-06's check "against the `Machine` bounds in `minos-sandbox`" cannot compile in core, because core depends on no internal crate.
**How to avoid:** Move `MIN_MEMORY_MIB`, `MAX_MEMORY_MIB`, and `MAX_VCPUS` into `minos-core::limits`. Have `minos-sandbox` use them, and keep `minos-vmm`'s restatement pinned by the existing paired tests (review WR-02).

### Pitfall 4: `InputDisk(BlobId)` vs D-15
**What goes wrong:** Phase 4's `InputDisk` names a stored disk blob, but D-15 says the disk is never a blob.
**How to avoid:** Reshape `InputDisk` into the part IDs (Pattern 4). Keep `preflight`'s `JudgeDataIsInput` check meaningful by comparing judge data against each input part.

### Pitfall 5: Guest-influenced text forging helper markers
**What goes wrong:** Installing a `log` backend (DIAG-01) puts msb_krun messages on the same stderr as the K7 markers. A device log line that includes guest-chosen bytes and a newline could spell `minos-vmm: watchdog parent-gone` (retried as infrastructure) or `console-cap`.
**How to avoid:** The logger writes `minos-vmm: log <level>: <sanitized>` with every control character and newline replaced, under a total byte cap (for example 64 KiB, then one `minos-vmm: log truncated` line). The supervisor matches markers as exact whole lines. Set the max level to `Warn`.

### Pitfall 6: Chunk throughput killed by sleep polling
**What goes wrong:** The current supervisor sleeps 10 ms when no connection is pending and polls reads every 1 ms. At 4 KiB per connection, 16 MiB is about 4100 connections, so it could take tens of seconds, or fill the 128-entry listen backlog and refuse the guest's connects.
**How to avoid:** Use a dedicated accept thread blocking in `rustix::event::poll` with a timeout, per-connection read deadlines of a few seconds, guest connect retry with backoff, and the Wave 0 measurement.
**Warning signs:** Guest connect errors in the status file, or a time over budget for the max output.

### Pitfall 7: Reaping races `std::process::Child`
**What goes wrong:** PID 1's orphan reaper (`wait(NOHANG)`) can reap the stage child itself, and then `Child::try_wait` fails with `ECHILD`, losing the exit status.
**How to avoid:** Wait on the stage child by pid with `waitpid(Some(pid))`, or record any reaped status whose pid equals the stage child. Reap everything else only after `kill_all`.

### Pitfall 8: Wrong `minos-guest` binary packed as `/init`
**What goes wrong:** Next to `target/debug/minos` sits the macOS build of `minos-guest` (Mach-O). Packed as `/init`, the VM fails to boot (exit 0, no frame, `Crashed`), which is blamed on the candidate.
**How to avoid:** Look for the guest at `<target>/aarch64-unknown-linux-musl/release/minos-guest` in a dev checkout. Verify ELF magic, `e_machine` 183 (AArch64), and no `PT_INTERP` before packing or recording `guest_sha256`.

### Pitfall 9: Image files the candidate uid cannot read
**What goes wrong:** The packer zeroes uid and gid, so every image file is root-owned. A toolchain installed under `/root` (mode 0700) is unusable by uid 64000, so the build fails as `BuildFailed`.
**How to avoid:** The sample Dockerfile installs into world-readable prefixes (`/usr/local`) and sets `CARGO_HOME` and `RUSTUP_HOME` there. Document it in the `env build` help and the sample project.

### Pitfall 10: `RLIMIT_AS` breaks runtimes that reserve virtual space
**What goes wrong:** Go, the JVM, V8, and sanitizers reserve large virtual ranges and fail under a tight `RLIMIT_AS`.
**How to avoid:** Set `RLIMIT_AS` to the candidate allowance as D-05 requires, but document it. The VM's memory is the real bound, and tmpfs pages count against it. Keep defaults generous (Open Question 3).

### Pitfall 11: `EPERM` when probing a dying process group on macOS
**What goes wrong:** The deferred flaky test `supervise_run_kills_the_whole_process_group_at_the_host_deadline` saw `EPERM` from `kill(-pgid, 0)`, probably a zombie member awaiting reaping by launchd (unproven).
**How to avoid:** The sweep and the supervisor must not treat `EPERM` as "gone" or as fatal without proof. Prove the mechanism first (Phase 4 deferred note).

### Pitfall 12: Host timeout attributed to the wrong stage
**What goes wrong:** If the host deadline fires (PID 1 never reported), the host does not know which stage was running, and `Outcome::TimedOut` needs a `Stage`.
**How to avoid:** PID 1 enforces per-stage deadlines (`build_ms`, `run_ms`, `judge_ms`) itself and reports `timed_out` in the status file, so the host deadline is the backstop only. For the backstop, map Candidate to `Run` and Judge to `Judge`, and document that choice for Phase 7.

## Code Examples

### Streaming hasher in core (D-09)

```rust
// Source: sha2 0.11 Digest API (update/finalize), same plain-digest rule as BlobId::of
use sha2::{Digest, Sha256};

/// Hashes content incrementally; `finish` gives the same ID as `BlobId::of` over the concatenated bytes.
#[derive(Default)]
pub struct BlobHasher(Sha256);

impl BlobHasher {
    pub fn update(&mut self, bytes: &[u8]) { self.0.update(bytes); }
    #[must_use]
    pub fn finish(self) -> BlobId { BlobId::from_digest(self.0.finalize().into()) }
}
```

A property test asserts `BlobHasher` fed in arbitrary splits equals `BlobId::of(whole)`. `ImageDigest::from_blob_id` mirrors `TreeId::to_blob_id`.

### `BlobSource` (D-13)

```rust
/// How a backend reads the blobs a plan names. Implemented by the CLI over FileStore and by tests over a map.
pub trait BlobSource {
    /// The bytes of `id`, refused over `max` before any copy, rehashed before return.
    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, BlobSourceError>;
    /// A canonical, symlink-free, regular-file path to the root image blob, checked by symlink_metadata and inode.
    fn root_image(&self, digest: &ImageDigest) -> Result<VerifiedPath, BlobSourceError>;
}
```

A `BlobSourceError` maps to `LaunchError` (infrastructure, retried), except a missing root image, which `doctor` and `check` report as "run `minos env build`".

### Seatbelt argv (profile embedded)

```rust
// Source: 02-SPIKE-EVIDENCE.md S05-BOOT argv shape; `-p` passes the profile text instead of a file.
const PROFILE: &str = include_str!("minos-vmm.sb"); // moved next to the code that launches it
fn seatbelt_argv(p: &JobPaths, vmm: &Path, krunfw: &Path, krunfw_link: &Path, root: &Path) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec!["/usr/bin/sandbox-exec".into(), "-p".into(), PROFILE.into()];
    for (key, value) in [
        ("CONFIG", p.config()), ("CONSOLE", Path::new("/dev/fd/1")), ("INPUT_IMG", p.input()),
        ("KRUNFW", krunfw), ("KRUNFW_LINK", krunfw_link), ("ROOT_IMG", root), ("SOCK", p.socket()), ("VMM", vmm),
    ] {
        argv.push("-D".into());
        let mut kv = OsString::from(key);
        kv.push("=");
        kv.push(value);
        argv.push(kv);
    }
    argv.push(vmm.into());
    argv.push(p.config().into());
    argv
}
```

### Helper's own rlimits at `main` entry (host-side layer)

```rust
// After the parent PID and start instant (DEFECT-01 order stays first).
// RLIMIT_CORE 0: no core files. RLIMIT_FSIZE 0: with console and stderr on pipes the helper writes no regular file
// (Wave 0 must confirm msb_krun writes none, else SIGXFSZ kills the helper). RLIMIT_NOFILE: bound fds.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Exit code carries the guest result (ARCHITECTURE.md Pattern 5) | Host-observed exit, markers, and the guest's authenticated stream | K7, 2026-10 | Classification rule, refined here for chunks and the console pipe |
| One vsock connection per output | ≤4 KiB chunk per connection (S03-ANSWER), plus a token (this research) | 2026-10-10 / 2026-10-18 | Throughput unmeasured: Wave 0 |
| Scratch raw disk `/dev/vdc` | tmpfs in guest memory (D-05) | 2026-10-18 | vmm config v2, Seatbelt loses `SCRATCH_IMG` |
| Initramfs boot | Root-disk boot (D-18) | 2026-10-09 | `Boot::Initramfs` and `initramfs.rs` are negative evidence; recommend removing them from the production config surface |
| Cgroup kill before pack (ARCHITECTURE.md) | `kill(-1)` from PID 1 | — | libkrunfw has no cgroups |

**Deprecated/outdated:**
- `GuestBytes::status_frame` (loose JSON): its own doc says Phase 5 replaces it with a closed type in core.
- Spike step types (`Connect`, `VsockStream`, `ScratchRoundtrip`, `ConsoleFlood`, `Hang`): replaced by the real job. Probes move to a test fixture binary run as the candidate.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | devtmpfs creates `/dev/vdb` as root `0600`, so the candidate cannot read the token | Pitfall 1, Security | Token readable, so forgery is possible. Must be a VM test assertion; if wrong, PID 1 `chmod 0000`s the node after reading |
| A2 | `docker buildx build -` accepts a tar build context on stdin, as `docker build -` does | Pattern 10 | Fallback: pass the snapshot through a temporary directory context made from the trusted tarball, or use `docker build` (which is buildx on Docker Desktop) |
| A3 | The deny-default profile plus `(literal "/dev/fd/1")` lets msb_krun's `File::create("/dev/fd/1")` succeed | Pattern 7 | Fallback: the proven file console plus the 50 ms watchdog, with the host budget sized at cap + 64 KiB (K7-04) |
| A4 | msb_krun writes no regular file except the console, so `RLIMIT_FSIZE = 0` is safe on the helper | Code Examples | SIGXFSZ kills the helper. Use a small nonzero cap or drop FSIZE |
| A5 | The VMM keeps guest connection order, or reorder is harmless because the host buffers by seq | Pattern 2 | None for blame (buffering absorbs it); only memory under the total cap |
| A6 | Default `memory_mib` 2048, `scratch_mib` 1024, guest reserve 256 MiB, uid/gid 64000, `RLIMIT_NPROC` 512, `RLIMIT_NOFILE` 4096 | Discretion values | Too small breaks real builds; values are constants with reasons and are manifest-tunable (D-06) |
| A7 | `kern.hv_support == 1` is a sufficient quick Hypervisor.framework probe | Pattern 9 | A host could still refuse `hv_vm_create`; the canary boot in `doctor` (optional) or exit 70 at launch catches it |
| A8 | `codesign --entitlements` needs a file path, not stdin | Pattern 9 | Writing the embedded plist to a 0700 temp file works either way |

## Open Questions

Each has a recommendation; the planner decides, and items 1, 2, and 4 should be confirmed with the user.

1. **What does `ConfinementLayer::Rlimits` mean?**
   - What we know: core documents it as "Resource limits on the helper process". D-01 calls the active layer "in-guest rlimits".
   - What's unclear: whether the layer is host-side (the helper's own rlimits), guest-side (the candidate's), or both.
   - Recommendation: apply both. The helper sets its own `CORE`, `FSIZE`, and `NOFILE` limits at `main` entry, and the guest applies the candidate limits. Record `Rlimits` only when the helper's own limits were set, since provenance layers are host confinement. Fix the core doc comment in the same change. Ask the user to confirm the wording.
2. **`KRUN_BOOT_START_NS` in the guest environment and cmdline.**
   - What we know: msb_krun 0.1.41 always appends `KRUN_BOOT_START_NS=<host wall-clock ns>` (`build_kernel_cmdline`). It is the only host-derived token, removing it needs a fork, and the guest clock reveals the same time.
   - Recommendation: the GUEST-05 test allows exactly that key with an all-digit value (the spike already normalizes it) and asserts a host canary value appears nowhere. Record the allowance in CLAUDE.md.
3. **Default memory and scratch numbers.**
   - Recommendation: `memory_mib` 2048, `scratch_mib` 1024, reserve 256. The budget check requires an allowance of at least 256. The sample project sets its own values. Measure one Rust build of the sample in the two-experiment test.
4. **How does the lock feed `ManifestId` (D-10)?**
   - What we know: `ManifestId::of(manifest_bytes)` hashes the manifest only.
   - Recommendation: Phase 5 writes the lock. Each `Judged` already records `root_image` in `BackendInfo`, so lineage is visible. Whether `RunStarted` should also carry the lock digest is a Phase 7 question to raise with the user.
5. **Keep virtio-rng?**
   - Recommendation: keep it, explicitly set with `.rng(true)`. Without it, early `getrandom` in candidate builds can stall on hosts without `RNDR` [ASSUMED]. Its data flows host to guest only. Record the decision in CLAUDE.md.
6. **Copy libkrunfw privately before `dlopen`?**
   - Recommendation: no. D-04 fixes the Cellar path. The hash-to-`dlopen` window can be exploited only by a host writer to `/opt/homebrew/Cellar`, and the threat model's attacker is the guest. Record the decision in CLAUDE.md.
7. **Remove `Boot::Initramfs` from the entitled helper?**
   - Recommendation: yes. It shrinks the config surface of the one entitled binary, and the negative evidence stays in 02-SPIKE-EVIDENCE.md. This changes a Phase 2 artifact, so note it in the SUMMARY.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | all | ✓ | rustc 1.97.1, cargo 1.97.1 | — |
| `aarch64-unknown-linux-musl` target | guest | ✓ | installed | — |
| cargo-deny | gate | ✓ | 0.20.2 | — |
| libkrunfw (Cellar) | VM tests, launches | ✓ | 5.6.2, SHA-256 `59764391…e5217` (matches pin) | — |
| `/usr/bin/codesign` | KRUN-08, `host setup` | ✓ | system | — |
| `/usr/bin/sandbox-exec` | Seatbelt layer (D-03) | ✓ | system, macOS 27.0.1 | none (refusal by design) |
| Hypervisor.framework | VMs | ✓ | `kern.hv_support` = 1 | — |
| Host memory / CPUs | two sequential VMs | ✓ | 48 GiB, 16 CPUs | — |
| Docker CLI + buildx | `env build`, D-12 test | ✓ CLI / ✗ daemon | buildx v0.35.0-desktop.2; daemon not running | Start Docker Desktop before `env build` tests (human action) |
| `/usr/bin/nc`, `/bin/cat`, `/bin/ps`, `/usr/sbin/sysctl` | doctor probes, sweep | ✓ | system | — |

**Missing dependencies with no fallback:** none blocking code work.

**Missing dependencies with fallback:** the Docker daemon is stopped. Every `env build` VM or integration test must be an ignored test the user runs with Docker started. Code and unit tests do not need it.

**Note:** this host runs macOS 27.0.1. Phase 2 evidence was recorded on 27.0. Re-run the Seatbelt boot test once in Wave 0.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + insta 1.49.0 (`.config/insta.yaml` `update: no`) + proptest 1.11.0 + trybuild 1.0.122 |
| Config file | `.config/insta.yaml`; per-crate `clippy.toml` |
| Quick run command | `cargo test -p <crate>` for the crate being changed |
| Full suite command | the six gate commands in CLAUDE.md Commands |
| VM suite command | `cargo build -p minos-vmm && codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm && cargo build -p minos-guest --release --target aarch64-unknown-linux-musl && MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-backend-krun --test <name> -- --ignored --nocapture --test-threads=1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GUEST-01 | Static ELF, PID 1 path, no per-run state before job | unit (ELF check) + VM | `cargo test -p minos-backend-krun --test vm_boot -- --ignored` | ❌ Wave 0 (from `spike_boot.rs`) |
| GUEST-02 | Apply mismatch yields `ApplyFailed`, `Rejected` | unit (host-compilable guest core) + VM | `cargo test -p minos-guest`; VM `vm_experiment` | ❌ |
| GUEST-03 | Commands run as uid 64000 under rlimits; writes outside scratch fail | VM (probe as candidate) | `--test vm_experiment` | ❌ |
| GUEST-04 | Byte-capped tar via chunks; forged chunk is `ChunkProtocol`; scorecard only from Judge | unit (core chunk proptests) + trybuild (exists) + VM forge test | `cargo test -p minos-core --test chunk`; `--test exit_forge` | partial (trybuild `candidate_scorecard.rs` exists) |
| GUEST-05 | No host env and no host values in `env` or `/proc/cmdline` (canary) | VM, permanent exit test | `--test exit_env` | ❌ (from `spike_boot`) |
| KRUN-01 | Cleared env, stdin null, console pipe capped | unit (supervisor with a fake helper) + VM | `cargo test -p minos-backend-krun --lib`; `--test vm_console` | partial |
| KRUN-02 | Clippy bans fire; no net or fs virtio device | existing D-PROBE + VM device list | `cargo clippy -p minos-vmm`; `--test vm_boot` | ✅ bans / ❌ VM |
| KRUN-03 | Egress fails closed from the candidate uid | VM, permanent exit test | `--test exit_egress` | ❌ (from `spike_net`) |
| KRUN-04 | Two disks only; scratch is tmpfs; no virtio-fs | unit (vmm config v2 golden) + VM | `cargo test -p minos-vmm` | ❌ golden |
| KRUN-05 | `kill -9` launcher leaves no helper; sweep cleans | VM, permanent exit test + unit (sweep with fake dirs) | `--test exit_kill9`; `cargo test -p minos-backend-krun --lib jobdir` | ❌ |
| KRUN-06 | Each probe failure gives its `Refusal` | unit (probes against fixtures) | `cargo test -p minos-backend-krun --lib host` | ❌ |
| KRUN-07 | Layers in `Capabilities` and `BackendInfo`; Seatbelt never absent | unit | `cargo test -p minos-sandbox` | ❌ |
| KRUN-08 | Signed helper with both keys verified | unit (XML matcher) + manual `codesign -d` | `cargo test -p minos-backend-krun --lib` | partial |
| KRUN-09 | Dockerfile becomes an image with `/init` last; context is the snapshot | integration (Docker) | `cargo test -p minos-cli --test env_build -- --ignored` | ❌ |
| KRUN-10 | Two sequential experiments, both outcomes, no shared build output (marker) | VM | `--test vm_two_experiments` | ❌ |
| CLI-02 | Lock written; refuse on mismatch without `--update` | unit + golden + integration | `cargo test -p minos-core --test lock`; `env_build` | ❌ |
| CLI-03 | `doctor` prints every refusal with a fix | unit (render) + manual run | `cargo test -p minos-cli` | ❌ |

### Sampling Rate

- **Per task commit:** `cargo test -p <touched crate>` and `cargo clippy -p <touched crate> --all-targets -- -D warnings`. For guest changes, also the musl clippy command.
- **Per wave merge:** the six gate commands, plus the VM suite after build, sign, and guest build.
- **Phase gate:** gate green, the three permanent exit tests (`exit_egress`, `exit_env`, `exit_kill9`) pass and are recorded in `05-EVIDENCE.md`, the C-02 grep prints nothing, then `cargo clean` (GATE-06).

### Wave 0 Gaps

- [ ] `crates/minos-backend-krun/tests/wave0_chunks.rs`: throughput and backlog at 16 MiB (S03-ANSWER measurement)
- [ ] `crates/minos-backend-krun/tests/wave0_console_pipe.rs`: `/dev/fd/1` console under the deny-default profile, exact cap
- [ ] `crates/minos-backend-krun/tests/wave0_store_root.rs`: root image under a store path with ancestor metadata
- [ ] A probe fixture binary (second `[[bin]]` in `minos-guest`, never packed into user images) that the exit tests run as the candidate: egress connects, env and cmdline dump, forged chunk, write outside scratch
- [ ] `05-EVIDENCE.md` skeleton
- [ ] Framework install: none; existing infrastructure covers everything else

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (output channel) | 32-byte per-launch token from `/dev/urandom` in every chunk; constant-time compare |
| V3 Session Management | no | — |
| V4 Access Control | yes | Seatbelt deny-default on the helper; unprivileged uid in the guest; root-only `/dev/vdb`; `/proc` `hidepid`; role-typed `Untrusted` exits |
| V5 Input Validation | yes | `try_from` plus `deny_unknown_fields` on vmm config v2, lock, guest job, status file; chunk codec caps; `read_output` caps; `RelPath` names; `O_NOFOLLOW` packer walk |
| V6 Cryptography | yes | SHA-256 via `sha2` only (IDs, section digests, END digest); no hand-rolled MAC; token is a capability, not a cipher |
| V12 Files and Resources | yes | No host directory sharing; never mount a guest-written disk; no scratch on host (D-05); job dir 0700; `create_new`; console and stderr never on disk |
| V14 Configuration | yes | Entitlements limited to two keys; clippy bans on VMM APIs; `cargo deny`; env cleared for every exec |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Candidate forges output chunks on the vsock port | Spoofing / Tampering | Per-launch token known only to PID 1 (Pattern 2) |
| Candidate floods connections to exhaust host caps | Denial of service | Caps produce `ChunkProtocol` or `OutputTooLarge`, a fault on the candidate's own experiment |
| Guest text forges helper stderr markers (forces infinite retry) | Tampering | Sanitized, prefixed log lines; exact whole-line marker match (Pitfall 5) |
| Symlink in `/out` makes root PID 1 pack a root-only file (judge data in a Judge VM) | Information disclosure | Kill-all before pack; `O_NOFOLLOW`; one-link regular files only |
| Guest kernel exploit gives the candidate root, then it reads the token | Elevation | Out of scope for integrity of that VM's own self-report. Judge and candidate are separate VMs, so judge data is never in a candidate VM; status is still parsed as hostile |
| TSI or virtio-fs enabled by a code change | Elevation | 13 clippy bans; `egress_fails_closed` permanent test; device-list assertion |
| Stale VM after `kill -9` | Denial of service / exposure | Watchdog 126; pidfile sweep with `ps` argv match |
| Build context leaks secrets or judge data | Information disclosure | Snapshot tarball as the only context; env cleared |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`: K7 table, Classification rule for Phase 5, K7-04, K7-11, DEFECT-01, S03-ANSWER, DIAG-01 mechanism, SPIKE-05 answer, S05-RECIPE, S05-DENY-NET, S05-DENY-FILE, RI-02 to RI-04, S01-FRAME, S01-NETIFACES
- msb_krun 0.1.41 source (local registry): `src/api/builders.rs` (rng, console `output` and `disable_implicit` and `custom`, vsock routes, hijack default), `src/api/vm.rs` (`configure_vsock`, `compute_tsi_flags`, `build_kernel_cmdline`, `KRUN_BOOT_START_NS`)
- msb_krun_vmm 0.1.41 `src/builder.rs` (`autoconfigure_console_ports`, `create_explicit_ports`, `attach_console_devices`); msb_krun_devices 0.1.41 `virtio/console/port_io.rs` (`output_to_raw_fd_dup`, `wait_until_writable`, `ConsolePortBackend`), `virtio/console/device.rs` (console port requires a terminal)
- rustix 1.1.5 source: `process/rlimit.rs`, `process/mod.rs` cfg, `process/kill.rs`, `pid.rs`, `mount/mount_unmount.rs`, `fs/at.rs` (`mkfifoat` and `mknodat` not on Apple), `event/poll.rs`
- vsock 0.5.4 `src/lib.rs`
- Repository code: `crates/minos-vmm/src/*`, `crates/minos-backend-krun/src/{supervise,input_disk,rootfs}.rs`, `crates/minos-guest/src/*`, `crates/minos-sandbox/src/*`, `crates/minos-core/src/{job,ledger,outcome,id,diff/*}.rs`, `crates/minos-store/src/file/blobs.rs`, `deny.toml`, `Cargo.toml`
- Local experiments, 2026-10-18: Seatbelt `/dev/fd/1` literal (scratchpad); rustc 1.97.1 compile probes (`std::io::pipe`, `File::try_lock`, `Command::uid/gid/process_group` stable; `std::random` unstable E0658); `sysctl -n kern.hv_support` = 1; libkrunfw SHA-256 matches the pin

### Secondary (MEDIUM confidence)

- https://docs.docker.com/reference/cli/docker/buildx/build/ : context `PATH | URL | -`, tar exporter `dest=-`, `--provenance` default
- https://docs.docker.com/build/metadata/attestations/ : tar and local exporters write attestations as JSON files in the export root
- https://docs.docker.com/build/concepts/context/ : piped tarball context, `--file` relative to the tarball root

### Tertiary (LOW confidence)

- WebSearch for buildx stdin tar context (docker-cli commit history): suggests `-` accepts a tar context; unconfirmed for buildx, so Wave 0 verifies it

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. No new crates; every API named was read in the pinned source or compiled.
- Architecture: MEDIUM-HIGH. The locked decisions are fixed. The token, console pipe, trampoline, and store-path root are reasoned from source and one local experiment, and each has a Wave 0 VM proof.
- Pitfalls: HIGH for 2, 3, 4, 7, 8, and 11 (code read); MEDIUM for 1, 5, 6, 9, 10, and 12.

**Research date:** 2026-10-18
**Valid until:** 2026-11-17. Re-verify if `msb_krun`, libkrunfw, Docker Desktop, or macOS changes.
