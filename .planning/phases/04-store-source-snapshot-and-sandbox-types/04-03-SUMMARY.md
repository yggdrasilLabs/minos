---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 03
subsystem: sandbox
tags: [minos-sandbox, typestate, sealed-trait, launch-key, preflight, golden-vectors]

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    provides: "04-01: minos-sandbox dependency edges (base16ct, proptest) and the crate clippy.toml bans"
  - phase: 03-core-types-and-pure-logic
    provides: "IdHasher, ExperimentId, JudgeVersion, ImageDigest, BlobId, JobRequest, JobBudget, Arch, Isolation"
provides:
  - "Sealed Role trait with uninhabited Candidate and Judge markers; RoleKind tags 1 and 2 (3 reserved for Agent)"
  - "LaunchKey over exp, role tag, judge version, and trial, pinned by two shasum-computed golden vectors"
  - "SandboxSpec<R> built from a JobRequest, Judge-only secrets, seal as the only LaunchPlan constructor"
  - "Hard maximum duration = budgets + 30 s boot allowance, refused (never clamped) over 3,600,000 ms"
  - "SpecError documented as a configuration error with its Phase 7 and Phase 8 handlers"
  - "Capabilities, Refusal (12 variants), Cleared<R> owning its plan, and fail-closed preflight"
affects: [04-08, phase-5-krun-backend, phase-7-runner, phase-8-cli]

tech-stack:
  added: []
  patterns:
    - "Role typestate: PhantomData<fn() -> R> marker over a sealed trait; role-specific methods live in impl SandboxSpec<Judge>"
    - "Preflight collects refusals into a BTreeSet and returns them as a sorted Vec, so the order is stable"

key-files:
  created:
    - crates/minos-sandbox/src/role.rs
    - crates/minos-sandbox/src/key.rs
    - crates/minos-sandbox/src/spec.rs
    - crates/minos-sandbox/src/preflight.rs
    - crates/minos-sandbox/tests/spec.rs
    - crates/minos-sandbox/tests/preflight.rs
  modified:
    - crates/minos-sandbox/src/lib.rs

key-decisions:
  - "04-03: preflight takes the backend's Capabilities, not the backend, so the sandbox types need no Backend trait to clear a plan; the runner calls preflight(&backend.capabilities(), plan)"
  - "04-03: InputDisk and JudgeData are publicly constructible in Phase 4; Phase 5, which owns the input-disk format, may restrict InputDisk; preflight refuses a Judge plan whose judge data equals its input disk"
  - "04-03: LaunchKey Debug prints LaunchKey(<hex>), matching the core ID convention; Display prints the bare 64-character hex"

patterns-established:
  - "Machine and duration bounds restate crates/minos-vmm/src/config.rs constants with a comment naming the source, because minos-sandbox cannot depend on the launcher binary"

requirements-completed: [SBX-01, SBX-03, SBX-06]

duration: 17min
completed: 2026-10-14
status: complete
---

# Phase 4 Plan 03: Sandbox roles, launch key, sealed plans, and preflight Summary

**Sealed `Role` typestate (Candidate, Judge), a `LaunchKey` pinned by shasum-computed vectors, `SandboxSpec<R>` whose only way to judge data is `SandboxSpec<Judge>::secrets` and whose only exit is `seal`, and a fail-closed `preflight` that is the only constructor of a plan-owning `Cleared<R>`.**

## Performance

- **Duration:** about 17 min
- **Started:** 2026-10-14T20:23:30Z
- **Completed:** 2026-10-14T21:40:33Z
- **Tasks:** 3 (each RED then GREEN)
- **Files modified:** 7 (6 created, 1 modified)

## Accomplishments

- `role.rs`: `Role: sealed::Sealed + 'static` with `const KIND: RoleKind`. It is implemented only for the uninhabited `Candidate` and `Judge`. `RoleKind::tag` uses explicit match arms (Candidate 1, Judge 2), and its doc reserves 3 for Agent.
- `key.rs`: `LaunchKey::of::<R>(exp, judge, trial)` hashes `IdHasher::new("minos/launch/v1").digest(exp).u64(tag).digest(judge).u64(trial)`. Its golden vectors are Candidate `4747473e...71534fb` and Judge `a208611f...31169d05`. I recomputed both with shasum before writing the tests. A proptest shows that changing any single field changes the key.
- `spec.rs`: `Machine` allows 1 to 8 vCPUs and 128 to 16,384 MiB. `InputDisk` and `JudgeData` are blob-ID newtypes. `SandboxSpec<R>::new` reads the key and budgets from a `JobRequest`. Its hard maximum is build + run + 30 s for a Candidate and judge + 30 s for a Judge, and it refuses anything over 3,600,000 ms with `DurationOverCeiling { ms }` rather than clamping. `seal` moves every field into `LaunchPlan<R>`, which has private fields and read-only accessors and implements neither Clone nor Default. No type has a network field or method.
- `preflight.rs`: `Refusal` has the seven plan checks plus the five KRUN-06 host facts. `Capabilities` has private fields and no Default. `Cleared<R>` has no derive. `preflight` collects every refusal into a `BTreeSet` and returns `Ok(Cleared)` only when the set is empty.

## Task Commits

1. **Task 1: Sealed roles and the launch key**: `2e89677` (test, RED), `cde0dad` (feat, GREEN)
2. **Task 2: SandboxSpec, seal, and LaunchPlan**: `b26b120` (test, RED), `5b5ff7c` (feat, GREEN)
3. **Task 3: Capabilities, Refusal, Cleared, and preflight**: `2932664` (test, RED), `910b071` (feat, GREEN)

## Files Created/Modified

- `crates/minos-sandbox/src/role.rs`: the sealed `Role` trait, the `Candidate` and `Judge` markers, `RoleKind`
- `crates/minos-sandbox/src/key.rs`: `LaunchKey` and its private domain constant, with a unit test that the domain has no NUL
- `crates/minos-sandbox/src/spec.rs`: bounds constants, `Machine`, `InputDisk`, `JudgeData`, `SandboxSpec`, `LaunchPlan`, `SpecError`
- `crates/minos-sandbox/src/preflight.rs`: `Refusal`, `Capabilities`, `Cleared`, `preflight`
- `crates/minos-sandbox/src/lib.rs`: crate doc, modules, root re-exports
- `crates/minos-sandbox/tests/spec.rs`: golden vectors, role tags, hex printing, key sensitivity property, machine and duration bounds, seal and secrets
- `crates/minos-sandbox/tests/preflight.rs`: the admitted path, each refusal alone, host refusals, and sorted unique multi-refusal

## Decisions Made

- Preflight takes `&Capabilities` rather than a backend, as the plan recorded, so the sandbox types need no `Backend` trait.
- `LaunchKey`'s `Debug` output wraps the hex as `LaunchKey(<hex>)`, the same convention as the core `digest_id!` IDs. The test checks that the Debug output contains the hex and that Display equals it.
- `LaunchPlan` and `SandboxSpec` derive `Debug` only. It prints IDs and bounds, never bytes. `Cleared` derives nothing.

## Deviations from Plan

### Acceptance grep vs rustfmt

- **Found during:** Task 3
- **Issue:** One acceptance criterion greps for the `preflight` signature on a single line. The signature is 105 characters, and rustfmt (default `max_width` 100, `style_edition = "2024"`) wraps it across four lines. `cargo fmt --all --check` is a gate command.
- **Action:** I kept the rustfmt layout and did not add `#[rustfmt::skip]`. The signature tokens are exactly as planned: `pub fn preflight<R: Role>(host: &Capabilities, plan: LaunchPlan<R>) -> Result<Cleared<R>, Vec<Refusal>>` (crates/minos-sandbox/src/preflight.rs line 179). A single-line grep misses it, so a multi-line check is needed.

### Clippy `duration_suboptimal_units` in a test

- **Found during:** Task 2
- **Issue:** `Duration::from_millis(210_000)` in `tests/spec.rs` failed clippy under `-D warnings`.
- **Fix:** Changed it to `Duration::from_secs(210)`. The assertion is the same.

**Total deviations:** 2 (one formatting conflict with an acceptance grep, one lint fix in a test). **Impact:** none on behavior or invariants.

## Issues Encountered

None. `cargo test --workspace` passed on the first run with 763 passed, 0 failed, and 25 ignored. The known flaky backend-krun test did not fail.

## Verification

- `cargo fmt --all --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace`: 763 passed, 0 failed, 25 ignored
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`
- `cargo doc --workspace --no-deps`: exit 0
- `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0
- C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: prints nothing
- `grep -rinE 'fn [a-z_]*network' crates/minos-sandbox/src`: prints nothing
- `grep -B4 'pub struct Cleared<R: Role>' ... | grep -c derive`: 0
- No workspace member was added, so C-01 is unchanged from 04-01

## Threat Mitigations

- T-04-03-01: `secrets` exists only in `impl SandboxSpec<Judge>`, and `Role` is sealed. The compile-fail proof comes in 04-08.
- T-04-03-02: No method or field names a network. The grep above shows this.
- T-04-03-03: `Cleared` owns its plan, has a private field, and derives nothing. `preflight` is its only constructor.
- T-04-03-04: `preflight` refuses a Judge plan whose judge data equals its input disk (`JudgeDataIsInput`, tested).
- T-04-03-05: The duration is computed from budgets plus the boot allowance and refused over the ceiling (tested at 3,630,000 and accepted at exactly 3,600,000).
- T-04-03-06: The key hashes the judge version for every role. The golden vectors and the sensitivity property pin this.

## Next Phase Readiness

04-08 can add the `Backend` trait (`capabilities() -> Capabilities`, `launch(Cleared<R>)`) and the trybuild cases against these shapes. 04-08 Task 3 records the `SpecError` hand-off in CLAUDE.md.

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/src/role.rs, key.rs, spec.rs, preflight.rs, tests/spec.rs, tests/preflight.rs
- FOUND: commits 2e89677, cde0dad, b26b120, 5b5ff7c, 2932664, 910b071
