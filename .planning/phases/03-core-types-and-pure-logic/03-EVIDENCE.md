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

<!-- final-gate -->
