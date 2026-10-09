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
