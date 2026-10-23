//! The cumulative-diff invariant as a compile-time fact (07-CONTEXT D-12, 03-REVIEW WR-03).
//!
//! Each file under `tests/ui/fail/` is a minimal program that must not compile, and its `.stderr` holds the one
//! error that proves the invariant: a parent `Tree` cannot stand in for the run's `BaseTree` when a
//! `CumulativeDiff` is minted, and a bare `Diff` cannot become an `Experiment`, so a diff relative to the parent,
//! or a `Diff::parse` result, never becomes an `ExperimentId` by mistake.
//!
//! Every `.stderr` is committed and was reviewed to hold exactly the intended error. The gate never sets
//! `TRYBUILD`, so a changed or missing `.stderr` fails the test instead of being written; this is the same
//! discipline as insta's `update: no`. The stderr text is pinned by the rustc toolchain pin in
//! `rust-toolchain.toml`: a toolchain bump regenerates the files with `TRYBUILD=overwrite`, and each one is
//! reviewed again before it is committed.

#[test]
fn cumulative_diff_invariants_are_compile_time_facts() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/fail/*.rs");
}
