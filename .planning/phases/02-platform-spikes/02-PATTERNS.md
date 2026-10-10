# Phase 2: Platform Spikes - Pattern Map

**Mapped:** 2026-10-09
**Files analyzed:** 19 new or modified
**Analogs found:** 11 / 19 (the workspace is Phase 1 stubs: config analogs are strong, Rust code analogs are thin)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `rust-toolchain.toml` (add musl target) | config | n/a | itself, lines 1-7 | exact (edit in place) |
| `.cargo/config.toml` (musl `linker = "rust-lld"`, NO rustflags) | config | n/a | itself, lines 1-10 | exact |
| `crates/minos-vmm/clippy.toml` | config (lint policy) | n/a | `crates/minos-core/clippy.toml` | exact |
| `crates/minos-vmm/minos-vmm.entitlements` | config (signing plist) | n/a | none in repo | no analog |
| `crates/minos-vmm/minos-vmm.sb` | config (Seatbelt SBPL) | n/a | none in repo | no analog |
| `crates/minos-vmm/Cargo.toml` (maybe toml/serde deps) | config | n/a | itself; `crates/minos-guest/Cargo.toml` | exact |
| `crates/minos-vmm/src/main.rs` | binary entry | request-response (argv config -> exit code) | current stub, line 1-3 | role-match (stub) |
| `crates/minos-vmm/src/config.rs` | model (validated wire type) | transform (deserialize = validate) | none in code; CLAUDE.md "Deserialization is validation" + `rules/api-parse-dont-validate.md` | no code analog |
| `crates/minos-vmm/src/watchdog.rs` | utility | event-driven (deadline, parent death) | none | no analog |
| `crates/minos-guest/Cargo.toml` (panic=abort note, deps) | config | n/a | itself lines 13-20 | exact |
| `crates/minos-guest/src/lib.rs` | library (host-compilable) | transform | `crates/minos-core/src/lib.rs` (doc header only) | partial |
| `crates/minos-guest/src/main.rs` | binary entry (PID 1) | request-response | current stub | role-match |
| `crates/minos-guest/src/linux.rs` | utility, cfg(linux) | streaming (vsock tar out) | none | no analog |
| `crates/minos-backend-krun/src/rootfs.rs` | utility | file-I/O / transform (tar -> SquashFS) | none | no analog |
| `crates/minos-backend-krun/src/initramfs.rs` | utility | transform (newc writer) | none | no analog |
| `crates/minos-backend-krun/src/input_disk.rs` | utility | transform | none | no analog |
| `crates/minos-backend-krun/tests/spike_vm.rs` | test (#[ignore] integration) | event-driven | none (no tests/ dirs exist) | no analog |
| `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md` | doc (evidence) | n/a | `.planning/phases/01-workspace-and-gates/01-GATE-EVIDENCE.md` | exact |
| `CLAUDE.md`, `.claude/CLAUDE.md` (DOCS-05 amendments) | doc | n/a | themselves; 01-05 clap correction as precedent | exact |

## Pattern Assignments

### `rust-toolchain.toml` (config)
**Analog:** itself (lines 1-7). Replace the comment block on lines 5-7 with a `targets` line and a short reason (guest is always Linux; `/init` must be static, no libc in the VM):
```toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy"]
profile = "minimal"
# targets: add "aarch64-unknown-linux-musl" in Phase 5 (D-09). ...   <- replace (D-03 pulls it forward)
```

### `.cargo/config.toml` (config)
**Analog:** itself, lines 1-10. The header comment (lines 4-5) is the binding constraint:
```toml
# A RUSTFLAGS environment variable silently replaces this list, and so do CARGO_ENCODED_RUSTFLAGS and any
# target.<triple> or target.<cfg> rustflags key. With any of them set, the backstop is gone.
[build]
rustflags = ["-Funsafe_code", "-Fclippy::unwrap_used", "-Fclippy::expect_used", "-Fclippy::indexing_slicing"]
```
Add `[target.aarch64-unknown-linux-musl] linker = "rust-lld"` only; never a `rustflags` key there (G-02-CLOSED). Prove with the `.unwrap()` probe built for the musl target and record it in the evidence file.

### `crates/minos-vmm/clippy.toml` (lint policy)
**Analog:** `crates/minos-core/clippy.toml` lines 1-16. Copy the header comment verbatim in spirit (replaces, does not merge; every path proven to fire once; mistyped path only warns):
```toml
# Replaces (does not merge with) any ancestor clippy.toml. ...
# Every path below is a real std path, proven to fire once (01-GATE-EVIDENCE.md V-09, ...).
# A mistyped path only warns, so re-prove any entry you change.
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "minos-core has no clock: time arrives as an argument" },
```
Entries per RESEARCH Anti-Patterns: `msb_krun::api::builders::FsBuilder::root`, `FsBuilder::path`, `ExecBuilder::env`/`envs`, `VsockBuilder::inet_hijack`, `MachineBuilder::enable_inet_hijack`, `VsockBuilder::tcp_listen_remap`, `DiskBuilder::format`, `DiskBuilder::layers` (and `FsBuilder::custom` unless Option B). Verify each definition path in the pinned 0.1.41 source and prove each fires with a temporary probe (the V-09 method), recorded in 02-SPIKE-EVIDENCE.md.

### `crates/minos-vmm/src/main.rs` and `crates/minos-guest/src/main.rs` (binary entry)
**Analog:** the stubs, which fix the crate-doc convention (`missing_docs = "warn"`):
```rust
//! Per-VM launcher: the only binary that links the VMM crate.

fn main() {}
```
Grow into `fn main() -> std::process::ExitCode` that maps a `thiserror` error enum to documented exit codes. No `unwrap`/`expect`/`[i]` (use `args().nth(1)`, `.get(..)`, `?`). Read config path from argv; never stdin (KRUN-01 stdin is `/dev/null`). Guest `main.rs`: `#[cfg(target_os = "linux")] mod linux;` with the PID 1 path, non-Linux branch returns an error, so the host gate still compiles everything else.

### `crates/minos-vmm/src/config.rs` (validated wire type)
**No code analog yet.** Apply CLAUDE.md "Deserialization is validation":
```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVmmConfig { v: u32, kernel: ..., disks: Vec<...>, vsock: ..., max_duration: ..., console_cap: ... }

#[derive(Deserialize)]
#[serde(try_from = "RawVmmConfig")]
pub struct VmmConfig { /* private fields, no setters */ }
impl TryFrom<RawVmmConfig> for VmmConfig { type Error = ConfigError; ... }
```
`v` tag present from the start; absolute canonical paths; never `#[serde(flatten)]`. Deps available: `serde`, `serde_json`, `thiserror` already in `crates/minos-vmm/Cargo.toml` lines 13-17.

### `crates/minos-guest/Cargo.toml`
**Analog:** itself lines 13-20, which already shows the split to keep:
```toml
[dependencies]
minos-core.workspace = true
thiserror.workspace = true
tar.workspace = true

[target.'cfg(target_os = "linux")'.dependencies]
rustix.workspace = true
vsock.workspace = true
```
Linux-only deps stay in the target table; host-compilable logic goes in `lib.rs` (D-03).

### `crates/minos-backend-krun/src/{rootfs,initramfs,input_disk}.rs`
**Analog:** none in code. Dependencies already declared in `crates/minos-backend-krun/Cargo.toml` lines 13-19 (`backhand` gzip-only, `tempfile`, `thiserror`, `serde_json`). Follow `.claude/CLAUDE.md` Question 3 for tar: never `Archive::unpack`; `Read::take` caps; check `entry_type()` Regular and size before reading; paths through `RelPath`. Golden hash test for the newc writer: plain `assert_eq!` against a hex literal (kept out of insta).

### `crates/minos-backend-krun/tests/spike_vm.rs`
**Analog:** none. Every test `#[ignore]`; return `Result<(), Box<dyn std::error::Error>>` and use `?` instead of `unwrap` (the forbid lints apply in tests; never add an allow). Paths from env vars; job dir short and 0700 (socket path length, Pitfall 30); spawn `minos-vmm` with `env_clear()`.

### `.planning/phases/02-platform-spikes/02-SPIKE-EVIDENCE.md`
**Analog:** `01-GATE-EVIDENCE.md` lines 1-30:
```markdown
# Phase 1: Gate Evidence (D-10, D-11)

Date: 2026-10-09

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- ...
- HEAD at start of proofs: `<sha>`

## Code-lint proofs

### V-01: unsafe block

Requirement: GATE-01

Edit: <file>: <probe>

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
<verbatim output>
```
```
Use one `### S-xx` (or `K1`..`K8`) heading per spike with Requirement, Edit/Setup, Command, Exit code, verbatim output; state truncation rules up front; note which research claim was overturned (D-09). Redact email/orgId from `claude auth status`.

## Shared Patterns

### Lints and no-allow (all Rust files)
**Source:** `Cargo.toml` lines 31-63 and `.cargo/config.toml` line 10. Every crate already has `[lints] workspace = true`. No `#[allow(` anywhere (C-02 grep), no `unwrap`/`expect`/indexing including tests and probes. Pedantic is warn and gate runs `-D warnings`, so add `# Errors` doc sections and `#[must_use]` where clippy asks.

### Errors
**Source:** `rules/err-thiserror-lib.md`; CLAUDE.md "Errors separate blame". `thiserror` enums in `minos-vmm`, `minos-guest`, `minos-backend-krun`; distinguish guest fault from infra fault (K7 exit classification).

### Commands are argv
`std::process::Command` with `env_clear()` plus allowlist for both `minos-vmm` and `claude -p`; never a shell string. codesign is one documented argv in CLAUDE.md Commands (D-11), no xtask/build.rs/script.

### Untrusted output
Anything read from the guest or vsock stays bytes under caps until a validating parser; never logged, no `Debug`.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `minos-vmm.entitlements` | signing plist | n/a | first signing artifact; use RESEARCH/STACK plist (hypervisor only, K5) |
| `minos-vmm.sb` | Seatbelt profile | n/a | use RESEARCH Pattern 6 SBPL |
| `minos-vmm/src/config.rs` | validated wire type | transform | no serde types exist yet; follow CLAUDE.md try_from pattern |
| `minos-vmm/src/watchdog.rs` | utility | event-driven | use RESEARCH (poll `parent_id()`, deadline, ExitHandle) |
| `minos-guest/src/linux.rs` | PID 1 / vsock | streaming | use RESEARCH boot recipe |
| `minos-backend-krun/src/{rootfs,initramfs,input_disk}.rs` | disk builders | transform | use STACK Supporting Libraries + RESEARCH structure |
| `minos-backend-krun/tests/spike_vm.rs` | integration test | event-driven | no `tests/` dir exists in the workspace |

## Metadata

**Analog search scope:** `crates/*`, root `Cargo.toml`, `.cargo/config.toml`, `rust-toolchain.toml`, `rules/`, `.planning/phases/01-workspace-and-gates/`
**Files scanned:** 14
**Pattern extraction date:** 2026-10-09
