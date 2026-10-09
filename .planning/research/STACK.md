# Stack Research

**Domain:** A local-only autonomous experiment loop in Rust. Untrusted code runs in libkrun microVMs on macOS arm64 (Hypervisor.framework), with a file-backed event-sourced ledger, content-addressed blobs, sweep and single-shot proposers, and a CLI.
**Researched:** 2026-10-09
**Confidence:** HIGH for crate versions and APIs. MEDIUM for the libkrun integration recipe: it compiles, but no VM has been booted yet.

> **How confidence was assigned.** Every version below comes from the crates.io API on 2026-10-09. The libkrun, msb_krun, diffy, and tar claims come from reading the published crate source, not from READMEs. Four claims were also proven by compiling probes on this machine (macOS 27.0, rustc 1.97.1):
> (a) a crate with `#![forbid(unsafe_code)]` builds against `msb_krun =0.1.41` with features `["blk"]`, and the dependency tree has no `cc` or `-sys` C build;
> (b) a static `aarch64-unknown-linux-musl` binary links with `rust-lld` and no C toolchain;
> (c) `sha2 0.11` finalizes into `[u8; 32]` with `.into()`;
> (d) `diffy 0.5.2` `PatchSet` in `gitdiff` mode detects renames, and `toml 1.1.8` honors `try_from` together with `deny_unknown_fields`.
> The GSD `classify-confidence` seam rates plain web fetches LOW. The ratings here are higher where source was read or code was compiled, and each section says which.

---

## Decisions at a glance

| Question | Answer | Confidence |
|---|---|---|
| 1. Driving libkrun with `unsafe_code = "forbid"` | Use **`msb_krun` 0.1.41**, a safe, native-Rust builder API over the libkrun VMM, inside a **separate per-VM launcher binary** (`minos-vmm`). Do not use `krun-sys` or `libkrun-sys`: both are raw FFI and need `unsafe` at every call site. Do not use krunvm or krunkit as helpers. | MEDIUM (compiles; not yet booted) |
| 1a. Homebrew tap | **`libkrun/krun`**, not `slp/krun`. The repo moved. Run `brew tap libkrun/krun && brew trust libkrun/krun`. It ships libkrun 1.19.6, libkrunfw 5.6.2 (Linux 6.12.91), krunkit 1.3.2, krunvm 0.2.7. Minos only needs **libkrunfw** from it. | HIGH |
| 1b. No network device | Possible, but **libkrun's default is not air-gapped.** Upstream libkrun turns on TSI (guest INET sockets proxied to the host) whenever no virtio-net device exists. msb_krun reverses that default (`inet_hijack` defaults to `false`). Build without msb_krun's `net` feature and ban the hijack methods with clippy. | HIGH (source read) |
| 1c. vsock on macOS | A guest AF_VSOCK port maps to a **host Unix domain socket**. The host binds the listener and the guest connects out to it (`VsockBuilder::unix_connect(port, path)`). There is no host AF_VSOCK on macOS. | HIGH (source read) |
| 1d. Disks | virtio-blk, `DiskImageFormat::Raw`, `.read_only(true)`. Never use qcow2 or vmdk, and never probe a format. libkrun's own header calls format probing a guest escape. | HIGH |
| 2. Diff parse and apply | **`diffy` 0.5.2.** `PatchSet::parse(.., ParseOptions::gitdiff())` detects renames, copies, mode changes, and binary patches so `Diff::parse` can reject them. Write a **strict exact-position applier in `minos-core`** over diffy's parsed hunks rather than using `diffy::apply`, which searches every offset. | HIGH |
| 3. Tar, hashing, encoding | `tar` 0.4.46 (`default-features = false`) with hand-built headers. `sha2` 0.11.0. `base16ct` 1.0.0 for hex (`base64ct` 1.8.3 later for S3 checksums). | HIGH |
| 4. Serde, TOML, JSON | `serde` 1.0.229, `toml` 1.1.8 (TOML 1.1), `serde_json` 1.0.151. The `try_from` plus `deny_unknown_fields` pattern was verified by compiling it. | HIGH |
| 5. Errors and CLI | `thiserror` 2.0.21 in library crates, `anyhow` 1.0.104 only in `minos-cli`'s `main`, `clap` 4.6.7 with `derive`. | HIGH |
| 6. Tests | `proptest` 1.11.0, `insta` 1.49.0 (with `cargo-insta` 1.49.0), `trybuild` 1.0.122. | HIGH |
| 7. Supply chain | `cargo-deny` 0.20.2. Set `[sources] required-git-spec = "rev"`, `unknown-git = "deny"`, and a curated `[bans.build] allow-build-scripts` allowlist. | HIGH |
| 8. Money | Hand-rolled integer fixed-point in core: rates in **pico-dollars** (`u128`), totals in micro-dollars. **CLAUDE.md's "nano-dollar precision" cannot represent published AWS rates** such as `$0.0000276944`/vCPU-s, which has 10 decimal places. | HIGH |
| 9. `claude -p` | Use `std::process::Command` with `env_clear()` plus an **allowlist**, not `env_remove`. Pass the prompt on stdin, run in an empty private cwd, and parse the envelope leniently but `structured_output` strictly. | HIGH for flags and fields. Behavior of the flag combination is unverified. |
| 10. tokio? | **No.** v1 is synchronous: `std::process`, `std::thread::scope`, `std::os::unix::net`, `std::fs::File::lock` (stable since 1.89). Nothing in v1 needs an async runtime. | HIGH |
| 11. Signing | Only `minos-vmm` needs `com.apple.security.hypervisor`. Ad-hoc sign it with `codesign --force -s - --entitlements minos-vmm.entitlements`. The `minos` CLI stays unsigned and has no entitlement. | MEDIUM-HIGH |

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|---|---|---|---|
| Rust toolchain | **1.97.1**, pinned in `rust-toolchain.toml`, edition 2024 | Everything | The durable SDK later needs at least 1.94.1. Pinning keeps `trybuild` stderr snapshots and clippy output stable across machines. Add targets `aarch64-apple-darwin` and `aarch64-unknown-linux-musl`. |
| `msb_krun` | **=0.1.41** (exact pin), `default-features = false`, `features = ["blk"]` | VMM inside the `minos-vmm` launcher | The only maintained crate that exposes libkrun as a **safe Rust builder API** (`VmBuilder`, `DiskBuilder`, `VsockBuilder`, `KernelBuilder`). Our crate keeps `forbid(unsafe_code)`, and all `unsafe` lives in the external VMM crate, as it would with any hypervisor. microsandbox (8.6k stars) pins the same `=0.1.41`. On macOS the tree is pure Rust: it links `Hypervisor.framework` and `dlopen`s libkrunfw. Verified with a compile probe. |
| libkrunfw | **5.6.2** (Homebrew `libkrun/krun`) | Guest Linux kernel (6.12.91) packaged as a dylib | msb_krun loads the kernel from it. Its config has `VIRTIO_BLK`, `VIRTIO_VSOCKETS`, `EXT4_FS`, `SQUASHFS` (zlib and zstd), `EROFS_FS`, `OVERLAY_FS`, `TMPFS`, `SECCOMP`, `CGROUPS`, and `MODULES` off. Pass its **absolute path** via `KernelBuilder::krunfw_path` and record its SHA-256 in provenance. |
| `diffy` | **0.5.2**, `default-features = false`, `features = ["std"]` | Parsing unified and git diffs; generating diffs for reports | Maintained (2026-08), contains no `unsafe`, MSRV 1.85. `PatchSet` exposes `FileOperation::{Create, Delete, Modify, Rename, Copy}`, `old_mode`/`new_mode`, and `is_binary()`, which is exactly what `Diff::parse` must reject. `create_patch` produces the report's cumulative diff. Leave the `binary` feature off; it pulls in `zlib-rs`. |
| `tar` | **0.4.46**, `default-features = false` | Deterministic source tarball, and output streams over vsock | The standard crate. The repo now lives at `composefs/tar-rs`. Turning off default features drops `xattr`. Build headers by hand and **never call `Archive::unpack`**. |
| `sha2` | **0.11.0** | Every ID hash | RustCrypto, pure Rust, and S3 verifies SHA-256 natively. Version 0.11 moved from `GenericArray` to `hybrid-array`. `h.finalize().into()` into `[u8; 32]` was verified by compiling it. |
| `serde` and `serde_json` | **1.0.229** and **1.0.151** | Wire formats, ledger lines, guest protocol, the claude envelope | The standard. JSON is what DynamoDB and S3 will store later, so file-backed rows carry the same `Record` bytes. |
| `toml` | **1.1.8+spec-1.1.0** | `minos.toml`, `prices.toml`, `minos.lock` | toml 1.x implements TOML 1.1. `try_from` plus `deny_unknown_fields` works and reports a line and column for unknown keys. A `try_from` error has no location (`"empty\n"`), so wrap it with the field path. |
| `thiserror` | **2.0.21** | Typed errors in every library crate | Matches the "errors separate blame" rule and `rules/err-thiserror-lib.md`. |
| `clap` | **4.6.7**, `features = ["derive"]` | `minos` CLI | The standard. MSRV 1.85. |

### Guest-side (`minos-guest`: static Linux arm64 binary, PID 1 in the VM)

| Library | Version | Purpose | Notes |
|---|---|---|---|
| `rustix` | **1.1.5**, features `fs`, `mount`, `process`, `system` | Safe wrappers for `mount`, `reboot(PowerOff)`, `setrlimit`, `kill_process_group`, `waitpid` | All are safe functions, so the guest keeps `forbid(unsafe_code)`. As PID 1 it must reap orphans with `waitpid(None, ..)`. |
| `vsock` | **0.5.4** | Guest `VsockStream::connect` to host CID 2 | Safe API over `nix` and `libc`. `rustix` has no `SocketAddrVsock`. |
| `tar` | 0.4.46 | Streams outputs to the host | Same crate as the host side. |
| `minos-core` | workspace | `Diff` strict applier, `Limits`, `RelPath` | The guest and the host's report builder share **one** apply implementation. |

Build: `cargo build --release --target aarch64-unknown-linux-musl` with `[target.aarch64-unknown-linux-musl] linker = "rust-lld"` in `.cargo/config.toml`. This was verified to produce a statically linked ELF on this machine with no C toolchain. (Homebrew's `aarch64-linux-musl-gcc` and `cargo-zigbuild` are installed but not needed.)

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---|---|---|---|
| `anyhow` | 1.0.104 | Error reporting at `minos-cli`'s `main` only | Follows `rules/err-anyhow-app.md`. Never use it in library crates. |
| `base16ct` | 1.0.0 | Lowercase hex for IDs and blob paths | RustCrypto, the same publisher as `sha2`, so there is one trusted org for the hashing path. Add `base64ct` 1.8.3 when the S3 `x-amz-checksum-sha256` header arrives. |
| `ignore` | 0.4.33 | Source packing that respects `.gitignore` and `.minosignore` without git | Use `WalkBuilder` with `.require_git(false)`, `.add_custom_ignore_filename(".minosignore")`, `.follow_links(false)`, `.hidden(false)`, and `.sort_by_file_path(..)`. Then re-sort the collected `RelPath`s with `BTreeSet`, because the walker is the wrong layer to rely on for determinism. |
| `regex` | 1.13.1 | Secret scan before packing | Pure Rust, with linear-time matching, so a hostile file cannot cause catastrophic backtracking. |
| `rand_chacha` and `rand_core` | 0.10.0 and 0.10.1 | Seeded sweep sampling in core | Seed with `ChaCha8Rng::from_seed(sha256("minos/sweep/v1" ‖ run ‖ generation))`. **Do not use `rand`'s distributions or `gen_range`.** They are not value-stable across versions, which would break durable replay and golden vectors. Map to ranges with a hand-written unbiased method and golden-test the output. |
| `backhand` | 0.25.5, `default-features = false`, `features = ["gzip"]` | Pure-Rust SquashFS writer that converts the environment's rootfs tar into a read-only root image | **The defaults pull in `liblzma-sys` and `zstd-sys` (C).** The `gzip` feature uses `flate2` with `miniz_oxide`, which is pure Rust, and the libkrunfw kernel has `SQUASHFS_ZLIB=y`. MSRV 1.90. |
| `cpio` | 0.4.1, or a hand-rolled `newc` writer (about 60 lines) | Deterministic initramfs containing only `/init` = `minos-guest` | Hand-rolled is preferable. The format is trivial, and owning it guarantees zeroed metadata and a stable hash. |
| `tempfile` | 3.27.0 | Atomic blob writes (`NamedTempFile::persist` in the same directory), and test fixtures | Production code uses it in `minos-store`'s file implementation only. |
| `serde_with` | 3.24.0 (optional) | Hex and byte encodings in serde | Only if hand-written `Serialize`/`Deserialize` for IDs becomes repetitive. Each ID still goes through its constructor. |
| `tracing` and `tracing-subscriber` | 0.1.44 and 0.3.23 | Operator diagnostics in the CLI and launcher | **Never log `Untrusted<T>` content.** The type has no `Debug`, which helps. |

### Development Tools

| Tool | Version | Purpose | Notes |
|---|---|---|---|
| `cargo-deny` | 0.20.2 | Licenses, advisories, bans, sources, build-script allowlist | Runs on every build. Config shape is below. |
| `cargo-insta` | 1.49.0 | Reviewing golden wire-format snapshots | Run with `INSTA_UPDATE=no` (or `CI=true`) in the gate so a missing snapshot fails instead of being written. |
| `trybuild` | 1.0.122 (dev-dependency) | Compile-fail tests: `SandboxSpec<Candidate>` cannot take judge data, and `launch` requires `Cleared` | stderr snapshots depend on the rustc version, which is why the toolchain is pinned. |
| `cargo-nextest` | 0.9.148 (optional) | Faster test runs and per-test timeouts | VM integration tests benefit from per-test timeouts. |
| `/usr/bin/codesign` | system | Ad-hoc signing `minos-vmm` with the hypervisor entitlement | Called from an `xtask` (argv array), not from `build.rs`, because build scripts run before linking. |
| `/usr/bin/sandbox-exec` | system (present on macOS 27.0) | Confines the `minos-vmm` process (research flag below) | Deprecated by Apple but shipped and functional. |
| `cargo-lambda` | 1.9.2 (installed: 1.8.6) | Later milestone only | Not used in v1. |

---

## Question 1 in depth: the libkrun integration recipe

### Why a separate launcher process is mandatory, whatever the binding

Both upstream `krun_start_enter` and msb_krun's `Vm::enter()` **never return on success**. When the guest shuts down, the VMM calls `exit()`/`_exit()` and **kills the whole host process**. So the VMM cannot live inside the `minos` CLI process. Each VM needs its own process. This also gives Minos the isolation shape libkrun's own security model asks for. From the libkrun README: *"both the guest and the VMM pertain to the same security context… use the host's OS security features to run the VMM inside an isolated context."*

```
minos (CLI, loop driver, no entitlement, never links msb_krun)
  └─ minos-backend-krun (lib): builds per-job dir, binds UnixListener, spawns + supervises
       └─ spawn ─▶ minos-vmm (bin, ad-hoc signed w/ com.apple.security.hypervisor)
                     links msb_krun; reads a validated VmmConfig from stdin; calls enter()
                     └─ guest: libkrunfw kernel + initramfs(/init = minos-guest)
                              /dev/vda  root image   (raw, read-only, shared per env digest)
                              /dev/vdb  input image  (raw, read-only, per job)
                              /dev/vdc  scratch      (raw, read-write, size-capped, per job)
                              vsock port P ──▶ host Unix socket <jobdir>/out.sock (parent listens)
```

### Binding options compared

| Option | `forbid(unsafe_code)` in our crates? | Status | Verdict |
|---|---|---|---|
| **`msb_krun` 0.1.41** (superradcompany fork of libkrun, Apache-2.0) | **Yes.** The public API is safe builders; the compile probe passed. | Releases about weekly (0.1.41 on 2026-10-08). Used by microsandbox. Pins a forked `msb-vm-memory =0.18.0-msb.2`. | **Recommended.** It is the only route that keeps the invariant without an escape hatch. |
| `krun-sys` 1.10.1 (upstream) | No. Every call is `unsafe extern "C"`. | Stale (2025-02, libkrun 1.10 API). | Reject. |
| `libkrun-sys` 0.10.5 (boxlite), `bux-krun`, `a3s-libkrun-sys` | No. Raw FFI. | Active, third party. | Reject. |
| `libkrun` 1.19.6 crate used as an rlib | No. The functions that take paths are `pub unsafe extern "C" fn`. Only a few, such as `krun_create_ctx` and `krun_add_vsock`, are safe to call. | Upstream, current. | Reject. |
| `krunkit` 1.3.2 or `krunvm` 0.2.7 as the helper binary | Yes, because we would only exec it. | krunkit boots EFI firmware and needs an EFI-bootable disk, and it runs a RESTful status endpoint. krunvm depends on buildah. | Reject. Wrong boot model, extra attack surface, and less control over devices. |
| Our own `-sys` crate with `#![allow(unsafe_code)]` | Only by exempting a crate. CLAUDE.md treats that as an escape hatch. | n/a | **Not without explicit user approval.** If msb_krun fails the boot spike, this is the "stop and ask" point. |

**Upstream direction (LOW confidence on timing).** The upstream `libkrun/libkrun` main-branch header is now `@generated` from a Rust-native v2 API (`VmmBuilder`, `BlockDevice`, `VsockDevice::new(cid, tsi_features)`; issue #634 says v2 creates **no implicit devices**). If upstream publishes that Rust API as a crate, switching from msb_krun is a change contained to `minos-vmm`. Keep the launcher thin so that switch stays cheap.

### Concrete launcher recipe (sketch; verify method names against the pinned source before coding)

```rust
// crates/minos-backend-krun/src/bin/minos-vmm.rs   (crate keeps #![forbid(unsafe_code)])
// Reads ONE VmmConfig (serde, deny_unknown_fields, try_from-validated) from stdin.
// VmmConfig is derived from a LaunchPlan by the trusted backend. It is NOT a LaunchPlan,
// so deserializing it is not a second constructor of LaunchPlan.
msb_krun::VmBuilder::new()
    .machine(|m| m.vcpus(cfg.vcpus).memory_mib(cfg.memory_mib))      // inet hijack left at default false
    .kernel(|k| k.krunfw_path(&cfg.krunfw)                            // absolute path, hash recorded
                 .initramfs_path(&cfg.initramfs)                      // /init = minos-guest
                 .init_path("/init"))
    .disk(|d| d.path(&cfg.root_img).read_only(true))                  // Raw is the default format
    .disk(|d| d.path(&cfg.input_img).read_only(true))
    .disk(|d| d.path(&cfg.scratch_img).read_only(false))
    .vsock(|v| v.unix_connect(cfg.out_port, &cfg.out_sock))           // guest connects out to host
    .console(|c| c.disable_implicit() /* + capped log port, treated as Untrusted */)
    .build()?
    .enter()?;   // never returns on success
```

Hard rules for the launcher:

1. **Build without msb_krun's `net` feature.** Then `VmBuilder::net` does not exist (it is `#[cfg(feature = "net")]`).
2. **Add a crate-level `clippy.toml` in `minos-backend-krun`** that bans, via `disallowed-methods`, `msb_krun::api::builders::VsockBuilder::inet_hijack`, `msb_krun::api::builders::MachineBuilder::enable_inet_hijack`, `msb_krun::api::builders::VsockBuilder::tcp_listen_remap`, `msb_krun::api::builders::FsBuilder::root`, and `msb_krun::api::builders::FsBuilder::path`. The last two are virtio-fs host-directory sharing; libkrun's README warns that it *"does not provide any protection against the guest attempting to access other directories."* Confirm the exact paths against the pinned source. A crate-level `clippy.toml` **replaces** the root one rather than merging with it, so repeat the workspace-wide entries there.
3. **Spawn `minos-vmm` with `env_clear()`.** msb_krun reads `MSB_KRUN_KERNEL_CMDLINE`, which appends to the guest kernel command line, and `HOSTNAME`. Neither must be controllable. Clearing the environment also strips `DYLD_*`.
4. **No virtio-fs at all.** Boot from the initramfs, then mount `/dev/vda` from inside the guest. msb_krun always appends `root=/dev/root init=…`. With an initramfs present the kernel runs `/init` from the initramfs and does not mount `root=`. **Unverified; see the spike list.**
5. **Bind the host Unix listener before spawning.** Create it in a `0700` per-job directory. The parent reads one capped tar stream from it into `Untrusted<Bytes>`. The host never mounts or reads a disk the guest wrote: the scratch image is deleted, unread, when the job ends.
6. **Deadline.** The parent spawns `minos-vmm` with `CommandExt::process_group(0)`, and at budget plus grace calls `rustix::process::kill_process_group` (a safe function). VM teardown is guaranteed because the process that hosts the VMM dies. `Running`'s `Drop` kills and waits. The guest also enforces the budget from inside.
7. **Input disk without a filesystem.** The host writes a raw image containing a small fixed header (magic, `v` tag, lengths, SHA-256s), then the base tarball bytes, then the diff bytes, padded to 512 bytes. The guest reads `/dev/vdb` as a byte stream and verifies the hashes. No filesystem builder runs on the host, and the host never unpacks or applies anything.
8. **Scratch disk.** Created with `File::set_len` (sparse on APFS). The guest formats or uses it. An alternative is to clone a pre-formatted template per job: `std::fs::copy` on macOS uses `fclonefileat`, which is copy-on-write and nearly free.
9. **Root image.** One read-only image per environment digest. SquashFS via `backhand` (gzip) is pure Rust. EROFS (`composefs` 0.9.4 from containers/composefs-rs has a pure-Rust EROFS writer, LOW confidence) is the alternative. The guest mounts it read-only, with tmpfs or scratch as the writable upper layer via overlayfs.

### What remains unverified (Phase "krun backend" must spike these first)

| # | Item | Why it matters | How to check |
|---|---|---|---|
| K1 | A full boot: msb_krun 0.1.41 + libkrunfw 5.6.2 + initramfs `/init` + raw root disk, with no virtiofs device, on macOS 27 | The entire backend depends on it | `brew tap libkrun/krun && brew trust libkrun/krun && brew install libkrunfw`, then sign the probe and boot. |
| K2 | The kernel ignores `root=/dev/root` when an initramfs supplies `/init` | Otherwise a NullFs virtiofs root is needed (upstream `krun_set_root_disk_remount` uses NullFs, and msb_krun's `FsBuilder::custom` could provide one) | The K1 boot log |
| K3 | With no `inet_hijack`, the guest has **no** INET egress: `connect()` to `1.1.1.1:443` fails, and so does the host loopback | This is the core isolation claim | Guest test binary. Make it a permanent integration test. |
| K4 | `unix_connect` vsock route semantics and throughput for a tar stream of tens of MB | The output channel | Integration test with a byte cap |
| K5 | Ad-hoc signing with `--options runtime` (hardened runtime) still allows `dlopen` of Homebrew libkrunfw without `com.apple.security.cs.disable-library-validation` | Keep the entitlement minimal. microsandbox and krunkit add the disable-library-validation entitlement. | Try without it first, add it only if needed, and document the reason. |
| K6 | Confining `minos-vmm` with a `sandbox-exec` SBPL profile (deny `network*`, restrict file reads to the root, input, and initramfs images, libkrunfw, and system libraries, restrict writes to the job dir) while keeping `hv_vm_create` working | Local must be at least as strict as Lambda, and libkrun says the VMM shares the guest's security context | Experiment. A dedicated `_minos` user is the fallback or complement (`minos host setup`). |
| K7 | Whether msb_krun's `ExitHandle` or `on_exit` gives the parent a clean exit code that separates guest exit from VMM error | Separating `Outcome::Crashed` from an infrastructure error | Read `exit_handle.rs` and test |
| K8 | Throughput of `RunMicrovm`-equivalent launches per second locally, and memory per VM | `width = 4` parallel candidates | Benchmark |

### macOS signing specifics (question 11)

- Hypervisor.framework refuses `hv_vm_create` (`HV_DENIED`) unless the binary carries `com.apple.security.hypervisor`. **Ad-hoc signing is enough for local use.** The Homebrew krunkit formula does exactly this: `codesign --entitlements krunkit.entitlements --force -s - krunkit`. The `applevisor` crate and QEMU document the same.
- Only **`minos-vmm`** gets the entitlement. Every `cargo build` relinks and drops custom entitlements, so:
  - Add an `xtask` (`cargo xtask build-vmm`) that builds `minos-vmm`, then runs `codesign --force --options runtime -s - --entitlements crates/minos-backend-krun/minos-vmm.entitlements <bin>` as an argv array, then verifies the result with `codesign -d --entitlements - <bin>`. Apple forum threads show `HV_DENIED` caused by an entitlement that silently failed to apply, so always verify.
  - VM integration tests locate the signed binary by path. A `.cargo/config.toml` `runner` cannot help, because the runner wraps the *test* binary, not the spawned launcher.
  - `minos host setup` (and `minos host doctor`) checks the installed `minos-vmm` signature and entitlements as part of preflight, and refuses to proceed if they are missing.
- Entitlements plist (minimal; add `com.apple.security.cs.disable-library-validation` only if K5 forces it):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.hypervisor</key><true/>
</dict></plist>
```

---

## Question 2: diff parsing and patch application

| Crate | Version | Parse multi-file | Detect renames, modes, binary | Apply | Verdict |
|---|---|---|---|---|---|
| **diffy** | 0.5.2 (2026-08-31) | Yes (`PatchSet`) | **Yes** (gitdiff mode) | Yes, exact context, but searches every offset | **Use for parsing and generating.** Write our own strict applier. |
| flickzeug | 0.6.0 | Yes | Fork of diffy | **Fuzzy** apply | Reject. Fuzz is the wrong semantics for a judge-integrity system, and it pulls in `clap` as a non-dev dependency. |
| patch | 0.7.0 (2022) | Yes | Partial | No | Stale. Reject. |
| unidiff | 0.4.1 | Yes | Partial | No | Parse only. Reject. |
| similar | 3.2.0 | n/a | n/a | No | Diff generation only. diffy already covers generation. |
| imara-diff | 0.2.0 | n/a | n/a | No | Diff generation only. |

**Recipe.**
1. `Diff::parse(text, &manifest)` enforces the size cap, then runs `PatchSet::parse(text, ParseOptions::gitdiff())`. Reject any `FilePatch` where `operation().is_rename() || is_copy()`, `old_mode().is_some() || new_mode().is_some()`, or `patch().is_binary()`. Every path goes through `RelPath` and must be in `writable`. Decide explicitly whether create and delete are allowed; the default is to allow only `Modify` on writable paths.
2. Apply with a **minos-core strict applier**. For each hunk, require the pre-image to match **at the hunk's stated old-range start**, with no offset search and no fuzz. Reject overlapping or out-of-order hunks. This is pure, deterministic, and linear, which avoids diffy's O(n·m) offset search on adversarial model output. It is also one implementation, shared by the guest (applying the candidate) and `build_report` (composing lineage diffs into the cumulative baseline-to-winner diff, then re-diffing with `diffy::create_patch`).

---

## Question 3: deterministic tar, SHA-256, encoding

- **Packing:** `tar::Builder` over a `Vec<u8>`. For each `RelPath` in sorted order, build a `Header::new_ustar()` by hand: `set_path`, `set_size`, `set_mode(0o644 | 0o755)` (keep only the executable bit, since builds need it), `set_uid(0)`, `set_gid(0)`, `set_mtime(0)`, `set_entry_type(Regular)`, empty username and groupname, then `set_cksum()`. Do **not** use `HeaderMode::Deterministic`: it sets mtime to `1153704088`, not 0, which contradicts "zeroed metadata". Regular files only. Reject symlinks, devices, and hardlinks at walk time.
- **Unpacking hostile streams (host side):** never `unpack()`. Wrap the reader in `Read::take(limits.max_total_bytes)`, iterate `Archive::entries()`, cap the entry count, check `header().entry_type()` is `Regular` and `header().size()` is within the cap **before** reading, push `entry.path()` through `RelPath::try_from`, and read each entry with `take(size_cap)`. Everything goes into `Untrusted<_>` and leaves only through the validating parsers.
- **IDs:** `sha2::Sha256` with the domain-separated, length-prefixed scheme in CLAUDE.md. Golden vectors from the first commit.
- **Hex:** `base16ct::lower`. Blob paths are `blobs/sha256/ab/cdef…`.

## Question 4: serde patterns (verified by a compile probe)

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest { /* mirrors minos.toml */ }

#[derive(Deserialize)]
#[serde(try_from = "RawManifest")]
pub struct Manifest { /* private fields */ }

impl TryFrom<RawManifest> for Manifest { type Error = ManifestError; /* cross-field checks */ }
```

Pitfalls:
- `deny_unknown_fields` **does not work with `#[serde(flatten)]`**. Never flatten raw input structs.
- Internally tagged enums (`#[serde(tag = "v")]`, `tag = "tool"`) buffer content. That is fine for these sizes, but put `deny_unknown_fields` on each variant's struct, not on the enum.
- **The `claude` JSON envelope is the exception to `deny_unknown_fields`.** Claude Code adds optional result fields often: `SDKResultMessage` already has more than 30, including `ttft_ms`, `fast_mode_state`, and `terminal_reason`. Deserialize a minimal envelope struct that ignores unknown fields, then parse `structured_output` with strict `deny_unknown_fields` and `try_from`, because that payload is model output and hostile.

## Question 5: errors and CLI

- Library crates: `thiserror` 2.0.21 enums that split `CandidateFault` (becomes an `Outcome`, never retried) from `InfraFault` (retried). `minos-cli` `main` returns `anyhow::Result<()>`. Nothing else uses `anyhow`.
- `clap` 4.6.7 derive. Treat every user-facing string that came from a sandbox or a model with the core escape-stripping renderer (below).

## Question 6: testing

| Need | Tool | Notes |
|---|---|---|
| Parsers, `RelPath`, `Diff::parse`, strict applier, policy permutation-invariance, ledger fold, money arithmetic | `proptest` 1.11.0 | Commit `proptest-regressions/` files. |
| Golden wire formats (`Record`, checkpoints, guest protocol, `VmmConfig`, report) | `insta` 1.49.0 with `json` and `toml` features; `cargo-insta` 1.49.0 | Snapshot the serialized string. Run the gate with `INSTA_UPDATE=no`. |
| Golden hash vectors | Plain `assert_eq!` against hex literals | Keep them separate from insta so they can never be auto-accepted. |
| Compile-fail invariants | `trybuild` 1.0.122 | Pinned toolchain keeps the stderr stable. |
| VM integration (K1 through K4) | `#[ignore]`d tests, or a feature-gated test crate run by `cargo xtask vm-test` | Needs the signed `minos-vmm` and libkrunfw present. |

## Question 7: supply chain (`cargo-deny` 0.20.2)

```toml
# deny.toml
[graph]
targets = ["aarch64-apple-darwin", "aarch64-unknown-linux-musl"]   # add Linux/Lambda triples later

[advisories]
unmaintained = "workspace"
yanked = "deny"

[licenses]
allow = ["Apache-2.0", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib", "MPL-2.0", "CC0-1.0"]

[bans]
multiple-versions = "warn"
wildcards = "deny"
deny = [
  { crate = "openssl-sys", reason = "no OpenSSL" },
  { crate = "openssl", reason = "no OpenSSL" },
  { crate = "native-tls", reason = "no OpenSSL" },
  { crate = "git2", reason = "no libgit2 / no git in trusted path" },
  { crate = "libgit2-sys", reason = "no libgit2" },
  { crate = "gix", reason = "no git in trusted path" },
  { crate = "zstd-sys", reason = "C; use gzip squashfs" },
  { crate = "liblzma-sys", reason = "C" },
  { crate = "cc", reason = "no C compiled in our tree (revisit for rustls provider later)" },
]

[bans.build]
# Curated allowlist; with msb_krun[blk] the probe found exactly these build scripts:
# crc32fast, crossbeam-utils, getrandom, libc, memoffset, msb_krun_hvf (links Hypervisor.framework),
# msb-imago (nightly detection), nix, proc-macro2, quote, serde, serde_core, thiserror, zerocopy.
allow-build-scripts = [ { crate = "libc" }, { crate = "proc-macro2" }, { crate = "serde" }, { crate = "serde_core" },
  { crate = "thiserror" }, { crate = "nix" }, { crate = "getrandom" }, { crate = "crc32fast" }, { crate = "crossbeam-utils" },
  { crate = "memoffset" }, { crate = "zerocopy" }, { crate = "quote" }, { crate = "msb_krun_hvf" }, { crate = "msb-imago" } ]
executables = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
required-git-spec = "rev"          # enforces "pin git deps by rev"
allow-git = []                     # add aws-durable-execution-sdk-rust URL in the AWS milestone
```

A git dependency (AWS milestone) looks like `aws-durable-execution-sdk = { git = "https://github.com/aws/aws-durable-execution-sdk-rust", rev = "<40-hex sha>" }`. `required-git-spec = "rev"` makes a `branch`/`tag` spec fail the gate.

**Note (MEDIUM):** the `cc` ban must be lifted, or the crate given a `wrappers` entry, when the rustls crypto provider arrives in the AWS milestone. That is the single accepted C exception.

## Question 8: exact money arithmetic

**Recommendation: hand-rolled, in `minos-core/usage.rs`, with no dependency.**
- `Rate`: parsed from a decimal string into `u128` **pico-dollars** (10⁻¹² USD) per unit. Reject negative numbers, exponents, more than 12 fractional digits (**fail closed**, never round on parse), and overflow. Published AWS rates have 10 fractional digits. Lambda's own pricing page shows MicroVM `$0.0000276944`/vCPU-second, `$0.0000036667`/GB-second, and Lambda x86 `$0.0000166667`/GB-s. **CLAUDE.md's "nano-dollar precision" (9 digits) cannot represent these exactly. Update CLAUDE.md to pico-dollars.**
- `Cost`: `u128` pico-dollar accumulation from integer usage (vCPU-milliseconds, MB-milliseconds, tokens). Convert to integer **micro-dollars** once, at display or comparison time, with one documented rounding rule (round half up) and a property test.
- One `ApproxCost` type owns `approx. cost: $31.40` formatting.
- A clippy `disallowed-methods` rule in core bans `f64::from`/`as f64` paths on these types, and `Metric` remains the only float type.

Alternative: `rust_decimal` 1.43.0 (`from_str_exact`, `checked_mul`; 96-bit mantissa, scale up to 28). It is acceptable, but it brings float conversion APIs and many optional DB features into core for something that takes about 150 lines of integer code.

## Question 9: driving `claude -p` from Rust

**Verified against the installed Claude Code 2.1.295 `--help` and the docs (code.claude.com/docs/en/headless; Agent SDK TypeScript reference).**

```rust
// minos-propose, ModelProvider::ClaudeCli (sketch)
let mut cmd = std::process::Command::new(&resolved_claude_abs_path);   // resolved once, version recorded
cmd.env_clear()                                   // ALLOWLIST, not env_remove denylist
   .envs(allowlisted_env)                          // HOME, PATH, USER, LOGNAME, TMPDIR, LANG; CLAUDE_CONFIG_DIR only if set
   .current_dir(empty_private_tempdir)             // never the project dir
   .process_group(0)
   .args(["-p", "--output-format", "json", "--json-schema", SCHEMA_JSON,
          "--tools", "", "--model", model_alias,
          "--no-session-persistence", "--permission-mode", "dontAsk",
          "--permission-prompts", "none", "--strict-mcp-config", "--safe-mode",
          "--max-budget-usd", cap])
   .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
// write the prompt to stdin (docs: piped stdin capped at 10 MB; argv is limited by ARG_MAX)
// reader threads drain stdout/stderr with byte caps; the main thread polls try_wait() to a deadline,
// then rustix::process::kill_process_group(SIGTERM then SIGKILL)
```

Why each choice:
- **Allowlist the environment.** A denylist of `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and `CLAUDE_CODE_USE_*` misses `ANTHROPIC_BASE_URL` (which redirects traffic) and any future variable. `env_clear()` plus an allowlist fails closed.
- **Empty cwd plus `--safe-mode` plus `--strict-mcp-config`.** The docs say: *"Without `--bare`, a `-p` session runs the hooks in a project's `.claude/settings.json` and connects the servers in its `.mcp.json`, even in a folder you've never trusted."* Hooks are arbitrary code that would see the prompt. `--safe-mode` disables CLAUDE.md, skills, plugins, hooks, MCP, and custom agents, while "auth, model selection… work normally".
- **Do not use `--bare`** with the subscription path. In bare mode, *"Anthropic auth is strictly ANTHROPIC_API_KEY or apiKeyHelper… OAuth and keychain are never read."* `--bare` is right later for the API-key provider.
- **Result parsing:** `structured_output` (schema payload); `subtype` ∈ {`success`, `error_max_turns`, `error_during_execution`, `error_max_budget_usd`, `error_max_structured_output_retries`}; `is_error`; `num_turns`; `usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}`; `modelUsage` (camelCase, per model). **Record tokens only. `total_cost_usd` is an f64 "client-side estimate", and it must never enter the ledger.**
- **Use sync `std::process`, not `tokio::process`.** One call per propose step, and no other async work in v1.
- **Unverified (spike before building the provider):** whether `--tools ""` combined with `--json-schema` still yields `structured_output` (structured output may use an internal tool; the `error_max_structured_output_retries` subtype hints at retries); `--max-turns 1` behavior with structured output; whether `--safe-mode` and `--permission-prompts none` combine cleanly with `-p`; whether subscription auth works with the allowlisted environment (Keychain access); and whether `--tools ""` also removes the Agent and Skill tools.
- **The JSON Schema string:** a hand-written `const`, golden-tested, and round-tripped against an example payload in a unit test. `schemars` 1.2.2 is the alternative if the proposal type grows.

## Question 10: sync or async?

**Sync for all of v1. Do not add tokio.**

| Need | Sync primitive |
|---|---|
| Parallel candidates (`width = 4`) | `std::thread::scope`, one thread per VM supervisor |
| Launching and killing VMs | `std::process::Command` + `CommandExt::process_group` + `rustix::process::kill_process_group` |
| Output stream | `std::os::unix::net::UnixListener` + `Read::take` caps; timeouts via `set_read_timeout` |
| `claude` subprocess | `std::process` + reader threads + `try_wait` deadline polling |
| Ledger lock | `std::fs::File::lock`/`try_lock` (**stable since Rust 1.89.0**, verified in std docs); no `fd-lock` |
| Atomic blob write | temp file in the same directory, then `sync_all`, `rename`, and fsync of the directory |

Rationale: core is sync by rule, and nothing in v1 does network I/O. Every async rule in `rules/` (`async-*`, `anti-lock-across-await`) then becomes moot. The cost appears later: the Lambda backend and orchestrator will use tokio through the AWS SDK and the durable SDK. **Keep `minos-sandbox`'s `Backend` trait synchronous in v1, and do not build `AnyBackend` with a Lambda arm yet.** Revisit when the Lambda backend lands. That backend runs inside the CPU runner function, never in the local driver, so the trait may end up split by host (local supervisor vs runner) rather than forced into async everywhere. Flag that decision for the AWS milestone.

---

## Installation

```toml
# Cargo.toml (workspace root), sketch
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
edition = "2024"
rust-version = "1.97"
license = "Apache-2.0 OR MIT"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"

[workspace.dependencies]
serde        = { version = "=1.0.229", features = ["derive"] }
serde_json   = "=1.0.151"
toml         = "=1.1.8"
thiserror    = "=2.0.21"
anyhow       = "=1.0.104"
clap         = { version = "=4.6.7", features = ["derive"] }
sha2         = "=0.11.0"
base16ct     = { version = "=1.0.0", features = ["alloc"] }
diffy        = { version = "=0.5.2", default-features = false, features = ["std"] }
tar          = { version = "=0.4.46", default-features = false }
ignore       = "=0.4.33"
regex        = "=1.13.1"
rand_chacha  = { version = "=0.10.0", default-features = false }
rand_core    = "=0.10.1"
rustix       = { version = "=1.1.5", features = ["fs", "mount", "process", "system"] }
vsock        = "=0.5.4"
msb_krun     = { version = "=0.1.41", default-features = false, features = ["blk"] }
backhand     = { version = "=0.25.5", default-features = false, features = ["gzip"] }
tempfile     = "=3.27.0"
tracing      = "=0.1.44"
tracing-subscriber = "=0.3.23"
# dev
proptest     = "=1.11.0"
insta        = { version = "=1.49.0", features = ["json", "toml"] }
trybuild     = "=1.0.122"
```

Exact `=` pins are deliberate. Golden vectors and durable replay require output-stable dependencies, `Cargo.lock` is committed anyway, and upgrades become explicit reviewed changes. Check that each feature name exists for the pinned version when writing the manifest; for example, verify the `base16ct` `alloc` feature.

```bash
rustup target add aarch64-unknown-linux-musl
brew tap libkrun/krun && brew trust libkrun/krun && brew install libkrunfw
cargo install --locked cargo-deny@0.20.2 cargo-insta@1.49.0
```

```toml
# .cargo/config.toml
[target.aarch64-unknown-linux-musl]
linker = "rust-lld"
```

```toml
# crates/minos-core/clippy.toml  (replaces, does not merge with, any root clippy.toml)
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "time arrives as an argument" },
  { path = "std::time::Instant::now", reason = "time arrives as an argument" },
]
disallowed-types = [
  { path = "std::collections::HashMap", reason = "order must be stable: use BTreeMap" },
  { path = "std::collections::HashSet", reason = "order must be stable: use BTreeSet" },
]
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use the Alternative |
|---|---|---|
| msb_krun (fork, safe Rust API) | Upstream libkrun 1.19.6 via the C ABI | Only if the user explicitly approves an `unsafe`-permitting binding crate, or once upstream publishes its v2 Rust-native API as a crate (then switch). |
| Per-VM `minos-vmm` launcher | Re-exec the `minos` binary itself (microsandbox's pattern) | Never for Minos. It would put the hypervisor entitlement and the VMM code on the main CLI binary. |
| Initramfs `/init` + virtio-blk root | virtio-fs root (`FsBuilder::root`) | Never. virtio-fs shares a host directory, and libkrun says it gives no protection. |
| SquashFS via `backhand` (gzip) | EROFS via `composefs` 0.9.4 | If SquashFS mount performance or page-cache behavior disappoints. EROFS is the modern read-only rootfs. |
| SquashFS via `backhand` (gzip) | ext4 built inside a builder VM | If the guest needs a writable root. It does not: use an overlayfs upper on tmpfs or scratch. |
| Hand-rolled pico-dollar integers | `rust_decimal` 1.43.0 | If the price table needs arithmetic beyond multiply, add, and round. |
| `diffy` + own strict applier | `diffy::apply` directly | Never for candidates: offset search plus O(n·m) is wrong for adversarial input. It is acceptable for tests. |
| `base16ct` | `hex` 0.4.3 / `data-encoding` 2.11.1 | `data-encoding` if one crate should cover hex and base32 or base64 with a different publisher. `hex` is fine but last released in 2021. |
| `rand_chacha` + own range mapping | Hand-rolled SplitMix64 or PCG | If even rand_chacha's seeding API churn is unacceptable. A 20-line PRNG is value-stable forever. |
| Sync std | tokio 1.53.2 | The AWS milestone (Lambda runtime, AWS SDK, durable SDK). |

## What NOT to Use

| Avoid | Why | Use Instead |
|---|---|---|
| `krun-sys`, `libkrun-sys`, `bux-krun`, `a3s-libkrun-sys`, the `libkrun` crate's `extern "C"` fns | Raw FFI that needs `unsafe` in our crates, which violates `unsafe_code = "forbid"` | `msb_krun` behind `minos-vmm` |
| libkrun **implicit vsock** (upstream default) | Silently enables TSI `HIJACK_INET` when no NIC exists, which gives the guest the host's network | msb_krun with `inet_hijack` at its default `false`, the `net` feature off, and a clippy ban |
| virtio-fs (`krun_set_root`, `FsBuilder::root`/`path`) | Shares a host directory; libkrun: *"does not provide any protection"* | Raw read-only block devices and an initramfs |
| qcow2 or vmdk images, any format probing | libkrun's header: a guest can rewrite a raw image into qcow2 that references host files, and that becomes an escape if the image is re-probed | `DiskImageFormat::Raw` always, chosen by code, never detected |
| krunkit / krunvm as helpers | EFI-boot model, a REST status endpoint, a buildah dependency, coarse device control | Own launcher |
| `tar::Archive::unpack` / `Entry::unpack_in` on the host | Writes guest-chosen paths, symlinks, and modes to the host filesystem | Iterate entries in memory with caps into `Untrusted<_>` |
| `tar::HeaderMode::Deterministic` | mtime 1153704088, not 0; mode normalization does not match ours | Hand-built `Header::new_ustar()` |
| `flickzeug` and any fuzzy patching | Fuzz makes applied content depend on heuristics, which breaks reproducibility and judge integrity | Strict applier |
| `rand::Rng::gen_range` and `rand` distributions in core | Not value-stable across versions, which breaks golden vectors and durable replay | `rand_chacha` stream + own mapping |
| `backhand` default features | Pull in `liblzma-sys` and `zstd-sys` (C) | `default-features = false, features = ["gzip"]` |
| `openssl`, `native-tls`, `git2`, `libgit2-sys`, `gix` | CLAUDE.md: no OpenSSL, no libgit2, no git in the trusted path | n/a in v1 (no network). rustls later. |
| tokio in v1 | Adds a runtime and the async rule surface for zero benefit locally | std threads and processes |
| `fd-lock` / `fs2` | Superseded by `std::fs::File::lock` (1.89) | std |
| `deny_unknown_fields` on the claude result envelope | Claude Code adds result fields frequently, so it would break on CLI upgrades | Lenient envelope, strict `structured_output` |
| `claude --bare` with subscription auth | Bare mode never reads OAuth or Keychain | `--safe-mode` + empty cwd + `--strict-mcp-config` (subscription); `--bare` (API-key provider, later) |
| `total_cost_usd` from claude | f64 client-side estimate | Record tokens and derive cost from `prices.toml` |
| `strip-ansi-escapes` / `vte` for the renderer | A denylist parser of escape sequences | Hand-rolled **allowlist** in core: keep printable characters plus `\n` and `\t`; drop all C0 and C1 controls including ESC, and Unicode bidi overrides U+202A–U+202E and U+2066–U+2069 (Trojan Source) |

## Stack Patterns by Variant

**If the K1/K2 boot spike shows msb_krun cannot boot from an initramfs without a virtiofs root:**
- Use `FsBuilder::custom` with a NullFs-style backend (no host directory) serving only `/init`, mirroring upstream's `krun_set_root_disk_remount`.
- This is still no host directory sharing, so the invariant holds.

**If msb_krun fails the boot spike entirely, or its maintenance stalls:**
- Stop and ask the user. The remaining options are an `unsafe` binding crate (an invariant exception) or waiting for upstream's v2 Rust API.
- Do not quietly add `#![allow(unsafe_code)]` anywhere.

**If `sandbox-exec` breaks `hv_vm_create` (K6):**
- Fall back to a dedicated `_minos` user created by `minos host setup`, and file the gap in the report's provenance as reduced local hardening. Never fall back silently.

## Version Compatibility

| Package A | Compatible With | Notes |
|---|---|---|
| msb_krun =0.1.41 | macOS 27.0 arm64, rustc 1.97.1 | Compile probe passed. It pins `msb-vm-memory =0.18.0-msb.2`; let it own that, and never add upstream `vm-memory` alongside. |
| msb_krun =0.1.41 | libkrunfw 5.6.2 | **Unverified at runtime (K1).** msb_krun's `KernelBuilder::krunfw_path` `dlopen`s the dylib. |
| sha2 0.11.0 | digest 0.11 / hybrid-array | Breaking change from 0.10 (`GenericArray` gone). Do not mix crates pinned to digest 0.10. |
| rand_chacha 0.10.0 | rand_core 0.10.1 | rand_core 0.10 changed its traits; keep both on 0.10. |
| toml 1.1.8 | serde 1.0.229 | TOML 1.1 syntax is accepted, for example multi-line inline tables. A manifest valid for Minos may not parse in TOML-1.0-only tools. |
| trybuild 1.0.122 | MSRV 1.88 | stderr snapshots are tied to the pinned 1.97.1. |
| backhand 0.25.5 | MSRV 1.90 | Fine on 1.97.1. |
| diffy 0.5.2, clap 4.6.7, proptest 1.11.0, toml 1.1.8, sha2 0.11.0 | MSRV 1.85 | All fine. |

## Corrections to current planning assumptions

1. **The Homebrew tap is `libkrun/krun`** (`brew tap libkrun/krun && brew trust libkrun/krun`). PROJECT.md says `slp/krun`. GitHub redirects the old name, but the tap README and formulae now use `libkrun/*`. The upstream repo also moved to `github.com/libkrun/libkrun`.
2. **"No network device" is not the libkrun default.** With no NIC, upstream turns TSI on. The isolation claim depends on msb_krun's opposite default plus the compile-time and lint-time bans above, and it must be proven by a permanent integration test (K3).
3. **The VMM process terminates when the guest exits**, so a separate launcher process is required by the API, not just preferred.
4. **Nano-dollar precision is too coarse.** Use pico-dollars.
5. **`--bare` cannot be used with the subscription login.** Use `--safe-mode` and an empty cwd instead.

## Sources

- crates.io API (`/api/v1/crates/<name>`), queried 2026-10-09: every version, MSRV, and feature list above. HIGH.
- msb_krun 0.1.41 crate source (`static.crates.io/crates/msb_krun/msb_krun-0.1.41.crate`): `src/lib.rs` lifecycle docs (`enter()` never returns, `_exit()`), `src/api/builders.rs` (`enable_inet_hijack` "Defaults to `false` — guests with no virtio-net are air-gapped by default"; `DiskBuilder`, `VsockBuilder::unix_connect`, `KernelBuilder::initramfs_path`), `src/api/vm.rs` (cmdline `root=/dev/root init=…`; `MSB_KRUN_KERNEL_CMDLINE` env read). Source read. HIGH for the claims, MEDIUM for runtime behavior.
- libkrun 1.19.6 crate source and `include/libkrun.h` at tag v1.19.6: `krun_start_enter` exits the process; implicit-vsock TSI heuristic (`enable_tsi = net.list.is_empty() && legacy_net_cfg.is_none()`); `krun_disable_implicit_vsock` / `krun_add_vsock(ctx, 0)`; `krun_add_disk2` security note on format probing; `krun_set_root_disk_remount` NullFs root. HIGH.
- libkrun README (github.com/libkrun/libkrun): security model, virtio-fs warning, TSI auto-enable. HIGH.
- libkrun main-branch `include/libkrun.h` (`@generated` v2 API with `VmmBuilder`, `krun_vsock_device_new(cid, tsi_features)`): direction only. LOW for timing.
- libkrunfw `config-libkrunfw_aarch64` (main): kernel 6.12.91 and built-in filesystems and devices. HIGH.
- github.com/libkrun/homebrew-krun README and `Formula/{libkrun,libkrunfw,krunkit,krunvm}.rb`: tap name, versions, krunkit ad-hoc `codesign --entitlements`. HIGH.
- superradcompany/microsandbox `Cargo.toml` (`msb_krun = "=0.1.41"`), `msb-entitlements.plist`, `justfile` (codesign step): adoption and signing pattern. HIGH.
- diffy 0.5.2 crate source (`src/patch_set/mod.rs`, `src/apply.rs`) plus a compile probe (rename detection). HIGH.
- tar 0.4.46 crate source (`HeaderMode::Deterministic` sets mtime 1153704088; features). HIGH.
- cargo-deny docs (`docs/src/checks/sources/cfg.md`: `required-git-spec`, `unknown-git`, `allow-git`; `bans/cfg.md`: `[bans.build] allow-build-scripts`, `executables`) and CHANGELOG (0.20.2, 2026-07-09). HIGH.
- Claude Code docs: code.claude.com/docs/en/headless (`structured_output`, bare-mode auth, hooks and `.mcp.json` running under `-p`, 10 MB stdin cap) and code.claude.com/docs/en/agent-sdk/typescript.md (`SDKResultMessage`, `NonNullableUsage`, `ModelUsage`). Local `claude --help` (2.1.295): `--tools`, `--json-schema`, `--safe-mode`, `--strict-mcp-config`, `--permission-prompts`, `--max-budget-usd`, `--bare` auth text. HIGH for flags and fields; flag interplay is unverified.
- doc.rust-lang.org `std::fs::File::lock` (since 1.89.0) and `CommandExt::process_group`. HIGH.
- aws.amazon.com/lambda/pricing (MicroVM `$0.0000276944`/vCPU-s, `$0.0000036667`/GB-s; Lambda `$0.0000166667`/GB-s): basis for the pico-dollar correction. HIGH.
- Hypervisor entitlement and ad-hoc signing: [docs.rs/applevisor](https://docs.rs/applevisor), [QEMU hvf entitlement patch](https://patchwork-proxy.ozlabs.org/comment/2587584), [Apple Developer Forums 808360 (HV_DENIED on macOS 26)](https://developer.apple.com/forums/thread/808360), [Apple Developer Forums 668496 (Hypervisor in an inherited App Sandbox)](https://developer.apple.com/forums/thread/668496). MEDIUM.
- Local compile probes in the session scratchpad (`krunprobe`, `shaprobe`, `guesttest`). HIGH for what they prove (build and link only).

---
*Stack research for: Minos v1, a local libkrun-isolated experiment loop on macOS arm64*
*Researched: 2026-10-09*
