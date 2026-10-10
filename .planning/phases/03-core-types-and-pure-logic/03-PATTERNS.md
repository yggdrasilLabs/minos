# Phase 3: Core Types and Pure Logic - Pattern Map

**Mapped:** 2026-10-10
**Files analyzed:** 26 (module layout from 03-RESEARCH.md "Recommended Module Layout")
**Analogs found:** 18 / 26 (the rest are pure domain logic with no in-repo analog; use RESEARCH.md Patterns 2-13)

`crates/minos-core/` today: `Cargo.toml` (all deps already declared: serde, serde_json, toml, thiserror, sha2, base16ct, diffy, rand_chacha, rand_core; dev: proptest, insta), `clippy.toml` (do not weaken), `src/lib.rs` (one doc line).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/minos-core/src/lib.rs` (modify) | config/module root | n/a | existing one-line doc | n/a |
| `src/limits.rs` | config (constants) | n/a | `crates/minos-guest/src/job.rs` lines 17-43 | exact |
| `src/id.rs` | model + utility (hashing) | transform | `crates/minos-backend-krun/src/initramfs.rs` 117-126 (sha256 + base16ct); `crates/minos-vmm/src/config.rs` 455-467 (hex decode) | role-match |
| `src/path.rs` (`RelPath`) | model (validated) | transform | `crates/minos-vmm/src/config.rs` 436-453 (path normality walk) | role-match |
| `src/metric.rs` (`Metric`, `Scorecard`, `RawScorecard`) | model | transform | `job.rs` RawJob/Job 155-213 | exact (shape) |
| `src/outcome.rs` | model (enum) | n/a | `job.rs` RawStep/Step 220-275 | role-match |
| `src/manifest.rs` (`RawManifest` -> `Manifest`) | model (validated, TOML) | transform | `job.rs` 155-213, 286-346; `config.rs` | exact |
| `src/diff/mod.rs`, `apply.rs`, `compose.rs` | utility (parser/applier) | transform | none (diffy-based) | no analog |
| `src/experiment.rs` | model | n/a | `job.rs` Job accessors 172-191 | role-match |
| `src/usage.rs` | model | n/a | `job.rs` | role-match |
| `src/money.rs` | model + utility | transform | none | no analog |
| `src/ledger.rs` (`Record`, `Event`) | model (wire format) | event-driven | `job.rs` tagged RawStep -> Step 220-275 | role-match |
| `src/state.rs` (fold) | service (pure) | event-driven | none | no analog |
| `src/policy.rs` | service (pure) | batch | none | no analog |
| `src/route.rs` | service (pure) | request-response | none | no analog |
| `src/sweep.rs` | utility | transform | none | no analog |
| `src/job.rs` (`JobRequest`, `JobResult`) | model (wire format) | request-response | `crates/minos-guest/src/job.rs` | exact |
| `src/text.rs` (`SafeText`) | utility | transform | none (byte-class matching in `config.rs` 457-460 is the nearest idiom) | partial |
| `src/report.rs` | service (pure) | transform | none | no analog |
| `src/error.rs` or per-module error enums | model (errors) | n/a | `job.rs` 45-105 (`JobError`, `InvalidField`) | exact |
| `tests/golden_ids.rs` | test | n/a | `initramfs.rs` 155-163 (`initramfs_golden`) | exact |
| `tests/golden_wire.rs` | test (insta + byte pin) | n/a | `initramfs.rs` golden + RESEARCH "insta snapshot plus byte pin" | role-match (no insta in repo yet) |
| `tests/prop_*.rs` | test (proptest) | n/a | none in repo | no analog (use RESEARCH example) |
| `tests/manifest.rs`, `tests/diff.rs` etc. | test | n/a | `config.rs` tests 520-616 | exact |
| `tests/fixtures/*` | fixtures | file-I/O (include_str!) | `crates/minos-backend-krun/tests/fixtures/` | role-match |
| `.config/insta.yaml` (workspace root) | config | n/a | none | no analog (RESEARCH gives exact content) |

## Pattern Assignments

### `src/manifest.rs`, `src/metric.rs`, `src/job.rs`, every `Raw*` -> validated type

**Analog:** `crates/minos-guest/src/job.rs`

**Raw mirror + validated type** (lines 155-170):
```rust
/// The job JSON as written, before range checks.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJob {
    v: u32,
    vsock_port: u32,
    steps: Vec<RawStep>,
    finish: Finish,
}

/// A validated job. If a value of this type exists, every field is within its limits.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawJob")]
pub struct Job {
    vsock_port: u32,
    steps: Vec<Step>,
    finish: Finish,
}
```

**Accessors, no setters** (lines 172-191): `#[must_use] pub const fn vsock_port(&self) -> u32`, slices returned as `&[T]`.

**Constructor = `TryFrom<Raw>`; nested types validate via their own `try_from`** (lines 193-213):
```rust
impl TryFrom<RawJob> for Job {
    type Error = InvalidField;
    fn try_from(raw: RawJob) -> Result<Self, Self::Error> {
        if raw.v != PROTOCOL_VERSION {
            return Err(InvalidField::new("v", "1"));
        }
        let vsock_port = check_port("vsock_port", raw.vsock_port)?;
        if !(MIN_STEPS..=MAX_STEPS).contains(&raw.steps.len()) {
            return Err(InvalidField::new("steps", "1 to 64 entries"));
        }
        let steps = raw.steps.into_iter().map(Step::try_from).collect::<Result<Vec<_>, _>>()?;
        Ok(Self { vsock_port, steps, finish: raw.finish })
    }
}
```
Field errors use the dotted path (`"steps.connect.targets"`, lines 319-339). For core, use `BTreeMap`/`BTreeSet` where job.rs uses `Vec` if order matters; add `Serialize` (`into = "Raw*"` or hand-written) since core types round-trip through the ledger (job.rs is deserialize-only).

### `src/limits.rs`

**Analog:** `job.rs` lines 17-43. Every cap is a documented `pub const` with its unit and reason:
```rust
/// Longest timeout of a `run` step, in milliseconds: one hour, the launcher's own `max_duration_ms` ceiling.
pub const MAX_RUN_TIMEOUT_MS: u64 = 3_600_000;
```

### Error enums (all modules)

**Analog:** `job.rs` lines 45-105.
```rust
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("job JSON: {0}")]
    Json(#[source] serde_json::Error),
    #[error("job field {0}")]
    Invalid(#[from] InvalidField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{field} must be {limit}")]
pub struct InvalidField { field: &'static str, limit: &'static str }
impl InvalidField {
    const fn new(field: &'static str, limit: &'static str) -> Self { Self { field, limit } }
    #[must_use] pub const fn field(&self) -> &'static str { self.field }
}
```
Private fields, `const fn` constructor private to the crate, `&'static str` messages (RESEARCH Pitfall 10: never put hostile text unescaped in an error; carry `SafeText` or a line number instead).

### `src/ledger.rs`, `src/outcome.rs` (tagged enums)

**Analog:** `job.rs` lines 220-275: a `RawStep` tagged enum whose variants hold `Raw*` structs each with `deny_unknown_fields`, mapped to `Step` via `#[serde(try_from = "RawStep")]` and a per-variant `try_from`. Put `deny_unknown_fields` on variant structs, not on the enum (RESEARCH, internally tagged enums). Ledger uses `#[serde(tag = "v")]` per CLAUDE.md.

### `src/id.rs`

**Analogs:** `crates/minos-backend-krun/src/initramfs.rs` lines 117-126 and `crates/minos-vmm/src/config.rs` lines 455-467.
```rust
use sha2::{Digest, Sha256};
let digest: [u8; 32] = Sha256::digest(bytes).into();
base16ct::lower::encode_string(&digest)
```
Hex parse (strict lowercase, 64 chars):
```rust
fn sha256(field: &'static str, text: &str) -> Result<[u8; 32], ConfigError> {
    let lower_hex = text.len() == SHA256_HEX_LEN
        && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    let mut digest = [0_u8; 32];
    if lower_hex && base16ct::lower::decode(text, &mut digest).is_ok() { Ok(digest) }
    else { Err(ConfigError::Sha256 { field }) }
}
```
Domain-separated, length-prefixed builder: RESEARCH Pattern 2 (no analog).

### `src/path.rs` (`RelPath`)

**Analog:** `crates/minos-vmm/src/config.rs` lines 436-453. Walk the raw bytes as well as `Path::components`, because `components` silently drops `.`, empty, and trailing segments:
```rust
let plain_segments = rest.split(|byte| *byte == b'/')
    .all(|segment| !segment.is_empty() && segment != b"." && segment != b"..");
```
Invert the absolute check (RelPath must NOT start with `/`). Grammar per RESEARCH Pattern 3. Note `std::path` use is fine; no `fs`.

### Tests (all `tests/*.rs` and `#[cfg(test)]` modules)

**Analog:** `crates/minos-vmm/src/config.rs` tests 520-616 and `initramfs.rs` 113-163.
```rust
type TestResult = Result<(), Box<dyn Error>>;

fn rejected(text: &str) -> Result<ConfigError, Box<dyn Error>> {
    match parse(text.as_bytes()) {
        Ok(_) => Err("the config was accepted".into()),
        Err(error) => Ok(error),
    }
}

#[test]
fn config_serde_try_from_runs_the_same_rules() -> TestResult {
    let config: VmmConfig = serde_json::from_str(E1)?;
    assert_eq!(config.vsock_port(), 1024);
    let refused = serde_json::from_str::<VmmConfig>(&edit(E1, r#""v":1"#, r#""v":2"#)?);
    assert!(refused.is_err());
    Ok(())
}
```
Error assertions use `matches!` with guards (lines 528-535): `assert!(matches!(error, ConfigError::OutOfRange { field, .. } if field == expected), "...{error}")`. Unknown-field test asserts message contains ``unknown field `x` `` (546-555). Indexing via `.get(a..b)` compared to `Some(..)` (initramfs.rs 136-151), never `[i]`.

**Golden hash** (`initramfs.rs` 155-163):
```rust
#[test]
fn initramfs_golden() -> TestResult {
    let image = initramfs_image(b"hello")?;
    assert_eq!(sha256_hex(&image), "6b2629df82fc94f9798c3853a508935c6a448a7e48fabd58e4e730a99148ad1a");
    Ok(())
}
```
For core ID vectors, compute the literal outside the code with `shasum` and record the recipe in a comment (RESEARCH "Golden hash vector computed outside the code under test").

**Core-specific:** fixtures load with `include_str!`/`include_bytes!` only; `std::fs`, `println!`, `HashMap` are banned in core tests too by `clippy.toml`.

## Shared Patterns

### Lint discipline (apply to every file)
- No `unwrap`/`expect`/indexing (forbid). Use `?`, `.get()`, `ok_or("msg")?`, `.err().ok_or(...)?`.
- No `#[allow(` anywhere (C-02 grep).
- `BTreeMap`/`BTreeSet` only (`clippy.toml` bans `HashMap`/`HashSet`).
- Time, randomness seeds, file contents arrive as arguments. `clippy.toml` bans clock, env, fs, stdin, Command, sockets, print macros.
- Doc comments on every public item (`cargo doc` enforces rustdoc lints); module `//!` header describing the wire format and its caps, as in `job.rs` lines 1-6.

### Validation
**Source:** `job.rs` 155-213. Apply to `Manifest`, `Prices`, `Scorecard`, `JobRequest`/`JobResult`, `RelPath`, every ID, `Metric`, event payloads.

### Errors
**Source:** `job.rs` 45-105. `thiserror` enums, private-field struct errors with accessors, dotted field paths.

### Golden tests
**Source:** `initramfs.rs` 120-163 (`sha256_hex` helper + hex literal). insta is new to the repo: add `.config/insta.yaml` with `behavior: update: no` and pair every snapshot with a byte-hash pin (RESEARCH Code Examples).

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/diff/{mod,apply,compose}.rs` | utility | transform | No diffy use yet; follow RESEARCH Patterns 4-5 |
| `src/money.rs` | utility | transform | No fixed-point code; RESEARCH Pattern 11 |
| `src/state.rs` | service | event-driven | No fold exists; RESEARCH Pattern 7 |
| `src/policy.rs`, `src/route.rs` | service | batch | RESEARCH Pattern 8 |
| `src/sweep.rs` | utility | transform | No rand_chacha use; RESEARCH Pattern 9 |
| `src/text.rs` | utility | transform | Allowlist renderer; RESEARCH Pattern 12 |
| `src/report.rs` | service | transform | RESEARCH Pattern 13 |
| `tests/prop_*.rs` | test | n/a | No proptest in repo; RESEARCH permutation example |

## Metadata

**Analog search scope:** `crates/minos-core`, `crates/minos-guest/src`, `crates/minos-vmm/src`, `crates/minos-backend-krun/src` and `tests`
**Files scanned:** 8
**Pattern extraction date:** 2026-10-10
