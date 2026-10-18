# Phase 5: Guest, krun Backend, and Environment Build - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-18
**Phase:** 05-guest-krun-backend-and-environment-build
**Areas discussed:** `_minos` user layer and host setup, Scratch space for the candidate, Root images and minos.lock, Blob hand-off to the backend

---

## `_minos` user layer and host setup

**Q1: Implement the dedicated unprivileged `_minos` user in Phase 5, or defer it?**

| Option | Description | Selected |
|--------|-------------|----------|
| Defer, record as missing layer (Recommended) | Seatbelt and rlimits are the active layers; BackendInfo and the report show the user layer as missing; no sudo in Phase 5; the recipe stays recorded | ✓ |
| Implement with a pinned sudoers rule | `host setup` prints the NOPASSWD line pinned to the helper path; supervisor execs `sudo -u _minos`; job-dir ACLs and socket permissions solved in this phase | |
| Implement with a LaunchDaemon | A root daemon starts helpers as `_minos`; a privileged service, an IPC contract, and installer steps inside Phase 5 | |

**User's choice:** Defer, record as missing layer.

**Q2: What should `minos host setup` do in Phase 5?**

| Option | Description | Selected |
|--------|-------------|----------|
| Sign the helper and verify (Recommended) | Runs the documented codesign argv on the built minos-vmm, verifies entitlements with `codesign -d`, reports libkrunfw and the builder as doctor does | ✓ |
| Verify only, never sign | setup prints the codesign command for the user to run by hand | |
| Skip host setup entirely | Only host doctor ships in Phase 5 | |

**User's choice:** Sign the helper and verify.

**Q3: How should `host doctor` and BackendInfo treat a run where Seatbelt cannot be applied?**

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse to launch (Recommended) | Seatbelt is required in v1; a host where the deny checks fail gets a preflight Refusal and doctor explains the fix | ✓ |
| Launch and record the missing layer | Like the user layer: hardware isolation only, Seatbelt listed as missing, report flags reduced hardening | |

**User's choice:** Refuse to launch.

**Q4: How does the CLI locate `minos-vmm`, `minos-guest`, and libkrunfw at run time?**

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed locations, no config (Recommended) | Helper and guest next to the minos binary (or the cargo target dir for a dev checkout); libkrunfw at the Homebrew canonical path with its pinned SHA-256; doctor names each path; env overrides only for the ignored VM tests | ✓ |
| A host.toml under ~/.config/minos | Paths and the hash in a user-editable file created by host setup | |
| Environment variables as in Phase 2 | MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW stay the production mechanism | |

**User's choice:** Fixed locations, no config.

---

## Scratch space for the candidate

**Q1: What backs the candidate's writable scratch space?**

The first presentation of this question (tmpfs / guest-formatted raw disk / host-pre-formatted raw disk) was interrupted: the user asked, "what's the most secure way to do this, so that it works locally and in the cloud as planned." The answer given: tmpfs, because scratch bytes never exist on the host (no guest-written file to protect or mount by mistake), every VM starts empty, the cap sits inside the hypervisor's memory limit, and Lambda MicroVMs attach no per-job block device so a raw scratch disk would be a local-only mechanism with a second guest code path; the cost is a memory budget enforced with rlimits. The question was then re-asked:

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, tmpfs with a memory budget (Recommended) | Capped tmpfs at a fixed path; Machine memory = guest reserve + tmpfs cap + candidate allowance; rlimits on the candidate uid; KRUN-04 amended; ext4 scratch noted as deferred, local-only | ✓ |
| tmpfs now, keep the raw scratch disk in the launch config | minos-vmm keeps attaching an unused empty raw scratch disk for a later disk-backed mode | |
| Raw ext4 scratch disk anyway | Local-only disk scratch with a Rust formatter in the guest; Lambda needs a second path | |

**User's choice:** tmpfs with a memory budget.
**Notes:** The user's security-and-portability framing is recorded in CONTEXT.md `<specifics>` as the test for any other local-versus-cloud fork.

**Q2: Where do the VM memory and scratch cap numbers come from in v1?**

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed defaults in code, manifest override later (Recommended) | Constants with reasons bounded by the Machine limits; a manifest field is a later addition | |
| Manifest fields now | `[environment] memory_mib` and `scratch_mib` land in `Manifest::try_from` in this phase, range-checked | ✓ |

**User's choice:** Manifest fields now (against the recommendation).

**Q3: How far should Phase 5 go in abstracting guest input (disk locally, run hook on Lambda)?**

| Option | Description | Selected |
|--------|-------------|----------|
| Keep decoders byte-oriented, disk read stays in linux.rs (Recommended) | Only the Linux entry point knows /dev/vdb and the vsock; the AWS milestone adds a hook entry point | |
| Add a JobSource / OutputSink abstraction now | Traits in minos-guest with the disk and vsock implementations in Phase 5 and a hook implementation later | ✓ |

**User's choice:** Add the abstraction now (against the recommendation).

**Q4: Which uid runs the candidate's build and run commands inside the VM?**

| Option | Description | Selected |
|--------|-------------|----------|
| One fixed unprivileged uid for both (Recommended) | A constant uid/gid created by the guest in memory, owning scratch and /out; build and run share it; PID 1 and the packer stay root | ✓ |
| Honor the Dockerfile USER if set | Ties separation to untrusted image contents; could be root | |
| Separate uids for build and run | Run cannot modify build output; permission handling between stages; no host-security gain | |

**User's choice:** One fixed unprivileged uid for both.

---

## Root images and minos.lock

**Q1: Where do built root images live on disk?**

| Option | Description | Selected |
|--------|-------------|----------|
| In .minos/blobs with a streaming put (Recommended) | One content-addressed store; ImageDigest is the blob ID; a streaming put that hashes in core closes the large-blob open question | ✓ |
| A separate .minos/images/<digest> directory | A second content-addressed tree with its own symlink and tamper checks | |
| In the project dir next to minos.lock | A multi-hundred-MB file in the working tree the snapshot must exclude | |

**User's choice:** In `.minos/blobs` with a streaming put.

**Q2: When `env build` produces a digest that differs from minos.lock, what happens?**

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse unless --update (Recommended) | Like cargo --locked; prints both digests; --update rewrites the lock | ✓ |
| Overwrite and print both digests | env build is the act of updating; no guard against an accidental rebuild | |

**User's choice:** Refuse unless `--update`.

**Q3: What does minos.lock record per backend?**

| Option | Description | Selected |
|--------|-------------|----------|
| Root digest plus its inputs (Recommended) | Root image digest, Dockerfile SHA-256, every FROM base-image digest, the guest binary SHA-256, builder name and version; a run refuses to start on mismatch | ✓ |
| Root digest only | The minimal lock | |

**User's choice:** Root digest plus its inputs.

**Q4: What reproducibility does Phase 5 require?**

| Option | Description | Selected |
|--------|-------------|----------|
| Packed image must match on rebuild; tar may differ (Recommended) | The SquashFS digest is the claim; an env build test builds twice and asserts equal digests | ✓ |
| Byte-identical tars too | Needs SOURCE_DATE_EPOCH and rewrite-timestamp support in the builder | |

**User's choice:** Packed image must match; the tar may differ.

---

## Blob hand-off to the backend

**Q1: How does the krun backend get the bytes behind LaunchPlan IDs without linking minos-store?**

| Option | Description | Selected |
|--------|-------------|----------|
| A BlobSource trait in minos-sandbox (Recommended) | Read-only get by BlobId under a cap plus a verified path for large blobs; the CLI implements it over FileStore; layering unchanged | ✓ |
| A staging directory the runner fills | The runner writes each blob as <digest> into a per-job 0700 dir; a copy of every image per job | |
| Let backend crates depend on minos-store | Relax the deny.toml wrapper | |

**User's choice:** A `BlobSource` trait in `minos-sandbox`.

**Q2: How does a multi-hundred-MB root image blob reach the VM?**

| Option | Description | Selected |
|--------|-------------|----------|
| Read-only path into the store, verified once (Recommended) | The store exposes the path after a symlink and inode check; env build verified the hash on write; the launch config names that path read-only; the Seatbelt profile lists it | ✓ |
| Copy into the job directory every launch | Rehash on copy; a full image copy per experiment | |

**User's choice:** Read-only path into the store, verified once.

**Q3: Who writes the per-job input disk and where does it live?**

| Option | Description | Selected |
|--------|-------------|----------|
| Backend writes it into the job dir from BlobSource bytes (Recommended) | The runner passes IDs; the backend fetches under caps, writes temp-then-rename in the 0700 job dir, deletes it with the job | ✓ |
| Runner builds the input disk as a blob first | Every job's input disk enters the blob store and stays | |

**User's choice:** The backend writes it into the job directory.

---

## Claude's Discretion

- rlimit values, the guest reserve, default `memory_mib` and `scratch_mib`.
- Chunked vsock framing details and the throughput measurement, within the S03-ANSWER rules.
- The console cap mechanism; the `log` backend in `minos-vmm`; what of the console reaches the report.
- Pidfile sweep layout and parent-death watchdog wiring.
- `doctor` output wording and ordering.
- Whether `minos-cli` gets its real clap skeleton now.
- Concurrency defaults (sequential in this phase; `width` and K8 with Phase 7).
- The virtio-rng device and the libkrunfw private-copy open questions.

## Deferred Ideas

- Dedicated `_minos` user (sudoers rule or LaunchDaemon) with job-dir ACLs and socket permissions.
- Local-only ext4 scratch disk with a Rust formatter, if tmpfs proves limiting.
- Concurrent candidates and K8 measurements (Phase 7, NEXT-04).
- Narrowing the Seatbelt profile's starting allowances.
- Reporting the `msb_krun_devices` vsock defects upstream (the user's call).
