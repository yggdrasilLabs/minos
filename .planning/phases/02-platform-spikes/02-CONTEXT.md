# Phase 2: Platform Spikes - Context

**Gathered:** 2026-10-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 turns every unverified fact that the krun backend and the `claude` proposer rest on into an evidence-backed answer, and amends CLAUDE.md so it describes the v1 local-first design. It delivers facts, probe code kept as ignored integration tests, and docs. It delivers no loop, no runner, no CLI command, and no sandbox type.

Requirements: SPIKE-01 through SPIKE-08 and DOCS-05. Cross-cutting: GATE-06 (`cargo clean` after verification passes) and `rules/` adherence. The five gate commands from Phase 1 apply to every line of spike code, plus the musl clippy line that this phase pulls forward (D-03).

The spikes, in the order their dependencies impose: signing (SPIKE-04) is a prerequisite for any boot, since Hypervisor.framework refuses `hv_vm_create` without the entitlement. The boot (SPIKE-01, with K7 folded in) is the stop-and-ask gate. Egress (SPIKE-02), vsock (SPIKE-03), and Seatbelt (SPIKE-05) need a booting VM. The root image spike (SPIKE-06) is independent up to image production and needs the boot for its proof. The `claude -p` spike (SPIKE-07) is fully independent.

**Stop-and-ask point (SPIKE-08):** if msb_krun cannot boot libkrunfw on this Mac, the VM-dependent spikes stop and the user is asked, with the pre-researched alternatives from `.planning/research/ARCHITECTURE.md` Pattern 6 and `.claude/CLAUDE.md` Question 1 laid out. No `unsafe` exception, no `#![allow(unsafe_code)]`, no `-sys` crate, no krunkit fallback is tried before the user answers. The independent spikes (SPIKE-06 up to image production, SPIKE-07) continue while the question is open.

</domain>

<decisions>
## Implementation Decisions

### Spike code home and afterlife
- **D-01:** Spike code lives in the real crates, not in a `spikes/` member and not outside the workspace. `minos-vmm` gets its first real `main`: it reads a launch config (D-04) and drives msb_krun. `minos-guest` gets its first real `/init`. The SquashFS packer (backhand, gzip only) and the hand-rolled `newc` initramfs writer are written in Rust now, in the crate where Phase 5 places `env build` (the planner confirms the crate against `.planning/research/ARCHITECTURE.md` and the deny.toml layering; it is never `minos-core` or `minos-loop`). Everything passes the gate; no new workspace member, no deny.toml exception.
- **D-02:** Probes are kept, not deleted. Each spike that needs a VM becomes an `#[ignore]`d integration test (it needs libkrunfw and a signed helper on the host). Phase 5 promotes them into the permanent exit tests: egress fails closed (KRUN-03), no host environment in the guest (GUEST-05), no VM after `kill -9` (KRUN-05). The spike result must therefore be reproducible by running the test again, not only recorded.
- **D-03:** The `aarch64-unknown-linux-musl` target is added to `rust-toolchain.toml` in this phase, pulling Phase 1's D-09 forward. The reason, confirmed with the user: the libkrun guest is always Linux (libkrunfw is Linux 6.12.91), and `/init` must be a fully static binary because the initramfs has no libc. CLAUDE.md Commands moves the musl clippy line from "Phase 5 onward" to Phase 2 onward, and the host gate keeps building `minos-guest` for the host target so the forbid backstop covers the non-Linux parts (keep the guest free of `#[cfg(target_os = "linux")]` on anything the host can compile). `cargo test --workspace` still cannot run the guest's tests on macOS; that waits for a Linux runner.
- **D-04:** `minos-vmm` receives its launch settings as a minimal versioned JSON config (a `v` tag, `deny_unknown_fields`, `try_from` validation), read from stdin or a file path given on argv. Fields are the ones the real launcher needs: kernel (libkrunfw path and expected hash), disks (shared read-only root, per-job read-only input, size-capped scratch, all raw), vsock port and host Unix socket path, hard maximum duration, console output cap. No hard-coded paths and no ad-hoc flag soup. Phase 5 keeps the shape and adds the golden file.
- **D-05:** K7 is folded into SPIKE-01: the spike records how a guest exit and a VMM error each surface to the parent (exit code, msb_krun `ExitHandle` or `on_exit` behavior), since Phase 5 needs to separate `Outcome::Crashed` from an infrastructure error. K8 (launches per second, memory per VM) is deferred with NEXT-04.

### Evidence and the CLAUDE.md rewrite
- **D-06:** Raw evidence goes in `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`, following `01-GATE-EVIDENCE.md`: for every spike, the verbatim command, exit code, and output (truncated only where noted, never summarized). CLAUDE.md gets the conclusion and the recipe, not the logs.
- **D-07:** CLAUDE.md is amended in place. No `docs/` split in this phase; Phase 8 writes SECURITY.md and README and may split then. The DOCS-05 amendments to make: the local-first build order stated up front; the `ModelProvider` decision and the `claude` CLI provider as the personal default; no tokio in any `minos-*` crate in v1; pico-dollar rates (verify the Cost section already says pico, and fix any remaining "nano"); the `libkrun/krun` tap; libkrun's networking default (implicit TSI) and the msb_krun counter-default; the `minos-loop` crate in the Workspace table and dependency rules (verify it is already listed; add the loop-never-links-sandbox rule if missing); every spike result in Verified facts with the date; the codesign line and the musl clippy line in Commands; the P0 open questions answered and removed from Open questions.
- **D-08:** The AWS-first content (durable execution rules, Lambda limits, orchestrator, MicroVM facts) stays in CLAUDE.md and is labelled as the later AWS milestone. The AWS open questions move to a clearly marked deferred list. Nothing is trimmed or removed.
- **D-09:** When a spike result contradicts the stack of record, both `CLAUDE.md` and `.claude/CLAUDE.md` are corrected in the same change, as 01-05 did for clap. The research files under `.planning/research/` stay as written, because they are dated inputs; the evidence file notes which research claim was overturned and where.

### Host changes and signing
- **D-10:** The spike plans may do these on this Mac without asking: `brew tap libkrun/krun`, `brew trust libkrun/krun`, `brew install libkrunfw` (only libkrunfw; not libkrun, krunkit, or krunvm), and the rustup musl target install that `rust-toolchain.toml` triggers. The plans must stop and ask before: creating a dedicated `_minos` user (needs sudo; the SPIKE-05 fallback is recorded as a recipe, not executed), and installing or starting any container engine (starting Docker Desktop if its daemon is down, creating a lima VM, installing podman or Apple's container tool).
- **D-11:** Signing without an xtask, consistent with Phase 1 D-01 and D-07: an entitlements plist at `crates/minos-vmm/minos-vmm.entitlements` and one documented `codesign` argv in CLAUDE.md Commands, run by hand after `cargo build`. No script, no `build.rs`, no cargo alias (aliases only expand to cargo subcommands). Phase 5's `minos host setup` may run the same argv from Rust later. Every `cargo build` relinks and drops the signature, so the documented sequence is build, then sign, then run.
- **D-12:** SPIKE-04 evidence is two verbatim runs: the unsigned helper refused by Hypervisor.framework (the exact error, expected `HV_DENIED`), then the signed helper booting. Both go in the evidence file. The entitlement-versus-signature distinction (re-signing with an empty entitlements file) is not required.
- **D-13:** K5 is resolved by trying the hardened runtime first: `codesign --force -s - --options runtime` with only `com.apple.security.hypervisor`. If `dlopen` of the Homebrew libkrunfw dylib then fails, add `com.apple.security.cs.disable-library-validation`, record the failure that forced it, and keep the smallest set that boots. The final entitlement set and its reason go in CLAUDE.md.

### Root image engine (SPIKE-06)
- **D-14:** The first tool chain tried is the Docker CLI already on PATH: `docker build` for arm64, then export the filesystem to a rootfs tar (`docker export` of a created container, or `buildx --output type=tar`), then the Rust SquashFS packer (D-01) turns the tar into a read-only raw image. Docker Desktop is one option for the public project, not a requirement; CLAUDE.md states the requirement as "any OCI builder that can export a rootfs tar" and lists lima, podman, and Apple's container tool as alternatives, with whichever of them the spike actually checked marked as such. If the Docker daemon is not running, that is a host change under D-10 and the executor asks first.
- **D-15:** SPIKE-06 is answered only when the produced image boots: a trivial Dockerfile (base image pinned by digest, arm64) becomes a SquashFS image, the SPIKE-01 probe boots with it as the root disk, and a command from that image runs and its output comes back. Producing and inspecting the image is not enough.
- **D-16:** Reproducibility is noted, not required. The spike builds twice and records whether the SquashFS digests match and, if not, why (timestamps, package metadata). Byte-identical builds and `minos.lock` are Phase 5.
- **D-17:** The SquashFS writer and the initramfs writer are Rust in the real crate, not `mksquashfs` or `cpio` from Homebrew. The initramfs contains only `/init` = `minos-guest`, with zeroed metadata and a stable hash.

### Boot path (decided by the user on 2026-10-09, after research)
- **D-18:** Stock libkrunfw 5.6.2, the version the `libkrun/krun` tap installs, has `CONFIG_BLK_DEV_INITRD` unset (verified in upstream `config-libkrunfw_aarch64` at tag v5.6.2), so the kernel cannot unpack an initramfs and SPIKE-01 as written cannot pass. Presented with the three `forbid(unsafe_code)`-preserving options from `02-RESEARCH.md` Open Question 1, the user chose the **root-disk boot**: the VM boots straight from the raw read-only SquashFS root on `/dev/vda`, with the static musl `/init` = `minos-guest` written into the image by the trusted packer. No initramfs, no virtiofs, no `unsafe`. The launcher places `root=/dev/vda` last on the kernel command line through `KernelBuilder::init_path` (Linux keeps the last `root=`), and the boot test asserts the exact `/proc/cmdline` so an msb_krun bump cannot change the layout silently. The literal initramfs attempt (`02-RESEARCH.md` Pattern 2) is run once and recorded as negative evidence, together with the config line that explains it; the newc initramfs writer from D-17 stays for that run and keeps its golden test. Rejected: an in-process single-file virtiofs root (breaks the no-virtiofs rule and exposes a FUSE server to the guest) and rebuilding libkrunfw with initrd support (a new supply-chain step that diverges from Homebrew). The wording of SPIKE-01 and KRUN-09 ("an initramfs"), ROADMAP Phase 2 success criterion 1, and KRUN-08 ("an `xtask`", already contradicted by Phase 1 D-01 and this phase's D-11) is amended in the same change that records the decision, each marked as superseded on 2026-10-09 with this reason. SPIKE-08 still applies: if the root-disk boot itself fails, VM-dependent work stops at a decision checkpoint and no alternative is tried before the user answers.

### Claude's Discretion
- **SPIKE-07 spend and flags.** Use the cheapest model alias, one call per flag combination, and stop as soon as each question has an answer. Test at minimum: `--tools ""` with `--json-schema`, `--disallowedTools`, `--output-format json`, `--no-session-persistence`, `--strict-mcp-config` with an empty MCP config, `--safe-mode`, the permission flag that works with `-p`, and `--max-turns 1`; from a fresh empty temp directory outside any repo; with `env_clear()` plus the `HOME`, `PATH`, `USER`, `LOGNAME`, `TMPDIR`, `LANG` allowlist. Record the exact `structured_output` shape, the result `subtype` values seen, the usage field names, whether `--tools ""` removes the Agent and Skill tools, and whether subscription auth works through the allowlisted environment (Keychain access). Record tokens only; never record `total_cost_usd`. The hostile-fixture test (PROP-03) is Phase 6, not this spike.
- **Seatbelt profile scope (SPIKE-05).** Deny-default; allow only what the VMM needs: reading the root, input, and initramfs images, the libkrunfw dylib, and system libraries; writing the scratch disk and the socket directory; the mach lookups Hypervisor.framework requires; no network. Record the result either way with the profile text in the evidence file. If deny-default breaks `hv_vm_create`, record the smallest set of allowances that restores it, and record the dedicated-user fallback as a recipe (D-10).
- **Spike order within the dependency graph** in the domain section, and how many plans the phase splits into.
- **Launch config field names**, the short socket directory location (macOS Unix socket path limit, PITFALLS Pitfall 30), and the console output cap value.
- **The trivial Dockerfile** used for SPIKE-06 and the command it runs for the boot proof.
- **What the SPIKE-01 guest does to prove the boot:** at least print a marker, read `/proc/cmdline`, and exit with a known code so K7 can be recorded.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design of record and stack of record
- `CLAUDE.md` — Threat model; Workspace (crate list, dependency direction); Code rules; Local backends; Environments and source; Commands (the gate, the musl line, `cargo clean`); Build order; Open questions; Verified facts. This is the file DOCS-05 amends (D-07, D-08).
- `.claude/CLAUDE.md` — Decisions at a glance (1, 1a, 1b, 1c, 1d, 11); Question 1 in depth, including the K1 through K8 table and the macOS signing specifics; Question 3 (deterministic tar, host-side unpack rules); Question 9 (`claude -p` flags, the unverified list); Question 10 (no tokio); Stack Patterns by Variant (what to do if the initramfs boot needs a NullFs root, or if msb_krun fails); What NOT to Use. Corrected alongside CLAUDE.md on any contradiction (D-09).
- `rules/` — every rule applies to the probe code; in particular `rules/err-thiserror-lib.md` (typed errors in `minos-vmm` and `minos-guest`) and the lint rules Phase 1 adopted.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — SPIKE-01 through SPIKE-08 and DOCS-05 (this phase); GUEST-01, KRUN-01, KRUN-02, KRUN-03, KRUN-04, KRUN-05, KRUN-08, KRUN-09, PROP-02, PROP-04 (what Phase 5 and Phase 6 will build on the spike answers, so the probes should take their shape).
- `.planning/ROADMAP.md` — Phase 2 goal, success criteria, and the stop-and-ask point; Dependencies and Parallelism (Phase 2 runs alongside Phase 3; Phases 5 and 6 wait on it).
- `.planning/PROJECT.md` — Context section (verified `claude -p` facts of 2026-10-09, terms of use, libkrun binding note) and Key Decisions.

### Prior phase
- `.planning/phases/01-workspace-and-gates/01-CONTEXT.md` — D-01 (no scripts, no xtask), D-07 (no xtask member), D-09 (musl target deferred; this phase pulls it forward), D-15 (C-01 and C-02 checks every plan runs); Deferred Ideas (the codesign mechanism, resolved here by D-11).
- `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` — R-01 to R-03 (what `#[expect]` can and cannot do), G-02-CLOSED (a `target.<triple>` rustflags key silently replaces the forbid backstop: never add one for the musl target).

### Research
- `.planning/research/STACK.md` — Question 1 (msb_krun recipe, K1 to K8, entitlements), Question 3 (tar), Question 9 (`claude -p`), Supporting Libraries (backhand gzip-only, hand-rolled `newc` initramfs), Stack Patterns by Variant.
- `.planning/research/PITFALLS.md` — Pitfalls 1 (implicit TSI), 2 (libkrun is not a boundary alone; Seatbelt), 3 (`krun_start_enter` takes over the process), 4 (host environment and TTY leak into the guest), 5 (no virtiofs), 6 (raw only, never probe a format), 7 (hard deadline, orphaned VMs), 12 (`claude -p` runs project hooks and MCP), 21 (unsafe versus the C ABI), 22 (the entitlement disappears on rebuild), 30 (socket path length), 31 and 32 (guest startup and PID 1 duties).
- `.planning/research/ARCHITECTURE.md` — Pattern 6 (helper form; the stop-and-ask options), the P0 spike row, amendment A11 (which open questions this phase answers), the disk layout table.
- `.planning/research/SUMMARY.md` — the Phase 1 spike list it originally proposed (K1 to K8 plus the `claude` contract) and the stop-and-ask wording.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- The ten-crate workspace from Phase 1, with every v1 dependency already pinned in `[workspace.dependencies]`: `msb_krun =0.1.41` (`default-features = false`, `features = ["blk"]`) in `minos-vmm`; `rustix` and `vsock` under a `cfg(target_os = "linux")` target table in `minos-guest`; `tar`, `backhand` (gzip only), `serde`, `serde_json`, `thiserror` declared. The planner verifies which crate declares `backhand` before placing the SquashFS writer (D-01), since `[bans.workspace-dependencies] unused = "deny"` means it is declared somewhere.
- `deny.toml` already lists `aarch64-unknown-linux-musl` in `[graph] targets`, allowlists the `msb_krun_hvf` and `kvm-ioctls` build scripts, and confines `msb_krun` to `minos-vmm` and `tokio` to `msb-imago`. Adding the toolchain target changes nothing there.
- `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` is the model for `02-SPIKE-EVIDENCE.md` (D-06).

### Established Patterns
- Policy as Cargo-native configuration, never a script or helper crate (Phase 1 D-01). D-11 keeps signing inside that rule: a plist plus a documented argv.
- No `unwrap`, `expect`, or slice indexing anywhere, including tests and probes; tests return `Result` and use `?`. `#[expect]` cannot silence the three forbid lints.
- Every plan ends with the five gate commands plus, from this phase, the musl clippy line; then the C-01 and C-02 checks from Phase 1 D-15; then `cargo clean` at the end of the phase.
- Validated types: the launch config (D-04) uses `#[serde(try_from = ...)]` and `deny_unknown_fields`, the same pattern core will use.

### Integration Points
- `minos-vmm` and `minos-guest` stubs (`src/main.rs` each) are where the probe bodies go (D-01). The guest keeps its Linux-only code behind the existing target table and everything else host-compilable (D-03).
- CLAUDE.md Commands gains the codesign line and moves the musl clippy line to Phase 2 onward (D-03, D-11).
- Phase 5 consumes: the launch config shape, the entitlement set, the egress and vsock tests, the SquashFS and initramfs writers, and the Seatbelt answer. Phase 6 consumes the `claude -p` contract. Phase 8 consumes the root-image alternatives list for the README.

</code_context>

<specifics>
## Specific Ideas

- The user asked why a Linux target is needed for a local macOS build; the answer (the guest is always Linux, `/init` must be static) should appear in CLAUDE.md where the musl target is introduced, so the next reader does not ask again.
- Evidence means the actual failing or passing command text, as in Phase 1, not a summary. An unsigned run that is refused is as much a deliverable as a signed run that boots.
- Docker is what is on this machine today, but the public project must not read as "requires Docker Desktop".

</specifics>

<deferred>
## Deferred Ideas

- **K8 benchmark** (launches per second, memory per VM) — with NEXT-04, concurrent candidate VMs.
- **`docs/` split of CLAUDE.md** — Phase 8, alongside SECURITY.md and README.
- **Creating the `_minos` dedicated user** — Phase 5 `minos host setup`; this phase records the recipe only (D-10).
- **Reproducible image builds and `minos.lock`** — Phase 5 `env build` (D-16).
- **Checking lima, podman, and Apple's container tool as engines** — Phase 5 or Phase 8 docs; this phase lists them as untested alternatives unless the spike had reason to try one (D-14).
- **Hostile `.claude/settings.json`, hooks, and `.mcp.json` fixture for the `claude` provider** — Phase 6, PROP-03.
- **Promoting the ignored spike tests into the permanent exit tests** — Phase 5 (D-02).
- **Re-signing with an empty entitlements file to separate signature from entitlement** — not needed; Phase 5 may add it to `host doctor` output if useful.

</deferred>

---

*Phase: 02-platform-spikes*
*Context gathered: 2026-10-09*
