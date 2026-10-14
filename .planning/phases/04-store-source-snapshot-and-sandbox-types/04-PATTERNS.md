# Phase 4: Store, Source Snapshot, and Sandbox Types - Pattern Map

**Mapped:** 2026-10-14
**Files analyzed:** 30 (three crates, workspace plumbing, tests)
**Analogs found:** 28 / 30

All excerpts obey the workspace `forbid` lints (no unwrap/expect/indexing, no `#[allow]`, including tests and `tests/ui`).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `Cargo.toml` (members + `minos-source` workspace dep) | config | n/a | `Cargo.toml` lines 5-16 | exact |
| `deny.toml` (`minos-source` wrapper; add to `minos-core` wrappers) | config | n/a | `deny.toml` lines 61-69 | exact |
| `crates/minos-source/Cargo.toml` | config | n/a | `crates/minos-store/Cargo.toml` | exact |
| `crates/minos-cli/Cargo.toml` (drop `ignore`, `regex`; add `minos-source`) | config | n/a | itself | exact |
| `crates/minos-{sandbox}/Cargo.toml` (add `base16ct`, dev `proptest`) | config | n/a | `crates/minos-store/Cargo.toml` | exact |
| `crates/minos-{store,sandbox,source}/clippy.toml` | config | n/a | `crates/minos-core/clippy.toml` | exact |
| `crates/minos-store/src/lib.rs` (Ledger, Blobs traits, StoreError) | service trait | CRUD | `crates/minos-core/src/ledger.rs` (Line API), `crates/minos-vmm/src/config.rs` (error enum) | role-match |
| `crates/minos-store/src/memory.rs` | service | CRUD | none in repo (BTreeMap in core state fold) | partial |
| `crates/minos-store/src/file/{mod,ledger,blobs}.rs` | service | file-I/O | `crates/minos-backend-krun/src/supervise.rs` (`JobFiles`, capped reads) | partial |
| `crates/minos-sandbox/src/role.rs` | model (typestate) | n/a | none | no analog |
| `crates/minos-sandbox/src/key.rs` (LaunchKey) | utility | transform | `crates/minos-core/src/id.rs` lines 43-93 (`IdHasher`), `ExperimentId::of` line 299 | exact |
| `crates/minos-sandbox/src/spec.rs` (SandboxSpec, Machine, seal) | model | transform | `crates/minos-vmm/src/config.rs` (range consts lines 17-39, validated ctor) | role-match |
| `crates/minos-sandbox/src/preflight.rs` | service | request-response | `crates/minos-vmm/src/preflight.rs` | role-match |
| `crates/minos-sandbox/src/backend.rs` | trait | request-response | none (generic trait, no objects) | partial |
| `crates/minos-sandbox/src/untrusted.rs` | model (wrapper) | transform | `crates/minos-backend-krun/src/supervise.rs` lines 770-830 (`GuestBytes`) | exact |
| `crates/minos-sandbox/src/output.rs` (raw tar reader) | parser | streaming/transform | `crates/minos-backend-krun/src/rootfs.rs` lines 286-520 (`HeaderGuard`, caps) | role-match |
| `crates/minos-source/src/{lib,walk,denylist,scan,pack}.rs` | service | file-I/O, batch | `crates/minos-backend-krun/src/rootfs.rs` (deterministic pack, consts with reasons) | role-match |
| `crates/minos-source/src/clean.rs` (git exec) | service | request-response (subprocess) | `crates/minos-backend-krun/src/supervise.rs` lines 440, 631 (`Command` + `env_clear`) | exact |
| `crates/minos-store/tests/{contract,file_ledger,file_blobs}.rs` | test | CRUD | `crates/minos-core/tests/prop_fold.rs` | role-match |
| `crates/minos-sandbox/tests/{spec,preflight,untrusted,output_tar}.rs` | test | transform | `crates/minos-core/tests/golden_wire.rs`, `prop_path.rs` | role-match |
| `crates/minos-sandbox/tests/compile_fail.rs` + `tests/ui/*.rs/.stderr` | test | n/a | none (first trybuild use) | no analog |
| `crates/minos-sandbox/tests/fixtures/output_tar/*.tar` | test fixture | n/a | golden SHA pins in `golden_wire.rs` | partial |
| `crates/minos-source/tests/{pack,scan,clean}.rs` | test | file-I/O | `crates/minos-core/tests/golden_ids.rs` (hex literal pins) | role-match |

## Pattern Assignments

### `crates/minos-sandbox/src/key.rs` (LaunchKey)

**Analog:** `crates/minos-core/src/id.rs` lines 43-93. `IdHasher` is public precisely for this ("the launch key in `minos-sandbox` hashes through it too", line 41-42).
```rust
pub fn new(domain: &'static str) -> Self      // NUL-terminated domain
pub fn digest(mut self, digest: &[u8; 32]) -> Self
pub fn u64(mut self, value: u64) -> Self
pub fn bytes(mut self, bytes: &[u8]) -> Self  // u64 BE length prefix
pub fn finish(self) -> [u8; 32]
```
Usage shape, `ExperimentId::of` (id.rs ~line 299):
```rust
Self::from_digest(IdHasher::new("minos/experiment/v1").digest(base.as_bytes()).bytes(diff.as_bytes()).finish())
```
LaunchKey: `IdHasher::new("minos/launch/v1").digest(exp.as_bytes()).u64(role_tag).u64(trial).finish()`, hex via `base16ct::lower::encode_string`. Pin with a hex literal golden vector (style of `crates/minos-core/tests/golden_ids.rs`; plain `assert_eq!`, never insta).

### `crates/minos-sandbox/src/untrusted.rs`

**Analog:** `crates/minos-backend-krun/src/supervise.rs` lines 770-830 (`GuestBytes`).
```rust
pub struct GuestBytes {
    bytes: Vec<u8>,
    overflowed: bool,
}
impl GuestBytes {
    fn capped(mut bytes: Vec<u8>, cap: u64) -> Self {
        let keep = usize::try_from(cap).unwrap_or(usize::MAX);
        let overflowed = bytes.len() > keep;
        bytes.truncate(keep);
        Self { bytes, overflowed }
    }
    #[must_use] pub fn len(&self) -> usize { self.bytes.len() }
    #[must_use] pub fn sha256_hex(&self) -> String { /* Sha256 + base16ct::lower */ }
}
```
Copy: private field, crate-private constructor, no derives at all (no `Debug`, `Clone`, `Display`, `Deref`). Exits delegate to core validators: `Scorecard::parse_json(bytes, &Limits)` (`crates/minos-core/src/metric.rs:276`), `LogExcerpt::from_bytes(bytes, max)` (`crates/minos-core/src/text.rs:192`). `into_scorecard` exists only on `Untrusted<.., Judge>` (compile-fail `candidate_scorecard.rs`).

### `crates/minos-sandbox/src/output.rs` (hostile tar reader)

**Analog:** `crates/minos-backend-krun/src/rootfs.rs`.
- Caps as module-top consts with doc reasons (lines 334-366): `const BLOCK_BYTES: usize = 512;`, `pub const PAX_HEADER_BYTES: u64 = 65_536;`.
- Bounded input: `rootfs_tar.take(limits.tar_bytes.saturating_add(1))` (line 293), over-cap detected by the `+1`.
- Overflow-safe conversions: `u64::try_from(x).unwrap_or(u64::MAX).saturating_add(1)` (lines 418-420).
- Error payload carries numbers, not hostile text (`ExtensionHeaderHit { what: &'static str, size, limit }`, lines 392-407).
Deviation per RESEARCH Pattern (line 116): use `archive.entries()?.raw(true)` and refuse every typeflag except `b'0'` and non-ustar magic, instead of a `HeaderGuard`. Paths go through `RelPath::try_from`. Map `OutputFault::reason()` to `Reason::OutputInvalid` / `Reason::OutputTooLarge` (`crates/minos-core/src/outcome.rs:96,98`). Never `unpack`/`unpack_in` (ban in clippy.toml).

### `crates/minos-sandbox/src/spec.rs` (SandboxSpec, Machine, LaunchPlan)

**Analog:** `crates/minos-vmm/src/config.rs`.
Range consts (lines 31-37), restated with a comment naming the source because sandbox cannot depend on vmm:
```rust
pub const MAX_DURATION_MS: u64 = 3_600_000;
pub const MAX_VCPUS: u8 = 8;
pub const MIN_MEMORY_MIB: u16 = 128;
pub const MAX_MEMORY_MIB: u16 = 16_384;
```
Error enum style (lines 42-60): `#[derive(Debug, thiserror::Error)]`, each variant doc-commented, `#[error(...)]` with consts interpolated, `#[source]` for wrapped errors, `&'static str` field names rather than input text. `SpecError::DurationOverCeiling` above `MAX_LAUNCH_MS`, never clamp (RESEARCH Pitfall, line 594). Private fields, `PhantomData<R>`, `seal()` the only `LaunchPlan` constructor.

### `crates/minos-sandbox/src/preflight.rs`

**Analog:** `crates/minos-vmm/src/preflight.rs` (fail-closed checks returning refusals). `Cleared<R>` has a private unit field so `Cleared { .. }` literal fails (ui `cleared_literal.rs`). Return `Result<Cleared<R>, Vec<Refusal>>`, refusals sorted (stable order).

### `crates/minos-store/src/lib.rs`, `memory.rs`, `file/*.rs`

**Analogs:** `crates/minos-core/src/ledger.rs` for the wire contract: `Line::new(seq, &run, record)` (line 723) computes the key; `Line::verify` must run on every read; duplicate `EventKey` append is a no-op. Errors: copy the `config.rs` thiserror shape, split infra (`Io`, `Lock`) from corruption (`BadLine { seq }`, `BlobMismatch`). Memory impl: `Mutex<BTreeMap<..>>` (never HashMap). File impl: capped reads like `GuestBytes::capped`; temp-then-rename via `tempfile::NamedTempFile::persist` in same dir, `sync_all`, dir fsync; `std::fs::File::lock` per run. `BlobId::of(&[u8])` (id.rs:206) only; no `from_digest` (private).

### `crates/minos-source/src/clean.rs` (git exec)

**Analog:** `crates/minos-backend-krun/src/supervise.rs` lines 440-443 and 631-633:
```rust
let child = Command::new(program)
    .env_clear()
    ...
```
Argv array only, env allowlist re-added explicitly, deadline polling with `try_wait`, capped stdout read.

### `crates/minos-source/src/pack.rs`, `walk.rs`, `denylist.rs`, `scan.rs`

**Analog:** `crates/minos-backend-krun/src/rootfs.rs` (deterministic image, `PackReport` counters with `const fn` getters lines 100-142, consts with reasons). Hand-built `Header::new_ustar()` with mtime 0, uid/gid 0, mode 0o644/0o755, `Builder::append`; ban `append_data`/`append_path`/`append_dir_all` and `HeaderMode::Deterministic` in clippy.toml. Collect into `BTreeSet<RelPath>` after walking. Scan test patterns must be assembled at runtime (no literal secrets in source; RESEARCH line 585).

### `crates/*/clippy.toml`

**Analog:** `crates/minos-core/clippy.toml` (whole file). Header comment ("Replaces ... proven to fire once ... re-prove any entry you change"), and keep the `disallowed-types` HashMap/HashSet entries verbatim:
```toml
disallowed-types = [
  { path = "std::collections::HashMap",  reason = "order must be stable: use BTreeMap" },
  { path = "std::collections::HashSet",  reason = "order must be stable: use BTreeSet" },
]
```
Add crate-specific `disallowed-methods` (`tar::Archive::unpack`, `tar::Entry::unpack_in`, `std::fs::write` in store, builder conveniences in source). Each entry must be proven to fire once.

### `crates/minos-source/Cargo.toml`, `Cargo.toml`, `deny.toml`

**Analog:** `crates/minos-store/Cargo.toml` (package `*.workspace = true`, `[lints] workspace = true`, deps `*.workspace = true`). Root `Cargo.toml` members lines 5-16: insert `"crates/minos-source"`. `deny.toml` line 65: append `"minos-source"` to the `minos-core` wrappers; add after line 67:
```toml
{ crate = "minos-source", wrappers = ["minos-cli"], reason = "layering: only the CLI packs a source tree" },
```
Move `ignore` and `regex` out of `crates/minos-cli/Cargo.toml` (`unused = "deny"`).

## Shared Patterns

### Golden wire / hash pins
**Source:** `crates/minos-core/tests/golden_wire.rs` lines 1-40. Module doc explaining insta `update: no` + `INSTA_UPDATE=always` creation; `type TestResult = Result<(), Box<dyn Error>>;` so tests use `?` instead of unwrap; fixtures from public constructors only; one SHA-256 literal pinning exact bytes (`sha2::{Digest, Sha256}`). Apply to: LaunchKey vectors, tar fixtures, packed tarball determinism, ledger JSONL bytes.

### Errors separate blame
**Source:** `crates/minos-vmm/src/config.rs` lines 42-60. Apply to `StoreError`, `SpecError`, `OutputFault`, `SnapshotError`, `LaunchError`/`InfraError`. Candidate faults map to `Reason`; infra errors are retried.

### Constants with reasons at module top
**Source:** `crates/minos-vmm/src/config.rs` lines 17-39, `rootfs.rs` 334-366. Apply to snapshot caps (D-05), output caps, ledger line caps, denylist.

### Property tests
**Source:** `crates/minos-core/tests/prop_fold.rs`, `prop_path.rs` (commit `*.proptest-regressions`). Apply to store contract (both impls), output tar reader never-panic strategy (D-16).

## No Analog Found

| File | Role | Reason |
|---|---|---|
| `crates/minos-sandbox/src/role.rs` | typestate | No sealed-trait pattern in repo yet; use RESEARCH `mod sealed` sketch |
| `crates/minos-sandbox/tests/compile_fail.rs` + `tests/ui/` | trybuild | First trybuild use; follow RESEARCH lines 598 and 782 (commit `.stderr`, never set `TRYBUILD` in gate, one pass case) |

## Metadata

**Analog search scope:** `crates/minos-core/{src,tests}`, `crates/minos-backend-krun/src`, `crates/minos-vmm/src`, root `Cargo.toml`, `deny.toml`, crate `Cargo.toml`/`clippy.toml`
**Files scanned:** ~20
**Pattern extraction date:** 2026-10-14
