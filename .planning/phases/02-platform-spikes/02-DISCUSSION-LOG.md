# Phase 2: Platform Spikes - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-09
**Phase:** 02-platform-spikes
**Areas discussed:** Spike code home and afterlife, Evidence and the CLAUDE.md rewrite, Host changes and signing, Root image engine (SPIKE-06)

---

## Spike code home and afterlife

### Where does the spike code live while the spikes run?

| Option | Description | Selected |
|--------|-------------|----------|
| In the real crates | minos-vmm gets a first real main; minos-guest a first real init; the gate covers them; Phase 5 builds on them | ✓ |
| A spikes/ workspace member | A throwaway member crate with its own deny.toml entry and msb_krun exception | |
| Outside the workspace | Probe code under a non-member directory; not covered by the gate | |

**User's choice:** In the real crates

### What happens to the spike probes once a result is recorded?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep as #[ignore] integration tests | Phase 5 promotes them into the permanent exit tests (KRUN-03, GUEST-05, KRUN-05) | ✓ |
| Delete after recording | Only the evidence file and CLAUDE.md survive | |
| Keep the helper, drop the tests | Production scaffolding stays; ad-hoc drivers removed | |

**User's choice:** Keep as #[ignore] integration tests
**Notes:** The user paused here to ask why a Linux target is needed for a local macOS build. Answer given: the libkrun guest is always Linux (libkrunfw is Linux 6.12.91); `/init` must be a static binary because the initramfs has no libc; the musl target is the standard way to get that. The user then said "ok, go ahead then".

### Pull the aarch64-unknown-linux-musl target into rust-toolchain.toml now?

| Option | Description | Selected |
|--------|-------------|----------|
| Add the musl target now | Pull Phase 1 D-09 forward; move the Phase 5 clippy line in CLAUDE.md Commands to Phase 2 onward | ✓ |
| Build the probe guest ad hoc | rustup install for this phase only; toolchain pin follows in Phase 5 | |
| Use a prebuilt static init for the spike | Boot with busybox static; weaker evidence for SPIKE-01 | |

**User's choice:** Add the musl target now (confirmed in free text after the clarification)

### How should minos-vmm receive its launch settings for the probes?

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal versioned JSON config | serde config (kernel, disks, vsock port, socket path, max duration) from stdin or a file; Phase 5 keeps the shape | ✓ |
| Hard-coded paths and CLI flags | Fastest to write; Phase 5 replaces it | |
| You decide | Claude picks whichever boots fastest within the lints | |

**User's choice:** Minimal versioned JSON config

### Include K7 (exit codes) and K8 (launch rate, memory per VM)?

| Option | Description | Selected |
|--------|-------------|----------|
| Fold K7 in, defer K8 | Exit-code behavior falls out of SPIKE-01; K8 ties to NEXT-04 | ✓ |
| Include both | Record exit codes and a small concurrency/memory measurement | |
| Neither | Stick to the eight SPIKE items exactly | |

**User's choice:** Fold K7 in, defer K8

---

## Evidence and the CLAUDE.md rewrite

### Where does the raw spike evidence go?

| Option | Description | Selected |
|--------|-------------|----------|
| 02-SPIKE-EVIDENCE.md in .planning | Same pattern as 01-GATE-EVIDENCE.md; CLAUDE.md gets the conclusion and recipe | ✓ |
| Everything in CLAUDE.md | Conclusions and supporting output in the design of record | |
| A docs/ file in the repo | Evidence becomes public documentation | |

**User's choice:** 02-SPIKE-EVIDENCE.md in .planning

### How far should the CLAUDE.md rewrite go?

| Option | Description | Selected |
|--------|-------------|----------|
| Amend in place | Edit the existing sections; no docs/ split yet | ✓ |
| Start the docs/ split now | Move design sections into docs/; keep CLAUDE.md to rules, layout, commands | |
| You decide | Single design of record unless the amendments make it unwieldy | |

**User's choice:** Amend in place

### How should the v1 rewrite treat the AWS-first content?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep, mark deferred | Label the sections as the later AWS milestone; AWS open questions to a deferred list; local-first order up front | ✓ |
| Trim to a summary | Collapse AWS sections to a paragraph with a pointer to research | |
| Remove entirely | Cut AWS content; restore from git history later | |

**User's choice:** Keep, mark deferred

### If a spike result contradicts .claude/CLAUDE.md or the research files, what gets corrected?

| Option | Description | Selected |
|--------|-------------|----------|
| CLAUDE.md and .claude/CLAUDE.md both | Fix both in the same change; research files stay as dated inputs with a note in the evidence file | ✓ |
| Only CLAUDE.md | Design of record is the single source | |
| All three | Also correct the research file that made the claim | |

**User's choice:** CLAUDE.md and .claude/CLAUDE.md both

---

## Host changes and signing

### Which host changes may the spike plans make without asking? (multi-select)

| Option | Description | Selected |
|--------|-------------|----------|
| brew tap libkrun/krun and install libkrunfw | Required for SPIKE-01; no sudo | ✓ |
| rustup target add for musl | Follows from the musl decision; no sudo | ✓ |
| Create a dedicated _minos user | SPIKE-05 fallback; needs sudo; lasting system change | |
| Install or start a container engine | For SPIKE-06 if the chosen engine is not usable | |

**User's choice:** brew libkrunfw and the musl target only; the other two require asking first

### How does minos-vmm get its hypervisor entitlement with no xtask?

| Option | Description | Selected |
|--------|-------------|----------|
| Documented codesign command | Plist in crates/minos-vmm; one codesign line in CLAUDE.md Commands; Phase 5 host setup may run the same argv | ✓ |
| cargo alias in .cargo/config.toml | Not possible: aliases only expand to cargo subcommands | |
| Defer to Phase 5 minos host setup | Sign by hand for the spike; record nothing | |

**User's choice:** Documented codesign command

### How strong should the unsigned-helper-refused evidence be?

| Option | Description | Selected |
|--------|-------------|----------|
| Run the unsigned binary and capture HV_DENIED | Unsigned run recorded, then signed boot recorded | ✓ |
| Strip the entitlement from a signed copy | Also re-sign with empty entitlements to separate signature from entitlement | |
| Unsigned run only, no re-sign variant | Minimum proof | |

**User's choice:** Run the unsigned binary and capture HV_DENIED

### K5: try the hardened runtime without disable-library-validation first?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, try hardened first, fall back and record | Hypervisor entitlement only under --options runtime; add disable-library-validation only if dlopen fails, and record why | ✓ |
| Plain ad-hoc signing only | Hypervisor entitlement, no hardened runtime, as the krunkit formula does | |
| You decide | Minimal entitlement set that boots | |

**User's choice:** Yes, try hardened first, fall back and record

---

## Root image engine (SPIKE-06)

### Which tool chain should SPIKE-06 try first?

| Option | Description | Selected |
|--------|-------------|----------|
| Docker CLI already on PATH | docker build then export to a rootfs tar; backhand packs SquashFS; Docker Desktop is one option, not a requirement | ✓ |
| Builder VM inside libkrun | No engine dependency; much bigger spike; builder VM needs egress | |
| lima or podman | lima installed without a VM; podman absent; both need setup that must be asked about | |
| Apple container (macOS 26+) | Not installed; future option | |

**User's choice:** Docker CLI already on PATH

### What is proof that SPIKE-06 is answered?

| Option | Description | Selected |
|--------|-------------|----------|
| Boot the produced image | Trivial Dockerfile to rootfs tar to SquashFS; the SPIKE-01 probe boots it as vda and runs a command | ✓ |
| Produce the image and inspect it | Stop at a SquashFS whose contents match the Dockerfile | |
| Document the chain only | Commands verified individually, no end-to-end run | |

**User's choice:** Boot the produced image

### Must the image build be reproducible in the spike?

| Option | Description | Selected |
|--------|-------------|----------|
| Note it, do not require it | Record whether two builds match and why not; reproducibility is Phase 5 | ✓ |
| Require byte-identical images | Find a chain with matching SquashFS digests | |
| You decide | | |

**User's choice:** Note it, do not require it

### SquashFS and initramfs writers: Rust now or host tools?

| Option | Description | Selected |
|--------|-------------|----------|
| Rust now, in the real crate | backhand SquashFS packer and newc initramfs writer as first real code under the gate | ✓ |
| Host tools for the spike | mksquashfs and cpio from Homebrew now; Rust writers in Phase 5 | |
| You decide | | |

**User's choice:** Rust now, in the real crate

---

## Claude's Discretion

The user chose "I'm ready for context" when offered three further gray areas, leaving them to Claude:

- SPIKE-07 spend and flag matrix (cheapest model alias, one call per combination, the flag list in CONTEXT.md).
- Whether independent spikes continue while a SPIKE-01 failure waits on the user (they do; VM-dependent spikes stop).
- Seatbelt profile scope for SPIKE-05 (deny-default, allow only the VMM's file, mach, and scratch needs; no network).

Also at Claude's discretion: spike order within the dependency graph and plan split; launch config field names, socket directory location, console cap; the trivial Dockerfile and its boot-proof command; what the SPIKE-01 guest prints and exits with.

## Deferred Ideas

- K8 benchmark (launches per second, memory per VM) — with NEXT-04.
- docs/ split of CLAUDE.md — Phase 8.
- Creating the `_minos` user — Phase 5 `minos host setup`.
- Reproducible image builds and `minos.lock` — Phase 5.
- lima, podman, Apple container as engines — Phase 5 or Phase 8 docs.
- Hostile `.claude/settings.json` fixture for the claude provider — Phase 6 (PROP-03).
- Promoting ignored spike tests into permanent exit tests — Phase 5.
- Empty-entitlements re-sign variant — not needed; possibly `host doctor` in Phase 5.
