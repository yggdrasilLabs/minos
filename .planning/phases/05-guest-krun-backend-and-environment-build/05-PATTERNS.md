# Phase 5: Guest, krun Backend, and Environment Build - Pattern Map

**Mapped:** 2026-10-18
**Files analyzed:** 33 (from 05-RESEARCH.md "Recommended Project Structure" plus Wave 0 tests)
**Analogs found:** 33 / 33 (most are Phase 2 spike code being promoted)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `crates/minos-core/src/guest.rs` (NEW, job v2, status, disk layout) | model / wire codec | transform | `crates/minos-core/src/job.rs` | exact |
| `crates/minos-core/src/chunk.rs` (NEW, chunk header + reassembly) | utility (pure codec) | streaming | `crates/minos-core/src/job.rs` (codec) + `minos-backend-krun/src/supervise.rs` `FrameError` (l.199) | role-match |
| `crates/minos-core/src/lock.rs` (NEW, minos.lock TOML) | model / wire | transform | `crates/minos-core/src/manifest.rs` (Raw + try_from), `job.rs` | exact |
| `crates/minos-core/src/id.rs` (+ BlobHasher) | utility | streaming hash | same file, `IdHasher` l.43-93 | exact |
| `crates/minos-core/src/limits.rs` (+ memory/vcpu/scratch bounds) | config | - | same file `Limits` l.199; `minos-vmm/src/config.rs` l.505 bounds test | exact |
| `crates/minos-core/src/manifest.rs` (+ `[environment] memory_mib, scratch_mib`) | model | validation | same file | exact |
| `crates/minos-store/src/file/blobs.rs` (+ put_reader, verified_path) | store | file-I/O | same file `put` l.32-58, `get` l.60-99 | exact |
| `crates/minos-sandbox/src/blob_source.rs` (NEW trait) | trait/service | request-response | `crates/minos-sandbox/src/backend.rs` | role-match |
| `crates/minos-sandbox/src/spec.rs` (InputDisk reshape) | model | - | same file | exact |
| `crates/minos-sandbox/src/preflight.rs` (+ Refusals, layers) | model | validation | same file l.23-120 | exact |
| `crates/minos-sandbox/src/finished.rs` (+ elapsed, Ended variant, console) | model | - | same file l.25-94 | exact |
| `crates/minos-vmm/src/config.rs` (v2) | config parser | transform | same file l.120-330 | exact |
| `crates/minos-vmm/src/launch.rs` (+ `.rng(true)`) | launcher | - | same file | exact |
| `crates/minos-vmm/src/log.rs` (NEW capped logger) | utility | streaming | `crates/minos-vmm/src/watchdog.rs` (console cap logic) | partial |
| `crates/minos-vmm/src/main.rs` (own rlimits at entry) | entrypoint | - | same file | exact |
| `crates/minos-backend-krun/src/backend.rs` (NEW `KrunBackend`) | service | request-response | `minos-sandbox/src/backend.rs` trait + `supervise.rs::run` l.596 | role-match |
| `crates/minos-backend-krun/src/host.rs` (NEW probes) | service | request-response (subprocess) | `supervise.rs` `has_hypervisor_entitlement` l.439, `sha256_file` l.411; `minos-source/src/clean.rs` `run_capped` l.196 | exact |
| `crates/minos-backend-krun/src/seatbelt.rs` (NEW) | config/utility | - | `tests/spike_seatbelt.rs` + `crates/minos-vmm/minos-vmm.sb` | exact |
| `crates/minos-backend-krun/src/jobdir.rs` (NEW) | utility | file-I/O | `supervise.rs` `short_job_dir` l.250, `JobFiles` l.261-360; store `ensure_dir`/`symlink_metadata` checks | exact |
| `crates/minos-backend-krun/src/chunks.rs` (NEW accept loop) | service | streaming | `supervise.rs` accept loop l.640-660, `read_connection` | exact |
| `crates/minos-backend-krun/src/input_disk.rs` (v2) | utility | file-I/O | same file l.25-60 | exact |
| `crates/minos-backend-krun/src/launch_config.rs` (v2) | config | transform | same file | exact |
| `crates/minos-backend-krun/src/rootfs.rs` (+ /out, /scratch, streaming) | utility | file-I/O | same file | exact |
| `crates/minos-backend-krun/src/supervise.rs` (threads, classification) | service | streaming | same file l.596-760 | exact |
| `crates/minos-backend-krun/tests/wave0_{chunks,console_pipe,store_root}.rs` | test (ignored VM) | - | `tests/spike_boot.rs`, `spike_seatbelt.rs` | exact |
| `crates/minos-guest/src/{job,source,sink,stage,pack}.rs` | guest service | streaming / file-I/O | `crates/minos-guest/src/job.rs`, `report.rs`, `linux.rs` | role-match |
| `crates/minos-guest/src/linux.rs` (entry, mounts, trampoline, kill-all) | entrypoint | event-driven | same file l.60-110 | exact |
| `crates/minos-cli/src/main.rs` (clap builder) | controller | request-response | none in repo (3-line stub) | none |
| `crates/minos-cli/src/{env_build,host,locate,blob_source}.rs` | controller / adapter | subprocess + file-I/O | `minos-source/src/clean.rs` (subprocess), `minos-store/src/file/blobs.rs` | role-match |
| `crates/minos-core/tests/golden_wire.rs` (+ guest v2, lock, chunk goldens) | test | - | same file l.743-776 | exact |

## Pattern Assignments

### Wire types: `minos-core/src/{guest,lock,chunk}.rs`, `minos-vmm/src/config.rs` v2
**Analog:** `crates/minos-core/src/job.rs` (l.38-66 errors, l.151-188 Raw+try_from, l.312-330) and `crates/minos-vmm/src/config.rs`.

Raw + validated pair (config.rs l.120-133, 313-330):
```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVmmConfig { v: u32, kernel: RawKernel, boot: RawBoot, /* ... */ max_duration_ms: u64, machine: RawMachine }

pub fn parse(bytes: &[u8]) -> Result<VmmConfig, ConfigError> {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len > MAX_CONFIG_BYTES { return Err(ConfigError::TooLarge); }   // size cap BEFORE parse
    let raw: RawVmmConfig = serde_json::from_slice(bytes).map_err(ConfigError::Json)?;
    VmmConfig::try_from(raw)
}
impl TryFrom<RawVmmConfig> for VmmConfig {
    type Error = ConfigError;
    fn try_from(raw: RawVmmConfig) -> Result<Self, ConfigError> {
        if raw.v != CONFIG_VERSION { return Err(ConfigError::Version(raw.v)); }
```
job.rs uses `#[serde(try_from = "RawJobRequest", into = "RawJobRequest")]` on the validated type (l.188) with private fields and getter methods (config.rs l.234-305). Errors: `#[derive(Debug, thiserror::Error)]` with fixed messages, no input bytes (job.rs l.38-66). Tests in-module return `TestResult = Result<(), Box<dyn Error>>`, one test per rejected rule (config.rs l.523-958: `assert_unknown_field`, `config_rejects_*`, boundary pairs like 103/104-byte socket path).

### `minos-core/src/id.rs` BlobHasher
**Analog:** `IdHasher` l.43-93: newtype over `Sha256`, `#[must_use]` builder methods, `finish(self) -> [u8; 32]` via `finalize().into()`. BlobHasher should wrap `Sha256` the same way, with `update(&mut self, &[u8])` and `finish(self) -> BlobId` (plain digest, no domain, so it equals `BlobId::of`). Add a golden-vector test asserting equality with `BlobId::of` on the same bytes.

### `minos-store/src/file/blobs.rs` put_reader
**Analog:** same file, `put` l.32-58: ensure each dir and fsync its parent when created, `NamedTempFile::new_in(&shard)`, `write_all`, `sync_all`, `persist`, `fsync_dir(&shard)`. For streaming: hash with BlobHasher while copying (cap with `Read::take(max+1)`), then persist to the path derived from the final id (D-09 says a staging dir; rename into shard, fsync both). `get` l.60-99 shows the symlink_metadata + size-before-open + rehash pattern to reuse for `verified_path`. Errors map through `io_error("<fixed op>")`.

### `minos-sandbox/src/blob_source.rs`, preflight/finished/spec changes
**Analog:** `crates/minos-sandbox/src/backend.rs` (whole file, 102 lines): module doc stating blame split; `#[non_exhaustive] #[derive(Debug, thiserror::Error)]` enums with `Io { op: &'static str, #[source] source: io::Error }` and `Backend { what: &'static str }`; trait docs with `# Errors`; "never a trait object". Refusal (preflight.rs l.20-40): `#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, thiserror::Error)]`, payloads numbers/enums only, message ends with the fix ("; ask for fewer"). `Capabilities` l.96-120 holds `host_refusals: BTreeSet<Refusal>` via `const fn new` - extend with layers as a `BTreeSet` too. `Finished::new(cleared, ended, stream)` l.65 consumes `Cleared`; keep that when adding elapsed/console.

### `minos-backend-krun/src/backend.rs`, `supervise.rs`, `chunks.rs`
**Analog:** `supervise.rs::run` l.596-660:
```rust
let child = Command::new(program)
    .args(args)
    .env_clear()
    .stdin(Stdio::null())
    .stdout(stdout)
    .stderr(stderr)
    .process_group(0)
    .spawn()
    .map_err(|source| SuperviseError::Spawn { program: program_path.to_path_buf(), source })?;
let mut child = Reaper(child);   // drop guard kills the group
...
let status = loop {
    if connections.len() < launch.max_connections
        && let Some(stream) = accept_pending(&launch.listener, socket)? {
        let cap = launch.conn_cap(connections.len());
        connections.push(read_connection(&stream, cap, deadline, socket)?);
        continue;
    }
    if let Some(status) = child.0.try_wait().map_err(io_at("wait for", program_path))? { ... }
```
Also: absolute-program check, `host_deadline > max_duration` check, `Instant::checked_add` (no panics), handles via `try_clone` not reopening paths, `io_at("<op>", path)` error helper. `KrunBackend::launch` wraps this and returns `LaunchError`/`InfraError` from sandbox `backend.rs`; candidate faults go in `Ended`.

### `host.rs` probes, `minos-cli` env_build (docker buildx) subprocesses
**Analog:** `minos-source/src/clean.rs` `run_capped` l.196-215 (argv, `current_dir`, `env_clear()` + explicit `.env(...)` allowlist, `Stdio::null()` stdin, piped capped stdout, deadline) and `supervise.rs` `has_hypervisor_entitlement` l.439-480 (codesign, `env_clear`, parse XML), `sha256_file` l.411.

### `seatbelt.rs`
**Analog:** `crates/minos-backend-krun/tests/spike_seatbelt.rs` (argv through `/usr/bin/sandbox-exec`) and profile `crates/minos-vmm/minos-vmm.sb`; embed with `include_str!` and pin its SHA-256 in a unit test (golden pin style below).

### `jobdir.rs`
**Analog:** `supervise.rs` `short_job_dir` l.250 and `JobFiles` l.261-360 (private paths, getter per file, `File::create_new`). Owner lock: `std::fs::File::lock` as in `minos-store` ledger append; symlink checks via store's `symlink_metadata` + dev/inode pattern.

### `input_disk.rs`, `launch_config.rs`, `rootfs.rs` (v2)
Same files; keep their in-module test style (input_disk.rs l.76-141: exact-boundary, one-over-limit, sector padding tests).

### `minos-guest` files
**Analog:** `crates/minos-guest/src/linux.rs` l.60-110: `run() -> ExitCode`, `mount_at(...)` via rustix, `console_line(MARKER)`, `reap_orphans()` per step, any error -> console line then `power_off()`. Keep host-compilable codecs out of `linux.rs` (CLAUDE.md). `job.rs` (870 lines) and `report.rs` show the guest-side decode/encode against core types.

### Wave 0 ignored VM tests `tests/wave0_*.rs`
**Analog:** `crates/minos-backend-krun/tests/spike_boot.rs` / `spike_seatbelt.rs`: `#[ignore]`, env `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW`, run `--test-threads=1`, `TestResult` returns, no unwrap.

## Shared Patterns

### Golden wire file + SHA-256 pin
**Source:** `crates/minos-core/tests/golden_wire.rs` l.743-776. Apply to guest job v2, chunk header, `minos.lock`, VmmConfig v2, Seatbelt profile.
```rust
for (name, line) in lines { insta::assert_snapshot!(name, line); }
...
assert_eq!(sha256_hex(all.as_bytes()), "8020cfde...1039");  // computed outside code under test via awk | shasum
```
Adding goldens there changes the count (`assert_eq!(lines.len(), 24)`) and the pin; update the awk comment list.

### Errors separate blame
`backend.rs` `LaunchError`/`InfraError` (infra, retried) vs `Ended`/`OutputFault` in `finished.rs` (candidate). Chunk gaps/duplicates/over-cap are candidate faults (CLAUDE.md), never `InfraError`.

### Subprocesses
argv arrays, absolute program, `env_clear()` + allowlist, `stdin(Stdio::null())`, `process_group(0)` for VMs (supervise.rs l.633-640, clean.rs l.204-215).

### No panics
No `unwrap`/`expect`/indexing even in tests: `split_first().ok_or(...)`, `checked_add`, `u64::try_from(..).unwrap_or(u64::MAX)` (config.rs l.314 is the accepted saturating idiom), `.get()`. No `#[allow]`.

### Ordered output
`BTreeSet<Refusal>` in `Capabilities`; use `BTreeSet`/`BTreeMap` for layers, lock entries, and job dir sweeps.

## No Analog Found

| File | Role | Reason |
|---|---|---|
| `crates/minos-cli/src/main.rs` (clap builder API) | controller | CLI is a 3-line stub; use RESEARCH.md + clap builder (no derive) |
| `crates/minos-vmm/src/log.rs` | capped `log` backend | No logger exists yet (DIAG-01); only watchdog's console-cap logic is related |

## Metadata
**Search scope:** `crates/*/src`, `crates/*/tests`
**Files scanned:** ~25
**Date:** 2026-10-18
