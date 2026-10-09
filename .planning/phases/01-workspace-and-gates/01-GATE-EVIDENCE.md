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

## Open decisions for the user

The four gate commands cannot catch these three gaps on stable Rust 1.97.1. Each was verified in research (RESEARCH.md Open Questions 1 to 3) and is demonstrated again as G-01 to G-03 in 01-GATE-EVIDENCE.md. This phase does not resolve them: it does not change the CLAUDE.md lint levels and it does not add a gate command.

1. **An inner `#![allow(..., reason = "...")]` passes.** clippy's `allow_attributes` only lints outer `#[allow]`. `#![allow(clippy::unwrap_used, reason = "...")]` plus an `unwrap()` passes `cargo clippy --workspace --all-targets -- -D warnings`. A reasonless inner allow is still caught by `allow_attributes_without_reason`. Options: (A) accept and document the gap; (B) raise `unwrap_used`, `expect_used`, and `indexing_slicing` to `forbid`, which also makes `#[expect]` impossible for those three and is an edit to CLAUDE.md Code rules.
2. **A crate that omits `[lints] workspace = true` escapes every workspace lint.** Cargo rejects a partial override but not a missing table. Options: (A) the one-time check this phase runs, plus review of every new crate manifest; (B) a `.cargo/config.toml` `[build] rustflags` backstop (`-Funsafe_code`, `-Dclippy::unwrap_used`, `-Dclippy::expect_used`, `-Dclippy::indexing_slicing`), which duplicates the lint list, is silently overridden by a `RUSTFLAGS` environment variable, and is not one of D-01's policy files.
3. **`broken_intra_doc_links = "deny"` is enforced only by `cargo doc`.** `cargo clippy` and `cargo test` ignore `[workspace.lints.rustdoc]`; only `cargo doc --workspace --no-deps` fails on a broken link. Options: (A) add `cargo doc --workspace --no-deps` as a fifth command, which changes CLAUDE.md Commands and ROADMAP Phase 1 criterion 1; (B) document that the rustdoc table takes effect only when docs are built.


## cargo clean

Run from the repository root after every verification above passed (GATE-06). No cargo command was run after it.

Command: `cargo clean`

Exit code: 0

```text
     Removed 4871 files, 1.1GiB total
```
