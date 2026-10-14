# Phase 3 Evidence: deliberate violations and the final gate

Recorded 2026-10-14 by plan 03-13 on macOS 27.0 arm64 (Darwin 27.0.0), toolchain pinned by `rust-toolchain.toml`.

Each violation section gives the test, the input, the exact error Display string the test asserts (copied from the test source), the verbatim output of the test run with `--exact`, and a probe run. In the probe, the expected string in the test was temporarily replaced with a wrong value, so the failing assertion prints the real error Display as `left:`; the test file was then restored with `git checkout -- <file>`, and no probe edit was committed. Output lines that are only blank or cargo progress (`Compiling`, `Finished`, `Running`) are omitted.

## 1. Diff::parse rejects a rename

- **Test:** `rejects_rename_header` in `crates/minos-core/tests/prop_diff.rs`
- **Input:** The valid one-file diff with `rename from src/old.rs` and `rename to src/parse.rs` inserted after its `diff --git` line. `Diff::parse` returns `DiffError::ForbiddenHeader { line: 2, header: ForbiddenHeader::Rename }` (D-02: a diff may only modify an existing writable file).
- **Asserted Display string:** `line 2: forbidden header: rename`

### Test run

```
$ cargo test -p minos-core --test prop_diff rejects_rename_header -- --exact
running 1 test
test rejects_rename_header ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 66 filtered out; finished in 0.00s
exit status: 0
```

### Probe: the actual rejection text

```
$ cargo test -p minos-core --test prop_diff rejects_rename_header -- --exact   # expected text temporarily replaced
running 1 test
test rejects_rename_header ... FAILED
failures:
---- rejects_rename_header stdout ----
thread 'rejects_rename_header' (2818571) panicked at crates/minos-core/tests/prop_diff.rs:434:5:
assertion `left == right` failed
  left: "line 2: forbidden header: rename"
 right: "PROBE: deliberately wrong expected text"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    rejects_rename_header
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 66 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p minos-core --test prop_diff`
exit status: 101
```

## 2. Manifest::try_from rejects a writable/readable overlap

- **Test:** `rejects_writable_overlapping_readable` in `crates/minos-core/tests/manifest.rs`
- **Input:** The CLAUDE.md example manifest with `readable = ["src/lib.rs", "src/parse.rs"]`, so `src/parse.rs` is both writable and readable. `Manifest::parse_toml` deserializes the raw manifest and calls `Manifest::try_from`, which returns `ManifestError::Overlap { path: src/parse.rs, with: "surface.readable" }` (D-01).
- **Asserted Display string:** `src/parse.rs is writable, so it cannot also be in surface.readable`

### Test run

```
$ cargo test -p minos-core --test manifest rejects_writable_overlapping_readable -- --exact
running 1 test
test rejects_writable_overlapping_readable ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 88 filtered out; finished in 0.00s
exit status: 0
```

### Probe: the actual rejection text

```
$ cargo test -p minos-core --test manifest rejects_writable_overlapping_readable -- --exact   # expected text temporarily replaced
running 1 test
test rejects_writable_overlapping_readable ... FAILED
failures:
---- rejects_writable_overlapping_readable stdout ----
thread 'rejects_writable_overlapping_readable' (2818752) panicked at crates/minos-core/tests/manifest.rs:916:5:
assertion `left == right` failed
  left: "src/parse.rs is writable, so it cannot also be in surface.readable"
 right: "PROBE: deliberately wrong expected text"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    rejects_writable_overlapping_readable
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 88 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p minos-core --test manifest`
exit status: 101
```

## 3. Metric::new rejects NaN

- **Test:** `metric_rejects_nan_with_a_fixed_message` in `crates/minos-core/tests/prop_metric.rs`
- **Input:** `Metric::new(f64::NAN)` returns `MetricError::NotFinite` (CORE-06: a NaN never reaches policy code).
- **Asserted Display string:** `a metric must be a finite number`

### Test run

```
$ cargo test -p minos-core --test prop_metric metric_rejects_nan_with_a_fixed_message -- --exact
running 1 test
test metric_rejects_nan_with_a_fixed_message ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 34 filtered out; finished in 0.00s
exit status: 0
```

### Probe: the actual rejection text

```
$ cargo test -p minos-core --test prop_metric metric_rejects_nan_with_a_fixed_message -- --exact   # expected text temporarily replaced
running 1 test
test metric_rejects_nan_with_a_fixed_message ... FAILED
failures:
---- metric_rejects_nan_with_a_fixed_message stdout ----
thread 'metric_rejects_nan_with_a_fixed_message' (2818993) panicked at crates/minos-core/tests/prop_metric.rs:59:9:
assertion `left == right` failed
  left: "a metric must be a finite number"
 right: "PROBE: deliberately wrong expected text"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    metric_rejects_nan_with_a_fixed_message
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 34 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p minos-core --test prop_metric`
exit status: 101
```

## 4. Rate::parse rejects 13 fractional digits

- **Test:** `rate_rejects_a_thirteenth_fractional_digit_without_rounding` in `crates/minos-core/tests/money.rs`
- **Input:** `Rate::parse("1.0000000000001")` returns `RateError::FractionTooLong`. The rate is refused, never rounded to `1.000000000000` (CORE-14: pico-dollar precision, fail closed).
- **Asserted Display string:** `a rate has at most 12 fractional digits and is never rounded`

### Test run

```
$ cargo test -p minos-core --test money rate_rejects_a_thirteenth_fractional_digit_without_rounding -- --exact
running 1 test
test rate_rejects_a_thirteenth_fractional_digit_without_rounding ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.00s
exit status: 0
```

### Probe: the actual rejection text

```
$ cargo test -p minos-core --test money rate_rejects_a_thirteenth_fractional_digit_without_rounding -- --exact   # expected text temporarily replaced
running 1 test
test rate_rejects_a_thirteenth_fractional_digit_without_rounding ... FAILED
failures:
---- rate_rejects_a_thirteenth_fractional_digit_without_rounding stdout ----
thread 'rate_rejects_a_thirteenth_fractional_digit_without_rounding' (2819215) panicked at crates/minos-core/tests/money.rs:111:5:
assertion `left == right` failed
  left: "a rate has at most 12 fractional digits and is never rounded"
 right: "PROBE: deliberately wrong expected text"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    rate_rejects_a_thirteenth_fractional_digit_without_rounding
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p minos-core --test money`
exit status: 101
```

## 5. fold rejects a duplicate RunStarted

- **Test:** `a_second_different_run_started_is_refused` in `crates/minos-core/tests/prop_fold.rs`
- **Input:** A valid ledger plus a second `RunStarted` whose `started_at` (and so its `RunId`) differs from the first. `fold` returns `FoldError::DuplicateRunStarted`. An exactly equal second `RunStarted` is a duplicate replay and is skipped; a different one is refused (CORE-09).
- **Asserted Display string:** `the ledger has more than one RunStarted record`

### Test run

```
$ cargo test -p minos-core --test prop_fold a_second_different_run_started_is_refused -- --exact
running 1 test
test a_second_different_run_started_is_refused ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.00s
exit status: 0
```

### Probe: the actual rejection text

```
$ cargo test -p minos-core --test prop_fold a_second_different_run_started_is_refused -- --exact   # expected text temporarily replaced
running 1 test
test a_second_different_run_started_is_refused ... FAILED
failures:
---- a_second_different_run_started_is_refused stdout ----
thread 'a_second_different_run_started_is_refused' (2819486) panicked at crates/minos-core/tests/prop_fold.rs:475:5:
assertion `left == right` failed
  left: "the ledger has more than one RunStarted record"
 right: "PROBE: deliberately wrong expected text"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
failures:
    a_second_different_run_started_is_refused
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p minos-core --test prop_fold`
exit status: 101
```

## Final gate

Run on 2026-10-14 at `ec1eba6` (the commit after Task 2), in the order plan 03-13 Task 3 lists, from the repository root. Each command's output and exit status were captured to files and are copied here. Every check passed, so `cargo clean` ran afterwards as the last action of the phase; its output is recorded in 03-13-SUMMARY.md, not here, so the working tree stays clean after this commit.

| # | Check | Exit status | Result |
|---|---|---|---|
| 1 | `cargo fmt --all --check` | 0 | no output |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warning |
| 3 | `cargo test --workspace` | 0 | 40 test binaries, 713 passed, 0 failed, 25 ignored, all `#[ignore]` by design: the 24 VM spike tests and the `minos-backend-krun` initramfs cpio cross-check, which need a signed `minos-vmm` and environment variables |
| 4 | `cargo deny check` | 0 | `advisories ok, bans ok, licenses ok, sources ok`; warnings only (duplicate versions, and the existing unused-wrapper entries for crates that do not link core or sandbox yet) |
| 5 | `cargo doc --workspace --no-deps` | 0 | no warning |
| 6 | C-01 `[lints] workspace = true` in every crate | 0 | 10 `ok` lines, no `MISSING` |
| 7 | C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | 1 | no output (grep exits 1 when nothing matches, which is the pass) |
| 8 | `cargo check -p minos-core --target aarch64-unknown-linux-musl` | 0 | core builds for the guest's target |
| 8b | `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` | 0 | no warning (the sixth CLAUDE.md gate command, Phase 2 onward) |
| 9 | `git log --oneline cf607f2..HEAD -- crates/minos-core/clippy.toml` | 0 | no output: core's clippy.toml was not changed in the phase |
| 10 | `.snap.new` count and `git status --porcelain crates/minos-core` | 0 | `0`, and no uncommitted file under crates/minos-core |

### 1. Format

```
$ cargo fmt --all --check
(no output)
exit status: 0
```

### 2. Clippy

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
    Checking minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
    Checking minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
    Checking minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
    Checking minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
    Checking minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
    Checking minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.68s
exit status: 0
```

### 3. Tests

One line per test binary: the `Running` or `Doc-tests` line (dependency hash removed) and its `test result` line (timing removed). The full 986-line output passed with no `FAILED` or `panicked` line.

```
$ cargo test --workspace
Running unittests src/lib.rs [minos_backend_krun]  ->  test result: ok. 74 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
Running tests/spike_boot.rs [spike_boot]  ->  test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out
Running tests/spike_exits.rs [spike_exits]  ->  test result: ok. 0 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out
Running tests/spike_net.rs [spike_net]  ->  test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
Running tests/spike_rootimg.rs [spike_rootimg]  ->  test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
Running tests/spike_seatbelt.rs [spike_seatbelt]  ->  test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out
Running unittests src/main.rs [minos]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_core]  ->  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/breaker.rs [breaker]  ->  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/golden_ids.rs [golden_ids]  ->  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/golden_wire.rs [golden_wire]  ->  test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/manifest.rs [manifest]  ->  test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/money.rs [money]  ->  test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_apply.rs [prop_apply]  ->  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_diff.rs [prop_diff]  ->  test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_fold.rs [prop_fold]  ->  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_keys.rs [prop_keys]  ->  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_metric.rs [prop_metric]  ->  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_path.rs [prop_path]  ->  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/prop_policy.rs [prop_policy]  ->  test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/render.rs [render]  ->  test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/report.rs [report]  ->  test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/route.rs [route]  ->  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests/sweep.rs [sweep]  ->  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_guest]  ->  test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/main.rs [minos_guest]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_loop]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_propose]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_runner]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_sandbox]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/lib.rs [minos_store]  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running unittests src/main.rs [minos_vmm]  ->  test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_backend_krun  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_core  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_guest  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_loop  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_propose  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_runner  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_sandbox  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests minos_store  ->  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit status: 0
```

### 4. cargo-deny

Each warning's headline and location, then the final summary line; the dependency trees under each duplicate are omitted.

```
$ cargo deny check
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'   at Cargo.lock:16:1
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'   at Cargo.lock:52:1
warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'   at Cargo.lock:74:1
warning[duplicate]: found 2 duplicate entries for crate 'nix'   at Cargo.lock:98:1
warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'   at Cargo.lock:114:1
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'   at Cargo.lock:116:1
warning[duplicate]: found 2 duplicate entries for crate 'syn'   at Cargo.lock:140:1
warning[unused-wrapper]: wrapper for banned crate was not encountered   at deny.toml:65:72
warning[unused-wrapper]: wrapper for banned crate was not encountered   at deny.toml:65:94
warning[unused-wrapper]: wrapper for banned crate was not encountered   at deny.toml:67:43
advisories ok, bans ok, licenses ok, sources ok
exit status: 0
```

### 5. Docs

```
$ cargo doc --workspace --no-deps
    Checking minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
 Documenting minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
    Checking minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
    Checking minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
    Checking minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
 Documenting minos-store v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-store)
 Documenting minos-sandbox v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-sandbox)
 Documenting minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
    Checking minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
 Documenting minos-runner v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-runner)
    Checking minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
 Documenting minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
 Documenting minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.88s
   Generated /Users/seanmcauliffe/Repos/minos/target/doc/minos_backend_krun/index.html and 9 other files
exit status: 0
```

### 6. C-01

```
$ for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done
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
exit status: 0
```

### 7. C-02

```
$ grep -rnE '#!?\[allow\(' crates --include='*.rs'
(no output)
exit status: 1
```

### 8. minos-core for the guest's target

```
$ cargo check -p minos-core --target aarch64-unknown-linux-musl
   Compiling serde_core v1.0.229
    Checking typenum v1.20.1
   Compiling zerocopy v0.8.62
   Compiling libc v0.2.190
    Checking winnow v1.0.4
    Checking const-oid v0.10.2
    Checking foldhash v0.2.0
    Checking toml_writer v1.1.3+spec-1.1.0
    Checking rand_core v0.10.1
    Checking base16ct v1.0.0
    Checking hashbrown v0.17.1
    Checking toml_parser v1.1.5+spec-1.1.0
    Checking diffy v0.5.2
    Checking hybrid-array v0.4.15
    Checking block-buffer v0.12.1
    Checking crypto-common v0.2.2
    Checking cpufeatures v0.3.1
    Checking digest v0.11.3
    Checking serde_spanned v1.1.2
    Checking toml_datetime v1.1.2+spec-1.1.0
    Checking serde_json v1.0.151
    Checking serde v1.0.229
    Checking sha2 v0.11.0
    Checking toml v1.1.8+spec-1.1.0
    Checking ppv-lite86 v0.2.21
    Checking rand_chacha v0.10.0
    Checking minos-core v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.60s
exit status: 0
```

### 8b. Guest clippy for its target

```
$ cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
exit status: 0
```

### 9. Core clippy.toml unchanged

```
$ git log --oneline cf607f2..HEAD -- crates/minos-core/clippy.toml
(no output)
exit status: 0
```

### 10. No pending snapshots or regressions

Run under `/bin/sh`. The first run under zsh printed the same `0` plus zsh's `no matches found` message for the unmatched glob, which is a shell difference, not a finding.

```
$ ls crates/minos-core/tests/snapshots/*.snap.new 2>/dev/null | wc -l; git status --porcelain crates/minos-core
0
exit status: 0
```
