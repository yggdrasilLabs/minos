# Security

This document says what Minos isolates, what it trusts, what its preflight checks, and where it falls short. It
describes the first milestone, which runs locally on macOS arm64. No isolation is perfect, and Minos does not claim
to be. If you find a place where this document says more than the code does, that is a bug; please report it.

## Threat model

Minos runs code it did not write: experiments proposed by a model, and experiments other people send you to run on
your hardware. Two kinds of actor are in scope:

- **A sloppy agent** that wanders outside its lane: edits a file it should not, reads what it was not given, or
  tries to make its score look better than it is.
- **Adversarial code**, written to escape the sandbox, reach your network, read your keys, or tamper with the
  judge that scores it.

The target is that escape requires a zero-day: a vulnerability in Apple's Hypervisor.framework or the macOS kernel,
in the libkrun virtual machine monitor, or in the guest kernel together with one of those. Escape should never need
only a mistake in Minos's own code or configuration. That is a goal the design is built around and tested against,
not a guarantee: Minos is new, has not had an outside audit, and runs on a trust base it does not control (listed
below).

Everything that leaves a sandbox is treated as hostile input: logs, results, artifacts, file names, diffs, model
output, and tool results.

## What Minos isolates

- **Each experiment runs in its own hardware-virtualized VM.** The VMs run under Hypervisor.framework through
  libkrun. Process-only isolation does not exist in this milestone: the launch preflight refuses a host that
  isolates below hardware virtualization.
- **No VM has a network device.** No VM is given a virtio-net device, and none has a shared host directory
  (virtio-fs). This rests on one point that is worth stating plainly: the guest kernel (libkrunfw) has TSI
  compiled in, a libkrun feature that proxies guest sockets through the host, and upstream libkrun turns it on
  whenever a VM has no network device. Minos drives libkrun through the `msb_krun` crate, whose default is off,
  builds it without its `net` feature, and bans the methods that would enable it with a clippy rule in the
  `minos-vmm` crate. No egress therefore depends on the VMM never enabling that proxy. The ignored VM test
  `exit_egress` checks it: every guest TCP connect, to loopback, the LAN, and public IPv4 and IPv6 addresses,
  fails, no host canary sees a connection, and the guest sees only `lo`.
- **Files go in as read-only disk images, and output comes out as a stream.** The root image and a per-job input
  disk are attached as raw, read-only block devices, with the format fixed by code and never probed. Scratch space
  is a size-capped tmpfs in guest memory, so it never exists on the host. Output comes back as a tar stream over
  vsock, carried in short chunks, each authenticated with a 32-byte per-launch token that only the guest's PID 1
  can read; the candidate cannot forge the stream (the ignored test `exit_forge`). The host parses the stream in
  Rust under caps and never unpacks it to disk, and never mounts a disk the guest wrote.
- **The candidate and the judge run in separate VMs.** The candidate's VM builds and runs the experiment. The
  judge's VM receives only the judge data and the candidate's `out/` files, re-packed by the host, and unpacks them
  under `/scratch/candidate/out/`. The type system enforces this: a candidate sandbox spec has no way to attach
  judge data, and no role has a way to attach a network; compile-fail tests hold both.
- **Judge data never reaches the candidate or the proposer.** Every path the judge command names must lie under a
  `[source] exclude` entry, so the judge is in neither the source snapshot nor the image build context. A
  snapshot is refused if any packed file has the same SHA-256 as a pushed judge file.
- **Scores come only from the judge and from host measurement.** The candidate reports nothing about itself. Time
  and resource usage are measured by the host, and no number a guest writes is read as usage.
- **Output is rendered through an allowlist.** Every string from a sandbox or a model is printed through a renderer
  that keeps printable characters, newlines, and tabs, and drops control characters, terminal escape sequences,
  and Unicode bidirectional overrides. Stage logs print only when asked for (`minos show --logs`).
- **Nothing reaches your working tree without review.** `minos report` prints a report and its hash. `minos adopt`
  rebuilds the report and refuses unless you pass the hash of the report as it stands now, records the
  acknowledgment, and only then writes a patch file. It never applies a patch; you apply it yourself.

## Trust base

Minos's isolation is only as strong as these components. Versions are the ones this milestone is pinned to and
tested with.

- **Hypervisor.framework and the macOS kernel** (tested on macOS 27, arm64): the hardware virtualization boundary.
- **The libkrun VMM, through `msb_krun` 0.1.41**, compiled into the `minos-vmm` helper. One helper process runs
  per VM. The VMM shares the guest's security context, so a VMM bug gives code in the helper process, which is
  why the helper is confined (next items). `minos-vmm` is the only binary that links the VMM.
- **libkrunfw 5.6.2** from the Homebrew tap `libkrun/krun`, the guest Linux kernel (6.12.109), loaded by absolute
  path after its SHA-256 is checked against a pinned value.
- **The Seatbelt profile**: `minos-vmm` runs under `sandbox-exec` with a deny-default profile embedded in the
  binary. It allows reading the system libraries, the helper's own files, libkrunfw, and the job's two disk
  images; reading the metadata of those files' parent directories and a short list of sysctls; writing the
  helper's console pipe; and one network operation, the Unix socket connect for vsock.
- **Resource limits (rlimits)**: the helper sets its own (no core files, bounded open files, a file size limit of
  zero) before reading its config, and the guest sets per-stage limits on the candidate (address space, file size, processes,
  CPU, open files, no core files) before running it. Each half fails closed.
- **`minos-guest`**, the static binary that is PID 1 in every VM. The trusted packer writes it into the root image
  as `/init` after every file of your image, so a Dockerfile cannot replace it. The build and run stages run as
  uid 64000.
- **The Docker builder, at image build time only.** `minos env build` runs `docker buildx` on a snapshot of your
  project to produce a root filesystem. The build has registry access, so it is part of your supply chain; base
  images must be pinned by digest. The builder's output is treated as hostile input to Minos's own SquashFS
  packer.
- **The `claude` CLI, outside every sandbox.** The default model provider runs your own `claude` install as a
  subprocess on your machine, from a fresh empty directory with a cleared environment and all tools off. It is not
  sandboxed by Minos. Its output is model output and is parsed strictly as hostile input.
- **The Rust dependency tree**, locked by a committed `Cargo.lock` and checked by `cargo deny` (licenses,
  advisories, sources, and which crates may link which).

## What the preflight checks

`minos host doctor` runs every check below, prints each with the paths it examined and a fix, and exits 0 only
when nothing is refused. The same probes run once when a backend starts, and a run cannot launch a VM on a host
whose probes failed.

- **The helper:** `minos-vmm` is a regular file, and its signature sets both entitlements it needs,
  `com.apple.security.hypervisor` and `com.apple.security.cs.disable-library-validation`, to true. (`minos host
  setup` signs it.)
- **The guest kernel:** libkrunfw is at its Homebrew path and its SHA-256 equals the pinned value.
- **The hypervisor:** Hypervisor.framework support is reported by the kernel (`kern.hv_support`), and the largest
  VM is sized to leave half the host's memory to the host.
- **The job directory:** the per-user job root under `/private/tmp` is refused if it is a symlink, belongs to
  another user, or is not mode 0700, and the vsock socket path fits a Unix socket address. Left-over job
  directories from a dead process are swept; an orphaned helper is killed only after `ps` proves it is one.
- **Seatbelt:** `/usr/bin/sandbox-exec` is present, and the production profile is seen denying two things right
  now: a loopback TCP connect to a listener the probe owns (after an unsandboxed control connect succeeds) and a
  read of an unlisted file (after a listed file reads back). A failed control is inconclusive and fails closed.
  **No run is recorded without Seatbelt:** a host without it is refused, never silently downgraded.
- **The guest binary:** `minos-guest` is a static aarch64 Linux ELF, so the image boots the binary Minos built.

Before every launch, the preflight also refuses a plan whose architecture differs from the host's, whose vCPUs,
memory, or duration exceed the host's limits, or a judge plan without judge data or with judge data on its input
disk. Every launch has a hard maximum duration enforced from outside the VM, and the helper exits on its own if
the process that launched it dies (the ignored test `exit_kill9`).

What the preflight does not check: that your Dockerfile or base image is benign, that the macOS kernel or
Hypervisor.framework is free of bugs, that the Seatbelt profile denies everything beyond the two probes, or
anything about the `claude` CLI beyond its version and its login (its own preflight, run before each command that
calls a model).

Every run records which confinement layers were active in its ledger, and the report shows them under provenance.

## Known limits and recorded exceptions

- **`KRUN_BOOT_START_NS` on the guest's kernel command line.** `msb_krun` 0.1.41 always writes this one
  host-derived value, the host's wall-clock time at VM build, and no option removes it. It reveals the host's
  clock at boot, which the guest's own clock reveals anyway. The ignored test `exit_env` allows that one token and
  matches every other token of `/proc/cmdline`. It is a recorded exception, not an endorsement; the removal path
  is to vendor the one `msb_krun` crate with a one-line change.
- **The dedicated `_minos` user is not implemented.** The helper runs as you, under Seatbelt and rlimits. Every
  run of this milestone records the dedicated user as missing, and the report shows "reduced hardening" in its
  provenance.
- **libkrunfw is checked, then loaded.** The helper hashes libkrunfw and then loads it from the Homebrew Cellar
  path. Between the two, a process that can write `/opt/homebrew/Cellar` could swap the file. That attacker is
  already on your machine with your privileges; the threat model's attacker is the guest.
- **The pinned VMM's vsock has defects.** On `msb_krun_devices` 0.1.41 one guest-to-host connection loses data or
  stalls above 8192 bytes, for three defects in that crate. Minos works within the limit: output is a sequence of
  short connections of at most 4 KiB of payload each, authenticated and checked for gaps, duplicates, and length,
  so a lost chunk is a detected failure, never silent corruption.
- **`sandbox-exec` is deprecated by Apple.** It still works on macOS 27 and is required; `minos host doctor`
  probes that it still enforces the profile, and a host where it does not is refused.
- **The Seatbelt profile's allowances were not minimized one by one.** Only the socket rule was shown to be
  needed; the rest are the starting allowances the VM needed to boot.
- **A committed `minos.lock` reveals judge file names and digests.** It never holds judge data, but anyone can
  guess the content of a low-entropy judge file (a short expected answer, a small integer) by hashing guesses
  against its digest.
- **The `claude` CLI runs outside every sandbox.** It is your own install under your own login. Minos runs it with
  tools off, safe mode, no MCP servers, and an empty working directory. A live test on `claude` 2.1.312
  (2026-10-28) ran the CLI with a hostile fixture project as its working directory and saw none of that fixture's
  hooks, MCP server, or CLAUDE.md take effect; that holds for that version and that fixture, and a later CLI
  version needs the test rerun.
- **Side channels are not addressed.** VMs share the host's CPUs and memory with each other and with you. Minos
  makes no claim about timing or cache side channels.
- **Running several VMs at once is unmeasured.** The loop in this milestone runs one VM at a time.

## Reporting a vulnerability

Please report a vulnerability privately through this repository's GitHub security advisories ("Report a
vulnerability" under the Security tab), never in a public issue. No personal address is published for reports.
Include what you ran, what you saw, and the versions of macOS, libkrunfw, Minos, and the `claude` CLI involved.
