# Phase 1: Gate Evidence (D-10, D-11)

Date: 2026-10-09

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- HEAD at start of proofs: `84fb45e25133e3232c57f2e990a1296650e54e3f`

This file holds the D-10 evidence (verbatim gate output for every deliberate violation) because gsd-verifier writes and owns 01-VERIFICATION.md and would overwrite anything placed there first.

Each proof was applied alone to the committed tree, the named command was run, and the edit was reverted from HEAD before the next proof. Cargo progress lines (Compiling, Checking, Downloaded, Downloading, Updating, Locking, Adding, Blocking, Fetching, and Finished on a failing run) are removed; every other line is verbatim.

## Code-lint proofs

### V-01: unsafe block

Requirement: GATE-01

Edit: crates/minos-store/src/lib.rs: appended probe `pub fn gate_probe(x: &[u8]) -> u8 { unsafe { *x.get_unchecked(0) } }` (with the standard probe doc lines and `#[must_use]`)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: usage of an `unsafe` block
  --> crates/minos-store/src/lib.rs:10:5
   |
10 |     unsafe { *x.get_unchecked(0) }
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: requested on the command line with `-F unsafe-code`

error: unsafe block missing a safety comment
  --> crates/minos-store/src/lib.rs:10:5
   |
10 |     unsafe { *x.get_unchecked(0) }
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: consider adding a safety comment on the preceding line
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#undocumented_unsafe_blocks
   = note: requested on the command line with `-D clippy::undocumented-unsafe-blocks`

error: could not compile `minos-store` (lib) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 2 previous errors
```

### V-02: unwrap

Requirement: GATE-01

Edit: crates/minos-store/src/lib.rs: appended probe `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }` (with the standard probe doc lines and `#[must_use]`)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: used `unwrap()` on a `Result` value
  --> crates/minos-store/src/lib.rs:10:5
   |
10 |     s.parse::<u8>().unwrap()
   |     ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: if this value is an `Err`, it will panic
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#unwrap_used
   = note: requested on the command line with `-D clippy::unwrap-used`

error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### V-03: expect

Requirement: GATE-01

Edit: crates/minos-store/src/lib.rs: appended probe `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().expect("n") }` (with the standard probe doc lines and `#[must_use]`)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: used `expect()` on a `Result` value
  --> crates/minos-store/src/lib.rs:10:5
   |
10 |     s.parse::<u8>().expect("n")
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: if this value is an `Err`, it will panic
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#expect_used
   = note: requested on the command line with `-D clippy::expect-used`

error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### V-04: slice index

Requirement: GATE-01

Edit: crates/minos-store/src/lib.rs: appended probe `pub fn gate_probe(x: &[u8]) -> u8 { x[0] }` (with the standard probe doc lines and `#[must_use]`)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: indexing may panic
  --> crates/minos-store/src/lib.rs:10:5
   |
10 |     x[0]
   |     ^^^^
   |
   = help: consider using `.get(n)` or `.get_mut(n)` instead
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#indexing_slicing
   = note: requested on the command line with `-D clippy::indexing-slicing`

error: could not compile `minos-store` (lib test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib) due to 1 previous error
```

### V-05: outer allow without a reason

Requirement: GATE-05, D-02

Edit: crates/minos-store/src/lib.rs: appended the V-04 probe with the outer attribute `#[allow(clippy::indexing_slicing)]` on the function

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: #[allow] attribute found
 --> crates/minos-store/src/lib.rs:8:3
  |
8 | #[allow(clippy::indexing_slicing)]
  |   ^^^^^ help: replace it with: `expect`
  |
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#allow_attributes
  = note: requested on the command line with `-D clippy::allow-attributes`

error: `allow` attribute without specifying a reason
 --> crates/minos-store/src/lib.rs:8:1
  |
8 | #[allow(clippy::indexing_slicing)]
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = help: try adding a reason at the end with `, reason = ".."`
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#allow_attributes_without_reason
  = note: requested on the command line with `-D clippy::allow-attributes-without-reason`

error: could not compile `minos-store` (lib) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 2 previous errors
```

### V-06: outer allow with a reason

Requirement: GATE-05, D-02

Edit: crates/minos-store/src/lib.rs: appended the V-04 probe with the outer attribute `#[allow(clippy::indexing_slicing, reason = "gate self-test")]` on the function

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: #[allow] attribute found
 --> crates/minos-store/src/lib.rs:8:3
  |
8 | #[allow(clippy::indexing_slicing, reason = "gate self-test")]
  |   ^^^^^ help: replace it with: `expect`
  |
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#allow_attributes
  = note: requested on the command line with `-D clippy::allow-attributes`

error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### V-07: positive control, expect with a reason on code that indexes

Requirement: GATE-05, D-02

Edit: crates/minos-store/src/lib.rs: appended the V-04 probe with `#[expect(clippy::indexing_slicing, reason = "gate self-test positive control")]` on the function. No adjustment to the probe was needed: the first run is the recorded run.

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

### V-08: unfulfilled expectation

Requirement: GATE-05

Edit: crates/minos-store/src/lib.rs: appended `/// Gate self-test probe.`, `#[expect(clippy::indexing_slicing, reason = "gate self-test")]`, `#[must_use]`, `pub fn gate_probe(x: &[u8]) -> usize { x.len() }` (no Panics section)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: this lint expectation is unfulfilled
 --> crates/minos-store/src/lib.rs:4:10
  |
4 | #[expect(clippy::indexing_slicing, reason = "gate self-test")]
  |          ^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: gate self-test
  = note: `-D unfulfilled-lint-expectations` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(unfulfilled_lint_expectations)]`

error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### V-09: clock in core

Requirement: GATE-05

Edit: crates/minos-core/src/lib.rs: appended `/// Gate self-test probe.`, `#[must_use]`, `pub fn gate_probe() -> std::time::SystemTime { std::time::SystemTime::now() }` (no Panics section)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: use of a disallowed method `std::time::SystemTime::now`
 --> crates/minos-core/src/lib.rs:6:5
  |
6 |     std::time::SystemTime::now()
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: minos-core has no clock: time arrives as an argument
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
  = note: `-D clippy::disallowed-methods` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`

error: could not compile `minos-core` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-core` (lib test) due to 1 previous error
```

### V-10: hash map in core

Requirement: GATE-05

Edit: crates/minos-core/src/lib.rs: appended `/// Gate self-test probe.`, `#[must_use]`, `pub fn gate_probe() -> std::collections::HashMap<u8, u8> { std::collections::HashMap::new() }` (no Panics section)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: use of a disallowed type `std::collections::HashMap`
 --> crates/minos-core/src/lib.rs:5:24
  |
5 | pub fn gate_probe() -> std::collections::HashMap<u8, u8> {
  |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: order must be stable: use BTreeMap
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
  = note: `-D clippy::disallowed-types` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`

error: use of a disallowed type `std::collections::HashMap`
 --> crates/minos-core/src/lib.rs:6:5
  |
6 |     std::collections::HashMap::new()
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: order must be stable: use BTreeMap
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types

error: could not compile `minos-core` (lib) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-core` (lib test) due to 2 previous errors
```

## Gap demonstrations (recorded, not fixed)

Each item below shows something the four gate commands do not catch. None was fixed; see "Open decisions for the user".

### G-01: inner allow with a reason

Requirement: GATE-05 (gap, open decision 1)

Edit: crates/minos-store/src/lib.rs: inserted `#![allow(clippy::unwrap_used, reason = "gate gap demonstration")]` on the line after the `//!` doc, and appended the V-02 probe `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }`

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```

### G-02: crate without inherited lints

Requirement: GATE-01 (gap, open decision 2)

Edit: crates/minos-loop/Cargo.toml: deleted the two lines `[lints]` and `workspace = true`; crates/minos-loop/src/lib.rs: appended `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }` (no doc, no attribute)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```

### G-03: rustdoc lint outside the four commands

Requirement: GATE-01, D-12 (gap, open decision 3)

Edit: crates/minos-store/src/lib.rs: appended `pub const GATE_PROBE: u8 = 0;` with the doc line ``/// Gate probe that links to [`DoesNotExist`].``. `cargo doc` is run only to demonstrate the gap; it is not part of the gate.

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```

Command: `cargo test --workspace`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.40s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-bda2b316f1f16fad)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/minos-0a774703e7599d0a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_core-cae5489e7b9c95c9)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/minos_guest-d76265ead7088bad)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_loop-e38611b5e3892c40)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_propose-537621b17e99ad0a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_runner-a4fda74d27c6ec2c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-f05f201c10f3f4b9)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/minos_store-0f568eafc6c6e157)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/minos_vmm-ddae4c37ddaf60a6)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_backend_krun

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_loop

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_propose

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_runner

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_sandbox

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests minos_store

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

Command: `cargo doc --workspace --no-deps`

Exit code: 101

```text
 Documenting minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
 Documenting minos-guest v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-guest)
 Documenting minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
 Documenting minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
 Documenting minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
 Documenting minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)
 Documenting minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
 Documenting minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
 Documenting minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
error: unresolved link to `DoesNotExist`
 --> crates/minos-store/src/lib.rs:3:32
  |
3 | /// Gate probe that links to [`DoesNotExist`].
  |                                ^^^^^^^^^^^^ no item named `DoesNotExist` in scope
  |
  = help: to escape `[` and `]` characters, add '\' before them like `\[` or `\]`
  = note: requested on the command line with `-D rustdoc::broken-intra-doc-links`

error: could not document `minos-store`
warning: build failed, waiting for other jobs to finish...
```

## One-time checks

### C-01: every crate manifest inherits the workspace lints

Requirement: D-06 (RESEARCH.md Pitfall 4)

Command: `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`

```text
ok crates/minos-backend-krun/Cargo.toml
ok crates/minos-cli/Cargo.toml
ok crates/minos-core/Cargo.toml
ok crates/minos-guest/Cargo.toml
ok crates/minos-loop/Cargo.toml
ok crates/minos-propose/Cargo.toml
ok crates/minos-runner/Cargo.toml
ok crates/minos-sandbox/Cargo.toml
ok crates/minos-store/Cargo.toml
ok crates/minos-vmm/Cargo.toml
```

### C-02: no allow attribute in the committed crates

Requirement: GATE-05

Command: `grep -rnE '#!?\[allow\(' crates`

```text
```

### C-03: no unknown lint name in the configuration

Requirement: RESEARCH.md Pitfall 3

Command: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -ci 'unknown lint'`

```text
0
```

## Supply-chain and layering proofs

### V-11: git dependency without rev

Requirement: GATE-03

Edit: crates/minos-store/Cargo.toml: added `cfg-if = { git = "https://github.com/rust-lang/cfg-if", branch = "main" }` under `[dependencies]`

Command: `cargo deny check sources`

Exit code: 8

```text
error[git-source-underspecified]: 'git' source is underspecified, expected 'rev', but found 'branch'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:22:14
   │
22 │ cfg-if 1.0.5 git+https://github.com/rust-lang/cfg-if?branch=main
   │              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ source
   │
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:77:22
   │
77 │ required-git-spec = "rev"   # GATE-03: a git dependency must be pinned by rev
   │                      ━━━ minimum spec defined here
   │
   ├ cfg-if v1.0.5
     └── minos-store v0.1.0
         ├── minos-cli v0.1.0
         ├── minos-loop v0.1.0
         │   └── minos-cli v0.1.0 (*)
         └── minos-runner v0.1.0
             └── minos-cli v0.1.0 (*)

error[source-not-allowed]: detected 'git' source not explicitly allowed
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:22:14
   │
22 │ cfg-if 1.0.5 git+https://github.com/rust-lang/cfg-if?branch=main
   │              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ source
   │
   ├ cfg-if v1.0.5 (*)

sources FAILED
```

### V-12: git source not on the allow list

Requirement: GATE-03

Edit: crates/minos-store/Cargo.toml: added `cfg-if = { git = "https://github.com/rust-lang/cfg-if", rev = "14432a28e180ec717933714aed7828fc7ed2fd29" }` under `[dependencies]`

Command: `cargo deny check sources`

Exit code: 8

```text
error[source-not-allowed]: detected 'git' source not explicitly allowed
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:22:14
   │
22 │ cfg-if 1.0.5 git+https://github.com/rust-lang/cfg-if?rev=14432a28e180ec717933714aed7828fc7ed2fd29
   │              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ source
   │
   ├ cfg-if v1.0.5
     └── minos-store v0.1.0
         ├── minos-cli v0.1.0
         ├── minos-loop v0.1.0
         │   └── minos-cli v0.1.0 (*)
         └── minos-runner v0.1.0
             └── minos-cli v0.1.0 (*)

sources FAILED
```

### V-13: unknown registry

Requirement: GATE-03

Edit: created a temporary `.cargo/config.toml` at the repository root containing `[registries.mirror]` with `index = "sparse+https://rsproxy.cn/index/"` (the repository had no `.cargo` before this proof); crates/minos-store/Cargo.toml: added `cfg-if = { version = "=1.0.4", registry = "mirror" }` under `[dependencies]`. Reverted by deleting exactly `<repo root>/.cargo` (never `~/.cargo`) and restoring from HEAD. The mirror cache entry under the user cargo home was already present from research and is left in place.

Command: `cargo deny check sources`

Exit code: 8

```text
error[source-not-allowed]: detected 'registry' source not explicitly allowed
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:22:14
   │
22 │ cfg-if 1.0.4 sparse+https://rsproxy.cn/index/
   │              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ source
   │
   ├ cfg-if v1.0.4
     └── minos-store v0.1.0
         ├── minos-cli v0.1.0
         ├── minos-loop v0.1.0
         │   └── minos-cli v0.1.0 (*)
         └── minos-runner v0.1.0
             └── minos-cli v0.1.0 (*)

sources FAILED
```

### V-14: build script not on the allowlist

Requirement: GATE-03

Edit: crates/minos-store/Cargo.toml: added `httparse = "=1.10.1"` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:77:1
   │  
77 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
78 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:101:1
    │  
101 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
102 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:117:1
    │  
117 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
118 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:119:1
    │  
119 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
120 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:143:1
    │  
143 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
144 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

error[build-script-not-allowed]: crate 'httparse = 1.10.1' has a build script but is not allowed to have one
 ├ path = '$CARGO_HOME/registry/src/index.crates.io-1949cf8c6b5b557f/httparse-1.10.1/build.rs'
 ├ checksum = 'eba8e341d746a6cf54249c62afe9eb237e7c21fcf9937223e894549e3a34d489'
 ├ httparse v1.10.1
   └── minos-store v0.1.0
       ├── minos-cli v0.1.0
       ├── minos-loop v0.1.0
       │   └── minos-cli v0.1.0 (*)
       └── minos-runner v0.1.0
           └── minos-cli v0.1.0 (*)

bans FAILED
```

### V-15: loop to sandbox

Requirement: GATE-04

Edit: crates/minos-loop/Cargo.toml: added `minos-sandbox.workspace = true` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   ├── minos-loop v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   ├── minos-loop v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   ├── minos-loop v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   ├── minos-loop v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   ├── minos-loop v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   ├── minos-loop v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-loop v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   ├── minos-loop v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   ├── minos-loop v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-loop = 0.1.0' of banned crate 'minos-sandbox = 0.1.0' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:48:14
   │
48 │   { crate = "minos-sandbox", wrappers = ["minos-backend-krun", "minos-runner", "minos-cli"], reason = "core and loop never see sandbox types" },
   │              ━━━━━━━━━━━━━                                                                             ───────────────────────────────────── reason
   │              │                                                                                          
   │              banned here
   │
   ├ minos-sandbox v0.1.0
     ├── minos-backend-krun v0.1.0
     │   └── minos-cli v0.1.0
     ├── minos-cli v0.1.0 (*)
     ├── minos-loop v0.1.0
     │   └── minos-cli v0.1.0 (*)
     └── minos-runner v0.1.0
         └── minos-cli v0.1.0 (*)

error[banned]: crate 'minos-sandbox = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:48:14
   │
48 │   { crate = "minos-sandbox", wrappers = ["minos-backend-krun", "minos-runner", "minos-cli"], reason = "core and loop never see sandbox types" },
   │              ━━━━━━━━━━━━━                                                                             ───────────────────────────────────── reason
   │              │                                                                                          
   │              banned here
   │
   ├ minos-sandbox v0.1.0 (*)

bans FAILED
```

### V-16: loop to backend

Requirement: GATE-04

Edit: crates/minos-loop/Cargo.toml: added `minos-backend-krun.workspace = true` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   ├── minos-cli v0.1.0
     │   │   │   └── minos-loop v0.1.0
     │   │   │       └── minos-cli v0.1.0 (*)
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   ├── minos-cli v0.1.0
         │       │   │   └── minos-loop v0.1.0
         │       │   │       └── minos-cli v0.1.0 (*)
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   ├── minos-cli v0.1.0
         │   │   │   └── minos-loop v0.1.0
         │   │   │       └── minos-cli v0.1.0 (*)
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       ├── minos-cli v0.1.0
         │       └── minos-loop v0.1.0
         │           └── minos-cli v0.1.0 (*)
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   ├── minos-cli v0.1.0
      │   │   │   └── minos-loop v0.1.0
      │   │   │       └── minos-cli v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   ├── minos-cli v0.1.0
          │   └── minos-loop v0.1.0
          │       └── minos-cli v0.1.0 (*)
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   ├── minos-cli v0.1.0
      │       │   │   └── minos-loop v0.1.0
      │       │   │       └── minos-cli v0.1.0 (*)
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   ├── minos-cli v0.1.0
      │   │   └── minos-loop v0.1.0
      │   │       └── minos-cli v0.1.0 (*)
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   ├── minos-cli v0.1.0
      │   │                   └── minos-loop v0.1.0
      │   │                       └── minos-cli v0.1.0 (*)
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0
      │   │   │   │   │       └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-loop = 0.1.0' of banned crate 'minos-backend-krun = 0.1.0' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:49:14
   │
49 │   { crate = "minos-backend-krun", wrappers = ["minos-cli"], reason = "only the CLI picks a concrete backend" },
   │              ━━━━━━━━━━━━━━━━━━                                       ───────────────────────────────────── reason
   │              │                                                         
   │              banned here
   │
   ├ minos-backend-krun v0.1.0
     ├── minos-cli v0.1.0
     └── minos-loop v0.1.0
         └── minos-cli v0.1.0 (*)

error[banned]: crate 'minos-backend-krun = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:49:14
   │
49 │   { crate = "minos-backend-krun", wrappers = ["minos-cli"], reason = "only the CLI picks a concrete backend" },
   │              ━━━━━━━━━━━━━━━━━━                                       ───────────────────────────────────── reason
   │              │                                                         
   │              banned here
   │
   ├ minos-backend-krun v0.1.0 (*)

bans FAILED
```

### V-17: loop to runner (the transitive path to sandbox)

Requirement: GATE-04

Edit: crates/minos-loop/Cargo.toml: added `minos-runner.workspace = true` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-loop = 0.1.0' of banned crate 'minos-runner = 0.1.0' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:51:14
   │
51 │   { crate = "minos-runner", wrappers = ["minos-cli"], reason = "loop reaches the runner only through its JobRunner trait" },
   │              ━━━━━━━━━━━━                                       ──────────────────────────────────────────────────────── reason
   │              │                                                   
   │              banned here
   │
   ├ minos-runner v0.1.0
     ├── minos-cli v0.1.0
     └── minos-loop v0.1.0
         └── minos-cli v0.1.0 (*)

error[banned]: crate 'minos-runner = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:51:14
   │
51 │   { crate = "minos-runner", wrappers = ["minos-cli"], reason = "loop reaches the runner only through its JobRunner trait" },
   │              ━━━━━━━━━━━━                                       ──────────────────────────────────────────────────────── reason
   │              │                                                   
   │              banned here
   │
   ├ minos-runner v0.1.0 (*)

bans FAILED
```

### V-18: core to sandbox as a dev-dependency

Requirement: GATE-04

Edit: crates/minos-core/Cargo.toml: added `minos-sandbox.workspace = true` under `[dev-dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   ├── (dev) minos-core v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   ├── (dev) minos-core v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   ├── (dev) minos-core v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   ├── (dev) minos-core v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   ├── (dev) minos-core v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   ├── (dev) minos-core v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── (dev) minos-core v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   ├── (dev) minos-core v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   ├── (dev) minos-core v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-core = 0.1.0' of banned crate 'minos-sandbox = 0.1.0' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:48:14
   │
48 │   { crate = "minos-sandbox", wrappers = ["minos-backend-krun", "minos-runner", "minos-cli"], reason = "core and loop never see sandbox types" },
   │              ━━━━━━━━━━━━━                                                                             ───────────────────────────────────── reason
   │              │                                                                                          
   │              banned here
   │
   ├ minos-sandbox v0.1.0
     ├── minos-backend-krun v0.1.0
     │   └── minos-cli v0.1.0
     ├── minos-cli v0.1.0 (*)
     ├── (dev) minos-core v0.1.0
     │   ├── minos-backend-krun v0.1.0 (*)
     │   ├── minos-cli v0.1.0 (*)
     │   ├── minos-guest v0.1.0
     │   ├── minos-loop v0.1.0
     │   │   └── minos-cli v0.1.0 (*)
     │   ├── minos-propose v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   └── minos-loop v0.1.0 (*)
     │   ├── minos-runner v0.1.0
     │   │   └── minos-cli v0.1.0 (*)
     │   ├── minos-sandbox v0.1.0 (*)
     │   └── minos-store v0.1.0
     │       ├── minos-cli v0.1.0 (*)
     │       ├── minos-loop v0.1.0 (*)
     │       └── minos-runner v0.1.0 (*)
     └── minos-runner v0.1.0 (*)

error[banned]: crate 'minos-sandbox = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:48:14
   │
48 │   { crate = "minos-sandbox", wrappers = ["minos-backend-krun", "minos-runner", "minos-cli"], reason = "core and loop never see sandbox types" },
   │              ━━━━━━━━━━━━━                                                                             ───────────────────────────────────── reason
   │              │                                                                                          
   │              banned here
   │
   ├ minos-sandbox v0.1.0 (*)

bans FAILED
```

### V-19: core to backend as a dev-dependency

Requirement: GATE-04

Edit: crates/minos-core/Cargo.toml: added `minos-backend-krun.workspace = true` under `[dev-dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   ├── minos-cli v0.1.0
     │   │   │   └── (dev) minos-core v0.1.0 (*)
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   ├── minos-cli v0.1.0
         │       │   │   └── (dev) minos-core v0.1.0 (*)
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   ├── minos-cli v0.1.0
         │   │   │   └── (dev) minos-core v0.1.0 (*)
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       ├── minos-cli v0.1.0
         │       └── (dev) minos-core v0.1.0
         │           ├── minos-backend-krun v0.1.0 (*)
         │           ├── minos-cli v0.1.0 (*)
         │           ├── minos-guest v0.1.0
         │           ├── minos-loop v0.1.0
         │           │   └── minos-cli v0.1.0 (*)
         │           ├── minos-propose v0.1.0
         │           │   ├── minos-cli v0.1.0 (*)
         │           │   └── minos-loop v0.1.0 (*)
         │           ├── minos-runner v0.1.0
         │           │   └── minos-cli v0.1.0 (*)
         │           ├── minos-sandbox v0.1.0
         │           │   ├── minos-backend-krun v0.1.0 (*)
         │           │   ├── minos-cli v0.1.0 (*)
         │           │   └── minos-runner v0.1.0 (*)
         │           └── minos-store v0.1.0
         │               ├── minos-cli v0.1.0 (*)
         │               ├── minos-loop v0.1.0 (*)
         │               └── minos-runner v0.1.0 (*)
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   ├── minos-cli v0.1.0
      │   │   │   └── (dev) minos-core v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   ├── minos-cli v0.1.0
          │   └── (dev) minos-core v0.1.0 (*)
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   ├── minos-cli v0.1.0
      │       │   │   └── (dev) minos-core v0.1.0 (*)
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   ├── minos-cli v0.1.0
      │   │   └── (dev) minos-core v0.1.0 (*)
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   ├── minos-cli v0.1.0
      │   │                   └── (dev) minos-core v0.1.0
      │   │                       ├── minos-backend-krun v0.1.0 (*)
      │   │                       ├── minos-cli v0.1.0 (*)
      │   │                       ├── minos-guest v0.1.0
      │   │                       ├── minos-loop v0.1.0
      │   │                       │   └── minos-cli v0.1.0 (*)
      │   │                       ├── minos-propose v0.1.0
      │   │                       │   ├── minos-cli v0.1.0 (*)
      │   │                       │   └── minos-loop v0.1.0 (*)
      │   │                       ├── minos-runner v0.1.0
      │   │                       │   └── minos-cli v0.1.0 (*)
      │   │                       ├── minos-sandbox v0.1.0
      │   │                       │   ├── minos-backend-krun v0.1.0 (*)
      │   │                       │   ├── minos-cli v0.1.0 (*)
      │   │                       │   └── minos-runner v0.1.0 (*)
      │   │                       └── minos-store v0.1.0
      │   │                           ├── minos-cli v0.1.0 (*)
      │   │                           ├── minos-loop v0.1.0 (*)
      │   │                           └── minos-runner v0.1.0 (*)
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── (dev) minos-core v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-core = 0.1.0' of banned crate 'minos-backend-krun = 0.1.0' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:49:14
   │
49 │   { crate = "minos-backend-krun", wrappers = ["minos-cli"], reason = "only the CLI picks a concrete backend" },
   │              ━━━━━━━━━━━━━━━━━━                                       ───────────────────────────────────── reason
   │              │                                                         
   │              banned here
   │
   ├ minos-backend-krun v0.1.0
     ├── minos-cli v0.1.0
     └── (dev) minos-core v0.1.0
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-cli v0.1.0 (*)
         ├── minos-guest v0.1.0
         ├── minos-loop v0.1.0
         │   └── minos-cli v0.1.0 (*)
         ├── minos-propose v0.1.0
         │   ├── minos-cli v0.1.0 (*)
         │   └── minos-loop v0.1.0 (*)
         ├── minos-runner v0.1.0
         │   └── minos-cli v0.1.0 (*)
         ├── minos-sandbox v0.1.0
         │   ├── minos-backend-krun v0.1.0 (*)
         │   ├── minos-cli v0.1.0 (*)
         │   └── minos-runner v0.1.0 (*)
         └── minos-store v0.1.0
             ├── minos-cli v0.1.0 (*)
             ├── minos-loop v0.1.0 (*)
             └── minos-runner v0.1.0 (*)

error[banned]: crate 'minos-backend-krun = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:49:14
   │
49 │   { crate = "minos-backend-krun", wrappers = ["minos-cli"], reason = "only the CLI picks a concrete backend" },
   │              ━━━━━━━━━━━━━━━━━━                                       ───────────────────────────────────── reason
   │              │                                                         
   │              banned here
   │
   ├ minos-backend-krun v0.1.0 (*)

bans FAILED
```

### V-20: core to sandbox as a normal dependency

Requirement: GATE-04

Edit: crates/minos-core/Cargo.toml: added `minos-sandbox.workspace = true` under `[dependencies]`

Command: `cargo deny check`

Exit code: 1

```text
2026-10-09 21:36:22 [ERROR] failed to fetch crates: error: cyclic package dependency: package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)` depends on itself. Cycle:
package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)`
    ... which satisfies path dependency `minos-sandbox` (locked to 0.1.0) of package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)`

2026-10-09 21:36:22 [ERROR] `cargo metadata` exited with an error: error: cyclic package dependency: package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)` depends on itself. Cycle:
package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)`
    ... which satisfies path dependency `minos-sandbox` (locked to 0.1.0) of package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)`

```

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: cyclic package dependency: package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)` depends on itself. Cycle:
package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)`
    ... which satisfies path dependency `minos-sandbox` (locked to 0.1.0) of package `minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)`
    ... which satisfies path dependency `minos-core` (locked to 0.1.0) of package `minos-backend-krun v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-backend-krun)`
```

### V-21: anyhow in a library

Requirement: rules/err-anyhow-app.md (beyond D-11)

Edit: crates/minos-store/Cargo.toml: added `anyhow.workspace = true` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-store = 0.1.0' of banned crate 'anyhow = 1.0.104' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:39:14
   │
39 │   { crate = "anyhow", wrappers = ["minos-cli"], reason = "anyhow only at the CLI main (rules/err-anyhow-app.md)" },
   │              ━━━━━━                                       ───────────────────────────────────────────────────── reason
   │              │                                             
   │              banned here
   │
   ├ anyhow v1.0.104
     ├── minos-cli v0.1.0
     └── minos-store v0.1.0
         ├── minos-cli v0.1.0 (*)
         ├── minos-loop v0.1.0
         │   └── minos-cli v0.1.0 (*)
         └── minos-runner v0.1.0
             └── minos-cli v0.1.0 (*)

error[banned]: crate 'anyhow = 1.0.104' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:39:14
   │
39 │   { crate = "anyhow", wrappers = ["minos-cli"], reason = "anyhow only at the CLI main (rules/err-anyhow-app.md)" },
   │              ━━━━━━                                       ───────────────────────────────────────────────────── reason
   │              │                                             
   │              banned here
   │
   ├ anyhow v1.0.104 (*)

bans FAILED
```

### V-22: VMM linked outside minos-vmm

Requirement: KRUN-01 (beyond D-11)

Edit: crates/minos-backend-krun/Cargo.toml: added `msb_krun.workspace = true` under `[dependencies]`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   ├── minos-backend-krun v0.1.0
         │   │   │   │   └── minos-cli v0.1.0
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0 (*)
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   ├── minos-backend-krun v0.1.0
             │   │   └── minos-cli v0.1.0
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 ├── minos-backend-krun v0.1.0 (*)
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   ├── minos-backend-krun v0.1.0 (*)
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   └── minos-cli v0.1.0
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0 (*)
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   ├── minos-backend-krun v0.1.0 (*)
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   ├── minos-backend-krun v0.1.0
      │           │   │   └── minos-cli v0.1.0
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0 (*)
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

warning[unmatched-wrapper]: direct parent 'minos-backend-krun = 0.1.0' of banned crate 'msb_krun = 0.1.41' was not marked as a wrapper
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:40:14
   │
40 │   { crate = "msb_krun", wrappers = ["minos-vmm"], reason = "only minos-vmm links the VMM (KRUN-01)" },
   │              ━━━━━━━━                                       ────────────────────────────────────── reason
   │              │                                               
   │              banned here
   │
   ├ msb_krun v0.1.41
     ├── minos-backend-krun v0.1.0
     │   └── minos-cli v0.1.0
     └── minos-vmm v0.1.0

error[banned]: crate 'msb_krun = 0.1.41' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:40:14
   │
40 │   { crate = "msb_krun", wrappers = ["minos-vmm"], reason = "only minos-vmm links the VMM (KRUN-01)" },
   │              ━━━━━━━━                                       ────────────────────────────────────── reason
   │              │                                               
   │              banned here
   │
   ├ msb_krun v0.1.41 (*)

bans FAILED
```

### V-23: C code through a feature

Requirement: CLAUDE.md no-C rule (beyond D-11)

Edit: crates/minos-backend-krun/Cargo.toml: replaced the line `backhand.workspace = true` with `backhand = { workspace = true, features = ["zstd"] }`

Command: `cargo deny check bans`

Exit code: 2

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:55:1
   │  
55 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
56 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:79:1
   │  
79 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
80 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:103:1
    │  
103 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
104 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:120:1
    │  
120 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
121 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:122:1
    │  
122 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
123 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:147:1
    │  
147 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
148 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

error[banned]: crate 'cc = 1.6.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:37:14
   │
37 │   { crate = "cc", reason = "no C compiled in our tree" },
   │              ━━             ───────────────────────── reason
   │              │               
   │              banned here
   │
   ├ cc v1.6.0
     └── (build) zstd-sys v2.1.1+zstd.1.5.7
         └── zstd-safe v7.3.0
             ├── backhand v0.25.5
             │   └── minos-backend-krun v0.1.0
             │       └── minos-cli v0.1.0
             └── zstd v0.13.3
                 └── backhand v0.25.5 (*)

error[build-script-not-allowed]: crate 'zstd-safe = 7.3.0' has a build script but is not allowed to have one
 ├ path = '$CARGO_HOME/registry/src/index.crates.io-1949cf8c6b5b557f/zstd-safe-7.3.0/build.rs'
 ├ checksum = '2342e59833e2ebca2980884d4f242a6bf1b0143037c212e05514626ad5213505'
 ├ zstd-safe v7.3.0
   ├── backhand v0.25.5
   │   └── minos-backend-krun v0.1.0
   │       └── minos-cli v0.1.0
   └── zstd v0.13.3
       └── backhand v0.25.5 (*)

error[banned]: crate 'zstd-sys = 2.1.1+zstd.1.5.7' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:35:14
   │
35 │   { crate = "zstd-sys", reason = "C; backhand uses gzip only" },
   │              ━━━━━━━━             ────────────────────────── reason
   │              │                     
   │              banned here
   │
   ├ zstd-sys v2.1.1+zstd.1.5.7
     └── zstd-safe v7.3.0
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── zstd v0.13.3
             └── backhand v0.25.5 (*)

error[build-script-not-allowed]: crate 'zstd-sys = 2.1.1+zstd.1.5.7' has a build script but is not allowed to have one
 ├ path = '$CARGO_HOME/registry/src/index.crates.io-1949cf8c6b5b557f/zstd-sys-2.1.1+zstd.1.5.7/build.rs'
 ├ checksum = 'ff97fca52b72ba76c2c71f0d4ca3475d5fa746e184e7d01c6085f9efee360171'
 ├ zstd-sys v2.1.1+zstd.1.5.7 (*)

bans FAILED
```

### Deny re-check

The tree after every supply-chain and layering proof was reverted.

Command: `cargo deny check`

Exit code: 0

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:16:1
   │  
16 │ ╭ bitflags 1.3.2 registry+https://github.com/rust-lang/crates.io-index
17 │ │ bitflags 2.13.2 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ bitflags v1.3.2
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
     │       └── msb_krun v0.1.41 (*)
     ├── msb_krun_utils v0.1.41
     │   ├── msb_krun v0.1.41 (*)
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_kernel v0.1.41
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_polly v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     └── vmm-sys-util v0.15.0
         ├── kvm-bindings v0.14.2
         │   ├── kvm-ioctls v0.25.1
         │   │   ├── msb_krun v0.1.41 (*)
         │   │   ├── msb_krun_arch v0.1.41 (*)
         │   │   ├── msb_krun_devices v0.1.41 (*)
         │   │   └── msb_krun_vmm v0.1.41 (*)
         │   ├── msb_krun v0.1.41 (*)
         │   ├── msb_krun_arch v0.1.41 (*)
         │   ├── msb_krun_devices v0.1.41 (*)
         │   ├── msb_krun_utils v0.1.41 (*)
         │   └── msb_krun_vmm v0.1.41 (*)
         ├── kvm-ioctls v0.25.1 (*)
         └── msb_krun_utils v0.1.41 (*)
   ├ bitflags v2.13.2
     ├── kvm-ioctls v0.25.1
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   ├── msb_krun_arch v0.1.41
     │   │   ├── msb_krun_devices v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41
     │   │   │       └── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_hvf v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.30.1
     │   ├── msb-imago v0.1.7
     │   │   └── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_devices v0.1.41 (*)
     │   ├── msb_krun_utils v0.1.41
     │   │   ├── msb_krun v0.1.41 (*)
     │   │   ├── msb_krun_arch v0.1.41 (*)
     │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   ├── msb_krun_kernel v0.1.41
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   ├── msb_krun_polly v0.1.41
     │   │   │   ├── msb_krun v0.1.41 (*)
     │   │   │   ├── msb_krun_devices v0.1.41 (*)
     │   │   │   └── msb_krun_vmm v0.1.41 (*)
     │   │   └── msb_krun_vmm v0.1.41 (*)
     │   └── msb_krun_vmm v0.1.41 (*)
     ├── nix v0.31.3
     │   └── vsock v0.5.4
     │       └── minos-guest v0.1.0
     ├── proptest v1.11.0
     │   ├── (dev) minos-core v0.1.0
     │   │   ├── minos-backend-krun v0.1.0
     │   │   │   └── minos-cli v0.1.0
     │   │   ├── minos-cli v0.1.0 (*)
     │   │   ├── minos-guest v0.1.0 (*)
     │   │   ├── minos-loop v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-propose v0.1.0
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-loop v0.1.0 (*)
     │   │   ├── minos-runner v0.1.0
     │   │   │   └── minos-cli v0.1.0 (*)
     │   │   ├── minos-sandbox v0.1.0
     │   │   │   ├── minos-backend-krun v0.1.0 (*)
     │   │   │   ├── minos-cli v0.1.0 (*)
     │   │   │   └── minos-runner v0.1.0 (*)
     │   │   └── minos-store v0.1.0
     │   │       ├── minos-cli v0.1.0 (*)
     │   │       ├── minos-loop v0.1.0 (*)
     │   │       └── minos-runner v0.1.0 (*)
     │   └── (dev) minos-store v0.1.0 (*)
     └── rustix v1.1.5
         ├── minos-guest v0.1.0 (*)
         └── tempfile v3.27.0
             ├── insta v1.49.0
             │   ├── (dev) minos-core v0.1.0 (*)
             │   └── (dev) minos-propose v0.1.0 (*)
             ├── minos-backend-krun v0.1.0 (*)
             ├── minos-store v0.1.0 (*)
             ├── proptest v1.11.0 (*)
             └── rusty-fork v0.3.1
                 └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:53:1
   │  
53 │ ╭ getrandom 0.3.4 registry+https://github.com/rust-lang/crates.io-index
54 │ │ getrandom 0.4.3 registry+https://github.com/rust-lang/crates.io-index
   │ ╰─────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ getrandom v0.3.4
     └── rand_core v0.9.5
         ├── rand v0.9.5
         │   ├── msb_krun_devices v0.1.41
         │   │   ├── msb_krun v0.1.41
         │   │   │   └── minos-vmm v0.1.0
         │   │   └── msb_krun_vmm v0.1.41
         │   │       └── msb_krun v0.1.41 (*)
         │   └── proptest v1.11.0
         │       ├── (dev) minos-core v0.1.0
         │       │   ├── minos-backend-krun v0.1.0
         │       │   │   └── minos-cli v0.1.0
         │       │   ├── minos-cli v0.1.0 (*)
         │       │   ├── minos-guest v0.1.0
         │       │   ├── minos-loop v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-propose v0.1.0
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-loop v0.1.0 (*)
         │       │   ├── minos-runner v0.1.0
         │       │   │   └── minos-cli v0.1.0 (*)
         │       │   ├── minos-sandbox v0.1.0
         │       │   │   ├── minos-backend-krun v0.1.0 (*)
         │       │   │   ├── minos-cli v0.1.0 (*)
         │       │   │   └── minos-runner v0.1.0 (*)
         │       │   └── minos-store v0.1.0
         │       │       ├── minos-cli v0.1.0 (*)
         │       │       ├── minos-loop v0.1.0 (*)
         │       │       └── minos-runner v0.1.0 (*)
         │       └── (dev) minos-store v0.1.0 (*)
         ├── rand_chacha v0.9.0
         │   ├── proptest v1.11.0 (*)
         │   └── rand v0.9.5 (*)
         └── rand_xorshift v0.4.0
             └── proptest v1.11.0 (*)
   ├ getrandom v0.4.3
     └── tempfile v3.27.0
         ├── insta v1.49.0
         │   ├── (dev) minos-core v0.1.0
         │   │   ├── minos-backend-krun v0.1.0
         │   │   │   └── minos-cli v0.1.0
         │   │   ├── minos-cli v0.1.0 (*)
         │   │   ├── minos-guest v0.1.0
         │   │   ├── minos-loop v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-propose v0.1.0
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-loop v0.1.0 (*)
         │   │   ├── minos-runner v0.1.0
         │   │   │   └── minos-cli v0.1.0 (*)
         │   │   ├── minos-sandbox v0.1.0
         │   │   │   ├── minos-backend-krun v0.1.0 (*)
         │   │   │   ├── minos-cli v0.1.0 (*)
         │   │   │   └── minos-runner v0.1.0 (*)
         │   │   └── minos-store v0.1.0
         │   │       ├── minos-cli v0.1.0 (*)
         │   │       ├── minos-loop v0.1.0 (*)
         │   │       └── minos-runner v0.1.0 (*)
         │   └── (dev) minos-propose v0.1.0 (*)
         ├── minos-backend-krun v0.1.0 (*)
         ├── minos-store v0.1.0 (*)
         ├── proptest v1.11.0
         │   ├── (dev) minos-core v0.1.0 (*)
         │   └── (dev) minos-store v0.1.0 (*)
         └── rusty-fork v0.3.1
             └── proptest v1.11.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
   ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:76:1
   │  
76 │ ╭ miniz_oxide 0.8.9 registry+https://github.com/rust-lang/crates.io-index
77 │ │ miniz_oxide 0.9.1 registry+https://github.com/rust-lang/crates.io-index
   │ ╰───────────────────────────────────────────────────────────────────────┘ lock entries
   │  
   ├ miniz_oxide v0.8.9
     └── msb-imago v0.1.7
         └── msb_krun_devices v0.1.41
             ├── msb_krun v0.1.41
             │   └── minos-vmm v0.1.0
             └── msb_krun_vmm v0.1.41
                 └── msb_krun v0.1.41 (*)
   ├ miniz_oxide v0.9.1
     └── flate2 v1.1.10
         ├── backhand v0.25.5
         │   └── minos-backend-krun v0.1.0
         │       └── minos-cli v0.1.0
         └── msb_krun_vmm v0.1.41
             └── msb_krun v0.1.41
                 └── minos-vmm v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'nix'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:100:1
    │  
100 │ ╭ nix 0.30.1 registry+https://github.com/rust-lang/crates.io-index
101 │ │ nix 0.31.3 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ nix v0.30.1
      ├── msb-imago v0.1.7
      │   └── msb_krun_devices v0.1.41
      │       ├── msb_krun v0.1.41
      │       │   └── minos-vmm v0.1.0
      │       └── msb_krun_vmm v0.1.41
      │           └── msb_krun v0.1.41 (*)
      ├── msb_krun_devices v0.1.41 (*)
      ├── msb_krun_utils v0.1.41
      │   ├── msb_krun v0.1.41 (*)
      │   ├── msb_krun_arch v0.1.41
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_devices v0.1.41 (*)
      │   ├── msb_krun_kernel v0.1.41
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   ├── msb_krun_polly v0.1.41
      │   │   ├── msb_krun v0.1.41 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   └── msb_krun_vmm v0.1.41 (*)
      │   └── msb_krun_vmm v0.1.41 (*)
      └── msb_krun_vmm v0.1.41 (*)
    ├ nix v0.31.3
      └── vsock v0.5.4
          └── minos-guest v0.1.0

warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:116:1
    │  
116 │ ╭ rand_chacha 0.9.0 registry+https://github.com/rust-lang/crates.io-index
117 │ │ rand_chacha 0.10.0 registry+https://github.com/rust-lang/crates.io-index
    │ ╰────────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_chacha v0.9.0
      ├── proptest v1.11.0
      │   ├── (dev) minos-core v0.1.0
      │   │   ├── minos-backend-krun v0.1.0
      │   │   │   └── minos-cli v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-guest v0.1.0
      │   │   ├── minos-loop v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-loop v0.1.0 (*)
      │   │   ├── minos-runner v0.1.0
      │   │   │   └── minos-cli v0.1.0 (*)
      │   │   ├── minos-sandbox v0.1.0
      │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   └── minos-runner v0.1.0 (*)
      │   │   └── minos-store v0.1.0
      │   │       ├── minos-cli v0.1.0 (*)
      │   │       ├── minos-loop v0.1.0 (*)
      │   │       └── minos-runner v0.1.0 (*)
      │   └── (dev) minos-store v0.1.0 (*)
      └── rand v0.9.5
          ├── msb_krun_devices v0.1.41
          │   ├── msb_krun v0.1.41
          │   │   └── minos-vmm v0.1.0
          │   └── msb_krun_vmm v0.1.41
          │       └── msb_krun v0.1.41 (*)
          └── proptest v1.11.0 (*)
    ├ rand_chacha v0.10.0
      └── minos-core v0.1.0
          ├── minos-backend-krun v0.1.0
          │   └── minos-cli v0.1.0
          ├── minos-cli v0.1.0 (*)
          ├── minos-guest v0.1.0
          ├── minos-loop v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-propose v0.1.0
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-loop v0.1.0 (*)
          ├── minos-runner v0.1.0
          │   └── minos-cli v0.1.0 (*)
          ├── minos-sandbox v0.1.0
          │   ├── minos-backend-krun v0.1.0 (*)
          │   ├── minos-cli v0.1.0 (*)
          │   └── minos-runner v0.1.0 (*)
          └── minos-store v0.1.0
              ├── minos-cli v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              └── minos-runner v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:118:1
    │  
118 │ ╭ rand_core 0.9.5 registry+https://github.com/rust-lang/crates.io-index
119 │ │ rand_core 0.10.1 registry+https://github.com/rust-lang/crates.io-index
    │ ╰──────────────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ rand_core v0.9.5
      ├── rand v0.9.5
      │   ├── msb_krun_devices v0.1.41
      │   │   ├── msb_krun v0.1.41
      │   │   │   └── minos-vmm v0.1.0
      │   │   └── msb_krun_vmm v0.1.41
      │   │       └── msb_krun v0.1.41 (*)
      │   └── proptest v1.11.0
      │       ├── (dev) minos-core v0.1.0
      │       │   ├── minos-backend-krun v0.1.0
      │       │   │   └── minos-cli v0.1.0
      │       │   ├── minos-cli v0.1.0 (*)
      │       │   ├── minos-guest v0.1.0
      │       │   ├── minos-loop v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-propose v0.1.0
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-loop v0.1.0 (*)
      │       │   ├── minos-runner v0.1.0
      │       │   │   └── minos-cli v0.1.0 (*)
      │       │   ├── minos-sandbox v0.1.0
      │       │   │   ├── minos-backend-krun v0.1.0 (*)
      │       │   │   ├── minos-cli v0.1.0 (*)
      │       │   │   └── minos-runner v0.1.0 (*)
      │       │   └── minos-store v0.1.0
      │       │       ├── minos-cli v0.1.0 (*)
      │       │       ├── minos-loop v0.1.0 (*)
      │       │       └── minos-runner v0.1.0 (*)
      │       └── (dev) minos-store v0.1.0 (*)
      ├── rand_chacha v0.9.0
      │   ├── proptest v1.11.0 (*)
      │   └── rand v0.9.5 (*)
      └── rand_xorshift v0.4.0
          └── proptest v1.11.0 (*)
    ├ rand_core v0.10.1
      ├── minos-core v0.1.0
      │   ├── minos-backend-krun v0.1.0
      │   │   └── minos-cli v0.1.0
      │   ├── minos-cli v0.1.0 (*)
      │   ├── minos-guest v0.1.0
      │   ├── minos-loop v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-propose v0.1.0
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-loop v0.1.0 (*)
      │   ├── minos-runner v0.1.0
      │   │   └── minos-cli v0.1.0 (*)
      │   ├── minos-sandbox v0.1.0
      │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   └── minos-runner v0.1.0 (*)
      │   └── minos-store v0.1.0
      │       ├── minos-cli v0.1.0 (*)
      │       ├── minos-loop v0.1.0 (*)
      │       └── minos-runner v0.1.0 (*)
      └── rand_chacha v0.10.0
          └── minos-core v0.1.0 (*)

warning[duplicate]: found 2 duplicate entries for crate 'syn'
    ┌─ /Users/seanmcauliffe/Repos/minos/Cargo.lock:142:1
    │  
142 │ ╭ syn 2.0.119 registry+https://github.com/rust-lang/crates.io-index
143 │ │ syn 3.0.6 registry+https://github.com/rust-lang/crates.io-index
    │ ╰───────────────────────────────────────────────────────────────┘ lock entries
    │  
    ├ syn v2.0.119
      ├── darling_core v0.21.3
      │   ├── darling v0.21.3
      │   │   └── deku_derive v0.20.3
      │   │       └── deku v0.20.3
      │   │           └── backhand v0.25.5
      │   │               └── minos-backend-krun v0.1.0
      │   │                   └── minos-cli v0.1.0
      │   └── darling_macro v0.21.3
      │       └── darling v0.21.3 (*)
      ├── darling_macro v0.21.3 (*)
      ├── deku_derive v0.20.3 (*)
      ├── tracing-attributes v0.1.31
      │   └── tracing v0.1.44
      │       ├── minos-cli v0.1.0 (*)
      │       └── msb-imago v0.1.7
      │           └── msb_krun_devices v0.1.41
      │               ├── msb_krun v0.1.41
      │               │   └── minos-vmm v0.1.0
      │               └── msb_krun_vmm v0.1.41
      │                   └── msb_krun v0.1.41 (*)
      └── zerocopy-derive v0.8.62
          └── zerocopy v0.8.62
              ├── kvm-bindings v0.14.2
              │   ├── kvm-ioctls v0.25.1
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   ├── msb_krun_hvf v0.1.41
              │   │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   ├── msb_krun v0.1.41 (*)
              │   ├── msb_krun_arch v0.1.41 (*)
              │   ├── msb_krun_devices v0.1.41 (*)
              │   ├── msb_krun_utils v0.1.41
              │   │   ├── msb_krun v0.1.41 (*)
              │   │   ├── msb_krun_arch v0.1.41 (*)
              │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   ├── msb_krun_kernel v0.1.41
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   ├── msb_krun_polly v0.1.41
              │   │   │   ├── msb_krun v0.1.41 (*)
              │   │   │   ├── msb_krun_devices v0.1.41 (*)
              │   │   │   └── msb_krun_vmm v0.1.41 (*)
              │   │   └── msb_krun_vmm v0.1.41 (*)
              │   └── msb_krun_vmm v0.1.41 (*)
              └── ppv-lite86 v0.2.21
                  ├── rand_chacha v0.10.0
                  │   └── minos-core v0.1.0
                  │       ├── minos-backend-krun v0.1.0 (*)
                  │       ├── minos-cli v0.1.0 (*)
                  │       ├── minos-guest v0.1.0
                  │       ├── minos-loop v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-propose v0.1.0
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-loop v0.1.0 (*)
                  │       ├── minos-runner v0.1.0
                  │       │   └── minos-cli v0.1.0 (*)
                  │       ├── minos-sandbox v0.1.0
                  │       │   ├── minos-backend-krun v0.1.0 (*)
                  │       │   ├── minos-cli v0.1.0 (*)
                  │       │   └── minos-runner v0.1.0 (*)
                  │       └── minos-store v0.1.0
                  │           ├── minos-cli v0.1.0 (*)
                  │           ├── minos-loop v0.1.0 (*)
                  │           └── minos-runner v0.1.0 (*)
                  └── rand_chacha v0.9.0
                      ├── proptest v1.11.0
                      │   ├── (dev) minos-core v0.1.0 (*)
                      │   └── (dev) minos-store v0.1.0 (*)
                      └── rand v0.9.5
                          ├── msb_krun_devices v0.1.41 (*)
                          └── proptest v1.11.0 (*)
    ├ syn v3.0.6
      ├── async-trait v0.1.92
      │   └── msb-imago v0.1.7
      │       └── msb_krun_devices v0.1.41
      │           ├── msb_krun v0.1.41
      │           │   └── minos-vmm v0.1.0
      │           └── msb_krun_vmm v0.1.41
      │               └── msb_krun v0.1.41 (*)
      ├── clap_derive v4.6.7
      │   └── clap v4.6.7
      │       └── minos-cli v0.1.0
      ├── serde_derive v1.0.229
      │   ├── serde v1.0.229
      │   │   ├── bincode v2.0.1
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── insta v1.49.0
      │   │   │   ├── (dev) minos-core v0.1.0
      │   │   │   │   ├── minos-backend-krun v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-guest v0.1.0
      │   │   │   │   ├── minos-loop v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-propose v0.1.0
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-loop v0.1.0 (*)
      │   │   │   │   ├── minos-runner v0.1.0
      │   │   │   │   │   └── minos-cli v0.1.0 (*)
      │   │   │   │   ├── minos-sandbox v0.1.0
      │   │   │   │   │   ├── minos-backend-krun v0.1.0 (*)
      │   │   │   │   │   ├── minos-cli v0.1.0 (*)
      │   │   │   │   │   └── minos-runner v0.1.0 (*)
      │   │   │   │   └── minos-store v0.1.0
      │   │   │   │       ├── minos-cli v0.1.0 (*)
      │   │   │   │       ├── minos-loop v0.1.0 (*)
      │   │   │   │       └── minos-runner v0.1.0 (*)
      │   │   │   └── (dev) minos-propose v0.1.0 (*)
      │   │   ├── kvm-bindings v0.14.2
      │   │   │   ├── kvm-ioctls v0.25.1
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_hvf v0.1.41
      │   │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   ├── msb_krun_utils v0.1.41
      │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   ├── msb_krun_arch v0.1.41 (*)
      │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   ├── msb_krun_kernel v0.1.41
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   ├── msb_krun_polly v0.1.41
      │   │   │   │   │   ├── msb_krun v0.1.41 (*)
      │   │   │   │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   │   └── msb_krun_vmm v0.1.41 (*)
      │   │   ├── minos-cli v0.1.0 (*)
      │   │   ├── minos-core v0.1.0 (*)
      │   │   ├── minos-propose v0.1.0 (*)
      │   │   ├── minos-store v0.1.0 (*)
      │   │   ├── minos-vmm v0.1.0 (*)
      │   │   ├── msb_krun_devices v0.1.41 (*)
      │   │   ├── msb_krun_hvf v0.1.41 (*)
      │   │   ├── msb_krun_vmm v0.1.41 (*)
      │   │   └── trybuild v1.0.122
      │   │       └── (dev) minos-sandbox v0.1.0 (*)
      │   └── trybuild v1.0.122 (*)
      └── thiserror-impl v2.0.21
          └── thiserror v2.0.21
              ├── backhand v0.25.5
              │   └── minos-backend-krun v0.1.0 (*)
              ├── minos-backend-krun v0.1.0 (*)
              ├── minos-core v0.1.0 (*)
              ├── minos-guest v0.1.0 (*)
              ├── minos-loop v0.1.0 (*)
              ├── minos-propose v0.1.0 (*)
              ├── minos-runner v0.1.0 (*)
              ├── minos-sandbox v0.1.0 (*)
              ├── minos-store v0.1.0 (*)
              ├── minos-vmm v0.1.0 (*)
              └── msb-vm-memory v0.18.0-msb.2
                  ├── msb-imago v0.1.7 (*)
                  ├── msb_krun v0.1.41 (*)
                  ├── msb_krun_arch v0.1.41 (*)
                  ├── msb_krun_devices v0.1.41 (*)
                  ├── msb_krun_kernel v0.1.41 (*)
                  ├── msb_krun_smbios v0.1.41
                  │   └── msb_krun_arch v0.1.41 (*)
                  └── msb_krun_vmm v0.1.41 (*)

advisories ok, bans ok, licenses ok, sources ok
```

## Final gate

The four gate commands on the final tree (after the REQUIREMENTS.md and ROADMAP.md wording edits), run from the repository root. Only each command's closing summary lines are shown, verbatim.

Command: `cargo fmt --all --check`

Exit code: 0

```text
(no output)
```

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
```

Command: `cargo test --workspace`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.46s
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
```

Command: `cargo deny check`

Exit code: 0

```text
advisories ok, bans ok, licenses ok, sources ok
```

## Invariant check

CLAUDE.md and rules/ are unchanged by this phase. `git diff --quiet HEAD -- CLAUDE.md rules` returned status 0 (no difference from HEAD), and `git log --format=%h 7195911..HEAD -- CLAUDE.md rules` printed nothing:

```text
```

No lint level was changed and no gate command was added.

## Decisions (resolved)

The user resolved all three in the UAT session of 2026-10-09 (01-UAT.md tests 1 to 3). G-01 to G-03 above are the before-state. The closing proofs are in "Gap closure (01-04)" at the end of this file.

1. **An inner `#![allow(..., reason = "...")]` passed.** Chosen: (B). `unwrap_used`, `expect_used`, and `indexing_slicing` are `forbid` in `Cargo.toml` and in the CLAUDE.md Code rules. Closed: G-01-CLOSED. Consequences: `#[expect]` is no longer possible for those three (R-01). For other lints `#[expect]` still passes (R-02), and the gate still does not catch an inner `allow` with a reason (R-03). `#[derive(clap::Parser)]` fails clippy (F-01), while the clap builder API (F-02) and serde and thiserror derives (F-03) pass.
2. **A crate that omits `[lints] workspace = true` escaped every workspace lint.** Chosen: (A) and (B). (A) is D-15 in 01-CONTEXT.md: every plan that adds a crate runs the C-01 check. (B) is `.cargo/config.toml` with `[build] rustflags = ["-Funsafe_code", "-Fclippy::unwrap_used", "-Fclippy::expect_used", "-Fclippy::indexing_slicing"]`, at `forbid` to match decision 1. Closed: G-02-CLOSED. Dependencies are unaffected: B-01. Caveats: the four lints are duplicated, and a `RUSTFLAGS` environment variable silently replaces the backstop (shown in G-02-CLOSED).
3. **`broken_intra_doc_links = "deny"` was enforced only by `cargo doc`.** Chosen: (A). `cargo doc --workspace --no-deps` is the fifth gate command in CLAUDE.md Commands, ROADMAP Phase 1 criterion 1, REQUIREMENTS GATE-02, and 01-CONTEXT.md D-01. Closed: G-03-CLOSED, with no `RUSTDOCFLAGS`.


## cargo clean

Run from the repository root after every verification above passed (GATE-06). No cargo command was run after it.

Command: `cargo clean`

Exit code: 0

```text
     Removed 4871 files, 1.1GiB total
```

## Gap closure (01-04)

Date: 2026-10-09

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- HEAD at start of closure: `e0ca5ece62c21fc5fbc85c06662cc761098210cd`

This section applies the user's three decisions from 01-UAT.md tests 1 to 3 and records each closing proof under the same protocol as the proofs above: one edit at a time, the gate command, verbatim output, and a revert from HEAD. The G-01 to G-03 sections above are the before-state and are unchanged.

### G-01-CLOSED: inner allow with a reason under forbid

Requirement: GATE-05, UAT test 1

Edit: crates/minos-store/src/lib.rs: inserted `#![allow(clippy::unwrap_used, reason = "gate gap demonstration")]` on the line after the `//!` doc, and appended `/// Gate self-test probe.`, a `# Panics` section (`/// Panics if the input is not a number.`), `#[must_use]`, `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }`

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error[E0453]: allow(clippy::unwrap_used) incompatible with previous forbid
 --> crates/minos-store/src/lib.rs:2:10
  |
2 | #![allow(clippy::unwrap_used, reason = "gate gap demonstration")]
  |          ^^^^^^^^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::unwrap_used`)

For more information about this error, try `rustc --explain E0453`.
error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### R-01: V-07 re-run, expect on a forbidden lint

Requirement: GATE-05, D-02

Edit: crates/minos-store/src/lib.rs: appended `/// Gate self-test probe.`, a `# Panics` section (`/// Panics if the slice is empty.`), `#[expect(clippy::indexing_slicing, reason = "gate self-test positive control")]`, `#[must_use]`, `pub fn gate_probe(x: &[u8]) -> u8 { x[0] }` (the 01-03 V-07 probe)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error[E0453]: expect(clippy::indexing_slicing) incompatible with previous forbid
 --> crates/minos-store/src/lib.rs:8:10
  |
8 | #[expect(clippy::indexing_slicing, reason = "gate self-test positive control")]
  |          ^^^^^^^^^^^^^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::indexing_slicing`)

For more information about this error, try `rustc --explain E0453`.
error: could not compile `minos-store` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-store` (lib test) due to 1 previous error
```

### R-02: expect on a lint that is not forbidden

Requirement: GATE-05, D-02

Edit: crates/minos-store/src/lib.rs: appended `/// Gate self-test probe.`, `#[expect(clippy::get_first, reason = "gate self-test positive control")]`, `#[must_use]`, `pub fn gate_probe(x: &[u8]) -> Option<&u8> { x.get(0) }` (no Panics section)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
```

### R-03: inner allow with a reason on a lint that is not forbidden

Requirement: GATE-05, residual of UAT test 1

Edit: crates/minos-store/src/lib.rs: inserted `#![allow(clippy::get_first, reason = "gate residual demonstration")]` on the line after the `//!` doc, and appended `/// Gate self-test probe.`, `#[must_use]`, `pub fn gate_probe(x: &[u8]) -> Option<&u8> { x.get(0) }` (R-02's probe without its expect attribute; no Panics section)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s
```

### F-01: clap derive under forbid

Requirement: D-12, consequence of decision B (UAT test 1)

Edit: crates/minos-cli/src/main.rs: replaced the line `fn main() {}` with `/// Gate self-test probe.`, `#[derive(clap::Parser)]`, `struct GateProbe { /// Gate self-test argument. #[arg(long)] n: u8 }`, and `fn main() { let probe = <GateProbe as clap::Parser>::parse(); let _ = probe.n; }`

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error[E0453]: allow(clippy::restriction) incompatible with previous forbid
 --> crates/minos-cli/src/main.rs:4:10
  |
4 | #[derive(clap::Parser)]
  |          ^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::indexing_slicing`)
  = note: this error originates in the derive macro `clap::Parser` (in Nightly builds, run with -Z macro-backtrace for more info)

error[E0453]: allow(clippy::restriction) incompatible with previous forbid
 --> crates/minos-cli/src/main.rs:4:10
  |
4 | #[derive(clap::Parser)]
  |          ^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::expect_used`)
  = note: this error originates in the derive macro `clap::Parser` (in Nightly builds, run with -Z macro-backtrace for more info)

error[E0453]: allow(clippy::restriction) incompatible with previous forbid
 --> crates/minos-cli/src/main.rs:4:10
  |
4 | #[derive(clap::Parser)]
  |          ^^^^^^^^^^^^ overruled by previous forbid
  |
  = note: `forbid` lint level was set on command line (`-F clippy::unwrap_used`)
  = note: this error originates in the derive macro `clap::Parser` (in Nightly builds, run with -Z macro-backtrace for more info)

For more information about this error, try `rustc --explain E0453`.
error: could not compile `minos-cli` (bin "minos") due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-cli` (bin "minos" test) due to 3 previous errors
```

### F-02: clap builder API under forbid

Requirement: D-12, positive control

Edit: crates/minos-cli/src/main.rs: replaced the line `fn main() {}` with `fn main() { let matches = clap::Command::new("minos").arg(clap::Arg::new("n").long("n")).get_matches(); let _ = matches.get_one::<String>("n"); }`

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

### F-03: serde and thiserror derives under forbid

Requirement: D-12, positive control

Edit: crates/minos-store/src/lib.rs: appended `/// Gate self-test probe.`, `#[derive(Debug, serde::Serialize, serde::Deserialize)]`, `#[serde(deny_unknown_fields)]`, `pub struct GateProbe { /// Gate self-test field. pub n: u8 }`, then `/// Gate self-test probe error.`, `#[derive(Debug, thiserror::Error)]`, `pub enum GateProbeError { /// Gate self-test variant. #[error("gate probe {0}")] Probe(u8) }`

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.21s
```

### G-02-CLOSED: crate without inherited lints under the rustflags backstop

Requirement: GATE-01, UAT test 2

Edit (applies to both runs): crates/minos-loop/Cargo.toml: deleted the two lines `[lints]` and `workspace = true`; crates/minos-loop/src/lib.rs: appended `pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }`, `pub fn gate_probe_expect(s: &str) -> u8 { s.parse::<u8>().expect("n") }`, `pub fn gate_probe_index(x: &[u8]) -> u8 { x[0] }`, and `pub fn gate_probe_unsafe(x: &[u8]) -> u8 { unsafe { *x.get_unchecked(0) } }` (no doc, no attribute)

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 101

```text
error: usage of an `unsafe` block
 --> crates/minos-loop/src/lib.rs:5:44
  |
5 | pub fn gate_probe_unsafe(x: &[u8]) -> u8 { unsafe { *x.get_unchecked(0) } }
  |                                            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: requested on the command line with `-F unsafe-code`

error: used `unwrap()` on a `Result` value
 --> crates/minos-loop/src/lib.rs:2:36
  |
2 | pub fn gate_probe(s: &str) -> u8 { s.parse::<u8>().unwrap() }
  |                                    ^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: if this value is an `Err`, it will panic
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#unwrap_used
  = note: requested on the command line with `-F clippy::unwrap-used`

error: used `expect()` on a `Result` value
 --> crates/minos-loop/src/lib.rs:3:43
  |
3 | pub fn gate_probe_expect(s: &str) -> u8 { s.parse::<u8>().expect("n") }
  |                                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: if this value is an `Err`, it will panic
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#expect_used
  = note: requested on the command line with `-F clippy::expect-used`

error: indexing may panic
 --> crates/minos-loop/src/lib.rs:4:43
  |
4 | pub fn gate_probe_index(x: &[u8]) -> u8 { x[0] }
  |                                           ^^^^
  |
  = help: consider using `.get(n)` or `.get_mut(n)` instead
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#indexing_slicing
  = note: requested on the command line with `-F clippy::indexing-slicing`

error: could not compile `minos-loop` (lib) due to 4 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `minos-loop` (lib test) due to 4 previous errors
```

Command: `RUSTFLAGS= cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

### C-03-RERUN: no unknown lint name with the backstop

Requirement: RESEARCH.md Pitfall 3

Command: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -ci 'unknown lint'`

```text
0
```

### B-01: untouched tree under the rustflags backstop

Requirement: GATE-01, GATE-02

Edit: none (untouched tree)

For `cargo build` every line except cargo progress lines is shown. For `cargo test` and `cargo deny check` only the closing summary lines are shown, verbatim.

Command: `cargo build --workspace`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.29s
```

Command: `cargo test --workspace`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.02s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-52d5905ac48406d1)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos-318232a1b6d97755)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_core-c619c5be5a10c8c9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_guest-3a96ca21e5db09e7)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_loop-994e138925915d56)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_propose-1a9a8fd769d6a47b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_runner-89243ff6043495d6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-dcc21162d5b68e3e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_store-4edbb133c2d25261)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_vmm-0703513a634d49f4)
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
```

Command: `cargo deny check`

Exit code: 0

```text
advisories ok, bans ok, licenses ok, sources ok
```

### G-03-CLOSED: broken intra-doc link under cargo doc

Requirement: GATE-01, D-12, UAT test 3

Edit (applies to run 2 only; run 1 is on the untouched tree): crates/minos-store/src/lib.rs: appended `pub const GATE_PROBE: u8 = 0;` with the doc line ``/// Gate probe that links to [`DoesNotExist`].``

Command: `cargo doc --workspace --no-deps`

Exit code: 0

```text
 Documenting minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
 Documenting minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
   Generated /Users/seanmcauliffe/Repos/minos/target/doc/minos_backend_krun/index.html and 9 other files
```

Command: `cargo doc --workspace --no-deps`

Exit code: 101

```text
 Documenting minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
 Documenting minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
 Documenting minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
 Documenting minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
error: unresolved link to `DoesNotExist`
 --> crates/minos-store/src/lib.rs:3:32
  |
3 | /// Gate probe that links to [`DoesNotExist`].
  |                                ^^^^^^^^^^^^ no item named `DoesNotExist` in scope
  |
  = help: to escape `[` and `]` characters, add '\' before them like `\[` or `\]`
  = note: requested on the command line with `-D rustdoc::broken-intra-doc-links`

error: could not document `minos-store`
warning: build failed, waiting for other jobs to finish...
```

RUSTDOCFLAGS check: `printenv RUSTDOCFLAGS` printed nothing, and `grep -c rustdocflags .cargo/config.toml` printed `0`.

### C-01-RERUN: every crate manifest inherits the workspace lints

Requirement: D-15

Command: `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`

```text
ok crates/minos-backend-krun/Cargo.toml
ok crates/minos-cli/Cargo.toml
ok crates/minos-core/Cargo.toml
ok crates/minos-guest/Cargo.toml
ok crates/minos-loop/Cargo.toml
ok crates/minos-propose/Cargo.toml
ok crates/minos-runner/Cargo.toml
ok crates/minos-sandbox/Cargo.toml
ok crates/minos-store/Cargo.toml
ok crates/minos-vmm/Cargo.toml
```

### Final gate (five commands)

The five gate commands on the final tree (after the 01-CONTEXT.md, REQUIREMENTS.md, ROADMAP.md, and PROJECT.md wording edits), run from the repository root. Only each command's closing summary lines are shown, verbatim.

Command: `cargo fmt --all --check`

Exit code: 0

```text
(no output)
```

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

Command: `cargo test --workspace`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-52d5905ac48406d1)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos-318232a1b6d97755)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_core-c619c5be5a10c8c9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_guest-3a96ca21e5db09e7)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_loop-994e138925915d56)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_propose-1a9a8fd769d6a47b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_runner-89243ff6043495d6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-dcc21162d5b68e3e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_store-4edbb133c2d25261)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_vmm-0703513a634d49f4)
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
```

Command: `cargo deny check`

Exit code: 0

```text
advisories ok, bans ok, licenses ok, sources ok
```

Command: `cargo doc --workspace --no-deps`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
   Generated /Users/seanmcauliffe/Repos/minos/target/doc/minos_backend_krun/index.html and 9 other files
```

### Invariant check (01-04)

Command: `git diff --numstat c4b5bab HEAD -- CLAUDE.md`

```text
6	3	CLAUDE.md
```

Command: `git diff --quiet c4b5bab HEAD -- rules; echo $?`

```text
0
```

Command: `git ls-files .cargo`

```text
.cargo/config.toml
```

`git diff c4b5bab HEAD -- CLAUDE.md` shows changes in exactly two blocks: the Code rules TOML fence (the three clippy lints from `"deny"` to `"forbid"`, plus the one-sentence paragraph and a blank line after the fence) and the Commands block (the new `cargo doc --workspace --no-deps` line); rules/ is unchanged.

### cargo clean (01-04)

Run from the repository root after every verification above passed (GATE-06). No cargo command was run after it.

Command: `cargo clean`

Exit code: 0

```text
     Removed 5800 files, 1.3GiB total
```

## Review fix (01-REVIEW.md CR-01): RUSTSEC-2025-0141 recorded

Finding: `deny.toml` had `[advisories] unmaintained = "workspace"`, which reports unmaintained advisories for direct workspace dependencies only. With cargo-deny 0.20.2's default scope (`"all"`), the advisories check fails on `bincode 2.0.1`, reachable only through `msb_krun_devices` and `msb_krun_vmm` inside `minos-vmm`, the hypervisor-entitled binary. The hit was never reported.

Decision (2026-10-09): restore the default scope and carry the advisory as one named, reasoned `ignore` entry. Scope narrowing is a blanket, unnamed ignore of every present and future transitive unmaintained advisory; an `ignore` entry names the advisory, carries a reason, and makes cargo-deny warn when it stops matching. `bincode` has no replacement without replacing `msb_krun`; the VMM is replaced, not patched. Re-review on every `msb_krun` bump. Also recorded next to the `msb_krun` row in `.planning/research/STACK.md`.

Reproduction before the fix. Command: `cargo deny --config <scratch copy with unmaintained = "all"> check advisories`

```text
error[unmaintained]: Bincode is unmaintained
   ├ ID: RUSTSEC-2025-0141
   ├ Advisory: https://rustsec.org/advisories/RUSTSEC-2025-0141
advisories FAILED
```

After the fix. Command: `cargo deny check`

Exit code: 0

```text
advisories ok, bans ok, licenses ok, sources ok
```

No `target/` directory was created (`cargo deny` only resolves the graph).

## Review fix (01-REVIEW.md WR-03): minos-core purity bans extended

`crates/minos-core/clippy.toml` now bans `std::thread::sleep`, `std::env::{var, var_os, args}`, `std::fs::{read, read_to_string, write}`, and `std::io::stdin` (methods); `std::fs::File`, `std::process::Command`, and `std::net::{TcpStream, TcpListener, UdpSocket}` (types); and `std::println`, `std::eprintln`, and `std::dbg` (macros), alongside the clock and hash-collection entries proven in V-09 and V-10.

### C-03b: no invalid disallowed path with the extended tables

Command: `cargo clippy -p minos-core --all-targets -- -D warnings` on the untouched crate

Exit code: 0. `grep -ciE "unknown path|invalid path|could not resolve|disallowed"` over the output: 0.

### V-09b / V-10b: one probe per table fires

A throwaway function appended to `crates/minos-core/src/lib.rs` (reverted with `git checkout --` afterward, not committed) used `std::env::var` (method), `std::fs::File` (type), and `println!` (macro).

Command: `cargo clippy -p minos-core --all-targets -- -D warnings`

Exit code: 101

```text
1 error: use of a disallowed macro `std::println`
1 error: use of a disallowed method `std::env::var`
2 error: use of a disallowed type `std::fs::File`
```

## Review fix (01-REVIEW.md WR-04): C-linking helpers banned

`deny.toml` `[bans] deny` gains `pkg-config`, `vcpkg`, `cmake`, `bindgen`, `bzip2-sys`, `libz-sys`, `aws-lc-sys`, and `ring`. `cargo tree -i <crate>` for both checked targets found none of them in the aarch64-apple-darwin or aarch64-unknown-linux-musl graph (`pkg-config` and `bzip2-sys` exist in the lock only for x86_64 arms of `msb_krun_vmm`).

Command: `cargo deny check`

Exit code: 0

```text
advisories ok, bans ok, licenses ok, sources ok
```
