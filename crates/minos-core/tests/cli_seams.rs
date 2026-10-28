//! The small core seams the Phase 8 CLI composes (plan 08-01): the D-08 advisory arithmetic in `limits`
//! (`batch_output_tokens`, `near_output_cap`), the one run-cost sum `policy::run_cost`, and
//! `RunState::proposals`.

use minos_core::limits::{
    BYTES_PER_TOKEN_CONSERVATIVE, CLI_DEFAULT_OUTPUT_TOKENS, batch_output_tokens, near_output_cap,
};

// ---------------------------------------------------------------------------------------------------------------
// D-08 advisory arithmetic
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn d08_constants_are_pinned() {
    assert_eq!(CLI_DEFAULT_OUTPUT_TOKENS, 32_000);
    assert_eq!(BYTES_PER_TOKEN_CONSERVATIVE, 2);
}

#[test]
fn batch_output_tokens_is_the_ceiling_of_bytes_over_two() {
    assert_eq!(batch_output_tokens(10, 1), 5);
    assert_eq!(batch_output_tokens(11, 1), 6);
    assert_eq!(batch_output_tokens(0, 4), 0);
    assert_eq!(batch_output_tokens(10, 4), 20);
}

#[test]
fn batch_output_tokens_saturates() {
    assert_eq!(batch_output_tokens(u64::MAX, 16), u64::MAX);
    assert_eq!(batch_output_tokens(u64::MAX, 1), u64::MAX / 2 + 1);
}

#[test]
fn near_output_cap_fires_at_eighty_percent() {
    assert!(!near_output_cap(0));
    assert!(!near_output_cap(25_599));
    assert!(near_output_cap(25_600));
    assert!(near_output_cap(32_000));
    assert!(near_output_cap(u64::MAX));
}
