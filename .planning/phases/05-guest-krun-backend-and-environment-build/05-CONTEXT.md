# Phase 5: Guest, krun Backend, and Environment Build - Context

**Gathered:** 2026-10-18
**Status:** Ready for planning

<domain>
## Phase Boundary

An experiment runs inside a libkrun microVM on this Mac with no network path, no host environment, read-only disk-image inputs, and a capped tar stream out, and no VM outlives the process that launched it. Concretely, this phase delivers:

- `minos-guest` as the real PID 1: reads the job, applies the base tarball plus canonical diff with the core applier (`Rejected` on mismatch), runs manifest argv commands as a non-root uid under rlimits writing only to scratch, and streams a byte-capped tar back over the chunked vsock protocol (GUEST-01 to GUEST-05).
- `minos-backend-krun` as a real `Backend`: one signed `minos-vmm` helper per VM with a cleared environment, stdin from `/dev/null`, a capped console; preflight that builds `Capabilities` and refuses with a reason; a supervisor that kills the helper's process group at the hard maximum duration; a parent-death watchdog and pidfile sweep; `BackendInfo` that records the active confinement layers with no silent fallback (KRUN-01 to KRUN-08, KRUN-10).
- `minos env build`: the project Dockerfile becomes a raw read-only SquashFS root image with `/init` written in by the trusted packer, stored as a blob, with `minos.lock` recording its digest and inputs (KRUN-09, CLI-02).
- `minos host doctor` and `minos host setup`: the launch preflight explained per refusal with a fix; signing and verifying the helper (KRUN-06, CLI-03).
- The three permanent exit tests promoted from the Phase 2 spikes: egress fails closed (KRUN-03), no host values in the guest's `env` or `/proc/cmdline` (GUEST-05), and `kill -9` on the launching process leaves no VM and no helper (KRUN-05).

Out of scope: the runner pipeline and the loop (Phase 7), the proposers (Phase 6), the rest of the CLI (Phase 8), the dedicated `_minos` user (deferred, see D-01), the Lambda and Cloud Hypervisor backends, and GPU passthrough.

</domain>

<decisions>
## Implementation Decisions

### The `_minos` user layer and `host setup`
- **D-01:** The dedicated unprivileged `_minos` user is **deferred**. Phase 5 runs with hardware virtualization, the Seatbelt profile, and in-guest rlimits as the active layers. `BackendInfo` records the user layer as missing, so every v1 run shows reduced hardening in the report's provenance (03 D-18, `EXPECTED_KRUN_LAYERS`). Nothing in Phase 5 needs `sudo`. The S05-RECIPE stays recorded for the phase that implements it; the sudoers-rule versus LaunchDaemon choice is not made now.
- **D-02:** `minos host setup` **signs the helper and verifies it**: it runs the one documented `codesign` argv from CLAUDE.md Commands against the built `minos-vmm`, then verifies with `codesign -d --entitlements -` that `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation` are present. It also reports libkrunfw and the container builder the way `doctor` does. No xtask, script, `build.rs`, or cargo alias (02 D-11); `setup` is the "may run the same argv from Rust later" that D-11 allowed.
- **D-03:** **Seatbelt is mandatory in v1.** A host where `/usr/bin/sandbox-exec` is missing, or where `doctor`'s denial checks (an outbound TCP connect and an unlisted file read must both be denied under the profile) no longer hold, gets a preflight `Refusal` and `doctor` prints the fix. No run is ever recorded without the Seatbelt layer; there is no "launch and record it missing" path for Seatbelt, unlike the user layer.
- **D-04:** Binaries and the kernel are found at **fixed locations with no config file**: `minos-vmm` and `minos-guest` next to the `minos` binary, or in the cargo target directory for a development checkout; libkrunfw at the Homebrew canonical path (`/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`) checked against its pinned SHA-256 before use (K5). `doctor` names every path it checked. The `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW` environment variables remain only for the ignored VM tests, never for production launches.

### Scratch space for the candidate
- **D-05:** Scratch is a **size-capped tmpfs in guest memory**, chosen as the most secure option that behaves identically locally and on Lambda MicroVMs. Scratch bytes never exist on the host, so "scratch is deleted unread" is structural rather than a cleanup step; every VM starts empty, so build output cannot leak between experiments (KRUN-10); and Lambda MicroVMs attach no per-job block device, so a raw scratch disk would have been a local-only mechanism with a second guest code path. The VM's memory is a **budget**: guest reserve + tmpfs cap + the candidate's allowance, with the candidate's processes under `RLIMIT_AS`, `RLIMIT_FSIZE`, `RLIMIT_NPROC`, and `RLIMIT_CPU`. Over-budget candidates are killed inside the VM and reported as a stage `Outcome`, never as an infrastructure failure. Consequences: KRUN-04's "size-capped scratch disk, all raw images" is amended to "size-capped scratch space in guest memory", the scratch disk leaves the `minos-vmm` launch config (02 D-04 shape, one field fewer), and the Phase 2 scratch round-trip step retargets to the tmpfs. The amendment is recorded in REQUIREMENTS.md and CLAUDE.md in the same change.
- **D-06:** VM memory and the scratch cap are **manifest fields now**, in `[environment]` (working names `memory_mib` and `scratch_mib`), parsed and range-checked in `Manifest::try_from` against the `Machine` bounds in `minos-sandbox`, with defaults, so a project sizes itself and the sample project can declare what it needs. The budget rule (reserve + scratch + allowance must fit the machine) is a `Manifest::try_from` cross-field check. The `RawManifest` table keeps `deny_unknown_fields`.
- **D-07:** The guest gets a **`JobSource` / `OutputSink` abstraction now**: traits in `minos-guest` with the input-disk `JobSource` and the chunked-vsock `OutputSink` implemented in Phase 5, so the AWS milestone adds a lifecycle-hook pair without touching the decoders, the applier, or the command runner. The decoders stay byte-oriented behind the traits; only the Linux entry point names `/dev/vdb` and the vsock port. The host-compilable half keeps building for `aarch64-apple-darwin` (02 D-03).
- **D-08:** **One fixed unprivileged uid/gid** runs both the build and the run commands. The guest creates it in memory (no `/etc/passwd` dependency on the user's image), it owns the scratch mount and `/out`, and build and run share it because build output must be runnable. PID 1 and the output packer stay root and the candidate's process tree is killed before packing, so a candidate cannot race the packer. The Dockerfile `USER` is ignored. Separate uids for build and run were rejected: they buy no host security (the VM is the boundary) and add permission handling between stages.

### Root images and `minos.lock`
- **D-09:** Built root images live in **`.minos/blobs` through a streaming put**. The `ImageDigest` recorded in `BackendInfo` and `minos.lock` is the blob ID. This needs a `Blobs::put` variant that streams from a reader and hashes as it writes, with the hashing constructor in `minos-core` (the open question about large blobs is closed this way: a streaming SHA-256 builder in core, the file store writing temp-then-rename in the shard as today). The vmm reads the image by its blob path (D-14).
- **D-10:** `minos env build` **refuses when the produced digest differs from `minos.lock` unless `--update` is passed**, like `cargo --locked`. The refusal prints both digests. `--update` rewrites the lock; the next run records a new `ManifestId` lineage because the lock's digest feeds the manifest's image reference.
- **D-11:** `minos.lock` records, **per backend and architecture**: the root image digest, the Dockerfile's SHA-256, every `FROM` base-image digest (as `image::base_images` extracts them), the `minos-guest` binary's SHA-256 that was written in as `/init`, and the builder's name and version. A run refuses to start when the Dockerfile or guest binary on disk no longer matches the lock. The lock is TOML with a `v` tag, `deny_unknown_fields`, `try_from`, and a golden file.
- **D-12:** Reproducibility claim: **the packed SquashFS image must reproduce; the builder's tar may differ.** The packer zeroes every mtime, uid, and gid (RI-04), so an `env build` test builds the same Dockerfile twice and asserts equal image digests. Byte-identical rootfs tars are not required and not claimed in docs, because not every OCI builder honors `SOURCE_DATE_EPOCH`.

### Blob hand-off to the backend
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record
- `CLAUDE.md` — the design of record: Threat model, Architecture, minos-sandbox, Local backends, Environments and source, Commands (the `codesign` argv, the musl clippy gate), Open questions (Phase 5 items), Verified facts local.
- `.claude/CLAUDE.md` — stack decisions (msb_krun pin, libkrunfw, backhand gzip, rustix and vsock in the guest), the Phase 2 corrections block.
- `.planning/ROADMAP.md` §Phase 5 — goal, the five success criteria, the exit criterion (success criterion 1 is never traded for schedule).
- `.planning/REQUIREMENTS.md` — GUEST-01 to GUEST-05, KRUN-01 to KRUN-10, CLI-02, CLI-03; KRUN-04 is amended by D-05.

### Phase 2 evidence that this phase builds on
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §"Classification rule for Phase 5" — how a run is classified from the helper's exit code, the stderr marker, and the status frame.
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §S03-ANSWER — the chunked vsock output decision and its rules.
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §DIAG-01 — the three `msb_krun_devices` vsock defects the chunking must keep dormant.
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §SPIKE-05 answer, §S05-RECIPE — the Seatbelt profile result and the deferred user recipe.
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §K7 table, §K7-04, §K7-11 — exit codes, console cap overshoot, no helper left running.
- `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` §RI-03, §RI-04, §S06-BOOT — root image production and reproducibility.
- `.planning/phases/02-platform-spikes/02-CONTEXT.md` — D-01 (spike code lives in the real crates), D-02 (spikes become exit tests), D-03 (musl target), D-04 (launch config shape), D-10 (host changes that need asking), D-11 (signing), D-14 (builder), D-18 (root-disk boot).

### Phase 3 and 4 decisions this phase must honor
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-CONTEXT.md` — D-10 to D-13 (store layout and durability, D-13 as amended 2026-10-18), D-14 (role-typed output), D-15 (launch key), D-17 (specs carry IDs; backends read blobs by ID).
- `.planning/phases/03-core-types-and-pure-logic/03-CONTEXT.md` — D-18 (`BackendInfo` isolation and layers rendered in the report).
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/04-REVIEW.md` — IN-01 to IN-06 remain open; IN-03 (`Appended::Conflict` shape) matters to Phase 7, not here.
- `.planning/phases/04-store-source-snapshot-and-sandbox-types/deferred-items.md` — the flaky supervisor process-group test; prove the mechanism before changing it.

### Research inputs (dated; evidence wins over them)
- `.planning/research/ARCHITECTURE.md` — crate table (`env build` lives in `minos-cli`, the packer in `minos-backend-krun`), the disk table, Pattern 5 as corrected by the classification rule.
- `.planning/research/STACK.md` — stack of record, corrected by the Phase 2 block in `.claude/CLAUDE.md`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/minos-vmm/` (1,919 lines): a working launcher with `VmmConfig::try_from` (versioned JSON, `deny_unknown_fields`), preflight (entitlement, libkrunfw hash, hypervisor), the watchdog with exit codes 124, 125, 126, the Seatbelt profile `minos-vmm.sb`, and the entitlements plist. Phase 5 drops the scratch disk field (D-05), installs a `log` backend, and adds the golden file 02 D-04 promised.
- `crates/minos-backend-krun/src/rootfs.rs` (1,538 lines): the SquashFS packer that writes `/init` last and zeroes metadata; `supervise.rs` (1,848 lines): a one-helper supervisor with per-connection caps and a host deadline; `input_disk.rs`: the 1 MiB job disk writer that D-15 grows; `launch_config.rs`: the serializer for the vmm config.
- `crates/minos-guest/` (1,958 lines): the job and status-frame codecs and the Linux PID 1 path that mounts `/proc`, `/sys`, and a `/tmp` tmpfs, reads `/dev/vdb`, and powers off. The spike step types (`Connect`, `VsockStream`, `ScratchRoundtrip`, `ConsoleFlood`, `Hang`) are replaced by the real job, with the exit tests keeping what they need.
- `crates/minos-sandbox/`: `Backend`, `RunningVm`, `Capabilities`, `Refusal` (already lists `EntitlementMissing`, `KernelMissing`, `KernelHashMismatch`, `HypervisorUnsupported`, `SocketPathTooLong`), `Cleared`, `Finished::new` (consumes the clearance since quick 261018-eiw), `read_output` (the raw-mode tar reader with caps), and `LaunchKey`.
- `crates/minos-core/src/job.rs`: `JobRequest`, `JobResult`, `JobBudget`, `verify_diff`; `diff/apply.rs`: the strict applier the guest reuses; `image.rs`: `ImageRef` and `base_images` for the lock's `FROM` digests.
- `crates/minos-store/src/file/blobs.rs`: temp-then-rename blob writes in the shard; the base for the streaming put (D-09) and the verified-path accessor (D-14).
- The five ignored spike tests under `crates/minos-backend-krun/tests/` (`spike_boot`, `spike_exits`, `spike_net`, `spike_seatbelt`, `spike_rootimg`): the source of the three permanent exit tests.

### Established Patterns
- Validated types with private fields; `#[serde(try_from)]` plus `deny_unknown_fields` on every raw input (manifest fields in D-06, the lock in D-11, the vmm config).
- Errors separate blame: candidate faults become `Outcome`s, infrastructure failures are `InfraError` and retried; the classification rule maps exit codes and frames onto that split.
- `BTreeMap` for anything ordered; golden files pinned by SHA-256 for every wire format; `trybuild` for invariants.
- No `allow` attributes anywhere; the three clippy lints are `forbid` in tests too; pedantic is on (`needless_pass_by_value` bit the quick task).
- `cargo clean` at the end of the phase after verification; the musl clippy command is a gate command; `minos-vmm` must be re-signed after every build before a VM run.

### Integration Points
- `KrunBackend::new(paths, blob_source)` implements `Backend`; `capabilities()` is what `host doctor` prints; `launch(Cleared<R>)` spawns `minos-vmm` under `sandbox-exec` with the profile, through the supervisor; `wait` classifies and returns `Finished<R>`.
- `BlobSource` (D-13) sits in `minos-sandbox` next to `Backend`; `minos-cli` implements it over `FileStore` and constructs `Runner<KrunBackend, FileStore>` later (Phase 7).
- `Manifest::try_from` gains the `[environment] memory_mib` and `scratch_mib` fields (D-06) and the budget check; `Machine` bounds in `minos-sandbox` are the ceilings.
- `minos env build` in `minos-cli` calls Docker buildx (argv array, environment cleared to an allowlist), feeds the tar to the packer in `minos-backend-krun`, streams the image into `FileStore` (D-09), and writes `minos.lock` (D-10, D-11).
- `BackendInfo` layers: hardware isolation, Seatbelt (required, D-03), rlimits; the user layer absent (D-01); the report's provenance shows it as reduced hardening.

</code_context>

<specifics>
## Specific Ideas

- "What's the most secure way to do this, so that it works locally and in the cloud as planned": the user's framing for scratch, which drove D-05 and D-07. The planner should apply the same test to any other local-versus-Lambda fork it finds: prefer the one mechanism that is secure on both over a local-only shortcut.
- The memory budget is a manifest fact (D-06), not a hidden constant, so `minos check` (Phase 8) can print it next to the noise floor.

</specifics>

<deferred>
## Deferred Ideas

- **Dedicated `_minos` user** (sudoers rule pinned to the helper path, or a LaunchDaemon), with job-directory ACLs and socket permissions: a later phase; the S05-RECIPE is the starting point (D-01).
- **Local-only ext4 scratch disk** with a Rust formatter, for projects whose build output exceeds what guest memory can hold (D-05). Only if tmpfs proves limiting in practice.
- **Concurrent candidates (`width`) and K8 measurements** (launches per second, memory per VM): Phase 7 and NEXT-04.
- **Narrowing the Seatbelt profile's starting allowances** (rows 1 to 6 of the SPIKE-05 answer): still an open question; not required to pass Phase 5.
- **Reporting the three `msb_krun_devices` vsock defects upstream**: the user's call, unchanged since S03-ANSWER.
- **D-10 lineage: the lock's digest in `ManifestId`**: deferred to Phase 7, which wires `minos run`. Phase 5 delivers D-10's refusal gate (`minos env build` refuses a changed image digest unless `--update`, printing both digests, and rewrites the lock atomically) and `Lock::check`. D-10's sentence that "the next run records a new `ManifestId` lineage because the lock's digest feeds the manifest's image reference" is implemented when Phase 7 wires `minos run`, which decides how the lock digest enters the run's identity (`ManifestId` today hashes the manifest bytes only; a lock digest in `RunStarted` is the alternative 05-RESEARCH Open Question 4 names) and raises that choice with the user. Meanwhile each `Judged` event records `root_image` in `BackendInfo`, so image lineage is visible in the ledger. Recorded by plan 05-16, because the Phase 5 plans deliver only the gate.

</deferred>

---

*Phase: 05-guest-krun-backend-and-environment-build*
*Context gathered: 2026-10-18*
