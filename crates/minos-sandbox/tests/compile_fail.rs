//! The sandbox invariants as compile-time facts (SBX-01, SBX-02, SBX-03, SBX-04, D-14).
//!
//! Each file under `tests/ui/fail/` is a minimal program that must not compile, and its `.stderr` holds the one
//! error that proves the invariant: a Candidate spec cannot take judge data, no role has a network method,
//! `LaunchPlan` and `Cleared` cannot be built with a struct literal, `Cleared` cannot be cloned, one `Cleared` cannot
//! build two `Finished` values, a backend cannot launch a bare `LaunchPlan`, `Untrusted` refuses `{:?}`, `{}`, `*`, and `.0`, and candidate output has no
//! `into_scorecard`. The one file under `tests/ui/pass/` runs the whole judge flow, so a broken fixture cannot make
//! every fail case pass for the wrong reason.
//!
//! Every `.stderr` is committed and was reviewed to hold exactly the intended error. The gate never sets
//! `TRYBUILD`, so a changed or missing `.stderr` fails the test instead of being written; this is the same
//! discipline as insta's `update: no`. The stderr text is pinned by the rustc 1.97.1 toolchain pin in
//! `rust-toolchain.toml`: a toolchain bump regenerates the files with `TRYBUILD=overwrite`, and each one is
//! reviewed again before it is committed.

#[test]
fn sandbox_invariants_are_compile_time_facts() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/fail/*.rs");
    cases.pass("tests/ui/pass/*.rs");
}
