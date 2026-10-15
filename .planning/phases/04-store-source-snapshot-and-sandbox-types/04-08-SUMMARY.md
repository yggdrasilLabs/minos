---
phase: 04-store-source-snapshot-and-sandbox-types
plan: 08
subsystem: sandbox
tags: [minos-sandbox, backend, trybuild, compile-fail, judge-integrity, isolation]
status: complete

requires:
  - phase: 04-store-source-snapshot-and-sandbox-types
    plan: 03
    provides: sealed Role, Candidate, Judge, LaunchKey, SandboxSpec, secrets, seal, LaunchPlan, Capabilities, Refusal, Cleared, preflight
  - phase: 04-store-source-snapshot-and-sandbox-types
    plan: 06
    provides: Untrusted, GuestOutput, OutputFile, Finished, Ended, CrashCause, the output reader, the role-typed exits
provides:
  - Backend trait (type Running<R: Role>: RunningVm<R>; capabilities; launch(ok: Cleared<R>))
  - RunningVm<R> trait (wait(self) -> Result<Finished<R>, InfraError>)
  - LaunchError and InfraError (Io, Backend), both infrastructure failures the runner retries
  - trybuild compile-fail suite (12 cases) plus one pass case running the whole judge flow
  - CLAUDE.md minos-sandbox, Local backends, and Testing sections matching the shipped API
affects: [phase-5-krun-backend, phase-7-runner, phase-8-cli]

tech-stack:
  added: []
  patterns:
    - "Backend as a trait with a generic associated type Running<R: Role>, so one backend type returns a role-typed running VM and the runner stays generic over B: Backend with no trait object"
    - "Compile-fail probes take every needed value as a function parameter and coerce the probe to a function pointer in main, so nothing is constructed and no dead-code warning joins the stderr"
    - "trybuild snapshots generated once with TRYBUILD=overwrite, reviewed for exactly one ^error line and no warning, then verified with TRYBUILD unset"

key-files:
  created:
    - crates/minos-sandbox/src/backend.rs
    - crates/minos-sandbox/tests/backend.rs
    - crates/minos-sandbox/tests/compile_fail.rs
    - crates/minos-sandbox/tests/ui/fail/*.rs and *.stderr (12 cases)
    - crates/minos-sandbox/tests/ui/pass/judge_flow.rs
  modified:
    - crates/minos-sandbox/src/lib.rs
    - CLAUDE.md

key-decisions:
  - "04-08: Backend has exactly two methods, capabilities() and launch(ok: Cleared<R>), plus the GAT Running<R: Role>: RunningVm<R>; RunningVm::wait(self) returns Result<Finished<R>, InfraError>. A candidate fault is a value inside Finished, never an Err."
  - "04-08: Backend is not sealed (unlike Role), because backend crates such as minos-backend-krun must implement it; the security comes from Cleared, which only preflight mints and which launch takes by value."
  - "04-08: the Backend trait takes no blob store (04-RESEARCH Open Question 7); how a backend receives blob bytes or paths is a Phase 5 decision."
  - "04-08: the compile-fail suite holds exactly the twelve planned cases. The two extra cases 04-06 suggested (Untrusted::new from outside the crate, Finished::new with a Cleared of the wrong role) were not added, because the plan's acceptance criteria fix the count at twelve; both are candidates for a follow-up."

patterns-established:
  - "Generic runner shape: run<B: Backend, R: Role>(backend, plan) = preflight(&backend.capabilities(), plan) then backend.launch(cleared)?.wait()?"

requirements-completed: [SBX-01, SBX-02, SBX-03, SBX-04, SBX-06]

duration: 6min
completed: 2026-10-15
---

# Phase 4 Plan 08: Backend trait and sandbox compile-fail suite Summary

**A two-method `Backend` trait with a role-generic `Running<R>`, whose `launch` takes only a `Cleared<R>` and whose `wait` returns `Finished<R>`. Twelve reviewed trybuild compile-fail cases and one runnable judge-flow pass case back it, and the CLAUDE.md sandbox signatures now match what shipped.**

## Performance

- **Duration:** about 6 min (measured from plan start to the last task commit)
- **Started:** 2026-10-15T14:52:59Z
- **Completed:** 2026-10-15T14:58:28Z
- **Tasks:** 3
- **Files modified:** 30 (27 created, 2 modified, plus this summary)

## Accomplishments

- `crates/minos-sandbox/src/backend.rs`: `Backend { type Running<R: Role>: RunningVm<R>; fn capabilities(&self) -> Capabilities; fn launch<R: Role>(&self, ok: Cleared<R>) -> Result<Self::Running<R>, LaunchError>; }` and `RunningVm<R> { fn wait(self) -> Result<Finished<R>, InfraError>; }`. Both error enums are `#[non_exhaustive]` thiserror enums with `Io { op, source }` and `Backend { what }`, documented as retried infrastructure failures and never an `Outcome`. The docs state the contract: the idempotency token comes from `ok.plan().key()`, `ok.plan().max_duration()` is enforced on every launch, and dropping the running value destroys the VM but nothing depends on that drop.
- `lib.rs` re-exports the four new items, and its crate doc now walks the full flow: spec, seal, preflight, launch, wait, into_output, then the exits.
- `tests/backend.rs`: an in-file `FakeBackend` that records each launch's key and hard maximum under a `Mutex` (a poisoned lock becomes an error). Six tests cover the judge and candidate flows end to end, that a refused plan never launches, that the launch key is the idempotency token, that `TimedOut` is a host fact with readable output, and that every launch carries the plan's key and hard limit (210 s for a Candidate, 2 min for a Judge).
- `tests/compile_fail.rs` with twelve fail cases and one pass case. Every `.stderr` holds exactly one `error` line and no warning.
- CLAUDE.md: the minos-sandbox sketch, the spec paragraph (IDs only, hard maximum, the `SpecError` hand-off to the Phase 7 runner and Phase 8 `minos check`, the `minos/launch/v1` key, preflight as the only `Cleared<R>` constructor), the role-typed `Untrusted` exits, the `Backend`/`RunningVm`/`Finished<R>` sentence, the synchronous preflight and launch signatures under Local backends, and the extended compile-fail bullet under Testing.

## The first error line of each .stderr

| Case | First `error` line |
|---|---|
| candidate_network | `error[E0599]: no method named `network` found for struct `SandboxSpec<R>` in the current scope` |
| candidate_scorecard | `error[E0599]: no method named `into_scorecard` found for struct `Untrusted<GuestOutput<minos_sandbox::Candidate>>` in the current scope` (note: the method was found for `Untrusted<GuestOutput<minos_sandbox::Judge>>`) |
| candidate_secrets | `error[E0599]: no method named `secrets` found for struct `SandboxSpec<minos_sandbox::Candidate>` in the current scope` (note: the method was found for `SandboxSpec<minos_sandbox::Judge>`) |
| cleared_clone | `error[E0599]: no method named `clone` found for struct `Cleared<R>` in the current scope` |
| cleared_literal | `error[E0451]: field `plan` of struct `Cleared` is private` |
| judge_network | `error[E0599]: no method named `network` found for struct `SandboxSpec<R>` in the current scope` |
| launch_bare_plan | `error[E0308]: mismatched types` (expected `Cleared<Candidate>`, found `LaunchPlan<Candidate>`) |
| launch_plan_literal | `error[E0451]: fields `key`, `root`, `input`, `budget`, `machine`, `arch`, `max_duration_ms`, `judge_data` and `role` of struct `LaunchPlan` are private` |
| untrusted_debug | `error[E0277]: `Untrusted<GuestOutput<minos_sandbox::Judge>>` doesn't implement `Debug`` |
| untrusted_deref | `error[E0614]: type `Untrusted<GuestOutput<minos_sandbox::Judge>>` cannot be dereferenced` |
| untrusted_display | `error[E0277]: `Untrusted<GuestOutput<minos_sandbox::Judge>>` doesn't implement `std::fmt::Display`` |
| untrusted_field | `error[E0616]: field `0` of struct `Untrusted` is private` |

## Task Commits

1. **Task 1: Backend, RunningVm, LaunchError, InfraError, and a fake-backend flow test**
   - `fba7675` (test, RED: the imports did not resolve, E0432)
   - `3623836` (feat, GREEN: 6 of 6 pass)
2. **Task 2: The trybuild compile-fail suite and the pass case**: `a2aa97d` (test)
3. **Task 3: CLAUDE.md sandbox signatures match what shipped**: `99f5b85` (docs)

## Files Created/Modified

- `crates/minos-sandbox/src/backend.rs`: Backend, RunningVm, LaunchError, InfraError
- `crates/minos-sandbox/src/lib.rs`: `pub mod backend`, re-exports, flow doc
- `crates/minos-sandbox/tests/backend.rs`: fake backend and six flow tests
- `crates/minos-sandbox/tests/compile_fail.rs`: trybuild driver `sandbox_invariants_are_compile_time_facts`
- `crates/minos-sandbox/tests/ui/fail/*.rs`, `*.stderr`: twelve compile-fail cases with their reviewed stderr
- `crates/minos-sandbox/tests/ui/pass/judge_flow.rs`: runnable judge flow against an in-file fake backend
- `CLAUDE.md`: minos-sandbox, Local backends, and Testing sections

## Decisions Made

See `key-decisions` in the frontmatter. In brief: the trait shape is exactly as planned, `Backend` is not sealed (backend crates implement it, and `Cleared` carries the security), the trait takes no blob store, and the suite stays at the planned twelve cases.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Ambiguous intra-doc link to `preflight`**
- **Found during:** Task 1
- **Issue:** `preflight` is both a module and a function at the crate root, so `[`preflight`]` in the new crate doc and `[`crate::preflight`]` in backend.rs failed `cargo doc` (`rustdoc::broken-intra-doc-links`).
- **Fix:** Linked the function form, `[`preflight()`]` and `[`crate::preflight()`]`.
- **Files modified:** crates/minos-sandbox/src/lib.rs, crates/minos-sandbox/src/backend.rs
- **Commit:** 3623836

**2. [Rule 3 - Blocking] clippy `duration_suboptimal_units` in the flow test**
- **Found during:** Task 1
- **Issue:** `Duration::from_millis(210_000)` and `from_millis(120_000)` failed clippy under `-D warnings`.
- **Fix:** `Duration::from_secs(210)` and `Duration::from_mins(2)`.
- **Files modified:** crates/minos-sandbox/tests/backend.rs
- **Commit:** 3623836

**3. [Plan shape] `launch_bare_plan.rs` has an empty `main`**
- **Found during:** Task 2
- **Issue:** The plan asks every fail case's `main` to reference its probe. This probe is generic over `B: Backend`, so it cannot be coerced to a function pointer without a concrete backend.
- **Fix:** `main` holds a comment explaining this. The E0308 stops compilation before the dead-code lint runs, so no warning joins the stderr, which the review confirmed. The comment sits after the error line, so the snapshot did not move.
- **Commit:** a2aa97d

**4. [Formatting] UI files run through rustfmt**
- **Found during:** Task 2
- **Issue:** `cargo fmt` does not reach `tests/ui/`, and three files had over-long lines.
- **Fix:** Ran `rustfmt --edition 2024` on the UI files. Every change was below the error lines (in `main`) or in the pass case, and the suite was re-run with `TRYBUILD` unset to confirm the snapshots still match.
- **Commit:** a2aa97d

---

**Total deviations:** 4 (two lint or doc fixes, two shape notes). **Impact:** none on scope or on any invariant.

## TDD Gate Compliance

Task 1 followed RED (`fba7675`, test) and then GREEN (`3623836`, feat). Task 2 is a test-only task: the invariants it proves already shipped in 04-03 and 04-06, so it has no implementation step. Its "RED" is the review that each case fails to compile for the intended reason.

## Issues Encountered

- `cargo deny check` exits 0 but prints `unused-wrapper` warnings for wrapper entries in `deny.toml` that are declared ahead of crate edges that do not exist yet. They predate this plan, and this plan does not edit `deny.toml`.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (including the trybuild suite with `TRYBUILD` unset), `cargo deny check`, `cargo doc --workspace --no-deps`, and the musl clippy for `minos-guest`: all exit 0.
- C-01: 11 `ok` lines, no `MISSING`. C-02 prints nothing. `grep -rnE 'dyn Backend|enum [A-Za-z]*Backend' crates/minos-sandbox/src` prints nothing.
- 12 fail `.rs` and 12 `.stderr` files, each with exactly one `^error` line. No `wip/` directory, and no `.stderr` for the pass case.

## Threat Model Coverage

- T-04-08-01: `launch` takes `Cleared<R>` by value; covered by launch_bare_plan, cleared_literal, and cleared_clone.
- T-04-08-02: covered by candidate_secrets, candidate_network, and judge_network.
- T-04-08-03: covered by candidate_scorecard.
- T-04-08-04: covered by untrusted_debug, untrusted_display, untrusted_deref, and untrusted_field.
- T-04-08-05: one reviewed error per `.stderr`, the judge_flow pass case, and `TRYBUILD` never set in the gate.
- T-04-08-06: the `Backend::launch` and `RunningVm` docs require `max_duration` on every launch; `every_launch_carries_a_hard_limit` shows that the backend sees it.

## Next Phase Readiness

- Phase 5's krun backend implements `Backend` for `KrunBackend`, with `Running<R>` wrapping the supervised `minos-vmm` process. It must derive its idempotency token from `ok.plan().key()` and enforce `ok.plan().max_duration()`.
- Follow-up candidates: compile-fail cases for `Untrusted::new` from outside the crate and for `Finished::new` with a `Cleared` of the wrong role (04-06's suggestion).

## Self-Check: PASSED

- FOUND: crates/minos-sandbox/src/backend.rs, crates/minos-sandbox/tests/backend.rs, crates/minos-sandbox/tests/compile_fail.rs, crates/minos-sandbox/tests/ui/pass/judge_flow.rs, 12 fail cases and 12 stderr files
- FOUND commits: fba7675, 3623836, a2aa97d, 99f5b85
