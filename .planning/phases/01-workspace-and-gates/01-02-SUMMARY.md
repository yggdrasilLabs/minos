---
phase: 01-workspace-and-gates
plan: 02
subsystem: infra
tags: [cargo-deny, supply-chain, licenses, advisories, build-scripts, layering]
status: complete

requires:
  - phase: 01-01
    provides: "Ten-crate workspace, exact pins with versioned path deps, committed Cargo.lock"
provides:
  - "deny.toml: sources, licenses, advisories, C/OpenSSL/libgit2 bans, centralized pins, 25-crate build-script allowlist"
  - "Crate-layering policy as data: one wrappers entry per internal library, plus anyhow->minos-cli and msb_krun->minos-vmm"
  - "All four gate commands passing together on the committed tree (GATE-02)"
affects: [01-03 gate proofs, every later dependency change]

tech-stack:
  added: []
  patterns:
    - "Layering = one [bans] deny entry per internal crate whose wrappers list its allowed direct dependents"
    - "A new build-script-not-allowed, license, or advisory error is a stop-and-review point, never an automatic allowlist entry"

key-files:
  created:
    - deny.toml
  modified: []

key-decisions:
  - "deny.toml taken from RESEARCH.md Code Examples verbatim (values and lists); comments expanded only"
  - "multiple-versions stays warn: 7 duplicate crates from msb_krun, proptest, and getrandom are visible but not failing"

patterns-established:
  - "cargo deny check is the fourth gate; all policy lives in Cargo.toml, clippy.toml, deny.toml, rust-toolchain.toml"

requirements-completed: [GATE-02, GATE-03, GATE-04]

coverage:
  - id: D1
    description: "Supply-chain policy: crates.io only, git needs rev + allow-git, build-script allowlist, executables denied, C/OpenSSL/libgit2 banned, licenses and advisories checked"
    requirement: GATE-03
    verification:
      - kind: other
        ref: "cargo deny check -> advisories ok, bans ok, licenses ok, sources ok; acceptance greps (25 build-script entries, 9 C bans)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Crate layering encoded as wrappers entries for all seven internal libraries"
    requirement: GATE-04
    verification:
      - kind: other
        ref: "cargo deny check passes the allowed graph; grep count of minos-* wrappers entries = 7. Firing proofs are Plan 01-03 V-15 to V-22"
        status: pass
    human_judgment: false
  - id: D3
    description: "fmt, clippy -D warnings, test, and deny pass together"
    requirement: GATE-02
    verification:
      - kind: other
        ref: "four gate commands, each exit 0 (output below)"
        status: pass
    human_judgment: false

duration: 1min
completed: 2026-10-09
---

# Phase 1 Plan 02: deny.toml Supply-Chain and Layering Policy Summary

**cargo-deny 0.20.2 policy that admits only crates.io at exact pins with 25 reviewed build scripts, seven allowlisted licenses, no C/OpenSSL/libgit2, and the whole internal crate DAG as `wrappers` data; all four gate commands pass together.**

## Performance

- **Duration:** about 1 min
- **Started:** 2026-10-09T21:30:02Z
- **Completed:** 2026-10-09T21:31:01Z
- **Tasks:** 2
- **Files created:** 1 (deny.toml)

## Accomplishments

- `[graph]` checks both `aarch64-apple-darwin` and `aarch64-unknown-linux-musl` with `all-features = false` (D-09).
- `[advisories]` fails on RustSec vulnerabilities, yanked crates, and unmaintained direct dependencies.
- `[licenses]` allows exactly Apache-2.0, MIT, BSD-3-Clause, BSL-1.0, ISC, Unicode-3.0, Zlib, with the crate that needs each.
- `[bans]`: wildcards denied; nine C-related crates banned (openssl-sys, openssl, native-tls, git2, libgit2-sys, gix, zstd-sys, liblzma-sys, cc); anyhow only in minos-cli; msb_krun only in minos-vmm; seven layering entries (minos-core, -store, -sandbox, -backend-krun, -propose, -runner, -loop).
- `[bans.workspace-dependencies]`: duplicates and unused denied, path dependencies included.
- `[bans.build]`: 25-crate `allow-build-scripts` list and `executables = "deny"`.
- `[sources]`: unknown registry and git denied, `required-git-spec = "rev"`, `allow-git = []`.

## Task Commits

1. **Task 1: deny.toml supply-chain policy** - `97418de` (chore)
2. **Task 2: crate layering and who-may-link bans, full gate** - `4b97b08` (chore)

## Measurements

- **`cargo deny check` summary line (verbatim):** `advisories ok, bans ok, licenses ok, sources ok`
- **multiple-versions warnings (7):** bitflags 1.3.2 / 2.13.2; getrandom 0.3.4 / 0.4.3; miniz_oxide 0.8.9 / 0.9.1; nix 0.30.1 / 0.31.3; rand_chacha 0.9.0 / 0.10.0; rand_core 0.9.5 / 0.10.1; syn 2.0.119 / 3.0.6. These match RESEARCH.md Pitfall 2's list (msb_krun, proptest, getrandom).
- **Full four-command gate wall time:** 2 s (warm target dir, advisory DB already cached). `cargo deny check` alone took about 1 s.
- No error-level diagnostics from cargo-deny; no `build-script-not-allowed`, `rejected`, `wildcard`, or `license-not-encountered`.

## Gate Output (final tree, actual text)

```
$ cargo fmt --all --check
fmt exit 0            (no output)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
clippy exit 0
$ cargo test --workspace   (filtered to Finished/Running/Doc-tests/test result lines)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.17s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-bda2b316f1f16fad)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos-0a774703e7599d0a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_core-cae5489e7b9c95c9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_guest-d76265ead7088bad)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_loop-e38611b5e3892c40)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_propose-537621b17e99ad0a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_runner-a4fda74d27c6ec2c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-f05f201c10f3f4b9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_store-0f568eafc6c6e157)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_vmm-ddae4c37ddaf60a6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_backend_krun
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_loop
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_propose
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_runner
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_sandbox
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_store
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test exit 0
$ cargo deny check   (diagnostic headers and summary; dependency trees omitted)
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
warning[duplicate]: found 2 duplicate entries for crate 'nix'
warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'syn'
advisories ok, bans ok, licenses ok, sources ok
deny exit 0
```

Acceptance output (actual):

```
Task 1: all 15 required key strings found by grep -F
grep -oE '\{ crate = "[^"]+" \}' deny.toml | wc -l  -> 25
C bans (each `{ crate = "<name>", reason = ` count): openssl-sys 1, openssl 1, native-tls 1, git2 1, libgit2-sys 1, gix 1, zstd-sys 1, liblzma-sys 1, cc 1
Task 2: all 9 wrappers strings found by grep -F
grep -cE '\{ crate = "minos-[a-z-]+", wrappers = ' deny.toml  -> 7
git ls-files Cargo.lock -> Cargo.lock ; git status --porcelain Cargo.lock -> (empty)
git status --porcelain --untracked-files=all -- . ':(exclude).planning' after Task 2 commit -> (empty)
```

## Decisions Made

- Values and lists are exactly RESEARCH.md's verified deny.toml. Comments were expanded where the plan asked (build-script review rule, layering explanation, binaries need no entry, dev edges are checked).

## Deviations from Plan

None. The plan was executed as written. No allowlist, ignore, or exception entry was added beyond the plan's lists; cargo-deny reported nothing those lists do not cover.

## Issues Encountered

- The shell is zsh, so the first `PIPESTATUS` read printed empty; the exit code was re-checked directly (`exit 0`). No effect on the result.

## Notes for Plan 01-03

- This plan shows the allowed graph passes. It does not prove each ban fires; that is V-11 to V-23 in Plan 01-03.
- `cargo clean` was not run (reserved for Plan 01-03).

## Next Phase Readiness

Plan 01-03 can run the violation proofs against this deny.toml and the 01-01 lint baseline.

## Self-Check: PASSED

- deny.toml exists and is tracked
- Commits 97418de and 4b97b08 found in git log
- Working tree outside .planning is clean
