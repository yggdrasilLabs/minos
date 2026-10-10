# Phase 2: Platform Spikes - Research

**Researched:** 2026-10-09
**Domain:** libkrun (msb_krun 0.1.41) on macOS 27 arm64 with Hypervisor.framework; static musl guest; SquashFS root images from a Dockerfile; Seatbelt; `claude -p` print-mode contract
**Confidence:** HIGH for API surfaces and kernel facts (pinned source read, local probes). MEDIUM for runtime behavior that only the spikes can show (boot, Seatbelt, K5 dlopen).

## Summary

The spikes in this phase can be planned as concrete, reproducible procedures. This research read the pinned crate sources in `~/.cargo/registry/src/` and ran four cheap local probes: a static musl link, a buildx tar export, a `claude auth status` check, and two `claude -p` calls. One finding changes the phase, and the planner must handle it before any boot task.

**Blocking finding: the stock libkrunfw kernel cannot boot an initramfs.** `config-libkrunfw_aarch64` at tag `v5.6.2`, the version the `libkrun/krun` Homebrew formula installs, contains `# CONFIG_BLK_DEV_INITRD is not set`. Linux 6.12 builds `init/initramfs.o` only when `CONFIG_BLK_DEV_INITRD=y`, and otherwise builds `noinitramfs.o`, so the kernel never unpacks an external initramfs. msb_krun will place the image in guest memory (`KernelBuilder::initramfs_path`), but the kernel ignores it. The kernel then mounts `root=/dev/root rootfstype=virtiofs`, which msb_krun always appends, finds no virtio-fs device, panics, and reboots. That reboot turns into **host exit code 0**. SPIKE-01 as worded ("boots libkrunfw from an initramfs whose `/init` is a static musl binary ... and no virtiofs") is therefore not achievable with stock libkrunfw. This is the SPIKE-08 stop-and-ask case. Three paths preserve `forbid(unsafe_code)`. The recommended one, Option A, boots straight from the raw read-only SquashFS root disk with `root=/dev/vda`, with `/init` written into the root image by our own packer. It uses no initramfs and no virtio-fs. See Open Question 1.

The rest is well mapped. msb_krun's safe builder API covers everything the launcher needs. TSI is off by default, and the vsock device attaches only when a route exists. `unix_connect(port, path)` makes the VMM connect to a host Unix socket that the host must already be listening on. Disks default to `Raw`. `on_exit` and `Vm::exit_code()` exist. K7 has a sharp answer from source: without virtio-fs the guest has **no** channel to set the process exit code. Guest power-off, guest reboot, and a kernel panic all exit 0. The guest's result must therefore travel over vsock, and the host's deadline and status frame decide the outcome. A launcher watchdog can set a distinct code through `Vm::exit_code()` and then `ExitHandle::trigger()`. The `claude -p` flag set already works under subscription login with an allowlisted environment: `structured_output` is returned through an internal `StructuredOutput` tool, `--tools ""` leaves only that tool, and `num_turns` is 2 under `--max-turns 1`. Docker 29.6.1 is running, and `docker buildx build --output type=tar` works with the default driver.

**Primary recommendation:** Plan four waves. Wave 1 builds the toolchain, the guest and launcher skeletons, the packers, and runs SPIKE-07. Wave 2 does signing and the boot, with a decision checkpoint on the boot path. Wave 3 runs egress, vsock, Seatbelt, and the root-image proof. Wave 4 writes the CLAUDE.md amendments, finishes the evidence file, runs the gate, and runs `cargo clean`. Ask the user now, before execution if possible, to choose between Option A (block root, recommended), Option B (single-file in-process virtio-fs root), and Option C (rebuild libkrunfw with `CONFIG_BLK_DEV_INITRD=y`).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Spike code home and afterlife
- **D-01:** Spike code lives in the real crates, not in a `spikes/` member and not outside the workspace. `minos-vmm` gets its first real `main`: it reads a launch config (D-04) and drives msb_krun. `minos-guest` gets its first real `/init`. The SquashFS packer (backhand, gzip only) and the hand-rolled `newc` initramfs writer are written in Rust now, in the crate where Phase 5 places `env build` (the planner confirms the crate against `.planning/research/ARCHITECTURE.md` and the deny.toml layering; it is never `minos-core` or `minos-loop`). Everything passes the gate; no new workspace member, no deny.toml exception.
- **D-02:** Probes are kept, not deleted. Each spike that needs a VM becomes an `#[ignore]`d integration test (it needs libkrunfw and a signed helper on the host). Phase 5 promotes them into the permanent exit tests: egress fails closed (KRUN-03), no host environment in the guest (GUEST-05), no VM after `kill -9` (KRUN-05). The spike result must therefore be reproducible by running the test again, not only recorded.
- **D-03:** The `aarch64-unknown-linux-musl` target is added to `rust-toolchain.toml` in this phase, pulling Phase 1's D-09 forward. The reason, confirmed with the user: the libkrun guest is always Linux (libkrunfw is Linux 6.12.91), and `/init` must be a fully static binary because the initramfs has no libc. CLAUDE.md Commands moves the musl clippy line from "Phase 5 onward" to Phase 2 onward, and the host gate keeps building `minos-guest` for the host target so the forbid backstop covers the non-Linux parts (keep the guest free of `#[cfg(target_os = "linux")]` on anything the host can compile). `cargo test --workspace` still cannot run the guest's tests on macOS; that waits for a Linux runner.
- **D-04:** `minos-vmm` receives its launch settings as a minimal versioned JSON config (a `v` tag, `deny_unknown_fields`, `try_from` validation), read from stdin or a file path given on argv. Fields are the ones the real launcher needs: kernel (libkrunfw path and expected hash), disks (shared read-only root, per-job read-only input, size-capped scratch, all raw), vsock port and host Unix socket path, hard maximum duration, console output cap. No hard-coded paths and no ad-hoc flag soup. Phase 5 keeps the shape and adds the golden file.
- **D-05:** K7 is folded into SPIKE-01: the spike records how a guest exit and a VMM error each surface to the parent (exit code, msb_krun `ExitHandle` or `on_exit` behavior), since Phase 5 needs to separate `Outcome::Crashed` from an infrastructure error. K8 (launches per second, memory per VM) is deferred with NEXT-04.

#### Evidence and the CLAUDE.md rewrite
- **D-06:** Raw evidence goes in `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`, following `01-GATE-EVIDENCE.md`: for every spike, the verbatim command, exit code, and output (truncated only where noted, never summarized). CLAUDE.md gets the conclusion and the recipe, not the logs.
- **D-07:** CLAUDE.md is amended in place. No `docs/` split in this phase; Phase 8 writes SECURITY.md and README and may split then. The DOCS-05 amendments to make: the local-first build order stated up front; the `ModelProvider` decision and the `claude` CLI provider as the personal default; no tokio in any `minos-*` crate in v1; pico-dollar rates (verify the Cost section already says pico, and fix any remaining "nano"); the `libkrun/krun` tap; libkrun's networking default (implicit TSI) and the msb_krun counter-default; the `minos-loop` crate in the Workspace table and dependency rules (verify it is already listed; add the loop-never-links-sandbox rule if missing); every spike result in Verified facts with the date; the codesign line and the musl clippy line in Commands; the P0 open questions answered and removed from Open questions.
- **D-08:** The AWS-first content (durable execution rules, Lambda limits, orchestrator, MicroVM facts) stays in CLAUDE.md and is labelled as the later AWS milestone. The AWS open questions move to a clearly marked deferred list. Nothing is trimmed or removed.
- **D-09:** When a spike result contradicts the stack of record, both `CLAUDE.md` and `.claude/CLAUDE.md` are corrected in the same change, as 01-05 did for clap. The research files under `.planning/research/` stay as written, because they are dated inputs; the evidence file notes which research claim was overturned and where.

#### Host changes and signing
- **D-10:** The spike plans may do these on this Mac without asking: `brew tap libkrun/krun`, `brew trust libkrun/krun`, `brew install libkrunfw` (only libkrunfw; not libkrun, krunkit, or krunvm), and the rustup musl target install that `rust-toolchain.toml` triggers. The plans must stop and ask before: creating a dedicated `_minos` user (needs sudo; the SPIKE-05 fallback is recorded as a recipe, not executed), and installing or starting any container engine (starting Docker Desktop if its daemon is down, creating a lima VM, installing podman or Apple's container tool).
- **D-11:** Signing without an xtask, consistent with Phase 1 D-01 and D-07: an entitlements plist at `crates/minos-vmm/minos-vmm.entitlements` and one documented `codesign` argv in CLAUDE.md Commands, run by hand after `cargo build`. No script, no `build.rs`, no cargo alias (aliases only expand to cargo subcommands). Phase 5's `minos host setup` may run the same argv from Rust later. Every `cargo build` relinks and drops the signature, so the documented sequence is build, then sign, then run.
- **D-12:** SPIKE-04 evidence is two verbatim runs: the unsigned helper refused by Hypervisor.framework (the exact error, expected `HV_DENIED`), then the signed helper booting. Both go in the evidence file. The entitlement-versus-signature distinction (re-signing with an empty entitlements file) is not required.
- **D-13:** K5 is resolved by trying the hardened runtime first: `codesign --force -s - --options runtime` with only `com.apple.security.hypervisor`. If `dlopen` of the Homebrew libkrunfw dylib then fails, add `com.apple.security.cs.disable-library-validation`, record the failure that forced it, and keep the smallest set that boots. The final entitlement set and its reason go in CLAUDE.md.

#### Root image engine (SPIKE-06)
- **D-14:** The first tool chain tried is the Docker CLI already on PATH: `docker build` for arm64, then export the filesystem to a rootfs tar (`docker export` of a created container, or `buildx --output type=tar`), then the Rust SquashFS packer (D-01) turns the tar into a read-only raw image. Docker Desktop is one option for the public project, not a requirement; CLAUDE.md states the requirement as "any OCI builder that can export a rootfs tar" and lists lima, podman, and Apple's container tool as alternatives, with whichever of them the spike actually checked marked as such. If the Docker daemon is not running, that is a host change under D-10 and the executor asks first.
- **D-15:** SPIKE-06 is answered only when the produced image boots: a trivial Dockerfile (base image pinned by digest, arm64) becomes a SquashFS image, the SPIKE-01 probe boots with it as the root disk, and a command from that image runs and its output comes back. Producing and inspecting the image is not enough.
- **D-16:** Reproducibility is noted, not required. The spike builds twice and records whether the SquashFS digests match and, if not, why (timestamps, package metadata). Byte-identical builds and `minos.lock` are Phase 5.
- **D-17:** The SquashFS writer and the initramfs writer are Rust in the real crate, not `mksquashfs` or `cpio` from Homebrew. The initramfs contains only `/init` = `minos-guest`, with zeroed metadata and a stable hash.

### Claude's Discretion
- **SPIKE-07 spend and flags.** Use the cheapest model alias, one call per flag combination, and stop as soon as each question has an answer. Test at minimum: `--tools ""` with `--json-schema`, `--disallowedTools`, `--output-format json`, `--no-session-persistence`, `--strict-mcp-config` with an empty MCP config, `--safe-mode`, the permission flag that works with `-p`, and `--max-turns 1`; from a fresh empty temp directory outside any repo; with `env_clear()` plus the `HOME`, `PATH`, `USER`, `LOGNAME`, `TMPDIR`, `LANG` allowlist. Record the exact `structured_output` shape, the result `subtype` values seen, the usage field names, whether `--tools ""` removes the Agent and Skill tools, and whether subscription auth works through the allowlisted environment (Keychain access). Record tokens only; never record `total_cost_usd`. The hostile-fixture test (PROP-03) is Phase 6, not this spike.
- **Seatbelt profile scope (SPIKE-05).** Deny-default; allow only what the VMM needs: reading the root, input, and initramfs images, the libkrunfw dylib, and system libraries; writing the scratch disk and the socket directory; the mach lookups Hypervisor.framework requires; no network. Record the result either way with the profile text in the evidence file. If deny-default breaks `hv_vm_create`, record the smallest set of allowances that restores it, and record the dedicated-user fallback as a recipe (D-10).
- **Spike order within the dependency graph** in the domain section, and how many plans the phase splits into.
- **Launch config field names**, the short socket directory location (macOS Unix socket path limit, PITFALLS Pitfall 30), and the console output cap value.
- **The trivial Dockerfile** used for SPIKE-06 and the command it runs for the boot proof.
- **What the SPIKE-01 guest does to prove the boot:** at least print a marker, read `/proc/cmdline`, and exit with a known code so K7 can be recorded.

### Deferred Ideas (OUT OF SCOPE)
- **K8 benchmark** (launches per second, memory per VM) — with NEXT-04, concurrent candidate VMs.
- **`docs/` split of CLAUDE.md** — Phase 8, alongside SECURITY.md and README.
- **Creating the `_minos` dedicated user** — Phase 5 `minos host setup`; this phase records the recipe only (D-10).
- **Reproducible image builds and `minos.lock`** — Phase 5 `env build` (D-16).
- **Checking lima, podman, and Apple's container tool as engines** — Phase 5 or Phase 8 docs; this phase lists them as untested alternatives unless the spike had reason to try one (D-14).
- **Hostile `.claude/settings.json`, hooks, and `.mcp.json` fixture for the `claude` provider** — Phase 6, PROP-03.
- **Promoting the ignored spike tests into the permanent exit tests** — Phase 5 (D-02).
- **Re-signing with an empty entitlements file to separate signature from entitlement** — not needed; Phase 5 may add it to `host doctor` output if useful.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SPIKE-01 | Safe libkrun wrapper boots libkrunfw from an initramfs whose `/init` is static musl, raw read-only root disk, no virtiofs | **The literal form cannot pass with stock libkrunfw 5.6.2 (`BLK_DEV_INITRD` not set).** The boot recipe, Options A/B/C, and the K7 exit-code map are in Architecture Patterns 1–3 and Open Question 1 |
| SPIKE-02 | Guest `connect()` to host loopback and a public IP both fail with implicit vsock disabled and a plain vsock added | msb_krun mapping: "implicit vsock disabled" means `inet_hijack` stays at its default `false` and there is no `tcp_listen_remap`; "plain vsock" means `.vsock(\|v\| v.unix_connect(port, path))`. Probe design is in Pattern 5 |
| SPIKE-03 | vsock port to host Unix socket carries a byte-capped guest-to-host stream | `unix_connect` semantics read from the muxer source; host binds first; capped reader. See Pattern 5 and Pitfall 6 |
| SPIKE-04 | Ad-hoc signed helper with `com.apple.security.hypervisor` starts a VM; unsigned is refused | "Unsigned" on arm64 means linker-signed ad-hoc with no entitlements (verified). codesign argv and plist are in Code Examples |
| SPIKE-05 | Seatbelt deny-default answer with evidence; dedicated-user fallback recorded | Prior art: BoxLite runs its libkrun shim under `(deny default)` with no hypervisor-specific rule. The profile skeleton and the Unix-socket grant syntax are in Pattern 6 |
| SPIKE-06 | Root image from a user Dockerfile on macOS by a documented tool chain | `docker buildx build --platform linux/arm64 --output type=tar,dest=…` works with the default driver (verified). backhand gzip writer recipe is in Pattern 4 |
| SPIKE-07 | `claude -p` contract under subscription login | Verified in this session: flag set, `structured_output`, usage field names, `tools: ["StructuredOutput"]`, and `num_turns: 2` under `--max-turns 1`. Remaining items are in Pattern 7 |
| SPIKE-08 | If SPIKE-01 fails, stop and ask; no `unsafe` exception | Triggered by the initramfs finding. Options are laid out in Open Question 1 |
| DOCS-05 | CLAUDE.md amended for v1 and each spike result | Current-state audit of CLAUDE.md and the edit list are in "DOCS-05 edit map" |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Every line of spike code, tests and probes included, must satisfy these:

- `unsafe_code = "forbid"`. Clippy `unwrap_used`, `expect_used`, and `indexing_slicing` are `forbid`, so `#[allow]` and `#[expect]` cannot silence them. Tests return `Result` and use `?`. No `#[allow(` anywhere: the C-02 grep `grep -rnE '#!?\[allow\(' crates --include='*.rs'` must print nothing.
- `allow_attributes = "deny"` and `allow_attributes_without_reason = "deny"`. Only `#[expect(lint, reason = "...")]` is allowed, and only for lints that are not forbidden.
- Never weaken a security invariant. No `pub` field, `Deref`, or bypass. If blocked, stop and ask.
- No C in our crates. No OpenSSL or libgit2. No git in the trusted path. Commands are argv arrays and never shell strings.
- Deserialization is validation: `#[serde(try_from = ...)]`, `deny_unknown_fields`, and no `flatten`.
- Errors separate blame. Library crates use `thiserror`. `anyhow` is allowed only in `minos-cli` (enforced by deny.toml).
- Use `BTreeMap`/`BTreeSet` or a sorted `Vec` wherever order affects output or hashing.
- Policy is Cargo-native: no script, no xtask, no `build.rs`, no cargo alias (Phase 1 D-01, D-07, D-11).
- **Never add a `rustflags` key under `[target.<triple>]` or `[target.<cfg>]` in `.cargo/config.toml`.** It silently replaces the `[build] rustflags` forbid backstop (01-GATE-EVIDENCE G-02-CLOSED). A `linker` key alone is safe.
- The gate is `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, and `cargo doc --workspace --no-deps`. From this phase it also includes `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`. Then run C-01 and C-02. Run `cargo clean` at the end of the phase (GATE-06).
- deny.toml layering: `msb_krun` is allowed only under `minos-vmm`, and `tokio` only under `msb-imago`. `minos-backend-krun` has only `minos-cli` as a dependent. `minos-vmm` and `minos-guest` are binaries. `[bans.workspace-dependencies] unused = "deny"`.
- Public repo. Never claim "ironclad". The evidence file is committed, so redact PII (see Pitfall 10).
- Keep this file current. When a spike result changes a decision, edit CLAUDE.md and `.claude/CLAUDE.md` in the same change (D-09).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| VMM, devices, HVF calls | `minos-vmm` helper process (host, entitled) | — | Only binary linking msb_krun. `enter()` takes over and exits the process, so there is one VM per process |
| Launch config validation | `minos-vmm` (parse, `try_from`) | `minos-backend-krun` (produces JSON) | The type lives in the helper. The backend writes the same shape, and a Phase 5 golden file binds them. No shared crate, so the hypervisor code never reaches the CLI binary |
| Root image (SquashFS) + `/init` injection | `minos-backend-krun` (declares `backhand`) | host OCI builder (Docker) | A trusted packer turns an untrusted rootfs tar into a raw read-only image. It never unpacks to the host filesystem |
| newc initramfs writer (D-17) | `minos-backend-krun` | — | Same crate as the packer. Under Option A it has no consumer (see Open Question 1) |
| Input disk (raw header + payload) | `minos-backend-krun` (spike test writes it) | `minos-guest` reads `/dev/vdb` raw | The host never builds a filesystem |
| Guest PID 1 duties, probes, output stream | `minos-guest` (static musl) | — | Linux-only code goes behind `cfg(target_os = "linux")`. The pure parts are host-compilable lib code |
| Deadline, kill, parent-death | parent test or supervisor | `minos-vmm` watchdog thread | The host is authoritative. The helper watchdog is the backstop |
| Seatbelt confinement | `/usr/bin/sandbox-exec` wrapping `minos-vmm` | dedicated `_minos` user (recipe only) | macOS has no seccomp or cgroups |
| `claude -p` contract | spike harness (manual commands this phase) | `minos-propose` (Phase 6) | No provider code this phase; facts only |
| Docs of record | `CLAUDE.md`, `.claude/CLAUDE.md` | `02-SPIKE-EVIDENCE.md` | Conclusions in CLAUDE.md, raw logs in the evidence file (D-06) |

## Standard Stack

Every crate is already pinned in `[workspace.dependencies]`. **This phase installs no new external crate.** The only additions are new `workspace = true` uses inside existing members.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `msb_krun` | =0.1.41 (`default-features=false`, `features=["blk"]`) | VMM builder API in `minos-vmm` | The only safe Rust API over libkrun; source read for this research [VERIFIED: crate source in ~/.cargo/registry] |
| libkrunfw (Homebrew `libkrun/krun`) | 5.6.2, `libkrunfw.5.dylib` | Guest kernel loaded by `dlopen` | Formula URL is `libkrunfw-prebuilt-aarch64.tgz` v5.6.2. Bottles exist only for `arm64_tahoe` and `arm64_sequoia`, so macOS 27 builds from source with `make` (clang, kernel.c) [VERIFIED: raw formula fetched] |
| `rustix` | =1.1.5 (guest, linux only) | `mount`, `reboot(RebootCommand::PowerOff)`, `waitpid`, `setrlimit` | Safe syscalls [VERIFIED: crate source] |
| `vsock` | =0.5.4 (guest, linux only) | `VsockStream::connect_with_cid_port(VMADDR_CID_HOST, port)` | Safe; re-exports `VMADDR_CID_HOST` [VERIFIED: crate source] |
| `backhand` | =0.25.5 (`default-features=false`, `features=["gzip"]`) | SquashFS writer | Pure Rust. gzip uses `flate2` with `zlib-rs` [VERIFIED: crate source] |
| `tar` | =0.4.46 | Read the Docker rootfs tar (hostile input) | Add `tar.workspace = true` to `minos-backend-krun` |
| `sha2` + `base16ct` | =0.11.0 / =1.0.0 | libkrunfw hash check in `minos-vmm` (D-04) | Add to `minos-vmm` |
| `serde` / `serde_json` / `thiserror` | pinned | Launch config, errors | Already declared in `minos-vmm` |

### Supporting (tools, not crates)
| Tool | Version on this Mac | Purpose |
|------|---------------------|---------|
| `rust-lld` | ships in 1.97.1 `rustc` component | Linker for `aarch64-unknown-linux-musl`. **Verified:** produces `ELF 64-bit LSB executable, ARM aarch64, statically linked` from macOS |
| `/usr/bin/codesign` | system | Ad-hoc signing with entitlements |
| `/usr/bin/sandbox-exec` | system (deprecated, present) | Seatbelt confinement |
| Docker CLI + buildx | 29.6.1, buildx v0.35.0-desktop.2, BuildKit v0.31.1, daemon **running** | Rootfs tar export |
| `claude` | 2.1.296 at `~/.local/bin/claude` → `~/.local/share/claude/versions/2.1.296` | SPIKE-07. **The `claude` first on PATH here is a cmux shim**; see Pitfall 9 |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rust-lld` | Homebrew `aarch64-linux-musl-gcc` (present: `filosottile/musl-cross` 0.9.9_2) | A C toolchain from a third-party tap. `rust-lld` ships with the pinned toolchain, so it is reproducible on any machine |
| buildx `--output type=tar` | `docker create` + `docker export` + `docker rm` | `docker export` adds `/.dockerenv` and empty `/etc/hosts`, `/etc/resolv.conf`, `/etc/hostname` mount points, and leaves a container to clean up. buildx tar is cleaner |
| Watchdog that polls the console file size | Custom `ConsolePortBackend` (`ConsoleBuilder::custom`) | The custom backend caps exactly, but whether it becomes the kernel console `hvc0` is unverified. Use the watchdog in the spike and leave the choice to Phase 5 |

**Installation (host changes allowed by D-10):**
```bash
brew tap libkrun/krun && brew trust libkrun/krun && brew install libkrunfw
# rustup musl target: triggered by rust-toolchain.toml `targets`; ALREADY INSTALLED during this research
# (rustup target add aarch64-unknown-linux-musl --toolchain 1.97.1)
```

## Package Legitimacy Audit

No new packages are added this phase. The audit below re-checks the crates the spike code starts to use. All were already pinned and `cargo deny`-gated in Phase 1.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| msb_krun | crates | first publish 2026-03 | 4,790/wk | github.com/zerocore-ai/libkrun | OK | Approved (already pinned =0.1.41) |
| backhand | crates | 2023 | 8,611/wk | github.com/wcampbell0x2a/backhand | OK | Approved (pinned, gzip only) |
| vsock | crates | 2019 | 314k/wk | github.com/rust-vsock/vsock-rs | OK | Approved (pinned) |
| rustix | crates | 2021 | 22M/wk | github.com/bytecodealliance/rustix | OK | Approved (pinned) |
| tar, sha2 | crates | mature | high | composefs/tar-rs, RustCrypto | OK | Approved (pinned) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
 host (macOS, login user)                                     guest (Linux 6.12, libkrunfw)
 ─────────────────────────                                    ─────────────────────────────
 spike test (minos-backend-krun/tests, #[ignore])
   │ 1. pack root image: rootfs.tar (untrusted) + /init (trusted minos-guest) ──► root.sqfs (raw, RO)
   │ 2. write input disk: header{magic,v,len} + spike-job JSON, pad 512 ──────► input.img (raw, RO)
   │ 3. create scratch: File::set_len(cap) (sparse) ──────────────────────────► scratch.img (raw, RW)
   │ 4. UnixListener::bind(<short dir>/o.sock)  (BEFORE spawn)
   │ 5. TcpListener::bind(127.0.0.1:0)  (egress canary; must accept 0 connections)
   │ 6. write vmm.json (v=1) into 0700 job dir
   │ 7. Command(signed minos-vmm | sandbox-exec -f minos-vmm.sb … minos-vmm)
   │      .env_clear().stdin(null).arg(vmm.json)
   ▼
 minos-vmm ── parse+validate config ── sha256(libkrunfw) == expected? ── watchdog thread
   │            (deadline | console cap | parent gone) → exit_code.store(N); ExitHandle::trigger()
   ▼
 VmBuilder ─ kernel(krunfw_path, cmdline, init_path) ─ disk vda/vdb/vdc (Raw) ─ vsock unix_connect(P, o.sock)
   │           console(output = job/console.log) ─ machine(vcpus, mem, balloon off, msb_metrics off)
   ▼ .build()?.enter()   ← never returns on success; Err only before VMM takeover (e.g. HV_DENIED, dlopen fail)
 HVF vCPU ─────────────────────────────────────────────────────► kernel boots, mounts /dev/vda (squashfs, ro)
                                                                  devtmpfs auto-mounted on /dev
                                                                  /init = minos-guest (PID 1)
                                                                    mount /proc; read /dev/vdb job
                                                                    print marker + /proc/cmdline + env (console)
                                                                    egress probes: TCP 1.1.1.1:443, 127.0.0.1:P, host IP
                                                                    vsock: connect(CID 2, port P) ──┐
 host UnixListener.accept() ◄── VMM connects to o.sock ◄──────────────────────────────────────────┘
   read with Read::take(cap+1) → Untrusted bytes → verify
                                                                    reboot(PowerOff) → PSCI SYSTEM_OFF
 minos-vmm _exit(0) ◄─────────────────────────────────────────── VcpuExit::Shutdown → FC_EXIT_CODE_OK
 parent: wait() → exit status + status frame + deadline → classify
```

### Recommended Project Structure (spike additions only)
```
rust-toolchain.toml            # + targets = ["aarch64-unknown-linux-musl"]
.cargo/config.toml             # + [target.aarch64-unknown-linux-musl] linker = "rust-lld"   (NO rustflags key)
crates/minos-vmm/
  minos-vmm.entitlements       # D-11 (hypervisor only; + disable-library-validation only if K5 forces it)
  minos-vmm.sb                 # SPIKE-05 Seatbelt profile, parameterized with (param "...")
  clippy.toml                  # disallowed-methods: TSI, virtio-fs, non-raw formats, guest env
  src/main.rs                  # read config path from argv, run, map errors to documented exit codes
  src/config.rs                # VmmConfig { v, kernel, disks, vsock, max_duration, console_cap } + RawVmmConfig
  src/watchdog.rs              # deadline / console cap / parent-death → exit_code + ExitHandle
crates/minos-guest/
  src/lib.rs                   # host-compilable: job header parse, report formatting, capped writer (unit tests)
  src/main.rs                  # cfg(linux): PID 1 path; non-linux: return an error
  src/linux.rs                 # #[cfg(target_os = "linux")] mount, probes, vsock, reboot
crates/minos-backend-krun/
  src/rootfs.rs                # tar (untrusted) + /init → SquashFS via backhand (gzip)
  src/initramfs.rs             # newc writer (D-17), golden-hash unit test
  src/input_disk.rs            # raw header + payload, 512-padded
  tests/spike_vm.rs            # #[ignore] boot, K7, egress, vsock, rootimg, seatbelt
```

Placement rationale. `backhand` is already declared by `minos-backend-krun`, and ARCHITECTURE.md puts disk builders there. `minos-vmm` must not become a library dependency of `backend-krun`: Cargo dependencies are per package, so that would link the VMM into the CLI graph. The launch config is therefore defined once in `minos-vmm`, the backend serializes the same shape, and Phase 5 adds the golden file [CITED: deny.toml layering block; ARCHITECTURE.md Structure].

### Pattern 1: The boot recipe (Option A, recommended)
**What:** Boot straight from the raw read-only SquashFS root on `/dev/vda`. `/init` is the static musl guest, written into the image by the packer. There is no initramfs and no virtio-fs.
**Why it works:** msb_krun builds the kernel command line as `DEFAULT + " " + user_cmdline + " root=/dev/root init={init}" + krun_env + " -- " + args`, where `DEFAULT = "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf"` on aarch64 [VERIFIED: msb_krun-0.1.41 src/api/vm.rs build_kernel_cmdline; msb_krun_vmm src/vmm_config/kernel_cmdline.rs]. Linux's `root=` handler does `strscpy(saved_root_name, line, …)`, so the **last `root=` wins**. `rootfstype=`, `ro`, and `rw` also take the last occurrence [VERIFIED: linux v6.12 init/do_mounts.c]. `init_path` is spliced in right after `root=/dev/root`, so `init_path("/init root=/dev/vda")` places our `root=` last. `rootfstype=squashfs ro` can go in `cmdline()`, because it only has to follow the defaults.
**Caveat:** this relies on msb_krun's private formatting. It is safe because the version is pinned (`=0.1.41`), and the boot test must assert the exact `/proc/cmdline` shape so a bump cannot change it silently. Phase 5 could upstream a `KernelBuilder::root_block_device`.
```rust
// Source: msb_krun-0.1.41 src/api/builder.rs, src/api/builders.rs (method names verified)
msb_krun::VmBuilder::new()
    .machine(|m| m.vcpus(1).memory_mib(256).balloon(false).msb_metrics(false)) // inet hijack stays false
    .kernel(|k| k.krunfw_path(&cfg.kernel.path)                  // absolute; sha256 checked first
                 .cmdline("rootfstype=squashfs ro")              // later than DEFAULT's rootfstype=virtiofs rw
                 .init_path("/init root=/dev/vda"))              // lands after msb_krun's root=/dev/root
    .disk(|d| d.path(&cfg.root).read_only(true))                 // vda; Raw is the default — never call .format()
    .disk(|d| d.path(&cfg.input).read_only(true))                // vdb
    .disk(|d| d.path(&cfg.scratch).read_only(false))             // vdc
    .vsock(|v| v.unix_connect(cfg.vsock_port, &cfg.socket_path)) // guest → CID 2:port → VMM connects to path
    .console(|c| c.output(&cfg.console_path))                    // input becomes empty; NOT host stdin
    .on_exit(|code| { /* runs before _exit; eprintln a marker for K7 evidence */ })
    .build()?    // Err: config errors
    .enter()?;   // Err only before takeover; Ok is Infallible
```
Note: `DiskBuilder` setters apply to the disk opened by the last `.path()`, so `read_only` must come **after** `path` [VERIFIED: builders.rs finish_current]. Disk names follow attach order: `vda`, `vdb`, `vdc`.

**The read-only root needs its mount points created at pack time.** The packer must make `/dev`, `/proc`, `/sys`, `/tmp`, and `/mnt` (or the scratch mount point), because the guest cannot `mkdir` on SquashFS. `CONFIG_DEVTMPFS_MOUNT=y` mounts devtmpfs on `/dev` after the root mounts, and `/dev` must already exist for that [VERIFIED: libkrunfw v5.6.2 config].

### Pattern 2: The literal SPIKE-01 attempt (initramfs), recorded as negative evidence
Run it once to honor the requirement and D-17. Build the newc initramfs containing only `/init`, then call `.kernel(|k| k.krunfw_path(..).initramfs_path(&initrd))` with no `init_path` override and no virtio-fs. Add `cmdline("ignore_loglevel")` so the panic text reaches the console despite `quiet`. Expected result: the kernel never runs `/init`; the console shows a VFS root-mount failure followed by "Kernel panic"; the host exit code is 0 (because of `panic=-1`). The evidence for the stop-and-ask is the console log, the exit code, and the config line `# CONFIG_BLK_DEV_INITRD is not set` [VERIFIED: libkrunfw v5.6.2 config-libkrunfw_aarch64 line 210; linux v6.12 init/Makefile].

### Pattern 3: K7, how exits surface (from source; the spike confirms each row)
| Scenario | Mechanism (source) | Expected `minos-vmm` exit status |
|---|---|---|
| Guest `reboot(POWER_OFF)` | PSCI `SYSTEM_OFF` → `VcpuExit::Shutdown` → `Stopped` → `exit(FC_EXIT_CODE_OK)` | 0 |
| Guest `/init` exits or panics | "Attempted to kill init" → kernel panic → `panic=-1` → PSCI `SYSTEM_RESET` → `Shutdown` | **0** (indistinguishable) |
| Guest sets an exit code | only through the virtio-fs ioctl (`passthrough.rs` stores into the `exit_code` Arc). **There is no virtio-fs here, so the guest has no way to set it** | n/a |
| Launcher watchdog: `vm.exit_code().store(N)` then `exit_handle.trigger()` | VMM prefers the stored code over the vCPU code (`lib.rs` `process`) | N (choose distinct codes, e.g. deadline 124, console cap 125, parent gone 126) |
| `SIGTERM` to `minos-vmm` | registered handler writes `exit_evt` → graceful | 0 (unless a code was stored) |
| `SIGKILL` from parent | process dies, and the VM with it (in-process HVF) | signal 9 |
| `enter()` returns `Err` before takeover (HV_DENIED, krunfw `dlopen` failure, `build_microvm` error) | `Error::Build(Start/Krunfw(..))` back to our `main` | our own documented code (e.g. 70) plus a typed stderr line |
| vCPU emulation error | `exit(FC_EXIT_CODE_GENERIC_ERROR)` | 1 |
| Panic in a VMM thread (e.g. unknown PSCI fn `panic!`) | under `panic=unwind` that thread dies, and the process may hang | caught by the parent deadline |

[VERIFIED: msb_krun_vmm-0.1.41 src/lib.rs (stop, process), src/macos/vstate.rs (Stopped → FC_EXIT_CODE_OK), msb_krun_hvf src/lib.rs (PSCI handling), src/exit_signal.rs; msb_krun_devices virtio/fs/macos/passthrough.rs]

**Consequence for Phase 5:** the exit code is a hint at best. `Outcome::Crashed` means "no valid status frame on vsock"; `TimedOut` means "the host deadline fired"; an infrastructure error means "our documented pre-takeover code, or a typed stderr line". This **overturns** ARCHITECTURE.md Pattern 5 ("libkrun exit()s the helper with the workload's exit code"), which describes libkrun's `init.krun` plus virtio-fs path. Record that under D-09.

### Pattern 4: SquashFS packer (backhand 0.25.5, gzip only)
```rust
// Source: backhand-0.25.5 src/v4/filesystem/writer.rs, src/v4/compressor.rs
use backhand::{FilesystemWriter, FilesystemCompressor, NodeHeader, compression::Compressor};
let mut fs = FilesystemWriter::default();          // mod_time 0 by default — never call set_current_time()
fs.set_compressor(FilesystemCompressor::new(Compressor::Gzip, None)?); // REQUIRED: default is Xz, not compiled in
fs.set_root_mode(0o755);
// for each validated tar entry, in sorted RelPath order (BTreeMap):
//   dir     → fs.push_dir(path, NodeHeader::new(mode & 0o7777, uid, gid, 0))?
//   file    → fs.push_file(std::io::Cursor::new(bytes), path, NodeHeader::new(mode & 0o7777, uid, gid, 0))?
//   symlink → fs.push_symlink(path, target, header)?
//   hardlink→ copy target bytes (backhand has no hardlink API) or reject; record count
//   device/fifo → reject (buildx tar has none)
// ensure /dev /proc /sys /tmp /mnt dirs exist; then overwrite /init with the trusted guest bytes, mode 0o755
let mut out = std::fs::File::create(&img_path)?;
fs.write(&mut out)?;                                // (SuperBlock, bytes_written)
```
Determinism: `parallel` is off, `mod_time` is 0, and per-node mtimes are forced to 0. Identical input therefore gives identical bytes, and D-16 records the result. Input is hostile: wrap the tar reader in `Read::take(total_cap)`, cap the entry count and per-entry size **before** reading, and normalize each path. Strip a leading `./` and reject `..`, absolute paths, and NUL. Never call `Archive::unpack`. **Inject `/init` after all tar entries** so a Dockerfile cannot supply its own `/init`. Record the `minos-guest` SHA-256.

### Pattern 5: Guest probes for SPIKE-01/02/03 (one guest binary, job read from `/dev/vdb`)
- PID 1 startup: mount `proc` on `/proc` with `rustix::mount::mount("proc", "/proc", "proc", MountFlags::empty(), None)`. `/dev` is already devtmpfs. Read the job from `/dev/vdb` with `std::fs::File` and a fixed header (magic `MINOSJB1`, `u32` version, `u64` length, then JSON), reading at most `len` bytes.
- Boot proof: print the marker `MINOS-GUEST-BOOT-OK`, `/proc/cmdline`, `/proc/sys/kernel/osrelease` (records the real kernel version; see Pitfall 7), `std::env::args()`, and `std::env::vars()` to the console. Expect the kernel default env `HOME=/` and `TERM=linux` plus `KRUN_BOOT_START_NS=<host epoch ns>`. Unknown cmdline words without `=` (for example `no-kvmapf`) arrive as **argv** to init [ASSUMED: kernel unknown-param passing, standard behavior].
- Interfaces: list `/sys/class/net` (mount sysfs) or read `/proc/net/dev`. Expected: only `lo`, and it is down.
- Egress (SPIKE-02): `std::net::TcpStream::connect_timeout` to `1.1.1.1:443`, `127.0.0.1:<host canary port>`, and the host's LAN IP. Record each `io::Error` with its raw OS errno; expect `ENETUNREACH` (101) and no TSI. The host asserts its canary `TcpListener` accepted **0** connections. Also try `VsockStream::connect_with_cid_port(2, <unmapped port>)`, which should be refused or reset.
- vsock (SPIKE-03): `VsockStream::connect_with_cid_port(vsock::VMADDR_CID_HOST, port)`, write a deterministic pattern of N bytes with N > host cap, then `shutdown(Write)`. The host reads `take(cap + 1)`, asserts it read exactly `cap` bytes and that the prefix SHA-256 matches, then drops the connection. The guest records the write error (EPIPE or ECONNRESET). For K4, time a 64 MiB transfer under a large cap.
- Scratch: write a pattern to `/dev/vdc` and read it back in the guest. The host never reads it and deletes it (KRUN-04).
- Finish: `rustix::system::reboot(RebootCommand::PowerOff)`. For the K7 "init dies" row, a job mode simply returns from `main` instead.
- Host-compilable split (D-03): put the header parse, job JSON types (`deny_unknown_fields`), and capped writer in `src/lib.rs` with unit tests that run on macOS. `src/main.rs` calls `linux::run()` under `cfg(target_os = "linux")` and returns an error otherwise. This avoids host `dead_code` warnings under `-D warnings` for code used only on Linux.

### Pattern 6: Seatbelt (SPIKE-05)
Prior art: BoxLite runs its libkrun host process (`boxlite-shim`) on macOS under a `(deny default)` profile. The profile has **no hypervisor-specific rule**, which strongly suggests `hv_vm_create` passes a Seatbelt deny-default [CITED: github.com/boxlite-ai/boxlite src/boxlite/resources/seatbelt/seatbelt_base_policy.sbpl, seatbelt_file_read_policy.sbpl; docs.boxlite.ai/guides/macos-sandbox-debugging]. Start narrower than BoxLite and add allowances only from logged denials:
```scheme
(version 1)
(deny default)
(allow process-exec (literal (param "VMM")))
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library")
                  (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/urandom"))
(allow file-read* (literal (param "VMM")) (literal (param "CONFIG"))
                  (literal (param "KRUNFW")) (literal (param "KRUNFW_REAL"))   ; symlink + resolved target
                  (literal (param "ROOT_IMG")) (literal (param "INPUT_IMG")))
(allow file-read* file-write* (literal (param "SCRATCH_IMG")) (literal (param "CONSOLE_LOG")))
(allow network-outbound (literal (param "SOCK")))   ; AF_UNIX connect to the vsock route path only (BoxLite syntax)
(allow sysctl-read (sysctl-name "hw.pagesize") (sysctl-name "hw.ncpu") (sysctl-name "hw.memsize")
                   (sysctl-name-prefix "hw.optional.arm.") (sysctl-name-prefix "kern.proc.pid."))
; NO (allow network*) otherwise. Add mach-lookup / iokit-open / more sysctls ONLY from logged denials.
```
Run it as `/usr/bin/sandbox-exec -f crates/minos-vmm/minos-vmm.sb -D VMM=<abs> -D CONFIG=<abs> … <abs minos-vmm> <config>`, with every path **canonical** (`/private/tmp`, not `/tmp`). Read denials with `log show --last 2m --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'` or `log stream --predicate 'subsystem == "com.apple.sandbox"' --level error`. `sandbox-exec` returns 65 for an SBPL syntax error and 71 for "Operation not permitted" [CITED: search result citing agent-safehouse.dev]. Record the final profile text and the denial log in the evidence file whatever the outcome.

Dedicated-user fallback (record only, D-10; needs sudo):
```bash
sudo dscl . -create /Users/_minos UniqueID <unused id < 500>
sudo dscl . -create /Users/_minos PrimaryGroupID <matching _minos group id>
sudo dscl . -create /Users/_minos UserShell /usr/bin/false
sudo dscl . -create /Users/_minos NFSHomeDirectory /var/empty
sudo dscl . -create /Users/_minos IsHidden 1
```
[ASSUMED: standard macOS service-account recipe]. Open issue for Phase 5: an unprivileged parent cannot start a process as `_minos` without `sudo` (for example a sudoers `NOPASSWD` rule pinned to the exact helper path) or a LaunchDaemon. Record this as part of the recipe.

### Pattern 7: SPIKE-07 `claude -p` contract
**Verified in this session** (two calls to `haiku`, from an empty dir in the session scratchpad, with `env -i HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin USER LOGNAME TMPDIR LANG`):
```bash
printf '<prompt>' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" \
  TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/bin/claude -p --model haiku --tools "" --output-format json \
  --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' \
  --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode \
  --permission-prompts none --max-turns 1
```
- Exit 0, `"subtype":"success"`, `"is_error":false`, `"structured_output":{"n":7}`, `"result":"{\"n\":7}"`, `"num_turns":2` (**with `--max-turns 1`**), `"stop_reason":"tool_use"`, `"terminal_reason":"completed"`, `"permission_denials":[]` [VERIFIED: local probe].
- Usage field names: `usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens_details.thinking_tokens, server_tool_use, service_tier, cache_creation.{ephemeral_1h_input_tokens, ephemeral_5m_input_tokens}, iterations[]}` and `modelUsage.<model-id>.{inputTokens, outputTokens, cacheReadInputTokens, cacheCreationInputTokens, webSearchRequests, costUSD, contextWindow, maxOutputTokens, thinkingTokens, canonicalModel, provider, costBasis}`. The model resolved to `claude-haiku-5-5`. `costUSD` and `total_cost_usd` are estimates and must never be recorded [VERIFIED: local probe].
- `stream-json --verbose` `system/init`: `tools = ["StructuredOutput"]`, so `--tools ""` removes Agent, Skill, and every built-in tool, and structured output is an internal tool. `mcp_servers = []`, `apiKeySource = "none"` (subscription OAuth), and `permissionMode = "default"`. `skills`, `slash_commands`, and `agents` are still **listed**, but cannot be invoked without the Skill and Agent tools. `--disable-slash-commands` should empty them, which the executor should verify. Three builtin plugins are listed (`cc-plugin-agents-md`, `cc-plugin-telemetry`, `cc-plugin-plugin-authoring`); `--safe-mode` says built-in plugins "work normally" [VERIFIED: local probe; `claude --help` text].
- `claude auth status` under the same environment printed `"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty", "subscriptionType": "max"`. It also prints the account **email and orgId**; redact them (Pitfall 10) [VERIFIED: local probe].
- `--max-turns` does **not** appear in `claude --help` for 2.1.296, but it is accepted [VERIFIED: help text + probe].
- Still for the executor: `--disallowedTools '*'` or `"mcp__*"`; `--disable-slash-commands`; `--setting-sources ""` or `user`; `--restricted` (new: "ignores user, project and local settings files ... add --strict-mcp-config to skip MCP servers too"); `--permission-mode dontAsk` compared with `--permission-prompts none`; `--tools ""` **without** `--json-schema` (expect `tools: []`); a schema the model fails, to see `error_max_structured_output_retries` and its interaction with `--max-turns 1`; and the `subtype` values seen. Each is one cheap call.

### Anti-Patterns to Avoid
- **`FsBuilder::root`, `FsBuilder::path`, `ExecBuilder::env/envs`, `VsockBuilder::inet_hijack`, `MachineBuilder::enable_inet_hijack`, `VsockBuilder::tcp_listen_remap`, `DiskBuilder::format`, `DiskBuilder::layers`.** Ban them in `crates/minos-vmm/clippy.toml` `disallowed-methods`. Also ban `FsBuilder::custom` unless Option B is chosen. Use the definition paths (`msb_krun::api::builders::VsockBuilder::inet_hijack` and so on), and prove each path is valid with a temporary probe, as Phase 1 did with C-03b.
- **Leaving `console.output` unset.** The implicit console then reads host **stdin** and adds `krun-stdin`, `krun-stdout`, and `krun-stderr` ports wired to the helper's fds [VERIFIED: msb_krun_vmm builder.rs autoconfigure_console_ports].
- **Spawning `minos-vmm` without `env_clear()`.** msb_krun reads `MSB_KRUN_KERNEL_CMDLINE` (appended to the guest cmdline), `MSB_KRUN_BOOT_TRACE`, and on Linux `HOSTNAME` [VERIFIED: source grep].
- **Treating the `minos-vmm` exit code as the guest result.** See Pattern 3.
- **Reading the config from stdin and also using stdin as a lifeline.** KRUN-01 wants stdin from `/dev/null`. Take the config as a file path in argv (D-04 allows either). Detect parent death by polling `std::os::unix::process::parent_id()`, which changes when the process is reparented to launchd. That is safe std with no extra fd.
- **Building the guest with plain `cargo build` for the host and copying it.** It must be `--target aarch64-unknown-linux-musl`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| VMM, virtio devices | own FFI to libkrun | `msb_krun` builders | `forbid(unsafe_code)` |
| SquashFS writer | own squashfs format | `backhand` (gzip) | superblock, id table, fragments, compression |
| Static Linux linking from macOS | a C cross toolchain | `rust-lld` via `[target.aarch64-unknown-linux-musl] linker = "rust-lld"` | ships with rustc 1.97.1; verified |
| vsock sockets in the guest | raw `socket(AF_VSOCK)` | `vsock` crate | safe API |
| Mount, reboot, waitpid | libc calls | `rustix` | safe |
| Rootfs from a Dockerfile | an OCI layer flattener | `docker buildx build --output type=tar` | BuildKit flattens layers, so there are no whiteouts to handle |
| Parent-death detection | kqueue `EVFILT_PROC` | `std::os::unix::process::parent_id()` polling | no unsafe, no new dependency |
| newc initramfs | (hand-roll is the decision, D-17) | about 60 lines: `"070701"` + 13 eight-digit uppercase-or-lowercase hex fields, name + NUL padded to 4, data padded to 4, `TRAILER!!!` | [VERIFIED: linux v6.12 Documentation/driver-api/early-userspace/buffer-format.rst] |

**Key insight:** the hard parts (VMM, filesystem format, linking) already exist as safe crates and tools. What the phase must build is the **glue invariants**: env_clear, raw-only disks, no TSI, a capped reader, `/init` injection, and canonical paths. Each needs a test.

## Common Pitfalls

### Pitfall 1: Expecting the initramfs to boot
**What goes wrong:** the VM "starts", prints a root-mount panic, and the helper exits 0. It looks like success to anyone checking only the exit code.
**Why:** `CONFIG_BLK_DEV_INITRD` is not set in libkrunfw 5.6.2.
**Avoid:** the boot test requires the `MINOS-GUEST-BOOT-OK` marker **and** a valid vsock status frame, never exit 0 alone.
**Warning sign:** `VFS: Unable to mount root fs` or `Kernel panic` in console.log.

### Pitfall 2: `HV_DENIED` mistaken for a libkrun bug (SPIKE-04)
**What:** every `cargo build` relinks `minos-vmm` as `flags=0x20002(adhoc,linker-signed)` with **no entitlements** [VERIFIED: codesign -dvv on a cargo-built binary]. On arm64 a binary with no signature at all cannot run. "Unsigned" in SPIKE-04 therefore means linker-signed with no entitlement.
**Avoid:** the ignored VM tests check `codesign -d --entitlements - <path>` first, and fail with the documented sign argv if `com.apple.security.hypervisor` is absent. Cargo does not relink a binary after `codesign` rewrites it, because the output is newer than its inputs. Any source change does relink it and drops the entitlement.

### Pitfall 3: backhand defaults to XZ
**What:** `FilesystemWriter::default()` uses `Compressor::Xz`, which is not compiled in with the gzip-only feature set, so `write` fails.
**Avoid:** always call `set_compressor(FilesystemCompressor::new(Compressor::Gzip, None)?)`. Never call `set_current_time()`, because it reads `SystemTime::now()` [VERIFIED: backhand source].

### Pitfall 4: Seatbelt paths and Unix sockets
**What:** a deny-default profile blocks `connect()` to the vsock route socket, because AF_UNIX connect is a `network-outbound` operation, and `/tmp` versus `/private/tmp` mismatches silently deny access.
**Avoid:** use `(allow network-outbound (literal "<canonical sock path>"))` and canonicalize every `-D` parameter. Allow both the libkrunfw symlink (`/opt/homebrew/lib/libkrunfw.5.dylib`) and its Cellar target.

### Pitfall 5: Unix socket path length (macOS `sun_path` = 104 bytes)
**Avoid:** use a short 0700 directory such as `/private/tmp/mnsXXXXXXXX/o.sock` in the spike, or `~/.minos/run/<8hex>/o.sock`, and assert `len < 104` before binding (Pitfall 30 in research).

### Pitfall 6: The VMM connects to the host socket only when the guest connects
**What:** `unix_connect` routes a guest connect on (CID 2, port) to `connect(path)` from the VMM. If nothing is listening, the guest gets a reset (`push_packet(Reset)`) [VERIFIED: msb_krun_devices vsock/muxer.rs].
**Avoid:** the host binds the `UnixListener` before spawning and accepts with a deadline. Never configure `unix_listen` for the output port; connecting to a listen-mode port resets.

### Pitfall 7: Kernel version drift
**What:** the libkrunfw v5.6.2 Makefile says `KERNEL_VERSION = linux-6.12.109`, but the committed config header says 6.12.91 [VERIFIED: both files at tag v5.6.2].
**Avoid:** record `uname -r` (from `/proc/sys/kernel/osrelease`) from the guest in the evidence file, and correct CLAUDE.md and `.claude/CLAUDE.md`, which both say 6.12.91 (D-09).

### Pitfall 8: Console flood
**What:** `console.output(path)` is a plain `File::create` with no cap. A guest can fill the host disk.
**Avoid:** the `minos-vmm` watchdog polls the file length (for example every 50 ms) against `console_cap`, then stores code 125 and calls `trigger()`. Record the overshoot observed in a flood test. Phase 5 decides whether to replace this with a custom `ConsolePortBackend`.

### Pitfall 9: Hijacked `claude` on PATH
**What:** in this terminal, `command -v claude` resolves to a cmux shim that `exec`s `/Applications/cmux.app/.../cmux-claude-wrapper`. Inside cmux that wrapper **injects `--settings` (hooks) and `--session-id`** [VERIFIED: wrapper source header]. Any user may have similar wrappers.
**Avoid:** exec an absolute, canonicalized path (`~/.local/bin/claude` resolves to `~/.local/share/claude/versions/2.1.296`). Use a fixed minimal `PATH=/usr/bin:/bin:/usr/sbin:/sbin` rather than the inherited one, and record the resolved path and version. Subscription auth works with this minimal PATH (verified).

### Pitfall 10: PII and host paths in a public evidence file
**What:** `claude auth status` prints the email and orgId, and `system/init` prints the cwd. Console logs contain `KRUN_BOOT_START_NS`.
**Avoid:** redact email and orgId as `<redacted>` and state the redaction in the evidence entry. That is the one allowed "truncated only where noted" exception to D-06.

### Pitfall 11: Host `dead_code` in the guest under `-D warnings`
**What:** helpers called only from `cfg(target_os = "linux")` code are dead on macOS, so `cargo clippy --workspace` fails.
**Avoid:** split into lib and bin. `pub` lib items are not dead. Linux-only code stays in `#[cfg(target_os = "linux")] mod linux;`.

### Pitfall 12: The `.cargo/config.toml` target table
**What:** adding a `[target.aarch64-unknown-linux-musl]` table is safe **only** without a `rustflags` key.
**Avoid:** add only `linker = "rust-lld"` plus a comment pointing to G-02-CLOSED. Prove it with a probe: temporarily add `.unwrap()` in `crates/minos-guest/src/linux.rs`, then run `cargo build -p minos-guest --target aarch64-unknown-linux-musl`. It must fail with the forbid error. Record it as G-02-style evidence.

## Code Examples

### Entitlements plist (D-11, D-13)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.hypervisor</key><true/>
</dict></plist>
```

### Build, sign, verify (CLAUDE.md Commands candidate)
```bash
cargo build -p minos-vmm
codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
codesign --display --entitlements - target/debug/minos-vmm      # must show com.apple.security.hypervisor
cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings
```
The musl clippy line was run during this research against the current stub and passed with the backstop active [VERIFIED: local run, CARGO_TARGET_DIR in scratchpad].

### Toolchain and cargo config
```toml
# rust-toolchain.toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy"]
targets = ["aarch64-unknown-linux-musl"]   # D-03: the guest is always Linux; /init must be static
profile = "minimal"

# .cargo/config.toml (append; keep [build] rustflags untouched)
# linker only. NEVER a rustflags key here: it would replace the [build] rustflags forbid backstop (G-02-CLOSED).
[target.aarch64-unknown-linux-musl]
linker = "rust-lld"
```

### Launch config shape (D-04; names at the planner's discretion)
```rust
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVmmConfig {
    v: u32,                         // must be 1
    kernel: RawKernel,              // { path: absolute, sha256: 64 lowercase hex }
    root: PathBuf, input: PathBuf, scratch: PathBuf,   // absolute, canonical, regular files, raw
    vsock_port: u32,                // 1..u32::MAX-1 (msb_krun rejects 0 and MAX)
    socket_path: PathBuf,           // absolute, < 104 bytes
    console_path: PathBuf, console_cap_bytes: u64,
    max_duration_ms: u64,           // clamped to a const ceiling
    vcpus: u8, memory_mib: u32,
}
#[derive(serde::Deserialize)]
#[serde(try_from = "RawVmmConfig")]
pub struct VmmConfig { /* private validated fields */ }
```

### Watchdog (safe, std only)
```rust
let exit_handle = vm.exit_handle();     // must be taken BEFORE enter()
let exit_code = vm.exit_code();         // Arc<AtomicI32>; i32::MAX means "unset"
let parent = std::os::unix::process::parent_id();
std::thread::spawn(move || loop {
    std::thread::sleep(Duration::from_millis(50));
    let reason = if start.elapsed() >= max { Some(124) }
        else if console_len(&console_path) > cap { Some(125) }
        else if std::os::unix::process::parent_id() != parent { Some(126) }
        else { None };
    if let Some(code) = reason { exit_code.store(code, Ordering::SeqCst); exit_handle.trigger(); return; }
});
vm.enter()?;
```

## Runtime State Inventory

Not a rename or refactor phase. Host state that the phase does change, recorded so verification can account for it:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None. No ledger, database, or blob store exists yet | none |
| Live service config | Docker Desktop build cache gains spike layers. One scratch buildx build was run during research | none (optionally `docker buildx prune` after the phase) |
| OS-registered state | Homebrew tap `libkrun/krun`, trusted, with `libkrunfw` installed (D-10). No `_minos` user (recipe only) | record in evidence |
| Secrets/env vars | None created. The `claude` login is read, never copied | none |
| Build artifacts | rustup target `aarch64-unknown-linux-musl` for 1.97.1 (**already installed by this research**). Signed `target/debug/minos-vmm`, removed by `cargo clean` | `cargo clean` (GATE-06) |

## DOCS-05 edit map (current CLAUDE.md audited this session)

| Item | Current state in `CLAUDE.md` | Edit |
|---|---|---|
| Local-first build order | Build order is AWS-first (durable function, then MicroVM, ...) | State the v1 local order up front and move the AWS order to "Later milestone (AWS)" (D-08) |
| `ModelProvider` + `claude` CLI provider | "The model stays outside the sandbox" says Bedrock, "no API key anywhere" | Add the local `ClaudeCli` provider as the personal default, with the SPIKE-07 flag set and env allowlist |
| No tokio in v1 | Not stated in root CLAUDE.md (only in `.claude/CLAUDE.md`) | Add to Code rules |
| Pico-dollar money | **Already "pico-dollar precision (10^-12 USD)"**. Verify that no "nano" remains | grep `nano` |
| `libkrun/krun` tap | absent | Add to Local backends / Verified facts |
| libkrun networking default | absent | Upstream libkrun turns on TSI when there is no NIC; msb_krun defaults `inet_hijack=false`. Add the SPIKE-02 result |
| `minos-loop` crate + loop-never-links-sandbox | **Already present** in Workspace and dependency rules | verify only |
| Boot path | "Files go in as disk images: a shared read-only root ..." | Add the chosen boot option (OQ1), `/init` injection, the K7 exit-code table, and `env_clear` |
| Commands | musl clippy line marked "Phase 5 onward"; no codesign line | Move the musl line to Phase 2 onward, add the codesign and musl build lines, and add the "why a Linux target" sentence (CONTEXT specifics) |
| Open questions | AWS-only list | Move it to a "Deferred (AWS milestone)" list. P0 local questions get answered and removed. Add any new open items (Seatbelt residue, console cap mechanism) |
| Verified facts | "as of 2026-10-08", AWS only | Add a dated local section: libkrunfw config facts, msb_krun facts, spike results, claude 2.1.296 contract |
| `.claude/CLAUDE.md` (D-09) | says "initramfs `/init`", "Linux 6.12.91", K2 "kernel ignores root=" | Correct the initramfs claim, the kernel version, and the K1–K7 rows, and drop "add an `xtask`" (superseded by D-11) |
| REQUIREMENTS wording | SPIKE-01 "from an initramfs"; KRUN-09 "root image and an initramfs"; KRUN-08 "An `xtask` signs" | Flag to the user (not this researcher's call). The planner raises it at the OQ1 checkpoint |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| libkrun C API `krun_disable_implicit_vsock` + `krun_add_vsock(ctx,0)` | msb_krun: the vsock device exists only if routes or TSI exist; TSI defaults off | msb_krun 0.1.x | SPIKE-02 needs no special call, but must **prove** it |
| libkrun `init.krun` sets the exit code through a virtio-fs ioctl | no virtio-fs, so no guest exit code; the vsock status frame is authoritative | this design | Phase 5 outcome classification |
| `docker export` of a container | `docker buildx build --output type=tar` | BuildKit | cleaner rootfs, no container cleanup |
| clap derive, xtask signing | builder API; plist + documented argv | Phase 1 UAT, D-11 | KRUN-08 wording is stale |

**Deprecated/outdated:** `sandbox-exec` is marked deprecated by Apple but ships and works on macOS 27 [ASSUMED for "works": the spike proves it].

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `init_path("/init root=/dev/vda")` reaches the kernel verbatim; msb_krun does not reject spaces in `init_path` | Pattern 1 | Option A needs another injection point (an `exec` env `root` key also lands after `root=/dev/root`), or Option B or C |
| A2 | `hv_vm_create` works under Seatbelt deny-default with no special rule (inferred from BoxLite) | Pattern 6 | SPIKE-05 answer becomes "no"; record the minimal allowances or the dedicated-user fallback |
| A3 | A hardened-runtime, ad-hoc signed `minos-vmm` can `dlopen` the locally built Homebrew libkrunfw without disable-library-validation | Code Examples, D-13 | Add `com.apple.security.cs.disable-library-validation` and record why (D-13 already plans this) |
| A4 | Unknown kernel params without `=` become init argv; with `=`, init env | Pattern 5 | Only affects GUEST-05 expectations; the spike records the actual values |
| A5 | No guest-triggerable VMM thread panic is reachable from guest **userspace** (PSCI needs EL1) | Pattern 3 | A hang until the parent deadline; consider `panic = "abort"` for the helper in Phase 5 |
| A6 | The dedicated `_minos` dscl recipe is complete | Pattern 6 | Recipe only, not executed |
| A7 | macOS 27 has no libkrunfw bottle, so Homebrew builds it from source with Xcode clang (present) | Environment | Install takes longer; the executor records what happened |

## Open Questions (RESOLVED)

1. **BLOCKING: which boot path replaces "initramfs" (SPIKE-01 / SPIKE-08)?** RESOLVED by 02-CONTEXT.md D-18 (2026-10-09): the user chose Option A, the root-disk boot (raw read-only SquashFS root on `vda`, `/init` written in by the trusted packer, `root=/dev/vda` placed last through `init_path`, the exact `/proc/cmdline` asserted). Options B and C are rejected. The newc writer keeps its golden test and is used once, by 02-06, for the literal initramfs attempt as negative evidence. SPIKE-08 still applies if the root-disk boot fails (02-06 Task 3).
   - What we know: stock libkrunfw 5.6.2 has `BLK_DEV_INITRD` unset (HIGH). msb_krun always appends `root=/dev/root init=…` and uses `rootfstype=virtiofs` by default.
   - Option A (recommended): raw read-only SquashFS root on `vda` via the `root=` tail (Pattern 1); `/init` injected by the trusted packer. No initramfs, no virtio-fs, no unsafe. Cost: relies on pinned cmdline formatting, guarded by a `/proc/cmdline` assertion. The newc writer (D-17) has no consumer; keep it with its golden test, or let the user drop it.
   - Option B: an in-process single-file virtio-fs root (`FsBuilder::custom` with a safe `DynFileSystem` serving only `/init` from an open `File`; `stat64` is constructible as a literal of public fields). Then `/init` mounts `vda`. Matches upstream's `krun_set_root_disk_remount`. Cost: puts the FUSE server in the guest-facing surface and breaks the literal "no virtiofs" wording. It would also give a guest exit-code channel through `ioctl`.
   - Option C: rebuild libkrunfw with `CONFIG_BLK_DEV_INITRD=y`. libkrunfw's macOS build uses `build_on_krunvm.sh` (krunvm plus libkrun) or a Linux build of `kernel.c`. Cost: new supply-chain step and host tooling, and it diverges from Homebrew.
   - Recommendation: the orchestrator asks the user before execution. Otherwise the plan runs Pattern 2 as negative evidence, then a `checkpoint:decision`. SPIKE-02/03/05/06 boot tasks wait on the answer; SPIKE-07 and image production continue (CONTEXT domain).

2. **Console cap mechanism for KRUN-01 ("capped console pipe").** RESOLVED (deferred to Phase 5): the spikes use the watchdog-plus-file cap and record its overshoot (02-07 K7-04); whether a custom console backend can serve as `hvc0` is a Phase 5 decision, listed in CLAUDE.md Open questions by 02-11 (E-8). The spike uses the watchdog plus file approach (Pitfall 8). Whether a `ConsoleBuilder::custom` backend becomes `hvc0` is unverified and is a Phase 5 decision. Record what the spike observed.

3. **Is `rng(true)` wanted?** RESOLVED: keep the default (on) for the spikes; 02-06 records the guest's virtio device list from the S01-ROOTDISK frame (Task 2 step 6), and Phase 5 decides whether to turn it off. It is a virtio-rng device reading host entropy. Leaving it on avoids early-boot entropy stalls. Turning it off removes one device. Recommendation: keep the default (on) for the spike and record the device list (`/sys/bus/virtio/devices`) in the evidence. Note that msb_krun always attaches its `virtio-msb-vmgenid` generation device.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| macOS arm64 + HVF | all VM spikes | ✓ | macOS 27.0 (26A428), arm64 | — |
| Homebrew | libkrunfw | ✓ | 7.0.6; `libkrun/krun` **not yet tapped** | — |
| libkrunfw | SPIKE-01..06 | ✗ (install allowed, D-10) | 5.6.2 via tap; source build on macOS 27 | none; blocks VM spikes |
| Xcode / clang | libkrunfw source build | ✓ | /Applications/Xcode.app | — |
| Rust 1.97.1 + clippy | all | ✓ | active toolchain | — |
| `aarch64-unknown-linux-musl` std | guest build | ✓ (installed during research) | 1.97.1 | — |
| `rust-lld` | musl link | ✓ | in rustc component (verified link) | Homebrew `aarch64-linux-musl-gcc` (present) |
| `codesign` | SPIKE-04 | ✓ | /usr/bin | — |
| `sandbox-exec` | SPIKE-05 | ✓ | /usr/bin (deprecated) | dedicated user (recipe) |
| Docker daemon + buildx | SPIKE-06 | ✓ running | 29.6.1 / buildx 0.35.0 / BuildKit 0.31.1 | lima 2.0.3 installed (instances stopped; starting one needs asking, D-10) |
| podman / orbstack / Apple `container` | SPIKE-06 alternatives | ✗ | buildx lists orbstack and podman builders in **error** state | list as untested (D-14) |
| `claude` CLI | SPIKE-07 | ✓ | 2.1.296; subscription `max`, `authMethod: claude.ai` | — |
| cargo-deny | gate | ✓ (Phase 1) | 0.20.2 | — |

**Missing dependencies with no fallback:** libkrunfw (installation pre-approved by D-10).
**Missing dependencies with fallback:** none blocking.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`), `#[ignore]`d host-dependent integration tests |
| Config file | none beyond `Cargo.toml` / `rust-toolchain.toml` / `.cargo/config.toml` |
| Quick run command | `cargo test -p minos-backend-krun -p minos-vmm -p minos-guest` (unit tests: config validation, packer, newc, input disk, guest lib) |
| Full suite command | the gate: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check && cargo doc --workspace --no-deps && cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`, then C-01 and C-02 |
| VM spike command | `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl && cargo build -p minos-vmm && codesign … && MINOS_VMM=… MINOS_GUEST=… MINOS_KRUNFW=… cargo test -p minos-backend-krun --test spike_vm -- --ignored --nocapture --test-threads=1` |

The ignored tests take their host paths from environment variables and fail with the exact setup argv if a variable is unset or the helper lacks the entitlement. Never skip silently.

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SPIKE-01 | initramfs attempt fails as predicted (negative evidence) | integration, ignored | `… --test spike_vm boot_initramfs_attempt -- --ignored` | ❌ Wave 0 |
| SPIKE-01 | chosen boot path: marker + `/proc/cmdline` shape + `osrelease` + status frame | integration, ignored | `… --test spike_vm boot_marker -- --ignored` | ❌ Wave 0 |
| SPIKE-01 (K7) | exit status per scenario (poweroff, init exit, watchdog 124/125/126, SIGKILL, bad krunfw path) | integration, ignored | `… --test spike_vm exit_codes -- --ignored` | ❌ Wave 0 |
| SPIKE-02 | TCP to 1.1.1.1:443, 127.0.0.1 canary, host IP all fail; canary accepts 0; only `lo` | integration, ignored (→ KRUN-03) | `… --test spike_vm egress_fails_closed -- --ignored` | ❌ Wave 0 |
| SPIKE-03 | capped stream: host reads exactly cap, prefix hash matches; 64 MiB throughput | integration, ignored | `… --test spike_vm vsock_capped_stream -- --ignored` | ❌ Wave 0 |
| SPIKE-04 | linker-signed helper → `HV_DENIED` error; signed → boots | manual (two verbatim runs, D-12) + entitlement precheck in tests | `target/debug/minos-vmm cfg.json` before and after codesign | manual |
| SPIKE-05 | boot under `sandbox-exec` deny-default profile | integration, ignored | `… --test spike_vm seatbelt_boot -- --ignored` | ❌ Wave 0 |
| SPIKE-06 | Dockerfile → buildx tar → SquashFS → boots → command output returns | integration, ignored + unit (packer) | `… --test spike_vm rootimg_from_dockerfile -- --ignored`; `cargo test -p minos-backend-krun rootfs` | ❌ Wave 0 |
| SPIKE-06 (D-16) | two builds: compare tar and SquashFS SHA-256 | manual + recorded | `shasum -a 256` on both | manual |
| SPIKE-07 | flag matrix results | manual (CLI calls recorded verbatim, PII redacted) | per-call argv in evidence | manual |
| SPIKE-08 | no `unsafe` anywhere; decision recorded | gate + grep | `grep -rn 'unsafe' crates --include='*.rs'` prints nothing; C-02 | ✅ (gate exists) |
| DOCS-05 | CLAUDE.md contains each amendment | grep checklist | `grep -n 'libkrun/krun\|inet_hijack\|rust-lld\|codesign\|ModelProvider\|tokio' CLAUDE.md` | manual review |
| (unit) | VmmConfig rejects unknown fields, bad `v`, relative paths, socket ≥104 bytes, port 0/MAX | unit | `cargo test -p minos-vmm config` | ❌ Wave 0 |
| (unit) | newc writer golden hash; packer deterministic; rejects `..`, absolute paths, devices; `/init` overrides tar | unit | `cargo test -p minos-backend-krun` | ❌ Wave 0 |
| (unit) | clippy `disallowed-methods` paths valid (temporary probe per banned method) | gate probe | `cargo clippy -p minos-vmm` with probe → error; recorded | manual |
| (G-02 check) | musl target table did not drop the backstop | probe | `.unwrap()` probe + `cargo build -p minos-guest --target aarch64-unknown-linux-musl` → forbid error | manual |

### Sampling Rate
- **Per task commit:** the quick run command plus `cargo clippy --workspace --all-targets -- -D warnings`
- **Per wave merge:** the full gate (six commands) plus C-01 and C-02; VM waves also run `spike_vm -- --ignored`
- **Phase gate:** full gate green, every ignored spike test passing or failing as recorded, evidence file complete, then `cargo clean`

### Wave 0 Gaps
- [ ] `crates/minos-backend-krun/tests/spike_vm.rs`: shared setup (env-var paths, entitlement precheck, short 0700 job dir, canary listener)
- [ ] `crates/minos-vmm/src/config.rs` unit tests
- [ ] `crates/minos-backend-krun/src/{rootfs,initramfs,input_disk}.rs` unit tests
- [ ] `crates/minos-guest/src/lib.rs` unit tests (host-runnable parts)
- [ ] `rust-toolchain.toml` `targets` and `.cargo/config.toml` linker (musl std already installed)

## Security Domain

### Applicable ASVS Categories (Level 1)

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (SPIKE-07 only) | never read, store, or forward credentials; `claude` uses its own login; env allowlist strips `ANTHROPIC_*` and `CLAUDE_CODE_USE_*` |
| V3 Session Management | yes (SPIKE-07) | `--no-session-persistence` |
| V4 Access Control | yes | entitlement only on `minos-vmm`; Seatbelt deny-default; 0700 job dirs |
| V5 Input Validation | yes | `VmmConfig` `try_from` + `deny_unknown_fields`; capped tar reader for the rootfs (hostile); capped vsock reader; guest job header length checks |
| V6 Cryptography | yes (integrity) | `sha2` for libkrunfw and guest hashes; never hand-roll |
| V12 Files and Resources | yes | raw-only disks, no format probing, no host directory shares, scratch deleted unread, no `Archive::unpack` |
| V14 Configuration | yes | `env_clear()`, minimal entitlements, hardened runtime, no `RUSTFLAGS` |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Guest egress via TSI | Information disclosure | `inet_hijack` false, `net` feature off, clippy ban, egress test with canary |
| Host env leaks into the guest (`MSB_KRUN_KERNEL_CMDLINE`, `DYLD_*`) | Tampering / disclosure | `env_clear()` on spawn; ban `ExecBuilder::env/envs`; guest dumps env and cmdline as evidence |
| Host stdin or TTY handed to the guest | Tampering | always `console.output(path)`; stdin `/dev/null` |
| Path traversal or device nodes in a Docker rootfs tar | Tampering / elevation | in-memory parse, path normalization, reject devices, `/init` injected last |
| Format confusion (qcow2 referencing host files) | Elevation | `DiskImageFormat::Raw` only; ban `format`/`layers` |
| Output or console flood | DoS | `Read::take` caps, watchdog cap, host deadline |
| libkrunfw swapped between hash and `dlopen` (TOCTOU) | Tampering | hash in `minos-vmm`; Phase 5 may copy into a private 0500 dir and load the copy |
| Helper outlives the parent | DoS | `parent_id()` watchdog, max duration, parent SIGKILL |
| Hooks or settings injected into `claude -p` (shim, cwd) | Elevation | absolute binary path, minimal `PATH`, empty cwd, `--safe-mode`, `--strict-mcp-config`, `--tools ""` |
| PII in the committed evidence | Disclosure | redact email and orgId |

## Sources

### Primary (HIGH confidence)
- `~/.cargo/registry/src/index.crates.io-*/msb_krun-0.1.41/src/{lib.rs, api/builder.rs, api/builders.rs, api/vm.rs, api/exit_handle.rs, backends/fs.rs, backends/console.rs}`: builder surface, cmdline assembly, `enter()`, exit handle, env reads, TSI flag computation
- `msb_krun_vmm-0.1.41/src/{lib.rs, builder.rs, macos/vstate.rs, exit_signal.rs, vmm_config/kernel_cmdline.rs}` and `msb_krun_hvf-0.1.41/src/lib.rs`: exit-code paths, console autoconfig, PSCI
- `msb_krun_devices-0.1.41/src/virtio/{vsock/muxer.rs, fs/dyn_filesystem.rs, console/port_io.rs, fs/macos/passthrough.rs}`
- `backhand-0.25.5/src/v4/{filesystem/writer.rs, compressor.rs}`, `rustix-1.1.5/src/{system.rs, mount/mount_unmount.rs, process/*}`, `vsock-0.5.4/src/lib.rs`
- https://raw.githubusercontent.com/containers/libkrunfw/v5.6.2/config-libkrunfw_aarch64 (and `main`): `# CONFIG_BLK_DEV_INITRD is not set`, `CONFIG_TSI=y`, `DEVTMPFS_MOUNT=y`, `SQUASHFS(_ZLIB)=y`, `VIRTIO_BLK=y`
- https://raw.githubusercontent.com/containers/libkrunfw/v5.6.2/Makefile (`linux-6.12.109`, `libkrunfw.5.dylib`)
- https://raw.githubusercontent.com/libkrun/homebrew-krun/main/Formula/libkrunfw.rb (prebuilt tgz, bottles for tahoe and sequoia only)
- https://raw.githubusercontent.com/torvalds/linux/v6.12/init/{do_mounts.c, Makefile, main.c}; Documentation/driver-api/early-userspace/buffer-format.rst
- Local probes (2026-10-09): musl static link via `rust-lld`; musl clippy of `minos-guest`; `codesign -dvv` of a cargo binary; buildx tar export; `claude auth status`; two `claude -p` calls (json and stream-json)

### Secondary (MEDIUM confidence)
- https://github.com/boxlite-ai/boxlite `src/boxlite/resources/seatbelt/*.sbpl`, `src/boxlite/src/jailer/sandbox/seatbelt.rs`: deny-default profile for a libkrun host process, AF_UNIX `network-outbound (literal ...)` grants
- https://docs.boxlite.ai/guides/macos-sandbox-debugging: denial log predicates
- https://docs.docker.com/build/exporters/local-tar/ and https://docs.docker.com/build/builders/drivers/. The driver table marks "Tarball output" unsupported for the `docker` driver, but the local probe shows `type=tar` works on Docker Desktop's `desktop-linux` builder. Trust the probe for this machine and note the doc discrepancy.
- `.planning/research/{STACK,PITFALLS,ARCHITECTURE,SUMMARY}.md` and `.claude/CLAUDE.md` (dated inputs)

### Tertiary (LOW confidence)
- Web search summaries on SBPL exit codes (65/71); the dscl service-account recipe (training knowledge)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH. Everything is pinned and source-read, and the musl link was verified.
- Architecture: HIGH for API shapes and the initramfs blocker. MEDIUM for Option A at runtime (assumption A1) until the boot runs.
- Pitfalls: HIGH (source-backed). Seatbelt specifics are MEDIUM (prior art, not run).
- SPIKE-07: HIGH for the core contract (run this session). The flag matrix remainder is for the executor.

**Research date:** 2026-10-09
**Valid until:** about 2026-10-23 for `claude` CLI behavior (fast-moving). About 2026-11-08 for the pinned msb_krun and libkrunfw facts, unless the Homebrew formula moves past 5.6.2.
