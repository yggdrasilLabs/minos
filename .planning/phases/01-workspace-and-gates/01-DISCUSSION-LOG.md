# Phase 1: Workspace and Gates - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-09
**Phase:** 1-Workspace and Gates
**Areas discussed:** ci entry point form, Crate skeleton scope, Gate self-test, Lint baseline breadth

---

## ci entry point form

| Option | Description | Selected |
|--------|-------------|----------|
| cargo xtask ci | Rust binary in an `xtask` crate; layering check in Rust | |
| Shell script ./ci | POSIX sh running fmt, clippy, test, deny, and a layering grep | |
| Both: ./ci wraps cargo xtask ci | Shell shim over Rust logic | |
| No script at all | Policy in Cargo.toml, clippy.toml, deny.toml; the gate is the four cargo commands | ✓ |

**User's choice:** No script, no xtask. Free-text: "why wouldn't we just set all of these in cargo.toml under lints" and "if we have them as cargo.toml configs we don't need .ci".
**Notes:** The `allow(clippy::...)` ban becomes clippy `allow_attributes` lints; the layering rule becomes `deny.toml` `[bans]` wrappers. A follow-up question about adding a GitHub Actions workflow was withdrawn; the answer is no workflow in Phase 1.

---

## Crate skeleton scope

| Option | Description | Selected |
|--------|-------------|----------|
| All v1 crates as empty stubs | core, store, sandbox, backend-krun, vmm, guest, propose, runner, loop, cli with real dependency edges | ✓ |
| Only the three named plus cli | core, loop, sandbox, cli | |
| Let Claude decide | Planner picks | |

**User's choice:** All v1 crates as empty stubs.

| Option | Description | Selected |
|--------|-------------|----------|
| Apache-2.0 OR MIT | Rust-ecosystem dual license | ✓ |
| Apache-2.0 only | Single license with patent grant | |
| MIT only | Simplest permissive license | |

**User's choice:** Apache-2.0 OR MIT.

| Option | Description | Selected |
|--------|-------------|----------|
| No, host target only | Build on aarch64-apple-darwin; musl in Phase 5; deny.toml lists both targets | ✓ |
| Yes, cross-check guest now | Add rustup target and linker config in Phase 1 | |

**User's choice:** Host target only.

---

## Gate self-test

| Option | Description | Selected |
|--------|-------------|----------|
| One-time manual proof | Apply each violation during verification, capture failing output into VERIFICATION.md, revert | ✓ |
| Permanent fixture tests | Fixture crates outside the workspace plus a subprocess test asserting failure | |
| trybuild for rustc lints only | Pin `unsafe_code = forbid` permanently; manual proof for the rest | |

**User's choice:** One-time manual proof.

---

## Lint baseline breadth

| Option | Description | Selected |
|--------|-------------|----------|
| Full rules/ lint set | correctness deny; complexity, perf, style, suspicious warn; pedantic warn with carve-outs; missing_docs; rustdoc links; clippy::cargo | ✓ |
| Four locked lints plus correctness only | Minimal set; pedantic and docs later | |
| Let Claude decide | Planner reads rules/lint-*.md | |

**User's choice:** Full rules/ lint set.

| Option | Description | Selected |
|--------|-------------|----------|
| Exact = pins | Every dependency `=x.y.z` | ✓ |
| Caret ranges with Cargo.lock | Standard ranges; lockfile pins | |

**User's choice:** Exact pins.

| Option | Description | Selected |
|--------|-------------|----------|
| Full research list, wired into stubs | Declare the whole v1 dependency table now; each stub pulls in what it will use | ✓ |
| Only what the stubs need | Later phases add their own dependencies | |

**User's choice:** Full research list, wired into stubs.

---

## Claude's Discretion

- Lib versus bin shape of each stub and minimal contents to satisfy `missing_docs`.
- `rustfmt.toml` contents.
- `deny.toml` `multiple-versions` level and license allowlist.
- Exact pedantic carve-out list, each with a reason.

## Deferred Ideas

- Phase 5 codesign mechanism, since there is no `xtask` crate.
- GitHub Actions workflow.
- musl cross-build of `minos-guest` (Phase 5).
